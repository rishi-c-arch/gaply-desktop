//! The AI Check flow (Set 5): TWO-WAY tiered detection — human-written vs
//! AI-associated — with the one-at-a-time model lifecycle.
//!
//! # Why two-way (the Set-4 live-probe decision)
//!
//! Set 4 built the full generated-vs-paraphrased classification path (seam,
//! gates, cap) and the live probe then showed qwen3:4b CANNOT make that
//! distinction usably: fast mode (think:false) blurs every passage into one
//! category; reasoning mode (think:true) separates them but at 5–14 MINUTES
//! per call and still forces a category onto clearly-human text. So the
//! shipped flow runs `classify_passages(None, …)` — the honest two-way
//! fallback that was built and tested for exactly this: every deep-verified
//! passage keeps its AI-associated signal, the paraphrase lane reports
//! "unavailable", and nothing is ever guessed. The Set-4 machinery stays
//! correct (schema-as-format fixed) for probes and for a future local model
//! that can actually make the distinction.
//!
//! # Stages (at most ONE model in memory at any point)
//!
//! 1. **Stage 1** — heuristic pre-pass over the whole document (seconds).
//! 2. **Stage 2** — the on-device deep verifier (candle, in-process) re-scores
//!    the top candidates within the budget (B2: non-dropping, no self-baseline;
//!    absolute norm placement is the separate `apply_verifier_norm` step). The
//!    model is scoped to its block and DROPS at its end.
//!
//! LOCAL-ONLY: AI Check is free and never touches the cloud proxy. All
//! honesty guarantees (signal labels, tiers, cautions, the deterministic %,
//! the Set-5 language downgrade) live in gaply-core.

use gaply_core::ai_detect::{
    self, ClassifiedAnalysis, HeuristicModel, COMPACT_MAX_DEEP_TOKENS,
    DEFAULT_MAX_CLASSIFIED_PASSAGES, DEFAULT_MAX_DEEP_PASSAGES, DEFAULT_MAX_DEEP_TOKENS,
};
use gaply_core::extract::ExtractionResult;
use serde::Serialize;

/// Progress/terminal events streamed to the frontend over the IPC `Channel`
/// during a run (the pipeline.rs precedent). `Report` is terminal-success (the
/// AiCheckResult returns on the command's promise); `Cancelled` is terminal-stop.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AiCheckEvent {
    /// Parsing + section extraction.
    Extract,
    /// The fast heuristic pre-pass.
    PrePass,
    /// The Stage-1 real-LM (0.5B) signal.
    Stage1Lm,
    /// The deep verifier — per-passage progress.
    DeepVerify { done: usize, total: usize },
    /// A model was skipped THIS RUN (honest live note — memory, or a gate).
    MemorySkip { model: String, reason: String },
    /// Terminal: analysis complete; the result returns on the promise.
    Report,
    /// Terminal: the user cancelled; nothing is shown (partial != valid signal).
    Cancelled { stage: String, done: usize, total: usize },
}

/// Pre-flight memory status for the AI Check page (re-checkable). TRUTHFUL about
/// the tier THIS machine can run — never implies freeing memory unlocks a model
/// the machine isn't entitled to (the structural `deep_tier` gate is honest).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AiCheckMemoryStatus {
    pub free_mb: u64,
    pub total_gb: f64,
    pub stage1_fits: bool,
    /// Whether the Stage-1 LM (the bundled 0.5B) is present on disk — so the
    /// "heuristic-only" DEEP tier isn't misreported as "no language model": with
    /// the 0.5B bundled, the pre-pass includes a real on-device LM reading.
    pub stage1_available: bool,
    pub deep_fits: bool,
    /// FREE memory the attainable deep model needs (its 1.5× courtesy guard), in
    /// MB. `None` when no deep tier applies (heuristic-only). Names the gap so a
    /// warning becomes an action.
    pub deep_need_mb: Option<u64>,
    /// Machine-matchable: "full_7b" | "compact_1_5b" | "heuristic_only".
    pub tier_attainable: String,
    pub tier_label: String,
    pub hint: String,
}

