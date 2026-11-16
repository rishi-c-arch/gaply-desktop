# Structural equation models, as an analysis

`stats.analyze.sem`, version 1, engine version 1. Research OS, Phase 9C-4 (decisions 792 to
805), `src-tauri/gaply-core/src/stats_sem/`, with the measures of fit it shares with the
confirmatory factor analysis in `src-tauri/gaply-core/src/stats_fit/`, its distributions in
`src-tauri/gaply-core/src/stats_dist/` and its linear algebra in
`src-tauri/gaply-core/src/stats_linalg/`.

**Status: Beta, with no window until 9C-5.** 9C-4 built the engine, registered it (Planned at
Part 2's end, decision 801) and made it Beta with six commands and a run kind (decision 803). No
menu item reaches it: Coming later still says "Structural equation models", the old home lists
its card with no screen, Help lists it with its limitations, and a finished run cannot be saved
into a project (`project.run_not_saveable`, pinned by
`a_finished_structural_equation_model_is_not_saveable_until_it_has_a_row` in
`tests/project_commands.rs`) until 9C-5 gives it a row, a window and a path diagram.

The goal is AMOS's numbers (decision 795): the model fitted as AMOS fits a drawn path diagram,
its estimates, standard errors, critical ratios, standardized estimates, squared multiple
correlations, effects and fit measures as AMOS prints them.

## 1. Inputs and the spec

`SemParams`, camelCase, unknown fields refused, no field defaulted:

| Field | Meaning |
|---|---|
| `indicators` | 0 to 16 factors, each a list of 2 or more column aliases, its columns in the dataset's order; the factors in the order of their first columns; no column in two factors or twice in one. Factor n is the list at position n − 1. |
| `paths` | 0 to 64 single-headed arrows `{to, from}`, each end `{"factor": n}` or `{"column": alias}`. |
| `covariances` | 0 to 256 double-headed arrows `{a, b}`: between the variables when both are exogenous, between their error terms when either is endogenous, as AMOS draws it. |
| `equal` | Groups of parameters held equal, as AMOS's shared labels: each 2 or more members of one kind, `{"loading": {factor, column}}`, `{"path": {to, from}}`, `{"covariance": {a, b}}` or `{"variance": {variable}}`; at most 128 members over all groups. |
| `missingCodes` | Finite, ascending, each once; applied to every column. |

The order that makes each model one spec (decision 797): variables are factors first, by number,
then columns by alias; paths ascend by (to, from); each covariance has a before b and the
covariances ascend by (a, b); parameters order as loading, path, covariance, variance, then by
their fields; each group's members ascend and the groups ascend by their first members. No name
a user typed is in the spec, the result or a log (ADR-011); factors are numbered.

The model's columns are every column the spec names, 2 to 128, in alias order. A column in a
factor is that factor's indicator: its loading is an arrow from the factor, the first fixed at 1,
the others free. An endogenous variable is an indicator or a path's `to`; every other variable is
exogenous. Each exogenous variable has a free variance, each endogenous one a free error variance,
each spec covariance is free and every other covariance is 0. No means. NPAR q is the free
loadings, paths, covariances and variances, less, for each equality group, its members but one;
DF d = p (p + 1) / 2 − q.

The checks, in this order (`validate` returns every error the spec's checks find, then
identification only when they find none, then the rows' checks only when there is no other
error):

1. the codes and the input, as CFA's;
2. the model's shape: `empty_model` (no factor, path or covariance), `too_many_factors`,
   `factor_too_few_items` (the factor's number), `items_out_of_order`, `factors_out_of_order`,
   `item_repeated`;
3. the paths: `too_many_paths`; then each, by its position from 1: `unknown_factor`,
   `path_to_itself`, `indicator_in_path` (a factor's column at either end), `paths_out_of_order`,
   `path_repeated`;
4. the covariances: `too_many_covariances`; then each: `unknown_factor`,
   `covariance_with_itself`, `covariances_out_of_order`, `covariance_repeated`;
5. the groups held equal: `too_many_equal_members`; then each group (`index`, and `value` for a
   member's position): `equal_group_too_small`, `equal_mixed_kinds`; then its members one by
   one, each `equal_unknown_parameter` or `equal_fixed_parameter` (a first loading); then
   `equal_out_of_order` (the group against the one before it, then its members in turn); then
   `equal_repeated` for each member already held (`sem_cases.rs`,
   `the_spec_checks_come_in_decision_797s_order_and_validate_lists_every_one`, pins this order;
   decision 797 and this page said "unknown, then fixed" for each group until 9C-5, decision 812);
6. the columns: `too_few_columns`, `too_many_columns`; then each in alias order:
   `unknown_column`, `column_non_finite`, `column_holds_text`; then, in a second pass after every
   column, each column's `code_equals_score`, as CFA's checks do;
7. identification from the spec alone (§2): `model_not_identified` with `count`, the
   constraints the model lacks;
8. then, each only when none before it applies: `no_complete_rows`; `too_few_rows` (fewer than
   p + 1); `column_constant`.

A scale score may be a column like any other. Rows: listwise over every model column, codes as
missing; a row left out is counted once, under the first column without a value in alias order,
as `missing_value` or `missing_code`.

## 2. Formulas, and where each number comes from

**S and F.** S is the covariance matrix of the complete rows with divisor N, two-pass means, in
alias order and CFA's summation order (`sample_covariance`, a copy of `stats_cfa`'s arithmetic, not
shared code). A mean or
entry that is not finite refuses the run as `overflow`; S's Cholesky factor (the 1e-9 rule)
refuses it as `sample_covariance_singular`, naming the column. F(θ) = ln|Σ(θ)| + tr(S Σ(θ)⁻¹) −
ln|S| − p; CMIN = (N − 1) F at the minimum (decisions 743, 795).

**The RAM model** (decision 798). Over m = k + p variables in the model's order, A (m × m) holds
the arrows, A[t][s] the weight from s to t; Φ (m × m, symmetric) the variances and covariances;
B = (I − A)⁻¹; Ω = B Φ Bᵀ; Σ = F Ω Fᵀ, F selecting the columns; G = F B and K = F Ω. Without a
loop, B is built by substitution in a topological order of the arrows, B[t] = e_t + Σ A[t][s] B[s]
over t's sources: no pivot and no tolerance, since I − A is then always invertible, and an
arrow that is the only route gives an indirect effect of exactly 0. With a loop, B is
`stats_linalg::inverse` (Gauss–Jordan with partial pivoting, refusing a pivot at or below 1e-12
of the largest entry) of I − D⁻¹ A D, D each variable's natural unit as a power of two (the
exponent of its representative column's √s, so the scaling is exact), scaled back. Inverting
I − A as it stands refused every loading above about 10⁶: pivoting on a loading λ above 1 leaves
a pivot near 1/λ. The hunt found that on 9C-2's h226, a CFA the CFA solves, whose columns are
on scales 10⁵ apart (`a_column_on_another_scale_changes_only_its_own_weights`). F(θ) is undefined
when I − A cannot be inverted or Σ fails `cholesky_within(Σ, 0)`.

θ holds the free loadings (factor by factor, the first skipped), the paths, the covariances and
the variances, in that order; an equality group's members share the position of its first
member. Each entry's derivative of Σ is one or two outer products: A[t][s] gives
g_t h_sᵀ + h_s g_tᵀ, an off-diagonal Φ[a][b] gives g_a g_bᵀ + g_b g_aᵀ, a diagonal Φ[v][v]
gives g_v g_vᵀ, a parameter's the sum over its entries. With R = Σ⁻¹ − Σ⁻¹ S Σ⁻¹ the gradient
is tr(R Σ_a) and the expected information H_ab = tr(Σ⁻¹ Σ_a Σ⁻¹ Σ_b), each a sum over pieces of
entries of Gᵀ Σ⁻¹ G, Gᵀ Σ⁻¹ K and Kᵀ Σ⁻¹ K, so an evaluation costs about p³ + m² p + q² and
holds no p² × q array.

The observed information, for Newton's attempt only: J = −H + 2T + U, T_ab = tr(Σ⁻¹ Σ_a P Σ_b)
with P = Σ⁻¹ S Σ⁻¹, and U_ab = tr(R Σ_ab) from the second derivatives of Σ (decision 798 gives
each pair of entries' term; two Φ entries give 0). `the_observed_information_agrees_with_differences_of_the_gradient`
compares J with central differences of the analytic gradient at random points of a model with a
loop, an equality and a correlated error: within its gate of 5.4e-10, ten times the measured
5.308e-11.

**The starts** (attempt 1's; r(v) is v for a column, its first column for a factor): a free
loading s_i,r(f) / φ₀_f with φ₀_f = s_r(f),r(f) / 2, as CFA's; a path into t the least-squares
weights of r(t) on the representatives of all t's sources, from S; a factor's variance or its
error's φ₀_f; an exogenous column's s_vv; an endogenous column's error s_vv / 2; a covariance of
two exogenous columns s_ab, of exogenous variables one or both factors s_r(a),r(b) / 2, of error
terms 0; an equality group its first member's start. On a CFA's model these are CFA's starts.

**Three attempts**, in this order, each capped at 500 iterations, each made only when the ones
before it ended without a solution (decision 798):

1. Fisher scoring from the starts, as CFA's: δ = −H⁻¹ g, halved up to 30 times until F is defined
   and, while gᵀH⁻¹g is above 1e-10, does not increase; stop at gᵀH⁻¹g ≤ 1e-20.
2. Newton's method on J, damped, from the same starts. It stops when H passes Cholesky and
   gᵀH⁻¹g ≤ 1e-20, and that is a solution only when J passes `cholesky_within(J, 0)` (a minimum,
   not a saddle). Otherwise δ = −(J + μD)⁻¹ g, D the diagonal of H floored at 1e-12 of its
   largest; μ grows from 0 to 1e-4 and then tenfold while the solve fails, F is undefined or F
   rises, up to 60 solves a step, and falls tenfold (to 0 at or below 1e-4) after an accepted
   step. `newtons_attempt_refuses_a_saddle_and_accepts_a_minimum` drives the loop on x² − y² at the origin.
   A rise in F is accepted in one case only, near a minimum where F's rounding exceeds what a
   step can lower it by: H passes Cholesky with gᵀH⁻¹g ≤ 1e-10, Fisher scoring's own test
   (decision 812), **and** the solve's own decrement gᵀ(J + μD)⁻¹g, the computed step dotted with
   g, is at most 1e-10 too (decision 823). The second condition is what bounds the step: the step
   solves J + μD, not H, so a small gᵀH⁻¹g alone says nothing about its size, and on a function
   with H = 1 and J = 1e-6 it once allowed a step from t = 10 to 0 that raised F from 5e-5 to
   6.25e5 (the review of 365b638). With both, a relaxed step is bounded in the measure it was
   solved in, gᵀ(J + μD)⁻¹g = δᵀ(J + μD)δ ≤ 1e-10, and so δᵀJδ ≤ 1e-10 always, since
   δᵀJδ = δᵀ(J + μD)δ − μδᵀDδ with μ ≥ 0 and D ≥ 0; the bound is on the step in that measure,
   not on its length. `newtons_relaxed_step_is_bounded_by_its_own_solves_decrement`,
   `newtons_attempt_relaxes_nothing_where_h_fails_its_factor` and
   `newtons_attempt_relaxes_nothing_where_h_s_decrement_is_large` pin the rule; on 9C-4's 338
   models, 9C-2's 500 and the 200 seeds of id384's shape the bound changed no outcome and no
   value (9C-5b's hunt rerun, byte-identical to 9C-5's run after its M1 fix).
   A solve whose step rounds away, its candidate equal to θ in every coordinate, is not taken,
   and F is not evaluated there (decision 828, as decision 833 refines it). If a smaller μ has
   not yet been tried at this θ, which happens where the iteration began with a μ carried above
   0 from the last, the next solve takes the next smaller μ (μ / 10, or 0 at or below 1e-4),
   not counted as an iteration; otherwise the attempt ends at once, since a larger μ only
   shrinks the step further. Each such solve still counts against the 60 solves of one step,
   and μ still rises after a refused step, so the attempt cannot loop. Before decision 828 the
   strict test took a zero-length step and the attempt stood still to its cap, inflating a
   refusal's count and its time; decision 828 alone ended the attempt even where a smaller μ,
   untried, might have moved it (the review of 9C-5c, finding 1).
   `newtons_attempt_ends_at_a_step_of_zero_length`,
   `newtons_attempt_tries_a_smaller_mu_after_a_zero_length_step` and
   `newtons_attempt_ends_where_the_smaller_mu_raises_f` pin it. Rerun on 9C-4's 338 models,
   9C-2's 500 and the 200 seeds of id384's shape, decision 828 changed no line (9C-5c's dump and
   table are byte-identical to 9C-5b's), and decision 833's refinement changed none either
   (9C-5d's are byte-identical to 9C-5c's; how often its step down was reached there was not
   counted): no solved case changed, and no refusal's count fell
   (a refusal reports the most iterations any attempt made). On the Walsh columns, where
   Newton's attempt makes the most, the count falls from 500 to 9, and stays 9.
3. Fisher scoring from the triad starts: each factor of three or more columns given CFA's triad
   φ₀ (decision 761) and its loadings from it; made only when these differ from attempt 1's.

A run no attempt solves refuses as `sem_not_converged` with `count` (the most iterations an
attempt made) and `limit` (the largest cap among them). The result carries `iterations` (the
successful attempt's steps) and `attempt` (1, 2 or 3).

**Identification from the spec alone** (decision 798; `validate` and the run). H at two points
drawn from the spec alone by SplitMix64, seeded 0x9C4 and 0x9C5: a free loading 0.5 + u, a path
u − 0.5, a variance 1 + u, a covariance (u − 0.5) / (1 + c), c the most covariances at any end of
its members, so Φ is diagonally dominant. H is scaled to unit diagonal and factored by
`stats_linalg::pivoted_rank` (Cholesky on the largest remaining diagonal, the lowest index on a
tie), its rank the pivots above 1e-9. If the larger rank of the two points is below q the spec is
refused as `model_not_identified` with `count` q − rank; a point where Σ is undefined counts as
rank 0, and where both are, the count is q. This replaces CFA's rule that H failing at the start
means not identified: here that ends one attempt. CFA's `two_item_factor_uncorrelated` is one
case of it.

The counts on known specs, against R's ranks (`the_identification_check_counts_the_constraints_a_model_lacks`, R in
`logs/ff_identification.log`): a two-column factor alone 1 (q 4, rank 3); two uncorrelated
two-column factors 2 (q 8, rank 6); two uncorrelated factors of columns 1-2 and 3-4 with the
errors of 1 and 3, 2 and 4, and 1 and 4 correlated 2 (q 11, 10 moments, rank 9); a loop between
two columns with nothing pointing into either 1 (rank 3); Wheaton's Models A to D identified.

**Identification at the solution** (the run only). With each parameter's natural unit u_a
(√(s_r(t)r(t) / s_r(s)r(s)) for A[t][s], √(s_r(a)r(a) s_r(b)r(b)) for Φ[a][b]), a solution where
√((H⁻¹)_aa) / u_a exceeds 1e8 for any parameter refuses as `solution_not_identified`: moving that
estimate by 10⁸ of its unit, the others refitted, raises F by only about 0.5. At the Walsh columns
of `cfa_cases.rs`, the point the CFA accepts measures 3.2339e15 and is refused
(`the_check_at_the_solution_refuses_the_walsh_point_and_passes_example_8`); at Example 8's solution it passes.
No dataset is known to reach this refusal through the attempts: on those Walsh columns every
loading starts at 0, H fails at attempt 1's start, the triad attempt is not made (its starts are
attempt 1's), and Newton's attempt moves 9 times and then its step rounds to zero length, so the
run refuses as `sem_not_converged` (count 9, limit 500, `sem_cases.rs`; count 500 before decision
828 ended the attempt at a step of zero length).

**The statistics** (decision 799), in AMOS's tables' order:

| Statistic | Formula |
|---|---|
| Regression weights | One per arrow of A, loadings first (the fixed first ones included), then the paths in the spec's order: estimate; SE √(2 (H⁻¹)_aa / (N − 1)); C.R. estimate / SE; P two-tailed normal; standardized a √Ω[s][s] / √Ω[t][t]. A first loading's SE, C.R. and P are `fixed_parameter`. |
| Covariances | One per spec covariance: estimate, SE, C.R., P; correlation Φ[a][b] / √(Φ[a][a] Φ[b][b]) (error variances where the ends are endogenous). |
| Variances | One per variable in the model's order, `error` true for an endogenous variable's error term. |
| Squared multiple correlations | One per endogenous variable: 1 − Φ[v][v] / Ω[v][v] where Ω[v][v] > 0. |
| Effects | For each ordered pair of structural variables (the factors and the columns no factor holds) joined by a directed path: total (B − I)[to][from], direct A[to][from], indirect their difference, each also standardized by √Ω[from][from] / √Ω[to][to]. In a model with a loop, B − I is the effect at equilibrium, as AMOS's. |
| Fit | The confirmatory factor analysis's three rows, unchanged (decision 796; `confirmatory_factor_analysis.md` §2), with this model's q, F and Σ̂. |

The fit's code moved to `stats_fit` in Part 1 with its arithmetic and its order unchanged:
every CFA golden, the AMOS tier, h043, the export pin and every CFA stored fixture passed
unedited. Each interval's bisection now calls its row's check point
(`noncentral_chi_square_ncp_with`), at unchanged counts.

Everything is in +, −, ×, ÷ and √ but ln in `log_det`, so the same bits on every platform.

## 3. Notes and not-computable values

Notes, bare codes, no verdict, in this order: `negative_variance` (a variance or error variance
estimate below 0: AMOS's "This solution is not admissible"); `covariance_not_positive_definite`
(the block of Φ̂ over the factors and every variable a covariance names fails
`cholesky_within(·, 0)`; on a CFA's model, CFA's `factor_covariance_not_positive_definite`);
`nonrecursive` (the paths hold a loop); `df_zero`. Each note is reached by a test:
`sem_cases.rs`'s `each_note_comes_in_its_order`, whose `covariance_not_positive_definite` cases
are a block of two factors (`phi_not_pd`) and, since 9C-5 (decision 812), a block through error
covariances alone (one factor, so the factors' own block is 1 × 1 and positive: the golden's
`error_block_not_pd`, with `df_zero`).

Not computable, each written where CFA's is, and where each is reached:

- `fixed_parameter` (a first loading's S.E., C.R. and P), `df_zero`, `cmin_zero`,
  `independence_cmin_zero` and `too_few_rows_for_bcc`: in `sem_cases.rs`
  (`each_not_computable_path_writes_its_code`), and the first three in `sem_r_golden.rs` too
  (`cmin_zero` in the `phi_not_pd` case's HOELTER);
- `negative_variance` as a value (a standardized weight, correlation, squared multiple
  correlation or standardized effect whose implied or error variance is at or below 0): in
  `stats_sem`'s unit test
  `a_factor_variance_below_0_leaves_its_standardized_values_not_computable_and_is_noted` only;
  neither the golden nor `sem_cases.rs` holds such a value (the golden's one
  `negative_variance` is a note);
- `too_small` and `overflow`: written by `stats_sem`'s own `stat` for the estimates, S.E.s,
  C.R.s, P values and standardized values, and by `stats_fit`'s for the fit. Both write a value
  that is not a number (a 0/0 of quantities that vanished) as `too_small` and an infinity as
  `overflow`, and keep a finite value, a subnormal included, as a value, as
  confirmatory_factor_analysis.md says of CFA's. No SEM test reaches either;
- `distribution_out_of_range`: `stats_fit`'s, for a distribution that gives no value. No SEM
  test reaches it.

## 4. The check points

One before the arithmetic, one before each column of S, one before the estimation (every
iteration of every attempt, every solve of Newton's step and the check at the solution calling it
again at its unchanged count), one before the estimates and effects, one before the default
model's fit and one before the independence model's (each interval's bisection calling the row's
own again): 1 + p + 4 (`the_check_points_come_in_their_order_and_a_stop_ends_the_run_anywhere`). The run manager reads the cancel flag and the clock
at each call, so a cancel or the 30 s ceiling stops a run during its iterations
(`stats_sem_commands.rs`: a cancel mid-covariances, one during the iterations, and the ceiling
on an injected clock).

## 5. Precision, the gates and the oracles

**The R golden** (`tests/fixtures/stats_sem/make_sem_fixtures.R`, R 4.6.1 and lavaan 0.7-3,
compared in `tests/sem_r_golden.rs`): each case fitted by lavaan with every parameter written
out (normal likelihood, expected information, `fixed.x = FALSE`), then refined by the script's
own Fisher scoring on the RAM matrices (written from decisions 795 and 798, not from Gaply's
code) to a decrement of 1e-20, every statistic computed in R from decision 799's definitions,
and cross-checked against lavaan (its standard errors times √(N / (N − 1)), `std.all`,
`rsquare` and total effects within 1e-10; the Wishart fit's measures within 1e-6; the script
stops at any disagreement: the largest seen were 1.49e-13 on the standard errors and 6.7e-7 on
a fit measure). 21 cases, 6,458 values: Wheaton's Models A to D; Example 7 (`felson.csv`, 209
rows with Fels_fem's correlations and standard deviations); Example 8 as an SEM;
PoliticalDemocracy, and with its loadings held equal; a MIMIC model on HolzingerSwineford1939
(grade's empty cell a missing value); a path model of columns with two paths held equal; a loop
between two factors; a Heywood case; Φ̂ not positive definite; a model at 0 degrees of freedom;
missing values and codes; a scale score; 30 columns on 6 factors with 5 paths and 128 on 16
with a chain of 15, by CFA's formula; and the refusals (a two-column factor alone,
`model_not_identified` count 1; more parameters than moments; a singular S).

Each family's gate is ten times its worst measured error, relative (absolute where R's value is
0), macOS aarch64, 15 Nov 2026:

| Family | Worst | Gate | Where |
|---|---|---|---|
| estimate | 2.120e-8 | 2.2e-7 | Example 7, academic ← attract (−0.002, C.R. −0.04, a flat likelihood both stop on) |
| se | 3.601e-10 | 3.7e-9 | missing and codes, a variance |
| cr | 2.108e-8 | 2.2e-7 | Example 7, the same weight |
| p | 9.284e-10 | 9.3e-9 | missing and codes, a variance |
| standardized | 2.120e-8 | 2.2e-7 | Example 7, the same weight |
| effect | 2.120e-8 | 2.2e-7 | Example 7, that weight's direct effect |
| fit_chi | 6.687e-12 | 6.7e-11 | 30 columns, P |
| fit_index | 2.345e-12 | 2.4e-11 | missing and codes, AGFI |
| fit_interval | 9.038e-13 | 9.1e-12 | Wheaton A, PCLOSE |
| fit_rmr | 6.864e-12 | 6.9e-11 | missing and codes, RMR |

**The linear algebra's golden** (`make_sem_linalg_fixtures.R`, base R only; `linalg.tsv` and
`cases.tsv` untouched): `inverse` against `solve()` on 40 seeded matrices of order 2 to 60,
gated at 1.5e-13 (general), 5.6e-15 (shuffled unit triangular) and 1.3e-10 (smallest singular
value 1e-6 of the largest), ten times the measured 1.457e-14, 5.551e-16 and 1.268e-11;
`pivoted_rank` against `qr(…, tol = 1e-9)$rank` on 24 symmetric matrices of known rank, among
them ones where taking the first remaining diagonal gives another rank. Exact.

**The AMOS tier** (`tests/sem_amos.rs`): Gaply's value rounded to AMOS's printed decimals, a P
below .001 as `***`, HOELTER whole; a one-in-the-last-place difference fails, no gate.

- Wheaton's four models (Example 6, pp. 104, 108, 115, and Appendix C): CMIN, DF, P, NPAR,
  CMIN/DF, RMR, GFI, AGFI, PGFI and HOELTER .05 and .01. All match.
- Example 7 (pp. 129-135): every printed estimate, SE, C.R., P, standardized weight,
  correlation, squared multiple correlation, CMIN, DF and P. All match.
- Example 8 as an SEM: decision 743's numbers. All match.
- Model B's 120 critical ratios for differences between parameters (pp. 112-113), from the
  estimates' covariance matrix V = [(N − 1) H / 2]⁻¹, in a unit test in `stats_sem`
  (`wheaton_model_bs_critical_ratios_for_differences_are_amoss_printed_output`): all 120 match, so every standard error
  and every covariance between two estimates is AMOS's.

**The CFA as an SEM** (`sem_cases.rs`, `a_cfa_spec_fitted_as_a_structural_model_gives_the_cfas_result`):
on Example 8 and Holzinger and Swineford's three factors, the SEM's estimates, standard errors
and fit are within the CFA golden's gates of the CFA's, at attempt 1. 9C-2's hunt cases are not
committed, so the test cannot read them; the hunt's probe compared all 500 of them as SEMs against
the CFA golden (below).

**The attempts** (`sem_cases.rs`): on Example 8, caps (500, 500, 500), (1, 500, 1) and
(1, 1, 500) reach the same solution at attempts 1, 2 and 3 (15, 7 and 16 iterations); (1, 1, 1)
refuses as `sem_not_converged` with count 1 and limit 1. Wheaton's four models and Example 7
are solved at attempt 1.

**The bug hunt** (decision 800; the scripts and data in `~/gaply-corpus/os/phase9/9c4/hunt/`,
not committed). 330 random models, seed 20260800: 137 normal, 54 weakly determined, 49
misspecified, 46 Heywood cases and 44 small samples, 2 to 110 columns, 190 with covariances and
47 with parameters held equal. Each was fitted by lavaan and refined by the golden script's
Fisher scoring, giving 54,145 values. 9C-2's 500 CFA hunt cases were also run as SEMs.

The hunt's first run was void. Its rank check reseeded R's random stream at every case, so its
330 files were about 11 datasets repeated, each giving the same outcome. With the stream saved
and restored, the 330 files have 330 different hashes.

Outcomes, Gaply against R:

- **The same answer.** 259 solved by both; 256 of these reached the same minimum, F within
  2.6e-13. 41 refused as `model_not_identified` where R's rank also finds the model
  unidentified, 1 as `too_many_paths` and 2 as `too_few_rows` (R: S singular). Gaply's count
  and R's rank agreed on every case.
- **Neither converged.** 13 cases: Gaply's three attempts and lavaan both fail.
- **Gaply solved and R did not.** 10 cases: 6 where lavaan did not converge, 4 where lavaan
  did and the refinement did not.
- **A lower minimum than R's, improper.** h113 (F 1.7101 against 1.7817; an error variance of
  −6,138) and h222 (11.907 against 12.962; −20.8). All three attempts reach the same point, so
  R stopped at a higher, proper local minimum, as 9C-2's h043 did for the CFA.
- **A higher minimum than R's.** h287, a model with a loop: Gaply stops at F 0.7987, improper
  (a variance of −1.4e-4) and with a covariance matrix not positive definite, where R reached
  0.6711, proper. Attempt 3 alone stops at 0.9147. Gaply's check at the solution passes all
  three points: Fisher scoring finds a local minimum, and nothing in decision 798 promises the
  global one. The notes (`negative_variance`, `covariance_not_positive_definite`) are on the
  result.
- **Refused where R reached a proper minimum.** h067 and h161, `sem_not_converged`. Traced
  with the cap raised to 20,000: from every start, Fisher scoring creeps along a ridge where one
  weight grows past 300, another past −90 and a variance falls toward 0. F settles toward
  0.21898 (R's minimum is 0.21402), until H fails its factor at step 6,732 (h161: 7.8185
  against 7.5037, step 3,004). The refusal is the right outcome for that path; reaching R's
  point would need another start.

Attempts: of the 269 solutions, 265 at attempt 1, 3 at Newton's (11 to 44 iterations, median
17) and 1 at attempt 3. The identification check's margins: the smallest pivot counted on an
identified model was 1.06e-3 (h045), and the largest that stopped a refused one 7.6e-16 (h240).
The largest solution measure accepted was 2.74e4 (h097), against the limit of 1e8.

Outside h113, h222 and h287, six values pass the golden's gates, by less than twice the gate:
standard errors 6.75e-9 and 6.75e-9 (h077) and 4.16e-9 and 4.16e-9 (h288), against 3.7e-9;
AGFI 2.76e-11 (h038) and RMSEA 3.09e-11 (h166), against 2.4e-11. These are the same flat
families as 9C-2's. Every HOELTER and every note matches outside those three cases.

9C-2's cases as SEMs:

- **Solved by both.** 410 cases (372 at attempt 1, 27 at Newton's, 11 at attempt 3).
- **The same refusal.** 19 `column_constant` against the CFA golden's `item_constant` (the same
  refusal, named for the SEM); 10 singular; 2 `too_few_rows`.
- **`model_not_identified`.** 2 cases where the golden also gave it, and 2 where it gave
  `item_constant` (1) or `sample_covariance_singular` (1).
- **30 `model_not_identified` where the golden solved.** All are uncorrelated models with a
  two-column factor, which today's CFA also refuses (`two_item_factor_uncorrelated`, decision
  762; the golden predates it).
- **25 `sem_not_converged` where the golden solved.** Each is also refused by today's CFA
  (`cfa_not_converged`); only R reached those solutions (h139, for one, is improper: a factor
  variance of −1.02 and a loading of −168 at 0 degrees of freedom).

Over the gates on 9C-2's data: standard errors to 1.73e-8 (h041), fit indices to 9.5e-11 (h056),
the intervals to 1.58e-11 (h153), all in the families 9C-2 measured flat. h018's CFI is 1
against R's 0: decision 761's change at 0 degrees of freedom.

**What the hunt found and what changed.** 9C-2's h226, a model the CFA solves, was refused:
any weight above about 10⁶ made inverting I − A fail its 1e-12 pivot rule. The fix:

- **Models without a loop:** B is built by substitution in topological order, with no pivot
  and no tolerance.
- **Models with a loop:** I − A is inverted in power-of-two natural units, which rescale
  exactly.

`a_column_on_another_scale_changes_only_its_own_weights` in `sem_cases.rs` was red before the
fix and green after: Example 8 with a column times 10⁷, Example 7 with one times 10⁻⁹, and the golden's loop between two factors with C2 times 10⁷. The loop case was added when a mutation setting every natural unit to 1 left the first two green: neither puts a large weight below a diagonal of I − A. With it, that mutation refuses as h226 did. h226
is now solved at attempt 1 in 23 iterations, as the CFA solves it.

The export pin and two wire views were rewritten:

- **The export pin:** 85 lines changed, at most 2.4e-13 relative.
- **The wire views:** at most 9.2e-9, on one P in the loop case with a Heywood variance, where
  the likelihood is flat.

No code, note or word changed, and no zero flipped. The R golden, the AMOS tier and the cases
pass unedited.

**The large cases** (`hunt_large.R`, seed 202608800): 8 models of 93 to 128 columns on 135 to
595 rows, 15,494 values. R solved 7: Gaply solved each at attempt 1 in 12 to 34 iterations, F
within 7.7e-13 of R's, every value within its gate. On the eighth (128 columns, a small sample)
lavaan did not converge, and Gaply solved it at attempt 1.

## 6. Speed and size

Measured in a release build on macOS aarch64, 15 Nov 2026, by a probe never committed: data from a
formula of the row and column numbers on 100,000 rows, seven runs each. The load average was 18.9, falling to 12.1, after
a build; one core was held by a browser throughout.

| Model | Median | Fastest, slowest | Iterations, attempt | Peak memory |
|---|---|---|---|---|
| 12 columns, 3 factors, 3 paths | 0.124 s | 0.124 s, 0.126 s | 9, attempt 1 | 0.17 GB |
| 60 columns, 10 factors, 9 paths | 0.774 s | 0.768 s, 0.851 s | 10, attempt 1 | 0.74 GB |
| 128 columns, 16 factors, 15 paths, 64 covariances | 3.465 s | 3.446 s, 4.393 s | 14, attempt 1 | 1.11 GB |

Peak memory is the probe's whole process, its 100,000-row table included (`/usr/bin/time -l`). No
success comes near 20 s. `validate` on the 128-column spec, where the identification check is
the cost, takes 0.096 s (median of seven).

Newton's attempt now ends at a step of zero length instead of standing still to its cap, so a
run every attempt refuses can end as `sem_not_converged` before the 30 s limit; Fisher's attempts
1 and 3 still run to 500 iterations each, and the time limit is still what a user sees for a run
that has not ended by 30 s. Two such runs were measured. 9C-4's case, 128 columns of Walsh patterns over
2¹⁷ rows on 16 factors with a chain of 15 paths and a check point that stops the run after
30 s, met the ceiling's stop at 30.01 s at 9C-4 (decision 800); after decisions 828 and 833 it
is refused as `sem_not_converged`, count 7 of 500, in 2.14 s (release, 16 Nov 2026,
9c5d/r1_runs_1794824308.log). The review of 735ec94c's model, 128 such columns over 256 rows
(S = I), 16 factors of 8 columns in order and the paths factor k → factor k + 1, is refused at
count 7 of 500 in 0.74 s, and before decision 828 met the stop at 30.04 s (the review's
measurement). Decision 767's third answer, that such a run meets the time limit and not a
refusal, described the code before decision 828: Newton's attempt then stood still to its cap.
`a_model_every_attempt_refuses_ends_at_newtons_count_not_its_cap` (sem_cases.rs) pins the
review's model's count, 7 of a cap of 10.

The widest result within the limits (128 columns: 16 factors of five columns and 48 columns no
factor holds; 64 paths in one loop through the 64 structural variables; 256 covariances)
serialises to 1,330,981 bytes with 4,032 effects, solved at attempt 1 in 19 iterations
(`sem_cases.rs`, under its 2 MiB assertion; the stored result's read limit is 4 MiB).

## 7. The export

A long CSV in the eight fields every method's export has (`table`, `statistic`, `column_index`,
`column`, `column_index_2`, `column_2`, `value`, `word`): the rows used, the iterations and the
attempt; each loading and path (estimate, SE, C.R., P, standardized) by its two ends; each
covariance with its correlation; each variance and error variance; each squared multiple
correlation; each effect, total, direct and indirect, raw and standardized; every fit measure in
tables `fit`, `fit_saturated` and `fit_independence`; the rows left out; the notes; the
limitations; the manifest's hashes. A column is its index in the model's columns and its alias,
never a header; a factor is its number in the index field with its name field empty; a value not
defined is an empty field with its closed code in `word`. Pinned on macOS aarch64
(`tests/sem_export.rs`, `export_golden.csv`, 11,910 bytes); every other platform checks the pin's
layout until a Linux run gives its bytes.

## 8. Limitations

The three shared ones (`text_files_utf8_only`, `limits_by_memory`, `no_weights`), then:

| Code | Sentence |
|---|---|
| `ml_only` | Maximum likelihood is the only estimator; generalized and unweighted least squares, asymptotically distribution-free and Bayesian estimation are not offered. |
| `no_means` | Means and intercepts are not estimated. |
| `one_group_only` | One group is analysed; multiple-group models are not offered. |
| `indicators_measure_one_factor` | Each column measures at most one factor, and a factor's columns are not the ends of paths; cross-loadings and paths into a factor's column are not offered. |
| `equal_one_kind` | Each group of parameters held equal holds one kind of parameter; a path cannot be held equal to a variance, for example. |
| `marker_identification_only` | Each factor's scale is set by fixing its first column's loading to 1, as AMOS does when a model is drawn; fixing a factor's variance to 1 instead is not offered. |
| `no_fixed_values` | No parameter can be fixed at a value of your choosing; only the first loadings are fixed, at 1. |
| `effects_structural_only` | Total, direct and indirect effects are given among the factors and the columns that measure no factor; the effects on a factor's own columns are not listed. |
| `no_indirect_effect_tests` | Indirect effects have no standard errors or confidence intervals; bootstrapping is not offered. |
| `no_stability_index` | A model with a loop gets no stability index, so whether its effects settle is not assessed. |
| `no_modification_indices` | Modification indices are not computed. |
| `normal_theory_standard_errors` | The standard errors and the chi-square assume multivariate normality; bootstrap and robust corrections are not offered. |
| `no_srmr` | The standardized RMR is not computed; AMOS gives it only through a plugin. |
| `listwise_only` | A row is used only when every column in the model has a value; full information maximum likelihood is not offered. |

The sentences are `capabilityWording.ts`'s (decision 802); the window's own words for the run's
codes come with 9C-5.

## 9. What follows AMOS, and how it was checked

Each item is marked **reproduced** (from AMOS's printed output, with the example and page),
**formula** (from AMOS's documented formula, not checked against printed output) or **not
checked**. Sources: the IBM SPSS Amos 22 User's Guide (decision 743's URL).

- **Reproduced.** The discrepancy, S with divisor N, CMIN = (N − 1) F, and the standard errors
  from the expected information (Example 6, Models A to D; Example 7; Example 8).
- **Reproduced.** RAM's parameter count: each endogenous variable's error term with its weight
  fixed at 1 and its variance a parameter, the first loadings fixed at 1 (Example 6, p. 103:
  15 parameters, 21 moments, 6 DF).
- **Reproduced.** Parameters held equal as shared labels (Example 6, Models C and D, p. 115).
- **Reproduced.** A covariance between two endogenous variables is between their error terms
  (Example 6, Model B; Example 7).
- **Reproduced.** The standardized weights from the implied standard deviations of both ends,
  the errors' correlation from the two error variances, and the squared multiple correlation as
  1 minus the error variance over the implied variance (Example 7, pp. 131-135).
- **Reproduced.** The estimates' covariance matrix, through the critical ratios for differences
  (Example 6, Model B, pp. 112-113: 120 of 120).
- **Reproduced.** Appendix C's fit measures for path models (Example 6's four models; the
  measures not printed there are CFA's, reproduced in 9C-1 and 9C-2).
- **Formula.** Total, direct and indirect effects as (I − A)⁻¹ − I, A and their difference, and
  their standardized forms; checked against lavaan's total effects, not against AMOS's printed
  tables.
- **Not offered.** The stability index for a loop: AMOS prints 0.003 for Example 7, which for a
  two-variable loop is |b₁₂ b₂₁|, but the guide does not define it and a two-variable loop cannot
  tell candidate definitions apart (`no_stability_index`).
- **Differs.** AMOS fits whatever converges; Gaply refuses a model not identified from the spec
  alone (`model_not_identified`) and a solution the data barely determine
  (`solution_not_identified`), and accepts Newton's stop only at a minimum.
- **Differs.** AMOS lets one label hold parameters of different kinds; Gaply refuses it
  (`equal_mixed_kinds`, limitation `equal_one_kind`).
- **Not checked.** AMOS's own iteration path, its starting values and its choice among local
  minima: Gaply's three attempts are its own (decision 798), and where two minima exist it
  reports the first attempt's.

## 10. Blueprint §18.2 acceptance

Against the blueprint's seven items (§18.2), as 9C-4 leaves the method:

- **Formula, method and variant documented**: §2, and §9 against AMOS.
- **Golden fixtures and edge cases pass**: lavaan and an independent R refinement as the oracle
  (`sem_r_golden.rs`), AMOS's printed Examples 6, 7 and 8 (`sem_amos.rs`, and the 120 critical
  ratios in `stats_sem`'s unit tests), the edge cases, the attempts and the widest model
  (`sem_cases.rs`).
- **Missing values, ties, weights, precision and sample size handled**: listwise, each row once
  under its first column without a value; codes; no weights (a limitation); at least the columns
  plus 1 complete rows; precision as §5 gives it.
- **Diagnostic meanings and limitations documented**: §3 and §8.
- **Independent reference comparison completed**: lavaan 0.7-3 through R 4.6.1, an independent R
  refinement, and AMOS's printed output (§5, §9).
- **Unsupported designs return explicit reasons**: every refusal a closed code with its params;
  identification refused from the spec alone and at the solution.
- **Result manifest and export verified**: the export pinned (`sem_export.rs`), aliases only in
  the result, the export and the manifest (`stats_sem_privacy_matrix.rs`), the wire fixtures
  (`stats_sem_wire.rs`).

Not yet: a window, a project row and a rerun (9C-5), the real-app checklist
(`realapp_9c4.md`, which only the user runs, and which can show only the card and Help), and
Windows and Linux runs.
