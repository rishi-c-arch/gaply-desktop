# Scale scores

Capability `stats.prepare.scale_score`, version 1, **Planned**, in the
Statistics workspace (Research OS, Phase 8M; decisions 404 to 412 of 2 Nov
2026; ADR-011). This page is the capability's `documentation_id`
(`methods/scale_scores`).

**This is a stub (decision 411).** The full page, with every rule and its
source, the oracle, each gate and its measurement, the sentence about a sum
over fewer items and the differences from SPSS's documented functions, comes
with the capability's screen.

What exists now:

- **The pure core:** `gaply_core::stats_prep`, checked against base R 4.6.1 in
  `tests/prep_r_golden.rs` (the descriptor's validation suite), with its rules
  in `tests/prep_cases.rs` and its metamorphic tests and pinned columns in
  `tests/prep_properties.rs`.
  - A new numeric column that is the sum or the mean of two to 127 items the
    user chooses from the file's columns, any of them reverse-scored on the
    scale's ends the user types (min + max − x).
  - No default for the statistic or for the rule for missing answers: every
    item has a value, or at least n do, n from 1 to one fewer than the items.
  - A row that cannot be scored has an empty cell, never a code. Four counts
    say what happened to the rows. A score no row can have is refused.
  - The steps are kept as a recipe, and the file is never changed. The prepared
    data's CSV is made as bytes.

What does not exist yet: a command, a project's copy of the recipe, a save path
and a screen. Until they do, the capability is Planned, so:

- every façade refuses it in every build;
- the Statistics home lists it only among the planned methods, with no way to
  open or run it.

What it does not do, as its descriptor states:

- A score's items are the file's columns; a score of other scores is not offered.
- Recoding values, computing from a formula and selecting cases are not offered yet.
