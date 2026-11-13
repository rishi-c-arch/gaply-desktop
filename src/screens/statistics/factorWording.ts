// Gaply — the factor analysis's words (Research OS, Phase 9B-3, decisions 729 and 730).
// Every code the card and dialog can show has one sentence here, keyed by the closed set in
// factorCodes.json, which src-tauri/tests/stats_factor_wire.rs pins to FactorCode, RunCode,
// WorkspaceCode and CapabilityCode exactly. A code another method also has takes that
// method's sentence where it says what the factor analysis means (the rank tests', alpha's
// or the regression's); a shared sentence that names another method, another limit or
// another idea is written here instead. The run's, the registry's and the workspace's
// sentences are the related rank tests', but the two that name the method. Each limitation
// is the registry's sentence for its limit (capabilityWording.ts). A refusal from the file
// ingest is said by methodsIngestWording.
//
// The tone is neutral (decision 66): a factor analysis says what the numbers are; nothing
// here passes or fails a KMO, a loading or a factor, or gives a band a name. "Remove" and
// "Remove all" are labels, and the verdict list refuses them, so the settings take them from
// windowWording and they are not here. factorWording.vitest.ts reads every sentence for the
// words a verdict would use.
import { refusalOf } from '../statsverifier/l1/l1Wording';
import { methodsIngestSentence } from './methodsIngestWording';
import type { WireCode } from '../statsverifier/statsL1Types';
// The run manager's runs kept, pinned to runs::MAX_RUNS.
import LIMITS from './describeLimits.json';
// Pinned to stats_factor::MAX_ITEMS by stats_factor_wire.rs.
import FACTOR_LIMITS from './fixtures/factorLimits.json';
import { CAPABILITY_WORDS } from './capabilityWording';
import { RANKS_WORDS } from './ranksWording';
import { ALPHA_WORDS } from './alphaWording';
import {
  RELATED_RANKS_DIGITS_NOTE,
  RELATED_RANKS_EXPORT_CHOICES,
  RELATED_RANKS_EXPORT_HEADING,
  RELATED_RANKS_EXPORT_TEXT,
  RELATED_RANKS_ROWS_USED_NOTE,
  RELATED_RANKS_WORDS,
  type RelatedRanksNames,
} from './relatedRanksWording';
import { LINEAR_REGRESSION_WORDS, OVERFLOW_SENTENCE, TOO_SMALL_SENTENCE } from './linearRegressionWording';
import type { Coded } from './anovaWording';
import type { Extraction, FactorStat, Rotation } from './factorTypes';
import { shownNumber } from './ttestWording';

/** The most items a run takes: stats_factor::MAX_ITEMS, from the wire fixture. */
export const MAX_ITEMS: number = FACTOR_LIMITS.maxItems;
/** The fewest items a run takes: stats_factor::MIN_ITEMS (stats_factor/mod.rs:84). */
export const MIN_ITEMS = 3;
/** The fewest iterations: 1. The most: stats_factor::MAX_ITERATIONS (stats_factor/mod.rs:89). */
export const MIN_ITERATIONS = 1;
export const MAX_ITERATIONS = 1000;
/** The convergence's range: stats_factor::MIN_CONVERGENCE and MAX_CONVERGENCE
 *  (stats_factor/mod.rs:91, :93). */
export const MIN_CONVERGENCE = 1e-12;
export const MAX_CONVERGENCE = 0.1;
/** The window's starting values, SPSS's defaults (factor_analysis.md:39-40). */
export const DEFAULT_ITERATIONS = 25;
export const DEFAULT_CONVERGENCE = 0.001;

/** A column's name in a sentence, from the user's own screen. */
export type FactorNames = RelatedRanksNames;
const NO_NAMES: FactorNames = { column: (a) => `C${a}` };

