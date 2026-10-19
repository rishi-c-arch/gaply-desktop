//! **The project commands (Research OS, Phase 5B-2; decisions 118 to 127;
//! ADR-009): the one module that calls the project store, and the only way
//! a command reaches a project.**
//!
//! No screen calls these in 5B-2; 5B-3 adds the screens, behind
//! `REACT_APP_FEATURE_PROJECTS` (decision 119). A flag only hides: the checks
//! below are the boundary. Once merged, the app's own frontend can call every
//! one of them, with or without a screen, and ADR-009's Accepted parts
//! (decisions 99 to 111) cover the writes they make.
//!
//! **Shape.** Each command's work is a plain function over the projects root
//! and the window's [`WorkspaceSession`], so a test needs no `AppHandle`. A
//! Tauri command is a thin async wrapper: it resolves the root through
//! `project_store::projects_root` and runs the work under `spawn_blocking`,
//! so saving, hashing and deleting never run on the main thread. A command
//! never takes a path or a file name from the webview.
//!
//! **The workspace check (decision 120).** Saving, opening, closing, listing
//! a project's runs, reading a run and rendering either export need
//! Statistics active: the Tauri command's first statement is the check, and
//! so is the work function's (`tests/workspace_guards.rs`). Listing
//! projects, the sizes and caps, preparing a delete, deleting and asking
//! which project is open (`project_current`, Phase 5B-3b, decision 148)
//! never need a workspace, and never check it.
//!
//! **The session (decision 122).** At most one project is open, held in
//! [`ProjectSession`] beside the active workspace. Leaving Statistics closes
//! it ([`ProjectSession::close_on_leave`], called by the workspace session).
//! Every command that writes or reads the open project holds its lock for the
//! whole operation and checks the workspace again under it, so a Save that a
//! deactivation overtakes is refused whole, and one already past the check
//! completes whole before the project is closed. The store's own
//! process-wide lock covers every Save and delete from the cap check to the
//! commit.
//!
//! **A Save (decision 123)** names a run by its id. Only a run the registry
//! still holds, that Succeeded or Failed with its code, and that this project
//! has not saved yet, is saved; every other case is its own code. The data
//! file is the copy of the original bytes the dataset slot has held since
//! ingest, and it is saved only when its SHA-256 equals the hash the run
//! recorded when it started (and the manifest's input hash). One store write
//! then holds the data file, the spec, the result bundle and the manifest
//! (or, for a failed run, its failure record), the run row and its edges. A
//! first Save makes its project, inside the project's creating folder
//! (decision 121).
//!
//! **Deleting (decision 124)** needs a confirmation issued for that project
//! just before: 32 random bytes as a [`ConfirmationId`], used once, void
//! after five minutes or when a newer one is issued, held only here, so a
//! restart voids it.
//!
//! **What reaches the webview.** Ids, hashes, closed words and numbers; the
//! project's name only in the list and the open view; a run's spec and
//! result bundle only in [`RunResultView`], read back from the open project
//! after every one of its artifacts' hashes is verified; the two exports as
//! bytes with a fixed file name (decision 125). Never a data file's bytes,
//! and never a path. A refusal is a closed code with numeric parameters.
//! Logs carry a project by its id only.

