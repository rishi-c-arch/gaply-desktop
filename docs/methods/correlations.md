# Correlations, as an analysis

Capability `stats.analyze.correlations`, version 1, **Beta**, in the
Statistics workspace (Research OS, Phase 8H and 8H-2; decisions 334 to 361 of
1 Nov 2026; D307). This page is the capability's `documentation_id`
(`methods/correlations`).

It computes, between two or more numeric columns of the user's own data, the
coefficients the user ticks: Pearson's r, Spearman's rho and Kendall's tau-b,
each pair with its n, test statistic, two-tailed p and 95% interval, and each
variable's n, mean and SD over the rows used. It is **not** a reproduction of
a reported result; those are Level 1's `stats.verify.pearson` and
`stats.verify.spearman`, which this capability does not change. The pure core
is `gaply_core::stats_corr`; its façade is `stats_adapters::corr_admit` /
`corr_validate` / `corr_run`. The app reaches it through the six
`stats_corr_*` commands and the run manager's correlations kind; the screen is
at `/app/statistics/correlations`, behind `REACT_APP_FEATURE_STATS_DESCRIBE`,
which hides unfinished UI and is not an authorization boundary. Planned in 8H
(decision 344), Beta since 8H-2's Part 1 (decision 351).

`ENGINE_VERSION` is 1, and stays 1 through 8H-2's change to Pearson's rule at
±1 (decision 352, §4): the method was Planned until 8H-2, so no result of it
was ever saved or run in a released build.

## 1. Inputs and the spec

`CorrParams { variables, coefficients, missing, missing_codes }`:

- `variables`: two or more numeric columns by Level 1's alias (`C1`, `C2`, …),
  in the file's column order.
- `coefficients`: the ones the user ticks, of `pearson`, `spearman` and
  `kendall`, in that order; **none by default**. Gaply says which coefficients
  the user chose, never which they should use.
- `missing`: `pairwise` or `listwise` (§3); **no default**.
- `missing_codes`: values to treat as missing in every chosen variable,
  ascending.

**One choice, one spec, one hash (decision 338).** A spec with a variable, a
coefficient or a code repeated, or out of its order, is refused with a closed
code (`variables_out_of_order`, `coefficients_out_of_order`,
`missing_codes_out_of_order`, and the `_repeated` codes), never sorted
silently; this departs on purpose from the ANOVA, whose check sorts its
levels. The screen builds the spec in column, coefficient and ascending code
order, so whatever order the user ticks in, one choice sends one spec
(`corrTypes.vitest.ts`). The manifest hashes the parameters.

A chosen column that holds text is refused (`variable_holds_text`), one with
NaN or an infinity too (`variable_non_finite`). A candidate missing code that
the suspect-code rule (`stats_l1::binding::suspect_codes_in`) finds in a
chosen column, and that the user did not list, is a warning
(`suspect_missing_code`). The screen offers, as a question, the union of the
ticked columns' candidate codes, ascending; a ticked code applies to every
chosen column.

## 2. Formulas, and where each number comes from

One formula, one place (decisions 62, 275 and 339):

