// Gaply — what the Statistics window says (Research OS, Phase 8P, decisions
// 441, 449 to 453). Its own file, since capabilityWording.ts has no such
// sentences and is pinned to Rust's codes (capabilityWording.ts:4-6). Fixed
// text, or text built from counts and the user's own column names, for the
// user's own screen. No verdict, no advice, no date.

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
