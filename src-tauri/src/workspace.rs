//! **The window's workspace session (Research OS, Phase 3): which domain
//! workspace is active, checked in Rust, and the Statistics engine that exists
//! only while Statistics is.**
//!
//! * The state is `contracts::workspace::WorkspaceState`, changed only through
//!   its `transition`. One session per app, so one active workspace per
//!   window (the app has one window).
//! * **Activation** succeeds only when the descriptor is `Available` and the
//!   workspace's readiness check returns ready; otherwise it is refused with a
//!   closed code and the state is `NoWorkspace` again. The check runs outside
//!   the lock, so while it runs the state reads `Activating` and every
//!   operation command is refused.
//! * **The Statistics engine** (Phase 2B's run records and dataset slots) is
//!   built when Statistics becomes active and dropped when it is left. While
//!   no workspace is active, no engine value exists. Compute and Design have
//!   no engine in this build: there is nothing to construct for them, and the
//!   session has no field that could hold one.
//! * **Deactivation** is refused while a run is queued or running, with
//!   `workspace.run_active` and the run's id; the state stays `Active`, so the
//!   screen can offer Wait and Cancel through the run commands. Nothing is
//!   abandoned silently. The refusal and the end of new runs are one step:
//!   `Runs::seal` checks for an unfinished run and closes the manager under
//!   the run manager's own lock, so no run can start between them.
//! * **Ids continue** across activations: a new engine's first run and slot
//!   come after the last engine's, so an old id reads as unknown rather than
//!   naming something new. Slots are numbered from one counter the session
//!   shares with every engine, so even an open that lands in an engine after
//!   it was dropped takes a number the next engine never hands out
//!   (evaluation of Phases 2 and 3, F8).
//! * **Entitlement is not checked here** (decision 9): the list view carries
//!   `entitled: null` for the frontend to fill from `useEntitlement`.
//! * Only `workspace_activate` activates. Nothing else in the app calls
//!   [`WorkspaceSession::activate`].
//! * **The verifier's operation commands check it too (Phase 4A, decision
//!   49):** Level 0's `run_stats_consistency`, the legacy
//!   `run_stats_preview` and `run_stats_verify`, and every Level 1 command
//!   that opens, edits, proposes, previews, sends, confirms or computes.
//!   The commands that only remove a key or data, read or change settings,
//!   open a link, or stop a send are never gated; `tests/workspace_guards.rs`
//!   holds both lists by name.
//! * [`build`] is what every façade is told about this build: Experimental
//!   capabilities run only with the `devtools` feature (decision 48).
//!
//! * **The open project (Research OS, Phase 5B-2, decision 122)** is held
//!   beside the active workspace, in the session's
//!   [`ProjectSession`](crate::project_commands::ProjectSession). Leaving
//!   Statistics closes it: after the state has left `Active`, outside the
//!   session's lock, so a Save that holds the project finishes whole first and
//!   one that has not yet checked the state again is refused whole.
//!
//! Every command is `#[tracing::instrument(skip_all)]` and returns a view, or
//! a refusal that is a closed code with closed-word or numeric parameters.
//! Nothing here holds or logs user data.

use std::sync::atomic::AtomicU64;
use std::sync::{Arc, Mutex, MutexGuard};

use serde::Serialize;

use gaply_core::contracts::capability::{Build, CapabilityCode};
use gaply_core::contracts::codes::Closed;
use gaply_core::contracts::workspace::{
    self, Availability, WorkspaceCode, WorkspaceDescriptor, WorkspaceEvent, WorkspaceState,
};
use gaply_core::contracts::{Diagnostic, ParamKey, ParamValue, WorkspaceId};
use gaply_core::stats_registry;
use gaply_core::GaplyError;

use crate::project_commands::ProjectSession;
use crate::runs::{DatasetSlots, RunKind, Runs};

/// A readiness check's answer (blueprint §9.2's `health_check`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Health {
    Ready,
    NotReady,
}

/// The readiness check, per workspace. Production's is [`readiness`].
pub type HealthCheck = Arc<dyn Fn(WorkspaceId) -> Health + Send + Sync>;

/// Builds the Statistics run manager. Production's is `Runs::default`; a test
/// passes one with hooks.
pub type RunsFactory = Arc<dyn Fn() -> Arc<Runs> + Send + Sync>;

