# Crosstabs and chi-square, as an analysis

Capability `stats.analyze.crosstabs`, version 1, **Beta**, in the Statistics
workspace (Research OS, Phase 8X-1; decisions 573 to 582, 9 Nov 2026; D313;
and Phase 8X-2, decisions 586 to 594; D314).
This page is the capability's `documentation_id` (`methods/crosstabs`).

It counts how often each pair of levels of two columns of the user's own data
occurs together, and gives each cell's expected count, proportions and
residuals, Pearson's chi-square, the likelihood ratio, for a table of two rows
and two columns the continuity-corrected chi-square and Fisher's exact test,
and phi, Cramér's V and the contingency coefficient. It is **not** a
reproduction of a reported result; that is Level 1's
`stats.verify.chi_square_independence`, which this capability does not change.
The pure core is `gaply_core::stats_xtab`; its façade is
`stats_adapters::xtab_admit` / `xtab_validate` / `xtab_run`. The app reaches it
through the six `stats_xtab_*` commands and the run manager's `crosstabs`
kind. Planned in 8X-1's Part 2 (decision 578), Beta since its Part 3 (decision
580). Since 8X-2 (decisions 588 to 591) a user runs it from the Statistics
window: Analyze, Descriptive statistics, **Crosstabs…**, after Summary…, where
SPSS has it, opens its dialog (two pickers, rows and columns, each offering a
file's columns with 2 to 50 distinct values in the profile, and the two
columns' candidate missing codes); its result is a card in the Output, with
one cell statistic shown at a time; it is saved to a project, named in the
project report by its two columns' positions and the roles `table_rows` and
`table_columns`, and run again from the project page or the window's Project
menu. It has no page and no route of its own (ADR-012:233-241); the old
Statistics home shows the card a method with no page shows.

## 1. Inputs and the spec

`CrosstabsParams { rows, columns, missingCodes }`, camelCase, unknown fields
refused:

- `rows`, `columns`: Level 1's column aliases (`C1`, `C2`, …), two different
  columns of the file. A scale score is never either (`score_as_column`),
  as it is never a group.
- `missingCodes`: numbers to treat as missing, **ascending**, each finite and
  listed once. A code that is NaN or an infinity, a code repeated, or codes
  out of order are refused by index (`missing_code_not_finite`,
  `missing_code_repeated`, `missing_codes_out_of_order`), never sorted, so one
  choice is one spec and one parameters hash (decision 338's rule).

The codes apply **by value to both columns**, which no group column did
before: a cell that is a number equal to a code is left out, compared as a
number before its level code is taken. A text cell never matches a code: in a
column the file declares as text, the cell `99` is a level, not the code 99. A
value an SPSS file declares user-missing is already missing when the file is
read, so it counts as `missing_row` or `missing_column`, never `missing_code`.

There is no levels command and no choice of levels: the table takes every
level of both columns among the included rows (decision 575).

## 2. Formulas, and where each number comes from

With r rows, c columns, Oᵢⱼ the observed counts, Rᵢ and Cⱼ the totals and n
the included rows:

| number | formula | from |
|---|---|---|
| expected | Eᵢⱼ = Rᵢ Cⱼ / n, the product of two whole numbers divided once | `stats_xtab` |
| proportions | O / Rᵢ, O / Cⱼ, O / n | `stats_xtab` |
| residual | O − E | `stats_xtab` |
| Pearson residual | (O − E) / √E, R's `chisq.test()$residuals` | `stats_xtab` |
| adjusted standardized residual | (O − E) / √(E (1 − Rᵢ/n)(1 − Cⱼ/n)), R's `$stdres` | `stats_xtab` |
| Pearson's χ² | Σ (O − E)² / E, df (r − 1)(c − 1) | `stats_verify::chi_square_independence`, the one implementation, Level 1's too; its p (statrs's 1 − cdf) is never read |
| likelihood ratio | G² = 2 Σ O ln(O / E) over cells with O > 0, clamped at 0 from below, the same df | `stats_xtab` |
| continuity-corrected χ² (2 × 2) | Σ (|O − E| − min(0.5, |O − E|))² / E, df 1, R's `chisq.test(correct = TRUE)` | `stats_verify::chi_square_2x2_yates` |
| every χ² p | the upper tail at its df | `stats_dist::chi_square_upper`, `stats_l0::pvalue::chi_square_upper` bit for bit, df to 2,401 |
| Fisher's exact test (2 × 2) | the hypergeometric probabilities of the first cell given the margins; less P(X ≤ a), greater P(X ≥ a), each its own tail; two-sided the tables no more probable than the observed one's × (1 + 1e-7) | `stats_dist::fisher_2x2` |
| phi, 2 × 2 | (n₁₁ n₂₂ − n₁₂ n₂₁) / √(R₁ R₂ C₁ C₂), signed | `stats_xtab` |
| phi, larger | √(χ² / n) | `stats_xtab` |
| Cramér's V | √(χ² / (n (min(r, c) − 1))) | `stats_xtab` |
| contingency coefficient | √(χ² / (χ² + n)) | `stats_xtab` |

Sources: K. Pearson (1900), Philosophical Magazine 50(302), 157–175; F. Yates
(1934), Supplement to the JRSS 1(2), 217–235; R. A. Fisher (1922), JRSS
85(1), 87–94; H. Cramér (1946), Mathematical Methods of Statistics, §21.9; A.
Agresti (2013), Categorical Data Analysis, 3rd ed., §2.4.5 for the adjusted
residual. Each formula is written from its published text; nothing is taken
from R's or any other program's source (ADR-008).

**The p of phi, V and C.** They carry no p of their own: the approximate p
SPSS reports beside them in its Symmetric Measures table is Pearson's p.

**Fisher's one-sided p.** Gaply gives both one-sided p values, less and
greater. SPSS gives one, "Exact Sig. (1-sided)", which is the less or the
greater one, whichever is in the direction of the data; with both, the reader
can take either.

**The expected counts' facts.** The result says how many cells have an
expected count below 5 (decided in whole numbers, Rᵢ Cⱼ < 5n, so it is exact),
of how many cells, and the smallest expected count, and carries the note
`expected_below_five` when any cell is below 5. No verdict: the result says
what the counts are, never whether a test holds.

**The tie rule's tolerance (a departure for approval).** R's `fisher.test`'s
results count as ties two tables equally probable in exact arithmetic and a
few units in the last place apart as doubles; Gaply counts a table whose
probability is at most the observed one's × (1 + 1e-7). R's help page states
no value, and nothing is taken from R's source, so the value is decision
576's, its effect fixed by `fisher_p.csv`. `fisher_two_sided_sum` is the rule,
tested on supplied probabilities. A two-sided p is clamped at 1 (R's itself
can print 1.0000000000000009).

## 3. The levels' order and the signs

Levels are in **code-point order** (decision 78), as every method's group is:
`stats_ttest::group_levels`. Upper case before lower case, letters outside
ASCII after both (`Zebra`, `apple`, `banana`, `Äpfel`, `Ölung`, `éclair`), and
a column of numbers lists 10 before 2 and −1 before −2. SPSS lists a numeric
column's values in ascending order. So:

- the sign of a 2 × 2 table's phi is that of n₁₁ n₂₂ − n₁₂ n₂₁ in Gaply's
  order: where SPSS's first row is 2 and Gaply's is 10, the two signs are
  opposite and the sizes equal;
- Fisher's less and greater swap in the same case.

The golden case `numeric_2_10` holds 2 and 10 and checks this: its phi is
−0.4629 with 10 on the first row. The limitation
`levels_in_code_point_order` says so on every result.

## 4. Exclusions and edge cases

A row is left out **once**, by the first reason found, the rows' column first:

1. the rows' cell is empty or a missing-value token: `missing_row`;
2. the rows' cell is a number equal to a code: `missing_code`, naming the
   rows' column;
3. the columns' cell is empty or a missing-value token: `missing_column`;
4. the columns' cell is a number equal to a code: `missing_code`, naming the
   columns' column.

One exclusion per code and column, each with its count and its column; a
reason with no row is not listed. Rows used plus rows left out are the rows
read.

The missing-value tokens are `stats_verify::is_missing`'s: empty, `NA`,
`N/A`, `NaN`, `NULL`, `NONE`, `.` and `#N/A`, in any case. So **`NaN` written
out is missing**, while an infinity written out (`Inf`, `-inf`) is a text level
like any other, and is kept (decision 575 said both were text levels; the
reading is the code's, `stats_l1/dataset.rs`).

Refusals, checked in this order by `validate` and `run` alike, each naming its
column by alias and its counts as numbers, never a label: the codes
(`missing_code_not_finite`, `missing_code_repeated`,
`missing_codes_out_of_order`); each column exists (`unknown_column`); not one
column twice (`same_column`); neither a score (`score_as_column`); each column
has at least 2 levels among the included rows (`too_few_levels {column,
count}`) and at most 50, `MAX_GROUP_LEVELS` (`too_many_levels {column, count,
limit}`). The levels are counted **among the included rows**, unlike the
ANOVA's group, which counts its whole column: a second level that is a code
leaves one. So a table has at most 2,500 cells and 2,401 df.

Not computable: none of the checks above leaves a statistic not finite, so
`overflow`, `arithmetic_refused` and `distribution_out_of_range` do not arise;
if one ever does, it is that code in place of the number, never a number.

## 5. Precision, the gates and the oracles

Every gate is ten times the largest error measured (decision 316), asserted on
every platform. Measured 9 Nov 2026, macOS aarch64.

**The distributions** (`tests/stats_dist_reference.rs` over
`make_xtab_dist_fixtures.R`'s `chi_p.csv` and `fisher_p.csv`, a pure relative
error, a row whose R p is below the smallest normal double not compared and
Gaply's below it too):

| function | rows | gate | measured |
|---|---|---|---|
| `chi_square_upper` against `pchisq(lower.tail = FALSE)` | 316, df 1 to 2,401, p down to 1e-300 | 2e-11 | 1.812e-12 (df 2,401) |
| `fisher_2x2` against `fisher.test` | 197 tables × 3 p, cells 0 to 99,000, 7 of a total of 100,000 | 4e-10 | 3.351e-11 |

**The method** (`tests/crosstabs_r_golden.rs`, the descriptor's validation
suite: 23 cases, 14 results and 9 refusals, 21,385 numbers; counts, df, labels
and exclusions exact; every p a pure relative error; every other number
relative below a floor of 1e-12):

| family | gate | measured |
|---|---|---|
| cells (expected, proportions, residuals) and the smallest expected | 4e-14 | 3.68e-15 |
| Pearson's χ² | 9e-15 | 8.78e-16 |
| likelihood ratio | 6e-12 | 5.68e-13 |
| continuity-corrected χ² | 1e-15 | 0 (every case bit for bit) |
| every p (χ², G², Yates, Fisher) | 8e-11 | 7.91e-12 |
| phi, V, C | 5e-15 | 4.05e-16 |
| `near_independent`'s statistics and residuals | 1.4e-7 | 1.36e-8 |

`near_independent` is n₁₁ n₂₂ − n₁₂ n₂₁ = 1 at n = 99,988: O − E is 1/n in
every cell and χ² and G² are 1.6e-14, where a sum of terms in doubles is
noise. R's values there are taken with O − E in whole numbers. Summed as
`stats_xtab` sums it, G² is −5.5e-12 before its clamp, so Gaply's G² is 0 and
its p 1: those two are checked exactly against R's 1.6e-14, not gated. That is
the case that shows the clamp acts.

**The oracles** (ADR-008, for fixtures only, nothing copied): base R 4.6.1's
`table`, `chisq.test` (expected counts, residuals, adjusted residuals, the
corrected statistic), `fisher.test`, `pchisq`; Pearson's χ² checked against
`chisq.test(correct = FALSE)` within 1e-9; G², phi, V and C from their formulas
on R's numbers (**the same formulas written twice**, a check of the
arithmetic). The generator is
`tests/fixtures/stats_xtab/make_xtab_fixtures.R`, run as
`/usr/local/bin/Rscript`; levels are set in code-point order there with
`sort(method = "radix")`, numeric levels plain integers only.

## 6. Speed

One check point, before the arithmetic: Cancel and the run manager's 30 s
ceiling act there. Fisher's probabilities are one pass over the possible
first cells. Measured in a release build (9 Nov 2026, a probe kept under
`~/gaply-corpus/os/phase8/8x1/`, seven runs each): a 50 × 50 table at 100,000
rows, median 32.9 ms; a 2 × 2 table at 100,000 rows, Fisher's test included,
median 13.8 ms. No timing test is in the suite.

## 7. The export

`stats_xtab::export::result_csv(result, manifest)` and `manifest_json`: one
long CSV, `table, statistic, row_index, row_level, column_index, column_level,
value, word`. Each cell's eight numbers row by row (observed, expected, the
three proportions, the three residuals); the row totals, the column totals, n,
the cell count, the cells below 5 and the smallest expected count; Pearson's,
the likelihood ratio's and, for a 2 × 2 table, the continuity-corrected
statistic, df and p, and Fisher's three p values; phi, V and C; then each row
left out as two lines, its count and the column it was found in; the note, the
limitations and the manifest's input, parameters and output hashes. Every
digit (Rust's shortest round-trip form); a value not computable is empty with
its code in `word`. The descriptive summary's field writer gives the one
formula-injection rule (a label `=lo` is written `'=lo`). Pinned by
`tests/xtab_export.rs`: read back field by field, a golden CSV and manifest on
macOS aarch64 and, since 8X-2 (decisions 586 and 592), Linux x86_64 checked
exactly against the same pin, measured outside this Mac at 1c6462a8 to give the
same bytes, as alpha's was (decision 389); every other platform checks that
pin's layout within 4 ulps; ten runs byte-identical.

## 8. What is not assessed, and the limitations

Every result carries `no_layers`, `fisher_two_by_two_only`,
`no_further_nominal_measures`, `no_weights` and `code_point_order`; the
descriptor lists the table limitations, the levels' reading and its own five
wording codes:

- two columns; a third, layer column is not offered;
- Fisher's exact test for a table of two rows and two columns only; no exact
  test for a larger table;
- no lambda, Goodman and Kruskal's tau, uncertainty coefficient,
  linear-by-linear test, or measures for ordered levels (gamma, Somers' d,
  Kendall's tau-b and tau-c);
- each row counts once; no weights;
- levels in code-point order (§3);
- the chi-square tests assume independent rows, which the data cannot show;
- how large a file opens depends on this computer's memory
  ([large_samples.md](large_samples.md) §8): 100,000 data rows and 128 columns
  on every computer.

The wording is neutral: no verdict word, no stars (decision 66).

The screen's sentence for each of crosstabs' 26 codes (`xtabWording.ts`,
8X-2), word for word; {column} is the column's name as the user sees it, and
{n}, {count}, {limit} and {value} are the code's numbers:

| code | sentence |
|---|---|
| unknown_capability | These settings name a capability Gaply does not have here. |
| capability_version_mismatch | These settings were made for version {found} of crosstabs, and this is version {expected}. |
| input_count | These settings name {count} files, and crosstabs reads exactly one. |
| input_mismatch | These settings were made for a different file from the one open. Open the file again. |
| preparation_mismatch | These settings do not match how the open data was prepared. Choose the settings again. |
| missing_code_not_finite | Missing code {n} is not a finite number. |
| missing_code_repeated | Missing code {n} is listed twice. |
| missing_codes_out_of_order | Missing code {n} is out of ascending order, so Gaply refuses these settings rather than reorder them. |
| unknown_column | Column {column} is not in this file. |
| same_column | The rows and the columns are the same column. Choose two different columns. |
| score_as_column | Column {column} is a score, so it cannot be the rows or the columns of a table. Choose a column from your file. |
| too_few_levels | Column {column} has {count} levels among the rows included, and a table needs at least two in each column. (1 level, at 1) |
| too_many_levels | Column {column} has {count} levels among the rows included, more than the {limit} a table takes in each column. |
| suspect_missing_code | Column {column} holds {value} in {count} cells, which may be a code for a missing value. It is counted as a level unless you tick it in Settings. (1 cell, at 1) |
| overflow | Not computable: the arithmetic went past the largest number a double can hold. |
| arithmetic_refused | Not computable: the arithmetic refused to give a value. |
| distribution_out_of_range | Not computable: this needs the chi-square distribution at more than 2,401 degrees of freedom, where Gaply has not measured its accuracy. |
| expected_below_five | Some expected counts are below 5, where the chi-square p values rest on a rougher approximation; for a table of two rows and two columns, Fisher's exact test does not rely on it. |
| missing_row | {count} rows had no value (an empty cell or a missing-value token) in {column}, the rows' column, and were left out. (1 row … was left out, at 1) |
| missing_column | {count} rows had no value (an empty cell or a missing-value token) in {column}, the columns' column, and were left out. (as missing_row) |
| missing_code | {count} rows held a missing code you ticked in {column}, and were left out. (as missing_row) |
| no_layers | Crosstabs takes two columns; a third, layer column is not offered. |
| fisher_two_by_two_only | Fisher's exact test is given for a table of two rows and two columns only; no exact test is given for a larger table. |
| no_further_nominal_measures | Lambda, Goodman and Kruskal's tau, the uncertainty coefficient, the linear-by-linear test and the measures for ordered levels are not given. |
| no_weights | Each row is counted once; weighting rows by a column is not offered. |
| code_point_order | Levels are listed in the order Gaply reads them, so a column of numbers lists 10 before 2; the sign of phi and the direction of Fisher's one-sided tests follow that order. |

Each code is `stats.xtab.` and the word in the first column. The card shows,
always, "{k} of {cells} cells have an expected count below 5; the smallest is
{smallest}." (decision 589), 0 included. The run manager's, the registry's and
the workspace's sentences are the one-way ANOVA's, but "run crosstabs again"
and "This run is not a crosstabs run, so this screen cannot read it." The
progress shows no steps sentence: a crosstab has one check point.

## 9. What SPSS documents, and where it differs

IBM's documentation, read 9 Nov 2026 as documentation, never a gate:
the CROSSTABS STATISTICS subcommand,
https://www.ibm.com/docs/en/spss-statistics/30.0.0?topic=crosstabs-statistics-subcommand-command,
and the Statistics Base 27 manual's Crosstabs pages (its search summary),
https://public.dhe.ibm.com/software/analytics/spss/documentation/statistics/27.0/en/client/Manuals/IBM_SPSS_Statistics_Base.pdf.
IBM's Algorithms PDF (SSLVMB_29) was too large to fetch, so SPSS's own
formulas were not read; the comparison below is of what SPSS reports.

| statistic | SPSS, as documented | Gaply |
|---|---|---|
| Pearson χ², likelihood ratio | under CHISQ | the same |
| linear-by-linear (Mantel–Haenszel) | under CHISQ, numeric variables only | not given |
| continuity correction, Fisher | "computed for all 2 × 2 tables" (v30); the v27 manual says Fisher when an expected count is below 5 | every 2 × 2 table |
| Fisher one-sided | one "Exact Sig. (1-sided)" | both, less and greater |
| phi, Cramér's V | PHI, Symmetric Measures | the same; phi signed for 2 × 2 |
| contingency coefficient | CC | the same |
| lambda, tau, uncertainty, gamma, d, tau-b, tau-c, eta, kappa, risk, McNemar, CMH | other keywords | not given |
| layers | further BY variables | not given |
| weights | WEIGHT BY; cells rounded or truncated before exact tests | not given |
| order of a numeric column | ascending | code point (§3) |

## 10. Blueprint §18.2 acceptance

| item | evidence | status |
|---|---|---|
| Formula/method and variant documented | §2, §3; `stats_xtab/mod.rs` and `stats_dist/mod.rs` headers | done |
| Golden fixtures and edge cases pass | `crosstabs_r_golden` (23 cases), `xtab_cases`, `xtab_export`, `stats_dist_reference` | done |
| Missing values, ties, weights, precision and sample size handled | §1, §4, §5, §6; weights not supported, stated | done |
| Diagnostic meanings and limitations documented | §4, §8; `XtabCode` docs; a sentence for each of the 48 codes the screen can show, crosstabs' own 26 listed in §8 (`xtabWording.ts`, `xtabCodes.json` pinned to Rust by `stats_xtab_wire.rs`, `xtabWording.vitest.ts`) | done |
| Independent reference comparison completed | R, §5 | done |
| Unsupported designs return explicit reasons | §4; each refusal's code through IPC (`stats_xtab_commands.rs`) | done |
| Result manifest and export verified | §7; through IPC the CSV equals the module's export of the run's result and manifest; the privacy matrix (`stats_xtab_privacy_matrix.rs`); since 8X-2 a crosstab saved to a project, closed and opened, its report's lines, and its rerun `Identical`, on a file's own table and on a prepared slot (`project_xtab.rs`), and the rerun privacy matrix's crosstabs section (`project_rerun_privacy_matrix.rs`) | done |