const num = (d: Coded, k: string): number | undefined => {
  const v = d.params?.[k];
  return typeof v === 'number' ? v : undefined;
};
const cells = (n: number | undefined) => (n === 1 ? '1 cell' : `${n ?? 0} cells`);
const rows = (n: number | undefined) => (n === 1 ? '1 row' : `${n ?? 0} rows`);
const has = (n: number | undefined) => (n === 1 ? 'has' : 'have');
const itemsWord = (n: number | undefined) => (n === 1 ? '1 item' : `${n ?? 0} items`);
const column = (d: Coded, n: FactorNames) => n.column(num(d, 'column') ?? 0);

type Words = (d: Coded, names: FactorNames) => string;

/** The rank tests' sentence for a code whose suffix they share, as the regression takes it. */
const asRanks = (suffix: string): Words => (d, n) => RANKS_WORDS[`stats.ranks.${suffix}`](d, { ...n, level: (i) => `level ${i + 1}` });
/** Alpha's sentence for a code whose suffix it shares (an item's rule). */
const asAlpha = (suffix: string): Words => (d, n) => ALPHA_WORDS[`stats.alpha.${suffix}`](d, n);
/** The regression's sentence for a code whose suffix it shares. */
const asRegression = (suffix: string): Words => (d, n) => LINEAR_REGRESSION_WORDS[`stats.linear_regression.${suffix}`](d, n);
const fromRelated = (code: string): Words => RELATED_RANKS_WORDS[code] as unknown as Words;
/** The registry's sentence for one of the method's limits (capabilityWording.ts). */
const asLimit = (limit: string): Words => () => (CAPABILITY_WORDS as Record<string, string>)[`stats.capability.limit.analyze.${limit}`];

/** Decision 730's not-computable sentences, word for word. */
export const DISTRIBUTION_OUT_OF_RANGE_SENTENCE =
  'Not computable: this needs a chi-square distribution beyond the range where Gaply has measured its accuracy.';
export const CORRELATIONS_SINGULAR_SENTENCE =
  'Not computable: the correlation matrix is singular, as when an item is a combination of other items or there are no more rows than items.';
export const NO_CORRELATION_SENTENCE =
  'Not computable: the items this measure covers correlate with no other item, so there is nothing for it to compare.';
export { OVERFLOW_SENTENCE, TOO_SMALL_SENTENCE };

/** Decision 730's notes, word for word. */
export const FACTOR_NOTES = {
  communalityAboveOne:
    'During principal axis factoring a communality went above 1 (a Heywood case). A communality is a share of an item\'s variance, so these loadings do not describe a possible factor model as they stand. SPSS gives the same warning.',
  extractionEigenvalueNegative:
    'At some iteration an eigenvalue among those extracted was below 0; as SPSS does, its absolute value was used for the loadings.',
  oneFactorNotRotated: 'One factor was extracted, so there is nothing to rotate.',
  rotationNotConverged: 'The rotation did not converge within the maximum iterations, so no rotated solution is given.',
  rotationSingular: "Promax's fit cannot be computed for these loadings, so no rotated solution is given.",
  eigenvaluesNotDistinct:
    'The last eigenvalue extracted and the next are equal within 1e-8 of the larger, so these loadings are one of many that fit equally well.',
} as const;

/** One sentence per code. A code with no entry is shown as itself, never hidden;
 *  factorWording.vitest.ts keeps every code in factorCodes.json covered. */
