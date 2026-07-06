//! Thin IPC adapters. Every command is a one-line delegation into
//! `gaply_core` — no business logic lives here, so the Tauri layer can be
//! swapped out without touching the core.

use serde::Serialize;
use tauri::State;

use gaply_core::ai_detect::{self, AiDetectionReport, HeuristicModel};
use gaply_core::db::{DbHealth, DbInitReport, MigrationReport};
use gaply_core::extract::{self, docparse, ExtractionResult};
use gaply_core::projects::{self, Project};
use gaply_core::rag::{self, RagHit};
use gaply_core::validate::{self, StatsValidityReport};
use gaply_core::GaplyError;

use crate::state::AppState;

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
#[tauri::command]
#[tracing::instrument]
pub fn detect_ai(path: String) -> Result<AiDetectionReport, GaplyError> {
    let text = docparse::parse_path(std::path::Path::new(&path))?;
    let extraction = extract::extract_from_text(&text);
    let model = HeuristicModel::gpt2_like();
    Ok(ai_detect::detect_extraction(&model, &extraction))
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
