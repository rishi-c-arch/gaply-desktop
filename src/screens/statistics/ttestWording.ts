// Gaply — the independent-samples t-test's words (Research OS, Phase 4B-2).
// Every code the t-test screen can show has one sentence here, keyed by the
// closed set in ttestCodes.json, which src-tauri/tests/stats_ttest_wire.rs
// pins to TTestCode, RunCode, WorkspaceCode and CapabilityCode exactly. A
// refusal from the file ingest is Level 1's own code, so its sentence is
// Level 1's (l1Wording's sentenceFor), as on the describe screen.
//
// The tone is neutral (decision 66): the t-test describes a difference, it
// does not pass or fail it. No sentence here judges a p-value, and none uses
// the words a verdict would; ttestWording.vitest.ts reads every sentence and
// the rendered card for them.
import { refusalOf, sentenceFor } from '../statsverifier/l1/l1Wording';
import type { WireCode } from '../statsverifier/statsL1Types';
import { workspaceSentence } from '../workspace/workspaceWording';
import type { WorkspaceRefusal } from '../workspace/workspaceTypes';
// The run manager's ceiling and the runs it keeps, pinned to
// runs::RUN_CEILING_MS (F11) and runs::MAX_RUNS (Phase 4 evaluation, F6).
import LIMITS from './describeLimits.json';
// The group limit, pinned to stats_ttest::MAX_GROUP_LEVELS (decision 87).
import TTEST_LIMITS from './ttestLimits.json';
import type { Stage } from './describeWording';
import type { Standardizer, TTestStat, Variant } from './ttestTypes';

export const TTEST_STAGE_HEADINGS: Record<Stage, string> = {
  import: 'Import a data file',
  profile: 'Profile of the file',
  settings: 'Choose the outcome, the group and the test',
  validate: 'Check the settings',
  run: 'Run the t-test',
  results: 'Results',
  export: 'Export',
};

export const TTEST_ENTRY_TEXT =
  "An independent-samples t-test on your own data: the difference between the means of a numeric outcome in two groups, with Student's or Welch's t-test, which you choose. The file is read on this device and nothing is sent anywhere. The data stays in memory while this screen is open and is dropped when you close the file or the app.";

/** Decision 65 and 78: how levels are read, ordered and chosen. */
export const TTEST_LEVELS_RULE =
  "Levels follow Level 1's reading of cells: 1 and 1.0 are one level, and text is trimmed of spaces at either end. They are listed in the order Gaply reads them, and the first level you choose is group 1: the mean difference is group 1 minus group 2.";

/** The variant question (decision 64): asked, never answered for the user.
 *  Each choice has one neutral line beside it (decision 93): what the test
 *  assumes about the two variances. Information, never a recommendation. */
export const VARIANT_QUESTION = 'Which t-test? Gaply does not choose one for you.';
export const VARIANT_CHOICES: ReadonlyArray<{ variant: Variant; label: string; line: string }> = [
  {
    variant: 'student',
    label: "Student's t-test",
    line: "Assumes the two groups' variances are equal, and pools them into one estimate.",
  },
  {
    variant: 'welch',
    label: "Welch's t-test",
    line: "Does not assume the two groups' variances are equal: it uses each group's own variance and adjusts the degrees of freedom.",
  },
];

export const VARIANT_NAMES: Record<Variant, string> = {
  student: "Student's t-test (equal variances assumed)",
  welch: "Welch's t-test (equal variances not assumed)",
};

export const STANDARDIZER_NAMES: Record<Standardizer, string> = {
  pooled_sd: 'the pooled SD',
  root_mean_variance: 'the square root of the mean of the two variances',
};

/** The most distinct values a group column may have (decision 87). */
export const MAX_GROUP_LEVELS: number = TTEST_LIMITS.maxGroupLevels;

/** Why a column with more is not offered as a group. */
export const GROUP_LIMIT_SENTENCE = `A column with more than ${MAX_GROUP_LEVELS} distinct values is not offered as a group: a t-test compares two groups, and a column with that many values is a measurement rather than a grouping.`;

export function levelsQuestion(count: number): string {
  return `The group has ${count} levels. Choose the two to compare: the first is group 1 and the second is group 2.`;
}

/** A column's name in a sentence, and a level's, from the user's own
 *  profile on the user's own screen. */
export interface TTestNames {
  column: (alias: number) => string;
  level: (index: number) => string;
}
const NO_NAMES: TTestNames = { column: (a) => `C${a}`, level: (i) => `level ${i + 1}` };

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
const values = (n: number | undefined) => (n === 1 ? '1 value' : `${n ?? '?'} values`);
/** The verb agrees with the rows counted. */
const was = (n: number | undefined) => (n === 1 ? 'was' : 'were');

