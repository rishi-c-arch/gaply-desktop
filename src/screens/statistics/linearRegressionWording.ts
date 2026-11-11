// Gaply — the linear regression's words (Research OS, Phase 9A-2, decisions 684 and 685).
// Every code the card and dialog can show has one sentence here, keyed by the closed set in
// linearRegressionCodes.json, which src-tauri/tests/stats_linear_regression_wire.rs pins to
// LinearRegressionCode, RunCode, WorkspaceCode and CapabilityCode exactly. The run's, the
// registry's and the workspace's sentences are the related rank tests'
// (relatedRanksWording.ts), but the two that name the method. A refusal from the file ingest
// is said by methodsIngestWording.
//
// The tone is neutral (decision 66): a regression says what the numbers are; nothing here
// passes or fails a model or a predictor. No sentence judges a p value, an R² or a fit, or
// names a threshold for one. The progress shows no steps sentence: a run has one check
// point. "Remove" and "Remove all" are labels, and the verdict list refuses them, so the
// settings take them from windowWording and they are not here.
// linearRegressionWording.vitest.ts reads every sentence for the words a verdict would use.
import { refusalOf } from '../statsverifier/l1/l1Wording';
import { methodsIngestSentence } from './methodsIngestWording';
import type { WireCode } from '../statsverifier/statsL1Types';
// The run manager's runs kept, pinned to runs::MAX_RUNS.
import LIMITS from './describeLimits.json';
// Pinned to stats_linear_regression::MAX_PREDICTORS by stats_linear_regression_wire.rs.
import REGRESSION_LIMITS from './fixtures/linearRegressionLimits.json';
import { CAPABILITY_WORDS } from './capabilityWording';
import { RANKS_WORDS } from './ranksWording';
import {
  RELATED_RANKS_DIGITS_NOTE,
  RELATED_RANKS_EXPORT_CHOICES,
  RELATED_RANKS_EXPORT_HEADING,
  RELATED_RANKS_EXPORT_TEXT,
  RELATED_RANKS_ROWS_USED_NOTE,
  RELATED_RANKS_WORDS,
  type RelatedRanksNames,
} from './relatedRanksWording';
import type { Coded } from './anovaWording';
import type { LinearRegressionStat } from './linearRegressionTypes';
import { shownNumber } from './ttestWording';

/** The most predictors a run takes. */
export const MAX_PREDICTORS: number = REGRESSION_LIMITS.maxPredictors;

/** A column's name in a sentence, from the user's own screen. */
export type LinearRegressionNames = RelatedRanksNames;
const NO_NAMES: LinearRegressionNames = { column: (a) => `C${a}` };

const num = (d: Coded, k: string): number | undefined => {
  const v = d.params?.[k];
  return typeof v === 'number' ? v : undefined;
};
const cells = (n: number | undefined) => (n === 1 ? '1 cell' : `${n ?? 0} cells`);
const rows = (n: number | undefined) => (n === 1 ? '1 row' : `${n ?? 0} rows`);
const was = (n: number | undefined) => (n === 1 ? 'was' : 'were');
const has = (n: number | undefined) => (n === 1 ? 'has' : 'have');
const predictorsWord = (n: number | undefined) => (n === 1 ? '1 predictor' : `${n ?? 0} predictors`);
const countOf = (d: Coded): number | undefined => (typeof d.count === 'number' ? d.count : num(d, 'count'));
const column = (d: Coded, n: LinearRegressionNames) => n.column(num(d, 'column') ?? 0);

type Words = (d: Coded, names: LinearRegressionNames) => string;

/** The rank tests' sentence for a code whose suffix they share and that names no idea only
 *  they have, as the related rank tests take it. */
const asRanks = (suffix: string): Words => (d, n) => RANKS_WORDS[`stats.ranks.${suffix}`](d, { ...n, level: (i) => `level ${i + 1}` });
const fromRelated = (code: string): Words => RELATED_RANKS_WORDS[code] as unknown as Words;
/** The registry's sentence for one of the method's limits (capabilityWording.ts). */
const asLimit = (limit: string): Words => () => (CAPABILITY_WORDS as Record<string, string>)[`stats.capability.limit.analyze.${limit}`];