use std::collections::{BTreeSet, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Instant;

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use gaply_core::contracts::capability::ConfirmationId;
use gaply_core::contracts::codes::Closed;
use gaply_core::contracts::workspace::WorkspaceState;
use gaply_core::contracts::{
    ContentHash, Diagnostic, InputRef, OperationSpec, RunId, RunManifest, RunState, WorkspaceId,
};
use gaply_core::research_project::{
    hex_lower, ArtifactHash, ArtifactKind, ArtifactRef, Caps, ColumnRole, DataFormat, ExportCode,
    ExportCount, ExportManifest, ExportParameter, ExportRun, ProjectCode, ProjectExportInput,
    ProjectId, ProjectRefusal, ProjectTitle, ProvEdge, StoredRun, StoredRunId, TestVariant,
    UsedRole, PROV_FILE_NAME, REPORT_FILE_NAME,
};
use gaply_core::research_project::{render_prov_json, render_report_markdown};
use gaply_core::stats_desc::{self, DescCode, DescribeParams, TreatAs};
use gaply_core::stats_ttest::{self, TTestCode, TTestParams, Variant};
use gaply_core::GaplyError;

use crate::project_store::{
    projects_root, ArtifactInput, ProjectHandle, ProjectRoot, RunToSave, SaveRequest,
};
use crate::runs::{Clock, RunCopy};
use crate::workspace::{refuse as refuse_workspace, refuse_inactive, WorkspaceSession};

/// How long a delete confirmation stays good (decision 124).
pub const CONFIRMATION_LIFETIME_MS: u64 = 5 * 60 * 1000;

/// How many confirmations the session remembers, so a used or replaced one
/// is refused as such rather than as unknown.
const CONFIRMATIONS_REMEMBERED: usize = 16;

/// The most a run's result may come to when it is read back: its spec, its
/// result bundle and its manifest or failure record together. The data file
/// is never part of it.
pub const RESULT_READ_LIMIT_BYTES: u64 = 4 * 1024 * 1024;

/// The schema of a failed run's failure record.
const FAILURE_RECORD_SCHEMA: u32 = 1;

/* ------------------------------------------------------------------------ *
 *  The session
 * ------------------------------------------------------------------------ */

/// Milliseconds since the session was made.
struct Monotonic(Instant);

impl Clock for Monotonic {
    fn now_ms(&self) -> u64 {
        u64::try_from(self.0.elapsed().as_millis()).unwrap_or(u64::MAX)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Standing {
    Pending,
    Used,
    Replaced,
}

/// One confirmation issued, in memory only.
struct Issued {
    id: ConfirmationId,
    project: ProjectId,
    issued_ms: u64,
    standing: Standing,
}

/// The open project and the delete confirmations, beside the active
/// workspace (decision 122). In memory only.
pub struct ProjectSession {
    caps: Caps,
    clock: Arc<dyn Clock>,
    /// The store under the root, opened once: its start finishes what an
    /// interrupted create or delete left, which must not run beside a create.
    store: Mutex<Option<(PathBuf, Arc<ProjectRoot>)>>,
    open: Mutex<Option<ProjectHandle>>,
    book: Mutex<VecDeque<Issued>>,
    /// The runs each project has saved in this process, by the run's id:
    /// a run is never saved twice into one project (decision 123).
    saved: Mutex<BTreeSet<(ProjectId, u64)>>,
}

impl Default for ProjectSession {
    fn default() -> ProjectSession {
        ProjectSession::with(Caps::REAL, Arc::new(Monotonic(Instant::now())))
    }
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

impl ProjectSession {
    /// A session with its caps and clock given: tests pass small caps and a
    /// clock they move.
    pub fn with(caps: Caps, clock: Arc<dyn Clock>) -> ProjectSession {
        ProjectSession {
            caps,
            clock,
            store: Mutex::new(None),
            open: Mutex::new(None),
            book: Mutex::new(VecDeque::new()),
            saved: Mutex::new(BTreeSet::new()),
        }
    }

    fn store(&self, root: &Path) -> Result<Arc<ProjectRoot>, ProjectRefusal> {
        let mut held = lock(&self.store);
        if let Some((at, store)) = held.as_ref() {
            if at == root {
                return Ok(Arc::clone(store));
            }
        }
        let store = Arc::new(ProjectRoot::open_root_with(root, self.caps)?);
        *held = Some((root.to_path_buf(), Arc::clone(&store)));
        Ok(store)
    }

    /// The open project's id, if one is open.
    pub fn open_id(&self) -> Option<ProjectId> {
        lock(&self.open).as_ref().map(|h| h.id().clone())
    }

    /// Closes the open project, if any: what leaving Statistics does
    /// (decision 122). It waits for a Save holding the project to finish.
    pub fn close_on_leave(&self) {
        if let Some(h) = lock(&self.open).take() {
            tracing::info!(project = %h.id().as_str(), "project: closed with the workspace");
            h.close();
        }
    }

    /// A new confirmation for `project`; the one before it, for any
    /// project, is void from now on.
    fn issue(&self, project: &ProjectId) -> Result<ConfirmationId, ProjectRefusal> {
        let mut b = [0u8; 32];
        getrandom::getrandom(&mut b).map_err(|_| ProjectRefusal::from(ProjectCode::IoFailed))?;
        let id = ConfirmationId::parse(&hex_lower(&b)).expect("64 lowercase hex characters");
        let mut book = lock(&self.book);
        for i in book.iter_mut() {
            if i.standing == Standing::Pending {
                i.standing = Standing::Replaced;
            }
        }
        book.push_back(Issued {
            id: id.clone(),
            project: project.clone(),
            issued_ms: self.clock.now_ms(),
            standing: Standing::Pending,
        });
        while book.len() > CONFIRMATIONS_REMEMBERED {
            book.pop_front();
        }
        Ok(id)
    }

    /// Uses the confirmation for deleting `project`: refused when there is
    /// none, when it is unknown, used, replaced, expired, or for another
    /// project. Any confirmation found is used up by this call.
    fn redeem(&self, project: &ProjectId, given: Option<&str>) -> Result<(), ProjectCode> {
        let given = match given {
            None => return Err(ProjectCode::ConfirmationMissing),
            Some(g) if g.is_empty() => return Err(ProjectCode::ConfirmationMissing),
            Some(g) => g,
        };
        let now = self.clock.now_ms();
        let mut book = lock(&self.book);
        let Some(i) = book.iter_mut().find(|i| same_bytes(i.id.as_str(), given)) else {
            return Err(ProjectCode::ConfirmationUnknown);
        };
        let standing = i.standing;
        i.standing = Standing::Used;
        match standing {
            Standing::Used => Err(ProjectCode::ConfirmationUsed),
            Standing::Replaced => Err(ProjectCode::ConfirmationReplaced),
            Standing::Pending if now.saturating_sub(i.issued_ms) >= CONFIRMATION_LIFETIME_MS => {
                Err(ProjectCode::ConfirmationExpired)
            }
            Standing::Pending if &i.project != project => {
                Err(ProjectCode::ConfirmationOtherProject)
            }
            Standing::Pending => Ok(()),
        }
    }
}

/// Equal bytes, compared in time that does not depend on where they differ.
fn same_bytes(a: &str, b: &str) -> bool {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    a.len() == b.len() && a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

/* ------------------------------------------------------------------------ *
 *  Views
 * ------------------------------------------------------------------------ */

/// A refusal, as the webview reads it: `{"code":"project.not_open"}`, and
/// for a cap the bytes needed and the limit.
#[derive(Serialize)]
pub struct ProjectRefusalView {
    pub code: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub needed: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<u64>,
}

fn refuse(r: impl Into<ProjectRefusal>) -> GaplyError {
    let r = r.into();
    tracing::info!(code = r.code.wire(), "project: refused");
    let view = ProjectRefusalView {
        code: r.code.wire(),
        needed: r.needed,
        limit: r.limit,
    };
    GaplyError::Validation(serde_json::to_string(&view).expect("a code and two numbers"))
}

/// One project in the list. A project that cannot be read has its id, its
/// size and the code, and no name, so it can still be deleted.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectListEntryView {
    pub project_id: String,
    pub name: Option<String>,
    pub bytes: u64,
    pub saved_at: Option<i64>,
    pub run_count: Option<u64>,
    pub refused: Option<ProjectCode>,
}

/// What all projects hold, and both caps (decision 107).
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectSizesView {
    pub total_bytes: u64,
    pub project_count: u64,
    pub project_cap_bytes: u64,
    pub all_projects_cap_bytes: u64,
}

/// A delete confirmation and the project's size: never its name.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteConfirmationView {
    pub project_id: String,
    pub confirmation: String,
    pub bytes: u64,
    pub expires_in_ms: u64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectDeletedView {
    pub project_id: String,
    /// The project was open and was closed first.
    pub closed: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SavedRunView {
    pub project_id: String,
    pub stored_run_id: u64,
    /// This Save made the project.
    pub created: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectOpenView {
    pub project_id: String,
    pub name: String,
    pub created_at: i64,
    pub run_count: u64,
    pub bytes: u64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectClosedView {
    pub closed: bool,
}

/// The project open in this copy of the app, by its id, or none (decision
/// 148). Never its name: a screen reads that from the list.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectCurrentView {
    pub project_id: Option<String>,
}

/// A saved run, as its row holds it.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StoredRunView {
    pub stored_run_id: u64,
    pub capability_id: String,
    pub capability_version: u32,
    pub state: RunState,
    /// The store's own end code (`project.run_interrupted`), if any.
    pub end_code: Option<ProjectCode>,
    pub started_at: i64,
    pub ended_at: Option<i64>,
    pub saved_at: i64,
    pub agent_version: String,
}

/// An artifact by its row: hash, kind, format and size.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StoredArtifactView {
    pub sha256: String,
    pub kind: ArtifactKind,
    pub format: DataFormat,
    pub bytes: u64,
}

/// One saved run read back: its spec and its result bundle and manifest, or
/// its failure record, each verified against its hash first and returned as
/// the exact JSON text stored, for the webview to parse. Not re-parsed here:
/// `serde_json`'s default float parser can land one unit in the last place
/// away from the stored digits (measured on a t-test's standard deviations),
/// and JavaScript's `JSON.parse` reads them exactly. The data file is
/// described, and verified, but its bytes never leave the store.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RunResultView {
    pub stored_run_id: u64,
    pub capability_id: String,
    pub state: RunState,
    pub data_file: StoredArtifactView,
    pub spec_json: String,
    pub bundle_json: Option<String>,
    pub manifest_json: Option<String>,
    pub failure_json: Option<String>,
}

/// An export's bytes, its fixed file name and its SHA-256. Rust writes
/// nothing: 5B-3's save dialog writes the one file the user picks.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectExportView {
    pub file_name: &'static str,
    pub bytes: Vec<u8>,
    pub sha256: String,
}

/// Where a Save goes: a new project the Save makes, named by the user, or
/// the open project.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "into", rename_all = "camelCase", deny_unknown_fields)]
pub enum SaveTarget {
    NewProject {
        name: String,
    },
    OpenProject {
        #[serde(rename = "projectId")]
        project_id: String,
    },
}

/* ------------------------------------------------------------------------ *
 *  Helpers
 * ------------------------------------------------------------------------ */

type Res<T> = Result<T, GaplyError>;

/// The workspace check again, under the open project's lock: a deactivation
/// that came after the command's first check refuses the whole operation.
fn still_active(ws: &WorkspaceSession) -> Res<()> {
    if ws.state() == WorkspaceState::Active(WorkspaceId::Statistics) {
        Ok(())
    } else {
        Err(refuse_inactive())
    }
}

/// The open project, if `raw` is its id; `project.not_open` otherwise.
fn the_open<'a>(
    open: &'a mut Option<ProjectHandle>,
    raw: &str,
) -> Result<&'a mut ProjectHandle, ProjectRefusal> {
    let id = ProjectId::parse(raw)?;
    match open {
        Some(h) if h.id() == &id => Ok(h),
        _ => Err(ProjectCode::NotOpen.into()),
    }
}

