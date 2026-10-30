# Large samples: the methods' distribution functions

**Status:** shipped in Research OS Phase 8E (decision 275, D301), 30 Oct 2026,
in `gaply_core::stats_dist` (`src-tauri/gaply-core/src/stats_dist/mod.rs`).
The three t-tests on the user's data ([independent_t.md](independent_t.md),
[one_sample_t.md](one_sample_t.md), [paired_t.md](paired_t.md)) take every p
and quantile from it; the one-way ANOVA, the correlations and the regression
will (decision 275). Level 0 and Level 1 keep `stats_l0::pvalue` as it is.

## 1. Why

Before 8E the methods took every p from `stats_l0::pvalue::t_two_tailed` and
the interval's quantile from a bisection over it, and refused a run above
df 1e4 with `quantile_out_of_range` (decision 242.7). The 8A proposal's §4
measured that engine where larger files would take it, against R 4.6.1:

| function | df range | largest relative error | gate | held |
|---|---|---|---|---|
| t quantile 0.975, the bisection | ≤ 4,998 | 2.9e-13 | 1e-12 | yes |
| | 1e4 | 5.17e-13 | 1e-12 | yes |
| | 1.5e4 | **1.01e-12** | 1e-12 | **no** |
| | 2e4 | 2.48e-13 | 1e-12 | yes |
| | 3e4 | **3.38e-12** | 1e-12 | **no** |
| | 5e4 | 4.73e-13 | 1e-12 | yes |
| | 1e5, 1e6, 1e7 | 1.76e-11, 8.78e-11, 3.58e-9 | 1e-12 | no |
| `f_upper`, df1 = 1 | df2 5e3 | 1.97e-11 | 1e-9 | yes |
| | df2 5e4, 5e5, 1e6 | **1.77e-9, 1.45e-9, 1.71e-8** | 1e-9 | **no** |
| `f_upper`, df1 2, 9, 10, 50 | df2 5e3 to 1e6 | 3.3e-10, 8.8e-10, 7.9e-10, 3.4e-10 | 1e-9 | yes |
| `t_two_tailed` | to 1e5 | within 1e-9 | 1e-9 | yes |
| | 2e5; 1e7 | 1.4e-9; 1.1e-7 | 1e-9 | no |

The quantile's error is not monotone above 1e4 (over the gate at 1.5e4 and
3e4, under it at 2e4 and 5e4), so no higher limit for the bisection was clean;
and 8F lets larger files in. Hence 8E: a published formula for each function,
measured against R to df 1e7, switched on above 1e4 so that no number at or
below 1e4 changes by a bit.

## 2. The four functions and their sources

Each is implemented from its published text, never from R's source
(ADR-008; Prompt 0). Every coefficient was read from the source named and is
decision 275's.

| function | at df ≤ 1e4 (`LARGE_DF`) | above 1e4, to 1e7 (`MEASURED_MAX_DF`) |
|---|---|---|
| `t_two_tailed(t, df)` | `stats_l0::pvalue::t_two_tailed`, bit for bit | Hill's formula |
| `t_quantile_975(df)` | the 4B-1 bisection over `stats_l0`'s p, moved verbatim | `cornish_fisher_975(df)`, Abramowitz and Stegun 26.7.5 |
| `f_upper(f, df1, df2)` | `stats_l0::pvalue::f_upper`, bit for bit | df1 = 1: Hill's formula on f/df2 (F(1, d) is the square of t with d df); df1 ≥ 2: `stats_l0`'s, ungated |
| `normal_quantile_975()` | statrs 0.18.0's `Normal(0, 1).inverse_cdf(0.975)` | the same |

One private predicate, `df > LARGE_DF`, decides the switch for the p, the
quantile and F alike. Above `MEASURED_MAX_DF` every function that takes a df
returns `None`, and the methods refuse with `quantile_out_of_range`.

