# Confirmatory factor analysis, as an analysis

`stats.analyze.cfa`, version 1, engine version 1. Research OS, Phase 9C-1 (decisions 743 to
751), `src-tauri/gaply-core/src/stats_cfa/`, with its distributions in
`src-tauri/gaply-core/src/stats_dist/` and its linear algebra in
`src-tauri/gaply-core/src/stats_linalg/`.

**Status: Beta, with no window yet.** 9C-1 built the core, registered it (Planned at its Part
2's end, decision 747) and made it Beta with six commands and a run kind (decision 749). No
window item reaches it until 9C-2 adds the dialog, the card, saving and rerun; the Statistics
window's Coming later list still says "CFA and SEM", and a saved project cannot hold a run of it
(the project's methods table has no row for it yet). 9C-3 and 9C-4 extend the model to
structural equation models.

The goal is AMOS's numbers (decision 743): the model fitted as AMOS fits a drawn measurement
model, its estimates, standard errors, critical ratios and fit measures as AMOS prints them.

## 1. Inputs and the spec

`CfaParams`, camelCase, unknown fields refused:

| Field | Meaning |
|---|---|
| `indicators` | 1 to 16 factors, each a list of 2 or more column aliases, its columns in the dataset's order; the factors in the order of their first columns; every alias in exactly one factor; 3 to 128 aliases in all. |
| `correlatedFactors` | Whether the factors covary (true in 9C-2's window, as lavaan's `cfa` does and as an AMOS user draws a model). |
| `missingCodes` | Finite, ascending, each once; applied to every column. |

Factors are numbered 1 to k. No name a user typed is in the spec, the result or a log
(ADR-011): naming factors is a later decision.

The checks, in this order: the missing codes; the factors' count (`no_factors`,
`too_many_factors`); each factor's columns (`factor_too_few_items`, naming the factor's number);
the columns' count (`too_few_items`, `too_many_items`); their order within a factor
(`items_out_of_order`); the factors' order (`factors_out_of_order`); a column named twice, within
a factor or across two (`item_repeated`); then each column (`unknown_column`,
`column_non_finite`, `column_holds_text`) and a code equal to a score's value
(`code_equals_score`). Only when there is no other error, the first of: `no_complete_rows`;
`too_few_rows` (fewer than p + 1, so S could not be positive definite); `item_constant`. A scale
score may be a column like any other.

Rows: listwise over every column, codes as missing. A row left out is counted once, under the
first column in the dataset's order without a value, as `missing_value` or `missing_code`.

## 2. Formulas, and where each number comes from

**The model.** Column i measures its factor f(i): x_i = λ_i ξ_f(i) + e_i. The first column of
each factor has λ = 1. Free: the other loadings, each error variance θ_ii, each factor variance
φ_ff and, when the factors covary, each covariance φ_fg (otherwise 0). No means. Σ(θ) = Λ Φ Λᵀ +
Θ. NPAR q = (p − k) + p + k (+ k (k − 1) / 2 when correlated); DF d = p (p + 1) / 2 − q.

**S** is the covariance matrix of the complete rows with divisor N (AMOS's maximum likelihood
estimate), from two-pass means in a fixed order. Its Cholesky factor (the 1e-9 rule of
`stats_linalg`) refuses the run as `sample_covariance_singular`, naming the column, when a column
is within the arithmetic of a combination of the columns before it. A column whose squares
overflow refuses there too.

**The discrepancy** F(θ) = ln|Σ(θ)| + tr(S Σ(θ)⁻¹) − ln|S| − p is minimised by Fisher scoring.
The starts: φ_ff = s_mm / 2 for the factor's first column m; each other loading s_im / φ_ff; θ_ii
= s_ii / 2; φ_fg = s_{m_f m_g} / 2. If a start loading is exactly 0 and the information fails at
the start, the run starts once more with every free loading 1 (reached in `cfa_cases.rs` by two
factors of two columns, the second exactly uncorrelated with its marker). Each step is δ = −H⁻¹ g with g_a =
tr(Σ⁻¹ (Σ − S) Σ⁻¹ Σ_a) and H_ab = tr(Σ⁻¹ Σ_a Σ⁻¹ Σ_b); each Σ_a is one or two outer products, so
each entry of H is a product of two bilinear forms in Σ⁻¹, Σ⁻¹Λ and ΛᵀΣ⁻¹Λ and no duplication
matrix is formed. θ moves by α δ, α = 1 first, halved up to 30 times until Σ is positive
definite (`cholesky_within(Σ, 0)`) and, while the Newton decrement gᵀH⁻¹g is above 1e-10, F does
not increase. The run stops when the decrement is at most 1e-20, which does not depend on the
data's units; it refuses as `cfa_not_converged` (with its limit) after 500 iterations or when no
step passes, and as `model_not_identified` when H fails its Cholesky factor (the 1e-9 rule) at
any iteration. A model with more parameters than moments is refused with that count; the checks
make it unreachable.

