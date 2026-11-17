// Gaply — the structural equation model's words (Research OS, Phase 9C-5, decisions 814 and
// 815). Every code the card and dialog can show has one sentence here, keyed by the closed set in
// semCodes.json, which src-tauri/tests/stats_sem_wire.rs pins to SemCode, RunCode, WorkspaceCode
// and CapabilityCode exactly. Decision 815's sentences come first. A code the confirmatory factor
// analysis also has takes its sentence (cfaWording.ts), and where it has none, the rank tests',
// alpha's or the regression's, as the CFA's own module takes them; where that sentence names
// another method, it is reworded here for this method, and each rewording is listed in the
// step's report. The run's, the registry's and the workspace's sentences are the related rank
// tests', but the two that name the method. Each limitation is the registry's sentence for its
// limit (capabilityWording.ts), and no_weights the rank tests'. A refusal from the file ingest is
// said by methodsIngestWording.
//
// Three codes are both a not-computable value and something else: negative_variance and df_zero
// are also notes, and overflow is also the run's refusal naming a column. A cell's sentence is
// SEM_NOT_COMPUTABLE's; a note's and a diagnostic's are SEM_WORDS'.
//
// The tone is neutral (decision 66): the card says what the numbers are; nothing here passes or
// fails a model, gives a fit measure a cutoff or a band's name, or uses a word a verdict would
// use. semWording.vitest.ts reads every sentence for those words. The buttons' "Remove" is
// windowWording.ts's ALPHA_REMOVE_ITEM, imported by the settings, outside this module's scan.
import { refusalOf } from '../statsverifier/l1/l1Wording';
import { methodsIngestSentence } from './methodsIngestWording';
import type { WireCode } from '../statsverifier/statsL1Types';
// The run manager's runs kept, pinned to runs::MAX_RUNS.
import LIMITS from './describeLimits.json';
import { semLimits } from './fixtures/semFixtures';
import { CAPABILITY_WORDS } from './capabilityWording';
import { RELATED_RANKS_WORDS } from './relatedRanksWording';
import {
  CFA_DIGITS_NOTE,
  CFA_EXPORT_CHOICES,
  CFA_EXPORT_HEADING,
  CFA_EXPORT_TEXT,
  CFA_NOT_COMPUTABLE,
  CFA_NOTES,
  CFA_ROWS_USED_NOTE,
  CFA_WORDS,
  CF_HEADINGS,
  CF_LABELS,
  cfaFactorTooFew,
  cfaMissingCodeQuestion,
  type CfaNames,
} from './cfaWording';
import type { Coded } from './anovaWording';
import type { SemStat, SemVariable } from './semTypes';
import { shownNumber } from './ttestWording';

/** The most columns in a model: stats_sem::MAX_COLUMNS, from the wire fixture. */
export const MAX_COLUMNS: number = semLimits.maxColumns;
/** The most covariances: stats_sem::MAX_COVARIANCES, from the wire fixture. */
export const MAX_COVARIANCES: number = semLimits.maxCovariances;
/** The most members over every group held equal: stats_sem::MAX_EQUAL_MEMBERS, from the wire fixture. */
export const MAX_EQUAL_MEMBERS: number = semLimits.maxEqualMembers;
/** The most factors: stats_sem::MAX_FACTORS, from the wire fixture. */
export const MAX_FACTORS: number = semLimits.maxFactors;
/** The most paths: stats_sem::MAX_PATHS, from the wire fixture. */
export const MAX_PATHS: number = semLimits.maxPaths;
/** The fewest columns in a model: stats_sem::MIN_COLUMNS, from the wire fixture. */
export const MIN_COLUMNS: number = semLimits.minColumns;
/** The fewest columns measuring one factor: stats_sem::MIN_FACTOR_ITEMS, from the wire fixture. */
export const MIN_FACTOR_ITEMS: number = semLimits.minFactorItems;
/** Each attempt's cap on iterations: stats_sem::MAX_ITERATIONS (stats_sem/mod.rs:99), its Rust
 *  twin. */
