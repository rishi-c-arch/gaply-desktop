// Gaply — the independent-samples t-test's wire types (Research OS, Phase
// 4B-2). TypeScript mirrors of the views src-tauri/src/stats_ttest_commands.rs
// returns and of the spec it takes. Every view field is pinned by a JSON
// fixture in ./fixtures (ttest*.json, capabilityList.json), which
// src-tauri/tests/stats_ttest_wire.rs serialises byte for byte and
// ttestTypes.vitest.ts deep-compares with a typed copy.
//
// The dataset itself is opened with Phase 2B's stats_desc_open and closed
// with stats_desc_close, so DatasetView, InputRef and the profile types are
// describe's (describeTypes.ts), unchanged.
//
// What these hold: level labels and result numbers, for this device's
// screen only. The spec names columns by alias and levels by index (decision
// 78); never a label, a row or a path.
import type {
  InputRef,
  Platform,
  Reproducibility,
  ResourceEstimate,
  RunState,
  Severity,
  ValidationStatus,
} from './describeTypes';

/** Every finding, outcome, exclusion and limitation of the t-test. */
export type TTestCode =
  | 'stats.ttest.unknown_capability'
  | 'stats.ttest.capability_version_mismatch'
  | 'stats.ttest.input_count'
  | 'stats.ttest.input_mismatch'
  | 'stats.ttest.unknown_column'
  | 'stats.ttest.same_column'
  | 'stats.ttest.outcome_holds_text'
  | 'stats.ttest.outcome_non_finite'
  | 'stats.ttest.group_one_level'
  | 'stats.ttest.level_out_of_range'
  | 'stats.ttest.levels_equal'
  | 'stats.ttest.missing_code_not_finite'
  | 'stats.ttest.variant_not_chosen'
  | 'stats.ttest.levels_not_chosen'
  | 'stats.ttest.suspect_missing_code'
  | 'stats.ttest.group_too_small'
  | 'stats.ttest.both_groups_constant'
  | 'stats.ttest.overflow'
  | 'stats.ttest.underflow'
  | 'stats.ttest.arithmetic_refused'
  | 'stats.ttest.quantile_out_of_range'
  | 'stats.ttest.sd_ratio_undefined'
  | 'stats.ttest.missing_group'
  | 'stats.ttest.other_level'
  | 'stats.ttest.missing_outcome'
  | 'stats.ttest.missing_code'
  | 'stats.ttest.normality_not_assessed'
  | 'stats.ttest.cohens_d_has_no_interval'
  | 'stats.ttest.too_many_levels'
  | 'stats.ttest.groups_essentially_constant';

export type TTestParamKey = 'column' | 'count' | 'index' | 'value' | 'limit' | 'expected' | 'found';

/** A finding: a closed code with numeric parameters. */
export interface TTestDiagnostic {
  code: TTestCode;
  params: Partial<Record<TTestParamKey, number>>;
  severity: Severity;
}

export interface TTestValidationReportView {
  status: ValidationStatus;
  errors: TTestDiagnostic[];
  warnings: TTestDiagnostic[];
  missingInformation: TTestDiagnostic[];
  estimatedResources: ResourceEstimate;
}

/** A t-test run's state: describe's RunStateView without the column
 *  progress, which a t-test does not have. */
export interface TTestRunStateView {
  runId: number;
  state: RunState;
  terminal: boolean;
  elapsedMs: number;
  cancelRequested: boolean;
  diagnostics: TTestDiagnostic[];
}

export type Variant = 'student' | 'welch';
export type Standardizer = 'pooled_sd' | 'root_mean_variance';

/** A number, or the closed reason it is not defined. */
export type TTestStat = { value: number } | { not_computable: TTestCode };

/** One group: its level's index and label (the user's own text), and what
 *  it holds after the exclusions. */
export interface GroupSummary {
  level: number;
  label: string;
  n: number;
  mean: number;
  sd: number;
}

export interface TTestExclusion {
  code: TTestCode;
  count: number;
  params: Partial<Record<TTestParamKey, number>>;
}

export interface TTestResult {
  variant: Variant;
  t: number;
  df: number;
  /** Two-sided. */
  p: number;
  /** Group 1's mean minus group 2's. */
  meanDifference: number;
  standardError: number;
  confidenceLevel: number;
  ciLower: number;
  ciUpper: number;
  cohensD: number;
  dStandardizer: Standardizer;
  dStandardizerValue: number;
  groups: [GroupSummary, GroupSummary];
  /** Information, never a recommendation (decision 64). */
  sdRatio: TTestStat;
  exclusions: TTestExclusion[];
  limitations: TTestCode[];
}

/** Hashes, versions and counts: no header, label, cell, name or path. */
export interface TTestRunManifest {
  schemaVersion: 1;
  capabilityId: string;
  capabilityVersion: number;
  engineVersion: number;
  inputs: string[];
  parametersHash: string;
  seed: number | null;
  rowsRead: number;
  rowsUsed: number;
  exclusions: TTestExclusion[];
  platform: Platform;
  elapsedMs: number;
  outputHash: string;
  reproducibility: Reproducibility;
}

export interface TTestResultView {
  runId: number;
  status: RunState;
  result: TTestResult;
  diagnostics: TTestDiagnostic[];
  manifest: TTestRunManifest;
}

/* ---- a group's levels (decision 81) ------------------------------------ */

/** One level of a group column: its index, which is what a spec's `levels`
 *  names, its label (the user's own text) and its non-missing cells. */
export interface GroupLevel {
  index: number;
  label: string;
  count: number;
}

/** stats_ttest_group_levels: a column's levels as the t-test indexes them,
 *  for any column, a group coded in numbers included. */
export interface GroupLevelsView {
  slot: number;
  column: string;
  levels: GroupLevel[];
}

/* ---- the capability list (the method selector) ------------------------ */

export type Lifecycle = 'planned' | 'experimental' | 'beta' | 'stable' | 'deprecated';

/** One capability as capability_list gives it. `runnable` is Rust's answer
 *  for this build (decision 76); the screen never computes it. */
export interface CapabilityView {
  id: string;
  version: number;
  lifecycle: Lifecycle;
  titleCode: string;
  descriptionCode: string;
  licenseNotes: string[];
  knownLimitations: string[];
  runnable: boolean;
}

/* ---- what the screen sends -------------------------------------------- */

export interface TTestParams {
  /** The outcome's alias, e.g. "C1". */
  outcome: string;
  /** The group's alias. */
  group: string;
  /** The two levels by their position in the profile's list, group 1
   *  first; null only when the group has exactly two (decision 78). */
  levels: [number, number] | null;
  /** Never defaulted: null until the user chooses (decision 64). */
  variant: Variant | null;
  /** Ticked candidate missing codes; the outcome only. */
  missingCodes: number[];
}

export interface TTestSpec {
  schemaVersion: 1;
  operationId: number;
  workspaceId: 'statistics';
  capabilityId: 'stats.analyze.independent_t';
  capabilityVersion: 1;
  inputs: InputRef[];
  parameters: TTestParams;
  randomSeed: null;
  /** The t-test takes no confirmation. */
  confirmationRefs: [];
}
