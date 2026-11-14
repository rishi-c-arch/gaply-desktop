// Gaply — the confirmatory factor analysis's wire types (Research OS, Phase 9C-3, decision
// 782), on factorTypes.ts's pattern. TypeScript mirrors of the views
// src-tauri/src/stats_cfa_commands.rs returns and of the spec it takes. Every view field is
// pinned by a JSON fixture in ./fixtures (cfa*View*.json), which
// src-tauri/tests/stats_cfa_wire.rs serialises byte for byte and cfaTypes.vitest.ts
// deep-compares with a typed copy.
//
// The dataset is opened with Phase 2B's stats_desc_open and closed with stats_desc_close,
// unchanged. The method has no levels, so there is no levels command.
//
// What these hold: result numbers, for this device's screen only. The spec and the result
// name columns by alias; never a label, a row or a path.
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

/** Every refusal, outcome, exclusion, note and limitation of the confirmatory factor
 *  analysis (stats_cfa::CfaCode), in Rust's order. */
export type CfaCode =
  | 'stats.cfa.unknown_capability'
  | 'stats.cfa.capability_version_mismatch'
  | 'stats.cfa.input_count'
  | 'stats.cfa.input_mismatch'
  | 'stats.cfa.preparation_mismatch'
  | 'stats.cfa.missing_code_not_finite'
  | 'stats.cfa.missing_code_repeated'
  | 'stats.cfa.missing_codes_out_of_order'
  | 'stats.cfa.no_factors'
  | 'stats.cfa.too_many_factors'
  | 'stats.cfa.factor_too_few_items'
  | 'stats.cfa.two_item_factor_uncorrelated'
  | 'stats.cfa.too_few_items'
  | 'stats.cfa.too_many_items'
  | 'stats.cfa.items_out_of_order'
  | 'stats.cfa.factors_out_of_order'
  | 'stats.cfa.item_repeated'
  | 'stats.cfa.unknown_column'
  | 'stats.cfa.column_non_finite'
  | 'stats.cfa.column_holds_text'
  | 'stats.cfa.code_equals_score'
  | 'stats.cfa.no_complete_rows'
  | 'stats.cfa.too_few_rows'
  | 'stats.cfa.item_constant'
  | 'stats.cfa.sample_covariance_singular'
  | 'stats.cfa.model_not_identified'
  | 'stats.cfa.cfa_not_converged'
  | 'stats.cfa.suspect_missing_code'
  | 'stats.cfa.fixed_parameter'
  | 'stats.cfa.df_zero'
  | 'stats.cfa.cmin_zero'
  | 'stats.cfa.independence_cmin_zero'
  | 'stats.cfa.too_few_rows_for_bcc'
  | 'stats.cfa.negative_variance'
  | 'stats.cfa.too_small'
  | 'stats.cfa.overflow'
  | 'stats.cfa.distribution_out_of_range'
  | 'stats.cfa.factor_covariance_not_positive_definite'
  | 'stats.cfa.missing_value'
  | 'stats.cfa.missing_code'
  | 'stats.cfa.ml_only'
  | 'stats.cfa.no_means'
  | 'stats.cfa.one_group_only'
  | 'stats.cfa.simple_structure_only'
  | 'stats.cfa.marker_identification_only'
  | 'stats.cfa.no_modification_indices'
  | 'stats.cfa.normal_theory_standard_errors'
  | 'stats.cfa.no_srmr'
  | 'stats.cfa.listwise_only'
  | 'stats.cfa.no_weights';

export type CfaParamKey = 'column' | 'count' | 'index' | 'value' | 'limit' | 'expected' | 'found';

/** A finding: a closed code with numeric parameters. */
export interface CfaDiagnostic {
  code: CfaCode;
  params: Partial<Record<CfaParamKey, number>>;
  severity: Severity;
}

export interface CfaValidationView {
  status: ValidationStatus;
  errors: CfaDiagnostic[];
  warnings: CfaDiagnostic[];
  missingInformation: CfaDiagnostic[];
  estimatedResources: ResourceEstimate;
}

/** A run's state, with its progress in steps: the check points done and their total. */
export interface CfaRunStateView {
  runId: number;
  state: RunState;
  terminal: boolean;
  elapsedMs: number;
  cancelRequested: boolean;
  stepsDone: number;
  stepsTotal: number;
  diagnostics: CfaDiagnostic[];
}

/** A number, or the closed reason it is not defined. */
export type CfaStat = { value: number } | { not_computable: CfaCode };

/** One column's loading on its factor (numbered from 1): the estimate, its standard error,
 *  C.R. and P, and the standardized weight. A factor's first column's loading is 1, its
 *  S.E., C.R. and P fixed_parameter. */
export interface CfaLoading {
  factor: number;
  item: string;
  estimate: CfaStat;
  se: CfaStat;
  cr: CfaStat;
  p: CfaStat;
  standardized: CfaStat;
}

