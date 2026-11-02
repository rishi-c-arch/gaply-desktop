# Cronbach's alpha, as an analysis

Capability `stats.analyze.cronbach_alpha`, version 1, **Beta**, in the
Statistics workspace (Research OS, Phase 8I and 8I-2; decisions 362 to 387 of
1 and 2 Nov 2026; D308). This page is the capability's `documentation_id`
(`methods/cronbach_alpha`).

It computes, for a scale of two or more numeric items in the user's own data,
over the rows in which every item has a value: Cronbach's alpha with a 95%
interval by Feldt's F; standardized alpha with the mean, smallest and largest
inter-item correlation; and for each item its mean, SD, the scale's mean and
variance if it were left out, its corrected item-total correlation and alpha
if it were left out. The items the user chooses are reverse-scored on the
scale's ends the user types. The pure core is `gaply_core::stats_alpha`; its
façade is `stats_adapters::alpha_admit` / `alpha_validate` / `alpha_run`. The
app reaches it through the six `stats_alpha_*` commands and the run manager's
Cronbach's alpha kind; the screen is at `/app/statistics/alpha`, behind
`REACT_APP_FEATURE_STATS_DESCRIBE`, which hides unfinished UI and is not an
authorization boundary. Planned in 8I (decision 372), Beta since 8I-2's Part 2
(decision 377).

`ENGINE_VERSION` is 1: the method was Planned until 8I-2, so no result of it
was ever saved or run in a released build (decision 377).

## 1. Inputs and the spec

`AlphaParams { items, reversed, scale, missingCodes }`, in JSON:

```json
{ "items": ["C2", "C3", "C5"], "reversed": ["C3"], "scale": { "min": 1, "max": 5 }, "missingCodes": [99] }
```

- **items**: two or more numeric columns, by alias, in the file's column
  order, none repeated. None is ticked by default; "Tick every numeric column"
  and "Untick all" are actions the user takes.
- **reversed**: the items to reverse-score, a subset of the items, in the same
  order. None by default; unticking an item unticks its box.
- **scale**: the lowest and highest values the scale allows, both finite, the
  lowest below the highest, given **only** when an item is reversed, and
  `null` otherwise. The screen's two fields are empty when they appear and are
  never filled in, from the data or anything else.
- **missingCodes**: the candidate missing codes the user ticked, ascending,
  once each; they apply to every item. The screen offers the union of the
  ticked items' candidate codes.

One choice gives one spec (decision 338): a spec out of order, with a repeat,
with a reversed item that is not an item, with ends and nothing reversed or
something reversed and no ends, is refused with its own closed code, never
corrected silently. A text column or one holding an infinity or NaN is
refused. A candidate code the user did not tick is a warning.

## 2. Formulas, and where each number comes from

With k items, n rows used, sᵢ² each item's sample variance and s²ₜ the
variance of the rows' totals:

- **Cronbach's alpha** = k/(k − 1) · (1 − Σ sᵢ² / s²ₜ) (L. J. Cronbach,
  "Coefficient alpha and the internal structure of tests", Psychometrika
  16(3), 297–334, 1951, https://doi.org/10.1007/BF02310555).
