# The one-way ANOVA, as an analysis

Capability `stats.analyze.one_way_anova`, version 1, **Beta**, in the
Statistics workspace (Research OS, Phase 8G; decisions 310 to 333, 31 Oct and
1 Nov 2026; D306). This page is the capability's `documentation_id`
(`methods/one_way_anova`).

It computes a one-way analysis of variance on the user's own data: a numeric
outcome split by a group of 2 to 50 levels. It is **not** a reproduction of a
reported result; that is Level 1's `stats.verify.one_way_anova`, which this
capability does not change. The pure core is `gaply_core::stats_anova`; its
façade is `stats_adapters::anova_admit` / `anova_validate` / `anova_run`. The
app reaches it through the `stats_anova_*` commands and the run manager's
ANOVA kind; the screen is at `/app/statistics/anova`, behind
`REACT_APP_FEATURE_STATS_DESCRIBE`, which hides unfinished UI and is not an
authorization boundary. Planned in 8G (decision 321), Beta since 8G-2's Part 3
(decision 333).

## 1. Inputs and the spec

`AnovaParams { outcome, group, levels, post_hoc, missing_codes }`:

- `outcome`, `group`: Level 1's column aliases (`C1`, `C2`, …), two different
  columns.
- `levels`: `null` includes every level of the group; otherwise the included
  levels' indexes in code-point order, at least two. The screen ticks every
  level, lets the user untick any, and sends `null` when all are ticked and
  the ticked indexes **ascending** otherwise, so one choice is one spec and
  one parameters hash (decision 329). The manifest hashes the parameters.
- `post_hoc`: the tests the user ticked, any of `tukey`, `games_howell`,
  `bonferroni`, sent in that order; **none by default**. Gaply says which
  test the user chose, never which they should use (decision 312).
- `missing_codes`: values of the outcome to treat as missing; never applied to
  the group.

The levels come from `stats_ttest::group_levels`, the core's own level
function, through the method's own command `stats_anova_group_levels`
(decisions 312 and 329).

## 2. Formulas, and where each number comes from

k groups, group i holding nᵢ values with mean mᵢ and sample variance sᵢ²
(denominator nᵢ − 1), N = Σ nᵢ.

