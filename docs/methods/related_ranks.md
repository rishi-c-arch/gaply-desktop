# The Wilcoxon signed-rank and Friedman tests, as an analysis

Capability `stats.analyze.related_ranks`, version 1, **Beta**, in the Statistics
workspace (Research OS, Phase 8Z-1, decisions 633 to 643, 10 Nov 2026, D317).
This page is the capability's `documentation_id` (`methods/related_ranks`).

It compares two or more numeric columns of the user's own data, measured on the
same rows, by the ranks of their values. The test is chosen by the count of
columns: the **Wilcoxon signed-rank test** when two columns are chosen, with an
exact p for small samples without ties or zero differences, and the **Friedman
test** when three or more are; with each column's median, each sign's or
column's rank sum and mean rank, the matched-pairs rank-biserial correlation
(two columns) and Kendall's W (three or more). It reproduces no reported result.
The pure core is `gaply_core::stats_related_ranks`; its façade is
`stats_adapters::related_ranks_admit` / `related_ranks_validate` /
`related_ranks_run`. The app reaches it through six `stats_related_ranks_*`
commands and the run manager's `related_ranks` kind. Planned in 8Z-1's Part 2
(decision 639), Beta since its Part 3 (decision 641).

**What a user sees at this step (decision 641's departure, as decision 607
recorded the rank tests'):** Beta with no item in the window, the old home's
card of a method with no screen, Help listing it, Coming later still saying
"Related-samples nonparametric tests", a workspace switch polling a run through
the summary's commands, and a saved run's report refusing it, until 8Z-2.

## 1. Inputs and the spec

`RelatedRanksParams { columns, missingCodes }`, camelCase, unknown fields
refused:

- `columns`: 2 to 50 column aliases (`C1`, …) in the user's order, each of finite
  numbers, never text, none named twice. Any may be a scale score. The order fixes
  the differences, first less second.
- `missingCodes`: numbers to treat as missing, applied by value to every chosen
  column; each finite.

The checks, first error first (`check()`): `missing_code_not_finite` (index);
`too_few_columns` (count) or `too_many_columns` (count, limit 50);
`column_repeated` (column); for each column `unknown_column` (column),
`column_non_finite` (column, count; a written-out infinity) or
`column_holds_text` (column, count); `code_equals_score` (column, count); then,
only when there is no other error, `no_complete_rows`. A refusal names its column
by alias and its counts as numbers. `validate` also refuses a spec of another
capability, version or input, a `preparation_mismatch` as every method does, and
warns of a suspect missing code in a chosen column (`suspect_missing_code`:
column, value, count) the user did not list.

## 2. Formulas, and where each number comes from

A row is used only when every chosen column has a number that is not a ticked
code. Ranks come from `stats_verify::average_ranks`, tie runs from
`stats_corr::runs`, medians from `stats_anova::median`, every p from
`stats_dist`: one implementation of each (decision 636).

**Two columns, the Wilcoxon signed-rank test** (F. Wilcoxon, 1945). For each
complete row d = first − second, as the paired t-test's.

- A d of zero is dropped and counted (`zero_differences`); n is the pairs left.
- The |d| are ranked from 1, ties given their average rank, compared as doubles
  with no rounding, as R's `digits.rank = Inf` default: 0.3 − 0.1 and 0.4 − 0.2 are
  0.19999999999999998 and 0.2, so they count as two different sizes, and R's help
  warns of the same. V is the sum of the positive d's ranks, R's V, and T− =
  n (n + 1) / 2 − V. Each sign's count, rank
  sum and mean rank are given; a sign with no difference has no mean rank.
- z = (V − n (n + 1) / 4) / σ with σ² = n (n + 1) (2n + 1) / 24 − Σ(t³ − t) / 48,
  **without a continuity correction**, as `wilcox.test(paired = TRUE, exact =
  FALSE, correct = FALSE)`. Its two-sided p is `stats_dist::normal_two_tailed(z)`.
- The exact two-sided p, min(1, 2 min(P(V ≤ v), P(V ≥ v))), from
  `stats_dist::signed_rank_exact_tails`, **only when no two |d| are tied, no d is
  zero and fewer than 50 pairs remain**. When it is absent a note says why:
  `exact_not_given_ties`, `exact_not_given_zeros`, `exact_not_given_size`, one or
  more of them.
- The exact distribution: the counts of the 2ⁿ sign patterns giving each V, by
  adding one rank at a time, in doubles, by additions only; each tail its own sum,
  the upper from the top down, never 1 minus the other, each divided by the total
  once. For 1 to 49 pairs (`SIGNED_RANK_EXACT_MAX`); `None` above.
- The matched-pairs rank-biserial correlation, (V − T−) / (n (n + 1) / 2)
  (D. S. Kerby, 2014): positive when the first column's values tend to be the
  larger. The median difference is the median of every complete row's d, zeros
  included.

**What R does by default, read from R 4.6.1's help and run 10 Nov 2026**
(`~/gaply-corpus/os/phase8/8z1/logs/r_help_1794308852.log`). `wilcox.test`'s help
reads: "By default (if `exact` is not specified), an exact p-value is computed if
the samples contain less than 50 finite values. Otherwise, a normal approximation
is used. If there are ties, exact inference is performed using the
conditional/permutation distribution given the observed ranks". So R's default
differs from Gaply in two ways: it gives a conditional exact p with ties, and its
exact test keeps zero differences, where `exact = FALSE` drops them (in that log, five pairs of which one is
zero: R's default gives V = 5 and `exact = FALSE, correct = FALSE` gives V = 4). It also
applies a continuity correction by default (`correct = TRUE`) when it uses the
normal approximation. The fixtures therefore ask for `exact = FALSE, correct =
FALSE` for V and the asymptotic p, and `exact = TRUE` only on cases with no tied
|d| and no zero.

**Three or more columns, the Friedman test** (M. Friedman, 1937). Within each
complete row the k values are ranked from 1, ties given their average rank; R_j
is column j's rank sum, its mean rank R_j / n.

- Q = (12 / (n k (k + 1))) Σ D_j² / C, with D_j = R_j − n (k + 1) / 2, exact since
  both are half-integers, and C = 1 − Σ over rows Σ(t³ − t) / (n (k³ − k)), the tie
  correction, computed as the exact integer n (k³ − k) − Σ(t³ − t) over n (k³ − k).
  Written as 1 minus a quotient it cancels when nearly every row is tied: with
  99,999 tied rows and one row 1, 2, 3, Q is exactly 2 and that form gives
  2.000000000009102 (the independent review's finding; `related_ranks_cases.rs`).
- df = k − 1, at most 49; its p, `stats_dist::chi_square_upper(Q, df)`.
- Kendall's W, Q / (n (k − 1)).

No base R function gives z, T−, the rank-biserial correlation, each sign's sums or
Kendall's W: **the same formulas are written twice**, in
`make_related_ranks_fixtures.R` and in `stats_related_ranks`, a check of the
arithmetic, and z's p is compared with R's. `friedman.test`'s own statistic is
compared with Gaply's Q as a separate family.

## 3. The columns' order and the signs

The columns are taken in the order the user chose them, never sorted. The
differences are the first column less the second, so V, T−, z and the rank-biserial
correlation are signed for that order, and choosing the columns the other way
round swaps V and T− and changes the sign of z. The limitation `column_order` says
so on every result. The Friedman test does not depend on the order.

## 4. Exclusions and edge cases

A row left out is counted once, under the first column in the list's order
without a usable number, as `missing_value` (the cell empty or a missing-value
token) or `missing_code` (the cell a ticked code). The complete rows plus the rows
left out are the data's rows.

- **Every d zero**: n = 0, so z, its p and the rank-biserial correlation are not
  computable (`all_differences_zero`), and the exact p is absent.
- **Every row's values tied**: C is 0, so Q, its p and W are not computable
  (`all_values_tied`). The rank sums and mean ranks are given.
- A difference that overflows from two finite values is ranked by its size, an
  infinity above every finite difference, as R ranks it. Any statistic it makes
  non-finite is not computable with `overflow`, statistic by statistic, never a
  refusal of the run. `distribution_out_of_range` is the code for a chi-square df
  outside what `stats_dist` measures; 50 columns gives df 49, so no run reaches it.
- Fewer than 5 non-zero differences, or complete rows, is noted (`rows_below_five`):
  the approximation is rougher there.
- A pair whose two values are equal is noted (`zero_differences_dropped`).
- -0 and 0 are one value, so a difference of -0 is a zero.

## 5. Precision, the gates and the oracles

Every gate is ten times the largest error measured (decision 316), or a stated
floor, asserted on every platform. Measured 10 Nov 2026, macOS aarch64.

**The distribution** (`tests/stats_dist_reference.rs` over
`make_signed_rank_dist_fixtures.R`'s `signrank_p.csv`, a script of its own, so the
files of the three scripts before it stay byte for byte; a pure relative error):

| function | rows | gate | measured |
|---|---|---|---|
| `signed_rank_exact_tails` against `psignrank` | 2,010 | 3.7e-14 | 3.650e-15 (n 48, v 353 and 823) |

**The method** (`tests/related_ranks_r_golden.rs`, the descriptor's validation
suite: 28 cases, 20 results and 8 refusals, 817 expected numbers, 782 compared, 6
not computable and 12 absent checked as such; counts and notes exact):

| family | gate | measured |
|---|---|---|
| medians, rank sums, mean ranks | 1e-15 (a floor) | 0 |
| z | 1e-15 (a floor) | 0 |
| rank-biserial, W | 1e-15 (a floor) | 0 |
| Q against the form written twice in R | 1e-15 (a floor) | 1.903e-16 (`f_ties`) |
| every p (asymptotic, exact, Friedman) | 1.1e-9 | 1.030e-10 (`w_tied_abs`) |
| Q against `friedman.test`'s | 1.3e-15 | 1.903e-16 (`f_ties`) |

The 1.3e-15 gate was set at ten times 1.219e-16, measured before the tie correction was computed from its exact integer (§2). That change moved the largest error to 1.903e-16, and the gate was left where it was rather than loosened.

**The bug hunt** (decision 616's standing rule; `~/gaply-corpus/os/phase8/8z1/hunt/`,
not committed): seed 8380638, 240 seeded synthetic datasets, 120 Wilcoxon and 120
Friedman, 10,899 complete rows, 15 datasets with overflowing pairs, 14,506 numbers
compared with R and with the formulas written twice, 128 not computable and 96
absent checked as such. No failure. Worst: median 2.172e-16, the Wilcoxon p
9.357e-11, `friedman.test`'s statistic 2.015e-16, the Friedman p 9.286e-14, every
other family 0; Q against its exact value in i128 on 80 datasets, worst 2.015e-16.

**The oracles** (ADR-008, for fixtures only, nothing copied): base R 4.6.1's `rank`,
`median`, `rle`, `wilcox.test`, `friedman.test`, `pnorm`, `pchisq`, `psignrank`; the
generator is `tests/fixtures/stats_related_ranks/make_related_ranks_fixtures.R`, run
as `/usr/local/bin/Rscript`. Its cases: `w_small`, `w_tied_abs`, `w_zeros`,
`w_all_zero`, `w_49`, `w_50`, `w_one_pair`, `w_small_reversed`, `w_one_sign`,
`w_overflow`, `w_overflow_both_signs`, `w_large`; `f_small`, `f_ties`, `f_all_tied`,
`f_50_columns`, `f_large`; `missing_three`, `missing_three_order`, `missing_two`; and
a refusal for each of eight checks data decides.

## 6. Speed

One check point, before the arithmetic: Cancel and the run manager's 30 s ceiling
act there. The exact distribution is computed only when the exact p is given.
Measured in a release build (10 Nov 2026, a probe kept at
`~/gaply-corpus/os/phase8/8z1/p8z1_speed.rs`, seven runs each):

| run | min | median | max |
|---|---|---|---|
| Wilcoxon, 2 columns, 100,000 rows | 5.51 ms | 6.35 ms | 7.27 ms |
| Friedman, 50 columns, 100,000 rows | 274.21 ms | 275.21 ms | 275.56 ms |
| `signed_rank_exact_tails(600, 49)` alone | 0.02 ms | 0.02 ms | 0.03 ms |
| Wilcoxon with its exact p, 49 pairs | 0.02 ms | 0.02 ms | 0.03 ms |

No timing test is in the suite.

## 7. The export

`stats_related_ranks::export::result_csv(result, manifest)` and `manifest_json`: one
long CSV in the eight fields every method's export has, `table, statistic,
column_index, column, column_index_2, column_2, value, word`. Each chosen column's
median; the rows used; then the Wilcoxon test, each line naming the first column
and the second, or the Friedman test, each column's rank sum and mean rank, then Q,
df, its p and Kendall's W; each row left out as two lines, its count and the column
it was found in; the notes, the limitations and the manifest's hashes. A column is
its index in the chosen list and its alias, never a header. Every digit (Rust's
shortest round-trip form); a value not computable is empty with its code in `word`,
an absent exact p carries the first note that says why, and a side with no
difference has an empty mean. Pinned by `tests/related_ranks_export.rs`: a golden CSV
and manifest on macOS aarch64, the layout elsewhere. The CSV pin is `-text` in
`.gitattributes`, so its CRLF line ends are committed as written.

## 8. What is not assessed, and the limitations

Every result carries `no_pairwise_comparisons`, `two_sided_only`,
`no_location_estimate`, `exact_without_ties_or_zeros_only`, `pratt_not_offered`,
`no_further_related_tests`, `no_weights` and `column_order`; the descriptor lists the
table limitations, crosstabs' weights and its own seven wording codes, word for word:

- "Pairwise comparisons after a Friedman test are not given."
- "Each Wilcoxon p value is two-sided; one-sided tests are not offered."
- "No estimate of the typical difference between two columns, such as the Hodges-Lehmann estimate, and no interval for it, is given."
- "The Wilcoxon test's exact p value is given only when no two differences are tied in size, none is zero and there are fewer than 50 pairs; otherwise only the normal approximation's p value is given."
- "A pair whose two values are equal is left out of the Wilcoxon test; Pratt's way of keeping such pairs is not offered."
- "The sign test, the marginal homogeneity test and Cochran's Q test are not given."
- "Columns are taken in the order you choose them: the Wilcoxon differences are the first column less the second, and the signs of z and of the matched-pairs rank-biserial correlation follow that order."
- and crosstabs': "Each row is counted once; weighting rows by a column is not offered."

Also: no exact p for the Friedman test, and no interval for the rank-biserial
correlation or Kendall's W. The tests assume the rows are independent of one
another, which the data cannot show. How large a file opens depends on this
computer's memory ([large_samples.md](large_samples.md) §8): 100,000 data rows and
128 columns on every computer.

The wording is neutral: no verdict word, no stars (decision 66). Every p value and
statistic is a number, never called significant. The screen's sentence for each of
the method's 34 codes comes with the screen, in 8Z-2.

## 9. What SPSS documents, and where it differs

**None of SPSS's behaviour for these two tests was read, so every row below is "not
checked".** Attempted 10 Nov 2026: IBM's Command Syntax Reference PDF
(https://www.ibm.com/docs/en/SSLVMB_31.0.0/pdf/IBM_SPSS_Statistics_Command_Syntax_Reference.pdf)
is over the 10 MB fetch limit; the NPTESTS RELATED topic
(https://www.ibm.com/docs/en/spss-statistics/30.0.0?topic=nptests-related-subcommand)
returned a page title and no body; two searches returned the documents' names and
third-party guides, and no IBM text on any item below. The statement in the module's
header that the test is chosen "as SPSS's related-samples nonparametric tests choose"
rests on that design's earlier decision, not on a page read here.

| item | SPSS | Gaply | status |
|---|---|---|---|
| test chosen by the count of columns | not checked | Wilcoxon for two, Friedman for three or more | not checked |
| significance by default | not checked | asymptotic always; exact without ties or zeros below 50 pairs | not checked |
| zero differences | not checked | dropped and counted | not checked |
| continuity correction in z | not checked | none | not checked |
| tie correction | not checked | σ² and Q tie-corrected | not checked |
| pairwise comparisons after Friedman | not checked | not given | not checked |
| effect sizes | not checked | rank-biserial, Kendall's W | not checked |
| Hodges-Lehmann, sign test, marginal homogeneity, Cochran's Q | not checked | not given | not checked |
| missing values | not checked | a row used only when complete in every chosen column | not checked |

## 10. Blueprint §18.2 acceptance

| item | evidence | status |
|---|---|---|
| Formula/method and variant documented | §2, §3; `stats_related_ranks/mod.rs` and `stats_dist/mod.rs` headers | done |
| Golden fixtures and edge cases pass | `related_ranks_r_golden` (28 cases), `related_ranks_cases` (23), `related_ranks_export` (8), `stats_dist_reference` | done |
| Missing values, ties, weights, precision and sample size handled | §1, §2, §4, §5, §6; weights not supported, stated | done |
| Diagnostic meanings and limitations documented | §4, §8; `RelatedRanksCode` docs; `relatedRanksCodes.json` pinned to Rust by `stats_related_ranks_wire.rs`; the screen's sentences are 8Z-2's | partly |
| Independent reference comparison completed | R, §5 | done |
| Unsupported designs return explicit reasons | §1, §4; each refusal's code through IPC (`stats_related_ranks_commands.rs`) | done |
| Result manifest and export verified | §7; through IPC the CSV equals the module's export of the run's result and manifest; `stats_related_ranks_privacy_matrix.rs`; saving and rerunning are 8Z-2's | partly |
