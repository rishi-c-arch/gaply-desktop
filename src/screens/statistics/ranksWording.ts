// Gaply — the rank tests' words (Research OS, Phase 8Y-2, decisions 619 and 620).
// Every code the card and dialog can show has one sentence here, keyed by the closed
// set in ranksCodes.json, which src-tauri/tests/stats_ranks_wire.rs pins to RanksCode,
// RunCode, WorkspaceCode and CapabilityCode exactly. The run's, the registry's and the
// workspace's sentences are the ANOVA's (anovaWording.ts:141), but the two that name
// the method. A refusal from the file ingest is said by methodsIngestWording.
//
// The tone is neutral (decision 66): a rank test says what the numbers are; nothing
// here passes or fails a difference. No sentence judges a p value or names a threshold
// for one. The progress shows no steps sentence: a run has one check point, and
// "0 of 1 steps" says nothing. ranksWording.vitest.ts reads every sentence for the
// words a verdict would use.
import { refusalOf } from '../statsverifier/l1/l1Wording';
import { methodsIngestSentence } from './methodsIngestWording';
import type { WireCode } from '../statsverifier/statsL1Types';
// The run manager's runs kept, pinned to runs::MAX_RUNS.
import LIMITS from './describeLimits.json';
import { ANOVA_DIGITS_NOTE, ANOVA_EXPORT_TEXT, ANOVA_ROWS_USED_NOTE, ANOVA_WORDS, MAX_GROUP_LEVELS, type AnovaNames, type Coded } from './anovaWording';
import type { RanksStat } from './ranksTypes';
import { shownNumber } from './ttestWording';

/** A column's name and a level's label in a sentence, from the user's own screen. */
export type RanksNames = AnovaNames;
const NO_NAMES: RanksNames = { column: (a) => `C${a}`, level: (i) => `level ${i + 1}` };

const num = (d: Coded, k: string): number | undefined => {
  const v = d.params?.[k];
  return typeof v === 'number' ? v : undefined;
};
const levels = (n: number | undefined) => (n === 1 ? '1 level' : `${n ?? 0} levels`);

type Words = (d: Coded, names: RanksNames) => string;

/** The ANOVA's sentence for a code that names no ANOVA-only idea. */
const shared = (code: string): Words => ANOVA_WORDS[code] as unknown as Words;
const asAnova = (suffix: string): Words => shared(`stats.anova.${suffix}`);

/** One sentence per code. A code with no entry is shown as itself, never hidden;
 *  ranksWording.vitest.ts keeps every code in ranksCodes.json covered. */
