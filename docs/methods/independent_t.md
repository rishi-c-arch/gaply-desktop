# The independent-samples t-test, as an analysis

Capability `stats.analyze.independent_t`, version 1, **Beta**, in the Statistics
workspace (Research OS, Phase 4B-1; decisions 60 to 67 of 18 Oct 2026). This
page is the capability's `documentation_id` (`methods/independent_t`).

It computes the t-test on the user's own data: a numeric outcome split by a
group with two chosen levels. It is **not** a reproduction of a reported
result; that is Level 1's `stats.verify.independent_t`, which this capability
does not change. The pure core is `gaply_core::stats_ttest`; its façade is
`stats_adapters::ttest_validate` / `ttest_run`. The run manager, commands,
screen and method selector are Phase 4B-2: nothing here is reachable from the
app yet, and the analysis area stays behind `REACT_APP_FEATURE_STATS_DESCRIBE`,
which is off (decision 66). The tier is decided before it ships.

## 1. Inputs and the spec

`TTestParams { outcome, group, levels, variant, missing_codes }`:

- `outcome`, `group`: Level 1's column aliases (`C1`, `C2`, …). They must be
  two different columns.
- `levels`: two indices into the group's levels in code-point order, group 1
  first, or absent when the group has exactly two levels (then `[0, 1]`).
- `variant`: `student` or `welch`. **Never chosen for the user** (decision 64):
  while it is absent, validation is `NeedsUserInput` with
  `stats.ttest.variant_not_chosen`, and a run is refused with that code.
- `missing_codes`: values of the **outcome** to treat as missing (decision 38).
  They never apply to the group: a group cell `-99` is a level.

## 2. Formulas

Let group 1 hold x₁ … x_{n₁} and group 2 y₁ … y_{n₂}, with means m₁, m₂ and
sample variances v₁, v₂ (denominator n − 1).

| quantity | Student (pooled) | Welch |
|---|---|---|
| standard error | se = s_p · √(1/n₁ + 1/n₂), s_p² = ((n₁−1)v₁ + (n₂−1)v₂)/(n₁+n₂−2) | se = √(v₁/n₁ + v₂/n₂) |
| t | (m₁ − m₂) / se | (m₁ − m₂) / se |
| df | n₁ + n₂ − 2 | (v₁/n₁ + v₂/n₂)² / ((v₁/n₁)²/(n₁−1) + (v₂/n₂)²/(n₂−1)), Welch–Satterthwaite |
| p | 2 · P(T_df > \|t\|) | same |
| 95% interval | (m₁ − m₂) ± q · se, q = the 0.975 quantile of t_df | same |
| Cohen's d | (m₁ − m₂) / s_p | (m₁ − m₂) / √((v₁ + v₂)/2) |

**Where each number comes from (one implementation of each, decision 62).**

- t, df and se: `stats_verify::t_test_student` and `t_test_welch`. Their
  result gained one field, `se`, the standard error t divides by; nothing else
  in them changed, and Level 1's golden set is unchanged (`l1_r_golden`, and
  `stats_adapters_golden`'s 43 rows, identical before and after).
- p: `stats_l0::pvalue::t_two_tailed`, Level 0's p.
- Each group's mean and variance: `stats_verify::mean` (R's two-pass mean) and
  `sample_variance`, given the values in ascending order, as the descriptive
  summary gives them. So permuting the rows changes no bit of the result.
- Student's d divides by s_p **recovered from Student's own se**,
  se / √(1/n₁ + 1/n₂), so no second pooled variance exists.
