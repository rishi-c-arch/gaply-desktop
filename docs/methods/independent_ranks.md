# The Mann-Whitney and Kruskal-Wallis tests, as an analysis

Capability `stats.analyze.independent_ranks`, version 1, **Beta**, in the
Statistics workspace (Research OS, Phase 8Y-1, decisions 600 to 609, 10 Nov
2026, D315; and Phase 8Y-2, decisions 615 to 625, D316). This page is the capability's `documentation_id`
(`methods/independent_ranks`).

It compares a numeric column of the user's own data across two or more
groups by the ranks of its values. The test is chosen by the count of
included levels, as SPSS's independent-samples nonparametric tests choose:
the **Mann-Whitney U test** when two groups are included, with an exact p for
small groups without ties, and the **Kruskal-Wallis H test** when three or
more are; with each group's count, median, rank sum and mean rank, the
rank-biserial correlation (two groups) and epsilon-squared (three or more).
It reproduces no reported result; Level 1 has no rank test of this kind. The
pure core is `gaply_core::stats_ranks`; its façade is
`stats_adapters::ranks_admit` / `ranks_validate` / `ranks_run`. The app
reaches it through seven `stats_ranks_*` commands, a levels command among
them, and the run manager's `independent_ranks` kind. Planned in 8Y-1's Part
2 (decision 605), Beta since its Part 3 (decision 607).

Since 8Y-2 (decisions 619 to 622) a user runs it from the Statistics window:
Analyze, a new group **Nonparametric tests** after Scale, **Independent
samples…**, as SPSS places it and as its dialog chooses the test by the number
of groups; Coming later's "Nonparametric tests" became "Related-samples
nonparametric tests", since the Wilcoxon signed-rank test is not built
(decision 624). There is no page and no route (ADR-012:233-241): the old home
shows the card of a method with no screen of its own, the window's dialog is
its one screen, and the Output's card is drawn by `RanksCard.tsx`. A result can
be saved to a project, read back, and run again `Identical`, its report naming
the outcome and the group by role, the included levels by index and the
missing codes by count; a workspace switch polls and cancels a run through the
method's own commands.

## 1. Inputs and the spec

`IndependentRanksParams { outcome, group, levels, missingCodes }`, camelCase,
unknown fields refused, each as the one-way ANOVA's `AnovaParams`, with no
post hoc tests:

- `outcome`: a column alias (`C1`, …) of finite numbers, never text. It may
  be a scale score.
- `group`: a column alias, never a scale score (`score_as_group`, as
  `stats_ttest::levels_for_group` refuses one), 2 to 50 levels over the whole
  column, in code-point order.
- `levels`: `null` for every level, or the ticked levels' indexes in that
  order; each in range and not repeated; at least two.
- `missingCodes`: numbers to treat as missing, applied by value to the
  outcome only; each finite. A group cell equal to a code is a level.

The checks, in the ANOVA's order (`check()`), first error first:
`missing_code_not_finite` (index), `unknown_column` (column),
`score_as_group` (column), `code_equals_score` (column, count),
`same_column`, `outcome_non_finite` (column, count; a written-out infinity),
`outcome_holds_text` (column, count), `group_one_level` (column, count),
`too_many_levels` (count, limit 50), `level_out_of_range` (index, limit),
`level_repeated` (index), `too_few_levels` (count); then, only when there is
no other error, `group_empty` (index): an included level with no value after
the exclusions. A refusal names its column by alias and its counts as
numbers, never a label. `validate` also refuses a spec of another capability,
version or input, a `preparation_mismatch` as every method does, and warns of
a suspect missing code in the outcome (`suspect_missing_code`: column, value,
count) the user did not list.

The levels command, `stats_ranks_group_levels`, is the ANOVA's: the group
column's levels by index, label and count, through
`stats_ttest::levels_for_group`; an unknown column is `unknown_column`, a
score `score_as_group`, more than 50 levels `too_many_levels`, before any
label is listed.

## 2. Formulas, and where each number comes from

