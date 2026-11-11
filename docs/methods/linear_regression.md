# Linear regression, as an analysis

`stats.analyze.linear_regression`, version 1, engine version 1. Research OS,
Phase 9A-1 (decisions 669 to 676), `src-tauri/gaply-core/src/stats_linear_regression/`.

**Status: Beta.** 9A-1 built the core, its registration, its six commands and its
run kind; 9A-2 (decisions 683 to 690) made it usable. In the Statistics window,
Analyze, Regression, Linear… opens a dialog: the outcome from a list of the numeric
columns, scores included, and the predictors in two panes, in the order the user
chooses, each movable. Run adds a card to the Output with six tables (model summary,
analysis of variance, coefficients, correlations and collinearity, descriptive
statistics, residuals statistics), the note, the rows left out and the limitations.
A result can be exported, saved into a project, read back and run again; the report
names the outcome's column and the predictors by position in the order chosen.
Coming later no longer lists "Regression".

Ordinary least squares regression of one numeric outcome on one to 50 numeric
predictors, entered together and listwise, as SPSS's REGRESSION does with method
Enter and one block (§9 says what was read of SPSS's documentation). R's `lm` is
the oracle, and an exact rational solution of the normal equations (CRAN's gmp) is
a second one for the coefficients (§5).

## 1. Inputs and the spec

`LinearRegressionParams` (camelCase, unknown fields refused):

| field | meaning |
|---|---|
| `outcome` | one column alias |
| `predictors` | aliases in the user's order, 1 to `MAX_PREDICTORS` (50) |
| `missingCodes` | values treated as missing in the outcome and every predictor |

The checks, in this order (validate lists every error; a run reports the first):

1. a missing code that is NaN or infinite: `missing_code_not_finite` (index);
2. no predictor: `no_predictors`; more than 50: `too_many_predictors` (count, limit);
3. a column named twice, among the predictors or the outcome among them:
   `column_repeated`, naming the alias named the second time;
4. each column, the outcome first then the predictors: `unknown_column`,
   `column_non_finite` (count), `column_holds_text` (count);
5. a ticked code equal to a value of a chosen scale score: `code_equals_score`;
6. only with no other error, the first of: `no_complete_rows`; `too_few_rows`
   (count the complete rows, limit the predictors and two); `outcome_constant`;
   `predictor_constant` (the first constant predictor in the list's order).
   Constant is every value equal as doubles, −0 equal to 0, Level 1's rule.

A spec whose preparation does not describe the table is `preparation_mismatch`.
Any column may be a scale score. The run's own two refusals come after the checks:
`predictors_collinear`, naming the first predictor the QR's rank test stops at
(where R's `lm` gives NA), and `fit_out_of_range`, when the fit's sums of squares,
or any mean, variance or correlation of the complete rows, cannot be held in a
double. The run manager records a refusal from the run's work as a failed run
carrying that refusal (`runs.rs`'s worker), as for every method.

## 2. Formulas, and where each number comes from

**One fit (decision 670).** `stats_verify::ols_qr_parts` is the body of Level 1's
`ols_qr`, moved without changing a bit: Householder QR with an intercept column of
ones, LINPACK `dqrdc2`'s rank test at R's tolerance 1e-7 against each column's own
original norm (R 4.6.1's `lm.fit` and `qr` both default `tol` to 1e-7, printed in
this session). `ols_qr` is now a wrapper over it, its messages and bits unchanged
(`l1_r_golden`, `l1_precision_sweep` and its unit tests pass unedited). It returns
the coefficients (intercept first), R⁻¹, the residuals in row order, RSS, MSS
about the fitted values' mean, and df = n − k − 1.

With n the complete rows and k the predictors:

| number | formula |
|---|---|
| σ², σ | RSS / df, its root |
| SE of coefficient j | √(σ² s_j), s_j = Σ_m (R⁻¹)²_jm, as `ols_qr` computes it |
| t, p | b / SE; `stats_ttest::two_sided_p` over `stats_dist::t_two_tailed` at df |
| 95% interval | `stats_ttest::interval_95` at df |
| R², adjusted R², R | MSS / (MSS + RSS); 1 − (1 − R²)(n − 1) / df; √R² |
| ANOVA | regression SS = MSS on k df, residual SS = RSS on df, total MSS + RSS on n − 1; mean squares MSS / k and σ² |
| F, its p | (MSS / k) / σ²; `stats_dist::f_upper(F, k, df)` |
| column means and SDs | `stats_verify::mean`; √`stats_verify::sample_variance` |
| standardized coefficient | b_j sd_j / sd_y |
| zero-order correlation | `stats_verify::pearson_clamped(x_j, y)` |
| partial, part | t_j / √(t_j² + df); t_j √((RSS / (MSS + RSS)) / df) |
| VIF, tolerance | (n − 1) var_j · s_j; 1 / VIF; exactly 1 with one predictor |
| Durbin-Watson | Σ_{t ≥ 2} (e_t − e_{t−1})² / RSS, rows in the file's order |
| residuals' summary | the predicted values' and residuals' min, max, mean, SD (n − 1); the standardized predicted values' and standardized residuals' (e / σ) min and max |

The note `essentially_perfect_fit` is R 4.6.1's `summary.lm` rule, read in this
session: the residual variance is finite and below (mean(fitted)² +
var(fitted)) · 1e-30.

## 3. The predictors' order and the outcome

The coefficients, intervals and every per-predictor number follow the list's
order; reordering the predictors permutes them and changes nothing else
(`linear_regression_cases.rs`). The rows' order changes only the Durbin-Watson
statistic. Scaling a predictor by c divides its b by c and keeps its t, p and
standardized coefficient; adding c to the outcome moves the intercept, the
outcome's mean and the predicted values by c.

## 4. Exclusions and edge cases

A row is used only when the outcome and every predictor hold a number that is not
a ticked code. A row left out is counted once, under the first column without
one, the outcome first, as `missing_value` or `missing_code` with its column.
Suspect missing codes are warned of per column, a score's skipped.

Not computable, statistic by statistic, never a refusal of the run (decision 683):

* `perfect_fit`, RSS exactly 0 (an exact integer fit such as x = 0, 3, 4 and
  y = 2, 11, 14): σ, the residual mean square, every SE, t, p and interval, F
  and its p, the partial and part correlations, the Durbin-Watson statistic and
  the standardized residuals; R² and adjusted R² are 1, the coefficients,
  standardized coefficients, VIF and tolerance are computed. R also warns
  "essentially perfect fit" there, so the note is given too.
* `distribution_out_of_range`, where `stats_dist` returns no value (a df above
  its measured 1e7).
* `overflow` means too large only: a value along the way is not finite. Since
  9A-2 a value too small has its own code.
* `too_small`, where a statistic's own value is below the smallest normal double,
  so it cannot hold its precision: the residual and regression mean squares, an
  SE, F and its p, t and its p, VIF and tolerance where s_j is subnormal, and beta.
  Never b, a mean, a minimum or a maximum, which stay values. Where a product or
  quotient would be subnormal but its result is a normal double, it is computed in
  a form whose intermediates stay normal: σ as √RSS / √df, an SE as σ √s_j, F as
  (MSS / RSS)(df / k), each standard deviation from deviations scaled by the largest
  before squaring, the Durbin-Watson statistic from scaled residuals where its
  numerator overflows, and the partial correlation as sign(t) / √(1 + df / t²) where
  t² overflows. Two exceptions: an s_j that is itself subnormal withholds its
  coefficient's SE, t, p and interval and its VIF and tolerance as `too_small`, though
  σ √s_j could be normal (one predictor whose centred sum of squares lies between about
  4.5e307 and 1.8e308); and the split F's quotient MSS / RSS is not itself guarded,
  judged unreachable. In the normal range 9A-1's order is kept, so no golden moved.
  9A-1's hunt, rerun, has no withheld cell left: the 189 cells in 12 datasets where
  9A-1 gave `overflow` (and R's own SE is Inf, 0 or subnormal, the 189 rows D319 names)
  are now numbers within 5.022e-13 of gmp's exact values
  (`~/gaply-corpus/os/phase9/9a2/hunt/hunt_summary.md`).
* `predicted_no_spread`, for both standardized predicted values, where the predicted
  values' spread is at the level of their own rounding: MSS exactly 0, or
  s_ŷ ≤ 8 ε (m + κ s_y), with m the larger of |min ŷ| and |max ŷ|, s_y the outcome's
  SD and κ the largest over the predictors of √VIF_j √(1 + (n / (n − 1))(mean_j / sd_j)²).
  The constant 8 was measured: over 19,800 datasets whose exact covariance with every
  predictor is 0 the largest ratio was 3.635, and over 18,156 whose exact R² is 1e-12
  to 1e-10 with |mean y| at most 1e8 sd y the smallest was 45.86. b, beta, F and p stay
  numbers there, as R and SPSS give them.

Refused as `fit_out_of_range`, the run's own refusal: a fit whose sums of squares or
Householder norms underflow or overflow, at scales near 1e-140 or 1e-155 (the review of
9A-1, finding 3). `ols_qr` is shared with Level 1 and is not changed (decision 683, finding
3), so the regression cannot be computed at those scales; rescaling the data is the way
round it.

## 5. Precision, the gates and the oracles

**R's `lm` is the oracle** (`linear_regression_r_golden.rs` over
`tests/fixtures/stats_linear_regression/`, written by
`make_linear_regression_fixtures.R`, base R 4.6.1): 29 cases (14 results, 15
refusals), 2,497 numbers compared. The standardized coefficients, VIF and
tolerance, partial and part correlations, Durbin-Watson statistic and residual
summary are computed in R by a different route (auxiliary regressions,
`1 / (1 − R²)`, `diff`, `e / sigma`). Each gate is ten times its measured error:

| family | measured (worst) | gate |
|---|---|---|
| p, well-conditioned (pure relative) | 8.083e-11 (big_49 predictor 36) | 8.1e-10 |
| coefficient scale (relative, floor 1e-12) | 5.944e-12 (big_49 partial) | 6e-11 |
| residual scale (against sd(residuals)) | 8.855e-15 (big_49 residual max) | 8.9e-14 |
| p, ill-conditioned | 6.201e-9 (tolerance) | 6.3e-8 |
| coefficient scale, ill-conditioned | 2.551e-10 (offsets) | 2.6e-9 |
| residual scale, ill-conditioned | 6.123e-10 (tolerance) | 6.2e-9 |
| essentially perfect fit (coefficients against the largest b) | 5.551e-16 | 5.6e-15 |

In `essentially_perfect` only the coefficients, R², the note and the codes are
compared; its 65 residual-scale and derived keys are R's rounding noise and are
checked to exist only.

**The exact tier** (`make_linear_regression_exact.R`, CRAN gmp 0.7.5.1): for the
12 cases of up to 2,000 rows, each cell is taken as the double its text names
(R's own parser was off by an ulp on 624 cells and is corrected by exact
comparison), the normal equations are solved in exact rationals, and each of 86
coefficients is rounded once. Gaply's error and R's, each relative to the case's
largest |coefficient|:

| case | Gaply | R | gate |
|---|---|---|---|
| tolerance | 4.406e-11 | 7.452e-11 | 7.452e-10 |
| offsets | 2.043e-13 | 1.238e-12 | 1.238e-11 |
| correlated_99999 | 1.460e-13 | 3.936e-14 | 3.936e-13 |
| every other case | ≤ 6.5e-15 | ≤ 2.1e-14 | 1e-14 or ten times R's |

**F at the regression's degrees of freedom** (`stats_dist_reference.rs`,
`make_linreg_dist_fixtures.R`): 1,200 rows, df1 1 to 50, df2 1 to 1e5, upper
tails 0.9 down to 1e-100; worst 1.096e-10 relative (df1 49, df2 1e5, tail 1e-100),
gate 1.1e-9, decision 316's stop 1e-8 asserted too.

**The bug hunt** (seed 20260671, 240 seeded datasets, kept under
`~/gaply-corpus/os/phase9/9a1/hunt/`): 1 to 50 predictors, 3 to 2,000 rows and
four of 20,000, magnitudes 1e-100 to 1e100, offsets, near-collinearity, codes,
headers outside ASCII, permuted predictors. 220 compared (47,879 numbers), 5
refusals matched R's implied ones, 15 skipped within a factor of 10 of the rank
threshold. One bug, the subnormal SE (§4), fixed with a regression test. Two
differences shown to be R's by exact arithmetic (a VIF of 3.8e11, where R's
`1 / (1 − R²)` loses 2.6e-5 to cancellation and Gaply is within 5e-11), and 189
rows where Gaply gives `overflow` and R's own SE is Inf, 0 or subnormal.

## 6. Speed

Release, 100,000 rows, `compute` alone, median of seven (macOS aarch64, 11 Nov
2026): 1 predictor 6.38 ms, 10 predictors 36.01 ms, 50 predictors 350.70 ms;
the probe process's peak memory 52 MB, 187 MB and 806 MB (the whole process, the
dataset included). An outcome and 50 predictors at 100,000 rows is 5.1 million
cells, above the 8 GB tier's 5,000,000, so that size needs the 16 or 32 GB tier.

## 7. The export

`export.rs`: the long CSV in the eight fields every method's export has. A
column is its index in the model's list (the outcome 0, the predictors 1 to k)
and its alias, never a header; the related rank tests' export numbers its
columns by the user's list the same way. **The intercept's lines leave both
column fields empty.** A value not computable is an empty field with its code.
An exclusion's `column` parameter, in the result, the manifest and the CSV's exclusion
lines, is the alias's number (C3 is 3), not the model's column index. Pinned exactly on
macOS aarch64 and on Linux x86_64 (`linear_regression_export.rs`, `export_golden.csv`,
98 records, CRLF; decision 681); every other platform checks the pin's layout.

## 8. What is not assessed, and the limitations

No verdict, no "significant", no stars: every p is a number. The limitations, in
the result's order: every predictor entered at once (no stepwise methods or
blocks); no indicator coding of categorical predictors; no interaction or
polynomial terms; the intercept always; listwise deletion only; no
heteroscedasticity-consistent standard errors; no casewise diagnostics, plots or
saved columns; the Durbin-Watson statistic takes the file's row order; intervals
at 95% only; no weights. A fit at a scale whose sums of squares or Householder norms a
double cannot hold, near 1e-140 or 1e-155, is refused as `fit_out_of_range` (§4). Not assessed: linearity, normality of the residuals,
equal variance, independence, influence.

Every code's sentence, word for word, as `linearRegressionWording.ts` writes it (a
column is shown here as {column n}; the screen names it as the user sees it):

