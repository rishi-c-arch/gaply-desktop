# Cronbach's alpha, as an analysis

Capability `stats.analyze.cronbach_alpha`, version 1, **Planned**, in the
Statistics workspace (Research OS, Phase 8I; decisions 362 to 373 of
1 Nov 2026). This page is the capability's `documentation_id`
(`methods/cronbach_alpha`).

**This is a stub (decision 372).** The full page, with every formula and its
source, each oracle, each gate and its measurement, the speed measured and
the differences from SPSS's documented formulas, comes with the method's
screen in 8I-2.

What exists now:

- **The pure core:** `gaply_core::stats_alpha`, checked against R 4.6.1 and
  psych 2.6.9 in `tests/alpha_r_golden.rs` (the descriptor's validation
  suite) and `tests/alpha_cases.rs`.
  - Two or more numeric items, in the file's column order, over the rows in
    which every item has a value.
  - The items the user chooses reverse-scored on the scale's ends the user
    gives (min + max − x), never on the values seen.
  - Cronbach's alpha with a 95% interval by Feldt's F, standardized alpha
    with the mean, smallest and largest inter-item correlation, and each
    item's mean, SD, scale mean and variance if left out, corrected
    item-total correlation and alpha if left out.
- **The F quantile:** `gaply_core::stats_dist::f_quantile_upper`, which
  Feldt's interval is taken from, measured against an R `uniroot` oracle in
  `tests/alpha_r_golden.rs`.

What does not exist yet: a command, a run, a project, an export path and a
screen. Until they do, the capability is Planned, so:

- every façade refuses it in every build;
- the Statistics home lists it only among the planned methods, with no way to
  open or run it.

What it does not do, as its descriptor states:

- Alpha assumes the items measure one thing; Gaply does not check that.
- McDonald's omega and squared multiple correlations are not shown yet.