/// Pure builder (testable): maps free/total/tier into the honest status. `free`
/// None (non-macOS / query failure) → treated as "fits" (allow), mirroring
/// `enough_free_memory`.
pub fn memory_status(
    free: Option<u64>,
    total: Option<u64>,
    tier: crate::models::DeepTier,
    stage1_available: bool,
) -> AiCheckMemoryStatus {
    const MB: u64 = 1024 * 1024;
    const STAGE1: u64 = 400 * MB;
    const MINI: u64 = 1600 * MB;
    const FULL: u64 = 6 * 1024 * MB;
    // Mirror enough_free_memory: free >= 1.5x need (None free → allow).
    let fits = |need: u64| free.map(|f| f >= need.saturating_mul(3) / 2).unwrap_or(true);
    let (tier_attainable, tier_label, deep_need) = match tier {
        crate::models::DeepTier::Full7B => ("full_7b", "the full 7B deep verifier", Some(FULL)),
        crate::models::DeepTier::Mini => ("compact_1_5b", "the compact 1.5B deep verifier", Some(MINI)),
        crate::models::DeepTier::HeuristicOnly => ("heuristic_only", "the fast pre-pass only", None),
    };
    let stage1_fits = fits(STAGE1);
    let deep_fits = deep_need.map(fits).unwrap_or(false);
    // The FREE memory the deep model needs = 1.5× its resident guard (the
    // courtesy-check threshold). NAMES THE GAP so a warning becomes an action.
    let deep_need_mb = deep_need.map(|n| n.saturating_mul(3) / 2 / MB);
    // TRUTHFUL: keyed on the ATTAINABLE tier — on an 8GB machine `tier_label` is
    // "the compact 1.5B", so we never tell them freeing memory unlocks the 7B.
    let hint = match (deep_need, deep_fits) {
        // No deep model. Distinguish the Stage-1 LM (bundled 0.5B) being present —
        // the machine still runs a real language-model reading — from the truly
        // degraded case (no on-device model at all). Conflating them would tell a
        // post-bundle stranger they get the weak detector when they don't.
        (None, _) if stage1_available => "This device runs the fast pre-pass and the on-device Stage-1 language model. Deep verification by a larger model isn't available on this machine.".to_string(),
        (None, _) => "This device runs the fast pre-pass only; no on-device model is available.".to_string(),
        (Some(_), true) => format!("Ready: {tier_label} will run."),
        (Some(n), false) => {
            let need_gb = (n.saturating_mul(3) / 2) as f64 / 1_073_741_824.0;
            format!("Needs about {need_gb:.1} GB free to run; closing browsers usually frees the most.")
        }
    };
    AiCheckMemoryStatus {
        free_mb: free.map(|b| b / MB).unwrap_or(0),
        total_gb: total.map(|b| (b as f64 / 1_073_741_824.0 * 10.0).round() / 10.0).unwrap_or(0.0),
        stage1_fits,
        stage1_available,
        deep_fits,
        deep_need_mb,
        tier_attainable: tier_attainable.to_string(),
        tier_label: tier_label.to_string(),
        hint,
    }
}

/// Per-tier deep-token budget: the compact 1.5B is ~15x faster than the 7B on
/// the same hardware, so it gets the larger budget and still finishes fast;
/// every other case (the slow 7B, or no model) uses the tight default.
fn deep_budget_tokens(kind: ai_detect::DeepKind) -> usize {
    match kind {
        ai_detect::DeepKind::Compact => COMPACT_MAX_DEEP_TOKENS,
        _ => DEFAULT_MAX_DEEP_TOKENS,
    }
}

/// Map one `ReferenceVerification` (metadata-only) to a distilled verdict. PURE
/// (no network) so it's unit-testable. `exists=Some` = found; `exists=None` with
/// a "no match" warning = genuinely not found (fabricated); a network problem
/// (rate-limit/unavailable) = unchecked (never silently a "not found").
fn to_verdict(
    rv: &gaply_core::refverify::ReferenceVerification,
    reference: &gaply_core::extract::citations::Reference,
) -> gaply_core::ai_signals::CitationVerdict {
    use gaply_core::ai_signals::CitationVerdict;
    let retracted = rv.retraction.as_ref().map(|r| r.retracted).unwrap_or(false)
        || rv.exists.as_ref().and_then(|e| e.is_retracted_hint).unwrap_or(false);
    let norm = |d: &str| d.trim().to_lowercase();
    match &rv.exists {
        Some(e) => {
            let doi_mismatch = e.found
                && match (reference.doi.as_deref(), e.doi.as_deref()) {
                    (Some(a), Some(b)) => norm(a) != norm(b),
                    _ => false,
                };
            CitationVerdict { found: e.found, doi_mismatch, retracted, unchecked: false }
        }
        None => {
            let w = rv.warnings.join(" ").to_lowercase();
            let network_problem = w.is_empty()
                || w.contains("rate-limit")
                || w.contains("unavailable")
                || w.contains("timeout");
            if network_problem {
                CitationVerdict { unchecked: true, retracted, ..Default::default() }
            } else {
                // A definite "no match" from a reachable source → not found.
                CitationVerdict { found: false, unchecked: false, retracted, doi_mismatch: false }
            }
        }
    }
}

