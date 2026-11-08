// Gaply — what the Statistics window says (Research OS, Phase 8P, decisions
// 441, 449 to 453). Its own file, since capabilityWording.ts has no such
// sentences and is pinned to Rust's codes (capabilityWording.ts:4-6). Fixed
// text, or text built from counts and the user's own column names, for the
// user's own screen. No verdict, no advice, no date.
import { HIGHEST_LABEL, LOWEST_LABEL, REVERSE_LABEL } from '../alphaWording';
import { POST_HOC_CHOICES } from '../anovaWording';
import { COEFFICIENT_CHOICES, MISSING_CHOICES } from '../corrWording';
import { SCORE_NAME_MAX_CHARS, type Statistic } from '../prepTypes';
import { VARIANT_CHOICES } from '../ttestWording';

export const WINDOW_TITLE = 'Statistics';

/** Decision 441's footer. */
export const FOOTER_PHRASES = ['Statistics workspace', 'Read-only grid: your file is never changed', 'Everything stays on this computer'];

/** The line above the grid, always there (ADR-012 question 3). */
export const AS_READ_LINE = 'As Gaply read it: numbers in full, missing cells empty, no value labels.';

export const NO_PROJECT_LINE = 'Project: none open';
export const PROJECT_LINE_PREFIX = 'Project: ';

export const STATE_UNREAD = 'The workspace state could not be read.';

/** The prepared note above the grid; null for none. */
export function preparedNote(count: number): string | null {
  if (count === 0) return null;
  return count === 1 ? '1 prepared column.' : `${count} prepared columns.`;
}

/** Each tab's words. */
export const TABS = {
  data: 'Data View',
  variable: 'Variable View',
  output: (count: number) => `Output (${count})`,
};

export const OPEN_BUTTON = 'Open a data file';
/** The Data View's grid (decisions 460 and 461; 462 for the counter). */
export const GRID_LABEL = 'The data, as Gaply read it';
export const ROW_HEAD = 'Row';
export const PREPARED_WORD = 'prepared';
export const CELL_EMPTY = 'empty';
export const ACTIVE_WAITING = 'Reading this cell.';
export const GRID_UNREADABLE = 'This part of the file could not be read. Scroll away and back to try again.';

/** The active cell's place, from its row counted from 0. */
export function activeCellLabel(row: number, alias: string): string {
  return `Row ${row + 1}, ${alias}`;
}

/** The timing counter's line: three figures in ms, each named. */
export function timingSentence(f: { windowMs: number | null; paintMs: number | null; frameP95: number | null }): string {
  const ms = (v: number | null) => (v === null ? 'not read' : `${v.toFixed(1)} ms`);
  return `Window: ${ms(f.windowMs)}. First paint: ${ms(f.paintMs)}. Frames, 95th percentile: ${ms(f.frameP95)}.`;
}

export const VARIABLE_EMPTY = 'Open a data file first.';
export const OUTPUT_EMPTY = 'No output yet. Run an analysis from the Analyze menu; each result is added here, newest last.';

/** Why a menu item is disabled (ADR-012 Part 5; decision 451). */
export const NO_FILE = 'Open a data file first.';
export const RUN_GOING = 'A run is going. Wait for it, or cancel it in the Output.';
export const FILE_BUSY = 'Gaply is opening or closing a file. Wait for it.';
export const NOT_RUNNABLE = 'This version of Gaply cannot run this method.';
export const LATER_STEP = 'This comes in a later step of the build.';

/** The menus' own words: decision 441's. */
export const MENU_WORDS = {
  bar: 'Statistics menus',
  gaply: 'Gaply',
  file: 'File',
  transform: 'Transform',
  analyze: 'Analyze',
  project: 'Project',
  help: 'Help',
  comingLater: 'Coming later',
};

/** The Variable View (decision 441's columns and sentence; decision 452). */
export const VARIABLE_HEADERS = ['#', 'Alias', 'Name', 'Type', 'Distinct values', 'Empty cells', 'Possible missing codes', 'From'];
export const VARIABLE_NOTE = 'Read-only. Gaply never changes your file; a prepared column is kept as a recipe in the project.';
export const FROM_THE_FILE = 'The file';
export const FROM_A_RECIPE = 'A recipe';

export function missingCodeWords(code: number, cells: number): string {
  return `${code} (${cells} ${cells === 1 ? 'cell' : 'cells'})`;
}

/** One score's four counts, in decision 452's words. The alias and name are
 *  the column's own; everything else is a number. */
