// Gaply — the confirmatory factor analysis's words (Research OS, Phase 9C-3, decisions 782 and
// 783). Every code the card and dialog can show has one sentence here, keyed by the closed set
// in cfaCodes.json, which src-tauri/tests/stats_cfa_wire.rs pins to CfaCode, RunCode,
// WorkspaceCode and CapabilityCode exactly. Decision 783's sentences come first. A code another
// method also has takes that method's sentence (the factor analysis's first, then the
// regression's, the rank tests' or alpha's); where that sentence names another method or says
// "item", it is reworded here for this method, and each rewording is listed in the step's
// report. The run's, the registry's and the workspace's sentences are the related rank tests',
// but the two that name the method. Each limitation is the registry's sentence for its limit
// (capabilityWording.ts), and no_weights the rank tests'. A refusal from the file ingest is
// said by methodsIngestWording.
//
// Three codes are both a not-computable value and something else: negative_variance and
// df_zero are also notes, and overflow is also the run's refusal naming a column. A cell's
// sentence is CFA_NOT_COMPUTABLE's; a note's and a diagnostic's are CFA_WORDS'.
//
// The tone is neutral (decision 66): the card says what the numbers are; nothing here passes
// or fails a model, gives a fit measure a cutoff or a band's name, or uses a word a verdict
// would use. cfaWording.vitest.ts reads every sentence for those words.
import { refusalOf } from '../statsverifier/l1/l1Wording';
import { methodsIngestSentence } from './methodsIngestWording';
import type { WireCode } from '../statsverifier/statsL1Types';
// The run manager's runs kept, pinned to runs::MAX_RUNS.
import LIMITS from './describeLimits.json';
import { cfaLimits } from './fixtures/cfaFixtures';
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
import { LINEAR_REGRESSION_WORDS } from './linearRegressionWording';
import { DISTRIBUTION_OUT_OF_RANGE_SENTENCE, OVERFLOW_SENTENCE, TOO_SMALL_SENTENCE } from './factorWording';
import type { Coded } from './anovaWording';
import type { CfaStat } from './cfaTypes';
import { FIT_DASH, FIT_ROWS, FIT_TABLES, FIT_THREE_MODELS, fitCellText, fitDashLine, fitWholeText, type FitColumn, type FitRow } from './ModelFitTables';
import { shownNumber } from './ttestWording';

/** The most columns in a model: stats_cfa::MAX_ITEMS, from the wire fixture. */
export const MAX_ITEMS: number = cfaLimits.maxItems;
/** The most factors: stats_cfa::MAX_FACTORS, from the wire fixture. */
export const MAX_FACTORS: number = cfaLimits.maxFactors;
/** The fewest columns measuring one factor: stats_cfa::MIN_FACTOR_ITEMS, from the wire fixture. */
export const MIN_FACTOR_ITEMS: number = cfaLimits.minFactorItems;
/** The fewest columns in a model: stats_cfa::MIN_ITEMS, from the wire fixture. */
export const MIN_ITEMS: number = cfaLimits.minItems;
/** The iterations before a run refuses as cfa_not_converged: stats_cfa::MAX_ITERATIONS
 *  (stats_cfa/mod.rs:97), its Rust twin. */
export const MAX_ITERATIONS = 500;

/** A column's name in a sentence, from the user's own screen. */
export type CfaNames = RelatedRanksNames;
const NO_NAMES: CfaNames = { column: (a) => `C${a}` };

const num = (d: Coded, k: string): number | undefined => {
  const v = d.params?.[k];
  return typeof v === 'number' ? v : undefined;
};
const cells = (n: number | undefined) => (n === 1 ? '1 cell' : `${n ?? 0} cells`);
const rows = (n: number | undefined) => (n === 1 ? '1 row' : `${n ?? 0} rows`);
const has = (n: number | undefined) => (n === 1 ? 'has' : 'have');
const columnsWord = (n: number | undefined) => (n === 1 ? '1 column' : n === 0 ? 'no columns' : `${n ?? 0} columns`);
const iterationsWord = (n: number | undefined) => (n === 1 ? '1 iteration' : `${n ?? 0} iterations`);
const was = (n: number | undefined) => (n === 1 ? 'was' : 'were');
const column = (d: Coded, n: CfaNames) => n.column(num(d, 'column') ?? 0);

