//! **Scale scores' commands (Research OS, Phase 8M-3, decisions 424 to 426):
//! thin wrappers over `gaply_core::stats_prep`, through its façade, and the
//! slots in [`crate::runs`].**
//!
//! Each command is as the methods' (`stats_alpha_commands.rs`):
//! * it carries `#[tracing::instrument(skip_all)]`, so no argument reaches a
//!   span (decision 8), and no line here logs a name, a path or a value;
//! * it checks, first, that Statistics is the active workspace
//!   (`workspace.inactive`), then the capability's workspace and the registry,
//!   whose `capability.*` refusal it returns as it is (decision 74);
//! * it returns a view, or a refusal that is a closed code in JSON with numeric
//!   parameters, and never a path;
//! * it holds no logic of its own: the recipe's rules, the build and the CSV
//!   are `stats_prep`'s, reached only through `stats_adapters`.
//!
//! **There is no run, so no run kind, no ceiling and no cancel.** Measured for
//! decision 424 at each tier's largest table: the build itself 0.02 to 0.17 s,
//! the whole make 1.8 to 5.2 s, nearly all of it the profile of the whole table.
//! Since decision 498 the slot keeps the open's profile of the file's columns
//! and a make profiles only the new ones: measured by a release probe at
//! 100,000 x 49 with one score of 49 items, the make's median went from 1.47 s
//! to 0.08 s, and with 40 numeric and 10 text columns from 1.41 s to 0.11 s,
//! the view's bytes unchanged. Making, replacing and clearing are refused with
//! `run.run_busy` while any run is unfinished. A run keeps the handle it started
//! with.
//!
//! **A refused make changes nothing.** It works from what the slot holds, or a
//! copy of the file's table rebuilt from it, and the new slot replaces the old
//! only after the build has succeeded (decision 424). So a replace holds up to
//! three tables at its peak. A recipe a report refuses comes back as a report
//! with no dataset, as a method's validation does, and is not an error.
//!
//! **Nothing here writes a file.** The CSV comes back as bytes, and the screen's
//! save dialog writes the one file the user names (decision 5, ADR-004a).

use std::sync::Arc;

use serde::Serialize;

use gaply_core::contracts::codes::{Closed, RunCode};
use gaply_core::contracts::workspace::check_spec;
use gaply_core::contracts::{
    ContentHash, Diagnostic, ResourceEstimate, ValidationReport, ValidationStatus, WorkspaceId,
};
use gaply_core::stats_adapters;
use gaply_core::stats_prep::{PrepCode, PrepRefusal, Recipe, ScoreCounts};
use gaply_core::GaplyError;

use crate::runs::{DatasetSlots, Runs, Slot};
use crate::stats_desc_commands::{view_of, DatasetView, ExportView};
use crate::supplementary::IngestLimits;
use crate::workspace::{refuse as refuse_workspace, refuse_capability};

/* ------------------------------------------------------------------------ *
 *  Views
 * ------------------------------------------------------------------------ */

/// A make's answer: the report, each score's counts, and the prepared view
/// when the slot now holds it. A refused make has no `dataset`, and any counts
/// are of the score no row can have.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PrepMadeView {
    pub status: ValidationStatus,
    pub errors: Vec<Diagnostic<PrepCode>>,
    pub warnings: Vec<Diagnostic<PrepCode>>,
    pub missing_information: Vec<Diagnostic<PrepCode>>,
    pub estimated_resources: ResourceEstimate,
    pub counts: Vec<ScoreCounts>,
    pub dataset: Option<DatasetView>,
}

/// The recipe a slot holds and its counts. It holds the names the user typed,
/// for the user's own screen.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PrepRecipeView {
    pub recipe: Recipe,
    pub counts: Vec<ScoreCounts>,
}

fn made(
    report: ValidationReport<PrepCode>,
    counts: Vec<ScoreCounts>,
    dataset: Option<DatasetView>,
) -> PrepMadeView {
    PrepMadeView {
        status: report.status,
        errors: report.errors,
        warnings: report.warnings,
        missing_information: report.missing_information,
        estimated_resources: report.estimated_resources,
        counts,
        dataset,
    }
}