/// Pure lane decision — the network is a `verify_one` CLOSURE so tests can
/// assert exactly how many times it's invoked (0 on opt-out). Returns
/// `NotEnabled`/`NoReferences` WITHOUT ever calling `verify_one`; otherwise runs
/// it per reference and folds all-unchecked → `Offline`.
fn build_citation_lane<F>(
    verify_citations: bool,
    references: &[gaply_core::extract::citations::Reference],
    mut verify_one: F,
) -> gaply_core::ai_signals::CitationLane
where
    F: FnMut(&gaply_core::extract::citations::Reference) -> gaply_core::ai_signals::CitationVerdict,
{
    use gaply_core::ai_signals::CitationLane;
    if !verify_citations {
        return CitationLane::NotEnabled;
    }
    if references.is_empty() {
        return CitationLane::NoReferences;
    }
    let verdicts: Vec<_> = references.iter().map(&mut verify_one).collect();
    if verdicts.iter().all(|v| v.unchecked) {
        CitationLane::Offline
    } else {
        CitationLane::Verified(verdicts)
    }
}

/// The NETWORK citation-verification lane (Set C2). Merges its evidence into an
/// AI-Check result. Metadata-only (RefVerifier sends author/year/title/DOI,
/// never manuscript text), cache-first, rate-limited. `verify_citations` is the
/// AND of the user's opt-in and the global cloud gate (decided by the caller).
/// EVERY outcome yields an evidence row — no state is silent.
pub fn apply_citation_verification(
    analysis: &mut ClassifiedAnalysis,
    extraction: &ExtractionResult,
    db: &gaply_core::Database,
    now: i64,
    verify_citations: bool,
) {
    use gaply_core::ai_signals::{
        citation_verification_evidence, citation_verification_summary, CitationLane, CitationVerdict,
    };
    let lane = if verify_citations && !extraction.references.is_empty() {
        match crate::http_fetcher::RefVerifier::new() {
            Err(_) => CitationLane::Offline,
            Ok(verifier) => build_citation_lane(true, &extraction.references, |r| {
                match verifier.verify(db, r, now) {
                    Ok(rv) => to_verdict(&rv, r),
                    Err(_) => CitationVerdict { unchecked: true, ..Default::default() },
                }
            }),
        }
    } else {
        // Opted-out or no references: the verifier is NEVER constructed and the
        // closure is NEVER called — zero network by construction.
        build_citation_lane(verify_citations, &extraction.references, |_| CitationVerdict::default())
    };
    analysis
        .document_score
        .evidence
        .extend(citation_verification_evidence(&lane));
    analysis.document_features.citation_verification = Some(citation_verification_summary(&lane));
}

