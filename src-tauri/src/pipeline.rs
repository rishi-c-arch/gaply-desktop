//! Phase 3, Stage 2 — the ONE full-analysis command.
//!
//! `run_full_analysis` chains the real local agents exactly as the golden
//! `report/tests.rs` does, but as a PRODUCTION caller: extract → validate → ai
//! → plagiarism → rag → verify → debate → compile. Per-stage progress streams
//! to the frontend over a typed `tauri::ipc::Channel<AnalysisEvent>`; the
//! compiled report is cached under `report:{id}` for the report viewer (Stage 4).
//!
//! Honesty:
//! * The five local agents (extraction, validation, ai, plagiarism, rag) run
//!   for real (interim HeuristicModel/HashEmbedder stay until the SLM lands).
//! * The verify stage does REAL refverify HTTP (CrossRef/OpenAlex/… via the
//!   Stage-1 ReqwestFetcher) for reference existence/retraction. The cloud
//!   hallucination verdict goes through `verify_citations` as a real caller,
//!   but its `ProxyClient` is the mock until you deploy the proxy — so those
//!   verdicts come back UNKNOWN rather than fabricated.
//! * On ANY stage error we emit `Failed` (surfaced as a toast) and stop — never
//!   a fabricated completion.
//!
//! Threading: the whole pipeline is synchronous CPU + blocking IO, so it runs
//! inside one `tokio::task::spawn_blocking`. Nothing holds a lock across an
//! `.await`, so the std-guard-not-Send hazard never arises (a
//! `tokio::sync::Mutex` would be the tool if it did).

use std::sync::Arc;

use serde::Serialize;
use tauri::ipc::Channel;
use tauri::State;

use gaply_core::embed::Embedder;
use gaply_core::extract::citations::Reference;
use gaply_core::refverify::ReferenceVerification;
use gaply_core::reviewer_agent::LaneExamination;
use gaply_core::report::{build_checklist, compile_report};
use gaply_core::swarm::{adapters, run_debate, DebateConfig, PrecomputedAgent, SwarmAgent};
use gaply_core::verify_agent::{verify_citations, MockProxyClient};
use gaply_core::{ai_detect, extract, now_epoch, plagiarism, rag, validate, Database, GaplyError};

use crate::http_fetcher::RefVerifier;
use crate::state::AppState;

/// Report cache TTL: 30 days. The viewer reads it back by `report_id`.
const REPORT_TTL_SECS: i64 = 30 * 24 * 3600;

/// Cache key for a compiled report — ONE definition, so writer and readers
/// cannot drift apart.
///
/// **`v2` retires every report compiled before the PDF paragraph reflow**
/// (`docparse::reflow_pdf_text`). That fix changes `Location.paragraph` values
/// and the parsed reference count, both of which are user-visible. With a
/// 30-day TTL and an unversioned key, the same manuscript could be served a
/// pre-fix and a post-fix report for a month — two different answers, one of
/// them wrong, with nothing to tell them apart. Bumping the key makes stale
/// entries unreachable instead.
///
/// # `EVIDENCE_SCHEMA_VERSION` is part of the key
///
/// A cached report carries a serialized `Vec<EvidenceRecord>` under `evidence`,
/// and `escalation.rs:72` deserializes it with `unwrap_or_default()`. **A cached
/// report written by an older binary whose `EvidenceRecord` shape has since
/// changed therefore deserializes to an EMPTY vector**, the early return fires,
/// the aggregator sees zero findings, and the run yields `Accept` at 0.92 — a
/// silent wrong answer with no error and no log line (ARCHITECTURE_TRACE
/// §25.10). Cache entries survive rebuilds, so an upgrade is exactly when this
/// fires.
///
/// Including the version makes a stale entry MISS rather than mis-parse: the
/// report is recomputed from the manuscript, which is always correct and merely
/// slower. This closes the demonstrable path; it does not close the class — a
/// genuinely malformed record still deserializes to empty.
///
/// **RELEASE CONSTRAINT: any schema evolution that affects cached-report
/// compatibility must require an `EVIDENCE_SCHEMA_VERSION` bump.** Compatibility,
/// not modification — an optional field with a serde default leaves cached
/// reports readable and needs no bump. Requiring one for every change would
/// train reflexive bumping, which is how versions stop meaning anything.
pub(crate) fn report_cache_key(report_id: &str) -> String {
    format!("report:v2:e{}:{report_id}", gaply_core::evidence::EVIDENCE_SCHEMA_VERSION)
}
/// The six agent lanes the frontend renders. Debate + compile happen after,
/// under the "synthesis" pseudo-stage.
const LANE_TOTAL: usize = 6;

/// Below this the stylometry signals cannot be computed — MTLD needs ~100
/// tokens and the sentence-length CV needs several sentences, so no ELIGIBLE
/// AI-detection finding is possible and the lane examined nothing.
const MIN_STYLOMETRY_WORDS: usize = 100;

