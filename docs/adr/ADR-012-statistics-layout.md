# ADR-012: The Statistics layout

**Status: Proposed.** Not accepted until Rishi approves it.
Written 3 Nov 2026, Research OS Phase 8N (decisions 439 to 445), from the code at
6a4a3a41. Nothing in it is built. No code, test or fixture changed in writing it.
Once accepted, it replaces ADR-011's Part 10 and question 12. ADR-011 itself is not
edited.

## In plain words

**What you will see.** One Statistics window that works the way SPSS users know, in
Gaply's own style. The whole file is a grid you can scroll. A second tab lists the
columns. A third tab holds your results, in order, with a numbered list beside them.
Every analysis starts from a menu and opens as a dialog over the data. Nothing in a
dialog is chosen for you. When you press Run, the dialog closes and the result joins
the others.

```
+------------------------------------------------------------------------------+
| Gaply  Statistics | File  Transform  Analyze  Project  Help | Project: none open |
+------------------------------------------------------------------------------+
| [ Data View ]  [ Variable View ]  [ Output (3) ]                             |
+------------------------------------------------------------------------------+
| survey.csv: 250 rows, 11 columns. 1 prepared column.                         |
| As Gaply read it: numbers in full, missing cells empty, no value labels.     |
+------+-----------+-----------+-----------+-----------------------------------+
|      | C1  age   | C2  q1    | C3  q2    | C11  total (prepared)             |
+------+-----------+-----------+-----------+-----------------------------------+
|    1 |        41 |         4 |         2 |                                21 |
|    2 |        35 |           |         5 |                                   |
|    3 |        52 |         3 |         3 |                                18 |
|  ... | only what is on screen is drawn, down to row 100,000                  |
+------+-----------------------------------------------------------------------+
| Statistics workspace | Read-only grid: your file is never changed |            |
|                      | Everything stays on this computer                     |
+------------------------------------------------------------------------------+

The Output tab:
+----------------------+-------------------------------------------------------+
| 1 Cronbach's alpha   | 2 Independent-samples t-test                          |
| 2 Independent-sample | Settings: outcome C11, group C5, Welch, no codes      |
| 3 Correlations       | [ the method's tables, as today ]                     |
|                      | [ Save to project... ]  [ Export CSV... ]             |
+----------------------+-------------------------------------------------------+

A dialog, over the data:
+------------------------------------------------------------+
| Reliability (Cronbach's alpha)                             |
| Numeric columns       | Items, in the file's order         |
|   C2 q1               | None yet. Nothing is chosen for    |
|   C3 q2               | you.                               |
|   C4 q3               |                                    |
| Are these codes for a missing answer?                      |
|                                         [ Cancel ] [ Run ] |
+------------------------------------------------------------+
```

**What it adds to your computer's disk.** Nothing new. The grid and the results are
in memory. Saving to a project and exporting work as they do now. One new button,
Run every saved result again, repeats a save you already approved.

**What the design takes away.** The separate pages for each method and the Statistics
home, once the new window has passed your checks. Nothing is removed before that.

**What the code showed that the mockup could not.**

- Gaply remembers only your 8 most recent runs. A result older than that could still be
  on screen but could not be saved or exported. The design raises that limit.
- A result can be saved only while its file is open. So closing a file has to warn you.
- Making a second score today stops you saving a result that used the first score. I
  recommend fixing that when the score dialog is built.
- The mockup's menus have no way to reach the Stats Check, the Stats Verifier, Settings
  or Switch workspace. I recommend putting them under "Gaply" in the top bar.
- Nothing on the "Coming later" lists exists in Gaply's capability list. Each would be a
  plain label.

**What you are asked to approve.**

1. The design below, built in the seven steps of Part 13, one at a time.
2. Showing your rows on your own screen. This is the first screen that does. The cells
   go to the window and nowhere else.
3. Keeping more runs in memory than decision 6 allows, from 8 to 40, if the measurement
   in the Output's step agrees.
4. The small additions to the mockup, each listed in the table below.

**Each open choice, with my recommendation.** The numbered questions near the end say the
same in more detail.

| choice | recommendation |
|---|---|
| Write the grid ourselves or use a library | Ourselves, about the size of one screen's code. A library only if our grid fails its speed check |
| Copying cells out of the grid | Not in the first version |
| Opening another file when one is open | Say what happens first. Your results stay on screen and can still be exported, but those not saved can no longer be saved |
| Closing a file | The same sentence |
| Leaving Statistics | Say first that the results and prepared columns will be gone. Then they are |
| How many results the Output holds | 32. At 32 Gaply asks you to remove one, and never removes one by itself |
| The Run button when a method has a warning | The dialog stays open and shows it. A second press runs it |
| "Tick every numeric column" in the dialogs | Keep it, as an action you choose, because 128 clicks is too many |
| Run every saved result again | In the first version, one line for each, with a Stop button |
| Where saved results show | On the project page, as today. The Output is only for this session |
| Listing "Coming later" items before they exist | Yes, as plain labels with no date, as you chose |
| A line saying how the grid reads your data | Yes, always there. An SPSS user would otherwise think a 99 was lost |
| Where a whole long cell can be read | A line under the grid, for the cell you are on |
| A second score and earlier results | Keep each run's own recipe with it, so those results can still be saved |

## Context

**Paths.** In this ADR `core/` is `src-tauri/gaply-core/src/`, `app/` is `src-tauri/src/`,
`tests/` is `src-tauri/tests/`, `screens/` is `src/screens/statistics/`, and `ws/` is
`src/screens/workspace/`. Every file:line is as at 6a4a3a41 and is confirmed: each was
read in the file at that commit. A statement about the code with no file:line is marked
*(inferred)*. Every time, memory size and frame rate not attributed to a named
measurement is *(inferred)*, and the build steps measure it. Every name given to something
not yet built is a proposal.

Decision 398 chose the layout. Decision 441 says what the accepted mockup shows, region by
region. This ADR says how that sits on the code.

### What the code does today

**The routes.** `statisticsRoutes` gives the home and six method routes while the
`statsDescribe` flag is on (`screens/statisticsRoute.tsx:34-53`, the flag at `:43`). The
summary's route is separate (`screens/describeRoute.tsx:13-15`). The app mounts them at
`src/App.tsx:683-693`. The home has no workspace gate (`src/screens/GaplyScreens.tsx:241-244`),
and each method route has `RequireWorkspace` (`src/screens/GaplyScreens.tsx:227-232`).
`METHODS` holds one row per method (`screens/methods.ts:14-69`).

**The home.** Its cards come from Rust's list, one for anything runnable
(`screens/StatisticsHome.tsx:62-71`, `:87-119`). The Stats Verifier and the Stats Check have
cards of their own (`screens/StatisticsHome.tsx:123-159`, `:209-233`). The roadmap lists
what is Planned (`:238-250`), retired methods follow (`:251-263`), and the Projects panel
is last (`:269`).

**How a method opens a file, and closes it.** Each screen holds its slot in a ref and calls
`fileActions` (`screens/flowHooks.ts:142-172`), which closes the old slot only after the new
one has opened (`screens/flowHooks.ts:160-162`). A screen closes its slot when it unmounts
(`screens/flowHooks.ts:28-38`), called at `screens/TTestFlow.tsx:196`,
`screens/OneSampleFlow.tsx:148`, `screens/PairedFlow.tsx:147`, `screens/AnovaFlow.tsx:186`,
`screens/CorrFlow.tsx:182` and `screens/AlphaFlow.tsx:190`. The summary has its own two copies
of the same behaviour (`screens/DescribeFlow.tsx:299-304`, `:353-354`). So no two screens
share a file today.

**What crosses to the webview.** `DatasetView` carries no row
(`app/stats_desc_commands.rs:104-129`). No command returns a window of cells. A search for
`column_cells` in `src-tauri/src/*.rs` finds one use, in the reviewer's ingest
(`app/stats_l1_ingest.rs:1134`). The only command that returns a table's cells to the
webview is the prepared CSV, whole (`app/stats_prep_commands.rs:227`), which no screen calls
yet. Level labels do cross, in the profile (`core/stats_desc/mod.rs:263-280`).

**The window.** 1440 × 900 by default, 1024 × 700 at least, resizable
(`src-tauri/tauri.conf.json:17-21`). Today's screens sit in `AppShell` inside a div of
`height: 100vh; overflow: auto` (`ws/WorkspaceShell.tsx:339-340`,
`screens/flowParts.tsx:370-371`), with a 216 px rail (`src/design-system/shell.css:21`). That
leaves 808 px at the minimum and two nested scrollers. The new window does not sit inside it.

**Projects from a method's screen.** `SaveToProject` takes a run id and shows itself only when
`projectsShown()` says so (`screens/projects/SaveToProject.tsx:56-57`,
`screens/projects/projectGate.ts:13-15`). Its dialog is `ProjectDialog`, which keeps the focus
inside and returns it (`screens/projects/ProjectDialog.tsx:7-13`). The design system's `Modal`
does neither (`src/design-system/Modal.tsx:13-19`). Exports reach `saveExactly`
(`screens/flowHooks.ts:178-192`) and `saveTextFile` (`src/utils/saveTextFile.ts:15-27`).

### Where decisions 441 and 442 did not match the code

Each goes on into the part it affects.

1. **The Output cannot outlive 8 runs today.** `MAX_RUNS` is 8 (`app/runs.rs:136`), and a ninth
   record drops the oldest finished one whatever its kind (`app/runs.rs:1857-1865`). A Save reads
   the record (`app/project_commands.rs:1562-1566`), and an export does too
   (`app/stats_desc_commands.rs:426-427`). So a ninth result takes Save and Export away from the
   first, with a rerun's record counting among the 8. Decision 442g's "every result added in order"
   needs Part 7's change.