/// Run the full AI Check over an extraction. Never fails: an absent SLM-1
/// means honest heuristic-only labels — degraded tiers are data in the
/// result, not errors.
#[tracing::instrument(skip(extraction, emit, should_cancel), fields(sections = extraction.sections.len()))]
pub fn run_aicheck_flow(
    extraction: &ExtractionResult,
    emit: &dyn Fn(AiCheckEvent),
    should_cancel: &dyn Fn() -> bool,
) -> Result<ClassifiedAnalysis, ai_detect::Cancelled> {
    // Measured resident of the Stage-1 LM (Qwen2.5-0.5B Q4) ≈ 385MB → guard ~400MB.
    // (The DEEP tiers' guards live with the shared gate in `crate::models`, since
    // the pipeline's AI lane must apply the identical numbers.)
    const STAGE1_LM_RESIDENT_BYTES: u64 = 400 * 1024 * 1024;
    let mut stage1_skipped_for_memory = false;
    // Bundled norms (no model) — used by BOTH phases.
    let norms = gaply_core::stage1_norms::Stage1Norms::bundled();

    emit(AiCheckEvent::PrePass);

    // PHASE A — fast pre-pass + the Stage-1 LM (0.5B). The model is scoped to
    // THIS block and DROPS at the closing brace, so its ~1GB working set returns
    // to the OS BEFORE the deep verifier's memory check + load. The two candle
    // models NEVER coexist (true one-at-a-time; Set 1 made the phases model-
    // disjoint so this is enforceable).
    let stage1_analysis = {
        let fast = HeuristicModel::default();
        // RAM COURTESY CHECK (the Chrome lesson): skip the load if free memory
        // can't hold the working set, rather than swap-thrashing at 0.1 tok/s.
        let stage1_lm = if crate::models::stage1_lm_present()
            && !crate::models::enough_free_memory(STAGE1_LM_RESIDENT_BYTES)
        {
            stage1_skipped_for_memory = true;
            tracing::warn!("AI Check: Stage-1 LM skipped this run — insufficient free memory");
            emit(AiCheckEvent::MemorySkip {
                model: "Stage-1 language model".into(),
                reason: "insufficient free memory this run".into(),
            });
            None
        } else {
            crate::models::stage1_lm_model()
        };
        let stage1_id = crate::models::stage1_lm_model_id();
        let stage1_cfg = match (&stage1_lm, &stage1_id) {
            (Some(m), Some(id)) => match norms.for_model(id) {
                Some(norm) => {
                    tracing::info!(model_id = %id, "AI Check: Stage-1 LM signal enabled");
                    Some(ai_detect::Stage1Config { lm: m.as_ref(), norm, provisional: norms.provisional })
                }
                None => {
                    tracing::warn!(model_id = %id, "AI Check: Stage-1 LM present but no norm for it — proxy-only");
                    None
                }
            },
            _ => {
                tracing::warn!("AI Check: Stage-1 LM absent — proxy-only pre-pass (no real-LM signal)");
                None
            }
        };
        emit(AiCheckEvent::Stage1Lm);
        // Cancel is wired (per-candidate in phase A); progress rides the deep
        // loop below (the determinate bar). A cancelled phase A returns early.
        let hooks = ai_detect::RunHooks { progress: None, should_cancel: Some(should_cancel) };
        match ai_detect::analyze_stage1(&fast, stage1_cfg, extraction, hooks) {
            ai_detect::Cancellable::Completed(s) => s,
            ai_detect::Cancellable::Cancelled(c) => return Err(c),
        }
    }; // ← the 0.5B Stage-1 LM is DROPPED HERE (memory returned before phase B)

    // PHASE B — the deep verifier (1.5B/7B). Its RAM courtesy check now reads the
    // memory FREED by phase A's drop — a LOWER bar than when the 0.5B was still
    // resident. Model scoped to this block, dropped at the brace.
    let tiered = {
        // DEEP VERIFIER (Set E — un-idled): the DeepTier gate decides which model
        // the machine is entitled to; the RAM courtesy check guards the actual
        // load. Both now live in ONE place — `crate::models::select_deep_model`,
        // shared with the pipeline's AI lane so the two can't drift apart.
        // `deep_kind` carries the honest outcome (incl. the two low-memory
        // states) into the coverage note. `deep_norm` places the re-scored
        // passages against the deep model's OWN norm (mini only today).
        let crate::models::DeepSelection {
            model: deep_lm,
            kind: deep_kind,
            norm_id: deep_norm_id,
        } = crate::models::select_deep_model();
        let deep_norm = deep_norm_id.as_deref().and_then(|id| norms.for_model(id));

        // Honest LIVE note when the deep tier was skipped/gated this run.
        match deep_kind {
            ai_detect::DeepKind::SkippedLowMemory => emit(AiCheckEvent::MemorySkip {
                model: "deep verifier".into(),
                reason: "insufficient free memory this run".into(),
            }),
            ai_detect::DeepKind::GatedLowRam => emit(AiCheckEvent::MemorySkip {
                model: "7B deep verifier".into(),
                reason: "this machine has under ~16 GB of RAM".into(),
            }),
            _ => {}
        }

        // The determinate progress bar + per-passage cancel live here. Progress
        // fires ONLY when a deep model actually ran (no DeepVerify spam for the
        // instant heuristic-only pass when deep is None).
        let deep_progress = |done, total| emit(AiCheckEvent::DeepVerify { done, total });
        let progress: Option<&dyn Fn(usize, usize)> =
            if deep_lm.is_some() { Some(&deep_progress) } else { None };
        let hooks = ai_detect::RunHooks { progress, should_cancel: Some(should_cancel) };
        let mut tiered = match ai_detect::analyze_deep(
            stage1_analysis,
            deep_lm.as_deref(),
            DEFAULT_MAX_DEEP_PASSAGES,
            deep_budget_tokens(deep_kind),
            deep_kind,
            extraction,
            hooks,
        ) {
            ai_detect::Cancellable::Completed(t) => t,
            ai_detect::Cancellable::Cancelled(c) => return Err(c),
        };
        // Place the deep verifier's re-scored passages against its own norm WHILE
        // the model is still alive (before the block drops it). No-op unless a
        // norm resolved (the 7B has none yet → verifies without a placement row).
        if let (Some(dm), Some(n)) = (deep_lm.as_deref(), deep_norm) {
            ai_detect::apply_verifier_norm(&mut tiered, dm, n, deep_kind);
        }
        tiered
    }; // ← the deep verifier is DROPPED HERE

    // TWO-WAY collapse (probe decision): `None` is deliberate, even when
    // Ollama is running — no supported local model makes the
    // generated-vs-paraphrased distinction reliably, so it is never asked.
    // The result honestly reports the distinction as unavailable.
    let mut analysis = ai_detect::classify_passages(None, &tiered, DEFAULT_MAX_CLASSIFIED_PASSAGES);
    // The RAM-skip is surfaced as an honest Evidence row (never silent).
    if stage1_skipped_for_memory {
        analysis.document_score.evidence.push(gaply_core::ai_features::SignalEvidence::unavailable(
            "Language-model perplexity",
            gaply_core::ai_features::BiasTier::Stylometric,
            "skipped this run: insufficient free memory",
        ));
    }
    Ok(analysis)
}