**Hill's two-tailed p.** G. W. Hill, "Algorithm 395: Student's
t-distribution", Communications of the ACM 13(10), 617–619, 1970,
<https://doi.org/10.1145/355598.362775>. The ACM page answered HTTP 403 to
this step on 30 Oct 2026; the algorithm's large-df branch was read in Alan
Miller's Fortran 90 transcription of Algorithm 395,
<https://jblevins.org/mirror/amiller/cacm395.f90> (read 30 Oct 2026), which
carries the same coefficients as decision 275. With a = df − 0.5, b = 48a²
and y = a · ln(1 + t²/df):

    z = (((((−0.4y − 3.3)y − 24)y − 85.5) / (0.8y² + 100 + b) + y + 3) / b + 1) · √y
    p = 2 · (1 − Φ(z)) = erfc(z / √2)        (statrs's erfc)

`ln_1p` takes the place of Hill's small-y branch, which took log(1 + y) only
above 1e-6 because log(1 + y) loses digits there. A y of 1,500 or more gives
p = 0 (z ≈ 38.7, where the normal tail is below the smallest double), so an
overflowing t² gives 0, never NaN; p(−t) is p(t) bit for bit since only t²
enters.

**The quantile above 1e4.** Abramowitz and Stegun, Handbook of Mathematical
Functions (1964), formula 26.7.5, p. 949, read as the page's scan at
<https://personal.math.ubc.ca/~cbm/aands/page_949.htm> on 30 Oct 2026. With
x = `normal_quantile_975()`:

    t = x + g1/df + g2/df² + g3/df³ + g4/df⁴
    g1 = (x³ + x)/4
    g2 = (5x⁵ + 16x³ + 3x)/96
    g3 = (3x⁷ + 19x⁵ + 17x³ − 15x)/384
    g4 = (79x⁹ + 776x⁷ + 1482x⁵ − 1920x³ − 945x)/92160

`cornish_fisher_975` is public and answers for df from 1e3 to 1e7, so the test
checks the coefficients at df 1e3, 2,500 and 4,998, where the last terms are
largest; the methods reach it only above 1e4, through `t_quantile_975`.

**The bisection at df ≤ 1e4** is 4B-1's (decision 62's fallback: statrs's
`StudentsT::inverse_cdf` missed R's `qt` by 3.5e-6 at df 1e5), moved from
`stats_ttest` verbatim. Its target is `2.0 * (1.0 - UPPER_TAIL_POINT)` with
`UPPER_TAIL_POINT = 0.975`, which is 0.050000000000000044 and not the double
0.05: writing 0.05 would change bits, and `tests/ttest_quantile.rs` pins all
30 of them (`the_quantile_keeps_its_bits_to_df_1e4`), on every platform. The
reviewer computed the same 30 on Linux x86_64 from the same code; all 30 are
equal to this Mac's.

**The normal 0.975 quantile** is statrs 0.18.0's `Normal::new(0.0, 1.0)`
(which cannot fail; its `expect` says why) `.inverse_cdf(0.975)`. 8H's
correlations use it.

## 3. What stays with `stats_l0`, and why

* `stats_l0::pvalue` is Level 0's and Level 1's engine, and nothing under
  `src/stats_l0` changed. `stats_dist` names its two functions under aliases
  (`l0_t_two_tailed`, `l0_f_upper`), because its own functions have the same
  names, and returns their values bit for bit wherever 8A measured them within
  their gate: every t p and quantile at df ≤ 1e4, F with df1 = 1 at df2 ≤ 1e4,
  and F with df1 ≥ 2 at every df2 to 1e7 (8A measured df1 2, 9, 10 and 50
  within 1e-9 to df2 1e6; 8G measures the range its ANOVA reaches).
* Level 0's Could not check above df 1e5 (decision 90) stays. Level 0 could
  take Hill's p above 1e5 in a verifier step of its own; 8E does not.
* **The range.** Above `MEASURED_MAX_DF` (1e7) every function returns `None`,
  and the t-tests refuse with `quantile_out_of_range`, the code a df above 1e4
  carried from 4B-1 to 8E. The sentence on the screens:
  "The test needs the t distribution at more than 10,000,000 degrees of
  freedom, where Gaply has not measured its accuracy, so nothing was
  computed." It cannot be reached through a dataset: a df above 1e7 needs more
  than 10,000,002 rows, and the methods open at most 100,000 data rows on every
  computer (5,000 before 8F-2). So `stats_ttest`'s unit test pins the check
  directly.

## 4. Measured against R 4.6.1 on this Mac (macOS aarch64)

`tests/stats_dist_reference.rs` over `tests/fixtures/stats_dist/` (written by
`make_dist_fixtures.R` with `/usr/local/bin/Rscript`, R 4.6.1, 30 Oct 2026; a
statistic placed at a target p through R's quantile function and R's p
computed from it, as 8A's references were). Run 30 Oct 2026; every row is
printed by `cargo test -p gaply_core --test stats_dist_reference -- --nocapture`.

**The quantile** (20 df; gate 1e-12 for `t_quantile_975`, 1e-14 for the
expansion itself):

| df | `t_quantile_975` route | \|gaply − R\| / R | the expansion alone |
|---|---|---|---|
| 1,000 | bisection | 1.4e-14 | 0 (equal) |
| 2,500 | bisection | 1.7e-13 | 3.4e-16 |
| 4,998 | bisection | 2.9e-13 | 2.3e-16 |
| 10,000 | bisection (the pinned bits) | 5.2e-13 | 2.3e-16 |
| 10,000.5, 10,001, 10,002, 12,345.6 | expansion | 2.3e-16, 3.4e-16, 4.5e-16, 1.1e-16 | the same |
| 1.5e4, 2e4, 3e4, 5e4 | expansion | 3.4e-16, 3.4e-16, 1.1e-16, 1.1e-16 | the same |
| 99,998, 1e5, 2e5, 5e5 | expansion | 3.4e-16, 2.3e-16, 1.1e-16, 3.4e-16 | the same |
| 1e6, 2e6, 5e6, 1e7 | expansion | 2.3e-16, 3.4e-16, 1.1e-16, 2.3e-16 | the same |

Largest: 5.171e-13 at df 1e4 (the bisection); the expansion's largest is
4.531e-16 at df 10,002. The quantile does not increase over the 20 df, across
the switch. At every df ≥ 1e4, `t_two_tailed` at Gaply's own quantile is 0.05
within 4.5e-11 (largest at df 5e5), and at R's quantile within the same gate.

**The normal quantile:** `normal_quantile_975()` is 1.95996398454005383; R's
`qnorm(0.975)` is 1.95996398454005338; relative 2.266e-16, **2 ulps**, inside
the 1e-15 gate.

**The two-tailed t p** (338 rows: 13 df, 17 fixed |t| from 0 to 30 and 9 |t|
placed at two-sided p from 0.05 down to 1e-300; gate 1e-9 · p). The largest
|gaply − R| / p per df, with `stats_l0`'s own error on the same rows, printed
ungated above 1e4:

| df | `stats_dist` | at t | `stats_l0::pvalue` | at t |
|---|---|---|---|---|
| 10,000 (stats_l0's route, bit for bit) | 3.31e-11 | 0.001 | 3.31e-11 | 0.001 |
| 10,000.5 | 8.57e-11 | 1 | 4.80e-11 | 0.001 |
| 10,001 | 8.57e-11 | 1 | 6.10e-11 | 0.001 |
| 15,000 | 8.57e-11 | 1 | 3.71e-11 | 1.5 |
| 20,000 | 8.57e-11 | 1 | 3.31e-11 | 0.001 |
| 50,000 | 8.57e-11 | 1 | 3.30e-11 | 0.001 |
| 99,998 | 8.57e-11 | 1 | 3.52e-9 | 0.001 |
| 1e5 | 8.57e-11 | 1 | 4.40e-9 | 0.001 |
| 2e5 | 8.57e-11 | 1 | 1.45e-9 | 1.5 |
| 5e5 | 8.57e-11 | 1 | 1.33e-8 | 0.001 |
| 1e6 | 8.57e-11 | 1 | 8.83e-9 | 0.001 |
| 5e6 | 8.57e-11 | 1 | 1.24e-7 | 0.001 |
| 1e7 | 8.57e-11 | 1 | 1.24e-7 | 0.001 |

The 8.57e-11 is the same at every df above 1e4 and always at t = 1: it is
statrs's `erfc` at z ≈ 1, not a property of the df (the reviewer's limit, §5).
`stats_l0` is over its 1e-9 gate from df 99,998 on this grid, at the smallest
|t|, where its `sf` is 1 minus a number near 1 (8A: 1.4e-9 at 2e5, 1.1e-7 at
1e7). Properties, at df 10,001, 1e5 and 1e7: p(0) = 1, p(−t) = p(t) bit for
bit, p in [0, 1] and not increasing over 201 |t| from 0 to 40; p(1e200, 1e5)
= 0.

**F with df1 = 1** (144 rows: 16 df2, F = 0 and 8 F placed at upper p from
0.5 down to 1e-300; gate 1e-9 · p):

| df2 | `stats_dist` | at F | `stats_l0::pvalue` | at F |
|---|---|---|---|---|
| 5,000 | 2.73e-12 | 0.455 | the same (bit for bit) | |
| 7,500 | 1.19e-12 | 1,508 | the same | |
| 9,999 | 6.47e-12 | 0.455 | the same | |
| 10,000 | 2.41e-12 | 10.83 | the same | |
| 10,001 | 8.21e-11 | 10.83 | 4.83e-12 | 464 |
| 15,000 | 8.21e-11 | 10.83 | 6.26e-12 | 23.9 |
| 20,000 | 8.21e-11 | 10.83 | 3.04e-12 | 3.84 |
| 30,000 | 8.21e-11 | 10.83 | 1.58e-11 | 225 |
| 50,000 | 8.21e-11 | 10.83 | 4.67e-12 | 225 |
| 75,000 | 8.21e-11 | 10.83 | 7.13e-11 | 1,387 |
| 99,998 | 8.21e-11 | 10.83 | 8.08e-12 | 1,383 |
| 1e5 | 8.21e-11 | 10.83 | 8.37e-11 | 1,383 |
| 2e5 | 8.21e-11 | 10.83 | 2.42e-10 | 0.455 |
| 5e5 | 8.21e-11 | 10.83 | 1.87e-10 | 1,374 |
| 1e6 | 8.21e-11 | 10.83 | 3.95e-10 | 1,374 |
| 1e7 | 8.21e-11 | 10.83 | 1.72e-8 | 3.84 |

8A's unmeasured stretch, df2 5e3 to 5e4, is within 1.6e-11 for `stats_l0` on
this grid, and 8A's 1.77e-9 at 5e4 was 1 of its 21 rows on another grid; the
switch stays at 1e4 with the t family. F = 0 gives 1 on both routes; a
negative, NaN or infinite F gives `None` on both.

**F with df1 = 2 and 3** (54 rows: df2 10,001, 1e5 and 1e6; `stats_l0`'s bits
on every row, its error against R printed ungated): df1 = 2: 5.6e-12,
6.3e-12, 5.9e-11; df1 = 3: 9.7e-12, 1.9e-11, 6.2e-10 (at df2 1e6; the reviewer
measured 6.1e-10 there on Linux). Inside 1e-9, and not gated by 8E.

**The goldens.** `ttest_r_golden` (34 computed pairs over 19 cases; the new
`large_df`, 5,100 per group, Student's df 10,198 and Welch's 10,194.4) and
`onesample_r_golden` (27 computed; `one_df_10001` and `paired_df_10001`
compute since 8E): every statistic inside its gate; the largest ratios are on
the three method pages. Every case at df ≤ 1e4 keeps its bytes
(`essentially_constant_bytes_pin.rs`: 32 and 25 lines, byte for byte the
lines of the run before 8E; the four cases above 1e4 are a second digest per
file, asserted on macOS aarch64 only, decision 277).

## 5. The reviewer's measurements against exact values (Linux x86_64)

Decision 275 records the reviewer's measurements, on Linux x86_64 with statrs
0.18 and mpmath at 40 digits, labelled as such and not re-measured here:

| function | range | largest relative error | note |
|---|---|---|---|
| Hill's p (`t_two_tailed` above 1e4) | df 1e4 to 1e9, p down to 1e-300 | 8.6e-11 | the limit is statrs's `erfc`, good to about 1e-10 |
| `stats_l0`'s t engine | df 1e5, 1e6, 1e7 | 1.7e-10, 8.7e-10, 3.6e-8 | and 1.1e-7 at 1e7 on the Mac (8A) |
| the expansion (`cornish_fisher_975`) | df 1e4 to 1e7; df 1e3 | 2.6e-16; 6.7e-16 | |
| a bisection over Hill's p | | 9.8e-12 | measured too; missed the 1e-12 gate, because of `erfc`, so not shipped |
| `normal_quantile_975` | | 2.3e-16 | 2 ulps below the nearest double; the same 2 ulps here against R |
| `f_upper`, df1 = 1 (Hill on f/df2) | | 7.1e-11 | |
| `stats_l0`'s `f_upper`, df1 = 3 | df2 1e6 | 6.1e-10 | 6.2e-10 here against R |

## 6. The tests, and what breaks them

* `tests/stats_dist_reference.rs`: the gates of §4, the provenance of each
  fixture, the range (`None` at a df just above 1e7, at ∞, NaN, 0 and −1; the
  expansion `None` just below 1e3; `None` for a t or F that is not a number;
  0 for t = 1e200; 1 for F = 0) and the properties.
* `tests/ttest_quantile.rs`: 33 df against `qt` within 1e-12 (none refused),
  the 30 pinned bits, and `None` just above 1e7.
* `stats_ttest::tests`: `two_sided_p` at df 1e7, just above it, at a NaN df
  and at a NaN t.
* Mutations recorded in 8E's report, each red then green after a
  SHA-256-checked restore: Hill's a = df − 0.5 made df; g3's −15x made +15x
  (red at the coefficient rows); the predicate df > 1e4 made df ≥ 1e4 (red at
  the df 1e4 rows); f_upper's df1 = 1 condition removed (red at the df1 = 2
  and 3 bit-for-bit check); the `None` above 1e7 removed from `t_two_tailed`
  (red at the range test); `two_sided_p`'s range check removed (red at its
  unit test); the bisection's target written as 0.05 (red at the 30-bit pin).