Every included value is ranked once, together, from 1, ties given their
average rank, through `stats_verify::average_ranks` — Spearman's ranks, the
one implementation, made `pub(crate)` with its body unchanged — which gives
R's `rank(ties.method = "average")` (checked on finite values with -0 and 0
tied, `stats_ranks`' unit test). The tie runs' sizes t come from
`stats_corr::runs` over the sorted values, Σ(t³ − t) summed in u128 and
converted to a double once; a tie run is a run of two or more. Each group's
median is `stats_anova::median` (the middle value, or the mean of the two
middle ones, summed first, and halved first only where that sum overflows, so
it is finite for every two finite values and every other median keeps its bits;
decision 618), its rank sum R the sum of its ranks, its mean rank R / n. N is
the number of included values.

**Two groups, the Mann-Whitney test** (H. B. Mann and D. R. Whitney, 1947).
The first group is the included level with the lower index (§3):

- U1 = R1 − n1 (n1 + 1) / 2: R's `wilcox.test(first, second)` statistic W;
  U2 = n1 n2 − U1.
- z = (U1 − n1 n2 / 2) / σ with σ² = (n1 n2 / 12) ((N + 1) − Σ(t³ − t) /
  (N (N − 1))), the tie-corrected variance, **without a continuity
  correction**, as SPSS's asymptotic test and R's `wilcox.test(correct =
  FALSE)`; R corrects by 0.5 by default (`correct = TRUE`). Its two-sided p is
  `stats_dist::normal_two_tailed(z)`, erfc(|z| / √2).
- The exact two-sided p, min(1, 2 min(P(U ≤ U1), P(U ≥ U1))), from
  `stats_dist::rank_sum_exact_tails`, **only when no two included values are
  tied and each group has fewer than 50 values**: R's rule for its default
  before R 4.6.0. Since R 4.6.0 `wilcox.test` by default gives a conditional
  exact p with ties too, which Gaply does not give (a limitation); the
  golden fixtures ask R for `exact = TRUE` only on cases without ties and
  `exact = FALSE` everywhere else. When the exact p is absent, a note says
  why: `exact_not_given_ties`, `exact_not_given_size`, or both.
- The exact distribution: the counts of the C(n1 + n2, n1) orders giving each
  U, by the recurrence f(m, n, u) = f(m − 1, n, u − n) + f(m, n − 1, u) (the
  largest value is the first group's or the second's), in doubles, by
  additions only, one row per first-group size updated in place for each
  second-group size; each tail summed as its own sum, the upper from n1 n2
  down, never 1 minus the other, each divided by the total once. For 1 to 49
  values a group (`RANK_SUM_EXACT_MAX`); `None` above. At 49 and 49 it is
  49 × 49 × 2,401 additions, 1.58 ms in release (§6).
- The rank-biserial correlation, 2 U1 / (n1 n2) − 1 (D. S. Kerby, 2014, the
  simple difference formula): positive when the first group's values tend to
  be the larger.

**Three or more groups, the Kruskal-Wallis test** (W. H. Kruskal and W. A.
Wallis, 1952):

- H = (12 / (N (N + 1))) Σ (Dᵢ² / nᵢ) / C with Dᵢ = Rᵢ − nᵢ (N + 1) / 2 and
  C = 1 − Σ(t³ − t) / (N³ − N), the tie correction. Dᵢ is a difference of two
  half-integers below 2⁵², so it is exact (decision 618), and this form has
  neither the cancellation of 12 / (N (N + 1)) Σ Rᵢ² / nᵢ − 3 (N + 1), the form
  R's `kruskal.test` computes, nor that of the mean rank less (N + 1) / 2,
  8Y-1's form, which lost digits on the near-null case. On `kw_large`, the 99,917
  rows of that case, H is **1.8318061856922257e-8**, equal to its exact rational
  value (computed from the data in i128 by `ranks_cases.rs`) within 1e-15; 8Y-1's
  form gave 1.8318061857470532e-8, 2.99e-11 relative from it, and
  `kruskal.test`'s form gives 1.833724711306231e-8, 1.047e-3 relative from it.
- df = k − 1, at most 49; its p, `stats_dist::chi_square_upper(H, df)`.
- Epsilon-squared, H / (N − 1) (M. Tomczak and E. Tomczak, 2014).

No base R function gives z, the rank-biserial correlation, H in this form or
epsilon-squared: **the same formulas are written twice**, in
`make_ranks_fixtures.R` and in `stats_ranks`, a check of the arithmetic, and
z is checked against R's p within 1e-9.

## 3. The groups' order and the signs

