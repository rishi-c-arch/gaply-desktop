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
//! **A rerun (Phase 5C-1a, decisions 185 to 190)** runs a saved run that
//! Succeeded again, in the open project: its stored spec, read back to
//! exactly its stored bytes, on the project's copy of its data file, read
//! through the ingest's own checks and parse into a slot only that run holds.
//! It runs as an ordinary run, through the run manager and the façades. When
//! it ends, its end hook saves it into the project in one write, through the
//! same inner save a Save uses, with `rerun_of`, if it Succeeded or Failed
//! with its code; any other end, or a project no longer open, writes nothing.
//! Nothing is written while it runs. From `project_rerun` until that save has
//! returned it is going: closing the project, opening another, a Save into a
//! new project, deleting the open project and a second rerun are refused with
//! `project.rerun_in_progress`, and leaving Statistics with the workspace's
//! own `workspace.run_active`. `project_current` says where it stands.
//!
//! **What reaches the webview.** Ids, hashes, closed words and numbers; the
//! project's name only in the list and the open view; a run's spec and
//! result bundle only in [`RunResultView`], read back from the open project
//! after every one of its artifacts' hashes is verified; the two exports as
//! bytes with a fixed file name (decision 125). Never a data file's bytes,
//! and never a path. A refusal is a closed code with numeric parameters.
//! Logs carry a project by its id only.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, Weak};
use std::time::Instant;

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use gaply_core::contracts::capability::{Build, ConfirmationId};
use gaply_core::contracts::codes::Closed;
use gaply_core::contracts::workspace::{check_spec, WorkspaceState};
use gaply_core::contracts::{
    ContentHash, Diagnostic, InputRef, OperationSpec, Platform, Preparation, RunId, RunManifest,
    RunState, ValidationReport, ValidationStatus, WorkspaceId,
};
use gaply_core::research_project::{
    compare_rerun, ArtifactHash, ArtifactKind, ArtifactRef, Caps, ColumnRole,
    CorrelationCoefficient, DataFormat, ExportCode, ExportCount, ExportManifest, ExportParameter,
    ExportRun, MissingRows, PostHocTest, ProjectCode, ProjectExportInput, ProjectId,
    ProjectRefusal, ProjectTitle, ProvEdge, ProvGraph, RerunComparison, ScoreRule, ScoreStatistic,
    StoredRun, StoredRunId, TestVariant, UsedRole, PROV_FILE_NAME, REPORT_FILE_NAME,
};
use gaply_core::research_project::{
    numbers_agree_to_last_digits, platforms_differ, render_prov_json, render_report_markdown,
};
use gaply_core::stats_adapters;
use gaply_core::stats_alpha::{self, AlphaCode, AlphaParams};
use gaply_core::stats_anova::{self, AnovaCode, AnovaParams, PostHoc};
use gaply_core::stats_corr::{self, Coefficient, CorrCode, CorrParams, MissingHandling};
use gaply_core::stats_desc::{self, DescCode, DescribeParams, TreatAs};
use gaply_core::stats_l1::dataset::Dataset;
use gaply_core::stats_onesample::{self, OneSampleCode, OneSampleParams, PairedParams};
use gaply_core::stats_prep::{
    MissingRule, PrepCode, PrepRefusal, Prepared, Recipe, RecipeStep, ScaleScoreStep, Statistic,
};
use gaply_core::stats_registry;
use gaply_core::stats_ttest::{self, TTestCode, TTestParams, Variant};
use gaply_core::stats_xtab::{self, CrosstabsParams, XtabCode};
use gaply_core::GaplyError;

#[cfg(test)]
use crate::confirmation::same_bytes;
use crate::confirmation::{Confirmations, Monotonic, NoRandomness, Refused};
use crate::project_store::{
    projects_root, ArtifactInput, ProjectHandle, ProjectRoot, RunToSave, SaveRequest,
};
use crate::runs::{Clock, EndHook, NotStarted, RunCopy, RunInput, RunKind, Runs, Slot, SourceFile};
#[cfg(test)]
use crate::stats_l1_ingest::load_dataset_from_bytes;
use crate::stats_l1_ingest::{load_dataset_from_bytes_with, L1IngestRefusal};
use crate::supplementary::IngestLimits;
use crate::workspace::{refuse as refuse_workspace, refuse_inactive, WorkspaceSession};

#[cfg(test)]
use crate::confirmation::CONFIRMATIONS_REMEMBERED;
/// How long a delete confirmation stays good (decision 124). The rule lives
/// in `crate::confirmation`, shared with the Stored data section (Phase
/// 5D-1b, decision 213).
pub use crate::confirmation::CONFIRMATION_LIFETIME_MS;

/// The most a run's result may come to when it is read back: its spec, its
/// result bundle and its manifest or failure record together. The data file
/// is never part of it.
pub const RESULT_READ_LIMIT_BYTES: u64 = 4 * 1024 * 1024;

/// The schema of a failed run's failure record.
const FAILURE_RECORD_SCHEMA: u32 = 1;

/* ------------------------------------------------------------------------ *
 *  The session
 * ------------------------------------------------------------------------ */

/// The open project and the delete confirmations, beside the active
/// workspace (decision 122). In memory only.
pub struct ProjectSession {
    caps: Caps,
    clock: Arc<dyn Clock>,
    /// The store under the root, opened once: its start finishes what an
    /// interrupted create or delete left, which must not run beside a create.
    store: Mutex<Option<(PathBuf, Arc<ProjectRoot>)>>,
    open: Mutex<Option<ProjectHandle>>,
    /// The delete confirmations (decision 124): this session's own book.
    book: Mutex<Confirmations<ProjectId>>,
    /// The runs each project has saved in this process, by the run's id:
    /// a run is never saved twice into one project (decision 123).
    saved: Mutex<BTreeSet<(ProjectId, u64)>>,
    /// The last rerun this copy started, and where it stands (decision 187):
    /// in its own mutex, so `project_current` reads it without waiting for
    /// a save at the end that holds `open`.
    rerun: Mutex<Option<RerunState>>,
    /// A rerun is going (decision 190), from `project_rerun` until its save
    /// at the end has returned: written only under `open`'s lock, read
    /// without it by `leave()` and `project_current`.
    going: AtomicBool,
    /// The going rerun's in-memory run, once known; 0 before.
    going_run: AtomicU64,
    /// Every rerun's in-memory run in this process, and whether its save at
    /// the end succeeded: no such run is ever saved by hand (decision 187).
    rerun_runs: Mutex<BTreeMap<u64, bool>>,
    /// Numbers each rerun, so a late word about one never lands on another.
    tickets: AtomicU64,
}

/// Decision 193: armed while a rerun's save at the end runs, under `open`'s
/// lock. If that save panics, its drop during the unwind records the rerun as
/// not saved with `project.io_failed` and clears the going flag, so the
/// project is not left going until the app restarts. It does not catch the
/// panic, which unwinds on with Rust's own report.
struct EndGuard<'a> {
    session: &'a ProjectSession,
    ticket: u64,
    project: &'a ProjectId,
    of: StoredRunId,
    run: RunId,
    armed: bool,
}

impl Drop for EndGuard<'_> {
    fn drop(&mut self) {
        if !self.armed {
            return;
        }
        let s = self.session;
        let code = ProjectCode::IoFailed;
        *lock(&s.rerun_runs).entry(self.run.0).or_insert(false) = false;
        {
            let mut state = lock(&s.rerun);
            if state.as_ref().map(|r| r.ticket) == Some(self.ticket) {
                *state = Some(RerunState {
                    ticket: self.ticket,
                    project: self.project.clone(),
                    of: self.of,
                    run: Some(self.run.0),
                    status: RerunStatus::NotSaved(code),
                });
            }
        }
        s.going.store(false, Ordering::SeqCst);
        tracing::warn!(
            project = %self.project.as_str(),
            run = self.run.0,
            code = code.wire(),
            "project: a rerun's save at the end panicked; not saved"
        );
    }
}

/// Where a rerun stands (decision 187).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RerunStatus {
    Going,
    Saved(StoredRunId),
    NotSaved(ProjectCode),
}

/// The last rerun this copy started.
#[derive(Debug, Clone)]
struct RerunState {
    ticket: u64,
    project: ProjectId,
    of: StoredRunId,
    /// Its in-memory run, once `start` or its end hook has named it.
    run: Option<u64>,
    status: RerunStatus,
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
            book: Mutex::new(Confirmations::default()),
            saved: Mutex::new(BTreeSet::new()),
            rerun: Mutex::new(None),
            going: AtomicBool::new(false),
            going_run: AtomicU64::new(0),
            rerun_runs: Mutex::new(BTreeMap::new()),
            tickets: AtomicU64::new(0),
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

    /// The going rerun's in-memory run (0 for the moment before its run is
    /// named), or `None` when no rerun is going (decision 190). An atomic
    /// read: `leave()` calls it under the workspace's lock and takes no
    /// project lock.
    pub fn rerun_going(&self) -> Option<u64> {
        if self.going.load(Ordering::SeqCst) {
            Some(self.going_run.load(Ordering::SeqCst))
        } else {
            None
        }
    }

    /// `project.rerun_in_progress` while a rerun is going. Called under
    /// `open`'s lock by each action decision 190 refuses.
    fn not_rerunning(&self) -> Result<(), ProjectCode> {
        if self.going.load(Ordering::SeqCst) {
            Err(ProjectCode::RerunInProgress)
        } else {
            Ok(())
        }
    }

    /// A rerun of `of` in `project` is going from now on; called under
    /// `open`'s lock. Returns its ticket and the state it replaces, kept in
    /// case the run does not start.
    fn begin_rerun(&self, project: &ProjectId, of: StoredRunId) -> (u64, Option<RerunState>) {
        let ticket = self.tickets.fetch_add(1, Ordering::SeqCst) + 1;
        self.going_run.store(0, Ordering::SeqCst);
        self.going.store(true, Ordering::SeqCst);
        let before = lock(&self.rerun).replace(RerunState {
            ticket,
            project: project.clone(),
            of,
            run: None,
            status: RerunStatus::Going,
        });
        (ticket, before)
    }

    /// The rerun's run was made: its id, unless its end hook named it first.
    fn rerun_named(&self, ticket: u64, run: u64) {
        let mut state = lock(&self.rerun);
        if let Some(s) = state.as_mut().filter(|s| s.ticket == ticket) {
            if s.run.is_none() {
                s.run = Some(run);
                if s.status == RerunStatus::Going {
                    self.going_run.store(run, Ordering::SeqCst);
                }
            }
        }
        lock(&self.rerun_runs).entry(run).or_insert(false);
    }

    /// The run was not started (another run, or the workspace was left):
    /// nothing is going, and the state before it stands again.
    fn rerun_not_started(&self, before: Option<RerunState>) {
        let _open = lock(&self.open);
        *lock(&self.rerun) = before;
        self.going.store(false, Ordering::SeqCst);
    }

    /// The rerun's end hook: the save at the end, holding only a weak
    /// reference to this session, so once the session is gone (the app is
    /// closing) the project counts as not open and nothing is written.
    fn end_hook(
        projects: &Arc<ProjectSession>,
        ticket: u64,
        project: ProjectId,
        of: StoredRunId,
        slot: Arc<Slot>,
    ) -> EndHook {
        let session: Weak<ProjectSession> = Arc::downgrade(projects);
        Box::new(move |run: RunId, copy: RunCopy| match session.upgrade() {
            Some(s) => s.end_rerun(ticket, &project, of, run, &copy, &slot),
            None => tracing::info!(
                run = run.0,
                "project: a rerun ended after its session; nothing written"
            ),
        })
    }

    /// The save at the end (decisions 186 and 187), called once a rerun's
    /// run has ended, with no lock held, on the thread that ended it. It
    /// takes `open`'s lock, then the store's write lock inside
    /// [`ProjectHandle::save_rerun`], then SQLite's; never the workspace's or
    /// the run manager's, since it has the copy it was handed and the run's
    /// own slot. A run that Succeeded, or Failed with its code, is saved,
    /// with `rerun_of`, if `project` is still the one open here; any other
    /// end, or a project no longer open, writes nothing and is recorded as
    /// not saved with its closed code. Then the rerun is no longer going.
    fn end_rerun(
        &self,
        ticket: u64,
        project: &ProjectId,
        of: StoredRunId,
        run: RunId,
        copy: &RunCopy,
        slot: &Slot,
    ) {
        // Named for `leave()` at once: the run may end before `start` has
        // returned to `project_rerun`.
        self.going_run.store(run.0, Ordering::SeqCst);
        let mut open = lock(&self.open);
        // Decision 193: dropped by a panic in the save below while still
        // armed, it records the rerun as not saved and no longer going.
        let mut guard = EndGuard {
            session: self,
            ticket,
            project,
            of,
            run,
            armed: true,
        };
        let status = match saveable(copy) {
            Err(code) => RerunStatus::NotSaved(code),
            Ok(()) => match open.as_mut() {
                Some(h) if h.id() == project => match save_rerun_into(h, copy, slot, of) {
                    Ok(stored) => RerunStatus::Saved(stored),
                    Err(r) => RerunStatus::NotSaved(r.code),
                },
                _ => RerunStatus::NotSaved(ProjectCode::NotOpen),
            },
        };
        guard.armed = false;
        let saved = matches!(status, RerunStatus::Saved(_));
        if saved {
            lock(&self.saved).insert((project.clone(), run.0));
        }
        *lock(&self.rerun_runs).entry(run.0).or_insert(false) = saved;
        {
            let mut state = lock(&self.rerun);
            if state.as_ref().map(|s| s.ticket) == Some(ticket) {
                *state = Some(RerunState {
                    ticket,
                    project: project.clone(),
                    of,
                    run: Some(run.0),
                    status,
                });
            }
        }
        self.going.store(false, Ordering::SeqCst);
        drop(open);
        match status {
            RerunStatus::Saved(stored) => tracing::info!(
                project = %project.as_str(),
                run = run.0,
                stored = stored.0,
                "project: rerun saved"
            ),
            RerunStatus::NotSaved(code) => tracing::info!(
                project = %project.as_str(),
                run = run.0,
                code = code.wire(),
                "project: rerun not saved"
            ),
            RerunStatus::Going => {}
        }
    }

    /// `project_current`'s answer (decisions 148 and 187).
    fn current_view(&self) -> ProjectCurrentView {
        let going = lock(&self.rerun)
            .as_ref()
            .filter(|s| s.status == RerunStatus::Going)
            .map(|s| s.project.clone());
        let open = match going {
            Some(project) => Some(project),
            None => self.open_id(),
        };
        ProjectCurrentView {
            rerun: self.rerun_view(open.as_ref()),
            project_id: open.map(|id| id.as_str().to_string()),
        }
    }

    /// The store's own tests, which arm a fault on a rerun's worker and so
    /// run with no workspace session: `h` becomes the open project.
    #[cfg(test)]
    pub(crate) fn hold_open_for_test(&self, h: ProjectHandle) {
        *lock(&self.open) = Some(h);
    }

    /// The store's own tests: `project_rerun` after its workspace check, on
    /// `runs`, and `project_current`'s answer.
    #[cfg(test)]
    pub(crate) fn rerun_for_test(
        self: &Arc<Self>,
        runs: &Arc<Runs>,
        run_id: u64,
    ) -> Res<RerunStartedView> {
        rerun_open(self, lock(&self.open), runs, run_id)
    }

    #[cfg(test)]
    pub(crate) fn current_for_test(&self) -> ProjectCurrentView {
        self.current_view()
    }

    /// The store's own tests: what Close and decision 190's other refused
    /// actions check.
    #[cfg(test)]
    pub(crate) fn not_rerunning_for_test(&self) -> Result<(), ProjectCode> {
        self.not_rerunning()
    }

