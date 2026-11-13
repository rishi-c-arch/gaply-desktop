// Gaply — the factor analysis's wire types (Research OS, Phase 9B-3, decision 729).
// TypeScript mirrors of the views src-tauri/src/stats_factor_commands.rs returns and of the
// spec it takes. Every view field is pinned by a JSON fixture in ./fixtures (factor*View*.json),
// which src-tauri/tests/stats_factor_wire.rs serialises byte for byte and factorTypes.vitest.ts
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

/** Every refusal, outcome, exclusion, note and limitation of the factor analysis
 *  (stats_factor::FactorCode), in Rust's order. */
export type FactorCode =
  | 'stats.factor.unknown_capability'
  | 'stats.factor.capability_version_mismatch'
  | 'stats.factor.input_count'
  | 'stats.factor.input_mismatch'
  | 'stats.factor.preparation_mismatch'
  | 'stats.factor.missing_code_not_finite'
  | 'stats.factor.missing_code_repeated'
  | 'stats.factor.missing_codes_out_of_order'
  | 'stats.factor.too_few_items'
  | 'stats.factor.too_many_items'
  | 'stats.factor.items_out_of_order'
  | 'stats.factor.item_repeated'
  | 'stats.factor.unknown_column'
  | 'stats.factor.column_non_finite'
  | 'stats.factor.column_holds_text'
  | 'stats.factor.code_equals_score'
  | 'stats.factor.factors_out_of_range'
  | 'stats.factor.iterations_out_of_range'
  | 'stats.factor.convergence_out_of_range'
  | 'stats.factor.no_complete_rows'
  | 'stats.factor.too_few_rows'
  | 'stats.factor.item_constant'
  | 'stats.factor.correlation_out_of_range'
  | 'stats.factor.no_eigenvalue_over_one'
  | 'stats.factor.correlations_singular'
  | 'stats.factor.extraction_eigenvalue_not_positive'
  | 'stats.factor.extraction_not_converged'
  | 'stats.factor.rotation_not_converged'
  | 'stats.factor.rotation_singular'
  | 'stats.factor.extraction_eigenvalue_negative'
  | 'stats.factor.eigen_not_converged'
  | 'stats.factor.suspect_missing_code'
  | 'stats.factor.too_small'
  | 'stats.factor.no_correlation'
  | 'stats.factor.distribution_out_of_range'
  | 'stats.factor.overflow'
  | 'stats.factor.communality_above_one'
  | 'stats.factor.one_factor_not_rotated'
  | 'stats.factor.eigenvalues_not_distinct'
  | 'stats.factor.missing_value'
  | 'stats.factor.missing_code'
  | 'stats.factor.no_ml_or_other_extraction'
  | 'stats.factor.no_oblique_other_than_promax'
  | 'stats.factor.promax_kappa_four'
  | 'stats.factor.no_factor_scores'
  | 'stats.factor.no_matrices_shown'
  | 'stats.factor.pearson_only'
  | 'stats.factor.listwise_only'
  | 'stats.factor.no_scree_plot'
  | 'stats.factor.no_weights';

export type FactorParamKey = 'column' | 'count' | 'index' | 'value' | 'limit' | 'expected' | 'found';

/** A finding: a closed code with numeric parameters. */
export interface FactorDiagnostic {
  code: FactorCode;
  params: Partial<Record<FactorParamKey, number>>;
  severity: Severity;
}

export interface FactorValidationView {
  status: ValidationStatus;
  errors: FactorDiagnostic[];
  warnings: FactorDiagnostic[];
  missingInformation: FactorDiagnostic[];
  estimatedResources: ResourceEstimate;
}

/** A run's state, with its progress in steps: the check points done and their total. */
export interface FactorRunStateView {
  runId: number;
  state: RunState;
  terminal: boolean;
  elapsedMs: number;
  cancelRequested: boolean;
  stepsDone: number;
  stepsTotal: number;
  diagnostics: FactorDiagnostic[];
}

/** A number, or the closed reason it is not defined. */
export type FactorStat = { value: number } | { not_computable: FactorCode };

export type Extraction = 'principal_components' | 'principal_axis';
export type Rotation = 'none' | 'varimax' | 'promax';