type Words = (d: Coded, names: CfaNames) => string;

/** The rank tests' sentence for a code whose suffix they share. */
const asRanks = (suffix: string): Words => (d, n) => RANKS_WORDS[`stats.ranks.${suffix}`](d, { ...n, level: (i) => `level ${i + 1}` });
/** Alpha's sentence for a code whose suffix it shares. */
const asAlpha = (suffix: string): Words => (d, n) => ALPHA_WORDS[`stats.alpha.${suffix}`](d, n);
/** The regression's sentence for a code whose suffix it shares. */
const asRegression = (suffix: string): Words => (d, n) => LINEAR_REGRESSION_WORDS[`stats.linear_regression.${suffix}`](d, n);
const fromRelated = (code: string): Words => RELATED_RANKS_WORDS[code] as unknown as Words;
/** The registry's sentence for one of the method's limits (capabilityWording.ts). */
const asLimit = (limit: string): Words => () => (CAPABILITY_WORDS as Record<string, string>)[`stats.capability.limit.analyze.${limit}`];

/** Decision 783's not-computable sentences, word for word; too_small, overflow and
 *  distribution_out_of_range are the factor analysis's. A cell shows these. */
export const CFA_NOT_COMPUTABLE: Record<string, string> = {
  'stats.cfa.fixed_parameter': "Fixed at 1, to set the factor's scale.",
  'stats.cfa.df_zero': 'Not defined at 0 degrees of freedom.',
  'stats.cfa.cmin_zero': 'Not defined when the chi-square is 0.',
  'stats.cfa.independence_cmin_zero': 'Not defined: the independence model fits exactly, so there is nothing to compare with.',
  'stats.cfa.too_few_rows_for_bcc': 'Not defined: this needs more rows than the number of columns plus 2.',
  'stats.cfa.negative_variance': 'Not defined: a variance it takes the square root of is at or below 0.',
  'stats.cfa.too_small': TOO_SMALL_SENTENCE,
  'stats.cfa.overflow': OVERFLOW_SENTENCE,
  'stats.cfa.distribution_out_of_range': DISTRIBUTION_OUT_OF_RANGE_SENTENCE,
};

/** Decision 783's notes, word for word. */
export const CFA_NOTES = {
  negativeVariance:
    'At least one variance estimate is below 0, so this solution is not admissible, as AMOS says of such a solution. A standardized estimate, correlation or squared multiple correlation is not given where its formula takes the square root of a variance at or below 0. Where only an error variance is below 0 they are given, and a squared multiple correlation can then be above 1.',
  factorCovarianceNotPositiveDefinite:
    "The factors' estimated covariance matrix is not positive definite, so this solution is not admissible, as AMOS says of such a solution.",
  dfZero:
    'The model has 0 degrees of freedom: it has as many parameters as the data have variances and covariances, so it reproduces them exactly, and the measures that need degrees of freedom are not defined.',
} as const;

/** One sentence per code. A code with no entry is shown as itself, never hidden;
 *  cfaWording.vitest.ts keeps every code in cfaCodes.json covered. */
