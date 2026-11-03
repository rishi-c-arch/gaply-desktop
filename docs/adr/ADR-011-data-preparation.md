# ADR-011: Data preparation, scale scores first

**Status: Accepted, 2 Nov 2026 (decision 397), for Parts 1 to 9 and 11 to 14 and the disk list. Part 10 and question 12 are replaced by
ADR-012, the Statistics layout (decision 447).** Each part below is marked **[Accepted, decision 397]** or **[Replaced by ADR-012, decision 447]**.
Written 2 Nov 2026, Research OS Phase 8J (decisions 391 to 395), from the code
at d5fd559. Nothing in it is built. No code, test or fixture changed in writing
it.

## In plain words

**What scale scores will do.** You choose the items of a scale. You tick any
that are reverse-scored and type the scale's lowest and highest values. You
choose the sum or the mean, how missing answers are treated, and a name. Gaply
adds a new numeric column, and every method can use it like any other numeric
column. Your file is never changed.

**How it is kept.** Gaply keeps the steps, which we call the recipe, and not a
changed copy of your data. When you save a run to a project, the recipe is
saved with it. A rerun applies the recipe again to the project's copy of your
file. You can also save the prepared data as a CSV, through the save dialog.

**What Gaply will not do.** It chooses no item, reversal, missing rule or name
for you. It gives no advice. AI proposes nothing here. A score's name and values
never leave your computer. A person who cannot be scored gets an empty cell.

**What you are asked to approve.**

1. The design below.
2. Writing the recipe, which holds the names you type, into a project when you
   click Save. It lives where projects live now, with the same protection, and
   is deleted with the project.
3. Saving the prepared data as a CSV, only to a file you name in the save
   dialog.

**Each open choice, with my recommendation.** The numbered questions near the
end say the same in more detail.

| choice | recommendation |
|---|---|
| Which missing rules to offer | Two: a score only when every item has a value, or when at least the number of items you type has one. No default. |
| A sum over fewer items | Allowed if you choose it, with a sentence saying it is not on the same scale as a full sum. Gaply never rescales it. |
| How the mean is worked out | With the same mean function every method already uses. |
| Missing codes | One list for each score, as every method has now. |
| A method's missing code that equals a real score | Refuse, and say which score. Never drop that person silently. |
| Where the new column goes | After the file's columns, numbered on from them. |
| Can a score be a grouping column | No. It is offered and accepted only as a numeric column. |
| A score that no row can have | Refuse when you make it. |
| At the column or cell limit | Refuse, and say why. No extra allowance. |
| Where you make a score | From the Transform menu of the Statistics window, over the open file (ADR-012) |
| What the CSV holds | The data as Gaply read it, plus your scores. It is not your original file with columns added, and the save step says so. |
| Is this a capability of its own | Yes, with its own name, version and lifecycle. |

## Context

**Paths.** In this ADR `core/` is `src-tauri/gaply-core/src/`, `app/` is
`src-tauri/src/`, and `screens/` is `src/screens/statistics/`. Every file:line
is as at d5fd559. A statement about the code with no file:line is marked
*(inferred)*. Every name this ADR gives to something not yet built is a
proposal.

A thesis that checks a scale's alpha next needs the scale's score. Today a user
has to make that column in another program and open the file again. Decision
391 sets the scope: scale scores first, kept as a recipe, with the prepared data
exportable through the save dialog. Recoding, formulas and selecting cases are
not in the first version.

**What the code allows today.**

- **A Dataset can only be built by parsing.** The cells are private
  (`core/stats_l1/dataset.rs:589-592`), and the one constructor takes a
  `RawTable` of strings and a fingerprint (`core/stats_l1/dataset.rs:798`).
  `type_column` is private (`core/stats_l1/dataset.rs:662`). So no code outside
  `stats_l1` can add a column.
- **A fingerprint is SHA-256 of bytes the caller passes**
  (`core/stats_l1/dataset.rs:180-182`). The ingest passes the file's bytes
  (`app/stats_l1_ingest.rs:440`), but the type does not require that. Part 5
  uses this.
- **Every method compares its one input with `input_ref`**, which reads the
  fingerprint (`core/stats_desc/mod.rs:545-552`). Six modules hold the seven
  methods, `stats_onesample` holding both the one-sample and the paired t-test.
  Their checks are at
  `core/stats_desc/mod.rs:650-659`, `core/stats_ttest/mod.rs:623`,
  `core/stats_onesample/mod.rs:470`, `core/stats_anova/mod.rs:669`,
  `core/stats_corr/mod.rs:741` and `core/stats_alpha/mod.rs:621-630`.
- **A Save insists the file kept is the file the run read.** The spec must have
  one input (`app/project_commands.rs:829-836`), equal to the slot's bytes' hash
  and to the manifest's inputs (`app/project_commands.rs:939-947`).
- **The Rerun reloads the project's copy**
  (`app/project_commands.rs:1897-1899`) and validates the stored spec against it
  (`app/project_commands.rs:1902`).

**Where the prompt's premises did not match the code.** Each is carried into the
part it affects.

1. Decision 392d cites `research_project/mod.rs:119-120` for the report's columns
   by position. Those lines are the store's cap codes. The enum is in `render.rs`
   (`core/research_project/render.rs:120`).
2. Decision 392e cites `dataset.rs:805-807` for the 128-column refusal. It is at
   `core/stats_l1/dataset.rs:803-805`.
3. **A name "never in a result" does not hold if a score's name is its header.**
   The descriptive summary's result holds each column's header
   (`core/stats_desc/mod.rs:45-46`, `core/stats_desc/mod.rs:253`) and copies it
   from the schema (`core/stats_desc/mod.rs:747`). The other methods' results hold
   no header (`core/stats_alpha/mod.rs:77`, `core/stats_corr/mod.rs:82`; for the
   t-tests and the ANOVA, confirmed by a search for `header` in their modules, which
   finds none). Part 4 resolves this by never putting the name in the schema.
4. **Typing does not decide what is offered as a group.** Decision 392d says an
   integer sum may be typed Integer and offered as a group column. The screens
   offer any column by its count of distinct values, whatever its kind
   (`screens/TTestFlow.tsx:124-125`, `screens/AnovaFlow.tsx:123-124`), and neither
   the t-test nor the ANOVA calls `is_group` (confirmed by search of
   `core/stats_ttest/mod.rs` and `core/stats_anova/mod.rs`). So a mean score with
   few values would be offered too. Part 4 needs a flag, not a type.
5. **A method's screen closes its file when you leave it**
   (`screens/flowHooks.ts:28`, `screens/AlphaFlow.tsx:190`,
   `screens/CorrFlow.tsx:182`). No two screens share an open file today. A score
   made anywhere would vanish on leaving that screen, so Part 10 has to change
   this.
