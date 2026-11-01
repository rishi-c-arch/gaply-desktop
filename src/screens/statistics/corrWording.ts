// Gaply — the correlations' words (Research OS, Phase 8H-2, decisions 356 and
// 359). Every code the correlations' screen can show has one sentence here,
// keyed by the closed set in corrCodes.json, which
// src-tauri/tests/stats_corr_wire.rs pins to CorrCode, RunCode, WorkspaceCode
// and CapabilityCode exactly. A refusal from the file ingest is said by
// methodsIngestWording, as on the ANOVA's screen.
//
// The tone is neutral (decision 66): a coefficient describes how two columns
// move together; nothing here passes or fails it. No sentence judges a
// p-value, and Gaply says which coefficients the user chose, never which they
// should use (decision 338). corrWording.vitest.ts reads every sentence for
// the words a verdict would use.
import { refusalOf } from '../statsverifier/l1/l1Wording';
import { methodsIngestSentence } from './methodsIngestWording';
import type { WireCode } from '../statsverifier/statsL1Types';
import { workspaceSentence } from '../workspace/workspaceWording';
import type { WorkspaceRefusal } from '../workspace/workspaceTypes';
// The run manager's ceiling and the runs it keeps, pinned to
// runs::RUN_CEILING_MS and runs::MAX_RUNS.
import LIMITS from './describeLimits.json';
// The budget, pinned to stats_corr::BUDGET_SECONDS.
import CORR_LIMITS from './corrLimits.json';
import type { Stage } from './describeWording';
import type { Bounded, Coefficient, CorrStat, MissingHandling } from './corrTypes';
import { shownNumber } from './ttestWording';

export const CORR_STAGE_HEADINGS: Record<Stage, string> = {
  import: 'Import a data file',
  profile: 'Profile of the file',
  settings: 'Choose the variables, the coefficients and how missing values are handled',
  validate: 'Check the settings',
  run: 'Run the correlations',
  results: 'Results',
  export: 'Export',
};

export const CORR_ENTRY_TEXT =
  "Correlations on your own data: Pearson's r, Spearman's rho and Kendall's tau-b between two or more numeric columns, whichever you tick, with each pair's n, p and 95% interval. The file is read on this device and nothing is sent anywhere. The data stays in memory while this screen is open and is dropped when you close the file or the app.";

/** The longest a run may be estimated to take, in seconds (decision 342). */
export const BUDGET_SECONDS: number = CORR_LIMITS.budgetSeconds;

/** How many pairs the pairs table shows at a time (decision 359). */
export const PAIRS_PAGE = 1225;

/** The most variables the matrix is drawn for (decision 359). */
export const MATRIX_MAX_VARIABLES = 20;

export const VARIABLES_QUESTION =
  'Variables. None is ticked unless you tick it; tick at least two numeric columns.';

export const COEFFICIENTS_QUESTION =
  'Coefficients. None is ticked unless you tick it; Gaply does not choose one for you. Tick at least one.';

export const COEFFICIENT_CHOICES: ReadonlyArray<{ coefficient: Coefficient; label: string; line: string }> = [
  {
    coefficient: 'pearson',
    label: "Pearson's r",
    line: 'The linear association of the values themselves; its t has n − 2 degrees of freedom.',
  },
  {
    coefficient: 'spearman',
    label: "Spearman's rho",
    line: "Pearson's r on the values' ranks, ties given their average rank; its t has n − 2 degrees of freedom.",
  },
  {
    coefficient: 'kendall',
    label: "Kendall's tau-b",
    line: 'The pairs of rows that agree in order minus those that disagree, corrected for ties; its p comes from z.',
  },
];

export const COEFFICIENT_NAMES: Record<Coefficient, string> = {
  pearson: "Pearson's r",
  spearman: "Spearman's rho",
  kendall: "Kendall's tau-b",
};

export const MISSING_QUESTION = 'Missing values. Neither is chosen unless you choose it.';

export const MISSING_CHOICES: ReadonlyArray<{ missing: MissingHandling; label: string; line: string }> = [
  {
    missing: 'pairwise',
    label: 'Pairwise',
    line: 'Each pair uses every row in which both of its columns have a value, so pairs can have different n.',
  },
  {
    missing: 'listwise',
    label: 'Listwise',
    line: 'Every pair uses only the rows in which every ticked column has a value, so every pair has the same n.',
  },
];

