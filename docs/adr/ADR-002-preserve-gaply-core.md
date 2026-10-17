# ADR-002: Preserve `gaply_core` and the verifier; build beside, not inside

**Status: Proposed.** Not accepted until Rishi approves it.
Written 17 Oct 2026, Research OS Phase 3, from the Phase 0 audit's proposal
(`~/gaply-corpus/os/phase0/phase0_audit.md` §7) and what Phases 2A, 2B and 3
did under it (D271, D272 and the Phase 3 D-entry in `docs/AI_ENGINE_PLAN.md`).

## Context

`gaply_core` is the portable, Tauri-free crate that holds every business
rule (audit §2.3: about 108k lines, pure of HTTP, AI and UI). The
Statistical Analysis Verifier's Level 0 and Level 1 (`stats_l0`, `stats_l1`,
`stats_assist`) are its most tested part: R fixtures, scan guards, privacy
matrices. The blueprint (§6) asks for façades first and a split only when
measured. The Research OS needs generic contracts, a run manager and a
workspace rule, and each could have been written by copying or moving
verifier code.

## Decision

1. **No module is copied or renamed.** Each formula and parser has one
   implementation. When the Research OS needs one, it calls the verifier's:
   the descriptive summary uses `stats_verify::mean`,
   `stats_verify::sample_variance` and `binding::suspect_codes_in`, made
   `pub(crate)` with their bodies unchanged (decision 13), and reads files
   through Level 1's ingest, `load_dataset`.
2. **New generic code is a new module beside the verifier**,
   `gaply_core::contracts` (decision 2), guarded as pure and as naming no
   verifier or capability type (`tests/contracts_are_pure.rs`). Phase 3's
   `contracts::workspace` is inside that guard.
3. **The app crate holds state and I/O** (`runs.rs`, `workspace.rs`,
   `stats_desc_commands.rs`); `gaply_core` stays free of Tauri, clocks,
   threads and files in these modules.
4. **The verifier's rules and gates are unchanged** until a step says
   otherwise: Level 0 and Level 1 stay the one source of truth for their
   calculations, and are wrapped as capabilities in Phase 4 rather than
   moved. Phase 3 removed one command beside them, the legacy chat
   `run_stats_chat` (decision 8); `gaply_core::stats_chat` itself is
   unchanged and now has no app caller.
5. **A split into new crates waits for a measurement** (build time,
   dependency weight or licence), recorded first.

## Alternatives considered

- **Extract a `gaply_statistics` crate now.** Rejected: blueprint §6.2 says
  façades first; no build-time or dependency measurement asks for it.
- **A second copy of the mean and variance for the Research OS.** Rejected:
  two implementations drift, and the verifier's carry the part C fixes
  (two-pass mean, range-checked sum of squares).
- **Contracts that wrap the verifier's types** (`WireCode`,
  `ClaimReproduction`). Rejected in Phase 2A: the contracts would name a
  domain; each capability brings its own types as parameters instead.

## Consequences

- Visibility widens where reuse needs it (`pub(crate)`), each named in the
  purity guard, so a new use is visible in review.
- The verifier's test targets keep their counts through each phase; a change
  in one of them is a finding, not noise (Phase 2A, 2B and 3 reports list
  every target before and after).
- Moving code later is a sequence of small changes, each with its own tests,
  rather than one rewrite.

## Evidence

- `gaply-core/tests/contracts_are_pure.rs` (7 tests): no file, network,
  process, clock, model or log; imports limited to `std`, `serde`,
  `serde_json`, `sha2` and named crate paths; contracts name no domain type.
- Phase 2A report §2.4 and Phase 2B report §2: every earlier target's counts
  unchanged. Phase 3 report: the same, apart from the tests removed with the
  legacy chat, each listed.
- `gaply-core/tests/desc_output_hashes.rs`: the descriptive summary's bytes
  unchanged by Phase 3.

## Revision trigger

A measured reason to split (build time, a dependency that must not reach
the verifier, a licence), or a capability that cannot be written without
changing a verifier rule.
