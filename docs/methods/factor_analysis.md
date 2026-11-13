# Exploratory factor analysis, as an analysis

`stats.analyze.factor`, version 1, engine version 1. Research OS, Phase 9B-1
(decisions 699 to 706), with its rotations made SPSS's in Phase 9B-2 (decisions
710, 712 to 721), `src-tauri/gaply-core/src/stats_factor/`, with its linear
algebra in `src-tauri/gaply-core/src/stats_linalg/`.

**Status: Beta, in the Statistics window since 9B-3.** 9B-1 built the core, registered it
(Planned at its Part 2's end, decision 702) and made it Beta with six commands and a run
kind (decision 704). Since Research OS, Phase 9B-3 (decisions 726 to 735) a user reaches it
from Analyze, Dimension reduction, Factor…: the dialog draws the numeric columns on the left
and the chosen items on the right in the file's order, with principal components, every
factor whose eigenvalue is above 1, no rotation, 25 iterations and a convergence of 0.001
chosen at the start, and Run off until 3 to 128 items are chosen. The result is a card of
SPSS's tables, "Component" for principal components and "Factor" for principal axis (§8).
A run is saved into a project with its own row, read back through the stored checker
(`storedShape.ts`, decision 733), and run again Identical. Coming later no longer says
"Factor analysis".

Exploratory factor analysis of three to 128 numeric items, as SPSS's FACTOR does
it (decision 710), with principal components or principal axis factoring, for a
number of factors the user gives or for every eigenvalue of the correlation
matrix above 1, and with two factors or more a varimax or promax rotation as
SPSS computes them: Kaiser's pairwise varimax with SPSS's stopping rule and
iteration limit, its factors reflected and sorted, and promax from a
row-normalised target (IBM's FACTOR algorithms, §9). The oracles: R 4.6.1 (base
R's `cor`, `eigen`, `solve`, `chol`, `determinant`, `pchisq`, `qr`) for
everything before the rotation; EFAtools 1.1.0's SPSS route, checked against a
second route in base R written from IBM's document, for the rotations; SPSS's own
solutions as EFAtools ships them; and psych 2.6.9 as a second tier for KMO,
Bartlett's test and principal axis (§5). Each algorithm is written from its
published statement, not from any tool's source (ADR-008).

## 1. Inputs and the spec

`FactorParams` (camelCase, unknown fields refused):

| field | meaning |
|---|---|
| `items` | 3 to `MAX_ITEMS` (128) aliases, in the dataset's column order |
| `extraction` | `principal_components` or `principal_axis` |
| `factors` | `null` for every eigenvalue above 1 (Kaiser's rule, SPSS's default), or m from 1 to the items (components) or to one fewer (principal axis) |
| `rotation` | `none`, `varimax` or `promax` |
| `iterations` | 1 to 1,000, SPSS's ITERATE, limiting principal axis's iterations and varimax's cycles alike; the window starts at 25 |
| `convergence` | 1e-12 to 0.1, SPSS's ECONVERGE; the window starts at 0.001 |
| `missingCodes` | finite, ascending, once each, applied to every item |

The checks, in this order: the missing codes; the items' count; their order; a
repeated item; each item (unknown, a non-finite value, text); a code equal to a
score's value; the factors; the iterations; the convergence. Then, only with no
other error, the first of: no complete row, fewer than three rows, an item with
one value. Any item may be a scale score. A data table holds at most 128 columns,
so `too_many_items` is reached only by naming a column twice.

## 2. Formulas, and where each number comes from

Rows are listwise: a row is used only when every item holds a number that is not
a code; a row left out is counted once, under the first item without one.

- **Descriptives:** each item's mean (`stats_verify::mean`) and SD (n − 1).
- **Correlations:** Pearson's, each pair from `stats_alpha`'s own `correlation()`
  in alpha's pair order, bit for bit; one it cannot give refuses the run
  (`correlation_out_of_range`), naming the first item, in the dataset's order,
  whose own variance the arithmetic cannot give, or the pair's first item when
  every item's can (decision 714).
- **The Cholesky factor** of R (`stats_linalg::cholesky`) refuses at the first
  row whose value left before its square root is at most 1e-9 (each a_jj is 1):
  that item's squared multiple correlation with the items before it is at least
  1 − 1e-9.
- **Determinant** Π l_jj², taken in order (it never rises, so a value below the
  smallest normal double is `too_small`); **log** 2 Σ ln l_jj.
- **KMO and each MSA:** q_ij = −(R⁻¹)_ij / √((R⁻¹)_ii (R⁻¹)_jj), with R⁻¹ from the
  Cholesky factor; KMO = Σ_{i≠j} r² / (Σ_{i≠j} r² + Σ_{i≠j} q²), each MSA the same
  over its row. With no correlation at all both sums are 0 and the value is
  `no_correlation` (R gives NaN; decision 714).
- **Bartlett's test:** χ² = −(n − 1 − (2p + 5)/6) ln|R|, df = p(p − 1)/2, p from
  `stats_dist::chi_square_upper_wide` (Level 0's function, measured to df 8,128).
  ln|R| ≤ 0 for a correlation matrix (Hadamard's inequality); a log above 0 is
  rounding, and χ² is then 0.
- **Eigenvalues** of R (`stats_linalg::eigen_symmetric`, cyclic Jacobi with
  Rutishauser's update), all p, descending, each with λ/p × 100 and the
  cumulative percent. Each eigenvector's sign makes its sum positive, or, where
  the sum is 0 within 1e-10 of its 1-norm, its first entry above 1e-10.
- **m:** the eigenvalues strictly above 1, or the spec's number.
- **Principal components:** loadings v_j √λ_j for j ≤ m (each λ_j above 0, else
  `extraction_eigenvalue_not_positive`); communalities 1 initially, the rows' sums
  of squares after.
- **Principal axis factoring:** initial communalities the squared multiple
  correlations 1 − 1/(R⁻¹)_ii (refused as `correlations_singular`, naming the item,
  where the Cholesky factor refuses); each iteration puts them on R's diagonal,
  takes the m largest eigenvalues (by signed value) and their vectors, forms the
  loadings v_j √|λ_j|, as SPSS does (IBM, pp. 1 to 2: Γ = diag(|γ_1|, …, |γ_m|);
  decision 717), noting `extraction_eigenvalue_negative` when any iteration used
  one below 0, and takes the rows' sums of squares as the new communalities; it
  stops when the largest change is below the convergence, or refuses as
  `extraction_not_converged` at the iterations' limit.
- **Sums of squared loadings** per factor, with percent of p and cumulative.
- **Varimax** (m ≥ 2), as SPSS computes it (IBM, pp. 10 to 12; decision 715):
  - Kaiser's row normalisation, each row over w_i = √Σ_j λ_ij², a row with w_i
    below 1e-15 left unscaled (an item with no loadings stays 0 throughout);
  - T from the identity; SV = Σ_j (p Σ_i z_ij⁴ − (Σ_i z_ij²)²), not divided by p²,
    every power a product, before the first cycle;
  - one cycle rotates every pair of factors (j, k), j < k: u_i = z_ij² − z_ik²,
    v_i = 2 z_ij z_ik, A = Σu, B = Σv, C = Σ(u² − v²), D = Σ2uv, X = D − 2AB/p,
    Y = C − (A² − B²)/p, and the angle P = ¼ atan2(X, Y), whose cosine and sine
    are found by halving twice with no atan2, sin or cos; a pair with |sin P| at
    most 1e-15 is not rotated; columns j and k of Z and of T become
    z_j cos P + z_k sin P and −z_j sin P + z_k cos P;
  - after each cycle SV again; the rotation stops at the first cycle whose |ΔSV|
    is at most 1e-4 (§9 says where this comes from), within the spec's
    iterations; past them, no rotation is given and the note
    `rotation_not_converged` says so;
  - each row times w_i; then each factor whose loadings sum below 0 is reversed
    with its column of T (step f), by decision 699's sign rule: where |sum| is at
    most 1e-10 of the column's 1-norm, the first entry above 1e-10 decides instead,
    where SPSS (and EFAtools) reverse on the sign of the sum alone; and the factors are sorted by their sums of
    squared loadings, largest first, ties in order, with T's columns (step g). A
    rotated row is the extracted row times T. Its iterations are the cycles.
- **Promax** at power 4 (IBM, pp. 18 to 19; decision 716), from that varimax's
  loadings Λ_R: the target p_ij = λ*_ij |λ*_ij|³ with λ*_ij = λ_ij / h_i (each row
  normalised, a row below 1e-15 left as it is); L the least squares of Λ_R L = P,
  no intercept (`stats_linalg::least_squares`); D = diag(LᵀL)^(-1/2) and Q = L D;
  C = diag((QᵀQ)⁻¹)^(-1/2); the pattern Λ_R Q C⁻¹, the factor correlations
  Φ = C (QᵀQ)⁻¹ C and the structure the pattern times Φ. Promax neither reflects
  nor sorts after its fit. Where the least squares or the Cholesky factor of QᵀQ
  refuses, no rotation is given and the note `rotation_singular` says so. Its
  iterations are varimax's cycles.

## 3. Notes and not-computable values

Notes, as bare codes, in this order: `communality_above_one` (a principal axis
iteration or the result holds one; principal components cannot, beyond rounding,
and are not checked); `extraction_eigenvalue_negative`; `one_factor_not_rotated`;
`rotation_not_converged`; `rotation_singular`; `eigenvalues_not_distinct` (the
m-th and next eigenvalue of the final decomposition within 1e-8 of the larger).
A rotation that cannot be given is a note, never a refusal: the result keeps
everything else and its rotation is null (decision 717). Since 9B-3 (decision 728) promax
is `rotation_singular` too when any column of varimax's loadings has a length at most
1e-7 times the longest's: such a factor is rounding noise, which least squares' own rank
test (each column against its own norm) let through, and EFAtools' SPSS route refuses.
Before it, the golden's `data_uncorrelated_item_varimax.csv` with principal axis, 4
factors and promax gave `rotation_singular` unscaled but a Φ row of rounding noise times
0.1 or divided by 10. Φ is computed on its upper triangle and mirrored, so it is exactly
symmetric and the structure is exactly the pattern times it; its diagonal stays as
computed, within 4.4e-16 of 1, as SPSS's own within 2.2e-16.

Not computable, statistic by statistic: `correlations_singular` for the
determinant and its log, KMO, the MSAs and Bartlett's test where the Cholesky
factor refuses; `no_correlation` for a KMO or an MSA of 0/0; `too_small` for a
determinant below the smallest normal double and for any other 0/0; `overflow`
for a value too large; `distribution_out_of_range` where the chi-square refuses.
With the Cholesky factor holding, ln|R| ≥ p ln(1e-9) is finite and df ≤ 8,128,
so neither of the last two is reached for Bartlett's p by any dataset.

## 4. The check points

One before the arithmetic, one before each item's row, one before each of the
p(p − 1)/2 correlations, one before the extraction and one before the rotation.
Every principal axis iteration and every varimax cycle calls its point again at
an unchanged count, so a cancel or the run manager's 30 s ceiling stops the run
at any iteration or cycle while the progress does not move (`factor_cases.rs`;
`stats_factor_commands.rs`, on an injected clock and at the rotation point's
second call).

## 5. Precision, the gates and the oracles

**The linear algebra** (`stats_linalg_r_golden.rs`, 24 matrices of 3 to 128
rows, condition numbers 1 to 1.5e10, near-ties and exact ties, against R's
`eigen`, `chol`, `chol2inv`, `determinant` and `qr.solve`): each family's gate is
ten times its measured error, tiered by condition number; the table is in the
file. The six polar factor matrices of 9B-1 went with the polar factor (decision
719); the other 24 cases' rows are byte for byte as 9B-1 measured them.

**The chi-square beyond df 2,401** (`make_factor_dist_fixtures.R`, 88 rows, df
1,225 to 8,128, tails 0.9 to 1e-300): worst relative error 8.965e-12 (df 8,000,
tail 0.05), gate 9e-11.

**The method** (`factor_r_golden.rs` over `make_factor_fixtures.R`, 42 cases: 38
results and 4 refusals, 11,049 numbers compared, 6 skipped within a tie). Gates
by family and condition tier (≤ 1e3, ≤ 1e8, > 1e8), ten times the golden's
measurement; the largest measured is 5.699e-10, a determinant at condition 1e7.
The rotation families are against EFAtools' SPSS route since 9B-2: rotation
6.717e-14, structure 3.385e-14, factor correlations 2.530e-14; varimax's cycles
compared exactly. The script itself stops unless EFAtools' route and a second
route in base R from IBM's document agree within 1e-12 with equal cycles (they
agreed within 1.998e-15), IBM's D and C give EFAtools' pattern and Φ within
1e-13 (1.443e-15), and its principal axis loop gives EFAtools' `.PAF` within
1e-10 (5.029e-14). New cases: the review's dataset (formula(2000, 20, 2) at three
factors, both extractions, both rotations, 2 cycles, where the review of 9B-1
found R's varimax needing 1,421 and 2,779 iterations, decision 713); a varimax reflected by step f and reordered by step g; a
varimax not converged at iterations 2; a structure whose cycles differ under a
rule on SV's relative change; principal axis using a negative eigenvalue; and the
uncorrelated item, now rotated, its row 0.

**SPSS's own output** (`src/stats_factor/spss_tier.rs`, from EFAtools' data
`SPSS_23` and `SPSS_27`, written by `make_factor_spss_fixtures.R`): SPSS's own
unrotated principal axis loadings rotated by Gaply's varimax and promax. The eight
solutions on EFAtools' four simulated models are committed: varimax within
1.527e-15, promax pattern 1.887e-15, factor correlations 1.665e-15, every cycle
count the one that reproduces SPSS's varimax. The other nine come from real
studies (IDS-2, WJIV, DOSPERT, NEO-PI-R) and were checked in the session only:
all 17 within 3.136e-15, every cycle count equal.

**The psych tier** (`make_factor_psych_fixtures.R`, psych 2.6.9): KMO 3.331e-16,
Bartlett 1.339e-13, principal axis communalities and loadings within 2.930e-10
(psych stops on the change in the communalities' sum, Gaply on the largest
change). psych's rotations follow R's, not SPSS's, so the tier has held no
rotation since 9B-2 (decision 719).

**The bug hunts.** 9B-1's (seed 20260701, 240 datasets) found no defect; its
record is in its report. 9B-2's (seed 20260719, 240 datasets, 3 to 128 items, 1
to 8 true factors, over-extraction by up to 4, near-singular, exactly singular,
Heywood-prone, Likert, codes and missing values, headers outside ASCII on every
third, both extractions, both rotations, iterations 25 and 1,000) against
EFAtools' SPSS route: 157 results and 57 refusals, each R's own by code and
parameter; 149 rotations with their cycles equal; 67,291 numbers compared; 24
Cholesky borderlines and 2 datasets near the |ΔSV| criterion skipped. Two
numbers were above the golden's gates (three failure lines, one being Φ's
symmetric twin), each shown to be rounding (the golden's file says how): h215's
factor correlations, 2.783e-13 (EFAtools' own Φ moves by 2.726e-13 between
Gaply's and R's extracted loadings, which differ by 2.916e-14; Gaply's promax on
its own loadings gives EFAtools' Φ within 7.494e-15), and h232's seventh
eigenvalue's percent, 1.026e-13 (R's `eigen` disagrees with itself by 9.226e-14
on that matrix, with vectors against values only, and Gaply lies within 4.0e-15
of its values-only answer in five item orders). The hunt produced no
`rotation_not_converged` and no `rotation_singular`; both are reached by data: the first
by the golden's and the cases' datasets, the second by the golden's
`data_uncorrelated_item_varimax.csv` with principal axis, 4 factors and promax (the
golden's case `uncorrelated_item_pa_promax` since 9B-3, decision 728; EFAtools refuses its
promax, `lm.fit` rank 3 of 4), by the same data times 0.1 and divided by 10, and by
`data_three_factor_12.csv` with a copy of item 4 and every component (`factor_cases.rs`).
The review of 9B-2 said this page, 9B-2's report and D322 had it reached only by unit
tests; that was wrong (the review's m1).

## 6. Speed and size

Release, 100,000 rows, median of seven, an Apple M1 with 8 GB, 12 Nov 2026:
principal axis with promax at iterations 25 (m = 3, 8 and 8; the data's eight
true factors), 10 items 33.97 ms (peak footprint
149 MB), 50 items 729.49 ms (718 MB), 128 items 4,654.06 ms (1.81 GB); 128 items
with principal components and varimax at iterations 1,000, m = 64 4,541.28 ms
(22 cycles) and m = 127 4,648.31 ms (21 cycles); on items with no common factor,
m = 64 4,552.28 ms (38 cycles) and m = 127 4,698.94 ms (29 cycles), against
4,527.00 ms with no rotation, so about 5.9 ms a cycle at m = 127. The 30 s
ceiling is far off at every size; a rotation reaching its 1,000 cycles at m = 127
would add about 6 s, and is stopped by its check point in any case.

The largest result the spec allows, every number at its widest and four notes,
is 2,982,612 bytes (2.844 MiB). A save stores more: the spec, the whole bundle
(the result, its diagnostics, its limitations and its manifest) and the manifest
again. Measured on a real 128-item run (`stats_factor_saved_size.rs`): the spec
1,241 bytes, the bundle less the result 1,241, the manifest 482; with the widest
result, 2,985,576 bytes, 1,208,728 under the 4 MiB a saved result may be read at.

## 7. The export

`export.rs`: the long CSV of every method, eight fields. An item is its index in
the spec's list and its alias; a factor, an eigenvalue or a component is its
number from 1 in the second index field; a line of a factor by a factor (the
transformation, the factor correlations) gives the first factor's number in the
first index field. Pinned exactly on macOS aarch64 and on Linux x86_64 (`factor_export.rs`,
since 9B-3: the user's Linux run gave the pin's bytes exactly, decisions 726 and 728),
checked by layout elsewhere. The window offers it as `factor_analysis.csv` and the
manifest as `factor_analysis_manifest.json`.

## 8. Limitations

The shared three (text files UTF-8 only, limits set by memory, no weights), then:
principal components and principal axis only; varimax and promax only; promax at
power 4 only; no factor scores; the correlation, anti-image, reproduced and
residual matrices not shown; Pearson's correlations only; listwise only; no scree
plot.

What the window says for each code (`factorWording.ts`, Research OS, Phase 9B-3, decision
730), word for word, an item named C4 and the counts as shown here. A limitation is the
registry's sentence for its limit. The run's, the registry's and the workspace's 22 codes
are the related rank tests' sentences, but `run.unknown_run` ("This run is no longer held.
Gaply keeps the last 40 runs in memory only; run the factor analysis again.") and
`run.wrong_kind` ("This run is not a factor analysis, so this screen cannot read it.").

| code | sentence |
|---|---|
| `capability_version_mismatch` | These settings were made for version 2 of the factor analysis, and this is version 1. |
| `code_equals_score` | Column C4 is a score, and it holds a missing code you ticked in 3 cells. Untick the code, or leave this score out. |
| `column_holds_text` | Column C4 holds text in 3 cells, so it cannot be used as an item. |
| `column_non_finite` | Column C4 holds an infinity or "not a number" in 3 cells, so it cannot be used as an item. |
| `communality_above_one` | During principal axis factoring a communality went above 1 (a Heywood case). A communality is a share of an item's variance, so these loadings do not describe a possible factor model as they stand. SPSS gives the same warning. |
| `convergence_out_of_range` | The convergence is outside 1e-12 to 0.1, or is not a number. |
| `correlation_out_of_range` | C4's values are too large or too small for its correlations to be computed at this scale. |
| `correlations_singular` | Not computable: the correlation matrix is singular, as when an item is a combination of other items or there are no more rows than items. |
| `correlations_singular` (a refusal, naming C4) | C4 is a combination of the items before it, or within one part in a billion of its variance of being one, so the correlation matrix is singular and principal axis factoring cannot start. Take it out of the items, or use principal components. |
| `distribution_out_of_range` | Not computable: this needs a chi-square distribution beyond the range where Gaply has measured its accuracy. |
| `eigen_not_converged` | The eigenvalues of these data could not be found to the arithmetic's precision. |
| `eigenvalues_not_distinct` | The last eigenvalue extracted and the next are equal within 1e-8 of the larger, so these loadings are one of many that fit equally well. |
| `extraction_eigenvalue_negative` | At some iteration an eigenvalue among those extracted was below 0; as SPSS does, its absolute value was used for the loadings. |
| `extraction_eigenvalue_not_positive` | A component asked for has an eigenvalue of 0 or below, so it has no loadings. Ask for fewer components. |
| `extraction_not_converged` | Principal axis factoring did not converge within 25 iterations. Raise the maximum iterations, or ask for fewer factors. |
| `factors_out_of_range` | 7 factors are asked for, and these items allow 1 to 5. |
| `input_count` | These settings name 2 files, and a factor analysis reads exactly one. |
| `input_mismatch` | These settings were made for a different file from the one open. Open the file again. |
| `item_constant` | Column C4 takes one value in every complete row, so its correlations are not defined. Take it out of the items. |
| `item_repeated` | Column C4 is chosen twice as an item. |
| `items_out_of_order` | These settings list item C4 out of the file's column order, so Gaply refuses them rather than reorder them. |
| `iterations_out_of_range` | 0 iterations are asked for, and a run takes 1 to 1000. |
| `listwise_only` | A row is used only when every item has a value; pairwise deletion and mean substitution are not offered. |
| `missing_code` | 3 rows held a missing code you ticked in C4, the first item without a value, and were left out. |
| `missing_code_not_finite` | Missing code 1 is not a finite number. |
| `missing_code_repeated` | Missing code 2 is listed twice. |
| `missing_codes_out_of_order` | Missing code 2 is out of ascending order, so Gaply refuses these settings rather than reorder them. |
| `missing_value` | 3 rows had no value (an empty cell or a missing-value token) in C4, the first item without one, and were left out. |
| `no_complete_rows` | No row has a value in every chosen item, so there is nothing to analyse. |
| `no_correlation` | Not computable: the items this measure covers correlate with no other item, so there is nothing for it to compare. |
| `no_eigenvalue_over_one` | No eigenvalue of the correlation matrix is above 1, so that rule extracts no factor. Choose a fixed number of factors instead. |
| `no_factor_scores` | Factor scores are not computed or saved as columns. |
| `no_matrices_shown` | The correlation, anti-image, reproduced and residual matrices are not shown; the determinant, KMO and each item's sampling adequacy are. |
| `no_ml_or_other_extraction` | Principal components and principal axis factoring are offered; maximum likelihood, least squares, alpha and image extraction are not. |
| `no_oblique_other_than_promax` | Varimax and promax rotations are offered; oblimin, quartimax and equamax are not. |
| `no_scree_plot` | The scree plot is not drawn; the eigenvalues it would show are listed. |
| `no_weights` | Each row is counted once; weighting rows by a column is not offered. |
| `one_factor_not_rotated` | One factor was extracted, so there is nothing to rotate. |
| `overflow` | Not computable: a value along the way is too large for the arithmetic. |
| `pearson_only` | Correlations are Pearson's; polychoric correlations for ordinal items are not offered. |
| `preparation_mismatch` | These settings do not match how the open data was prepared. Choose the settings again. |
| `promax_kappa_four` | Promax uses a power of 4, SPSS's default; another power is not offered. |
| `rotation_not_converged` | The rotation did not converge within the maximum iterations, so no rotated solution is given. |
| `rotation_singular` | Promax's fit cannot be computed for these loadings, so no rotated solution is given. |
| `suspect_missing_code` | Column C4 holds -99 in 3 cells, which may be a code for a missing value. It is counted as a number unless you tick it in Settings. |
| `too_few_items` | 2 items chosen, and a factor analysis needs at least 3. |
| `too_few_rows` | 2 rows have a value in every chosen item, and a factor analysis needs at least 3. |
| `too_many_items` | 129 items chosen, and a factor analysis takes at most 128. |
| `too_small` | Not computable: a value along the way is too small for the arithmetic to keep its precision. |
| `unknown_capability` | These settings name a capability Gaply does not have here. |
| `unknown_column` | Column C4 is not in this file. |

## 9. What follows SPSS, and how it was checked

Read, each on 12 Nov 2026:
- IBM, "FACTOR" algorithms, SPSS Statistics 14.0
  (https://public.dhe.ibm.com/software/analytics/spss/support/Stats/Docs/Statistics/Algorithms/14.0/factor.pdf),
  pp. 1 to 2, 10 to 13 and 18 to 19;
- IBM, SPSS Statistics 23.0.0 Command Syntax Reference, FACTOR and its CRITERIA
  subcommand (https://www.ibm.com/docs/en/SSLVMB_23.0.0/spss/base/syn_factor.html
  and .../syn_factor_criteria.html);
- EFAtools 1.1.0 from CRAN (Grieder and Steiner, Behavior Research Methods 54,
  54 to 74, 2022): its SPSS presets and functions printed, and its
  `src/paf_iter.cpp` read, as an oracle only. Nothing of it is in Gaply.
IBM's current Algorithms guide's FACTOR chapter was not read: its pages returned
no body; whether it exists is not checked.

- **What follows SPSS.** Principal components; principal axis with absolute
  eigenvalues (p. 1's Γ, EFAtools' `abs_eigen` for SPSS); varimax's pairwise
  rotations, its skip of a pair at |sin P| ≤ 1e-15, its reflection (but see the
  sign rule below) and its sort (pp. 10 to 12); promax's row-normalised target, D, C, pattern, Φ and structure
  (pp. 18 to 19); Bartlett's test and KMO (p. 20, as 9B-1 read it; not read
  again in 9B-2).
- **The stopping rule, which IBM's document does not give as SPSS runs it.**
  p. 12 says iteration "is terminated when |SV(i) − SV(i−1)| ≤ 10⁻⁵" with SV
  divided by n² (p. 10). That rule reproduces SPSS's own varimax in 3 of
  EFAtools' 17 SPSS solutions. The rule Gaply uses, |ΔSV| ≤ 1e-4 on SV not
  divided by p², is RCONVERGE's default in the syntax ("The default is 0.0001",
  "Convergence criterion for rotation") and reproduces all 17: in each, exactly
  one cycle count gives SPSS's varimax within 1e-13, and it is the first cycle
  where the rule holds (the largest change where SPSS stopped 9.451e-5, the
  smallest where it went on 1.908e-4). Inferred, not documented. The 17 also fit a
  second rule, about 2.6e-5 on |ΔSV| / √p (the review of 9B-2, m5); the two part only
  at large p, and the 17 have p of at most 47, so they cannot tell them apart. 1e-4 is
  RCONVERGE's default and the natural reading.
- **The licence of SPSS's numbers as Gaply holds them.** `spss_solutions.tsv` holds
  numbers from EFAtools' data SPSS_23 and SPSS_27 (EFAtools 1.1.0, GPL-3), written
  beside their provenance in its header since 9B-3 (decision 728). They are test data
  only; nothing of EFAtools is in Gaply.
- **The limit.** ITERATE "Maximum number of iterations for solutions in the
  extraction or rotation phases", default 25 (syntax reference): the spec's
  iterations limit varimax's cycles too.
- **How it was checked against SPSS itself.** SPSS's own varimax, promax pattern
  and factor correlations for 17 solutions (§5), from SPSS's own unrotated
  loadings. Gaply's extraction was not compared with SPSS's own unrotated
  loadings, and no dataset was run through SPSS in this program.
- **Not given.** SPSS gives a Rotation Sums of Squared Loadings total for promax;
  Gaply gives none (decision 705).
- **Not documented.** SPSS's sign rule for its unrotated matrices: Gaply's is
  decision 699's. The power's default (4) is not stated in IBM's document; Gaply
  fixes it at 4. Kaiser's rule (MINEIGEN) and SPSS's handling of a communality
  above 1 are not stated there; not checked.
- **Step f's sign rule differs at the edge.** Where a factor's loadings sum to 0
  within 1e-10 of their 1-norm, Gaply's reflection follows decision 699's rule
  (the first entry above 1e-10), where SPSS and EFAtools reflect on the sign of
  the sum alone. Not compared: the hunt found no such factor and skips them.
- **X = Y = 0.** Gaply's angle gives no rotation there. atan2(±0, −0) would give
  a 45° turn, but Y is a sum built from +0, never −0, so that case does not arise
  from data (the review of this step).
- **Principal axis's stop.** IBM's words are "less than the convergence
  criterion", as Gaply's; EFAtools' SPSS route stops at "at most". They differ
  only when the change equals the criterion exactly.

## 10. Blueprint §18.2 acceptance

Inputs refused by closed code before any arithmetic; every number from a
deterministic engine; aliases only in the result, the export and the manifest
(`stats_factor_privacy_matrix.rs`); cancel and the ceiling cooperative at every
iteration and cycle; Reproducibility::Bitwise (`factor_cases.rs`,
`factor_export.rs`).
