// Gaply — the related rank tests' words (Research OS, Phase 8Z-2, decisions 653 and 654).
// Every code the card and dialog can show has one sentence here, keyed by the closed set in
// relatedRanksCodes.json, which src-tauri/tests/stats_related_ranks_wire.rs pins to
// RelatedRanksCode, RunCode, WorkspaceCode and CapabilityCode exactly. The run's, the
// registry's and the workspace's sentences are the rank tests' (ranksWording.ts), but the
// two that name the method. A refusal from the file ingest is said by methodsIngestWording.
//
// The tone is neutral (decision 66): a rank test says what the numbers are; nothing here
// passes or fails a difference. No sentence judges a p value or names a threshold for one.
// The progress shows no steps sentence: a run has one check point. "Remove" and "Remove
// all" are labels, and the verdict list refuses them, so the settings take them from
// windowWording and they are not here. relatedRanksWording.vitest.ts reads every sentence
// for the words a verdict would use.
import { refusalOf } from '../statsverifier/l1/l1Wording';
import { methodsIngestSentence } from './methodsIngestWording';
import type { WireCode } from '../statsverifier/statsL1Types';
// The run manager's runs kept, pinned to runs::MAX_RUNS.
import LIMITS from './describeLimits.json';
// Pinned to stats_related_ranks::MAX_RELATED_COLUMNS by stats_related_ranks_wire.rs.
import RELATED_LIMITS from './fixtures/relatedRanksLimits.json';
import { RANKS_DIGITS_NOTE, RANKS_EXACT_NOT_GIVEN_CELL, RANKS_EXPORT_HEADING, RANKS_EXPORT_TEXT, RANKS_WORDS, type RanksNames } from './ranksWording';
import type { Coded } from './anovaWording';
import type { RelatedRanksStat } from './relatedRanksTypes';
import { shownNumber } from './ttestWording';

/** The most columns a run takes. */
export const MAX_RELATED_COLUMNS: number = RELATED_LIMITS.maxColumns;

/** A column's name in a sentence, from the user's own screen. */
export type RelatedRanksNames = Pick<RanksNames, 'column'>;
const NO_NAMES: RelatedRanksNames = { column: (a) => `C${a}` };

const num = (d: Coded, k: string): number | undefined => {
  const v = d.params?.[k];
  return typeof v === 'number' ? v : undefined;
};
const cells = (n: number | undefined) => (n === 1 ? '1 cell' : `${n ?? 0} cells`);
const rows = (n: number | undefined) => (n === 1 ? '1 row' : `${n ?? 0} rows`);
const was = (n: number | undefined) => (n === 1 ? 'was' : 'were');
const columnsWord = (n: number | undefined) => (n === 1 ? '1 column' : `${n ?? 0} columns`);
const countOf = (d: Coded): number | undefined => (typeof d.count === 'number' ? d.count : num(d, 'count'));

type Words = (d: Coded, names: RelatedRanksNames) => string;

/** The rank tests' sentence for a code whose suffix they share and that names no idea
 *  only they have. */
const asRanks = (suffix: string): Words => (d, n) => RANKS_WORDS[`stats.ranks.${suffix}`](d, { ...n, level: (i) => `level ${i + 1}` });
const fromRanks = (code: string): Words => RANKS_WORDS[code] as unknown as Words;

/** One sentence per code. A code with no entry is shown as itself, never hidden;
 *  relatedRanksWording.vitest.ts keeps every code in relatedRanksCodes.json covered. */
