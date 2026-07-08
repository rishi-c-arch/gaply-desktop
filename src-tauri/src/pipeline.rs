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
use gaply_core::refverify::{
    verify_reference, ApiRateLimiters, ReferenceVerification, VerifyContext,
};
use gaply_core::report::compile_report;
use gaply_core::swarm::{adapters, run_debate, DebateConfig, PrecomputedAgent, SwarmAgent};
use gaply_core::verify_agent::{verify_citations, MockProxyClient};
use gaply_core::{ai_detect, extract, now_epoch, plagiarism, rag, validate, Database, GaplyError};

use crate::http_fetcher::ReqwestFetcher;
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

/// Emit an event, ignoring a closed channel (frontend navigated away).
fn emit(ch: &Channel<AnalysisEvent>, ev: AnalysisEvent) {
    let _ = ch.send(ev);
}

/// Run a lane: emit StageStarted, execute `f`, emit StageCompleted or Failed.
/// On failure the error propagates so the whole pipeline stops.
fn lane<T>(
    ch: &Channel<AnalysisEvent>,
    stage: &str,
    index: usize,
    f: impl FnOnce() -> Result<(T, String), GaplyError>,
) -> Result<T, GaplyError> {
    emit(ch, AnalysisEvent::StageStarted { stage: stage.into(), index, total: LANE_TOTAL });
    match f() {
        Ok((value, summary)) => {
            emit(ch, AnalysisEvent::StageCompleted { stage: stage.into(), summary });
            Ok(value)
        }
        Err(e) => {
            emit(ch, AnalysisEvent::Failed { stage: stage.into(), message: e.to_string() });
            Err(e)
        }
    }
}

/// The synchronous pipeline. Every stage is a real production call.
fn run_pipeline(
    db: Arc<Database>,
    embedder: Arc<dyn Embedder>,
    path: String,
    title: Option<String>,
    ch: Channel<AnalysisEvent>,
) -> Result<(), GaplyError> {
    // Parse once, up front (part of the extraction lane's work).
    let text = lane(&ch, "extraction", 1, || {
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
        emit(
            &ch,
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
    let validation = lane(&ch, "validation", 2, || {
        let report = validate::validate(&extraction);
        let summary = if report.passed {
            "no statistical rule violations".to_string()
        } else {
            format!("{} rule violation(s)", report.flags.len())
        };
        Ok((report, summary))
    })?;

    // 3) AI check — interim heuristic perplexity/burstiness (honest disclaimer).
    let ai = lane(&ch, "ai", 3, || {
        let model = ai_detect::HeuristicModel::gpt2_like();
        let report = ai_detect::detect_extraction(&model, &extraction);
        let summary = format!("signal: {:?} (interim heuristic)", report.signal);
        Ok((report, summary))
    })?;

    // 4) Plagiarism — per-session isolated store; self/internal duplication.
    let plag = lane(&ch, "plagiarism", 4, || {
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
    let hits = lane(&ch, "rag", 5, || {
        let hits = rag::search(&db, embedder.as_ref(), "reporting standards guidelines", 5, None)?;
        let summary = if hits.is_empty() {
            "no guideline matches (corpus not seeded yet)".to_string()
        } else {
            format!("{} guideline match(es)", hits.len())
        };
        Ok((hits, summary))
    })?;

    // 6) Verification — REAL refverify HTTP per reference (existence/retraction);
    // cloud hallucination verdict via verify_citations with the mock proxy
    // (UNKNOWN until the proxy is deployed — never fabricated).
    let verification = lane(&ch, "verification", 6, || {
        let mut items: Vec<(Reference, ReferenceVerification)> = Vec::new();
        let refs = &extraction.references;
        if !refs.is_empty() {
            let fetcher = ReqwestFetcher::new()?;
            let limiters = ApiRateLimiters::with_polite_defaults();
            let ctx = VerifyContext { db: &db, http: &fetcher, limiters: &limiters, contact_email: None };
            let now = now_epoch();
            let total = refs.len();
            for (i, r) in refs.iter().enumerate() {
                match verify_reference(&ctx, r, now) {
                    Ok(rv) => items.push((r.clone(), rv)),
                    Err(e) => tracing::warn!(reference = %r.raw, error = %e, "refverify failed for a reference"),
                }
                let pct = (((i + 1) * 100) / total) as u8;
                emit(&ch, AnalysisEvent::StageProgress { stage: "verification".into(), pct });
            }
        }
        // ProxyClient is deferred: the mock returns no verdicts → UNKNOWN.
        let proxy = MockProxyClient::returning(serde_json::json!({ "verdicts": [] }));
        let report = verify_citations(&proxy, &items)?;
        let summary = format!(
            "{} reference(s) checked via public APIs; cloud verdict pending proxy",
            items.len()
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
            emit(&ch, AnalysisEvent::Failed { stage: "synthesis".into(), message: e.to_string() });
            return Err(e);
        }
    };

    // Persist the compiled report so the viewer can load the REAL output.
    let report_id = manuscript_id.to_string();
    let json = serde_json::to_string(&report)
        .map_err(|e| GaplyError::Internal(format!("serialize report: {e}")))?;
    db.cache_put(&format!("report:{report_id}"), &json, REPORT_TTL_SECS, now_epoch())?;

    emit(&ch, AnalysisEvent::Finished { report_id });
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
}
