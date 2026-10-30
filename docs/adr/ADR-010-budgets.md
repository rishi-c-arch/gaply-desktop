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

## Addendum: each capability's resource profile (Phase 4A)

**Status: Proposed** (this ADR stays Proposed). Research OS Phase 4A,
18 Oct 2026. The profiles are literals in `gaply_core::stats_registry`,
pinned to the app's constants by `src-tauri/tests/capability_profiles.rs`
and to `import_guard::MAX_BYTES` by `gaply-core/tests/capability_registry.rs`.
A limit that does not exist is `None`.

| capability | RAM class | max rows | max columns | max input bytes | ceiling | cancel | GPU | temp disk |
|---|---|---|---|---|---|---|---|---|
| `stats.check` | 8 GB (unconfirmed at the cap, below) | none | none | 104,857,600: the manuscript file (`import_guard::MAX_BYTES`) | none | none | no | no |
| `stats.verify.*` (7) | 8 GB | 5,000 | 128 | 2,097,152: cell text in UTF-8 bytes (`MAX_TOTAL_CHARS`) | none | none: compute is one call | no | no |
| `stats.describe` | 8 GB | 5,000; 100,000 since 8F-2 | 128 | 2,097,152, as above; none since 8F-2, set by the computer (below) | 30,000 ms (`RUN_CEILING_MS`) | cooperative, per column | no | no |

**What was measured, and how.** Peak resident memory with `/usr/bin/time -l`,
one capability per process, through a probe in `src-tauri/examples/` that
existed only for the run (kept, with its SHA-256, in
`~/gaply-corpus/os/phase4/mem/`). macOS aarch64, 8 GiB RAM, the **dev
profile** (`debug = "line-tables-only"`); a release build was not made (no
release cache, and the disk was near its 15 GiB floor). Synthetic inputs
only.

| run | input | peak RSS | time (dev) |
|---|---|---|---|
| an idle process | none | 10,305,536 B (9.8 MiB) | — |
| `stats.describe` | 5,000 × 128 numbers of three characters (1.92 MB of cell text, under the 2 MiB cap; the row and column caps both reached), all 128 columns numeric | 98,615,296 B (94.0 MiB) | 1.24 s |
| `stats.verify.ols` | the same table; outcome C1 and 127 predictors, the overall F, through open, confirm and compute | 723,140,608 B (689.6 MiB) | 239 s |
| `stats.check` | a 10 MiB manuscript of results (134,433 read) | 439,566,336 B (419.2 MiB) | 341 s, under load |
| `stats.check` | the 100 MiB cap | **not measured** | — |

The 100 MiB run was not made: the disk was at 14.9 GiB, under the 15 GiB
floor, and on an 8 GiB machine a run of several GiB would swap onto the same
volume. *Inferred, not measured:* if peak RSS grows with the results read, as
from 10 MiB it would, a manuscript at the cap needs about ten times 419 MiB,
which does not fit the 8 GB class beside the app. Until it is measured,
`stats.check`'s 8 GB class holds for manuscripts up to the measured size
only. Level 1's OLS at the caps is seven times the descriptive summary's
peak; both fit the 8 GB class.

## Addendum: the methods' open at the 8 GB limits (Phase 8F-1, decision 285)

**What was measured, and how.** A release probe (`cargo build --release
--example p8f1_probe`, 3 min 41 s; the source kept as
`~/gaply-corpus/os/phase8/8f1/p8f1_probe.rs.kept`, SHA-256 `e610ef5e…`, and
removed from `examples/` after the runs) called the methods' own open,
`stats_desc_commands::open_with`, with each tier's `IngestLimits` and a
counter for its rows read, then `validate`, then the summary through the run
manager (`Runs::start`, polled to its end). One case per process under
`/usr/bin/time -l`, through `run_probe.sh`, which refused to run under 15 GiB
free and waited while anything built. The footprint is `proc_pid_rusage`'s
`ri_lifetime_max_phys_footprint` (decision 242.9), with stops at 3 GiB and
120 s; neither was reached. Synthetic CSVs, numbers `ddd.dd`, seed 20261026.
macOS aarch64, 8 GiB RAM, 30 Oct 2026. A run during which swap grew was run
once more, as 8A did; the clean run is quoted.