2. **A Save needs the slot the run used.** `save_run` looks the run's slot up by number and refuses
   `data_unavailable` if it is gone (`app/project_commands.rs:1609-1612`), and a run keeps only that
   number and a hash (`app/runs.rs:660-663`). Opening the same file again makes a new slot, so it
   does not help *(inferred from `app/runs.rs:437`, which takes the next number)*. So a result from
   a closed file can be exported but not saved.
3. **A second score blocks saving results on the first.** A prepared run's Save compares the slot's
   recipe with the run's (`app/project_commands.rs:951`, decision 433), and a recipe holds every score,
   so adding a score is a new recipe. Part 8 proposes a change.
4. **No "Coming later" item has a registry entry.** The capability list is pinned with no Planned
   entry at all (`src-tauri/gaply-core/tests/capability_registry.rs:41-59`), so the home's roadmap
   list is empty today. `stats.verify.chi_square_independence` and `stats.verify.ols` are the verifier's
   reproductions of a paper's result, not analyses of a user's file. See Part 5.
5. **The menus reach neither the Stats Check nor the Stats Verifier**, nor Settings, nor Switch workspace.
   The home and the shell reach them (`screens/StatisticsHome.tsx:209-236`, `ws/WorkspaceShell.tsx:74-75`,
   `:346`, `:354-356`). Part 1 places them.
6. **Decision 441's menus have no place for** a project's delete, its PROV export or Close project. These
   stay on the Projects panel and the project page (`screens/projects/ProjectPage.tsx:686`). See Part 9.
7. **Today a method has two steps before it runs:** Check the settings, then Run
   (`screens/AlphaFlow.tsx:471`, `:488-494`). A warning is shown between them. The mockup has only Run. See
   Part 6.
8. **Today's alpha offers "Tick every numeric column" and "Untick all"** (decision 384,
   `screens/AlphaFlow.tsx:378-390`). The mockup adds items one by one. See Part 6.
9. **A dialog's Run needs a running state.** A run can take seconds and can be cancelled
   (`screens/flowParts.tsx:190-248`). If the dialog closes at once, its entry in the Output has to show that.
   See Parts 6 and 7.
10. **The screens do not yet know a score's name or Part 4 of ADR-011's group filter.** A search for `scoreName`
    in `screens/` finds only the types and their fixtures. The t-test's and ANOVA's group lists offer any column
    by its count of values (`screens/TTestFlow.tsx:124-125`, `screens/AnovaFlow.tsx:123-124`), though Rust refuses
    a score as a group (decision 425). A score's `header` is its alias word (`core/stats_l1/dataset.rs:665`), so
    today's `c.header` would label it `C52`. Part 6 includes both.
11. **"50 columns at 8 GB" is the widest file at 100,000 rows, not the widest file.** The 8 GB tier also opens 128
    columns (`app/supplementary.rs:151-158`), up to 39,062 rows *(inferred: 5,000,000 ÷ 128)*. So the grid
    handles 128 columns on every tier.
12. **Decision 442b's "leaving Statistics" is two things today.** Switch workspace ends the workspace
    (`ws/WorkspaceShell.tsx:278-285`). Going Home with the rail leaves the route and keeps the workspace active
    (`screens/flowParts.tsx:375`). Part 2 says what each does.
13. **The frame rate cannot be measured by a cargo test or a vitest, and I may not start the real app.** Part 3's
    frame budget is therefore read by Rishi in a real-app check, from a counter the build step adds.
14. **The order in decision 442m's example has the dialogs before the Output.** A dialog's Run has nowhere to go
    without the Output, so Part 13 builds the Output with the first dialog.
15. **SPSS 31's tabs include an Overview** (IBM's Brief Guide, Figure 12). The mockup has none, and neither does this
    design.

**Blueprint.** §4.2 lays out Statistics as left, centre, right and bottom panels. Decision 441's tabs differ, and the
step prompt wins. §4.4's Inputs, Settings, Validation, Run and Results survive inside a dialog and an Output entry, not
as a wizard's pages. §3.4 asks that a switch restore the last view; this design keeps it only until the workspace ends.

## Decision

### 1. The shell (decision 442a)

| where the window sits | for | against | verdict |
|---|---|---|---|
| inside `WorkspaceShell` | the switch and the job area exist | its rail takes 216 px of 1024, its header and `overflow: auto` give a second scroller, and it lists tools rather than holding a screen | rejected |
| inside `MethodPage`'s `AppShell` | no new frame | the same | rejected |
| **a full-window screen of its own, `StatisticsWindow`, in place of the rail and header, behind `RequireWorkspace`** | every pixel for the grid; the gate exists | it must carry what the rail gave | **recommended** |

**What it is.** One route, gated by `RequireWorkspace` as the methods are, so opening Statistics is still one click by
the user and the webview never decides (`ws/RequireWorkspace.tsx:43-58`; ADR-001 decision 7). It holds no business logic.

**What the old pages become.**

- **While it is being built:** a new flag, `statsLayout` (`REACT_APP_FEATURE_STATS_LAYOUT`), default off, shows the window at
  `/app/statistics/window`. Every old page works as today.
- **In the last step:** the window moves to `/app/statistics`, `STATISTICS_HOME_PATH`. So the hub tile, the shell's tool link
  and the project page's Back all land on it with no change to them (`screens/statisticsRoute.tsx:16`, `:28`, `:64-66`,
  `screens/ProjectPageScreen.tsx:16`). The six method routes and the summary's go.
- **The home's content** moves to Help, Methods and limits (Part 5): `place` (`screens/StatisticsHome.tsx:62-71`) still decides
  what Rust says is runnable, and its Limitations, roadmap and retired lists go with it.
- **The project page's route stays** (`screens/projects/projectRoute.tsx:15`).

**"Gaply" in the top bar opens a small menu**, which the mockup does not show: Home, Statistical Analysis Check (while its flag is on),
Statistical Analysis Verifier, Settings, and Switch workspace. Switch is today's `leave`, Wait and Cancel dialog included
(`ws/WorkspaceShell.tsx:278-298`, `:404-423`), moved out of `WorkspaceShell` into one shared hook and not copied. Question 1.

**The job area.** `WorkspaceShell` polls for the newest run (`ws/WorkspaceShell.tsx:254-276`). The window does not need that area:
an Output entry shows its own run, and a project's rerun has its own line (Part 9). The shell itself stays, with its tools list.

**At 1024 × 700, and at each region** *(inferred sizes, read in the real-app check)*:

| region | height | at the minimum |
|---|---|---|
| top bar | 40 px | the project's name is clipped first, inside `<bdi>`, whole in its tooltip; the menus never wrap |
| tabs | 32 px | always whole |
| line above the grid | 44 px, two lines | the file name is clipped first, its whole base name in a tooltip |
| grid | what is left, about 540 px | a header of 44 px and about 20 rows of 24 px; a row-number column of 72 px and about 8 columns of 112 px |
| footer | 24 px | the three phrases on one line at 12 px, wrapping to two if the fonts are larger |
| Output | the same as the grid | outline 220 px; a result's table scrolls inside the result pane |
| a dialog | at most 640 px high and 800 px wide | its body scrolls, Cancel and Run always visible |

The window sets `min-height: 0` on its grid area, as the design system's shell does
(`src/design-system/shell.css:11`), and only the grid, the Variable View table and the Output's two panes scroll.

### 2. One open data file for the session (decision 442b)

ADR-011 Part 10's idea carries over, as decision 397 says.

| where the session lives | for | against | verdict |
|---|---|---|---|
| a React provider above the routes | idiomatic | sits at `App.tsx`'s level, or is lost on a visit to Settings | rejected |
| a state inside the window's component | simple | going Home or to the project page loses the file and the Output | rejected |
| **a module-level store, on `openProjectStore`'s pattern (`screens/projects/projectSession.ts:24-51`, `:90-92`)** | exists, tested, memory only, survives a route change, clears when Rust's workspace does | one more module-level store | **recommended** |

**`statisticsSession` holds:** the open slot's number, its `DatasetView`, the Output's entries, and nothing on disk: never browser
storage, as `screens/projects/projectSession.ts:9` says of its own. The cells are never in it (Part 3). `noteWorkspaceState`'s rule
applies: anything but Statistics active clears it (`screens/projects/projectSession.ts:63-65`).

**Each action:**

| action | what happens |
|---|---|
| Open a data file, none open | the picker, then the open with its progress line and the limits sentence (`screens/flowParts.tsx:48-58`, `screens/statsInvoke.ts:35`) |
| Open, one open | the sentence below first if any result is not saved. The picker. A cancelled picker or a refused open changes nothing, as today's order does (`screens/flowHooks.ts:160-162`). Then the old slot is closed |
| Close this file | the sentence if any result is not saved, then `stats_desc_close`, and the slot's rows and prepared columns go. The Output stays |
| a run is going | Open and Close wait: each is disabled, with "A run is going. Wait for it, or cancel it in the Output." A run keeps its own handle on a slot (`app/runs.rs:411-412`), so this is for clarity, not safety |
| Home, Settings, the project page | nothing is dropped. The file stays open in Rust, as a project stays open, and the store holds the Output. Coming back shows them |
| Switch workspace | the sentence, then `workspace_deactivate`, which drops the engine and its slots and run records (ADR-001 decision 3; `docs/adr/ADR-001-one-active-workspace.md:50-54`). Refused while a run is going, as today (ADR-001 decision 5) |
| the app quits, or the webview reloads | everything in memory goes. Nothing asks: a quit has no such hook today *(inferred; not searched for)* |

**The sentences** are in Part 7, since what they warn about is the Output.

