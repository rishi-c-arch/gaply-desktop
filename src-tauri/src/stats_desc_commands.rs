//! **The descriptive summary's commands (Research OS, Phase 2B): thin
//! wrappers over `gaply_core::stats_desc` and the run manager in
//! [`crate::runs`].**
//!
//! Each command:
//! * carries `#[tracing::instrument(skip_all)]`, so no argument, a path
//!   above all, reaches a span (decision 8);
//! * returns a view, or a refusal that is a closed code in JSON with numeric
//!   parameters, and never a path;
//! * holds no logic of its own: profile, validate, describe and manifest are
//!   `stats_desc`'s, the run's states are `contracts::transition`'s.
//!
//! `stats_desc_open` reads the file through Level 1's ingest,
//! [`load_dataset`]: the regular-file check, one open, a bounded read, the
//! counted inflation of an archive and the caps. There is no second ingest
//! path, and a refusal there is Level 1's own code
//! (`stats_l1::codes::l1_ingest_refusal`). The ingest writes nothing to the
//! database (measured for Phase 2B: `check_file` and `dataset_from_checked`
//! take no `Database`; `import_guard::record_rate`, the one writer of the
//! import-guard rate, is called only by the citation-link and open-access
//! imports).
//!
//! Since Research OS Phase 5B-2 (decision 123), `stats_desc_open` reads
//! through [`load_dataset_keeping_bytes`], the same ingest, and the slot holds
//! the bytes it read, with their SHA-256 (the fingerprint's) and their format,
//! for as long as the dataset is open: a project Save copies those bytes and
//! no others. They never cross IPC.
//!
//! The file's display name, its base name, crosses into [`DatasetView`] only
//! (decision 4). The manifest holds hashes and counts (decision 12). Export
//! returns bytes, and Rust writes nothing: the screen's save dialog writes
//! the one file the user names (decision 5, ADR-004a).
//!
//! **The workspace check comes first (Research OS, Phase 3, decision 27).**
//! Every command's first statement is
//! `state.workspace.require_statistics()`: unless Statistics is the active
//! workspace, the command is refused with `workspace.inactive` before
//! anything else runs, and the run records and slots it would use do not
//! exist. `validate` and `run` then refuse a spec whose `workspaceId`, or
//! whose capability's workspace, is not the active one, however it was
//! built (`workspace.wrong_workspace`). `tests/workspace_guards.rs` reads
//! `lib.rs`'s handler list and fails if a registered `stats_desc_*` command
//! does not start with the check.
//!
//! **Research OS, Phase 8F-1 (decisions 280, 284 and 286).** The open's body
//! is [`open_with`], which takes its limits last and an optional counter for
//! its rows read. `stats_desc_open` takes an optional `progressId`: while that
//! open runs, `stats_desc_open_progress` answers its rows read, a count only.
//! `stats_desc_limits` answers this computer's memory tier and the methods'
//! limits for it, numbers and closed words only, never the memory itself.
//!
//! **Phase 8F-2 (decisions 288 to 290) switched them on.** `stats_desc_open`
//! passes `IngestLimits::this_computer()` (on an 8 GB Mac: 100,000 rows,
//! 128 columns, 5,000,000 cells, 64 MiB), and the rerun does too; [`open`],
//! the tests' wrapper, keeps `IngestLimits::METHODS`
//! (`tests/ingest_limits_guard.rs`). Each method's screen reads
//! `stats_desc_limits` once for its limits line, and polls
//! `stats_desc_open_progress` during an open for its progress line.

use std::path::Path;
use std::sync::atomic::AtomicU64;
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use gaply_core::contracts::codes::{Closed, RunCode};
use gaply_core::contracts::{
    ContentHash, Diagnostic, InputRef, OperationSpec, ResourceEstimate, RunId, RunManifest,
    RunState, ValidationStatus,
};
use gaply_core::stats_adapters;
use gaply_core::stats_desc::export::{manifest_json, result_csv};
use gaply_core::stats_desc::{self, ColumnResult, DatasetProfile, DescCode, DescribeParams};
use gaply_core::stats_l1::codes::{l1_ingest_refusal, WireCode};
use gaply_core::GaplyError;

use gaply_core::contracts::workspace::check_spec;
use gaply_core::contracts::WorkspaceId;

use gaply_core::research_project::DataFormat;

