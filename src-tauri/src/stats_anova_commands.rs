//! **The one-way ANOVA's commands (Research OS, Phase 8G-2, decision 318):
//! thin wrappers over `gaply_core::stats_anova`, through its façade, and the
//! run manager in [`crate::runs`].**
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
//! * it holds no logic of its own: validate, run, check points and manifest
//!   are `stats_anova`'s, the run's states are `contracts::transition`'s.
//!
//! The run's state view carries its progress as steps: the check points done
//! and their total, `stats_anova::check_points` for the included levels
//! (decision 328), so the screen says "{done} of {total} steps", never
//! columns. The dataset is opened and closed with Phase 2B's
//! `stats_desc_open` and `stats_desc_close`, unchanged. A run of another
//! kind is refused by status, result, cancel and export with
//! `run.wrong_kind`. The manifest holds hashes and counts (decision 12).
//! Export returns bytes, and Rust writes nothing: the screen's save dialog
//! writes the one file the user names (decision 5, ADR-004a).
//!
//! `stats_anova_group_levels` (decisions 312 and 329) gives a column's levels
//! as the ANOVA indexes them, from `stats_ttest::group_levels`, the core's
//! own level function, which the ANOVA reads them through, so the screen's
//! indexes are the core's by construction. The labels appear in its view
//! only. A column with more than `stats_ttest::MAX_GROUP_LEVELS` (50) levels
//! is refused with `stats.anova.too_many_levels {count, limit}`, before any
//! label is listed.

use std::sync::Arc;

use serde::Serialize;

use gaply_core::contracts::codes::{Closed, RunCode};
use gaply_core::contracts::workspace::check_spec;
use gaply_core::contracts::{
    ContentHash, Diagnostic, OperationSpec, ParamKey, ResourceEstimate, RunId, RunManifest,
    RunState, ValidationStatus, WorkspaceId,
};
use gaply_core::stats_adapters;
use gaply_core::stats_anova::export::{manifest_json, result_csv};
use gaply_core::stats_anova::{AnovaCode, AnovaOutcome, AnovaParams};
use gaply_core::stats_desc::Alias;
use gaply_core::stats_ttest::{self, MAX_GROUP_LEVELS};
use gaply_core::GaplyError;

use crate::runs::{DatasetSlots, NotStarted, RunSnapshot, Runs};
use crate::stats_desc_commands::{ExportFormat, ExportView, RunStartedView};
use crate::stats_ttest_commands::{GroupLevel, GroupLevelsView};
use crate::workspace::{refuse as refuse_workspace, refuse_capability, refuse_inactive};

/* ------------------------------------------------------------------------ *
 *  Views
 * ------------------------------------------------------------------------ */

/// The ANOVA's validation, in its own codes.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AnovaValidationView {
    pub status: ValidationStatus,
    pub errors: Vec<Diagnostic<AnovaCode>>,
    pub warnings: Vec<Diagnostic<AnovaCode>>,
    pub missing_information: Vec<Diagnostic<AnovaCode>>,
    pub estimated_resources: ResourceEstimate,
}

/// An ANOVA run's state: the t-test's, with its progress in steps, the
/// check points done and their total (decision 328).
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AnovaRunStateView {
    pub run_id: u64,
    pub state: RunState,
    pub terminal: bool,
    pub elapsed_ms: u64,
    pub cancel_requested: bool,
    pub steps_done: u64,
    pub steps_total: u64,
    pub diagnostics: Vec<Diagnostic<AnovaCode>>,
}

/// A succeeded ANOVA: the result (the level labels are the user's own), the
/// validation's warnings, and the manifest (hashes and counts only).
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AnovaResultView {
    pub run_id: u64,
    pub status: RunState,
    pub result: AnovaOutcome,
    pub diagnostics: Vec<Diagnostic<AnovaCode>>,
    pub manifest: RunManifest<AnovaCode>,
}