| number | from | source |
|---|---|---|
| Pearson's r and t | `stats_verify::pearson_clamped`: `r_to_result` of `pearson_r` clamped to [−1, 1], t = r √((n − 2)/(1 − r²)) | K. Pearson; R's `cor.test` |
| Spearman's rho and t | `stats_verify::spearman`: average ranks, then Pearson on the ranks; t as Pearson's | C. Spearman; R's `cor(method = "spearman")` |
| Kendall's tau-b | S / √((n0 − n1)(n0 − n2)), written once in `stats_corr` | M. G. Kendall, *Rank Correlation Methods*, 4th ed., Griffin, 1970 |
| Kendall's S | counted in O(n log n): sort by x (ties by y), count the pairs tied in x, in y and in both, and the discordant pairs as the exchanges of a merge sort of y | W. R. Knight, "A computer method for calculating Kendall's tau with ungrouped data", JASA 61(314), 436–439, 1966, doi:10.1080/01621459.1966.10480879 (written from the description; no code copied) |
| Kendall's z | z = S / √var(S), var(S) = (v0 − vt − vu)/18 + v1/(2n(n − 1)) + v2/(9n(n − 1)(n − 2)), no continuity correction | Kendall (1970), ch. 4 |
| p of t | `stats_dist::t_two_tailed(t, n − 2)`; `stats_verify`'s own p is never read | [large_samples.md](large_samples.md) |
| p of z | `stats_dist::normal_two_tailed(z)` = erfc(\|z\|/√2), statrs's `erfc` | added in 8H, additive |
| intervals | Fisher's z: tanh(atanh(c) ± 1.959964… · SE), the quantile `stats_dist::normal_quantile_975()` | R. A. Fisher, 1921 |
| Pearson SE | 1/√(n − 3) | Fisher; R's `cor.test` |
| Spearman SE | √((1 + rho²/2)/(n − 3)) | D. G. Bonett and T. A. Wright, Psychometrika 65(1), 23–28, 2000, doi:10.1007/BF02294183 |
| Kendall SE | √(0.437/(n − 4)) | E. C. Fieller, H. O. Hartley and E. S. Pearson, Biometrika 44(3/4), 470–481, 1957, doi:10.1093/biomet/44.3-4.470 |
| mean, SD | `stats_verify::mean` and `sample_variance` on the values ascending | as `stats_anova` |

In Kendall's variance, n0 = n(n − 1)/2; n1 and n2 sum t(t − 1)/2 over x's and
y's tie groups; v0 = n(n − 1)(2n + 5); vt and vu sum t(t − 1)(2t + 5) over each
variable's tie groups; v1 = Σ t(t − 1) · Σ u(u − 1); v2 = Σ t(t − 1)(t − 2) ·
Σ u(u − 1)(u − 2). S is a signed i128 and every count and sum is an exact
integer (u128), each converted to f64 once.

Spearman's p uses the t approximation only; no exact p is offered for small n.

## 3. Pairwise and listwise: which rows each uses

A **value** is a number that is not a ticked missing code.

- **Listwise**: a row is used when every chosen variable has a value, and
  every pair uses those rows, so every pair has the same n. A row left out is
  counted once, under the first chosen variable in the column order without a
  value: `missing_value` (an empty cell or a missing-value token) or
  `missing_code`, that column its parameter. Refused before any run with
  fewer than 3 complete rows (`too_few_complete_rows`).
- **Pairwise**: each pair uses every row in which both of its variables have a
  value, so pairs can have different n. A row is used when at least two chosen
  variables have one; a row with fewer is left out as
  `fewer_than_two_values`, which names no column. Refused before any run when
  no pair has 3 rows (`no_pair_of_three`).

Either way, rows used plus the exclusions' counts equal the rows read, as every
method's manifest says. The descriptives are over the rows used: listwise the
complete rows, pairwise each variable's own values in the rows used.

## 4. Edge cases, and the perfect rules

Decided from the values before each call, never from an error's text, each
with a closed code and its sentence where the number would be:

- a pair with fewer than 3 rows (pairwise only): `pair_too_few_rows`, every
  statistic of the pair;
- a variable constant over the pair's rows: `variable_constant`, that pair's
  three coefficients;
- an overflow or underflow `stats_verify` refuses with (values near 1e200,
  spreads near the smallest double): `overflow` or `underflow`, by
  `RangeFault::of`, that coefficient of that pair;
- the interval only: n of 3 or fewer for Pearson and Spearman, 4 or fewer for
  Kendall (`interval_too_few_rows`);
- the descriptives: the mean of a variable with no value in the rows used
  (`no_values`), the SD of one with fewer than 2 (`too_few_values`).

**A perfect coefficient is ±1 exactly**, by one predicate per coefficient:

