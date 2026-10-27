// Gaply — the one-sample and paired-samples t-tests' wire types (Research
// OS, Phase 8D-2b, decision 268). TypeScript mirrors of the views
// src-tauri/src/stats_onesample_commands.rs returns and of the specs it
// takes. The two tests share their codes, so they share the validation and
// run-state views; each has its own result view. Every view field is pinned
// by a JSON fixture in ./fixtures (onesample*.json, pairedResultView.json),
// which src-tauri/tests/stats_onesample_wire.rs serialises byte for byte and
// onesampleTypes.vitest.ts deep-compares with a typed copy.
//
// The dataset is opened with Phase 2B's stats_desc_open and closed with
// stats_desc_close, so DatasetView and InputRef are describe's.
//
// What these hold: result numbers and the user's own test value, for this
// device's screen only. A spec names columns by alias; never a header, a
// row or a path.
import type {
  InputRef,
  Platform,
  Reproducibility,
  ResourceEstimate,
  RunState,
  Severity,
  ValidationStatus,
} from './describeTypes';

/** Every finding, outcome, exclusion and limitation of the two tests, in
 *  Rust's order (OneSampleCode). onesampleTypes.vitest.ts pins it to
 *  onesampleCodes.json both ways. */
export type OneSampleCode =
  | 'stats.onesample.unknown_capability'
  | 'stats.onesample.capability_version_mismatch'
  | 'stats.onesample.input_count'
  | 'stats.onesample.input_mismatch'
  | 'stats.onesample.unknown_column'
  | 'stats.onesample.same_column'
  | 'stats.onesample.column_holds_text'
  | 'stats.onesample.column_non_finite'
  | 'stats.onesample.missing_code_not_finite'
  | 'stats.onesample.test_value_not_chosen'
  | 'stats.onesample.test_value_not_finite'
  | 'stats.onesample.suspect_missing_code'
  | 'stats.onesample.too_few_values'
  | 'stats.onesample.too_few_pairs'
  | 'stats.onesample.values_constant'
  | 'stats.onesample.differences_constant'
  | 'stats.onesample.overflow'
  | 'stats.onesample.underflow'
  | 'stats.onesample.arithmetic_refused'
  | 'stats.onesample.quantile_out_of_range'
  | 'stats.onesample.missing_value'
  | 'stats.onesample.missing_first'
  | 'stats.onesample.missing_second'
  | 'stats.onesample.missing_code'
  | 'stats.onesample.values_normality_not_assessed'
  | 'stats.onesample.differences_normality_not_assessed'
  | 'stats.onesample.cohens_d_has_no_interval'
  | 'stats.onesample.values_essentially_constant'
  | 'stats.onesample.differences_essentially_constant';

export type OneSampleParamKey = 'column' | 'count' | 'index' | 'value' | 'limit' | 'expected' | 'found';

/** A finding: a closed code with numeric parameters. */
export interface OneSampleDiagnostic {
  code: OneSampleCode;
  params: Partial<Record<OneSampleParamKey, number>>;
  severity: Severity;
}

export interface OneSampleValidationView {
  status: ValidationStatus;
  errors: OneSampleDiagnostic[];
  warnings: OneSampleDiagnostic[];
  missingInformation: OneSampleDiagnostic[];
  estimatedResources: ResourceEstimate;
}

/** Either test's run state: no column progress. */
export interface OneSampleRunStateView {
  runId: number;
  state: RunState;
  terminal: boolean;
  elapsedMs: number;
  cancelRequested: boolean;
  diagnostics: OneSampleDiagnostic[];
}

export interface OneSampleExclusion {
  code: OneSampleCode;
  count: number;
  params: Partial<Record<OneSampleParamKey, number>>;
}

export interface OneSampleResult {
  /** The values used. */
  n: number;
  mean: number;
  sd: number;
  /** The value the user gave, as the test used it. */
  testValue: number;
  t: number;
  df: number;
  /** Two-sided. */
  p: number;
  /** The mean minus the test value. */
  meanDifference: number;
  standardError: number;
  confidenceLevel: number;
  ciLower: number;
  ciUpper: number;
  cohensD: number;
  exclusions: OneSampleExclusion[];
  limitations: OneSampleCode[];
}

/** One column of a paired result. */
export interface ColumnStats {
  mean: number;
  sd: number;
}

export interface PairedResult {
  /** The pairs used. */
  nPairs: number;
  first: ColumnStats;
  second: ColumnStats;
  t: number;
  df: number;
  /** Two-sided. */
  p: number;
  /** The mean of first minus second. */
  meanDifference: number;
  sdDifferences: number;
  standardError: number;
  confidenceLevel: number;
  ciLower: number;
  ciUpper: number;
  cohensDz: number;
  exclusions: OneSampleExclusion[];
  limitations: OneSampleCode[];
}

/** Hashes, versions and counts: no header, cell, name or path. */
export interface OneSampleRunManifest {
  schemaVersion: 1;
  capabilityId: string;
  capabilityVersion: number;
  engineVersion: number;
  inputs: string[];
  parametersHash: string;
  seed: number | null;
  rowsRead: number;
  rowsUsed: number;
  exclusions: OneSampleExclusion[];
  platform: Platform;
  elapsedMs: number;
  outputHash: string;
  reproducibility: Reproducibility;
}

export interface OneSampleResultView {
  runId: number;
  status: RunState;
  result: OneSampleResult;
  diagnostics: OneSampleDiagnostic[];
  manifest: OneSampleRunManifest;
}

export interface PairedResultView {
  runId: number;
  status: RunState;
  result: PairedResult;
  diagnostics: OneSampleDiagnostic[];
  manifest: OneSampleRunManifest;
}

/* ---- what the screens send -------------------------------------------- */

export interface OneSampleParams {
  /** The column's alias, e.g. "C1". */
  outcome: string;
  /** Never filled in by Gaply: null until the user gives a finite number,
   *  which Rust answers with test_value_not_chosen (decision 268). */
  testValue: number | null;
  /** Ticked candidate missing codes. */
  missingCodes: number[];
}

export interface PairedParams {
  /** The first column's alias; the difference is first minus second. */
  first: string;
  second: string;
  missingCodes: number[];
}

interface SpecOf<C extends string, P> {
  schemaVersion: 1;
  operationId: number;
  workspaceId: 'statistics';
  capabilityId: C;
  capabilityVersion: 1;
  inputs: InputRef[];
  parameters: P;
  randomSeed: null;
  /** Neither test takes a confirmation. */
  confirmationRefs: [];
}

export type OneSampleSpec = SpecOf<'stats.analyze.one_sample_t', OneSampleParams>;
export type PairedSpec = SpecOf<'stats.analyze.paired_t', PairedParams>;
