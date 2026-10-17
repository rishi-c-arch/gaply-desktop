# ADR-001: One active workspace per window, checked in Rust

**Status: Proposed.** Not accepted until Rishi approves it.
Written 17 Oct 2026, Research OS Phase 3, from the Phase 0 audit's proposal
(`~/gaply-corpus/os/phase0/phase0_audit.md` §6.8, §7) as built in Phase 3
(the "Research OS, Phase 3" D-entry in `docs/AI_ENGINE_PLAN.md`).

## Context

Blueprint §3 makes one workspace at a time a product rule: the user picks
Statistics, Compute or Design; only that workspace's engines are initialised
and only it can submit work; switching is explicit and never abandons a run.
§3.2 keeps three facts apart (available, entitled, active), and says a
feature flag hides unfinished UI and is never an authorization boundary.

Before Phase 3 there was no workspace concept: every command was registered
unconditionally and every screen was one click from the hub (audit §2.1,
§2.2). Phase 2B added the descriptive summary's eight `stats_desc_*`
commands and its run records and dataset slots, held in `AppState` from
startup whether or not anyone used them.

## Decision

1. **The state machine is pure and lives in the contracts**
   (`gaply_core::contracts::workspace`, under the Phase 2A purity guard):
   `WorkspaceState { NoWorkspace, Activating(w), Active(w), Deactivating(w) }`
   and `transition(state, event) -> Result<WorkspaceState,
   Diagnostic<WorkspaceCode>>` with five edges:

   | from | event | to |
   |---|---|---|
   | NoWorkspace | activate(w) | Activating(w) |
   | Activating(w) | activated | Active(w) |
   | Activating(w) | activation_refused | NoWorkspace |
   | Active(w) | deactivate | Deactivating(w) |
   | Deactivating(w) | deactivated | NoWorkspace |

   Every other pair is `workspace.illegal_transition`. From `Active(w)` the
   only way to another workspace is through `NoWorkspace`.
2. **Availability is static per build** (`workspace::availability`):
   Statistics `Available`; Compute and Design `Missing`, shown as "Coming
   later". A `Missing` descriptor claims nothing (version 0, no capability,
   no platform, not offline), and a capability id in its namespace is
   refused by `CapabilityId::parse`.
3. **The session is the app's** (`src-tauri/src/workspace.rs`,
   `WorkspaceSession` in `AppState`). Activation succeeds only when the
   descriptor is `Available` and the readiness check returns ready;
   otherwise it is refused and the state is `NoWorkspace` again. The
   Statistics engine (Phase 2B's run records and dataset slots) is built on
   activation and dropped on deactivation; while no workspace is active no
   engine value exists, and there is no field that could hold a Compute or
   Design engine.
4. **Enforcement is in Rust** (decision 27): every `stats_desc_*` command's
   first statement is the workspace check, and a spec whose workspace, or
   whose capability's workspace, is not the active one is refused
   (`workspace.wrong_workspace`). The verifier's Level 0 and Level 1
   commands are not gated until Phase 4 wraps them as capabilities.
5. **Leaving is never silent.** Deactivation while a run is queued or
   running is refused with `workspace.run_active` and the run's id; the
   state stays `Active`, and the screen offers Wait and Cancel through the
   run commands. The refusal and the end of new runs are one step under the
   run manager's lock (`Runs::seal`).
6. **Entitled is shown, not enforced, in Phase 3** (decision 9): the list
   view carries `entitled: null`, filled by the frontend from
   `useEntitlement`.
7. **The webview reads, never decides.** Only `workspace_activate` calls
   `WorkspaceSession::activate`; the chooser, the shell and the describe
   screen's gate each make that one call after a user's click and show
   what Rust returns. Polling only (decision 24).

## Alternatives considered

- **React-only gating.** Rejected: a flag or a route is not an
  authorization boundary (blueprint §3.2); the webview is untrusted (§16).
- **The state in the app crate only, no pure machine.** Rejected: the edge
  table is a contract the frontend mirrors and tests pin, and the
  contracts' purity guard covers it at no cost.
- **Entering `Deactivating` before checking for a run, with an abort edge
  back to `Active`.** Rejected for now: it needs an extra edge and, while
  deactivating, the run commands the dialog needs (status, cancel) would be
  refused. Checking first keeps `Active` until the run has ended.
- **Separate binaries or plug-ins per workspace.** Rejected (blueprint
  §3.3): packaging and signing cost before a second workspace exists.

## Consequences

- Phase 2B's run records and slots are no longer in `AppState` directly;
  their ids continue across activations, so an old id reads as unknown.
- The descriptive summary is unusable until Statistics is activated. With
  only its own flag on, the describe screen shows a gate whose button
  activates Statistics on the user's click.
- The features outside Statistics (citations, notes, PublishReady, journal,
  copilot) stay reachable as today (decision 7); where they live is a later
  decision.
- A feature that adds an operation-bearing command must add the check;
  `tests/workspace_guards.rs` covers `stats_desc_*` only, by name.

## Evidence

- `gaply-core/tests/workspace_contract.rs`: 70 (state, event) pairs, 15 Ok
  and 55 refused; every state reachable; no edge from a named workspace to
  another; the descriptors; the checks; the wire.
- `src-tauri/tests/workspace_session.rs`: refusals for a missing workspace
  and a failing readiness check, every `stats_desc_*` command refused while
  `NoWorkspace` and while `Activating`, a forged Compute spec, deactivation
  refused during a parked run and allowed after cancel, ids unknown after
  reactivation, nothing built for Compute or Design.
- `src-tauri/tests/workspace_guards.rs` and `workspace_privacy.rs`.
- Mutation probes (Phase 3 report): the check removed from one command, Compute
  made available, deactivation allowed during a run — each red.

## Revision trigger

A second workspace reaches Experimental (it then needs its own engine,
readiness check and operation commands under the same check), or the user
asks for background work in a second workspace (blueprint §3.4).