/* ------------------------------------------------------------------------ *
 *  Refusals
 * ------------------------------------------------------------------------ */

/// A run-manager refusal: `{"code":"run.run_busy"}`.
fn refuse_run(code: RunCode) -> GaplyError {
    tracing::info!(code = code.wire(), "stats_prep: refused");
    GaplyError::Validation(serde_json::json!({ "code": code.wire() }).to_string())
}

/// The capability registry's check on a recipe, its refusal returned as it is
/// (decision 74).
pub fn admit(recipe: &Recipe) -> Result<(), GaplyError> {
    stats_adapters::prep_admit(recipe, crate::workspace::build()).map_err(refuse_capability)
}

/// The registry's check for a command that takes no recipe.
fn runnable() -> Result<(), GaplyError> {
    stats_adapters::prep_runnable(crate::workspace::build()).map_err(refuse_capability)
}

/// Making, replacing and clearing wait for no run: they are refused while one
/// is unfinished, as a second run is (decision 424).
fn not_busy(runs: &Runs) -> Result<(), GaplyError> {
    match runs.unfinished() {
        Some(_) => Err(refuse_run(RunCode::RunBusy)),
        None => Ok(()),
    }
}

/* ------------------------------------------------------------------------ *
 *  The commands' bodies: what each command runs, and what the tests drive
 * ------------------------------------------------------------------------ */

/// Makes or replaces the slot's recipe, always from the file's table: what the
/// slot holds, or a copy rebuilt from its prepared table. `cells_cap` is last,
/// as the open takes its limits (decision 286); `stats_prep_make` passes this
/// computer's. The slot stays findable by every other command throughout, and
/// is replaced only once the build has succeeded.
pub fn make(
    runs: &Runs,
    slots: &DatasetSlots,
    slot: u64,
    recipe: &Recipe,
    cells_cap: u64,
) -> Result<PrepMadeView, GaplyError> {
    admit(recipe)?;
    not_busy(runs)?;
    let held = slots.get(slot).map_err(refuse_run)?;
    let build = crate::workspace::build();
    let rebuilt = held.prepared().map(|_| original_of(&held));
    let table = rebuilt.as_ref().unwrap_or_else(|| held.dataset());

    let report = stats_adapters::prep_validate(recipe, build, table, cells_cap);
    if !report.may_run() {
        return Ok(made(report, Vec::new(), None));
    }
    let prepared = match stats_adapters::prep_build(recipe, build, table, cells_cap) {
        Ok(prepared) => prepared,
        Err(PrepRefusal::Report(report)) => return Ok(made(report, Vec::new(), None)),
        Err(PrepRefusal::NoRowScored { column, counts }) => {
            let refused = ValidationReport::new(
                Vec::new(),
                vec![Diagnostic::error(PrepCode::NoRowScored).with(
                    gaply_core::contracts::ParamKey::Column,
                    gaply_core::contracts::ParamValue::Int(i64::from(column.0 .0)),
                )],
                Vec::new(),
                report.warnings,
                report.estimated_resources,
            );
            return Ok(made(refused, vec![counts], None));
        }
        Err(PrepRefusal::Table(diagnostic)) => {
            let refused = ValidationReport::new(
                Vec::new(),
                vec![diagnostic],
                Vec::new(),
                report.warnings,
                report.estimated_resources,
            );
            return Ok(made(refused, Vec::new(), None));
        }
    };
    drop(rebuilt);
    let counts = prepared.counts.clone();
    let next = slots.hold_prepared(&held, prepared).map_err(refuse_run)?;
    tracing::info!(
        rows = next.dataset().rows(),
        columns = next.dataset().schema.columns.len(),
        "stats_prep: recipe held"
    );
    Ok(made(report, counts, Some(view_of(&next))))
}