- **Pearson** (decision 352, 8H-2): `stats_verify`'s own boundary, an infinite
  t, on r clamped to [−1, 1]; **or** 1 − |r| ≤ `RHO_ONE_ULPS` · ε / 2, Level
  1's test for rho (`stats_l1/compute.rs`, `RHO_ONE_ULPS = 4`, made
  `pub(crate)` by name in 8H-2, visibility only, so Level 1's rule stays in
  one place and no Level 1 result changes). On an exact line `pearson_r` can
  give 1 − 2 ulp (x = 1…5, y = 2x + 1: 0.9999999999999998, a finite t of
  about 8.2e7 before 8H-2), where R's `cor` gives exactly 1 and an infinite t.
- **Spearman**: from the values, before any call: the two variables put the
  pair's rows in the same order with the same ties, or in exactly the reverse
  order. `stats_verify::spearman` can return 1 − 2 ulp there.
- **Kendall**: in integers, |S| = n0 − n1 = n0 − n2.

A perfect Pearson or Spearman coefficient's t is infinite: the result holds
the closed code `perfect_coefficient` in its place, never an f64 infinity
(serde would write it as null), and its p is 0. Kendall's z stays finite, and
its p always comes from z. A perfect coefficient's interval is [±1, ±1],
tanh's limit, as R's `cor.test` gives it for Pearson, unless the n rule above
makes it not computable first.

## 5. Precision, the gates, and the oracles

Every gate is ten times the largest error measured (decision 341), asserted on
every platform (decision 277). Relative unless marked; a p is gated
relatively where both values are at least `f64::MIN_POSITIVE`, and below that
both must be below it.

**Against R 4.6.1** (`tests/corr_r_golden.rs`, the descriptor's validation
suite; 23 cases, 2,621 expected values, 2, 3, 5 and 10 variables, n of 3, 4,
5, 10 and 30 and a 100,000-row pair; measured 1 Nov 2026, macOS aarch64):

| family | gate | measured | at |
|---|---|---|---|
| `normal_two_tailed` vs `2*pnorm(-abs(z))`, p down to 9.2e-308 | 1e-9 | 9.958e-11 | z 0.75 (statrs `erfc`) |
| descriptives' mean | 2e-13 | 1.66e-14 | the 1e9-offset case |
| descriptives' SD | 3e-13 | 2.12e-14 | the 1e9-offset case |
| coefficient | 9e-15 | 8.34e-16 | 10 variables, n 30 |
| coefficient at ±1 (absolute) | 0 | 0 | every one of the 36 perfect coefficients (4 Pearson, 16 Spearman, 16 Kendall) |
| t | 2e-9 | 1.12e-10 | \|r\| 0.999999 |
| z | 3e-9 | 2.20e-10 | the 100,000-row pair |
| p | 4e-8 | 3.13e-9 | p 2.2e-81 |
| intervals | 7e-13 | 6.52e-14 | Likert, Spearman |

No golden row other than the 36 perfect ones lies within `RHO_ONE_ULPS` of ±1
(the closest is 9.9e-7 from 1), so decision 352's rule changed no golden row
(checked 1 Nov 2026).

**The oracles** (ADR-008, for fixtures only, nothing copied), in the
generator `tests/fixtures/stats_corr/make_corr_fixtures.R`, run as
`/usr/local/bin/Rscript`, which subsets each pair's rows itself so R's `use=`
rules never decide them:

- `cor` and `cor.test(method = "pearson")`; R's `cor` gives exactly 1 on the
  data where `pearson_r` gives 1 + 1 ulp;
- `cor.test(method = "spearman", exact = FALSE)`, whose statistic is S, not t:
  Spearman's t and p are computed in the generator with `pt`, **the same
  formula written twice**;
- `cor.test(method = "kendall", exact = FALSE, continuity = FALSE)`; on the
  Likert items the generator's z from §2's variance equals `cor.test`'s within
  3.3e-16, which shows the tie correction numerically. `?cor` states that its
  Kendall coefficient is tau-b (read 1 Nov 2026);
- the Spearman and Kendall intervals have no base R function: the generator
  computes them by §2's formulas, the same formula written twice. confintr's
  `ci_cor` was installed and is not used: its documentation names bootstrap
  intervals only for Kendall and Spearman.