**A dialog never closes the shared slot.** Today's rule, that a method closes only a slot it opened, becomes a type: a dialog gets
the `view` and the slot's number as props, and its bridge type omits `open` and `close` (for example `Omit<AlphaBridge, 'open' |
'close'>`, on `screens/alphaBridge.ts:13-25`). So a dialog that closed the slot would not compile. A vitest also pins that no
dialog's mount, Cancel, Run or unmount calls `stats_desc_close`. `useCloseOnUnmount` and `fileActions` stay for the old pages until
they go, and `fileActions` then serves the session store.

**The budget and eviction with one slot.** Nothing changes in Rust: `MAX_SLOTS` 4 and the tier's budget stay
(`app/runs.rs:138`, `app/memory_tier.rs:100-108`). With one slot held, only an open replacing a file holds two, and at the 8 GB tier
two full tables are exactly the budget (10,000,000 cells), so neither is evicted before the old one closes. A make replaces its slot
in place, and the eviction never drops that one (`app/runs.rs:455-468`). While old pages exist they open their own slots, which could
evict the window's, so a slot the window finds gone (`run.unknown_input`) says "This file is no longer held in memory. Open it again."
and clears the file. The flag keeps both from being in ordinary use at once.

### 3. The Data View (decision 442c)

**What a cell shows.** The Dataset's cell (`core/stats_l1/dataset.rs:549-553`), as the prepared CSV writes it
(`core/stats_prep/export.rs:43-45`):

| the cell | on screen |
|---|---|
| a number | Rust's shortest form that reads back to the same double, right-aligned: `1.50` shows `1.5`, a large value at all its digits, an infinity as `inf` |
| missing | empty, and read as "empty" by a screen reader. It includes tokens and SPSS's user-missing values (`core/stats_l1/dataset.rs:15-19`) |
| text | as read, at most 512 characters (`app/supplementary.rs:55`), one line, clipped with an ellipsis, inside `<bdi>`, always a text node and never HTML |
| a value label | never: a Dataset holds none |

**A long cell.** CLAUDE.md's rule is that a clipped value cannot be checked, so a line under the grid gives the active cell's whole
value, with its row and column. Question 2.

**A column's head.** Two lines: the alias, and the name (`scoreName`, or else `header`; the one helper of Part 6). A prepared column adds
the word "prepared" in its head, never colour alone, and its head has a thin edge in the design system's accent. A column is sized by its
kind: 112 px, 200 px for categorical, not resizable in the first version.

**The line above the grid** is decision 441's, plus one sentence always there, because an SPSS user would otherwise read a blank where they
see 99: "As Gaply read it: numbers in full, missing cells empty, no value labels." Question 3.

**How the cells reach the screen.**

| way | cost | verdict |
|---|---|---|
| all the cells at once | 8M-3 measured a whole table's CSV at 35 MB and 89 MB, and 105 MB and 266 MB on the wire. Then every cell is a JS string in the heap *(inferred: well past the 670 MiB open on an 8 GB Mac)* | rejected |
| the CSV parsed in the webview | the same, plus a parser, a second reading of the data | rejected |
| one command per cell or row | a call per row drawn | rejected |
| **a window of cells, one command** | small, and stateless in Rust | **recommended** |

```
stats_desc_cells(slot, firstRow, rowCount, firstColumn, columnCount) ->
  CellWindowView {
    slot, inputHash,            // whose table this is
    rows, columns,              // the table's size
    firstRow, firstColumn,      // what this holds, after clamping
    cells: [[string | null]]    // by row, null for missing
  }
```

- **Named `stats_desc_`** so `every_registered_stats_desc_command_checks_the_workspace_first` covers it by name (`tests/workspace_guards.rs:111`),
  and it sits in `app/stats_desc_commands.rs`, which is scanned for writes already. It starts no run, needs no capability and is read-only.
- **A pure function in `gaply_core`, `stats_desc::cells_window`,** does the reading, with one number-to-text function it shares with
  `prepared_csv`, so there is one rule (Prompt 0). A test pins that a small table's cells, read through windows of every shape, equal the prepared CSV's
  fields.
- **Numbers as strings,** for three reasons: a JSON number cannot hold an infinity, and serde would write it as null, which here means missing *(inferred; a
  test pins it)*; JS would reprint `1e21` differently from Rust; and strings let the screen show what Rust wrote, with no second formatter.
- **`inputHash`** is the `inputRef`'s, so a window that arrives after a make, for the slot's old table, is dropped. A make keeps the slot's number
  (`app/runs.rs:476-497`), so the number alone cannot tell them apart.
- **The cap per call:** at most 200 rows, 64 columns and 12,800 cells, and at most 4 MiB of cell text, stopping at a whole row. A larger or
  out-of-range request is clamped and says what it holds, so there is no new code. A slot gone is `run.unknown_input`, as today (`app/runs.rs:513-518`).
- **Its cost** *(inferred; the step measures it)*: about 12,800 lookups and number formats, a few milliseconds, since columns are `Vec`s
  (`core/stats_l1/dataset.rs:591`). No memory stays in Rust.
- **Never logged but by counts,** `#[tracing::instrument(skip_all)]`, as every command here. The view has no `Debug`.

**Only what is in view is drawn.**

| way | licence and upkeep | size | fit | verdict |
|---|---|---|---|---|
| **a virtual grid in Gaply, no new package** | ours | small: arithmetic on fixed row and column sizes *(inferred: a few hundred lines with the keyboard)* | read-only, one row height, our roles and test ids; `role="grid"` has to be right by us anyway | **recommended** |
| `@tanstack/react-virtual`, headless | MIT, version 3.14.25 and one dependency, its core, peer React up to 19 (its `package.json`) | 10.1 kB gzipped (Bundlephobia) | gives the arithmetic, which is the easy part; keyboard and roles stay ours | the fallback, named here for approval only if the stop below fires |
| `react-data-grid` | MIT, version 7.0.0-beta.63, a beta, peer React `^19.2`, no dependencies (its `package.json`) | not read | virtualises rows and columns, has keyboard support, makes no explicit screen-reader claim on its page; built for editing and its own look | rejected for now: a beta, more than is needed, and its look is not ours |

The app has React `^19.2.0` and TypeScript `^4.9.5` (`package.json:60`, `:68`). A library's types on 4.9.5 and at this project's es5 target are not checked. CLAUDE.md
records that es5 traps reach only `npm run build`, so a library's step would run it first.

**How it scrolls.** One scroller with a spacer of `rows × 24` px by `columns' widths`: 2,400,000 px at 100,000 rows, within the engines' limits *(inferred; WebKit's, WebView2's and
WebKitGTK's are read in the step's checks)*. Headers stick. Rows and columns in view, plus a few beyond, are absolutely placed. A scroll asks for the windows it lacks, the newest request winning; a cell not
yet here is blank with `aria-busy`, and never shows a stale value. A small cache holds at most 16 windows, so at most 64 MiB at the worst text, and is emptied on a close, an open or a make.

**The keyboard, in the first version.** Tab enters the grid at the active cell, starting at row 1, column 1. Arrow keys, Page Up and Down, Home and End, and Ctrl or Cmd with Home and End move it, and the grid
scrolls to it. Tab leaves.

**Selection and copying: out of the first version.** One active cell, no range. No Copy handler, and `user-select: none` on the cells. A copy would put rows on the system clipboard, where other apps can read them. That is the
user's right, and a later step can add it knowingly. The active cell's line can be selected, since that is one value the user chose to read. Question 4.

**A screen reader.** `role="grid"`, with `aria-rowcount`, `aria-colcount` and `aria-readonly="true"`. Each drawn row has `aria-rowindex`, each cell `aria-colindex`, each head `role="columnheader"` and each row number
`role="rowheader"`. Focus by `aria-activedescendant`, so virtual rows can come and go. Tested for roles in vitest. Whether VoiceOver reads it well is a line in the real-app check, since a jsdom cannot say.

**Budgets.** Measured in the step on ADR-010's pattern: a release probe under `/usr/bin/time -l` with a 1 ms sampler for the Rust half, at each tier's largest table, and a timing counter behind the devtools build for the webview's half, read by Rishi.
These figures are proposals, none measured.

| what | target | stop: tell Rishi with the figures | measured by |
|---|---|---|---|
| one window of 40 rows × 20 columns, Rust's side | 5 ms | 50 ms | release probe |
| the cap's window, 200 × 64, Rust's side | 20 ms | 100 ms | release probe |
| the same, to the webview and back, at the widest table | 50 ms | 200 ms | counter, real app |
| the first paint after an open has returned | 100 ms | 500 ms | counter, real app |
| one frame while scrolling, 95th percentile, 100,000 × 50 on an 8 GB Mac | 16 ms | 33 ms | counter, real app |
| the webview's memory after scrolling the whole file once | under 150 MiB over its start | 300 MiB | Activity Monitor, real app |

**If the scroll's stop fires,** the library above is proposed by name before anything else changes.

### 4. The Variable View (decision 442d)

A plain table of at most 128 rows, one for each column of `profile.columns`.

| decision 441's column | from | shown as |
|---|---|---|
| # | the alias's number | `3` |
| Alias | `alias` (`core/stats_desc/mod.rs:264`) | `C3` |
| Name | `scoreName` (`app/stats_desc_commands.rs:154`) or `header` | inside `<bdi>` |
| Type | `kind` | Numeric, Categorical or Empty, as `screens/flowParts.tsx:150` |
| Distinct values | `distinct` | a count |
| Empty cells | `missing` | a count |
| Possible missing codes | `suspectMissingCodes` | `99 (3 cells)`, and none is blank. A score has none (decision 418) |
| From | `prepared` (`app/stats_desc_commands.rs:151`) | "The file" or "A recipe" |

Under it, decision 441's sentence, then what the SPSS reader noted (`screens/flowParts.tsx:120-127`), then each score's four counts. Today's table also gives up to twelve levels
(`screens/flowParts.tsx:154-155`), which decision 441's columns do not. The dialogs that need a group's levels read them as today, so they are not lost. A column's `nonFinite`
count has no column either, and is one line under the table when it is above 0, since a method refuses such a column and the user needs to see why.