use crate::memory_tier::{self, MemoryReading, MethodLimits};
use crate::runs::{
    DatasetSlots, NotStarted, OpenProgress, ProgressEntry, RunSnapshot, Runs, SourceFile,
};
use crate::stats_l1_ingest::load_dataset_keeping_bytes_with_progress;
use crate::supplementary::IngestLimits;
use crate::workspace::{refuse as refuse_workspace, refuse_inactive};

/* ------------------------------------------------------------------------ *
 *  Views
 * ------------------------------------------------------------------------ */

/// An opened dataset, for the user's own screen: its slot, the file's base
/// name, the profile (headers, level labels and counts) and the input it
/// becomes in a spec. Never a path, never a row.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DatasetView {
    pub slot: u64,
    pub display_name: String,
    pub profile: DatasetProfile,
    pub input_ref: InputRef,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ValidationReportView {
    pub status: ValidationStatus,
    pub errors: Vec<Diagnostic<DescCode>>,
    pub warnings: Vec<Diagnostic<DescCode>>,
    pub missing_information: Vec<Diagnostic<DescCode>>,
    pub estimated_resources: ResourceEstimate,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RunStartedView {
    pub run_id: u64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RunStateView {
    pub run_id: u64,
    pub state: RunState,
    pub terminal: bool,
    pub columns_done: u64,
    pub columns_total: u64,
    pub elapsed_ms: u64,
    pub cancel_requested: bool,
    pub diagnostics: Vec<Diagnostic<DescCode>>,
}

/// A succeeded run's result: the table (headers and level labels are the
/// user's own), the validation's warnings, and the manifest (hashes and
/// counts only).
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResultBundleView {
    pub run_id: u64,
    pub status: RunState,
    pub columns: Vec<ColumnResult>,
    pub diagnostics: Vec<Diagnostic<DescCode>>,
    pub manifest: RunManifest<DescCode>,
}

/// The bytes of an export and their SHA-256. Rust writes nothing.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportView {
    pub bytes: Vec<u8>,
    pub sha256: String,
}

/// This computer's memory tier and the methods' limits for it (Research OS,
/// Phase 8F-1, decision 284): the tier's word (`8gb`, `16gb` or `32gb`),
/// whether the memory was read, and the numbers of the tier's
/// `IngestLimits`. Never the memory itself.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MethodLimitsView {
    pub tier: &'static str,
    pub memory_known: bool,
    pub max_rows: u64,
    pub max_columns: u64,
    pub max_cells: u64,
    pub max_file_bytes: u64,
    pub max_text_bytes: u64,
    pub max_workbook_bytes: u64,
}

/// An open in flight's rows read (decision 280). A count only.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenProgressView {
    pub rows_read: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ExportFormat {
    Csv,
    ManifestJson,
}

fn run_state_view(s: RunSnapshot) -> RunStateView {
    RunStateView {
        run_id: s.record.run_id.0,
        state: s.record.state,
        terminal: s.record.state.is_terminal(),
        columns_done: s.columns_done as u64,
        columns_total: s.columns_total as u64,
        elapsed_ms: s.elapsed_ms,
        cancel_requested: s.cancel_requested,
        diagnostics: s.diagnostics,
    }
}

/* ------------------------------------------------------------------------ *
 *  Refusals
 * ------------------------------------------------------------------------ */

/// A run-manager refusal: `{"code":"run.unknown_run"}`.
fn refuse_run(code: RunCode) -> GaplyError {
    tracing::info!(code = code.wire(), "stats_desc: refused");
    GaplyError::Validation(serde_json::json!({ "code": code.wire() }).to_string())
}

/// An ingest refusal, in Level 1's words: `{"code":"too_large","bytes":…}`.
fn refuse_ingest(code: WireCode) -> GaplyError {
    tracing::info!(code = code.code, "stats_desc: file refused");
    GaplyError::Validation(serde_json::to_string(&code).expect("a code serialises"))
}

