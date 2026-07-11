//! Thin IPC adapters. Every command is a one-line delegation into
//! `gaply_core` — no business logic lives here, so the Tauri layer can be
//! swapped out without touching the core.

use serde::{Deserialize, Serialize};
use tauri::State;

use gaply_core::ai_detect::{self, AiDetectionReport, HeuristicModel};
use gaply_core::db::{DbHealth, DbInitReport, MigrationReport};
use gaply_core::extract::citations::Reference;
use gaply_core::extract::{self, docparse, ExtractionResult};
use gaply_core::plagiarism::{PlagiarismReport, PlagiarismSession};
use gaply_core::projects::{self, Project};
use gaply_core::rag::{self, RagHit};
use gaply_core::refverify::ReferenceVerification;
use gaply_core::secrets;
use gaply_core::validate::{self, StatsValidityReport};
use gaply_core::{now_epoch, GaplyError};

use crate::http_fetcher::RefVerifier;
use crate::state::AppState;

/// Reference input from the Citation Manager (refverifyBridge). Only bibliographic
/// metadata — never manuscript text.
#[derive(Debug, Deserialize)]
pub struct ReferenceInput {
    pub raw: String,
    pub doi: Option<String>,
    pub title: Option<String>,
    pub year: Option<i32>,
}

#[derive(Debug, Serialize)]
pub struct HealthReport {
    pub core_version: String,
    pub db_ok: bool,
}

#[tauri::command]
#[tracing::instrument(skip(state))]
pub fn create_project(
    state: State<'_, AppState>,
    name: String,
    description: Option<String>,
) -> Result<Project, GaplyError> {
    projects::create_project(state.store.as_ref(), &name, description.as_deref().unwrap_or(""))
}

#[tauri::command]
#[tracing::instrument(skip(state))]
pub fn list_projects(state: State<'_, AppState>) -> Result<Vec<Project>, GaplyError> {
    projects::list_projects(state.store.as_ref())
}

#[tauri::command]
#[tracing::instrument(skip(state))]
pub fn get_project(state: State<'_, AppState>, id: i64) -> Result<Project, GaplyError> {
    projects::get_project(state.store.as_ref(), id)
}

#[tauri::command]
#[tracing::instrument(skip(state))]
pub fn health_check(state: State<'_, AppState>) -> Result<HealthReport, GaplyError> {
    let db_ok = state.store.ping().is_ok();
    Ok(HealthReport { core_version: gaply_core::core_version().to_string(), db_ok })
}

#[tauri::command]
#[tracing::instrument(skip(state))]
pub fn db_init(state: State<'_, AppState>) -> Result<DbInitReport, GaplyError> {
    state.db.init_report()
}

#[tauri::command]
#[tracing::instrument(skip(state))]
pub fn db_migrate(state: State<'_, AppState>) -> Result<MigrationReport, GaplyError> {
    state.db.migrate()
}

#[tauri::command]
#[tracing::instrument(skip(state))]
pub fn db_health(state: State<'_, AppState>) -> Result<DbHealth, GaplyError> {
    state.db.health()
}

/// Parse a manuscript file (PDF/DOCX/TXT), extract its structure and claims,
/// store the result, and return the typed `ExtractionResult`. Pure offline:
/// the file is read from the local disk and never leaves the machine.
#[tauri::command]
#[tracing::instrument(skip(state))]
pub fn extract_manuscript(
    state: State<'_, AppState>,
    path: String,
    title: Option<String>,
) -> Result<ExtractionResult, GaplyError> {
    let text = docparse::parse_path(std::path::Path::new(&path))?;
    let result = extract::extract_from_text(&text);
    let manuscript_title = title
        .or_else(|| result.title.clone())
        .unwrap_or_else(|| "Untitled manuscript".to_string());
    let manuscript_id = state.db.create_manuscript(&manuscript_title, "", "")?;
    extract::persist::store_extraction(&state.db, manuscript_id, &result)?;
    Ok(result)
}

/// Full offline analysis pipeline: parse → extract → deterministically
/// validate. Stores the extraction and the validation findings (severity
/// CRITICAL/MAJOR), and returns the validity report. No network, no LLM.
#[tauri::command]
#[tracing::instrument(skip(state))]
pub fn validate_manuscript(
    state: State<'_, AppState>,
    path: String,
    title: Option<String>,
) -> Result<StatsValidityReport, GaplyError> {
    let text = docparse::parse_path(std::path::Path::new(&path))?;
    let extraction = extract::extract_from_text(&text);
    let manuscript_title = title
        .or_else(|| extraction.title.clone())
        .unwrap_or_else(|| "Untitled manuscript".to_string());
    let manuscript_id = state.db.create_manuscript(&manuscript_title, "", "")?;
    let store = extract::persist::store_extraction(&state.db, manuscript_id, &extraction)?;
    let report = validate::validate(&extraction);
    validate::store_validation(&state.db, manuscript_id, Some(store.extraction_id), &report)?;
    Ok(report)
}

/// Offline AI-text detection: parse → extract → per-section perplexity /
/// burstiness scoring. Returns a report that is an explicit statistical
/// signal (never proof — see the mandatory disclaimer field). No network,
/// no LLM call. Uses the interim heuristic scorer until GPT-2 is wired.
/// Offline plagiarism / semantic-similarity check. Parses the manuscript,
/// embeds its chunks into a PER-SESSION ISOLATED store (never the shared
/// corpus), and compares against the shared corpus and against itself.
/// Returns matched spans with similarity scores and source provenance.
#[tauri::command]
#[tracing::instrument(skip(state))]
pub fn check_plagiarism(
    state: State<'_, AppState>,
    path: String,
) -> Result<PlagiarismReport, GaplyError> {
    let text = docparse::parse_path(std::path::Path::new(&path))?;
    let mut session = PlagiarismSession::new()?;
    session.ingest_manuscript(state.embedder.as_ref(), &text)?;
    session.report(&state.db, None)
}

