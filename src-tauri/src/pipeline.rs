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
use gaply_core::report::compile_report;
use gaply_core::swarm::{adapters, run_debate, DebateConfig, PrecomputedAgent, SwarmAgent};
use gaply_core::verify_agent::{verify_citations, MockProxyClient};
use gaply_core::{ai_detect, extract, now_epoch, plagiarism, rag, validate, Database, GaplyError};

use crate::http_fetcher::RefVerifier;
use crate::state::AppState;

/// Report cache TTL: 30 days. The viewer reads it back by `report_id`.
const REPORT_TTL_SECS: i64 = 30 * 24 * 3600;
/// The six agent lanes the frontend renders. Debate + compile happen after,
/// under the "synthesis" pseudo-stage.
const LANE_TOTAL: usize = 6;

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
    tokio::task::spawn_blocking(move || run_pipeline(db, embedder, path, title, on_event))
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
    ch: Channel<AnalysisEvent>,
) -> Result<(), GaplyError> {
    run_pipeline_inner(db, embedder, path, title, &|ev| {
        let _ = ch.send(ev);
    })
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
    emit: &dyn Fn(AnalysisEvent),
) -> Result<(), GaplyError> {
    run_pipeline_inner(db, embedder, path, title, emit)
}

/// The synchronous pipeline. Every stage is a real production call. `emit` is
/// abstracted (Fn) so tests can drive the full pipeline with a collector.
fn run_pipeline_inner(
    db: Arc<Database>,
    embedder: Arc<dyn Embedder>,
    path: String,
    title: Option<String>,
    emit: &dyn Fn(AnalysisEvent),
) -> Result<(), GaplyError> {
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

    // 3) AI check — real SLM-1 (candle) perplexity/burstiness when the model is
    // present, else the interim heuristic. The model is scoped INSIDE this lane
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
        let proxy = crate::models::verify_proxy();
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
        Ok((report, summary))
    })?;

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
        // Checklist needs seeded guidelines (deferred) — empty for now.
        Ok(compile_report(&outcome, &validation, Some(&verification), Vec::new()))
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
    db.cache_put(&format!("report:{report_id}"), &json, REPORT_TTL_SECS, now_epoch())?;

    emit(AnalysisEvent::Finished { report_id });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

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
            .cache_get(&format!("report:{report_id}"), now_epoch())
            .unwrap()
            .expect("report cached under report:{id}");
        let report: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert!(report["verdict"].is_string(), "report has a verdict");
        assert!(
            report["disclaimer"].as_str().map(|s| !s.is_empty()).unwrap_or(false),
            "report has a non-empty disclaimer"
        );
        assert!(report["findings"].is_array(), "report has a findings array");
    }
}