6. `screens/projects/storedShape.ts:403` and `screens/projects/SavedResult.tsx:65`
   do not strip `/^C/` as decision 392d lists them. The first checks the alias's
   form, the second writes `C` plus a position. Both still work for Part 4's
   choice.
7. **Blueprint §10.2 says to store derived datasets as separate versions.**
   Decision 391 keeps a recipe instead. The step prompt wins, and Part 6 says what
   that costs.

## Decision

### 1. The score (decision 392a) [Accepted, decision 397]

**What SPSS computes** (IBM SPSS Statistics 25 Command Syntax Reference, Chapter
2, Universals; read 2 Nov 2026; URLs under Evidence):

- `SUM` "Returns the sum of its arguments that have valid, nonmissing values."
- `MEAN` "Returns the arithmetic mean of its arguments that have valid,
  nonmissing values."
- `.n`: "MEAN.2(A,B,C,D) returns the mean of the valid values for variables A, B,
  C, and D only if at least two of the variables have valid values. The default
  for n is ... 1 for other statistical functions. If the number specified exceeds
  the number of arguments in the function, the result is system-missing."
- "The divisor for the calculation of the mean is the number of nonmissing
  values."
- Table 5: `MEAN.n` and `SUM.n` return system-missing if "Fewer than n arguments
  are valid; the default n is 1."
- `(var1+var2+var3)/3` is missing if any item is missing.

So `SUM.n` adds what is there and does not rescale. Whether SPSS 31 words this the
same way is **not confirmed**: its function pages rendered empty here. Only its
COMPUTE page could be read, and that page does not state the rule.

**A sum over fewer items is not on the same scale as a full one.** On ten items
scored 1 to 5, someone who answered eight scores at most 40 and someone who
answered ten scores at most 50. The lower sum can come from skipped items. A mean
does not have this problem in the same way.

**Alternatives:**

| rule | what it does | verdict |
|---|---|---|
| every item has a value | listwise for the row, as `(a+b+c)/3` | offered |
| at least n items have one, n from 1 to k − 1 | as `SUM.n`, `MEAN.n` | offered |
| SPSS's default, n = 1 | scores a person from one answer | not a default; the user may type 1 |
| a sum prorated up to k items | a different statistic from `SUM.n` | rejected: not asked for, and no oracle says it is what a thesis means by a total |

**Recommendation.**

- Two to 128 items, chosen by the user.
- The statistic is `sum` or `mean`, the rule is `every_item` or `at_least n`, and
  neither has a default. Until both are chosen, the settings need input, as the
  t-test's variant does (decision 64).
- `at_least k` is refused, so one choice has one form.
- A sum under `at_least n` is allowed. The screen and the method page state the
  sentence above, and Gaply does not refuse it, because the choice is the user's.
- A score that cannot be computed is an empty cell (`Cell::Missing`,
  `core/stats_l1/dataset.rs:549-553`), never a code. That covers too few items and
  a total that overflows.
- The make step reports four counts per score: rows scored from every item, from
  fewer, not scored for too few, and not scored for overflow. They are counts, with
  no values.
- An item must pass the checks alpha's items pass: a numeric column with no
  non-finite and no text cells (`core/stats_alpha/mod.rs:537-551`).
- A score's items are the file's columns. A score of scores is not in the first
  version: it would make the recipe a graph.

### 2. One formula, one place (decision 392b) [Accepted, decision 397]

**Reversal.** `reverse` is a private function (`core/stats_alpha/mod.rs:407-412`),
called at `core/stats_alpha/mod.rs:364` and in its test at
`core/stats_alpha/mod.rs:1063`. The ends' checks are at
`core/stats_alpha/mod.rs:518-528`. The outside-the-ends refusal counts numeric
cells, ticked codes removed (`core/stats_alpha/mod.rs:559-582`).

| way to share it | cost | verdict |
|---|---|---|
| make `reverse` and `ScaleEnds` `pub(crate)`, and extract the outside count into a `pub(crate)` function alpha also calls | visibility, one small extraction | **recommended** |
| move them to the new module, alpha imports them | moves code alpha's wire type depends on | rejected: Prompt 0 says wrap first, move later |
| write `min + max - x` again | one line | rejected: decision 62, a second copy |

Decision 352 set the precedent, when `RHO_ONE_ULPS` was made `pub(crate)`,
visibility only.

**The row total.** Alpha folds from 0.0 in column order
(`core/stats_alpha/mod.rs:762-765`). That expression sits inside a larger function.
**Recommended:** extract it to one `pub(crate)` function taking the row's values in
column order, and call it from both.

**Without changing a byte alpha writes.** Every pin stays unedited and must pass:
`alpha_r_golden`, `alpha_cases`, `alpha_export`'s golden files and the stored
fixtures. A mutation in the shared functions must turn alpha's tests red as well as
the score's, which shows the sharing is real.

**The mean.**

| way | for | against |
|---|---|---|
| `stats_verify::mean` (`core/stats_verify.rs:177-185`) | the one mean in the product, already what the methods use (decision 62, ADR-002 §1) and already `pub(crate)` | its second pass may move a value by a last-digit unit from total ÷ count *(inferred; not measured)* |
| the total ÷ the count, as IBM states the divisor and R's `rowMeans` is documented as equivalent to `apply` with `mean` | one short sentence on the method page | a second way to compute a mean, which decision 62 and Prompt 0 forbid |

**Recommended: `stats_verify::mean`** on the row's values in column order. Decision
368 feeds the methods' columns to it ascending. A row's items need no such sort,
since the spec fixes their order and alpha's totals use the same order. The gate
against R is measured (Part 13), and exact only where IEEE arithmetic makes it so.
The first building step counts how many fixture rows differ from total ÷ count, and
reports them. Question 3 asks you to confirm.

### 3. Missing codes (decision 392c) [Accepted, decision 397]

The methods take one ascending list for all their columns
(`core/stats_desc/mod.rs:180-189`, `core/stats_corr/mod.rs:262-274`,
`core/stats_alpha/mod.rs:226-236`, `screens/AlphaFlow.tsx:208`).

**One list per score, or one per item?**

| | for | against |
|---|---|---|
| one list per score | the same rule, wording and checks as every method; items of one scale share a range *(inferred)* | cannot say that 99 is a code in one item and a real value in another |
| one per item | matches a file where codes differ | a new shape no method has, more screen, more ways to mistype |

**Recommended: one ascending list per score, codes applying by value to its items.**
What a file declares as user-missing is already empty before this
(`core/stats_l1/dataset.rs:13-22`), so only undeclared codes reach this list. The
revision trigger covers a real case that needs one per item.

**A later method's ticked codes.** They apply by value to every chosen column
(`core/stats_alpha/mod.rs:333-341`). So a score of exactly 99 with 99 ticked for
another column would silently become missing. That is a real person dropped with
no message.