fn sha256(bytes: &[u8]) -> ArtifactHash {
    ArtifactHash::parse(ContentHash::of_bytes(bytes).as_hex()).expect("SHA-256 hex")
}

fn json_artifact(kind: ArtifactKind, bytes: Vec<u8>) -> ArtifactInput {
    ArtifactInput {
        kind,
        format: DataFormat::Json,
        schema_version: 1,
        expected: sha256(&bytes),
        bytes,
    }
}

/// A time on the run manager's clock as seconds since the epoch.
fn wall_clock(copy: &RunCopy, ms: u64, now: i64) -> i64 {
    let ago = copy.now_ms.saturating_sub(ms) / 1000;
    now.saturating_sub(i64::try_from(ago).unwrap_or(i64::MAX))
}

/// The spec's one input hash: the data file, as the run was asked to read it.
fn spec_input(spec_json: &[u8]) -> Option<ContentHash> {
    let v: Value = serde_json::from_slice(spec_json).ok()?;
    let inputs = v.get("inputs")?.as_array()?;
    if inputs.len() != 1 {
        return None;
    }
    ContentHash::from_hex(inputs[0].get("contentHash")?.as_str()?)
}

/// Whether a run may be saved, by its state (decision 123): one that
/// Succeeded with its bundle and manifest, or that Failed with its code.
/// Never a cancelled or an unfinished one; each case has its own code.
fn saveable(copy: &RunCopy) -> Result<(), ProjectCode> {
    match copy.state {
        RunState::Succeeded if copy.bundle_json.is_some() && copy.manifest_json.is_some() => {}
        RunState::Failed if copy.failure.is_some() => {}
        RunState::Failed => return Err(ProjectCode::RunFailedWithoutCode),
        RunState::Cancelled => return Err(ProjectCode::RunCancelled),
        s if !s.is_terminal() => return Err(ProjectCode::RunNotFinished),
        _ => return Err(ProjectCode::RunNotSaveable),
    }
    if copy.spec_json.is_none() || copy.capability.is_none() {
        return Err(ProjectCode::RunNotSaveable);
    }
    Ok(())
}

/// The finished run as a store request: the data file, the spec, and the
/// bundle and manifest or the failure record; `used` the data and the spec,
/// `wasGeneratedBy` the rest. The renderer derives `wasDerivedFrom`.
///
/// The data file is expected to hash to the hash the run recorded when it
/// started, not to whatever the bytes in hand hash to: the store refuses
/// bytes that do not (`project.hash_mismatch`), so what is saved is shown to
/// be the file the run read, and the `used` edge names that hash
/// (decision 123).
fn request_for(
    copy: &RunCopy,
    data: (Vec<u8>, DataFormat, ArtifactHash),
    now: i64,
) -> Result<SaveRequest, ProjectCode> {
    saveable(copy)?;
    let (Some(spec_json), Some((capability_id, capability_version))) =
        (copy.spec_json.clone(), copy.capability.clone())
    else {
        return Err(ProjectCode::RunNotSaveable);
    };
    let outputs: Vec<ArtifactInput> = match (&copy.bundle_json, &copy.manifest_json, &copy.failure)
    {
        (Some(bundle), Some(manifest), _) if copy.state == RunState::Succeeded => vec![
            json_artifact(ArtifactKind::ResultBundle, bundle.clone()),
            json_artifact(ArtifactKind::RunManifest, manifest.clone()),
        ],
        (_, _, Some(failure)) if copy.state == RunState::Failed => {
            let record = serde_json::json!({
                "schemaVersion": FAILURE_RECORD_SCHEMA,
                "status": RunState::Failed,
                "failure": failure,
            });
            vec![json_artifact(
                ArtifactKind::RunFailure,
                serde_json::to_vec(&record).expect("a diagnostic of codes and numbers"),
            )]
        }
        _ => return Err(ProjectCode::RunNotSaveable),
    };
    let state_artifacts = outputs;
    let (data_bytes, format, data_hash) = data;
    let spec = json_artifact(ArtifactKind::OperationSpec, spec_json);
    let used = vec![
        (data_hash.clone(), UsedRole::Data),
        (spec.expected.clone(), UsedRole::Spec),
    ];
    let generated = state_artifacts.iter().map(|a| a.expected.clone()).collect();
    let mut artifacts = vec![
        ArtifactInput {
            kind: ArtifactKind::DataFile,
            format,
            schema_version: 1,
            expected: data_hash,
            bytes: data_bytes,
        },
        spec,
    ];
    artifacts.extend(state_artifacts);
    Ok(SaveRequest {
        run: RunToSave {
            capability_id,
            capability_version,
            state: copy.state,
            started_at: wall_clock(copy, copy.started_ms, now),
            ended_at: wall_clock(copy, copy.ended_ms.unwrap_or(copy.now_ms), now),
            used,
            generated,
            derived: Vec::new(),
        },
        artifacts,
    })
}

/* ------------------------------------------------------------------------ *
 *  The commands' bodies: exempt (decision 120)
 * ------------------------------------------------------------------------ */

/// Every project under the root, readable or not.
pub fn list(root: &Path, ws: &WorkspaceSession) -> Res<Vec<ProjectListEntryView>> {
    let store = ws.projects().store(root).map_err(refuse)?;
    let entries = store.list().map_err(refuse)?;
    Ok(entries
        .into_iter()
        .map(|e| ProjectListEntryView {
            project_id: e.id.as_str().to_string(),
            name: e.title.map(|t| t.as_str().to_string()),
            bytes: e.bytes,
            saved_at: e.saved_at,
            run_count: e.run_count,
            refused: e.refused,
        })
        .collect())
}

/// What all projects hold on disk, and both caps.
pub fn sizes(root: &Path, ws: &WorkspaceSession) -> Res<ProjectSizesView> {
    let projects = ws.projects();
    let store = projects.store(root).map_err(refuse)?;
    let (total_bytes, project_count) = store.usage().map_err(refuse)?;
    Ok(ProjectSizesView {
        total_bytes,
        project_count,
        project_cap_bytes: projects.caps.project,
        all_projects_cap_bytes: projects.caps.all_projects,
    })
}

/// A confirmation for deleting `project_id`, and its size.
pub fn prepare_delete(
    root: &Path,
    ws: &WorkspaceSession,
    project_id: &str,
) -> Res<DeleteConfirmationView> {
    let projects = ws.projects();
    let store = projects.store(root).map_err(refuse)?;
    let id = ProjectId::parse(project_id).map_err(refuse)?;
    let bytes = store.project_bytes(&id).map_err(refuse)?;
    let confirmation = projects.issue(&id).map_err(refuse)?;
    tracing::info!(project = %id.as_str(), "project: delete confirmation issued");
    Ok(DeleteConfirmationView {
        project_id: id.as_str().to_string(),
        confirmation: confirmation.as_str().to_string(),
        bytes,
        expires_in_ms: CONFIRMATION_LIFETIME_MS,
    })
}

