# Confirmatory factor analysis, as an analysis

`stats.analyze.cfa`, version 1, engine version 1. Research OS, Phase 9C-1 (decisions 743 to
751) and Phase 9C-2 (decisions 758 to 765), `src-tauri/gaply-core/src/stats_cfa/`, with its
distributions in `src-tauri/gaply-core/src/stats_dist/` and its linear algebra in
`src-tauri/gaply-core/src/stats_linalg/`.

**Status: Beta, with no window yet.** 9C-1 built the core, registered it (Planned at its Part
2's end, decision 747) and made it Beta with six commands and a run kind (decision 749). 9C-2
fixed 9C-1's review (decision 761), made the model's checks and refusals say what happened
(decision 762) and gave each model-fit table AMOS's three rows (decision 763). No window item
reaches it until 9C-3 adds the dialog, the card, saving and rerun; the Statistics window's Coming
later list still says "CFA and SEM", and a saved project cannot hold a run of it (the project's
methods table has no row for it yet). 9C-4 builds the structural equation model's engine, with
its general optimiser, and 9C-5 its window and path diagram.

The goal is AMOS's numbers (decision 743): the model fitted as AMOS fits a drawn measurement
model, its estimates, standard errors, critical ratios and fit measures as AMOS prints them.

## 1. Inputs and the spec

`CfaParams`, camelCase, unknown fields refused:

| Field | Meaning |
|---|---|
| `indicators` | 1 to 16 factors, each a list of 2 or more column aliases, its columns in the dataset's order; the factors in the order of their first columns; every alias in exactly one factor; 3 to 128 aliases in all. |
| `correlatedFactors` | Whether the factors covary (true in 9C-3's window, as lavaan's `cfa` does and as an AMOS user draws a model). |
| `missingCodes` | Finite, ascending, each once; applied to every column. |

Factors are numbered 1 to k. No name a user typed is in the spec, the result or a log
(ADR-011): naming factors is a later decision.

The checks, in this order: the missing codes; the factors' count (`no_factors`,
`too_many_factors`); each factor's columns (`factor_too_few_items`, naming the factor's number);
with the factors uncorrelated and more than one factor, each factor of exactly two columns
(`two_item_factor_uncorrelated`, naming the factor's number: on its own such a factor is a
one-factor model of 3 moments and 4 parameters, so no data identify it; decisions 760 and 762);
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
is within the arithmetic of a combination of the columns before it. Before it, a mean or an entry
of S that is not finite refuses the run as `overflow`, naming the first column in the spec's
order whose mean or variance is not finite, or else the later column of the first covariance
that is not, the lower triangle read row by row (decision 762; 9C-1 refused a column times 1e300
as a singular S). Since Σ ab ≤ max(Σ a², Σ b²), a covariance overflows beside two finite variances
only by rounding at the edge: a seeded search of 2,000,000 columns near √(MAX / 4) found 82 such,
and `cfa_cases.rs` holds the first.

**The discrepancy** F(θ) = ln|Σ(θ)| + tr(S Σ(θ)⁻¹) − ln|S| − p is minimised by Fisher scoring.
The starts: φ_ff = s_mm / 2 for the factor's first column m; each other loading s_im / φ_ff; θ_ii
= s_ii / 2; φ_fg = s_{m_f m_g} / 2. If a start loading is exactly 0 and the information fails at
the start, the run starts once more with every free loading 1 (reached in `cfa_cases.rs` by two
factors of two columns, the second exactly uncorrelated with its marker). Each step is δ = −H⁻¹ g with g_a =
tr(Σ⁻¹ (Σ − S) Σ⁻¹ Σ_a) and H_ab = tr(Σ⁻¹ Σ_a Σ⁻¹ Σ_b); each Σ_a is one or two outer products, so
each entry of H is a product of two bilinear forms in Σ⁻¹, Σ⁻¹Λ and ΛᵀΣ⁻¹Λ and no duplication
matrix is formed. θ moves by α δ, α = 1 first, halved up to 30 times until Σ is positive
definite (`cholesky_within(Σ, 0)`) and, while the Newton decrement gᵀH⁻¹g is above 1e-10, F does
not increase. The attempt stops when the decrement is at most 1e-20, which does not depend on
the data's units. It ends without a solution after 500 iterations, when no step passes, when Σ
is not positive definite at a point, or when H fails its Cholesky factor (the 1e-9 rule).