| option | cost | verdict |
|---|---|---|
| let it happen | none | rejected: a silent change to a result |
| each method skips codes on a score column | changes six modules' compute paths, and the summary's tally | rejected: touches the arithmetic of every method for a rare case |
| **refuse a spec whose ticked code equals a value in a chosen score column, naming the column and the count** | one shared check, one closed code per method | **recommended**: an ambiguous case asks the user, as Prompt 0 says |

**The suspect-code rule** (`core/stats_l1/binding.rs:645-669`, reported by the
profile at `core/stats_desc/mod.rs:518`). **Recommended: not applied to a score.**
Its codes were dealt with when it was made, so a score's `suspectMissingCodes` is
empty. Otherwise the rule, whose job is to flag a stray code in raw data, could
flag a genuine score.

### 4. The new column's identity (decision 392d) [Accepted, decision 397]

**Today.** An alias is a position (`core/stats_l1/dataset.rs:213-227`,
`core/stats_l1/dataset.rs:498-500`). The wire form is `C` and digits
(`core/stats_desc/mod.rs:124-147`). Diagnostics carry an integer
(`core/stats_desc/mod.rs:480-482`, `core/stats_alpha/mod.rs:442-444`). Order checks
compare aliases (`core/stats_alpha/mod.rs:447-460`). The report prints positions
(`core/research_project/render.rs:120`, `core/research_project/render.rs:393`,
`core/research_project/render.rs:463-464`,
`core/research_project/render.rs:516-518`). Seven screens strip `C`
(`screens/DescribeFlow.tsx:91`, `screens/TTestFlow.tsx:115`,
`screens/OneSampleFlow.tsx:93`, `screens/PairedFlow.tsx:93`,
`screens/AnovaFlow.tsx:119`, `screens/CorrFlow.tsx:123`,
`screens/AlphaFlow.tsx:111`).

| | `C(k+1)`, `C(k+2)`, … after the file's columns | a namespace, `S1`, `S2` |
|---|---|---|
| file's aliases | none shift | none shift |
| `ColumnAlias(u32)`, `index()` | unchanged | must carry a kind |
| wire parser | unchanged | changed, and every stored alias's check |
| diagnostics' integer column | unchanged | cannot say `S1` as an integer; every method changes |
| order checks | unchanged: scores sort last | need a rule for the two kinds |
| seven screens, `screens/projects/storedShape.ts:403` | unchanged | each changed |
| report | "Column 52" needs a line saying it is prepared | says so by itself |
| stability | removing a score renumbers the later ones | the same unless numbers are never reused |

**Recommended: `C(k+j)`, in the order the recipe lists them.** The one thing a
namespace gives for free, the report saying which columns are prepared, costs one
line in the report (Part 6). Renumbering is safe. A saved run names its own recipe,
which fixes what its aliases mean, and a changed recipe gives the screen a new input,
so its settings start again (Part 10).

**The name.**

- **It is never a schema header.** A score's `header` in the Dataset is its alias
  word, `C52`. So the descriptive summary's result, its CSV and every other path that
  copies a header carry `C52`, by construction (premise 3 above). This honours
  "never in a result, a manifest or a log" with no change to any method.
- **Where it lives in memory:** in the recipe, held in the slot beside the Dataset.
  `DatasetView` overlays it on the profile for the user's own screen. `core`'s
  `profile()` never sees it.
- **Where it lives on disk:** only inside the recipe artifact, as ADR-009 §3 requires
  (`docs/adr/ADR-009-projects-artifacts-provenance.md:124-127`).
- **The spec holds no name**, only the recipe's hash. So `SPEC_FIELDS`
  (`app/project_commands.rs:1962`, its test `app/project_commands.rs:2815`) gains no
  `Excluded` name, unlike what decision 392d anticipated. Part 6 lists what it gains.
- **Rules.** Typed by the user, with no default. 1 to 128 characters after trimming.
  Not equal to a file header or another score's name. The project title's rule on
  invisible format characters applies (decision 121). It is on decision 393's list.
- **A consequence to accept.** Renaming a score makes a new recipe with a new hash,
  though no number changes. A rerun repeats the saved recipe and is unaffected.

**Its type.** Level 1's typing (`core/stats_l1/dataset.rs:470-483`) would make an
integer sum Integer, and a score missing in every row Empty.

- **Wanted:** always `Numeric`. Never a group, never levels.
- **How:** the constructor of Part 5 sets `Numeric`, `distinct: None`,
  `levels: None`, and adds `has_missing` when a cell is empty. A method's numeric slot
  accepts it through the existing `kind_of` (`core/stats_desc/mod.rs:458-464`).
- **A flag, because the type is not enough (premise 4).** The Dataset records how many
  of its columns came from the file, in one new private field. A score reaches the
  screen as `prepared: true`, skipped when false so today's wire bytes stay. The
  t-test's and ANOVA's group lists filter it out, and Rust refuses it as a group,
  because the webview is untrusted.
- **A score no row can have is refused when it is made**, with its counts. An `Empty`
  column would otherwise appear, and every method would then refuse it one by one.

### 5. How a method sees it (decision 392e) [Accepted, decision 397]

**Which input does the spec name?** `compare_rerun` checks only the method's engine
version (`core/research_project/mod.rs:543-558`), and the manifest has one
(`core/contracts/mod.rs:643`). `InputRef` differs only by columns if the hash is kept
(`core/contracts/mod.rs:226-232`).

| option | methods' input check | Save's check | two recipes of one width |
|---|---|---|---|
| A. two inputs, the original's and the recipe's | **broken**: `InputCount` in all six modules | **broken**: `spec_input` needs one (`app/project_commands.rs:832-834`) | told apart |
| B. a fingerprint of the prepared table | **kept, unchanged** | **broken at one place**: the bytes' hash is not the input's | told apart |
| C. the original's hash, the recipe elsewhere | kept | kept | **look alike**: each method must add its own recipe check or a spec passes on the wrong table |

**Recommended: B, with the fingerprint over the recipe rather than the cells.**

```
prepared input hash = SHA-256 of
  "gaply.prepared.v1\n" + the original bytes' SHA-256 + "\n" + the recipe's SHA-256
```

- It goes through the existing `DataFingerprint::of_bytes`, passed those bytes. So
  `input_ref` needs no change, no method's check changes, and two recipes are two
  inputs.
- **Why not hash the prepared cells:** a cell hash depends on every float's bytes, and
  costs a pass over the table. It would make a platform's last-digit difference look
  like different data, and refuse a legitimate rerun. The recipe's hash states what was
  asked for, and the manifest's engine version states what did it.