Groups are listed in code-point order of their labels (decision 78), whatever
the locale: upper case before lower case, letters outside ASCII after
(`Zebra, apple, banana, Äpfel, Ölung, éclair`), and a group column of numbers
`10` before `2`. The first group of the Mann-Whitney test is the included
level with the lower index in that order, so U1, z and the rank-biserial
correlation are signed for it: with groups 2 and 10, the first is 10
(`numeric_2_10` in the golden fixtures; `ranks_cases.rs`). The limitation
`rank_groups_in_code_point_order` says so on every result.

## 4. Exclusions and edge cases

A row is left out once, for the first reason that applies, each reason
reported with its count and the column it was found in (the ANOVA's four, in
its order): `missing_group` (the group cell empty or a missing token, the
group's column), `other_level` (a level not included, the group's column),
`missing_outcome` (the outcome empty or a missing token), `missing_code` (the
outcome a listed code). N plus the rows left out is the data's rows.

- **Every included value equal**: σ and C are 0, so z and its p, and H, its p
  and epsilon-squared, are not computable (`all_values_tied`), never a
  number. U1, U2, the rank-biserial correlation (0), the rank sums and the
  mean ranks are given, and the exact p is absent with `exact_not_given_ties`.
  This is decided from the values (one tie run of N), not from a zero.
