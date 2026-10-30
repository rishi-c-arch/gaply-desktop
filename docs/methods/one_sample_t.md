# The one-sample t-test, as an analysis

Capability `stats.analyze.one_sample_t`, version 1, **Beta**, in the
Statistics workspace (Research OS, Phase 8D-1; decisions 252 to 257 of 26 Oct
2026; Beta since Phase 8D-2a, decisions 262 to 265; its screen since Phase
8D-2b, decisions 266 to 269). This page is the capability's `documentation_id` (`methods/one_sample_t`).

It computes the t-test of one numeric column of the user's own data against a
value the user gives. The pure core is `gaply_core::stats_onesample`, which it
shares with the paired t-test (`methods/paired_t`); its façades are
`stats_adapters::one_sample_admit`, `one_sample_validate` and
`one_sample_run`. Planned in 8D-1, it is Beta since 8D-2a: the run manager
runs it (`RunKind::OneSample`), the `stats_onesample_*` commands validate,
run, report, cancel and export it, and a project saves, reruns and exports it,
with the test value on a line of its own in the report ("Test value: …", as the
CSV writes it). Since 8D-2b it has its own screen (`OneSampleFlow`, at
`/app/statistics/one-sample`), opened from its card on the Statistics home: the
test value is a text field, empty at first, and Gaply never fills it in. A saved
result renders through the same result card, and its failure through its
code's sentence.

## 1. Inputs and the spec

`OneSampleParams { outcome, testValue, missingCodes }`:

- `outcome`: Level 1's column alias (`C1`, `C2`, …) of a numeric column.
- `testValue`: the value the mean is compared with. **Never defaulted**
  (decision 255): while it is absent, validation is `NeedsUserInput` with
  `stats.onesample.test_value_not_chosen`, and a run is refused with that
  code. A test value that is NaN or infinite is `Invalid`,
  `test_value_not_finite`.
- `missingCodes`: values of the column to treat as missing (decision 38).

## 2. Formulas

Let the column hold x₁ … x_n after the exclusions, with mean m, sample
variance v (denominator n − 1) and sd s = √v; let μ be the test value.

| quantity | formula |
|---|---|
| standard error | se = √(v / n) |
| t | (m − μ) / se |
| df | n − 1 |
| p | 2 · P(T_df > \|t\|) |
| mean difference | m − μ |
| 95% interval | (m − μ) ± q · se, q = the 0.975 quantile of t_df |
| Cohen's d | (m − μ) / s |

**Where each number comes from (one implementation of each, decision 254).**

- t, df and se: `stats_verify::t_test_one_sample(x, μ)`, R's formula. Its
  arithmetic is one private function, `one_sample_t`, which
  `stats_verify::t_test_paired` also calls, on the differences with μ = 0, so
  Level 1's paired t-test and this test run the same lines. A pin written before
  the change shows Level 1's paired path kept every bit and every refusal
  (`tests/t_test_paired_pin.rs`: 203 inputs computed, 5 refused, one digest of
  t, df, se, both means and every refusal checked on every platform, and one of
  p checked on macOS aarch64).
- p: `stats_ttest::two_sided_p`, the t family's one p, over
  `stats_dist::t_two_tailed` (Level 0's `stats_l0::pvalue::t_two_tailed` at
  df ≤ 1e4, bit for bit; Hill's 1970 formula above, to df 1e7; Research OS,
  Phase 8E, decision 275, [large_samples.md](large_samples.md)), from t and df.
  `stats_verify`'s own `p_two_sided` is not used, as in `stats_ttest`.
