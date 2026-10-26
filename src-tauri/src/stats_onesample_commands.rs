//! **The one-sample and paired-samples t-tests' commands (Research OS, Phase
//! 8D-2a, decision 263): thin wrappers over `gaply_core::stats_onesample`,
//! through its façades, and the run manager in [`crate::runs`].**
//!
//! Each command is as the independent t-test's (`stats_ttest_commands.rs`):
//! * it carries `#[tracing::instrument(skip_all)]`, so no argument reaches a
//!   span (decision 8);
//! * it checks, first, that Statistics is the active workspace
//!   (`workspace.inactive`), then that the spec's workspace is the active
//!   one (`workspace.wrong_workspace`), then the capability registry, before
//!   the slot is read, whose `capability.*` refusal it returns as it is
//!   (decision 74);
//! * it returns a view, or a refusal that is a closed code in JSON with
//!   numeric or closed-word parameters, and never a path;
//! * it holds no logic of its own: validate, run and manifest are
//!   `stats_onesample`'s, the run's states are `contracts::transition`'s.
//!
//! The two tests share their codes (`OneSampleCode`), so they share the
//! validation view and the run-state view; each has its own result view. The
//! dataset is opened and closed with Phase 2B's `stats_desc_open` and
//! `stats_desc_close`, unchanged. A run of another kind is refused by status,
//! result, cancel and export with `run.wrong_kind`. The manifest holds hashes
//! and counts (decision 12). Export returns bytes, and Rust writes nothing:
//! the screen's save dialog writes the one file the user names (decision 5,
//! ADR-004a). Neither test reads a group, so neither has a levels command.

use std::sync::Arc;

use serde::Serialize;

use gaply_core::contracts::codes::{Closed, RunCode};
use gaply_core::contracts::workspace::check_spec;
use gaply_core::contracts::{
    ContentHash, Diagnostic, OperationSpec, ResourceEstimate, RunId, RunManifest, RunState,
    ValidationReport, ValidationStatus, WorkspaceId,
};
use gaply_core::stats_adapters;
use gaply_core::stats_onesample::export::{
    manifest_json, one_sample_result_csv, paired_result_csv,
};
use gaply_core::stats_onesample::{
    OneSampleCode, OneSampleParams, OneSampleResult, PairedParams, PairedResult,
};
use gaply_core::GaplyError;

use crate::runs::{DatasetSlots, NotStarted, RunSnapshot, Runs};
use crate::stats_desc_commands::{ExportFormat, ExportView, RunStartedView};
use crate::workspace::{refuse as refuse_workspace, refuse_capability, refuse_inactive};

/* ------------------------------------------------------------------------ *
 *  Views
 * ------------------------------------------------------------------------ */

/// Either test's validation, in their shared codes.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OneSampleValidationView {
    pub status: ValidationStatus,
    pub errors: Vec<Diagnostic<OneSampleCode>>,
    pub warnings: Vec<Diagnostic<OneSampleCode>>,
    pub missing_information: Vec<Diagnostic<OneSampleCode>>,
    pub estimated_resources: ResourceEstimate,
}

/// Either test's run state: the independent t-test's, in these codes.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OneSampleRunStateView {
    pub run_id: u64,
    pub state: RunState,
    pub terminal: bool,
    pub elapsed_ms: u64,
    pub cancel_requested: bool,
    pub diagnostics: Vec<Diagnostic<OneSampleCode>>,
}

/// A succeeded one-sample t-test: the result, the validation's warnings and
/// the result's limitations, and the manifest (hashes and counts only).
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OneSampleResultView {
    pub run_id: u64,
    pub status: RunState,
    pub result: OneSampleResult,
    pub diagnostics: Vec<Diagnostic<OneSampleCode>>,
    pub manifest: RunManifest<OneSampleCode>,
}

/// A succeeded paired t-test, as [`OneSampleResultView`].
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PairedResultView {
    pub run_id: u64,
    pub status: RunState,
    pub result: PairedResult,
    pub diagnostics: Vec<Diagnostic<OneSampleCode>>,
    pub manifest: RunManifest<OneSampleCode>,
}

fn validation_view(r: ValidationReport<OneSampleCode>) -> OneSampleValidationView {
    OneSampleValidationView {
        status: r.status,
        errors: r.errors,
        warnings: r.warnings,
        missing_information: r.missing_information,
        estimated_resources: r.estimated_resources,
    }
}

fn run_state_view(s: RunSnapshot<OneSampleCode>) -> OneSampleRunStateView {
    OneSampleRunStateView {
        run_id: s.record.run_id.0,
        state: s.record.state,
        terminal: s.record.state.is_terminal(),
        elapsed_ms: s.elapsed_ms,
        cancel_requested: s.cancel_requested,
        diagnostics: s.diagnostics,
    }
}

/* ------------------------------------------------------------------------ *
 *  Refusals
 * ------------------------------------------------------------------------ */

/// A run-manager refusal: `{"code":"run.wrong_kind"}`.
fn refuse_run(code: RunCode) -> GaplyError {
    tracing::info!(code = code.wire(), "stats_onesample: refused");
    GaplyError::Validation(serde_json::json!({ "code": code.wire() }).to_string())
}