#[cfg(test)]
mod tests {
    use gaply_core::ai_detect::{AnalysisDepth, PassageCategory};
    use gaply_core::extract::extract_from_text;

    use crate::models::ollama_verify::scripted::scripted_ollama;

    use super::*;

    /// Serializes the flow tests — they mutate process-global env (GAPLY_*).
    static FLOW_ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    // --- Set C2a: citation verification ---
    use gaply_core::extract::citations::Reference;
    use gaply_core::refverify::{ExistenceCheck, Provenance, ReferenceVerification, RetractionCheck};

    fn prov() -> Provenance {
        Provenance { source: "crossref".into(), url: "x".into(), fetched_at: 0, checksum: "".into(), from_cache: false }
    }
    fn rv(exists: Option<ExistenceCheck>, retraction: Option<RetractionCheck>, warnings: Vec<&str>) -> ReferenceVerification {
        ReferenceVerification {
            reference_raw: "ref".into(),
            exists,
            retraction,
            open_access: None,
            enrichment: None,
            provenance: vec![],
            warnings: warnings.into_iter().map(|w| w.to_string()).collect(),
        }
    }
    fn found_check(doi: Option<&str>) -> ExistenceCheck {
        ExistenceCheck {
            source: "crossref", found: true, doi: doi.map(|d| d.to_string()),
            title: None, matched_authors: None, matched_year: None, is_retracted_hint: None, provenance: prov(),
        }
    }
    fn refr(doi: Option<&str>) -> Reference {
        Reference { raw: "r".into(), authors: "".into(), year: None, title: None, doi: doi.map(|d| d.to_string()) }
    }

    #[test]
    fn to_verdict_maps_found_notfound_ratelimited_retracted_and_chimera() {
        // found, DOIs agree → clean
        let v = super::to_verdict(&rv(Some(found_check(Some("10.1/x"))), None, vec![]), &refr(Some("10.1/x")));
        assert!(v.found && !v.doi_mismatch && !v.unchecked);
        // found, DOIs DISAGREE → chimera
        let v = super::to_verdict(&rv(Some(found_check(Some("10.9/other"))), None, vec![]), &refr(Some("10.1/x")));
        assert!(v.found && v.doi_mismatch, "wrong-DOI chimera");
        // exists=None + "no match" → genuinely not found (fabricated), CHECKED
        let v = super::to_verdict(&rv(None, None, vec!["crossref: no match"]), &refr(None));
        assert!(!v.found && !v.unchecked, "not found = checked-and-absent");
        // exists=None + rate-limited → UNCHECKED (never silently 'not found')
        let v = super::to_verdict(&rv(None, None, vec!["crossref: rate-limited, retry after 5s"]), &refr(None));
        assert!(v.unchecked, "network problem = unchecked, not fabricated");
        // retracted flows through
        let v = super::to_verdict(&rv(Some(found_check(None)), Some(RetractionCheck { retracted: true, reasons: vec![], notice_url: None, provenance: prov() }), vec![]), &refr(None));
        assert!(v.retracted);
    }