/** What is still missing before the settings can be checked, or null. */
export function settingsMissingSentence(variables: number, coefficients: number, missingChosen: boolean): string | null {
  const parts: string[] = [];
  if (variables < 2) parts.push(variables === 1 ? 'one more variable' : 'two variables');
  if (coefficients < 1) parts.push('a coefficient');
  if (!missingChosen) parts.push('how missing values are handled');
  if (parts.length === 0) return null;
  const list = parts.length === 1 ? parts[0] : `${parts.slice(0, -1).join(', ')} and ${parts[parts.length - 1]}`;
  return `To check the settings, choose ${list}.`;
}

/** A coefficient's button: its name and its pair count (decision 359). */
export function coefficientHeading(c: Coefficient, pairs: number): string {
  return `${COEFFICIENT_NAMES[c]}: ${pairs === 1 ? '1 pair' : `${count(pairs)} pairs`}`;
}

/** Which pairs the table shows (decision 359). */
export function pairsShownSentence(first: number, last: number, total: number): string {
  return `Pairs ${count(first)} to ${count(last)} of ${count(total)} are shown.`;
}

/** The run's progress, in what it counts (decision 355). */
export function stepsSentence(done: number, total: number): string {
  return `${done} of ${total} steps`;
}

/** A whole number with a comma between each three digits, whatever the
 *  locale. */
export function count(n: number): string {
  return String(n).replace(/\B(?=(\d{3})+(?!\d))/g, ',');
}

/** A column's name in a sentence, from the user's own profile on the user's
 *  own screen, or by position on a saved result. */
export interface CorrNames {
  column: (alias: number) => string;
}
const NO_NAMES: CorrNames = { column: (a) => `C${a}` };

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
const cells = (n: number | undefined) => (n === 1 ? '1 cell' : `${n ?? '?'} cells`);
const rows = (n: number | undefined) => (n === 1 ? '1 row' : `${n === undefined ? '?' : count(n)} rows`);
const was = (n: number | undefined) => (n === 1 ? 'was' : 'were');

type Words = (d: Coded, names: CorrNames) => string;

const LIFECYCLE_NAMES: Record<string, string> = {
  planned: 'planned',
  experimental: 'experimental',
  beta: 'in beta',
  stable: 'stable',
  deprecated: 'retired',
};

const workspace: Words = (d) => workspaceSentence({ code: d.code, params: d.params ?? {}, severity: 'error' } as WorkspaceRefusal);

/** One sentence per code. A code with no entry is shown as itself, never
 *  hidden; corrWording.vitest.ts keeps every code in corrCodes.json
 *  covered. */