export const CFA_WORDS: Record<string, Words> = {
  // Decision 783's validation sentences, word for word.
  'stats.cfa.no_factors': () => 'The model has no factor. Choose a factor for a column.',
  'stats.cfa.too_many_factors': (d) => `The model has ${num(d, 'count') ?? 0} factors; Gaply fits at most ${num(d, 'limit') ?? MAX_FACTORS}.`,
  'stats.cfa.factor_too_few_items': (d) => cfaFactorTooFew(num(d, 'index') ?? 0, num(d, 'count') ?? 0, num(d, 'limit') ?? MIN_FACTOR_ITEMS),
  'stats.cfa.two_item_factor_uncorrelated': (d) =>
    `Factor ${num(d, 'index') ?? 0} has two columns and is not correlated with any other factor, so no data can identify it. Tick "The factors are correlated", or give it a third column.`,
  'stats.cfa.items_out_of_order': (d, n) =>
    `These settings list ${column(d, n)} out of the file's order, which this screen never does. Choose the factors again.`,
  'stats.cfa.factors_out_of_order': (d) =>
    `These settings list factor ${num(d, 'index') ?? 0} out of the order of the factors' first columns, which this screen never does. Choose the factors again.`,
  // The factor analysis's sentence, which reads its limit from the code's param, reworded.
  'stats.cfa.too_few_rows': (d) =>
    `${rows(num(d, 'count'))} ${has(num(d, 'count'))} a value in every column in the model, and the confirmatory factor analysis needs at least ${num(d, 'limit') ?? 0}.`,
  // Decision 783's refusals, word for word.
  'stats.cfa.sample_covariance_singular': (d, n) =>
    `${column(d, n)} is, to within one part in a billion of its variance, a combination of the columns before it in the model, so their covariance matrix is singular and the model cannot be fitted. Take it out of the model.`,
  // The run's refusal names its column; a statistic's code has none (as the factor
  // analysis's correlations_singular chooses).
  'stats.cfa.overflow': (d, n) =>
    num(d, 'column') === undefined
      ? OVERFLOW_SENTENCE
      : `${column(d, n)}'s values are too large to square, so its variance cannot be computed. Divide the column by a power of 10 and run again.`,
  'stats.cfa.model_not_identified': (d) =>
    num(d, 'count') === undefined
      ? "The model's parameters cannot be told apart with these data: the information matrix was singular at the start of every attempt Gaply made. This happens when a factor's columns are uncorrelated with each other or with the other factors' columns."
      : `The model has ${num(d, 'count')} more parameters than the data have variances and covariances, so it is not identified.`,
  'stats.cfa.cfa_not_converged': (d) =>
    `Maximum likelihood did not converge from Gaply's starting points; the longest attempt stopped after ${iterationsWord(num(d, 'count'))}, of at most ${num(d, 'limit') ?? MAX_ITERATIONS}. A model with fewer factors, or with columns that measure their factor more strongly, may converge.`,
  // Decision 783's notes; their cells' sentences are CFA_NOT_COMPUTABLE's.
  'stats.cfa.negative_variance': () => CFA_NOTES.negativeVariance,
  'stats.cfa.factor_covariance_not_positive_definite': () => CFA_NOTES.factorCovarianceNotPositiveDefinite,
  'stats.cfa.df_zero': () => CFA_NOTES.dfZero,
  // Decision 783's not-computable sentences, which are only ever a cell's.
  'stats.cfa.fixed_parameter': () => CFA_NOT_COMPUTABLE['stats.cfa.fixed_parameter'],
  'stats.cfa.cmin_zero': () => CFA_NOT_COMPUTABLE['stats.cfa.cmin_zero'],
  'stats.cfa.independence_cmin_zero': () => CFA_NOT_COMPUTABLE['stats.cfa.independence_cmin_zero'],
  'stats.cfa.too_few_rows_for_bcc': () => CFA_NOT_COMPUTABLE['stats.cfa.too_few_rows_for_bcc'],
  'stats.cfa.too_small': () => TOO_SMALL_SENTENCE,
  'stats.cfa.distribution_out_of_range': () => DISTRIBUTION_OUT_OF_RANGE_SENTENCE,
  // Shared, as the method that has them says them (the rank tests' are the ANOVA's).
  'stats.cfa.unknown_capability': asRanks('unknown_capability'),
  'stats.cfa.input_mismatch': asRanks('input_mismatch'),
  'stats.cfa.preparation_mismatch': asRanks('preparation_mismatch'),
  'stats.cfa.missing_code_not_finite': asRanks('missing_code_not_finite'),
  'stats.cfa.unknown_column': asRanks('unknown_column'),
  'stats.cfa.code_equals_score': asRanks('code_equals_score'),
  'stats.cfa.missing_code_repeated': asAlpha('missing_code_repeated'),
  'stats.cfa.missing_codes_out_of_order': asAlpha('missing_codes_out_of_order'),
  'stats.cfa.suspect_missing_code': asRegression('suspect_missing_code'),
  // The factor analysis's, reworded for this method and its columns.
  'stats.cfa.capability_version_mismatch': (d) =>
    `These settings were made for version ${num(d, 'found') ?? 0} of the confirmatory factor analysis, and this is version ${num(d, 'expected') ?? 0}.`,
  'stats.cfa.input_count': (d) => `These settings name ${num(d, 'count') ?? 0} files, and a confirmatory factor analysis reads exactly one.`,
  'stats.cfa.too_few_items': (d) =>
    `The model has ${columnsWord(num(d, 'count'))}, and the confirmatory factor analysis needs at least ${num(d, 'limit') ?? MIN_ITEMS}.`,
  'stats.cfa.too_many_items': (d) =>
    `The model has ${columnsWord(num(d, 'count'))}, and the confirmatory factor analysis takes at most ${num(d, 'limit') ?? MAX_ITEMS}.`,
  'stats.cfa.item_repeated': (d, n) => `Column ${column(d, n)} is in the model twice.`,
  'stats.cfa.column_non_finite': (d, n) =>
    `Column ${column(d, n)} holds an infinity or "not a number" in ${cells(num(d, 'count'))}, so it cannot be in the model.`,
  'stats.cfa.column_holds_text': (d, n) => `Column ${column(d, n)} holds text in ${cells(num(d, 'count'))}, so it cannot be in the model.`,
  'stats.cfa.no_complete_rows': () => 'No row has a value in every column in the model, so there is nothing to analyse.',
  'stats.cfa.item_constant': (d, n) =>
    `Column ${column(d, n)} takes one value in every complete row, so its correlations are not defined. Take it out of the model.`,
  // Alpha's exclusions, each row counted once under its first column without a value, reworded.
  'stats.cfa.missing_value': (d, n) =>
    `${rows(d.count)} had no value (an empty cell or a missing-value token) in ${column(d, n)}, the first column in the model without one, and ${was(d.count)} left out.`,
  'stats.cfa.missing_code': (d, n) =>
    `${rows(d.count)} held a missing code you ticked in ${column(d, n)}, the first column in the model without a value, and ${was(d.count)} left out.`,
  // Each limitation as the registry's limit with the same meaning says it.
  'stats.cfa.ml_only': asLimit('cfa_ml_only'),
  'stats.cfa.no_means': asLimit('cfa_no_means'),
  'stats.cfa.one_group_only': asLimit('cfa_one_group'),
  'stats.cfa.simple_structure_only': asLimit('cfa_simple_structure'),
  'stats.cfa.marker_identification_only': asLimit('cfa_marker_identification'),
  'stats.cfa.no_modification_indices': asLimit('cfa_no_modification_indices'),
  'stats.cfa.normal_theory_standard_errors': asLimit('cfa_normal_theory'),
  'stats.cfa.no_srmr': asLimit('cfa_no_srmr'),
  'stats.cfa.listwise_only': asLimit('cfa_listwise_only'),
  'stats.cfa.no_weights': asRanks('no_weights'),
  // Decision 783's run sentences that name the method.
  'run.unknown_run': () => `This run is no longer held. Gaply keeps the last ${LIMITS.maxRuns} runs in memory only; run the confirmatory factor analysis again.`,
  'run.wrong_kind': () => 'This run is not a confirmatory factor analysis, so this screen cannot read it.',
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

/** Decision 783: "Factor {n} has {count} columns; each factor needs at least {limit}.", with
 *  "1 column" in the singular and "no columns" at 0. The settings' missing sentence and the
 *  validation code's alike. */
export function cfaFactorTooFew(index: number, count: number, limit: number): string {
  return `Factor ${index} has ${columnsWord(count)}; each factor needs at least ${limit}.`;
}

/** The sentence for a code: a diagnostic, an exclusion, a note or a limitation. */
export function cfaSentence(d: Coded, names: CfaNames = NO_NAMES): string {
  const f = CFA_WORDS[d.code];
  return f ? f(d, names) : `Gaply has no wording for the code ${d.code}.`;
}

/** A not-computable value's sentence, as a cell says it. */
export function cfaCellSentence(code: string): string {
  return CFA_NOT_COMPUTABLE[code] ?? cfaSentence({ code });
}

/** One plain sentence for a command's refusal. */
export function cfaRefusalSentence(e: unknown): string {
  const c = refusalOf(e);
  if (!c) {
    const message = e instanceof Error ? e.message : typeof e === 'string' ? e : '';
    return message.trim() ? `This could not be done: ${message.trim()}` : 'This could not be done.';
  }
  if (c.code in CFA_WORDS) {
    const params = (c as { params?: unknown }).params;
    return cfaSentence({ code: c.code, params: params && typeof params === 'object' ? (params as Record<string, unknown>) : {} });
  }
  return methodsIngestSentence(c as WireCode);
}

/** Why a run that ended without a result has none. */
export function cfaRunEndedSentence(state: string): string {
  switch (state) {
    case 'cancelled':
      return cfaSentence({ code: 'run.cancelled' });
    case 'timed_out':
      return cfaSentence({ code: 'run.timed_out' });
    case 'resource_limit':
      return cfaSentence({ code: 'run.resource_limit' });
    case 'refused':
      return 'Gaply refused these settings, so nothing ran. The reasons are below.';
    case 'needs_user_input':
      return 'The settings need an answer from you, so nothing ran. See below.';
    case 'failed':
      return 'The confirmatory factor analysis could not be computed. The reason is below.';
    default:
      return cfaSentence({ code: 'run.no_result' });
  }
}

/* ---- the settings (decision 783) ------------------------------------------ */

export const CF_RULE =
  "Each column measures one factor. Factors are numbered by their first column in the file, and each factor's first column has its loading fixed at 1 to set the factor's scale, as AMOS does when a model is drawn. A row is used only when every column in the model has a value.";
export const CF_COLUMNS_HEADING = 'Numeric columns';
export const CF_NOT_IN_MODEL = 'Not in the model';
export const cfaFactorOption = (n: number): string => `Factor ${n}`;
export const CF_NEW_FACTOR = 'A new factor';
export const CF_FACTORS_HEADING = "Factors, each in the file's order";
export const cfaFactorHeading = (n: number): string => `Factor ${n}`;
export const CF_FIXED_MARK = '(loading fixed at 1)';
export const CF_NO_FACTOR = 'No factor yet. Choose a factor for a column on the left.';
export const CF_CORRELATED_LABEL = 'The factors are correlated';
export const CF_CORRELATED_LINE = 'Ticked, every pair of factors has a covariance to estimate. Unticked, those covariances are fixed at 0.';
export const CF_CODES_LEGEND = 'Candidate missing codes';
export const CF_PUT_COLUMNS = `Put at least ${MIN_ITEMS} columns in factors.`;
export const CF_AT_CAP = `The model has ${MAX_ITEMS} columns, the most Gaply fits; take one out to add another.`;

/** A candidate missing code in the model's columns, as a question naming the value and where
 *  it appears, its places written as alphaMissingCodeQuestion writes them. */
export function cfaMissingCodeQuestion(code: number, where: ReadonlyArray<{ header: string; cells: number }>): string {
  const places = where.map((w) => `"${w.header}" (${cells(w.cells)})`).join(', ');
  return `Is ${code} a code for a missing value? It appears in ${places}. If you tick it, it is treated as missing in every column in the model, and a row with it is left out.`;
}

/* ---- the card (decision 783) ---------------------------------------------- */

export const CF_HEADINGS = {
  notes: 'Notes for the model',
  estimates: 'Estimates',
  weights: 'Regression weights',
  standardized: 'Standardized regression weights',
  covariances: 'Covariances',
  correlations: 'Correlations',
  variances: 'Variances',
  smc: 'Squared multiple correlations',
  fit: 'Model fit',
} as const;

export const CF_LABELS = {
  moments: 'Distinct sample moments',
  parameters: 'Distinct parameters to be estimated',
  df: 'Degrees of freedom',
  estimate: 'Estimate',
  se: 'S.E.',
  cr: 'C.R.',
  p: 'P',
  /** The model-fit tables' first column, as AMOS heads it. */
  model: 'Model',
} as const;

export const CF_ROWS = FIT_ROWS;
export type CfaRow = FitRow;

export const cfaWeightRow = (column: string, factor: number): string => `${column} ← Factor ${factor}`;
export const cfaCovarianceRow = (f: number, g: number): string => `Factor ${f} ↔ Factor ${g}`;
export const cfaFactorRow = (f: number): string => `Factor ${f}`;
export const cfaErrorRow = (column: string): string => `Error of ${column}`;
export const CF_NO_COVARIANCES = "The factors' covariances are fixed at 0.";

/** Decision 783: the minimization's line. */
export function cfaIterationsSentence(n: number): string {
  if (n === 0) return "Maximum likelihood's starting values were already its minimum.";
  return `Maximum likelihood reached its minimum in ${iterationsWord(n)}.`;
}

/** Decision 783: the chi-square's line, P as a value or, where it has none, its sentence. */
export function cfaChiSquareSentence(cmin: CfaStat, df: number, p: CfaStat): string {
  const c = cfaStatText(cmin);
  return 'value' in p
    ? `Chi-square = ${c}, degrees of freedom = ${df}, probability level = ${shownNumber(p.value)}.`
    : `Chi-square = ${c}, degrees of freedom = ${df}. Probability level: ${cfaCellSentence(p.not_computable)}`;
}

/* The ten model-fit tables, their rows' names, the dash, its line, the whole numbers and the
 * cells' text moved to ModelFitTables.tsx (Research OS, Phase 9C-5, decision 814), which both
 * cards draw; they are re-exported here under their old names, unchanged. */
export type CfaFitColumn = FitColumn;
export const CFA_FIT_TABLES = FIT_TABLES;
export const CF_THREE_MODELS = FIT_THREE_MODELS;
export const CF_DASH = FIT_DASH;
/** Under a model-fit table, once per code its cells hold: "–: " and the code's sentence. */
export const cfaDashLine = (code: string): string => fitDashLine(code, cfaCellSentence);

/** A number, or its sentence where its number would be (the estimates' tables). */
export function cfaStatText(s: CfaStat): string {
  return 'value' in s ? shownNumber(s.value) : cfaCellSentence(s.not_computable);
}

export const cfaWholeText = fitWholeText;

/** A model-fit cell: a number as the card shows it (a whole one with every digit), or "–". */
export function cfaFitText(s: CfaStat | number, whole: boolean): string {
  return fitCellText(s, whole);
}

export const CFA_DIGITS_NOTE = RELATED_RANKS_DIGITS_NOTE;

export const CFA_EXPORT_HEADING = RELATED_RANKS_EXPORT_HEADING;

/** Decision 783: the factor analysis's choices' words, this method's files. */
export const CFA_EXPORT_CHOICES = [
  { format: 'csv', label: RELATED_RANKS_EXPORT_CHOICES[0].label, file: 'confirmatory_factor_analysis.csv', mime: 'text/csv' },
  { format: 'manifestJson', label: RELATED_RANKS_EXPORT_CHOICES[1].label, file: 'confirmatory_factor_analysis_manifest.json', mime: 'application/json' },
] as const;

export const CFA_EXPORT_TEXT = RELATED_RANKS_EXPORT_TEXT;

/** Rows used is the complete rows, so the three counts add up. */
export const CFA_ROWS_USED_NOTE = RELATED_RANKS_ROWS_USED_NOTE;
