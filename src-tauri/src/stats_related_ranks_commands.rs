//! **The related rank tests' commands (Research OS, Phase 8Z-1, decision 641):
//! thin wrappers over `gaply_core::stats_related_ranks`, through its façade, and
//! the run manager in [`crate::runs`].**
//!
//! Each command is as Cronbach's alpha's (`stats_alpha_commands.rs`):
//! * it carries `#[tracing::instrument(skip_all)]`, so no argument reaches a
//!   span (decision 8);
//! * it checks, first, that Statistics is the active workspace
//!   (`workspace.inactive`), then that the spec's workspace is the active
//!   one (`workspace.wrong_workspace`), then the capability registry, before
//!   the slot is read, whose `capability.*` refusal it returns as it is
//!   (decision 74);
//! * it returns a view, or a refusal that is a closed code in JSON with
//!   numeric or closed-word parameters, and never a path;
//! * it holds no logic of its own: validate, run, the check point and the
//!   manifest are `stats_related_ranks`'s, the run's states are
//!   `contracts::transition`'s.
//!
//! The run's state view carries its progress as steps: the check points done
//! and their total, `stats_related_ranks::check_points`, one. The dataset is
//! opened and closed with Phase 2B's `stats_desc_open` and `stats_desc_close`,
//! unchanged. A run of another kind is refused by status, result, cancel and
//! export with `run.wrong_kind`. The manifest holds hashes and counts (decision
//! 12). Export returns bytes, and Rust writes nothing: the screen's save dialog
//! writes the one file the user names (decision 5, ADR-004a).

use std::sync::Arc;

use serde::Serialize;

use gaply_core::contracts::codes::{Closed, RunCode};
use gaply_core::contracts::workspace::check_spec;
use gaply_core::contracts::{
    ContentHash, Diagnostic, OperationSpec, ResourceEstimate, RunId, RunManifest, RunState,
    ValidationStatus, WorkspaceId,
};
use gaply_core::stats_adapters;
use gaply_core::stats_related_ranks::export::{manifest_json, result_csv};
use gaply_core::stats_related_ranks::{RelatedRanksOutcome, RelatedRanksParams, RelatedRanksCode};
use gaply_core::GaplyError;

use crate::runs::{DatasetSlots, NotStarted, RunSnapshot, Runs};
use crate::stats_desc_commands::{ExportFormat, ExportView, RunStartedView};
use crate::workspace::{refuse as refuse_workspace, refuse_capability, refuse_inactive};

/* ------------------------------------------------------------------------ *
 *  Views
 * ------------------------------------------------------------------------ */

/// The related rank tests' validation, in their own codes.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RelatedRanksValidationView {
    pub status: ValidationStatus,
    pub errors: Vec<Diagnostic<RelatedRanksCode>>,
    pub warnings: Vec<Diagnostic<RelatedRanksCode>>,
    pub missing_information: Vec<Diagnostic<RelatedRanksCode>>,
    pub estimated_resources: ResourceEstimate,
}

/// A related rank tests run's state: the rank tests', its progress in steps, the check
/// points done and their total.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RelatedRanksRunStateView {
    pub run_id: u64,
    pub state: RunState,
    pub terminal: bool,
    pub elapsed_ms: u64,
    pub cancel_requested: bool,
    pub steps_done: u64,
    pub steps_total: u64,
    pub diagnostics: Vec<Diagnostic<RelatedRanksCode>>,
}

/// A succeeded related rank tests run: the result (aliases and numbers, no
/// label), the validation's warnings and the result's limitations, and the
/// manifest (hashes and counts only).
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RelatedRanksResultView {
    pub run_id: u64,
    pub status: RunState,
    pub result: RelatedRanksOutcome,
    pub diagnostics: Vec<Diagnostic<RelatedRanksCode>>,
    pub manifest: RunManifest<RelatedRanksCode>,
}

fn run_state_view(s: RunSnapshot<RelatedRanksCode>) -> RelatedRanksRunStateView {
    RelatedRanksRunStateView {
        run_id: s.record.run_id.0,
        state: s.record.state,
        terminal: s.record.state.is_terminal(),
        elapsed_ms: s.elapsed_ms,
        cancel_requested: s.cancel_requested,
        steps_done: s.columns_done as u64,
        steps_total: s.columns_total as u64,
        diagnostics: s.diagnostics,
    }
}

/* ------------------------------------------------------------------------ *
 *  Refusals
 * ------------------------------------------------------------------------ */

/// A run-manager refusal: `{"code":"run.wrong_kind"}`.
fn refuse_run(code: RunCode) -> GaplyError {
    tracing::info!(code = code.wire(), "stats_related_ranks: refused");
    GaplyError::Validation(serde_json::json!({ "code": code.wire() }).to_string())
}

/// The capability registry's check, its refusal returned as it is
/// (decision 74).
pub fn admit(spec: &OperationSpec<RelatedRanksParams>) -> Result<(), GaplyError> {
    stats_adapters::related_ranks_admit(spec, crate::workspace::build()).map_err(refuse_capability)
}

/* ------------------------------------------------------------------------ *
 *  The commands' bodies: what each command runs, and what the tests drive
 * ------------------------------------------------------------------------ */

pub fn validate(
    slots: &DatasetSlots,
    slot: u64,
    spec: &OperationSpec<RelatedRanksParams>,
) -> Result<RelatedRanksValidationView, GaplyError> {
    admit(spec)?;
    let held = slots.get(slot).map_err(refuse_run)?;
    let r = stats_adapters::related_ranks_validate(spec, crate::workspace::build(), held.dataset());
    Ok(RelatedRanksValidationView {
        status: r.status,
        errors: r.errors,
        warnings: r.warnings,
        missing_information: r.missing_information,
        estimated_resources: r.estimated_resources,
    })
}

