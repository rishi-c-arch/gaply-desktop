//! **The Stats Verifier page's legacy lane, as two commands (Research OS,
//! Phase 4A, decision 47).**
//!
//! `run_stats_preview` and `run_stats_verify` are not capabilities and are
//! not in the registry: they are the older Set 2 engine the Stats Verifier
//! page still calls, removed by a later step once it no longer does. They get
//! the workspace check all the same (decision 49): each command's first
//! statement refuses with `workspace.inactive` unless Statistics is active,
//! and only then calls the body, which stays in `commands.rs` with its
//! arguments unchanged, so the webview's call and the span test that calls
//! the body directly (`tests/no_path_in_command_spans.rs`) are as before.

use tauri::State;

use gaply_core::stats_verdict::{AnalysisSpec, VerificationReport as StatsVerificationReport};
use gaply_core::GaplyError;

use crate::commands::StatsPreview;
use crate::state::AppState;

#[tauri::command]
#[tracing::instrument(skip_all)]
pub fn run_stats_preview(
    state: State<'_, AppState>,
    path: String,
    manuscript_path: Option<String>,
) -> Result<StatsPreview, GaplyError> {
    let _ = state
        .workspace
        .require_statistics()
        .map_err(crate::workspace::refuse)?;
    crate::commands::run_stats_preview(path, manuscript_path)
}

#[tauri::command]
#[tracing::instrument(skip_all)]
pub fn run_stats_verify(
    state: State<'_, AppState>,
    path: String,
    spec: AnalysisSpec,
    manuscript_path: Option<String>,
) -> Result<StatsVerificationReport, GaplyError> {
    let _ = state
        .workspace
        .require_statistics()
        .map_err(crate::workspace::refuse)?;
    crate::commands::run_stats_verify(path, spec, manuscript_path)
}