**The statistics.**

| Statistic | Formula |
|---|---|
| Standard error | √ diag [(N − 1) H / 2]⁻¹ at the solution: the expected information at Σ̂ (AMOS) |
| C.R. | estimate / standard error |
| P | two-tailed standard normal of C.R. |
| Standardized weight | λ_i √φ_ff / √σ̂_ii |
| Correlation | φ_fg / √(φ_ff φ_gg) |
| SMC | 1 − θ_ii / σ̂_ii, given whenever σ̂_ii > 0, above 1 when θ_ii < 0 |

At the solution σ̂_ii is s_ii to rounding: R's own scoring refined to 1e-20 gives them within
2.361e-13 relative over the golden and 1.875e-12 over the hunt's 155 refined solutions, and
`cfa_cases.rs` pins it on a model that fits and one that does not. So reading s_ii for σ̂_ii in
the two rows above changes nothing measurable, and no test can tell the two apart; the formulas
are AMOS's, with σ̂_ii.

**The fit,** as AMOS's Appendix C, with C = CMIN, d = DF, q = NPAR, n = N − 1, p* = p (p + 1) / 2,
b the independence model (only the variances free: q_b = p, d_b = p (p − 1) / 2, C_b = n (Σ ln
s_ii − ln|S|)), and δ_L, δ_U the noncentrality at which the noncentral chi-square's lower tail at
C is 0.95 and 0.05:

| Measure | Formula |
|---|---|
| CMIN | (N − 1) · max(F, 0): at 0 degrees of freedom the minimum is rounding of either sign |
| P | the chi-square's upper tail at C with d degrees of freedom |
| CMIN/DF | C / d |
| RMR | √(Σ_{i≤j} (s_ij − σ̂_ij)² / p*), on S with divisor N |
| GFI, AGFI, PGFI | 1 − tr[(Σ̂⁻¹S − I)²] / tr[(Σ̂⁻¹S)²]; 1 − (p*/d)(1 − GFI); (d/p*) GFI |
| NFI, RFI, IFI, TLI | 1 − C/C_b; 1 − (C/d)/(C_b/d_b); (C_b − C)/(C_b − d); (C_b/d_b − C/d)/(C_b/d_b − 1) |
| CFI | 1 − max(C − d, 0) / max(C_b − d_b, C − d, 0), 1 when that denominator is 0 |
| PRATIO, PNFI, PCFI | d/d_b; NFI · PRATIO; CFI · PRATIO |
| NCP (LO 90, HI 90) | max(C − d, 0) (δ_L, δ_U) |
| FMIN, F0 (LO 90, HI 90) | C/n; max(C − d, 0)/n (δ_L/n, δ_U/n) |
| RMSEA (LO 90, HI 90) | √(F0/d) (√(δ_L/(n d)), √(δ_U/(n d))) |
| PCLOSE | the noncentral chi-square's upper tail at C with ncp 0.0025 n d |
| AIC, BCC, BIC, CAIC | C + 2q; C + 2q (N − 1)/(N − p − 2); C + q ln N; C + q (ln N + 1) |
| ECVI (LO 90, HI 90), MECVI | (C + 2q)/n ((δ_L + d + 2q)/n, (δ_U + d + 2q)/n); BCC/n |
| HOELTER .05, .01 | ⌊n · χ²_{1−α}(d) / C + 1⌋ |

TLI and IFI are not truncated at 1. The independence model's NPAR, CMIN and DF are given too.

**The distributions** (decision 744): the chi-square's upper tail is Level 0's
(`chi_square_upper_wide`, statrs), and its upper quantile a bisection on it to a relative 1e-14.
The noncentral chi-square's two tails are the Poisson mixture Σ_j Pois(j; λ) P(d/2 + j, x/2),
λ = ncp/2, each tail on its own: the Poisson weights and the incomplete gamma's terms e^{−y} y^b /
Γ(b + 1) by Loader's saddle-point form (an exact product for a whole or half-whole b up to 15),
each tail's terms by the incomplete gamma's recurrence in the direction in which it only adds,
started from the incomplete gamma's series or continued fraction 3 √y clear of y = b where its
start is near there, the range reaching from the Poisson mode outward until a bound on what is
left is below 1e-17 of the sum, and the terms summed from the mode outward with compensation. The
ncp at which the lower tail takes a value is a bisection to a relative 1e-12.