/// Deletes `project_id` with its confirmation, closing it first if it is
/// the open project.
pub fn delete(
    root: &Path,
    ws: &WorkspaceSession,
    project_id: &str,
    confirmation: Option<&str>,
) -> Res<ProjectDeletedView> {
    let projects = ws.projects();
    let store = projects.store(root).map_err(refuse)?;
    let id = ProjectId::parse(project_id).map_err(refuse)?;
    projects.redeem(&id, confirmation).map_err(refuse)?;
    // Held to the end, so nothing opens the project between its close and
    // its deletion.
    let mut open = lock(&projects.open);
    let closed = open.as_ref().is_some_and(|h| h.id() == &id);
    if closed {
        if let Some(h) = open.take() {
            h.close();
        }
    }
    store.delete(id.as_str()).map_err(refuse)?;
    lock(&projects.saved).retain(|(p, _)| p != &id);
    tracing::info!(project = %id.as_str(), closed, "project: deleted");
    Ok(ProjectDeletedView {
        project_id: id.as_str().to_string(),
        closed,
    })
}

/// Which project this copy of the app has open, or none (decision 148). It
/// reads this session only: another copy's open project is not known here.
/// Leaving Statistics, Close and deleting the open project each leave none.
pub fn current(ws: &WorkspaceSession) -> Res<ProjectCurrentView> {
    Ok(ProjectCurrentView {
        project_id: ws.projects().open_id().map(|id| id.as_str().to_string()),
    })
}

/* ------------------------------------------------------------------------ *
 *  The commands' bodies: gated by Statistics (decision 120)
 * ------------------------------------------------------------------------ */

/// Saves the run `run_id` into a new project named by the user, or into the
/// open project (decisions 121 to 123). The project saved into is then the
/// open one.
pub fn save_run(
    root: &Path,
    ws: &WorkspaceSession,
    run_id: u64,
    target: SaveTarget,
) -> Res<SavedRunView> {
    let engine = ws.require_statistics().map_err(refuse_workspace)?;
    let projects = ws.projects();
    let store = projects.store(root).map_err(refuse)?;
    let mut open = lock(&projects.open);
    still_active(ws)?;

    let run = RunId(run_id);
    let copy = match engine.runs.copy_for_save(run) {
        Ok(c) => c,
        Err(_) if engine.runs.was_issued(run) => return Err(refuse(ProjectCode::RunEvicted)),
        Err(_) => return Err(refuse(ProjectCode::RunUnknown)),
    };
    let into = match &target {
        SaveTarget::NewProject { .. } => None,
        SaveTarget::OpenProject { project_id } => Some(
            the_open(&mut open, project_id)
                .map_err(refuse)?
                .id()
                .clone(),
        ),
    };
    if let Some(id) = &into {
        if lock(&projects.saved).contains(&(id.clone(), run_id)) {
            return Err(refuse(ProjectCode::RunAlreadySaved));
        }
    }
    // The state first: an unfinished or cancelled run has its own code
    // whatever its data.
    saveable(&copy).map_err(refuse)?;
    let title = match &target {
        SaveTarget::NewProject { name } => Some(ProjectTitle::from_typed(name).map_err(refuse)?),
        SaveTarget::OpenProject { .. } => None,
    };

    // The data file: the slot's copy of the original bytes, only when it is
    // byte for byte the file the run read (decision 123).
    let input = copy
        .input
        .as_ref()
        .ok_or_else(|| refuse(ProjectCode::DataUnavailable))?;
    let slot = engine
        .datasets
        .get(input.slot)
        .map_err(|_| refuse(ProjectCode::DataUnavailable))?;
    let source = slot
        .source()
        .ok_or_else(|| refuse(ProjectCode::DataUnavailable))?;
    let manifest_agrees = copy.state != RunState::Succeeded
        || copy.manifest_inputs.as_slice() == std::slice::from_ref(&input.sha256);
    if source.sha256() != &input.sha256
        || copy.spec_json.as_deref().and_then(spec_input).as_ref() != Some(&input.sha256)
        || !manifest_agrees
    {
        return Err(refuse(ProjectCode::DataMismatch));
    }
    let req = request_for(
        &copy,
        (
            source.bytes().to_vec(),
            source.format(),
            ArtifactHash::parse(input.sha256.as_hex()).map_err(refuse)?,
        ),
        gaply_core::now_epoch(),
    )
    .map_err(refuse)?;

    let (id, stored, created) = match title {
        Some(title) => {
            let (handle, stored) = store.create_with_save(&title, req).map_err(refuse)?;
            let id = handle.id().clone();
            if let Some(previous) = open.replace(handle) {
                previous.close();
            }
            (id, stored, true)
        }
        None => {
            let h = open.as_mut().expect("checked above, under the same lock");
            let stored = h.save(req).map_err(refuse)?;
            (h.id().clone(), stored, false)
        }
    };
    lock(&projects.saved).insert((id.clone(), run_id));
    tracing::info!(project = %id.as_str(), run = stored.0, created, "project: run saved");
    Ok(SavedRunView {
        project_id: id.as_str().to_string(),
        stored_run_id: stored.0,
        created,
    })
}

/// Opens `project_id`; the project open before it is closed.
pub fn open(root: &Path, ws: &WorkspaceSession, project_id: &str) -> Res<ProjectOpenView> {
    let _engine = ws.require_statistics().map_err(refuse_workspace)?;
    let projects = ws.projects();
    let store = projects.store(root).map_err(refuse)?;
    let mut open = lock(&projects.open);
    still_active(ws)?;
    let id = ProjectId::parse(project_id).map_err(refuse)?;
    if open.as_ref().map(|h| h.id()) != Some(&id) {
        let handle = store.open(id.as_str()).map_err(refuse)?;
        if let Some(previous) = open.replace(handle) {
            previous.close();
        }
    }
    let h = open.as_ref().expect("opened above");
    let record = h.record().map_err(refuse)?;
    let run_count = h.runs().map_err(refuse)?.len() as u64;
    let bytes = h.size_on_disk().map_err(refuse)?;
    tracing::info!(project = %id.as_str(), run_count, "project: open");
    Ok(ProjectOpenView {
        project_id: id.as_str().to_string(),
        name: record.title.as_str().to_string(),
        created_at: record.created_at,
        run_count,
        bytes,
    })
}

/// Closes the open project, if any.
pub fn close(_root: &Path, ws: &WorkspaceSession) -> Res<ProjectClosedView> {
    let _engine = ws.require_statistics().map_err(refuse_workspace)?;
    let closed = match lock(&ws.projects().open).take() {
        Some(h) => {
            tracing::info!(project = %h.id().as_str(), "project: closed");
            h.close();
            true
        }
        None => false,
    };
    Ok(ProjectClosedView { closed })
}

/// The open project's saved runs.
pub fn runs(_root: &Path, ws: &WorkspaceSession, project_id: &str) -> Res<Vec<StoredRunView>> {
    let _engine = ws.require_statistics().map_err(refuse_workspace)?;
    let mut open = lock(&ws.projects().open);
    still_active(ws)?;
    let h = the_open(&mut open, project_id).map_err(refuse)?;
    Ok(h.runs()
        .map_err(refuse)?
        .into_iter()
        .map(|r| StoredRunView {
            stored_run_id: r.run_id.0,
            capability_id: r.capability_id.as_str().to_string(),
            capability_version: r.capability_version,
            state: r.state,
            end_code: r.end_code,
            started_at: r.started_at,
            ended_at: r.ended_at,
            saved_at: r.saved_at,
            agent_version: r.agent.as_str().to_string(),
        })
        .collect())
}