export const RELATED_RANKS_WORDS: Record<string, Words> = {
  'stats.related_ranks.unknown_capability': asRanks('unknown_capability'),
  'stats.related_ranks.capability_version_mismatch': asRanks('capability_version_mismatch'),
  'stats.related_ranks.input_count': asRanks('input_count'),
  'stats.related_ranks.input_mismatch': asRanks('input_mismatch'),
  'stats.related_ranks.preparation_mismatch': asRanks('preparation_mismatch'),
  'stats.related_ranks.missing_code_not_finite': asRanks('missing_code_not_finite'),
  'stats.related_ranks.unknown_column': asRanks('unknown_column'),
  'stats.related_ranks.code_equals_score': asRanks('code_equals_score'),
  'stats.related_ranks.too_few_columns': (d) => `${columnsWord(num(d, 'count'))} chosen, and a related-samples rank test needs at least two.`,
  'stats.related_ranks.too_many_columns': (d) => `${columnsWord(num(d, 'count'))} chosen, and a related-samples rank test takes at most ${num(d, 'limit') ?? MAX_RELATED_COLUMNS}.`,
  'stats.related_ranks.column_repeated': (d, n) => `Column ${n.column(num(d, 'column') ?? 0)} is chosen more than once. Choose each column once.`,
  'stats.related_ranks.column_non_finite': (d, n) =>
    `Column ${n.column(num(d, 'column') ?? 0)} holds an infinity or "not a number" in ${cells(num(d, 'count'))}, so it cannot be compared.`,
  'stats.related_ranks.column_holds_text': (d, n) => `Column ${n.column(num(d, 'column') ?? 0)} holds text in ${cells(num(d, 'count'))}, so it cannot be compared.`,
  'stats.related_ranks.no_complete_rows': () => 'No row has a value in every chosen column, so there is nothing to compare.',
  'stats.related_ranks.suspect_missing_code': (d, n) =>
    `Column ${n.column(num(d, 'column') ?? 0)} holds ${num(d, 'value') ?? 0} in ${cells(num(d, 'count'))}, which may be a code for a missing value. It is counted as a number unless you tick it in Settings.`,
  'stats.related_ranks.all_differences_zero': () => 'Not computable: no pair has a difference, so there is nothing to rank.',
  'stats.related_ranks.all_values_tied': () => "Not computable: in every row the chosen columns' values are equal, so the ranks do not differ between columns.",
  'stats.related_ranks.distribution_out_of_range': asRanks('distribution_out_of_range'),
  'stats.related_ranks.overflow': asRanks('overflow'),
  'stats.related_ranks.exact_not_given_ties': () => 'The exact p value is not given, because two or more differences are equal in size.',
  'stats.related_ranks.exact_not_given_zeros': () => 'The exact p value is not given, because one or more pairs have no difference.',
  'stats.related_ranks.exact_not_given_size': () => 'The exact p value is not given, because 50 or more pairs have a difference.',
  'stats.related_ranks.zero_differences_dropped': () => 'Pairs with no difference are left out of the ranks; their count is given above.',
  'stats.related_ranks.rows_below_five': () => 'Fewer than 5 pairs with a difference, or fewer than 5 complete rows, are used, where the normal and chi-square approximations are rougher.',
  'stats.related_ranks.missing_value': (d, n) => `${rows(countOf(d))} had no value in ${n.column(num(d, 'column') ?? 0)} (an empty cell or a missing-value token), and ${was(countOf(d))} left out.`,
  'stats.related_ranks.missing_code': (d, n) => `${rows(countOf(d))} held a missing code you ticked in ${n.column(num(d, 'column') ?? 0)}, and ${was(countOf(d))} left out.`,
  'stats.related_ranks.no_pairwise_comparisons': () => 'Pairwise comparisons after a Friedman test are not given.',
  'stats.related_ranks.two_sided_only': () => 'Each Wilcoxon p value is two-sided; one-sided tests are not offered.',
  'stats.related_ranks.no_location_estimate': () => 'No estimate of the typical difference between two columns, such as the Hodges-Lehmann estimate, and no interval for it, is given.',
  'stats.related_ranks.exact_without_ties_or_zeros_only': () =>
    "The Wilcoxon test's exact p value is given only when no two differences are tied in size, none is zero and there are fewer than 50 pairs; otherwise only the normal approximation's p value is given.",
  'stats.related_ranks.pratt_not_offered': () => "A pair whose two values are equal is left out of the Wilcoxon test; Pratt's way of keeping such pairs is not offered.",
  'stats.related_ranks.no_further_related_tests': () => "The sign test, the marginal homogeneity test and Cochran's Q test are not given.",
  'stats.related_ranks.no_weights': asRanks('no_weights'),
  'stats.related_ranks.column_order': () =>
    'Columns are taken in the order you choose them: the Wilcoxon differences are the first column less the second, and the signs of z and of the matched-pairs rank-biserial correlation follow that order.',
  'run.unknown_run': () => `This run is no longer held. Gaply keeps the last ${LIMITS.maxRuns} runs in memory only; run the related-samples rank test again.`,
  'run.wrong_kind': () => 'This run is not a related-samples rank test, so this screen cannot read it.',
  'run.run_busy': fromRanks('run.run_busy'),
  'run.unknown_input': fromRanks('run.unknown_input'),
  'run.not_finished': fromRanks('run.not_finished'),
  'run.no_result': fromRanks('run.no_result'),
  'run.cancelled': fromRanks('run.cancelled'),
  'run.timed_out': fromRanks('run.timed_out'),
  'run.resource_limit': fromRanks('run.resource_limit'),
  'run.illegal_transition': fromRanks('run.illegal_transition'),
  'run.already_terminal': fromRanks('run.already_terminal'),
  'capability.unknown_capability': fromRanks('capability.unknown_capability'),
  'capability.version_mismatch': fromRanks('capability.version_mismatch'),
  'capability.not_runnable': fromRanks('capability.not_runnable'),
  'capability.confirmation_missing': fromRanks('capability.confirmation_missing'),
  'capability.confirmation_unmatched': fromRanks('capability.confirmation_unmatched'),
  'workspace.unavailable': fromRanks('workspace.unavailable'),
  'workspace.not_ready': fromRanks('workspace.not_ready'),
  'workspace.inactive': fromRanks('workspace.inactive'),
  'workspace.wrong_workspace': fromRanks('workspace.wrong_workspace'),
  'workspace.run_active': fromRanks('workspace.run_active'),
  'workspace.illegal_transition': fromRanks('workspace.illegal_transition'),
};

