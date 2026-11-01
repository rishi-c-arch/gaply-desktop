// Gaply — the one-way ANOVA's words (Research OS, Phase 8G-2, decisions 318
// and 331). Every code the ANOVA screen can show has one sentence here, keyed
// by the closed set in anovaCodes.json, which
// src-tauri/tests/stats_anova_wire.rs pins to AnovaCode, RunCode,
// WorkspaceCode and CapabilityCode exactly. A refusal from the file ingest
// is said by methodsIngestWording, as on the t-test's screen.
//
// The tone is neutral (decision 66): the ANOVA describes differences between
// means; it does not pass or fail them. No sentence judges a p-value, and
// Gaply says which post hoc test the user chose, never which they should use
// (decision 312). anovaWording.vitest.ts reads every sentence for the words
// a verdict would use.
import { refusalOf } from '../statsverifier/l1/l1Wording';
import { methodsIngestSentence } from './methodsIngestWording';
import type { WireCode } from '../statsverifier/statsL1Types';
import { workspaceSentence } from '../workspace/workspaceWording';
import type { WorkspaceRefusal } from '../workspace/workspaceTypes';
// The run manager's ceiling and the runs it keeps, pinned to
// runs::RUN_CEILING_MS and runs::MAX_RUNS.
import LIMITS from './describeLimits.json';
// The group limit, which is also the post hoc cap (decisions 317 and 322),
// pinned to stats_ttest::MAX_GROUP_LEVELS and stats_anova::POST_HOC_MAX_LEVELS.
import TTEST_LIMITS from './ttestLimits.json';
import type { Stage } from './describeWording';
import type { AnovaStat, PostHoc, PostHocP } from './anovaTypes';
import { shownNumber } from './ttestWording';

export const ANOVA_STAGE_HEADINGS: Record<Stage, string> = {
  import: 'Import a data file',
  profile: 'Profile of the file',
  settings: 'Choose the outcome, the group, the levels and any post hoc tests',
  validate: 'Check the settings',
  run: 'Run the one-way ANOVA',
  results: 'Results',
  export: 'Export',
};

export const ANOVA_ENTRY_TEXT =
  "A one-way ANOVA on your own data: the differences between the means of a numeric outcome in 2 to 50 groups, with Welch's ANOVA, Levene's tests, eta and omega squared, and any post hoc tests you tick. The file is read on this device and nothing is sent anywhere. The data stays in memory while this screen is open and is dropped when you close the file or the app.";

/** Decisions 78 and 329: how levels are read, ordered and included. */
export const ANOVA_LEVELS_RULE =
  "Levels follow Level 1's reading of cells: 1 and 1.0 are one level, and text is trimmed of spaces at either end. They are listed in the order Gaply reads them, which is the order of every table, and each pair is the first level minus the second.";

/** The most levels a group may have, which is also the most a post hoc test
 *  takes. */
export const MAX_GROUP_LEVELS: number = TTEST_LIMITS.maxGroupLevels;

export const ANOVA_GROUP_LIMIT_SENTENCE = `A column with more than ${MAX_GROUP_LEVELS} distinct values is not offered as a group.`;

export const LEVELS_QUESTION = 'Levels included. Every level is ticked; untick any to leave it out. At least two are needed.';

export function levelsTooFewSentence(ticked: number): string {
  return `${ticked === 1 ? '1 level is' : `${ticked} levels are`} ticked. Tick at least two.`;
}

/** The post hoc question: asked, never answered for the user. */
export const POST_HOC_QUESTION = 'Post hoc tests. None is ticked unless you tick it; Gaply does not choose one for you.';

export const POST_HOC_CHOICES: ReadonlyArray<{ test: PostHoc; label: string; line: string }> = [
  {
    test: 'tukey',
    label: "Tukey's HSD (Tukey–Kramer for unequal group sizes)",
    line: 'Pools the variance of every group (MS within), with simultaneous 95% intervals from the studentized range.',
  },
  {
    test: 'games_howell',
    label: 'Games–Howell',
    line: "Uses each pair's own two variances and Welch's degrees of freedom, with simultaneous 95% intervals from the studentized range.",
  },
  {
    test: 'bonferroni',
    label: 'Bonferroni',
    line: 'Pools the variance of every group; each p is multiplied by the number of pairs, at most 1, and each interval is at 1 − 0.05 / pairs.',
  },
];