    /// The last rerun, as `project_current` shows it, if it is in `project`
    /// and its run has been named.
    fn rerun_view(&self, project: Option<&ProjectId>) -> Option<RerunView> {
        let state = lock(&self.rerun).clone()?;
        if Some(&state.project) != project {
            return None;
        }
        let (status, stored_run_id, code) = match state.status {
            RerunStatus::Going => (RerunProgress::Going, None, None),
            RerunStatus::Saved(s) => (RerunProgress::Saved, Some(s.0), None),
            RerunStatus::NotSaved(c) => (RerunProgress::NotSaved, None, Some(c)),
        };
        Some(RerunView {
            run_id: state.run?,
            rerun_of: state.of.0,
            status,
            stored_run_id,
            code,
        })
    }

    /// A new confirmation for `project`; the one before it, for any
    /// project, is void from now on.
    pub(crate) fn issue(&self, project: &ProjectId) -> Result<ConfirmationId, ProjectRefusal> {
        let now = self.clock.now_ms();
        lock(&self.book)
            .issue(project.clone(), now)
            .map_err(|NoRandomness| ProjectRefusal::from(ProjectCode::IoFailed))
    }

    /// Uses the confirmation for deleting `project`: refused when there is
    /// none, when it is unknown, used, replaced, expired, or for another
    /// project. Any confirmation found is used up by this call.
    pub(crate) fn redeem(
        &self,
        project: &ProjectId,
        given: Option<&str>,
    ) -> Result<(), ProjectCode> {
        let now = self.clock.now_ms();
        lock(&self.book)
            .redeem(project, given, now)
            .map_err(|r| match r {
                Refused::Missing => ProjectCode::ConfirmationMissing,
                Refused::Unknown => ProjectCode::ConfirmationUnknown,
                Refused::Used => ProjectCode::ConfirmationUsed,
                Refused::Replaced => ProjectCode::ConfirmationReplaced,
                Refused::Expired => ProjectCode::ConfirmationExpired,
                Refused::OtherSubject => ProjectCode::ConfirmationOtherProject,
            })
    }
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
    /// The last rerun this copy started in that project, once its run has a
    /// number (Phase 5C-1a, decision 187); `null` otherwise.
    pub rerun: Option<RerunView>,
}

/// Where a rerun stands (decision 187): going until its save at the end has
/// returned, then saved or not saved.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RerunProgress {
    Going,
    Saved,
    NotSaved,
}

/// A rerun, as `project_current` shows it: its in-memory run, the saved run
/// it repeats, where it stands, and the saved run it became or the closed
/// code it was not saved with. Ids, a word and a code; never a value.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RerunView {
    pub run_id: u64,
    pub rerun_of: u64,
    pub status: RerunProgress,
    pub stored_run_id: Option<u64>,
    pub code: Option<ProjectCode>,
}

/// `project_rerun`'s answer: the rerun's in-memory run, and its kind, whose
/// status and cancel commands read it.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RerunStartedView {
    pub run_id: u64,
    pub kind: RunKind,
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
    /// The stored run this one repeats, if it is a rerun (Phase 5C-1a).
    pub rerun_of: Option<u64>,
    /// For a rerun that Succeeded, its result beside the repeated run's, by
    /// their manifests' output hashes (decision 189); `null` otherwise.
    pub comparison: Option<RerunComparison>,
    /// For a rerun that Succeeded, whether it ran on another engine version
    /// than the run it repeats; `null` when that is not known.
    pub engine_version_differs: Option<bool>,
    /// For a rerun that Succeeded, the platform its manifest names, and that
    /// of the run it repeats (Research OS, Phase 8I, decision 366). Each of
    /// these four is absent when not known, so a view without them is the
    /// same bytes as before.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub platform: Option<Platform>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repeated_platform: Option<Platform>,
    /// Whether the two platforms differ, known when both manifests name one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub platform_differs: Option<bool>,
    /// For a rerun whose output hash differs and whose platform differs,
    /// whether every number agrees with the repeated result to within its
    /// last digits (`research_project::numbers_agree_to_last_digits`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub numbers_agree_to_last_digits: Option<bool>,
    /// The run used a recipe: the loaded graph holds a `used` edge of role
    /// `recipe` for it (Research OS, Phase 8T-2b, decision 516; §11 D309).
    /// Sent only when true, so a view without it is the same bytes as before.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub prepared: bool,
}

/// A recipe the open project keeps, as `project_recipes` gives it (Research
/// OS, Phase 8U-3, decision 552): the stored runs that used it, from the
/// loaded graph's `used` edges of role `recipe`, and what its file says.
/// `sourceColumns` and `scores` are `null` when the file cannot be read as an
/// artifact or as a recipe. Its `Debug` prints counts only: a derived one
/// would print the names, and no scan covers an app view.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecipeView {
    /// Ascending; a rerun's save reuses its recipe by hash, so reruns are here.
    pub used_by: Vec<u64>,
    pub source_columns: Option<u32>,
    pub scores: Option<Vec<ScoreView>>,
}

impl std::fmt::Debug for RecipeView {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RecipeView")
            .field("used_by", &self.used_by.len())
            .field("scores", &self.scores.as_ref().map(Vec::len))
            .finish_non_exhaustive()
    }
}

/// One score of a kept recipe: its column's C-number, the name as the user
/// typed it, its statistic and how many items it has (decision 552). The
/// name is in memory only, for the project page; never in a log or a refusal.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScoreView {
    pub position: u32,
    pub name: String,
    pub statistic: Option<Statistic>,
    pub items: u64,
}

/// Counts only, never the name, as `ScaleScoreStep`'s.
impl std::fmt::Debug for ScoreView {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ScoreView")
            .field("position", &self.position)
            .field("items", &self.items)
            .finish_non_exhaustive()
    }
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

/// The spec's preparation, if it has one (decision 433). One that does not read
/// back as a preparation is no spec this build wrote.
fn spec_preparation(spec_json: &[u8]) -> Result<Option<Preparation>, ProjectCode> {
    let Ok(v) = serde_json::from_slice::<Value>(spec_json) else {
        return Ok(None);
    };
    match v.get("preparation") {
        None | Some(Value::Null) => Ok(None),
        Some(p) => serde_json::from_value(p.clone())
            .map(Some)
            .map_err(|_| ProjectCode::DataMismatch),
    }
}

/// A run on prepared data, against its own record and the slot a Save would
/// keep its file from (decision 433). Since Phase 8T-1a (decision 491) the
/// record keeps the recipe the run ran on ([`RunInput::recipe`]), so the recipe
/// comes from the record, never the slot: a recipe replaced or cleared on the
/// slot since the run leaves the run savable, with its own recipe. The file's
/// bytes still come from the slot. Each comparison is its own, so none can
/// stand in for another.
fn check_prepared(
    copy: &RunCopy,
    input: &RunInput,
    source: &SourceFile,
    preparation: &Preparation,
) -> Result<(), ProjectCode> {
    let recipe = input.recipe.as_deref().ok_or(ProjectCode::DataMismatch)?;
    if ContentHash::of_bytes(recipe) != preparation.recipe_hash {
        return Err(ProjectCode::DataMismatch);
    }
    // The run's input is always the source's (runs.rs's `queue`).
    if source.sha256() != &input.sha256 {
        return Err(ProjectCode::DataMismatch);
    }
    if source.sha256() != &preparation.source_hash {
        return Err(ProjectCode::DataMismatch);
    }
    let prepared_input = ContentHash::from_hex(&stats_adapters::prep_input_hash(
        source.sha256().as_hex(),
        preparation.recipe_hash.as_hex(),
    ))
    .ok_or(ProjectCode::DataMismatch)?;
    if copy.spec_json.as_deref().and_then(spec_input).as_ref() != Some(&prepared_input) {
        return Err(ProjectCode::DataMismatch);
    }
    if copy.state == RunState::Succeeded
        && copy.manifest_inputs.as_slice() != std::slice::from_ref(&prepared_input)
    {
        return Err(ProjectCode::DataMismatch);
    }
    Ok(())
}

/// The data file a Save keeps: `slot`'s copy of the original bytes, only
/// when it is byte for byte the file the run read (decision 123): it hashes
/// to what the run recorded when it started, which is also the spec's input
/// hash and, for a run that Succeeded, the manifest's. A run on prepared data
/// names the prepared input instead, so [`check_prepared`] compares it (decision
/// 433). A Save and a rerun's save at the end check their slot here, the same
/// way.
fn data_of(
    copy: &RunCopy,
    input: &RunInput,
    slot: &Slot,
) -> Result<(Vec<u8>, DataFormat, ArtifactHash), ProjectCode> {
    let source = slot.source().ok_or(ProjectCode::DataUnavailable)?;
    let preparation = copy
        .spec_json
        .as_deref()
        .map(spec_preparation)
        .transpose()?;
    if let Some(preparation) = preparation.flatten() {
        check_prepared(copy, input, source, &preparation)?;
    } else {
        let manifest_agrees = copy.state != RunState::Succeeded
            || copy.manifest_inputs.as_slice() == std::slice::from_ref(&input.sha256);
        if source.sha256() != &input.sha256
            || copy.spec_json.as_deref().and_then(spec_input).as_ref() != Some(&input.sha256)
            || !manifest_agrees
        {
            return Err(ProjectCode::DataMismatch);
        }
    }
    Ok((
        source.bytes().to_vec(),
        source.format(),
        ArtifactHash::parse(input.sha256.as_hex())?,
    ))
}

/// What a Save keeps of a run's recipe, for a run whose spec names one: its
/// canonical bytes, from the run's own record [`data_of`] has checked (decision
/// 491), and the hash the spec recorded.
fn recipe_of(
    copy: &RunCopy,
    input: &RunInput,
) -> Result<Option<(Vec<u8>, ArtifactHash)>, ProjectCode> {
    let preparation = copy
        .spec_json
        .as_deref()
        .map(spec_preparation)
        .transpose()?;
    let Some(preparation) = preparation.flatten() else {
        return Ok(None);
    };
    let recipe = input.recipe.clone().ok_or(ProjectCode::DataMismatch)?;
    Ok(Some((
        recipe,
        ArtifactHash::parse(preparation.recipe_hash.as_hex())?,
    )))
}

/// `req` with the run's recipe, if it has one: an artifact of kind `recipe` and
/// a `used` edge with its role. It is expected to hash to what the run's spec
/// recorded, not to whatever the bytes in hand hash to, so the store's
/// `project.hash_mismatch` stays the last check, as for the data file (decision
/// 433).
fn with_recipe(mut req: SaveRequest, recipe: Option<(Vec<u8>, ArtifactHash)>) -> SaveRequest {
    if let Some((bytes, expected)) = recipe {
        req.run.used.push((expected.clone(), UsedRole::Recipe));
        req.artifacts.push(ArtifactInput {
            kind: ArtifactKind::Recipe,
            format: DataFormat::Json,
            schema_version: 1,
            expected,
            bytes,
        });
    }
    req
}

/// What a Save and a rerun's save at the end both write, checked as each checks
/// its slot: the data file, the recipe of a run on prepared data, the spec and
/// the outputs.
fn request_from_slot(
    copy: &RunCopy,
    input: &RunInput,
    slot: &Slot,
    now: i64,
) -> Result<SaveRequest, ProjectCode> {
    let data = data_of(copy, input, slot)?;
    let recipe = recipe_of(copy, input)?;
    Ok(with_recipe(request_for(copy, data, now)?, recipe))
}

/// A rerun, saved (decision 186): the same request a Save builds, its data
/// file the run's own slot's copy, checked as a Save checks a slot's, through
/// the one inner save a Save uses, with `rerun_of`.
fn save_rerun_into(
    h: &mut ProjectHandle,
    copy: &RunCopy,
    slot: &Slot,
    of: StoredRunId,
) -> Result<StoredRunId, ProjectRefusal> {
    let input = copy.input.as_ref().ok_or(ProjectCode::DataUnavailable)?;
    let req = request_from_slot(copy, input, slot, gaply_core::now_epoch())?;
    h.save_rerun(req, of)
}

/* ------------------------------------------------------------------------ *
 *  The methods a project runs again and exports: one table (Research OS,
 *  Phase 8C, decision 249)
 * ------------------------------------------------------------------------ */

/// One method's row, keyed by its capability id: how its stored spec is read
/// back exactly and its rerun checked and started (through what that gives),
/// which export parameters its spec gives, and which code type its outputs
/// use. Private, as everything here that names a method is.
struct MethodRow {
    capability: &'static str,
    /// The run manager's word for it, which `project_rerun` answers with.
    kind: RunKind,
    /// Check 8: the stored spec, read back to exactly its bytes as this
    /// method's spec.
    read_back: fn(&[u8]) -> Result<Box<dyn Rerun>, ProjectCode>,
    /// The exports' input for one run: its settings from its spec, by the
    /// allow-list, and its codes and fingerprints from its outputs, read in
    /// this method's codes.
    export:
        fn(&mut ProjectHandle, Value, &[ArtifactRef], &mut ExportRun) -> Result<(), ProjectRefusal>,
}

/// Every method a project runs again and exports. A new method adds one row.
const METHODS: &[MethodRow] = &[
    MethodRow {
        capability: stats_desc::CAPABILITY_ID,
        kind: RunKind::Describe,
        read_back: read_back::<OperationSpec<DescribeParams>>,
        export: export_with::<OperationSpec<DescribeParams>>,
    },
    MethodRow {
        capability: stats_ttest::CAPABILITY_ID,
        kind: RunKind::Ttest,
        read_back: read_back::<OperationSpec<TTestParams>>,
        export: export_with::<OperationSpec<TTestParams>>,
    },
    // Research OS, Phase 8D-2a (decision 264).
    MethodRow {
        capability: stats_onesample::ONE_SAMPLE_ID,
        kind: RunKind::OneSample,
        read_back: read_back::<OperationSpec<OneSampleParams>>,
        export: export_with::<OperationSpec<OneSampleParams>>,
    },
    MethodRow {
        capability: stats_onesample::PAIRED_ID,
        kind: RunKind::Paired,
        read_back: read_back::<OperationSpec<PairedParams>>,
        export: export_with::<OperationSpec<PairedParams>>,
    },
    // Research OS, Phase 8G-2 (decision 330).
    MethodRow {
        capability: stats_anova::CAPABILITY_ID,
        kind: RunKind::Anova,
        read_back: read_back::<OperationSpec<AnovaParams>>,
        export: export_with::<OperationSpec<AnovaParams>>,
    },
    // Research OS, Phase 8H-2 (decision 357).
    MethodRow {
        capability: stats_corr::CAPABILITY_ID,
        kind: RunKind::Correlations,
        read_back: read_back::<OperationSpec<CorrParams>>,
        export: export_with::<OperationSpec<CorrParams>>,
    },
    // Research OS, Phase 8I-2 (decision 382).
    MethodRow {
        capability: stats_alpha::CAPABILITY_ID,
        kind: RunKind::CronbachAlpha,
        read_back: read_back::<OperationSpec<AlphaParams>>,
        export: export_with::<OperationSpec<AlphaParams>>,
    },
    // Research OS, Phase 8X-2 (decision 590).
    MethodRow {
        capability: stats_xtab::CAPABILITY_ID,
        kind: RunKind::Crosstabs,
        read_back: read_back::<OperationSpec<CrosstabsParams>>,
        export: export_with::<OperationSpec<CrosstabsParams>>,
    },
];

/// The row of a capability, if a method has one.
fn method_for(capability: &str) -> Option<&'static MethodRow> {
    METHODS.iter().find(|m| m.capability == capability)
}

/// A stored spec, read back: how its rerun is checked and started.
trait Rerun {
    /// Check 9: the spec passes validation, as an ordinary run's must: its
    /// workspace and capability, then its method's own check, through its
    /// façade.
    fn valid_on(&self, build: Build, dataset: &Dataset) -> bool;
    /// The rerun's run, with its end hook.
    fn launch(
        self: Box<Self>,
        runs: &Arc<Runs>,
        slot: Arc<Slot>,
        end: EndHook,
    ) -> Result<RunId, NotStarted>;
}

/// One method's spec, as a project's table reads it.
trait SavedSpec: Rerun + Serialize + DeserializeOwned + 'static {
    /// The codes its outputs are in.
    type Code: Closed + DeserializeOwned;
    /// Its settings, by the allow-list.
    fn parameters(&self) -> Vec<ExportParameter>;
}