/** The sentence for a code: a diagnostic, an exclusion, a note or a limitation. */
export function relatedRanksSentence(d: Coded, names: RelatedRanksNames = NO_NAMES): string {
  const f = RELATED_RANKS_WORDS[d.code];
  return f ? f(d, names) : `Gaply has no wording for the code ${d.code}.`;
}

/** One plain sentence for a command's refusal. */
export function relatedRanksRefusalSentence(e: unknown): string {
  const c = refusalOf(e);
  if (!c) {
    const message = e instanceof Error ? e.message : typeof e === 'string' ? e : '';
    return message.trim() ? `This could not be done: ${message.trim()}` : 'This could not be done.';
  }
  if (c.code in RELATED_RANKS_WORDS) {
    const params = (c as { params?: unknown }).params;
    return relatedRanksSentence({ code: c.code, params: params && typeof params === 'object' ? (params as Record<string, unknown>) : {} });
  }
  return methodsIngestSentence(c as WireCode);
}

/** Why a run that ended without a result has none. */
export function relatedRanksRunEndedSentence(state: string): string {
  switch (state) {
    case 'cancelled':
      return relatedRanksSentence({ code: 'run.cancelled' });
    case 'timed_out':
      return relatedRanksSentence({ code: 'run.timed_out' });
    case 'resource_limit':
      return relatedRanksSentence({ code: 'run.resource_limit' });
    case 'refused':
      return 'Gaply refused these settings, so nothing ran. The reasons are below.';
    case 'needs_user_input':
      return 'The settings need an answer from you, so nothing ran. See below.';
    case 'failed':
      return 'The related-samples rank test could not be computed. The reason is below.';
    default:
      return relatedRanksSentence({ code: 'run.no_result' });
  }
}

/* ---- the settings --------------------------------------------------------- */

export const RELATED_RANKS_LEGEND = 'Which columns do you want to compare? Each row is one case, measured in every chosen column.';
export const RELATED_RANKS_LIST_HEADING = 'Compared, in this order';
export const RELATED_RANKS_RULE =
  'Two columns give the Wilcoxon signed-rank test; three or more give the Friedman test. A row is used only when every chosen column has a value.';
export const RELATED_RANKS_TOO_FEW = 'Choose at least two columns.';
export const RELATED_RANKS_CODES_LEGEND = 'Candidate missing codes';