export const FACTOR_WORDS: Record<string, Words> = {
  'stats.factor.unknown_capability': asRanks('unknown_capability'),
  'stats.factor.capability_version_mismatch': (d) =>
    `These settings were made for version ${num(d, 'found') ?? 0} of the factor analysis, and this is version ${num(d, 'expected') ?? 0}.`,
  'stats.factor.input_count': (d) => `These settings name ${num(d, 'count') ?? 0} files, and a factor analysis reads exactly one.`,
  'stats.factor.input_mismatch': asRanks('input_mismatch'),
  'stats.factor.preparation_mismatch': asRanks('preparation_mismatch'),
  'stats.factor.missing_code_not_finite': asRanks('missing_code_not_finite'),
  'stats.factor.missing_code_repeated': asAlpha('missing_code_repeated'),
  'stats.factor.missing_codes_out_of_order': asAlpha('missing_codes_out_of_order'),
  'stats.factor.too_few_items': (d) =>
    `${itemsWord(num(d, 'count'))} chosen, and a factor analysis needs at least ${num(d, 'limit') ?? MIN_ITEMS}.`,
  'stats.factor.too_many_items': (d) => `${itemsWord(num(d, 'count'))} chosen, and a factor analysis takes at most ${num(d, 'limit') ?? MAX_ITEMS}.`,
  'stats.factor.items_out_of_order': asAlpha('items_out_of_order'),
  'stats.factor.item_repeated': (d, n) => `Column ${column(d, n)} is chosen twice as an item.`,
  'stats.factor.unknown_column': asRanks('unknown_column'),
  'stats.factor.column_non_finite': (d, n) =>
    `Column ${column(d, n)} holds an infinity or "not a number" in ${cells(num(d, 'count'))}, so it cannot be used as an item.`,
  'stats.factor.column_holds_text': (d, n) => `Column ${column(d, n)} holds text in ${cells(num(d, 'count'))}, so it cannot be used as an item.`,
  'stats.factor.code_equals_score': asRanks('code_equals_score'),
  'stats.factor.factors_out_of_range': (d) => `${num(d, 'count') ?? 0} factors are asked for, and these items allow 1 to ${num(d, 'limit') ?? 0}.`,
  'stats.factor.iterations_out_of_range': (d) =>
    `${num(d, 'count') ?? 0} iterations are asked for, and a run takes ${MIN_ITERATIONS} to ${num(d, 'limit') ?? MAX_ITERATIONS}.`,
  'stats.factor.convergence_out_of_range': () => `The convergence is outside ${MIN_CONVERGENCE} to ${MAX_CONVERGENCE}, or is not a number.`,
  'stats.factor.no_complete_rows': () => 'No row has a value in every chosen item, so there is nothing to analyse.',
  'stats.factor.too_few_rows': (d) =>
    `${rows(num(d, 'count'))} ${has(num(d, 'count'))} a value in every chosen item, and a factor analysis needs at least ${num(d, 'limit') ?? 0}.`,
  'stats.factor.item_constant': (d, n) =>
    `Column ${column(d, n)} takes one value in every complete row, so its correlations are not defined. Take it out of the items.`,
  // Decision 730's refusals, word for word.
  'stats.factor.correlation_out_of_range': (d, n) =>
    `${column(d, n)}'s values are too large or too small for its correlations to be computed at this scale.`,
  'stats.factor.no_eigenvalue_over_one': () =>
    'No eigenvalue of the correlation matrix is above 1, so that rule extracts no factor. Choose a fixed number of factors instead.',
  // A refusal names its item; a statistic's code has no column (decision 730).
  'stats.factor.correlations_singular': (d, n) =>
    num(d, 'column') === undefined
      ? CORRELATIONS_SINGULAR_SENTENCE
      : `${column(d, n)} is a combination of the items before it, or within one part in a billion of its variance of being one, so the correlation matrix is singular and principal axis factoring cannot start. Take it out of the items, or use principal components.`,
  'stats.factor.extraction_eigenvalue_not_positive': () =>
    'A component asked for has an eigenvalue of 0 or below, so it has no loadings. Ask for fewer components.',
  'stats.factor.extraction_not_converged': (d) =>
    `Principal axis factoring did not converge within ${num(d, 'limit') ?? 0} iterations. Raise the maximum iterations, or ask for fewer factors.`,
  'stats.factor.rotation_not_converged': () => FACTOR_NOTES.rotationNotConverged,
  'stats.factor.rotation_singular': () => FACTOR_NOTES.rotationSingular,
  'stats.factor.extraction_eigenvalue_negative': () => FACTOR_NOTES.extractionEigenvalueNegative,
  'stats.factor.eigen_not_converged': () => "The eigenvalues of these data could not be found to the arithmetic's precision.",
  'stats.factor.suspect_missing_code': asRegression('suspect_missing_code'),
  'stats.factor.too_small': () => TOO_SMALL_SENTENCE,
  'stats.factor.no_correlation': () => NO_CORRELATION_SENTENCE,
  'stats.factor.distribution_out_of_range': () => DISTRIBUTION_OUT_OF_RANGE_SENTENCE,
  'stats.factor.overflow': () => OVERFLOW_SENTENCE,
  'stats.factor.communality_above_one': () => FACTOR_NOTES.communalityAboveOne,
  'stats.factor.one_factor_not_rotated': () => FACTOR_NOTES.oneFactorNotRotated,
  'stats.factor.eigenvalues_not_distinct': () => FACTOR_NOTES.eigenvaluesNotDistinct,
  // Each row is counted once, under its first item without a value: alpha's rule and words.
  'stats.factor.missing_value': asAlpha('missing_value'),
  'stats.factor.missing_code': asAlpha('missing_code'),
  // Each limitation as the registry's limit with the same meaning says it.
  'stats.factor.no_ml_or_other_extraction': asLimit('factor_extraction_two_methods'),
  'stats.factor.no_oblique_other_than_promax': asLimit('factor_rotation_varimax_promax'),
  'stats.factor.promax_kappa_four': asLimit('factor_promax_kappa_four'),
  'stats.factor.no_factor_scores': asLimit('factor_no_scores'),
  'stats.factor.no_matrices_shown': asLimit('factor_matrices_not_shown'),
  'stats.factor.pearson_only': asLimit('factor_pearson_only'),
  'stats.factor.listwise_only': asLimit('factor_listwise_only'),
  'stats.factor.no_scree_plot': asLimit('factor_no_scree_plot'),
  'stats.factor.no_weights': asRanks('no_weights'),
  'run.unknown_run': () => `This run is no longer held. Gaply keeps the last ${LIMITS.maxRuns} runs in memory only; run the factor analysis again.`,
  'run.wrong_kind': () => 'This run is not a factor analysis, so this screen cannot read it.',
  'run.run_busy': fromRelated('run.run_busy'),
  'run.unknown_input': fromRelated('run.unknown_input'),
  'run.not_finished': fromRelated('run.not_finished'),
  'run.no_result': fromRelated('run.no_result'),
  'run.cancelled': fromRelated('run.cancelled'),
  'run.timed_out': fromRelated('run.timed_out'),
  'run.resource_limit': fromRelated('run.resource_limit'),
  'run.illegal_transition': fromRelated('run.illegal_transition'),
  'run.already_terminal': fromRelated('run.already_terminal'),
  'capability.unknown_capability': fromRelated('capability.unknown_capability'),
  'capability.version_mismatch': fromRelated('capability.version_mismatch'),
  'capability.not_runnable': fromRelated('capability.not_runnable'),
  'capability.confirmation_missing': fromRelated('capability.confirmation_missing'),
  'capability.confirmation_unmatched': fromRelated('capability.confirmation_unmatched'),
  'workspace.unavailable': fromRelated('workspace.unavailable'),
  'workspace.not_ready': fromRelated('workspace.not_ready'),
  'workspace.inactive': fromRelated('workspace.inactive'),
  'workspace.wrong_workspace': fromRelated('workspace.wrong_workspace'),
  'workspace.run_active': fromRelated('workspace.run_active'),
  'workspace.illegal_transition': fromRelated('workspace.illegal_transition'),
};