| code | sentence |
|---|---|
| `capability_version_mismatch` | These settings were made for version 2 of the linear regression, and this is version 1. |
| `code_equals_score` | Column {column 2} is a score, and it holds a missing code you ticked in 3 cells. Untick the code, or leave this score out. |
| `column_holds_text` | Column {column 2} holds text in 3 cells, so it cannot be used in the regression. |
| `column_non_finite` | Column {column 2} holds an infinity or "not a number" in 3 cells, so it cannot be used in the regression. |
| `column_repeated` | Column {column 2} is chosen more than once, as the outcome or among the predictors. Choose each column once. |
| `distribution_out_of_range` | Not computable: this needs a distribution at more than 10,000,000 degrees of freedom, where Gaply has not measured its accuracy. |
| `durbin_watson_row_order` | The Durbin-Watson statistic takes the rows in the file's order, so it describes serial correlation only when that order means something, such as time. |
| `enter_only` | Every chosen predictor is entered at once; stepwise, forward and backward methods, and blocks of predictors, are not offered. |
| `essentially_perfect_fit` | The residuals are at the level of rounding error, the case R's summary.lm calls an essentially perfect fit, so the standard errors, t, p and intervals, where they are given, rest on rounding. |
| `fit_out_of_range` | The values are too large or too small for the regression's sums of squares to be held, so it cannot be computed at this scale. |
| `input_count` | These settings name 3 files, and a linear regression reads exactly one. |
| `input_mismatch` | These settings were made for a different file from the one open. Open the file again. |
| `intercept_always` | The model always has an intercept; regression through the origin is not offered. |
| `intervals_95_only` | Confidence intervals are at 95%; other levels are not offered. |
| `listwise_only` | A row is used only when the outcome and every predictor have a value; pairwise deletion and mean substitution are not offered. |
| `missing_code` | 3 rows held a missing code you ticked in {column 2}, and were left out. |
| `missing_code_not_finite` | Missing code 1 is not a finite number. |
| `missing_value` | 3 rows had no value in {column 2} (an empty cell or a missing-value token), and were left out. |
| `no_casewise_diagnostics` | Casewise diagnostics, residual plots, influence statistics, and predicted values or residuals saved as columns are not given. |
| `no_categorical_predictors` | Each predictor is taken as a number; a categorical predictor is not coded into indicator columns for you. |
| `no_complete_rows` | No row has a value in the outcome and in every predictor, so there is nothing to fit. |
| `no_interaction_terms` | No interaction or polynomial terms are formed; only the columns you choose enter the model. |
| `no_predictors` | No predictor is chosen, and a linear regression needs at least one. |
| `no_robust_standard_errors` | Standard errors assume the residuals have equal variance; heteroscedasticity-consistent standard errors are not given. |
| `no_weights` | Each row is counted once; weighting rows by a column is not offered. |
| `outcome_constant` | The outcome takes one value in every complete row, so there is no variation to explain. |
| `overflow` | Not computable: a value along the way is too large for the arithmetic. |
| `perfect_fit` | Not computable: the model fits every row exactly, so there is no residual variation. |
| `predicted_no_spread` | Not computable: the predicted values do not vary, so they cannot be standardized. |
| `predictor_constant` | Column {column 2} takes one value in every complete row, so its effect cannot be estimated. Take it out of the predictors. |
| `predictors_collinear` | {column 2} is, within R's tolerance, a linear combination of the intercept and the predictors before it, so its effect cannot be separated from theirs. Take it out of the predictors. |
| `preparation_mismatch` | These settings do not match how the open data was prepared. Choose the settings again. |
| `suspect_missing_code` | Column {column 2} holds -99 in 3 cells, which may be a code for a missing value. It is counted as a number unless you tick it in Settings. |
| `too_few_rows` | 3 rows have a value in every chosen column, and a model with these predictors needs at least 5: the predictors, the intercept and one more. |
| `too_many_predictors` | 3 predictors chosen, and a linear regression takes at most 5. |
| `too_small` | Not computable: a value along the way is too small for the arithmetic to keep its precision. |
| `unknown_capability` | These settings name a capability Gaply does not have here. |
| `unknown_column` | Column {column 2} is not in this file. |