export interface CfaFactorVariance {
  factor: number;
  estimate: CfaStat;
  se: CfaStat;
  cr: CfaStat;
  p: CfaStat;
}

/** Two factors' covariance, [f, g] with f < g, and their correlation. */
export interface CfaFactorCovariance {
  factors: [number, number];
  estimate: CfaStat;
  se: CfaStat;
  cr: CfaStat;
  p: CfaStat;
  correlation: CfaStat;
}

export interface CfaErrorVariance {
  item: string;
  estimate: CfaStat;
  se: CfaStat;
  cr: CfaStat;
  p: CfaStat;
}

export interface CfaSmc {
  item: string;
  value: CfaStat;
}

/** One model's row of AMOS's measures of fit (its Appendix C): 38 measures, NPAR and DF
 *  whole numbers. The 90% intervals are `…Lo90` and `…Hi90`. */
export interface CfaModelFit {
  npar: number;
  cmin: CfaStat;
  df: number;
  p: CfaStat;
  cminDf: CfaStat;
  rmr: CfaStat;
  gfi: CfaStat;
  agfi: CfaStat;
  pgfi: CfaStat;
  nfi: CfaStat;
  rfi: CfaStat;
  ifi: CfaStat;
  tli: CfaStat;
  cfi: CfaStat;
  pratio: CfaStat;
  pnfi: CfaStat;
  pcfi: CfaStat;
  ncp: CfaStat;
  ncpLo90: CfaStat;
  ncpHi90: CfaStat;
  fmin: CfaStat;
  f0: CfaStat;
  f0Lo90: CfaStat;
  f0Hi90: CfaStat;
  rmsea: CfaStat;
  rmseaLo90: CfaStat;
  rmseaHi90: CfaStat;
  pclose: CfaStat;
  aic: CfaStat;
  bcc: CfaStat;
  bic: CfaStat;
  caic: CfaStat;
  ecvi: CfaStat;
  ecviLo90: CfaStat;
  ecviHi90: CfaStat;
  mecvi: CfaStat;
  hoelter05: CfaStat;
  hoelter01: CfaStat;
}

/** AMOS's three rows: the default model (the one specified), the saturated model and the
 *  independence model. */
export interface CfaFit {
  model: CfaModelFit;
  saturated: CfaModelFit;
  independence: CfaModelFit;
}

export interface CfaExclusion {
  code: CfaCode;
  count: number;
  params: Partial<Record<CfaParamKey, number>>;
}

/** stats_cfa::CfaOutcome. */
export interface CfaResult {
  /** Every column, factor by factor, as the spec names them. */
  items: string[];
  rowsUsed: number;
  /** The factors' columns, as the spec. */
  indicators: string[][];
  /** Every column's loading, in `items`'s order. */
  loadings: CfaLoading[];
  factorVariances: CfaFactorVariance[];
  /** Empty when the factors do not covary, or there is one factor. */
  factorCovariances: CfaFactorCovariance[];
  errorVariances: CfaErrorVariance[];
  smc: CfaSmc[];
  fit: CfaFit;
  /** The Fisher scoring steps taken, from 0 to 500. */
  iterations: number;
  exclusions: CfaExclusion[];
  notes: CfaCode[];
  limitations: CfaCode[];
}

/** Hashes, versions and counts: no header, label, cell, name or path. */
export interface CfaRunManifest {
  schemaVersion: 1;
  capabilityId: string;
  capabilityVersion: number;
  engineVersion: number;
  inputs: string[];
  parametersHash: string;
  seed: number | null;
  rowsRead: number;
  rowsUsed: number;
  exclusions: CfaExclusion[];
  platform: Platform;
  elapsedMs: number;
  outputHash: string;
  reproducibility: Reproducibility;
}

export interface CfaResultView {
  runId: number;
  status: RunState;
  result: CfaResult;
  diagnostics: CfaDiagnostic[];
  manifest: CfaRunManifest;
}

/* ---- what the screen sends -------------------------------------------- */

/** stats_cfa::CfaParams: camelCase, deny_unknown_fields, no defaults. */
export interface CfaParams {
  /** The factors, in the order of their first columns, each its columns' aliases in the
   *  file's order. */
  indicators: string[][];
  /** Whether every pair of factors has a covariance to estimate. */
  correlatedFactors: boolean;
  /** Ticked candidate missing codes, ascending, once each. */
  missingCodes: number[];
}

export interface CfaSpec {
  schemaVersion: 1;
  operationId: number;
  workspaceId: 'statistics';
  capabilityId: 'stats.analyze.cfa';
  capabilityVersion: 1;
  inputs: InputRef[];
  parameters: CfaParams;
  randomSeed: null;
  /** The confirmatory factor analysis takes no confirmation. */
  confirmationRefs: [];
  /** Decision 490: what made a prepared input. Absent on a file's own table, never null. */
  preparation?: Preparation;
}
