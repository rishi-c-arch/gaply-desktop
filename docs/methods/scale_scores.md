# Scale scores

Capability `stats.prepare.scale_score`, version 1, **Beta**
(`stats_registry/mod.rs:225`, since 8M-2, decision 416), in the Statistics
workspace (Research OS, Phases 8M to 8T-2b; decisions 404 to 520 of 2 to
7 Nov 2026; ADR-011; D309). This page is the capability's `documentation_id`
(`methods/scale_scores`).

It makes a new numeric column, a **score**, from items the user chooses among
the file's own numeric columns: the sum or the mean of their values, any item
reverse-scored on the scale's ends the user types, under a rule for missing
answers the user chooses. Gaply keeps those steps as a **recipe** and never
changes the file. The pure core is `gaply_core::stats_prep`; the app reaches
it through four commands, `stats_prep_make`, `_clear`, `_recipe` and `_csv`
(`stats_prep_commands.rs`), behind the workspace check. The screen is the
Statistics window's **Transform, Scale score…** (ADR-012; `ScoreDialog.tsx`,
`ScoreSettings.tsx`), behind `REACT_APP_FEATURE_STATS_LAYOUT`, which hides
unfinished UI and is not an authorization boundary. A score is used by every
method, saved with its recipe in a project and rebuilt from it on a rerun, and
the prepared data can be exported (§7).

`ENGINE_VERSION` is 1. A future change to how a score is computed is a bump,
and a saved prepared run is then refused on rerun, never rebuilt silently
(decision 439).

## 1. Inputs and the recipe

A recipe, in JSON (`src/screens/statistics/fixtures/prepRecipeView.json`;
`prep_cases.rs`'s
`the_recipes_spelling_is_snake_case_externally_tagged_and_camel_case_fields`):

```json
{ "schemaVersion": 1, "capabilityId": "stats.prepare.scale_score", "capabilityVersion": 1, "engineVersion": 1,
  "sourceColumns": 7,
  "steps": [ { "kind": "scale_score", "alias": "C8", "name": "wellbeing", "items": ["C1", "C2", "C3"],
               "reversed": ["C2"], "scale": { "min": 1.0, "max": 5.0 }, "statistic": "mean",
               "rule": { "at_least": 2 }, "missingCodes": [9.0] } ] }
```

- **sourceColumns**: the file's columns. A recipe is refused on another file
  width (`source_columns_mismatch`) and on a table that is already prepared
  (`source_already_prepared`), and needs at least one step (`no_steps`).
- **alias**: each score's column is numbered on from the file's, `C(k + j)`,
  in the recipe's order (`alias_not_next`; ADR-011 Part 4).
- **name**: as the user typed it, trimmed; 1 to 128 characters counted by
  code point (`NAME_MAX_CHARS`, `stats_prep/mod.rs:74`, pinned to the screen's
  `SCORE_NAME_MAX_CHARS` by `stats_prep_screen_wire.rs`); no invisible
  character; not a file header, another score's name, or an alias word such
  as `C3` (`prep_cases.rs`'s
  `a_name_is_taken_as_typed_trimmed_one_to_128_characters_and_visible` and
  `a_name_may_not_be_a_file_header_another_scores_name_or_an_alias_word`). The
  name is the one text in a recipe: it lives in the recipe alone and never
  reaches a schema or a result (§9).
- **items**: two or more of the file's numeric columns, in column order, once
  each (`MIN_ITEMS`, `stats_prep/mod.rs:77`). A column holding text or a
  non-finite cell is refused with its count, as alpha's items are (ADR-011
  Part 1). A score's items are the file's columns: a score of scores is not
  offered.
- **reversed** and **scale**: a subset of the items, in the same order; the
  scale's two ends are given **only** when an item is reversed, both finite,
  the lowest below the highest (`the_reversed_items_and_the_scale_have_one_form`).
- **statistic** (`sum` or `mean`) and **rule** (`every_item` or
  `{ "at_least": n }`, n from 1 to the items less one): **neither has a
  default**. Until both are chosen the recipe needs input and nothing is built
  (`a_statistic_or_rule_left_open_needs_input_and_builds_nothing`).
