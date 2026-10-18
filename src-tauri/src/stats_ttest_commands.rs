//! **The independent-samples t-test's commands (Research OS, Phase 4B-2):
//! thin wrappers over `gaply_core::stats_ttest`, through its façade, and the
//! run manager in [`crate::runs`].**
//!
//! Each command:
//! * carries `#[tracing::instrument(skip_all)]`, so no argument reaches a
//!   span (decision 8);
//! * checks, first, that Statistics is the active workspace
//!   (`workspace.inactive`), then that the spec's workspace is the active
//!   one (`workspace.wrong_workspace`), then the capability registry, whose
//!   `capability.*` refusal it returns as it is (decision 74);
//! * returns a view, or a refusal that is a closed code in JSON with numeric
//!   or closed-word parameters, and never a path;
//! * holds no logic of its own: validate, run and manifest are
//!   `stats_ttest`'s, the run's states are `contracts::transition`'s.
//!
//! The dataset is opened and closed with Phase 2B's `stats_desc_open` and
//! `stats_desc_close`, unchanged: one ingest, one set of slots. A run of the
//! other kind is refused by status, result, cancel and export with
//! `run.wrong_kind`. The manifest holds hashes and counts (decision 12).
//! Export returns bytes, and Rust writes nothing: the screen's save dialog
//! writes the one file the user names (decision 5, ADR-004a).
//!
//! `stats_ttest_group_levels` (the follow-up, decision 81) gives a column's
//! levels as the t-test indexes them, from `stats_ttest::group_levels`, the
//! core's own level function, so the screen's positions are the core's by
//! construction; it works for any column, a group coded in numbers
//! included, which `DatasetView`'s profile lists no levels for. The labels
//! appear in its view only. A column with more than
//! `stats_ttest::MAX_GROUP_LEVELS` (50) levels is refused with
//! `stats.ttest.too_many_levels {count, limit}` (decision 87).
//!
//! `capability_list` only reads the static registry, so it needs no
//! workspace (decision 76). Whether a capability is runnable is computed
//! here, with the registry's `require_runnable` and this build; the screen
//! never decides it.

use std::sync::Arc;

use serde::Serialize;

use gaply_core::contracts::capability::Lifecycle;
use gaply_core::contracts::codes::{Closed, RunCode};
use gaply_core::contracts::workspace::check_spec;
use gaply_core::contracts::{
    CapabilityId, ContentHash, Diagnostic, OperationSpec, ParamKey, ResourceEstimate, RunId,
    RunManifest, RunState, ValidationStatus, WorkspaceId,
};
use gaply_core::stats_adapters;
use gaply_core::stats_desc::Alias;
use gaply_core::stats_registry::{self, StatsWording};
use gaply_core::stats_ttest::export::{manifest_json, result_csv};
use gaply_core::stats_ttest::{self, TTestCode, TTestParams, TTestResult};
use gaply_core::GaplyError;

use crate::runs::{DatasetSlots, NotStarted, RunSnapshot, Runs};
use crate::stats_desc_commands::{ExportFormat, ExportView, RunStartedView};
use crate::workspace::{refuse as refuse_workspace, refuse_capability, refuse_inactive};

/* ------------------------------------------------------------------------ *
 *  Views
 * ------------------------------------------------------------------------ */

/// The t-test's validation, in its own codes.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ValidationReportView {
    pub status: ValidationStatus,
    pub errors: Vec<Diagnostic<TTestCode>>,
    pub warnings: Vec<Diagnostic<TTestCode>>,
    pub missing_information: Vec<Diagnostic<TTestCode>>,
    pub estimated_resources: ResourceEstimate,
}

/// A t-test run's state: describe's `RunStateView` without its column
/// progress, which a t-test does not have.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TTestRunStateView {
    pub run_id: u64,
    pub state: RunState,
    pub terminal: bool,
    pub elapsed_ms: u64,
    pub cancel_requested: bool,
    pub diagnostics: Vec<Diagnostic<TTestCode>>,
}

/// A succeeded t-test: the result (the two level labels are the user's own),
/// the validation's warnings, and the manifest (hashes and counts only).
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TTestResultView {
    pub run_id: u64,
    pub status: RunState,
    pub result: TTestResult,
    pub diagnostics: Vec<Diagnostic<TTestCode>>,
    pub manifest: RunManifest<TTestCode>,
}