/// The file's base name, never a directory.
fn display_name(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/* ------------------------------------------------------------------------ *
 *  The commands' bodies: what each command runs, and what the tests drive
 * ------------------------------------------------------------------------ */

/// [`open_with`] under [`IngestLimits::METHODS`], with no counter: what the
/// tests drive.
pub fn open(slots: &DatasetSlots, path: &Path) -> Result<DatasetView, GaplyError> {
    open_with(slots, path, None, IngestLimits::METHODS)
}

/// The open's body (Research OS, Phase 8F-1, decision 286): the file read
/// through the ingest under `limits`, its rows read stored into `progress`
/// while it is parsed, profiled and held in a slot. `stats_desc_open` passes
/// `IngestLimits::this_computer()` (Phase 8F-2, decision 288) and [`open`]
/// `IngestLimits::METHODS` (`tests/ingest_limits_guard.rs`).
pub fn open_with(
    slots: &DatasetSlots,
    path: &Path,
    progress: Option<&AtomicU64>,
    limits: IngestLimits,
) -> Result<DatasetView, GaplyError> {
    let (dataset, bytes, ext) = load_dataset_keeping_bytes_with_progress(path, progress, limits)
        .map_err(|r| refuse_ingest(l1_ingest_refusal(r)))?;
    let display_name = display_name(path);
    let profile = stats_desc::profile(&dataset);
    let input_ref = stats_desc::input_ref(&dataset);
    let (rows, columns) = (profile.rows, profile.columns.len());
    tracing::info!(rows, columns, "stats_desc: dataset opened");
    // The fingerprint is the SHA-256 of exactly these bytes, taken at ingest;
    // the ingest's extensions are the store's data formats, word for word.
    let source = DataFormat::from_wire(&ext)
        .map(|format| SourceFile::new(bytes, input_ref.content_hash.clone(), format));
    let slot = slots.insert_with_source(display_name.clone(), dataset, source);
    Ok(DatasetView {
        slot,
        display_name,
        profile,
        input_ref,
    })
}

pub fn validate(
    slots: &DatasetSlots,
    slot: u64,
    spec: &OperationSpec<DescribeParams>,
) -> Result<ValidationReportView, GaplyError> {
    let held = slots.get(slot).map_err(refuse_run)?;
    // The capability registry first, through `stats.describe`'s façade
    // (Research OS, Phase 4A): a spec it refuses is `Unsupported` with
    // `stats.desc.unknown_capability` or `capability_version_mismatch`.
    let r = stats_adapters::describe_validate(spec, crate::workspace::build(), held.dataset());
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
    spec: OperationSpec<DescribeParams>,
) -> Result<RunStartedView, GaplyError> {
    let held = slots.get(slot).map_err(refuse_run)?;
    let id = runs.start(held, spec).map_err(|n| match n {
        NotStarted::Busy => refuse_run(RunCode::RunBusy),
        // The workspace was left between the check and the start.
        NotStarted::Sealed => refuse_inactive(),
    })?;
    Ok(RunStartedView { run_id: id.0 })
}

pub fn status(runs: &Runs, run_id: u64) -> Result<RunStateView, GaplyError> {
    runs.snapshot(RunId(run_id))
        .map(run_state_view)
        .map_err(refuse_run)
}

pub fn result(runs: &Runs, run_id: u64) -> Result<ResultBundleView, GaplyError> {
    let b = runs.result(RunId(run_id)).map_err(refuse_run)?;
    Ok(ResultBundleView {
        run_id: b.run_id.0,
        status: b.status,
        columns: b.primary_outputs.columns,
        diagnostics: b.diagnostics,
        manifest: b.manifest,
    })
}

pub fn cancel(runs: &Runs, run_id: u64) -> Result<RunStateView, GaplyError> {
    runs.cancel(RunId(run_id))
        .map(run_state_view)
        .map_err(refuse_run)
}

pub fn export(runs: &Runs, run_id: u64, format: ExportFormat) -> Result<ExportView, GaplyError> {
    let b = runs.result(RunId(run_id)).map_err(refuse_run)?;
    let bytes = match format {
        ExportFormat::Csv => result_csv(&b.primary_outputs),
        ExportFormat::ManifestJson => manifest_json(&b.manifest),
    };
    let sha256 = ContentHash::of_bytes(&bytes).as_hex().to_string();
    Ok(ExportView { bytes, sha256 })
}

pub fn close(slots: &DatasetSlots, slot: u64) -> bool {
    slots.close(slot)
}

/// The view of a tier's limits (decision 284). Pure, so each tier's wire
/// fixture is built from an injected reading.
pub fn limits_view(reading: MemoryReading) -> MethodLimitsView {
    let l = MethodLimits::for_tier(reading.tier).ingest;
    MethodLimitsView {
        tier: reading.tier.word(),
        memory_known: reading.memory_known,
        max_rows: l.max_rows as u64,
        max_columns: l.max_cols as u64,
        max_cells: l.max_cells as u64,
        max_file_bytes: l.max_file_size_bytes,
        max_text_bytes: l.max_total_chars as u64,
        max_workbook_bytes: l.max_workbook_uncompressed_bytes,
    }
}

/// The rows read by the open in flight with `id`, or `None` when no open has
/// that id (decision 280).
pub fn open_progress(progress: &OpenProgress, id: u64) -> Option<OpenProgressView> {
    progress
        .rows_read(id)
        .map(|rows_read| OpenProgressView { rows_read })
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
        .map_err(|_| GaplyError::Internal("stats_desc task panicked".into()))?
}

#[tauri::command]
#[tracing::instrument(skip_all)]
pub async fn stats_desc_open(
    state: St<'_>,
    path: String,
    progress_id: Option<u64>,
) -> Result<DatasetView, GaplyError> {
    let engine = state
        .workspace
        .require_statistics()
        .map_err(refuse_workspace)?;
    let slots = engine.datasets.clone();
    let progress = engine.open_progress.clone();
    blocking(move || {
        // Removed when this closure returns, by the entry being dropped.
        let entry = progress_id.map(|id| progress.begin(id));
        open_with(
            &slots,
            Path::new(&path),
            entry.as_ref().map(ProgressEntry::counter),
            IngestLimits::this_computer(),
        )
    })
    .await
}

/// This computer's memory tier and the methods' limits for it (decision
/// 284). Each method's screen reads it once, for its limits line (Phase
/// 8F-2, decision 289).
#[tauri::command]
#[tracing::instrument(skip_all)]
pub async fn stats_desc_limits(state: St<'_>) -> Result<MethodLimitsView, GaplyError> {
    let _engine = state
        .workspace
        .require_statistics()
        .map_err(refuse_workspace)?;
    Ok(limits_view(memory_tier::this_computer()))
}

/// The rows read by the open in flight with `progress_id`, or `null` when
/// no open has that id (decision 280). A screen polls it every 250 ms during
/// an open, for its progress line (Phase 8F-2, decision 290).
#[tauri::command]
#[tracing::instrument(skip_all)]
pub async fn stats_desc_open_progress(
    state: St<'_>,
    progress_id: u64,
) -> Result<Option<OpenProgressView>, GaplyError> {
    let engine = state
        .workspace
        .require_statistics()
        .map_err(refuse_workspace)?;
    Ok(open_progress(&engine.open_progress, progress_id))
}

#[tauri::command]
#[tracing::instrument(skip_all)]
pub async fn stats_desc_validate(
    state: St<'_>,
    slot: u64,
    spec: OperationSpec<DescribeParams>,
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
pub async fn stats_desc_run(
    state: St<'_>,
    slot: u64,
    spec: OperationSpec<DescribeParams>,
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
pub async fn stats_desc_status(state: St<'_>, run_id: u64) -> Result<RunStateView, GaplyError> {
    let engine = state
        .workspace
        .require_statistics()
        .map_err(refuse_workspace)?;
    status(&engine.runs, run_id)
}

#[tauri::command]
#[tracing::instrument(skip_all)]
pub async fn stats_desc_result(state: St<'_>, run_id: u64) -> Result<ResultBundleView, GaplyError> {
    let engine = state
        .workspace
        .require_statistics()
        .map_err(refuse_workspace)?;
    result(&engine.runs, run_id)
}

#[tauri::command]
#[tracing::instrument(skip_all)]
pub async fn stats_desc_cancel(state: St<'_>, run_id: u64) -> Result<RunStateView, GaplyError> {
    let engine = state
        .workspace
        .require_statistics()
        .map_err(refuse_workspace)?;
    cancel(&engine.runs, run_id)
}

#[tauri::command]
#[tracing::instrument(skip_all)]
pub async fn stats_desc_export(
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
pub async fn stats_desc_close(state: St<'_>, slot: u64) -> Result<bool, GaplyError> {
    let engine = state
        .workspace
        .require_statistics()
        .map_err(refuse_workspace)?;
    Ok(close(&engine.datasets, slot))
}