**Read-only, and what is left out.** SPSS's Variable View lets the user change name, type, width, decimals, label, values, missing, columns and measure (IBM's page). In the first version, none:

| SPSS's column | why not now |
|---|---|
| Label, Values | Gaply would have to keep what the user typed. That belongs in a recipe with its own review, and today neither a result nor a report holds such text. A file's own value labels are not in a Dataset |
| Missing | a code is asked per run (decision 392c, ADR-011 Part 3). A setting here would be a second place, changing every later run silently |
| Measure | a level of measurement would suggest which method fits, and Gaply gives no advice (decision 66). Methods decide from the data's kind and the user's choice |
| Type, Width, Decimals, Name, Columns, Align, Role | Gaply never changes the file's reading. Display preferences are a later, small step if asked for |

### 5. The menus (decision 442e)

**In the window, not the operating system's menu bar.** Decision 441 places them in the top bar. The app registers no native menu in `app/lib.rs` (a search for `Menu` finds none).

| item | capability | command | opens |
|---|---|---|---|
| **File** Open a data file… | none: the session | `stats_desc_open`, `stats_desc_open_progress`, `stats_desc_limits` | the native picker |
| Close this file | none | `stats_desc_close` | Part 2's sentence |
| Export the prepared data (CSV)… | `stats.prepare.scale_score` | `stats_prep_csv` | ADR-011 Part 8's sentence, the save dialog |
| **Transform** Scale score… | `stats.prepare.scale_score` | `stats_prep_make`, `_recipe`, `_clear` | Part 8's dialog |
| **Analyze** Descriptive statistics, Summary… | `stats.describe` | `stats_desc_*` | its dialog |
| Compare means, Independent-samples t-test… | `stats.analyze.independent_t` | `stats_ttest_*` | its dialog |
| One-sample t-test… | `stats.analyze.one_sample_t` | `stats_onesample_*` | its dialog |
| Paired t-test… | `stats.analyze.paired_t` | `stats_paired_*` | its dialog |
| One-way ANOVA… | `stats.analyze.one_way_anova` | `stats_anova_*` | its dialog |
| Correlate, Correlations… | `stats.analyze.correlations` | `stats_corr_*` | its dialog |
| Scale, Reliability (Cronbach's alpha)… | `stats.analyze.cronbach_alpha` | `stats_alpha_*` | its dialog |
| **Project** Open a project… | none | `project_list`, `project_open` | Part 9 |
| Run every saved result again | none: each its method's | `project_runs`, `project_rerun`, `project_current` | Part 9 |
| Export the project report… | none | `project_report` | today's dialog (`screens/projects/ProjectPage.tsx:811`, `:855`) |
| **Help** Methods and limits | as Rust's list says | `capability_list`, `stats_desc_limits` | a dialog with the home's content |
| How Gaply reads a file | none | none | fixed text, the Data View's note in full, and the ADR-011 Part 8 sentence's facts |
| Keyboard | none | none | fixed text, Part 3's keys |

**A method is enabled by Rust's word, not by the menu.** Each Analyze and Transform item reads `capability_list`, as the home does (`screens/StatisticsHome.tsx:172-181`). An item not runnable is disabled. `METHODS` gains the menu's group and label, so a new method adds
a row, and `methods.vitest.ts`'s check of the table covers it. A kind's dialog comes from a table keyed by the same kind, so a row without a dialog fails a test, as `STORED_CHECKERS` does (decision 383).

**Disabled, each with its sentence.** A disabled item stays focusable (`aria-disabled`), and its sentence is its description, so a keyboard user hears why.

| item | when | sentence |
|---|---|---|
| Close this file, every method, Scale score… | no file open | "Open a data file first." |
| Export the prepared data (CSV)… | no recipe | "Make a scale score first." |
| Open a data file…, Close this file | a run is going | "A run is going. Wait for it, or cancel it in the Output." |
| Run every saved result again, Export the project report… | no project open | "Open a project first." |
| Run every saved result again | a run is going, or no saved results | "A run is going…", or "This project has no saved results yet." |
| Project's items | `projectsShown()` is no (`screens/projects/projectGate.ts:13-15`) | not shown, like every project control, and the line above is not either |

**"Coming later": plain labels.**

| item | registry entry? |
|---|---|
| Crosstabs and chi-square | none. `stats.verify.chi_square_independence` is the verifier's, not this. Next by decision 440 |
| Regression | none. `stats.verify.ols` is the verifier's |
| Factor analysis, Nonparametric tests, CFA and SEM | none |
| Recode values…, Compute from a formula…, Select cases… | none. A limitation says they are not offered yet (`screens/capabilityWording.ts:85`) |

| how to list them | cost | verdict |
|---|---|---|
| register each as Planned first | each is a pinned entry with counts, wording, a stub page and limitations in about fourteen places (decision 372), for work with no design | rejected |
| leave them off until registered | honest, and simple | not what decision 441 accepted |
| **a closed list of labels in the menu's table, under a heading "Coming later", disabled, with no id, no date, no handler and no sentence promising anything** | one small table; a test that none has a handler and none matches a word such as a date | **recommended**. Question 5 |

**How it relates to the rest.** The home's roadmap and `capabilityWording.ts` say what Rust's registry says, and stay so in Methods and limits. When a Planned entry exists, as chi-square's will at its first step (decision 440), it appears in the roadmap as it
does today, its label stays in "Coming later", and at Beta its label leaves and it becomes a live item, in one change. The chooser already uses these same words, for a workspace (`ws/workspaceWording.ts:32`). The blueprint asks each screen to mark what is planned, and a label is that.

**The keyboard.** `role="menubar"` and `role="menu"`. Tab reaches the bar. Left and Right change menu, Up and Down the item, Enter opens it, and Escape closes it and returns the focus. A letter jumps to an item. There are no global shortcuts in the first version, so none can be pressed by accident and none clashes with a
webview's. `Cmd+O` is a later small step. The design system has no menu to reuse (a search of `src/design-system` for `aria-haspopup` and `role="menu"` finds nothing), so a `Menu` joins it.

### 6. The dialogs (decision 442f)

**A dialog's frame.** One component, `MethodDialog`, on `ProjectDialog` (`screens/projects/ProjectDialog.tsx:36`), which keeps the focus inside, closes on Escape and returns the focus. `ProjectDialog` moves out of `projects/` so both can use it, unchanged. The data stays visible behind.

**Its behaviour, one rule for all seven:**

1. It opens from the menu with nothing chosen. It holds the settings for as long as it is open; Cancel and Escape drop them. Reopening starts empty. So one spec is one set of user's choices, and nothing carries over (decision 338).
2. **Run** is disabled until the method's own readiness rule holds, with today's sentence saying what is missing (`screens/AlphaFlow.tsx:465-469`).
3. **Run calls the method's `validate`.**
   - `valid`: it calls `run`, the dialog closes, and the entry appears in the Output, with the focus on its heading.
   - `valid_with_warnings`: the dialog stays, shows the warnings, and Run reads "Run with these warnings". A second press runs it. The warnings are not lost by having one button.
   - anything else: the dialog stays, with the sentences, and nothing runs. This covers `reversed_outside_scale` and the rest (`screens/AlphaFlow.tsx:478-483`).
   - A refused `run` (`run.run_busy`, or the workspace gone) stays in the dialog too.
4. **A dialog closes nothing and owns no file** (Part 2).
5. **No method's logic is copied.** Each `*Flow.tsx`'s settings move to one exported component and hook, which the old page and the dialog both render until the old page goes. So there is one copy at every commit. The test ids stay (`al-item-…`, `tt-outcome`), so their tests move almost unchanged.

**A score's label and group filter (premise 10)** go in here, once: one helper, `columnLabel(c)`, gives `scoreName` or `header`. Each flow's `names`, lists and questions call it. The t-test's and ANOVA's group lists leave out `prepared` columns, Rust's refusal staying as the boundary. Alpha, the correlations and every numeric list offer a score.

**Per method.** The bridge, types and wording move unchanged, except that the bridge's type loses `open` and `close`.

| method | moves into the dialog | stays as it is, reused by the Output | goes |
|---|---|---|---|
| Summary (`DescribeFlow.tsx`) | `buildSpec` (`:115`), the include and treat-as table (`:518-568`), the codes (`:570`), the levels rule (`:498`) | `DescribeResult` (`:221`), its export's choice (`:679`) | import, profile (`:495`), stages, its own open and close (`:299-304`, `:353-354`), page (`:732`) |
| Independent t (`TTestFlow.tsx`) | `buildSpec` (`:134`), outcome, group, levels, variant and codes (`:365-462`), `chooseGroup` (`:301`), the levels rule (`:357`) | `ResultCard` (`:537`), `ExportCard` | the rest, as above, page (`:647`) |
| One-sample (`OneSampleFlow.tsx`) | `testValueOf` (`:100`), `buildSpec` (`:106`), outcome, test value and codes (`:296-345`) | `OneSampleResultCard` (`:421`) | the rest, page (`:507`) |
| Paired (`PairedFlow.tsx`) | `buildSpec` (`:106`), the two columns and codes (`:322-355`) | `PairedResultCard` (`:432`) | the rest, page (`:526`) |
| ANOVA (`AnovaFlow.tsx`) | `buildAnovaSpec` (`:128`), outcome, group, levels, post hoc and codes (`:350-441`), its group's levels read (`:290`), the levels rule (`:342`) | `AnovaResultCard` (`:597`) | the rest, page (`:746`) |
| Correlations (`CorrFlow.tsx`) | `buildCorrSpec` (`:129`), variables, coefficients, missing rule and codes (`:337-429`) | `CorrResultCard` (`:632`) | the rest, page (`:709`) |
| Alpha (`AlphaFlow.tsx`) | `buildAlphaSpec` (`:134`), items, reversal, ends and codes (`:371-473`), its two toggles (`:303-317`) | `AlphaResultCard` (`:584`) | the rest, page (`:674`) |

A t-test with exactly two levels selects them today (`screens/TTestFlow.tsx:130-132`). That stays: with only two, there is no choice to make, and the user's choice was the column.

**The worked example: alpha.** The mockup's list gives two columns. Left, the numeric columns, including scores, each a button that adds it. Right, "Items, in the file's order", first showing "None yet. Nothing is chosen for you." Each item has a Reverse-score box and a remove button. The order the user clicks does
not matter: `buildAlphaSpec` writes column order whatever they do (`screens/AlphaFlow.tsx:143`), which is what "in the file's order" says. When something is reversed, decision 384's sentence and two empty fields appear (`screens/AlphaFlow.tsx:417-448`), and their fix (`:120-129`) comes along. Then the code question (`:449-464`). Removing an item unticks its reversal
(`:311-314`).

**The mockup and decision 384 differ on "Tick every numeric column" and "Untick all".** Without them, a 128-item scale takes 128 clicks. Each is an action the user takes, never a default. Recommended: keep them, reworded "Add every numeric column" and "Remove all", in the same place. Question 6.

### 7. The Output (decision 442g)

**An entry** holds: its number, the method's kind, the `view` at its run, the settings line, the run's id, and its state, then its result.

- **The `view` at its run** is a reference to that `DatasetView` object, so names survive a close, an open or a changed recipe. A score's alias can later mean another score, so names from the current view could be wrong. *(Inferred: little memory, since most entries share one view. A column of 100,000 levels is the worst case, and the step reads it.)*
- **The settings line** comes from the spec the dialog built: outcome, group, items, reversal, the ends, the codes. It reads through the same words as the method's, and is new: today's results show few of them.
- **While it runs:** `RunCard` (`screens/flowParts.tsx:190-248`), with its elapsed line, its Cancel and each method's progress, polling through `useRunPolling` (`screens/flowHooks.ts:42-64`), under the entry's heading. Its "Back to settings" goes: a new dialog is a new choice.
- **When it succeeds:** the bridge's `result`, then the method's existing card. The manifest panel stays in it (`screens/flowParts.tsx:305-358`), and a user reads it as today.
- **When it fails, times out or is cancelled:** its sentence and diagnostics, and for a failure with a code, Save to project (decision 140, `screens/projects/SaveToProject.tsx:35-38`), as the run card does now.

**Save and export.** `SaveToProject` with the entry's run id, unchanged. "Export CSV…" is `saveExactly` through the method's `bridge.export`, as today (`screens/AlphaFlow.tsx:287-299`, `screens/flowHooks.ts:178-192`, `src/utils/saveTextFile.ts:15`). The run manifest's JSON is today's second choice on the export card (`screens/flowParts.tsx:253-298`). Kept: "Export CSV…" opens that small card, so nothing is lost, and the first choice is the CSV.

**The outline.** A numbered list, in order, beside the results. Each line is the method's name, and clicking moves the result pane to it and the focus to its heading. Newest last, and after a run the pane goes to the newest, and a status line says so.
An empty Output shows decision 441's sentence. The tab's count is the entries. SPSS's outline also hides and reorders (IBM's Brief Guide); here it does neither in the first version.