export function scoreCountsSentence(columnLabel: string, c: { scoredFromEveryItem: number; scoredFromFewer: number; notScoredTooFew: number; notScoredOverflow: number }): string {
  return (
    `${columnLabel}: ${c.scoredFromEveryItem} ${c.scoredFromEveryItem === 1 ? 'row' : 'rows'} scored from every item, ` +
    `${c.scoredFromFewer} from fewer; ${c.notScoredTooFew} not scored, too few items; ${c.notScoredOverflow} not scored, too large to hold.`
  );
}

export function nonFiniteSentence(columnLabel: string, cells: number): string {
  return cells === 1
    ? `${columnLabel} has 1 cell that is not a finite number; a method refuses this column.`
    : `${columnLabel} has ${cells} cells that are not finite numbers; a method refuses this column.`;
}

/** Help's three dialogs (decision 453). */
export const HELP_TITLES = {
  methods: 'Methods and limits',
  file: 'How Gaply reads a file',
  keyboard: 'Keyboard',
};
export const HELP_CLOSE = 'Close';
export const HOW_GAPLY_READS =
  "Gaply reads your file once and never changes it. The Data View shows it as Gaply read it. Numbers are shown in full, so 1.50 shows as 1.5. A cell your file leaves empty or marks as missing, and an SPSS user-missing value, is shown empty. SPSS's value labels are not shown. Text is shown up to 512 characters a cell. A scale score is computed from your columns and kept as a recipe; your file is not changed.";
export const KEYBOARD_LINES = [
  'Tab reaches the menu bar, and Tab again leaves it.',
  'Left and Right move between the menus.',
  'Enter opens a menu.',
  'Up and Down move between its items, and Enter chooses one.',
  'Escape closes the menu and returns to the bar.',
  'A letter jumps to the next item that starts with it.',
  'Tab enters the grid at the active cell, and Tab again leaves it.',
  'The arrow keys move the active cell one cell.',
  'Page Up and Page Down move it one screen.',
  'Home and End move it to the start and the end of its row.',
  'Cmd or Ctrl with Home and End moves it to the first and the last cell of the file.',
];

/** A column by the name the user sees (decision 470): a score by what they
 *  typed, since its header is its alias word; any other column by its header. */
export function columnLabel(c: { header: string; scoreName?: string }): string {
  return c.scoreName ?? c.header;
}

/** Alpha's settings, on the page and in the dialog (decision 470). */
export const ALPHA_COLUMNS_HEADING = 'Numeric columns';
export const ALPHA_ITEMS_HEADING = "Items, in the file's order";
export const ALPHA_ITEMS_NONE = 'None yet. Nothing is chosen for you.';
export const ALPHA_ADD_ALL = 'Add every numeric column';
export const ALPHA_REMOVE_ALL = 'Remove all';
export const ALPHA_REMOVE_ITEM = 'Remove';

/** The Output's cap, as the dialog and the menu say it (decision 471). */
export const OUTPUT_FULL = 'The Output is full. Remove a result you have kept, then run this.';
export const RUN_WORD = 'Run';
export const RUN_WITH_WARNINGS = 'Run with these warnings';
export const DIALOG_CANCEL = 'Cancel';
export const ALPHA_DIALOG_TITLE = "Reliability (Cronbach's alpha)";

/** The settings line of an alpha entry (decision 471): the spec the dialog built,
 *  in alpha's own words, each column by the name the user sees at this run. */
export function alphaSettingsLine(
  p: { items: string[]; reversed: string[]; scale: { min: number; max: number } | null; missingCodes: number[] },
  view: { profile: { columns: Array<{ alias: string; header: string; scoreName?: string }> } }
): string {
  const named = (aliases: string[]) =>
    aliases
      .map((a) => {
        const c = view.profile.columns.find((x) => x.alias === a);
        return c ? `${a} ${columnLabel(c)}` : a;
      })
      .join(', ');
  const parts = [`Items: ${named(p.items)}.`];
  if (p.reversed.length === 0) parts.push(`${REVERSE_LABEL}: none.`);
  else {
    parts.push(`${REVERSE_LABEL}: ${named(p.reversed)}.`);
    if (p.scale) parts.push(`${LOWEST_LABEL}: ${p.scale.min}. ${HIGHEST_LABEL}: ${p.scale.max}.`);
  }
  parts.push(p.missingCodes.length === 0 ? 'Missing codes: none.' : `Missing codes: ${p.missingCodes.join(', ')}.`);
  return parts.join(' ');
}