/** The sentence for a code: a diagnostic, an exclusion, a note or a limitation. */
export function factorSentence(d: Coded, names: FactorNames = NO_NAMES): string {
  const f = FACTOR_WORDS[d.code];
  return f ? f(d, names) : `Gaply has no wording for the code ${d.code}.`;
}

/** One plain sentence for a command's refusal. */
export function factorRefusalSentence(e: unknown): string {
  const c = refusalOf(e);
  if (!c) {
    const message = e instanceof Error ? e.message : typeof e === 'string' ? e : '';
    return message.trim() ? `This could not be done: ${message.trim()}` : 'This could not be done.';
  }
  if (c.code in FACTOR_WORDS) {
    const params = (c as { params?: unknown }).params;
    return factorSentence({ code: c.code, params: params && typeof params === 'object' ? (params as Record<string, unknown>) : {} });
  }
  return methodsIngestSentence(c as WireCode);
}

/** Why a run that ended without a result has none. */
export function factorRunEndedSentence(state: string): string {
  switch (state) {
    case 'cancelled':
      return factorSentence({ code: 'run.cancelled' });
    case 'timed_out':
      return factorSentence({ code: 'run.timed_out' });
    case 'resource_limit':
      return factorSentence({ code: 'run.resource_limit' });
    case 'refused':
      return 'Gaply refused these settings, so nothing ran. The reasons are below.';
    case 'needs_user_input':
      return 'The settings need an answer from you, so nothing ran. See below.';
    case 'failed':
      return 'The factor analysis could not be computed. The reason is below.';
    default:
      return factorSentence({ code: 'run.no_result' });
  }
}

