# Linear regression, as an analysis

`stats.analyze.linear_regression`, version 1, engine version 1. Research OS,
Phase 9A-1 (decisions 669 to 676), `src-tauri/gaply-core/src/stats_linear_regression/`.

**Status: Beta, with no screen.** 9A-1 built the core, its registration, its six
commands and its run kind; 9A-2 adds the dialog, the card, saving and rerun. Until
then the old home lists it on its roadmap, Help lists its limitations, and the
Statistics window's Coming later still says "Regression" (decision 674's
departure).

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

Not computable, statistic by statistic, never a refusal of the run:

* `perfect_fit`, RSS exactly 0 (an exact integer fit such as x = 0, 3, 4 and
  y = 2, 11, 14): σ, the residual mean square, every SE, t, p and interval, F
  and its p, the partial and part correlations, the Durbin-Watson statistic and
  the standardized residuals; R² and adjusted R² are 1, the coefficients,
  standardized coefficients, VIF and tolerance are computed. R also warns
  "essentially perfect fit" there, so the note is given too.
* `distribution_out_of_range`, where `stats_dist` returns no value (a df above
  its measured 1e7).
* `overflow`, where a value is not finite, **and where σ² s_j or s_j falls below
  the smallest normal double**: a subnormal keeps a few significant bits, so its
  root is a finite, wrong SE. The bug hunt found this (§5): on a predictor
  scaled to 1e93 with a small outcome, R's `lm` printed t −8.70 where the exact t
  is −12.31; Gaply gives `overflow` there, and the partial and part correlations
  built on t with it. The same guard applies to VIF's s_j.

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
Pinned on macOS aarch64 (`linear_regression_export.rs`, `export_golden.csv`,
98 records, CRLF); every other platform checks the pin's layout until a Linux run
measures it.

## 8. What is not assessed, and the limitations

No verdict, no "significant", no stars: every p is a number. The limitations, in
the result's order: every predictor entered at once (no stepwise methods or
blocks); no indicator coding of categorical predictors; no interaction or
polynomial terms; the intercept always; listwise deletion only; no
heteroscedasticity-consistent standard errors; no casewise diagnostics, plots or
saved columns; the Durbin-Watson statistic takes the file's row order; intervals
at 95% only; no weights. Not assessed: linearity, normality of the residuals,
equal variance, independence, influence.

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
| A screen, saving and rerun | 9A-2 | not yet |