The run's two sentences that name the method: "This run is no longer held. Gaply keeps
the last 40 runs in memory only; run the linear regression again." and "This run is not
a linear regression, so this screen cannot read it." The run's other, the registry's and
the workspace's sentences are the related rank tests'.

## 9. What SPSS documents, and where it differs

Read on 11 Nov 2026 from IBM's SPSS Statistics 30.0.0 Command Syntax Reference
pages under
`https://www.ibm.com/docs/en/SSLVMB_30.0.0/statistics_reference_project_ddita/spss/base/`
(`syn_regression_method.html`, `_statistics.html`, `_missing.html`,
`_residuals.html`, `_origin_and_noorigin_s.html`, `_regwgt.html`) and
`https://www.ibm.com/docs/en/spss-statistics/30.0.0?topic=reference-regression`.
The `?topic=` subcommand pages returned only a title.

| item | SPSS (as read) | Gaply | status |
|---|---|---|---|
| ENTER | "All variables specified are entered in a single step in order of decreasing tolerance." | all at once, in the user's order | read |
| other methods, blocks | FORWARD, BACKWARD, STEPWISE, REMOVE, TEST; each METHOD applied to the previous one's equation | not offered | read |
| default statistics | R, ANOVA, COEFF and OUTS | model summary, ANOVA, coefficients; no OUTS | read |
| CI level | 95 by default, a value allowed (syntax diagram) | 95% only | read |
| ZPP, TOL | keywords in the syntax diagram; their wording not read | zero-order, partial, part; tolerance and VIF | not checked |
| missing values | LISTWISE by default; PAIRWISE, MEANSUBSTITUTION, INCLUDE | listwise only | read |
| intercept | NOORIGIN by default; ORIGIN suppresses the constant | always | read |
| Durbin-Watson | RESIDUALS DURBIN, in the Model Summary | always given | read |
| residual statistics table | "Descriptive statistics for ZRESID, RESID, PRED, and ZPRED are automatically displayed" (with RESIDUALS); its columns not read | min, max, mean, SD; standardized min and max | not checked |
| TOLERANCE criterion | 0.0001 by default (syntax diagram); its rule not read | R's 1e-7 rank test, a refusal naming the predictor | not checked |
| categorical predictors, interactions | not found on the pages read | neither | not checked |
| weights | REGWGT, weighted least squares | none | read |

## 10. Blueprint §18.2 acceptance

| item | evidence | status |
|---|---|---|
| Formula/method and variant documented | §2; the module's and `stats_verify`'s headers | done |
| Golden fixtures and edge cases pass | `linear_regression_r_golden` (2 tests: 29 cases, and the exact tier), `linear_regression_cases`, `linear_regression_export`, `stats_dist_reference` | done |
| Missing values, ties, weights, precision and sample size handled | §1, §4, §5, §6; weights not supported, stated | done |
| Result manifest and export verified | §7; through IPC the CSV equals the module's export (`stats_linear_regression_commands.rs`); `stats_linear_regression_privacy_matrix.rs` | done |
| A screen, saving and rerun | 9A-2: the dialog, the card, the stored checker, `project_linear_regression.rs`, the rerun privacy matrix's runs 22 to 25 | done |