/* ---- the settings (decision 730) ------------------------------------------ */

export const FA_RULE = "Every chosen item enters the analysis at once, in the file's order. A row is used only when every item has a value.";
export const FA_ITEMS_LEGEND = 'Items';
export const FA_EXTRACTION_LEGEND = 'Extraction';
export const FA_EXTRACTION_LABELS: Record<Extraction, string> = {
  principal_components: 'Principal components',
  principal_axis: 'Principal axis factoring',
};
export const FA_FACTORS_LEGEND = 'Number of factors';
export const FA_FACTORS_KAISER = 'Every factor whose eigenvalue is above 1';
export const FA_FACTORS_FIXED = 'A fixed number';
export const FA_FACTORS_INPUT = 'Fixed number of factors';
export const FA_ROTATION_LEGEND = 'Rotation';
export const FA_ROTATION_LABELS: Record<Rotation, string> = { none: 'None', varimax: 'Varimax', promax: 'Promax (power 4)' };
export const FA_ITERATIONS_LABEL = 'Maximum iterations, for the extraction and the rotation';
export const FA_CONVERGENCE_LABEL = 'Convergence, for principal axis factoring';
export const FA_CODES_LEGEND = 'Candidate missing codes';
export const FA_CHOOSE_ITEMS = 'Choose at least 3 items.';
export const FA_ITERATIONS_RANGE = 'Enter a whole number of iterations from 1 to 1000.';
export const FA_CONVERGENCE_RANGE = 'Enter a convergence from 1e-12 to 0.1.';

export function factorTooManyItems(n: number): string {
  return `Choose ${MAX_ITEMS} items or fewer; ${n} are chosen.`;
}

export function factorFactorsRange(max: number): string {
  return `Enter a whole number of factors from 1 to ${max}.`;
}

/** A candidate missing code in the chosen items, as a question naming the value and where it
 *  appears, its places written as alphaMissingCodeQuestion writes them. */
export function factorMissingCodeQuestion(code: number, where: ReadonlyArray<{ header: string; cells: number }>): string {
  const places = where.map((w) => `"${w.header}" (${cells(w.cells)})`).join(', ');
  return `Is ${code} a code for a missing value? It appears in ${places}. If you tick it, it is treated as missing in every item, and a row with it is left out.`;
}

/* ---- the card (decision 730) ---------------------------------------------- */

/** "Component" for principal components, "Factor" for principal axis, as SPSS names them. */
export const factorWord = (e: Extraction): string => (e === 'principal_components' ? 'Component' : 'Factor');

