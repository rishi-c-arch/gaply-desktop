// Gaply — the structural equation model's wire types (Research OS, Phase 9C-5, decision 814),
// on cfaTypes.ts's pattern. TypeScript mirrors of the views src-tauri/src/stats_sem_commands.rs
// returns and of the spec it takes. Every view field is pinned by a JSON fixture in ./fixtures
// (sem*View*.json), which src-tauri/tests/stats_sem_wire.rs serialises byte for byte and
// semTypes.vitest.ts deep-compares with a typed copy.
//
// The dataset is opened with Phase 2B's stats_desc_open and closed with stats_desc_close,
// unchanged. The method has no levels, so there is no levels command.
//
// What these hold: result numbers, for this device's screen only. The spec and the result
// name columns by alias and factors by number; never a label, a row or a path.
import type {
  InputRef,
  Platform,
  Preparation,
  Reproducibility,
  ResourceEstimate,
  RunState,
  Severity,
  ValidationStatus,
} from './describeTypes';

/** Every refusal, outcome, exclusion, note and limitation of the structural equation model
 *  (stats_sem::SemCode), in Rust's order. */
export type SemCode =
  | 'stats.sem.unknown_capability'
  | 'stats.sem.capability_version_mismatch'
  | 'stats.sem.input_count'
  | 'stats.sem.input_mismatch'
  | 'stats.sem.preparation_mismatch'
  | 'stats.sem.missing_code_not_finite'
  | 'stats.sem.missing_code_repeated'
  | 'stats.sem.missing_codes_out_of_order'
  | 'stats.sem.empty_model'
  | 'stats.sem.too_many_factors'
  | 'stats.sem.factor_too_few_items'
  | 'stats.sem.items_out_of_order'
  | 'stats.sem.factors_out_of_order'
  | 'stats.sem.item_repeated'
  | 'stats.sem.too_many_paths'
  | 'stats.sem.unknown_factor'
  | 'stats.sem.path_to_itself'
  | 'stats.sem.indicator_in_path'
  | 'stats.sem.paths_out_of_order'
  | 'stats.sem.path_repeated'
  | 'stats.sem.too_many_covariances'
  | 'stats.sem.covariance_with_itself'
  | 'stats.sem.covariances_out_of_order'
  | 'stats.sem.covariance_repeated'
  | 'stats.sem.too_many_equal_members'
  | 'stats.sem.equal_group_too_small'
  | 'stats.sem.equal_mixed_kinds'
  | 'stats.sem.equal_unknown_parameter'
  | 'stats.sem.equal_fixed_parameter'
  | 'stats.sem.equal_out_of_order'
  | 'stats.sem.equal_repeated'
  | 'stats.sem.too_few_columns'
  | 'stats.sem.too_many_columns'
  | 'stats.sem.unknown_column'
  | 'stats.sem.column_non_finite'
  | 'stats.sem.column_holds_text'
  | 'stats.sem.code_equals_score'
  | 'stats.sem.model_not_identified'
  | 'stats.sem.no_complete_rows'
  | 'stats.sem.too_few_rows'
  | 'stats.sem.column_constant'
  | 'stats.sem.suspect_missing_code'
  | 'stats.sem.sample_covariance_singular'
  | 'stats.sem.sem_not_converged'
  | 'stats.sem.solution_not_identified'
  | 'stats.sem.fixed_parameter'
  | 'stats.sem.df_zero'
  | 'stats.sem.cmin_zero'
  | 'stats.sem.independence_cmin_zero'
  | 'stats.sem.too_few_rows_for_bcc'
  | 'stats.sem.negative_variance'
  | 'stats.sem.too_small'
  | 'stats.sem.overflow'
  | 'stats.sem.distribution_out_of_range'
  | 'stats.sem.covariance_not_positive_definite'
  | 'stats.sem.nonrecursive'
  | 'stats.sem.missing_value'
  | 'stats.sem.missing_code'
  | 'stats.sem.ml_only'
  | 'stats.sem.no_means'
  | 'stats.sem.one_group_only'
  | 'stats.sem.indicators_measure_one_factor'
  | 'stats.sem.equal_one_kind'
  | 'stats.sem.marker_identification_only'
  | 'stats.sem.no_fixed_values'
  | 'stats.sem.effects_structural_only'
  | 'stats.sem.no_indirect_effect_tests'
  | 'stats.sem.no_stability_index'
  | 'stats.sem.no_modification_indices'
  | 'stats.sem.normal_theory_standard_errors'
  | 'stats.sem.no_srmr'
  | 'stats.sem.listwise_only'
  | 'stats.sem.no_weights';