- **Feldt's 95% interval** = [1 − (1 − α) · F₀.₀₂₅, 1 − (1 − α) · F₀.₉₇₅],
  F_p the F whose upper-tail probability is p, with df1 = n − 1 and
  df2 = (n − 1)(k − 1) (L. S. Feldt, Psychometrika 30(3), 357–370, 1965,
  https://doi.org/10.1007/BF02289499; Feldt, Woodruff and Salih, Applied
  Psychological Measurement 11(1), 93–103, 1987,
  https://doi.org/10.1177/014662168701100107). The quantiles are
  `stats_dist::f_quantile_upper` ([large_samples.md](large_samples.md) §2).
- **Standardized alpha** = k · r̄ / (1 + (k − 1) · r̄), r̄ the mean of the
  k(k − 1)/2 inter-item Pearson correlations, shown with their smallest and
  largest.
- **Per item**: its mean and SD (after reversing); the scale's mean and
  variance if it were left out, which are the mean and variance of the total
  of the others; the corrected item-total correlation, Pearson's r of the item
  with the total of the others; and alpha if left out, alpha's formula on the
  other k − 1 items.

**One formula, one place (decisions 62, 275 and 368).** Means and variances
are `stats_verify::mean` and `sample_variance` on the values ascending; every
correlation is `stats_verify::pearson_clamped`'s; the others' total is the
row's total minus the item. Alpha, standardized alpha, Feldt's interval, the
item statistics and the reversal are written once, in `stats_alpha`.

## 3. Reverse-scoring, and the rows used

**The reversal is on the ends the user types, never the values seen.** A
reversed item's value x becomes min + max − x before any arithmetic. The
screen says: "Reverse-scoring turns each value x into lowest + highest − x.
Type the ends your questionnaire allows, for example 1 and 5, not the lowest
and highest answers in your data. Every reverse-scored item uses these ends."

**Its refusal:** a reversed item with a value outside [min, max] in any row
read, ticked codes removed, is refused before any run, one finding per column
with its count (`stats.alpha.reversed_outside_scale`): the ends are wrong, or
a missing code is not ticked. Nothing is clipped.

**Listwise.** A row is used when every item has a value: a number that is not
a ticked code. A row left out is counted once, under the first item in the
column order without a value, as `missing_value` (an empty cell or a
missing-value token) or `missing_code` (a ticked code), that column its
parameter. Rows read = rows used + the rows left out. Fewer than 3 rows used
is refused (`too_few_complete_rows`).

## 4. Not computable, and the rules at the edges

Each is decided from the values before the arithmetic, and shown as its
sentence where its number would be:

| what | when | code |
|---|---|---|
| alpha and its interval | the totals' variance is within rounding of 0 | `total_constant` |
| standardized alpha, r̄ and its range, that item's corrected item-total r | an item is the same number in every row used | `item_constant` |
| standardized alpha | 1 + (k − 1) · r̄ ≤ 0 | `standardized_not_defined` |
| that item's corrected item-total r and alpha if left out | the others' total is constant, by the same bound | `others_constant` |
| every alpha if left out | k = 2 | `two_items` |
| the interval | df1 above 99,999 or df2 above 1e7 | `distribution_out_of_range` |
| any | a sum, square, quotient or root a double cannot hold | `overflow`, `underflow`, `arithmetic_refused` |

- **The rounding bound (accepted in decision 374).** A total, or the others'
  total, is constant when its variance is at most (k · ε · M)², M the largest
  row sum of its summands' absolute values. Items that add to a constant (a
  and 10 − a) leave a variance near 1e-31, not 0, which the formulas would
  turn into an alpha of −1e31.
