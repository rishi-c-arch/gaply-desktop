// Gaply — crosstabs' words (Research OS, Phase 8X-2, decisions 588 and 589).
// Every code the crosstabs card and dialog can show has one sentence here, keyed
// by the closed set in xtabCodes.json, which src-tauri/tests/stats_xtab_wire.rs
// pins to XtabCode, RunCode, WorkspaceCode and CapabilityCode exactly. The run's,
// the registry's and the workspace's sentences are the ANOVA's (anovaWording.ts:141),
// but the two that name the method (:203, :213), renamed here as the correlations'
// and alpha's are. A refusal from the file ingest is said by methodsIngestWording.
//
// The tone is neutral (decision 66): a crosstab says what the counts are; nothing
// here passes or fails an association. No sentence judges a p value or names a
// threshold for one. The progress shows no steps sentence: a crosstab has one check
// point, and "0 of 1 steps" says nothing. xtabWording.vitest.ts reads every
// sentence for the words a verdict would use.
import { refusalOf } from '../statsverifier/l1/l1Wording';
import { methodsIngestSentence } from './methodsIngestWording';
import type { WireCode } from '../statsverifier/statsL1Types';
// The run manager's runs kept, pinned to runs::MAX_RUNS.
import LIMITS from './describeLimits.json';
import { ANOVA_WORDS, MAX_GROUP_LEVELS } from './anovaWording';
import type { XtabStat } from './xtabTypes';
import { shownNumber } from './ttestWording';

/** A column's name in a sentence, from the user's own profile on the user's own
 *  screen. A level is never named in a sentence: the codes carry no level. */
export interface XtabNames {
  column: (alias: number) => string;
}
const NO_NAMES: XtabNames = { column: (a) => `C${a}` };

/** A code with its parameters, and an exclusion's row count. */
export interface Coded {
  code: string;
  params?: Record<string, unknown>;
  count?: number;
}

const num = (d: Coded, k: string): number | undefined => {
  const v = d.params?.[k];
  return typeof v === 'number' ? v : undefined;
};
const cells = (n: number | undefined) => (n === 1 ? '1 cell' : `${n ?? 0} cells`);
const rows = (n: number | undefined) => (n === 1 ? '1 row' : `${n ?? 0} rows`);
const levels = (n: number | undefined) => (n === 1 ? '1 level' : `${n ?? 0} levels`);
const was = (n: number | undefined) => (n === 1 ? 'was' : 'were');
const column = (d: Coded, n: XtabNames) => n.column(num(d, 'column') ?? 0);

type Words = (d: Coded, names: XtabNames) => string;

/** The ANOVA's sentence for a code that names no method. */
const shared = (code: string): Words => ANOVA_WORDS[code] as unknown as Words;

/** One sentence per code. A code with no entry is shown as itself, never
 *  hidden; xtabWording.vitest.ts keeps every code in xtabCodes.json covered. */