## 3. Notes and not-computable values

Notes are bare codes, in this order, with no verdict: `negative_variance` (a variance estimate
below 0, as AMOS's "The following variances are negative." and "This solution is not
admissible."), `factor_covariance_not_positive_definite`, `df_zero`.

Not computable, statistic by statistic: `fixed_parameter` (the first loadings' standard errors,
C.R.s and Ps); `df_zero` (P, CMIN/DF, AGFI, PGFI, RFI, TLI, PRATIO, PNFI, PCFI, the intervals of
NCP, F0, RMSEA and ECVI, RMSEA, PCLOSE and HOELTER when d is 0); `cmin_zero` (HOELTER when C is
0); `too_few_rows_for_bcc` (BCC and MECVI when N ≤ p + 2); `negative_variance` (a standardized
weight or correlation whose square root meets a variance at or below 0); `overflow`; `too_small`
(a 0/0 of quantities that vanished; a value below the smallest normal double is given as a value,
as the hunt's P of 4.4e-323 was); and `distribution_out_of_range`.

A factor variance below 0 is reached through the estimation, though rarely: the starts give each
factor s_mm / 2 > 0, and on two datasets of 13 Nov 2026 whose solution has one the path met a
singular information matrix first and the run refused as `model_not_identified`; but on the
hunt's h043 the run ends at φ11 = −2,512 after 102 iterations (an earlier version of this page
said it was not reached; the review of 9C-1 found it). There Gaply's minimum is lower than
lavaan's: R evaluates (N − 1) F at Gaply's estimates as 85.0684066616, Gaply's CMIN to twelve
digits, against F 1.273 at lavaan's solution, whose error variance is −2,480 instead. R's own
scoring could not refine it, so its other numbers are not compared. The standardized weight's and
the correlation's `negative_variance` are checked on a solution built for them
(`stats_cfa::tests`). `distribution_out_of_range` is not reachable within
the limits: d is at most p (p + 1) / 2 − 2p, 8,000 at 128 columns, within the chi-square's 8,128,
and C is finite.

No word anywhere says a fit is good, poor, close, acceptable or adequate, and no cutoff (such as
RMSEA .05 or CFI .95) is named.

## 4. The check points

One before the arithmetic, one before each column of S, one before the estimation (every
iteration calls it again at its unchanged count) and one before the fit: 1 + p + 2. The run
manager reads the cancel flag and the clock at each call, so a cancel or the 30 s ceiling stops a
run during its iterations (`stats_cfa_commands.rs`).

## 5. Precision, the gates and the oracles

**The R golden** (`tests/fixtures/stats_cfa/make_cfa_fixtures.R`, lavaan 0.7-3, compared in
`tests/cfa_r_golden.rs`): 14 cases (Example 8; Holzinger and Swineford's 301 rows, three factors
correlated and not; one factor of three columns at DF 0; two correlated factors of two columns at
DF 1; a Heywood case; a factor covariance matrix not positive definite; a model not identified; a
singular S; a model worse than the independence model; missing values and codes; a score as a
column; 30 columns on 6 factors and 128 on 16 by a formula identical in R and Rust), 3,541
values. Each case is fitted by lavaan with likelihood "normal", expected information and
structured h1 information, then refined by the script's own Fisher scoring, written from
decision 743's formulas and not from Gaply's code, to a decrement of 1e-20; every statistic is
computed there. lavaan, refitted at the refined estimates, agrees on the standard errors (times
√(N/(N − 1))) within 2.04e-13, std.all and R² exactly; lavaan's "wishart" fit agrees on its fit
measures within 6.71e-7 and its RMSEA interval within 1e-4. Two lavaan 0.7-3 behaviours differ
from the 0.6-17 decision 743 measured: its `gfi` entry reads 1.000 beside an AGFI and PGFI that
come from a GFI of 0.9659 (so `gfi` is not compared; AGFI and PGFI carry it), and its R² is NA
where an error variance is below 0.

| Family | Worst measured | Gate (ten times, rounded up) |
|---|---|---|
| estimates | 2.017e-9 | 2.1e-8 |
| standard errors | 1.235e-9 | 1.3e-8 |
| C.R.s | 2.831e-9 | 2.9e-8 |
| P (relative over max(1, C.R.²)) | 2.148e-9 | 2.2e-8 |
| standardized values, correlations, SMCs | 3.279e-9 | 3.3e-8 |
| CMIN and what is built on it | 2.124e-12 | 2.2e-11 |
| indices | 1.574e-12 | 1.6e-11 |
| interval measures and PCLOSE | 6.643e-13 | 6.7e-12 |
| RMR, of the largest fitted variance | 9.298e-12 | 9.3e-11 |

The worst estimates are in the case whose likelihood is nearly flat (a model worse than the
independence model); every other case agrees within 1e-10. Where both sides of a P are below the
smallest normal double, or both CMIN-type values within (N − 1)(p + 1) · 1e-13 of 0, they agree at
rounding.

**The AMOS tier** (`tests/cfa_amos.rs`): Example 8 from its data file; every number the guide
prints (13 estimates, 13 standard errors, 13 C.R.s, 6 standardized weights, the correlation, 6
SMCs, CMIN 7.853, DF 8, P .448, NPAR 13) equals Gaply's rounded to its printed decimals. There is
no gate.

**The distributions** (`tests/stats_dist_reference.rs`): R's `pchisq` with an ncp over 598 rows,
absolute worst 3.317e-12 (gate 3.4e-11) and relative 1.084e-10 where R's tail is at least 1e-4
(gate 1.1e-9); a Poisson mixture of `pgamma`'s tails where `pchisq` cannot reach (ncp 1e4 to 2e6,
135 rows), worst 1.360e-14 (gate 1e-8); `qchisq`, 18 rows, worst 1.607e-14 (gate 1.7e-13); the
RMSEA interval's ncp against `uniroot`, 30 pairs, worst 4.485e-13 (gate 1e-10). The two tails sum
to 1 within 3.5 ε over 1,772 points where neither is below 1e-3.

