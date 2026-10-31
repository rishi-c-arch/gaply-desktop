# The one-way ANOVA, as an analysis

Capability `stats.analyze.one_way_anova`, version 1, **Planned**, in the
Statistics workspace (Research OS, Phase 8G; decisions 310 to 324 of
31 Oct 2026). This page is the capability's `documentation_id`
(`methods/one_way_anova`).

**This is a stub (decision 321).** The full page, with every formula and its
source, each oracle, each gate and its measurement, the post hoc cap and the
studentized range's method and domain, is decision 319's and comes with the
method's screen.

What exists now:

- **The pure core:** `gaply_core::stats_anova`, checked against R 4.6.1 in
  `tests/anova_r_golden.rs` (the descriptor's validation suite) and
  `tests/anova_cases.rs`.
  - One numeric outcome, one group of 2 to 50 levels in code-point order.
  - The group table, the ANOVA table, Welch's ANOVA, Levene's test (mean and
    median), eta and omega squared.
  - Tukey, Games-Howell and Bonferroni, only those the user ticks.
- **The studentized range:** `gaply_core::stats_dist::studentized_range_upper`
  and `studentized_range_quantile`, which Tukey and Games-Howell take their p
  and interval from. Both are measured against an mpmath evaluation of the
  definition in `tests/studentized_range_reference.rs`.

What does not exist yet: a command, a run, a project, an export path and a
screen. Until they do, the capability is Planned, so:

- every façade refuses it in every build;
- the Statistics home lists it only among the planned methods, with no way to
  open or run it.

What it does not do, as its descriptor states:

- Normality is not assessed. Sphericity does not apply.
- Eta and omega squared have no confidence interval.
- There is no Brown–Forsythe test of means, no trend test and no planned
  contrast.