## 7. Platforms (decision 277)

The bisection's 30 bits, the t-test's canonical bytes, the essentially-constant
pins and Level 1's paired pin assert on every platform since 8E: the reviewer
ran each at `3d1253e` on Linux x86_64 with the assertion forced on and got
macOS's bytes; CI's Linux job checks them on every push, and the Windows job on
a pull request is Windows's measurement. The four golden cases above df 1e4 go
through `ln_1p` and statrs's `erfc`, which calls the platform's `exp`, so they
are a second digest per golden file. Since 8F-1 (decision 278) they too assert
on every platform: the reviewer ran them at `14d90c8` on Linux x86_64 with the
macOS gate forced off and got macOS's digests.

## 8. Limits by this computer's memory (8F-1 built them; 8F-2 switched them on)

8E makes the t family accurate to df 1e7, so the methods can take files above
5,000 rows. Phase 8F-1 built the limits that let them in, set by this
computer's total memory (decisions 279 and 281), and switched nothing on.
**Phase 8F-2 (D303, decision 288) switched them on** for the methods on the
user's data: the summary and the three t-tests open, and a project reruns,
through `IngestLimits::this_computer()`, and the open datasets take this
computer's budget. Each method's screen states the limits on its Import stage
before a file is chosen, and shows the rows read while a file opens. Level 1,
the Stats Verifier and the supplementary files keep their own limits (5,000
rows, 128 columns, 10 MiB, 2 MiB of cell text).