/** Before something is lost (decision 472, ADR-012 Part 7's table, word for word). */
export const ASK_CONTINUE = 'Continue';
export const ASK_CANCEL = 'Cancel';
const CLOSE_REST = 'Your results stay here and can still be exported, but those you have not saved to a project can no longer be saved. Any scores you made go with the file; only a saved run keeps their recipe.';
export const ASK_CLOSE = { title: 'Close this file', sentence: `Close this file? ${CLOSE_REST}` };
export const ASK_OPEN = { title: 'Open another file', sentence: `Open another file? ${CLOSE_REST}` };
export const ASK_LEAVE = { title: 'Leave Statistics', sentence: 'Leave Statistics? Your results and scores go when you leave, unless you saved them to a project. Exported files are not affected.' };
export const ASK_REMOVE = { title: 'Remove this result', sentence: 'Remove this result? It has not been saved to a project. It goes from the Output and from memory. Exported files are not affected.' };

/** The Output (decision 471). */
export const OUTPUT_LABEL = 'Results, in order';
export const OUTPUT_REMOVE = 'Remove';
export const OUTPUT_EXPORT = 'Export CSV…';
export const OUTPUT_SETTINGS = 'Settings: ';
export const OUTPUT_SAVED = 'Saved in the project ';
export function outputShownSentence(number: number): string {
  return `Result ${number} is shown.`;
}

/** A column by alias number, in a sentence: the name the user sees, quoted, or its
 *  alias when the view has no such column (decision 479). */
export function columnNames(view: { profile: { columns: Array<{ alias: string; header: string; scoreName?: string }> } } | null | undefined): (alias: number) => string {
  return (alias) => {
    const c = view?.profile.columns.find((col) => col.alias === `C${alias}`);
    return c ? `"${columnLabel(c)}"` : `C${alias}`;
  };
}

/** A column in a settings line: its alias and the name the user sees. */
function namedColumn(alias: string, view: { profile: { columns: Array<{ alias: string; header: string; scoreName?: string }> } }): string {
  const c = view.profile.columns.find((x) => x.alias === alias);
  return c ? `${alias} ${columnLabel(c)}` : alias;
}
const codesWords = (codes: number[]) => (codes.length === 0 ? 'Missing codes: none.' : `Missing codes: ${codes.join(', ')}.`);
type LineView = { profile: { columns: Array<{ alias: string; header: string; scoreName?: string }> } };

/** The summary's dialog (decision 482). */
export const DESCRIBE_DIALOG_TITLE = 'Descriptive statistics: Summary';
export const DESCRIBE_NOT_READY = 'To run the summary, include at least one column.';
export function describeSettingsLine(p: { columns: Array<{ alias: string; treatAs: string }>; missingCodes: number[] }, view: LineView): string {
  return `Columns: ${p.columns.map((c) => `${namedColumn(c.alias, view)} as ${c.treatAs}`).join(', ')}. ${codesWords(p.missingCodes)}`;
}

/** The independent-samples t-test's dialog (decision 483). */
export const TTEST_DIALOG_TITLE = 'Compare means: Independent-samples t-test';
export function ttestSettingsLine(
  p: { outcome: string; group: string; levels: [number, number] | null; variant: string | null; missingCodes: number[] },
  view: LineView,
  levels: ReadonlyArray<{ index: number; label: string }>
): string {
  const level = (i: number) => levels.find((l) => l.index === i)?.label ?? `level ${i + 1}`;
  const parts = [`Outcome: ${namedColumn(p.outcome, view)}.`, `Group: ${namedColumn(p.group, view)}.`];
  if (p.levels) parts.push(`Group 1: ${level(p.levels[0])}. Group 2: ${level(p.levels[1])}.`);
  const variant = VARIANT_CHOICES.find((c) => c.variant === p.variant);
  if (variant) parts.push(`Test: ${variant.label}.`);
  parts.push(codesWords(p.missingCodes));
  return parts.join(' ');
}

/** The one-sample and paired t-tests' dialogs (decision 483). */
export const ONE_SAMPLE_DIALOG_TITLE = 'Compare means: One-sample t-test';
export const PAIRED_DIALOG_TITLE = 'Compare means: Paired t-test';
export function oneSampleSettingsLine(p: { outcome: string; testValue: number | null; missingCodes: number[] }, view: LineView): string {
  return `Column: ${namedColumn(p.outcome, view)}. Test value: ${p.testValue}. ${codesWords(p.missingCodes)}`;
}
export function pairedSettingsLine(p: { first: string; second: string; missingCodes: number[] }, view: LineView): string {
  return `First column: ${namedColumn(p.first, view)}. Second column: ${namedColumn(p.second, view)}. ${codesWords(p.missingCodes)}`;
}