/// The file's table again, from `held`'s prepared table.
fn original_of(held: &Arc<Slot>) -> gaply_core::stats_l1::dataset::Dataset {
    stats_adapters::prep_original(held.dataset(), held.original_fingerprint().clone())
}

/// Clears the slot's recipe, the slot back on the file's table, and returns
/// its view. A slot with no recipe is left as it is.
pub fn clear(runs: &Runs, slots: &DatasetSlots, slot: u64) -> Result<DatasetView, GaplyError> {
    runnable()?;
    not_busy(runs)?;
    let held = slots.get(slot).map_err(refuse_run)?;
    if held.prepared().is_none() {
        return Ok(view_of(&held));
    }
    let next = slots
        .hold_original(&held, original_of(&held))
        .map_err(refuse_run)?;
    tracing::info!(
        rows = next.dataset().rows(),
        columns = next.dataset().schema.columns.len(),
        "stats_prep: recipe cleared"
    );
    Ok(view_of(&next))
}

/// The recipe the slot holds and its counts, or `None` when it holds none.
pub fn recipe(slots: &DatasetSlots, slot: u64) -> Result<Option<PrepRecipeView>, GaplyError> {
    runnable()?;
    let held = slots.get(slot).map_err(refuse_run)?;
    Ok(held.prepared().map(|p| PrepRecipeView {
        recipe: p.recipe.clone(),
        counts: p.counts.clone(),
    }))
}

/// The prepared data's CSV bytes and their SHA-256 (decision 426), from the
/// one copy the slot holds. A slot with no recipe has no prepared data:
/// `run.unknown_input`. Rust writes nothing.
pub fn csv(slots: &DatasetSlots, slot: u64) -> Result<ExportView, GaplyError> {
    runnable()?;
    let held = slots.get(slot).map_err(refuse_run)?;
    let prepared = held
        .prepared()
        .ok_or_else(|| refuse_run(RunCode::UnknownInput))?;
    let bytes = stats_adapters::prep_csv(prepared);
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
        .map_err(|_| GaplyError::Internal("stats_prep task panicked".into()))?
}

#[tauri::command]
#[tracing::instrument(skip_all)]
pub async fn stats_prep_make(
    state: St<'_>,
    slot: u64,
    recipe: Recipe,
) -> Result<PrepMadeView, GaplyError> {
    let engine = state
        .workspace
        .require_statistics()
        .map_err(refuse_workspace)?;
    check_spec(
        WorkspaceId::Statistics,
        WorkspaceId::Statistics,
        &recipe.capability_id,
    )
    .map_err(refuse_workspace)?;
    let (runs, slots) = (engine.runs.clone(), engine.datasets.clone());
    blocking(move || {
        make(
            &runs,
            &slots,
            slot,
            &recipe,
            IngestLimits::this_computer().max_cells as u64,
        )
    })
    .await
}

#[tauri::command]
#[tracing::instrument(skip_all)]
pub async fn stats_prep_clear(state: St<'_>, slot: u64) -> Result<DatasetView, GaplyError> {
    let engine = state
        .workspace
        .require_statistics()
        .map_err(refuse_workspace)?;
    let (runs, slots) = (engine.runs.clone(), engine.datasets.clone());
    blocking(move || clear(&runs, &slots, slot)).await
}

#[tauri::command]
#[tracing::instrument(skip_all)]
pub async fn stats_prep_recipe(
    state: St<'_>,
    slot: u64,
) -> Result<Option<PrepRecipeView>, GaplyError> {
    let engine = state
        .workspace
        .require_statistics()
        .map_err(refuse_workspace)?;
    recipe(&engine.datasets, slot)
}

#[tauri::command]
#[tracing::instrument(skip_all)]
pub async fn stats_prep_csv(state: St<'_>, slot: u64) -> Result<ExportView, GaplyError> {
    let engine = state
        .workspace
        .require_statistics()
        .map_err(refuse_workspace)?;
    let slots = engine.datasets.clone();
    blocking(move || csv(&slots, slot)).await
}