| | 8 GB | 16 GB | 32 GB |
|---|---|---|---|
| total memory | under 12 GiB, or unknown | 12 GiB to under 24 GiB | 24 GiB or more |
| data rows | 100,000 | 100,000 | 100,000 |
| columns | 128 | 128 | 128 |
| cells (data rows × columns) | 5,000,000 | 12,800,000 | 12,800,000 |
| file bytes | 64 MiB | 96 MiB | 96 MiB |
| cell text in all | 64 MiB | 96 MiB | 96 MiB |
| workbook inflated | 96 MiB | 256 MiB | 256 MiB |
| cells held across the open datasets | 10,000,000 | 51,200,000 | 51,200,000 |

- **A computer whose memory Gaply cannot read gets the 8 GB limits**, never
  "allow".
- **Cells** are the data rows kept times the columns kept (for a `.sav` or
  `.mat`, the variables kept). The cap is checked while the file is parsed: a
  CSV stops at the first row that takes it over; a workbook, `.sav` or `.mat`
  is refused before any cell is converted. Such a file is refused as
  `too_many_cells`: "The file has more than N cells (rows times columns), more
  than Gaply opens on this computer, so it was not opened."
- **A big-endian `.sav` is refused** (since 8F-3, D304), compressed or not, as
  `sav_big_endian`, before it is read: the reader takes its values as
  little-endian and gave wrong numbers with no warning.
- **The open datasets' budget counts cells only.** It does not count the
  file's bytes that each open dataset keeps for a project Save, nor the copy a
  rerun reads. An open drops the oldest open datasets until the cells held fit;
  it never drops the one just opened, and the four-dataset count still holds.
- **A rerun over this computer's limits** (its file's bytes, unpacked size,
  cell text or cells) is refused as `project.rerun_over_limits`: "This
  result's data file is larger than this computer's limits allow, so it cannot
  be run again here. It can be run again on a computer with more memory." Too
  many rows or columns stays `project.rerun_data_unreadable`: every tier has the
  same rows and columns.
- The 16 and 32 GB tiers are inferred, not measured on such a computer. The
  release measurement (ADR-010's 8F-1 addendum) is from an 8 GB Mac.
- **Not measured in 8F-2:** the run's 30 s ceiling stays, and the 16 GB tier's
  12,800,000-cell summary took 14.08 s on an 8 GB Mac, so a slower computer
  near that size could reach it; a text-heavy 64 MB file (an open dataset also
  keeps the file's bytes); a categorical column can now carry up to 100,000
  levels into the profile; and two 64 MB files cannot share one project,
  because decision 107's 100 MiB cap is per project.