/// One capability, for the method selector: what the registry says about
/// it, as closed codes, and whether this build may run it.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CapabilityView {
    pub id: CapabilityId,
    pub version: u32,
    pub lifecycle: Lifecycle,
    pub title_code: StatsWording,
    pub description_code: StatsWording,
    pub license_notes: Vec<StatsWording>,
    pub known_limitations: Vec<StatsWording>,
    /// `require_runnable` with this build (decision 76).
    pub runnable: bool,
}

/// One level of a group column: its index (what a spec's `levels` names),
/// its label (the user's own text) and its non-missing cells.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GroupLevel {
    pub index: u32,
    pub label: String,
    pub count: u64,
}

/// A column's levels, in the t-test's order (decision 81).
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GroupLevelsView {
    pub slot: u64,
    pub column: Alias,
    pub levels: Vec<GroupLevel>,
}

fn run_state_view(s: RunSnapshot<TTestCode>) -> TTestRunStateView {
    TTestRunStateView {
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
    tracing::info!(code = code.wire(), "stats_ttest: refused");
    GaplyError::Validation(serde_json::json!({ "code": code.wire() }).to_string())
}

/// A t-test refusal: `{"code":"stats.ttest.unknown_column","params":…}`.
fn refuse_ttest(d: Diagnostic<TTestCode>) -> GaplyError {
    tracing::info!(code = d.code.wire(), "stats_ttest: refused");
    GaplyError::Validation(serde_json::to_string(&d).expect("a diagnostic serialises"))
}

/// The capability registry's check, its refusal returned as it is
/// (decision 74): `{"code":"capability.version_mismatch","params":…}`.
pub fn admit(spec: &OperationSpec<TTestParams>) -> Result<(), GaplyError> {
    stats_adapters::ttest_admit(spec, crate::workspace::build()).map_err(refuse_capability)
}

/* ------------------------------------------------------------------------ *
 *  The commands' bodies: what each command runs, and what the tests drive
 * ------------------------------------------------------------------------ */

pub fn validate(
    slots: &DatasetSlots,
    slot: u64,
    spec: &OperationSpec<TTestParams>,
) -> Result<ValidationReportView, GaplyError> {
    admit(spec)?;
    let held = slots.get(slot).map_err(refuse_run)?;
    let r = stats_adapters::ttest_validate(spec, crate::workspace::build(), held.dataset());
    Ok(ValidationReportView {
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
    spec: OperationSpec<TTestParams>,
) -> Result<RunStartedView, GaplyError> {
    admit(&spec)?;
    let held = slots.get(slot).map_err(refuse_run)?;
    let id = runs.start_ttest(held, spec).map_err(|n| match n {
        NotStarted::Busy => refuse_run(RunCode::RunBusy),
        // The workspace was left between the check and the start.
        NotStarted::Sealed => refuse_inactive(),
    })?;
    Ok(RunStartedView { run_id: id.0 })
}

pub fn status(runs: &Runs, run_id: u64) -> Result<TTestRunStateView, GaplyError> {
    runs.ttest_snapshot(RunId(run_id))
        .map(run_state_view)
        .map_err(refuse_run)
}

pub fn result(runs: &Runs, run_id: u64) -> Result<TTestResultView, GaplyError> {
    let b = runs.ttest_result(RunId(run_id)).map_err(refuse_run)?;
    Ok(TTestResultView {
        run_id: b.run_id.0,
        status: b.status,
        result: b.primary_outputs,
        diagnostics: b.diagnostics,
        manifest: b.manifest,
    })
}

pub fn cancel(runs: &Runs, run_id: u64) -> Result<TTestRunStateView, GaplyError> {
    runs.ttest_cancel(RunId(run_id))
        .map(run_state_view)
        .map_err(refuse_run)
}

pub fn export(runs: &Runs, run_id: u64, format: ExportFormat) -> Result<ExportView, GaplyError> {
    let b = runs.ttest_result(RunId(run_id)).map_err(refuse_run)?;
    let bytes = match format {
        ExportFormat::Csv => result_csv(&b.primary_outputs),
        ExportFormat::ManifestJson => manifest_json(&b.manifest),
    };
    let sha256 = ContentHash::of_bytes(&bytes).as_hex().to_string();
    Ok(ExportView { bytes, sha256 })
}

/// `column`'s levels as the t-test indexes them, through the core's own
/// level function (decision 81).
pub fn group_levels(
    slots: &DatasetSlots,
    slot: u64,
    column: Alias,
) -> Result<GroupLevelsView, GaplyError> {
    let held = slots.get(slot).map_err(refuse_run)?;
    let levels = stats_ttest::group_levels(held.dataset(), column).ok_or_else(|| {
        refuse_ttest(
            Diagnostic::error(TTestCode::UnknownColumn)
                .int(ParamKey::Column, i64::from(column.0 .0)),
        )
    })?;
    // Decision 87: a column with more levels than a group may is refused
    // before its values are listed, so a measurement never reaches the
    // screen as a list.
    if levels.len() > stats_ttest::MAX_GROUP_LEVELS {
        return Err(refuse_ttest(
            Diagnostic::error(TTestCode::TooManyLevels)
                .int(ParamKey::Count, levels.len() as i64)
                .int(ParamKey::Limit, stats_ttest::MAX_GROUP_LEVELS as i64),
        ));
    }
    tracing::info!(levels = levels.len(), "stats_ttest: group levels read");
    Ok(GroupLevelsView {
        slot,
        column,
        levels: levels
            .into_iter()
            .enumerate()
            .map(|(i, (label, count))| GroupLevel {
                index: u32::try_from(i).unwrap_or(u32::MAX),
                label,
                count,
            })
            .collect(),
    })
}

/// Every Statistics capability, in the registry's order, with whether this
/// build may run it.
pub fn capabilities() -> Vec<CapabilityView> {
    let build = crate::workspace::build();
    stats_registry::Registry::statistics()
        .entries()
        .iter()
        .map(|e| {
            let d = &e.descriptor;
            CapabilityView {
                id: d.id.clone(),
                version: d.version,
                lifecycle: d.lifecycle,
                title_code: d.title_code,
                description_code: d.description_code,
                license_notes: d.license_notes.clone(),
                known_limitations: d.known_limitation_codes.clone(),
                runnable: stats_registry::require_runnable(&d.id, d.version, build).is_ok(),
            }
        })
        .collect()
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
        .map_err(|_| GaplyError::Internal("stats_ttest task panicked".into()))?
}

#[tauri::command]
#[tracing::instrument(skip_all)]
pub async fn stats_ttest_validate(
    state: St<'_>,
    slot: u64,
    spec: OperationSpec<TTestParams>,
) -> Result<ValidationReportView, GaplyError> {
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
pub async fn stats_ttest_run(
    state: St<'_>,
    slot: u64,
    spec: OperationSpec<TTestParams>,
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
pub async fn stats_ttest_status(
    state: St<'_>,
    run_id: u64,
) -> Result<TTestRunStateView, GaplyError> {
    let engine = state
        .workspace
        .require_statistics()
        .map_err(refuse_workspace)?;
    status(&engine.runs, run_id)
}

#[tauri::command]
#[tracing::instrument(skip_all)]
pub async fn stats_ttest_result(state: St<'_>, run_id: u64) -> Result<TTestResultView, GaplyError> {
    let engine = state
        .workspace
        .require_statistics()
        .map_err(refuse_workspace)?;
    result(&engine.runs, run_id)
}

#[tauri::command]
#[tracing::instrument(skip_all)]
pub async fn stats_ttest_cancel(
    state: St<'_>,
    run_id: u64,
) -> Result<TTestRunStateView, GaplyError> {
    let engine = state
        .workspace
        .require_statistics()
        .map_err(refuse_workspace)?;
    cancel(&engine.runs, run_id)
}

#[tauri::command]
#[tracing::instrument(skip_all)]
pub async fn stats_ttest_export(
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

#[tauri::command]
#[tracing::instrument(skip_all)]
pub async fn stats_ttest_group_levels(
    state: St<'_>,
    slot: u64,
    column: Alias,
) -> Result<GroupLevelsView, GaplyError> {
    let engine = state
        .workspace
        .require_statistics()
        .map_err(refuse_workspace)?;
    let slots = engine.datasets.clone();
    blocking(move || group_levels(&slots, slot, column)).await
}

/// Exempt from the workspace check: it reads only the static registry
/// (decision 76).
#[tauri::command]
#[tracing::instrument(skip_all)]
pub async fn capability_list() -> Result<Vec<CapabilityView>, GaplyError> {
    Ok(capabilities())
}