- m and v: `stats_verify::mean` (R's two-pass mean) and `sample_variance`,
  given the values in ascending order (`total_cmp`), so permuting the rows
  changes no bit.
- The interval: `stats_ttest::interval_95`, the one interval of the t family,
  around `stats_ttest::t_quantile_975` (`stats_dist`'s since 8E: the bisection
  at df ≤ 1e4, the Abramowitz and Stegun 26.7.5 expansion above, to df 1e7,
  [large_samples.md](large_samples.md)). It moved out of the independent
  t-test's arithmetic in 8D-1, as written there; the independent t-test's
  canonical bytes on three computed cases are unchanged
  (`tests/ttest_bytes_pin.rs`, captured before the move), and so is 8C's
  export pin.
- d: the mean difference over s, which is
  `effectsize::cohens_d(x, mu = μ)`'s one-sample d.

## 3. Why d has no interval yet

As for the independent t-test (decision 63): effectsize builds d's interval
with the noncentral-t pivot, and Gaply has no noncentral t distribution. Every
result carries `stats.onesample.cohens_d_has_no_interval`.

## 4. The sign

The mean difference, t, d and the interval are **mean minus test value**.
Shifting the values and the test value by the same c changes none of t, p, d,
se or the interval's width; scaling both by k > 0 scales the mean difference,
se, the interval and s by k, and leaves t, p and d (`tests/onesample_properties.rs`).

## 5. Exclusions and edge cases

A row is left out for **one** reason, the first that applies:

1. `missing_value`: the cell is empty or a missing-value token;
2. `missing_code`: the value is one the user listed.

The result lists each reason with its count, and the manifest's rows used plus
the rows left out equal the rows read.

| case | outcome |
|---|---|
| fewer than 2 values after exclusions | refused, `too_few_values` {count}, at validation and at run |
| every value equal (se exactly 0) | not computable, `values_constant`; validation passes, the run refuses. R stops ("data are essentially constant") when the mean is not 0 and answers t = NaN when every value is 0 |
| se not 0 but below 10 · ε · \|mean\| (ε = `f64::EPSILON`) | not computable, `values_essentially_constant`; validation passes, the run refuses. R's own rule and scale (decision 259) |
| df above 1e7 (more than 10,000,002 values) | not computable, `quantile_out_of_range`; unreachable through the ingest. Until 8E the limit was 1e4 (decision 242.7); 10,001 and 10,002 values, df 10,000 and 10,001, both compute (`onesample_validate`) |
| a sum, square, quotient or root a double cannot hold | not computable, `overflow` |
| a sum of squares below the smallest normal double though the values differ | not computable, `underflow` |
| any other refusal of the arithmetic | not computable, `arithmetic_refused` |
| the column holds text, NaN or an infinity | invalid, `column_holds_text` / `column_non_finite` {column, count} |
| no column has the alias | invalid, `unknown_column` {column} |
| a missing code that is NaN or infinite | invalid, `missing_code_not_finite` {index} |
| a candidate missing code in the column, not listed | warning, `suspect_missing_code` {column, value, count}; never applied |

Never NaN or infinity: every number in a result is finite, or the run is
refused with a closed code.

**Essentially constant (Phase 8D-1b, decision 259).** Values that differ only
in their last bits, such as 0.3 and 0.30000000000000004, have a standard error
that is not 0 but is rounding, not spread; t built on it (≈ 1.87e16 for 0.3,
0.30000000000000004, 0.3, 0.3 against 0, before this rule) means nothing.
Gaply refuses a standard error below 10 · ε · |mean|, ε being `f64::EPSILON`,
which is R's rule in `t.test.default` ("data are essentially constant"; R's
`src/library/stats/R/t.test.R`, lines 62–64 of trunk, read 26 Oct 2026, the
same in R 4.6.1). The exact check comes first: all values equal is still
`values_constant`. The rule only refuses: every result that computed before
computes with the same bits (`tests/essentially_constant_bytes_pin.rs`). What
the rule does not catch, as R's does not: a spread lost in rounding that is
still above 10 · ε · |mean|, for example values near 0 whose mean is tiny
beside their rounding. `tests/essentially_constant_r.rs` checks the boundary
against R on six cases, each at least ten times either side of R's threshold.
The screen's sentence (8D-2b, `onesampleWording.ts`): "The values vary so little, compared with
their size, that the standard error is lost in rounding, so t is not computed.
R refuses such data too: "data are essentially constant"."

## 6. Precision, the gates, and why

ε = 1e-12, M = |m| + |μ|, every reference value R's
(`tests/onesample_r_golden.rs`):