**Drawing many results.** An entry is mounted when it first comes near the pane, then stays mounted, so its Saved line stays and it never re-renders from nothing. A corr result at 128 variables took long enough in a vitest to reach its 20 s timeout under load (decision 430), so all of them cannot render at the start.
*(Inferred; the step measures 32 entries, with the corr result at 128 variables among them.)*

**In memory for the session, with no autosave (decision 105).** Never browser storage. It is not saved when a project is opened or closed.

**The cap: 32 entries, and `MAX_RUNS` from 8 to 40.** ADR-010 §4 and decision 6 say 8.

| option | cost | verdict |
|---|---|---|
| Output 8, `MAX_RUNS` 8 | no change, and a thesis's session outgrows it fast | rejected |
| an endless Output, Save and Export lost on older entries with `project.run_evicted` (`screens/projects/projectWording.ts:94`) | no Rust change; a result that looks whole but cannot be kept, unless the user is warned for each | rejected |
| the webview holds each entry's export and Save inputs | sends the spec and bundle both ways and trusts the webview's copy, which ADR-001 and the Save's checks do not allow | rejected |
| **Output 32, `MAX_RUNS` 40, a rerun's finished record dropped before a live one, and Remove dropping its record** | one constant, pinned by name; a flag on a record and a small change in `evict` (`app/runs.rs:1857-1865`) with its tests; one small command; memory measured | **recommended**. Question 7 |

- **A rerun must not push a live result out.** Reruns take records too. With Run every saved result again, 40 reruns would drop every live record. So `evict` drops a rerun's finished record first, oldest first. The 8 records above the Output's 32 are what makes this hold: 32 live results and a going rerun are 33 records, and a limit of 32 would have to drop a live one. `project_current` answers from its own state, not from a record (`app/project_commands.rs:504-520`), and a going rerun is unfinished, so it is never dropped.
- **At 32:** Run is refused in the dialog with "The Output is full. Remove a result you have kept, then run this." Each entry's outline line gains a Remove, which asks if the entry is not saved and is the only thing that ever removes an entry. No entry is ever dropped by itself. **Remove also drops the record,** through one new command, `stats_desc_forget_run`, refused for an unfinished run with today's `run.not_finished`, so no new code. Otherwise a removed result's newer record would stay, and the next run would push out the oldest entry still on screen. A forgotten run reads as `project.run_evicted` afterwards, as any dropped one does.
- **Memory** *(inferred)*: a stored result is read back under a 4 MiB limit (`app/project_commands.rs:138`), so a result is small next to the table, most far under. 40 at their largest is about 160 MiB in Rust, and 32 of them more in the webview. The step measures 32 at the largest. **Stop:** if the app's peak with 32 passes the peak with 8 by more than 25% at the 8 GB tier's largest table, tell Rishi with the figures.

**What happens to the Output, and the sentence before anything is lost.** Each is asked only if some entry is not saved, or a recipe exists.

| action | the Output | sentence, then Continue and Cancel |
|---|---|---|
| Close this file | stays. Entries from this file can no longer be saved, by `app/project_commands.rs:1609-1612`, though each can be exported; each entry's Save says why. | "Close this file? Your results stay here and can still be exported, but those you have not saved to a project can no longer be saved. Any scores you made go with the file; only a saved run keeps their recipe." |
| Open another file | the same | the same sentence, beginning "Open another file?" |
| Switch workspace | gone, with the file | "Leave Statistics? Your results and scores go when you leave, unless you saved them to a project. Exported files are not affected." |
| quit | gone | none, as today |

An entry whose Save cannot succeed shows the button disabled, with `project.data_unavailable`'s sentence (`screens/projects/projectWording.ts:99`). Rust's refusal stays the boundary, whatever the screen shows.

### 8. The scale score's dialog (decision 442h)

**This replaces ADR-011 Part 10 and question 12.** No page of its own, and no "use the prepared data" in each method: the file is shared, so every method sees a score the moment it exists, through `view.profile.columns`.

**The form,** in `MethodDialog`, with ADR-011 Part 10's rules. Nothing is filled in.

1. The new column's name, empty, on ADR-011 Part 4's rules.
2. Its items, as alpha's list, offering only file columns (ADR-011 question 13), so a score is not listed: 2 to 127 of them, since the score itself takes a column (decision 408). Reverse-score boxes unticked.
3. The scale's ends, empty and shown only when an item is reversed, with decision 384's sentence, never taken from the data.
4. "The score is the" sum or mean, unchosen.
5. "A person gets a score when" every item has a value, or at least n items have one, unchosen, n empty.
6. The missing-code question, unticked.
7. The sum's sentence, when the rule is "at least n" and the statistic is a sum.
8. **Make the column**, disabled until all of these are valid, with the sentence saying what is missing.

No verdict word, threshold or advice. No shortcut from alpha with its items (ADR-011 question 10). The screen trims the name before it sends it, since Rust refuses an untrimmed one (decision 408), and sends a typed `-0` as `0`, since Rust refuses that too (decision 413's answer 4).

**On Make.** `stats_prep_make` with the recipe: the scores already held, then this one, read through `stats_prep_recipe`. A refusal stays in the dialog with its sentence: the closed `PrepCode`s gain their wording in this step, since `src/` has none yet (decision 426). When it holds, the dialog closes and the store takes the new view. **So the column is in the Data View and the Variable View at once,** the grid's cache emptied. The four counts show in a status line and stay in the Variable View.
A make refused while a run is going says so (`app/stats_prep_commands.rs:115-116`). Removing a score, or removing them all (`stats_prep_clear`), is one more item in the same Transform menu's dialog, listing the scores made: without it a typing mistake is stuck until the file closes. The mockup does not show it. Question 8.

**Each earlier result, and the second score (premise 3).** A prepared run's Save requires the slot to hold the run's own recipe. So: alpha, score A, a t-test on A, then score B, and that t-test can no longer be saved.

| option | cost | verdict |
|---|---|---|
| a sentence at the make: "Results that used the earlier scores can no longer be saved." | none in Rust. It teaches an odd order of work, and a refused result looks like a fault | not enough on its own |
| one recipe per score, slots side by side | a graph of recipes, against ADR-011 | rejected |
| **a run's record keeps its recipe's bytes, and a Save writes those, compared by hash with the spec's** | the Save never needs the prepared cells (it writes the file's bytes, the recipe, the spec and the bundle). So only `recipe_of` and `check_prepared` change (`app/project_commands.rs:951`, `:1025`), and they compare the record's hash, not the slot's. A recipe is small, and a recipe holds typed names, so the record joins the matrix's allowed surfaces | **recommended**. Decision 433's "the slot is the only place the recipe can come from" changes. Question 9 |