fn run_state_view(s: RunSnapshot<AnovaCode>) -> AnovaRunStateView {
    AnovaRunStateView {
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
    tracing::info!(code = code.wire(), "stats_anova: refused");
    GaplyError::Validation(serde_json::json!({ "code": code.wire() }).to_string())
}

/// An ANOVA refusal: `{"code":"stats.anova.unknown_column","params":…}`.
fn refuse_anova(d: Diagnostic<AnovaCode>) -> GaplyError {
    tracing::info!(code = d.code.wire(), "stats_anova: refused");
    GaplyError::Validation(serde_json::to_string(&d).expect("a diagnostic serialises"))
}

/// The capability registry's check, its refusal returned as it is
/// (decision 74).
pub fn admit(spec: &OperationSpec<AnovaParams>) -> Result<(), GaplyError> {
    stats_adapters::anova_admit(spec, crate::workspace::build()).map_err(refuse_capability)
}

/* ------------------------------------------------------------------------ *
 *  The commands' bodies: what each command runs, and what the tests drive
 * ------------------------------------------------------------------------ */

pub fn validate(
    slots: &DatasetSlots,
    slot: u64,
    spec: &OperationSpec<AnovaParams>,
) -> Result<AnovaValidationView, GaplyError> {
    admit(spec)?;
    let held = slots.get(slot).map_err(refuse_run)?;
    let r = stats_adapters::anova_validate(spec, crate::workspace::build(), held.dataset());
    Ok(AnovaValidationView {
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
    spec: OperationSpec<AnovaParams>,
) -> Result<RunStartedView, GaplyError> {
    admit(&spec)?;
    let held = slots.get(slot).map_err(refuse_run)?;
    let id = runs.start_anova(held, spec).map_err(|n| match n {
        NotStarted::Busy => refuse_run(RunCode::RunBusy),
        // The workspace was left between the check and the start.
        NotStarted::Sealed => refuse_inactive(),
    })?;
    Ok(RunStartedView { run_id: id.0 })
}

pub fn status(runs: &Runs, run_id: u64) -> Result<AnovaRunStateView, GaplyError> {
    runs.anova_snapshot(RunId(run_id))
        .map(run_state_view)
        .map_err(refuse_run)
}

pub fn result(runs: &Runs, run_id: u64) -> Result<AnovaResultView, GaplyError> {
    let b = runs.anova_result(RunId(run_id)).map_err(refuse_run)?;
    Ok(AnovaResultView {
        run_id: b.run_id.0,
        status: b.status,
        result: b.primary_outputs,
        diagnostics: b.diagnostics,
        manifest: b.manifest,
    })
}

pub fn cancel(runs: &Runs, run_id: u64) -> Result<AnovaRunStateView, GaplyError> {
    runs.anova_cancel(RunId(run_id))
        .map(run_state_view)
        .map_err(refuse_run)
}

pub fn export(runs: &Runs, run_id: u64, format: ExportFormat) -> Result<ExportView, GaplyError> {
    let b = runs.anova_result(RunId(run_id)).map_err(refuse_run)?;
    let bytes = match format {
        ExportFormat::Csv => result_csv(&b.primary_outputs, &b.manifest),
        ExportFormat::ManifestJson => manifest_json(&b.manifest),
    };
    let sha256 = ContentHash::of_bytes(&bytes).as_hex().to_string();
    Ok(ExportView { bytes, sha256 })
}

/// `column`'s levels as the ANOVA indexes them, through the core's own level
/// function (decisions 312 and 329).
pub fn group_levels(
    slots: &DatasetSlots,
    slot: u64,
    column: Alias,
) -> Result<GroupLevelsView, GaplyError> {
    let held = slots.get(slot).map_err(refuse_run)?;
    let levels = stats_ttest::group_levels(held.dataset(), column).ok_or_else(|| {
        refuse_anova(
            Diagnostic::error(AnovaCode::UnknownColumn)
                .int(ParamKey::Column, i64::from(column.0 .0)),
        )
    })?;
    // As the t-test's (decision 87): a column with more levels than a group
    // may is refused before its values are listed.
    if levels.len() > MAX_GROUP_LEVELS {
        return Err(refuse_anova(
            Diagnostic::error(AnovaCode::TooManyLevels)
                .int(ParamKey::Count, levels.len() as i64)
                .int(ParamKey::Limit, MAX_GROUP_LEVELS as i64),
        ));
    }
    tracing::info!(levels = levels.len(), "stats_anova: group levels read");
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

/* ------------------------------------------------------------------------ *
 *  The Tauri commands
 * ------------------------------------------------------------------------ */

type St<'a> = tauri::State<'a, crate::state::AppState>;

async fn blocking<T: Send + 'static>(
    f: impl FnOnce() -> Result<T, GaplyError> + Send + 'static,
) -> Result<T, GaplyError> {
    tokio::task::spawn_blocking(f)
        .await
        .map_err(|_| GaplyError::Internal("stats_anova task panicked".into()))?
}

#[tauri::command]
#[tracing::instrument(skip_all)]
pub async fn stats_anova_validate(
    state: St<'_>,
    slot: u64,
    spec: OperationSpec<AnovaParams>,
) -> Result<AnovaValidationView, GaplyError> {
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
pub async fn stats_anova_run(
    state: St<'_>,
    slot: u64,
    spec: OperationSpec<AnovaParams>,
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
pub async fn stats_anova_status(
    state: St<'_>,
    run_id: u64,
) -> Result<AnovaRunStateView, GaplyError> {
    let engine = state
        .workspace
        .require_statistics()
        .map_err(refuse_workspace)?;
    status(&engine.runs, run_id)
}

#[tauri::command]
#[tracing::instrument(skip_all)]
pub async fn stats_anova_result(state: St<'_>, run_id: u64) -> Result<AnovaResultView, GaplyError> {
    let engine = state
        .workspace
        .require_statistics()
        .map_err(refuse_workspace)?;
    result(&engine.runs, run_id)
}

#[tauri::command]
#[tracing::instrument(skip_all)]
pub async fn stats_anova_cancel(
    state: St<'_>,
    run_id: u64,
) -> Result<AnovaRunStateView, GaplyError> {
    let engine = state
        .workspace
        .require_statistics()
        .map_err(refuse_workspace)?;
    cancel(&engine.runs, run_id)
}

#[tauri::command]
#[tracing::instrument(skip_all)]
pub async fn stats_anova_export(
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
pub async fn stats_anova_group_levels(
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
