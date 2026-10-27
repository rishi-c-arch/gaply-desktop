// Gaply — the one-sample and paired-samples t-tests' words (Research OS,
// Phase 8D-2b, decision 268). Every code either test's screen can show has
// one sentence here, keyed by the closed set in onesampleCodes.json, which
// src-tauri/tests/stats_onesample_wire.rs pins to OneSampleCode, RunCode,
// WorkspaceCode and CapabilityCode exactly. The paired test's codes are
// worded now, for its screen (8D-2c). A refusal from the file ingest is said
// by methodsIngestWording, as on the t-test's screen.
//
// The tone is neutral (decision 66): a t-test describes a difference, it
// does not pass or fail it. No sentence here judges a p-value, and none uses
// the words a verdict would; onesampleWording.vitest.ts reads every one.
import { refusalOf } from '../statsverifier/l1/l1Wording';
import { methodsIngestSentence } from './methodsIngestWording';
import type { WireCode } from '../statsverifier/statsL1Types';
import { workspaceSentence } from '../workspace/workspaceWording';
import type { WorkspaceRefusal } from '../workspace/workspaceTypes';
// The run manager's ceiling and the runs it keeps, pinned to
// runs::RUN_CEILING_MS and runs::MAX_RUNS.
import LIMITS from './describeLimits.json';
import type { Stage } from './describeWording';

export const ONE_SAMPLE_STAGE_HEADINGS: Record<Stage, string> = {
  import: 'Import a data file',
  profile: 'Profile of the file',
  settings: 'Choose the column and the test value',
  validate: 'Check the settings',
  run: 'Run the t-test',
  results: 'Results',
  export: 'Export',
};

export const ONE_SAMPLE_ENTRY_TEXT =
  'A one-sample t-test on your own data: the difference between the mean of a numeric column and a test value you give. The file is read on this device and nothing is sent anywhere. The data stays in memory while this screen is open and is dropped when you close the file or the app.';

/** The test value's field and the line beside it (decision 268). */
export const TEST_VALUE_LABEL = 'Test value';
export const TEST_VALUE_LINE = 'The mean difference is the mean minus the test value.';

/** A column's name in a sentence, from the user's own profile on the user's
 *  own screen. */
export interface OneSampleNames {
  column: (alias: number) => string;
}
const NO_NAMES: OneSampleNames = { column: (a) => `C${a}` };

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
const rows = (n: number | undefined) => (n === 1 ? '1 row' : `${n ?? '?'} rows`);
const valuesLeft = (n: number | undefined) => (n === 1 ? '1 value remains' : `${n ?? '?'} values remain`);
const pairsLeft = (n: number | undefined) => (n === 1 ? '1 pair remains' : `${n ?? '?'} pairs remain`);
/** The verb agrees with the rows counted. */
const was = (n: number | undefined) => (n === 1 ? 'was' : 'were');

type Words = (d: Coded, names: OneSampleNames) => string;

const LIFECYCLE_NAMES: Record<string, string> = {
  planned: 'planned',
  experimental: 'experimental',
  beta: 'in beta',
  stable: 'stable',
  deprecated: 'retired',
};

const workspace: Words = (d) => workspaceSentence({ code: d.code, params: d.params ?? {}, severity: 'error' } as WorkspaceRefusal);

/** One sentence per code. A code with no entry is shown as itself, never
 *  hidden; onesampleWording.vitest.ts keeps every code in onesampleCodes.json
 *  covered. */