    /// DIRECT zero-network pin: a call-counting closure proves the verifier is
    /// invoked EXACTLY 0 times when opted-out (and when there are no references),
    /// and once per reference otherwise. This is the "mock counting 0 calls".
    #[test]
    fn opt_out_invokes_the_verifier_zero_times() {
        use gaply_core::ai_signals::{CitationLane, CitationVerdict};
        let refs = vec![refr(Some("10.1/x")), refr(Some("10.2/y"))];

        // OPT-OUT → NotEnabled, verify_one NEVER called.
        let mut calls = 0usize;
        let lane = build_citation_lane(false, &refs, |_| { calls += 1; CitationVerdict::default() });
        assert_eq!(lane, CitationLane::NotEnabled);
        assert_eq!(calls, 0, "opt-out does ZERO network work");

        // No references → NoReferences, still 0 calls.
        calls = 0;
        let lane = build_citation_lane(true, &[], |_| { calls += 1; CitationVerdict::default() });
        assert_eq!(lane, CitationLane::NoReferences);
        assert_eq!(calls, 0);

        // Enabled + references → verify_one called once per reference.
        calls = 0;
        let lane = build_citation_lane(true, &refs, |_| { calls += 1; CitationVerdict { found: true, ..Default::default() } });
        assert_eq!(calls, 2, "one lookup per reference");
        assert!(matches!(lane, CitationLane::Verified(_)));

        // All unchecked (network down) → Offline.
        let lane = build_citation_lane(true, &refs, |_| CitationVerdict { unchecked: true, ..Default::default() });
        assert_eq!(lane, CitationLane::Offline);
    }

    /// The full command wiring still reports the opt-out summary honestly.
    #[test]
    fn apply_citation_verification_opt_out_summary_is_not_enabled() {
        let db = gaply_core::Database::in_memory().unwrap();
        let ex = extract_from_text("Introduction\n\nThe results are clear (Smith, 2020).\n\nReferences\n\nSmith, J. (2020). A study. doi:10.1/x\n");
        let tiered = ai_detect::analyze_tiered(
            &HeuristicModel::default(), None, &ex, DEFAULT_MAX_DEEP_PASSAGES, DEFAULT_MAX_DEEP_TOKENS,
            ai_detect::DeepKind::Absent, None,
        );
        let mut analysis = ai_detect::classify_passages(None, &tiered, DEFAULT_MAX_CLASSIFIED_PASSAGES);
        apply_citation_verification(&mut analysis, &ex, &db, 0, false);
        assert_eq!(analysis.document_features.citation_verification.as_ref().unwrap().status, "not_enabled");
        assert!(analysis.document_score.evidence.iter().any(|e| e.signal == "Citation verification" && e.detail == "not enabled"));
    }

    // Common-word AI-like prose the heuristic pre-pass reliably flags.
    const AI_SENT: &str = "The results show that the model can do the work well.";
    const HUMAN_SENT: &str =
        "Cerulean contraptions wheezed, magnificently preposterous, unfathomable.";