/** The ANOVA's and the correlations' dialogs (decision 484). */
export const ANOVA_DIALOG_TITLE = 'Compare means: One-way ANOVA';
export const CORR_DIALOG_TITLE = 'Correlate: Correlations';
export function anovaSettingsLine(
  p: { outcome: string; group: string; levels: number[] | null; postHoc: string[]; missingCodes: number[] },
  view: LineView,
  levels: ReadonlyArray<{ index: number; label: string }>
): string {
  const level = (i: number) => levels.find((l) => l.index === i)?.label ?? `level ${i + 1}`;
  const tests = p.postHoc.map((t) => POST_HOC_CHOICES.find((c) => c.test === t)?.label ?? t);
  return [
    `Outcome: ${namedColumn(p.outcome, view)}.`,
    `Group: ${namedColumn(p.group, view)}.`,
    `Levels: ${p.levels === null ? 'all' : p.levels.map(level).join(', ')}.`,
    `Post hoc: ${tests.length === 0 ? 'none' : tests.join(', ')}.`,
    codesWords(p.missingCodes),
  ].join(' ');
}
export function corrSettingsLine(p: { variables: string[]; coefficients: string[]; missing: string; missingCodes: number[] }, view: LineView): string {
  const coefficients = p.coefficients.map((c) => COEFFICIENT_CHOICES.find((x) => x.coefficient === c)?.label ?? c);
  const handling = MISSING_CHOICES.find((m) => m.missing === p.missing)?.label ?? p.missing;
  return `Variables: ${p.variables.map((a) => namedColumn(a, view)).join(', ')}. Coefficients: ${coefficients.join(', ')}. Missing values: ${handling}. ${codesWords(p.missingCodes)}`;
}

/** The scale score's dialog (decisions 504 to 507). Its form's words, Make's
 *  sentence, the scores it lists and the questions before a deletion. A score's
 *  name is the user's own, shown on their own screen. "Delete", never "Remove",
 *  so the item list's Remove all and a deletion cannot be confused. */
export const SCORE_DIALOG_TITLE = 'Scale score';
export const SCORE_NAME_LABEL = 'Name of the score';
export const SCORE_ITEMS_LEGEND = "Items. None is chosen unless you choose it; choose at least two of the file's numeric columns.";
export const SCORE_SCALE_LEGEND = "The scale's ends";
export const SCORE_STATISTIC_LEGEND = 'The score is the';
export const SCORE_STATISTIC_CHOICES: ReadonlyArray<{ statistic: Statistic; label: string }> = [
  { statistic: 'sum', label: 'sum of the items' },
  { statistic: 'mean', label: 'mean of the items' },
];
export const SCORE_RULE_LEGEND = 'A person gets a score when';
export const SCORE_RULE_EVERY = 'every item has a value';
export const SCORE_RULE_AT_LEAST = 'at least this many items have one';
export const SCORE_AT_LEAST_LABEL = 'Number of items';
export const SCORE_CODES_LEGEND = 'Candidate missing codes';
export const SCORE_MAKE = 'Make the column';
export const SCORE_HELD_READING = 'Reading the scores this file holds.';

/** Each piece Make waits for, in the form's order. */
export const SCORE_MISSING = {
  name: 'a name for the score',
  nameLong: `a name of at most ${SCORE_NAME_MAX_CHARS} characters`,
  twoItems: 'two items',
  oneMoreItem: 'one more item',
  lowest: 'a number for the lowest value on the scale',
  highest: 'a number for the highest value on the scale',
  statistic: 'the sum or the mean',
  rule: 'when a person gets a score',
  atLeast: (top: number) => `a whole number of items from 1 to ${top}`,
};

/** Everything still missing, in one sentence; null for nothing. */
export function scoreMissingSentence(parts: readonly string[]): string | null {
  if (parts.length === 0) return null;
  const list = parts.length === 1 ? parts[0] : `${parts.slice(0, -1).join(', ')} and ${parts[parts.length - 1]}`;
  return `To make the column, choose ${list}.`;
}

export const SCORES_HEADING = 'Scores in this file';
export const SCORE_DELETE = 'Delete';
export const SCORE_DELETE_ALL = 'Delete every score';
export function scoreDeleteLabel(name: string): string {
  return `Delete the score "${name}"`;
}
export const ASK_DELETE_SCORE = {
  title: 'Delete this score',
  sentence: (name: string) =>
    `Delete the score "${name}"? It is deleted from the table. The file on disk is not changed, results already in the Output keep it, and you can make it again.`,
};
export const ASK_DELETE_ALL = {
  title: 'Delete every score',
  sentence: 'Delete every score? Each is deleted from the table. The file on disk is not changed, results already in the Output keep them, and you can make them again.',
};