/// A refused start: another run is going, or the workspace was left between
/// the check and the start.
fn not_started(n: NotStarted) -> GaplyError {
    match n {
        NotStarted::Busy => refuse_run(RunCode::RunBusy),
        NotStarted::Sealed => refuse_inactive(),
    }
}

/// The capability registry's check for a one-sample spec, its refusal
/// returned as it is (decision 74).
pub fn admit_one_sample(spec: &OperationSpec<OneSampleParams>) -> Result<(), GaplyError> {
    stats_adapters::one_sample_admit(spec, crate::workspace::build()).map_err(refuse_capability)
}

/// The capability registry's check for a paired spec, as
/// [`admit_one_sample`].
pub fn admit_paired(spec: &OperationSpec<PairedParams>) -> Result<(), GaplyError> {
    stats_adapters::paired_admit(spec, crate::workspace::build()).map_err(refuse_capability)
}

/* ------------------------------------------------------------------------ *
 *  The commands' bodies: what each command runs, and what the tests drive
 * ------------------------------------------------------------------------ */

pub fn validate_one_sample(
    slots: &DatasetSlots,
    slot: u64,
    spec: &OperationSpec<OneSampleParams>,
) -> Result<OneSampleValidationView, GaplyError> {
    admit_one_sample(spec)?;
    let held = slots.get(slot).map_err(refuse_run)?;
    Ok(validation_view(stats_adapters::one_sample_validate(
        spec,
        crate::workspace::build(),
        held.dataset(),
    )))
}

pub fn validate_paired(
    slots: &DatasetSlots,
    slot: u64,
    spec: &OperationSpec<PairedParams>,
) -> Result<OneSampleValidationView, GaplyError> {
    admit_paired(spec)?;
    let held = slots.get(slot).map_err(refuse_run)?;
    Ok(validation_view(stats_adapters::paired_validate(
        spec,
        crate::workspace::build(),
        held.dataset(),
    )))
}

pub fn run_one_sample(
    runs: &Arc<Runs>,
    slots: &DatasetSlots,
    slot: u64,
    spec: OperationSpec<OneSampleParams>,
) -> Result<RunStartedView, GaplyError> {
    admit_one_sample(&spec)?;
    let held = slots.get(slot).map_err(refuse_run)?;
    let id = runs.start_one_sample(held, spec).map_err(not_started)?;
    Ok(RunStartedView { run_id: id.0 })
}

pub fn run_paired(
    runs: &Arc<Runs>,
    slots: &DatasetSlots,
    slot: u64,
    spec: OperationSpec<PairedParams>,
) -> Result<RunStartedView, GaplyError> {
    admit_paired(&spec)?;
    let held = slots.get(slot).map_err(refuse_run)?;
    let id = runs.start_paired(held, spec).map_err(not_started)?;
    Ok(RunStartedView { run_id: id.0 })
}

pub fn status_one_sample(runs: &Runs, run_id: u64) -> Result<OneSampleRunStateView, GaplyError> {
    runs.one_sample_snapshot(RunId(run_id))
        .map(run_state_view)
        .map_err(refuse_run)
}

pub fn status_paired(runs: &Runs, run_id: u64) -> Result<OneSampleRunStateView, GaplyError> {
    runs.paired_snapshot(RunId(run_id))
        .map(run_state_view)
        .map_err(refuse_run)
}

pub fn result_one_sample(runs: &Runs, run_id: u64) -> Result<OneSampleResultView, GaplyError> {
    let b = runs.one_sample_result(RunId(run_id)).map_err(refuse_run)?;
    Ok(OneSampleResultView {
        run_id: b.run_id.0,
        status: b.status,
        result: b.primary_outputs,
        diagnostics: b.diagnostics,
        manifest: b.manifest,
    })
}

pub fn result_paired(runs: &Runs, run_id: u64) -> Result<PairedResultView, GaplyError> {
    let b = runs.paired_result(RunId(run_id)).map_err(refuse_run)?;
    Ok(PairedResultView {
        run_id: b.run_id.0,
        status: b.status,
        result: b.primary_outputs,
        diagnostics: b.diagnostics,
        manifest: b.manifest,
    })
}

pub fn cancel_one_sample(runs: &Runs, run_id: u64) -> Result<OneSampleRunStateView, GaplyError> {
    runs.one_sample_cancel(RunId(run_id))
        .map(run_state_view)
        .map_err(refuse_run)
}

pub fn cancel_paired(runs: &Runs, run_id: u64) -> Result<OneSampleRunStateView, GaplyError> {
    runs.paired_cancel(RunId(run_id))
        .map(run_state_view)
        .map_err(refuse_run)
}

fn export_view(bytes: Vec<u8>) -> ExportView {
    let sha256 = ContentHash::of_bytes(&bytes).as_hex().to_string();
    ExportView { bytes, sha256 }
}

