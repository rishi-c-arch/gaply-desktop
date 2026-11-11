// Gaply — the linear regression's wire types (Research OS, Phase 9A-2, decision 684).
// TypeScript mirrors of the views src-tauri/src/stats_linear_regression_commands.rs returns
// and of the spec it takes. Every view field is pinned by a JSON fixture in ./fixtures
// (linearRegression*View*.json), which src-tauri/tests/stats_linear_regression_wire.rs
// serialises byte for byte and linearRegressionTypes.vitest.ts deep-compares with a typed copy.
//
// The dataset is opened with Phase 2B's stats_desc_open and closed with stats_desc_close,
// unchanged. The method has no levels, so there is no levels command.
//
// What these hold: result numbers, for this device's screen only. The spec and the
// result name columns by alias; never a label, a row or a path.
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

/** Every refusal, outcome, exclusion, note and limitation of the linear regression
 *  (stats_linear_regression::LinearRegressionCode), in Rust's order. */
export type LinearRegressionCode =
  | 'stats.linear_regression.unknown_capability'
  | 'stats.linear_regression.capability_version_mismatch'
  | 'stats.linear_regression.input_count'
  | 'stats.linear_regression.input_mismatch'
  | 'stats.linear_regression.preparation_mismatch'
  | 'stats.linear_regression.missing_code_not_finite'
  | 'stats.linear_regression.no_predictors'
  | 'stats.linear_regression.too_many_predictors'
  | 'stats.linear_regression.column_repeated'
  | 'stats.linear_regression.unknown_column'
  | 'stats.linear_regression.column_non_finite'
  | 'stats.linear_regression.column_holds_text'
  | 'stats.linear_regression.code_equals_score'
  | 'stats.linear_regression.no_complete_rows'
  | 'stats.linear_regression.too_few_rows'
  | 'stats.linear_regression.outcome_constant'
  | 'stats.linear_regression.predictor_constant'
  | 'stats.linear_regression.predictors_collinear'
  | 'stats.linear_regression.fit_out_of_range'
  | 'stats.linear_regression.suspect_missing_code'
  | 'stats.linear_regression.perfect_fit'
  | 'stats.linear_regression.distribution_out_of_range'
  | 'stats.linear_regression.overflow'
  | 'stats.linear_regression.too_small'
  | 'stats.linear_regression.predicted_no_spread'
  | 'stats.linear_regression.essentially_perfect_fit'
  | 'stats.linear_regression.missing_value'
  | 'stats.linear_regression.missing_code'
  | 'stats.linear_regression.enter_only'
  | 'stats.linear_regression.no_categorical_predictors'
  | 'stats.linear_regression.no_interaction_terms'
  | 'stats.linear_regression.intercept_always'
  | 'stats.linear_regression.listwise_only'
  | 'stats.linear_regression.no_robust_standard_errors'
  | 'stats.linear_regression.no_casewise_diagnostics'
  | 'stats.linear_regression.durbin_watson_row_order'
  | 'stats.linear_regression.intervals_95_only'
  | 'stats.linear_regression.no_weights';

export type LinearRegressionParamKey = 'column' | 'count' | 'index' | 'value' | 'limit' | 'expected' | 'found';

/** A finding: a closed code with numeric parameters. */
export interface LinearRegressionDiagnostic {
  code: LinearRegressionCode;
  params: Partial<Record<LinearRegressionParamKey, number>>;
  severity: Severity;
}

export interface LinearRegressionValidationView {
  status: ValidationStatus;
  errors: LinearRegressionDiagnostic[];
  warnings: LinearRegressionDiagnostic[];
  missingInformation: LinearRegressionDiagnostic[];
  estimatedResources: ResourceEstimate;
}

/** A run's state, with its progress in steps: the check points done and their total, one. */
export interface LinearRegressionRunStateView {
  runId: number;
  state: RunState;
  terminal: boolean;
  elapsedMs: number;
  cancelRequested: boolean;
  stepsDone: number;
  stepsTotal: number;
  diagnostics: LinearRegressionDiagnostic[];
}

/** A number, or the closed reason it is not defined. */
export type LinearRegressionStat = { value: number } | { not_computable: LinearRegressionCode };

/** One column of the model over the complete rows: the outcome first, then each predictor
 *  in the list's order. */
export interface RegressionColumn {
  column: string;
  mean: LinearRegressionStat;
  /** The standard deviation, n − 1. */
  sd: LinearRegressionStat;
}