fn read_back<S: SavedSpec>(stored: &[u8]) -> Result<Box<dyn Rerun>, ProjectCode> {
    Ok(Box::new(exact::<S>(stored)?))
}

fn export_with<S: SavedSpec>(
    h: &mut ProjectHandle,
    spec: Value,
    generated: &[ArtifactRef],
    run: &mut ExportRun,
) -> Result<(), ProjectRefusal> {
    run.parameters = parse::<S>(spec)?.parameters();
    outputs::<S::Code>(h, generated, run)
}

impl Rerun for OperationSpec<DescribeParams> {
    fn valid_on(&self, build: Build, dataset: &Dataset) -> bool {
        check_spec(
            WorkspaceId::Statistics,
            self.workspace_id,
            &self.capability_id,
        )
        .is_ok()
            && passes(stats_adapters::describe_validate(self, build, dataset).status)
    }

    fn launch(
        self: Box<Self>,
        runs: &Arc<Runs>,
        slot: Arc<Slot>,
        end: EndHook,
    ) -> Result<RunId, NotStarted> {
        runs.start_then(slot, *self, end)
    }
}

impl SavedSpec for OperationSpec<DescribeParams> {
    type Code = DescCode;

    fn parameters(&self) -> Vec<ExportParameter> {
        describe_parameters(self)
    }
}

impl Rerun for OperationSpec<TTestParams> {
    fn valid_on(&self, build: Build, dataset: &Dataset) -> bool {
        check_spec(
            WorkspaceId::Statistics,
            self.workspace_id,
            &self.capability_id,
        )
        .is_ok()
            && passes(stats_adapters::ttest_validate(self, build, dataset).status)
    }

    fn launch(
        self: Box<Self>,
        runs: &Arc<Runs>,
        slot: Arc<Slot>,
        end: EndHook,
    ) -> Result<RunId, NotStarted> {
        runs.start_ttest_then(slot, *self, end)
    }
}

impl SavedSpec for OperationSpec<TTestParams> {
    type Code = TTestCode;

    fn parameters(&self) -> Vec<ExportParameter> {
        ttest_parameters(self)
    }
}

impl Rerun for OperationSpec<OneSampleParams> {
    fn valid_on(&self, build: Build, dataset: &Dataset) -> bool {
        check_spec(
            WorkspaceId::Statistics,
            self.workspace_id,
            &self.capability_id,
        )
        .is_ok()
            && passes(stats_adapters::one_sample_validate(self, build, dataset).status)
    }

    fn launch(
        self: Box<Self>,
        runs: &Arc<Runs>,
        slot: Arc<Slot>,
        end: EndHook,
    ) -> Result<RunId, NotStarted> {
        runs.start_one_sample_then(slot, *self, end)
    }
}

impl SavedSpec for OperationSpec<OneSampleParams> {
    type Code = OneSampleCode;

    fn parameters(&self) -> Vec<ExportParameter> {
        one_sample_parameters(self)
    }
}

impl Rerun for OperationSpec<PairedParams> {
    fn valid_on(&self, build: Build, dataset: &Dataset) -> bool {
        check_spec(
            WorkspaceId::Statistics,
            self.workspace_id,
            &self.capability_id,
        )
        .is_ok()
            && passes(stats_adapters::paired_validate(self, build, dataset).status)
    }

    fn launch(
        self: Box<Self>,
        runs: &Arc<Runs>,
        slot: Arc<Slot>,
        end: EndHook,
    ) -> Result<RunId, NotStarted> {
        runs.start_paired_then(slot, *self, end)
    }
}

impl SavedSpec for OperationSpec<PairedParams> {
    type Code = OneSampleCode;

    fn parameters(&self) -> Vec<ExportParameter> {
        paired_parameters(self)
    }
}

impl Rerun for OperationSpec<AnovaParams> {
    fn valid_on(&self, build: Build, dataset: &Dataset) -> bool {
        check_spec(
            WorkspaceId::Statistics,
            self.workspace_id,
            &self.capability_id,
        )
        .is_ok()
            && passes(stats_adapters::anova_validate(self, build, dataset).status)
    }

    fn launch(
        self: Box<Self>,
        runs: &Arc<Runs>,
        slot: Arc<Slot>,
        end: EndHook,
    ) -> Result<RunId, NotStarted> {
        runs.start_anova_then(slot, *self, end)
    }
}

impl SavedSpec for OperationSpec<AnovaParams> {
    type Code = AnovaCode;

    fn parameters(&self) -> Vec<ExportParameter> {
        anova_parameters(self)
    }
}

impl Rerun for OperationSpec<CorrParams> {
    fn valid_on(&self, build: Build, dataset: &Dataset) -> bool {
        check_spec(
            WorkspaceId::Statistics,
            self.workspace_id,
            &self.capability_id,
        )
        .is_ok()
            && passes(stats_adapters::corr_validate(self, build, dataset).status)
    }

    fn launch(
        self: Box<Self>,
        runs: &Arc<Runs>,
        slot: Arc<Slot>,
        end: EndHook,
    ) -> Result<RunId, NotStarted> {
        runs.start_corr_then(slot, *self, end)
    }
}

impl SavedSpec for OperationSpec<CorrParams> {
    type Code = CorrCode;

    fn parameters(&self) -> Vec<ExportParameter> {
        corr_parameters(self)
    }
}

impl Rerun for OperationSpec<AlphaParams> {
    fn valid_on(&self, build: Build, dataset: &Dataset) -> bool {
        check_spec(
            WorkspaceId::Statistics,
            self.workspace_id,
            &self.capability_id,
        )
        .is_ok()
            && passes(stats_adapters::alpha_validate(self, build, dataset).status)
    }

    fn launch(
        self: Box<Self>,
        runs: &Arc<Runs>,
        slot: Arc<Slot>,
        end: EndHook,
    ) -> Result<RunId, NotStarted> {
        runs.start_alpha_then(slot, *self, end)
    }
}

impl SavedSpec for OperationSpec<AlphaParams> {
    type Code = AlphaCode;

    fn parameters(&self) -> Vec<ExportParameter> {
        alpha_parameters(self)
    }
}

// Research OS, Phase 8X-2 (decision 590): crosstabs, checked through its
// façade, never stats_xtab itself (verifier_facade_guards.rs).
impl Rerun for OperationSpec<CrosstabsParams> {
    fn valid_on(&self, build: Build, dataset: &Dataset) -> bool {
        check_spec(
            WorkspaceId::Statistics,
            self.workspace_id,
            &self.capability_id,
        )
        .is_ok()
            && passes(stats_adapters::xtab_validate(self, build, dataset).status)
    }

    fn launch(
        self: Box<Self>,
        runs: &Arc<Runs>,
        slot: Arc<Slot>,
        end: EndHook,
    ) -> Result<RunId, NotStarted> {
        runs.start_xtab_then(slot, *self, end)
    }
}

impl SavedSpec for OperationSpec<CrosstabsParams> {
    type Code = XtabCode;

    fn parameters(&self) -> Vec<ExportParameter> {
        xtab_parameters(self)
    }
}

/// The method of a stored run, if this build can run it again (decision
/// 188): its capability and capability version, from its row, are runnable
/// here, and the capability has a row in [`METHODS`].
fn rerun_method(stored: &StoredRun) -> Result<&'static MethodRow, ProjectCode> {
    stats_registry::require_runnable(
        &stored.capability_id,
        stored.capability_version,
        crate::workspace::build(),
    )
    .map_err(|_| ProjectCode::RerunUnsupported)?;
    method_for(stored.capability_id.as_str()).ok_or(ProjectCode::RerunUnsupported)
}

/// A stored spec, read back, only if it serialises again to exactly its
/// stored bytes (decision 188): a number read back one unit in the last
/// place away would make an honest rerun look different (decision 129).
fn exact<T: Serialize + DeserializeOwned>(stored: &[u8]) -> Result<T, ProjectCode> {
    let spec: T = serde_json::from_slice(stored).map_err(|_| ProjectCode::RerunUnsupported)?;
    match serde_json::to_vec(&spec) {
        Ok(again) if again == stored => Ok(spec),
        _ => Err(ProjectCode::RerunUnsupported),
    }
}