- Any other value a double cannot hold is not computable with its own code,
  statistic by statistic, never a refusal of the run (decision 618): `overflow`
  for z, its p, H, its p or epsilon-squared, `distribution_out_of_range` for the
  Kruskal-Wallis p (none is reachable through the checks; unit tests reach each
  with a crafted tie sum or 2,403 crafted groups, and the wire fixture carries
  the shape),
  `arithmetic_refused` (no producer: kept as the ANOVA's code list has it). The
  rank-biserial correlation and the exact p have no not-computable form in the
  wire shape, so `overflow` there and `distribution_out_of_range` for the exact p
  remain a refusal, unreachable.
- A group of fewer than 5 values is noted (`group_below_five`): the normal
  or chi-square approximation is rougher there. A group of one value is a
  group.
- -0 and 0 are one value (a tie).
- Permuting the rows changes no bit: each group's values are sorted first.

## 5. Precision, the gates and the oracles

Every gate is ten times the largest error measured (decision 316), asserted
on every platform. Measured 10 Nov 2026, macOS aarch64.

**The distribution** (`tests/stats_dist_reference.rs` over
`make_ranks_dist_fixtures.R`'s `wilcox_p.csv`, a script of its own, so the
six files of the two scripts before it stay byte for byte; a pure relative
error):

| function | rows | gate | measured |
|---|---|---|---|
| `rank_sum_exact_tails` against `pwilcox` | 1,063: n1 and n2 among 1, 2, 3, 5, 10, 20, 35 and 49, every u to 20 values, 17 u's above; both tails | 2e-13 | 1.310e-14 (the lower tail at 49, 49, u 1,200) |

**The method** (`tests/independent_ranks_r_golden.rs`, the descriptor's
validation suite: 30 cases, 19 results and 11 refusals, 685 numbers compared,
5 not computable and 4 exact p absent checked as such; counts, df, labels,
the first and second groups, exclusions and notes exact; every p a pure
relative error; every other number relative below a floor of 1e-12):

| family | gate | measured |
|---|---|---|
| group medians, rank sums, mean ranks | 1e-15 (a floor) | 0, bit for bit |
| U1, U2 | 1e-15 (a floor) | 0 |
| z | 1e-15 (a floor) | 0 |
| every p (asymptotic, exact, Kruskal-Wallis) | 9e-10 | 8.833e-11 (`mw_large`'s asymptotic p: statrs's `erfc` against R's `pnorm`, as `corr_r_golden.rs` measured 9.958e-11) |
| H (against the form written twice in R) | 1e-15 (a floor) | 0 |
| H against `kruskal.test`'s, every case but `kw_large` | 1.2e-14 | 1.151e-15 (`kw_small`) |
| rank-biserial correlation, epsilon-squared | 1e-15 (a floor) | 0 |

`kruskal.test`'s H differs from Gaply's by 1.047e-3 relative at `kw_large`
(§2), printed and not gated; `ranks_cases.rs` holds Gaply's H there to its exact
value. Gaply's p there is compared with `kruskal.test`'s p, within the p gate.

**The bug hunt** (decisions 616 and 618; `~/gaply-corpus/os/phase8/8y2/hunt/`, not
committed): 240 seeded synthetic datasets, two to fifty groups, sizes 1 to 300,
every kind of tie, missing codes and empty cells, magnitudes 1e-300 to 1e300 and
middle values above 9e307, labels outside ASCII and numbers as levels, compared
number by number with R's `wilcox.test`, `kruskal.test`, `median` and `rank` and
with the formulas written twice, and H with its exact value in integers. No
failure in Gaply's numbers (Gaply's H within 4.368e-16 of the exact value on all
92 datasets with an H). `kruskal.test`'s H and p are R's and were compared apart:
its H differed from Gaply's beyond the 1.2e-14 gate on 61 datasets. On 59 of them
that is its own form's cancellation, up to 3.476e-13 from the exact value, with its
p within 6.336e-13 of Gaply's. On the other 2, `table()` merges doubles that agree
to 15 digits while `rank()` does not, so its tie sum is wrong: its H is up to
1.329e-8 from the exact value and its p up to 7.072e-8 from Gaply's.

**The oracles** (ADR-008, for fixtures only, nothing copied): base R 4.6.1's
`rank(ties.method = "average")`, `median`, `table`, `wilcox.test(exact =
FALSE, correct = FALSE)` (W and the asymptotic p), `wilcox.test(exact =
TRUE)` (only without ties), `kruskal.test` (df, p), `pwilcox`; the generator
is `tests/fixtures/stats_ranks/make_ranks_fixtures.R`, run as
`/usr/local/bin/Rscript`, reading its data back as Gaply does (strip.white,
"" and "NaN" missing, levels in code-point order by `sort(method =
"radix")`). Its cases: two groups small, at U1 = n1 n2 / 2 (1, 4 against 2,
3: twice a tail is 4/3, the exact p 1), with ties, 49 and 49, 50 and 10, one
value against thirty, and 100,010 rows; three groups small, with many ties,
fifty groups (df 49), and 99,917 rows near the null; every value equal in two
and in three groups; a group column of 2 and 10; text levels outside ASCII;
four levels with two and with three ticked; missing codes in the outcome;
empty cells in each column; and a refusal for each check data decides.

## 6. Speed

One check point, before the arithmetic: Cancel and the run manager's 30 s
ceiling act there. The exact distribution is computed only when the exact p
is given. Measured in a release build (10 Nov 2026, a probe kept at
`~/gaply-corpus/os/phase8/8y1/p8y1_speed.rs`, seven runs each):

| run | min | median | max |
|---|---|---|---|
| Mann-Whitney, 2 groups, 100,000 rows | 10.20 ms | 10.66 ms | 13.88 ms |
| Kruskal-Wallis, 50 groups, 100,000 rows | 28.76 ms | 28.89 ms | 29.07 ms |
| `rank_sum_exact_tails(1200, 49, 49)` alone | 1.56 ms | 1.58 ms | 1.68 ms |
| Mann-Whitney with its exact p, 49 and 49 | 1.58 ms | 1.62 ms | 1.65 ms |

No timing test is in the suite.

## 7. The export

`stats_ranks::export::result_csv(result, manifest)` and `manifest_json`: one
long CSV in the eight fields every method's export has, `table, statistic,
group_index, group, group_index_2, group_2, value, word`. Each included
group's count, median, rank sum and mean rank; N and the tie runs; then the
Mann-Whitney test's U1, U2, z, p, exact p and rank-biserial correlation, each
line naming the first group and the second, or the Kruskal-Wallis test's H,
df, p and epsilon-squared; each row left out as two lines, its count and the
column it was found in; the notes, the limitations and the manifest's input,
parameters and output hashes. Every digit (Rust's shortest round-trip form);
a value not computable is empty with its code in `word`, and an absent exact
p carries the note that says why. The descriptive summary's field writer
gives the one formula-injection rule (a label `=lo` is written `'=lo`).
Pinned by `tests/ranks_export.rs`: read back field by field, a golden CSV
and manifest on macOS aarch64 and, since 8Y-2 (decisions 615 and 618), Linux
x86_64 checked exactly against the same pin, measured outside this Mac at
46772148 to give the same bytes; every other platform checks that pin's layout
within 4 ulps (decision 604); ten runs byte-identical. The CSV pin is `-text`
in `.gitattributes`, so its CRLF line ends are committed as written.

## 8. What is not assessed, and the limitations

Every result carries `no_pairwise_comparisons`, `two_sided_only`,
`no_location_shift_estimate`, `exact_without_ties_only`,
`no_further_k_sample_tests`, `no_weights` and `code_point_order`; the
descriptor lists the table limitations, the levels' reading, crosstabs'
weights and its own six wording codes, word for word:

- "Pairwise comparisons after a Kruskal-Wallis test, such as Dunn's test, are not given."
- "Each Mann-Whitney p value is two-sided; one-sided tests are not offered."
- "No estimate of the shift between two groups, such as the Hodges-Lehmann estimate, and no interval for it, is given."
- "The Mann-Whitney test's exact p value is given only when no two values are tied and each group has fewer than 50 values; otherwise only the normal approximation's p value is given."
- "The median test and the Jonckheere-Terpstra test are not given."
- "Groups are listed in the order Gaply reads them, so a group column of numbers lists 10 before 2; which group is first, and the signs of the Mann-Whitney z and of the rank-biserial correlation, follow that order."
- and crosstabs': "Each row is counted once; weighting rows by a column is not offered."

Also: the tests assume independent observations, which the data cannot show;
no exact p for the Kruskal-Wallis test; no interval for the rank-biserial
correlation or epsilon-squared. How large a file opens depends on this
computer's memory ([large_samples.md](large_samples.md) §8): 100,000 data
rows and 128 columns on every computer.

The wording is neutral: no verdict word, no stars (decision 66). Every p
value and statistic is a number, never called significant.

The screen's sentence for each of the method's 37 codes (`ranksWording.ts`, 8Y-2),
word for word; {column} is the column's name as the user sees it, {level} a
level's label, and {count}, {value}, {limit}, {found}, {expected}, {n} and {k} the
code's numbers. A count of 1 reads in the singular (1 cell, 1 row, 1 level, and
"was left out"):