/// A run's artifacts by what they were to it: the data file, the spec, and
/// what it generated, each with its row.
struct RunArtifacts {
    data: ArtifactRef,
    spec: ArtifactRef,
    generated: Vec<ArtifactRef>,
}

fn artifacts_of(
    h: &ProjectHandle,
    run: StoredRunId,
) -> Result<(StoredRun, RunArtifacts), ProjectRefusal> {
    let stored = h
        .runs()?
        .into_iter()
        .find(|r| r.run_id == run)
        .ok_or(ProjectCode::StoredRunUnknown)?;
    let rows = h.artifacts()?;
    let row = |hash: &ArtifactHash| -> Result<ArtifactRef, ProjectRefusal> {
        rows.iter()
            .find(|a| &a.hash == hash)
            .cloned()
            .ok_or_else(|| ProjectCode::UnknownArtifact.into())
    };
    let graph = h.provenance()?;
    let (mut data, mut spec, mut generated) = (None, None, Vec::new());
    for e in graph.edges() {
        match e {
            ProvEdge::Used {
                run: r,
                artifact,
                role,
            } if *r == run => match role {
                UsedRole::Data => data = Some(row(artifact)?),
                UsedRole::Spec => spec = Some(row(artifact)?),
            },
            ProvEdge::WasGeneratedBy { artifact, run: r } if *r == run => {
                generated.push(row(artifact)?)
            }
            _ => {}
        }
    }
    generated.sort_by(|a, b| a.hash.cmp(&b.hash));
    match (data, spec) {
        (Some(data), Some(spec)) => Ok((
            stored,
            RunArtifacts {
                data,
                spec,
                generated,
            },
        )),
        _ => Err(ProjectCode::UnknownArtifact.into()),
    }
}

/// An artifact's bytes, verified against its hash, as JSON.
fn read_json(h: &mut ProjectHandle, a: &ArtifactRef) -> Result<Value, ProjectRefusal> {
    let bytes = h.read_artifact(a.hash.as_hex())?;
    serde_json::from_slice(&bytes).map_err(|_| ProjectCode::DbUnreadable.into())
}

/// An artifact's bytes, verified against its hash, as the JSON text stored.
fn read_text(h: &mut ProjectHandle, a: &ArtifactRef) -> Result<String, ProjectRefusal> {
    let bytes = h.read_artifact(a.hash.as_hex())?;
    String::from_utf8(bytes).map_err(|_| ProjectCode::DbUnreadable.into())
}

/// One saved run read back from the open project. Every one of its
/// artifacts' hashes is verified first, the data file's included; a damaged
/// artifact gives its code and nothing else.
pub fn run_result(
    _root: &Path,
    ws: &WorkspaceSession,
    project_id: &str,
    stored_run_id: u64,
) -> Res<RunResultView> {
    let _engine = ws.require_statistics().map_err(refuse_workspace)?;
    let mut open = lock(&ws.projects().open);
    still_active(ws)?;
    let h = the_open(&mut open, project_id).map_err(refuse)?;
    let (stored, arts) = artifacts_of(h, StoredRunId(stored_run_id)).map_err(refuse)?;
    let shown: u64 = arts.spec.bytes + arts.generated.iter().map(|a| a.bytes).sum::<u64>();
    if shown > RESULT_READ_LIMIT_BYTES {
        return Err(refuse(ProjectRefusal::over(
            ProjectCode::ResultTooLarge,
            shown,
            RESULT_READ_LIMIT_BYTES,
        )));
    }
    // Verified, and dropped: the data file's bytes never leave the store.
    h.read_artifact(arts.data.hash.as_hex()).map_err(refuse)?;
    let spec = read_text(h, &arts.spec).map_err(refuse)?;
    let (mut bundle, mut manifest, mut failure) = (None, None, None);
    for a in &arts.generated {
        let v = read_text(h, a).map_err(refuse)?;
        match a.kind {
            ArtifactKind::ResultBundle => bundle = Some(v),
            ArtifactKind::RunManifest => manifest = Some(v),
            ArtifactKind::RunFailure => failure = Some(v),
            ArtifactKind::DataFile | ArtifactKind::OperationSpec => {
                return Err(refuse(ProjectCode::DbUnreadable))
            }
        }
    }
    Ok(RunResultView {
        stored_run_id: stored.run_id.0,
        capability_id: stored.capability_id.as_str().to_string(),
        state: stored.state,
        data_file: StoredArtifactView {
            sha256: arts.data.hash.as_hex().to_string(),
            kind: arts.data.kind,
            format: arts.data.format,
            bytes: arts.data.bytes,
        },
        spec_json: spec,
        bundle_json: bundle,
        manifest_json: manifest,
        failure_json: failure,
    })
}

/* ------------------------------------------------------------------------ *
 *  The exports' input: every parameter from an allow-list
 * ------------------------------------------------------------------------ */

/// What the report does with each field of a saved spec. Every field a spec
/// serialises must be listed (`every_spec_field_is_reported_or_excluded`), so
/// a new parameter cannot reach the report, or be left out of it, unread.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpecFieldShown {
    /// Printed as a position, an index, a closed word or a number.
    Reported,
    /// Printed as a count only.
    Counted,
    /// Not printed, for the reason given.
    Excluded(&'static str),
}

/// Every field of a saved describe or t-test spec, by JSON path (`[]` for
/// each element of an array).
pub const SPEC_FIELDS: &[(&str, SpecFieldShown)] = &[
    (
        "schemaVersion",
        SpecFieldShown::Excluded("a constant of the contracts"),
    ),
    (
        "operationId",
        SpecFieldShown::Excluded("a session's number, meaningless after it"),
    ),
    (
        "workspaceId",
        SpecFieldShown::Excluded("always Statistics, the project's own workspace"),
    ),
    ("capabilityId", SpecFieldShown::Reported),
    ("capabilityVersion", SpecFieldShown::Reported),
    ("inputs[].contentHash", SpecFieldShown::Reported),
    ("inputs[].rows", SpecFieldShown::Reported),
    ("inputs[].columns", SpecFieldShown::Reported),
    (
        "inputs[].semanticRole",
        SpecFieldShown::Excluded("always a table in Phase 5B"),
    ),
    ("randomSeed", SpecFieldShown::Reported),
    (
        "confirmationRefs[]",
        SpecFieldShown::Excluded("confirmation ids, never shown"),
    ),
    ("parameters.columns[].alias", SpecFieldShown::Reported),
    ("parameters.columns[].treatAs", SpecFieldShown::Reported),
    ("parameters.missingCodes[]", SpecFieldShown::Counted),
    ("parameters.outcome", SpecFieldShown::Reported),
    ("parameters.group", SpecFieldShown::Reported),
    ("parameters.levels[]", SpecFieldShown::Reported),
    ("parameters.variant", SpecFieldShown::Reported),
];

fn shape(spec_inputs: &[InputRef]) -> Vec<ExportParameter> {
    spec_inputs
        .iter()
        .map(|i| ExportParameter::InputShape {
            rows: i.rows,
            columns: i.columns,
        })
        .collect()
}

/// A describe spec's settings, by the allow-list.
fn describe_parameters(spec: &OperationSpec<DescribeParams>) -> Vec<ExportParameter> {
    let mut out = shape(&spec.inputs);
    for c in &spec.parameters.columns {
        out.push(ExportParameter::Column {
            position: c.alias.0 .0,
            role: match c.treat_as {
                TreatAs::Numeric => ColumnRole::SummarisedNumeric,
                TreatAs::Categorical => ColumnRole::SummarisedCategorical,
            },
        });
    }
    out.push(ExportParameter::MissingCodes {
        count: spec.parameters.missing_codes.len() as u64,
    });
    out.push(ExportParameter::Seed(spec.random_seed));
    out
}