// --- Secret handling ---------------------------------------------------------
// API keys live only in the OS keychain, accessed by the Rust core. The
// frontend may store, check, or delete a key the user typed, but there is NO
// command to read a raw key back — external calls that need it are made from
// the core, never from React.

/// Fetch a compiled report by id (the `report_id` from AnalysisEvent::Finished).
/// Returns the REAL cached compile_report output — the report viewer routes to
/// this instead of the sample fixture. NotFound if it expired / never existed.
#[tauri::command]
#[tracing::instrument(skip(state))]
pub fn get_report(state: State<'_, AppState>, report_id: String) -> Result<serde_json::Value, GaplyError> {
    let key = format!("report:{report_id}");
    match state.db.cache_get(&key, now_epoch())? {
        Some(json) => serde_json::from_str(&json)
            .map_err(|e| GaplyError::Internal(format!("parse cached report: {e}"))),
        None => Err(GaplyError::NotFound { entity: "report", id: report_id }),
    }
}

/// Write the bundled sample manuscript to a real temp file and return its
/// absolute path, so the one-click "sample scan" runs through the REAL pipeline
/// (the old `__bundled_sample__` sentinel pointed at no file). Text is embedded
/// in the binary, so this works in dev and in a packaged build.
#[tauri::command]
#[tracing::instrument]
pub fn sample_manuscript_path() -> Result<String, GaplyError> {
    const SAMPLE: &str = include_str!("../resources/sample_manuscript.txt");
    let path = std::env::temp_dir().join("gaply-sample-manuscript.txt");
    std::fs::write(&path, SAMPLE)?;
    Ok(path.to_string_lossy().to_string())
}

/// Store an API key (or any secret) the user entered, in the OS keychain.
#[tauri::command]
#[tracing::instrument(skip(value))]
pub fn store_secret(name: String, value: String) -> Result<(), GaplyError> {
    secrets::store_secret(&name, &value)
}

/// Whether a named secret exists (presence only — never the value).
#[tauri::command]
#[tracing::instrument]
pub fn has_secret(name: String) -> Result<bool, GaplyError> {
    secrets::has_secret(&name)
}

/// Delete a named secret from the OS keychain.
#[tauri::command]
#[tracing::instrument]
pub fn delete_secret(name: String) -> Result<(), GaplyError> {
    secrets::delete_secret(&name)
}

#[tauri::command]
#[tracing::instrument]
pub fn detect_ai(path: String) -> Result<AiDetectionReport, GaplyError> {
    let text = docparse::parse_path(std::path::Path::new(&path))?;
    let extraction = extract::extract_from_text(&text);
    let model = HeuristicModel::gpt2_like();
    Ok(ai_detect::detect_extraction(&model, &extraction))
}

/// Verify one reference against the live connectors (CrossRef / OpenAlex /
/// Retraction Watch / Unpaywall / Semantic Scholar) — real HTTP via rustls,
/// cache-first and rate-limited. The Citation Manager (refverifyBridge) calls
/// this; it was previously MISSING, so verify/enrich silently did nothing.
///
/// Async + spawn_blocking: the HTTP client is blocking, so it must not run on
/// the async runtime thread. Returns the structured ReferenceVerification
/// (metadata only — no manuscript text is involved).
#[tauri::command]
#[tracing::instrument(skip(state))]
pub async fn verify_reference(
    state: State<'_, AppState>,
    reference: ReferenceInput,
) -> Result<ReferenceVerification, GaplyError> {
    let db = state.db.clone();
    tokio::task::spawn_blocking(move || {
        let verifier = RefVerifier::new()?;
        let reference = Reference {
            raw: reference.raw,
            authors: String::new(),
            year: reference.year,
            title: reference.title,
            doi: reference.doi,
        };
        verifier.verify(&db, &reference, now_epoch())
    })
    .await
    .map_err(|e| GaplyError::Internal(format!("verify_reference task panicked: {e}")))?
}

#[tauri::command]
#[tracing::instrument(skip(state))]
pub fn rag_search(
    state: State<'_, AppState>,
    query: String,
    top_k: Option<usize>,
    source_filter: Option<String>,
) -> Result<Vec<RagHit>, GaplyError> {
    rag::search(
        &state.db,
        state.embedder.as_ref(),
        &query,
        top_k.unwrap_or(8).min(64),
        source_filter.as_deref(),
    )
}

/// Ingest a target journal's page and/or author-guidelines URL into the
/// `journal_guideline` corpus (PublishReady). Rate-limited fetch, all fetched
/// text treated as untrusted (sanitized/quarantined). Never fails the job:
/// unreachable/non-HTML/injected sources come back as Unavailable/Quarantined
/// with an honest note, and the checklist simply stays bare — no fabrication.
#[tauri::command]
#[tracing::instrument(skip(state))]
pub fn ingest_guidelines(
    state: State<'_, AppState>,
    journal_url: Option<String>,
    guidelines_url: Option<String>,
) -> Result<crate::guidelines::GuidelinesReport, GaplyError> {
    let ingestor = crate::guidelines::GuidelinesIngestor::new()?;
    Ok(ingestor.ingest(
        &state.db,
        state.embedder.as_ref(),
        journal_url.as_deref(),
        guidelines_url.as_deref(),
    ))
}