| statistic | gate on \|gaply − R\| | why |
|---|---|---|
| n, exclusions, df | 0 | counts |
| se | ε · \|R\| | a smooth function of the centred variance, which the two-pass mean keeps accurate |
| mean, sd | ε · max(\|R\|, s) | Phase 2A's gate |
| mean difference | ε · max(M, se) | m − μ carries m's rounding, about one ulp of M |
| t | ε · max(\|t\|, M / se) | that rounding divided by se |
| interval bounds | ε · max(\|b\|, M, se) + Q · q · se, Q = 1e-12 | the difference's rounding, the quantile's measured gate; R's bound is its interval of the mean minus μ, which adds one rounding of μ, inside M |
| Cohen's d | ε · max(\|d\|, M / s) | the difference's rounding over s |
| p | 1e-9 · p + 1e-15 + 2 f(\|t\|) · (t's gate) | Level 0's gate against R, an absolute floor, and t's permitted error carried through the t density f |

These are the independent t-test's gates with its cancellation term written
for one mean and a test value. None was loosened to make a case pass.

**Measured** (30 Oct 2026, macOS aarch64, debug build, after 8E), the largest
|gaply − R| / gate over the 27 computed one-sample and paired cases: ci_lower
3.124e-1 (one_df_10000), ci_upper 1.925e-1 (paired_df_10000), t 9.604e-2,
cohens_d 9.593e-2, mean_difference 9.481e-2 (all one_df_10001), p 4.232e-2
(paired_df_10001), se 3.306e-3, mean 1.653e-3 (one_df_10001), sd 1.448e-3
(one_large_5000). Every value is inside its gate; the largest are at df 10,000,
the bisection's last df, where the quantile's own error is largest, and at
df 10,001, the first df the expansion and Hill's p serve. Before 8E (26 Oct
2026, 25 cases) the largest was the same ci_lower 3.124e-1, and every case at
df ≤ 1e4 is unchanged bit for bit (`tests/essentially_constant_bytes_pin.rs`).

## 7. Properties (`tests/onesample_properties.rs`)

proptest, seed 20261026, failure persistence off, values on the grid j/128 in
±1e6, shifts on the same grid, scales k = m · 2⁻¹⁵ in [1e-3, 1e3], so every
transformed input is exact:

- shifting the values and the test value by c: t, p, d, se and the width
  unchanged; scaling both by k: the mean difference, se, the interval and the sd
  by k, t, p and d unchanged. Each side within its golden gate, so the two
  within the **sum** of their gates plus one ulp; p within t's carried error;
- a test value equal to the computed mean: t = 0 and p = 1, **exact**;
- permuting the rows changes no byte of the result: **exact**;
- p ∈ [0, 1] and the interval contains the mean difference: **exact**.

Ten runs give byte-identical result and manifest bytes, `elapsed_ms` set aside
(`tests/onesample_validate.rs`).

## 8. What is not assessed, and the limitations

Every result carries these codes, and the descriptor lists the matching wording
codes:

- `values_normality_not_assessed`: the t-test assumes the values come from a
  roughly normal distribution, or that there are many of them, and Gaply does
  not check either;
- `cohens_d_has_no_interval` (section 3);
- independence of the rows is the user's claim: repeated measurements of one
  subject given as one column cannot be detected from the table;
- no weights;
- how large a file opens depends on this computer's memory (Phase 8F-2,
  D303; [large_samples.md](large_samples.md) §8): 100,000 data rows and 128
  columns on every computer, and 5,000,000 cells and 64 MiB on an 8 GB
  computer, 12,800,000 cells and 96 MiB on 16 and 32 GB; the screen states
  the limits before a file is chosen. A CSV or TSV must be UTF-8;