- **One check changes:** `spec_input` and `data_of` compare the slot's bytes with the
  spec's named original when a preparation is present
  (`app/project_commands.rs:939-947`). The code still requires that what is kept is the
  file the run read. Tests must show each of the three comparisons still refuses.

**Where the recipe's hash and version travel.**

| where | cost | verdict |
|---|---|---|
| each method's parameters | six structs with `deny_unknown_fields`, six sets of `SPEC_FIELDS`, and the parameters' hash changes meaning | rejected |
| a new `InputRef` field | `input_ref` would need it from the Dataset, and the manifest's inputs are hashes only (`core/contracts/mod.rs:645`) | rejected |
| **an optional `preparation` field on `OperationSpec` and on `RunManifest`** | one type; 9 files build a spec and 13 build a manifest (confirmed by search), most of them tests | **recommended** |

`preparation` holds `{ sourceHash, recipeHash, sourceColumns, capabilityVersion,
engineVersion }`: hashes and numbers only. It is skipped when absent, so **every stored
spec, stored manifest, golden and wire fixture keeps its bytes**. The precedent is
`confirmation_refs` (`core/contracts/mod.rs:247-252`). `InputRef.columns` is k plus the
scores, as `input_ref` reports. The method's parameters and result do not change.

**How the table gets its column.**

| option | cost | verdict |
|---|---|---|
| append strings to the `RawTable` before `build_dataset` | the `RawTable` is not kept, so the file is parsed twice: once to read the items, again to build. 8F-1's open took 2.76 s with a 670 MiB peak at the 8 GB limit (ADR-010). Numbers go through text. Types as Integer, which Part 4 does not want | rejected |
| **a narrow `pub(crate)` constructor in `stats_l1`**, allowed by ADR-002's Consequences | one function and one private field; `build_dataset`, `type_column` and Level 1's goldens untouched | **recommended** |
| a wrapper type every method takes | six signatures, each method's `column` helper, `Slot` and the run manager's trait | rejected |
| a pure module of its own | does not replace the choices above: it needs one of them to make a Dataset | **also recommended**, for the recipe |

The summary's checks are kept honestly. A score cannot hold a non-finite or text cell,
since overflow is an empty cell (Part 1). So reading its summary
(`core/stats_alpha/mod.rs:537`, `core/stats_corr/mod.rs:672`,
`core/stats_desc/mod.rs:466-478`) is correct, not skipped. The constructor's unit tests
pin this. The constructor also refuses past `MAX_COLUMNS`, as `build_dataset` does
(`core/stats_l1/dataset.rs:803-805`).

**The module.** New `gaply_core::stats_prep`, pure, under `contracts_are_pure.rs`
(decision 343's pattern; Prompt 0: no new crate). It holds the recipe's types, validation, the build, the prepared
input hash and the export's text. It takes everything from the shared functions of Part
2. The app reaches it through `stats_adapters`, as it reaches every method.

**The recipe's shape** (proposed; canonical JSON, `deny_unknown_fields`):

```
{ "schemaVersion": 1, "capabilityId": "stats.prepare.scale_score",
  "capabilityVersion": 1, "engineVersion": 1, "sourceColumns": 50,
  "steps": [ { "kind": "scale_score", "alias": "C51", "name": "<typed>",
               "items": ["C3","C4","C7"], "reversed": ["C4"],
               "scale": {"min":1,"max":5}, "statistic": "mean",
               "rule": {"atLeast": 2}, "missingCodes": [-99] } ] }
```

**Room for what is not in the first version.** `steps` is an ordered list of tagged
kinds, each with its own rules, and a later kind is a new tag, a new capability and a new
engine version.

- **Recoding values** is a step that makes a new column and never overwrites the file's,
  so aliases stay stable.
- **Computing from a formula** is a step holding a closed expression tree of named
  operations, parsed by Gaply and never evaluated as code. AI does not write it.
- **Selecting cases** is a step that changes the rows. `InputRef.rows` already records
  that (`core/contracts/mod.rs:229`), and the prepared input hash already separates it.

None of them is designed further here.

### 6. The recipe's place in a project (decision 392f) [Accepted, decision 397]

| option | for | against | verdict |
|---|---|---|---|
| inside each saved run's spec | every run carries it | names in the spec, one copy per run, each method's `read_back` type changes | rejected |
| a project-level table of recipes | a list to show | a new table with a name in it, which ADR-009 §3 forbids in a row; a migration | rejected |
| **an artifact of its own, named by hash, linked from each run that used it** | content-addressed so identical recipes are one file; the spec holds only a hash; deleted with the project | new codes in two closed lists | **recommended** |

**What changes.**

- `ArtifactKind` gains `recipe` and `UsedRole` gains `recipe`
  (`core/research_project/mod.rs:418-430`, `core/research_project/mod.rs:466-469`).
  `admits` takes it as JSON (`core/research_project/mod.rs:453-461`). `artifacts_of`'s
  exhaustive match gains an arm (`app/project_commands.rs:1720-1723`).
- `project.db` needs **no SQL**: `kind` and `role` are unchecked text columns
  (`app/project_store.rs:122-168`). `SCHEMA_VERSION` stays 2
  (`core/research_project/mod.rs:45`, `app/project_store.rs:172-174`,
  `app/project_store.rs:178`).
- **The cost of that:** an older build reading a project that holds a recipe fails on
  the unknown code and reports `db_unreadable` (`app/project_store.rs:668`,
  `app/project_store.rs:2026`), not "newer version". Nothing is lost or changed, only
  the wording is wrong. A no-op version 3 would give the better word, but would lock every
  project an older build opens, recipe or not. **Recommended: no bump** (Question 8).
- **The `derived` table stays unused** (`app/project_commands.rs:923`). No prepared table
  is stored, so nothing is derived on disk. The export computes `wasDerivedFrom` from
  `used` and `wasGeneratedBy` (`core/research_project/graph.rs:225-230`), so a result is
  shown as derived from the data file, the spec and the recipe, with no new edge type.
- **Not storing the prepared table departs from blueprint §10.2.** What that costs: a
  saved run cannot be reread without recomputing, and a future change in the engine could
  change the column, which the engine version will show. What it buys: no second copy of
  user data, against a 100 MiB project cap (decision 107), and no stale copy.
- **`SPEC_FIELDS`** gains `preparation`, and `preparation.sourceHash`, `.recipeHash`,
  `.sourceColumns`, `.capabilityVersion`, `.engineVersion`, all `Reported`.
- **The report's parameter lines** gain, by position and never by name:
  `Data file as read: n rows, k columns; m prepared columns` in place of
  `InputShape`'s line when a preparation is present (`app/project_commands.rs:2020-2028`,
  since its `columns` is now k + m), and `Prepared column 51: mean of columns 3, 4 and 7;
  column 4 reversed on 1 to 5; at least 2 items; 1 missing code`.