/// The production readiness check. Statistics has nothing to load, so it is
/// ready. Compute and Design have no engine in this build; they are refused as
/// unavailable before this is asked, and would be not ready if they were.
pub fn readiness(id: WorkspaceId) -> Health {
    match id {
        WorkspaceId::Statistics => Health::Ready,
        WorkspaceId::Compute | WorkspaceId::Design => Health::NotReady,
    }
}

/// What the Statistics workspace runs on: Phase 2B's run records and dataset
/// slots, in memory only.
pub struct StatisticsEngine {
    pub runs: Arc<Runs>,
    pub datasets: Arc<DatasetSlots>,
}

struct Inner {
    state: WorkspaceState,
    /// Present exactly while Statistics is active.
    statistics: Option<Arc<StatisticsEngine>>,
    /// The last run id handed out by an engine since dropped. A run cannot
    /// start once its engine is sealed, so a number carried over is enough.
    last_run: u64,
    /// Every engine's slots are numbered from this one counter: an open that
    /// passed the check before a deactivation inserts into the dropped
    /// engine, after any number could be carried over (F8).
    slot_numbers: Arc<AtomicU64>,
}

pub struct WorkspaceSession {
    inner: Mutex<Inner>,
    health: HealthCheck,
    runs: RunsFactory,
    /// The open project and the delete confirmations (decision 122).
    projects: Arc<ProjectSession>,
}

impl Default for WorkspaceSession {
    fn default() -> WorkspaceSession {
        WorkspaceSession::with_hooks(Arc::new(readiness), Arc::new(|| Arc::new(Runs::default())))
    }
}

fn lock(m: &Mutex<Inner>) -> MutexGuard<'_, Inner> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

impl WorkspaceSession {
    /// A session with an injected readiness check and run-manager factory.
    pub fn with_hooks(health: HealthCheck, runs: RunsFactory) -> WorkspaceSession {
        WorkspaceSession::with_projects(health, runs, Arc::new(ProjectSession::default()))
    }

    /// [`WorkspaceSession::with_hooks`], with the project session given: tests
    /// pass one with small caps and a clock they move.
    pub fn with_projects(
        health: HealthCheck,
        runs: RunsFactory,
        projects: Arc<ProjectSession>,
    ) -> WorkspaceSession {
        WorkspaceSession {
            inner: Mutex::new(Inner {
                state: WorkspaceState::NoWorkspace,
                statistics: None,
                last_run: 0,
                slot_numbers: Arc::new(AtomicU64::new(0)),
            }),
            health,
            runs,
            projects,
        }
    }

    /// The open project and the delete confirmations, beside the workspace.
    pub fn projects(&self) -> &Arc<ProjectSession> {
        &self.projects
    }

    pub fn state(&self) -> WorkspaceState {
        lock(&self.inner).state
    }

    /// The workspaces whose engine exists now: `[Statistics]` while it is
    /// active, otherwise none.
    pub fn engines_held(&self) -> Vec<WorkspaceId> {
        let inner = lock(&self.inner);
        inner
            .statistics
            .iter()
            .map(|_| WorkspaceId::Statistics)
            .collect()
    }

    /// THE workspace check: the Statistics engine while Statistics is active,
    /// or `workspace.inactive`. Every `stats_desc_*` command calls this
    /// first (`tests/workspace_guards.rs`).
    pub fn require_statistics(&self) -> Result<Arc<StatisticsEngine>, Diagnostic<WorkspaceCode>> {
        let inner = lock(&self.inner);
        workspace::require(inner.state, WorkspaceId::Statistics)?;
        Ok(inner
            .statistics
            .clone()
            .expect("the Statistics engine exists while Statistics is active"))
    }

    /// The unfinished run of the active workspace, if any, and its newest run,
    /// each with its kind (decision 80).
    pub fn runs_brief(&self) -> (Option<RunEntryView>, Option<RunEntryView>) {
        let engine = lock(&self.inner).statistics.clone();
        match engine {
            Some(e) => {
                let brief = e.runs.brief();
                let view = |(run, kind): (gaply_core::contracts::RunId, RunKind)| RunEntryView {
                    run_id: run.0,
                    kind,
                };
                (brief.unfinished.map(view), brief.latest.map(view))
            }
            None => (None, None),
        }
    }

