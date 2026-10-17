# ADR-010: Budgets for runs, and how they are measured

**Status: Proposed.** Not accepted until Rishi approves it.
Written 17 Oct 2026, Research OS Phase 2B, from the Phase 0 audit's proposal
(`~/gaply-corpus/os/phase0/phase0_audit.md` §7) as updated by what Phase 2B
built.

## Context

Blueprint §17 says no operation may exceed its budget indefinitely, and
every capability should declare a resource profile. Before Phase 2 the only
budgets were the ingest's caps (100 MiB through `import_guard`, 10 MiB per
data file, 5,000 rows, 128 columns, 512 characters per cell, 64 MiB
inflated for an archive since evaluation fixes part C) and the AI helper's
request budgets. The deep evaluation measured a 1.44 GB peak from a 1.5 MB
workbook (M-9) before part C bounded inflation.

## Decision

1. **The input is bounded at the door, once.** `stats_desc_open` reads
   through Level 1's ingest (`stats_l1_ingest::load_dataset`): a regular
   file, one open, a bounded read, inflation counted before parsing, the
   row and column caps. A file over a cap is refused with Level 1's code
   (`too_large`, `file_too_large`, `archive_too_large`, …). There is no
   second ingest path.
2. **The run is bounded in time.** A wall-clock ceiling of 30 s from the
   run's start (`runs::RUN_CEILING_MS`), checked at the same check point as
   cancel, before each column. A run that reaches it ends `TimedOut`.
3. **Work is cancellable.** The check point reads the cancel flag before
   each column; a cancel accepted while the run runs always ends it
   `Cancelled`.
4. **Memory is bounded by count.** At most 8 run records and 4 dataset
   slots, in memory only.
5. **The estimate is reported before the run.** `ValidationReport.
   estimated_resources` carries rows, columns and cells.
6. **Not yet:** a per-capability resource profile (RAM class, startup cost,
   recommended maximum input), a memory budget and its `ResourceLimit`
   state, and peak-RSS measurement. When added, peak RSS is measured on the
   8 GB class in a release build (CLAUDE.md: a debug build is a different
   order of magnitude) with a named harness, and recorded with the fixture.

## Alternatives considered

- **No ceiling until a user hits one.** Rejected by blueprint §17.
- **Killing the worker thread at the ceiling.** Rejected: Rust cannot stop a
  thread safely from outside, and blueprint §8.5 says cancellation is
  cooperative first. A check point per column is enough for a summary of at
  most 128 columns; an operation with long inner loops needs finer check
  points, or a worker process (ADR-006).
- **A ceiling per capability now.** Deferred: one capability exists; the
  constant moves into its descriptor in Phase 4.

## Consequences

- A column is the unit of interruption: a cancel or the ceiling waits for
  the column in progress to finish. For 5,000 rows a column takes well under
  a second (inferred from the Phase 2A property tests: 5 × 1,024 cases in
  0.85 s, debug); not measured at the caps in a release build.
- `ResourceLimit` has no producer yet.
- The 30 s figure is a ceiling, not a measured budget: the descriptive
  summary at the caps is expected to finish far below it.

## Evidence

- `src-tauri/tests/stats_desc_commands.rs`: a ceiling of 0 ends a run
  `TimedOut` with no column done; a run parked at column k and cancelled
  ends `Cancelled` with k columns done; mutation probe: removing the cancel
  check lets the run process every column.
- `open_refuses_a_file_over_the_caps_with_its_code_and_holds_nothing`: 5,001
  rows, over 10 MiB, and a folder each refused with their code.

## Revision trigger

The first measured budget breach; a capability whose inner loop runs longer
than its check-point interval; or Phase 4's capability descriptors.