    /// The two-way collapse holds with AND without a live (scripted) Ollama —
    /// both scenarios in ONE test because they mutate process-wide env vars.
    /// The deep tier is DISABLED throughout (GAPLY_DISABLE_DEEP): no candle load
    /// of EITHER the 7B or the compact mini (which may be installed on the dev
    /// machine), no real network — fully deterministic, heuristic-only.
    #[test]
    fn flow_is_two_way_and_honest_with_or_without_ollama() {
        let _env = FLOW_ENV_LOCK.lock().unwrap();
        std::env::set_var("GAPLY_DISABLE_DEEP", "1");
        std::env::set_var("GAPLY_SLM1_GGUF", "/nonexistent/gaply-aicheck-test.gguf");
        // Stage-1 LM absent → no candle load of the 0.5B (installed on dev machines).
        std::env::set_var("GAPLY_STAGE1_LM_GGUF", "/nonexistent/stage1.gguf");
        let ex = extract_from_text(&format!(
            "Introduction\n\n{h} {a} {a} {a} {h}\n",
            a = AI_SENT,
            h = HUMAN_SENT
        ));

        // --- Scenario 1: a live (scripted) Ollama must make NO difference —
        // the probe decision is that the classifier is never consulted.
        std::env::set_var("GAPLY_SLM2_ENDPOINT", scripted_ollama("{}"));
        let out = run_aicheck_flow(&ex, &|_| {}, &|| false).expect("no cancel");
        assert!(out.deep_model.is_none(), "SLM-1 absent must be reported as None");
        assert!(!out.passages.is_empty(), "the AI-like run should flag");
        assert!(out
            .passages
            .iter()
            .all(|p| p.tiered.depth == AnalysisDepth::HeuristicOnly));
        assert!(
            out.classifier_model.is_none(),
            "TWO-WAY collapse: the classifier is never consulted, even with Ollama live"
        );
        assert!(out
            .passages
            .iter()
            .all(|p| p.category == PassageCategory::Unclassified));
        assert_eq!(out.classified, 0);
        assert!(out.classification_note.contains("UNAVAILABLE"));
        assert!(out.classification_note.contains("nothing was guessed"));
        assert!(out.classification_note.contains("two-way"));
        assert!(out.coverage_note.contains("heuristic-only"));

        // --- Scenario 2: Ollama absent — identical two-way result shape.
        let dead = {
            let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
            format!("http://{}", l.local_addr().unwrap())
        };
        std::env::set_var("GAPLY_SLM2_ENDPOINT", dead);
        let out2 = run_aicheck_flow(&ex, &|_| {}, &|| false).expect("no cancel");
        assert!(out2.classifier_model.is_none());
        assert_eq!(out2.classified, 0);
        assert!(out2.classification_note.contains("UNAVAILABLE"));

        // The un-strippable cautions + Set-5 language assessment ride always.
        for out in [&out, &out2] {
            assert!(!out.disclaimer.is_empty());
            assert!(!out.paraphrase_caution.is_empty());
            assert_eq!(out.language.detected, "english");
            assert!(out.language.calibration_reliable);
            assert!(!out.language.note.is_empty(), "language note is REQUIRED");
            for p in &out.passages {
                assert!(!p.category_note.is_empty());
                assert!(!p.tiered.depth_note.is_empty());
                assert!(!p.tiered.passage.uncertainty.is_empty());
            }
        }
    }

    /// Per-tier budgets: the compact tier gets 1500 tokens, everything else 700.
    #[test]
    fn per_tier_deep_budget_is_1500_for_compact_700_otherwise() {
        use gaply_core::ai_detect::DeepKind;
        assert_eq!(deep_budget_tokens(DeepKind::Compact), 1500);
        assert_eq!(deep_budget_tokens(DeepKind::Full), 700);
        assert_eq!(deep_budget_tokens(DeepKind::GatedLowRam), 700);
        assert_eq!(deep_budget_tokens(DeepKind::Absent), 700);
    }

    /// Non-English input downgrades the whole report, un-strippably.
    #[test]
    fn non_english_document_is_downgraded() {
        let _env = FLOW_ENV_LOCK.lock().unwrap();
        // Disable the deep tier + Stage-1 LM so no installed model loads candle here.
        std::env::set_var("GAPLY_DISABLE_DEEP", "1");
        std::env::set_var("GAPLY_SLM1_GGUF", "/nonexistent/gaply-aicheck-test.gguf");
        std::env::set_var("GAPLY_STAGE1_LM_GGUF", "/nonexistent/stage1.gguf");
        let ex = extract_from_text(
            "Introducción\n\nLos resultados de este estudio muestran que el método es eficaz y \
             los datos son consistentes con la interpretación de los hallazgos en la muestra.\n",
        );
        let out = run_aicheck_flow(&ex, &|_| {}, &|| false).expect("no cancel");
        assert_eq!(out.language.detected, "spanish");
        assert!(!out.language.calibration_reliable);
        assert!(out.language.note.contains("LOW-CONFIDENCE"));
    }

    // --- Set 2: events, cancel/reset, memory status ---

    fn hermetic_env() {
        // No candle load of either model.
        std::env::set_var("GAPLY_DISABLE_DEEP", "1");
        std::env::set_var("GAPLY_SLM1_GGUF", "/nonexistent/x.gguf");
        std::env::set_var("GAPLY_SLM1_MINI_GGUF", "/nonexistent/x.gguf");
        std::env::set_var("GAPLY_STAGE1_LM_GGUF", "/nonexistent/x.gguf");
    }

    #[test]
    fn flow_emits_events_in_order() {
        let _env = FLOW_ENV_LOCK.lock().unwrap();
        hermetic_env();
        let ex = extract_from_text("Introduction\n\nThe results show the method works well here.\n");
        let seen = std::cell::RefCell::new(Vec::<AiCheckEvent>::new());
        let emit = |ev: AiCheckEvent| seen.borrow_mut().push(ev);
        let _ = run_aicheck_flow(&ex, &emit, &|| false).expect("no cancel");
        // Hermetic (no models): PrePass then Stage1Lm; no DeepVerify (no model),
        // no MemorySkip (models ABSENT, not skipped-for-memory). Extract/Report
        // are emitted by the command, not the flow.
        assert_eq!(seen.into_inner(), vec![AiCheckEvent::PrePass, AiCheckEvent::Stage1Lm]);
    }

