// Gaply — the correlations' wire types (Research OS, Phase 8H-2, decision
// 356). TypeScript mirrors of the views src-tauri/src/stats_corr_commands.rs
// returns and of the spec it takes. Every view field is pinned by a JSON
// fixture in ./fixtures (corr*.json), which src-tauri/tests/stats_corr_wire.rs
// serialises byte for byte and corrTypes.vitest.ts deep-compares with a typed
// copy.
//
// The dataset is opened with Phase 2B's stats_desc_open and closed with
// stats_desc_close, so those types are describe's, unchanged.
//
// What these hold: aliases, numbers and closed codes. The result names columns
// by alias (C1, C2, …), never by header; the screen names them from the
// dataset's own profile, a saved result by position. The spec names columns by
// alias; never a header, a row or a path.
import type {
  InputRef,
  Platform,
  Reproducibility,
  ResourceEstimate,
  RunState,
  Severity,
  ValidationStatus,
} from './describeTypes';

/** Every finding, outcome, exclusion, note and limitation of the
 *  correlations. */
export type CorrCode =
  | 'stats.corr.unknown_capability'
  | 'stats.corr.capability_version_mismatch'
  | 'stats.corr.input_count'
  | 'stats.corr.input_mismatch'
  | 'stats.corr.unknown_column'
  | 'stats.corr.too_few_variables'
  | 'stats.corr.variable_repeated'
  | 'stats.corr.variables_out_of_order'
  | 'stats.corr.no_coefficient'
  | 'stats.corr.coefficient_repeated'
  | 'stats.corr.coefficients_out_of_order'
  | 'stats.corr.missing_code_not_finite'
  | 'stats.corr.missing_code_repeated'
  | 'stats.corr.missing_codes_out_of_order'
  | 'stats.corr.variable_holds_text'
  | 'stats.corr.variable_non_finite'
  | 'stats.corr.suspect_missing_code'
  | 'stats.corr.too_few_complete_rows'
  | 'stats.corr.no_pair_of_three'
  | 'stats.corr.over_budget'
  | 'stats.corr.pair_too_few_rows'
  | 'stats.corr.variable_constant'
  | 'stats.corr.distribution_out_of_range'
  | 'stats.corr.overflow'
  | 'stats.corr.underflow'
  | 'stats.corr.arithmetic_refused'
  | 'stats.corr.interval_too_few_rows'
  | 'stats.corr.no_values'
  | 'stats.corr.too_few_values'
  | 'stats.corr.perfect_coefficient'
  | 'stats.corr.missing_value'
  | 'stats.corr.missing_code'
  | 'stats.corr.fewer_than_two_values'
  | 'stats.corr.pearson_assumes_normality'
  | 'stats.corr.two_tailed_no_partial'
  | 'stats.corr.preparation_mismatch'
  | 'stats.corr.code_equals_score';

export type CorrParamKey = 'column' | 'count' | 'index' | 'value' | 'limit' | 'expected' | 'found';

/** A finding: a closed code with numeric parameters. */
export interface CorrDiagnostic {
  code: CorrCode;
  params: Partial<Record<CorrParamKey, number>>;
  severity: Severity;
}

export interface CorrValidationView {
  status: ValidationStatus;
  errors: CorrDiagnostic[];
  warnings: CorrDiagnostic[];
  missingInformation: CorrDiagnostic[];
  estimatedResources: ResourceEstimate;
}

/** A correlations run's state, with its progress in steps: the check points
 *  done and their total (decision 355), never columns. */
export interface CorrRunStateView {
  runId: number;
  state: RunState;
  terminal: boolean;
  elapsedMs: number;
  cancelRequested: boolean;
  stepsDone: number;
  stepsTotal: number;
  diagnostics: CorrDiagnostic[];
}

export type Coefficient = 'pearson' | 'spearman' | 'kendall';

/** Every coefficient, in the order the method runs them (decision 338). */
export const COEFFICIENTS: readonly Coefficient[] = ['pearson', 'spearman', 'kendall'];

export type MissingHandling = 'pairwise' | 'listwise';

/** A number, or the closed reason it is not defined. */
export type CorrStat = { value: number } | { not_computable: CorrCode };

/** A t: a number, or perfect_coefficient where it is infinite. */
export type Bounded = { value: number } | { infinite: CorrCode };

/** The test statistic: t with its df for Pearson and Spearman, z for
 *  Kendall. */
export type TestStatistic = { t: { t: Bounded; df: number } } | { z: number };

export type Interval = { computed: { lower: number; upper: number } } | { not_computable: CorrCode };

/** One variable's n, mean and SD over the rows used. */
export interface Descriptive {
  variable: string;
  n: number;
  mean: CorrStat;
  sd: CorrStat;
}

export interface PairStats {
  estimate: number;
  test: TestStatistic;
  p: number;
  interval: Interval;
}

export type PairOutcome = { computed: PairStats } | { not_computable: CorrCode };

/** One unordered pair, first before second in the column order. */
export interface Pair {
  first: string;
  second: string;
  n: number;
  outcome: PairOutcome;
}

export interface CoefficientTable {
  coefficient: Coefficient;
  pairs: Pair[];
}

export interface CorrExclusion {
  code: CorrCode;
  count: number;
  params: Partial<Record<CorrParamKey, number>>;
}

export interface CorrResult {
  /** In the variables' order. */
  descriptives: Descriptive[];
  /** The chosen coefficients, in COEFFICIENTS' order. */
  coefficients: CoefficientTable[];
  rowsUsed: number;
  exclusions: CorrExclusion[];
  limitations: CorrCode[];
}

/** Hashes, versions and counts: no header, cell, name or path. */
export interface CorrRunManifest {
  schemaVersion: 1;
  capabilityId: string;
  capabilityVersion: number;
  engineVersion: number;
  inputs: string[];
  parametersHash: string;
  seed: number | null;
  rowsRead: number;
  rowsUsed: number;
  exclusions: CorrExclusion[];
  platform: Platform;
  elapsedMs: number;
  outputHash: string;
  reproducibility: Reproducibility;
}

export interface CorrResultView {
  runId: number;
  status: RunState;
  result: CorrResult;
  diagnostics: CorrDiagnostic[];
  manifest: CorrRunManifest;
}

/* ---- what the screen sends -------------------------------------------- */

export interface CorrParams {
  /** The ticked columns' aliases, e.g. "C1", in the dataset's column order;
   *  two or more, none by default. */
  variables: string[];
  /** The ticked coefficients, in COEFFICIENTS' order; one or more, none by
   *  default. */
  coefficients: Coefficient[];
  /** The chosen handling; there is no default. */
  missing: MissingHandling;
  /** Ticked candidate missing codes, ascending; they apply to every chosen
   *  variable. */
  missingCodes: number[];
}

export interface CorrSpec {
  schemaVersion: 1;
  operationId: number;
  workspaceId: 'statistics';
  capabilityId: 'stats.analyze.correlations';
  capabilityVersion: 1;
  inputs: InputRef[];
  parameters: CorrParams;
  randomSeed: null;
  /** The correlations take no confirmation. */
  confirmationRefs: [];
}
