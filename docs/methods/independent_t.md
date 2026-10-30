# The independent-samples t-test, as an analysis

Capability `stats.analyze.independent_t`, version 1, **Beta**, in the Statistics
workspace (Research OS, Phase 4B-1; decisions 60 to 67 of 18 Oct 2026). This
page is the capability's `documentation_id` (`methods/independent_t`).

It computes the t-test on the user's own data: a numeric outcome split by a
group with two chosen levels. It is **not** a reproduction of a reported
result; that is Level 1's `stats.verify.independent_t`, which this capability
does not change. The pure core is `gaply_core::stats_ttest`; its façade is
`stats_adapters::ttest_validate` / `ttest_run`. Phase 4B-2 (D278) added the
run manager's t-test kind, the `stats_ttest_*` commands, the method selector
at `/app/statistics` and the screen at `/app/statistics/ttest`; the analysis
area stays behind `REACT_APP_FEATURE_STATS_DESCRIBE`, which is off (decision
66). The tier is decided before it ships. A group coded in numbers (0/1,
1/2/3) is chosen through `stats_ttest_group_levels`, which lists a column's
levels in this page's §4 order with their indices (D279, decision 81).

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
- p: `stats_ttest::two_sided_p`, the t family's one p, over
  `stats_dist::t_two_tailed`: Level 0's `stats_l0::pvalue::t_two_tailed` at
  df ≤ 1e4, bit for bit, and Hill's (1970) formula above it, to df 1e7
  (Research OS, Phase 8E, decision 275; [large_samples.md](large_samples.md)).
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
- The quantile q: `stats_dist::t_quantile_975`, re-exported by `stats_ttest`.
  At df ≤ 1e4 it is the q whose `t_two_tailed(q, df)` is 0.05, by bisection on
  [0, 1e3] (decision 62's fallback; statrs's `StudentsT::inverse_cdf` was
  measured first and missed the gate, section 6), moved to `stats_dist` in 8E
  with its 30 pinned bits unchanged; above 1e4 it is Abramowitz and Stegun
  26.7.5, to df 1e7 ([large_samples.md](large_samples.md)).

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
| se not 0 but below 10 · ε · max(\|mean 1\|, \|mean 2\|) (ε = `f64::EPSILON`), under Student and Welch | not computable, `groups_essentially_constant`; validation passes, the run refuses. R's own rule and scale (decision 259) |
| one group constant | computed under both variants, as R does; its sd is exactly 0 and the sd ratio is `sd_ratio_undefined` |
| a sum, square, quotient or root a double cannot hold | not computable, `overflow` |
| a sum of squares below the smallest normal double though the values differ | not computable, `underflow` |
| any other refusal of the arithmetic (no p for the statistic) | not computable, `arithmetic_refused` |
| df above 1e7 | not computable, `quantile_out_of_range` (unreachable through the ingest's 5,000 rows, and through any dataset: it needs more than 10,000,002 rows; until 8E the limit was 1e4, decision 242.7) |
| the outcome holds text, NaN or an infinity | invalid, `outcome_holds_text` / `outcome_non_finite` |
| the group has fewer than 2 levels | invalid, `group_one_level` |
| the group column has more than 50 levels | its levels are not listed: `stats_ttest_group_levels` refuses with `too_many_levels` {count, limit}, and the screen does not offer the column (decision 87); `validate` and `run` refuse it as the group with the same code, so a spec built anywhere is held to the limit (decision 94) |
| more than 2 levels and none chosen | `NeedsUserInput`, `levels_not_chosen` {count} |
| a level index out of range, or both the same | invalid, `level_out_of_range` {index, limit} / `levels_equal` |
| a missing code that is NaN or infinite | invalid, `missing_code_not_finite` |
| a candidate missing code in the outcome, not listed | warning, `suspect_missing_code` {column, value, count}; never applied |

Never NaN or infinity: every number in a result is finite, or the run is
refused with a closed code.

**Essentially constant (Phase 8D-1b, decision 259).** Groups whose values
differ only in their last bits (group A 1.0, 1.0000000000000002, 1.0 and group
B 2.0, 2.0000000000000004, 2.0) have a standard error that is not 0 but is
rounding, not spread; before this rule Gaply reported t ≈ −4.93e15 and p ≈ 0
for them, under both variants. Gaply refuses a standard error below 10 · ε ·
max(|mean 1|, |mean 2|), ε being `f64::EPSILON`, which is R's rule in
`t.test.default` ("data are essentially constant"; R's
`src/library/stats/R/t.test.R`, line 94 of trunk, read 26 Oct 2026, the same in
R 4.6.1). The scale is the LARGER mean: a group near 1 one ulp apart beside a
group near 1000 is refused (`tests/essentially_constant.rs`). The exact check
comes first: both groups constant is still `both_groups_constant`, and one
constant group still computes. The rule only refuses: every result that
computed before computes with the same bits
(`tests/essentially_constant_bytes_pin.rs`, all 32 computed golden cases). What
the rule does not catch, as R's does not: a spread lost in rounding that is
still above 10 · ε · the larger mean. `tests/essentially_constant_r.rs` checks
the boundary against R on six cases under both variants. The screen says: "The
values in each group vary so little, compared with their size, that the
standard error is lost in rounding, so t is not computed. R refuses such data
too: "data are essentially constant"."

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
d from a one-pass mean, `sum(x)/n`, differs from `effectsize::cohens_d` by
1.194e-8 (Student) and 1.200e-8 (Welch), while R's own `t.test` and
`effectsize::cohens_d` agree with each other to 2.2e-16 and exactly. That is
the means' rounding at this offset, about one ulp of M whoever sums them, and
the M term is what carries it. Measured 18 Oct 2026, R 4.6.1, effectsize
1.0.3 (decision 91; the Phase 4 evaluation's F2 corrected an earlier sentence
that put the 1.2e-8 between `t.test` and effectsize):

```r
x  <- read.csv("data_offset.csv", comment.char = "#")
lv <- sort(unique(x$g), method = "radix"); f <- factor(x$g, levels = lv)
y1 <- x$y[f == lv[1]]; y2 <- x$y[f == lv[2]]
s  <- t.test(y ~ f, data = data.frame(y = x$y, f), var.equal = TRUE)$stderr /
      sqrt(1 / length(y1) + 1 / length(y2))      # Welch: sqrt((var(y1) + var(y2)) / 2)
(sum(y1) / length(y1) - sum(y2) / length(y2)) / s -
  effectsize::cohens_d(y ~ f, data = data.frame(y = x$y, f), pooled_sd = TRUE, ci = NULL)$Cohens_d
```

**Measured** (30 Oct 2026, macOS aarch64, debug build, after 8E), the largest
|gaply − R| / gate over 34 computed (case, variant) pairs: cohens_d 5.412e-2,
mean_difference 5.412e-2, t 5.412e-2, ci_upper 3.069e-2, ci_lower 3.054e-2,
sd_ratio 3.019e-3, sd2 1.939e-3, mean2 1.602e-3, mean1 1.119e-3, sd1 1.109e-3
(all `large_df`, the 10,200-row case above df 1e4), se 9.214e-4 (large_5000),
p 5.150e-4 (large_5000), df (Welch) 4.480e-4 (numeric_group). Every value is
at least 18× inside its gate. Before 8E (18 Oct 2026, 32 pairs) the largest
was ci_upper 9.989e-3 (heavy_tail), and every row at df ≤ 1e4 is unchanged
bit for bit (`tests/essentially_constant_bytes_pin.rs`).

**The quantile** (`tests/ttest_quantile.rs`, 33 df from 1 to 1e7 against
`qt(0.975, df)`; gate ε · R):

| implementation | df ≤ 4,998 | df 1e4 | 1e5 | 1e6 | 1e7 |
|---|---|---|---|---|---|
| statrs `StudentsT::inverse_cdf` | ≤ 6.1e-13 | 9.7e-14 | 3.5e-6 | 6.1e-5 | 6.8e-2 |
| bisection over `t_two_tailed` (shipped at df ≤ 1e4) | ≤ 2.9e-13 | 5.2e-13 | 1.8e-11 | 8.8e-11 | 3.6e-9 |
| Abramowitz and Stegun 26.7.5 (shipped above 1e4 since 8E) | 3.4e-16 at 2,500 | 2.3e-16 | 2.3e-16 | 2.3e-16 | 2.3e-16 |

statrs fails the gate from 1e5. The bisection meets it up to 1e4 and not above,
where it inherits `pvalue::t_two_tailed`'s own error at large df; so above 1e4
the quantile is the expansion (Research OS, Phase 8E, decision 275: within
4.5e-16 of R's `qt` over 20 df from 1e3 to 1e7, `tests/stats_dist_reference.rs`,
and 2.6e-16 of exact values in the reviewer's measurement,
[large_samples.md](large_samples.md)), and above 1e7 (`QUANTILE_MAX_DF`) it is
refused rather than given. The ingest caps a table at 5,000 rows, so df ≤ 4,998
for a file a user opens.

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
read; `cells_bits.csv` holds the bits of every number R read (5,613;
`data_large_df.csv`'s 10,200 cells aside, to keep the file small), all equal
to Gaply's.

19 cases on 17 datasets: n of 2 per group; unequal n; unequal variances;
ties; a 1e9 offset; heavy tails (t with 1.5 df); negative values; 5,000 rows;
10,200 rows (`large_df`: Student's df 10,198, Welch's about 10,194, above 1e4,
so Hill's p and the expansion, 8E);
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
| Golden fixtures and edge cases pass | `ttest_r_golden` (34 computed, 4 refused pairs), `ttest_validate`, `ttest_quantile`, `stats_dist_reference` | done |
| Missing values, ties, weights, precision and sample size handled according to the method | §5 (exclusions, codes), ties fixture, §8 (no weights), §6 (gates), `group_too_small` | done; weights not supported, stated |
| Diagnostic meanings and limitations documented | §5, §8; `TTestCode` doc comments; a sentence for each of the 50 codes the screen can show in `src/screens/statistics/ttestWording.ts`, the list `ttestCodes.json` pinned to the Rust sets by `src-tauri/tests/stats_ttest_wire.rs`, every code covered and no sentence saying "significant" (`ttestWording.vitest.ts`); the five limitation and title codes' sentences in `capabilityWording.ts` | done |
| Independent reference comparison completed | R `t.test` and effectsize, §6, §9 | done |
| Unsupported designs return explicit reasons | §5 (`group_one_level`, `levels_not_chosen`, `variant_not_chosen`, …); undetectable designs listed in §8; at the command boundary each refusal's exact code (`src-tauri/tests/stats_ttest_commands.rs`, `each_refusal_has_its_exact_code`), and on the screen the sentence for each and the NeedsUserInput questions (`TTestFlow.vitest.tsx`) | done |
| Result manifest and export verified | `ttest_export` (CSV round trip, guard apostrophes, sentinels, rows add up, ten runs byte-identical); through IPC the CSV bytes and output hash equal `compute`'s, and csv, tsv, xlsx and BOM-plus-CRLF files give the same result, hash and CSV (`stats_ttest_commands.rs`); the manifest panel and the export through the save dialog, the written text checked against Rust's bytes (`TTestFlow.vitest.tsx`); the privacy matrix (`stats_ttest_privacy_matrix.rs`) | done |
