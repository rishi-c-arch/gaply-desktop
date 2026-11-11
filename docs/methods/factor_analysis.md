# Exploratory factor analysis, as an analysis

`stats.analyze.factor`, version 1, engine version 1. Research OS, Phase 9B-1
(decisions 699 to 706), `src-tauri/gaply-core/src/stats_factor/`, with its linear
algebra in `src-tauri/gaply-core/src/stats_linalg/`.

**Status: Beta, with no window yet.** 9B-1 built the core, registered it (Planned
at its Part 2's end, decision 702) and made it Beta with six commands and a run
kind (decision 704). No window item reaches it until 9B-2: the old home lists its
card, Help lists it with its limitations, Coming later still says "Factor
analysis", and a saved run's report refuses it.

Exploratory factor analysis of three to 128 numeric items, as SPSS's FACTOR does
it, with principal components or principal axis factoring, for a number of
factors the user gives or for every eigenvalue of the correlation matrix above 1,
and with two factors or more a varimax or promax rotation. R 4.6.1 is the oracle
(base R's `cor`, `eigen`, `solve`, `chol`, `determinant`, `pchisq`, `varimax` and
`promax`), with psych 2.6.9 as a second tier (§5). Each algorithm is written from
its published statement, not from any tool's source (ADR-008): Kaiser's (1958)
varimax with his row normalisation, Hendrickson and White's (1964) promax, and
principal axis factoring as SPSS states it.

## 1. Inputs and the spec

`FactorParams` (camelCase, unknown fields refused):

| field | meaning |
|---|---|
| `items` | 3 to `MAX_ITEMS` (128) aliases, in the dataset's column order |
| `extraction` | `principal_components` or `principal_axis` |
| `factors` | `null` for every eigenvalue above 1 (Kaiser's rule, SPSS's default), or m from 1 to the items (components) or to one fewer (principal axis) |
| `rotation` | `none`, `varimax` or `promax` |
| `iterations` | 1 to 1,000, SPSS's ITERATE; the window will default to 25 |
| `convergence` | 1e-12 to 0.1, SPSS's ECONVERGE; the window will default to 0.001 |
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
  (`correlation_out_of_range`, naming the pair's first item).
- **The Cholesky factor** of R (`stats_linalg::cholesky`) refuses at the first
  row whose value left before its square root is at most 1e-9 (each a_jj is 1):
  that item's squared multiple correlation with the items before it is at least
  1 − 1e-9.
- **Determinant** Π l_jj², taken in order (it never rises, so a value below the
  smallest normal double is `too_small`); **log** 2 Σ ln l_jj.
- **KMO and each MSA:** q_ij = −(R⁻¹)_ij / √((R⁻¹)_ii (R⁻¹)_jj), with R⁻¹ from the
  Cholesky factor; KMO = Σ_{i≠j} r² / (Σ_{i≠j} r² + Σ_{i≠j} q²), each MSA the same
  over its row. With no correlation at all both sums are 0 and the value is
  `too_small` (R gives NaN).
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
  takes the m largest eigenvalues and vectors of that matrix, forms the loadings,
  and takes the rows' sums of squares as the new communalities; it stops when the
  largest change is below the convergence, or refuses as
  `extraction_not_converged` at the iterations' limit.
- **Sums of squared loadings** per factor, with percent of p and cumulative.
- **Varimax** (m ≥ 2): each row of the loadings divided by its length h_i; T from
  the identity; each iteration Z = X T, B = Xᵀ(Z³ − Z diag(1ᵀZ²)/p), T the polar
  factor U Vᵀ of B (`stats_linalg::polar_factor`, one-sided Jacobi) and d the sum
  of B's singular values, until d < d_prev (1 + 1e-5), within 1,000 iterations;
  the loadings X T times h_i. An item whose extracted loadings are all exactly 0
  is refused first (`rotation_item_without_loadings`).
- **Promax** at power 4: varimax's loadings A and T; the target Q = A ∘ |A|³; U the
  least squares of A U = Q with no intercept (`stats_linalg::least_squares`); U
  scaled by √diag((UᵀU)⁻¹); the pattern A U, W = T U, the factor correlations
  Φ = (WᵀW)⁻¹ and the structure the pattern times Φ.
- **Signs after a rotation:** a factor whose loadings sum below 0 (the same rule
  as the eigenvectors') is reversed with its transformation and structure
  columns and its row and column of Φ. No factor is reordered.

## 3. Notes and not-computable values

Notes, as bare codes: `communality_above_one` (a principal axis iteration or the
result holds one; principal components cannot, beyond rounding, and are not
checked); `one_factor_not_rotated`; `eigenvalues_not_distinct` (the m-th and next
eigenvalue of the final decomposition within 1e-8 of the larger).

Not computable, statistic by statistic: `correlations_singular` for the
determinant and its log, KMO, the MSAs and Bartlett's test where the Cholesky
factor refuses; `too_small` for a determinant below the smallest normal double,
and for a 0/0 (KMO with no correlation); `overflow` for a value too large;
`distribution_out_of_range` where the chi-square refuses. With the Cholesky
factor holding, ln|R| ≥ p ln(1e-9) is finite and df ≤ 8,128, so neither of the
last two is reached for Bartlett's p by any dataset.

## 4. The check points

One before the arithmetic, one before each item's row, one before each of the
p(p − 1)/2 correlations, one before the extraction and one before the rotation.
Every principal axis and varimax iteration calls its point again at an unchanged
count, so a cancel or the run manager's 30 s ceiling stops the run at any
iteration while the progress does not move (`factor_cases.rs`;
`stats_factor_commands.rs`, on an injected clock).

## 5. Precision, the gates and the oracles

**The linear algebra** (`stats_linalg_r_golden.rs`, 30 matrices of 3 to 128 rows,
condition numbers 1 to 1.5e10, near-ties and exact ties, against R's `eigen`,
`chol`, `chol2inv`, `determinant`, `svd` and `qr.solve`): each family's gate is
ten times its measured error, tiered by condition number; the table is in the
file. 284 eigenvectors compared, 17 skipped where an eigenvalue gap is below 1e-6
of the largest. One matrix within a factor of 2 of the Cholesky rule is counted
and not compared.

**The chi-square beyond df 2,401** (`make_factor_dist_fixtures.R`, 88 rows, df
1,225 to 8,128, tails 0.9 to 1e-300): worst relative error 8.965e-12 (df 8,000,
tail 0.05), gate 9e-11.

**The method** (`factor_r_golden.rs` over `make_factor_fixtures.R`, 33 cases: 28
results and 5 refusals, 8,756 numbers compared, 6 skipped within a tie). Gates by
family and condition tier (≤ 1e3, ≤ 1e8, > 1e8), ten times the larger of the
golden's and the bug hunt's measurement; the largest measured is 5.699e-10, a
determinant at condition 1e7.

**The psych tier** (`make_factor_psych_fixtures.R`, psych 2.6.9): KMO 3.331e-16,
Bartlett 1.339e-13, principal axis communalities and loadings within 2.930e-10
(psych stops on the change in the communalities' sum, Gaply on the largest
change), varimax 2.134e-10. psych's `Promax` calls `GPArotation::Varimax`, which
does not normalise rows, so it is promax without Kaiser's normalisation; it
differs from Gaply's by up to 2.059e-4, and from the same steps in base R
unnormalised by up to 7.900e-5 (GPArotation's own tolerance), so the difference
is the normalisation.

**The bug hunt** (seed 20260701, 240 datasets, 3 to 128 items, headers outside
ASCII on every third): 148 results and 68 refusals, each R's own by code and
parameter; 43,515 numbers compared; 24 datasets within a factor of 100 of the
Cholesky rule skipped; no failure after three gates were set from its own
measurement (a standard deviation one ulp from R's; promax's structure and factor
correlations on four principal axis factors from five items, where R's own
promax moves by 3.7e-13 under a 1e-13 change of its loadings). The worst value an
exactly singular item left at the Cholesky step, in R, was 1.554e-15 over the
hunt's nine singular datasets, and 4.196e-14 over a separate search at 50,000 and
100,000 rows (`factor_cases.rs` holds three of them, which a 1e-14 rule would not
refuse); both far below the rule's 1e-9, the larger by a factor of 2.4e4.

## 6. Speed and size

Release, 100,000 rows, median of seven, principal axis with promax, an Apple M1
with 8 GB: 10 items 36.00 ms (peak 148 MiB); 50 items 819.20 ms (682 MiB); 128
items 4,783.98 ms (1.17 GiB); 128 items at 1,000 iterations and convergence
1e-12, 5,178.91 ms (it used 11). The largest result the spec allows, every number
at its widest, is 2.842 MiB, under the 4 MiB a saved result may be read at.

## 7. The export

`export.rs`: the long CSV of every method, eight fields. An item is its index in
the spec's list and its alias; a factor, an eigenvalue or a component is its
number from 1 in the second index field; a line of a factor by a factor (the
transformation, the factor correlations) gives the first factor's number in the
first index field. Pinned exactly on macOS aarch64 (`factor_export.rs`), checked
by layout elsewhere until a Linux run gives its bytes.

## 8. Limitations

The shared three (text files UTF-8 only, limits set by memory, no weights), then:
principal components and principal axis only; varimax and promax only; promax at
power 4 only; no factor scores; the correlation, anti-image, reproduced and
residual matrices not shown; Pearson's correlations only; listwise only; no scree
plot.

## 9. What SPSS documents, and where it differs

Read: IBM, "FACTOR" algorithms, SPSS Statistics 14.0
(https://public.dhe.ibm.com/software/analytics/spss/support/Stats/Docs/Statistics/Algorithms/14.0/factor.pdf),
read 11 Nov 2026. Nothing else of IBM's was read; SPSS's numbers were not
compared.

- Principal components: loadings Ω_m Γ_m^{1/2}, communalities their rows' sums of
  squares (p. 1). Same.
- Principal axis: the preceding communalities on R's diagonal; iterations
  "continue until the maximum number (default 25) is reached or until the maximum
  change in the communality estimates is less than the convergence criterion
  (default 0.001)" (p. 2). Same rule. psych instead stops on the change in the
  communalities' sum (§5).
- Orthogonal rotations: pairwise planar rotations of the Kaiser-normalised
  pattern, stopped when the criterion changes by at most 1e-5 or at the maximum
  number of iterations (pp. 10 to 12). Gaply reaches varimax's optimum by R's
  route (the polar factor of the gradient), always allowing 1,000 iterations; the
  spec's iterations limit the extraction only. Whether SPSS's ITERATE also limits
  the rotation is not stated there; not checked.
- After an orthogonal rotation SPSS reflects factors whose loadings sum below 0,
  as Gaply does, and then **rearranges the factors by their sums of squared
  loadings, largest first** (p. 12, steps f and g). Gaply does not reorder, as R's
  `varimax` does not (decision 700).
- **Promax** (pp. 18 to 19): SPSS's target is p_ij = |λ_ij / h_i|^{k+1} h_i / λ_ij,
  that is sign(λ_ij) |λ_ij / h_i|^k, from the varimax loadings divided by their
  rows' lengths. Gaply's target is R's, A ∘ |A|³, unnormalised (decision 700).
  The two targets differ wherever the items' communalities differ, so Gaply's
  promax is R's and not SPSS's; not compared with SPSS's numbers. SPSS's factor
  correlations C(Q′Q)⁻¹C′ and structure Λ R_ff are the same formulas as Gaply's
  Φ and pattern Φ. The power's default (4) is not stated in this document.
- The Rotation Sums of Squared Loadings for an oblique rotation are not given for
  promax (decision 705); this document does not describe SPSS's.
- Bartlett's test: χ² = −(W − 1 − (2p + 5)/6) log|R| with p(p − 1)/2 df, and KMO
  and each MSA from the anti-image correlations (p. 20). Same.
- Kaiser's rule (eigenvalues above 1, MINEIGEN) and SPSS's handling of a
  communality above 1 in principal axis are not stated in this document; not
  checked.

## 10. Blueprint §18.2 acceptance

Inputs refused by closed code before any arithmetic; every number from a
deterministic engine; aliases only in the result, the export and the manifest
(`stats_factor_privacy_matrix.rs`); cancel and the ceiling cooperative at every
iteration; Reproducibility::Bitwise (`factor_cases.rs`, `factor_export.rs`).