| quantity | formula | source |
|---|---|---|
| SS between, SS within, F, df | the one-way ANOVA | `stats_verify::one_way_anova`, as Level 1 takes it; its result gained `ss_between` and `ss_within` (decision 313), and its own p (statrs's 1 − cdf) is never read |
| SS total | SS between + SS within | here |
| MS between, MS within | SS / df | here |
| p of F | P(F > f; k − 1, N − k) | `stats_dist::f_upper` |
| Welch's ANOVA | wᵢ = nᵢ/sᵢ², W = Σ wᵢ, m = Σ wᵢ mᵢ / W, λ = Σ (1 − wᵢ/W)² / (nᵢ − 1); F = [Σ wᵢ (mᵢ − m)² / (k − 1)] / [1 + 2(k − 2)λ / (k² − 1)], df1 = k − 1, df2 = (k² − 1) / (3λ) | here; B. L. Welch (1951), Biometrika 38(3/4), 330–336, doi:10.2307/2332579; p from `f_upper` |
| Levene's test, mean | the ANOVA of \|x − mᵢ\| | `stats_verify::one_way_anova` on the deviations, each group sorted ascending; H. Levene (1960) |
| Levene's test, median | the ANOVA of \|x − medianᵢ\| (the mean of the two middle values for even nᵢ) | the same; M. B. Brown and A. B. Forsythe (1974), JASA 69(346), 364–367, doi:10.1080/01621459.1974.10482955 |
| eta squared | SS between / SS total | here |
| omega squared | (SS between − (k − 1) MS within) / (SS total + MS within), shown as computed when negative | here |
| group table | n, mean, SD, SE = SD/√n, the 95% interval of the mean (`stats_ttest::interval_95`, `t_quantile_975` at n − 1), min, max | `stats_verify::mean` and `sample_variance` on the values ascending, so permuting rows changes no bit |

Post hoc, for each unordered pair once, in level order, **first level minus
second**: the difference, its SE, the statistic, df, p and a 95% interval.

| test | SE | statistic | df | p | 95% interval |
|---|---|---|---|---|---|
| Tukey HSD (Tukey–Kramer for unequal n) | √(MS within (1/nᵢ + 1/nⱼ)) | q = √2 · \|d\| / SE | N − k | P(Q > q; k, N − k) | d ± c · SE / √2, c the 0.95 quantile of Q(k, N − k), simultaneous |
| Games–Howell | √(sᵢ²/nᵢ + sⱼ²/nⱼ) | q = √2 · \|d\| / SE | Welch's, per pair | P(Q > q; k, dfᵢⱼ) | d ± cᵢⱼ · SE / √2, cᵢⱼ the 0.95 quantile at the pair's own df, simultaneous |
| Bonferroni | √(MS within (1/nᵢ + 1/nⱼ)) | t = d / SE | N − k | min(1, m · 2 P(T > \|t\|)), m = k(k−1)/2 | d ± t_{1 − 0.05/(2m)} · SE |

Q is the studentized range (§3). Bonferroni's p is `stats_dist::t_two_tailed`
times m; its quantile is `stats_dist::t_quantile` (decision 314). No df is
rounded and no quantile is shared between pairs whose df differ (decision 322).

## 3. The studentized range

`stats_dist::studentized_range_upper(q, k, df)` and
`studentized_range_quantile(p, k, df)` (decision 314), written from the
distribution's definition, with std and statrs only, from the method in
M. D. Copenhaver and B. S. Holland (1988), J. Stat. Comput. Simul.,
doi:10.1080/00949658808811082, and R. E. Lund and J. R. Lund (1983),
Algorithm AS 190, Applied Statistics 32(2), 204–210, doi:10.2307/2347300. No
code, and no published listing, was copied.

- **The tail.** P(W > w) = k ∫ φ(z) [Φ(z)^(k−1) − (Φ(z) − Φ(z − w))^(k−1)] dz,
  the bracket computed as −Φ(z)^(k−1) · expm1((k − 1) · ln1p(−Φ(z − w)/Φ(z))),
  never as 1 − cdf; then P(Q > q) = ∫ P(W > q s) f(s) ds over the density of
  χ_df/√df, in u = ln s, with Stirling's remainder for the log-gamma.
- **The rules.** The inner integral on a fixed 40-panel × 10-node
  Gauss–Legendre grid; nodes whose term is bounded below 1e-30 are not summed
  (decision 322's skip, at most 400 × 1e-30 against a p of 1e-10 or more). The
  outer integral adaptive Gauss–Legendre (20 nodes), starting on 4 panels,
  centred and scaled on the χ density, whose width is about 1/√(2 df).
- **The quantile.** A bracketed search on ln p: its first upper end 1.5 Newton
  steps from the infinite-df quantile, then the secant through the last two
  points while it stays inside the bracket (regula falsi with the Illinois
  step, or halving, otherwise); stop at |Δ ln p| ≤ 1e-12 or width ≤ 1e-11 · q.
- **The domain.** k from 2 to 50; df from 1 to infinity. Outside it, `None`,
  and the method says `distribution_out_of_range`.
- **The floor.** p is computed to the gate down to 1e-10 (`SRANGE_P_FLOOR`). A
  p below it is never refused and never shown as a number: the result holds
  `below: 1e-10`, the screen shows "below 1e-10" and the CSV writes the floor
  with the word `below`, with the note `p_below_floor`.

## 4. Level order and the sign

The group column is read as categorical with Level 1's reading of cells
(decision 17) and its levels are sorted by **code point** (decision 78), never
by locale: `B` (U+0042) before `a` (U+0061) before `Ä` (U+00C4). Every table
is in that order, and each pair is the first level minus the second.

## 5. Exclusions and edge cases

A row is left out for the first reason that applies: `missing_group`,
`other_level` (a level left out), `missing_outcome`, `missing_code`. Each
reason with rows is listed with its count and column; rows used plus the rows
left out equal the rows read.

Edge cases are decided from the values before each call, never from an
error's text (decision 312):

| case | what is shown |
|---|---|
| an included group with fewer than 2 values | refused before any run, `group_too_small` {index, count} |
| the outcome constant in every row (SS total 0) | only the group table; everything else `outcome_constant` |
| every group constant, means differing (MS within 0) | the group table, the sums of squares and eta squared; F, its p, omega squared, Welch's, both Levene's and every post hoc test `within_variance_zero` |
| some groups, not all, of variance 0 | Welch's ANOVA `welch_variance_zero`; a Games–Howell pair of two such groups `pair_both_constant`, every other pair computed |
| every group's deviations equal within the group, per centre | that Levene's test `levene_deviations_equal` (always so when every group has n = 2) |
| omega squared below 0 | shown as computed, note `omega_squared_negative` |
| a level index out of range, repeated, or fewer than 2 included | invalid: `level_out_of_range`, `level_repeated`, `too_few_levels` |
| more than 50 levels | `too_many_levels` {count, limit}; the levels command refuses before listing any label |
| a post hoc test on more than `POST_HOC_MAX_LEVELS` levels | `post_hoc_too_many_levels` (the cap is the group's own 50, §7) |
| df above 1e7 | `distribution_out_of_range` |
| a value a double cannot hold | `overflow`, `underflow`, `arithmetic_refused` |

An ambiguous case is never a definite answer: every number is finite or
replaced by its closed code, and the screen shows that code's sentence where
the number would be (decision 331).

## 6. Precision, the gates, and the oracles

Every gate is ten times the largest error measured (decision 316), asserted on
every platform.

**Against R 4.6.1** (`tests/anova_r_golden.rs`, the descriptor's validation
suite; 17 cases, 21,876 numbers, 16 families): relative errors unless marked.

| family | gate | measured (31 Oct 2026, macOS aarch64) |
|---|---|---|
| group table | 7e-14 | 6.86e-15 |
| ANOVA table | 1e-11 | 9.33e-13 |
| ANOVA p | 6e-10 | 5.21e-11 (at p 5.1e-12) |
| Welch | 6e-12 | 5.75e-13 |
| Levene | 2e-10 | 1.46e-11 |
| effect sizes | 2e-11 | 1.00e-12 |
| Tukey q, Games–Howell q | 7e-14 | 6.2e-15 |
| Tukey p (absolute) | 7e-6 | 5.67e-7 after decision 322 (R's ptukey) |
| Tukey interval | 2e-5 | 1.99e-6 (R's qtukey) |
| Games–Howell p (absolute) | 3e-5 | 2.18e-6 (R's ptukey at df 4) |
| Games–Howell interval | 2e-4 | 1.70e-5 (R's qtukey) |
| Bonferroni | 8e-12 | 7.90e-13 |
| Bonferroni p | 1e-11 | 9.89e-13 |
| near 1e9 (`ties_offset`) | 3e-6 | 2.62e-7: means near 1e9 hold an ulp of 1.2e-7 |

The Tukey and Games–Howell p and interval gates are loose because R's
`ptukey` and `qtukey` are the reference there, and R documents `qtukey` as
accurate to about the fourth decimal place. They are R's error, not
Gaply's: Gaply's studentized range is gated against mpmath below.

**The studentized range against mpmath** (`tests/studentized_range_reference.rs`,
the definition's integral at 30 digits in mpmath 1.3.0, CPython 3.12.2,
`make_srange_fixtures.py`):

| function | rows | gate | measured |
|---|---|---|---|
| `studentized_range_upper`, p in [1e-10, 1], k 2–50, df 1–∞ | 570 (138 below df 2) | 7e-10 relative | 6.335e-11 |
| `studentized_range_quantile` | 137 | 5e-10 relative | 4.130e-11 |
| `t_quantile` against R's `qt` | 128 | 1e-10 | 9.804e-12 |
| `f_upper` at df1 2–49 against R's `pf` | 240 | 2e-9 | 1.086e-10 |

The remaining ~6e-11 is statrs's `erfc`, measured at 4.6e-11 to 8.1e-11
against mpmath for |x| ≥ 0.7: at k = 2, P(W > w) = erfc(w/2) exactly. R's
`ptukey` is measured, never the gate: it is 3.37e-3 off at q 55.72, k 50, df 2,
and returns NaN below df 2, so df 1 to 2 is gated against mpmath alone.

**The oracles** (ADR-008, for fixtures only, nothing copied): base R `aov`,
`oneway.test` (equal and unequal variances), `TukeyHSD`, `pairwise.t.test`
(Bonferroni, pooled SD), `ptukey`, `qtukey`, `pf`, `qt`; car 3.1.5's
`leveneTest` (centre mean and median); effectsize 1.0.3's `eta_squared` and
`omega_squared` (cross-checked where omega ≥ 0, since effectsize truncates it
at 0); Games–Howell computed in R from its formula with `ptukey` and `qtukey`
(**the same formula written twice**, a check of the arithmetic, not an
independent method); mpmath for the studentized range. The generator is
`tests/fixtures/stats_anova/make_anova_fixtures.R`, run as
`/usr/local/bin/Rscript`; levels are set in code-point order there with
`sort(method = "radix")`.

## 7. Speed, and the post hoc cap

The worst case, 50 levels and 100,000 rows with all three post hoc tests,
measured in a release build on an idle Mac (31 Oct 2026): 29.14–29.31 s before
decision 322's changes to the studentized range, **8.33–8.39 s** over three
runs after (Games–Howell 7.89–7.95 s, Tukey 0.46–0.47 s, Bonferroni 0.03 s),
bit-for-bit identical across runs and processes. That is within decision
322's 15 s and the run manager's 30 s ceiling, so `POST_HOC_MAX_LEVELS` is the
group's own 50 and the one place the cap lives. Re-measure whenever
`studentized_range.rs` changes; no timing test is in the suite, since a timing
test is not deterministic.

**Check points** (decisions 317 and 328): the run calls the run manager's
check point once before the arithmetic and once before each post hoc pair, so
Cancel and the 30 s ceiling act during the post hoc tests. The record's total
is `stats_anova::check_points` for the included levels (from the spec when it
names them, from the dataset's group when it does not), and the screen says
"{done} of {total} steps". Tested through IPC: every check point reaches the
hook in order; a cancel at the fifth Games–Howell pair ends the run Cancelled
with no result; a check point that "sleeps" 31 s on an injected clock ends it
TimedOut (`src-tauri/tests/stats_anova_commands.rs`).

**The screen at 50 levels**: each test's table, 1,225 rows, is shown in full,
one test at a time, under a heading with its row count. Measured in jsdom
(`AnovaFlow.vitest.tsx`, 1 Nov 2026): the card's first render with Tukey's
table 305 ms, switching to Games–Howell 59 ms and to Bonferroni 64 ms.

## 8. The export

`stats_anova::export::result_csv(result, manifest)` and `manifest_json`
(decision 327): one long CSV, every table in the screen's order (the group
table, the ANOVA table, Welch's ANOVA, Levene's two lines, the effect sizes,
then each chosen post hoc test's pairs), then the rows left out, the notes,
the limitations and the manifest's input, parameters and output hashes.
Columns `table, statistic, level_index, level, level_index_2, level_2, value,
word`. Every digit (Rust's shortest round-trip form); a value not computable is
empty with its code in `word`; a p below the floor is the floor with the word
`below`. The descriptive summary's field writer (`stats_desc::export::text`)
gives the one formula-injection rule. Pinned by `tests/anova_export.rs`
(field-by-field read-back, a golden CSV and manifest, ten runs
byte-identical).

## 9. What is not assessed, and the limitations

Every result carries `normality_not_assessed`, `effect_sizes_have_no_interval`
and `no_contrasts`, and the descriptor lists the matching wording codes:

- normality is not assessed; sphericity does not apply to a one-way design;
- eta and omega squared have no confidence interval;
- no Brown–Forsythe test of means, no trend tests, no planned contrasts;
- Levene's tests are information about the spreads, never advice on which
  test to use; Gaply never chooses a post hoc test;
- the design is the user's claim: a repeated-measures design given as
  independent groups cannot be detected;
- no weights;
- Levene's rule for "deviations equal within a group" is bit for bit (and by
  construction for n = 2): near-equal deviations are computed, and R's
  `leveneTest` gives an F of 8.3e30 on `data_n2.csv`, where Gaply says not
  computable;
- how large a file opens depends on this computer's memory, as for the
  t-tests ([large_samples.md](large_samples.md) §8): 100,000 data rows and 128
  columns on every computer.

The wording is neutral: no "significant", no stars, no verdict (decision 66).

## 10. Blueprint §18.2 acceptance

| item | evidence | status |
|---|---|---|
| Formula/method and variant documented | §2, §3; `stats_anova/mod.rs` and `stats_dist/studentized_range.rs` headers | done |
| Golden fixtures and edge cases pass | `anova_r_golden` (17 cases), `anova_cases`, `studentized_range_reference`, `anova_export` | done |
| Missing values, ties, weights, precision and sample size handled | §5, §6, §7; weights not supported, stated | done |
| Diagnostic meanings and limitations documented | §5, §9; `AnovaCode` docs; a sentence for each of the 57 codes the screen can show (`anovaWording.ts`, `anovaCodes.json` pinned to Rust by `stats_anova_wire.rs`, `anovaWording.vitest.ts`) | done |
| Independent reference comparison completed | R and mpmath, §6 | done |
| Unsupported designs return explicit reasons | §5; each refusal's code through IPC (`stats_anova_commands.rs`) | done |
| Result manifest and export verified | §8; through IPC the CSV equals the module's export of the run's result and manifest; the privacy matrix (`stats_anova_privacy_matrix.rs`); a saved result reruns identical, byte for byte (`project_anova.rs`) | done |