/// A t-test spec's settings, by the allow-list.
fn ttest_parameters(spec: &OperationSpec<TTestParams>) -> Vec<ExportParameter> {
    let p = &spec.parameters;
    let mut out = shape(&spec.inputs);
    out.push(ExportParameter::Column {
        position: p.outcome.0 .0,
        role: ColumnRole::Outcome,
    });
    out.push(ExportParameter::Column {
        position: p.group.0 .0,
        role: ColumnRole::Group,
    });
    out.push(ExportParameter::Levels(p.levels));
    out.push(ExportParameter::Variant(p.variant.map(|v| match v {
        Variant::Student => TestVariant::Student,
        Variant::Welch => TestVariant::Welch,
    })));
    out.push(ExportParameter::MissingCodes {
        count: p.missing_codes.len() as u64,
    });
    out.push(ExportParameter::Seed(spec.random_seed));
    out
}

fn parse<T: DeserializeOwned>(v: Value) -> Result<T, ProjectRefusal> {
    serde_json::from_value(v).map_err(|_| ProjectCode::DbUnreadable.into())
}

/// One kind's outputs as the exports show them: the manifest's counts and
/// hashes, the bundle's limitations and notes, or the failure's code.
fn outputs<C: Closed + DeserializeOwned>(
    h: &mut ProjectHandle,
    generated: &[ArtifactRef],
    run: &mut ExportRun,
) -> Result<(), ProjectRefusal> {
    for a in generated {
        let v = read_json(h, a)?;
        match a.kind {
            ArtifactKind::RunManifest => {
                let m: RunManifest<C> = parse(v)?;
                run.exclusions = m.exclusions.iter().map(ExportCount::of).collect();
                run.manifest = Some(ExportManifest {
                    engine_version: m.engine_version,
                    inputs: m.inputs,
                    parameters_hash: m.parameters_hash,
                    output_hash: m.output_hash,
                    rows_read: m.rows_read,
                    rows_used: m.rows_used,
                    elapsed_ms: m.elapsed_ms,
                    platform: m.platform,
                    reproducibility: m.reproducibility,
                });
            }
            ArtifactKind::ResultBundle => {
                let codes = |key: &str| -> Result<Vec<ExportCode>, ProjectRefusal> {
                    let list: Vec<Diagnostic<C>> =
                        parse(v.get(key).cloned().unwrap_or(Value::Null))?;
                    Ok(list.iter().map(ExportCode::of).collect())
                };
                run.limitations = codes("limitations")?;
                run.notes = codes("diagnostics")?;
            }
            ArtifactKind::RunFailure => {
                let d: Diagnostic<C> = parse(v.get("failure").cloned().unwrap_or(Value::Null))?;
                run.failure = Some(ExportCode::of(&d));
            }
            ArtifactKind::DataFile | ArtifactKind::OperationSpec => {
                return Err(ProjectCode::DbUnreadable.into())
            }
        }
    }
    Ok(())
}

/// The open project as both exports' input: every run's settings from its
/// spec, by the allow-list, and its codes and fingerprints from its outputs,
/// each artifact verified as it is read. No header, label, cell, path or file
/// name can enter it: no field of [`ProjectExportInput`] but the title holds
/// text.
fn export_input(h: &mut ProjectHandle) -> Result<ProjectExportInput, ProjectRefusal> {
    let record = h.record()?;
    let mut runs = Vec::new();
    for stored in h.runs()? {
        let (_, arts) = artifacts_of(h, stored.run_id)?;
        let spec = read_json(h, &arts.spec)?;
        let mut run = ExportRun {
            run: stored.clone(),
            parameters: Vec::new(),
            manifest: None,
            exclusions: Vec::new(),
            limitations: Vec::new(),
            notes: Vec::new(),
            failure: None,
        };
        match stored.capability_id.as_str() {
            stats_desc::CAPABILITY_ID => {
                run.parameters = describe_parameters(&parse(spec)?);
                outputs::<DescCode>(h, &arts.generated, &mut run)?;
            }
            stats_ttest::CAPABILITY_ID => {
                run.parameters = ttest_parameters(&parse(spec)?);
                outputs::<TTestCode>(h, &arts.generated, &mut run)?;
            }
            _ => return Err(ProjectCode::DbUnreadable.into()),
        }
        runs.push(run);
    }
    Ok(ProjectExportInput {
        title: record.title,
        project: record.id,
        created_at: record.created_at,
        created_by: record.app_version,
        artifacts: h.artifacts()?,
        runs,
        edges: h.provenance()?.edges().to_vec(),
    })
}

fn export_view(file_name: &'static str, bytes: Vec<u8>) -> ProjectExportView {
    let sha256 = ContentHash::of_bytes(&bytes).as_hex().to_string();
    ProjectExportView {
        file_name,
        bytes,
        sha256,
    }
}

/// The open project's report, as Markdown, with its fixed file name.
pub fn report(_root: &Path, ws: &WorkspaceSession, project_id: &str) -> Res<ProjectExportView> {
    let _engine = ws.require_statistics().map_err(refuse_workspace)?;
    let mut open = lock(&ws.projects().open);
    still_active(ws)?;
    let h = the_open(&mut open, project_id).map_err(refuse)?;
    let input = export_input(h).map_err(refuse)?;
    let text = render_report_markdown(&input);
    tracing::info!(project = %input.project.as_str(), runs = input.runs.len(), "project: report rendered");
    Ok(export_view(REPORT_FILE_NAME, text.into_bytes()))
}

/// The open project's provenance, as PROV JSON, with its fixed file name.
pub fn provenance(_root: &Path, ws: &WorkspaceSession, project_id: &str) -> Res<ProjectExportView> {
    let _engine = ws.require_statistics().map_err(refuse_workspace)?;
    let mut open = lock(&ws.projects().open);
    still_active(ws)?;
    let h = the_open(&mut open, project_id).map_err(refuse)?;
    let input = export_input(h).map_err(refuse)?;
    let bytes = render_prov_json(&input).map_err(refuse)?;
    tracing::info!(project = %input.project.as_str(), runs = input.runs.len(), "project: provenance rendered");
    Ok(export_view(PROV_FILE_NAME, bytes))
}

/* ------------------------------------------------------------------------ *
 *  The Tauri commands
 * ------------------------------------------------------------------------ */

type St<'a> = tauri::State<'a, crate::state::AppState>;

async fn blocking<T: Send + 'static>(f: impl FnOnce() -> Res<T> + Send + 'static) -> Res<T> {
    tokio::task::spawn_blocking(f)
        .await
        .map_err(|_| GaplyError::Internal("project task panicked".into()))?
}

fn root_of(app: &tauri::AppHandle) -> Res<PathBuf> {
    projects_root(app).map_err(refuse)
}

#[tauri::command]
#[tracing::instrument(skip_all)]
pub async fn project_list(
    app: tauri::AppHandle,
    state: St<'_>,
) -> Result<Vec<ProjectListEntryView>, GaplyError> {
    let root = root_of(&app)?;
    let ws = state.workspace.clone();
    blocking(move || list(&root, &ws)).await
}

#[tauri::command]
#[tracing::instrument(skip_all)]
pub async fn project_sizes(
    app: tauri::AppHandle,
    state: St<'_>,
) -> Result<ProjectSizesView, GaplyError> {
    let root = root_of(&app)?;
    let ws = state.workspace.clone();
    blocking(move || sizes(&root, &ws)).await
}