    #[test]
    fn cancel_token_flips_and_resets_across_runs() {
        use std::sync::atomic::{AtomicBool, Ordering};
        use std::sync::Arc;
        let _env = FLOW_ENV_LOCK.lock().unwrap();
        hermetic_env();
        // An AI-like doc so the pre-pass yields candidates (the cancel checkpoint).
        let ex = extract_from_text(
            "Introduction\n\nThe results show that the model can do the work well. \
             The system is fast. The user can see the way to go now.\n",
        );
        let token = Arc::new(AtomicBool::new(false));
        let sc = {
            let t = token.clone();
            move || t.load(Ordering::SeqCst)
        };

        // RUN 1 — cancel_aicheck flips the token → the flow yields Cancelled, NOT
        // a partial result.
        token.store(true, Ordering::SeqCst);
        let r1 = run_aicheck_flow(&ex, &|_| {}, &sc);
        assert!(matches!(r1, Err(_)), "flipped token → Cancelled (no partial result)");

        // RUN 2 — run_aicheck resets the token first → the next run is NOT poisoned.
        token.store(false, Ordering::SeqCst);
        let r2 = run_aicheck_flow(&ex, &|_| {}, &sc);
        assert!(r2.is_ok(), "reset token → the next run completes normally");
    }

    #[test]
    fn memory_status_is_truthful_about_the_attainable_tier() {
        use crate::models::DeepTier;
        let gb = 1024 * 1024 * 1024u64;

        // 8GB machine, tight memory, mini attainable → tier_label names the
        // COMPACT 1.5B (never the 7B); the hint NAMES THE GAP (~2.4 GB).
        let s = memory_status(Some(gb), Some(8 * gb), DeepTier::Mini, true);
        assert_eq!(s.tier_attainable, "compact_1_5b");
        assert!(s.tier_label.contains("compact 1.5B"));
        assert!(!s.deep_fits, "1GB free < ~2.4GB needed for the mini");
        assert_eq!(s.deep_need_mb, Some(2400), "1.5x the 1.6GB mini guard = 2400 MiB free");
        assert!(s.hint.contains("2.3 GB"), "hint names the gap (2400 MiB ≈ 2.3 GiB): {}", s.hint);
        assert!(!s.hint.contains("7B"), "must NOT dangle the 7B on an 8GB machine: {}", s.hint);
        assert!(s.hint.contains("closing browsers"));

        // Ample memory → deep fits, "Ready".
        let s = memory_status(Some(4 * gb), Some(8 * gb), DeepTier::Mini, true);
        assert!(s.deep_fits && s.hint.starts_with("Ready"), "hint: {}", s.hint);

        // 16GB machine → the 7B is genuinely attainable (named in tier_label);
        // the gap is ~9 GB (1.5x the 6GB guard).
        let s = memory_status(Some(2 * gb), Some(16 * gb), DeepTier::Full7B, true);
        assert_eq!(s.tier_attainable, "full_7b");
        assert!(s.tier_label.contains("7B"));
        assert_eq!(s.deep_need_mb, Some(9216));

        // No deep model, but the bundled 0.5B Stage-1 LM IS present → the hint must
        // say a real language model runs, NOT conflate it with the weak detector.
        let s = memory_status(Some(gb), Some(8 * gb), DeepTier::HeuristicOnly, true);
        assert_eq!(s.tier_attainable, "heuristic_only");
        assert!(s.stage1_available);
        assert!(s.hint.contains("Stage-1 language model"), "Stage-1 present → says so: {}", s.hint);
        assert!(!s.hint.contains("no on-device model"), "not the degraded message: {}", s.hint);
        assert!(!s.hint.contains("Free up memory"));

        // Truly no on-device model (not even Stage-1) → the honest degraded message.
        let s = memory_status(Some(gb), Some(8 * gb), DeepTier::HeuristicOnly, false);
        assert!(!s.stage1_available);
        assert!(s.hint.contains("fast pre-pass only"), "hint: {}", s.hint);
        assert!(s.hint.contains("no on-device model"), "hint: {}", s.hint);
    }
}
