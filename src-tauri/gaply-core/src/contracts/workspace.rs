//! **Workspaces (Research OS, Phase 3): what each domain workspace is in this
//! build, and the one state machine that says which of them is active.**
//!
//! Pure, like the rest of `contracts`: no file, clock, thread or log
//! (`tests/contracts_are_pure.rs`). The app holds the state; this module only
//! says how it may change.
//!
//! Three separate concepts, never one boolean (blueprint §3.2):
//! * **available**: [`WorkspaceDescriptor::availability`], fixed per build.
//!   Statistics is `Available`; Compute and Design are `Missing`, which the
//!   chooser shows as "Coming later". A `Missing` descriptor claims nothing:
//!   version 0, no capability, no platform, not offline.
//! * **entitled**: not here. In Phase 3 the frontend reads it from
//!   `useEntitlement` and shows it; Rust does not check it (decision 9).
//! * **active**: a [`WorkspaceState`], changed only through [`transition`]:
//!
//! ```text
//! NoWorkspace     –activate(w)→         Activating(w)
//! Activating(w)   –activated→           Active(w)
//! Activating(w)   –activation_refused→  NoWorkspace
//! Active(w)       –deactivate→          Deactivating(w)
//! Deactivating(w) –deactivated→         NoWorkspace
//! ```
//!
//! Every other pair is refused with `workspace.illegal_transition`. So a
//! second workspace can never become active beside the first: from
//! `Active(w)` the only way out is through `NoWorkspace`.

use serde::{Deserialize, Serialize};

use super::codes::closed;
use super::{CapabilityId, Diagnostic, Os, ParamKey, ParamValue, WorkspaceId};

/// Whether a workspace exists in this build and runs on this platform.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Availability {
    Available,
    /// Not in this build. Shown as "Coming later".
    Missing,
    /// In this build, but not for this platform. No workspace is, today.
    Incompatible,
}

/// How a workspace shares the window with the others. One value only: one
/// active workspace per window (blueprint §3.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActivationPolicy {
    SingleActive,
}

closed! {
    /// A workspace's name, as a code the screen's wording table turns into
    /// words.
    pub enum DisplayNameCode in "workspace.name." {
        Statistics => "statistics",
        Compute => "compute",
        Design => "design",
    }
}

closed! {
    /// Why a workspace request was refused.
    pub enum WorkspaceCode in "workspace." {
        /// The workspace is not available in this build. Param `workspace`.
        Unavailable => "unavailable",
        /// Its readiness check did not return ready. Param `workspace`.
        NotReady => "not_ready",
        /// The command needs a workspace that is not active. Params
        /// `expected` (the workspace it needs) and `from` (the phase now).
        Inactive => "inactive",
        /// A spec names a workspace that is not the active one, or a
        /// capability of another workspace. Params `expected` and `found`.
        WrongWorkspace => "wrong_workspace",
        /// A run is queued or running, so the workspace cannot be left.
        /// Params `run` (its id) and `workspace`.
        RunActive => "run_active",
        /// The event has no edge from the state. Params `from`, `event`, and
        /// `workspace` when the state names one.
        IllegalTransition => "illegal_transition",
    }
}

closed! {
    /// A [`WorkspaceState`] without its workspace: the word for the phase.
    pub enum WorkspacePhase in "" {
        NoWorkspace => "no_workspace",
        Activating => "activating",
        Active => "active",
        Deactivating => "deactivating",
    }
}

closed! {
    /// A [`WorkspaceEvent`] without its workspace.
    pub enum WorkspaceEventKind in "" {
        Activate => "activate",
        Activated => "activated",
        ActivationRefused => "activation_refused",
        Deactivate => "deactivate",
        Deactivated => "deactivated",
    }
}