| case | limits | open s (with the profile) | rows read | validate s | summary s | peak footprint MiB | note |
|---|---|---|---|---|---|---|---|
| 100,000 × 50, 5,000,000 cells | 8 GB | 2.76 | 100,000 | 1.30 | 5.27 | **670** | opens; the slot then holds 206 MiB (about 43 B a cell); 8A: 642–648 MiB, so +3.5% |
| 100,000 × 51 | 8 GB | 0.23 to the refusal | 98,039 | — | — | 278 | `too_many_cells`, while parsing |
| 100,000 × 80 | 8 GB | 0.23 to the refusal | 62,500 | — | — | 309 | `too_many_cells`, while parsing |
| three opens of 5,000,000 cells | 8 GB, budget 10,000,000 | 2.60, 2.53, 2.65 | — | — | — | 637 | the oldest dropped at the third; two held, 525 MiB after |
| 100,000 × 128, 12,800,000 cells | 16 GB, **on an 8 GB Mac** | 6.16 | 100,000 | 3.77 | 14.08 | 1,690 | swap grew in both runs: contaminated (first run 1,754); 8A: 1,611 |

Decision 285's stop (the 8 GB open's peak more than 25% above 8A's 0.65 GB)
was not reached: 670 MiB. The 16 and 32 GB tiers stay inferred: the row above
is their largest open measured on this 8 GB Mac. The app's own memory (the
webview and the rest) comes on top of every figure here; 8F-2's real-app
check reads it in Activity Monitor.

## Addendum: the methods' limits in force, by tier (Phase 8F-2, decision 288)

**Status: Proposed** (this ADR stays Proposed). Research OS Phase 8F-2,
30 Oct 2026, D303. The methods on the user's data (the summary and the three
t-tests) open, and a project reruns, through `IngestLimits::this_computer()`:
`IngestLimits::for_tier` of `memory_tier::this_computer()`'s tier, the memory
read once per process. The Statistics engine's open datasets take
`MethodLimits::for_tier`'s slot budget. Each method's screen states the limits
before a file is chosen (`stats_desc_limits`).

| in force | 8 GB (under 12 GiB, or unknown) | 16 GB | 32 GB |
|---|---|---|---|
| data rows | 100,000 | 100,000 | 100,000 |
| columns | 128 | 128 | 128 |
| cells | 5,000,000 | 12,800,000 | 12,800,000 |
| file bytes | 64 MiB | 96 MiB | 96 MiB |
| cell text in all | 64 MiB | 96 MiB | 96 MiB |
| workbook inflated | 96 MiB | 256 MiB | 256 MiB |
| cells held across the open datasets | 10,000,000 | 51,200,000 | 51,200,000 |

The four methods' resource profiles in `stats_registry` give 100,000 rows and
128 columns (the same on every tier) and no input bytes, since those depend on
the computer; their limitation `limit.table.limits_by_memory` says so in place
of `limit.table.cell_text_cap`. **Unchanged:** Level 1 (`stats.verify.*`, and
`stats_reproduction`), the Stats Verifier's legacy lane and the reviewer
agent's supplementary files keep 5,000 rows, 128 columns, 10 MiB and 2 MiB of
cell text, and their profiles and limitations; `stats_desc_commands::open`
(the tests' wrapper), `load_dataset_from_bytes` and `DatasetSlots::default()`
keep `IngestLimits::METHODS`'s numbers.

**Not measured in 8F-2:** the 30 s run ceiling stays, and the 16 GB tier's
12,800,000-cell summary took 14.08 s on an 8 GB Mac (8F-1 above), so a slower
computer near that size could reach it; a text-heavy 64 MB file; a
categorical column of up to 100,000 levels in the profile; and two 64 MB files
in one project, which decision 107's per-project cap refuses. The real-app
check (`~/gaply-corpus/os/phase8/realapp_8f2.md`) reads the app's memory in
Activity Monitor; its figure is recorded when it has run.