The file's bytes still come from the slot, so Close's sentence stands.

**File, Export the prepared data (CSV)….** ADR-011 Part 8's sentence before the dialog, word for word, then the save dialog, suggested name `prepared_data.csv`. A cancelled dialog writes nothing.

**What earlier steps left for the dialog, and where each goes in Part 13.**

| left by | what | where |
|---|---|---|
| decision 430, answer 1 | a make profiles only the new columns, the view's bytes equal to a full profile's | the score's step, red first |
| decision 430, answer 2 | the CSV's way to the webview, measured; that one command moves to a binary response if the array is the problem (`screens/flowHooks.ts:183-187` copies it three times) | the score's step, at 89 MB, with a stop; `ExportView` stays for the methods |
| decisions 433, 436 and the disk list | the amended save sentence: that the steps and the names are kept. Only for a run with a recipe, so `SAVE_SENTENCE` (`screens/projects/projectWording.ts:20-21`) stays for the rest | the score's step |
| decisions 433, 436 | the project page lists a project's recipes from the `used` edges, and a prepared saved result shows `Column 51 (name)` | the projects' step |
| decision 434, decision 439's answer 1 | `preparationEngineDiffers`' sentence: none needed now, since the two versions cannot differ on a rerun. A code and a sentence come at the first real bump | not built, and this says so |
| decision 439, answer 3 | the release note: an older build calls a project with a recipe unreadable | the old pages' retirement step, or the first build with the dialog that ships |
| decision 439, answer 4 | `rerun_over_limits` end to end, on the 8 GB tier's largest table: saved through the make's body at a larger cap, then rerun on this computer's | the score's step |
| decision 430's answer 4 | `run.run_busy` for a make overtaken: its sentence | the score's step |

### 9. Projects in the shell (decision 442i)

**The line.** "Project: none open", or "Project: " and the name, inside `<bdi>`, from `useOpenProject` (`screens/projects/projectSession.ts:90-92`), and clicking it opens the project's page. It is a read of Rust's answer, as every project screen's is, and asks `project_current` when the window appears (decision 148).

**Save to project… from a result.** `SaveToProject` unchanged. A Save into a new project closes the open one and opens the new (`app/project_commands.rs:1619-1621`), and the line follows through the store (`screens/projects/SaveToProject.tsx:129`).

**Open a project….** A dialog holding `ProjectsPanel` (`screens/projects/ProjectsPanel.tsx:57`), so its list, sizes, totals and Delete… move with it unchanged. `onOpened` then shows the project's page as now (`screens/StatisticsHome.tsx:269`). Its focus rule of decision 172 goes with the home.

**How saved results show.**

| where | for | against | verdict |
|---|---|---|---|
| in the Output, mixed in | one place | a saved result has a stored shape and its own renderers (`screens/projects/SavedResult.tsx`), no live run, no Save; mixes "this session" with "kept", and the cap | rejected |
| a fourth tab | stays in the window | the mockup has three; the page's dialogs and focus rules all move | rejected for now |
| **on the project page, as today** | every test and rule stays; the delete, the PROV export and Close project are there; the file and the Output survive a visit (Part 2) | one route away | **recommended** |

The page's Close and Back go to `STATISTICS_HOME_PATH` (`screens/ProjectPageScreen.tsx:16`), which is the window by then. A page's title and rail stay until it has its own step.

**Run every saved result again.** Today, one at a time: `project_rerun` by run id (`app/project_commands.rs:1944`), one going at a time (`app/project_commands.rs:321`), with the driver in the page (`screens/projects/ProjectPage.tsx:292-346`).

| option | cost | verdict |
|---|---|---|
| wait | a thesis's chain cannot be checked together, and the mockup shows the item | not recommended |
| a new Rust command that runs the list | a second rerun path, a new state machine, new guards | rejected |
| **a frontend driver over today's commands, in order, one line each** | no new Rust and no new write; all of a rerun's refusals and checks hold | **recommended**. Question 10 |

- **Which runs:** those with no `rerunOf`, oldest saved first, since a rerun of a rerun repeats the same thing. A chain saved in the order alpha, score, method is repeated in that order. There is no dependency between them, since each rerun rebuilds its own recipe.
- **The sentence first,** on `RERUN_SENTENCE` (`screens/projects/projectWording.ts:293-294`): "Run all {n} saved results again? Gaply runs each one, with the same settings, on the copy of the data file kept in this project. Each new run is saved beside the one it repeats, which does not change." A click starts it, and nothing runs by itself.
- **One line each,** as the run finishes, with its name and number: Identical, Differs or the refusal's sentence, through `RERUN_WORDS` (`screens/projects/projectWording.ts:305`), so no new comparison is written and no verdict is added. The kinds-of-computer lines stay (decision 366). Then one last line: "{n} repeated: {a} identical, {b} differ, {c} could not run."
- **Stop.** The going run's kind's cancel, then no more. The lines so far stay, and a cancelled run writes nothing (decision 186).
- **A refusal does not stop the list,** except for three that would refuse every line: `project_cap`, `all_projects_cap` and `disk_full`. A `project.rerun_over_limits` is one line.
- **Cost** *(inferred)*: each rerun reparses its file and rebuilds its recipe. At the 8 GB tier's largest table, about 3 s to open (ADR-010, 8M-3) and about 1 to 5 s to run, so 20 saved results take about 2 to 3 minutes. Each writes a new bundle and manifest (decision 186), so a project grows on every press. Its cap refuses cleanly (ADR-009 §9). A rerun's one-run slot is memory on top of the open file's (`app/runs.rs:266-274`), so an open table plus a rerun is about two tables: **the step measures that peak at the 8 GB tier's largest table, with a stop if it passes the budget of ADR-010's open by more than 25%.** If it does, the window closes nothing by itself: the list's sentence says to close the file first.
- **Nothing else runs meanwhile.** One foreground run at a time holds (`app/runs.rs:2049-2050`), so a dialog's Run reads `run.run_busy`, and Switch is refused with today's dialog (`ws/WorkspaceShell.tsx:404`). The driver lives in the session store, so going to the project page does not stop it, and the page's own rerun line shows the going one, with its Cancel.

**Export the project report….** Today's dialog and path (`screens/projects/ProjectPage.tsx:811-855`, `screens/projects/projectExport.ts`), lifted to a small component both use, and not copied.

**A thesis's chain.** Seen on the project page: the runs in order, each with its recipe named, after this ADR's projects step. Rerun together from the menu. Exported as one report, with its "Prepared column 51: …" lines (decision 435).

### 10. Privacy (decision 442j)

**The Data View is the first screen to show a table's rows.** They reach the webview for the user's own screen. Never a log, an outbound struct, a manifest, a project row, a refusal or an AI task. No AI task reads a cell, and none may be added without its own ADR.

**What pins it, on the existing ones' pattern:**

| guard | what it pins | status |
|---|---|---|
| `tests/stats_cells_privacy_matrix.rs`, on `tests/stats_prep_privacy_matrix.rs` | a cell sentinel, as text, as a printed number and as 8-byte doubles. Allowed: the window's view and the DatasetView's level labels as today. Forbidden: every other command's answer, every refusal (a window out of range, a slot gone), every log line from every thread, every app-data file, every spec, manifest and result of a run after scrolling. Its **negative control:** a window that does hold the sentinel, so the search can find what it seeks | new |
| `no_path_in_command_spans.rs` (`tests/no_path_in_command_spans.rs:32`) and `no_error_text_in_tracing.rs`, `no_path_in_tracing.rs` | the new command's span and logs. Counts only: rows and columns asked and returned | a row or by their scan |
| `every_registered_stats_desc_command_checks_the_workspace_first` (`tests/workspace_guards.rs:111`) | the workspace check first | by name |
| `stats_desc_writes_nothing.rs` (`tests/stats_desc_writes_nothing.rs:44`) | the command writes no file | by its scan |
| `project_store_guards.rs`'s `OUTBOUND` (`tests/project_store_guards.rs:54`) | `CellWindowView` and `cells_window` named in no outbound file | a sibling of `no_recipe_type_reaches_an_outbound_struct_or_a_transport` |
| `prep_guards.rs`'s scan of 21 Level 1, `stats_assist` and PublishReady files | the same two names | a row |
| `contracts_are_pure.rs` | its scan of `stats_desc` covers `cells_window`, `STATS_DESC_CRATE_PATHS` does not grow, and a view holding text joins its list as `ColumnProfile` did (decision 408) | by its scan, and a row |
| a new vitest over the grid's and the session's source, on `screens/projects/projectGuards.vitest.ts` | no `localStorage`, `sessionStorage`, `indexedDB`, `console`, `fetch`, Supabase client or AI bridge | new, with a mutation each |
| a vitest on the grid | a cell with `<img onerror>` in it is a text node. Rendered, never a `dangerouslySetInnerHTML` | new |

**And in the webview.** A search of `src/` for Sentry, PostHog, Mixpanel and Amplitude finds one file, `src/components/ErrorBoundary.tsx`, and only a comment naming Sentry (`:42-43`); `package.json` has none of them. That boundary logs to the console in development only (`:30-32`) and passes an error to its `onError` prop if given (`:35-37`). So a cell must never be in a thrown message: the grid throws closed codes only, and a vitest pins it.

**Other rules kept.** A cell has no `Debug`, counts only (`core/stats_l1/dataset.rs:556-564`). A file's base name stays where it is today, in `DatasetView`. The settings line and the names are the user's own, on their own screen, and never in a result (ADR-011 Part 4). A Run's spec holds aliases.

**The clipboard.** Part 3 leaves copying out, so no new route exists. The one value the user selects from the active cell's line is their own act.

### 11. The workspace rule (decision 442k)