export const RANKS_WORDS: Record<string, Words> = {
  'stats.ranks.unknown_capability': asAnova('unknown_capability'),
  'stats.ranks.capability_version_mismatch': (d) =>
    `These settings were made for version ${num(d, 'found') ?? 0} of the rank tests, and this is version ${num(d, 'expected') ?? 0}.`,
  'stats.ranks.input_count': (d) => `These settings name ${num(d, 'count') ?? 0} files, and a rank test reads exactly one.`,
  'stats.ranks.input_mismatch': asAnova('input_mismatch'),
  'stats.ranks.preparation_mismatch': asAnova('preparation_mismatch'),
  'stats.ranks.missing_code_not_finite': asAnova('missing_code_not_finite'),
  'stats.ranks.unknown_column': asAnova('unknown_column'),
  'stats.ranks.score_as_group': asAnova('score_as_group'),
  'stats.ranks.code_equals_score': asAnova('code_equals_score'),
  'stats.ranks.same_column': asAnova('same_column'),
  'stats.ranks.outcome_non_finite': asAnova('outcome_non_finite'),
  'stats.ranks.outcome_holds_text': asAnova('outcome_holds_text'),
  'stats.ranks.group_one_level': (d, n) =>
    `The group ${n.column(num(d, 'column') ?? 0)} has ${levels(num(d, 'count'))}, and a rank test needs at least two.`,
  'stats.ranks.too_many_levels': asAnova('too_many_levels'),
  'stats.ranks.level_out_of_range': asAnova('level_out_of_range'),
  'stats.ranks.level_repeated': asAnova('level_repeated'),
  'stats.ranks.too_few_levels': (d) => `${levels(num(d, 'count'))} included, and a rank test needs at least two.`,
  'stats.ranks.group_empty': (d, n) =>
    `The group "${n.level(num(d, 'index') ?? 0)}" has no value after the exclusions. Leave it out, or choose another group.`,
  'stats.ranks.suspect_missing_code': asAnova('suspect_missing_code'),
  'stats.ranks.all_values_tied': () => 'Not computable: every included value is equal, so the ranks do not differ between groups.',
  'stats.ranks.distribution_out_of_range': () =>
    'Not computable: this needs the chi-square distribution at more than 2,401 degrees of freedom, where Gaply has not measured its accuracy.',
  'stats.ranks.overflow': asAnova('overflow'),
  'stats.ranks.arithmetic_refused': asAnova('arithmetic_refused'),
  'stats.ranks.exact_not_given_ties': () =>
    'The exact p value is not given, because two or more included values are tied; the p value above comes from the normal approximation.',
  'stats.ranks.exact_not_given_size': () =>
    'The exact p value is not given, because a group has 50 values or more; the p value above comes from the normal approximation.',
  'stats.ranks.group_below_five': () => 'A group has fewer than 5 values, where the normal and chi-square approximations are rougher.',
  'stats.ranks.missing_group': asAnova('missing_group'),
  'stats.ranks.other_level': asAnova('other_level'),
  'stats.ranks.missing_outcome': asAnova('missing_outcome'),
  'stats.ranks.missing_code': asAnova('missing_code'),
  'stats.ranks.no_pairwise_comparisons': () => "Pairwise comparisons after a Kruskal-Wallis test, such as Dunn's test, are not given.",
  'stats.ranks.two_sided_only': () => 'Each Mann-Whitney p value is two-sided; one-sided tests are not offered.',
  'stats.ranks.no_location_shift_estimate': () =>
    'No estimate of the shift between two groups, such as the Hodges-Lehmann estimate, and no interval for it, is given.',
  'stats.ranks.exact_without_ties_only': () =>
    "The Mann-Whitney test's exact p value is given only when no two values are tied and each group has fewer than 50 values; otherwise only the normal approximation's p value is given.",
  'stats.ranks.no_further_k_sample_tests': () => 'The median test and the Jonckheere-Terpstra test are not given.',
  'stats.ranks.no_weights': () => 'Each row is counted once; weighting rows by a column is not offered.',
  'stats.ranks.code_point_order': () =>
    'Groups are listed in the order Gaply reads them, so a group column of numbers lists 10 before 2; which group is first, and the signs of the Mann-Whitney z and of the rank-biserial correlation, follow that order.',
  'run.unknown_run': () => `This run is no longer held. Gaply keeps the last ${LIMITS.maxRuns} runs in memory only; run the rank test again.`,
  'run.run_busy': shared('run.run_busy'),
  'run.unknown_input': shared('run.unknown_input'),
  'run.not_finished': shared('run.not_finished'),
  'run.no_result': shared('run.no_result'),
  'run.cancelled': shared('run.cancelled'),
  'run.timed_out': shared('run.timed_out'),
  'run.resource_limit': shared('run.resource_limit'),
  'run.illegal_transition': shared('run.illegal_transition'),
  'run.already_terminal': shared('run.already_terminal'),
  'run.wrong_kind': () => 'This run is not a rank test, so this screen cannot read it.',
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
export function ranksSentence(d: Coded, names: RanksNames = NO_NAMES): string {
  const f = RANKS_WORDS[d.code];
  return f ? f(d, names) : `Gaply has no wording for the code ${d.code}.`;
}

/** One plain sentence for a command's refusal. */
export function ranksRefusalSentence(e: unknown): string {
  const c = refusalOf(e);
  if (!c) {
    const message = e instanceof Error ? e.message : typeof e === 'string' ? e : '';
    return message.trim() ? `This could not be done: ${message.trim()}` : 'This could not be done.';
  }
  if (c.code in RANKS_WORDS) {
    const params = (c as { params?: unknown }).params;
    return ranksSentence({ code: c.code, params: params && typeof params === 'object' ? (params as Record<string, unknown>) : {} });
  }
  return methodsIngestSentence(c as WireCode);
}

/** Why a run that ended without a result has none. */
export function ranksRunEndedSentence(state: string): string {
  switch (state) {
    case 'cancelled':
      return ranksSentence({ code: 'run.cancelled' });
    case 'timed_out':
      return ranksSentence({ code: 'run.timed_out' });
    case 'resource_limit':
      return ranksSentence({ code: 'run.resource_limit' });
    case 'refused':
      return 'Gaply refused these settings, so nothing ran. The reasons are below.';
    case 'needs_user_input':
      return 'The settings need an answer from you, so nothing ran. See below.';
    case 'failed':
      return 'The rank test could not be computed. The reason is below.';
    default:
      return ranksSentence({ code: 'run.no_result' });
  }
}

/* ---- the dialog ----------------------------------------------------------- */

/** Decision 602: how levels are read and ordered, and which group is first. */
export const RANKS_LEVELS_RULE =
  "Levels follow Level 1's reading of cells: 1 and 1.0 are one level, and text is trimmed of spaces at either end. They are listed in the order Gaply reads them, so a column of numbers lists 10 before 2, and with two levels ticked the first listed is the first group.";

export const RANKS_OUTCOME_LABEL = 'Outcome (a numeric column)';
export const RANKS_GROUP_LABEL = `Group (a column with 2 to ${MAX_GROUP_LEVELS} levels)`;

/* ---- the card ------------------------------------------------------------- */

export const RANKS_HEADINGS = {
  ranks: 'Ranks',
  mannWhitney: 'Mann-Whitney U test',
  kruskalWallis: 'Kruskal-Wallis H test',
} as const;

export const RANKS_GROUP_COLUMNS = ['Group', 'N', 'Median', 'Mean rank', 'Sum of ranks'] as const;

export const MW_ROW_LABELS = {
  u1: 'U, first group',
  u2: 'U, second group',
  z: 'z',
  p: 'p (normal approximation)',
  exactP: 'Exact p',
  rankBiserial: 'Rank-biserial correlation',
} as const;

export const KW_ROW_LABELS = {
  h: 'H',
  df: 'df',
  p: 'p',
  epsilonSquared: 'Epsilon-squared',
} as const;

/** Decision 620: under the Mann-Whitney table, the two groups as text. */
export function ranksFirstSecondSentence(first: string, second: string): string {
  return `The first group is ${first}, the second ${second}. U for the first group counts the pairs in which its value is the larger, a tie counting one half; z and the rank-biserial correlation are positive when the first group's values tend to be the larger.`;
}

/** A number, or its sentence where its number would be. */
export function ranksStatText(s: RanksStat): string {
  return 'value' in s ? shownNumber(s.value) : ranksSentence({ code: s.not_computable });
}

export const RANKS_DIGITS_NOTE = ANOVA_DIGITS_NOTE;

export const RANKS_EXPORT_HEADING = 'Export';

export const RANKS_EXPORT_CHOICES = [
  { format: 'csv', label: 'The result tables (CSV)', file: 'independent_ranks.csv', mime: 'text/csv' },
  { format: 'manifestJson', label: 'The run manifest (JSON)', file: 'independent_ranks_manifest.json', mime: 'application/json' },
] as const;

export const RANKS_EXPORT_TEXT = ANOVA_EXPORT_TEXT;

/** Rows used is the groups' n, so the three counts add up. */
export const RANKS_ROWS_USED_NOTE = ANOVA_ROWS_USED_NOTE;
