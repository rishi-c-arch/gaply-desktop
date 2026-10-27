# The paired-samples t-test, as an analysis

Capability `stats.analyze.paired_t`, version 1, **Beta**, in the Statistics
workspace (Research OS, Phase 8D-1; decisions 252 to 257 of 26 Oct 2026; Beta
since Phase 8D-2a, decisions 262 to 265). This
page is the capability's `documentation_id` (`methods/paired_t`).

It computes the paired t-test on two numeric columns of the user's own data,
measured on the same rows. It is **not** a reproduction of a reported result;
that is Level 1's `stats.verify.paired_t`, which this capability does not
change. **The paired t-test is the one-sample t-test of the row-wise
differences against 0, and Gaply computes it so**: `stats_verify::t_test_paired`
forms the differences and calls the one arithmetic the one-sample test uses
(`methods/one_sample_t`), with a test value of 0. The pure core is
`gaply_core::stats_onesample`; its façades are `stats_adapters::paired_admit`,
`paired_validate` and `paired_run`. Planned in 8D-1, it is Beta since 8D-2a:
the run manager runs it (`RunKind::Paired`), the `stats_paired_*` commands
validate, run, report, cancel and export it, and a project saves, reruns and
exports it, its columns named in the report by position and the roles `first`
and `second`. Its screen is 8D-2c's (decision 266 split the screens by method);
until then the Statistics home shows it as a card that says it has no screen of
its own here yet. Its words are already written, in 8D-2b's `onesampleWording.ts`.

## 1. Inputs and the spec

`PairedParams { first, second, missingCodes }`:

- `first`, `second`: Level 1's column aliases of two numeric columns. They must
  be two different columns (`same_column` otherwise). **Gaply does not choose
  which is first**: the difference is first minus second, as the user names
  them.
- `missingCodes`: values to treat as missing, in **either** column (decision
  255).

## 2. Formulas

Let the pairs used be (a₁, b₁) … (a_n, b_n), with differences dᵢ = aᵢ − bᵢ, their
mean m_d, sample variance v_d and sd s_d = √v_d.

| quantity | formula |
|---|---|
| standard error | se = √(v_d / n) |
| t | m_d / se |
| df | n − 1 |
| p | 2 · P(T_df > \|t\|) |
| mean difference | m_d, the mean of the differences |
| 95% interval | m_d ± q · se, q = the 0.975 quantile of t_df |
| Cohen's d_z | m_d / s_d |
| each column | its mean and sd over the pairs used |

**Where each number comes from (one implementation of each, decision 254).**

- t, df and se: `stats_verify::t_test_paired(a, b)`, whose arithmetic since
  8D-1 is `one_sample_t(d, 0.0)`, the one-sample test's own. `md − 0.0` is
  `md` bit for bit, so its t is `md / se` as before: a pin written before the
  change shows Level 1's paired path kept every bit and every refusal
  (`tests/t_test_paired_pin.rs`), and Level 1's goldens and precision sweep are
  unchanged.
- The mean difference: `stats_verify::mean` of the differences, in their sorted
  order, **never** the mean of the first minus the mean of the second; R's
  estimate is mean(x − y). The sd of the differences: `sample_variance` of the
  same values.
- p: `stats_l0::pvalue::t_two_tailed`, from t and df.
- The interval: `stats_ttest::interval_95`, the one interval of the t family.
- Each column's mean and sd: `stats_verify::mean` and `sample_variance` over
  the pairs used, in the pairs' order.