#[tauri::command]
#[tracing::instrument(skip_all)]
pub async fn project_prepare_delete(
    app: tauri::AppHandle,
    state: St<'_>,
    project_id: String,
) -> Result<DeleteConfirmationView, GaplyError> {
    let root = root_of(&app)?;
    let ws = state.workspace.clone();
    blocking(move || prepare_delete(&root, &ws, &project_id)).await
}

#[tauri::command]
#[tracing::instrument(skip_all)]
pub async fn project_delete(
    app: tauri::AppHandle,
    state: St<'_>,
    project_id: String,
    confirmation: Option<String>,
) -> Result<ProjectDeletedView, GaplyError> {
    let root = root_of(&app)?;
    let ws = state.workspace.clone();
    blocking(move || delete(&root, &ws, &project_id, confirmation.as_deref())).await
}

#[tauri::command]
#[tracing::instrument(skip_all)]
pub async fn project_current(state: St<'_>) -> Result<ProjectCurrentView, GaplyError> {
    let ws = state.workspace.clone();
    blocking(move || current(&ws)).await
}

#[tauri::command]
#[tracing::instrument(skip_all)]
pub async fn project_save_run(
    app: tauri::AppHandle,
    state: St<'_>,
    run_id: u64,
    target: SaveTarget,
) -> Result<SavedRunView, GaplyError> {
    let _engine = state
        .workspace
        .require_statistics()
        .map_err(refuse_workspace)?;
    let root = root_of(&app)?;
    let ws = state.workspace.clone();
    blocking(move || save_run(&root, &ws, run_id, target)).await
}

#[tauri::command]
#[tracing::instrument(skip_all)]
pub async fn project_open(
    app: tauri::AppHandle,
    state: St<'_>,
    project_id: String,
) -> Result<ProjectOpenView, GaplyError> {
    let _engine = state
        .workspace
        .require_statistics()
        .map_err(refuse_workspace)?;
    let root = root_of(&app)?;
    let ws = state.workspace.clone();
    blocking(move || open(&root, &ws, &project_id)).await
}

#[tauri::command]
#[tracing::instrument(skip_all)]
pub async fn project_close(
    app: tauri::AppHandle,
    state: St<'_>,
) -> Result<ProjectClosedView, GaplyError> {
    let _engine = state
        .workspace
        .require_statistics()
        .map_err(refuse_workspace)?;
    let root = root_of(&app)?;
    let ws = state.workspace.clone();
    blocking(move || close(&root, &ws)).await
}

#[tauri::command]
#[tracing::instrument(skip_all)]
pub async fn project_runs(
    app: tauri::AppHandle,
    state: St<'_>,
    project_id: String,
) -> Result<Vec<StoredRunView>, GaplyError> {
    let _engine = state
        .workspace
        .require_statistics()
        .map_err(refuse_workspace)?;
    let root = root_of(&app)?;
    let ws = state.workspace.clone();
    blocking(move || runs(&root, &ws, &project_id)).await
}

#[tauri::command]
#[tracing::instrument(skip_all)]
pub async fn project_run_result(
    app: tauri::AppHandle,
    state: St<'_>,
    project_id: String,
    stored_run_id: u64,
) -> Result<RunResultView, GaplyError> {
    let _engine = state
        .workspace
        .require_statistics()
        .map_err(refuse_workspace)?;
    let root = root_of(&app)?;
    let ws = state.workspace.clone();
    blocking(move || run_result(&root, &ws, &project_id, stored_run_id)).await
}

#[tauri::command]
#[tracing::instrument(skip_all)]
pub async fn project_report(
    app: tauri::AppHandle,
    state: St<'_>,
    project_id: String,
) -> Result<ProjectExportView, GaplyError> {
    let _engine = state
        .workspace
        .require_statistics()
        .map_err(refuse_workspace)?;
    let root = root_of(&app)?;
    let ws = state.workspace.clone();
    blocking(move || report(&root, &ws, &project_id)).await
}