| code | sentence |
|---|---|
| all_values_tied | Not computable: every included value is equal, so the ranks do not differ between groups. |
| arithmetic_refused | Not computable: the arithmetic refused to give a value. |
| capability_version_mismatch | These settings were made for version {found} of the rank tests, and this is version {expected}. |
| code_equals_score | Column {column} is a score, and it holds a missing code you ticked in {count} cells. Untick the code, or leave this score out. |
| code_point_order | Groups are listed in the order Gaply reads them, so a group column of numbers lists 10 before 2; which group is first, and the signs of the Mann-Whitney z and of the rank-biserial correlation, follow that order. |
| distribution_out_of_range | Not computable: this needs the chi-square distribution at more than 2,401 degrees of freedom, where Gaply has not measured its accuracy. |
| exact_not_given_size | The exact p value is not given, because a group has 50 values or more. |
| exact_not_given_ties | The exact p value is not given, because two or more included values are tied. |
| exact_without_ties_only | The Mann-Whitney test's exact p value is given only when no two values are tied and each group has fewer than 50 values; otherwise only the normal approximation's p value is given. |
| group_below_five | A group has fewer than 5 values, where the normal and chi-square approximations are rougher. |
| group_empty | The group "{level}" has no value after the exclusions. Leave it out, or choose another group. |
| group_one_level | The group {column} has {count} levels, and a rank test needs at least two. |
| input_count | These settings name {count} files, and a rank test reads exactly one. |
| input_mismatch | These settings were made for a different file from the one open. Open the file again. |
| level_out_of_range | Level {n} is not one of the group's levels: it has {k}. |
| level_repeated | Level {n} is included twice. |
| missing_code | {count} rows held a missing code you ticked in the outcome, and were left out. |
| missing_code_not_finite | Missing code {n} is not a finite number. |
| missing_group | {count} rows had no group (an empty cell or a missing-value token), and were left out. |
| missing_outcome | {count} rows had no outcome (an empty cell or a missing-value token), and were left out. |
| no_further_k_sample_tests | The median test and the Jonckheere-Terpstra test are not given. |
| no_location_shift_estimate | No estimate of the shift between two groups, such as the Hodges-Lehmann estimate, and no interval for it, is given. |
| no_pairwise_comparisons | Pairwise comparisons after a Kruskal-Wallis test, such as Dunn's test, are not given. |
| no_weights | Each row is counted once; weighting rows by a column is not offered. |
| other_level | {count} rows were in levels left out, and were left out. |
| outcome_holds_text | The outcome {column} holds text in {count} cells, so it cannot be used as a numeric outcome. |
| outcome_non_finite | The outcome {column} holds an infinity or "not a number" in {count} cells, so it cannot be used as a numeric outcome. |
| overflow | Not computable: the arithmetic went past the largest number a double can hold. |
| preparation_mismatch | These settings do not match how the open data was prepared. Choose the settings again. |
| same_column | The outcome and the group are the same column. Choose two different columns. |
| score_as_group | Column {column} is a score, so it cannot be the group. Choose a column from your file. |
| suspect_missing_code | The outcome {column} holds {value} in {count} cells, which may be a code for a missing value. It is counted as a number unless you tick it in Settings. |
| too_few_levels | {count} levels included, and a rank test needs at least two. |
| too_many_levels | This column has {count} distinct values, more than the {limit} a group may have, so its values are not listed. |
| two_sided_only | Each Mann-Whitney p value is two-sided; one-sided tests are not offered. |
| unknown_capability | These settings name a capability Gaply does not have here. |
| unknown_column | Column {column} is not in this file. |