pub fn run(
    runs: &Arc<Runs>,
    slots: &DatasetSlots,
    slot: u64,
    spec: OperationSpec<RelatedRanksParams>,
) -> Result<RunStartedView, GaplyError> {
    admit(&spec)?;
    let held = slots.get(slot).map_err(refuse_run)?;
    let id = runs.start_related_ranks(held, spec).map_err(|n| match n {
        NotStarted::Busy => refuse_run(RunCode::RunBusy),
        // The workspace was left between the check and the start.
        NotStarted::Sealed => refuse_inactive(),
    })?;
    Ok(RunStartedView { run_id: id.0 })
}

pub fn status(runs: &Runs, run_id: u64) -> Result<RelatedRanksRunStateView, GaplyError> {
    runs.related_ranks_snapshot(RunId(run_id))
        .map(run_state_view)
        .map_err(refuse_run)
}

pub fn result(runs: &Runs, run_id: u64) -> Result<RelatedRanksResultView, GaplyError> {
    let b = runs.related_ranks_result(RunId(run_id)).map_err(refuse_run)?;
    Ok(RelatedRanksResultView {
        run_id: b.run_id.0,
        status: b.status,
        result: b.primary_outputs,
        diagnostics: b.diagnostics,
        manifest: b.manifest,
    })
}

pub fn cancel(runs: &Runs, run_id: u64) -> Result<RelatedRanksRunStateView, GaplyError> {
    runs.related_ranks_cancel(RunId(run_id))
        .map(run_state_view)
        .map_err(refuse_run)
}

pub fn export(runs: &Runs, run_id: u64, format: ExportFormat) -> Result<ExportView, GaplyError> {
    let b = runs.related_ranks_result(RunId(run_id)).map_err(refuse_run)?;
    let bytes = match format {
        ExportFormat::Csv => result_csv(&b.primary_outputs, &b.manifest),
        ExportFormat::ManifestJson => manifest_json(&b.manifest),
    };
    let sha256 = ContentHash::of_bytes(&bytes).as_hex().to_string();
    Ok(ExportView { bytes, sha256 })
}

/* ------------------------------------------------------------------------ *
 *  The Tauri commands
 * ------------------------------------------------------------------------ */

type St<'a> = tauri::State<'a, crate::state::AppState>;

async fn blocking<T: Send + 'static>(
    f: impl FnOnce() -> Result<T, GaplyError> + Send + 'static,
) -> Result<T, GaplyError> {
    tokio::task::spawn_blocking(f)
        .await
        .map_err(|_| GaplyError::Internal("stats_related_ranks task panicked".into()))?
}

#[tauri::command]
#[tracing::instrument(skip_all)]
pub async fn stats_related_ranks_validate(
    state: St<'_>,
    slot: u64,
    spec: OperationSpec<RelatedRanksParams>,
) -> Result<RelatedRanksValidationView, GaplyError> {
    let engine = state
        .workspace
        .require_statistics()
        .map_err(refuse_workspace)?;
    check_spec(
        WorkspaceId::Statistics,
        spec.workspace_id,
        &spec.capability_id,
    )
    .map_err(refuse_workspace)?;
    let slots = engine.datasets.clone();
    blocking(move || validate(&slots, slot, &spec)).await
}

#[tauri::command]
#[tracing::instrument(skip_all)]
pub async fn stats_related_ranks_run(
    state: St<'_>,
    slot: u64,
    spec: OperationSpec<RelatedRanksParams>,
) -> Result<RunStartedView, GaplyError> {
    let engine = state
        .workspace
        .require_statistics()
        .map_err(refuse_workspace)?;
    check_spec(
        WorkspaceId::Statistics,
        spec.workspace_id,
        &spec.capability_id,
    )
    .map_err(refuse_workspace)?;
    let (runs, slots) = (engine.runs.clone(), engine.datasets.clone());
    blocking(move || run(&runs, &slots, slot, spec)).await
}

#[tauri::command]
#[tracing::instrument(skip_all)]
pub async fn stats_related_ranks_status(
    state: St<'_>,
    run_id: u64,
) -> Result<RelatedRanksRunStateView, GaplyError> {
    let engine = state
        .workspace
        .require_statistics()
        .map_err(refuse_workspace)?;
    status(&engine.runs, run_id)
}

#[tauri::command]
#[tracing::instrument(skip_all)]
pub async fn stats_related_ranks_result(
    state: St<'_>,
    run_id: u64,
) -> Result<RelatedRanksResultView, GaplyError> {
    let engine = state
        .workspace
        .require_statistics()
        .map_err(refuse_workspace)?;
    result(&engine.runs, run_id)
}

#[tauri::command]
#[tracing::instrument(skip_all)]
pub async fn stats_related_ranks_cancel(
    state: St<'_>,
    run_id: u64,
) -> Result<RelatedRanksRunStateView, GaplyError> {
    let engine = state
        .workspace
        .require_statistics()
        .map_err(refuse_workspace)?;
    cancel(&engine.runs, run_id)
}

#[tauri::command]
#[tracing::instrument(skip_all)]
pub async fn stats_related_ranks_export(
    state: St<'_>,
    run_id: u64,
    format: ExportFormat,
) -> Result<ExportView, GaplyError> {
    let engine = state
        .workspace
        .require_statistics()
        .map_err(refuse_workspace)?;
    let runs = engine.runs.clone();
    blocking(move || export(&runs, run_id, format)).await
}