#[tauri::command]
#[tracing::instrument(skip_all)]
pub async fn project_provenance(
    app: tauri::AppHandle,
    state: St<'_>,
    project_id: String,
) -> Result<ProjectExportView, GaplyError> {
    let _engine = state
        .workspace
        .require_statistics()
        .map_err(refuse_workspace)?;
    let root = root_of(&app)?;
    let ws = state.workspace.clone();
    blocking(move || provenance(&root, &ws, &project_id)).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use gaply_core::contracts::capability::ConfirmationId;
    use gaply_core::contracts::{OperationId, SchemaVersion, SemanticRole};
    use gaply_core::stats_desc::{Alias, ColumnRequest};
    use gaply_core::stats_l1::dataset::ColumnAlias;
    use std::sync::atomic::{AtomicU64, Ordering};

    fn input() -> InputRef {
        InputRef {
            content_hash: ContentHash::of_bytes(b"data"),
            rows: 12,
            columns: 3,
            semantic_role: SemanticRole::Table,
        }
    }

    /// A spec of each kind with every optional field filled.
    fn specs() -> (OperationSpec<DescribeParams>, OperationSpec<TTestParams>) {
        let confirmation = ConfirmationId::parse(&"c".repeat(64)).unwrap();
        let describe = OperationSpec {
            schema_version: SchemaVersion,
            operation_id: OperationId(3),
            workspace_id: WorkspaceId::Statistics,
            capability_id: stats_desc::capability_id(),
            capability_version: 1,
            inputs: vec![input()],
            parameters: DescribeParams {
                columns: vec![
                    ColumnRequest {
                        alias: Alias(ColumnAlias(1)),
                        treat_as: TreatAs::Numeric,
                    },
                    ColumnRequest {
                        alias: Alias(ColumnAlias(3)),
                        treat_as: TreatAs::Categorical,
                    },
                ],
                missing_codes: vec![-99.0, 999.0],
            },
            random_seed: Some(7),
            confirmation_refs: vec![confirmation.clone()],
        };
        let ttest = OperationSpec {
            schema_version: SchemaVersion,
            operation_id: OperationId(4),
            workspace_id: WorkspaceId::Statistics,
            capability_id: stats_ttest::capability_id(),
            capability_version: 1,
            inputs: vec![input()],
            parameters: TTestParams {
                outcome: Alias(ColumnAlias(2)),
                group: Alias(ColumnAlias(3)),
                levels: Some([0, 2]),
                variant: Some(Variant::Student),
                missing_codes: vec![-1.0],
            },
            random_seed: Some(9),
            confirmation_refs: vec![confirmation],
        };
        (describe, ttest)
    }

    /// Every leaf of a JSON value, by path, `[]` for an array's elements.
    fn paths(v: &Value, at: &str, out: &mut BTreeSet<String>) {
        match v {
            Value::Object(m) => {
                for (k, v) in m {
                    let p = if at.is_empty() {
                        k.clone()
                    } else {
                        format!("{at}.{k}")
                    };
                    paths(v, &p, out);
                }
            }
            Value::Array(a) => {
                for v in a {
                    paths(v, &format!("{at}[]"), out);
                }
            }
            _ => {
                out.insert(at.to_string());
            }
        }
    }

    #[test]
    fn every_spec_field_is_reported_or_excluded() {
        let (d, t) = specs();
        let mut seen = BTreeSet::new();
        paths(&serde_json::to_value(&d).unwrap(), "", &mut seen);
        paths(&serde_json::to_value(&t).unwrap(), "", &mut seen);
        let listed: BTreeSet<String> = SPEC_FIELDS.iter().map(|(p, _)| p.to_string()).collect();
        let unlisted: Vec<&String> = seen.difference(&listed).collect();
        assert!(
            unlisted.is_empty(),
            "a spec field neither reported nor excluded: {unlisted:?}"
        );
        let stale: Vec<&String> = listed.difference(&seen).collect();
        assert!(stale.is_empty(), "listed but no spec has it: {stale:?}");
        for (path, shown) in SPEC_FIELDS {
            if let SpecFieldShown::Excluded(why) = shown {
                assert!(!why.is_empty(), "{path}: an exclusion says why");
            }
        }
    }

    #[test]
    fn the_settings_are_positions_roles_indexes_and_counts() {
        let (d, t) = specs();
        assert_eq!(
            describe_parameters(&d),
            [
                ExportParameter::InputShape {
                    rows: 12,
                    columns: 3
                },
                ExportParameter::Column {
                    position: 1,
                    role: ColumnRole::SummarisedNumeric
                },
                ExportParameter::Column {
                    position: 3,
                    role: ColumnRole::SummarisedCategorical
                },
                ExportParameter::MissingCodes { count: 2 },
                ExportParameter::Seed(Some(7)),
            ]
        );
        assert_eq!(
            ttest_parameters(&t),
            [
                ExportParameter::InputShape {
                    rows: 12,
                    columns: 3
                },
                ExportParameter::Column {
                    position: 2,
                    role: ColumnRole::Outcome
                },
                ExportParameter::Column {
                    position: 3,
                    role: ColumnRole::Group
                },
                ExportParameter::Levels(Some([0, 2])),
                ExportParameter::Variant(Some(TestVariant::Student)),
                ExportParameter::MissingCodes { count: 1 },
                ExportParameter::Seed(Some(9)),
            ]
        );
    }

    #[derive(Default)]
    struct Moved(AtomicU64);

    impl Clock for Moved {
        fn now_ms(&self) -> u64 {
            self.0.load(Ordering::SeqCst)
        }
    }

    fn pid(c: char) -> ProjectId {
        ProjectId::parse(&c.to_string().repeat(32)).unwrap()
    }

    #[test]
    fn a_confirmation_is_random_used_once_short_lived_and_replaced_by_a_newer_one() {
        let clock = Arc::new(Moved::default());
        let s = ProjectSession::with(Caps::REAL, clock.clone());
        let (a, b) = (pid('a'), pid('b'));

        let first = s.issue(&a).unwrap();
        let second = s.issue(&a).unwrap();
        assert_ne!(first, second, "random each time");
        assert_eq!(
            s.redeem(&a, Some(first.as_str())),
            Err(ProjectCode::ConfirmationReplaced)
        );
        assert_eq!(s.redeem(&a, Some(second.as_str())), Ok(()));
        assert_eq!(
            s.redeem(&a, Some(second.as_str())),
            Err(ProjectCode::ConfirmationUsed)
        );

        let other = s.issue(&b).unwrap();
        assert_eq!(
            s.redeem(&a, Some(other.as_str())),
            Err(ProjectCode::ConfirmationOtherProject)
        );
        assert_eq!(
            s.redeem(&b, Some(other.as_str())),
            Err(ProjectCode::ConfirmationUsed),
            "a mismatched one is used up too"
        );

        let late = s.issue(&a).unwrap();
        clock
            .0
            .store(CONFIRMATION_LIFETIME_MS - 1, Ordering::SeqCst);
        let in_time = s.issue(&a).unwrap();
        clock
            .0
            .store(2 * CONFIRMATION_LIFETIME_MS - 2, Ordering::SeqCst);
        assert_eq!(
            s.redeem(&a, Some(in_time.as_str())),
            Ok(()),
            "1 ms short of five minutes"
        );
        let _ = late;
        let expired = s.issue(&a).unwrap();
        clock
            .0
            .fetch_add(CONFIRMATION_LIFETIME_MS, Ordering::SeqCst);
        assert_eq!(
            s.redeem(&a, Some(expired.as_str())),
            Err(ProjectCode::ConfirmationExpired)
        );

        assert_eq!(s.redeem(&a, None), Err(ProjectCode::ConfirmationMissing));
        assert_eq!(
            s.redeem(&a, Some("")),
            Err(ProjectCode::ConfirmationMissing)
        );
        assert_eq!(
            s.redeem(&a, Some(&"0".repeat(64))),
            Err(ProjectCode::ConfirmationUnknown)
        );
        assert_eq!(
            s.redeem(&a, Some("not a confirmation")),
            Err(ProjectCode::ConfirmationUnknown)
        );
    }

    #[test]
    fn the_session_remembers_a_bounded_number_and_a_restart_forgets_them() {
        let s = ProjectSession::default();
        let a = pid('a');
        let first = s.issue(&a).unwrap();
        for _ in 0..CONFIRMATIONS_REMEMBERED {
            s.issue(&a).unwrap();
        }
        assert_eq!(lock(&s.book).len(), CONFIRMATIONS_REMEMBERED);
        assert_eq!(
            s.redeem(&a, Some(first.as_str())),
            Err(ProjectCode::ConfirmationUnknown),
            "the oldest is forgotten"
        );
        let kept = s.issue(&a).unwrap();
        let restarted = ProjectSession::default();
        assert_eq!(
            restarted.redeem(&a, Some(kept.as_str())),
            Err(ProjectCode::ConfirmationUnknown),
            "held only in memory"
        );
    }

    fn failed_copy(recorded: &ContentHash) -> RunCopy {
        RunCopy {
            kind: crate::runs::RunKind::Ttest,
            state: RunState::Failed,
            started_ms: 10,
            ended_ms: Some(20),
            now_ms: 30,
            spec_json: Some(b"{}".to_vec()),
            capability: Some((stats_ttest::capability_id(), 1)),
            input: Some(crate::runs::RunInput {
                slot: 1,
                sha256: recorded.clone(),
            }),
            bundle_json: None,
            manifest_json: None,
            manifest_inputs: Vec::new(),
            failure: Some(serde_json::json!({"code": "stats.ttest.both_groups_constant"})),
        }
    }

    #[test]
    fn the_data_file_is_expected_to_hash_to_what_the_run_recorded() {
        let recorded = ContentHash::of_bytes(b"the file the run read");
        let copy = failed_copy(&recorded);
        let expected = ArtifactHash::parse(recorded.as_hex()).unwrap();
        let req = request_for(
            &copy,
            (b"other bytes".to_vec(), DataFormat::Csv, expected.clone()),
            1_000,
        )
        .unwrap();
        let data = &req.artifacts[0];
        assert_eq!(data.kind, ArtifactKind::DataFile);
        assert_eq!(
            data.expected, expected,
            "the recorded hash, not the bytes' own"
        );
        assert_ne!(
            sha256(&data.bytes),
            expected,
            "so the store refuses these bytes"
        );
        assert!(
            req.run.used.contains(&(expected, UsedRole::Data)),
            "and `used` names it"
        );
        assert_eq!(
            req.run.started_at, 1_000,
            "times on the wall clock, to the second"
        );
    }

    #[test]
    fn equal_bytes_compare_in_full() {
        assert!(same_bytes("abc", "abc"));
        assert!(!same_bytes("abc", "abd"));
        assert!(!same_bytes("abc", "ab"));
        assert!(!same_bytes("", "a"));
    }
}