export const POST_HOC_NAMES: Record<PostHoc, string> = {
  tukey: "Tukey's HSD",
  games_howell: 'Games–Howell',
  bonferroni: 'Bonferroni',
};

/** The statistic a test's pairs carry. */
export const POST_HOC_STATISTIC: Record<PostHoc, string> = {
  tukey: 'q',
  games_howell: 'q',
  bonferroni: 't',
};

/** A post hoc table's heading: its test and its row count (decision 331). */
export function postHocHeading(test: PostHoc, rows: number): string {
  return `${POST_HOC_NAMES[test]}: ${rows === 1 ? '1 pair' : `${rows} pairs`}`;
}

/** The run's progress, in what it counts (decision 328). */
export function stepsSentence(done: number, total: number): string {
  return `${done} of ${total} steps`;
}

/** A column's name in a sentence, and a level's, from the user's own
 *  profile on the user's own screen. */
export interface AnovaNames {
  column: (alias: number) => string;
  level: (index: number) => string;
}
const NO_NAMES: AnovaNames = { column: (a) => `C${a}`, level: (i) => `level ${i + 1}` };

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
const levels = (n: number | undefined) => (n === 1 ? '1 level' : `${n ?? '?'} levels`);
const was = (n: number | undefined) => (n === 1 ? 'was' : 'were');

type Words = (d: Coded, names: AnovaNames) => string;

const LIFECYCLE_NAMES: Record<string, string> = {
  planned: 'planned',
  experimental: 'experimental',
  beta: 'in beta',
  stable: 'stable',
  deprecated: 'retired',
};

const workspace: Words = (d) => workspaceSentence({ code: d.code, params: d.params ?? {}, severity: 'error' } as WorkspaceRefusal);

/** One sentence per code. A code with no entry is shown as itself, never
 *  hidden; anovaWording.vitest.ts keeps every code in anovaCodes.json
 *  covered. */