Two places where R itself is not the reference, both recorded in the
generator's `measurements.txt`: on a perfect rank pair of n 5 R's `cor.test`
gives rho and tau 1 − 2 ulp, so the generator writes the exact ±1; and at
100,000 rows `cor.test`'s Kendall z is 1.8e-7 from an exact-fraction value,
so for that one pair the generator writes §2's z with an exact S.

## 6. Speed, and the budget

There is no cap on variables: a cap would refuse the common thesis case (40 to
60 items at a few hundred rows) to protect a rare one. The budget is on work
(decision 342):

- estimated seconds = Σ over the chosen coefficients of pairs × rows read ×
  that coefficient's worst measured seconds per pair-row;
- a spec whose estimate is above **15 s** (`BUDGET_SECONDS`) is refused by
  validation, before any run, with `over_budget`, whose parameters are the
  pairs and the rows; its sentence says to choose fewer variables or fewer
  coefficients. The run manager's `begin` validates first, so a run over the
  budget ends `refused` without reaching a check point
  (`src-tauri/tests/stats_corr_commands.rs`);
- the rates live in one place, `stats_corr::seconds_per_pair_row`, measured in
  release on this Mac (`tests/corr_speed.rs`, `GAPLY_CORR_SPEED=1`, three runs
  each, pairwise with every variable missing rows the others have):

| coefficient | seconds per pair-row | at |
|---|---|---|
| Pearson | 1.308e-8 | 100,000 rows, k 10 |
| Spearman | 1.481e-7 | 100,000 rows, k 30 |
| Kendall | 1.417e-7 | 100,000 rows, k 30 |

| rows | k | Pearson | Spearman | Kendall | all three |
|---|---|---|---|---|---|
| 100,000 | 10 | 0.06 s | 0.61 s | 0.56 s | 1.16 s |
| 100,000 | 20 | 0.20 s | 2.77 s | 2.63 s | 5.44 s |
| 100,000 | 30 | 0.44 s | 6.44 s | 6.16 s | 12.85 s |
| 10,000 | 30 | 0.04 s | 0.53 s | 0.53 s | 1.08 s |
| 10,000 | 60 | 0.17 s | 2.43 s | 2.34 s | 5.33 s |
| 1,000 | 128 | 0.08 s | 0.83 s | 0.77 s | 1.67 s |

So 30 variables fit at 100,000 rows with all three (estimated 13.18 s), 31 is
the most, and 128 (the ingest's widest file) fit at 1,000 rows (estimated
2.46 s). On a slower computer the 30 s run ceiling still stops a run. The
arithmetic is single-threaded, so a rerun is byte-identical.

**Check points** (decisions 342 and 355): one before the arithmetic and one
before each pair of each chosen coefficient, `1 + c · k(k − 1)/2`, from the
spec alone; the record's total is `stats_corr::check_points`, and the screen
says "{done} of {total} steps". Tested through IPC: every check point reaches
the hook in order; a cancel at Spearman's fourth pair ends the run Cancelled
with no result; a check point that "sleeps" 31 s on an injected clock ends it
TimedOut.

**The screen**: for 20 variables or fewer, a lower triangle of the shown
coefficient above its pairs table; above 1,225 pairs, the table in pages of
1,225 rows with Next and Previous and a line saying which pairs are shown.
Measured in jsdom (`CorrFlow.vitest.tsx`, 1 Nov 2026): 50 variables (1,225
pairs) 303 ms; 128 variables (8,128 pairs) 253 ms for the first page and 11 ms
for the last.

## 7. The export