export const XTAB_WORDS: Record<string, Words> = {
  'stats.xtab.unknown_capability': () => 'These settings name a capability Gaply does not have here.',
  'stats.xtab.capability_version_mismatch': (d) =>
    `These settings were made for version ${num(d, 'found') ?? 0} of crosstabs, and this is version ${num(d, 'expected') ?? 0}.`,
  'stats.xtab.input_count': (d) => `These settings name ${num(d, 'count') ?? 0} files, and crosstabs reads exactly one.`,
  'stats.xtab.input_mismatch': () => 'These settings were made for a different file from the one open. Open the file again.',
  'stats.xtab.preparation_mismatch': () => 'These settings do not match how the open data was prepared. Choose the settings again.',
  'stats.xtab.missing_code_not_finite': (d) => `Missing code ${(num(d, 'index') ?? 0) + 1} is not a finite number.`,
  'stats.xtab.missing_code_repeated': (d) => `Missing code ${(num(d, 'index') ?? 0) + 1} is listed twice.`,
  'stats.xtab.missing_codes_out_of_order': (d) =>
    `Missing code ${(num(d, 'index') ?? 0) + 1} is out of ascending order, so Gaply refuses these settings rather than reorder them.`,
  'stats.xtab.unknown_column': (d, n) => `Column ${column(d, n)} is not in this file.`,
  'stats.xtab.same_column': () => 'The rows and the columns are the same column. Choose two different columns.',
  'stats.xtab.score_as_column': (d, n) =>
    `Column ${column(d, n)} is a score, so it cannot be the rows or the columns of a table. Choose a column from your file.`,
  'stats.xtab.too_few_levels': (d, n) =>
    `Column ${column(d, n)} has ${levels(num(d, 'count'))} among the rows included, and a table needs at least two in each column.`,
  'stats.xtab.too_many_levels': (d, n) =>
    `Column ${column(d, n)} has ${levels(num(d, 'count'))} among the rows included, more than the ${num(d, 'limit') ?? MAX_GROUP_LEVELS} a table takes in each column.`,
  'stats.xtab.suspect_missing_code': (d, n) =>
    `Column ${column(d, n)} holds ${num(d, 'value') ?? 0} in ${cells(num(d, 'count'))}, which may be a code for a missing value. It is counted as a level unless you tick it in Settings.`,
  'stats.xtab.overflow': () => 'Not computable: the arithmetic went past the largest number a double can hold.',
  'stats.xtab.arithmetic_refused': () => 'Not computable: the arithmetic refused to give a value.',
  'stats.xtab.distribution_out_of_range': () =>
    'Not computable: this needs the chi-square distribution at more than 2,401 degrees of freedom, where Gaply has not measured its accuracy.',
  'stats.xtab.expected_below_five': () =>
    "Some expected counts are below 5, where the chi-square p values rest on a rougher approximation; for a table of two rows and two columns, Fisher's exact test does not rely on it.",
  'stats.xtab.missing_row': (d, n) =>
    `${rows(d.count)} had no value (an empty cell or a missing-value token) in ${column(d, n)}, the rows' column, and ${was(d.count)} left out.`,
  'stats.xtab.missing_column': (d, n) =>
    `${rows(d.count)} had no value (an empty cell or a missing-value token) in ${column(d, n)}, the columns' column, and ${was(d.count)} left out.`,
  'stats.xtab.missing_code': (d, n) => `${rows(d.count)} held a missing code you ticked in ${column(d, n)}, and ${was(d.count)} left out.`,
  'stats.xtab.no_layers': () => 'Crosstabs takes two columns; a third, layer column is not offered.',
  'stats.xtab.fisher_two_by_two_only': () =>
    "Fisher's exact test is given for a table of two rows and two columns only; no exact test is given for a larger table.",
  'stats.xtab.no_further_nominal_measures': () =>
    "Lambda, Goodman and Kruskal's tau, the uncertainty coefficient, the linear-by-linear test and the measures for ordered levels are not given.",
  'stats.xtab.no_weights': () => 'Each row is counted once; weighting rows by a column is not offered.',
  'stats.xtab.code_point_order': () =>
    "Levels are listed in the order Gaply reads them, so a column of numbers lists 10 before 2; the sign of phi and the direction of Fisher's one-sided tests follow that order.",
  'run.unknown_run': () => `This run is no longer held. Gaply keeps the last ${LIMITS.maxRuns} runs in memory only; run crosstabs again.`,
  'run.run_busy': shared('run.run_busy'),
  'run.unknown_input': shared('run.unknown_input'),
  'run.not_finished': shared('run.not_finished'),
  'run.no_result': shared('run.no_result'),
  'run.cancelled': shared('run.cancelled'),
  'run.timed_out': shared('run.timed_out'),
  'run.resource_limit': shared('run.resource_limit'),
  'run.illegal_transition': shared('run.illegal_transition'),
  'run.already_terminal': shared('run.already_terminal'),
  'run.wrong_kind': () => 'This run is not a crosstabs run, so this screen cannot read it.',
  'capability.unknown_capability': shared('capability.unknown_capability'),
  'capability.version_mismatch': shared('capability.version_mismatch'),
  'capability.not_runnable': shared('capability.not_runnable'),
  'capability.confirmation_missing': shared('capability.confirmation_missing'),
  'capability.confirmation_unmatched': shared('capability.confirmation_unmatched'),
  'workspace.unavailable': shared('workspace.unavailable'),
  'workspace.not_ready': shared('workspace.not_ready'),
  'workspace.inactive': shared('workspace.inactive'),
  'workspace.wrong_workspace': shared('workspace.wrong_workspace'),
  'workspace.run_active': shared('workspace.run_active'),
  'workspace.illegal_transition': shared('workspace.illegal_transition'),
};

/** The sentence for a code: a diagnostic, an exclusion, a note or a limitation. */
export function xtabSentence(d: Coded, names: XtabNames = NO_NAMES): string {
  const f = XTAB_WORDS[d.code];
  return f ? f(d, names) : `Gaply has no wording for the code ${d.code}.`;
}

/** One plain sentence for a command's refusal. */
export function xtabRefusalSentence(e: unknown): string {
  const c = refusalOf(e);
  if (!c) {
    const message = e instanceof Error ? e.message : typeof e === 'string' ? e : '';
    return message.trim() ? `This could not be done: ${message.trim()}` : 'This could not be done.';
  }
  if (c.code in XTAB_WORDS) {
    const params = (c as { params?: unknown }).params;
    return xtabSentence({ code: c.code, params: params && typeof params === 'object' ? (params as Record<string, unknown>) : {} });
  }
  return methodsIngestSentence(c as WireCode);
}

/** Why a run that ended without a result has none. */
export function xtabRunEndedSentence(state: string): string {
  switch (state) {
    case 'cancelled':
      return xtabSentence({ code: 'run.cancelled' });
    case 'timed_out':
      return xtabSentence({ code: 'run.timed_out' });
    case 'resource_limit':
      return xtabSentence({ code: 'run.resource_limit' });
    case 'refused':
      return 'Gaply refused these settings, so nothing ran. The reasons are below.';
    case 'needs_user_input':
      return 'The settings need an answer from you, so nothing ran. See below.';
    case 'failed':
      return 'The crosstab could not be computed. The reason is below.';
    default:
      return xtabSentence({ code: 'run.no_result' });
  }
}

