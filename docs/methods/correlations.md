# Correlations, as an analysis

Capability `stats.analyze.correlations`, version 1, **Planned**, in the
Statistics workspace (Research OS, Phase 8H; decisions 334 to 344 of
1 Nov 2026). This page is the capability's `documentation_id`
(`methods/correlations`).

**This is a stub (decision 344).** The full page, with every formula and its
source, each oracle, each gate and its measurement, the speed budget and the
differences from SPSS's documented formulas, comes with the method's screen
in 8H-2.

What exists now:

- **The pure core:** `gaply_core::stats_corr`, checked against R 4.6.1 in
  `tests/corr_r_golden.rs` (the descriptor's validation suite) and
  `tests/corr_cases.rs`.
  - Two or more numeric columns, in the file's column order.
  - Pearson's r, Spearman's rho and Kendall's tau-b, only those the user
    ticks, each pair with its n, statistic, two-tailed p and 95% interval.
  - Missing values handled pairwise or listwise, as the user chooses.
  - Each variable's n, mean and SD over the rows used.
- **The normal tail:** `gaply_core::stats_dist::normal_two_tailed`, which
  Kendall's p is taken from, measured against R's `pnorm` in
  `tests/corr_r_golden.rs`.

What does not exist yet: a command, a run, a project, an export path and a
screen. Until they do, the capability is Planned, so:

- every façade refuses it in every build;
- the Statistics home lists it only among the planned methods, with no way to
  open or run it.

What it does not do, as its descriptor states:

- Pearson's p and interval assume the two columns are roughly normal
  together; Gaply does not check.
- p values are two-tailed only, and there are no partial correlations.
