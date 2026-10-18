// Gaply — the descriptive summary's wire types (Research OS, Phase 2B).
// TypeScript mirrors of the views src-tauri/src/stats_desc_commands.rs
// returns and of the spec it takes. Every view field is pinned by a JSON
// fixture in ./fixtures, which a Rust test serialises byte for byte and
// describeTypes.vitest.ts deep-compares with a typed copy, so a renamed key
// fails tsc and a changed value fails the comparison.
//
// What these hold: headers, level labels, the file's base name and summary
// numbers, for this device's screen only. Never a row, never a path.

/** Every finding, outcome and exclusion of the descriptive summary. */
export type DescCode =
  | 'stats.desc.unknown_capability'
  | 'stats.desc.capability_version_mismatch'
  | 'stats.desc.input_count'
  | 'stats.desc.input_mismatch'
  | 'stats.desc.no_columns_selected'
  | 'stats.desc.unknown_column'
  | 'stats.desc.duplicate_column'
  | 'stats.desc.numeric_on_text'
  | 'stats.desc.non_finite_cell'
  | 'stats.desc.missing_code_not_finite'
  | 'stats.desc.suspect_missing_code'
  | 'stats.desc.no_values'
  | 'stats.desc.single_value'
  | 'stats.desc.overflow'
  | 'stats.desc.underflow'
  | 'stats.desc.arithmetic_refused'
  | 'stats.desc.missing_cell'
  | 'stats.desc.missing_code';

/** Why the run manager refused, or how a run ended without a result. */
export type RunCode =
  | 'run.unknown_run'
  | 'run.run_busy'
  | 'run.unknown_input'
  | 'run.not_finished'
  | 'run.no_result'
  | 'run.cancelled'
  | 'run.timed_out'
  | 'run.resource_limit'
  | 'run.illegal_transition'
  | 'run.already_terminal'
  | 'run.wrong_kind';

export type RunState =
  | 'draft'
  | 'validating'
  | 'needs_user_input'
  | 'awaiting_confirmation'
  | 'refused'
  | 'validated'
  | 'invalidated'
  | 'queued'
  | 'running'
  | 'succeeded'
  | 'failed'
  | 'cancelled'
  | 'timed_out'
  | 'resource_limit'
  | 'numerical_failure';

export type ValidationStatus = 'valid' | 'valid_with_warnings' | 'needs_user_input' | 'unsupported' | 'invalid';
export type Severity = 'error' | 'warning' | 'note';
export type ParamKey = 'column' | 'count' | 'index' | 'value' | 'limit' | 'expected' | 'found' | 'from' | 'event';

/** A finding: a closed code with numeric (or closed-word) parameters. */
export interface Diagnostic {
  code: DescCode;
  params: Partial<Record<ParamKey, number | string>>;
  severity: Severity;
}

export type ColumnKind = 'numeric' | 'categorical' | 'empty';
export type TreatAs = 'numeric' | 'categorical';

export interface LevelCount {
  label: string;
  count: number;
}

/** A candidate missing code Level 1's rule found. Reported, never applied. */
export interface SuspectCode {
  code: number;
  cells: number;
}

export interface ColumnProfile {
  alias: string;
  header: string;
  kind: ColumnKind;
  n: number;
  missing: number;
  distinct: number;
  /** A categorical column's levels; null for a numeric or empty column. */
  levels: LevelCount[] | null;
  suspectMissingCodes: SuspectCode[];
  nonFinite: number;
}

export interface DatasetProfile {
  rows: number;
  columns: ColumnProfile[];
}

export interface InputRef {
  contentHash: string;
  rows: number;
  columns: number;
  semanticRole: 'table';
}

export interface DatasetView {
  slot: number;
  /** The file's base name, never a path. */
  displayName: string;
  profile: DatasetProfile;
  inputRef: InputRef;
}

export interface ResourceEstimate {
  rows: number;
  columns: number;
  cells: number;
}

export interface ValidationReportView {
  status: ValidationStatus;
  errors: Diagnostic[];
  warnings: Diagnostic[];
  missingInformation: Diagnostic[];
  estimatedResources: ResourceEstimate;
}

export interface RunStartedView {
  runId: number;
}

export interface RunStateView {
  runId: number;
  state: RunState;
  terminal: boolean;
  columnsDone: number;
  columnsTotal: number;
  elapsedMs: number;
  cancelRequested: boolean;
  diagnostics: Diagnostic[];
}

/** A statistic, or the closed reason it could not be computed. */
export type Stat = { value: number } | { not_computable: DescCode };

export interface NumericSummary {
  alias: string;
  header: string;
  n: number;
  missing: number;
  mean: Stat;
  sd: Stat;
  min: Stat;
  q1: Stat;
  median: Stat;
  q3: Stat;
  max: Stat;
}

export interface CategoricalSummary {
  alias: string;
  header: string;
  n: number;
  missing: number;
  levels: LevelCount[];
}

export type ColumnResult = { numeric: NumericSummary } | { categorical: CategoricalSummary };

export interface Exclusion {
  code: DescCode;
  count: number;
  params: Partial<Record<ParamKey, number | string>>;
}

export interface Platform {
  os: 'macos' | 'windows' | 'linux' | 'other';
  arch: 'aarch64' | 'x86_64' | 'other';
}

export type Reproducibility = 'bitwise' | 'not_reproducible' | { within_tolerance: { relative: number } };

/** Hashes, versions and counts: no header, label, cell, name or path. */
export interface RunManifest {
  schemaVersion: 1;
  capabilityId: string;
  capabilityVersion: number;
  engineVersion: number;
  inputs: string[];
  parametersHash: string;
  seed: number | null;
  rowsRead: number;
  rowsUsed: number;
  exclusions: Exclusion[];
  platform: Platform;
  elapsedMs: number;
  outputHash: string;
  reproducibility: Reproducibility;
}

export interface ResultBundleView {
  runId: number;
  status: RunState;
  columns: ColumnResult[];
  diagnostics: Diagnostic[];
  manifest: RunManifest;
}

/** An export's bytes and their SHA-256. Rust writes nothing. */
export interface ExportView {
  bytes: number[];
  sha256: string;
}

export type ExportFormat = 'csv' | 'manifestJson';

/* ---- what the screen sends -------------------------------------------- */

export interface ColumnRequest {
  alias: string;
  treatAs: TreatAs;
}

export interface DescribeParams {
  columns: ColumnRequest[];
  missingCodes: number[];
}

/** The typed operation: validated by Rust before any run. */
export interface OperationSpec {
  schemaVersion: 1;
  operationId: number;
  workspaceId: 'statistics';
  capabilityId: 'stats.describe';
  capabilityVersion: 1;
  inputs: InputRef[];
  parameters: DescribeParams;
  randomSeed: number | null;
  /** The confirmations a run needs (Research OS, Phase 4A, decision 50).
   *  The descriptive summary needs none, so it is always empty. */
  confirmationRefs: [];
}

/** A command's refusal, parsed from its message: a closed code and its
 *  numeric parameters (an ingest refusal carries Level 1's own code). */
export interface Refusal {
  code: string;
  [param: string]: unknown;
}
