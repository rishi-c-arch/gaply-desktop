// Gaply — the related rank tests' wire types (Research OS, Phase 8Z-2, decision 653).
// TypeScript mirrors of the views src-tauri/src/stats_related_ranks_commands.rs returns
// and of the spec it takes. Every view field is pinned by a JSON fixture in ./fixtures
// (relatedRanks*View*.json), which src-tauri/tests/stats_related_ranks_wire.rs serialises
// byte for byte and relatedRanksTypes.vitest.ts deep-compares with a typed copy.
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

/** Every refusal, outcome, exclusion, note and limitation of the related rank tests
 *  (stats_related_ranks::RelatedRanksCode). */
export type RelatedRanksCode =
  | 'stats.related_ranks.unknown_capability'
  | 'stats.related_ranks.capability_version_mismatch'
  | 'stats.related_ranks.input_count'
  | 'stats.related_ranks.input_mismatch'
  | 'stats.related_ranks.preparation_mismatch'
  | 'stats.related_ranks.missing_code_not_finite'
  | 'stats.related_ranks.too_few_columns'
  | 'stats.related_ranks.too_many_columns'
  | 'stats.related_ranks.column_repeated'
  | 'stats.related_ranks.unknown_column'
  | 'stats.related_ranks.column_non_finite'
  | 'stats.related_ranks.column_holds_text'
  | 'stats.related_ranks.code_equals_score'
  | 'stats.related_ranks.no_complete_rows'
  | 'stats.related_ranks.suspect_missing_code'
  | 'stats.related_ranks.all_differences_zero'
  | 'stats.related_ranks.all_values_tied'
  | 'stats.related_ranks.distribution_out_of_range'
  | 'stats.related_ranks.overflow'
  | 'stats.related_ranks.exact_not_given_ties'
  | 'stats.related_ranks.exact_not_given_zeros'
  | 'stats.related_ranks.exact_not_given_size'
  | 'stats.related_ranks.zero_differences_dropped'
  | 'stats.related_ranks.rows_below_five'
  | 'stats.related_ranks.missing_value'
  | 'stats.related_ranks.missing_code'
  | 'stats.related_ranks.no_pairwise_comparisons'
  | 'stats.related_ranks.two_sided_only'
  | 'stats.related_ranks.no_location_estimate'
  | 'stats.related_ranks.exact_without_ties_or_zeros_only'
  | 'stats.related_ranks.pratt_not_offered'
  | 'stats.related_ranks.no_further_related_tests'
  | 'stats.related_ranks.no_weights'
  | 'stats.related_ranks.column_order';

export type RelatedRanksParamKey = 'column' | 'count' | 'index' | 'value' | 'limit' | 'expected' | 'found';

/** A finding: a closed code with numeric parameters. */
export interface RelatedRanksDiagnostic {
  code: RelatedRanksCode;
  params: Partial<Record<RelatedRanksParamKey, number>>;
  severity: Severity;
}

export interface RelatedRanksValidationView {
  status: ValidationStatus;
  errors: RelatedRanksDiagnostic[];
  warnings: RelatedRanksDiagnostic[];
  missingInformation: RelatedRanksDiagnostic[];
  estimatedResources: ResourceEstimate;
}

/** A run's state, with its progress in steps: the check points done and their total, one. */
export interface RelatedRanksRunStateView {
  runId: number;
  state: RunState;
  terminal: boolean;
  elapsedMs: number;
  cancelRequested: boolean;
  stepsDone: number;
  stepsTotal: number;
  diagnostics: RelatedRanksDiagnostic[];
}

/** A number, or the closed reason it is not defined. */
export type RelatedRanksStat = { value: number } | { not_computable: RelatedRanksCode };

/** One chosen column, by its alias, in the user's order. */
export interface RelatedColumn {
  column: string;
  median: number;
}

/** One sign's ranks; its mean is null when it has no pair. */
export interface SignRanks {
  count: number;
  rankSum: number;
  meanRank: number | null;
}

/** The Wilcoxon signed-rank test of the first column less the second. */
export interface Wilcoxon {
  first: string;
  second: string;
  n: number;
  zeroDifferences: number;
  tieRuns: number;
  negative: SignRanks;
  positive: SignRanks;
  v: number;
  tMinus: number;
  z: RelatedRanksStat;
  /** The normal approximation's two-sided p, without a continuity correction. */
  p: RelatedRanksStat;
  /** Given only with no tied size, no zero and fewer than 50 pairs; the notes say why not. */
  exactP: number | null;
  rankBiserial: RelatedRanksStat;
  medianDifference: RelatedRanksStat;
}

export interface FriedmanColumn {
  column: string;
  rankSum: number;
  meanRank: number;
}

/** The Friedman test of three or more columns. */
export interface Friedman {
  columns: FriedmanColumn[];
  q: RelatedRanksStat;
  df: number;
  p: RelatedRanksStat;
  kendallsW: RelatedRanksStat;
}

/** The test the count of columns chose. */
export type RelatedTest = { wilcoxon: Wilcoxon } | { friedman: Friedman };

export interface RelatedRanksExclusion {
  code: RelatedRanksCode;
  count: number;
  params: Partial<Record<RelatedRanksParamKey, number>>;
}

/** stats_related_ranks::RelatedRanksOutcome. */
export interface RelatedRanksResult {
  columns: RelatedColumn[];
  rowsUsed: number;
  test: RelatedTest;
  exclusions: RelatedRanksExclusion[];
  notes: RelatedRanksCode[];
  limitations: RelatedRanksCode[];
}

/** Hashes, versions and counts: no header, label, cell, name or path. */
export interface RelatedRanksRunManifest {
  schemaVersion: 1;
  capabilityId: string;
  capabilityVersion: number;
  engineVersion: number;
  inputs: string[];
  parametersHash: string;
  seed: number | null;
  rowsRead: number;
  rowsUsed: number;
  exclusions: RelatedRanksExclusion[];
  platform: Platform;
  elapsedMs: number;
  outputHash: string;
  reproducibility: Reproducibility;
}

export interface RelatedRanksResultView {
  runId: number;
  status: RunState;
  result: RelatedRanksResult;
  diagnostics: RelatedRanksDiagnostic[];
  manifest: RelatedRanksRunManifest;
}

/* ---- what the screen sends -------------------------------------------- */

/** stats_related_ranks::RelatedRanksParams: camelCase, deny_unknown_fields. */
export interface RelatedRanksParams {
  /** The columns' aliases, in the order the user chose them: the first less the second. */
  columns: string[];
  /** Ticked candidate missing codes, ascending, once each. */
  missingCodes: number[];
}

export interface RelatedRanksSpec {
  schemaVersion: 1;
  operationId: number;
  workspaceId: 'statistics';
  capabilityId: 'stats.analyze.related_ranks';
  capabilityVersion: 1;
  inputs: InputRef[];
  parameters: RelatedRanksParams;
  randomSeed: null;
  /** The related rank tests take no confirmation. */
  confirmationRefs: [];
  /** Decision 490: what made a prepared input. Absent on a file's own table, never null. */
  preparation?: Preparation;
}