type Words = (d: Coded, names: TTestNames) => string;

const LIFECYCLE_NAMES: Record<string, string> = {
  planned: 'planned',
  experimental: 'experimental',
  beta: 'in beta',
  stable: 'stable',
  deprecated: 'retired',
};

const workspace: Words = (d) => workspaceSentence({ code: d.code, params: d.params ?? {}, severity: 'error' } as WorkspaceRefusal);

/** One sentence per code. A code with no entry is shown as itself, never
 *  hidden; ttestWording.vitest.ts keeps every code in ttestCodes.json
 *  covered. */
export const TTEST_WORDS: Record<string, Words> = {
  'stats.ttest.unknown_capability': () => 'These settings name a capability Gaply does not have here.',
  'stats.ttest.capability_version_mismatch': (d) =>
    `These settings were made for version ${num(d, 'found') ?? '?'} of the t-test, and this is version ${num(d, 'expected') ?? '?'}.`,
  'stats.ttest.input_count': (d) => `These settings name ${num(d, 'count') ?? '?'} files, and the t-test reads exactly one.`,
  'stats.ttest.input_mismatch': () => 'These settings were made for a different file from the one open. Open the file again.',
  'stats.ttest.unknown_column': (d, n) => `Column ${n.column(num(d, 'column') ?? 0)} is not in this file.`,
  'stats.ttest.same_column': () => 'The outcome and the group are the same column. Choose two different columns.',
  'stats.ttest.outcome_holds_text': (d, n) =>
    `The outcome ${n.column(num(d, 'column') ?? 0)} holds text in ${cells(num(d, 'count'))}, so it cannot be used as a numeric outcome.`,
  'stats.ttest.outcome_non_finite': (d, n) =>
    `The outcome ${n.column(num(d, 'column') ?? 0)} holds an infinity or "not a number" in ${cells(num(d, 'count'))}, so it cannot be used as a numeric outcome.`,
  'stats.ttest.group_one_level': (d, n) =>
    `The group ${n.column(num(d, 'column') ?? 0)} has ${num(d, 'count') === 1 ? '1 level' : `${num(d, 'count') ?? '?'} levels`}, and the t-test needs two.`,
  'stats.ttest.level_out_of_range': (d) =>
    `Level ${(num(d, 'index') ?? 0) + 1} is not one of the group's levels: it has ${(num(d, 'limit') ?? 0) + 1}.`,
  'stats.ttest.levels_equal': () => 'The two levels chosen are the same level. Choose two different levels.',
  'stats.ttest.missing_code_not_finite': (d) => `Missing code ${(num(d, 'index') ?? 0) + 1} is not a finite number.`,
  'stats.ttest.variant_not_chosen': () =>
    "Choose Student's t-test or Welch's t-test in Settings. Gaply does not choose one for you.",
  'stats.ttest.levels_not_chosen': (d) => `The group has ${num(d, 'count') ?? '?'} levels. Choose the two to compare in Settings.`,
  'stats.ttest.suspect_missing_code': (d, n) =>
    `The outcome ${n.column(num(d, 'column') ?? 0)} holds ${num(d, 'value') ?? '?'} in ${cells(num(d, 'count'))}, which may be a code for a missing value. It is counted as a number unless you tick it in Settings.`,
  'stats.ttest.group_too_small': (d, n) =>
    `The group "${n.level(num(d, 'index') ?? 0)}" has ${values(num(d, 'count'))} after the exclusions, and each group needs at least 2.`,
  'stats.ttest.both_groups_constant': () =>
    'Every value in each group is the same, so the standard error is exactly 0 and t is not defined.',
  'stats.ttest.overflow': () => 'The arithmetic went past the largest number a double can hold.',
  'stats.ttest.underflow': () => 'The arithmetic went below the smallest number a double can hold, though the values differ.',
  'stats.ttest.arithmetic_refused': () => 'The arithmetic refused to give a value.',
  'stats.ttest.quantile_out_of_range': () =>
    'The interval needs a t quantile at more than 10,000 degrees of freedom, where Gaply has not measured its accuracy, so nothing was computed.',
  'stats.ttest.sd_ratio_undefined': () => "The SD ratio is not defined, because one group's SD is 0.",
  'stats.ttest.missing_group': (d) =>
    `${rows(d.count)} had no group (an empty cell or a missing-value token), and ${was(d.count)} left out.`,
  'stats.ttest.other_level': (d) => `${rows(d.count)} ${was(d.count)} in other levels of the group, and ${was(d.count)} left out.`,
  'stats.ttest.missing_outcome': (d) =>
    `${rows(d.count)} had no outcome (an empty cell or a missing-value token), and ${was(d.count)} left out.`,
  'stats.ttest.missing_code': (d) =>
    `${rows(d.count)} held a missing code you ticked in the outcome, and ${was(d.count)} left out.`,
  'stats.ttest.normality_not_assessed': () =>
    "Normality is not assessed: the t-test assumes each group's values come from a roughly normal distribution, or that the groups are large, and Gaply does not check either.",
  'stats.ttest.cohens_d_has_no_interval': () => "Cohen's d is shown without a confidence interval: Gaply does not compute one for d yet.",
  'stats.ttest.too_many_levels': (d) =>
    `This column has ${num(d, 'count') ?? '?'} distinct values, more than the ${num(d, 'limit') ?? TTEST_LIMITS.maxGroupLevels} a group may have, so its values are not listed.`,
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
  'run.wrong_kind': () => 'This run is not a t-test, so this screen cannot read it.',
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
export function ttestSentence(d: Coded, names: TTestNames = NO_NAMES): string {
  const f = TTEST_WORDS[d.code];
  return f ? f(d, names) : `Gaply has no wording for the code ${d.code}.`;
}

/** One plain sentence for a command's refusal: the run manager's, the
 *  registry's and the workspace's codes in these words, the ingest's in
 *  Level 1's. */
export function ttestRefusalSentence(e: unknown): string {
  const c = refusalOf(e);
  if (!c) {
    const message = e instanceof Error ? e.message : typeof e === 'string' ? e : '';
    return message.trim() ? `This could not be done: ${message.trim()}` : 'This could not be done.';
  }
  if (c.code in TTEST_WORDS) {
    const params = (c as { params?: unknown }).params;
    return ttestSentence({ code: c.code, params: params && typeof params === 'object' ? (params as Record<string, unknown>) : {} });
  }
  return sentenceFor(c as WireCode);
}

/** A candidate missing code in the outcome, as a question naming the
 *  value: never applied unless ticked. */
export function ttestMissingCodeQuestion(code: number, header: string, count: number): string {
  return `Is ${code} a code for a missing value? It appears in the outcome "${header}" (${cells(count)}). If you tick it, every row whose outcome is ${code} is left out.`;
}

/** Why a run that ended without a result has none. */
export function ttestRunEndedSentence(state: string): string {
  switch (state) {
    case 'cancelled':
      return ttestSentence({ code: 'run.cancelled' });
    case 'timed_out':
      return ttestSentence({ code: 'run.timed_out' });
    case 'resource_limit':
      return ttestSentence({ code: 'run.resource_limit' });
    case 'refused':
      return 'Gaply refused these settings, so nothing ran. The reasons are below.';
    case 'needs_user_input':
      return 'The settings need an answer from you, so nothing ran. See below.';
    case 'failed':
      return 'The t-test could not be computed. The reason is below.';
    default:
      return ttestSentence({ code: 'run.no_result' });
  }
}

/** A number as the card shows it: six figures, counted from the first digit
 *  that is not zero (`toPrecision(6)`). The CSV export keeps every digit. */
export function shownNumber(v: number): string {
  return String(Number.parseFloat(v.toPrecision(6)));
}

// Not "significant figures": the neutral-wording rule keeps that word off
// this screen (decision 66), so the sentence says how the figures are counted.
export const TTEST_DIGITS_NOTE =
  'Numbers are shown rounded to six figures, counted from the first digit that is not zero; the CSV export keeps every digit.';

export function statSentence(s: TTestStat): string {
  return 'value' in s ? shownNumber(s.value) : ttestSentence({ code: s.not_computable });
}

/** The SD ratio is information, never advice (decision 64). */
export const SD_RATIO_NOTE = 'The larger group SD over the smaller. Shown as information, not as advice on which t-test to use.';

export const TTEST_EXPORT_CHOICES = [
  { format: 'csv', label: 'The result table (CSV)', file: 'independent_t_test.csv', mime: 'text/csv' },
  { format: 'manifestJson', label: 'The run manifest (JSON)', file: 'independent_t_test_manifest.json', mime: 'application/json' },
] as const;

export const TTEST_EXPORT_TEXT =
  'Gaply writes only the file you name in the save dialog and keeps no copy. The result table carries the two group labels with the result. The manifest holds hashes, versions and counts, never your data, headers, level labels or file name.';

/** Rows used is the two groups' n, so the three counts add up. */
export const TTEST_ROWS_USED_NOTE = 'The two groups together. Rows read is rows used plus the rows left out above.';