/** sw-message after a make: the score's four counts, by its name. */
export function scoreMadeSentence(name: string, c: { scoredFromEveryItem: number; scoredFromFewer: number; notScoredTooFew: number; notScoredOverflow: number }): string {
  return `The score was made. ${scoreCountsSentence(`"${name}"`, c)}`;
}
export function scoreDeletedSentence(name: string): string {
  return `The score "${name}" was deleted.`;
}
export const SCORES_DELETED = 'Every score was deleted.';

/** File, Export the prepared data (CSV)… (decision 514; ADR-011 Part 8 and
 *  ADR-012 Part 5). Why it cannot be chosen on a file with no score, ADR-012
 *  Part 5's sentence; the question before it, ADR-011 Part 8's sentence word for
 *  word under the title decision 519 gives; and the name the save dialog
 *  suggests, fixed, carrying no file name. */
export const NO_SCORE = 'Make a scale score first.';
export const ASK_EXPORT_PREPARED = {
  title: 'Export the prepared data',
  sentence:
    "This file holds your data as Gaply read it, with your scores added. Missing answers are empty, SPSS's value labels are not included, and numbers are written in full. Your original file is not changed.",
};
export const EXPORT_PREPARED_NAME = 'prepared_data.csv';

/** Project, Open a project… (decision 525): its dialog's title, fixed text. */
export const OPEN_PROJECT_TITLE = 'Open a project';
/** Project, Export the project report… with no project open (decision 526;
 *  ADR-012 Part 5), word for word. */
export const NO_PROJECT_OPEN = 'Open a project first.';

/** Project, Run every saved result again… (decisions 537 and 546; ADR-012
 *  Part 9), each word for word. The list's lines are built by rerunAll.ts from
 *  these and the project page's own words. */
export const RERUN_ALL_TITLE = 'Run every saved result again';
export function rerunAllSentence(n: number): string {
  return n === 1
    ? 'Run the 1 saved result again? Gaply runs it, with the same settings, on the copy of the data file kept in this project. The new run is saved beside the one it repeats, which does not change.'
    : `Run all ${n} saved results again? Gaply runs each one, in the order they were saved, with the same settings, on the copy of the data file kept in this project. Each new run is saved beside the one it repeats, which does not change.`;
}
export const RERUN_ALL_PREPARED = 'For a result made on data with scale scores, Gaply first makes the scores again from the recipe kept in this project.';
export const RERUN_ALL_LEFT_OUT = 'Runs of an earlier result, and results that did not finish, are not included.';
/** Written by decision 546: twenty reruns beside a held file peaked at 871 MiB,
 *  over 837.5, and 709 MiB with no file held, 18.6% lower (8U-2 Part 1b). */
export const RERUN_ALL_MEMORY =
  "A data file is open in this window, and each run holds its own copy of its data in memory beside it. Closing the file first uses less of this computer's memory.";
export const RERUN_ALL_GOING_NOTE = 'The runs go on if you close this. Stop cancels the one running and starts no more.';
export const RERUN_ALL_STOP = 'Stop';
export const RERUN_ALL_NOT_RUN = 'It was not run again.';
export const RERUN_ALL_FAILED = 'The new run failed. Its reason is saved with it.';
export const RERUN_ALL_STOPPED = 'Stopped.';
export const RERUN_ALL_REST = 'The rest were not run again.';
export const RERUNS_GOING = 'Saved results are being run again. Wait for them, or stop them from the Project menu.';

/** What became of each run of the list: every run takes exactly one. */
export interface RerunAllCounts {
  identical: number;
  different: number;
  notCompared: number;
  failed: number;
  savedNotRead: number;
  notSaved: number;
  notKnown: number;
  notRun: number;
  notStarted: number;
}
const COUNT_WORDS: ReadonlyArray<[keyof RerunAllCounts, string]> = [
  ['identical', 'identical'],
  ['different', 'not identical'],
  ['notCompared', 'not compared'],
  ['failed', 'failed'],
  ['savedNotRead', 'saved but not read'],
  ['notSaved', 'not saved'],
  ['notKnown', 'not known'],
  ['notRun', 'not run'],
  ['notStarted', 'not started'],
];
/** The list's last line: the total, then each count above zero, in order. */
export function rerunAllCount(total: number, counts: RerunAllCounts): string {
  const head = total === 1 ? '1 saved result: ' : `${total} saved results: `;
  return `${head}${COUNT_WORDS.filter(([k]) => counts[k] > 0)
    .map(([k, w]) => `${counts[k]} ${w}`)
    .join(', ')}.`;
}