**A second attempt** (decision 761). When decision 745's start (and its restart from loadings of
1) ends without a solution, one more attempt starts from the triad starts: for a factor of three
or more columns, m its first and j and k its second and third, t = s_mj s_mk / s_jk, and φ_ff = t
when t is finite, above 0 and below s_mm, otherwise s_mm / 2; for a factor of two columns s_mm /
2; the rest as decision 745's. It has the same cap and check point; a run the first start solves
never makes it, so no result given before changed. The run reports the iterations of the attempt
that succeeded. On data drawn from uncorrelated models of 4 to 8 factors (decision 761's search,
seeds 1 to 300) 7bafd6df refused 66 and the second attempt solves 64; on 9C-1's hunt it solves 9
of its refusals (section 5).

**The run's refusal** when every attempt ends without a solution (decision 762) is chosen over
the attempts whose start has Σ positive definite: `model_not_identified` only when each of them
failed the 1e-9 rule at its start, and `cfa_not_converged` otherwise, with `count` the most
iterations any of them made and `limit` the cap (with `count` 0 when no start has Σ positive
definite). H failing the rule after the start is not `model_not_identified`: the model was
identified where the attempt started (`cfa_cases.rs` holds h098, both attempts failing at
iteration 3). No dataset of either hunt now fails the rule at the start of every attempt, but
small exact data do: columns of Walsh ±1 patterns, two correlated factors of two columns, reach
it on 1,941 of 20,000 seeded trials, and `cfa_cases.rs` holds one; `stats_cfa`'s unit tests hold
the rule's every case. A model with
more parameters than moments is refused as `model_not_identified` with that count; the checks
make it unreachable.

**What the second attempt does not reach** (decision 760, measured outside the Mac on 13 Nov 2026
with Gaply's estimation copied into R in its operation order, on 287 seeded datasets of 1 to 4
factors of 2 to 5 columns): of the copy's 85 refusals lavaan converged on 41; 17 of those are
uncorrelated models with a factor of two columns, which no data identify (now refused at the
check); 24 are identified at lavaan's solution, 15 stopping at the cap and 9 failing the rule
after the start. lavaan's default starts as a second attempt rescued 5 of the 41, damping H
(Levenberg–Marquardt) 11 of the 24, and seven stall with the decrement between 1e-14 and 1e-11,
where expected-information steps cycle near a misspecified minimum and the observed Hessian is
needed. The triad starts, as a second attempt only, rescued 7 and lost none. Robust estimation
for the harder cases waits for 9C-4, which builds the general optimiser the structural equation
model needs and measures it on 9C-1's hunt, those 287 datasets' generator, the review's 52 and
the nine of 9C-1's report.

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
| CMIN | (N − 1) · max(F, 0); at 0 degrees of freedom an F of at most 1e-12 is 0 (decision 761: a converged model with as many parameters as moments reproduces S, and rounding of either sign is not a fit; rounding above 0 made CFI 1 − C/C = 0 where C_b < d_b) |
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

TLI and IFI are not truncated at 1.

**The three rows** (decision 763). AMOS prints each model-fit table with a row for the default
model (the one the user specified), the saturated model and the independence model, and the
result's `fit` holds the three, `model`, `saturated` and `independence`, each with the 38 measures
above, computed by one function of (C, q, d, RMR, GFI) against the independence model's C_b and
d_b:

- the saturated model: Σ = S, q = p*, d = 0, C = 0, its RMR 0 and its GFI 1 written, not computed
  from S⁻¹S. Its P, CMIN/DF, AGFI, PGFI, RFI, TLI, PRATIO, PNFI, PCFI, NCP's, F0's, RMSEA's and
  ECVI's intervals, RMSEA, PCLOSE and HOELTER (21 cells) are `df_zero`, the cells the default
  model marks so at d = 0; its NPAR, CMIN, DF, RMR, GFI, NFI (1), IFI (1), CFI (1), NCP, FMIN, F0,
  AIC, BCC (or `too_few_rows_for_bcc`), BIC, CAIC, ECVI and MECVI (17) are values. AMOS's tables
  print the saturated row's P, CMIN/DF, AGFI and PGFI blank, which these cells match; where AMOS
  prints .000 for a saturated cell Gaply marks `df_zero` (PRATIO, PNFI, PCFI or the NCP interval,
  say), no printed table was found to say so, and that is unverified;