    /// Activates `id`: `NoWorkspace → Activating(id)`, then the availability
    /// and readiness checks, then `Active(id)` with its engine built, or
    /// `NoWorkspace` again and the refusal.
    pub fn activate(&self, id: WorkspaceId) -> Result<WorkspaceState, Diagnostic<WorkspaceCode>> {
        {
            let mut inner = lock(&self.inner);
            inner.state = workspace::transition(inner.state, WorkspaceEvent::Activate(id))?;
        }

        // Outside the lock: while this runs, the state reads Activating.
        let refusal = if workspace::availability(id) != Availability::Available {
            Some(WorkspaceCode::Unavailable)
        } else if (self.health)(id) != Health::Ready {
            Some(WorkspaceCode::NotReady)
        } else {
            None
        };

        let mut inner = lock(&self.inner);
        let engine = match (refusal, id) {
            (None, WorkspaceId::Statistics) => {
                let runs = (self.runs)();
                runs.continue_after(inner.last_run);
                let datasets = DatasetSlots::numbered_by(Arc::clone(&inner.slot_numbers));
                Ok(Arc::new(StatisticsEngine {
                    runs,
                    datasets: Arc::new(datasets),
                }))
            }
            // Compute and Design have no engine to build.
            (None, WorkspaceId::Compute | WorkspaceId::Design) => Err(WorkspaceCode::NotReady),
            (Some(code), _) => Err(code),
        };
        match engine {
            Ok(engine) => {
                inner.state = workspace::transition(inner.state, WorkspaceEvent::Activated)?;
                inner.statistics = Some(engine);
                tracing::info!(workspace = workspace_word(id), "workspace: active");
                let state = inner.state;
                drop(inner);
                // Activating a workspace closes any project (decision 122).
                // None can be open here, since leaving closed it; this keeps
                // the rule true however the state was reached.
                self.projects.close_on_leave();
                Ok(state)
            }
            Err(code) => {
                inner.state =
                    workspace::transition(inner.state, WorkspaceEvent::ActivationRefused)?;
                tracing::info!(
                    workspace = workspace_word(id),
                    code = code.wire(),
                    "workspace: activation refused"
                );
                Err(Diagnostic::error(code).with(ParamKey::Workspace, ParamValue::Workspace(id)))
            }
        }
    }

    /// Leaves the active workspace: refused with `workspace.run_active` and
    /// the run's id while a run is queued or running, the state still
    /// `Active`. Otherwise `Active → Deactivating`, the engine dropped,
    /// `→ NoWorkspace`.
    pub fn deactivate(&self) -> Result<WorkspaceState, Diagnostic<WorkspaceCode>> {
        let state = self.leave()?;
        // Decision 122: leaving Statistics closes the open project. After the
        // state left `Active` and outside the session's lock: a Save holding
        // the project's lock finishes whole first, and every project command
        // checks the state again under that lock.
        self.projects.close_on_leave();
        Ok(state)
    }

    fn leave(&self) -> Result<WorkspaceState, Diagnostic<WorkspaceCode>> {
        let mut inner = lock(&self.inner);
        let WorkspaceState::Active(id) = inner.state else {
            return Err(
                workspace::transition(inner.state, WorkspaceEvent::Deactivate)
                    .expect_err("only Active has a deactivate edge"),
            );
        };
        if let Some(engine) = &inner.statistics {
            if let Err(run) = engine.runs.seal() {
                tracing::info!(
                    run = run.0,
                    "workspace: deactivation refused, a run is unfinished"
                );
                return Err(Diagnostic::error(WorkspaceCode::RunActive)
                    .int(ParamKey::Run, run.0 as i64)
                    .with(ParamKey::Workspace, ParamValue::Workspace(id)));
            }
        }
        inner.state = workspace::transition(inner.state, WorkspaceEvent::Deactivate)?;
        if let Some(engine) = inner.statistics.take() {
            inner.last_run = inner.last_run.max(engine.runs.last_id());
        }
        inner.state = workspace::transition(inner.state, WorkspaceEvent::Deactivated)?;
        tracing::info!(workspace = workspace_word(id), "workspace: left");
        Ok(inner.state)
    }
}

fn workspace_word(id: WorkspaceId) -> &'static str {
    match id {
        WorkspaceId::Statistics => "statistics",
        WorkspaceId::Compute => "compute",
        WorkspaceId::Design => "design",
    }
}

/* ------------------------------------------------------------------------ *
 *  Views and refusals
 * ------------------------------------------------------------------------ */