The window exists only inside Statistics. `RequireWorkspace id="statistics"` gates it, and every command it calls checks in Rust whatever the screen shows (ADR-001 decisions 3 and 4). Compute and Design have no field and no route here. The `statsLayout` flag, under `statsDescribe`, hides it while it is unfinished, and is never a boundary, which a test of the screen's refusal pins: with the flag on and the workspace inactive, the gate shows and no command runs. When the old pages go, the flag goes.

No other workspace's menu or word appears. The Stats Check and the Verifier keep their routes and their own entry (ADR-001's addendum), and only Part 1's menu links them.

### 12. What the existing tests become (decision 442l)

| test | what happens |
|---|---|
| The seven `*Flow.vitest.tsx` | the settings, refusal, spec and result tests move to `*Dialog.vitest.tsx`, the test ids kept, each assertion unchanged, listed one by one in each step's report. The import, profile and stage tests stay while the page exists, and go with the page in the last step |
| `*Types`, `*Wording`, `*Bridge` vitests, `describeBridge.vitest.ts`, `statsInvoke.vitest.ts`, `methods.vitest.ts` | stay. `methods.vitest.ts` gains the menu's and the dialog's tables |
| `savFindings.vitest.tsx`, `methodLimitsScreens.vitest.tsx` | move to the Variable View and the empty Data View, each sentence the same |
| `StatisticsHome.vitest.tsx` | `place`, the limitations, the roadmap and retired tests move to Methods and limits. The cards' links change to a menu's, in the last step |
| `statisticsRoute.vitest.tsx`, `describeRoute.vitest.tsx` | gain the flag's cases at once. In the last step, the routes they pin go, and they pin the window's route under each flag |
| `projectScreens`, `projectPageScreens`, `projectReruns`, `ProjectPage`, `ProjectsPanel`, `SaveToProject`, `SavedResult`, `projectsLine`, `saved*`, `stored*`, `projectExport`, `projectGuards` vitests | stay. They test props and bridges, none a page around them *(inferred; each step confirms by running it)*. `projectsLine`'s line on the workspace page stays, and the window gets its own |
| `methodsSeam.vitest.tsx` | stays. Its stored-result and shell differences are unchanged. Any on a method's page move with it |
| `WorkspaceShell.vitest.tsx`, `workspaceRoute.vitest.tsx`, `EnterStatistics.vitest.tsx` | stay. The switch's hook moves, and its tests still reach it through the shell |
| Rust: `runs.rs`'s eviction tests | change for `MAX_RUNS` and the rerun-first rule: `eviction_drops_the_oldest_finished_record_and_never_an_unfinished_one` stays as is at the new number, and a new test pins the new rule |
| Rust: the Save's tests on a replaced recipe | `a_save_after_the_recipe_was_replaced_is_a_data_mismatch_and_writes_nothing` changes, if question 9 is yes |
| Every golden, wire fixture and stored fixture | byte-identical. No method's arithmetic, spec, result or manifest changes |

**New for every step:** the seven methods stay reachable at each commit, by old page or dialog.

**Real-app checklists from then on.** One per step, `~/gaply-corpus/os/phase8/realapp_<step>.md`, with its data written by a script of its own with no `#` line (decision 385). The first run of each is the release build, with `scripts/dev-detached.sh`. Each step quotes its timing counter's figures there. When a dialog replaces a page, the checklist repeats the earlier checks' numbers on the same files as `realapp_8i2.md`'s and those before it, so a number that moved is seen.

### 13. The implementation plan (decision 442m)

Each is its own commit, with `cargo test --workspace` and the frontend's commands. Letters are Rishi's to give. **Every step leaves a working app:** until the last, the flag is off by default and every old page is untouched.

**Step: the shell, the menus and the Variable View on the shared slot.** The flag and route; `StatisticsWindow`; the top bar, tabs, line and footer; `Menu`; every item of Part 5, the dialogs' items disabled with "This is coming in the next step of the build", which that flag hides; `statisticsSession`; Open and Close of a data file; the switch's hook; the Variable View; Help.
- *Tests:* the regions at 1024 × 700 by CSS rule; each menu item's state with no file, with a file and with a run going; menu keys; a refused open changing nothing; the store cleared when the workspace is not active; the Coming-later table has no handler and no date.
- *Mutations:* an item enabled with no file; a Coming-later item given a handler; a score's row showing the header rather than its name; the store surviving a deactivate; the `From` column always "The file".
- *Real-app check:* open at 100,000 × 50; each menu by keyboard; the window at its minimum; Variable View against a table the checklist gives; going Home and back keeps the file; Switch drops it.

**Step: the Data View and its window command, measured.** `stats_desc::cells_window`; `stats_desc_cells`; the grid; keyboard; the active cell's line; a release probe.
- *Tests:* windows of every shape equal the prepared CSV's fields; clamping; the cap; an infinity, a `-0` and a 1e300 as strings, an empty as `null`; a stale window dropped by `inputHash`; the draw is bounded (100,000 × 128 draws under a fixed count of DOM cells); roles; the privacy matrix and every row of Part 10.
- *Mutations:* the workspace check removed; a number through JSON; a cell value in a log line; a clamp off by one; `inputHash` ignored; the cache not emptied on a close; a cell drawn as HTML.
- *Real-app check:* Part 3's budgets at 100,000 × 50, read from the counter, each with its stop; 128 columns at 39,062 rows; a text-heavy file; VoiceOver over a dozen cells; Activity Monitor after a full scroll.

**Step: the Output and the first dialog, alpha.** `MethodDialog`; `ProjectDialog` moved; `AlphaSettings` extracted, the old page rendering it; `columnLabel`; the entry; the outline; Remove; Part 7's sentences; `MAX_RUNS` 40, the rerun-first rule and `stats_desc_forget_run`; the Analyze item for alpha live.
- *Tests:* the mockup's alpha, nothing chosen; refusal in the dialog; a warning's second press; a run cancelled in the Output; 32 entries, and a 33rd refused; names surviving a close; Save refused with its sentence after a close; Export still working; `AlphaFlow.vitest`'s settings tests move, each unchanged; 32 entries rendered, timed.
- *Mutations:* a default in any field; a dialog closing the slot; an entry using the current view's names; the rerun-first rule removed; Remove leaving its record; an entry dropped at the cap; the sentence skipped.
- *Real-app check:* `realapp_8i2.md`'s runs through the dialog, every number the same; the app's memory at 32 entries, with its stop.

**Step: the six other dialogs.** The same, one method at a time inside the step, each by moving its settings. The group filter.
- *Tests:* each `*Flow.vitest`'s settings tests moved; one choice gives one spec for each; a score listed as a number and not as a group.
- *Mutations:* per method, a default chosen; a dialog closing the slot. A score listed as a group, t-test and ANOVA.
- *Real-app check:* every method on the same file, the earlier checklists' numbers the same.

**Step: the scale score's dialog.** The capability's menu live; the form; the make's new-columns profile; `PrepCode`'s sentences; the amended save sentence; the prepared CSV's route, measured; the record's recipe if question 9 says so; `rerun_over_limits`; remove a score.
- *Tests:* no default anywhere; the view's bytes equal a full profile's; 128 items rendered, timed; each code's sentence; a closed save dialog writing nothing; the second-score Save.
- *Mutations:* ADR-011 Part 14's list: a default for each of item, reversal, statistic, rule, n, code and name; the sum's sentence hidden; a verdict word; and the shared slot closed by the dialog.
- *Real-app check:* ADR-011 Part 14's: a sum and a mean on a seeded CSV against values from R, a score in alpha, correlations and a t-test, save, reopen, rerun, export and reopen the CSV, delete, and the limits at 100,000 × 50.

**Step: projects in the shell.** The line; Open a project…; the driver; the report's dialog lifted; the page lists recipes and names a score.
- *Tests:* every run in order, one line each; Stop; a refusal does not end the list and a cap does; nothing started by itself; a rerun's memory, measured.
- *Mutations:* the driver including reruns; a Stop that starts the next; a line reporting a comparison it did not get; a click running it twice.
- *Real-app check:* alpha, a score and a t-test saved in a project, reopened, all three rerun, each Identical; a report exported; Stop in the middle; a delete from the panel.

**Step: the old pages retired.** Only after Rishi has run every checklist above. The window moves to `STATISTICS_HOME_PATH`; the flag, the seven `*Page`s, their stages and `StatisticsHome`'s page go, and `statisticsRoutes` shrinks. Nothing a test still reaches goes.
- *Tests:* the routes under each flag; no import of a removed page; every moved test still named.
- *Mutations:* a route left out; the home's `place` removed.
- *Real-app check:* the whole chain, from the hub tile, in a clean app. The release note of decision 439's answer 3.

## The disk list (decision 443)

**The layout's aim is to add no write. Operations it adds that write anything: one, and it writes only what a rerun already does.** It is blocked until this ADR is approved.

| operation | writes | where | protection, deletion | blocked? | save dialog? |
|---|---|---|---|---|---|
| Run every saved result again | for each saved result: one new bundle and manifest, a run row and its edges, as a Run again click does (decision 186). The recipe and data file reused by hash | the open project, `projects/<id>/` | ADR-009 §9 to §11; deleted with the project | **blocked** | no: a click on the sentence's button |

**Existing writes it reaches, each unchanged.**

| write | path | save dialog? |
|---|---|---|
| Save to project, with the recipe when the run has one | ADR-009 §5; ADR-011's disk list. If question 9 is yes, its recipe's bytes come from the run's record and are still expected to hash to the spec's | no: a Save click |
| A single Run again | ADR-009 §6 | no: its click |
| Export CSV and manifest JSON of a result | ADR-004a, `src/utils/saveTextFile.ts:15-27` | **yes** |
| Export the prepared data (CSV) | ADR-011's disk list, `app/stats_prep_commands.rs:227`. If the route changes to a binary response, the bytes and the sentence do not | **yes** |
| Export the project report, and the PROV JSON from the page | ADR-009 §7, `app/project_commands.rs:2646`, `:2658` | **yes** |