/// Current Gregorian year from the epoch clock, for `compile_report`'s
/// reference-recency count. Uses the mean Gregorian year (365.2425 days) rather
/// than a calendar library — gaply-core pulls in no date crate, and the only
/// consumer is a "how many references are older than N years" COUNT, where being
/// a day off at a New Year boundary changes nothing.
fn current_year() -> i32 {
    const SECS_PER_MEAN_YEAR: i64 = 31_556_952; // 365.2425 * 86_400
    (1970 + now_epoch() / SECS_PER_MEAN_YEAR) as i32
}

/// Semantic query used to retrieve ingested journal guidelines for the
/// checklist. Broad enough to surface the common requirement families
/// (length, abstract, disclosures, reference style).
const GUIDELINE_QUERY: &str =
    "author guidelines submission requirements word limit structured abstract \
     conflict of interest declaration reference style";

/// Typed progress events streamed to the webview over an IPC Channel.
/// `type` is the discriminant the frontend matches on.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum AnalysisEvent {
    StageStarted { stage: String, index: usize, total: usize },
    StageProgress { stage: String, pct: u8 },
    /// A parsed section, streamed so the UI's outline populates as extraction
    /// yields headings.
    Section { stage: String, index: usize, total: usize, title: String },
    StageCompleted { stage: String, summary: String },
    Finished { report_id: String },
    Failed { stage: String, message: String },
}

/// The async command. Extracts `Arc` handles from managed state (cheap, Send)
/// then runs the sync pipeline off the async runtime via spawn_blocking.
#[tauri::command]
#[tracing::instrument(skip(state, on_event))]
pub async fn run_full_analysis(
    state: State<'_, AppState>,
    path: String,
    title: Option<String>,
    on_event: Channel<AnalysisEvent>,
) -> Result<(), GaplyError> {
    let db = state.db.clone();
    let embedder = state.embedder.clone();
    // `user_token: None` — this command has no signed-in-user parameter, so the
    // cloud verification tier stays unauthenticated here and degrades to local
    // Ollama/mock exactly as before. PublishReady (`run_publishready`) is the
    // path that has a token and threads it. Giving this command a token needs a
    // frontend change (`src/screens/analysis/bridge.ts` invokes it without one)
    // and is deliberately out of scope for this fix.
    // guidelines_url: None — run_full_analysis is the general analysis command and
    // has no journal-guidelines input; the checklist stays structural-only, which
    // is the honest empty case. PublishReady is the path that carries one.
    tokio::task::spawn_blocking(move || run_pipeline(db, embedder, path, title, None, None, on_event))
        .await
        .map_err(|e| GaplyError::Internal(format!("analysis task panicked: {e}")))?
}

/// Run a lane: emit StageStarted, execute `f`, emit StageCompleted or Failed.
/// On failure the error propagates so the whole pipeline stops.
fn lane<T>(
    emit: &dyn Fn(AnalysisEvent),
    stage: &str,
    index: usize,
    f: impl FnOnce() -> Result<(T, String), GaplyError>,
) -> Result<T, GaplyError> {
    emit(AnalysisEvent::StageStarted { stage: stage.into(), index, total: LANE_TOTAL });
    match f() {
        Ok((value, summary)) => {
            emit(AnalysisEvent::StageCompleted { stage: stage.into(), summary });
            Ok(value)
        }
        Err(e) => {
            emit(AnalysisEvent::Failed { stage: stage.into(), message: e.to_string() });
            Err(e)
        }
    }
}

/// Channel wrapper: forward emitted events to the IPC channel (closed channel
/// ignored — the frontend navigated away).
fn run_pipeline(
    db: Arc<Database>,
    embedder: Arc<dyn Embedder>,
    path: String,
    title: Option<String>,
    user_token: Option<String>,
    guidelines_url: Option<String>,
    ch: Channel<AnalysisEvent>,
) -> Result<(), GaplyError> {
    // The lane state is for the PublishReady verdict path; the general analysis
    // command has no aggregator to feed, so it is discarded here deliberately.
    run_pipeline_inner(db, embedder, path, title, user_token, guidelines_url, &|ev| {
        let _ = ch.send(ev);
    })
    .map(|_lanes| ())
}

/// Measurement-only entry point (Set 4 8GB memory proof): drives the REAL
/// pipeline with a caller-supplied event sink so an example can instrument
/// per-stage memory. Not used by the app; `#[doc(hidden)]`, no behavior change.
#[doc(hidden)]
pub fn run_pipeline_measured(
    db: Arc<Database>,
    embedder: Arc<dyn Embedder>,
    path: String,
    title: Option<String>,
    user_token: Option<String>,
    guidelines_url: Option<String>,
    emit: &dyn Fn(AnalysisEvent),
) -> Result<LaneExamination, GaplyError> {
    run_pipeline_inner(db, embedder, path, title, user_token, guidelines_url, emit)
}