/// A workspace for the chooser: its descriptor, and `entitled`, which Rust
/// leaves `null` for the frontend to fill from `useEntitlement` (decision 9).
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceDescriptorView {
    #[serde(flatten)]
    pub descriptor: WorkspaceDescriptor,
    /// Not checked by Rust in Phase 3: always `null` here.
    pub entitled: Option<bool>,
}

/// A Statistics run as the shell names it: its id and its kind, so the shell
/// reads it through that kind's status and cancel commands (Research OS,
/// Phase 4B-2, decision 80).
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RunEntryView {
    pub run_id: u64,
    pub kind: RunKind,
}

/// The window's workspace state, and the Statistics runs the shell's job area
/// and switch dialog read (by id, through their kind's `*_status`).
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceStateView {
    pub state: WorkspaceState,
    /// The run that is queued or running, if any.
    pub active_run: Option<RunEntryView>,
    /// The newest run held, finished or not.
    pub latest_run: Option<RunEntryView>,
}

/// A workspace refusal as the webview reads it: the diagnostic in JSON,
/// `{"code":"workspace.inactive","params":{…},"severity":"error"}`.
pub fn refuse(d: Diagnostic<WorkspaceCode>) -> GaplyError {
    tracing::info!(code = d.code.wire(), "workspace: refused");
    GaplyError::Validation(serde_json::to_string(&d).expect("a diagnostic serialises"))
}

/// This build, as the capability registry reads it: `Devtools` exactly when
/// the app is built with its `devtools` feature (decision 48).
pub fn build() -> Build {
    Build::from_devtools(cfg!(feature = "devtools"))
}

/// A capability refusal as the webview reads it, in the same shape:
/// `{"code":"capability.not_runnable","params":{…},"severity":"error"}`.
pub fn refuse_capability(d: Diagnostic<CapabilityCode>) -> GaplyError {
    tracing::info!(code = d.code.wire(), "capability: refused");
    GaplyError::Validation(serde_json::to_string(&d).expect("a diagnostic serialises"))
}

/// `workspace.inactive` for the Statistics workspace, as a command refusal:
/// what a `stats_desc_*` command answers when the workspace was left between
/// its check and its work.
pub fn refuse_inactive() -> GaplyError {
    refuse(
        workspace::require(WorkspaceState::NoWorkspace, WorkspaceId::Statistics)
            .expect_err("nothing is active in NoWorkspace"),
    )
}

pub fn list() -> Vec<WorkspaceDescriptorView> {
    stats_registry::workspace_descriptors()
        .into_iter()
        .map(|descriptor| WorkspaceDescriptorView {
            descriptor,
            entitled: None,
        })
        .collect()
}

pub fn state_view(session: &WorkspaceSession) -> WorkspaceStateView {
    let state = session.state();
    let (active_run, latest_run) = session.runs_brief();
    WorkspaceStateView {
        state,
        active_run,
        latest_run,
    }
}

/* ------------------------------------------------------------------------ *
 *  The Tauri commands
 * ------------------------------------------------------------------------ */

type St<'a> = tauri::State<'a, crate::state::AppState>;

#[tauri::command]
#[tracing::instrument(skip_all)]
pub async fn workspace_list() -> Result<Vec<WorkspaceDescriptorView>, GaplyError> {
    Ok(list())
}

#[tauri::command]
#[tracing::instrument(skip_all)]
pub async fn workspace_state(state: St<'_>) -> Result<WorkspaceStateView, GaplyError> {
    Ok(state_view(&state.workspace))
}

#[tauri::command]
#[tracing::instrument(skip_all)]
pub async fn workspace_activate(
    state: St<'_>,
    id: WorkspaceId,
) -> Result<WorkspaceStateView, GaplyError> {
    let session = state.workspace.clone();
    tokio::task::spawn_blocking(move || {
        session.activate(id).map_err(refuse)?;
        Ok(state_view(&session))
    })
    .await
    .map_err(|_| GaplyError::Internal("workspace task panicked".into()))?
}

#[tauri::command]
#[tracing::instrument(skip_all)]
pub async fn workspace_deactivate(state: St<'_>) -> Result<WorkspaceStateView, GaplyError> {
    state.workspace.deactivate().map_err(refuse)?;
    Ok(state_view(&state.workspace))
}