export const CORR_WORDS: Record<string, Words> = {
  'stats.corr.unknown_capability': () => 'These settings name a capability Gaply does not have here.',
  'stats.corr.capability_version_mismatch': (d) =>
    `These settings were made for version ${num(d, 'found') ?? '?'} of the correlations, and this is version ${num(d, 'expected') ?? '?'}.`,
  'stats.corr.input_count': (d) => `These settings name ${num(d, 'count') ?? '?'} files, and the correlations read exactly one.`,
  'stats.corr.input_mismatch': () => 'These settings were made for a different file from the one open. Open the file again.',
  'stats.corr.unknown_column': (d, n) => `Column ${n.column(num(d, 'column') ?? 0)} is not in this file.`,
  'stats.corr.too_few_variables': (d) =>
    `${num(d, 'count') === 1 ? '1 variable is' : `${num(d, 'count') ?? '?'} variables are`} chosen, and the correlations need at least two.`,
  'stats.corr.variable_repeated': (d, n) => `Column ${n.column(num(d, 'column') ?? 0)} is chosen twice.`,
  'stats.corr.variables_out_of_order': (d, n) =>
    `These settings list column ${n.column(num(d, 'column') ?? 0)} out of the file's column order, so Gaply refuses them rather than reorder them.`,
  'stats.corr.no_coefficient': () => 'No coefficient is chosen. Tick at least one.',
  'stats.corr.coefficient_repeated': () => 'A coefficient is chosen twice.',
  'stats.corr.coefficients_out_of_order': () =>
    "These settings list the coefficients out of the order Pearson, Spearman, Kendall, so Gaply refuses them rather than reorder them.",
  'stats.corr.missing_code_not_finite': (d) => `Missing code ${(num(d, 'index') ?? 0) + 1} is not a finite number.`,
  'stats.corr.missing_code_repeated': (d) => `Missing code ${(num(d, 'index') ?? 0) + 1} is listed twice.`,
  'stats.corr.missing_codes_out_of_order': (d) =>
    `Missing code ${(num(d, 'index') ?? 0) + 1} is out of ascending order, so Gaply refuses these settings rather than reorder them.`,
  'stats.corr.variable_holds_text': (d, n) =>
    `Column ${n.column(num(d, 'column') ?? 0)} holds text in ${cells(num(d, 'count'))}, so it cannot be used as a numeric variable.`,
  'stats.corr.variable_non_finite': (d, n) =>
    `Column ${n.column(num(d, 'column') ?? 0)} holds an infinity or "not a number" in ${cells(num(d, 'count'))}, so it cannot be used as a numeric variable.`,
  'stats.corr.suspect_missing_code': (d, n) =>
    `Column ${n.column(num(d, 'column') ?? 0)} holds ${num(d, 'value') ?? '?'} in ${cells(num(d, 'count'))}, which may be a code for a missing value. It is counted as a number unless you tick it in Settings.`,
  'stats.corr.too_few_complete_rows': (d) =>
    `Listwise, ${rows(num(d, 'count'))} ${num(d, 'count') === 1 ? 'has' : 'have'} a value in every chosen column, and the correlations need at least 3. Choose fewer columns, or pairwise handling.`,
  'stats.corr.no_pair_of_three': (d) =>
    `No pair of chosen columns has 3 rows with a value in both; the most any pair has is ${rows(num(d, 'count'))}.`,
  'stats.corr.over_budget': (d) =>
    `These settings are refused before they run: ${count(num(d, 'count') ?? 0)} pairs over ${rows(num(d, 'found'))} are estimated to take longer than ${BUDGET_SECONDS} seconds on this computer. Choose fewer variables or fewer coefficients.`,
  'stats.corr.pair_too_few_rows': () => 'Not computable: this pair has fewer than 3 rows with a value in both columns.',
  'stats.corr.variable_constant': () =>
    "Not computable: a column of this pair is the same number in every one of the pair's rows.",
  'stats.corr.distribution_out_of_range': () =>
    'Not computable: this needs a distribution at more than 10,000,000 degrees of freedom, where Gaply has not measured its accuracy.',
  'stats.corr.overflow': () => 'Not computable: the arithmetic went past the largest number a double can hold.',
  'stats.corr.underflow': () => 'Not computable: a sum of squares went below the smallest number a double can hold, though the values differ.',
  'stats.corr.arithmetic_refused': () => 'Not computable: the arithmetic refused to give a value.',
  'stats.corr.interval_too_few_rows': () =>
    "Not computable: the interval's standard error divides by n − 3 (n − 4 for Kendall), and this pair has that many rows or fewer.",
  'stats.corr.no_values': () => 'Not computable: this column has no value in the rows used.',
  'stats.corr.too_few_values': () => 'Not computable: this column has fewer than two values in the rows used.',
  'stats.corr.perfect_coefficient': () =>
    'Infinite: the coefficient is exactly 1 or −1, so t has no finite value and p is 0.',
  'stats.corr.missing_value': (d, n) =>
    `${rows(d.count)} had no value (an empty cell or a missing-value token) in ${n.column(num(d, 'column') ?? 0)}, the first chosen column without one, and ${was(d.count)} left out.`,
  'stats.corr.missing_code': (d, n) =>
    `${rows(d.count)} held a missing code you ticked in ${n.column(num(d, 'column') ?? 0)}, the first chosen column without a value, and ${was(d.count)} left out.`,
  'stats.corr.fewer_than_two_values': (d) =>
    `${rows(d.count)} had a value in fewer than two chosen columns, and ${was(d.count)} left out.`,
  'stats.corr.pearson_assumes_normality': () =>
    "Pearson's p and interval assume the two columns are roughly normal together; Gaply does not check.",
  'stats.corr.two_tailed_no_partial': () => 'p values are two-tailed only, and there are no partial correlations.',
  'run.unknown_run': () => `This run is no longer held. Gaply keeps the last ${LIMITS.maxRuns} runs in memory only; run the correlations again.`,
  'run.run_busy': () => 'Another run is going in this workspace. Wait for it to end, or cancel it.',
  'run.unknown_input': () => 'This file is no longer open. Open it again.',
  'run.not_finished': () => 'This run has not ended yet.',
  'run.no_result': () => 'This run ended without a result.',
  'run.cancelled': () => 'This run was cancelled, so it has no result.',
  'run.timed_out': () => `This run passed its ${LIMITS.runCeilingMs / 1000}-second limit and was stopped, so it has no result.`,
  'run.resource_limit': () => 'This run passed a resource limit and was stopped, so it has no result.',
  'run.illegal_transition': () => 'Gaply could not change this run in that way.',
  'run.already_terminal': () => 'This run has already ended.',
  'run.wrong_kind': () => 'This run is not a correlations run, so this screen cannot read it.',
  'capability.unknown_capability': () => 'Gaply does not have this method in this version.',
  'capability.version_mismatch': (d) =>
    `These settings were made for version ${num(d, 'found') ?? '?'} of the method, and this is version ${num(d, 'expected') ?? '?'}.`,
  'capability.not_runnable': (d) => {
    const l = d.params?.lifecycle;
    return `This method is ${typeof l === 'string' ? LIFECYCLE_NAMES[l] ?? l : 'not available'}, so it cannot run in this version of Gaply.`;
  },
  'capability.confirmation_missing': () => 'This method needs a confirmation that these settings do not carry.',
  'capability.confirmation_unmatched': () => 'These settings carry a confirmation this method does not use.',
  'workspace.unavailable': workspace,
  'workspace.not_ready': workspace,
  'workspace.inactive': workspace,
  'workspace.wrong_workspace': workspace,
  'workspace.run_active': workspace,
  'workspace.illegal_transition': workspace,
};