/// The synchronous pipeline. Every stage is a real production call. `emit` is
/// abstracted (Fn) so tests can drive the full pipeline with a collector.
///
/// `user_token` is the signed-in user's JWT for the cloud verification tier's
/// entitlement gate. `None` = unauthenticated: that tier is skipped or refused
/// and the lane degrades to local Ollama/mock (honest UNKNOWN verdicts), never a
/// failure.
fn run_pipeline_inner(
    db: Arc<Database>,
    embedder: Arc<dyn Embedder>,
    path: String,
    title: Option<String>,
    user_token: Option<String>,
    // The guideline document the user asked for. Threaded from the frontend so
    // identity is PROPAGATED, not reconstructed from corpus state.
    guidelines_url: Option<String>,
    emit: &dyn Fn(AnalysisEvent),
) -> Result<LaneExamination, GaplyError> {
    // Parse once, up front (part of the extraction lane's work).
    let text = lane(emit, "extraction", 1, || {
        let text = extract::docparse::parse_path(std::path::Path::new(&path))?;
        Ok((text, String::new()))
    })?;

    // 1) Extraction — real parse → sections/claims/citations, persisted.
    let extraction = {
        let extraction = extract::extract_from_text(&text);
        let manuscript_title = title
            .clone()
            .or_else(|| extraction.title.clone())
            .unwrap_or_else(|| "Untitled manuscript".to_string());
        let manuscript_id = db.create_manuscript(&manuscript_title, "", "")?;
        extract::persist::store_extraction(&db, manuscript_id, &extraction)?;
        // Stream section headings so the outline panel populates.
        let total_sections = extraction.sections.len();
        for (i, s) in extraction.sections.iter().enumerate() {
            let title = if s.heading.is_empty() { format!("{:?}", s.kind) } else { s.heading.clone() };
            emit(AnalysisEvent::Section {
                stage: "extraction".into(),
                index: i + 1,
                total: total_sections,
                title,
            });
        }
        emit(
            AnalysisEvent::StageCompleted {
                stage: "extraction".into(),
                summary: format!(
                    "{} section(s), {} statistical claim(s), {} reference(s)",
                    extraction.sections.len(),
                    extraction.statistics.len(),
                    extraction.references.len()
                ),
            },
        );
        (extraction, manuscript_id)
    };
    let (extraction, manuscript_id) = extraction;

    // 2) Validation — deterministic 5-rule validator (hard constraint).
    let validation = lane(emit, "validation", 2, || {
        let report = validate::validate(&extraction);
        let summary = if report.passed {
            "no statistical rule violations".to_string()
        } else {
            format!("{} rule violation(s)", report.flags.len())
        };
        Ok((report, summary))
    })?;

    // 3) AI check — real candle perplexity/burstiness at whichever tier THIS
    // machine may safely run, decided by the SHARED memory-safety gate
    // (`models::select_deep_model` → `plan_deep_load`): the 7B only at ≥16GB,
    // else the compact 1.5B, else the interim heuristic. Same gate AI Check
    // uses — this lane used to bypass it and load the 7B unconditionally, which
    // is a multi-hour uncapped run on 8GB. The model is scoped INSIDE this lane
    // so it is dropped before the verification stage (one-at-a-time on 8GB).
    let ai = lane(emit, "ai", 3, || {
        let model = crate::models::perplexity_model();
        let report = ai_detect::detect_extraction(&*model, &extraction);
        let summary = format!("signal: {:?} (model: {})", report.signal, model.name());
        Ok((report, summary))
    })?;

    // 4) Plagiarism — per-session isolated store; self/internal duplication.
    let plag = lane(emit, "plagiarism", 4, || {
        let mut session = plagiarism::PlagiarismSession::new()?;
        session.ingest_manuscript(embedder.as_ref(), &text)?;
        let report = session.report(&db, None)?;
        let summary = format!(
            "{} corpus match(es), {} self-match(es)",
            report.corpus_matches.len(),
            report.self_matches.len()
        );
        Ok((report, summary))
    })?;

    // 5) RAG — semantic search over the (currently empty) corpus. Honest: with
    // no seeded guidelines it returns nothing; that lands until you seed it.
    let hits = lane(emit, "rag", 5, || {
        let hits = rag::search(&db, embedder.as_ref(), "reporting standards guidelines", 5, None)?;
        let summary = if hits.is_empty() {
            "no guideline matches (corpus not seeded yet)".to_string()
        } else {
            format!("{} guideline match(es)", hits.len())
        };
        Ok((hits, summary))
    })?;

    // 6) Verification — REAL refverify HTTP per reference (existence/retraction);
    // hallucination verdict via verify_citations routed to the local SLM-2
    // (Ollama) when reachable, else honest UNKNOWNs — never fabricated, never
    // fatal (see `crate::models::verify_proxy`).
    let verification = lane(emit, "verification", 6, || {
        let mut items: Vec<(Reference, ReferenceVerification)> = Vec::new();
        let refs = &extraction.references;
        if !refs.is_empty() {
            let verifier = RefVerifier::new()?;
            let now = now_epoch();
            let total = refs.len();
            for (i, r) in refs.iter().enumerate() {
                match verifier.verify(&db, r, now) {
                    Ok(rv) => items.push((r.clone(), rv)),
                    Err(e) => tracing::warn!(reference = %r.raw, error = %e, "refverify failed for a reference"),
                }
                let pct = (((i + 1) * 100) / total) as u8;
                emit(AnalysisEvent::StageProgress { stage: "verification".into(), pct });
            }
        }
        // Route to local SLM-2 if reachable, else honest UNKNOWNs. The belt: if
        // a reachable Ollama errors mid-call (model not pulled, timeout, bad
        // reply), degrade to the mock's empty verdicts rather than fail the run.
        let proxy = crate::models::verify_proxy(user_token.clone());
        let report = match verify_citations(&*proxy, &items) {
            Ok(r) => r,
            Err(e) => {
                tracing::warn!(error = %e, "SLM-2: verification call failed; recording UNKNOWN verdicts");
                let mock = MockProxyClient::returning(serde_json::json!({ "verdicts": [] }));
                verify_citations(&mock, &items)?
            }
        };
        // 8GB OOM guard: explicitly unload SLM-2 (verified via /api/ps) so
        // qwen3:4b is out of memory before any next analysis loads candle SLM-1.
        crate::models::unload_slm2();
        let definite = report
            .verdicts
            .iter()
            .filter(|v| v.verdict != gaply_core::verify_agent::Verdict::Unknown)
            .count();
        let summary = format!(
            "{} reference(s) checked via public APIs; {} definite verdict(s)",
            items.len(),
            definite
        );
        // Registry evidence retained for the LOCAL side. `matched_year` and
        // `citation_count` were already transmitted to the cloud reviewer inside
        // the evidence bundle (`verify_agent.rs:176`, `:206`); dropping `items`
        // here made them unavailable to any local deterministic finding — a
        // LOCALITY problem, not unused computation (ARCHITECTURE_TRACE §2).
        let registry: Vec<ReferenceVerification> = items.into_iter().map(|(_, rv)| rv).collect();
        Ok(((report, registry), summary))
    })?;
    let (verification, registry) = verification;

    // --- synthesis: round-table debate → compiled report --------------------
    let report = (|| -> Result<gaply_core::report::PublishReadyReport, GaplyError> {
        let mut agents: Vec<Box<dyn SwarmAgent>> = vec![
            Box::new(PrecomputedAgent::new(adapters::from_extraction(&extraction))),
            Box::new(PrecomputedAgent::new(adapters::from_validation(&validation))),
            Box::new(PrecomputedAgent::new(adapters::from_ai_detection(&ai))),
            Box::new(PrecomputedAgent::new(adapters::from_plagiarism(&plag))),
            Box::new(PrecomputedAgent::new(adapters::from_rag_hits(&hits))),
            Box::new(PrecomputedAgent::new(adapters::from_verification_report(&verification))),
        ];
        let outcome = run_debate(&mut agents, &DebateConfig::default())?;
        // Build the checklist from the ingested journal_guideline corpus. With
        // no guidelines ingested, build_checklist still returns the always-on
        // structural checks (section presence) — never fabricated guideline
        // items. It is a DB/embedder query (no model load), so the one-at-a-time
        // model lifecycle is unaffected. Degrade to empty on a query error
        // rather than fail the whole analysis.
        let checklist = build_checklist(&db, &extraction, &text, guidelines_url.as_deref())
            .unwrap_or_else(|e| {
                tracing::warn!(error = %e, "build_checklist failed; empty checklist");
                Vec::new()
            });
        // `extraction` unlocks the three extraction-derived finding families
        // (stylometry, tables, reference recency) — pure, no model, no network.
        // `current_year` is injected rather than read inside the core so
        // compile_report stays deterministic (the `now_epoch()` convention).
        Ok(compile_report(
            &outcome,
            &validation,
            Some(&verification),
            Some(&plag),
            Some(&extraction),
            current_year(),
            checklist,
            &registry,
        ))
    })();

    let report = match report {
        Ok(r) => r,
        Err(e) => {
            emit(AnalysisEvent::Failed { stage: "synthesis".into(), message: e.to_string() });
            return Err(e);
        }
    };

    // Persist the compiled report so the viewer can load the REAL output.
    let report_id = manuscript_id.to_string();
    let json = serde_json::to_string(&report)
        .map_err(|e| GaplyError::Internal(format!("serialize report: {e}")))?;
    db.cache_put(&report_cache_key(&report_id), &json, REPORT_TTL_SECS, now_epoch())?;

    emit(AnalysisEvent::Finished { report_id });

    // §26 PR-4's criterion, applied at the ONLY place the inputs are in scope.
    // (a) `Rag` is absent — it produces `ProcessState` only, so it could never
    // have contributed to a verdict and its silence says nothing.
    // (b) Each flag asks whether the INPUT to eligible-claim production was
    // present, never whether the lane produced output.
    Ok(LaneExamination {
        // The whole lane is gated on `!refs.is_empty()`.
        verification_examined: !extraction.references.is_empty(),
        // `validate()` iterates `result.statistics`; empty in, no flags out.
        validation_examined: !extraction.statistics.is_empty(),
        // Nothing to compare against, and too few chunks for self-overlap.
        plagiarism_examined: plag.corpus_chunks_available > 0 || plag.chunk_count >= 2,
        // Below the stylometry gates no eligible finding is possible.
        ai_detection_examined: text.split_whitespace().count() >= MIN_STYLOMETRY_WORDS,
        // Its eligible outputs are table-caption and reference-recency findings.
        extraction_examined: !extraction.tables.is_empty() || !extraction.references.is_empty(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The cache key must change when the evidence schema does, so a report
    /// written by an older binary MISSES rather than deserializing to an empty
    /// evidence vector — which `escalation.rs:72`'s `unwrap_or_default()` would
    /// turn into `Accept` at 0.92 (§25.10).
    #[test]
    fn the_cache_key_carries_the_evidence_schema_version() {
        let key = report_cache_key("42");
        assert!(key.starts_with("report:v2:"), "the v2 prefix is retained: {key}");
        assert!(key.ends_with(":42"), "the report id is the last segment: {key}");
        assert!(
            key.contains(&format!("e{}", gaply_core::evidence::EVIDENCE_SCHEMA_VERSION)),
            "the evidence schema version must be IN the key, or a stale cached report \
             is read back with a mismatched EvidenceRecord shape: {key}"
        );
        // The property that matters is DISCRIMINATION: two schema versions must
        // not collide on one key. Asserted by construction against the current
        // format, so the test fails if the version is ever dropped from it.
        let same_id_other_version = format!(
            "report:v2:e{}:42",
            gaply_core::evidence::EVIDENCE_SCHEMA_VERSION + 1
        );
        assert_ne!(key, same_id_other_version, "a version change must change the key");
        // Same version, different manuscript -> still distinct.
        assert_ne!(key, report_cache_key("43"));
    }

    /// A key written under the OLD (unversioned) format must not be readable
    /// under the new one: the entry misses and the report is recomputed, which
    /// is the whole mechanism.
    #[test]
    fn pre_versioning_keys_no_longer_match() {
        assert_ne!(report_cache_key("42"), "report:v2:42");
    }

    #[test]
    fn analysis_event_serializes_with_type_tag() {
        let ev = AnalysisEvent::StageStarted { stage: "extraction".into(), index: 1, total: 6 };
        let v = serde_json::to_value(&ev).unwrap();
        assert_eq!(v["type"], "stageStarted");
        assert_eq!(v["stage"], "extraction");
        assert_eq!(v["total"], 6);

        let done = AnalysisEvent::Finished { report_id: "42".into() };
        let v2 = serde_json::to_value(&done).unwrap();
        assert_eq!(v2["type"], "finished");
        assert_eq!(v2["reportId"], "42");

        let fail = AnalysisEvent::Failed { stage: "verification".into(), message: "boom".into() };
        let v3 = serde_json::to_value(&fail).unwrap();
        assert_eq!(v3["type"], "failed");
        assert_eq!(v3["message"], "boom");
    }

    // A small but real IMRaD manuscript with a proper stat claim and NO
    // References section — so the verify stage makes no network call, keeping
    // this test hermetic while still exercising every lane end-to-end.
    const MANUSCRIPT: &str = "\
Title: Sleep and Memory Consolidation in Adults

Abstract
We examined whether a night of sleep improves memory consolidation in adults.

Introduction
Prior work suggests that sleep supports the consolidation of declarative memory.

Methods
We recruited 48 participants and analysed recall with a paired t-test.

Results
Sleep significantly improved recall (t(47) = 3.2, p = 0.002, d = 0.46).

Discussion
The results are consistent with a consolidation account of sleep.

Conclusion
A night of sleep improved memory consolidation in this sample.
";

    #[test]
    fn full_pipeline_runs_all_six_lanes_and_produces_a_real_report() {
        use std::cell::RefCell;

        // Lane completion + report shape don't depend on which perplexity model
        // ran, so take the heuristic and keep the suite off multi-GB GGUF loads.
        force_heuristic();
        let db = Arc::new(Database::in_memory().expect("in-memory db"));
        let embedder: Arc<dyn Embedder> = Arc::new(gaply_core::embed::HashEmbedder);

        let path = std::env::temp_dir().join(format!("gaply_pipeline_e2e_{}.txt", std::process::id()));
        std::fs::write(&path, MANUSCRIPT).expect("write temp manuscript");

        let events: RefCell<Vec<AnalysisEvent>> = RefCell::new(Vec::new());
        let emit = |e: AnalysisEvent| events.borrow_mut().push(e);

        let res = run_pipeline_inner(
            db.clone(),
            embedder,
            path.to_string_lossy().to_string(),
            Some("E2E manuscript".into()),
            None, // unauthenticated: keeps the verify lane off the cloud tier
            None, // no guidelines requested -> structural-only checklist
            &emit,
        );
        let _ = std::fs::remove_file(&path);
        res.expect("pipeline should complete without error");

        let ev = events.into_inner();

        // Every one of the six lanes must complete.
        let completed: Vec<String> = ev
            .iter()
            .filter_map(|e| match e {
                AnalysisEvent::StageCompleted { stage, .. } => Some(stage.clone()),
                _ => None,
            })
            .collect();
        for stage in ["extraction", "validation", "ai", "plagiarism", "rag", "verification"] {
            assert!(
                completed.iter().any(|s| s == stage),
                "lane '{stage}' did not complete; completed = {completed:?}"
            );
        }

        // No fabricated failure, and a real Finished report id.
        assert!(
            !ev.iter().any(|e| matches!(e, AnalysisEvent::Failed { .. })),
            "unexpected Failed event: {ev:?}"
        );
        let report_id = ev
            .iter()
            .find_map(|e| match e {
                AnalysisEvent::Finished { report_id } => Some(report_id.clone()),
                _ => None,
            })
            .expect("a Finished event with a report_id");

        // The REAL compiled report was cached and deserializes with content.
        let json = db
            .cache_get(&report_cache_key(&report_id), now_epoch())
            .unwrap()
            .expect("report cached under report:{id}");
        let report: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert!(report["verdict"].is_string(), "report has a verdict");
        assert!(
            report["disclaimer"].as_str().map(|s| !s.is_empty()).unwrap_or(false),
            "report has a non-empty disclaimer"
        );
        assert!(report["findings"].is_array(), "report has a findings array");

        // The pipeline must actually PASS the extraction to compile_report — the
        // core tests cover how the extraction-derived findings are built, but only
        // this asserts the call site is live. `signal:` provenance is unique to
        // them, and the fixture has a reference list, so recency always fires.
        let findings = report["findings"].as_array().unwrap();
        let derived: Vec<&serde_json::Value> = findings
            .iter()
            .filter(|f| {
                f["provenance"]
                    .as_array()
                    .map(|ps| ps.iter().any(|p| p.as_str().is_some_and(|s| s.starts_with("signal:"))))
                    .unwrap_or(false)
            })
            .collect();
        assert!(
            !derived.is_empty(),
            "no extraction-derived finding reached the report — the compile_report call site is \
             not passing the extraction; findings = {findings:?}"
        );
        // Their EvidenceRecords must exist 1:1 alongside, same as every other
        // finding (the pairing is by construction, this proves it survives here).
        assert_eq!(
            report["evidence"].as_array().map(|e| e.len()),
            Some(findings.len()),
            "evidence must stay 1:1 with findings after the new families are added"
        );
    }

    /// Force the interim heuristic perplexity model (no candle load) so pipeline
    /// runs in the test suite stay fast and don't stack multi-GB model loads in
    /// parallel on small machines. Harmless: no assertion here depends on which
    /// perplexity model ran.
    ///
    /// This drives the PRODUCT's own gate (`models::deep_tier` → HeuristicOnly)
    /// rather than the old trick of pointing `GAPLY_SLM1_GGUF` at a nonexistent
    /// file. That trick only ever hid the 7B — it left the compact 1.5B free to
    /// load — and it worked by defeating a file-existence check rather than by
    /// expressing an intent. Deliberately NOT unset afterwards: every pipeline
    /// test in this module wants the heuristic, so a leak across the shared test
    /// process is benign (and `set_var` is process-global regardless).
    fn force_heuristic() {
        std::env::set_var("GAPLY_DISABLE_DEEP", "1");
    }

    /// A REALISTIC-length manuscript: ~4,000 words across a full IMRaD body,
    /// versus the ~120-word `MANUSCRIPT` above.
    ///
    /// Length is the point. The 8GB memory proof
    /// (`examples/publishready_mem_probe.rs`) exercised this pipeline against a
    /// 260-word fixture, and at that length even an ungated 7B finishes in
    /// seconds — which is precisely why the AI lane's uncapped, gate-bypassing
    /// model load survived review. The AI lane's cost scales with TOKEN COUNT
    /// (~2x the manuscript's tokens, at window 512 / stride 256), so only a
    /// realistic length makes the wrong tier observable.
    ///
    /// Synthetic and repetitive by construction — the paragraphs rotate through
    /// four templates with a varying index — which is fine here: no assertion
    /// depends on the prose being novel, only on there being a lot of it. NO
    /// References section, so the verification lane makes no network call and
    /// the test stays hermetic (same reason as `MANUSCRIPT`).
    fn realistic_manuscript() -> String {
        const PARAS: [&str; 4] = [
            "Participants in cohort {i} completed the full assessment battery under standardised \
             laboratory conditions, with sessions scheduled at a consistent time of day to limit \
             circadian confounding. Each session opened with a short practice block that was \
             discarded before analysis. Trained assistants, blind to group allocation, \
             administered every instrument and recorded responses on paper forms that were later \
             double-entered by two independent coders. Discrepancies between coders were resolved \
             by consensus with a third rater. We logged room temperature, ambient noise, and \
             interruptions for each session, and treated any session with a documented \
             interruption as a candidate for sensitivity analysis rather than excluding it \
             outright.",
            "Analytic decisions for block {i} were fixed before the data were unblinded and \
             recorded in a dated internal protocol. We specified the primary contrast, the \
             covariate set, and the handling of missing observations in advance, and we report \
             every deviation from that plan. Continuous measures were screened for implausible \
             values against instrument-specific ranges, and flagged records were checked against \
             the original paper forms rather than silently corrected. Where a value could not be \
             verified against source documentation we treated it as missing. No observation was \
             removed on the basis of its effect on the primary estimate.",
            "The pattern observed in subgroup {i} was smaller than the effect reported in earlier \
             work, and the discrepancy deserves a plainer explanation than measurement noise. Our \
             sample skewed younger and more educated than the populations those studies \
             recruited, which plausibly compresses the range of the outcome. The instruments also \
             differ: what we scored as a single composite was previously reported as three \
             correlated subscales, and composites tend to attenuate contrasts that live in one \
             component. We therefore read our estimate as compatible with the earlier literature \
             rather than contradicting it, while acknowledging that neither reading is settled by \
             these data.",
            "Several limitations bound how far the result for stratum {i} should travel. \
             Recruitment ran through a single institution, so selection effects operating at the \
             point of referral are not addressed by our design. The follow-up window closed \
             before the outcome would be expected to stabilise in a minority of participants, and \
             those cases are necessarily represented by their last available measurement. The \
             analysis further assumes that dropout is unrelated to the outcome after conditioning \
             on the covariate set, an assumption we can probe but not verify. We report the \
             complete-case and imputed estimates side by side so readers can weigh both.",
        ];

        let mut m = String::from(
            "Title: Sleep Restriction and Declarative Memory Consolidation in Young Adults\n\n",
        );
        m.push_str(
            "Abstract\nWe examined whether a single night of restricted sleep degrades overnight \
             consolidation of declarative memory in healthy young adults, and whether any effect \
             survives adjustment for baseline encoding strength. Across four testing waves we \
             measured cued recall before and after a sleep opportunity, varying only the length \
             of that opportunity between arms.\n\n",
        );
        let mut i = 0usize;
        for heading in ["Introduction", "Methods", "Results", "Discussion"] {
            m.push_str(heading);
            m.push('\n');
            if heading == "Results" {
                m.push_str(
                    "Restricted sleep reduced overnight recall relative to the control arm \
                     (t(47) = 3.2, p = 0.002, d = 0.46). The adjusted contrast was unchanged in \
                     direction and magnitude (t(45) = 3.0, p = 0.004).\n\n",
                );
            }
            for _ in 0..10 {
                i += 1;
                m.push_str(&PARAS[i % PARAS.len()].replace("{i}", &i.to_string()));
                m.push_str("\n\n");
            }
        }
        m.push_str(
            "Conclusion\nA single night of restricted sleep measurably reduced overnight \
             declarative consolidation in this sample, with the caveats above.\n",
        );
        m
    }

    /// THE regression guard. The pipeline's AI lane must obey the SHARED
    /// memory-safety gate (`models::select_deep_model`), on a manuscript long
    /// enough for the choice to matter.
    ///
    /// Before the fix this lane called `slm1_model()` through an ungated
    /// `perplexity_model()`, so it loaded the full 7B on ANY machine with the
    /// GGUF on disk — bypassing both the >=16GB structural gate and the RAM
    /// courtesy check, and turning this fixture into a multi-hour run on 8GB.
    /// `GAPLY_DISABLE_DEEP=1` is the product's own "no deep model" decision; it
    /// had NO effect on this lane before, because the lane never consulted the
    /// gate that reads it. Asserting on the model NAME (not on wall-clock) keeps
    /// this deterministic on a loaded CI box.
    #[test]
    fn ai_lane_honours_the_shared_memory_gate_on_a_realistic_manuscript() {
        use std::cell::RefCell;

        force_heuristic();
        let text = realistic_manuscript();
        let words = text.split_whitespace().count();
        assert!(
            words > 3000,
            "fixture must stay realistic-length or it stops guarding anything; got {words} words"
        );

        let db = Arc::new(Database::in_memory().expect("in-memory db"));
        let embedder: Arc<dyn Embedder> = Arc::new(gaply_core::embed::HashEmbedder);
        let path = std::env::temp_dir()
            .join(format!("gaply_ai_gate_{}_{}.txt", std::process::id(), now_epoch()));
        std::fs::write(&path, &text).expect("write temp manuscript");

        let events: RefCell<Vec<AnalysisEvent>> = RefCell::new(Vec::new());
        let emit = |e: AnalysisEvent| events.borrow_mut().push(e);
        let res = run_pipeline_inner(
            db,
            embedder,
            path.to_string_lossy().to_string(),
            Some("AI gate regression".into()),
            None, // unauthenticated: keeps the verify lane off the cloud tier
            None, // no guidelines requested -> structural-only checklist
            &emit,
        );
        let _ = std::fs::remove_file(&path);
        res.expect("pipeline should complete without error");

        let summary = events
            .into_inner()
            .into_iter()
            .find_map(|e| match e {
                AnalysisEvent::StageCompleted { stage, summary } if stage == "ai" => Some(summary),
                _ => None,
            })
            .expect("the ai lane must complete and report which model ran");

        // The lane reports `model.name()`. The gate said "no deep model", so the
        // interim heuristic must be what scored the document.
        assert!(
            summary.contains("heuristic frequency proxy"),
            "gate said HeuristicOnly, so the heuristic must have scored it; got {summary:?}"
        );
        // And explicitly NOT either candle tier — the exact bypass that regressed.
        for forbidden in ["candle", "Qwen"] {
            assert!(
                !summary.contains(forbidden),
                "a gated-off run must not load a candle model; got {summary:?}"
            );
        }
    }

    fn run_and_get_report(db: &Arc<Database>, embedder: Arc<dyn Embedder>) -> serde_json::Value {
        run_and_get_report_with(db, embedder, None)
    }

    /// `guidelines_url` is now an INPUT to the pipeline, not something the
    /// checklist re-derives from corpus state — so a test that ingests
    /// guidelines must pass the document it ingested.
    fn run_and_get_report_with(
        db: &Arc<Database>,
        embedder: Arc<dyn Embedder>,
        guidelines_url: Option<String>,
    ) -> serde_json::Value {
        let path = std::env::temp_dir()
            .join(format!("gaply_ck_e2e_{}_{}.txt", std::process::id(), now_epoch()));
        std::fs::write(&path, MANUSCRIPT).expect("write manuscript");
        let events: std::cell::RefCell<Vec<AnalysisEvent>> = std::cell::RefCell::new(Vec::new());
        let emit = |e: AnalysisEvent| events.borrow_mut().push(e);
        run_pipeline_inner(
            db.clone(),
            embedder,
            path.to_string_lossy().to_string(),
            Some("checklist e2e".into()),
            None, // unauthenticated: keeps the verify lane off the cloud tier
            guidelines_url,
            &emit,
        )
        .expect("pipeline completes");
        let _ = std::fs::remove_file(&path);
        let report_id = events
            .into_inner()
            .iter()
            .find_map(|e| match e {
                AnalysisEvent::Finished { report_id } => Some(report_id.clone()),
                _ => None,
            })
            .expect("a Finished report id");
        let json = db
            .cache_get(&report_cache_key(&report_id), now_epoch())
            .unwrap()
            .expect("report cached");
        serde_json::from_str(&json).unwrap()
    }

    #[test]
    fn ingested_guidelines_appear_in_the_pipeline_checklist() {
        force_heuristic();
        let db = Arc::new(Database::in_memory().unwrap());
        let embedder: Arc<dyn Embedder> = Arc::new(gaply_core::embed::HashEmbedder);

        // Ingest a journal guideline into the corpus (as guidelines.rs would).
        let doc = gaply_core::rag::RawDocument {
            source_type: gaply_core::rag::SourceType::JournalGuideline,
            title: "Author Guidelines".into(),
            source_url: "http://journal.test/guidelines".into(),
            fetched_at: now_epoch(),
            content: "Manuscripts must not exceed 3000 words. A structured abstract is required. \
                      Authors must include a conflict of interest declaration. References must \
                      follow a numbered Vancouver style."
                .into(),
        };
        gaply_core::rag::ingest_document(&db, embedder.as_ref(), &doc).unwrap();

        let report = run_and_get_report_with(
            &db,
            embedder,
            Some("http://journal.test/guidelines".into()),
        );
        let checklist = report["checklist"].as_array().expect("checklist array");
        assert!(!checklist.is_empty(), "checklist should be populated");
        assert!(
            checklist.iter().any(|c| !c["guideline_source"].is_null()),
            "expected a guideline-derived checklist item (guideline_source set), got {checklist:?}"
        );
    }

    #[test]
    fn no_guidelines_yields_structural_only_checklist_no_crash() {
        force_heuristic();
        let db = Arc::new(Database::in_memory().unwrap());
        let embedder: Arc<dyn Embedder> = Arc::new(gaply_core::embed::HashEmbedder);

        // No guidelines ingested → pipeline still completes; checklist has only
        // always-on structural checks, none guideline-derived (no fabrication).
        let report = run_and_get_report(&db, embedder);
        let checklist = report["checklist"].as_array().expect("checklist array");
        assert!(
            checklist.iter().all(|c| c["guideline_source"].is_null()),
            "no guidelines → no guideline-derived items, got {checklist:?}"
        );
    }
}