- the independence model: Σ = diag(S), q = p, d = d_b, C = C_b, its RMR and GFI computed from
  diag(S); its NFI, RFI, IFI, TLI and CFI 0, its PRATIO 1 and its PNFI and PCFI 0, written, as
  AMOS prints the baseline against itself, since the formulas give other values where C_b ≤ d_b
  (CFI 1 by the zero denominator, IFI and TLI −0); every other cell by the formulas.

The default model's row is byte for byte as 9C-1 computed it, but at 0 degrees of freedom where
the F of at most 1e-12 is now 0 (decision 761: in the hunt h018 and h165, whose CFI was 0 and
is 1; 9C-1's R references for the hunt still have the old rule there, CFI 0 on h018). C_b is
written as max(C_b, 0), as C is: on columns uncorrelated to rounding it rounded to −3.4e-15
(the review of 9C-2, finding 6; `cfa_cases.rs`). On the Wheaton data of AMOS's
Example 6 (N = 932, the six columns' correlations and standard deviations, S with divisor N S_u (N
− 1) / N), the independence row is NPAR 6, CMIN 2131.790, DF 15, P .000, CMIN/DF 142.119, RMR
12.342, GFI .494, AGFI .292, PGFI .353, HOELTER 11 and 14, and the saturated row NPAR 21, CMIN .000,
DF 0, RMR .000, GFI 1.000, as AMOS prints them (`cfa_amos.rs`).

**The distributions** (decision 744): the chi-square's upper tail is Level 0's
(`chi_square_upper_wide`, statrs), and its upper quantile a bisection on it to a relative 1e-14.
The noncentral chi-square's two tails are the Poisson mixture Σ_j Pois(j; λ) P(d/2 + j, x/2),
λ = ncp/2, each tail on its own: the Poisson weights and the incomplete gamma's terms e^{−y} y^b /
Γ(b + 1) by Loader's saddle-point form (an exact product for a whole or half-whole b up to 15),
each tail's terms by the incomplete gamma's recurrence in the direction in which it only adds,
started from the incomplete gamma's series or continued fraction 3 √y clear of y = b where its
start is near there, the range reaching from the Poisson mode outward until a bound on what is
left is below 1e-17 of the sum, and the terms summed from the mode outward with compensation;
each tail is clamped to [0, 1] (decision 761: the upper tail came to 1.0000000000000002 at
(1e-4, 8, 20)), and an ncp above 1e10 gives no value, since the mixture's terms grow with √ncp
and at 2^53 its index stops moving (the release build sums at 1e10 in 0.19 s, at 1e11 in 0.6 to
0.8 s). The ncp at which the lower tail takes a value is a bisection to a relative 1e-12 of one
function, the mixture at every ncp, 0 among them (9C-1 took statrs's `gamma_lr` at 0, which
differs from the mixture there in the last bits, and the bisection never ended on such data:
decision 759's B1, 58 of 801 doubles near 34.764 at df 50 on the Mac), and it also stops when the
midpoint equals either end.

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
singular information matrix first; but on the hunt's h043 the run ends at φ11 = −2,512 after 102
iterations. There Gaply's minimum is lower than lavaan's, and `cfa_cases.rs` holds it
(decision 763): `data/h043.csv` is the hunt generator's, Gaply's estimates are written to
`h043_gaply.tsv`, and `check_h043.R` finds R's F at them 0.80253213831707981 (Gaply's
0.8025321383170834), R's gradient there at most 7.3e-9 in a component, the Newton decrement
9.7e-21 (H's reciprocal condition is 4.6e-18, so that decrement is right in order of magnitude
only), and lavaan 0.7-3's own minimum F 1.2731174595742836 (its error variance −2,480 instead).
The estimates' bits and the 102 iterations are held on macOS aarch64 only; elsewhere the
estimates within 1e-12, since the decrement sits beside the 1e-20 stop. The standardized weight's and the
correlation's `negative_variance` are checked on a solution built for them
(`stats_cfa::tests`). `distribution_out_of_range` is not reached within the limits (inferred):
d is at most p (p + 1) / 2 − 2p, 8,000 at 128 columns, within the chi-square's 8,128; the largest
ncp a run asks for is below the tails' 1e10, since S's Cholesky rule bounds ln|S| − Σ ln s_ii
below by p ln(1e-9), so C_b < n p · 20.73, at most 2.65e8 at 99,999 rows and 128 columns, the
intervals' bisection reaches at most twice its C where C is at least 1 (and 2 where it is less),
and PCLOSE's ncp .0025 n d is at most 2.0e6.