/// Whether a validation lets a run be queued.
fn passes(status: ValidationStatus) -> bool {
    matches!(
        status,
        ValidationStatus::Valid | ValidationStatus::ValidWithWarnings
    )
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
    // Held to the end, so nothing opens the project between its close and its
    // deletion. F8 (decision 226): the confirmation is redeemed under this
    // lock, after the rerun-going check, so a delete refused for a going
    // rerun leaves its confirmation good. `redeem`/`issue` take only the
    // confirmation book and acquire no other lock while holding it, so this
    // open-then-book order cannot deadlock.
    let mut open = lock(&projects.open);
    let closed = open.as_ref().is_some_and(|h| h.id() == &id);
    if closed {
        // Decision 190: not the open project while a rerun is going.
        projects.not_rerunning().map_err(refuse)?;
    }
    projects.redeem(&id, confirmation).map_err(refuse)?;
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
///
/// With it, the last rerun this copy started in that project (decision 187).
/// While a rerun is going, the open project cannot change and its save at
/// the end may hold the open project's lock, so the answer is read from the
/// rerun's own state, without waiting for that lock.
pub fn current(ws: &WorkspaceSession) -> Res<ProjectCurrentView> {
    Ok(ws.projects().current_view())
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
        // Decision 190: a Save into a new project would close the open one.
        SaveTarget::NewProject { .. } => {
            projects.not_rerunning().map_err(refuse)?;
            None
        }
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
    // Decision 187: a rerun's run is saved only by its save at the end, into
    // either project; by hand it is already saved, or its data is held by
    // no open dataset.
    if let Some(saved) = lock(&projects.rerun_runs).get(&run_id).copied() {
        return Err(refuse(if saved {
            ProjectCode::RunAlreadySaved
        } else {
            ProjectCode::DataUnavailable
        }));
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
    let req = request_from_slot(&copy, input, &slot, gaply_core::now_epoch()).map_err(refuse)?;

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
        // Decision 190: opening another project would close the open one.
        projects.not_rerunning().map_err(refuse)?;
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
    let projects = ws.projects();
    let mut open = lock(&projects.open);
    // Decision 190: not while a rerun is going.
    projects.not_rerunning().map_err(refuse)?;
    let closed = match open.take() {
        Some(h) => {
            tracing::info!(project = %h.id().as_str(), "project: closed");
            h.close();
            true
        }
        None => false,
    };
    Ok(ProjectClosedView { closed })
}

/// The open project's saved runs. For each rerun that Succeeded, its
/// comparison with the run it repeats (decision 189), read from the two
/// manifests, each verified as it is read: a manifest that cannot be read
/// leaves the pair not compared, and is marked damaged as any read marks it.
pub fn runs(_root: &Path, ws: &WorkspaceSession, project_id: &str) -> Res<Vec<StoredRunView>> {
    let _engine = ws.require_statistics().map_err(refuse_workspace)?;
    let mut open = lock(&ws.projects().open);
    still_active(ws)?;
    let h = the_open(&mut open, project_id).map_err(refuse)?;
    let loaded = load(h).map_err(refuse)?;
    let mut out = Vec::new();
    for r in &loaded.runs {
        let (mut comparison, mut engine_version_differs) = (None, None);
        let (mut platform, mut repeated_platform) = (None, None);
        let (mut platform_differs, mut numbers_agree) = (None, None);
        if let (Some(of), RunState::Succeeded) = (r.rerun_of, r.state) {
            let again = manifest_facts(h, &loaded, r.run_id);
            let before = manifest_facts(h, &loaded, of);
            let (c, e) = compare_rerun(
                again.as_ref().map(|(o, e, _)| (o, *e)),
                before.as_ref().map(|(o, e, _)| (o, *e)),
            );
            (comparison, engine_version_differs) = (Some(c), e);
            // Decision 366: the platforms, and for a different result on a
            // different platform, its numbers beside the repeated one's.
            platform = again.and_then(|(_, _, p)| p);
            repeated_platform = before.and_then(|(_, _, p)| p);
            platform_differs = platforms_differ(platform, repeated_platform);
            if c == RerunComparison::Different && platform_differs == Some(true) {
                let a = result_bundle(h, &loaded, r.run_id);
                let b = result_bundle(h, &loaded, of);
                numbers_agree =
                    numbers_agree_to_last_digits(c, platform_differs, a.as_ref(), b.as_ref());
            }
        }
        out.push(StoredRunView {
            stored_run_id: r.run_id.0,
            capability_id: r.capability_id.as_str().to_string(),
            capability_version: r.capability_version,
            state: r.state,
            end_code: r.end_code,
            started_at: r.started_at,
            ended_at: r.ended_at,
            saved_at: r.saved_at,
            agent_version: r.agent.as_str().to_string(),
            rerun_of: r.rerun_of.map(|s| s.0),
            comparison,
            engine_version_differs,
            platform,
            repeated_platform,
            platform_differs,
            numbers_agree_to_last_digits: numbers_agree,
            prepared: used_a_recipe(&loaded.graph, r.run_id),
        });
    }
    Ok(out)
}

/// Whether the graph holds a `used` edge of role `recipe` for `run`, as
/// [`artifacts_of`] reads that edge, from the graph already loaded (decision
/// 516). Not through `artifacts_of`, which refuses a run with two recipe
/// edges or a missing data or spec edge and so would refuse the whole listing.
fn used_a_recipe(graph: &ProvGraph, run: StoredRunId) -> bool {
    graph.edges().iter().any(|e| {
        matches!(
            e,
            ProvEdge::Used {
                run: r,
                role: UsedRole::Recipe,
                ..
            } if *r == run
        )
    })
}

/// The open project's recipes (Research OS, Phase 8U-3, decision 552), for its
/// page: each recipe its runs used, from the loaded graph's `used` edges of
/// role `recipe`, as [`used_a_recipe`] reads them and never through
/// [`artifacts_of`], which refuses a run with two recipe edges: such a run is
/// listed under both. In the order of each recipe's first user. A recipe whose
/// file cannot be read as an artifact, or as a recipe, is listed with its
/// file's fields `null`, and the read goes on; a database error refuses it.
pub fn recipes(_root: &Path, ws: &WorkspaceSession, project_id: &str) -> Res<Vec<RecipeView>> {
    let _engine = ws.require_statistics().map_err(refuse_workspace)?;
    let mut open = lock(&ws.projects().open);
    still_active(ws)?;
    let h = the_open(&mut open, project_id).map_err(refuse)?;
    let loaded = load(h).map_err(refuse)?;
    let mut users: BTreeMap<ArtifactHash, BTreeSet<u64>> = BTreeMap::new();
    for e in loaded.graph.edges() {
        if let ProvEdge::Used {
            run,
            artifact,
            role: UsedRole::Recipe,
        } = e
        {
            users.entry(artifact.clone()).or_default().insert(run.0);
        }
    }
    let mut out = Vec::with_capacity(users.len());
    for (hash, by) in users {
        let read = kept_recipe(h, &hash).map_err(refuse)?;
        out.push(RecipeView {
            used_by: by.into_iter().collect(),
            source_columns: read.as_ref().map(|r| r.source_columns),
            scores: read.as_ref().map(scores_of),
        });
    }
    out.sort_by_key(|v| v.used_by.first().copied());
    Ok(out)
}

/// A kept recipe, read and verified: `None` when `read_artifact` refuses it as
/// an artifact (an unknown row, a row already damaged, a missing file, a file
/// not regular, a wrong size, a wrong hash, each marking damage as any read
/// does) or its bytes are no recipe. Any other refusal is the database's, and
/// is returned.
fn kept_recipe(h: &mut ProjectHandle, hash: &ArtifactHash) -> Result<Option<Recipe>, ProjectRefusal> {
    match h.read_artifact(hash.as_hex()) {
        Ok(bytes) => Ok(stats_adapters::prep_recipe_from_bytes(&bytes).ok()),
        Err(e)
            if matches!(
                e.code,
                ProjectCode::UnknownArtifact
                    | ProjectCode::ArtifactMissing
                    | ProjectCode::ArtifactNotRegular
                    | ProjectCode::ArtifactWrongSize
                    | ProjectCode::ArtifactWrongHash
            ) =>
        {
            Ok(None)
        }
        Err(e) => Err(e),
    }
}

/// A recipe's scores, by ascending position (decision 552).
fn scores_of(recipe: &Recipe) -> Vec<ScoreView> {
    let mut out: Vec<ScoreView> = recipe
        .steps
        .iter()
        .map(|step| {
            let RecipeStep::ScaleScore(s) = step;
            ScoreView {
                position: s.alias.0 .0,
                name: s.name.clone(),
                statistic: s.statistic,
                items: s.items.len() as u64,
            }
        })
        .collect();
    out.sort_by_key(|s| s.position);
    out
}

/// The open project's runs, artifact rows and provenance graph, read once per
/// command and passed down (5E-fix-1, F3): `project_runs`, both exports and a
/// run's result load this once, never once per run.
struct Loaded {
    runs: Vec<StoredRun>,
    artifacts: Vec<ArtifactRef>,
    graph: ProvGraph,
}

/// Reads the runs, the artifact rows and the graph once. A single
/// `load_graph` (F3): the counter `project_store` keeps goes up by one.
fn load(h: &ProjectHandle) -> Result<Loaded, ProjectRefusal> {
    Ok(Loaded {
        runs: h.runs()?,
        artifacts: h.artifacts()?,
        graph: h.provenance()?,
    })
}

/// A run's manifest's output hash and engine version, if its manifest reads
/// back whole: the two fields decision 189 compares; with them its platform,
/// `None` for a manifest that names none or one this build cannot read
/// (decision 366).
fn manifest_facts(
    h: &mut ProjectHandle,
    loaded: &Loaded,
    run: StoredRunId,
) -> Option<(ContentHash, u32, Option<Platform>)> {
    let (_, arts) = artifacts_of(loaded, run).ok()?;
    let manifest = arts
        .generated
        .iter()
        .find(|a| a.kind == ArtifactKind::RunManifest)?;
    let v = read_json(h, manifest).ok()?;
    let output = ContentHash::from_hex(v.get("outputHash")?.as_str()?)?;
    let engine = u32::try_from(v.get("engineVersion")?.as_u64()?).ok()?;
    let platform = v
        .get("platform")
        .and_then(|p| serde_json::from_value::<Platform>(p.clone()).ok());
    Some((output, engine, platform))
}

/// A run's result bundle as JSON, read and verified as a run's result is,
/// within [`RESULT_READ_LIMIT_BYTES`]; `None` when it has none, is larger,
/// or cannot be read (decision 366). A read that fails marks it damaged, as
/// any read does.
fn result_bundle(h: &mut ProjectHandle, loaded: &Loaded, run: StoredRunId) -> Option<Value> {
    let (_, arts) = artifacts_of(loaded, run).ok()?;
    let bundle = arts
        .generated
        .iter()
        .find(|a| a.kind == ArtifactKind::ResultBundle)?;
    if bundle.bytes > RESULT_READ_LIMIT_BYTES {
        return None;
    }
    read_json(h, bundle).ok()
}

/// A run's artifacts by what they were to it: the data file, the spec, the
/// recipe of a run on prepared data, and what it generated, each with its row.
struct RunArtifacts {
    data: ArtifactRef,
    spec: ArtifactRef,
    recipe: Option<ArtifactRef>,
    generated: Vec<ArtifactRef>,
}

/// A run's artifacts, from the already-loaded rows and graph (5E-fix-1, F3):
/// no database read of its own.
fn artifacts_of(
    loaded: &Loaded,
    run: StoredRunId,
) -> Result<(StoredRun, RunArtifacts), ProjectRefusal> {
    let stored = loaded
        .runs
        .iter()
        .find(|r| r.run_id == run)
        .cloned()
        .ok_or(ProjectCode::StoredRunUnknown)?;
    let row = |hash: &ArtifactHash| -> Result<ArtifactRef, ProjectRefusal> {
        loaded
            .artifacts
            .iter()
            .find(|a| &a.hash == hash)
            .cloned()
            .ok_or_else(|| ProjectCode::UnknownArtifact.into())
    };
    let (mut data, mut spec, mut recipe, mut generated) = (None, None, None, Vec::new());
    for e in loaded.graph.edges() {
        match e {
            ProvEdge::Used {
                run: r,
                artifact,
                role,
            } if *r == run => match role {
                UsedRole::Data => data = Some(row(artifact)?),
                UsedRole::Spec => spec = Some(row(artifact)?),
                // A run has one recipe at most (decision 432): a second edge is
                // not what a Save writes, so which one it read cannot be known.
                UsedRole::Recipe if recipe.is_some() => {
                    return Err(ProjectCode::UnknownArtifact.into())
                }
                UsedRole::Recipe => recipe = Some(row(artifact)?),
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
                recipe,
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
    let loaded = load(h).map_err(refuse)?;
    let (stored, arts) = artifacts_of(&loaded, StoredRunId(stored_run_id)).map_err(refuse)?;
    let shown: u64 = arts.spec.bytes + arts.generated.iter().map(|a| a.bytes).sum::<u64>();
    if shown > RESULT_READ_LIMIT_BYTES {
        return Err(refuse(ProjectRefusal::over(
            ProjectCode::ResultTooLarge,
            shown,
            RESULT_READ_LIMIT_BYTES,
        )));
    }
    // Verified, and dropped: the data file's bytes never leave the store, nor
    // do the recipe's (decision 433).
    h.read_artifact(arts.data.hash.as_hex()).map_err(refuse)?;
    if let Some(recipe) = &arts.recipe {
        h.read_artifact(recipe.hash.as_hex()).map_err(refuse)?;
    }
    let spec = read_text(h, &arts.spec).map_err(refuse)?;
    let (mut bundle, mut manifest, mut failure) = (None, None, None);
    for a in &arts.generated {
        let v = read_text(h, a).map_err(refuse)?;
        match a.kind {
            ArtifactKind::ResultBundle => bundle = Some(v),
            ArtifactKind::RunManifest => manifest = Some(v),
            ArtifactKind::RunFailure => failure = Some(v),
            ArtifactKind::DataFile | ArtifactKind::OperationSpec | ArtifactKind::Recipe => {
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

/// Runs the open project's saved run `run_id` again (decisions 185 to 190).
/// Each refusal writes nothing, except the damage mark that reading a
/// damaged artifact makes. The rerun's save at the end is its end hook's,
/// never the screen's.
pub fn rerun(_root: &Path, ws: &WorkspaceSession, run_id: u64) -> Res<RerunStartedView> {
    let engine = ws.require_statistics().map_err(refuse_workspace)?;
    let projects = ws.projects();
    let open = lock(&projects.open);
    still_active(ws)?;
    rerun_open(projects, open, &engine.runs, run_id)
}

/// The code a rerun gives when the ingest refuses the project's copy of its
/// data file (Research OS, Phase 8F-1, decision 283). The four size
/// refusals that depend on this computer's tier (the file's bytes, its
/// unpacked size, its cell text and its cells) are `rerun_over_limits`: the
/// file can be run again on a computer with more memory. Every other
/// refusal is `rerun_data_unreadable`, as before 8F-1, `file_too_large` and
/// `too_wide` included: every tier has the same rows and columns, so a
/// smaller tier cannot cause them. Exhaustive, so a new refusal must be
/// placed here before it builds. Reachable since 8F-2, when the rerun reads
/// through `IngestLimits::this_computer()` (decision 288).
pub fn rerun_ingest_code(refusal: L1IngestRefusal) -> ProjectCode {
    use L1IngestRefusal as R;
    match refusal {
        R::TooLarge { .. }
        | R::ArchiveTooLarge { .. }
        | R::ContentTooLarge { .. }
        | R::TooManyCells { .. } => ProjectCode::RerunOverLimits,
        R::NotReadable
        | R::UnsupportedFormat
        | R::Malformed
        | R::NoTable
        | R::FileTooLarge { .. }
        | R::FileChangedWhileReading
        | R::XlsNotReadable
        | R::Dataset(_)
        | R::NotARegularFile
        | R::EncodingNotUtf8
        | R::SavBigEndian => ProjectCode::RerunDataUnreadable,
    }
}

/// The code a rerun gives when a stored recipe is refused (Research OS, Phase
/// 8M-4, decision 434). `cells_over_cap` depends on this computer's tier, so it
/// is `rerun_over_limits`, as the ingest's cells are ([`rerun_ingest_code`]):
/// the run can be repeated on a computer with more memory. Every other refusal
/// is `recipe_unusable`. Exhaustive, so a new code must be placed here before it
/// builds.
pub fn rerun_recipe_code(code: PrepCode) -> ProjectCode {
    use PrepCode as P;
    match code {
        P::CellsOverCap => ProjectCode::RerunOverLimits,
        P::RecipeUnreadable
        | P::UnknownCapability
        | P::CapabilityVersionMismatch
        | P::EngineVersionMismatch
        | P::SchemaVersionMismatch
        | P::SourceAlreadyPrepared
        | P::SourceColumnsMismatch
        | P::NoSteps
        | P::AliasNotNext
        | P::TooWide
        | P::NameNotTrimmed
        | P::NameLength
        | P::NameHasInvisibleCharacter
        | P::NameEqualsFileHeader
        | P::NameEqualsOtherScore
        | P::NameIsAnAliasWord
        | P::TooFewItems
        | P::ItemRepeated
        | P::ItemsOutOfOrder
        | P::ItemNotAFileColumn
        | P::ItemNotNumeric
        | P::ItemHoldsText
        | P::ItemNonFinite
        | P::ReversedRepeated
        | P::ReversedOutOfOrder
        | P::ReversedNotAnItem
        | P::ScaleMissing
        | P::ScaleWithoutReversed
        | P::ScaleNotFinite
        | P::ScaleEndsOutOfOrder
        | P::ReversedOutsideScale
        | P::AtLeastOutOfRange
        | P::MissingCodeNotFinite
        | P::MissingCodeRepeated
        | P::MissingCodesOutOfOrder
        | P::StatisticNotChosen
        | P::RuleNotChosen
        | P::NegativeZero
        | P::NoRowScored
        | P::TableRefusedColumn
        | P::SumOverFewerItems
        | P::ScoredFromEveryItem
        | P::ScoredFromFewer
        | P::NotScoredTooFew
        | P::NotScoredOverflow => ProjectCode::RecipeUnusable,
    }
}

/// The code of a report that refused a recipe. A refusal no computer could get
/// past outranks one this computer's tier causes: a recipe that is both too wide
/// and over the cap is not helped by more memory.
fn refused_recipe_code(report: &ValidationReport<PrepCode>) -> ProjectCode {
    let codes = report
        .errors
        .iter()
        .chain(&report.missing_information)
        .map(|d| rerun_recipe_code(d.code));
    let mut over_limits = false;
    for code in codes {
        if code != ProjectCode::RerunOverLimits {
            return code;
        }
        over_limits = true;
    }
    if over_limits {
        ProjectCode::RerunOverLimits
    } else {
        ProjectCode::RecipeUnusable
    }
}

/// The prepared table a stored recipe makes from `dataset`, the project's copy
/// of the file as read again, through the façade calls a live make uses
/// (`stats_prep_commands::make`). The artifact's hash is verified as it is read;
/// nothing is written but the damage mark that reading a damaged one makes.
fn prepared_again(
    h: &mut ProjectHandle,
    recipe: &ArtifactRef,
    build: Build,
    dataset: &Dataset,
    cells_cap: u64,
) -> Result<Prepared, ProjectRefusal> {
    let bytes = h.read_artifact(recipe.hash.as_hex())?;
    let recipe = stats_adapters::prep_recipe_from_bytes(&bytes).map_err(rerun_recipe_code)?;
    let report = stats_adapters::prep_validate(&recipe, build, dataset, cells_cap);
    if !report.may_run() {
        return Err(refused_recipe_code(&report).into());
    }
    stats_adapters::prep_build(&recipe, build, dataset, cells_cap).map_err(|r| {
        ProjectRefusal::from(match r {
            PrepRefusal::Report(report) => refused_recipe_code(&report),
            PrepRefusal::NoRowScored { .. } => rerun_recipe_code(PrepCode::NoRowScored),
            PrepRefusal::Table(d) => rerun_recipe_code(d.code),
        })
    })
}

/// [`rerun`] after its workspace check: checks 2 to 9, in order, under the
/// open project's lock; then the rerun is going, and the lock is released
/// before the run starts, because a run that ends at once (a refusal in the
/// run manager's queue, a failed spawn) calls its end hook on this thread,
/// and the hook takes that lock. The store's own tests call this with a
/// session and a run manager of their own, to arm a fault on the worker.
fn rerun_open(
    projects: &Arc<ProjectSession>,
    mut open: MutexGuard<'_, Option<ProjectHandle>>,
    runs: &Arc<Runs>,
    run_id: u64,
) -> Res<RerunStartedView> {
    // 2. A project is open in this copy.
    let h = open.as_mut().ok_or_else(|| refuse(ProjectCode::NotOpen))?;
    let project = h.id().clone();
    // 3. The stored run exists.
    let of = StoredRunId(run_id);
    let stored = h
        .runs()
        .map_err(refuse)?
        .into_iter()
        .find(|r| r.run_id == of)
        .ok_or_else(|| refuse(ProjectCode::StoredRunUnknown))?;
    // 4. It Succeeded: a saved failure is a record only.
    if stored.state != RunState::Succeeded {
        return Err(refuse(ProjectCode::RerunNotSucceeded));
    }
    // 5. Its capability and capability version are runnable in this build.
    let method = rerun_method(&stored).map_err(refuse)?;
    // 6. No run is going, a rerun included.
    projects.not_rerunning().map_err(refuse)?;
    if runs.unfinished().is_some() {
        return Err(refuse(ProjectCode::RerunBusy));
    }
    // 7. The data file and the spec read back with their hashes.
    let loaded = load(h).map_err(refuse)?;
    let (_, arts) = artifacts_of(&loaded, of).map_err(refuse)?;
    let data = h.read_artifact(arts.data.hash.as_hex()).map_err(refuse)?;
    let spec = h.read_artifact(arts.spec.hash.as_hex()).map_err(refuse)?;
    // 8. The spec reads back to exactly its stored bytes.
    let job = (method.read_back)(&spec).map_err(refuse)?;
    // The project's copy of the data file, through the ingest's own checks
    // and parse, as `stats_desc_open` reads a file.
    let dataset =
        load_dataset_from_bytes_with(&data, arts.data.format, IngestLimits::this_computer())
            .map_err(|r| refuse(rerun_ingest_code(r)))?;
    let build = crate::workspace::build();
    // A run on prepared data: its stored recipe, applied again to the project's
    // copy of the file, as a live make applies one (decision 434).
    let prepared = match &arts.recipe {
        Some(recipe) => Some(
            prepared_again(
                h,
                recipe,
                build,
                &dataset,
                IngestLimits::this_computer().max_cells as u64,
            )
            .map_err(refuse)?,
        ),
        None => None,
    };
    let source = SourceFile::new(
        data,
        arts.data.hash.content_hash().clone(),
        arts.data.format,
    );
    // The one-run slot holds one table: the file's is dropped here once the
    // prepared one is built.
    let slot = match prepared {
        Some(prepared) => {
            let original = dataset.schema.fingerprint.clone();
            drop(dataset);
            Slot::for_one_run_prepared(prepared, original, source)
        }
        None => Slot::for_one_run(dataset, source),
    };
    // 9. The spec passes validation, as an ordinary run's must. A changed recipe
    // or data file cannot pass: the spec names the input they made.
    if !job.valid_on(build, slot.dataset()) {
        return Err(refuse(ProjectCode::RerunRefused));
    }

    // Going from here until its save at the end has returned (decision 190).
    let slot = Arc::new(slot);
    let (ticket, before) = projects.begin_rerun(&project, of);
    drop(open);
    let end = ProjectSession::end_hook(projects, ticket, project.clone(), of, Arc::clone(&slot));
    let started = job.launch(runs, slot, end);
    match started {
        Ok(run) => {
            projects.rerun_named(ticket, run.0);
            tracing::info!(project = %project.as_str(), of = of.0, run = run.0, "project: rerun started");
            Ok(RerunStartedView {
                run_id: run.0,
                kind: method.kind,
            })
        }
        Err(n) => {
            projects.rerun_not_started(before);
            Err(match n {
                NotStarted::Busy => refuse(ProjectCode::RerunBusy),
                // The workspace was left between the check and the start.
                NotStarted::Sealed => refuse_inactive(),
            })
        }
    }
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

/// Every field of a saved describe, t-test, one-sample, paired, one-way
/// ANOVA, correlations or Cronbach's alpha spec, by JSON path (`[]` for each element of an
/// array). The ANOVA shares `outcome`, `group`, `levels[]` and
/// `missingCodes[]` with the t-test's rows, and adds `postHoc[]` (decision
/// 330); the correlations share `missingCodes[]` and add `variables[]`,
/// `coefficients[]` and `missing` (decision 357); Cronbach's alpha shares
/// `missingCodes[]` and adds `items[]`, `reversed[]` and `scale`, null or its
/// `min` and `max` (decision 382); crosstabs shares `missingCodes[]` and adds
/// `rows` and `columns` (decision 590).
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
    // Research OS, Phase 8D-2a (decision 264): the one-sample t-test's test
    // value, a setting the user chose, and the paired t-test's columns.
    ("parameters.testValue", SpecFieldShown::Reported),
    ("parameters.first", SpecFieldShown::Reported),
    ("parameters.second", SpecFieldShown::Reported),
    // Research OS, Phase 8G-2 (decision 330): the one-way ANOVA's post hoc
    // tests, closed words the user chose.
    ("parameters.postHoc[]", SpecFieldShown::Reported),
    // Research OS, Phase 8H-2 (decision 357): the correlations' columns by
    // position, and the coefficients and missing handling, closed words the
    // user chose.
    ("parameters.variables[]", SpecFieldShown::Reported),
    ("parameters.coefficients[]", SpecFieldShown::Reported),
    ("parameters.missing", SpecFieldShown::Reported),
    // Research OS, Phase 8I-2 (decision 382): Cronbach's alpha's items and
    // reverse-scored items by position, and the scale's ends the user typed
    // (null when nothing is reversed).
    ("parameters.items[]", SpecFieldShown::Reported),
    ("parameters.reversed[]", SpecFieldShown::Reported),
    ("parameters.scale", SpecFieldShown::Reported),
    ("parameters.scale.min", SpecFieldShown::Reported),
    ("parameters.scale.max", SpecFieldShown::Reported),
    // Research OS, Phase 8X-2 (decision 590): crosstabs' two columns by
    // position. `parameters.columns` is a string here, distinct from the
    // summary's `parameters.columns[].alias`.
    ("parameters.rows", SpecFieldShown::Reported),
    ("parameters.columns", SpecFieldShown::Reported),
    // Research OS, Phase 8M-4 (decision 435): what made a prepared input,
    // hashes and numbers. Its two hashes are the data file's and the recipe's
    // rows in the report's inputs table; its columns and versions are the lines
    // of `with_preparation`. Leaves only: the field is skipped when absent.
    ("preparation.sourceHash", SpecFieldShown::Reported),
    ("preparation.recipeHash", SpecFieldShown::Reported),
    ("preparation.sourceColumns", SpecFieldShown::Reported),
    ("preparation.capabilityVersion", SpecFieldShown::Reported),
    ("preparation.engineVersion", SpecFieldShown::Reported),
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

/// One score's line, by position and never by name (decision 435). `None` for a
/// score whose statistic or rule is open, which no build could have made.
fn score_parameter(step: &ScaleScoreStep) -> Option<ExportParameter> {
    let positions = |v: &[stats_desc::Alias]| v.iter().map(|a| a.0 .0).collect();
    Some(ExportParameter::PreparedColumn {
        position: step.alias.0 .0,
        statistic: match step.statistic? {
            Statistic::Sum => ScoreStatistic::Sum,
            Statistic::Mean => ScoreStatistic::Mean,
        },
        items: positions(&step.items),
        reversed: positions(&step.reversed),
        scale_ends: step.scale.map(|s| [s.min.to_bits(), s.max.to_bits()]),
        rule: match step.rule? {
            MissingRule::EveryItem => ScoreRule::EveryItem,
            MissingRule::AtLeast(n) => ScoreRule::AtLeast(n),
        },
        missing_codes: step.missing_codes.len() as u64,
    })
}

/// A run's settings on prepared data (decision 435): the input's shape told
/// apart into the file's columns and the scores, the versions that prepared it,
/// and how each score was made, in place of the plain shape's line.
fn with_preparation(
    parameters: Vec<ExportParameter>,
    preparation: &Preparation,
    recipe: &Recipe,
) -> Result<Vec<ExportParameter>, ProjectRefusal> {
    let mut out = Vec::with_capacity(parameters.len() + recipe.steps.len() + 1);
    for p in parameters {
        let ExportParameter::InputShape { rows, columns } = p else {
            out.push(p);
            continue;
        };
        let source_columns = u64::from(preparation.source_columns);
        out.push(ExportParameter::PreparedShape {
            rows,
            source_columns,
            prepared_columns: columns.saturating_sub(source_columns),
        });
        out.push(ExportParameter::PreparedBy {
            capability_version: preparation.capability_version,
            engine_version: preparation.engine_version,
        });
        for step in &recipe.steps {
            let RecipeStep::ScaleScore(step) = step;
            out.push(score_parameter(step).ok_or(ProjectCode::DbUnreadable)?);
        }
    }
    Ok(out)
}

/// What a saved spec and its recipe edge say of a run's preparation, read back
/// and verified (decision 435): both or neither, the recipe the one the spec
/// names. Anything else is no run this build saved, so it is `db_unreadable`, as
/// a spec that will not parse is.
fn preparation_of(
    h: &mut ProjectHandle,
    spec: &Value,
    arts: &RunArtifacts,
) -> Result<Option<(Preparation, Recipe)>, ProjectRefusal> {
    let preparation = match spec.get("preparation") {
        None | Some(Value::Null) => None,
        Some(p) => Some(parse::<Preparation>(p.clone())?),
    };
    match (preparation, &arts.recipe) {
        (None, None) => Ok(None),
        (Some(preparation), Some(a)) if a.hash.as_hex() == preparation.recipe_hash.as_hex() => {
            let bytes = h.read_artifact(a.hash.as_hex())?;
            let recipe = stats_adapters::prep_recipe_from_bytes(&bytes)
                .map_err(|_| ProjectCode::DbUnreadable)?;
            Ok(Some((preparation, recipe)))
        }
        _ => Err(ProjectCode::DbUnreadable.into()),
    }
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

/// A one-sample spec's settings, by the allow-list. A spec with no test value
/// gives no test-value line: such a run ends `NeedsUserInput`, which a Save
/// refuses (decision 264).
fn one_sample_parameters(spec: &OperationSpec<OneSampleParams>) -> Vec<ExportParameter> {
    let p = &spec.parameters;
    let mut out = shape(&spec.inputs);
    out.push(ExportParameter::Column {
        position: p.outcome.0 .0,
        role: ColumnRole::Outcome,
    });
    if let Some(v) = p.test_value {
        out.push(ExportParameter::TestValue { bits: v.to_bits() });
    }
    out.push(ExportParameter::MissingCodes {
        count: p.missing_codes.len() as u64,
    });
    out.push(ExportParameter::Seed(spec.random_seed));
    out
}

/// A paired spec's settings, by the allow-list.
fn paired_parameters(spec: &OperationSpec<PairedParams>) -> Vec<ExportParameter> {
    let p = &spec.parameters;
    let mut out = shape(&spec.inputs);
    out.push(ExportParameter::Column {
        position: p.first.0 .0,
        role: ColumnRole::First,
    });
    out.push(ExportParameter::Column {
        position: p.second.0 .0,
        role: ColumnRole::Second,
    });
    out.push(ExportParameter::MissingCodes {
        count: p.missing_codes.len() as u64,
    });
    out.push(ExportParameter::Seed(spec.random_seed));
    out
}

/// A one-way ANOVA spec's settings, by the allow-list (decision 330): its
/// two columns, the included levels as indexes or every level, and the post
/// hoc tests as chosen.
fn anova_parameters(spec: &OperationSpec<AnovaParams>) -> Vec<ExportParameter> {
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
    out.push(ExportParameter::IncludedLevels(p.levels.clone()));
    out.push(ExportParameter::PostHocTests(
        p.post_hoc
            .iter()
            .map(|t| match t {
                PostHoc::Tukey => PostHocTest::Tukey,
                PostHoc::GamesHowell => PostHocTest::GamesHowell,
                PostHoc::Bonferroni => PostHocTest::Bonferroni,
            })
            .collect(),
    ));
    out.push(ExportParameter::MissingCodes {
        count: p.missing_codes.len() as u64,
    });
    out.push(ExportParameter::Seed(spec.random_seed));
    out
}

/// A correlations spec's settings, by the allow-list (decision 357): its
/// columns by position, the coefficients and the missing handling as chosen,
/// and how many missing codes.
fn corr_parameters(spec: &OperationSpec<CorrParams>) -> Vec<ExportParameter> {
    let p = &spec.parameters;
    let mut out = shape(&spec.inputs);
    out.push(ExportParameter::Variables(
        p.variables.iter().map(|v| v.0 .0).collect(),
    ));
    out.push(ExportParameter::Coefficients(
        p.coefficients
            .iter()
            .map(|c| match c {
                Coefficient::Pearson => CorrelationCoefficient::Pearson,
                Coefficient::Spearman => CorrelationCoefficient::Spearman,
                Coefficient::Kendall => CorrelationCoefficient::Kendall,
            })
            .collect(),
    ));
    out.push(ExportParameter::MissingRows(match p.missing {
        MissingHandling::Pairwise => MissingRows::Pairwise,
        MissingHandling::Listwise => MissingRows::Listwise,
    }));
    out.push(ExportParameter::MissingCodes {
        count: p.missing_codes.len() as u64,
    });
    out.push(ExportParameter::Seed(spec.random_seed));
    out
}

/// A Cronbach's alpha spec's settings, by the allow-list (decision 382): its
/// items and reverse-scored items by position, the scale's ends the user
/// typed, as bits, and how many missing codes.
fn alpha_parameters(spec: &OperationSpec<AlphaParams>) -> Vec<ExportParameter> {
    let p = &spec.parameters;
    let positions = |v: &[stats_desc::Alias]| v.iter().map(|a| a.0 .0).collect();
    let mut out = shape(&spec.inputs);
    out.push(ExportParameter::Items(positions(&p.items)));
    out.push(ExportParameter::ReverseScored(positions(&p.reversed)));
    out.push(ExportParameter::ScaleEnds(
        p.scale.map(|s| [s.min.to_bits(), s.max.to_bits()]),
    ));
    out.push(ExportParameter::MissingCodes {
        count: p.missing_codes.len() as u64,
    });
    out.push(ExportParameter::Seed(spec.random_seed));
    out
}

/// A crosstab spec's settings, by the allow-list (Research OS, Phase 8X-2,
/// decision 590): its rows' and columns' columns by position, and how many
/// missing codes. No level is named: the table takes every level.
fn xtab_parameters(spec: &OperationSpec<CrosstabsParams>) -> Vec<ExportParameter> {
    let p = &spec.parameters;
    let mut out = shape(&spec.inputs);
    out.push(ExportParameter::Column {
        position: p.rows.0 .0,
        role: ColumnRole::TableRows,
    });
    out.push(ExportParameter::Column {
        position: p.columns.0 .0,
        role: ColumnRole::TableColumns,
    });
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
            ArtifactKind::DataFile | ArtifactKind::OperationSpec | ArtifactKind::Recipe => {
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
    let loaded = load(h)?;
    let mut runs = Vec::new();
    for stored in &loaded.runs {
        let (_, arts) = artifacts_of(&loaded, stored.run_id)?;
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
        // Looked up after the spec's read, which checks its hash and can mark
        // damage: a damaged spec says so, whatever its capability.
        let method = method_for(stored.capability_id.as_str())
            .ok_or_else(|| ProjectRefusal::from(ProjectCode::DbUnreadable))?;
        let preparation = preparation_of(h, &spec, &arts)?;
        (method.export)(h, spec, &arts.generated, &mut run)?;
        if let Some((preparation, recipe)) = &preparation {
            run.parameters = with_preparation(run.parameters, preparation, recipe)?;
        }
        runs.push(run);
    }
    let edges = loaded.graph.edges().to_vec();
    Ok(ProjectExportInput {
        title: record.title,
        project: record.id,
        created_at: record.created_at,
        created_by: record.app_version,
        artifacts: loaded.artifacts,
        runs,
        edges,
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
pub async fn project_recipes(
    app: tauri::AppHandle,
    state: St<'_>,
    project_id: String,
) -> Result<Vec<RecipeView>, GaplyError> {
    let _engine = state
        .workspace
        .require_statistics()
        .map_err(refuse_workspace)?;
    let root = root_of(&app)?;
    let ws = state.workspace.clone();
    blocking(move || recipes(&root, &ws, &project_id)).await
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
pub async fn project_rerun(
    app: tauri::AppHandle,
    state: St<'_>,
    run_id: u64,
) -> Result<RerunStartedView, GaplyError> {
    let _engine = state
        .workspace
        .require_statistics()
        .map_err(refuse_workspace)?;
    let root = root_of(&app)?;
    let ws = state.workspace.clone();
    blocking(move || rerun(&root, &ws, run_id)).await
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

    /// A spec of each kind with every optional field filled but `preparation`,
    /// whose rows 8M-4 adds (decision 417).
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
            preparation: None,
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
            preparation: None,
        };
        (describe, ttest)
    }

    /// A one-sample spec with its test value and missing codes filled, and a
    /// paired spec (Research OS, Phase 8D-2a, decision 264).
    fn onesample_specs() -> (OperationSpec<OneSampleParams>, OperationSpec<PairedParams>) {
        let confirmation = ConfirmationId::parse(&"c".repeat(64)).unwrap();
        let one = OperationSpec {
            schema_version: SchemaVersion,
            operation_id: OperationId(5),
            workspace_id: WorkspaceId::Statistics,
            capability_id: stats_onesample::one_sample_capability_id(),
            capability_version: 1,
            inputs: vec![input()],
            parameters: OneSampleParams {
                outcome: Alias(ColumnAlias(2)),
                test_value: Some(0.1),
                missing_codes: vec![-99.0],
            },
            random_seed: Some(11),
            confirmation_refs: vec![confirmation.clone()],
            preparation: None,
        };
        let paired = OperationSpec {
            schema_version: SchemaVersion,
            operation_id: OperationId(6),
            workspace_id: WorkspaceId::Statistics,
            capability_id: stats_onesample::paired_capability_id(),
            capability_version: 1,
            inputs: vec![input()],
            parameters: PairedParams {
                first: Alias(ColumnAlias(1)),
                second: Alias(ColumnAlias(2)),
                missing_codes: vec![-1.0],
            },
            random_seed: Some(13),
            confirmation_refs: vec![confirmation],
            preparation: None,
        };
        (one, paired)
    }

    /// A one-way ANOVA spec with a level left out, all three tests and a
    /// missing code (Research OS, Phase 8G-2, decision 330).
    fn anova_spec() -> OperationSpec<AnovaParams> {
        OperationSpec {
            schema_version: SchemaVersion,
            operation_id: OperationId(7),
            workspace_id: WorkspaceId::Statistics,
            capability_id: stats_anova::capability_id(),
            capability_version: 1,
            inputs: vec![input()],
            parameters: AnovaParams {
                outcome: Alias(ColumnAlias(2)),
                group: Alias(ColumnAlias(3)),
                levels: Some(vec![0, 2, 3]),
                post_hoc: vec![PostHoc::Tukey, PostHoc::GamesHowell, PostHoc::Bonferroni],
                missing_codes: vec![-9.0],
            },
            random_seed: None,
            confirmation_refs: vec![ConfirmationId::parse(&"c".repeat(64)).unwrap()],
            preparation: None,
        }
    }

    #[test]
    fn the_anova_settings_are_positions_roles_indexes_words_and_counts() {
        let a = anova_spec();
        assert_eq!(
            anova_parameters(&a),
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
                ExportParameter::IncludedLevels(Some(vec![0, 2, 3])),
                ExportParameter::PostHocTests(vec![
                    PostHocTest::Tukey,
                    PostHocTest::GamesHowell,
                    PostHocTest::Bonferroni
                ]),
                ExportParameter::MissingCodes { count: 1 },
                ExportParameter::Seed(None),
            ]
        );
        let mut none = a;
        none.parameters.levels = None;
        none.parameters.post_hoc = vec![];
        let p = anova_parameters(&none);
        assert_eq!(p[3], ExportParameter::IncludedLevels(None));
        assert_eq!(p[4], ExportParameter::PostHocTests(vec![]));
    }

    /// A correlations spec, pairwise, all three coefficients and a missing
    /// code (Research OS, Phase 8H-2, decision 357).
    fn corr_spec() -> OperationSpec<CorrParams> {
        OperationSpec {
            schema_version: SchemaVersion,
            operation_id: OperationId(8),
            workspace_id: WorkspaceId::Statistics,
            capability_id: stats_corr::capability_id(),
            capability_version: 1,
            inputs: vec![input()],
            parameters: CorrParams {
                variables: vec![Alias(ColumnAlias(1)), Alias(ColumnAlias(3))],
                coefficients: vec![
                    Coefficient::Pearson,
                    Coefficient::Spearman,
                    Coefficient::Kendall,
                ],
                missing: MissingHandling::Pairwise,
                missing_codes: vec![-9.0, 99.0],
            },
            random_seed: None,
            confirmation_refs: vec![ConfirmationId::parse(&"d".repeat(64)).unwrap()],
            preparation: None,
        }
    }

    /// Two Cronbach's alpha specs (Research OS, Phase 8I-2, decision 382):
    /// one with two items reversed, the scale's ends and a missing code; one
    /// with nothing reversed and so no scale.
    fn alpha_specs() -> (OperationSpec<AlphaParams>, OperationSpec<AlphaParams>) {
        let a = |n: u32| Alias(ColumnAlias(n));
        let spec = |parameters| OperationSpec {
            schema_version: SchemaVersion,
            operation_id: OperationId(10),
            workspace_id: WorkspaceId::Statistics,
            capability_id: stats_alpha::capability_id(),
            capability_version: 1,
            inputs: vec![input()],
            parameters,
            random_seed: None,
            confirmation_refs: Vec::new(),
            preparation: None,
        };
        (
            spec(AlphaParams {
                items: vec![a(1), a(2), a(3)],
                reversed: vec![a(1), a(3)],
                scale: Some(stats_alpha::ScaleEnds { min: 1.0, max: 5.0 }),
                missing_codes: vec![99.0],
            }),
            spec(AlphaParams {
                items: vec![a(2), a(3)],
                reversed: Vec::new(),
                scale: None,
                missing_codes: Vec::new(),
            }),
        )
    }

    /// Research OS, Phase 8X-2 (decision 590): a crosstab's spec, its rows' and
    /// columns' columns by alias, two codes.
    fn xtab_spec() -> OperationSpec<CrosstabsParams> {
        OperationSpec {
            schema_version: SchemaVersion,
            operation_id: OperationId(11),
            workspace_id: WorkspaceId::Statistics,
            capability_id: stats_xtab::capability_id(),
            capability_version: 1,
            inputs: vec![input()],
            parameters: CrosstabsParams {
                rows: Alias(ColumnAlias(2)),
                columns: Alias(ColumnAlias(3)),
                missing_codes: vec![-99.0, 99.0],
            },
            random_seed: None,
            confirmation_refs: Vec::new(),
            preparation: None,
        }
    }

    #[test]
    fn the_crosstabs_settings_are_positions_roles_and_counts() {
        assert_eq!(
            xtab_parameters(&xtab_spec()),
            [
                ExportParameter::InputShape {
                    rows: 12,
                    columns: 3
                },
                ExportParameter::Column {
                    position: 2,
                    role: ColumnRole::TableRows
                },
                ExportParameter::Column {
                    position: 3,
                    role: ColumnRole::TableColumns
                },
                ExportParameter::MissingCodes { count: 2 },
                ExportParameter::Seed(None),
            ]
        );
    }

    #[test]
    fn the_alpha_settings_are_positions_bits_and_counts() {
        let (reversed, plain) = alpha_specs();
        assert_eq!(
            alpha_parameters(&reversed),
            [
                ExportParameter::InputShape {
                    rows: 12,
                    columns: 3
                },
                ExportParameter::Items(vec![1, 2, 3]),
                ExportParameter::ReverseScored(vec![1, 3]),
                ExportParameter::ScaleEnds(Some([1f64.to_bits(), 5f64.to_bits()])),
                ExportParameter::MissingCodes { count: 1 },
                ExportParameter::Seed(None),
            ]
        );
        assert_eq!(
            alpha_parameters(&plain)[1..4],
            [
                ExportParameter::Items(vec![2, 3]),
                ExportParameter::ReverseScored(Vec::new()),
                ExportParameter::ScaleEnds(None),
            ]
        );
    }

    #[test]
    fn the_corr_settings_are_positions_words_and_counts() {
        let c = corr_spec();
        assert_eq!(
            corr_parameters(&c),
            [
                ExportParameter::InputShape {
                    rows: 12,
                    columns: 3
                },
                ExportParameter::Variables(vec![1, 3]),
                ExportParameter::Coefficients(vec![
                    CorrelationCoefficient::Pearson,
                    CorrelationCoefficient::Spearman,
                    CorrelationCoefficient::Kendall
                ]),
                ExportParameter::MissingRows(MissingRows::Pairwise),
                ExportParameter::MissingCodes { count: 2 },
                ExportParameter::Seed(None),
            ]
        );
        let mut one = c;
        one.parameters.coefficients = vec![Coefficient::Kendall];
        one.parameters.missing = MissingHandling::Listwise;
        let p = corr_parameters(&one);
        assert_eq!(
            p[2],
            ExportParameter::Coefficients(vec![CorrelationCoefficient::Kendall])
        );
        assert_eq!(p[3], ExportParameter::MissingRows(MissingRows::Listwise));
    }

    /// A summary's spec on prepared data (decision 435), kept apart from
    /// [`specs`], whose parameters a test pins.
    fn prepared_spec() -> OperationSpec<DescribeParams> {
        let (mut d, _) = specs();
        d.preparation = Some(Preparation {
            source_hash: ContentHash::of_bytes(b"source"),
            recipe_hash: ContentHash::of_bytes(b"recipe"),
            source_columns: 3,
            capability_version: 1,
            engine_version: 1,
        });
        d.inputs[0].columns = 5;
        d
    }

    #[test]
    fn a_prepared_runs_settings_tell_the_files_columns_from_the_scores() {
        use gaply_core::stats_prep;
        let spec = prepared_spec();
        let a = |n: u32| stats_desc::Alias(ColumnAlias(n));
        let step = |position, statistic, rule| {
            RecipeStep::ScaleScore(ScaleScoreStep {
                alias: a(position),
                name: "NAME_SENTINEL_a1".into(),
                items: vec![a(1), a(3)],
                reversed: vec![a(3)],
                scale: Some(stats_alpha::ScaleEnds { min: 1.0, max: 5.0 }),
                statistic,
                rule,
                missing_codes: vec![9.0, 99.0],
            })
        };
        let mut recipe = Recipe {
            schema_version: stats_prep::RECIPE_SCHEMA_VERSION,
            capability_id: gaply_core::contracts::CapabilityId::parse(stats_prep::CAPABILITY_ID)
                .unwrap(),
            capability_version: 1,
            engine_version: 1,
            source_columns: 3,
            steps: vec![
                step(4, Some(Statistic::Mean), Some(MissingRule::AtLeast(1))),
                step(5, Some(Statistic::Sum), Some(MissingRule::EveryItem)),
            ],
        };
        let got = with_preparation(
            describe_parameters(&spec),
            spec.preparation.as_ref().unwrap(),
            &recipe,
        )
        .unwrap();
        let score = |position, statistic, rule| ExportParameter::PreparedColumn {
            position,
            statistic,
            items: vec![1, 3],
            reversed: vec![3],
            scale_ends: Some([1f64.to_bits(), 5f64.to_bits()]),
            rule,
            missing_codes: 2,
        };
        assert_eq!(
            got[..4],
            [
                ExportParameter::PreparedShape {
                    rows: 12,
                    source_columns: 3,
                    prepared_columns: 2
                },
                ExportParameter::PreparedBy {
                    capability_version: 1,
                    engine_version: 1
                },
                score(4, ScoreStatistic::Mean, ScoreRule::AtLeast(1)),
                score(5, ScoreStatistic::Sum, ScoreRule::EveryItem),
            ]
        );
        assert_eq!(
            got[4..],
            describe_parameters(&spec)[1..],
            "every other line as it was"
        );
        assert!(!format!("{got:?}").contains("NAME_SENTINEL"));

        // A step left open cannot be described, and no build could have made it.
        recipe.steps[1] = step(5, None, Some(MissingRule::EveryItem));
        assert_eq!(
            with_preparation(
                describe_parameters(&spec),
                spec.preparation.as_ref().unwrap(),
                &recipe
            )
            .unwrap_err()
            .code,
            ProjectCode::DbUnreadable
        );
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
        let (one, paired) = onesample_specs();
        paths(&serde_json::to_value(&one).unwrap(), "", &mut seen);
        paths(&serde_json::to_value(&paired).unwrap(), "", &mut seen);
        paths(&serde_json::to_value(anova_spec()).unwrap(), "", &mut seen);
        paths(&serde_json::to_value(corr_spec()).unwrap(), "", &mut seen);
        // Alpha with the scale and without it (decision 382).
        let (reversed, plain) = alpha_specs();
        paths(&serde_json::to_value(&reversed).unwrap(), "", &mut seen);
        paths(&serde_json::to_value(&plain).unwrap(), "", &mut seen);
        // Crosstabs (decision 590): its `rows` and `columns`, a string each,
        // distinct from the summary's `columns[].alias`.
        paths(&serde_json::to_value(xtab_spec()).unwrap(), "", &mut seen);
        // A run on prepared data (decision 435).
        paths(
            &serde_json::to_value(prepared_spec()).unwrap(),
            "",
            &mut seen,
        );
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
                recipe: None,
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

    /* ---- a Save of a run on prepared data (decision 433) --------------------- */

    const PREPARED_CSV: &[u8] = b"q1,q2\n1,2\n3,4\n";

    fn two_item_recipe() -> gaply_core::stats_prep::Recipe {
        use gaply_core::stats_prep;
        let a = |n: u32| Alias(ColumnAlias(n));
        Recipe {
            schema_version: stats_prep::RECIPE_SCHEMA_VERSION,
            capability_id: gaply_core::contracts::CapabilityId::parse(stats_prep::CAPABILITY_ID)
                .unwrap(),
            capability_version: stats_prep::CAPABILITY_VERSION,
            engine_version: stats_prep::ENGINE_VERSION,
            source_columns: 2,
            steps: vec![RecipeStep::ScaleScore(ScaleScoreStep {
                alias: a(3),
                name: "a score".into(),
                items: vec![a(1), a(2)],
                reversed: Vec::new(),
                scale: None,
                statistic: Some(stats_prep::Statistic::Sum),
                rule: Some(MissingRule::EveryItem),
                missing_codes: Vec::new(),
            })],
        }
    }

    /// A slot holding [`PREPARED_CSV`]'s prepared table, with or without its
    /// source's bytes.
    fn a_prepared_slot(with_source: bool) -> Arc<Slot> {
        let slots = crate::runs::DatasetSlots::default();
        let dataset = load_dataset_from_bytes(PREPARED_CSV, DataFormat::Csv).unwrap();
        let source = with_source.then(|| {
            SourceFile::new(
                PREPARED_CSV.to_vec(),
                ContentHash::of_bytes(PREPARED_CSV),
                DataFormat::Csv,
            )
        });
        let n = slots.insert_with_source(String::new(), dataset, source);
        let held = slots.get(n).unwrap();
        let prepared = stats_adapters::prep_build(
            &two_item_recipe(),
            crate::workspace::build(),
            held.dataset(),
            1_000,
        )
        .unwrap();
        slots.hold_prepared(&held, prepared).unwrap()
    }

    /// The hashes a run's copy names. [`Forged::honest`] is what a live run on
    /// [`a_prepared_slot`] records; each test forges one of them.
    struct Forged {
        input: ContentHash,
        spec_input: ContentHash,
        source: ContentHash,
        recipe: ContentHash,
        manifest: Vec<ContentHash>,
        state: RunState,
        /// The recipe's bytes the record holds (decision 491).
        held_recipe: Option<Vec<u8>>,
    }

    impl Forged {
        fn honest() -> Forged {
            let source = ContentHash::of_bytes(PREPARED_CSV);
            let recipe =
                ContentHash::of_bytes(&stats_adapters::prep_recipe_bytes(&two_item_recipe()));
            let prepared = ContentHash::from_hex(&stats_adapters::prep_input_hash(
                source.as_hex(),
                recipe.as_hex(),
            ))
            .unwrap();
            Forged {
                input: source.clone(),
                spec_input: prepared.clone(),
                source,
                recipe,
                manifest: vec![prepared],
                state: RunState::Succeeded,
                held_recipe: Some(stats_adapters::prep_recipe_bytes(&two_item_recipe())),
            }
        }

        fn copy(&self) -> RunCopy {
            let spec = serde_json::json!({
                "inputs": [{ "contentHash": self.spec_input.as_hex() }],
                "preparation": {
                    "sourceHash": self.source.as_hex(),
                    "recipeHash": self.recipe.as_hex(),
                    "sourceColumns": 2,
                    "capabilityVersion": 1,
                    "engineVersion": 1,
                },
            });
            let mut copy = RunCopy {
                state: self.state,
                spec_json: Some(serde_json::to_vec(&spec).unwrap()),
                manifest_inputs: self.manifest.clone(),
                ..failed_copy(&self.input)
            };
            copy.input.as_mut().unwrap().recipe = self.held_recipe.clone();
            copy
        }

        fn checked(&self, slot: &Slot) -> Result<(), ProjectCode> {
            let copy = self.copy();
            data_of(&copy, copy.input.as_ref().unwrap(), slot).map(|_| ())
        }
    }

    fn another_hash() -> ContentHash {
        ContentHash::of_bytes(b"another")
    }

    /* ---- a rerun's refusals of a recipe (decision 434) ------------------------- */

    #[test]
    fn every_recipe_refusal_has_its_rerun_code_and_only_the_cells_cap_is_over_limits() {
        assert!(PrepCode::ALL.len() >= 45, "{}", PrepCode::ALL.len());
        for code in PrepCode::ALL.iter().copied() {
            let want = if code == PrepCode::CellsOverCap {
                ProjectCode::RerunOverLimits
            } else {
                ProjectCode::RecipeUnusable
            };
            assert_eq!(rerun_recipe_code(code), want, "{}", code.wire());
        }
    }

    #[test]
    fn a_report_over_the_cells_cap_alone_is_over_limits_and_with_anything_else_is_not() {
        use gaply_core::contracts::{Diagnostic, ResourceEstimate, ValidationReport};
        let report = |errors: &[PrepCode], open: &[PrepCode]| {
            let list = |codes: &[PrepCode]| codes.iter().map(|c| Diagnostic::error(*c)).collect();
            ValidationReport::new(
                Vec::new(),
                list(errors),
                list(open),
                Vec::new(),
                ResourceEstimate {
                    rows: 1,
                    columns: 1,
                    cells: 1,
                },
            )
        };
        assert_eq!(
            refused_recipe_code(&report(&[PrepCode::CellsOverCap], &[])),
            ProjectCode::RerunOverLimits
        );
        assert_eq!(
            refused_recipe_code(&report(&[PrepCode::CellsOverCap, PrepCode::TooWide], &[])),
            ProjectCode::RecipeUnusable
        );
        assert_eq!(
            refused_recipe_code(&report(&[PrepCode::TooWide, PrepCode::CellsOverCap], &[])),
            ProjectCode::RecipeUnusable
        );
        assert_eq!(
            refused_recipe_code(&report(&[], &[PrepCode::RuleNotChosen])),
            ProjectCode::RecipeUnusable
        );
        assert_eq!(
            refused_recipe_code(&report(&[], &[])),
            ProjectCode::RecipeUnusable,
            "a report with no code is no reason to call it this computer's limit"
        );
    }

    #[test]
    fn what_a_live_run_on_a_prepared_slot_records_passes_each_comparison() {
        let slot = a_prepared_slot(true);
        assert_eq!(Forged::honest().checked(&slot), Ok(()));
        let copy = Forged::honest().copy();
        let (bytes, format, hash) = data_of(&copy, copy.input.as_ref().unwrap(), &slot).unwrap();
        assert_eq!(bytes, PREPARED_CSV, "the file, not the prepared table");
        assert_eq!(format, DataFormat::Csv);
        assert_eq!(hash.as_hex(), ContentHash::of_bytes(PREPARED_CSV).as_hex());
    }

    /// Since Phase 8T-1a (decision 491), against the record's recipe, not the
    /// slot's.
    #[test]
    fn a_preparation_naming_another_recipe_than_the_records_is_a_mismatch() {
        let slot = a_prepared_slot(true);
        let mut forged = Forged::honest();
        forged.recipe = another_hash();
        // The prepared input hash agrees with the forged recipe, so only the
        // record's recipe can catch it.
        forged.spec_input = ContentHash::from_hex(&stats_adapters::prep_input_hash(
            forged.source.as_hex(),
            forged.recipe.as_hex(),
        ))
        .unwrap();
        forged.manifest = vec![forged.spec_input.clone()];
        assert_eq!(forged.checked(&slot), Err(ProjectCode::DataMismatch));
    }

    /// Phase 8T-1a (decision 491): a spec naming a preparation on a record that
    /// holds no recipe is a mismatch, though the slot holds the very recipe.
    #[test]
    fn a_spec_naming_a_preparation_on_a_record_holding_no_recipe_is_a_mismatch() {
        let slot = a_prepared_slot(true);
        let mut forged = Forged::honest();
        forged.held_recipe = None;
        assert_eq!(forged.checked(&slot), Err(ProjectCode::DataMismatch));
        let copy = forged.copy();
        assert_eq!(
            recipe_of(&copy, copy.input.as_ref().unwrap()).map(|_| ()),
            Err(ProjectCode::DataMismatch)
        );
    }

    #[test]
    fn a_runs_input_other_than_the_slots_source_is_a_mismatch() {
        let slot = a_prepared_slot(true);
        let mut forged = Forged::honest();
        forged.input = another_hash();
        assert_eq!(forged.checked(&slot), Err(ProjectCode::DataMismatch));
    }

    #[test]
    fn a_preparation_naming_another_source_than_the_slots_is_a_mismatch() {
        let slot = a_prepared_slot(true);
        let mut forged = Forged::honest();
        forged.source = another_hash();
        assert_eq!(forged.checked(&slot), Err(ProjectCode::DataMismatch));
    }

    #[test]
    fn a_spec_input_other_than_the_prepared_input_is_a_mismatch() {
        let slot = a_prepared_slot(true);
        let mut forged = Forged::honest();
        forged.spec_input = another_hash();
        assert_eq!(forged.checked(&slot), Err(ProjectCode::DataMismatch));
        forged.spec_input = forged.source.clone();
        assert_eq!(
            forged.checked(&slot),
            Err(ProjectCode::DataMismatch),
            "the source's own hash is not the prepared input"
        );
    }

    #[test]
    fn a_manifest_input_other_than_the_prepared_input_is_a_mismatch_for_a_success_only() {
        let slot = a_prepared_slot(true);
        for wrong in [vec![another_hash()], Vec::new()] {
            let mut forged = Forged::honest();
            forged.manifest = wrong;
            assert_eq!(forged.checked(&slot), Err(ProjectCode::DataMismatch));
            forged.state = RunState::Failed;
            assert_eq!(forged.checked(&slot), Ok(()), "a failure has no manifest");
        }
    }

    #[test]
    fn a_prepared_run_with_no_source_held_is_data_unavailable() {
        let slot = a_prepared_slot(false);
        assert_eq!(
            Forged::honest().checked(&slot),
            Err(ProjectCode::DataUnavailable)
        );
    }

    #[test]
    fn a_preparation_that_does_not_read_back_is_a_mismatch() {
        let slot = a_prepared_slot(true);
        let mut copy = Forged::honest().copy();
        let mut spec: Value = serde_json::from_slice(copy.spec_json.as_ref().unwrap()).unwrap();
        spec["preparation"]["sourceColumns"] = Value::from("two");
        copy.spec_json = Some(serde_json::to_vec(&spec).unwrap());
        assert_eq!(
            data_of(&copy, copy.input.as_ref().unwrap(), &slot).map(|_| ()),
            Err(ProjectCode::DataMismatch)
        );
    }

    #[test]
    fn the_recipe_is_expected_to_hash_to_what_the_spec_recorded() {
        let recorded = ArtifactHash::parse(another_hash().as_hex()).unwrap();
        let copy = failed_copy(&ContentHash::of_bytes(b"data"));
        let data = (
            b"data".to_vec(),
            DataFormat::Csv,
            ArtifactHash::parse(ContentHash::of_bytes(b"data").as_hex()).unwrap(),
        );
        let req = with_recipe(
            request_for(&copy, data, 1_000).unwrap(),
            Some((b"other bytes".to_vec(), recorded.clone())),
        );
        let recipe = req
            .artifacts
            .iter()
            .find(|a| a.kind == ArtifactKind::Recipe)
            .expect("a recipe artifact");
        assert_eq!(
            recipe.expected, recorded,
            "the recorded hash, not the bytes' own"
        );
        assert_ne!(
            sha256(&recipe.bytes),
            recorded,
            "so the store refuses these"
        );
        assert_eq!(recipe.format, DataFormat::Json);
        assert!(
            req.run.used.contains(&(recorded, UsedRole::Recipe)),
            "and `used` names it"
        );
        assert!(
            req.run.generated.iter().all(|h| h != &recipe.expected),
            "a recipe is used, never generated"
        );
    }

    #[test]
    fn equal_bytes_compare_in_full() {
        assert!(same_bytes("abc", "abc"));
        assert!(!same_bytes("abc", "abd"));
        assert!(!same_bytes("abc", "ab"));
        assert!(!same_bytes("", "a"));
    }

    #[test]
    fn activate_never_closes_a_project_under_a_going_rerun() {
        // F4 (decision 225): workspace_activate closes an open project only in
        // the Activating phase, before the switch is Active. This holds the
        // open-project lock while an activation runs on another thread.
        //
        // OLD code: the activation reaches Active (its trailing close_on_leave
        // blocks on the held lock), so this opens the project and starts a
        // rerun before dropping the lock, and the activation then closes the
        // project under the rerun — recorded not saved. FIXED code: the
        // activation blocks in the Activating phase, so this releases the
        // lock, lets it finish, then opens and reruns — the rerun saves. The
        // rerun's worker is parked at a gate until this opens it, so the rerun
        // cannot end before the race is set up.
        //
        // activate is reached by UFCS (`WorkspaceSession::activate(...)`), not
        // `.activate(`, so `only_workspace_activate_activates_a_workspace`
        // still counts one caller (decision 201's precedent, for the store's
        // panic test, routes around that guard the same way).
        use crate::runs::{CheckpointHook, Point, RUN_CEILING_MS};
        use crate::workspace::{readiness, RunsFactory};
        use gaply_core::contracts::CapabilityId;
        use std::sync::Condvar;
        use std::thread;
        use std::time::Duration;

        struct Gate {
            open: Mutex<bool>,
            cv: Condvar,
        }
        impl Gate {
            fn hook(self: &Arc<Self>) -> CheckpointHook {
                let g = Arc::clone(self);
                let hook: CheckpointHook = Arc::new(move |_run, p| {
                    if p == Point::BeforeStart {
                        let mut open = g.open.lock().unwrap();
                        while !*open {
                            open = g.cv.wait(open).unwrap();
                        }
                    }
                });
                hook
            }
            fn release(&self) {
                *self.open.lock().unwrap() = true;
                self.cv.notify_all();
            }
        }
        #[derive(Default)]
        struct Tick(AtomicU64);
        impl Clock for Tick {
            fn now_ms(&self) -> u64 {
                self.0.fetch_add(1, Ordering::SeqCst)
            }
        }
        fn settled(projects: &ProjectSession) -> RerunView {
            let start = Instant::now();
            loop {
                if let Some(r) = projects.current_view().rerun {
                    if r.status != RerunProgress::Going {
                        return r;
                    }
                }
                assert!(
                    start.elapsed() < Duration::from_secs(10),
                    "the rerun never settled"
                );
                thread::sleep(Duration::from_millis(2));
            }
        }
        fn build_summary_project(root: &Path) -> String {
            let csv: &[u8] = b"score,group\n1.5,a\n2.5,a\n3.0,a\n3.5,b\n4.0,b\n5.5,b\n";
            let dataset = load_dataset_from_bytes(csv, DataFormat::Csv).expect("csv parses");
            let profile = stats_desc::profile(&dataset);
            let spec = stats_desc::spec(
                OperationId(1),
                &dataset,
                DescribeParams::defaults_for(&profile),
            );
            let spec_bytes = serde_json::to_vec(&spec).expect("spec serialises");
            let bundle = br#"{"bundle":1}"#.to_vec();
            let manifest = br#"{"manifest":1}"#.to_vec();
            let h = |b: &[u8]| ArtifactHash::parse(ContentHash::of_bytes(b).as_hex()).unwrap();
            let (dh, sh, bh, mh) = (h(csv), h(&spec_bytes), h(&bundle), h(&manifest));
            let req = SaveRequest {
                run: RunToSave {
                    capability_id: CapabilityId::parse(stats_desc::CAPABILITY_ID).unwrap(),
                    capability_version: 1,
                    state: RunState::Succeeded,
                    started_at: 1_700_000_000,
                    ended_at: 1_700_000_001,
                    used: vec![(dh.clone(), UsedRole::Data), (sh.clone(), UsedRole::Spec)],
                    generated: vec![bh.clone(), mh.clone()],
                    derived: vec![],
                },
                artifacts: vec![
                    ArtifactInput {
                        kind: ArtifactKind::DataFile,
                        format: DataFormat::Csv,
                        schema_version: 1,
                        expected: dh,
                        bytes: csv.to_vec(),
                    },
                    ArtifactInput {
                        kind: ArtifactKind::OperationSpec,
                        format: DataFormat::Json,
                        schema_version: 1,
                        expected: sh,
                        bytes: spec_bytes,
                    },
                    ArtifactInput {
                        kind: ArtifactKind::ResultBundle,
                        format: DataFormat::Json,
                        schema_version: 1,
                        expected: bh,
                        bytes: bundle,
                    },
                    ArtifactInput {
                        kind: ArtifactKind::RunManifest,
                        format: DataFormat::Json,
                        schema_version: 1,
                        expected: mh,
                        bytes: manifest,
                    },
                ],
            };
            let store = ProjectRoot::open_root_with(root, Caps::REAL).expect("store opens");
            let title = ProjectTitle::parse("f4").unwrap();
            // UFCS, not `store.create_with_save(`, so the lock-codes guard
            // (which scans this file's tests too) still counts only the three
            // real commands as lock-taking store callers.
            let (handle, _) =
                ProjectRoot::create_with_save(&store, &title, req).expect("project created");
            let id = handle.id().as_str().to_string();
            handle.close();
            id
        }

        let root = std::env::temp_dir().join(format!("gaply-f4-{}", std::process::id()));
        let id = build_summary_project(&root);

        for i in 0..50 {
            let gate = Arc::new(Gate {
                open: Mutex::new(false),
                cv: Condvar::new(),
            });
            let projects = Arc::new(ProjectSession::with(
                Caps::REAL,
                Arc::new(Monotonic(Instant::now())),
            ));
            // A fresh gated run manager per activation, as production builds
            // one per activation: the setup activate/deactivate seals its
            // manager (Runs::seal), and reusing one sealed manager would make
            // the race's rerun fail `NotStarted::Sealed`. The gate is shared,
            // so one release frees whichever manager's worker is parked.
            let factory: RunsFactory = {
                let gate = Arc::clone(&gate);
                Arc::new(move || {
                    Arc::new(Runs::with_hooks(
                        Arc::new(Tick::default()),
                        Some(gate.hook()),
                        RUN_CEILING_MS,
                    ))
                })
            };
            let ws = Arc::new(WorkspaceSession::with_projects(
                Arc::new(readiness),
                factory,
                Arc::clone(&projects),
            ));
            WorkspaceSession::activate(&ws, WorkspaceId::Statistics).expect("Statistics activates");
            ws.deactivate().expect("then left");

            let mut g = lock(&projects.open);
            let ws2 = Arc::clone(&ws);
            let activating =
                thread::spawn(move || WorkspaceSession::activate(&ws2, WorkspaceId::Statistics));
            // OLD code reaches Active in microseconds; FIXED code blocks in the
            // Activating phase on the held lock and never reaches Active here.
            let start = Instant::now();
            let active = loop {
                match ws.state() {
                    WorkspaceState::Active(_) => break true,
                    _ if start.elapsed() > Duration::from_millis(300) => break false,
                    _ => thread::sleep(Duration::from_millis(1)),
                }
            };
            if active {
                let engine = ws.require_statistics().expect("active");
                let store = ProjectRoot::open_root_with(&root, Caps::REAL).unwrap();
                // UFCS, not `store.open(`, for the lock-codes guard (above).
                let handle = ProjectRoot::open(&store, &id).unwrap();
                *g = Some(handle);
                // Releases the lock as rerun_open does; the blocked activation
                // then closes the project under the going rerun (OLD bug).
                rerun_open(&projects, g, &engine.runs, 1).expect("the rerun starts");
                activating.join().unwrap().ok();
            } else {
                drop(g);
                activating.join().unwrap().expect("activation finishes");
                open(&root, &ws, &id).expect("open the project");
                rerun(&root, &ws, 1).expect("start the rerun");
            }
            gate.release();
            let r = settled(&projects);
            assert_eq!(
                r.status,
                RerunProgress::Saved,
                "iteration {i}: the rerun must be saved (F4)"
            );
        }
    }

    /// One saved run's export input, field by field, in the order the
    /// exports read them: everything but the stored row (wall-clock times)
    /// and the manifest's `elapsed_ms` and `platform`. The t-test's output
    /// hash goes through the platform's maths library (its p), so it is
    /// pinned only where it was captured, macOS aarch64; elsewhere it must be
    /// the saved manifest's own, which the other fields already compare.
    fn pinned(run: &ExportRun) -> String {
        let m = run.manifest.as_ref();
        let ttest = run.run.capability_id.as_str() == stats_ttest::CAPABILITY_ID;
        let captured = cfg!(all(target_os = "macos", target_arch = "aarch64"));
        format!(
            "capability: {}\nparameters: {:?}\nexclusions: {:?}\nlimitations: {:?}\nnotes: {:?}\nfailure: {:?}\nengine_version: {:?}\ninputs: {:?}\nparameters_hash: {:?}\noutput_hash: {}\nrows_read: {:?}\nrows_used: {:?}\nreproducibility: {:?}",
            run.run.capability_id.as_str(),
            run.parameters,
            run.exclusions,
            run.limitations,
            run.notes,
            run.failure,
            m.map(|m| &m.engine_version),
            m.map(|m| m.inputs.iter().map(|h| h.as_hex().to_string()).collect::<Vec<_>>()),
            m.map(|m| m.parameters_hash.as_hex().to_string()),
            match m {
                Some(_) if ttest && !captured => "pinned on macOS aarch64 only".to_string(),
                m => format!("{:?}", m.map(|m| m.output_hash.as_hex().to_string())),
            },
            m.map(|m| m.rows_read),
            m.map(|m| m.rows_used),
            m.map(|m| &m.reproducibility),
        )
    }

    /// A run of `start` on `csv`'s dataset, ended, as a Save would save it.
    fn saved_request(
        runs: &Arc<Runs>,
        csv: &[u8],
        start: impl FnOnce(&Arc<Runs>, Arc<Slot>) -> gaply_core::contracts::RunId,
    ) -> SaveRequest {
        let dataset = load_dataset_from_bytes(csv, DataFormat::Csv).expect("the csv opens");
        let hash = stats_desc::input_ref(&dataset).content_hash;
        let slot = Arc::new(Slot::for_one_run(
            dataset,
            SourceFile::new(csv.to_vec(), hash, DataFormat::Csv),
        ));
        let run = start(runs, Arc::clone(&slot));
        let begun = Instant::now();
        while !runs.copy_for_save(run).unwrap().state.is_terminal() {
            assert!(begun.elapsed().as_secs() < 10, "the run never ended");
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        let copy = runs.copy_for_save(run).unwrap();
        let data = data_of(&copy, copy.input.as_ref().expect("an input"), &slot).unwrap();
        request_for(&copy, data, 1_700_000_000).unwrap()
    }

    /// The methods table (Research OS, Phase 8C, decision 249): every row
    /// names a capability the Statistics registry has, no capability has two
    /// rows, no two rows share a run kind, and the four methods a project
    /// runs again and exports today each have exactly one (Phase 8D-2a, the
    /// one-sample and paired t-tests).
    #[test]
    fn every_method_row_is_a_registered_capability_and_each_shipped_method_has_one() {
        let registered: Vec<String> = gaply_core::stats_registry::Registry::statistics()
            .entries()
            .iter()
            .map(|e| e.descriptor.id.as_str().to_string())
            .collect();
        for row in METHODS {
            assert!(
                registered.iter().any(|id| id == row.capability),
                "{}: a row for a capability the registry does not have",
                row.capability
            );
        }
        for id in [
            stats_desc::CAPABILITY_ID,
            stats_ttest::CAPABILITY_ID,
            stats_onesample::ONE_SAMPLE_ID,
            stats_onesample::PAIRED_ID,
            // Research OS, Phase 8X-2 (decision 590).
            stats_xtab::CAPABILITY_ID,
        ] {
            assert_eq!(
                METHODS.iter().filter(|m| m.capability == id).count(),
                1,
                "{id}: exactly one row"
            );
        }
        let kinds: BTreeSet<String> = METHODS.iter().map(|m| format!("{:?}", m.kind)).collect();
        assert_eq!(kinds.len(), METHODS.len(), "one run kind per row");
        let ids: BTreeSet<&str> = METHODS.iter().map(|m| m.capability).collect();
        assert_eq!(ids.len(), METHODS.len(), "one row per capability");
    }

    /// Difference 14 of the 8C step: `export_input`'s `ExportRun` for a saved
    /// summary, a saved t-test and a saved failed t-test, pinned in full
    /// before the seam, so the seam's export path is shown to give the same.
    #[test]
    fn export_input_gives_each_saved_runs_settings_codes_and_manifest_as_before() {
        const TWO: &[u8] = b"x,g\n1,a\n2,a\n4,a\n3,b\n5,b\n8,b\n";
        const CONSTANT: &[u8] = b"x,g\n2,a\n2,a\n2,a\n5,b\n5,b\n5,b\n";
        let welch = |slot: &Slot| {
            stats_ttest::spec(
                OperationId(2),
                slot.dataset(),
                TTestParams {
                    outcome: Alias(ColumnAlias(1)),
                    group: Alias(ColumnAlias(2)),
                    levels: Some([0, 1]),
                    variant: Some(Variant::Welch),
                    missing_codes: vec![],
                },
            )
        };
        let runs = Arc::new(Runs::default());
        let summary = saved_request(&runs, TWO, |runs, slot| {
            let profile = stats_desc::profile(slot.dataset());
            let spec = stats_desc::spec(
                OperationId(1),
                slot.dataset(),
                DescribeParams::defaults_for(&profile),
            );
            runs.start(slot, spec).unwrap()
        });
        let ttest = saved_request(&runs, TWO, |runs, slot| {
            let spec = welch(&slot);
            runs.start_ttest(slot, spec).unwrap()
        });
        let failed = saved_request(&runs, CONSTANT, |runs, slot| {
            let spec = welch(&slot);
            runs.start_ttest(slot, spec).unwrap()
        });
        assert_eq!(failed.run.state, RunState::Failed, "both groups constant");

        let root = std::env::temp_dir().join(format!("gaply-8c-export-{}", std::process::id()));
        let store = ProjectRoot::open_root_with(&root, Caps::REAL).expect("store opens");
        // UFCS, as above, for the lock-codes guard.
        let (mut h, _) =
            ProjectRoot::create_with_save(&store, &ProjectTitle::parse("8c").unwrap(), summary)
                .expect("project created");
        h.save(ttest).expect("the t-test saves");
        h.save(failed).expect("the failed t-test saves");
        let input = export_input(&mut h);
        // Deleted before anything can fail, so a red run leaves no project.
        let id = h.id().as_str().to_string();
        h.close();
        ProjectRoot::delete(&store, &id).expect("the project is deleted");
        let got: Vec<String> = input
            .expect("the export's input")
            .runs
            .iter()
            .map(pinned)
            .collect();

        // Captured from 141c779's code on macOS aarch64, 26 Oct 2026.
        let t_hash = if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
            r#"Some("7704c6aecd08c1f3fe605e70fde3d2b2a91ff5b695c3c28c31b93f7be00fb52e")"#
        } else {
            "pinned on macOS aarch64 only"
        };
        let want_summary = r#"capability: stats.describe
parameters: [InputShape { rows: 6, columns: 2 }, Column { position: 1, role: SummarisedNumeric }, Column { position: 2, role: SummarisedCategorical }, MissingCodes { count: 0 }, Seed(None)]
exclusions: []
limitations: []
notes: []
failure: None
engine_version: Some(1)
inputs: Some(["36599ff119a0bc8f67dd861445867568ffe0e747820fa7199dad9a8772aba104"])
parameters_hash: Some("e73cf11e60d7076c3e45ce5112acbfbcdfe91e84805eab6c51fff9e7af9e4dc2")
output_hash: Some("bcfdf7de1051e9afdb4c8185184ffc9889cc01e9fd07e29e0f9ffb251b0384aa")
rows_read: Some(6)
rows_used: Some(6)
reproducibility: Some(Bitwise)"#;
        let want_ttest = format!(
            r#"capability: stats.analyze.independent_t
parameters: [InputShape {{ rows: 6, columns: 2 }}, Column {{ position: 1, role: Outcome }}, Column {{ position: 2, role: Group }}, Levels(Some([0, 1])), Variant(Some(Welch)), MissingCodes {{ count: 0 }}, Seed(None)]
exclusions: []
limitations: [ExportCode {{ code: ClosedWord("stats.ttest.normality_not_assessed"), params: [] }}, ExportCode {{ code: ClosedWord("stats.ttest.cohens_d_has_no_interval"), params: [] }}]
notes: []
failure: None
engine_version: Some(1)
inputs: Some(["36599ff119a0bc8f67dd861445867568ffe0e747820fa7199dad9a8772aba104"])
parameters_hash: Some("6384d9380407ced1c6d6dc07495f4c82f1c2e0d6763ff108838087a3bc3f8cd3")
output_hash: {t_hash}
rows_read: Some(6)
rows_used: Some(6)
reproducibility: Some(Bitwise)"#
        );
        let want_failed = r#"capability: stats.analyze.independent_t
parameters: [InputShape { rows: 6, columns: 2 }, Column { position: 1, role: Outcome }, Column { position: 2, role: Group }, Levels(Some([0, 1])), Variant(Some(Welch)), MissingCodes { count: 0 }, Seed(None)]
exclusions: []
limitations: []
notes: []
failure: Some(ExportCode { code: ClosedWord("stats.ttest.both_groups_constant"), params: [] })
engine_version: None
inputs: None
parameters_hash: None
output_hash: None
rows_read: None
rows_used: None
reproducibility: None"#;
        let want: [&str; 3] = [want_summary, &want_ttest, want_failed];
        assert_eq!(got.len(), 3);
        let wrong: Vec<String> = got
            .iter()
            .zip(want)
            .filter(|(g, w)| g.as_str() != *w)
            .map(|(g, _)| g.clone())
            .collect();
        assert!(
            wrong.is_empty(),
            "\n--- got ---\n{}\n",
            wrong.join("\n--- next ---\n")
        );
    }

    /// [`pinned`] for the one-sample and paired t-tests (Research OS, Phase
    /// 8D-2a): the same fields, the output hash pinned only on macOS aarch64,
    /// where it was captured, because each result's p goes through the
    /// platform's maths library.
    fn pinned_onesample(run: &ExportRun) -> String {
        let m = run.manifest.as_ref();
        let captured = cfg!(all(target_os = "macos", target_arch = "aarch64"));
        format!(
            "capability: {}\nparameters: {:?}\nexclusions: {:?}\nlimitations: {:?}\nnotes: {:?}\nfailure: {:?}\nengine_version: {:?}\ninputs: {:?}\nparameters_hash: {:?}\noutput_hash: {}\nrows_read: {:?}\nrows_used: {:?}\nreproducibility: {:?}",
            run.run.capability_id.as_str(),
            run.parameters,
            run.exclusions,
            run.limitations,
            run.notes,
            run.failure,
            m.map(|m| &m.engine_version),
            m.map(|m| m.inputs.iter().map(|h| h.as_hex().to_string()).collect::<Vec<_>>()),
            m.map(|m| m.parameters_hash.as_hex().to_string()),
            match m {
                Some(_) if !captured => "pinned on macOS aarch64 only".to_string(),
                m => format!("{:?}", m.map(|m| m.output_hash.as_hex().to_string())),
            },
            m.map(|m| m.rows_read),
            m.map(|m| m.rows_used),
            m.map(|m| &m.reproducibility),
        )
    }

    /// `export_input`'s `ExportRun` for a saved one-sample t-test, a saved
    /// paired t-test and a saved paired t-test that failed, pinned in full
    /// (Research OS, Phase 8D-2a, decision 264): the test value's bits, the
    /// roles first and second, the shared codes, and the failure's code.
    #[test]
    fn export_input_gives_the_one_sample_and_paired_runs_settings_codes_and_manifest() {
        const PAIRS: &[u8] = b"a,b\n10,12.5\n20,21\n30,33.25\n40,41\n50,54.5\n";
        const SHIFTED: &[u8] = b"a,b\n1,2\n2,3\n4,5\n8,9\n";
        let paired = |slot: &Slot| {
            stats_onesample::paired_spec(
                OperationId(2),
                slot.dataset(),
                PairedParams {
                    first: Alias(ColumnAlias(1)),
                    second: Alias(ColumnAlias(2)),
                    missing_codes: vec![],
                },
            )
        };
        let runs = Arc::new(Runs::default());
        let one = saved_request(&runs, PAIRS, |runs, slot| {
            let spec = stats_onesample::one_sample_spec(
                OperationId(1),
                slot.dataset(),
                OneSampleParams {
                    outcome: Alias(ColumnAlias(1)),
                    test_value: Some(12.5),
                    missing_codes: vec![-99.0],
                },
            );
            runs.start_one_sample(slot, spec).unwrap()
        });
        let pair = saved_request(&runs, PAIRS, |runs, slot| {
            let spec = paired(&slot);
            runs.start_paired(slot, spec).unwrap()
        });
        let failed = saved_request(&runs, SHIFTED, |runs, slot| {
            let spec = paired(&slot);
            runs.start_paired(slot, spec).unwrap()
        });
        assert_eq!(failed.run.state, RunState::Failed, "every difference is -1");

        let root = std::env::temp_dir().join(format!("gaply-8d2a-export-{}", std::process::id()));
        let store = ProjectRoot::open_root_with(&root, Caps::REAL).expect("store opens");
        let (mut h, _) =
            ProjectRoot::create_with_save(&store, &ProjectTitle::parse("8d2a").unwrap(), one)
                .expect("project created");
        h.save(pair).expect("the paired t-test saves");
        h.save(failed).expect("the failed paired t-test saves");
        let input = export_input(&mut h);
        // Deleted before anything can fail, so a red run leaves no project.
        let id = h.id().as_str().to_string();
        h.close();
        ProjectRoot::delete(&store, &id).expect("the project is deleted");
        let got: Vec<String> = input
            .expect("the export's input")
            .runs
            .iter()
            .map(pinned_onesample)
            .collect();

        // Captured from this step's code on macOS aarch64, 26 Oct 2026.
        let captured = cfg!(all(target_os = "macos", target_arch = "aarch64"));
        let hash = |h: &str| {
            if captured {
                format!("Some(\"{h}\")")
            } else {
                "pinned on macOS aarch64 only".to_string()
            }
        };
        let want_one = format!(
            r#"capability: stats.analyze.one_sample_t
parameters: [InputShape {{ rows: 5, columns: 2 }}, Column {{ position: 1, role: Outcome }}, TestValue {{ bits: 4623226492472524800 }}, MissingCodes {{ count: 1 }}, Seed(None)]
exclusions: []
limitations: [ExportCode {{ code: ClosedWord("stats.onesample.values_normality_not_assessed"), params: [] }}, ExportCode {{ code: ClosedWord("stats.onesample.cohens_d_has_no_interval"), params: [] }}]
notes: []
failure: None
engine_version: Some(1)
inputs: Some(["3a029340c4cc5537ead4c6762d1dfa90503a9cbab25f74d50bfbe2aca26a22f2"])
parameters_hash: Some("6e68870598bb459021cd90e072bf51f66d85a3dfbfa1ef01acd2058593e5534a")
output_hash: {}
rows_read: Some(5)
rows_used: Some(5)
reproducibility: Some(Bitwise)"#,
            hash("0dc436b81d6dac23acadf2773cba04c5aaa850bf2210078289ee512a5d97ac9f")
        );
        let want_pair = format!(
            r#"capability: stats.analyze.paired_t
parameters: [InputShape {{ rows: 5, columns: 2 }}, Column {{ position: 1, role: First }}, Column {{ position: 2, role: Second }}, MissingCodes {{ count: 0 }}, Seed(None)]
exclusions: []
limitations: [ExportCode {{ code: ClosedWord("stats.onesample.differences_normality_not_assessed"), params: [] }}, ExportCode {{ code: ClosedWord("stats.onesample.cohens_d_has_no_interval"), params: [] }}]
notes: []
failure: None
engine_version: Some(1)
inputs: Some(["3a029340c4cc5537ead4c6762d1dfa90503a9cbab25f74d50bfbe2aca26a22f2"])
parameters_hash: Some("0e53cb1333569e55199758fc17a457521bd92d30d38193362f20c9624eb007b9")
output_hash: {}
rows_read: Some(5)
rows_used: Some(5)
reproducibility: Some(Bitwise)"#,
            hash("dbceb46dc78b46c8a22d5fba3f5016908bd0aa94426cbc1204c8dee86fda227e")
        );
        let want_failed = r#"capability: stats.analyze.paired_t
parameters: [InputShape { rows: 4, columns: 2 }, Column { position: 1, role: First }, Column { position: 2, role: Second }, MissingCodes { count: 0 }, Seed(None)]
exclusions: []
limitations: []
notes: []
failure: Some(ExportCode { code: ClosedWord("stats.onesample.differences_constant"), params: [] })
engine_version: None
inputs: None
parameters_hash: None
output_hash: None
rows_read: None
rows_used: None
reproducibility: None"#
            .to_string();
        let want: [String; 3] = [want_one, want_pair, want_failed];
        assert_eq!(got.len(), 3);
        let wrong: Vec<String> = got
            .iter()
            .zip(&want)
            .filter(|(g, w)| g.as_str() != w.as_str())
            .map(|(g, _)| g.clone())
            .collect();
        assert!(
            wrong.is_empty(),
            "\n--- got ---\n{}\n",
            wrong.join("\n--- next ---\n")
        );
    }
}