pub fn export_one_sample(
    runs: &Runs,
    run_id: u64,
    format: ExportFormat,
) -> Result<ExportView, GaplyError> {
    let b = runs.one_sample_result(RunId(run_id)).map_err(refuse_run)?;
    Ok(export_view(match format {
        ExportFormat::Csv => one_sample_result_csv(&b.primary_outputs),
        ExportFormat::ManifestJson => manifest_json(&b.manifest),
    }))
}

pub fn export_paired(
    runs: &Runs,
    run_id: u64,
    format: ExportFormat,
) -> Result<ExportView, GaplyError> {
    let b = runs.paired_result(RunId(run_id)).map_err(refuse_run)?;
    Ok(export_view(match format {
        ExportFormat::Csv => paired_result_csv(&b.primary_outputs),
        ExportFormat::ManifestJson => manifest_json(&b.manifest),
    }))
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
        .map_err(|_| GaplyError::Internal("stats_onesample task panicked".into()))?
}

#[tauri::command]
#[tracing::instrument(skip_all)]
pub async fn stats_onesample_validate(
    state: St<'_>,
    slot: u64,
    spec: OperationSpec<OneSampleParams>,
) -> Result<OneSampleValidationView, GaplyError> {
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
    blocking(move || validate_one_sample(&slots, slot, &spec)).await
}

#[tauri::command]
#[tracing::instrument(skip_all)]
pub async fn stats_onesample_run(
    state: St<'_>,
    slot: u64,
    spec: OperationSpec<OneSampleParams>,
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
    blocking(move || run_one_sample(&runs, &slots, slot, spec)).await
}

#[tauri::command]
#[tracing::instrument(skip_all)]
pub async fn stats_onesample_status(
    state: St<'_>,
    run_id: u64,
) -> Result<OneSampleRunStateView, GaplyError> {
    let engine = state
        .workspace
        .require_statistics()
        .map_err(refuse_workspace)?;
    status_one_sample(&engine.runs, run_id)
}

#[tauri::command]
#[tracing::instrument(skip_all)]
pub async fn stats_onesample_result(
    state: St<'_>,
    run_id: u64,
) -> Result<OneSampleResultView, GaplyError> {
    let engine = state
        .workspace
        .require_statistics()
        .map_err(refuse_workspace)?;
    result_one_sample(&engine.runs, run_id)
}

#[tauri::command]
#[tracing::instrument(skip_all)]
pub async fn stats_onesample_cancel(
    state: St<'_>,
    run_id: u64,
) -> Result<OneSampleRunStateView, GaplyError> {
    let engine = state
        .workspace
        .require_statistics()
        .map_err(refuse_workspace)?;
    cancel_one_sample(&engine.runs, run_id)
}

#[tauri::command]
#[tracing::instrument(skip_all)]
pub async fn stats_onesample_export(
    state: St<'_>,
    run_id: u64,
    format: ExportFormat,
) -> Result<ExportView, GaplyError> {
    let engine = state
        .workspace
        .require_statistics()
        .map_err(refuse_workspace)?;
    let runs = engine.runs.clone();
    blocking(move || export_one_sample(&runs, run_id, format)).await
}

#[tauri::command]
#[tracing::instrument(skip_all)]
pub async fn stats_paired_validate(
    state: St<'_>,
    slot: u64,
    spec: OperationSpec<PairedParams>,
) -> Result<OneSampleValidationView, GaplyError> {
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
    blocking(move || validate_paired(&slots, slot, &spec)).await
}

#[tauri::command]
#[tracing::instrument(skip_all)]
pub async fn stats_paired_run(
    state: St<'_>,
    slot: u64,
    spec: OperationSpec<PairedParams>,
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
    blocking(move || run_paired(&runs, &slots, slot, spec)).await
}

#[tauri::command]
#[tracing::instrument(skip_all)]
pub async fn stats_paired_status(
    state: St<'_>,
    run_id: u64,
) -> Result<OneSampleRunStateView, GaplyError> {
    let engine = state
        .workspace
        .require_statistics()
        .map_err(refuse_workspace)?;
    status_paired(&engine.runs, run_id)
}

#[tauri::command]
#[tracing::instrument(skip_all)]
pub async fn stats_paired_result(
    state: St<'_>,
    run_id: u64,
) -> Result<PairedResultView, GaplyError> {
    let engine = state
        .workspace
        .require_statistics()
        .map_err(refuse_workspace)?;
    result_paired(&engine.runs, run_id)
}

#[tauri::command]
#[tracing::instrument(skip_all)]
pub async fn stats_paired_cancel(
    state: St<'_>,
    run_id: u64,
) -> Result<OneSampleRunStateView, GaplyError> {
    let engine = state
        .workspace
        .require_statistics()
        .map_err(refuse_workspace)?;
    cancel_paired(&engine.runs, run_id)
}

#[tauri::command]
#[tracing::instrument(skip_all)]
pub async fn stats_paired_export(
    state: St<'_>,
    run_id: u64,
    format: ExportFormat,
) -> Result<ExportView, GaplyError> {
    let engine = state
        .workspace
        .require_statistics()
        .map_err(refuse_workspace)?;
    let runs = engine.runs.clone();
    blocking(move || export_paired(&runs, run_id, format)).await
}
