# ADR-003: The operation contract and the run manager

**Status: Accepted, 17 Oct 2026 (decision 26).**
Written 17 Oct 2026, Research OS Phase 2B, from the Phase 0 audit's proposal
(`~/gaply-corpus/os/phase0/phase0_audit.md` §7) as updated by what Phases 2A
and 2B built (D271, D272 in `docs/AI_ENGINE_PLAN.md`).

## Context

The Statistical Analysis Verifier already has a closed method catalogue,
closed wire codes and a confirmation token, all specific to Level 1. The
Research OS needs one typed envelope that every capability's operation, its
validation, its run, its result and its manifest travel in (blueprint §8,
§9), without a universal request object and without touching the verifier.

Before Phase 2 nothing generic existed: 116 commands, each with its own
shape, and Level 1's run store (`L1Runs`, four runs, no state machine).

## Decision

1. **The contract is a pure module, `gaply_core::contracts`** (decision 2),
   guarded by `tests/contracts_are_pure.rs` (no file, network, process,
   clock, model or log; imports `std`, `serde`, `serde_json`, `sha2` only).
   What differs per capability is a type parameter: `OperationSpec<P>`,
   `ValidationReport<C>`, `Diagnostic<C>`, `ResultBundle<T, C>`,
   `RunManifest<C>`. It names no verifier type.
2. **One state machine.** `RunState` holds all fifteen states of blueprint
   §8.5, and `transition` holds its eighteen edges and no others. Every state
   change in the app goes through it (`src-tauri/src/runs.rs`, `Runs::step`).
   A terminal state has no way out; a changed spec is a new run (decision
   14).
3. **Closed codes, namespaced.** `run.*` for the run manager (`RunCode`,
   ten codes since Phase 2B added `unknown_input`, `not_finished` and
   `no_result`), `stats.desc.*` for the descriptive summary (`DescCode`).
   A diagnostic's parameters are integers or closed words, never text.
4. **The run manager is the app's, in memory** (`runs.rs`, decision 6): at
   most 8 run records, the oldest finished one dropped first and an
   unfinished one never; at most 4 dataset slots. One foreground run at a
   time (`run.run_busy`). A run is detached: created, validated, queued and
   started on its own thread, then polled. Cancel is cooperative, through a
   check point the operation calls between columns
   (`stats_desc::describe_until`); the check point also enforces the time
   ceiling (ADR-010). A cancelled run is never reported as succeeded: a
   cancel accepted while the run is queued or running ends it `Cancelled`,
   even when it lands after the last check point or as the ceiling passes,
   because the worker reads the flag again under the lock of its last
   transition and a set flag decides the end. (Sharpened 17 Oct 2026: the
   evaluation of Phases 2 and 3 found the claim held by one unpinned line,
   F1, and broken at the ceiling, F2.)
5. **Commands are thin and return views** (`stats_desc_commands.rs`): each
   `#[tracing::instrument(skip_all)]`, refusals as a closed code in JSON,
   never a path. The webview sends the full `OperationSpec`; Rust validates
   it, including that its input is the open dataset's content hash.
6. **The registry is Phase 4's.** `CapabilityId` is a closed grammar today,
   and `validate` compares it with `stats.describe` exactly; Phase 4 checks
   it against a static registry (decision 16) and brings the Level 1 wrap,
   with its confirmation token, through the same envelope.

## Alternatives considered

- **A universal request object** (one struct with optional fields for every
  domain). Rejected, as blueprint §8 says: it grows without bound and its
  validation becomes a list of exceptions.
- **A new `gaply_contracts` crate.** Rejected for now (decision 2): no
  measured build-time reason; a module behind a purity guard gives the same
  separation.
- **Wrapping `stats_l1::codes::WireCode` in `Diagnostic`** (the audit's first
  proposal). Rejected in 2A: it would make the contract name a verifier
  type. Each capability brings its own closed code enum instead.
- **Run state in React.** Rejected: blueprint §9.1 puts job lifecycle in the
  execution manager, and the webview is untrusted (§16). The screen polls a
  view and decides nothing.
- **An event channel for progress.** Not built in Phase 2B (the prompt
  excludes it): polling `stats_desc_status` is enough for a 30-second
  ceiling, and a channel can be added without changing the views.

## Consequences

- A second capability adds its own parameter, result and code types and
  reuses the envelope, the state machine, the run manager and the view
  pattern.
- `runs.rs` is specific to the descriptive summary's result type today;
  Phase 4 makes the job generic when a second capability exists.
- `AwaitingConfirmation`, `Invalidated`, `ResourceLimit` and
  `NumericalFailure` have no producer yet; the commands reach eleven of the
  eighteen edges (`tests/stats_desc_commands.rs`,
  `every_transition_the_commands_can_reach`).
- Blueprint §8.5's "persist terminal state even if the UI closes" is not met:
  a record lives as long as the process (ADR-004). A record keeps its
  terminal state for as long as it is held.

## Evidence

- `gaply-core/tests/contracts_wire.rs`: the edge table pair by pair (18 Ok,
  207 refused), the code lists, the wire rules.
- `src-tauri/tests/stats_desc_commands.rs`: the flow through IPC, refusals,
  park-and-cancel, a ceiling of 0, a second run refused, eviction, every
  reachable edge by the record's path.
- Mutation probes, Phase 2B report: removing the cancel check, or the busy
  check, turns a test red.
- Evaluation fixes (D274): `a_cancel_after_the_last_check_point_ends_cancelled_with_no_result`,
  `a_cancel_and_the_ceiling_at_the_same_check_point_end_cancelled_every_time`
  (1,000 runs, deterministic through an injected clock) and
  `a_cancel_accepted_before_the_describe_loop_panics_ends_cancelled`, each red
  with the worker's cancel arm removed.
- `src-tauri/tests/stats_desc_wire.rs` and `src/screens/statistics/
  describeTypes.vitest.ts`: each view's bytes pinned on both sides.

## Revision trigger

A second domain needs a result subtype or a run event the envelope cannot
carry; or Phase 4's Level 1 wrap needs a confirmation step that
`AwaitingConfirmation` cannot express.