- d_z: the mean difference over s_d. effectsize's own page for `cohens_d`
  (effectsize 1.0.3, the installed `cohens_d.Rd`, read 26 Oct 2026; the
  reference page is <https://easystats.github.io/effectsize/reference/cohens_d.html>)
  says of `paired = TRUE`: "This produces an effect size that is equivalent to
  the one-sample effect size on x - y." That is d_z. `repeated_measures_d` is
  not used: its default is another method, with a small-sample adjustment.

`tests/onesample_properties.rs` checks that the paired test on two columns gives
the same bits as the one-sample test, test value 0, on a column of their
differences.

## 3. Why d has no interval yet

As for the independent t-test (decision 63): effectsize builds d's interval with
the noncentral-t pivot, and Gaply has no noncentral t distribution. Every result
carries `stats.onesample.cohens_d_has_no_interval`.

## 4. The sign

The mean difference, t, d_z and the interval are **first minus second**.
Swapping first and second negates t, the mean difference and d_z, swaps and
negates the interval's ends, and leaves p, df and se unchanged. Not bit for bit:
the pairs are sorted by their difference before the arithmetic, and the
negated differences sort in the reverse order, so the sums run in another order;
each side meets its gate (`tests/onesample_properties.rs`). Adding c to both
columns leaves every statistic of the differences unchanged, bit for bit.

## 5. Exclusions and edge cases

A row is left out for **one** reason, the first that applies:

1. `missing_first`: the first cell is empty or a missing-value token;
2. `missing_second`: the second cell is;
3. `missing_code`: either value is one the user listed.

Before the arithmetic the pairs are sorted by their difference, then the first,
then the second, so permuting the rows changes no bit.

| case | outcome |
|---|---|
| fewer than 2 pairs after exclusions | refused, `too_few_pairs` {count}, at validation and at run |
| every difference equal (se exactly 0), identical columns included | not computable, `differences_constant`; validation passes, the run refuses. R stops ("data are essentially constant") when the mean difference is not 0, and answers t = NaN for identical columns |
| se not 0 but below 10 · ε · max(\|mean of the differences\|, mean of \|first\|, mean of \|second\|) | not computable, `differences_essentially_constant`; validation passes, the run refuses. R's scale is the first term only; the other two are Gaply's own, so Gaply refuses some data R computes (below) |
| df above 1e4 (10,002 pairs or more) | not computable, `quantile_out_of_range` (decision 242.7, until 8E); 10,001 pairs compute |
| the same column twice | invalid, `same_column` |
| a column holds text, NaN or an infinity | invalid, `column_holds_text` / `column_non_finite` {column, count} |
| overflow, underflow, any other refusal | `overflow`, `underflow`, `arithmetic_refused`, as for the one-sample test |
| a candidate missing code in either column, not listed | warning, `suspect_missing_code` {column, value, count}, one per column; never applied |

Never NaN or infinity.

**Essentially constant (Phase 8D-1b, decision 259).** When the second column is
the first minus a constant, rounding can leave differences that are equal in
decimal but not in binary: `b = a − 2` on a 3-decimal grid misses 2 by an ulp
on some rows, and before this rule Gaply computed t ≈ 1.42e16 there. Gaply
refuses a standard error below 10 · ε · scale, ε being `f64::EPSILON`, with
scale = max(|mean of the differences|, mean of |first|, mean of |second|). R's
`t.test.default` stops on the same comparison with |mean of the differences|
alone (R's `src/library/stats/R/t.test.R`, lines 52–55 and 62–64 of trunk, read
26 Oct 2026, the same in R 4.6.1).

**Where Gaply refuses and R computes.** The rounding in first − second is about
an ulp of the VALUES, not of the difference. When every pair differs by the
same small amount beside large values, that rounding is above R's threshold and
R computes a huge t; Gaply's larger scale refuses it. Measured on R 4.6.1:

| first, second | se | R's threshold | Gaply's threshold | R | Gaply |
|---|---|---|---|---|---|
| near 100, every pair 0.1 apart (`pair_tenth`) | 3.48e-15 | 2.22e-16 | 2.23e-13 | t ≈ 2.9e13 | `differences_essentially_constant` |
| near 300, 0.2 apart (`pair_300_fifth`) | 1.14e-14 | 4.44e-16 | 6.7e-13 | computes | refuses |
| near 20, 0.01 apart (`pair_20_hundredth`) | 8.70e-16 | 2.22e-17 | 4.6e-14 | computes | refuses |

Data whose differences really vary are unaffected: a spread ten times above
Gaply's threshold computes with the same bits as before
(`tests/essentially_constant.rs`). The exact check comes first: all differences
equal is still `differences_constant`. The rule only refuses: every result that
computed before computes with the same bits
(`tests/essentially_constant_bytes_pin.rs`). The screen's sentence (written in 8D-2b, shown from 8D-2c):
"The differences vary so little, compared with the size of the values, that
the standard error is lost in rounding, so t is not computed. This happens when
every pair differs by the same amount, such as 0.1, which a computer stores
with a tiny rounding error."

## 6. Precision, the gates, and why

ε = 1e-12, M = |mean of first| + |mean of second|, every reference value R's
(`tests/onesample_r_golden.rs`):

| statistic | gate on \|gaply − R\| | why |
|---|---|---|
| n_pairs, exclusions, df | 0 | counts |
| se, sd of the differences | ε · \|R\| | smooth functions of the centred variance |
| each column's mean and sd | ε · max(\|R\|, s) | Phase 2A's gate |
| mean difference | ε · max(M, se) | the differences carry the columns' rounding, about one ulp of M |
| t | ε · max(\|t\|, M / se) | that rounding divided by se |
| interval bounds | ε · max(\|b\|, M, se) + Q · q · se, Q = 1e-12 | the difference's rounding and the quantile's measured gate |
| d_z | ε · max(\|d_z\|, M / s_d) | the difference's rounding over s_d |
| p | 1e-9 · p + 1e-15 + 2 f(\|t\|) · (t's gate) | Level 0's gate against R, an absolute floor, and t's permitted error carried through f |

**Measured** (26 Oct 2026, macOS aarch64), over the 25 computed cases of both
tests: ci_upper 1.925e-1 (paired_df_10000), sd_differences 3.118e-3,
se 3.306e-3, mean_second 2.165e-3, sd_first 1.990e-3, sd_second 1.226e-3,
cohens_dz 9.989e-3 (all paired_df_10000 but sd_second, paired_large_5000),
mean_first 1.505e-4 (paired_ties). Every value is inside its gate.

## 7. Properties (`tests/onesample_properties.rs`)

proptest, seed 20261026, the grid of `methods/one_sample_t` §7:

- swapping first and second, as §4: within the sum of the two sides' gates;
- adding c to both columns: every statistic of the differences **exact**, each
  column's mean moves by c and its sd stays, within the rule;
- the paired test is the one-sample test, test value 0, of the differences:
  **exact**;
- permuting the rows: **exact**; p ∈ [0, 1] and the interval contains the mean
  difference: **exact**.

## 8. What is not assessed, and the limitations

- `differences_normality_not_assessed`: the paired t-test assumes the
  differences come from a roughly normal distribution, or that there are many
  pairs, and Gaply does not check either;
- `cohens_d_has_no_interval` (section 3);
- the pairing is the user's claim: that row i of both columns is one subject
  cannot be detected from the table;
- no weights; the shared ingest's limits, as for the one-sample test.

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

The one-sample test's script (`methods/one_sample_t` §9), with
`t.test(a, b, paired = TRUE, conf.level = 0.95)` in the (x, y) form, never a
formula (R 4.4 and later refuse `paired = TRUE` with a formula), and
`effectsize::cohens_d(a, b, paired = TRUE, ci = NULL)`, each d_z also computed
directly as mean(a − b) / sd(a − b) and required to agree within 1e-14 relative.

15 cases: n = 2; strongly and weakly related columns; negative differences;
ties; a 1e9 offset in both columns; heavy tails; 5,000 pairs; 10,001 pairs
(computes) and 10,002 (refused, `quantile_out_of_range`; R computes); a missing
first, a missing second and both on one row; −99 ticked in either column and in
both on one row; identical columns (refused, `differences_constant`; R: t = NaN);
a constant difference of 2 on a quarter grid (refused; R: "data are essentially
constant"); one pair (refused, `too_few_pairs`; R: "not enough 'x'
observations").

## 10. Blueprint §18.2 acceptance

| item | evidence | status |
|---|---|---|
| Formula/method and variant documented | this page §2, §4; `stats_onesample/mod.rs` header | done |
| Golden fixtures and edge cases pass | `onesample_r_golden` (15 paired cases, 4 refused), `onesample_validate` | done |
| Missing values, ties, weights, precision and sample size handled according to the method | §5, the ties and missing cases, §8, §6, `too_few_pairs`, the df boundary | done; weights not supported, stated |
| Diagnostic meanings and limitations documented | §5, §8; `OneSampleCode` doc comments; the title, description and normality sentences in `capabilityWording.ts` | partly: the code sentences are written (8D-2b, `onesampleWording.ts`); the screen is 8D-2c |
| Independent reference comparison completed | R `t.test(paired = TRUE)` and effectsize, §6, §9 | done |
| Unsupported designs return explicit reasons | §5 (`same_column`, `too_few_pairs`, `differences_constant`, …), §8; at the command boundary (`stats_onesample_commands::each_refusal_has_its_exact_code`, `differences_essentially_constant` included) and on the screen | core and commands done (8D-2a); screen 8D-2c |
| Result manifest and export verified | `onesample_validate` (CSV round trip with the column rows, sentinels, rows add up, ten runs byte-identical); through IPC (`stats_onesample_commands`), the privacy matrix (`stats_onesample_privacy_matrix`), a project's save, rerun and export (`project_onesample`, the export-input pin); the screen and the save dialog | core and app side done (8D-2a); screen and save dialog 8D-2c |