**A recipe is saved with a run, not alone**, in the first version. There is no autosave
(decision 105). A Save reusing an existing artifact is already how the store works
(ADR-009 §5). The project page lists a project's recipes from the `used` edges. Using a saved
recipe on a new run, rather than a rerun, is a later step (Question 11).

### 7. A rerun (decision 392g) [Accepted, decision 397]

Between the reload (`app/project_commands.rs:1897-1899`) and `valid_on`
(`app/project_commands.rs:1902`):

1. Read the recipe artifact, its hash verified on read as every artifact's is.
2. Parse it. A capability version this build cannot run, or a recipe that fails
   validation against the data, refuses with a closed code and writes nothing.
3. Build the prepared Dataset from the reloaded one, by the same function as a live make,
   with the prepared input hash.
4. `valid_on` then compares it with the stored spec's input, unchanged. A changed recipe
   or data file cannot pass.

**The rest of a rerun is unchanged.** The one-run slot holds it
(`app/runs.rs:236`). The recipe's write is reused by hash, and the new bundle is the one
write at its end (decision 186).

**The same bytes.** A live run reads the slot's prepared Dataset. Applying the recipe
again gives the same cells, since the build uses only IEEE addition, subtraction and
division in a fixed order, with no call into the maths library. So the columns should be
the same bits across platforms *(inferred; the first step pins a hash of the columns,
and CI's Linux run tests it)*. The method's own numbers keep whatever platform differences
they already have (decision 346), and decision 366's facts about a rerun on another kind
of computer apply unchanged.

**The manifest.** `inputs` holds the prepared input hash, and `preparation` holds the rest
(Part 5). `compare_rerun` takes the preparation's engine version too. Today's second value,
whether the engines differ, then covers both, and the sentence says which. Old manifests
have no field and read as before.

### 8. The export of the prepared data (decision 392h) [Accepted, decision 397]

**A Dataset is not the file.** Its cells are typed
(`core/stats_l1/dataset.rs:549-553`), missing tokens and SPSS user-missing values are
already empty (`core/stats_l1/dataset.rs:13-22`), and value labels are gone.

| built from | holds | cost | verdict |
|---|---|---|---|
| the original bytes, columns appended | the user's exact text | works for a CSV only; Gaply has no `.xlsx`, `.sav` or `.mat` writer *(inferred from no such use in the ingest)* | rejected |
| the bytes parsed again to a `RawTable`, plus the scores | cells as parsed, tokens and user-missing numbers kept | a second parse at the memory limit, and a file whose codes the scores did not use | rejected for the first version |
| **the prepared Dataset** | exactly what was analysed | loses tokens, labels and original number spelling | **recommended** |

**Why the Dataset.** It is the table the scores were computed on, so the file answers the
question "what did Gaply use?". It needs no second parse, and opening it again gives the
same numbers, which can be tested.

**What the file holds, and the sentence that says so before the dialog.**

- A header row: the file's headers and the scores' typed names.
- Every row, the file's columns in order and then the scores.
- A number at all its digits (Rust's shortest round-trip form, as
  `core/stats_desc/export.rs:81`). So `1.50` reads `1.5`.
- An empty field for every missing cell, including what a file's token or SPSS's
  user-missing range made missing.
- Text as Gaply read it, at most 512 characters a cell (ADR-010). No value labels.
- CRLF line ends and quoted text (`core/stats_desc/export.rs:16`).

> "This file holds your data as Gaply read it, with your scores added. Missing answers are
> empty, SPSS's value labels are not included, and numbers are written in full. Your
> original file is not changed."

**Text and numbers.** Headers, names and text cells go through
`core/stats_desc/export.rs:47`'s `text()`, so a formula cannot run in a spreadsheet.
Numbers must never pass through it: its rule includes `-`
(`core/stats_desc/export.rs:32`), so a negative value would gain an apostrophe. A test pins
a negative score unquoted.

**The path.** ADR-004a's: Rust returns the bytes and their hash, and the screen writes them
after the dialog (`src/utils/saveTextFile.ts:15`, `screens/flowParts.tsx:253`). The suggested
name is fixed, `prepared_data.csv`, and carries no file name. Gaply keeps no copy. A cancelled
dialog writes nothing. At 100,000 × 51 it is a roughly 10 MB string through IPC *(inferred:
8I's 100,000 × 50 CSV is 10,000,200 bytes)*. The largest file a tier admits is the first
step's to measure, with a refusal if it fails.

### 9. Limits (decision 392i) [Accepted, decision 397]

| limit | value and place | checked on added columns today? | recommended |
|---|---|---|---|
| columns | 128 (`app/supplementary.rs:52`, `core/stats_l1/dataset.rs:41`) | only by `build_dataset` (`core/stats_l1/dataset.rs:803-805`) | the constructor refuses past it. A 128-column file cannot take a score |
| cells | by tier (`app/supplementary.rs:147-168`), 5,000,000 on 8 GB | **no**: only while parsing (`app/stats_l1_ingest.rs:428-432`) | the make step checks k + m against this computer's cap, with a closed refusal. A file at the cap cannot take a score |
| slot budget | cells held (`app/memory_tier.rs:90-106`), counted by `cells_of` (`app/runs.rs:394-396`) | yes, automatically: `cells_of` reads the schema's width | keep. A prepared slot replacing its original is one copy, not two |

**The refusal at the caps** is strict rather than an allowance, because the caps are what
was measured (ADR-010): an allowance would need its own measurement. Question 7 asks.

**A second copy.** Held beside the original, a prepared table could evict it or another
dataset (`app/runs.rs:352-358`). **Recommended: the prepared Dataset replaces the slot's
Dataset**, in the same slot number, moved to the back so it is the one never dropped, with
the source bytes kept. Removing a score rebuilds by dropping trailing columns, without a
parse. The peak while building is transient: the slot's cells cloned, then the old dropped,
about 206 MiB at 5,000,000 cells (ADR-010) on top of the slot, below the open's measured 670
MiB peak *(inferred)*. If measurement says otherwise, the private columns can be shared
behind `Arc`, invisibly to every reader of `column_cells`.

**A running run** holds its own handle on the old slot (`app/runs.rs:223-228`), so a change
cannot affect it *(inferred from `Arc<Slot>`)*. The make step is refused while a run is going,
as a second run is.

**Time to build, inferred, for the first building step to measure.** 8I's alpha at 128 items
and 100,000 rows took 4.74 to 4.80 s, nearly all of it 8,128 inter-item correlations, so about
0.6 ms a pair. One score over 128 items does work comparable to 128 pairs, about 0.1 s,
perhaps 0.3 s for the mean's second pass and building row slices. Adding the clone, the whole
step at the 8 GB cap should take about one second or less. So **no run manager, no budget and
no check points for it**, unless it measures over 1 s, in which case it moves to the run
manager with a progress line (revision trigger). `stats_desc::profile` recomputes every column
(`core/stats_desc/mod.rs:489-528`), so the view adds a profile for the new columns only, and
tests that it equals a full profile.

### 10. The screen (decision 392j) [Replaced by ADR-012, decision 447]

| where | for | against | verdict |
|---|---|---|---|
| a step inside each method's screen | no new route | seven copies of one step in use; the score vanishes on leaving (premise 5), so alpha then correlations means making it twice | rejected |
| **a page of its own, "Prepare data", from the Statistics home** | one place; the file stays open across methods; the export belongs here | one change to how the seven screens open a file | **recommended** |

**The page** (`/app/statistics/prepare`, a new route beside
`screens/statisticsRoute.tsx`): opening a file through the same open and limits sentence, the
profile (`screens/flowParts.tsx:132`), a list of scores, each made by a form with nothing filled
in. Items as unticked boxes. Reverse-score boxes unticked. The scale's fields empty and shown only
when an item is reversed, with decision 384's sentence and never taken from the data, styled as AlphaFlow's after 8I-2's fix (decision 388). Sum or mean unchosen.
Missing rule unchosen, n empty. Codes unticked. Name empty. Then the four counts, and the sum's
sentence where it applies. No verdict word, threshold or advice. Alpha's own screen offers no
shortcut to this form in the first version, since prefilling would be choosing for the user.

**How a method sees the new columns.** Each screen's import stage gains one choice, shown only
while a prepared file is open: "Use the prepared data (file name, m scores)" or the existing open.
Choosing it hands the screen that slot's `DatasetView`, so its lists show the score by name,
through the code they already run (`screens/AlphaFlow.tsx:213`), with no per-method change to the
lists except the group filter of Part 4.

**The change to closing (premise 5).** A method screen closes only a slot it opened
(`screens/flowHooks.ts:28`). The prepared slot belongs to the Statistics session, one at a time,
held in a provider above the routes. It closes when the user closes it on the Prepare page, or
when leaving Statistics drops the engine and its slots, as today
(`docs/adr/ADR-001-one-active-workspace.md:51-52`). An evicted slot gives
the existing refusal.

**Results and saved results.** A live result names a score through the view, as any column. A
saved result reads the recipe from the project and shows `Column 51 (name)`, its fallback the
position alone (`screens/projects/SavedResult.tsx:65`). `storedShape.ts` learns the optional
`preparation`, keeping its exact field checks.

**The Statistics home** gives a card to a runnable capability by the route table
(`screens/StatisticsHome.tsx:38-42`), read from `METHODS`. This capability has no run kind
(`app/project_commands.rs:3352`), so it takes one entry beside the table, not a row in it.

### 11. Privacy (decision 392k) [Accepted, decision 397]

- **Names and values never leave the device.** `stats_prep` is pure and has no route out, under
  `contracts_are_pure.rs`. `stats_prep_commands.rs` joins `stats_desc_writes_nothing.rs`'s list. The
  store remains the one writer.
- **Level 1 and PublishReady never see them.** `SupplementaryEvidence`, what PublishReady's
  reviewer is sent, is built by its own ingest and never from a slot, and decision 365's test
  `the_publishready_payload_does_not_carry_the_level_1_fields` stays. Level 1 opens through its own run store
  (`app/state.rs:73`), never a Statistics slot: a search for `DatasetSlots` and `Slot`
  finds them in neither `stats_l1_commands.rs` nor any PublishReady file. The one outbound schema call is
  `core/stats_assist/outbound.rs:311`, over Level 1's own schema. A guard pins that nothing in
  `stats_assist`, `stats_l1`'s commands or the PublishReady path names `stats_prep`, a prepared
  Dataset or a recipe.
- **AI never proposes** items, reversal, a rule, n or a name. There is no AI task for it, and none
  may be added without its own ADR. A recipe type joins the scan of
  `project_store_guards.rs` (ADR-009 §16).
- **Not `Debug` with content.** The recipe's `Debug` prints counts, as `Cell` and `ColumnSummary`
  do (`core/stats_l1/dataset.rs:556-564`).
- **A log** holds a closed code and counts, never a name, a path or a value. The tracing guards scan
  the new files.
- **A hash of the recipe** appears in the manifest, and the recipe holds typed names. It stays on the
  device or in the export the user saves, as the data file's hash already does (ADR-004). It reaches
  no outbound struct.
- **The privacy matrix.** A name sentinel, an item value sentinel and a score value sentinel. Forbidden
  in the spec, every result, manifest and result CSV, every refusal, every log line and `project.db`.
  Allowed in the DatasetView, the recipe artifact and the prepared CSV only. The negative control is a
  score constant at the sentinel, which the mean in the summary's result must show: the search can find
  what it seeks.

### 12. The registry (decision 392l) [Accepted, decision 397]

| | a capability of its own | no capability, a helper of the slots |
|---|---|---|
| version recorded in recipes | yes | an invented field |
| lifecycle: Planned while being built, Beta when usable | yes, and `require_runnable` gates the commands | a flag, which Prompt 0 says is no boundary |
| wording, resource profile | the existing shape | new |
| run manager, `METHODS` row | not needed: no run | not needed |

**Recommended: `stats.prepare.scale_score`, version 1, engine version 1.** The id fits the grammar
(`core/contracts/mod.rs:110-121`). It joins the table (`core/stats_registry/mod.rs:207`) as Planned,
and its list pin changes. It turns Beta at the start of the step that first lets a user reach it, red
first, as the methods did (decision 377). Its resource profile says 100,000 rows, 128 columns, no ceiling
and no cancel until measured otherwise. Rust's rule that each `METHODS` row has a run kind stands, because
it has no row (`app/project_commands.rs:3332-3354`).

### 13. Validation (decision 392m) [Accepted, decision 397]

**The oracle.** Base R through `/usr/local/bin/Rscript`, in the fixtures' step, with no package, under
ADR-008. R computes on what it read: each file is written, read back with `read.csv`, and `cells_bits.csv`
records the bits, as ADR-008 §3 requires. A case where Gaply refuses and R computes is an expected refusal
with its closed code (ADR-008 §4). `rowSums` and `rowMeans` on a matrix after reversal and codes, with the rule applied by a count of
`!is.na`:

```
valid <- rowSums(!is.na(X))
score <- ifelse(valid >= n, rowMeans(X, na.rm = TRUE), NA)
```

R's page says `rowSums` gives 0 and `rowMeans` gives `NaN` on an empty row, which is why the rule is applied
first. The header records R's version, architecture and `.Machine$sizeof.longdouble`, since R may accumulate in a
longer type than Gaply's *(inferred; its page does not say)*.

**The fixtures** (seeded, synthetic, a `make_prep_fixtures.R` beside them):

- Sum and mean, every rule; integer and fractional items; items with reversal on 1 to 5, 0 to 10 and fractional or
  negative ends; a value at each end.
- Tokens and ticked codes; a code outside the ends; a reversed value outside them (refused).
- n at 1 and at k − 1; a row with exactly n − 1 and exactly n values; a row with none.
- 2 items and 128 items; 100,000 rows; a value near overflow.
- About 15 cases and a few thousand values *(inferred from 8I's 17 cases)*.

**The gates** (ADR-008 §6, decisions 341 and 277): per statistic, justified from its conditioning on the
method page, measured first with a margin of ten, asserted on every platform, and never loosened to pass a case.

- Which rows have a score, and the four counts: exact.
- A sum of integer items: exact, since no rounding occurs *(inferred; a test asserts it)*.
- A sum of other values, and a mean: relative to the row's rounding scale, k · ε · M with M the row's sum of
  magnitudes, the bound alpha's totals already use (`core/stats_alpha/mod.rs:60-62`). The 4 ulps of decision 346 is not
  this gate: it is the cross-platform layout check, and it fails a well-computed value after cancellation.
- The test prints the largest ratio to its gate and the case (ADR-008 §7).

**Tests with no oracle**, by proptest on a binary grid so a transformed input is exact (ADR-008 §5).
Reversing twice gives the value back. The sum of reversed and unreversed adds to
k(min + max) on complete rows. Item order is fixed by the column order. Descriptive summary of a score equals R's
on that column. Alpha's scale mean if deleted cross-checks against the summary's mean of a sum score over the other
items, for a complete table. Reopening the exported CSV gives the same numeric cells and, remade with the same recipe, the same
scores.

**The R gate and the guards are in the first step**, so nothing a user sees is built before the arithmetic is known
right.

### 14. The implementation plan (decision 392n) [Accepted, decision 397]

Each step is its own commit, with `cargo test --workspace`, and the frontend's commands where it touches them.
Letters are Rishi's to give.

**Step: the pure core.** `stats_prep` and its recipe, validation, build and prepared input hash; the visibility and
extractions in `stats_alpha`; the narrow constructor in `stats_l1`; the capability registered Planned.
- *Registration:* the pinned files decision 372 lists: the capability counts, the `Kind` match arms, the list's PIN,
  `workspaceList.json`, `capabilityCodes.json`, `capabilityWording.ts` and the home's roadmap test, with a stub method page
  and limitation codes.
- *Tests:* the R golden; the metamorphic tests; every refusal; the purity guard; alpha's and Level 1's pins unedited;
  a pinned hash of prepared columns; a release measurement of time and peak footprint at each tier's limit.
- *Mutations:* the shared reversal, the shared total, the shared outside count (each must redden alpha too); the n
  comparison off by one; a code not applied; overflow left infinite; a name reaching `header`; the type falling back to
  Integer; the column cap and cell cap removed; the hash over the original alone.
- *Real-app check:* none: no screen changes, and the report says so.

**Step: the app and the contract.** The optional `preparation` on the spec and the manifest; the six method modules
carrying them to their manifests; the group refusal and the code-equals-score refusal; the suspect rule's exemption;
`DatasetSlots`' replace; `stats_prep_*` commands behind the workspace check; the CSV export's bytes. In memory only.
- *Tests:* every existing stored fixture and wire fixture byte-identical; all seven methods through IPC on a prepared
  slot; a spec with a forged preparation refused; two recipes of one width refused for each other; the privacy
  matrix.
- *Mutations:* the field written when absent; the group refusal removed; the collision check removed; a replace that
  evicts its own slot; the profile's flag lost; numbers sent through `text()`; a method's manifest dropping the field.
- *Real-app check:* none unless a method's screen changes; the commands have no caller yet.

**Step: projects.** Blocked until this ADR is approved. The recipe artifact, role and kind; the Save; `spec_input`
and `data_of`; the rerun; `compare_rerun`; `SPEC_FIELDS`; the report and PROV lines; `storedShape.ts`.
- *Tests:* a saved prepared run reruns `Identical`; stored fixtures with a recipe; the rerun privacy matrix; a
  damaged or altered recipe refused with nothing written; a project holding a recipe through an older version's
  refusal, pinned as the question decides; delete removes it; an old project unchanged.
- *Mutations:* the rerun skipping the recipe; the Save comparing the prepared hash with the bytes; each of `data_of`'s
  three comparisons; a name in a row; `used` omitting the recipe.
- *Real-app check:* none until the screen exists.

**Step: the screens.** The capability turns Beta, red first. Prepare data; the session's slot; each method's import
choice and close rule; the group filters; saved results; the export card and its sentence; the home's card; the method
page, `large_samples.md`'s lines, a D-entry.
- *Tests:* vitest for no default anywhere; one spec for one choice; 128 items rendered, timed; each method on a
  prepared slot; a closed dialog writing nothing; a reopened project naming a score.
- *Mutations:* a default for each of item, reversal, statistic, rule, n, code and name; a method closing the shared slot;
  the sum's sentence hidden; a score listed as a group; a verdict word.
- *Real-app check:* yes, a checklist under `~/gaply-corpus/os/`, its CSVs written by a script of its own with no `#` line
  (decision 385), for Rishi: make a sum and a mean on a seeded CSV, compare
  each with a value the checklist gives from R, use a score in alpha, correlations and a t-test, save, reopen, rerun,
  export, reopen the CSV, delete the project, and the limits at 100,000 × 50.

## The disk list (decision 393) [Accepted, decision 397]

**Every operation the design adds. All are blocked until this ADR is approved.** Nothing in the list exists now.

| operation | writes | where | protection, deletion | blocked? | save dialog? |
|---|---|---|---|---|---|
| Save a run that used prepared data | **the recipe**: the scores' typed **names**, items and reversals by position, the scale's ends, statistic, rule, n, codes | an artifact in `projects/<id>/artifacts/`, kind `recipe` | ADR-009 §10, 0400; deleted with the project (§11); counted in the 100 MiB cap (§9) | **blocked** | no: a Save click, with the save sentence amended to say the steps and names are kept |
| the same Save | the run's edges: `used` with role `recipe` | a row of `used` in `project.db`: a hash and a code, no name | as ADR-009 §3 | **blocked** | no |
| the same Save | the spec and manifest, now with `preparation`: hashes and numbers | existing artifact kinds | as today | **blocked** | no |
| the same Save | the bundle: the method's result, with a score named by its alias | existing artifact kind | as today | **blocked** | no |
| Rerun of a prepared run | one write at its end, as today (decision 186): the new bundle and manifest; the recipe is reused by its hash | existing | as today | **blocked** | no: a Run again click |
| Export the prepared data | **every row and the new columns' names and values**, as Part 8 lists | only the file the user names | the user's own. Listed by ADR-009 §11's confirmation as beyond Gaply's reach | **blocked** | **yes** |
| Export the project report, and the PROV JSON, with new lines | positions, hashes, codes, no name | only the file the user names | the user's own | **blocked** | **yes** (existing path) |

**Writes it reuses unchanged:** the project's copy of the data file (decision 101), the store's atomic write (ADR-009
§5), recovery, the delete and the save-dialog path (ADR-004a).

**Never written:** the prepared table's cells. They are in memory, in the slot, for as long as it is held. Making a score
writes nothing. No score, recipe or name is written outside a Save or an export (decision 105).

## Alternatives considered

- **Changing the user's file, or a copy of it.** Rejected by decision 391.
- **Storing the prepared table as a data artifact** (blueprint §10.2). Rejected by decision 391, and costed in Part 6.
- **A generic transformation engine now.** Rejected: Prompt 0 asks for one narrow slice. Part 5 says how the list extends.
- **Driving R or SPSS to make the score.** Rejected: Prompt 0 forbids it. R is the test oracle only.
- **Prorating, imputing, or suggesting a rule.** Rejected: the user chooses, and each is another statistic.
- **Making a score from alpha's screen with its settings carried over.** Not in the first version: it would fill the form
  for the user. Question 10.
- **Each choice's own alternatives** are in Parts 1 to 12.

## Consequences

- A thesis can go from alpha to a score to a method inside Gaply, and the project shows how the score was made.
- The six method modules change by one line each in their manifests and one check in their validation, and two (the
  t-test's and the ANOVA's) by a group refusal. Their arithmetic does not change, and their pins prove it.
- `stats_l1` gains one `pub(crate)` constructor and one private field. Level 1's rules and goldens do not change.
- Two new closed codes in the project's lists mean an older build refuses a project with a recipe, under a word that is
  not quite right.
- A saved result depends on recomputation for its column, not a stored copy. The manifest's engine version says when that
  computation changed.
- A renamed score is a new recipe. A file at a cap cannot take a score. A 99 a user ticks can refuse a run a score
  happens to collide with: each is a visible refusal, not a silent change.
- The seven method screens gain one import choice and a rule about closing, which is the largest change to shared screen
  behaviour since `flowHooks.ts`.
- Exported data is not the original file plus columns, and the screen has to keep saying so.
- ADR-004's operations table and ADR-009's text would each need a line when this is accepted. They are not changed now.

## Evidence

**Read from the code at d5fd559:** every file:line above, each printed from `git show d5fd559:<path>` and compared by eye with
what the text says. The log is `~/gaply-corpus/os/phase8/8j/citations.txt`.

**Not measured:** nothing here ran. Every time, memory and bit-identity figure is marked *(inferred)*, with what it was inferred
from. ADR-010's figures and 8I's are measurements from those phases.

**Documentation, each read 2 Nov 2026:**

| URL | what it gave |
|---|---|
| https://www.ibm.com/docs/en/SSLVMB_25.0.0/pdf/en/IBM_SPSS_Statistics_Command_Syntax_Reference.pdf | every SPSS quotation in Part 1: Chapter 2, Universals, "Statistical functions", "Missing values" and Table 5 (p. 96). Version 25; its text says it applies "until otherwise indicated in new editions" |
| https://www.ibm.com/docs/en/spss-statistics/31.0.0?topic=reference-compute | COMPUTE "creates new numeric variables or modifies the values of existing string or numeric variables". No rule on missing values on this page |
| https://stat.ethz.ch/R-manual/R-devel/library/base/html/colSums.html | `na.rm`; per-row omission; empty range gives 0 or `NaN`; equivalence with `apply` |

**Could not be read:** these pages returned only a title, so SPSS 31's wording on `SUM`, `MEAN` and `.n` is **not confirmed**:
https://www.ibm.com/docs/en/spss-statistics/31.0.0?topic=functions-statistical-functions,
https://www.ibm.com/docs/en/spss-statistics/31.0.0?topic=reference-transformation-expressions and
https://www.ibm.com/docs/en/spss-statistics/31.0.0?topic=values-missing-in-numeric-functions (each tried 2 Nov 2026; the last URL
was a guess at the page's address). How SPSS counts a user-missing value in `SUM.n` is taken from "valid, nonmissing" and was not
separately read.

## Open questions

1. **Which missing rules?** Recommended: `every item` and `at least n`, n from 1 to k − 1, with no default.
2. **Allow a sum under `at least n`?** Recommended: yes, as SPSS's `SUM.n` does, with the sentence that it is not on the same
   scale as a full sum. The alternative is mean only for partial rows, which removes a choice decision 392a gives you.
3. **The mean: `stats_verify::mean`, or total ÷ count?** Recommended: `stats_verify::mean`, the one mean in the product. It may differ
   from total ÷ count in the last digit, and the first step reports how often.
4. **One code list per score, or one per item?** Recommended: one per score, until a real file needs more.
5. **A ticked code equal to a real score value?** Recommended: refuse, naming the column and the count.
6. **A score as a group column?** Recommended: never, in the list or in Rust. To compare by a banded score, recoding comes later.
7. **At the caps?** Recommended: refuse, with no allowance. A file at 128 columns, or at the 5,000,000 cell cap on an 8 GB computer,
   cannot take a score until its columns are fewer. An allowance needs its own measurement.
8. **`project.db`'s version?** Recommended: stays 2, with no SQL. An older build refuses a project with a recipe as unreadable, which
   loses nothing. A bump would lock every project an older build opens.
9. **What the CSV is built from?** Recommended: the prepared Dataset, with the sentence of Part 8. If you want tokens and SPSS's
   codes kept as written, say so, and it becomes a second parse, to be measured.
10. **A shortcut from alpha's screen to a score with the same items?** Recommended: not in the first version. It would prefill choices.
    Asked again after the real-app check.
11. **Reusing a saved recipe on a new run?** Recommended: not in the first version. A recipe is saved with a run and used by its rerun.
    The next data-preparation step can offer "apply this recipe" on a new file, whose columns then have to be matched by the user.
12. **[Replaced by ADR-012, decision 447]** **Where the page sits?** Recommended: its own page, Prepare data, with one prepared file open at a time for the session.
13. **Should a score's items be limited to the file's columns?** Recommended: yes. A score of scores makes the recipe a graph.

## Revision trigger

The first building step's measurement over 1 s or over the open's peak; a real file needing a code list per item; a platform on
which the prepared columns' hash differs; a rerun that differs for a reason the manifest does not state; IBM's current documentation
read and found to differ from version 25's; or the first of recoding, formulas or selecting cases being asked for.