No word anywhere says a fit is good, poor, close, acceptable or adequate, and no cutoff (such as
RMSEA .05 or CFI .95) is named.

## 4. The check points

One before the arithmetic, one before each column of S, one before the estimation (every
iteration of every attempt calls it again at its unchanged count), one before the estimates, one
before the default model's fit and one before the independence model's: 1 + p + 4 (decision 762;
9C-1's 1 + p + 2 left the fit measures past the last check point). The run manager reads the
cancel flag and the clock at each call, so a cancel or the 30 s ceiling stops a run during its
iterations or before either fit (`cfa_cases.rs`, `stats_cfa_commands.rs`). Within a fit there is
no check point: the independence row's two interval bisections and PCLOSE at a large C_b took
0.751 s at C_b 1.16e8 (128 nearly collinear columns, 99,999 rows, release, 14 Nov 2026) and
0.481 s at 5.8e7; at the bound of 2.65e8, growing as √ncp, about 1.1 s (an estimate, not
measured).

## 5. Precision, the gates and the oracles

**The R golden** (`tests/fixtures/stats_cfa/make_cfa_fixtures.R`, lavaan 0.7-3, compared in
`tests/cfa_r_golden.rs`): 15 cases (Example 8; Holzinger and Swineford's 301 rows, three factors
correlated and not; one factor of three columns at DF 0; two correlated factors of two columns at
DF 1; a Heywood case; a factor covariance matrix not positive definite; two uncorrelated factors
of two columns, refused at the check as `two_item_factor_uncorrelated`; a singular S;
`independent_columns`, six independent columns that one factor fits about as well as the
independence model, C − d = −0.65, named `worse_than_independence` until 9C-2; missing values
and codes; a score as a column; 30 columns on 6 factors and 128 on 16 by a formula identical in
R and Rust; and since 9C-2 `wheaton_two_factors`, the Wheaton data of section 2), 4,605 values,
each case's three rows among them (the saturated and independence rows by the script's own
`amos_row`, from decision 745's formulas). Regenerated in 9C-2, every row 9C-1 wrote is byte for
byte the same but the three independence keys the rows replace, the renamed case and the
refusal that is now a check. Each case is fitted by lavaan with likelihood "normal", expected information and
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
| indices | 3.960e-12 (Wheaton's AGFI; 1.574e-12 over 9C-1's cases) | 1.6e-11 |
| interval measures and PCLOSE | 6.643e-13 | 6.7e-12 |
| RMR, of the largest fitted variance | 9.298e-12 | 9.3e-11 |

Over the saturated and independence rows alone the worsts are 4.056e-13 (CMIN's family),
8.239e-14 (indices), 3.582e-13 (intervals) and 8.035e-17 (RMR), with NPAR, DF and HOELTER
exact. The worst estimates are in the case whose likelihood is nearly flat (`independent_columns`);
every other case agrees within 1e-10. Where both sides of a P are below the
smallest normal double, or both CMIN-type values within (N − 1)(p + 1) · 1e-13 of 0, they agree at
rounding.

**The AMOS tier** (`tests/cfa_amos.rs`): Example 8 from its data file; every number the guide
prints (13 estimates, 13 standard errors, 13 C.R.s, 6 standardized weights, the correlation, 6
SMCs, CMIN 7.853, DF 8, P .448, NPAR 13) equals Gaply's rounded to its printed decimals; and, since
9C-2, the Wheaton data's independence and saturated rows (section 2's numbers, HOELTER as whole
numbers; `data/wheaton.csv`, reproduced from the correlations and standard deviations, not read
from Wheaton.sav). There is no gate.

**The second attempt's fixture** (`make_cfa_rescue_fixtures.R`, `data/uncorrelated_rescued.csv`):
157 rows of 27 columns drawn from five uncorrelated factors, which 7bafd6df refused at the cap and
the triad attempt solves in 13 iterations; its CMIN within 1e-9 of lavaan's refined to 1e-20
(333.26153010359394) and its 59 estimates within 2.489e-11 (`cfa_cases.rs`).

**The distributions** (`tests/stats_dist_reference.rs`): R's `pchisq` with an ncp over 598 rows,
absolute worst 3.317e-12 (gate 3.4e-11) and relative 1.084e-10 where R's tail is at least 1e-4
(gate 1.1e-9); a Poisson mixture of `pgamma`'s tails where `pchisq` cannot reach (ncp 1e4 to 2e6,
135 rows), worst 1.360e-14 (gate 1e-8); `qchisq`, 18 rows, worst 1.607e-14 (gate 1.7e-13); the
RMSEA interval's ncp against `uniroot`, 30 pairs, worst 4.485e-13 (gate 1e-10). The two tails sum
to 1 within 3.5 ε over 1,772 points where neither is below 1e-3. Since 9C-2: the review's three
triples and five doubles near 34.764 at df 50 return 0 within 5 s (each call on its own thread,
so a hang fails the test), the tails stay in [0, 1] at (1e-4, 8, 20), and an ncp of 1e300
returns no value at once; every row above passes unedited.

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