export const ANOVA_WORDS: Record<string, Words> = {
  'stats.anova.unknown_capability': () => 'These settings name a capability Gaply does not have here.',
  'stats.anova.capability_version_mismatch': (d) =>
    `These settings were made for version ${num(d, 'found') ?? '?'} of the one-way ANOVA, and this is version ${num(d, 'expected') ?? '?'}.`,
  'stats.anova.input_count': (d) => `These settings name ${num(d, 'count') ?? '?'} files, and the one-way ANOVA reads exactly one.`,
  'stats.anova.input_mismatch': () => 'These settings were made for a different file from the one open. Open the file again.',
  'stats.anova.unknown_column': (d, n) => `Column ${n.column(num(d, 'column') ?? 0)} is not in this file.`,
  'stats.anova.same_column': () => 'The outcome and the group are the same column. Choose two different columns.',
  'stats.anova.outcome_holds_text': (d, n) =>
    `The outcome ${n.column(num(d, 'column') ?? 0)} holds text in ${cells(num(d, 'count'))}, so it cannot be used as a numeric outcome.`,
  'stats.anova.outcome_non_finite': (d, n) =>
    `The outcome ${n.column(num(d, 'column') ?? 0)} holds an infinity or "not a number" in ${cells(num(d, 'count'))}, so it cannot be used as a numeric outcome.`,
  'stats.anova.group_one_level': (d, n) =>
    `The group ${n.column(num(d, 'column') ?? 0)} has ${levels(num(d, 'count'))}, and the one-way ANOVA needs at least two.`,
  'stats.anova.too_many_levels': (d) =>
    `This column has ${num(d, 'count') ?? '?'} distinct values, more than the ${num(d, 'limit') ?? MAX_GROUP_LEVELS} a group may have, so its values are not listed.`,
  'stats.anova.level_out_of_range': (d) =>
    `Level ${(num(d, 'index') ?? 0) + 1} is not one of the group's levels: it has ${(num(d, 'limit') ?? 0) + 1}.`,
  'stats.anova.level_repeated': (d) => `Level ${(num(d, 'index') ?? 0) + 1} is included twice.`,
  'stats.anova.too_few_levels': (d) => `${levels(num(d, 'count'))} included, and the one-way ANOVA needs at least two.`,
  'stats.anova.missing_code_not_finite': (d) => `Missing code ${(num(d, 'index') ?? 0) + 1} is not a finite number.`,
  'stats.anova.suspect_missing_code': (d, n) =>
    `The outcome ${n.column(num(d, 'column') ?? 0)} holds ${num(d, 'value') ?? '?'} in ${cells(num(d, 'count'))}, which may be a code for a missing value. It is counted as a number unless you tick it in Settings.`,
  'stats.anova.group_too_small': (d, n) =>
    `The group "${n.level(num(d, 'index') ?? 0)}" has ${values(num(d, 'count'))} after the exclusions, and each group needs at least 2. Leave it out, or choose another group.`,
  'stats.anova.post_hoc_too_many_levels': (d) =>
    `A post hoc test takes at most ${num(d, 'limit') ?? MAX_GROUP_LEVELS} levels, and ${num(d, 'count') ?? '?'} are included.`,
  'stats.anova.outcome_constant': () =>
    'Not computable: the outcome is the same number in every row, so the total sum of squares is 0.',
  'stats.anova.within_variance_zero': () =>
    'Not computable: every group is constant, so the variance within groups (MS within) is 0.',
  'stats.anova.welch_variance_zero': () =>
    "Not computable: Welch's ANOVA weights each group by its size over its variance, and a group's variance is 0.",
  'stats.anova.levene_deviations_equal': () =>
    "Not computable: within every group the values are the same distance from the group's centre, so Levene's MS within is 0.",
  'stats.anova.pair_both_constant': () => 'Not computable: both groups of this pair have a variance of 0.',
  'stats.anova.distribution_out_of_range': () =>
    'Not computable: this needs a distribution at more than 10,000,000 degrees of freedom, where Gaply has not measured its accuracy.',
  'stats.anova.overflow': () => 'Not computable: the arithmetic went past the largest number a double can hold.',
  'stats.anova.underflow': () => 'Not computable: a sum of squares went below the smallest number a double can hold, though the values differ.',
  'stats.anova.arithmetic_refused': () => 'Not computable: the arithmetic refused to give a value.',
  'stats.anova.p_below_floor': () =>
    'A p shown as "below" a number is smaller than that number: the studentized range is computed accurately down to it, and Gaply does not show a figure beneath it.',
  'stats.anova.omega_squared_negative': () =>
    'Omega squared is negative, as computed: MS between is below MS within. It is shown as computed, not set to 0.',
  'stats.anova.missing_group': (d) =>
    `${rows(d.count)} had no group (an empty cell or a missing-value token), and ${was(d.count)} left out.`,
  'stats.anova.other_level': (d) => `${rows(d.count)} ${was(d.count)} in levels left out, and ${was(d.count)} left out.`,
  'stats.anova.missing_outcome': (d) =>
    `${rows(d.count)} had no outcome (an empty cell or a missing-value token), and ${was(d.count)} left out.`,
  'stats.anova.missing_code': (d) =>
    `${rows(d.count)} held a missing code you ticked in the outcome, and ${was(d.count)} left out.`,
  'stats.anova.normality_not_assessed': () =>
    "Normality is not assessed: the ANOVA assumes each group's values come from a roughly normal distribution, or that the groups are large, and Gaply does not check either.",
  'stats.anova.effect_sizes_have_no_interval': () =>
    'Eta squared and omega squared are shown without confidence intervals: Gaply does not compute them yet.',
  'stats.anova.no_contrasts': () =>
    'Not offered: the Brown–Forsythe test of means, trend tests and planned contrasts.',
  'run.unknown_run': () => `This run is no longer held. Gaply keeps the last ${LIMITS.maxRuns} runs in memory only; run the ANOVA again.`,
  'run.run_busy': () => 'Another run is going in this workspace. Wait for it to end, or cancel it.',
  'run.unknown_input': () => 'This file is no longer open. Open it again.',
  'run.not_finished': () => 'This run has not ended yet.',
  'run.no_result': () => 'This run ended without a result.',
  'run.cancelled': () => 'This run was cancelled, so it has no result.',
  'run.timed_out': () => `This run passed its ${LIMITS.runCeilingMs / 1000}-second limit and was stopped, so it has no result.`,
  'run.resource_limit': () => 'This run passed a resource limit and was stopped, so it has no result.',
  'run.illegal_transition': () => 'Gaply could not change this run in that way.',
  'run.already_terminal': () => 'This run has already ended.',
  'run.wrong_kind': () => 'This run is not a one-way ANOVA, so this screen cannot read it.',
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

/** The sentence for a code: a diagnostic, an exclusion, a note or a
 *  limitation. */
export function anovaSentence(d: Coded, names: AnovaNames = NO_NAMES): string {
  const f = ANOVA_WORDS[d.code];
  return f ? f(d, names) : `Gaply has no wording for the code ${d.code}.`;
}

/** One plain sentence for a command's refusal. */
export function anovaRefusalSentence(e: unknown): string {
  const c = refusalOf(e);
  if (!c) {
    const message = e instanceof Error ? e.message : typeof e === 'string' ? e : '';
    return message.trim() ? `This could not be done: ${message.trim()}` : 'This could not be done.';
  }
  if (c.code in ANOVA_WORDS) {
    const params = (c as { params?: unknown }).params;
    return anovaSentence({ code: c.code, params: params && typeof params === 'object' ? (params as Record<string, unknown>) : {} });
  }
  return methodsIngestSentence(c as WireCode);
}

/** A candidate missing code in the outcome, as a question naming the value. */
export function anovaMissingCodeQuestion(code: number, header: string, count: number): string {
  return `Is ${code} a code for a missing value? It appears in the outcome "${header}" (${cells(count)}). If you tick it, every row whose outcome is ${code} is left out.`;
}

/** Why a run that ended without a result has none. */
export function anovaRunEndedSentence(state: string): string {
  switch (state) {
    case 'cancelled':
      return anovaSentence({ code: 'run.cancelled' });
    case 'timed_out':
      return anovaSentence({ code: 'run.timed_out' });
    case 'resource_limit':
      return anovaSentence({ code: 'run.resource_limit' });
    case 'refused':
      return 'Gaply refused these settings, so nothing ran. The reasons are below.';
    case 'needs_user_input':
      return 'The settings need an answer from you, so nothing ran. See below.';
    case 'failed':
      return 'The one-way ANOVA could not be computed. The reason is below.';
    default:
      return anovaSentence({ code: 'run.no_result' });
  }
}

/** A number, or its sentence where its number would be (decision 331). */
export function anovaStatText(s: AnovaStat): string {
  return 'value' in s ? shownNumber(s.value) : anovaSentence({ code: s.not_computable });
}

/** A post hoc p: six figures, or "below" the floor, the floor's value from
 *  the result (decision 331). */
export function postHocPText(p: PostHocP): string {
  return 'value' in p ? shownNumber(p.value) : `below ${p.below}`;
}

export const ANOVA_DIGITS_NOTE =
  'Numbers are shown rounded to six figures, counted from the first digit that is not zero; the CSV export keeps every digit.';

/** Levene's tests are information, never advice. */
export const LEVENE_NOTE = "Shown as information about the groups' spreads, not as advice on which test to use.";

export const ANOVA_EXPORT_CHOICES = [
  { format: 'csv', label: 'The result tables (CSV)', file: 'one_way_anova.csv', mime: 'text/csv' },
  { format: 'manifestJson', label: 'The run manifest (JSON)', file: 'one_way_anova_manifest.json', mime: 'application/json' },
] as const;

export const ANOVA_EXPORT_TEXT =
  'Gaply writes only the file you name in the save dialog and keeps no copy. The result tables carry the group labels with the result, every digit, and the manifest\'s hashes. The manifest holds hashes, versions and counts, never your data, headers, level labels or file name.';

/** Rows used is the groups' n, so the three counts add up. */
export const ANOVA_ROWS_USED_NOTE = 'The included groups together. Rows read is rows used plus the rows left out above.';