- **±1 (decision 352's test, accepted in decision 374).** Every correlation
  within `RHO_ONE_ULPS` = 4 units in the last place of ±1 is ±1, Level 1's
  test for rho. x and 6 − x give r = −1 exactly (R's `cor` gives −1 + 1 ulp),
  so 1 + (k − 1) · r̄ is 0 and standardized alpha is not computable, not
  −1.8e16.
- **A negative alpha** is computed and carries the note "Alpha is negative:
  the items' average covariance is negative. An item may need to be
  reverse-scored." It names no item.

## 5. Precision, the gates, and the oracles

Every gate is ten times the largest error measured (decision 370), asserted on
every platform. Measured 1 Nov 2026, macOS aarch64.

**Against base R 4.6.1** (`tests/alpha_r_golden.rs`, the descriptor's
validation suite; 17 cases, 15 computed and 2 refused, 712 values; k 2, 3, 5,
10 and 20; n 3, 10, 30, 200 and 100,000 by formula):

| family | gate | measured |
|---|---|---|
| alpha | 2e-12 | 1.568e-13 (100,000 rows) |
| Feldt's interval | 5e-12 | 4.287e-13 |
| standardized alpha | 3e-14 | 2.725e-15 |
| inter-item r | 3e-15 | 2.205e-16 |
| r near ±1 (absolute) | 0 | 0 |
| item mean | 3e-13 | — |
| item SD | 2e-13 | — |
| scale mean if deleted | 3e-13 | — |
| scale variance if deleted | 2e-13 | — |
| corrected item-total r | 4e-15 | 3.687e-16 |
| alpha if deleted | 4e-13 | 3.3e-14 |

A dash: 8I's report gives that family's gate, ten times its measurement, but
not the measurement itself.

**Against psych 2.6.9** (CRAN, installed for fixtures only; 211 values): alpha
3e-8, interval 6e-8, corrected item-total r 5e-8, alpha if deleted 7e-8. Every
worst case is the 1e9-offset data, where psych loses about 5e-9 that base R
and Gaply keep. psych refuses the constant item with an error, so that case
is checked against base R alone (decision 374).

**The F quantile** against an R `uniroot` oracle (tol 1e-15), 213 rows,
relative error in F: 2e-9 where Feldt's interval reaches (df2 ≤ 127 · df1),
worst 1.107e-10 at p 0.5, df1 99,999, df2 1e7, and 2.99e-11 at Feldt's two
points; 2e-7 elsewhere, worst 1.887e-8 at df1 2, df2 1e7. `qf` is never the
oracle above df2 4e5: beside the oracle it is off by up to 4.76e-4 (p 0.025,
df1 99,999, df2 899,991), recorded in the fixtures' `measurements.txt`.

**The oracles** (ADR-008, nothing copied): the generator
`tests/fixtures/stats_alpha/make_alpha_fixtures.R`, run as
`/usr/local/bin/Rscript`, writes base R's alpha, `cor`, `var` and `sd` with the
same reversal, rows, rounding bound and ±1 rule (**the same rules written
twice**), and psych's `alpha()` beside them.

## 6. The interval's range, and the speed

- **df1 = n − 1 never passes 99,999**: every computer opens 100,000 data rows
  ([large_samples.md](large_samples.md) §8).
- **df2 = (n − 1)(k − 1) passes 1e7** at 100,000 rows from 102 items, which
  only the 16 and 32 GB tiers open; there the interval is not computable and
  alpha is still computed. The 8 GB tier's 5,000,000 cells stop at 50 items.
- **Speed:** 128 items at 100,000 rows took 4.801, 4.743 and 4.739 s in a
  release build (`tests/alpha_speed.rs`, `GAPLY_ALPHA_SPEED=1`), with the same
  bytes each run; 30 items 0.346 s, 64 items 1.323 s. Under 15 s, so alpha has
  **no budget** (decision 371); the run manager's 30 s ceiling applies, as to
  every kind.
- **Check points:** 1 + k + k(k − 1)/2, one before the arithmetic, one before
  each item's row and one before each inter-item correlation; the screen says
  "{done} of {total} steps". 50 items make 1,276.
- **The screen:** the result card at 128 items rendered in 52 ms in the test's
  jsdom (`AlphaFlow.vitest.tsx`, three runs).

## 7. The export

`stats_alpha::export::result_csv(result, manifest)` and `manifest_json`
(decision 379): one long CSV, one row per number, CRLF line ends, in the
correlations' eight fields `table, statistic, first_index, first,
second_index, second, value, word`, in the screen's order:

- `result`: confidence_level, k, rows_read (the manifest's), rows_used;
- `alpha`: alpha, ci_lower, ci_upper, standardized, inter_item_mean,
  inter_item_min, inter_item_max;
- `items`, one block per item in the items' order, `first_index` its index
  among the items and `first` its alias: `reversed` as **1 or 0**, mean, sd,
  scale_mean_if_deleted, scale_variance_if_deleted, corrected_item_total,
  alpha_if_deleted;
- `excluded`, `rows`, each with its column;
- `note` for negative_alpha, the code in `word`;
- `limitation`, then the manifest's input, parameters and output hashes.

Every digit (Rust's shortest round-trip form); a value not computable is empty
with its code in `word`. **Columns are named by alias** (`C1`, `C2`, …), as a
stored result names them, so the same run, live or rerun from a project,
exports the same bytes. **The scale's ends are settings, not results**: the
project's report gives them ("Scale ends: 1 to 5"), the CSV does not.

Pinned by `tests/alpha_export.rs`: read-back field by field, the codes, the
flag, the note on its own, aliases only (sentinel headers, with positive
controls), ten runs byte-identical, and a golden CSV and manifest for four
items over twelve rows, one reversed on 1 to 5, a ticked code, a row left out
by an empty cell and one by the code, and one item constant. The golden's
bytes are macOS aarch64's (alpha 0.858678955453149, interval 0.62820474026282
to 0.96057388031518, checked against R). Every other platform, Linux x86_64
included, checks its layout (decision 346's check, in `tests/pin_support`),
each number within 4 units in the last place, until a later step pins
Linux's bytes from a Linux run (decision 379).

## 8. What SPSS documents, and where it differs

IBM SPSS Statistics 14.0 Algorithms, RELIABILITY, read 1 Nov 2026 as
documentation, never a gate:
https://public.dhe.ibm.com/software/analytics/spss/support/Stats/Docs/Statistics/Algorithms/14.0/reliability.pdf

| statistic | SPSS, as documented | Gaply |
|---|---|---|
| alpha, standardized alpha | k/(k − 1)(1 − Σ sᵢ²/s²ₚ); k r̄/(1 + (k − 1) r̄) | the same |
| item-total statistics | written through cov(Xᵢ, P) | the same quantities, each from the others' total (different rounding) |
| interval | the two-way, consistency, average-measures ICC's F interval, which reduces to Feldt's | Feldt's |
| zero-variance items | "method 2", used whenever covariance statistics are asked for, deletes them and changes k | never drops an item: k is the user's, and what needs the item's correlations is not computable |
| missing values | listwise by default (inferred; the PDF does not state it) | listwise |
| reverse-scoring | not part of RELIABILITY (done before it, by the user) | on the ends the user types, inside the method |
| weights | frequency weights | none |

## 9. What is not offered, and the limitations

Every result carries `assumes_one_dimension` and `no_omega`, and the
descriptor lists the matching wording codes:

- alpha assumes the items measure one thing; Gaply does not check that;
- McDonald's omega and squared multiple correlations are not shown;
- no split-half reliability and no Guttman's lambdas;
- the 95% interval only, no other confidence level;
- no weights;
- **no verdict on alpha and no advice**: no "acceptable", "good",
  "excellent", "questionable", "poor" or "unacceptable", no threshold such as
  0.7, no stars, and no word on which item to drop or reverse. Gaply says
  which items the user reverse-scored, never which they should (decision
  384); `alphaWording.vitest.ts` and `AlphaFlow.vitest.tsx` read every
  sentence and the rendered card for those words.

## 10. Blueprint §18.2 acceptance

| item | evidence | status |
|---|---|---|
| Formula/method and variant documented | §2, §3, §4; `stats_alpha/mod.rs` header | done |
| Golden fixtures and edge cases pass | `alpha_r_golden` (17 cases), `alpha_cases`, `alpha_export` | done |
| Missing values, reversal, weights, precision and sample size handled | §3, §4, §5, §6; weights not supported, stated | done |
| Diagnostic meanings and limitations documented | §4, §9; `AlphaCode` docs; a sentence for each of the 59 codes the screen can show (`alphaWording.ts`, `alphaCodes.json` pinned to Rust by `stats_alpha_wire.rs`, `alphaWording.vitest.ts`) | done |
| Independent reference comparison completed | base R and psych, §5 | done |
| Unsupported designs return explicit reasons | §1, §3; each refusal's code through IPC (`stats_alpha_commands.rs`) | done |
| Result manifest and export verified | §7; through IPC the CSV equals the module's export of the run's result and manifest; the privacy matrix, which forbids every header in the result, the CSV and the manifest (`stats_alpha_privacy_matrix.rs`); a saved result reruns identical, byte for byte (`project_alpha.rs`) | done |
