# ADR-010: Budgets for runs, and how they are measured

**Status: Proposed.** Not accepted until Rishi approves it.
Written 17 Oct 2026, Research OS Phase 2B, from the Phase 0 audit's proposal
(`~/gaply-corpus/os/phase0/phase0_audit.md` §7) as updated by what Phase 2B
built.

## Context

Blueprint §17 says no operation may exceed its budget indefinitely, and
every capability should declare a resource profile. Before Phase 2 the only
budgets were the ingest's caps (100 MiB through `import_guard`, 10 MiB per
data file, 5,000 rows, 128 columns, 512 characters per cell, 2 MiB of cell
text in all, 64 MiB inflated for an archive since evaluation fixes part C)
and the AI helper's request budgets. The deep evaluation measured a 1.44 GB
peak from a 1.5 MB workbook (M-9) before part C bounded inflation.

## Decision

1. **The input is bounded at the door, once.** `stats_desc_open` reads
   through Level 1's ingest (`stats_l1_ingest::load_dataset`): a regular
   file, one open, a bounded read, inflation counted before parsing, then
   the parser's caps. There is no second ingest path. Every cap, with its
   value and what a file over it gets (listed in full since the evaluation
   of Phases 2 and 3, F4, which found the cell-text cap missing here and
   refused as `malformed`):

   | cap | value | over it |
   |---|---|---|
   | file bytes, `import_guard::MAX_BYTES` | 100 MiB | `too_large {bytes, limit}` |
   | data file bytes, `supplementary::MAX_FILE_SIZE_BYTES` | 10 MiB | `too_large {bytes, limit}` |
   | inflated bytes of a zip workbook or a `.mat`'s compressed elements, `MAX_WORKBOOK_UNCOMPRESSED_BYTES` | 64 MiB | `archive_too_large {declared, limit}` |
   | data rows, the header not counted, `supplementary::MAX_ROWS` | 5,000 | `file_too_large {limit_rows}` |
   | columns, `supplementary::MAX_COLS` | 128 | `dataset {too_wide, columns, limit}` |
   | characters per cell, `supplementary::MAX_CELL_LEN` | 512 | not refused: the cell is clamped to 512 |
   | cell text in all, `supplementary::MAX_TOTAL_CHARS`, counted in UTF-8 bytes after clamping, header included (CSV, TSV and spreadsheets) | 2 MiB (2,097,152 bytes) | `content_too_large {limit_bytes}` |

   The last cap binds before the row and column caps on a wide numeric
   table: 5,000 rows by 128 columns of 5-to-6-character numbers is about
   3.8 MB of cell text and is refused, while 3,000 by 128 opens. It stays
   as it is because it is the verifier's shared ingest limit (decision
   37).
2. **The run is bounded in time.** A wall-clock ceiling of 30 s from the
   run's start (`runs::RUN_CEILING_MS`, from Queued → Running), checked at
   the same check point as cancel, after the cancel flag, before each
   column. A run that reaches it ends `TimedOut`, unless a cancel was
   accepted before the run's last transition: then it ends `Cancelled`
   (§3).
3. **Work is cancellable.** The check point reads the cancel flag before
   each column, and the worker reads it again under the lock of its last
   transition, so a cancel accepted while the run runs always ends it
   `Cancelled`: also when it lands as the ceiling passes, or after the last
   check point, or before a panic. (Corrected 17 Oct 2026: the proposed text
   said this of the check point alone, which a cancel landing after the
   flag was read could miss and end `TimedOut`, evaluation of Phases 2 and
   3, F2.)
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
- `open_refuses_the_cell_text_cap_with_its_own_code_and_opens_a_file_at_it`
  (F4): the evaluation's 5,000 × 128 CSV and a workbook over 2 MiB of cell
  text refused as `content_too_large`, a CSV at exactly 2,097,152 bytes of
  cell text opened, one byte more refused; Level 1's open gives the same
  code (`a_data_file_over_the_cell_text_cap_is_refused_with_its_own_code`).
- `a_cancel_and_the_ceiling_at_the_same_check_point_end_cancelled_every_time`
  (F2): 1,000 runs, each with the cancel accepted inside the check point's
  reading of the clock, all `Cancelled`.
- The evaluation's debug measurement (F22): 3,000 × 128, every column, 685 ms
  of run time and 78.6 MiB peak; release still not measured.

## Revision trigger

The first measured budget breach; a capability whose inner loop runs longer
than its check-point interval; or Phase 4's capability descriptors.