`stats_corr::export::result_csv(result, manifest)` and `manifest_json`
(decision 354): one long CSV, one row per number, CRLF line ends, in the
ANOVA's eight fields, named `table, statistic, first_index, first,
second_index, second, value, word`, in the screen's order: the confidence
level, the rows read and used, the descriptives (n, mean, SD), each chosen
coefficient's pairs (n, the coefficient as `r`, `rho` or `tau_b`, `t` and `df`
or `z`, `p`, `ci_lower`, `ci_upper`), the rows left out, the limitations, and
the manifest's input, parameters and output hashes. Every digit (Rust's
shortest round-trip form); a value not computable is empty with its code in
`word`; an infinite t is the word `stats.corr.perfect_coefficient`, never a
number.

**Columns are named by alias** (`C1`, `C2`, …) and their index among the
chosen variables, never by header, as a stored result names them, so the
same run, live or rerun from a project, exports the same bytes. The
descriptive summary's field writer (`stats_desc::export::text`) gives the one
formula-injection rule.

Pinned by `tests/corr_export.rs`: field-by-field read-back, the word for a
perfect t, aliases only (sentinel headers, with positive controls), ten runs
byte-identical, and a golden CSV and manifest for one listwise case with all
three coefficients and a ticked code, a perfect pair, pairs not computable
and Kendall's intervals of too few rows. The golden's bytes are macOS
aarch64's; every other platform, Linux x86_64 included, checks its layout
(decision 346's check, in `tests/pin_support`), each number within 4 units in
the last place, until a later step pins Linux's bytes from a Linux run.

## 8. What SPSS documents, and where it differs

IBM SPSS Statistics 29 Algorithms (CORRELATIONS and NONPAR CORR), read 1 Nov
2026 as documentation, never a gate:
https://www.ibm.com/docs/en/SSLVMB_29.0.0/pdf/IBM_SPSS_Statistics_Algorithms.pdf

| statistic | SPSS, as documented | Gaply |
|---|---|---|
| Pearson p | t with N − 2 df, two-tailed by default | the same |
| Pearson interval | Fisher z, SE 1/√(W − 3), optionally a bias term r/(2(W − 1)) | the same without the bias term |
| Spearman p | t with N − 2 df | the same |
| Spearman interval | three options: Fieller 1.06/(W − 3), Caruso–Cliff, Bonett–Wright | Bonett–Wright, one of SPSS's three; SPSS's default is not stated in the text read |
| Kendall tau | tau-b | the same |
| Kendall p | z = S/√var(S), "(Kendall, 1955)" | Kendall's (1970) tie-corrected var(S); SPSS's variance is an image in the PDF and its tie terms were not confirmed |
| Kendall interval | Fisher z, SE √(0.437/(W − 4)) | the same |
| at ±1 | no interval (ζ not allowed at ±1) | [±1, ±1] |
| weights | frequency weights | none |

## 9. What is not offered, and the limitations

Every result carries `pearson_assumes_normality` and `two_tailed_no_partial`,
and the descriptor lists the matching wording codes:

- Pearson's p and interval assume the two columns are roughly normal together;
  Gaply does not check, and offers no normality check;
- p values are two-tailed only: no one-tailed p;
- no partial correlations;
- Spearman's p by the t approximation only, with no exact p;
- no weights;
- the coefficients describe association, never cause; the wording is neutral:
  no "significant", no stars, no verdict (decision 66);
- how large a file opens depends on this computer's memory, as for the other
  methods ([large_samples.md](large_samples.md) §8): 100,000 data rows and 128
  columns on every computer.

## 10. Blueprint §18.2 acceptance

| item | evidence | status |
|---|---|---|
| Formula/method and variant documented | §2, §4; `stats_corr/mod.rs` header | done |
| Golden fixtures and edge cases pass | `corr_r_golden` (23 cases), `corr_cases`, `corr_export` | done |
| Missing values, ties, weights, precision and sample size handled | §3, §4, §5, §6; weights not supported, stated | done |
| Diagnostic meanings and limitations documented | §4, §9; `CorrCode` docs; a sentence for each of the 57 codes the screen can show (`corrWording.ts`, `corrCodes.json` pinned to Rust by `stats_corr_wire.rs`, `corrWording.vitest.ts`) | done |
| Independent reference comparison completed | R, §5 | done |
| Unsupported designs return explicit reasons | §1, §3, §6; each refusal's code through IPC (`stats_corr_commands.rs`) | done |
| Result manifest and export verified | §7; through IPC the CSV equals the module's export of the run's result and manifest; the privacy matrix, which forbids every header in the result, the CSV and the manifest (`stats_corr_privacy_matrix.rs`); a saved result reruns identical, byte for byte (`project_corr.rs`) | done |