/* ---- the dialog ----------------------------------------------------------- */

/** Decision 575: how levels are read and ordered. */
export const XTAB_LEVELS_RULE =
  "Levels follow Level 1's reading of cells: 1 and 1.0 are one level, and text is trimmed of spaces at either end. Each column's levels are those of the rows included, listed in the order Gaply reads them, so a column of numbers lists 10 before 2.";

export const XTAB_ROWS_LABEL = `Rows (a column with 2 to ${MAX_GROUP_LEVELS} levels)`;
export const XTAB_COLUMNS_LABEL = `Columns (a column with 2 to ${MAX_GROUP_LEVELS} levels)`;
export const XTAB_COLUMN_LIMIT_SENTENCE = `A column with more than ${MAX_GROUP_LEVELS} distinct values is not offered.`;

/** A candidate missing code in either column, as a question naming the value
 *  and where it appears. A ticked code applies to both columns. */
export function xtabMissingCodeQuestion(code: number, where: ReadonlyArray<{ header: string; cells: number }>): string {
  const places = where.map((w) => `"${w.header}" (${cells(w.cells)})`).join(', ');
  return `Is ${code} a code for a missing value? It appears in ${places}. If you tick it, it is treated as missing in both columns.`;
}

/* ---- the card ------------------------------------------------------------- */

/** The statistic a cell shows, one at a time. */
export type CellStatistic = 'count' | 'expected' | 'row_percent' | 'column_percent' | 'total_percent' | 'residual' | 'pearson_residual' | 'adjusted_residual';

/** The card's switch, in decision 589's words, the count first. */
export const CELL_STATISTICS: ReadonlyArray<{ stat: CellStatistic; label: string }> = [
  { stat: 'count', label: 'Count' },
  { stat: 'expected', label: 'Expected count' },
  { stat: 'row_percent', label: '% of row' },
  { stat: 'column_percent', label: '% of column' },
  { stat: 'total_percent', label: '% of total' },
  { stat: 'residual', label: 'Residual' },
  { stat: 'pearson_residual', label: 'Pearson residual' },
  { stat: 'adjusted_residual', label: 'Adjusted residual' },
];

export const XTAB_SWITCH_LABEL = 'Which cell statistic to show';
export const XTAB_TOTAL = 'Total';

/** The table's caption: what each cell shows. */
export function xtabTableCaption(label: string): string {
  return `The table: ${label} in each cell`;
}

/** The tests' rows, in decision 589's words. */
export const XTAB_TEST_LABELS = {
  pearson: 'Pearson chi-square',
  likelihood: 'Likelihood ratio',
  continuity: 'Continuity correction',
  fisherTwoSided: "Fisher's exact test (two-sided)",
  fisherLess: "Fisher's exact test (less)",
  fisherGreater: "Fisher's exact test (greater)",
} as const;

export const XTAB_TESTS_HEADING = 'Tests';
export const XTAB_TEST_COLUMNS = ['Test', 'Value', 'df', 'p'] as const;

/** The measures, in decision 589's words. */
export const XTAB_MEASURE_LABELS = {
  phi: 'Phi',
  cramersV: "Cramer's V",
  contingency: 'Contingency coefficient',
} as const;

/** Decision 589: shown always, 0 included. */
export function xtabCellsBelowFive(k: number, cellCount: number, smallest: number): string {
  return `${k} of ${cellCount} cells have an expected count below 5; the smallest is ${shownNumber(smallest)}.`;
}

/** A number, or its sentence where its number would be. */
export function xtabStatText(s: XtabStat): string {
  return 'value' in s ? shownNumber(s.value) : xtabSentence({ code: s.not_computable });
}

export const XTAB_DIGITS_NOTE =
  'Numbers are shown rounded to six figures, counted from the first digit that is not zero; the CSV export keeps every digit.';

export const XTAB_EXPORT_HEADING = 'Export';

export const XTAB_EXPORT_CHOICES = [
  { format: 'csv', label: 'The result tables (CSV)', file: 'crosstabs.csv', mime: 'text/csv' },
  { format: 'manifestJson', label: 'The run manifest (JSON)', file: 'crosstabs_manifest.json', mime: 'application/json' },
] as const;

export const XTAB_EXPORT_TEXT =
  "Gaply writes only the file you name in the save dialog and keeps no copy. The result tables carry the level labels with the result, every digit, and the manifest's hashes. The manifest holds hashes, versions and counts, never your data, headers, level labels or file name.";

/** Rows used is n, so the three counts add up. */
export const XTAB_ROWS_USED_NOTE = 'The rows counted in the table. Rows read is rows used plus the rows left out above.';