/// What a domain workspace is in this build (blueprint §7.1, without its
/// engine requirements, memory class and migrations: none exist yet).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkspaceDescriptor {
    pub id: WorkspaceId,
    /// 0 for a workspace this build does not have.
    pub version: u32,
    pub display_name_code: DisplayNameCode,
    pub availability: Availability,
    pub activation_policy: ActivationPolicy,
    pub capability_ids: Vec<CapabilityId>,
    pub supports_offline: bool,
    pub supported_platforms: Vec<Os>,
}

/// Whether `id` is available in this build. Static: the one place it is
/// decided, read by [`descriptor`] and by `CapabilityId::parse`.
pub fn availability(id: WorkspaceId) -> Availability {
    match id {
        WorkspaceId::Statistics => Availability::Available,
        WorkspaceId::Compute => Availability::Missing,
        WorkspaceId::Design => Availability::Missing,
    }
}

/// The descriptor of `id` in this build, carrying the capability ids its
/// workspace's registry lists. The contracts name no capability, so the list
/// is given: `stats_registry::workspace_descriptor` passes the Statistics
/// table's ids, and there is no second list (decision 47). A `Missing`
/// workspace claims nothing, whatever it is given, and an id of another
/// workspace is dropped.
pub fn descriptor_with(id: WorkspaceId, capability_ids: Vec<CapabilityId>) -> WorkspaceDescriptor {
    let missing = |display_name_code| WorkspaceDescriptor {
        id,
        version: 0,
        display_name_code,
        availability: availability(id),
        activation_policy: ActivationPolicy::SingleActive,
        capability_ids: Vec::new(),
        supports_offline: false,
        supported_platforms: Vec::new(),
    };
    match id {
        WorkspaceId::Statistics => WorkspaceDescriptor {
            id,
            version: 1,
            display_name_code: DisplayNameCode::Statistics,
            availability: availability(id),
            activation_policy: ActivationPolicy::SingleActive,
            capability_ids: capability_ids
                .into_iter()
                .filter(|c| c.workspace() == id)
                .collect(),
            supports_offline: true,
            supported_platforms: vec![Os::Macos, Os::Windows, Os::Linux],
        },
        WorkspaceId::Compute => missing(DisplayNameCode::Compute),
        WorkspaceId::Design => missing(DisplayNameCode::Design),
    }
}

/// Which workspace is active in a window, if any, and whether it is on its
/// way in or out. On the wire: `{"phase":"active","workspace":"statistics"}`,
/// or `{"phase":"no_workspace"}`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(
    tag = "phase",
    content = "workspace",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum WorkspaceState {
    NoWorkspace,
    Activating(WorkspaceId),
    Active(WorkspaceId),
    Deactivating(WorkspaceId),
}

impl WorkspaceState {
    pub fn phase(self) -> WorkspacePhase {
        match self {
            WorkspaceState::NoWorkspace => WorkspacePhase::NoWorkspace,
            WorkspaceState::Activating(_) => WorkspacePhase::Activating,
            WorkspaceState::Active(_) => WorkspacePhase::Active,
            WorkspaceState::Deactivating(_) => WorkspacePhase::Deactivating,
        }
    }

    /// The workspace the state names, if any.
    pub fn workspace(self) -> Option<WorkspaceId> {
        match self {
            WorkspaceState::NoWorkspace => None,
            WorkspaceState::Activating(w)
            | WorkspaceState::Active(w)
            | WorkspaceState::Deactivating(w) => Some(w),
        }
    }

    /// Every state: `NoWorkspace`, then each phase with each workspace.
    pub fn all() -> Vec<WorkspaceState> {
        let mut all = vec![WorkspaceState::NoWorkspace];
        for w in WorkspaceId::ALL {
            all.push(WorkspaceState::Activating(*w));
            all.push(WorkspaceState::Active(*w));
            all.push(WorkspaceState::Deactivating(*w));
        }
        all
    }
}

/// What happened to the window's workspace.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WorkspaceEvent {
    /// The user chose a workspace.
    Activate(WorkspaceId),
    /// It is available and its readiness check returned ready.
    Activated,
    /// It is not available, or not ready.
    ActivationRefused,
    /// The user asked to leave it, and no run is queued or running.
    Deactivate,
    /// Its engine was dropped.
    Deactivated,
}