export function relatedRanksTooManyColumns(n: number): string {
  return `Choose ${MAX_RELATED_COLUMNS} columns or fewer; ${n} are chosen.`;
}

export function relatedRanksDifferenceSentence(first: string, second: string): string {
  return `The differences are ${first} less ${second}.`;
}

export const relatedRanksMoveUp = (alias: string) => `Move up ${alias}`;
export const relatedRanksMoveDown = (alias: string) => `Move down ${alias}`;

/** A candidate missing code in the chosen columns, as a question naming the value and where
 *  it appears, its places written as alphaMissingCodeQuestion writes them. */
export function relatedRanksMissingCodeQuestion(code: number, where: ReadonlyArray<{ header: string; cells: number }>): string {
  const places = where.map((w) => `"${w.header}" (${cells(w.cells)})`).join(', ');
  return `Is ${code} a code for a missing value? It appears in ${places}. If you tick it, it is treated as missing in every chosen column, and a row with it is left out.`;
}

/* ---- the card ------------------------------------------------------------- */

export const RELATED_RANKS_HEADINGS = {
  columns: 'Columns',
  ranks: 'Ranks of the differences',
  wilcoxon: 'Wilcoxon signed-rank test',
  friedman: 'Friedman test',
} as const;

export const RELATED_COLUMN_HEADS = ['Column', 'Median'] as const;
export const RELATED_FRIEDMAN_HEADS = ['Mean rank', 'Sum of ranks'] as const;
export const RELATED_RANK_HEADS = ['N', 'Mean rank', 'Sum of ranks'] as const;

export const RELATED_RANK_ROWS = {
  negative: 'Negative differences',
  positive: 'Positive differences',
  zero: 'No difference (left out)',
} as const;

/** A side's mean rank when it has no pair. */
export const RELATED_NO_PAIRS = 'No pairs.';

export const WILCOXON_ROW_LABELS = {
  v: 'V',
  z: 'z',
  p: 'p (normal approximation)',
  exactP: 'Exact p',
  rankBiserial: 'Matched-pairs rank-biserial correlation',
  medianDifference: 'Median of the differences',
} as const;

/** Decision 654: an absent exact p's cell; the notes say why. */
export const RELATED_EXACT_NOT_GIVEN_CELL = RANKS_EXACT_NOT_GIVEN_CELL;

export const FRIEDMAN_ROW_LABELS = {
  q: 'Q (chi-square)',
  df: 'df',
  p: 'p',
  kendallsW: "Kendall's W",
} as const;

/** Decision 654: under the Wilcoxon table, the two columns as text. */
export function relatedRanksWilcoxonSentence(first: string, second: string): string {
  return `The differences are ${first} less ${second}; a pair with no difference is left out of the ranks. V is the sum of the positive differences' ranks. z and the matched-pairs rank-biserial correlation are positive when ${first}'s values tend to be the larger.`;
}

/** Decision 654: under the Friedman table. */
export function relatedRanksFriedmanSentence(k: number): string {
  return `Within each row, the columns' values are ranked from 1 to ${k}, tied values sharing their mean rank.`;
}

/** A number, or its sentence where its number would be. */
export function relatedRanksStatText(s: RelatedRanksStat): string {
  return 'value' in s ? shownNumber(s.value) : relatedRanksSentence({ code: s.not_computable });
}

export const RELATED_RANKS_DIGITS_NOTE = RANKS_DIGITS_NOTE;

export const RELATED_RANKS_EXPORT_HEADING = RANKS_EXPORT_HEADING;

export const RELATED_RANKS_EXPORT_CHOICES = [
  { format: 'csv', label: 'The result tables (CSV)', file: 'related_ranks.csv', mime: 'text/csv' },
  { format: 'manifestJson', label: 'The run manifest (JSON)', file: 'related_ranks_manifest.json', mime: 'application/json' },
] as const;

export const RELATED_RANKS_EXPORT_TEXT = RANKS_EXPORT_TEXT;

/** Rows used is the complete rows, so the three counts add up. */
export const RELATED_RANKS_ROWS_USED_NOTE = 'The complete rows. Rows read is rows used plus the rows left out above.';