- Welch's d divides by √((v₁ + v₂)/2). This is `effectsize::cohens_d(…,
  pooled_sd = FALSE)`'s standardizer. effectsize's documentation
  (<https://easystats.github.io/effectsize/reference/cohens_d.html>, effectsize
  1.0.3, read 18 Oct 2026) says: "If `TRUE` (default), a `sd_pooled()` is used
  (assuming equal variance). Else the mean SD from both groups is used
  instead." The installed function (effectsize 1.0.3, from CRAN,
  `effectsize:::.effect_size_difference`) computes it as
  `s <- sqrt((s1^2 + s2^2)/2)`: the square root of the mean of the two
  variances, which is what decision 63 states.
- The quantile q: the q whose `t_two_tailed(q, df)` is 0.05, by bisection on
  [0, 1e3] (decision 62's fallback). statrs's `StudentsT::inverse_cdf` was
  measured first and missed the gate (section 6).

Every result says which standardizer d used (`dStandardizer`: `pooled_sd` or
`root_mean_variance`) and its value (`dStandardizerValue`).

## 3. Why d has no interval yet

effectsize builds d's interval with the noncentral-t pivot (its documentation:
CIs "estimated using the noncentrality parameter method"). Gaply has no
noncentral t distribution, and building one is a method of its own with its own
validation. Every result carries the limitation
`stats.ttest.cohens_d_has_no_interval`; a later step decides.

## 4. Level order and the sign

The group column is read as categorical with Level 1's reading of cells
(decision 17): `1`, `1.0` and `1.00` are one level, and text is trimmed. Its
levels are sorted by code point (`B` before `a`, `-99` before `a`), as the
descriptive summary sorts them. The spec names levels by **index** in that
order, never by label. Group 1 is the first index named; the mean difference,
t, d and the interval are **group 1 minus group 2**. Naming `[1, 0]` instead of
`[0, 1]` changes the sign of those four, bit for bit, and nothing else
(`ttest_properties.rs`).

## 5. Exclusions and edge cases

A row is left out for **one** reason, the first that applies:

1. `missing_group`: the group cell is empty or a missing-value token;
2. `other_level`: the group is not one of the two chosen levels;
3. `missing_outcome`: the outcome cell is empty or a missing-value token;
4. `missing_code`: the outcome is a value the user listed.

The result lists each reason with its count (reasons with no row are not
listed), and the manifest's rows used plus the rows left out equal the rows
read.

| case | outcome |
|---|---|
| a group with fewer than 2 values after exclusions | refused, `group_too_small` {index, count}, at validation and at run |
| both groups constant (se exactly 0) | not computable, `both_groups_constant`; validation passes, the run refuses |
| one group constant | computed under both variants, as R does; its sd is exactly 0 and the sd ratio is `sd_ratio_undefined` |
| a sum, square, quotient or root a double cannot hold | not computable, `overflow` |
| a sum of squares below the smallest normal double though the values differ | not computable, `underflow` |
| any other refusal of the arithmetic (no p for the statistic) | not computable, `arithmetic_refused` |
| df above 1e4 | not computable, `quantile_out_of_range` (unreachable through the ingest's 5,000 rows) |
| the outcome holds text, NaN or an infinity | invalid, `outcome_holds_text` / `outcome_non_finite` |
| the group has fewer than 2 levels | invalid, `group_one_level` |
| more than 2 levels and none chosen | `NeedsUserInput`, `levels_not_chosen` {count} |
| a level index out of range, or both the same | invalid, `level_out_of_range` {index, limit} / `levels_equal` |
| a missing code that is NaN or infinite | invalid, `missing_code_not_finite` |
| a candidate missing code in the outcome, not listed | warning, `suspect_missing_code` {column, value, count}; never applied |

Never NaN or infinity: every number in a result is finite, or the run is
refused with a closed code.

## 6. Precision, the gates, and why

ε = 1e-12, M = |m₁| + |m₂|, every reference value R's
(`tests/ttest_r_golden.rs`):

| statistic | gate on \|gaply − R\| | why |
|---|---|---|
| n₁, n₂, exclusions, levels, Student's df | 0 | counts |
| Welch's df, se, sd ratio | ε · \|R\| | smooth functions of the centred variances, which the two-pass mean keeps accurate |
| means, sds | ε · max(\|R\|, s) | Phase 2A's gate |
| mean difference | ε · max(M, se) | a difference of means carries the means' own rounding, about one ulp of M, whoever sums them |
| t | ε · max(\|t\|, M / se) | that rounding divided by se |
| interval bounds | ε · max(\|b\|, M, se) + Q · q · se, Q = 1e-12 | the difference's rounding, plus the quantile's measured gate |
| Cohen's d | ε · max(\|d\|, M / s_d) | the difference's rounding over d's standardizer |
| p | 1e-9 · p + 1e-15 + 2 f(\|t\|) · (t's gate) | Level 0's gate against R, an absolute floor so a tiny p cannot pass or fail on its relative error alone, and t's permitted error carried through the t density f |

The cancellation terms are not slack: on `offset_1e9` (values near 1e9, sd 10)
R's own `t.test` and `effectsize` disagree with each other by 1.2e-8 in d,
because they sum in different orders.

**Measured** (18 Oct 2026, macOS aarch64, debug build), the largest
|gaply − R| / gate over 32 computed (case, variant) pairs: ci_upper 9.989e-3
(heavy_tail), ci_lower 9.751e-3, se 9.214e-4, sd2 9.592e-4, sd1 8.296e-4, p
5.150e-4, cohens_d 4.485e-4, df (Welch) 4.480e-4, sd_ratio 4.318e-4, mean2
3.282e-4, t 2.571e-4, mean_difference 1.575e-4, mean1 1.418e-4. Every value is
at least 100× inside its gate.

**The quantile** (`tests/ttest_quantile.rs`, 33 df from 1 to 1e7 against
`qt(0.975, df)`; gate ε · R):

| implementation | df ≤ 4,998 | df 1e4 | 1e5 | 1e6 | 1e7 |
|---|---|---|---|---|---|
| statrs `StudentsT::inverse_cdf` | ≤ 6.1e-13 | 9.7e-14 | 3.5e-6 | 6.1e-5 | 6.8e-2 |
| bisection over `t_two_tailed` (shipped) | ≤ 2.9e-13 | 5.2e-13 | 1.8e-11 | 8.8e-11 | 3.6e-9 |

statrs fails the gate from 1e5. The bisection meets it up to 1e4 and not above,
where it inherits `pvalue::t_two_tailed`'s own error at large df; so the
quantile is refused above 1e4 (`QUANTILE_MAX_DF`) rather than given. The ingest
caps a table at 5,000 rows, so df ≤ 4,998 for a file a user opens.

## 7. Properties (`tests/ttest_properties.rs`)

proptest, seed 20261018, failure persistence off, values on the grid j/128 in
±1e6, shifts on the same grid, scales k = m · 2⁻¹⁵ in [1e-3, 1e3], so every
transformed input is exact:

- swapping the levels negates t, the difference, d and the interval, and keeps
  p, df, se and the sd ratio: **exact**, bit for bit;
- permuting the rows changes no byte of the result: **exact**;
- shifting by c changes only the means; scaling by k scales the difference,
  se, interval and sds by k and leaves t, p and d: each side within its golden
  gate, so the two within the **sum** of their gates plus one ulp (Phase 2A's
  rule), p within t's carried error;
- p ∈ [0, 1] and the interval contains the difference: **exact**;
- Welch's df ∈ [min(n₁, n₂) − 1, n₁ + n₂ − 2] within ε · (n₁ + n₂); Student's df
  is n₁ + n₂ − 2 exactly.

## 8. What is not assessed, and the limitations

Every result carries these codes, and the descriptor lists the matching
wording codes:

- `normality_not_assessed`: no normality check is made (decision 64);
- `cohens_d_has_no_interval` (section 3);
- the variances' equality is not tested: the sd ratio and each n are
  information, never a recommendation, and the variant is the user's;
- the design is the user's claim: a paired, clustered or repeated-measures
  design given as two independent groups cannot be detected from the table;
- no weights: the capability takes none;
- a CSV or TSV must be UTF-8, and 2 MiB of cell text binds first on a wide
  table (the shared ingest's limits).

The wording is neutral: no "significant", no stars, no verdict (decision 66).

## 9. The R reference

`src-tauri/gaply-core/tests/fixtures/stats_ttest/make_ttest_fixtures.R`, run
with `/usr/local/bin/Rscript` (R 4.6.1, aarch64; effectsize 1.0.3 from CRAN,
with insight 1.6.0, bayestestR 0.20.0, parameters 0.30.0, datawizard 1.4.0,
performance 0.18.2; `set.seed(20261018)`). R is run outside Gaply and nothing
of it is copied in (ADR-008). Calls: `t.test(y ~ f, var.equal = TRUE/FALSE,
conf.level = 0.95)` (statistic, parameter, p.value, conf.int, stderr,
estimate), `effectsize::cohens_d(y ~ f, pooled_sd = TRUE/FALSE, ci = NULL)`,
`mean`, `sd`, `qt(0.975, df)`. The factor's levels are set explicitly to the
two chosen, in Gaply's order (`sort(method = "radix")`), so the signs agree.
Each data file is written, read back with `read.csv`, and computed on what was
read; `cells_bits.csv` holds the bits of every number R read (5,613), all equal
to Gaply's.

18 cases on 16 datasets: n of 2 per group; unequal n; unequal variances;
ties; a 1e9 offset; heavy tails (t with 1.5 df); negative values; 5,000 rows;
one constant group; both constant; a group of one; three levels with two
chosen, in both orders; missing outcomes and groups; -99 ticked and not
ticked; code-point order (`B` before `a`); integer group codes. Expected
refusals: `both_constant` (R: "data are essentially constant"; Gaply
`both_groups_constant`) and `group_of_one` (R's Student t computes it, its
Welch stops; Gaply `group_too_small` for both, since a variance needs two
values).

## 10. Blueprint §18.2 acceptance

| item | evidence | status |
|---|---|---|
| Formula/method and variant documented | this page §2, §4; `stats_ttest/mod.rs` header | done |
| Golden fixtures and edge cases pass | `ttest_r_golden` (32 computed, 4 refused pairs), `ttest_validate`, `ttest_quantile` | done |
| Missing values, ties, weights, precision and sample size handled according to the method | §5 (exclusions, codes), ties fixture, §8 (no weights), §6 (gates), `group_too_small` | done; weights not supported, stated |
| Diagnostic meanings and limitations documented | §5, §8; `TTestCode` doc comments | done; their screen sentences are 4B-2 |
| Independent reference comparison completed | R `t.test` and effectsize, §6, §9 | done |
| Unsupported designs return explicit reasons | §5 (`group_one_level`, `levels_not_chosen`, `variant_not_chosen`, …); undetectable designs listed in §8 | done in the core; shown to a user in 4B-2 |
| Result manifest and export verified | `ttest_export` (CSV round trip, guard apostrophes, sentinels, rows add up, ten runs byte-identical) | done in the core; the save dialog is 4B-2 |