export const MAX_ITERATIONS = 500;

/** A column's name in a sentence, from the user's own screen. */
export type SemNames = CfaNames;
const NO_NAMES: SemNames = { column: (a) => `C${a}` };

const num = (d: Coded, k: string): number | undefined => {
  const v = d.params?.[k];
  return typeof v === 'number' ? v : undefined;
};
const rows = (n: number | undefined) => (n === 1 ? '1 row' : `${n ?? 0} rows`);
const has = (n: number | undefined) => (n === 1 ? 'has' : 'have');
const columnsWord = (n: number | undefined) => (n === 1 ? '1 column' : `${n ?? 0} columns`);
const pathsWord = (n: number | undefined) => (n === 1 ? '1 path' : `${n ?? 0} paths`);
const covariancesWord = (n: number | undefined) => (n === 1 ? '1 covariance' : `${n ?? 0} covariances`);
const iterationsWord = (n: number | undefined) => (n === 1 ? '1 iteration' : `${n ?? 0} iterations`);
const constraintsWord = (n: number | undefined) => (n === 1 ? '1 more constraint' : `${n ?? 0} more constraints`);
const column = (d: Coded, n: SemNames) => n.column(num(d, 'column') ?? 0);

type Words = (d: Coded, names: SemNames) => string;

/** The CFA's sentence for a code whose suffix it shares. */
const asCfa = (suffix: string): Words => (d, n) => CFA_WORDS[`stats.cfa.${suffix}`](d, n);
/** The CFA's sentence with its last words, "Choose the factors again.", made this method's, "Build
 *  the model again." (9C-5b, decision 823's N8). */
const rebuilt = (suffix: string): Words => (d, n) => asCfa(suffix)(d, n).replace(/ Choose the factors again\.$/, ' Build the model again.');
const fromRelated = (code: string): Words => RELATED_RANKS_WORDS[code] as unknown as Words;
/** The registry's sentence for one of the method's limits (capabilityWording.ts). */
const asLimit = (limit: string): Words => () => (CAPABILITY_WORDS as Record<string, string>)[`stats.capability.limit.analyze.${limit}`];
/** Decision 815: a setting this screen never writes, with the step's own phrase for it. */
const neverWrites = (what: (d: Coded, n: SemNames) => string): Words => (d, n) => `These settings ${what(d, n)}, which this screen never does. Build the model again.`;

/** The CFA's not-computable sentences (decision 815: a cell says what the CFA's says). A cell
 *  shows these. */
export const SEM_NOT_COMPUTABLE: Record<string, string> = Object.fromEntries(
  ['fixed_parameter', 'df_zero', 'cmin_zero', 'independence_cmin_zero', 'too_few_rows_for_bcc', 'negative_variance', 'too_small', 'overflow', 'distribution_out_of_range'].map((s) => [
    `stats.sem.${s}`,
    CFA_NOT_COMPUTABLE[`stats.cfa.${s}`],
  ])
);

/** Decision 815's notes, word for word; df_zero is the CFA's. */
export const SEM_NOTES = {
  negativeVariance:
    'At least one variance estimate is below 0, so this solution is not admissible, as AMOS says of such a solution. A value whose formula takes the square root of a variance at or below 0 is not given, and a squared multiple correlation can be above 1 or below 0 where a variance is below 0.',
  covarianceNotPositiveDefinite:
    'The estimated variances and covariances of the factors and of the variables in your covariances, each endogenous one by its error term, do not form a positive definite matrix, so this solution is not admissible, as AMOS says of such a solution.',
  nonrecursive:
    "The paths make a loop, so the model is nonrecursive. Gaply computes its effects as AMOS does, from the inverse of the loop's matrix, but not AMOS's stability index, which tells whether the loop's effects can be read as values it settles at.",
  dfZero: CFA_NOTES.dfZero,
} as const;

