// Gaply — the rank tests' wire types (Research OS, Phase 8Y-2, decision 619).
// TypeScript mirrors of the views src-tauri/src/stats_ranks_commands.rs returns and of
// the spec it takes. Every view field is pinned by a JSON fixture in ./fixtures
// (ranks*View*.json), which src-tauri/tests/stats_ranks_wire.rs serialises byte for
// byte and ranksTypes.vitest.ts deep-compares with a typed copy.
//
// The dataset is opened with Phase 2B's stats_desc_open and closed with
// stats_desc_close, unchanged. The group's levels come from
// stats_ranks_group_levels, the ANOVA's twin, as a GroupLevelsView.
//
// What these hold: level labels and result numbers, for this device's screen only.
// The spec names columns by alias and levels by index; never a label, a row or a path.
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

/** Every refusal, outcome, exclusion, note and limitation of the rank tests
 *  (stats_ranks::RanksCode). */
export type RanksCode =
  | 'stats.ranks.unknown_capability'
  | 'stats.ranks.capability_version_mismatch'
  | 'stats.ranks.input_count'
  | 'stats.ranks.input_mismatch'
  | 'stats.ranks.preparation_mismatch'
  | 'stats.ranks.missing_code_not_finite'
  | 'stats.ranks.unknown_column'
  | 'stats.ranks.score_as_group'
  | 'stats.ranks.code_equals_score'
  | 'stats.ranks.same_column'
  | 'stats.ranks.outcome_non_finite'
  | 'stats.ranks.outcome_holds_text'
  | 'stats.ranks.group_one_level'
  | 'stats.ranks.too_many_levels'
  | 'stats.ranks.level_out_of_range'
  | 'stats.ranks.level_repeated'
  | 'stats.ranks.too_few_levels'
  | 'stats.ranks.group_empty'
  | 'stats.ranks.suspect_missing_code'
  | 'stats.ranks.all_values_tied'
  | 'stats.ranks.distribution_out_of_range'
  | 'stats.ranks.overflow'
  | 'stats.ranks.arithmetic_refused'
  | 'stats.ranks.exact_not_given_ties'
  | 'stats.ranks.exact_not_given_size'
  | 'stats.ranks.group_below_five'
  | 'stats.ranks.missing_group'
  | 'stats.ranks.other_level'
  | 'stats.ranks.missing_outcome'
  | 'stats.ranks.missing_code'
  | 'stats.ranks.no_pairwise_comparisons'
  | 'stats.ranks.two_sided_only'
  | 'stats.ranks.no_location_shift_estimate'
  | 'stats.ranks.exact_without_ties_only'
  | 'stats.ranks.no_further_k_sample_tests'
  | 'stats.ranks.no_weights'
  | 'stats.ranks.code_point_order';

export type RanksParamKey = 'column' | 'count' | 'index' | 'value' | 'limit' | 'expected' | 'found';

/** A finding: a closed code with numeric parameters. */
export interface RanksDiagnostic {
  code: RanksCode;
  params: Partial<Record<RanksParamKey, number>>;
  severity: Severity;
}

export interface RanksValidationView {
  status: ValidationStatus;
  errors: RanksDiagnostic[];
  warnings: RanksDiagnostic[];
  missingInformation: RanksDiagnostic[];
  estimatedResources: ResourceEstimate;
}

/** A run's state, with its progress in steps: the check points done and their
 *  total, one. */
export interface RanksRunStateView {
  runId: number;
  state: RunState;
  terminal: boolean;
  elapsedMs: number;
  cancelRequested: boolean;
  stepsDone: number;
  stepsTotal: number;
  diagnostics: RanksDiagnostic[];
}

/** A number, or the closed reason it is not defined. */
export type RanksStat = { value: number } | { not_computable: RanksCode };

/** One included group, in the levels' code-point order. The label is the user's
 *  own text. */
export interface RankGroup {
  level: number;
  label: string;
  n: number;
  median: number;
  rankSum: number;
  meanRank: number;
}

/** The Mann-Whitney test of two included groups, by their level indexes. */
export interface MannWhitney {
  first: number;
  second: number;
  u1: number;
  u2: number;
  z: RanksStat;
  /** The normal approximation's two-sided p, without a continuity correction. */
  p: RanksStat;
  /** Given only without ties and below 50 values a group; the notes say why not. */
  exactP: number | null;
  rankBiserial: number;
}

/** The Kruskal-Wallis test of three or more included groups. */
export interface KruskalWallis {
  h: RanksStat;
  df: number;
  p: RanksStat;
  epsilonSquared: RanksStat;
}

/** The test the count of included levels chose. */
export type RankTest = { mann_whitney: MannWhitney } | { kruskal_wallis: KruskalWallis };

export interface RanksExclusion {
  code: RanksCode;
  count: number;
  params: Partial<Record<RanksParamKey, number>>;
}

/** stats_ranks::IndependentRanksOutcome. */
export interface IndependentRanksResult {
  groups: RankGroup[];
  n: number;
  tieRuns: number;
  test: RankTest;
  exclusions: RanksExclusion[];
  notes: RanksCode[];
  limitations: RanksCode[];
}

/** Hashes, versions and counts: no header, label, cell, name or path. */
export interface RanksRunManifest {
  schemaVersion: 1;
  capabilityId: string;
  capabilityVersion: number;
  engineVersion: number;
  inputs: string[];
  parametersHash: string;
  seed: number | null;
  rowsRead: number;
  rowsUsed: number;
  exclusions: RanksExclusion[];
  platform: Platform;
  elapsedMs: number;
  outputHash: string;
  reproducibility: Reproducibility;
}

export interface RanksResultView {
  runId: number;
  status: RunState;
  result: IndependentRanksResult;
  diagnostics: RanksDiagnostic[];
  manifest: RanksRunManifest;
}

/* ---- what the screen sends -------------------------------------------- */

/** stats_ranks::IndependentRanksParams: camelCase, deny_unknown_fields. */
export interface RanksParams {
  /** The alias of the outcome, e.g. "C1". */
  outcome: string;
  /** The alias of the group. */
  group: string;
  /** Null when every level is included; otherwise the ticked indexes, ascending. */
  levels: number[] | null;
  /** Ticked candidate missing codes, ascending, once each; the outcome's only. */
  missingCodes: number[];
}

export interface RanksSpec {
  schemaVersion: 1;
  operationId: number;
  workspaceId: 'statistics';
  capabilityId: 'stats.analyze.independent_ranks';
  capabilityVersion: 1;
  inputs: InputRef[];
  parameters: RanksParams;
  randomSeed: null;
  /** The rank tests take no confirmation. */
  confirmationRefs: [];
  /** Decision 490: what made a prepared input. Absent on a file's own table, never null. */
  preparation?: Preparation;
}