export type SemParamKey = 'column' | 'count' | 'index' | 'value' | 'limit' | 'expected' | 'found';

/** A finding: a closed code with numeric parameters. */
export interface SemDiagnostic {
  code: SemCode;
  params: Partial<Record<SemParamKey, number>>;
  severity: Severity;
}

export interface SemValidationView {
  status: ValidationStatus;
  errors: SemDiagnostic[];
  warnings: SemDiagnostic[];
  missingInformation: SemDiagnostic[];
  estimatedResources: ResourceEstimate;
}

/** A run's state, with its progress in steps: the check points done and their total. */
export interface SemRunStateView {
  runId: number;
  state: RunState;
  terminal: boolean;
  elapsedMs: number;
  cancelRequested: boolean;
  stepsDone: number;
  stepsTotal: number;
  diagnostics: SemDiagnostic[];
}

/** A number, or the closed reason it is not defined. */
export type SemStat = { value: number } | { not_computable: SemCode };

/** stats_sem::SemVariable: a factor by its number from 1, or a column by its alias. Factors
 *  come first, by number, then columns by alias. */
export type SemVariable = { factor: number } | { column: string };

/** A single-headed arrow from `from` to `to`. */
export interface SemPath {
  to: SemVariable;
  from: SemVariable;
}

/** A double-headed arrow between `a` and `b`, between their error terms where they are
 *  endogenous. */
export interface SemCovariance {
  a: SemVariable;
  b: SemVariable;
}

/** One member of a group held equal, as stats_sem::SemParameter serialises it. */
export type SemParameter =
  | { loading: { factor: number; column: string } }
  | { path: SemPath }
  | { covariance: SemCovariance }
  | { variance: { variable: SemVariable } };

/** One single-headed arrow's weight (AMOS's "Regression Weights"). */
export interface SemWeight {
  to: SemVariable;
  from: SemVariable;
  kind: 'loading' | 'path';
  estimate: SemStat;
  se: SemStat;
  cr: SemStat;
  p: SemStat;
  standardized: SemStat;
}

/** One double-headed arrow's covariance and its correlation. */
export interface SemCovarianceEstimate {
  a: SemVariable;
  b: SemVariable;
  estimate: SemStat;
  se: SemStat;
  cr: SemStat;
  p: SemStat;
  correlation: SemStat;
}

/** One variable's variance, or its error term's (`error`). */
export interface SemVariance {
  variable: SemVariable;
  error: boolean;
  estimate: SemStat;
  se: SemStat;
  cr: SemStat;
  p: SemStat;
}

/** One endogenous variable's squared multiple correlation. */
export interface SemSmc {
  variable: SemVariable;
  value: SemStat;
}

/** The total, direct and indirect effects of `from` on `to`, and each standardized. */
export interface SemEffect {
  to: SemVariable;
  from: SemVariable;
  total: SemStat;
  direct: SemStat;
  indirect: SemStat;
  totalStandardized: SemStat;
  directStandardized: SemStat;
  indirectStandardized: SemStat;
}

/** One model's row of AMOS's measures of fit (its Appendix C): 38 measures, NPAR and DF
 *  whole numbers. The 90% intervals are `…Lo90` and `…Hi90`. */