/** The model summary. */
export interface ModelSummary {
  r: LinearRegressionStat;
  rSquared: LinearRegressionStat;
  adjustedRSquared: LinearRegressionStat;
  /** σ, the root of the residual mean square. */
  standardErrorOfEstimate: LinearRegressionStat;
  durbinWatson: LinearRegressionStat;
}

export interface AnovaSource {
  sumOfSquares: LinearRegressionStat;
  df: number;
  meanSquare: LinearRegressionStat;
}

/** The total row has no mean square on the wire. */
export interface AnovaTotal {
  sumOfSquares: LinearRegressionStat;
  df: number;
}

export interface RegressionAnova {
  regression: AnovaSource;
  residual: AnovaSource;
  total: AnovaTotal;
  f: LinearRegressionStat;
  p: LinearRegressionStat;
}

export interface InterceptEstimate {
  b: LinearRegressionStat;
  standardError: LinearRegressionStat;
  t: LinearRegressionStat;
  p: LinearRegressionStat;
  lower: LinearRegressionStat;
  upper: LinearRegressionStat;
}

export interface PredictorEstimate {
  column: string;
  b: LinearRegressionStat;
  standardError: LinearRegressionStat;
  beta: LinearRegressionStat;
  t: LinearRegressionStat;
  p: LinearRegressionStat;
  lower: LinearRegressionStat;
  upper: LinearRegressionStat;
  zeroOrder: LinearRegressionStat;
  partial: LinearRegressionStat;
  part: LinearRegressionStat;
  tolerance: LinearRegressionStat;
  vif: LinearRegressionStat;
}

/** A minimum, maximum, mean and standard deviation. */
export interface Spread {
  minimum: LinearRegressionStat;
  maximum: LinearRegressionStat;
  mean: LinearRegressionStat;
  standardDeviation: LinearRegressionStat;
}

/** A minimum and maximum only. */
export interface Extremes {
  minimum: LinearRegressionStat;
  maximum: LinearRegressionStat;
}

export interface ResidualSummary {
  predicted: Spread;
  residual: Spread;
  standardizedPredicted: Extremes;
  standardizedResidual: Extremes;
}

export interface LinearRegressionExclusion {
  code: LinearRegressionCode;
  count: number;
  params: Partial<Record<LinearRegressionParamKey, number>>;
}

/** stats_linear_regression::LinearRegressionOutcome. */
export interface LinearRegressionResult {
  outcome: string;
  rowsUsed: number;
  /** The outcome, then each predictor in the list's order. */
  columns: RegressionColumn[];
  model: ModelSummary;
  anova: RegressionAnova;
  intercept: InterceptEstimate;
  /** In the list's order. */
  predictors: PredictorEstimate[];
  residuals: ResidualSummary;
  exclusions: LinearRegressionExclusion[];
  notes: LinearRegressionCode[];
  limitations: LinearRegressionCode[];
}

/** Hashes, versions and counts: no header, label, cell, name or path. */
export interface LinearRegressionRunManifest {
  schemaVersion: 1;
  capabilityId: string;
  capabilityVersion: number;
  engineVersion: number;
  inputs: string[];
  parametersHash: string;
  seed: number | null;
  rowsRead: number;
  rowsUsed: number;
  exclusions: LinearRegressionExclusion[];
  platform: Platform;
  elapsedMs: number;
  outputHash: string;
  reproducibility: Reproducibility;
}

export interface LinearRegressionResultView {
  runId: number;
  status: RunState;
  result: LinearRegressionResult;
  diagnostics: LinearRegressionDiagnostic[];
  manifest: LinearRegressionRunManifest;
}

/* ---- what the screen sends -------------------------------------------- */

/** stats_linear_regression::LinearRegressionParams: camelCase, deny_unknown_fields. */
export interface LinearRegressionParams {
  /** The outcome's alias. */
  outcome: string;
  /** The predictors' aliases, in the order the user chose them, never sorted. */
  predictors: string[];
  /** Ticked candidate missing codes, ascending, once each. */
  missingCodes: number[];
}

export interface LinearRegressionSpec {
  schemaVersion: 1;
  operationId: number;
  workspaceId: 'statistics';
  capabilityId: 'stats.analyze.linear_regression';
  capabilityVersion: 1;
  inputs: InputRef[];
  parameters: LinearRegressionParams;
  randomSeed: null;
  /** The linear regression takes no confirmation. */
  confirmationRefs: [];
  /** Decision 490: what made a prepared input. Absent on a file's own table, never null. */
  preparation?: Preparation;
}