**The bug hunt** (seed 20260746, 260 datasets of 3 to 128 columns, 1 to 16 factors, rows from p
+ 2 to 5,000, Heywood-prone, nearly singular, misspecified, with codes and missing values,
headers outside ASCII on every third): 159 results, 46,076 values compared; 35 refusals agreed
with R's. On the 57 datasets R could not refine itself, Gaply refused 56 (40
`model_not_identified`, 16 `cfa_not_converged`) and gave a result on one, h043, checked only
by R's value of its discrepancy (section 3's note on factor variances below 0). On nine datasets R reached a solution from lavaan's start where Gaply refuses
(seven `cfa_not_converged` at 500 iterations, two `model_not_identified`), and on each R's own
Fisher scoring from Gaply's starts does not reach it either (on h022 its step shrinks to 6e-8
with the decrement near 1). Five of those solutions are improper, an error variance below 0 or
a factor covariance matrix not positive definite (h041, h078, h123, h139, h218); four are proper
but on the boundary, an error variance between 6.4e-7 and 8.5e-6 (h022, h120, h211, h229). So
decision 745's starts and Fisher scoring do not reach 1e-20 in every case, as that decision
said: seven of the hunt's datasets stop at the 500-iteration cap. Values above the golden's
gates on the hunt's harder data: indices up to 9.479e-11, interval measures up to 1.308e-11
(decision 744's gate for the ncp itself is 1e-10), the ncp and F0 at 2.486e-11 (h232, gate
2.2e-11), and two error variances, one of 1.4e-6 at 4.527e-8 relative (6.3e-14 absolute, h053)
and one of 0.188 at 2.165e-8 (h217, gate 2.1e-8).

## 6. Speed and size

In release on 100,000 rows, correlated, seven runs: 12 columns on 3 factors in 0.022 s (8
iterations), 60 on 10 in 0.337 s (10), 128 on 16 in 1.526 s (10); peak memory, the dataset
included, 181 MB, 858 MB and 1.32 GB. A result of 128 columns and 16 correlated factors
serialises to 83,233 bytes (under 1 MiB, `cfa_cases.rs`).

## 7. The export

A long CSV in the eight fields every method's export has: the rows used and the iterations;
each loading (estimate, standard error, C.R., P, standardized weight) by column index, alias and
factor number; each factor variance by factor; each covariance by its two factors with its
correlation; each error variance and SMC by column; every fit measure; the independence model's
three; the rows left out; the notes; the limitations; the manifest's hashes. Pinned on macOS
aarch64 (`tests/cfa_export.rs`, `export_golden.csv`); every other platform checks the pin's
layout until a Linux run gives its bytes.

## 8. Limitations