impl WorkspaceEvent {
    pub fn kind(self) -> WorkspaceEventKind {
        match self {
            WorkspaceEvent::Activate(_) => WorkspaceEventKind::Activate,
            WorkspaceEvent::Activated => WorkspaceEventKind::Activated,
            WorkspaceEvent::ActivationRefused => WorkspaceEventKind::ActivationRefused,
            WorkspaceEvent::Deactivate => WorkspaceEventKind::Deactivate,
            WorkspaceEvent::Deactivated => WorkspaceEventKind::Deactivated,
        }
    }

    /// Every event: one `Activate` per workspace, then the others.
    pub fn all() -> Vec<WorkspaceEvent> {
        let mut all: Vec<WorkspaceEvent> = WorkspaceId::ALL
            .iter()
            .map(|w| WorkspaceEvent::Activate(*w))
            .collect();
        all.extend([
            WorkspaceEvent::Activated,
            WorkspaceEvent::ActivationRefused,
            WorkspaceEvent::Deactivate,
            WorkspaceEvent::Deactivated,
        ]);
        all
    }
}

/// The one way the window's workspace changes: the five edges in the module
/// header, and no others.
pub fn transition(
    from: WorkspaceState,
    event: WorkspaceEvent,
) -> Result<WorkspaceState, Diagnostic<WorkspaceCode>> {
    use WorkspaceEvent as E;
    use WorkspaceState as S;
    match (from, event) {
        (S::NoWorkspace, E::Activate(w)) => Ok(S::Activating(w)),
        (S::Activating(w), E::Activated) => Ok(S::Active(w)),
        (S::Activating(_), E::ActivationRefused) => Ok(S::NoWorkspace),
        (S::Active(w), E::Deactivate) => Ok(S::Deactivating(w)),
        (S::Deactivating(_), E::Deactivated) => Ok(S::NoWorkspace),
        _ => {
            let refusal = Diagnostic::error(WorkspaceCode::IllegalTransition)
                .with(ParamKey::From, ParamValue::Phase(from.phase()))
                .with(ParamKey::Event, ParamValue::WorkspaceEvent(event.kind()));
            Err(match from.workspace() {
                Some(w) => refusal.with(ParamKey::Workspace, ParamValue::Workspace(w)),
                None => refusal,
            })
        }
    }
}

/// The check every operation-bearing command makes first: `needed` must be
/// the active workspace. Refused with `workspace.inactive` otherwise,
/// `Activating` and `Deactivating` included.
pub fn require(
    state: WorkspaceState,
    needed: WorkspaceId,
) -> Result<(), Diagnostic<WorkspaceCode>> {
    if state == WorkspaceState::Active(needed) {
        return Ok(());
    }
    Err(Diagnostic::error(WorkspaceCode::Inactive)
        .with(ParamKey::Expected, ParamValue::Workspace(needed))
        .with(ParamKey::From, ParamValue::Phase(state.phase())))
}

/// A spec may run only in the active workspace, and only for a capability of
/// that workspace (blueprint §9.1): its `workspace_id` and its capability's
/// namespace must both be `active`. Refused with `workspace.wrong_workspace`,
/// whatever the payload claims.
pub fn check_spec(
    active: WorkspaceId,
    spec_workspace: WorkspaceId,
    capability: &CapabilityId,
) -> Result<(), Diagnostic<WorkspaceCode>> {
    let found = if spec_workspace != active {
        spec_workspace
    } else if capability.workspace() != active {
        capability.workspace()
    } else {
        return Ok(());
    };
    Err(Diagnostic::error(WorkspaceCode::WrongWorkspace)
        .with(ParamKey::Expected, ParamValue::Workspace(active))
        .with(ParamKey::Found, ParamValue::Workspace(found)))
}