- **missingCodes**: the codes the user ticked, finite, ascending, once each,
  applying by value to every item of that score (ADR-011 Part 3). A file's own
  declared user-missing values are already empty before this.
- **-0** as an end or a code is refused (`negative_zero`): it equals 0 and has
  other bytes, so it would make one recipe two hashes (decision 416). The
  screen sends a typed -0 as 0 (`prepTypes.ts`'s `typedNumber`).

One choice gives one recipe: a recipe out of order, with a repeat, a reversed
item that is not an item, ends with nothing reversed or a reversal with no
ends, or `at_least` equal to the items, is refused with its own closed code,
never corrected silently. The screen fills nothing in: no item, box, end,
statistic, rule or code is chosen until the user chooses it (decision 504).

## 2. Formulas, and where each number comes from

Per row, an item's **value** is a number that is not a ticked code, reversed
when the item is reversed. When the rule holds:

- **the sum** is the total of those values, in column order;
- **the mean** is `stats_verify::mean` of those values, the one mean in the
  product (decision 413, question 1).

**One formula, one place (decision 406; ADR-011 Part 2).** The reversal, the
count of values outside the ends and the row total are `stats_alpha`'s,
extracted so alpha and the score call the same functions with no byte of
alpha's changing; the mean is `stats_verify::mean`. No arithmetic is written
in `stats_prep`, and the build uses only addition, subtraction and division
(`stats_prep/mod.rs` header).

**The mean and total ÷ count.** `stats_verify::mean` is not total ÷ count in
its last digits: over the oracle's fixtures, 37,312 of 99,590 mean rows
(37.5%) differ from total ÷ count, every one within 0.067 of the row's
rounding scale k · ε · M (decision 413; `prep_r_golden.rs`'s total ÷ count
check, :322-391).

## 3. Reverse-scoring, missing codes and the rule

**The reversal is on the ends the user types, never the values seen.** A
reversed item's value x becomes min + max − x. The screen says the ends are
the questionnaire's, for example 1 and 5, not the lowest and highest answers
in the data. A code outside the ends is not reversed, since it is not a value
(`a_reversal_is_on_the_ends_given_and_a_code_outside_them_is_not_reversed`).

**Its refusal:** a reversed item with a value outside [min, max], ticked codes
removed, is refused before anything is built, one finding per column with its
count (`reversed_outside_scale`): the ends are wrong, or a code is not ticked.
Nothing is clipped.

**The rule.** `every_item` scores a row only when every item has a value;
`at_least n` scores a row with n or more values and not one fewer
(`every_item_scores_only_a_complete_row_and_counts_the_rest`,
`at_least_scores_exactly_n_values_and_not_one_fewer`). An unticked code is a
value; a ticked one is not (`an_unticked_code_is_a_value_and_a_ticked_one_is_not`).

**The codes in the methods.** A later method's ticked code equal to a value of
a chosen score is refused, naming the column and the count, rather than
silently making that person's score missing; the suspect-code rule is not
applied to a score (ADR-011 Part 3; decision 418).

## 4. Not computable, the counts, and the sum over fewer items

- **A row that cannot be scored has an empty cell, never a code**: too few
  values under the rule, or a sum or mean a double cannot hold, including a
  reversal that overflows (`a_total_that_overflows_is_an_empty_cell_and_is_counted`,
  `a_reversal_that_overflows_is_an_empty_cell_too`).
- **Four counts** per score say what happened to every row: scored from every
  item, scored from fewer, not scored for too few, not scored for overflow
  (`scored_from_every_item`, `scored_from_fewer`, `not_scored_too_few`,
  `not_scored_overflow`). They are counts, with no values. The window says
  them after a make, by the score's name.
- **A score no row can have is refused** with its counts (`no_row_scored`).
- **A sum over fewer items.** Under `at_least n`, a sum over fewer items is
  not on the same scale as a sum over every item: on ten items scored 1 to 5,
  someone who answered eight scores at most 40 and someone who answered ten at
  most 50 (ADR-011 Part 1). **Gaply does not rescale it**, and does not refuse
  it, since the choice is the user's: the recipe carries the note
  `sum_over_fewer_items` and the dialog says, before **Make the column**,
  "For score N, a person scored from fewer items has a sum over fewer items,
  which is not on the same scale as a sum over every item. Gaply does not
  rescale it." (naming the score once it has a name). A mean carries no note
  (`a_sum_under_at_least_carries_a_note_and_a_mean_does_not`).

## 5. Precision, the gates, and the oracle

Measured 2 Nov 2026, macOS aarch64 (decision 409); asserted on every platform,
and passing on Linux x86_64 outside this Mac at 8561dba (decision 413).

**Against base R 4.6.1** (`tests/prep_r_golden.rs`, the descriptor's
validation suite; 39 cases, 36 computed and 3 refused, 2,994 values). Its
gates, `prep_r_golden.rs:42-62`, as a multiple of the row's rounding scale
k · ε · M, M the row's sum of magnitudes:

| what | gate | measured |
|---|---|---|
| which rows are scored, the four counts | exact | exact |
| a sum of integer items | exact | exact |
| any other sum | 1.0 (the stated rounding bound) | 0, over 1,115 non-integer rows |
| a mean | 0.7 | 6.667e-2 (`rev_0to10_mean` row 1), over 1,571 rows |
| the summary of a score: mean, min, max (relative) | 2e-15 | 1.406e-16 |
| the summary of a score: SD, as a multiple of ε · \|mean\| / SD | 4.0 | 0.32 |

A sum's measured error is exactly 0 because Gaply and R add the same doubles
in the same order, and R accumulates in double here; ten times 0 is no gate,
so the gate is the scale itself. One case Gaply refuses where R computes is
pinned as expected: a row c − c − c whose mean's second pass overflows is an
empty cell counted as overflow (`GAPLY_OVERFLOWS_WHERE_R_COMPUTES`). Gaply
parses every cell to the double R read (`cells_bits.csv`).

**The oracle** (ADR-008, ADR-011 Part 13; nothing copied):
`tests/fixtures/stats_prep/make_prep_fixtures.R`, base R only, run as
`/usr/local/bin/Rscript`, computing each score from a `read.csv` of the
written file with `rowSums` and `rowMeans`, the same reversal and rules (the
same rules written twice). A near-overflow table is built by formula on both
sides, since R's parser misreads those doubles. Beside it: metamorphic tests
on a binary grid (`prep_properties.rs`), a pinned SHA-256 of the prepared
columns' bits, the seven methods on a score (`prep_methods.rs`), each result's
bytes equal to those on the same values as a file's column (decision 419),
and a real-app chain against R (`~/gaply-corpus/os/phase8/realapp_8t2a.md`,
whose scores, t-test, correlation and alpha matched R at the window's shown
precision; decision 512).

## 6. Limits, and the speed

- **Items:** 2 to 127. The table's columns, scores included, are held to 128
  (`MAX_COLUMNS`), so 127 items take a score and a 128-column file takes none
  (`one_hundred_and_twenty_seven_items_take_a_score_and_128_are_too_wide`;
  `too_wide`).
- **Cells:** the prepared table's rows times every column against this
  computer's cap, refusing only more (`stats_prep/mod.rs:609-610`,
  `cells_over_cap`; the cap is the caller's, so `stats_prep` holds no tier).
  On the 8 GB tier's 5,000,000 cells (`supplementary.rs:152-158`), 100,000 ×
  49 plus one score is the largest prepared table; see
  [large_samples.md](large_samples.md) §8.
- **Speed:** the build took 0.05 s at 5,000,000 cells and 0.15 to 0.18 s at
  100,000 × 128 (decisions 410 and 413), so a make needs no run manager, no
  progress and no cancel. Since 8T-1b a make profiles only the columns it
  adds: at 100,000 × 49 with one score of 49 items, the make's median went
  from 1.468 s to 0.083 s, and at 100,000 × (40 numeric + 10 distinct text)
  from 1.407 s to 0.114 s, five runs each in a release probe, every view's
  bytes unchanged (decision 502).
- **The screen:** the dialog with 127 items rendered in 85 to 116 ms in jsdom
  (`ScoreDialog.vitest.tsx`, decision 512).
- **A rerun over this computer's limits** is refused as
  `project.rerun_over_limits`, as for any run (decision 492).

## 7. The export

**File, Export the prepared data (CSV)…** in the Statistics window (decision
514; ADR-011 Part 8). It is live once the open file holds a score; with none
it says "Make a scale score first." A run going does not disable it, since
the command only reads. It asks first, under the title "Export the prepared
data", with ADR-011 Part 8's sentence:

> "This file holds your data as Gaply read it, with your scores added. Missing
> answers are empty, SPSS's value labels are not included, and numbers are
> written in full. Your original file is not changed."

**Continue** reads the CSV (`stats_prep_csv`) and opens the save dialog with
the fixed name `prepared_data.csv`, which carries no file name. A closed save
dialog writes nothing ("Not saved: the save dialog was closed."); a saved file
gives "Saved. SHA-256 of the file: …" with the start of its hash. Gaply keeps
no copy, and Rust writes nothing: it returns the bytes and their hash, and the
one file the user names is written by the save dialog (ADR-004a's path).

What the file holds (`stats_prep::export::prepared_csv`, ADR-011 Part 8):

- a header row of the file's headers and then the scores' typed names;
- every row, the file's columns in order and then the scores;
- every number at all its digits, Rust's shortest form that reads back to the
  same double, so `1.50` reads `1.5`, unquoted, a negative score included
  (`every_number_is_written_at_all_its_digits`,
  `a_negative_score_and_a_negative_item_are_written_unquoted`);
- an empty field for every missing cell, whatever made it missing; text as
  Gaply read it; no value labels;
- RFC 4180 with CRLF line ends; headers, names and text quoted through
  `stats_desc::export::text`, so a field a spreadsheet would run as a formula
  gains one leading apostrophe
  (`a_typed_name_that_starts_like_a_formula_is_guarded_in_the_header`).

Opening the file again gives the same numbers: the scores are then the file's
own columns. **The route** is an array of numbers through IPC
(`ExportView`), measured on the Rust side by 8M-3 (decision 430); the
webview's half is measured by `realapp_8t2b.md` at 100,000 × 49 plus one
score, and moves to a binary response, a step of its own, if it fails there
(decision 517).

## 8. What SPSS documents, and where it differs

IBM's own documentation, read as documentation, never a gate:

- IBM SPSS Statistics 31.0.0, Statistical functions, read 7 Nov 2026:
  https://www.ibm.com/docs/en/spss-statistics/31.0.0?topic=expressions-statistical-functions
- IBM SPSS Statistics 30.0.0, Statistical functions, read 7 Nov 2026:
  https://www.ibm.com/docs/en/spss-statistics/30.0.0?topic=expressions-statistical-functions
- IBM SPSS Statistics 25 Command Syntax Reference, Chapter 2, Universals,
  "Statistical functions", "Missing values" and Table 5, read 2 Nov 2026 for
  ADR-011 Part 1:
  https://www.ibm.com/docs/en/SSLVMB_25.0.0/pdf/en/IBM_SPSS_Statistics_Command_Syntax_Reference.pdf

What they say: `SUM` "Returns the sum of its arguments that have valid,
nonmissing values." `MEAN` "Returns the arithmetic mean of its arguments that
have valid, nonmissing values." "The .n suffix can be used with all
statistical functions to specify the number of valid arguments." "The default
for n is 2 for SD, VARIANCE, and CFVAR and 1 for other statistical
functions." "MEAN.2(A,B,C,D) returns the mean of the valid values for
variables A, B, C, and D only if at least two of the variables have valid
values." "If the number specified exceeds the number of arguments in the
function, the result is system-missing." (versions 30 and 31). Version 25's
Table 5 adds that `MEAN.n` and `SUM.n` are system-missing when "Fewer than n
arguments are valid; the default n is 1." Versions 30 and 31 do not restate
that table on the page read; it is not claimed for them.

| | SPSS, as documented | Gaply |
|---|---|---|
| the sum | `SUM.n`: the valid values added, not rescaled | the same: added, not rescaled, with the note of §4 |
| the mean | `MEAN.n`: divided by the number of valid values | the same quantity, through `stats_verify::mean` (last digits, §2) |
| n's default | 1: a person is scored from one answer | **no default**: the user chooses every item or at least n, n from 1 to k − 1 |
| n equal to or above the arguments | above: system-missing | `at_least k` refused, so one choice has one form; every item is the rule for k |
| a row not scored | system-missing | an empty cell, with the four counts |
| reverse-scoring | not part of SUM or MEAN (a separate transformation, by the user) | inside the step, on the ends the user types, refused outside them |
| missing codes | the file's user-missing values are not valid | the same for declared values; undeclared codes the user ticks per score |
| the result | a new variable in the user's data file | a column of the prepared table in memory; the file never changes, and the export is a separate file the user names |

## 9. What is not offered, and the limitations

- **No weights**, no standardised scores (z or T), **no imputation** of a
  missing answer, and **no rescaled or prorated sum**: a prorated sum is a
  different statistic from `SUM.n`, not asked for, with no oracle for what a
  thesis means by a total (ADR-011 Part 1).
- A score's items are the file's columns: no score of scores.
- Recoding values, computing from a formula and selecting cases are not
  offered (the window's Transform menu lists them under "Coming later").
- **No verdict and no advice**: the dialog and its sentences say what was
  made and counted, never whether a scale is good, which item to drop or
  which to reverse (decision 504); `scoreWording.vitest.ts` and
  `preparedSentences.vitest.tsx` read every sentence for alpha's and decision
  66's verdict words.
- **Privacy (ADR-011 Part 11):** the typed name lives in the recipe alone; a
  score's header in the table and every result is its alias word, so a name
  never reaches a schema or a result (`the_typed_name_never_reaches_the_schema`).
  A refusal holds closed codes and numbers, and `Debug` prints counts, never a
  name or a value (`debug_prints_counts_and_never_a_name_or_a_value`). The
  privacy matrices of 8M-3 and 8M-4 forbid a name, an item and a score
  sentinel on every surface (decisions 427 and 437).

**Saved and rerun (ADR-011 Parts 6 and 7).** A Save of a run on prepared data
keeps the recipe, with the names typed, as a project artifact beside the data
file; the Save dialog says so ("Saving keeps a copy of the data file, exactly
as it is, and the recipe of its scale scores, with the names you typed for
them, …", decision 515). A rerun reads the recipe, verifies its hash, and
builds the scores again from the project's copy of the file before running the
method, and the project page says so before it starts (decision 516). Every
method's saved prepared run reruns `Identical` (`prep_project.rs`). The
project's report and PROV export give the recipe by position and never by
name (decision 435).

## 10. Blueprint §18.2 acceptance

| item | evidence | status |
|---|---|---|
| Formula/method and variant documented | §1 to §4; `stats_prep/mod.rs` header; ADR-011 | done |
| Golden fixtures and edge cases pass | `prep_r_golden` (39 cases), `prep_cases`, `prep_properties`, `prep_methods`, `prep_original` | done |
| Missing values, reversal, weights, precision and sample size handled | §3, §4, §5, §6; weights not supported, stated | done |
| Diagnostic meanings and limitations documented | §1, §4, §9; `PrepCode` docs; a sentence for each code the screen can show (`prepWording.ts`, `prepCodes.json` pinned to Rust by `stats_prep_screen_wire.rs`) | done |
| Independent reference comparison completed | base R 4.6.1, §5; the real-app chain against R (`realapp_8t2a.md`) | done |
| Unsupported designs return explicit reasons | §1, §3, §4; each refusal's code through IPC (`stats_prep_commands.rs`) | done |
| Result manifest and export verified | §7, §9; every manifest carries the preparation's hashes; the command's CSV equals the module's (`stats_prep_slot.rs`); a saved prepared run reruns identical (`prep_project.rs`); the export's webview route in `realapp_8t2b.md` | done but the route's real-app figures, recorded once `realapp_8t2b.md` runs |