/** One item's mean and standard deviation (n − 1) over the complete rows. */
export interface ItemDescriptive {
  item: string;
  mean: FactorStat;
  sd: FactorStat;
}

export interface Determinant {
  value: FactorStat;
  log: FactorStat;
}

export interface ItemAdequacy {
  item: string;
  msa: FactorStat;
}

export interface SamplingAdequacy {
  overall: FactorStat;
  items: ItemAdequacy[];
}

export interface Sphericity {
  chiSquare: FactorStat;
  df: number;
  p: FactorStat;
}

/** A variance with its percent of the items' total and the cumulative percent. */
export interface FactorSums {
  total: FactorStat;
  percent: FactorStat;
  cumulative: FactorStat;
}

export interface Communality {
  item: string;
  initial: FactorStat;
  extraction: FactorStat;
}

/** The extraction: m factors, its iterations (0 for principal components), the
 *  communalities, each factor's sums of squared loadings, and the loadings, p rows of m. */
export interface ExtractedFactors {
  method: Extraction;
  factors: number;
  iterations: number;
  communalities: Communality[];
  sums: FactorSums[];
  loadings: FactorStat[][];
}

/** The rotation. Varimax gives the transformation and the sums; promax the structure and the
 *  factor correlations; a field the method does not give is null. Its iterations are
 *  varimax's cycles, promax's too. Never a 'none' method: no rotation is a null rotation. */
export interface RotatedFactors {
  method: Exclude<Rotation, 'none'>;
  iterations: number;
  loadings: FactorStat[][];
  transformation: FactorStat[][] | null;
  sums: FactorSums[] | null;
  structure: FactorStat[][] | null;
  factorCorrelations: FactorStat[][] | null;
}

export interface FactorExclusion {
  code: FactorCode;
  count: number;
  params: Partial<Record<FactorParamKey, number>>;
}

/** stats_factor::FactorOutcome. */
export interface FactorResult {
  items: string[];
  rowsUsed: number;
  descriptives: ItemDescriptive[];
  determinant: Determinant;
  kmo: SamplingAdequacy;
  bartlett: Sphericity;
  /** Every eigenvalue of the correlation matrix, descending. */
  eigenvalues: FactorSums[];
  extraction: ExtractedFactors;
  /** Null with no rotation, one factor, or a rotation that could not be given. */
  rotation: RotatedFactors | null;
  exclusions: FactorExclusion[];
  notes: FactorCode[];
  limitations: FactorCode[];
}

/** Hashes, versions and counts: no header, label, cell, name or path. */
export interface FactorRunManifest {
  schemaVersion: 1;
  capabilityId: string;
  capabilityVersion: number;
  engineVersion: number;
  inputs: string[];
  parametersHash: string;
  seed: number | null;
  rowsRead: number;
  rowsUsed: number;
  exclusions: FactorExclusion[];
  platform: Platform;
  elapsedMs: number;
  outputHash: string;
  reproducibility: Reproducibility;
}

export interface FactorResultView {
  runId: number;
  status: RunState;
  result: FactorResult;
  diagnostics: FactorDiagnostic[];
  manifest: FactorRunManifest;
}

/* ---- what the screen sends -------------------------------------------- */

/** stats_factor::FactorParams: camelCase, deny_unknown_fields, no defaults. */
export interface FactorParams {
  /** The items' aliases, in the file's order. */
  items: string[];
  extraction: Extraction;
  /** A fixed number of factors, or null for every eigenvalue above 1. */
  factors: number | null;
  rotation: Rotation;
  /** SPSS's ITERATE: principal axis's iterations and the rotation's cycles alike. */
  iterations: number;
  /** Principal axis's convergence. */
  convergence: number;
  /** Ticked candidate missing codes, ascending, once each. */
  missingCodes: number[];
}

export interface FactorSpec {
  schemaVersion: 1;
  operationId: number;
  workspaceId: 'statistics';
  capabilityId: 'stats.analyze.factor';
  capabilityVersion: 1;
  inputs: InputRef[];
  parameters: FactorParams;
  randomSeed: null;
  /** The factor analysis takes no confirmation. */
  confirmationRefs: [];
  /** Decision 490: what made a prepared input. Absent on a file's own table, never null. */
  preparation?: Preparation;
}