## 9. What SPSS documents, and where it differs

Read 10 Nov 2026 as documentation, never a gate. IBM's Command Syntax
Reference page for NPTESTS INDEPENDENT
(https://www.ibm.com/docs/en/spss-statistics/30.0.0?topic=nptests-independent-subcommand)
returned no body to the fetch, and IBM's Algorithms PDF was not read (8X-1
found it too large to fetch). What was read: the SPSS Exact Tests manual's
description (https://www.ibm.com/docs/en/SSLVMB_27.0.0/pdf/en/IBM_SPSS_Exact_Tests.pdf,
through a search summary) and IBM Community answers by IBM staff
(https://community.ibm.com/community/user/discussion/spss-statistics-v28-performs-kruskal-wallis-instead-of-mann-whitney-u-test,
https://community.ibm.com/community/user/discussion/post-hoc-comparisons-after-kruskal-wallis-test).

| item | SPSS | Gaply | status |
|---|---|---|---|
| test chosen by the count of groups | NPTESTS INDEPENDENT runs Mann-Whitney for two groups and Kruskal-Wallis for more | the same, by included levels | read (community, IBM staff) |
| significance by default | asymptotic; exact and Monte Carlo methods with the Exact Tests option | asymptotic always; exact without ties below 50 a group | read (Exact Tests manual, summary) |
| pairwise comparisons after Kruskal-Wallis | Dunn's tests, `KRUSKAL_WALLIS(COMPARE=PAIRWISE)` | not given | read (community) |
| continuity correction in z | not checked | none | not checked |
| tie correction | not checked | σ² and H tie-corrected | not checked |
| exact p with ties | not checked | not given | not checked |
| effect sizes | not checked | rank-biserial, epsilon-squared | not checked |
| Hodges-Lehmann estimate, median test, Jonckheere-Terpstra | not checked (other keywords) | not given | not checked |
| order of a numeric group | not checked | code point (§3) | not checked |

## 10. Blueprint §18.2 acceptance

| item | evidence | status |
|---|---|---|
| Formula/method and variant documented | §2, §3; `stats_ranks/mod.rs` and `stats_dist/mod.rs` headers | done |
| Golden fixtures and edge cases pass | `independent_ranks_r_golden` (30 cases), `ranks_cases` (19), `ranks_export` (7), `stats_dist_reference` | done |
| Missing values, ties, weights, precision and sample size handled | §1, §2, §4, §5, §6; weights not supported, stated | done |
| Diagnostic meanings and limitations documented | §4, §8; `RanksCode` docs; `ranksCodes.json` pinned to Rust by `stats_ranks_wire.rs`; the 37 sentences in `ranksWording.ts` and §8, with every code covered by `ranksWording.vitest.ts` | done |
| Independent reference comparison completed | R, §5 | done |
| Unsupported designs return explicit reasons | §1, §4; each refusal's code through IPC (`stats_ranks_commands.rs`) | done |
| Result manifest and export verified | §7; through IPC the CSV equals the module's export of the run's result and manifest; the privacy matrices (`stats_ranks_privacy_matrix.rs`, `project_rerun_privacy_matrix.rs`); saved and rerun Identical (`project_ranks.rs`) | done |