| Code | Sentence |
|---|---|
| `cfa_ml_only` | Maximum likelihood is the only estimator; generalized and unweighted least squares, asymptotically distribution-free and Bayesian estimation are not offered. |
| `cfa_no_means` | Means and intercepts are not estimated. |
| `cfa_one_group` | One group is analysed; multiple-group models are not offered. |
| `cfa_simple_structure` | Each column measures one factor; cross-loadings and correlated errors are not offered. |
| `cfa_marker_identification` | Each factor's scale is set by fixing its first column's loading to 1, as AMOS does when a model is drawn; fixing a factor's variance to 1 instead is not offered. |
| `cfa_no_modification_indices` | Modification indices are not computed. |
| `cfa_normal_theory` | The standard errors and the chi-square assume multivariate normality; bootstrap and robust corrections are not offered. |
| `cfa_no_srmr` | The standardized RMR is not computed; AMOS gives it only through a plugin. |
| `cfa_listwise_only` | A row is used only when every column in the model has a value; full information maximum likelihood is not offered. |
| `no_weights` | Each row is counted once; weighting rows by a column is not offered. |

The registry lists the table capabilities' `text_files_utf8_only` and `limits_by_memory` before
these. The result's own limitation codes (`ml_only`, `no_means`, `one_group_only`,
`simple_structure_only`, `marker_identification_only`, `no_modification_indices`,
`normal_theory_standard_errors`, `no_srmr`, `listwise_only`, `no_weights`) map one to one to these
ten.

## 9. What follows AMOS, and how it was checked

Each item is marked **reproduced** from AMOS's printed output (with its example), **formula**
(taken from AMOS's documented formula, not checked against printed output), or **not checked**.
The guide is the IBM SPSS Amos 22 User's Guide; its page numbers were not read in this session
except where given.

| AMOS's behaviour | Gaply | Mark |
|---|---|---|
| S with divisor N (the maximum likelihood estimate); unbiased input converted (Example 16; the FitUnbiasedMoments help) | the same | reproduced (Example 8's estimates) |
| CMIN = (N − 1) F at the minimum (Appendix A) | the same, F clamped at 0 | reproduced (Example 8: 7.853, DF 8, P .448) |
| Standard errors from the expected information at Σ̂, scaled by N − 1 | the same | reproduced (Example 8: 13 of 13) |
| C.R. and its two-tailed normal P (the guide, p. 30) | the same | reproduced (Example 8's C.R.s) |
| Standardized weights, correlations, SMCs | the same | reproduced (Example 8) |
| NPAR, RMR, GFI, AGFI, PGFI, HOELTER and NFI (Appendix C) | the same formulas | reproduced by decision 743's research (Example 6, Wheaton: all 40 values, NFI its one); not rerun against Example 6 in this session |
| RFI, IFI, TLI, CFI, PRATIO, PNFI, PCFI, NCP, F0, RMSEA and its interval, PCLOSE, ECVI, MECVI (Appendix C) | the same formulas | formula; each agrees with lavaan's own where lavaan defines it the same way |
| CFI's denominator max(C_b − d_b, C − d, 0) | the same | formula; it differs from max(C_b − d_b, 0) only where C − d is above both C_b − d_b and 0, which needs no model worse than the independence model, since d is below d_b: `cfa_cases.rs` reaches it on six independent columns (C − d 4.135, C_b − d_b 4.036, CFI 0); the golden case named `worse_than_independence` does not (C − d is −0.65, CFI 1) |
| BCC, BIC, CAIC | C + 2q (N − 1)/(N − p − 2); C + q ln N; C + q (ln N + 1) | not checked against printed values; AMOS's BIC help says BIC penalises more than CAIC, while these formulas give CAIC = BIC + q |
| Each factor's first regression weight fixed at 1, each error's at 1, variances free; no means | the same | reproduced (Example 8's model) |
| AMOS's stop: every first derivative below 1e-5 and a last step below 1e-3 | a Newton decrement of 1e-20, far tighter, so printed values agree | not checked against AMOS's iterations |
| Estimation from AMOS's own starts | decision 745's starts and Fisher scoring | not checked: on nine of the hunt's datasets, five with improper solutions and four on the boundary, Gaply refuses where lavaan, and so presumably AMOS, gives numbers |
| Negative variances: "The following variances are negative." and "This solution is not admissible." | the `negative_variance` note; the numbers are given | formula (the sentences quoted in decision 743) |
| Standardized RMR through a plugin | not computed | not checked |

## 10. Blueprint §18.2 acceptance

Not applicable until 9C-2 puts the method in the window; 9C-1 is the core, the commands and the
runs.