export const FA_HEADINGS = {
  descriptives: 'Descriptive statistics',
  kmo: "KMO and Bartlett's test",
  msa: "Each item's sampling adequacy",
  communalities: 'Communalities',
  variance: 'Total variance explained',
  matrix: (e: Extraction) => (e === 'principal_components' ? 'Component matrix' : 'Factor matrix'),
  rotated: (e: Extraction) => (e === 'principal_components' ? 'Rotated component matrix' : 'Rotated factor matrix'),
  transformation: (e: Extraction) => (e === 'principal_components' ? 'Component transformation matrix' : 'Factor transformation matrix'),
  pattern: 'Pattern matrix',
  structure: 'Structure matrix',
  correlations: (e: Extraction) => (e === 'principal_components' ? 'Component correlation matrix' : 'Factor correlation matrix'),
} as const;

export const FA_LABELS = {
  item: 'Item',
  mean: 'Mean',
  sd: 'Standard deviation',
  rowsUsed: 'Rows used',
  kmo: 'Kaiser-Meyer-Olkin measure of sampling adequacy',
  bartlett: "Bartlett's test of sphericity",
  chiSquare: 'Approximate chi-square',
  df: 'df',
  p: 'p',
  determinant: 'Determinant of the correlation matrix',
  logDeterminant: 'Natural log of the determinant',
  msa: 'Measure of sampling adequacy',
  initial: 'Initial',
  extraction: 'Extraction',
  initialEigenvalues: 'Initial eigenvalues',
  extractionSums: 'Extraction sums of squared loadings',
  rotationSums: 'Rotation sums of squared loadings',
  total: 'Total',
  percent: '% of variance',
  cumulative: 'Cumulative %',
} as const;

/** The matrix's footnote: the extraction. */
export function factorExtractedFootnote(e: Extraction, m: number, iterations: number): string {
  if (e === 'principal_components') return m === 1 ? '1 component extracted.' : `${m} components extracted.`;
  const factors = m === 1 ? '1 factor' : `${m} factors`;
  const its = iterations === 1 ? '1 iteration' : `${iterations} iterations`;
  return `${factors} extracted, in ${its}.`;
}

/** The rotated matrix's footnote. */
export function factorRotationFootnote(method: Exclude<Rotation, 'none'>, cycles: number): string {
  const c = cycles === 1 ? '1 cycle' : `${cycles} cycles`;
  return method === 'varimax'
    ? `Varimax with Kaiser normalization; the rotation converged in ${c}.`
    : `Promax (power 4) with Kaiser normalization; its varimax converged in ${c}.`;
}

/** Under the variance table, for promax (decision 730). */
export const FA_PROMAX_SUMS_SENTENCE =
  "Promax's factors are correlated, so the rotation sums of squared loadings that SPSS lists for them are not given here.";

/** A number, or its sentence where its number would be. */
export function factorStatText(s: FactorStat): string {
  return 'value' in s ? shownNumber(s.value) : factorSentence({ code: s.not_computable });
}

export const FACTOR_DIGITS_NOTE = RELATED_RANKS_DIGITS_NOTE;

export const FACTOR_EXPORT_HEADING = RELATED_RANKS_EXPORT_HEADING;

/** Decision 730: the regression's choices' words, the factor analysis's files. */
export const FACTOR_EXPORT_CHOICES = [
  { format: 'csv', label: RELATED_RANKS_EXPORT_CHOICES[0].label, file: 'factor_analysis.csv', mime: 'text/csv' },
  { format: 'manifestJson', label: RELATED_RANKS_EXPORT_CHOICES[1].label, file: 'factor_analysis_manifest.json', mime: 'application/json' },
] as const;

export const FACTOR_EXPORT_TEXT = RELATED_RANKS_EXPORT_TEXT;

/** Rows used is the complete rows, so the three counts add up. */
export const FACTOR_ROWS_USED_NOTE = RELATED_RANKS_ROWS_USED_NOTE;