**The hunt rerun in 9C-2** (decision 764) on the same 260 datasets: 169 results. Every result
7bafd6df gave is byte for byte the same, its estimates, standard errors and default row, but two at
0 degrees of freedom whose C (7.1e-15 and 3.9e-13) is now 0. Of 9C-1's refusals, 32
`model_not_identified` are now `two_item_factor_uncorrelated`, 12 are `cfa_not_converged` with a
count (each attempt's H failing after its start), 9 `cfa_not_converged` are results from the
second attempt (h022, h041, h067, h120, h123, h211, h218, h229, h236), 14 stay `cfa_not_converged`,
and one `item_constant` and one `sample_covariance_singular` are now refused at the check as
`two_item_factor_uncorrelated`. Of the nine, the seven R refined agree with it within the gates but
h041's two standard errors (1.73e-8 against 1.3e-8) and h120's two lower intervals (8.4e-12
against 6.7e-12); on h067 and h236 Gaply's F is below lavaan's (by 0.46 and 0.22). The saturated
and independence rows of the 169 against R's, 12,844 values: within the gates but h105's
independence row (to 9.8e-11 against CMIN's 2.2e-11), on nearly singular data, where its default
row exceeded them already. A second hunt of 240 datasets drawn from the review's class (seed
20260764; 2 to 16 factors of 3 to 8 columns, half correlated, 100 to 2,000 rows): without the
second attempt 28 of the 120 uncorrelated models are refused and none of the correlated; with it
one (b087, which R's refine cannot solve either), and the 239 results agree with lavaan refined to
1e-20 (27,189 values, the worst estimate 4.672e-9) and the new rows with R's (18,164 values).

## 6. Speed and size

In release on 100,000 rows, correlated, seven runs, 9C-1 measured: 12 columns on 3 factors in
0.022 s (8 iterations), 60 on 10 in 0.337 s (10), 128 on 16 in 1.526 s (10); peak memory, the
dataset included, 181 MB, 858 MB and 1.32 GB. 9C-2 measured the same runs, its check points
timed (14 Nov 2026, load 2.5 to 3.5): 0.124 s, 0.735 s and 2.704 s. The independence row takes
0.094 s, 0.228 s and 0.354 s of them (76%, 31% and 13%), its interval's bisection meeting a
large ncp at 100,000 rows, and the default and saturated rows 0.002 s to 0.050 s. The stages
before the fit took 0.028 s, 0.488 s and 2.30 s, slower than 9C-1's whole runs at 60 and 128
columns on code that changed there only by a finiteness scan of S; that difference was not
measured against 9C-1's build and is not explained.

A run every attempt refuses at the cap costs two capped attempts: 128 independent normal
columns on one factor, 300 rows, refused `cfa_not_converged` (count 500, limit 500) after 31.02 s
in release through `run`, 31 ms an iteration. That is past the run manager's 30 s ceiling, so at
the largest sizes a run that converges from neither start may end at the time ceiling
(`timed_out`) instead of `cfa_not_converged`, until 9C-4's estimation (inferred from the ceiling
and the check point at every iteration; not run through the app). A result of 128 columns and
16 correlated factors serialises to 85,749 bytes with its three rows (83,233 in 9C-1; under 1
MiB, `cfa_cases.rs`).

## 7. The export

A long CSV in the eight fields every method's export has: the rows used and the iterations;
each loading (estimate, standard error, C.R., P, standardized weight) by column index, alias and
factor number; each factor variance by factor; each covariance by its two factors with its
correlation; each error variance and SMC by column; every fit measure of the default model as
table `fit`, and of the saturated and independence models as tables `fit_saturated` and
`fit_independence` with the same statistic names (decision 763; 9C-1's three `independence` rows
are gone); the rows left out; the notes; the limitations; the manifest's hashes. Pinned on macOS
aarch64 (`tests/cfa_export.rs`, `export_golden.csv`); every other platform checks the pin's
layout until a Linux run gives its bytes (9C-1's pin was equal on Linux x86_64, decision 758;
9C-2's is to be measured there after its push).

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
| NPAR, RMR, GFI, AGFI, PGFI, HOELTER and NFI (Appendix C) | the same formulas | reproduced by decision 743's research (Example 6, Wheaton: all 40 values, NFI its one); the independence and saturated rows reproduced in 9C-2 (`cfa_amos.rs`: NPAR, CMIN, DF, P, CMIN/DF, RMR, GFI, AGFI, PGFI, HOELTER) |
| Each table's three rows: default, saturated, independence (Appendix C; Example 6) | the same; the saturated row's df_zero cells blank in AMOS; the independence row's NFI to PCFI written as AMOS prints the baseline against itself | reproduced for the cells above; a saturated cell AMOS may print as .000 where Gaply gives df_zero (PRATIO, PNFI, PCFI, the intervals) not checked |
| RFI, IFI, TLI, CFI, PRATIO, PNFI, PCFI, NCP, F0, RMSEA and its interval, PCLOSE, ECVI, MECVI (Appendix C) | the same formulas | formula; each agrees with lavaan's own where lavaan defines it the same way |
| CFI's denominator max(C_b − d_b, C − d, 0) | the same | formula; it differs from max(C_b − d_b, 0) only where C − d is above both C_b − d_b and 0, which needs no model worse than the independence model, since d is below d_b: `cfa_cases.rs` reaches it on six independent columns (C − d 4.135, C_b − d_b 4.036, CFI 0); the golden case `independent_columns` does not (C − d is −0.65, CFI 1) |
| BCC, BIC, CAIC | C + 2q (N − 1)/(N − p − 2); C + q ln N; C + q (ln N + 1) | not checked against printed values; AMOS's BIC help says BIC penalises more than CAIC, while these formulas give CAIC = BIC + q |
| Each factor's first regression weight fixed at 1, each error's at 1, variances free; no means | the same | reproduced (Example 8's model) |
| AMOS's stop: every first derivative below 1e-5 and a last step below 1e-3 | a Newton decrement of 1e-20, far tighter, so printed values agree | not checked against AMOS's iterations |
| Estimation from AMOS's own starts | decision 745's starts and Fisher scoring, then the triad starts (decision 761) | not checked against AMOS: of the nine hunt datasets 9C-1 refused where lavaan, and so presumably AMOS, gives numbers, the second attempt solves seven (h041, h123, h218 improper; h022, h120, h211, h229 on the boundary), and h078 and h139 stay refused as `cfa_not_converged` |
| Negative variances: "The following variances are negative." and "This solution is not admissible." | the `negative_variance` note; the numbers are given | formula (the sentences quoted in decision 743) |
| Standardized RMR through a plugin | not computed | not checked |

## 10. Blueprint §18.2 acceptance

Not applicable until 9C-3 puts the method in the window; 9C-1 and 9C-2 are the core, the
commands and the runs.