- not measured in 8F-2: the run's 30 s ceiling stays, and the 16 GB tier's
  12,800,000-cell summary took 14.08 s on an 8 GB Mac, so a slower computer
  near that size could reach it; a text-heavy 64 MB file (an open dataset
  also keeps the file's bytes, for a project Save); a categorical column can
  now carry up to 100,000 levels into the profile; and two 64 MB files cannot
  share one project, because decision 107's 100 MiB cap is per project..

The wording is neutral: no "significant", no stars, no verdict.

## 9. The R reference

**R's reader and Gaply's can read one CSV differently (decision 261).** R
4.6.1's `read.csv` reads some 17-digit cells as the neighbouring double:
measured in 8D-1b, it reads 1000.0000000000001 as 1000, and 3.8180000000000005
and 999999.60999999999 one unit in the last place off. Gaply reads every cell
as the double its text names (`str::parse::<f64>`). On such a file R and Gaply
compute on different numbers, so their results can differ in the last digits,
and one of them can call data "essentially constant" while the other does not
(`tests/fixtures/essentially_constant/r_outcomes.csv` names four such cases).
This is a property of R's reader, not of either test. Every R comparison here
checks the bits R read against Gaply's first, so no golden case rests on a
misread cell. No code changes for it.

`src-tauri/gaply-core/tests/fixtures/stats_onesample/make_onesample_fixtures.R`,
run with `/usr/local/bin/Rscript` (R 4.6.1 (2026-06-24), aarch64; effectsize
1.0.3 from CRAN; `set.seed(20261026)`). R is run outside Gaply and nothing of it
is copied in (ADR-008). Calls: `t.test(x, mu = m, conf.level = 0.95)`
(statistic, parameter, p.value, stderr, estimate; conf.int minus m) and
`effectsize::cohens_d(x, mu = m, ci = NULL)`, each d also computed directly as
(mean(x) − m) / sd(x), the script stopping unless the two agree within 1e-14
relative. Each data file is written, read back with `read.csv`, and computed on
what was read; `cells_bits.csv` holds the bits of every number R read (the four
datasets of 10,001 and 10,002 rows aside), all equal to Gaply's.

17 cases: n = 2; a spread; ties; a 1e9 offset with a test value near the mean;
heavy tails (t with 1.5 df); negative values; test values of 0, positive and
negative; a test value equal to a mean that is exact in any order (1, 2, 3, 6
against 3: t = 0, p = 1); 5,000 values; 10,001 values (df 10,000, the
bisection's last) and 10,002 (df 10,001, the first above 1e4: Hill's p and the
expansion; refused until 8E); missing values; −99 as a ticked
code; all values 2.5 against 1 (refused, `values_constant`; R: "data are
essentially constant"); one value (refused, `too_few_values`; R: "not enough 'x'
observations").

## 10. Blueprint §18.2 acceptance

| item | evidence | status |
|---|---|---|
| Formula/method and variant documented | this page §2, §4; `stats_onesample/mod.rs` header | done |
| Golden fixtures and edge cases pass | `onesample_r_golden` (17 one-sample cases, 2 refused), `onesample_validate`, `stats_dist_reference` | done |
| Missing values, ties, weights, precision and sample size handled according to the method | §5 (exclusions, codes), ties case, §8 (no weights), §6 (gates), `too_few_values`, the df boundary | done; weights not supported, stated |
| Diagnostic meanings and limitations documented | §5, §8; `OneSampleCode` doc comments; the title, description and normality sentences in `capabilityWording.ts`; the screen's sentence for each code (`onesampleWording.ts`, `onesampleWording.vitest.ts`: one per code, neutral) | done (8D-2b) |
| Independent reference comparison completed | R `t.test` and effectsize, §6, §9 | done |
| Unsupported designs return explicit reasons | §5 (`test_value_not_chosen`, `too_few_values`, …), §8; at the command boundary (`stats_onesample_commands::each_refusal_has_its_exact_code`, the essentially-constant codes included) and on the screen (`OneSampleFlow.vitest.tsx`: `test_value_not_chosen`, a refusal, a failed run) | done (core and commands 8D-2a; screen 8D-2b) |
| Result manifest and export verified | `onesample_validate` (CSV round trip, sentinels, rows add up, ten runs byte-identical); through IPC (`stats_onesample_commands`: the result, the CSV and the manifest export), the privacy matrix (`stats_onesample_privacy_matrix`), a project's save, rerun and export (`project_onesample`, `export_input_gives_the_one_sample_and_paired_runs_settings_codes_and_manifest`); the screen's export and Save to a project… (`OneSampleFlow.vitest.tsx`), the stored snapshots (`stored_one_sample_results_have_the_layout_of_their_fixtures`, `savedOneSample.vitest.tsx`) | done (core and app side 8D-2a; screen, save dialog and saved result 8D-2b; the real-app check `realapp_8d2b.md` is the user's) |
