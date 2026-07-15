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
//! 2. **Stage 2** — SLM-1 (candle, in-process) deep re-scores the top
//!    candidates within the Set-3 budget, self-calibrated against the
//!    document's own baseline. SLM-1 is scoped to this block and DROPS at its
//!    end. No other model is ever loaded by this flow.
//!
//! LOCAL-ONLY: AI Check is free and never touches the cloud proxy. All
//! honesty guarantees (signal labels, tiers, cautions, the deterministic %,
//! the Set-5 language downgrade) live in gaply-core.

use gaply_core::ai_detect::{
    self, ClassifiedAnalysis, HeuristicModel, PerplexityModel, COMPACT_MAX_DEEP_TOKENS,
    DEFAULT_MAX_CLASSIFIED_PASSAGES, DEFAULT_MAX_DEEP_PASSAGES, DEFAULT_MAX_DEEP_TOKENS,
};
use gaply_core::extract::ExtractionResult;

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
#[tracing::instrument(skip(extraction), fields(sections = extraction.sections.len()))]
pub fn run_aicheck_flow(extraction: &ExtractionResult) -> ClassifiedAnalysis {
    // Measured resident of the Stage-1 LM (Qwen2.5-0.5B Q4) ≈ 385MB → guard ~400MB.
    const STAGE1_LM_RESIDENT_BYTES: u64 = 400 * 1024 * 1024;
    // Compact 1.5B deep verifier (Q4 GGUF 986MB + candle repack cache/activations)
    // ≈ 1.5GB working set → guard 1.6GB (the courtesy check then asks ~2.4GB free).
    const MINI_RESIDENT_BYTES: u64 = 1600 * 1024 * 1024;
    // Full 7B: a conservative TRANSIENT guard on top of the structural ≥16GB
    // total-RAM gate (deep_tier) — ~6GB → the courtesy check asks ~9GB free.
    const FULL_7B_RESIDENT_BYTES: u64 = 6 * 1024 * 1024 * 1024;
    let mut stage1_skipped_for_memory = false;
    // Stage 1+2 — BOTH candle models (the Stage-1 LM and the deep verifier) are
    // scoped to this block: their memory is RELEASED at the closing brace.
    let tiered = {
        let fast = HeuristicModel::default();
        // STAGE-1 real-LM signal (root-cause fix): load the small on-device LM
        // (~0.5B) and resolve its ABSOLUTE per-model human-academic norm.
        //
        // The RAM COURTESY CHECK (the Chrome lesson) guards EACH candle load: if
        // free+reclaimable memory can't hold the working set, skip that model
        // THIS RUN with an honest note rather than swap-thrashing at 0.1 tok/s.
        // The Stage-1 LM is loaded first, so the free-memory reading for the deep
        // verifier already accounts for it (natural sequential budgeting).
        let stage1_lm = if crate::models::stage1_lm_present()
            && !crate::models::enough_free_memory(STAGE1_LM_RESIDENT_BYTES)
        {
            stage1_skipped_for_memory = true;
            tracing::warn!("AI Check: Stage-1 LM skipped this run — insufficient free memory");
            None
        } else {
            crate::models::stage1_lm_model()
        };
        let stage1_id = crate::models::stage1_lm_model_id();
        let norms = gaply_core::stage1_norms::Stage1Norms::bundled();
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

        // DEEP VERIFIER (Set E — un-idled): the DeepTier gate (unchanged) decides
        // which model the machine is entitled to; the RAM courtesy check guards
        // the actual load. `deep_kind` carries the honest outcome (incl. the two
        // low-memory states) into the coverage note. `deep_norm` places the
        // re-scored passages against the deep model's OWN norm (mini only today).
        let (deep_lm, deep_kind, deep_norm_id): (
            Option<Box<dyn PerplexityModel>>,
            ai_detect::DeepKind,
            Option<String>,
        ) = match crate::models::deep_tier_from_env() {
            crate::models::DeepTier::Full7B => {
                if crate::models::enough_free_memory(FULL_7B_RESIDENT_BYTES) {
                    match crate::models::slm1_model() {
                        Some(m) => (Some(m), ai_detect::DeepKind::Full, crate::models::slm1_model_id()),
                        None => (None, ai_detect::DeepKind::Absent, None),
                    }
                } else {
                    tracing::warn!("AI Check: 7B deep verifier skipped this run — low free memory");
                    (None, ai_detect::DeepKind::SkippedLowMemory, None)
                }
            }
            crate::models::DeepTier::Mini => {
                if crate::models::enough_free_memory(MINI_RESIDENT_BYTES) {
                    match crate::models::slm1_mini_model() {
                        Some(m) => (Some(m), ai_detect::DeepKind::Compact, crate::models::slm1_mini_model_id()),
                        None => (None, ai_detect::DeepKind::Absent, None),
                    }
                } else {
                    tracing::warn!("AI Check: compact deep verifier skipped this run — low free memory");
                    (None, ai_detect::DeepKind::SkippedLowMemory, None)
                }
            }
            crate::models::DeepTier::HeuristicOnly => {
                // A present-but-RAM-gated 7B (machine below the ~16GB floor with
                // no compact fallback) reads as GatedLowRam; anything else Absent.
                let kind = if crate::models::slm1_present() && !crate::models::slm1_mini_present() {
                    ai_detect::DeepKind::GatedLowRam
                } else {
                    ai_detect::DeepKind::Absent
                };
                (None, kind, None)
            }
        };
        let deep_norm = deep_norm_id.as_deref().and_then(|id| norms.for_model(id));

        let mut tiered = ai_detect::analyze_tiered(
            &fast,
            deep_lm.as_deref(),
            extraction,
            DEFAULT_MAX_DEEP_PASSAGES,
            deep_budget_tokens(deep_kind),
            deep_kind,
            stage1_cfg,
        );
        // Place the deep verifier's re-scored passages against its own norm WHILE
        // the model is still alive (before the block drops it). No-op unless a
        // norm resolved (the 7B has none yet → verifies without a placement row).
        if let (Some(dm), Some(n)) = (deep_lm.as_deref(), deep_norm) {
            ai_detect::apply_verifier_norm(&mut tiered, dm, n, deep_kind);
        }
        tiered
    }; // ← both candle models are dropped HERE

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
    analysis
}

#[cfg(test)]
mod tests {
    use gaply_core::ai_detect::{AnalysisDepth, PassageCategory};
    use gaply_core::extract::extract_from_text;

    use crate::models::ollama_verify::scripted::scripted_ollama;

    use super::*;

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
        let out = run_aicheck_flow(&ex);
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
        let out2 = run_aicheck_flow(&ex);
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
        // Disable the deep tier + Stage-1 LM so no installed model loads candle here.
        std::env::set_var("GAPLY_DISABLE_DEEP", "1");
        std::env::set_var("GAPLY_SLM1_GGUF", "/nonexistent/gaply-aicheck-test.gguf");
        std::env::set_var("GAPLY_STAGE1_LM_GGUF", "/nonexistent/stage1.gguf");
        let ex = extract_from_text(
            "Introducción\n\nLos resultados de este estudio muestran que el método es eficaz y \
             los datos son consistentes con la interpretación de los hallazgos en la muestra.\n",
        );
        let out = run_aicheck_flow(&ex);
        assert_eq!(out.language.detected, "spanish");
        assert!(!out.language.calibration_reliable);
        assert!(out.language.note.contains("LOW-CONFIDENCE"));
    }
}