/** One sentence per code. A code with no entry is shown as itself, never hidden;
 *  semWording.vitest.ts keeps every code in semCodes.json covered. */
export const SEM_WORDS: Record<string, Words> = {
  // Decision 815's validation sentences, word for word.
  'stats.sem.empty_model': () => 'The model has no factor, path or covariance. Add one.',
  'stats.sem.too_few_columns': (d) => `The model has ${columnsWord(num(d, 'count'))}; it needs at least ${num(d, 'limit') ?? MIN_COLUMNS}.`,
  'stats.sem.too_many_columns': (d) => `The model has ${columnsWord(num(d, 'count'))}; Gaply fits at most ${num(d, 'limit') ?? MAX_COLUMNS}.`,
  'stats.sem.too_many_paths': (d) => `The model has ${pathsWord(num(d, 'count'))}; Gaply fits at most ${num(d, 'limit') ?? MAX_PATHS}.`,
  'stats.sem.too_many_covariances': (d) => `The model has ${covariancesWord(num(d, 'count'))}; Gaply fits at most ${num(d, 'limit') ?? MAX_COVARIANCES}.`,
  'stats.sem.too_many_equal_members': (d) => `The groups hold ${num(d, 'count') ?? 0} parameters; Gaply holds at most ${num(d, 'limit') ?? MAX_EQUAL_MEMBERS} equal.`,
  // Decision 815: the codes this screen never writes, each {what} built only from its params.
  'stats.sem.unknown_factor': neverWrites(() => 'name a factor the model does not have'),
  'stats.sem.path_to_itself': neverWrites((d) => `draw path ${num(d, 'index') ?? 0} from a variable to itself`),
  'stats.sem.indicator_in_path': neverWrites((d, n) => `put ${column(d, n)}, a column that measures a factor, at an end of a path`),
  'stats.sem.paths_out_of_order': neverWrites((d) => `list path ${num(d, 'index') ?? 0} out of order`),
  'stats.sem.path_repeated': neverWrites((d) => `list path ${num(d, 'index') ?? 0} twice`),
  'stats.sem.covariance_with_itself': neverWrites((d) => `give covariance ${num(d, 'index') ?? 0} the same variable at both ends`),
  'stats.sem.covariances_out_of_order': neverWrites((d) => `list covariance ${num(d, 'index') ?? 0} out of order`),
  'stats.sem.covariance_repeated': neverWrites((d) => `list covariance ${num(d, 'index') ?? 0} twice`),
  'stats.sem.equal_group_too_small': neverWrites((d) => `hold fewer than ${num(d, 'limit') ?? 2} parameters equal in group ${num(d, 'index') ?? 0}`),
  'stats.sem.equal_mixed_kinds': neverWrites((d) => `hold parameters of different kinds equal in group ${num(d, 'index') ?? 0}`),
  'stats.sem.equal_unknown_parameter': neverWrites((d) => `hold equal, as parameter ${num(d, 'value') ?? 0} of group ${num(d, 'index') ?? 0}, one the model does not have`),
  'stats.sem.equal_fixed_parameter': neverWrites((d) => `hold equal, as parameter ${num(d, 'value') ?? 0} of group ${num(d, 'index') ?? 0}, a loading fixed at 1`),
  'stats.sem.equal_out_of_order': neverWrites((d) =>
    num(d, 'value') === undefined
      ? `list group ${num(d, 'index') ?? 0} out of order`
      : `list parameter ${num(d, 'value')} of group ${num(d, 'index') ?? 0} out of order`
  ),
  'stats.sem.equal_repeated': neverWrites((d) => `hold parameter ${num(d, 'value') ?? 0} of group ${num(d, 'index') ?? 0} equal in a second place`),
  'stats.sem.model_not_identified': (d) =>
    num(d, 'count') === undefined
      ? 'The model is not identified: no data can determine all of its estimates, as AMOS says of such a model. Take out a path or a covariance, give a factor more columns, or hold parameters equal.'
      : `The model is not identified: it needs ${constraintsWord(num(d, 'count'))} before any data can determine its estimates, as AMOS says of such a model. Take out a path or a covariance, give a factor more columns, or hold parameters equal.`,
  'stats.sem.column_constant': (d, n) => `${column(d, n)} has the same value in every row used, so it has no variance. Take it out of the model.`,
  // Decision 815's refusals: two the CFA's, two its own.
  'stats.sem.sample_covariance_singular': asCfa('sample_covariance_singular'),
  'stats.sem.overflow': asCfa('overflow'),
  'stats.sem.solution_not_identified': () =>
    'Maximum likelihood stopped where the data barely determine at least one estimate, so its standard error would have no meaning, and Gaply gives no estimates. A factor whose columns barely correlate with each other is one way this happens.',
  'stats.sem.sem_not_converged': (d) =>
    `Maximum likelihood did not converge from Gaply's starting values by any method Gaply tried; the longest attempt stopped after ${iterationsWord(num(d, 'count'))}, of at most ${num(d, 'limit') ?? MAX_ITERATIONS}. A smaller model, or columns that measure their factors more strongly, may converge.`,
  // Decision 815's notes; their cells' sentences are SEM_NOT_COMPUTABLE's.
  'stats.sem.negative_variance': () => SEM_NOTES.negativeVariance,
  'stats.sem.covariance_not_positive_definite': () => SEM_NOTES.covarianceNotPositiveDefinite,
  'stats.sem.nonrecursive': () => SEM_NOTES.nonrecursive,
  'stats.sem.df_zero': () => SEM_NOTES.dfZero,
  // The not-computable sentences, which are only ever a cell's.
  'stats.sem.fixed_parameter': () => SEM_NOT_COMPUTABLE['stats.sem.fixed_parameter'],
  'stats.sem.cmin_zero': () => SEM_NOT_COMPUTABLE['stats.sem.cmin_zero'],
  'stats.sem.independence_cmin_zero': () => SEM_NOT_COMPUTABLE['stats.sem.independence_cmin_zero'],
  'stats.sem.too_few_rows_for_bcc': () => SEM_NOT_COMPUTABLE['stats.sem.too_few_rows_for_bcc'],
  'stats.sem.too_small': () => SEM_NOT_COMPUTABLE['stats.sem.too_small'],
  'stats.sem.distribution_out_of_range': () => SEM_NOT_COMPUTABLE['stats.sem.distribution_out_of_range'],
  // The CFA's, which name no method and no item.
  'stats.sem.unknown_capability': asCfa('unknown_capability'),
  'stats.sem.input_mismatch': asCfa('input_mismatch'),
  'stats.sem.preparation_mismatch': asCfa('preparation_mismatch'),
  'stats.sem.missing_code_not_finite': asCfa('missing_code_not_finite'),
  'stats.sem.missing_code_repeated': asCfa('missing_code_repeated'),
  'stats.sem.missing_codes_out_of_order': asCfa('missing_codes_out_of_order'),
  'stats.sem.too_many_factors': asCfa('too_many_factors'),
  'stats.sem.factor_too_few_items': asCfa('factor_too_few_items'),
  // The CFA's, ending as this method's own never-written sentences end (9C-5b, decision 823's N8).
  'stats.sem.items_out_of_order': rebuilt('items_out_of_order'),
  'stats.sem.factors_out_of_order': rebuilt('factors_out_of_order'),
  'stats.sem.item_repeated': asCfa('item_repeated'),
  'stats.sem.unknown_column': asCfa('unknown_column'),
  'stats.sem.column_non_finite': asCfa('column_non_finite'),
  'stats.sem.column_holds_text': asCfa('column_holds_text'),
  'stats.sem.code_equals_score': asCfa('code_equals_score'),
  'stats.sem.no_complete_rows': asCfa('no_complete_rows'),
  'stats.sem.suspect_missing_code': asCfa('suspect_missing_code'),
  'stats.sem.missing_value': asCfa('missing_value'),
  'stats.sem.missing_code': asCfa('missing_code'),
  // The CFA's, reworded for this method (each listed in the step's report).
  'stats.sem.capability_version_mismatch': (d) =>
    `These settings were made for version ${num(d, 'found') ?? 0} of the structural equation model, and this is version ${num(d, 'expected') ?? 0}.`,
  'stats.sem.input_count': (d) => `These settings name ${num(d, 'count') ?? 0} files, and a structural equation model reads exactly one.`,
  'stats.sem.too_few_rows': (d) =>
    `${rows(num(d, 'count'))} ${has(num(d, 'count'))} a value in every column in the model, and the structural equation model needs at least ${num(d, 'limit') ?? 0}.`,
  // Each limitation as the registry's limit with the same meaning says it.
  'stats.sem.ml_only': asLimit('sem_ml_only'),
  'stats.sem.no_means': asLimit('sem_no_means'),
  'stats.sem.one_group_only': asLimit('sem_one_group'),
  'stats.sem.indicators_measure_one_factor': asLimit('sem_indicators_one_factor'),
  'stats.sem.equal_one_kind': asLimit('sem_equal_one_kind'),
  'stats.sem.marker_identification_only': asLimit('sem_marker_identification'),
  'stats.sem.no_fixed_values': asLimit('sem_no_fixed_values'),
  'stats.sem.effects_structural_only': asLimit('sem_effects_structural_only'),
  'stats.sem.no_indirect_effect_tests': asLimit('sem_no_indirect_effect_tests'),
  'stats.sem.no_stability_index': asLimit('sem_no_stability_index'),
  'stats.sem.no_modification_indices': asLimit('sem_no_modification_indices'),
  'stats.sem.normal_theory_standard_errors': asLimit('sem_normal_theory'),
  'stats.sem.no_srmr': asLimit('sem_no_srmr'),
  'stats.sem.listwise_only': asLimit('sem_listwise_only'),
  'stats.sem.no_weights': asCfa('no_weights'),
  // Decision 815's run sentences that name the method.
  'run.unknown_run': () => `This run is no longer held. Gaply keeps the last ${LIMITS.maxRuns} runs in memory only; run the structural equation model again.`,
  'run.wrong_kind': () => 'This run is not a structural equation model, so this screen cannot read it.',
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
export function semSentence(d: Coded, names: SemNames = NO_NAMES): string {
  const f = SEM_WORDS[d.code];
  return f ? f(d, names) : `Gaply has no wording for the code ${d.code}.`;
}

/** A not-computable value's sentence, as a cell says it. */
export function semCellSentence(code: string): string {
  return SEM_NOT_COMPUTABLE[code] ?? semSentence({ code });
}

/** One plain sentence for a command's refusal. */
export function semRefusalSentence(e: unknown): string {
  const c = refusalOf(e);
  if (!c) {
    const message = e instanceof Error ? e.message : typeof e === 'string' ? e : '';
    return message.trim() ? `This could not be done: ${message.trim()}` : 'This could not be done.';
  }
  if (c.code in SEM_WORDS) {
    const params = (c as { params?: unknown }).params;
    return semSentence({ code: c.code, params: params && typeof params === 'object' ? (params as Record<string, unknown>) : {} });
  }
  return methodsIngestSentence(c as WireCode);
}

/** Why a run that ended without a result has none. */
export function semRunEndedSentence(state: string): string {
  switch (state) {
    case 'cancelled':
      return semSentence({ code: 'run.cancelled' });
    case 'timed_out':
      return semSentence({ code: 'run.timed_out' });
    case 'resource_limit':
      return semSentence({ code: 'run.resource_limit' });
    case 'refused':
      return 'Gaply refused these settings, so nothing ran. The reasons are below.';
    case 'needs_user_input':
      return 'The settings need an answer from you, so nothing ran. See below.';
    case 'failed':
      return 'The structural equation model could not be computed. The reason is below.';
    default:
      return semSentence({ code: 'run.no_result' });
  }
}

/* ---- the settings (decision 815) ------------------------------------------ */

export const SM_RULE =
  "A column measures one factor, or stands on its own. Factors are numbered by their first column in the file, and each factor's first column has its loading fixed at 1 to set the factor's scale, as AMOS does when a model is drawn. A variable with a path into it has an error term; every other variable has a variance. Two variables are uncorrelated unless you add their covariance. A row is used only when every column in the model has a value.";
export const SM_COLUMNS_HEADING = 'Numeric columns';
export const SM_NOT_IN_MODEL = 'Not in the model';
export const SM_ON_ITS_OWN = 'On its own';
export const semFactorOption = (n: number): string => `Factor ${n}`;
export const SM_NEW_FACTOR = 'A new factor';
export const SM_FACTORS_HEADING = "Factors, each in the file's order";
export const SM_PATHS_HEADING = 'Paths';
export const SM_COVARIANCES_HEADING = 'Covariances';
export const SM_EQUAL_HEADING = 'Parameters held equal';
export const semFactorHeading = (n: number): string => `Factor ${n}`;
export const SM_FIXED_MARK = '(loading fixed at 1)';
export const semGroupHeading = (n: number): string => `Group ${n}`;
/** 9C-5b (decision 823's N7): the accessible names of a covariance row's two selects and a group's
 *  kind select, by number only, never a column's name. */
export const semCovarianceEndLabel = (i: number, end: 'first' | 'second'): string => `Covariance ${i}, ${end} end`;
/** A path row's select's accessible name, with no column's name in it; its visible words stay
 *  "From" and "To" (9C-5f, decision 848's E5). */
export const semPathEndLabel = (i: number, end: 'from' | 'to'): string => `Path ${i}, ${end}`;
export const semGroupKindLabel = (n: number): string => `Group ${n}, kind`;
export const SM_FROM = 'From';
export const SM_TO = 'To';
export const SM_CHOOSE = 'Choose a variable';
export const SM_KINDS = { loading: 'Loadings', path: 'Paths', covariance: 'Covariances', variance: 'Variances' } as const;
export type SemKind = keyof typeof SM_KINDS;
export const SM_ADD_PATH = 'Add a path';
export const SM_ADD_COVARIANCE = 'Add a covariance';
export const SM_ADD_EXOGENOUS = 'Add every pair of exogenous variables';
export const SM_ADD_GROUP = 'Add a group';
export const SM_NO_FACTOR = 'No factor yet. Choose a factor for a column on the left.';
export const SM_NO_PATH = 'No path yet. Add one to give one variable a regression weight on another.';
export const SM_NO_COVARIANCE = 'No covariance yet. Add one to let two variables, or their error terms, covary.';
export const SM_NO_GROUP = 'No parameters are held equal. Add a group to hold two or more equal.';
export const SM_CODES_LEGEND = 'Candidate missing codes';
/** A candidate missing code in the model's columns: the CFA's question. */
export const semMissingCodeQuestion = cfaMissingCodeQuestion;

/** Beneath a complete covariance row: which ends are error terms; null where neither is. */
export function semCovarianceLine(a: string, aEndogenous: boolean, b: string, bEndogenous: boolean): string | null {
  if (aEndogenous && bEndogenous) return `Between the error terms of ${a} and ${b}.`;
  if (aEndogenous) return `Between the error term of ${a} and ${b}.`;
  if (bEndogenous) return `Between ${a} and the error term of ${b}.`;
  return null;
}

export const SM_PATH_TO_ITSELF = 'A path needs two different variables. Change one of its ends.';
export const SM_PATH_REPEATED = 'This path is already listed. Change one of its ends.';
export const SM_COVARIANCE_WITH_ITSELF = 'A covariance needs two different variables. Change one of its ends.';
export const SM_COVARIANCE_REPEATED = 'This covariance is already listed. Change one of its ends.';
/** Beside a group with fewer than 2 parameters ticked. */
export function semGroupShort(n: number, ticked: number): string {
  return ticked === 1 ? `Group ${n} holds 1 parameter. Tick at least one more.` : `Group ${n} holds no parameters. Tick at least 2.`;
}
/** Beside a column on its own with no path or covariance. */
export const semAloneUnused = (column: string): string =>
  `${column} is on its own with no path or covariance, so it would not be fitted. Add one, or choose "Not in the model" for it.`;

/** Decision 815: the first thing missing, one sentence each, in decision 814's order. */
export const SM_MISSING = {
  empty: 'Add a factor, a path or a covariance.',
  columns: `Put at least ${MIN_COLUMNS} columns in the model.`,
  // 9C-5b (decision 823's N4): a model past a cap, which the screen never builds; said after the
  // column count's line, so a helper given such a model says what to do.
  tooManyColumns: (count: number): string =>
    `The model has ${count} columns, more than the ${MAX_COLUMNS} Gaply fits. Choose "Not in the model" for ${count - MAX_COLUMNS} of them.`,
  tooManyFactors: (count: number): string =>
    `The model has ${count} factors, more than the ${MAX_FACTORS} Gaply fits. Move every column of ${count - MAX_FACTORS} of them to another factor or out of the model.`,
  // 9C-5f (decision 848's E3): the other three caps, said after the factors' line.
  tooManyPaths: (count: number): string => `The model has ${count} paths, more than the ${MAX_PATHS} Gaply fits. Take out ${count - MAX_PATHS} of them.`,
  tooManyCovariances: (count: number): string =>
    `The model has ${count} covariances, more than the ${MAX_COVARIANCES} Gaply fits. Take out ${count - MAX_COVARIANCES} of them.`,
  tooManyMembers: (count: number): string =>
    `The groups hold ${count} parameters, more than the ${MAX_EQUAL_MEMBERS} Gaply holds equal. Untick ${count - MAX_EQUAL_MEMBERS} of them.`,
  factorShort: (n: number): string => `Factor ${n} has 1 column; each factor needs at least ${MIN_FACTOR_ITEMS}.`,
  alone: (column: string): string => `${column} is on its own with no path or covariance. Add one, or choose "Not in the model" for it.`,
  pathEnds: (i: number): string => `Choose both ends of path ${i}.`,
  pathItself: (i: number): string => `Path ${i} needs two different variables.`,
  pathRepeats: (i: number, j: number): string => `Path ${i} repeats path ${j}. Change one of its ends.`,
  covarianceEnds: (i: number): string => `Choose both ends of covariance ${i}.`,
  covarianceItself: (i: number): string => `Covariance ${i} needs two different variables.`,
  covarianceRepeats: (i: number, j: number): string => `Covariance ${i} repeats covariance ${j}. Change one of its ends.`,
  groupShort: (n: number): string => `Group ${n} holds fewer than 2 parameters. Tick at least 2.`,
} as const;

/** Decision 815: beside the control each cap disables. */
export const SM_CAPS = {
  columns: `The model has ${MAX_COLUMNS} columns, the most Gaply fits; take one out to add another.`,
  paths: `The model has ${MAX_PATHS} paths, the most Gaply fits.`,
  covariances: `The model has ${MAX_COVARIANCES} covariances, the most Gaply fits.`,
  exogenous: (count: number): string =>
    `Adding every pair would make ${count} covariances, more than the ${MAX_COVARIANCES} Gaply fits. Add the pairs you need one by one.`,
  equal: `The groups hold ${MAX_EQUAL_MEMBERS} parameters, the most Gaply holds equal; untick one to tick another.`,
} as const;

/** A factor's own heading's sentence where it has 1 column: the CFA's. */
export const semFactorTooFew = (n: number, count: number): string => cfaFactorTooFew(n, count, MIN_FACTOR_ITEMS);

/* ---- naming a variable ------------------------------------------------------ */

const aliasNumber = (alias: string) => Number(alias.replace(/^C/, ''));

/** A variable's name: "Factor n", or the column's name from the user's own screen. */
export function semVariableName(v: SemVariable, names: SemNames): string {
  return 'factor' in v ? `Factor ${v.factor}` : names.column(aliasNumber(v.column));
}
/** An error term's name. */
export const semErrorName = (name: string): string => `Error of ${name}`;
/** A weight's row: "{to} ← {from}". */
export const semWeightRow = (to: string, from: string): string => `${to} ← ${from}`;
/** A covariance's row: "{a} ↔ {b}". */
export const semCovarianceRow = (a: string, b: string): string => `${a} ↔ ${b}`;

/* ---- the card (decision 815) ---------------------------------------------- */

export const SM_HEADINGS = {
  ...CF_HEADINGS,
  effects: 'Effects',
  total: 'Total effects',
  totalStandardized: 'Standardized total effects',
  direct: 'Direct effects',
  directStandardized: 'Standardized direct effects',
  indirect: 'Indirect effects',
  indirectStandardized: 'Standardized indirect effects',
} as const;
export const SM_LABELS = CF_LABELS;

/** Decision 815: the line naming the attempt that reached the solution, null at attempt 1. */
export function semAttemptSentence(attempt: number): string | null {
  if (attempt === 2) return "Fisher scoring did not converge from Gaply's starting values; Newton's method, from the same values, did.";
  if (attempt === 3) return "Neither Fisher scoring nor Newton's method converged from Gaply's starting values; Fisher scoring from other starting values for the factors did.";
  return null;
}

export const SM_NO_COVARIANCES = 'The model has no covariances, so every pair of exogenous variables is uncorrelated.';
export const SM_BLANK_CELL = "A blank cell: its row and its column are the same variable, or no path of arrows leads from the column's variable to the row's.";
export const SM_NO_EFFECTS = 'The model has no paths, so it has no effects.';

/** The minimization's line: the CFA's words. */
export function semIterationsSentence(n: number): string {
  if (n === 0) return "Maximum likelihood's starting values were already its minimum.";
  return `Maximum likelihood reached its minimum in ${iterationsWord(n)}.`;
}

/** The chi-square's line, P as a value or, where it has none, its sentence: the CFA's words. */
export function semChiSquareSentence(cmin: SemStat, df: number, p: SemStat): string {
  const c = semStatText(cmin);
  return 'value' in p
    ? `Chi-square = ${c}, degrees of freedom = ${df}, probability level = ${shownNumber(p.value)}.`
    : `Chi-square = ${c}, degrees of freedom = ${df}. Probability level: ${semCellSentence(p.not_computable)}`;
}

/** A number, or its sentence where its number would be (the estimates' tables and the effects'
 *  matrices). shownNumber shows a -0 as 0 (ttestWording.ts: (-0).toPrecision(6) is "0.00000"). */
export function semStatText(s: SemStat): string {
  return 'value' in s ? shownNumber(s.value) : semCellSentence(s.not_computable);
}

export const SEM_DIGITS_NOTE = CFA_DIGITS_NOTE;

export const SEM_EXPORT_HEADING = CFA_EXPORT_HEADING;

/** Decision 815: the CFA's choices' words, this method's files. */
export const SEM_EXPORT_CHOICES = [
  { format: 'csv', label: CFA_EXPORT_CHOICES[0].label, file: 'structural_equation_model.csv', mime: 'text/csv' },
  { format: 'manifestJson', label: CFA_EXPORT_CHOICES[1].label, file: 'structural_equation_model_manifest.json', mime: 'application/json' },
] as const;

export const SEM_EXPORT_TEXT = CFA_EXPORT_TEXT;

/** Rows used is the complete rows, so the three counts add up. */
export const SEM_ROWS_USED_NOTE = CFA_ROWS_USED_NOTE;