export const ONE_SAMPLE_WORDS: Record<string, Words> = {
  'stats.onesample.unknown_capability': () => 'These settings name a capability Gaply does not have here.',
  'stats.onesample.capability_version_mismatch': (d) =>
    `These settings were made for version ${num(d, 'found') ?? '?'} of the t-test, and this is version ${num(d, 'expected') ?? '?'}.`,
  'stats.onesample.input_count': (d) => `These settings name ${num(d, 'count') ?? '?'} files, and the t-test reads exactly one.`,
  'stats.onesample.input_mismatch': () => 'These settings were made for a different file from the one open. Open the file again.',
  'stats.onesample.unknown_column': (d, n) => `Column ${n.column(num(d, 'column') ?? 0)} is not in this file.`,
  'stats.onesample.same_column': () => 'The first and the second column are the same column. Choose two different columns.',
  'stats.onesample.column_holds_text': (d, n) =>
    `The column ${n.column(num(d, 'column') ?? 0)} holds text in ${cells(num(d, 'count'))}, so it cannot be used as numbers.`,
  'stats.onesample.column_non_finite': (d, n) =>
    `The column ${n.column(num(d, 'column') ?? 0)} holds an infinity or "not a number" in ${cells(num(d, 'count'))}, so it cannot be used as numbers.`,
  'stats.onesample.missing_code_not_finite': (d) => `Missing code ${(num(d, 'index') ?? 0) + 1} is not a finite number.`,
  'stats.onesample.test_value_not_chosen': () =>
    'Type the test value in Settings: the value the mean is compared with. Gaply does not choose one for you.',
  'stats.onesample.test_value_not_finite': () => 'The test value is not a finite number. Type a number in Settings.',
  'stats.onesample.suspect_missing_code': (d, n) =>
    `The column ${n.column(num(d, 'column') ?? 0)} holds ${num(d, 'value') ?? '?'} in ${cells(num(d, 'count'))}, which may be a code for a missing value. It is counted as a number unless you tick it in Settings.`,
  'stats.onesample.too_few_values': (d) => `After the exclusions ${valuesLeft(num(d, 'count'))}, and the t-test needs at least 2.`,
  'stats.onesample.too_few_pairs': (d) => `After the exclusions ${pairsLeft(num(d, 'count'))}, and the paired t-test needs at least 2.`,
  'stats.onesample.values_constant': () => 'Every value is the same, so the standard error is exactly 0 and t is not defined.',
  'stats.onesample.differences_constant': () =>
    'Every pair differs by the same amount, so the standard error of the differences is exactly 0 and t is not defined.',
  'stats.onesample.overflow': () => 'The arithmetic went past the largest number a double can hold.',
  'stats.onesample.underflow': () => 'The arithmetic went below the smallest number a double can hold, though the values differ.',
  'stats.onesample.arithmetic_refused': () => 'The arithmetic refused to give a value.',
  'stats.onesample.quantile_out_of_range': () =>
    'The interval needs a t quantile at more than 10,000 degrees of freedom, where Gaply has not measured its accuracy, so nothing was computed.',
  'stats.onesample.missing_value': (d) =>
    `${rows(d.count)} had no value (an empty cell or a missing-value token), and ${was(d.count)} left out.`,
  'stats.onesample.missing_first': (d) =>
    `${rows(d.count)} had no first value (an empty cell or a missing-value token), and ${was(d.count)} left out.`,
  'stats.onesample.missing_second': (d) =>
    `${rows(d.count)} had no second value (an empty cell or a missing-value token), and ${was(d.count)} left out.`,
  'stats.onesample.missing_code': (d) => `${rows(d.count)} held a missing code you ticked, and ${was(d.count)} left out.`,
  'stats.onesample.values_normality_not_assessed': () =>
    'Normality is not assessed: the t-test assumes the values come from a roughly normal distribution, or that there are many of them, and Gaply does not check either.',
  'stats.onesample.differences_normality_not_assessed': () =>
    'Normality is not assessed: the paired t-test assumes the differences come from a roughly normal distribution, or that there are many pairs, and Gaply does not check either.',
  'stats.onesample.cohens_d_has_no_interval': () => "Cohen's d is shown without a confidence interval: Gaply does not compute one for d yet.",
  // Decision 259's sentences, word for word.
  'stats.onesample.values_essentially_constant': () =>
    'The values vary so little, compared with their size, that the standard error is lost in rounding, so t is not computed. R refuses such data too: "data are essentially constant".',
  'stats.onesample.differences_essentially_constant': () =>
    'The differences vary so little, compared with the size of the values, that the standard error is lost in rounding, so t is not computed. This happens when every pair differs by the same amount, such as 0.1, which a computer stores with a tiny rounding error.',
  'run.unknown_run': () => `This run is no longer held. Gaply keeps the last ${LIMITS.maxRuns} runs in memory only; run the t-test again.`,
  'run.run_busy': () => 'Another run is going in this workspace. Wait for it to end, or cancel it.',
  'run.unknown_input': () => 'This file is no longer open. Open it again.',
  'run.not_finished': () => 'This run has not ended yet.',
  'run.no_result': () => 'This run ended without a result.',
  'run.cancelled': () => 'This run was cancelled, so it has no result.',
  'run.timed_out': () => `This run passed its ${LIMITS.runCeilingMs / 1000}-second limit and was stopped, so it has no result.`,
  'run.resource_limit': () => 'This run passed a resource limit and was stopped, so it has no result.',
  'run.illegal_transition': () => 'Gaply could not change this run in that way.',
  'run.already_terminal': () => 'This run has already ended.',
  'run.wrong_kind': () => 'This run is not of this test, so this screen cannot read it.',
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
export function oneSampleSentence(d: Coded, names: OneSampleNames = NO_NAMES): string {
  const f = Object.prototype.hasOwnProperty.call(ONE_SAMPLE_WORDS, d.code) ? ONE_SAMPLE_WORDS[d.code] : undefined;
  return f ? f(d, names) : `Gaply has no wording for the code ${d.code}.`;
}

/** One plain sentence for a command's refusal: the run manager's, the
 *  registry's and the workspace's codes in these words, the ingest's in
 *  methodsIngestWording's. */
export function oneSampleRefusalSentence(e: unknown): string {
  const c = refusalOf(e);
  if (!c) {
    const message = e instanceof Error ? e.message : typeof e === 'string' ? e : '';
    return message.trim() ? `This could not be done: ${message.trim()}` : 'This could not be done.';
  }
  if (c.code in ONE_SAMPLE_WORDS) {
    const params = (c as { params?: unknown }).params;
    return oneSampleSentence({ code: c.code, params: params && typeof params === 'object' ? (params as Record<string, unknown>) : {} });
  }
  return methodsIngestSentence(c as WireCode);
}

/** A candidate missing code in a chosen column, as a question naming the
 *  value: never applied unless ticked. */
export function oneSampleMissingCodeQuestion(code: number, header: string, count: number): string {
  return `Is ${code} a code for a missing value? It appears in the column "${header}" (${cells(count)}). If you tick it, every row whose value is ${code} is left out.`;
}

/** Why a run that ended without a result has none. */
export function oneSampleRunEndedSentence(state: string): string {
  switch (state) {
    case 'cancelled':
      return oneSampleSentence({ code: 'run.cancelled' });
    case 'timed_out':
      return oneSampleSentence({ code: 'run.timed_out' });
    case 'resource_limit':
      return oneSampleSentence({ code: 'run.resource_limit' });
    case 'refused':
      return 'Gaply refused these settings, so nothing ran. The reasons are below.';
    case 'needs_user_input':
      return 'The settings need an answer from you, so nothing ran. See below.';
    case 'failed':
      return 'The t-test could not be computed. The reason is below.';
    default:
      return oneSampleSentence({ code: 'run.no_result' });
  }
}

/** Each test's export files. The paired test's screen (8D-2c) reads its own. */
export const ONE_SAMPLE_EXPORT_CHOICES = [
  { format: 'csv', label: 'The result table (CSV)', file: 'one_sample_t_test.csv', mime: 'text/csv' },
  { format: 'manifestJson', label: 'The run manifest (JSON)', file: 'one_sample_t_test_manifest.json', mime: 'application/json' },
] as const;

export const PAIRED_EXPORT_CHOICES = [
  { format: 'csv', label: 'The result table (CSV)', file: 'paired_t_test.csv', mime: 'text/csv' },
  { format: 'manifestJson', label: 'The run manifest (JSON)', file: 'paired_t_test_manifest.json', mime: 'application/json' },
] as const;

export const ONE_SAMPLE_EXPORT_TEXT =
  'Gaply writes only the file you name in the save dialog and keeps no copy. The result table holds the numbers and the test value you gave. The manifest holds hashes, versions and counts, never your data, headers or file name.';

/** Rows used is n, so the three counts add up. */
export const ONE_SAMPLE_ROWS_USED_NOTE = 'The values used. Rows read is rows used plus the rows left out above.';