export interface SemModelFit {
  npar: number;
  cmin: SemStat;
  df: number;
  p: SemStat;
  cminDf: SemStat;
  rmr: SemStat;
  gfi: SemStat;
  agfi: SemStat;
  pgfi: SemStat;
  nfi: SemStat;
  rfi: SemStat;
  ifi: SemStat;
  tli: SemStat;
  cfi: SemStat;
  pratio: SemStat;
  pnfi: SemStat;
  pcfi: SemStat;
  ncp: SemStat;
  ncpLo90: SemStat;
  ncpHi90: SemStat;
  fmin: SemStat;
  f0: SemStat;
  f0Lo90: SemStat;
  f0Hi90: SemStat;
  rmsea: SemStat;
  rmseaLo90: SemStat;
  rmseaHi90: SemStat;
  pclose: SemStat;
  aic: SemStat;
  bcc: SemStat;
  bic: SemStat;
  caic: SemStat;
  ecvi: SemStat;
  ecviLo90: SemStat;
  ecviHi90: SemStat;
  mecvi: SemStat;
  hoelter05: SemStat;
  hoelter01: SemStat;
}

/** AMOS's three rows: the default model (the one specified), the saturated model and the
 *  independence model. */
export interface SemFit {
  model: SemModelFit;
  saturated: SemModelFit;
  independence: SemModelFit;
}

export interface SemExclusion {
  code: SemCode;
  count: number;
  params: Partial<Record<SemParamKey, number>>;
}

/** stats_sem::SemOutcome. */
export interface SemResult {
  /** The model's columns, in alias order. */
  columns: string[];
  rowsUsed: number;
  /** The factors' columns, as the spec. */
  indicators: string[][];
  /** The loadings, factor by factor, the fixed first ones included, then the paths in the
   *  spec's order. */
  weights: SemWeight[];
  /** The spec's covariances, in its order. */
  covariances: SemCovarianceEstimate[];
  /** Every variable's variance, or its error term's, in the model's order. */
  variances: SemVariance[];
  /** Every endogenous variable's, in the model's order. */
  smc: SemSmc[];
  /** Among the structural variables, each pair joined by a directed path. */
  effects: SemEffect[];
  fit: SemFit;
  /** The steps taken by the attempt that reached the solution, from 0 to 500. */
  iterations: number;
  /** Which attempt reached it: 1, 2 or 3. */
  attempt: number;
  exclusions: SemExclusion[];
  notes: SemCode[];
  limitations: SemCode[];
}

/** Hashes, versions and counts: no header, label, cell, name or path. */
export interface SemRunManifest {
  schemaVersion: 1;
  capabilityId: string;
  capabilityVersion: number;
  engineVersion: number;
  inputs: string[];
  parametersHash: string;
  seed: number | null;
  rowsRead: number;
  rowsUsed: number;
  exclusions: SemExclusion[];
  platform: Platform;
  elapsedMs: number;
  outputHash: string;
  reproducibility: Reproducibility;
}

export interface SemResultView {
  runId: number;
  status: RunState;
  result: SemResult;
  diagnostics: SemDiagnostic[];
  manifest: SemRunManifest;
}

/* ---- what the screen sends -------------------------------------------- */

/** stats_sem::SemParams: camelCase, deny_unknown_fields, no defaults. */
export interface SemParams {
  /** The factors, in the order of their first columns, each its columns' aliases in the
   *  file's order. */
  indicators: string[][];
  /** Ascending by (to, from), in the model's order. */
  paths: SemPath[];
  /** Each with a before b, ascending by (a, b). */
  covariances: SemCovariance[];
  /** Each group ascending, the groups by their first members, in Rust's parameter order. */
  equal: SemParameter[][];
  /** Ticked candidate missing codes, ascending, once each. */
  missingCodes: number[];
}

export interface SemSpec {
  schemaVersion: 1;
  operationId: number;
  workspaceId: 'statistics';
  capabilityId: 'stats.analyze.sem';
  capabilityVersion: 1;
  inputs: InputRef[];
  parameters: SemParams;
  randomSeed: null;
  /** The structural equation model takes no confirmation. */
  confirmationRefs: [];
  /** Decision 490: what made a prepared input. Absent on a file's own table, never null. */
  preparation?: Preparation;
}