/** The sentence for a code: a diagnostic, an exclusion or a limitation. */
export function corrSentence(d: Coded, names: CorrNames = NO_NAMES): string {
  const f = CORR_WORDS[d.code];
  return f ? f(d, names) : `Gaply has no wording for the code ${d.code}.`;
}

/** One plain sentence for a command's refusal. */
export function corrRefusalSentence(e: unknown): string {
  const c = refusalOf(e);
  if (!c) {
    const message = e instanceof Error ? e.message : typeof e === 'string' ? e : '';
    return message.trim() ? `This could not be done: ${message.trim()}` : 'This could not be done.';
  }
  if (c.code in CORR_WORDS) {
    const params = (c as { params?: unknown }).params;
    return corrSentence({ code: c.code, params: params && typeof params === 'object' ? (params as Record<string, unknown>) : {} });
  }
  return methodsIngestSentence(c as WireCode);
}

/** A candidate missing code in the ticked columns, as a question naming the
 *  value and where it appears. A ticked code applies to every chosen
 *  column. */
export function corrMissingCodeQuestion(code: number, where: ReadonlyArray<{ header: string; cells: number }>): string {
  const places = where.map((w) => `"${w.header}" (${cells(w.cells)})`).join(', ');
  return `Is ${code} a code for a missing value? It appears in ${places}. If you tick it, it is treated as missing in every chosen column.`;
}

/** Why a run that ended without a result has none. */
export function corrRunEndedSentence(state: string): string {
  switch (state) {
    case 'cancelled':
      return corrSentence({ code: 'run.cancelled' });
    case 'timed_out':
      return corrSentence({ code: 'run.timed_out' });
    case 'resource_limit':
      return corrSentence({ code: 'run.resource_limit' });
    case 'refused':
      return 'Gaply refused these settings, so nothing ran. The reasons are below.';
    case 'needs_user_input':
      return 'The settings need an answer from you, so nothing ran. See below.';
    case 'failed':
      return 'The correlations could not be computed. The reason is below.';
    default:
      return corrSentence({ code: 'run.no_result' });
  }
}

/** A number, or its sentence where its number would be (decision 359). */
export function corrStatText(s: CorrStat): string {
  return 'value' in s ? shownNumber(s.value) : corrSentence({ code: s.not_computable });
}

/** A t: six figures, or its sentence ("Infinite: …") where it is infinite. */
export function boundedText(t: Bounded): string {
  return 'value' in t ? shownNumber(t.value) : corrSentence({ code: t.infinite });
}

export const CORR_DIGITS_NOTE =
  'Numbers are shown rounded to six figures, counted from the first digit that is not zero; the CSV export keeps every digit.';

export const CORR_EXPORT_CHOICES = [
  { format: 'csv', label: 'The result tables (CSV)', file: 'correlations.csv', mime: 'text/csv' },
  { format: 'manifestJson', label: 'The run manifest (JSON)', file: 'correlations_manifest.json', mime: 'application/json' },
] as const;

export const CORR_EXPORT_TEXT =
  "Gaply writes only the file you name in the save dialog and keeps no copy. The result tables name the columns by position (C1, C2, …), never by header, with every digit and the manifest's hashes. The manifest holds hashes, versions and counts, never your data, headers or file name.";

export const CORR_ROWS_USED_NOTE =
  'Listwise, the rows with a value in every chosen column; pairwise, the rows with a value in at least two. Rows read is rows used plus the rows left out above.';