/** Decision 685's not-computable sentences, word for word. */
export const PERFECT_FIT_SENTENCE = 'Not computable: the model fits every row exactly, so there is no residual variation.';
export const OVERFLOW_SENTENCE = 'Not computable: a value along the way is too large for the arithmetic.';
export const TOO_SMALL_SENTENCE = 'Not computable: a value along the way is too small for the arithmetic to keep its precision.';
export const PREDICTED_NO_SPREAD_SENTENCE = 'Not computable: the predicted values do not vary, so they cannot be standardized.';
/** As the ANOVA says it (anovaWording.ts:181-182). */
export const DISTRIBUTION_OUT_OF_RANGE_SENTENCE =
  'Not computable: this needs a distribution at more than 10,000,000 degrees of freedom, where Gaply has not measured its accuracy.';
/** Decision 685's note, word for word. */
export const ESSENTIALLY_PERFECT_FIT_NOTE =
  "The residuals are at the level of rounding error, the case R's summary.lm calls an essentially perfect fit, so the standard errors, t, p and intervals, where they are given, rest on rounding.";

/** One sentence per code. A code with no entry is shown as itself, never hidden;
 *  linearRegressionWording.vitest.ts keeps every code in linearRegressionCodes.json covered. */
export const LINEAR_REGRESSION_WORDS: Record<string, Words> = {
  'stats.linear_regression.unknown_capability': asRanks('unknown_capability'),
  'stats.linear_regression.capability_version_mismatch': (d) =>
    `These settings were made for version ${num(d, 'found') ?? 0} of the linear regression, and this is version ${num(d, 'expected') ?? 0}.`,
  'stats.linear_regression.input_count': (d) => `These settings name ${num(d, 'count') ?? 0} files, and a linear regression reads exactly one.`,
  'stats.linear_regression.input_mismatch': asRanks('input_mismatch'),
  'stats.linear_regression.preparation_mismatch': asRanks('preparation_mismatch'),
  'stats.linear_regression.missing_code_not_finite': asRanks('missing_code_not_finite'),
  'stats.linear_regression.no_predictors': () => 'No predictor is chosen, and a linear regression needs at least one.',
  'stats.linear_regression.too_many_predictors': (d) =>
    `${predictorsWord(num(d, 'count'))} chosen, and a linear regression takes at most ${num(d, 'limit') ?? MAX_PREDICTORS}.`,
  'stats.linear_regression.column_repeated': (d, n) =>
    `Column ${column(d, n)} is chosen more than once, as the outcome or among the predictors. Choose each column once.`,
  'stats.linear_regression.unknown_column': asRanks('unknown_column'),
  'stats.linear_regression.column_non_finite': (d, n) =>
    `Column ${column(d, n)} holds an infinity or "not a number" in ${cells(num(d, 'count'))}, so it cannot be used in the regression.`,
  'stats.linear_regression.column_holds_text': (d, n) => `Column ${column(d, n)} holds text in ${cells(num(d, 'count'))}, so it cannot be used in the regression.`,
  'stats.linear_regression.code_equals_score': asRanks('code_equals_score'),
  'stats.linear_regression.no_complete_rows': () => 'No row has a value in the outcome and in every predictor, so there is nothing to fit.',
  'stats.linear_regression.too_few_rows': (d) =>
    `${rows(num(d, 'count'))} ${has(num(d, 'count'))} a value in every chosen column, and a model with these predictors needs at least ${num(d, 'limit') ?? 0}: the predictors, the intercept and one more.`,
  'stats.linear_regression.outcome_constant': () => 'The outcome takes one value in every complete row, so there is no variation to explain.',
  'stats.linear_regression.predictor_constant': (d, n) =>
    `Column ${column(d, n)} takes one value in every complete row, so its effect cannot be estimated. Take it out of the predictors.`,
  'stats.linear_regression.predictors_collinear': (d, n) =>
    `${column(d, n)} is, within R's tolerance, a linear combination of the intercept and the predictors before it, so its effect cannot be separated from theirs. Take it out of the predictors.`,
  'stats.linear_regression.fit_out_of_range': () =>
    "The values are too large or too small for the regression's sums of squares to be held, so it cannot be computed at this scale.",
  'stats.linear_regression.suspect_missing_code': (d, n) =>
    `Column ${column(d, n)} holds ${num(d, 'value') ?? 0} in ${cells(num(d, 'count'))}, which may be a code for a missing value. It is counted as a number unless you tick it in Settings.`,
  'stats.linear_regression.perfect_fit': () => PERFECT_FIT_SENTENCE,
  'stats.linear_regression.distribution_out_of_range': () => DISTRIBUTION_OUT_OF_RANGE_SENTENCE,
  'stats.linear_regression.overflow': () => OVERFLOW_SENTENCE,
  'stats.linear_regression.too_small': () => TOO_SMALL_SENTENCE,
  'stats.linear_regression.predicted_no_spread': () => PREDICTED_NO_SPREAD_SENTENCE,
  'stats.linear_regression.essentially_perfect_fit': () => ESSENTIALLY_PERFECT_FIT_NOTE,
  'stats.linear_regression.missing_value': (d, n) =>
    `${rows(countOf(d))} had no value in ${column(d, n)} (an empty cell or a missing-value token), and ${was(countOf(d))} left out.`,
  'stats.linear_regression.missing_code': (d, n) => `${rows(countOf(d))} held a missing code you ticked in ${column(d, n)}, and ${was(countOf(d))} left out.`,
  // Decision 685: each limitation as the registry's limit with the same meaning says it.
  'stats.linear_regression.enter_only': asLimit('regression_enter_only'),
  'stats.linear_regression.no_categorical_predictors': asLimit('regression_no_categorical_predictors'),
  'stats.linear_regression.no_interaction_terms': asLimit('regression_no_interaction_terms'),
  'stats.linear_regression.intercept_always': asLimit('regression_intercept_always'),
  'stats.linear_regression.listwise_only': asLimit('regression_listwise_only'),
  'stats.linear_regression.no_robust_standard_errors': asLimit('regression_no_robust_errors'),
  'stats.linear_regression.no_casewise_diagnostics': asLimit('regression_no_casewise_diagnostics'),
  'stats.linear_regression.durbin_watson_row_order': asLimit('durbin_watson_row_order'),
  'stats.linear_regression.intervals_95_only': asLimit('regression_intervals_95_only'),
  'stats.linear_regression.no_weights': asRanks('no_weights'),
  'run.unknown_run': () => `This run is no longer held. Gaply keeps the last ${LIMITS.maxRuns} runs in memory only; run the linear regression again.`,
  'run.wrong_kind': () => 'This run is not a linear regression, so this screen cannot read it.',
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
export function linearRegressionSentence(d: Coded, names: LinearRegressionNames = NO_NAMES): string {
  const f = LINEAR_REGRESSION_WORDS[d.code];
  return f ? f(d, names) : `Gaply has no wording for the code ${d.code}.`;
}

/** One plain sentence for a command's refusal. */
export function linearRegressionRefusalSentence(e: unknown): string {
  const c = refusalOf(e);
  if (!c) {
    const message = e instanceof Error ? e.message : typeof e === 'string' ? e : '';
    return message.trim() ? `This could not be done: ${message.trim()}` : 'This could not be done.';
  }
  if (c.code in LINEAR_REGRESSION_WORDS) {
    const params = (c as { params?: unknown }).params;
    return linearRegressionSentence({ code: c.code, params: params && typeof params === 'object' ? (params as Record<string, unknown>) : {} });
  }
  return methodsIngestSentence(c as WireCode);
}

/** Why a run that ended without a result has none. */
export function linearRegressionRunEndedSentence(state: string): string {
  switch (state) {
    case 'cancelled':
      return linearRegressionSentence({ code: 'run.cancelled' });
    case 'timed_out':
      return linearRegressionSentence({ code: 'run.timed_out' });
    case 'resource_limit':
      return linearRegressionSentence({ code: 'run.resource_limit' });
    case 'refused':
      return 'Gaply refused these settings, so nothing ran. The reasons are below.';
    case 'needs_user_input':
      return 'The settings need an answer from you, so nothing ran. See below.';
    case 'failed':
      return 'The linear regression could not be computed. The reason is below.';
    default:
      return linearRegressionSentence({ code: 'run.no_result' });
  }
}

/* ---- the settings (decision 685) ------------------------------------------ */

export const LR_OUTCOME_LABEL = 'Outcome (the column to explain)';
export const LR_OUTCOME_EMPTY = 'Choose a column…';
export const LR_PREDICTORS_LEGEND = 'Predictors, entered together';
export const LR_LIST_HEADING = 'Predictors, in this order';
export const LR_RULE = 'Every chosen predictor enters the model at once. A row is used only when the outcome and every predictor have a value.';
export const LR_CHOOSE_OUTCOME = 'Choose the outcome.';
export const LR_CHOOSE_PREDICTOR = 'Choose at least one predictor.';
export const LR_CODES_LEGEND = 'Candidate missing codes';

export function linearRegressionTooManyPredictors(n: number): string {
  return `Choose ${MAX_PREDICTORS} predictors or fewer; ${n} are chosen.`;
}

export const linearRegressionMoveUp = (alias: string) => `Move up ${alias}`;
export const linearRegressionMoveDown = (alias: string) => `Move down ${alias}`;

/** A candidate missing code in the chosen columns, as a question naming the value and where
 *  it appears, its places written as alphaMissingCodeQuestion writes them. */
export function linearRegressionMissingCodeQuestion(code: number, where: ReadonlyArray<{ header: string; cells: number }>): string {
  const places = where.map((w) => `"${w.header}" (${cells(w.cells)})`).join(', ');
  return `Is ${code} a code for a missing value? It appears in ${places}. If you tick it, it is treated as missing in the outcome and every predictor, and a row with it is left out.`;
}

/* ---- the card (decision 685) ---------------------------------------------- */

export const LR_HEADINGS = {
  model: 'Model summary',
  anova: 'Analysis of variance',
  coefficients: 'Coefficients',
  correlations: 'Correlations and collinearity',
  descriptives: 'Descriptive statistics',
  residuals: 'Residuals statistics',
} as const;

export const LR_MODEL_LABELS = {
  r: 'R',
  rSquared: 'R²',
  adjustedRSquared: 'Adjusted R²',
  standardErrorOfEstimate: 'Standard error of the estimate',
  durbinWatson: 'Durbin-Watson',
  rowsUsed: 'Rows used',
} as const;

export const LR_ANOVA_ROWS = { regression: 'Regression', residual: 'Residual', total: 'Total' } as const;
export const LR_ANOVA_HEADS = ['Sum of squares', 'df', 'Mean square', 'F', 'p'] as const;

export const LR_INTERCEPT = '(Intercept)';
export const LR_COEFFICIENT_HEADS = ['B', 'Standard error', 'Beta', 't', 'p', '95% interval'] as const;
export const LR_CORRELATION_HEADS = ['Zero-order', 'Partial', 'Part', 'Tolerance', 'VIF'] as const;
export const LR_DESCRIPTIVE_HEADS = ['Mean', 'Standard deviation'] as const;

export const LR_RESIDUAL_ROWS = {
  predicted: 'Predicted value',
  residual: 'Residual',
  standardizedPredicted: 'Standardized predicted value',
  standardizedResidual: 'Standardized residual',
} as const;
export const LR_RESIDUAL_HEADS = ['Minimum', 'Maximum', 'Mean', 'Standard deviation'] as const;

/** A row's or column's head where the card names the predictor or outcome by text. */
export const LR_COLUMN_HEAD = 'Column';

/** Decision 685: under the coefficients table, the outcome as text. */
export function linearRegressionCoefficientsSentence(outcome: string): string {
  return `B is the change in ${outcome} for a one-unit change in a predictor, the other predictors held fixed; Beta is the change in ${outcome}'s standard deviations for a one-standard-deviation change in a predictor, the others held fixed.`;
}

/** The 95% interval in one cell, as AnovaFlow.tsx:445-447 writes it. */
export function linearRegressionInterval(lower: number, upper: number): string {
  return `${shownNumber(lower)} to ${shownNumber(upper)}`;
}

/** A number, or its sentence where its number would be. */
export function linearRegressionStatText(s: LinearRegressionStat): string {
  return 'value' in s ? shownNumber(s.value) : linearRegressionSentence({ code: s.not_computable });
}

export const LINEAR_REGRESSION_DIGITS_NOTE = RELATED_RANKS_DIGITS_NOTE;

export const LINEAR_REGRESSION_EXPORT_HEADING = RELATED_RANKS_EXPORT_HEADING;

/** Decision 685: the related rank tests' choices' words, the regression's files. */
export const LINEAR_REGRESSION_EXPORT_CHOICES = [
  { format: 'csv', label: RELATED_RANKS_EXPORT_CHOICES[0].label, file: 'linear_regression.csv', mime: 'text/csv' },
  { format: 'manifestJson', label: RELATED_RANKS_EXPORT_CHOICES[1].label, file: 'linear_regression_manifest.json', mime: 'application/json' },
] as const;

export const LINEAR_REGRESSION_EXPORT_TEXT = RELATED_RANKS_EXPORT_TEXT;

/** Rows used is the complete rows, so the three counts add up. */
export const LINEAR_REGRESSION_ROWS_USED_NOTE = RELATED_RANKS_ROWS_USED_NOTE;