**Never written, by this layout:** the Output, a settings line, a window of cells, the grid's cache and the session store. All are in memory. No browser storage. No autosave (decision 105).

## Alternatives considered

- **SPSS's look, or its icons.** Rejected by decision 398.
- **A window for the Output of its own, as SPSS's Viewer is.** Two windows would mean two webviews and a store shared across them, and ADR-001 keeps one active workspace per window. Decision 441 has tabs.
- **Editing cells.** Rejected: "Your file is never changed" (decision 441), and the layout is read-only. Recoding is a recipe step (ADR-011 Part 5).
- **Keeping the method pages alongside the window for good.** Two ways to do one thing, and two sets of tests and checks. The last step removes them, after the checks.
- **Rendering every row with no virtualising.** 100,000 rows at 50 columns is 5,000,000 DOM cells *(inferred to be impossible on 8 GB)*.
- **A Rust-drawn grid, or a canvas.** A canvas draws faster, but a screen reader and selection have to be rebuilt, and no such tool is approved. It is the second fallback if a library also fails.
- **A second process for the grid.** Prompt 0 forbids one with no evidence.
- **Each choice's own alternatives** are in Parts 1 to 9.

## Consequences

- The largest change to Statistics' screens since `flowHooks.ts`, across seven flows, done by moving code one method at a time.
- A user's rows reach the webview for the first time in ordinary use, in small windows. The webview's memory holds some rows while the file is open. Part 10's guards cover the paths Gaply writes; they cannot see what the operating system's accessibility or screen tools do with what is on the screen, as with anything on a screen.
- `MAX_RUNS` changes from 8 to 40, its eviction rule gains one clause, and a run can be forgotten by one small command. The 8 is decision 6's, and is stated in ADR-003 decision 4, which is Accepted, in ADR-004's 004b and in ADR-010 §4. Each needs a line when this is accepted, and none is changed now.
- Results named from the view they ran on hold that view in memory until removed or the workspace ends.
- A file closed cannot have its results saved. Part 7 says so first. This is the one thing a user may trip over, and it follows from "a Save copies the bytes the slot holds", which ADR-009 §4 and decision 123 ask for.
- Decision 433's "the slot is the only place the recipe can come from" changes, if question 9 is yes, with its mutations repeated.
- The Output's Save and Export depend on `MAX_RUNS`. So a later method with a very large result affects the same budget.
- The screens gain `Menu`, a virtual grid and `MethodDialog` in or near the design system. Compute and Design could reuse the first and third later, and nothing here requires it.
- The Project menu has fewer items than the project page. Delete, Close project and PROV stay on the page and the panel, one click away.
- ADR-011's Part 10 and question 12 stop applying on acceptance. Its Part 14's "screens" step is this ADR's score step and the rest. Decision 404's answer 2 says ADR-011's line 49 is corrected when ADR-012 settles question 12, and decision 444 says this step does not edit ADR-011. So it is not corrected here. Question 16.
- The "Coming later" labels promise nothing, but some readers will take them as a plan. That is the cost of decision 441's choice, and it is cheap to remove one.

## Evidence

**Read from the code at 6a4a3a41:** every file:line above, each printed from `git show 6a4a3a41:<path>` and compared with what the text says. The log is `~/gaply-corpus/os/phase8/8n/citations.txt`.

**Measurements quoted, each from its own phase, and none taken here:** ADR-010's 8 GB open (670 MiB, 206 MiB slot) and 8M-3's (open 626 and 1,596 MiB of increment, CSV 35 and 89 MB, wire 105 and 266 MB, decision 430). CorrFlow's render reaching 20 s under load (decision 430).

**Not measured:** nothing ran. Every budget, size, time and frame rate in Parts 1, 3, 7 and 9 is *(inferred)* or a proposal, and the steps of Part 13 measure them.

**Documentation, each read 3 Nov 2026:**

| URL | what it gave |
|---|---|
| https://www.ibm.com/docs/en/spss-statistics/31.0.0?topic=editor-data-view | Data View: "Rows are cases", "Columns are variables"; cells can be edited directly and "cannot contain formulas" |
| https://www.ibm.com/docs/en/spss-statistics/31.0.0?topic=editor-variable-view | the attributes: name, type, width, decimals, labels, missing, column width, measure; values, align and role as linked topics only; that a user can add and delete variables and modify attributes. Each attribute's editability is not stated separately |
| https://www.ibm.com/docs/en/SSLVMB_31.0.0/pdf/IBM_SPSS_Statistics_Brief_Guide.pdf | Chapter 1 (pp. 1 to 4): a variable's label on hover, value labels by a menu item, the Frequencies dialog's source and target lists with OK disabled until a variable is placed, "Results are displayed in the Viewer window", and Figure 6 of its outline beside its tables. Chapter 2, Figure 12: the tabs Overview, Data View, Variable View |
| https://raw.githubusercontent.com/TanStack/virtual/main/packages/react-virtual/package.json | `@tanstack/react-virtual` 3.14.25, MIT, peer React to 19 |
| https://bundlephobia.com/package/@tanstack/react-virtual | 33.8 kB minified, 10.1 kB gzipped |
| https://raw.githubusercontent.com/adazzle/react-data-grid/main/package.json | `react-data-grid` 7.0.0-beta.63, MIT, peer React `^19.2`, no dependencies |
| https://github.com/adazzle/react-data-grid | virtualises rows and columns, "React 19.2+", "Keyboard accessibility", no explicit screen-reader claim |

**Could not be read, 3 Nov 2026:**

- https://www.ibm.com/docs/en/spss-statistics/31.0.0?topic=output-viewer returned only a title. The Viewer's panes rest on the Brief Guide's text and Figure 6. Its hide and reorder features were reported by a search's summary of https://www.ibm.com/docs/en/SSLVMB_25.0.0/pdf/en/IBM_SPSS_Statistics_Brief_Guide.pdf and https://www.ibm.com/docs/en/SSLVMB_29.0.0/pdf/IBM_SPSS_Statistics_Brief_Guide.pdf (searched 3 Nov 2026) and were not read in either guide, so they are **not confirmed**. (The address was a guess at the page's.)
- https://www.npmjs.com/package/@tanstack/react-virtual returned HTTP 403, so `@tanstack/react-virtual`'s latest published version and its upkeep are **not confirmed**: 3.14.25 is the version in the repository's main branch.
- Upkeep and release dates of both libraries, and `react-data-grid`'s size, were not read. If either is ever proposed, its step reads them first.

## Open questions

1. **Where do Home, the Stats Check, the Stats Verifier, Settings and Switch workspace go?** Recommended: a small menu on "Gaply" in the top bar. The mockup shows that word with no menu, so this is an addition.
2. **Where is a long cell read whole?** Recommended: one line under the grid, for the active cell. A tooltip alone cannot be read by keyboard.
3. **A permanent line saying how the grid reads the data?** Recommended: yes, "As Gaply read it: numbers in full, missing cells empty, no value labels." An SPSS user who sees a blank where SPSS shows 99 should not have to ask.
4. **Copying cells?** Recommended: not in the first version. The clipboard is a route other apps can read, and it deserves its own decision.
5. **"Coming later" items with no registry entry?** Recommended: list them as plain labels, as you chose, with no date, and do not register them first. If you would rather not imply CFA and SEM are planned, remove those two: it is one line.
6. **"Tick every numeric column" and "Untick all" in the dialogs?** Recommended: keep them, as "Add every numeric column" and "Remove all". The mockup leaves them out, and decision 384 has them. They are actions the user takes.
7. **The Output's cap, and `MAX_RUNS`?** Recommended: 32 entries and 40 records, with a rerun's finished record dropped first and a Remove button that drops its record too, if the step's memory figure keeps within its stop. Below it, fewer: the step says how many fit.
8. **A way to remove a score?** Recommended: yes, in the Transform menu's dialog, though the mockup does not show it. Without it, a mistyped name stays until the file closes.
9. **Should a run keep its recipe, so a second score leaves earlier results savable?** Recommended: yes, in the score's step. It changes decision 433's comparison, and adds the record to the name sentinel's allowed surfaces in memory only. If no, the make's sentence warns instead, and a user must make all their scores before running on any.
10. **Run every saved result again in the first version?** Recommended: yes, over today's commands, with one line each and a Stop. It adds no new kind of write, and is the one place a thesis's chain is checked together.
11. **What does Open or Close do to the Output?** Recommended: keep it, after the sentence. Its results can be exported but not saved. The alternative is to drop it, which is simpler and loses a user's work on a click.
12. **Should a run on warnings need a second press?** Recommended: yes, as Part 6 says, so the mockup's one button keeps today's chance to read them.
13. **Saved results in the Output, or on the project page?** Recommended: the page, as today. A later step can bring it into the window as a fourth tab.
14. **The budgets of Part 3 and their stops?** Recommended: as listed. They are guesses until the step's run; if you want a different stop for the scroll on an 8 GB Mac, say so now.
15. **The order of the steps,** with the Output built together with the first dialog rather than after the dialogs? Recommended: yes, since a dialog's Run needs somewhere to go.
16. **ADR-011's line 49 still says a page of its own.** Decision 404 says it is corrected when ADR-012 settles this, and this step may not edit ADR-011. Recommended: correct that one line in the commit that records this ADR as accepted.

## Revision trigger

The grid's scroll, window or memory figure passing its stop; the Output's memory passing its stop; a rerun's peak with a file open passing its; a screen reader reading the grid badly enough that a different structure is needed; a real need to copy cells or edit a Variable View column; a method whose result is too large for the record's budget; a second workspace wanting the shell's menu; IBM's current documentation read and found to differ on what this ADR says of SPSS; or Rishi's first real-app check of the window.
