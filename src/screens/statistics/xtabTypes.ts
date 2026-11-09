// Gaply — crosstabs' wire types (Research OS, Phase 8X-2, decision 588).
// TypeScript mirrors of the views src-tauri/src/stats_xtab_commands.rs returns
// and of the spec it takes. Every view field is pinned by a JSON fixture in
// ./fixtures (xtab*View.json), which src-tauri/tests/stats_xtab_wire.rs
// serialises byte for byte and xtabTypes.vitest.ts deep-compares with a typed
// copy.
//
// The dataset is opened with Phase 2B's stats_desc_open and closed with
// stats_desc_close, unchanged. There is no levels command: the table takes every
// level of both columns (decision 575).
//
// What these hold: level labels and result numbers, for this device's screen
// only. The spec names columns by alias (decision 575); never a label, a row or
// a path.
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

/** Every refusal, outcome, exclusion, note and limitation of crosstabs
 *  (stats_xtab::XtabCode). */
export type XtabCode =
  | 'stats.xtab.unknown_capability'
  | 'stats.xtab.capability_version_mismatch'
  | 'stats.xtab.input_count'
  | 'stats.xtab.input_mismatch'
  | 'stats.xtab.preparation_mismatch'
  | 'stats.xtab.missing_code_not_finite'
  | 'stats.xtab.missing_code_repeated'
  | 'stats.xtab.missing_codes_out_of_order'
  | 'stats.xtab.unknown_column'
  | 'stats.xtab.same_column'
  | 'stats.xtab.score_as_column'
  | 'stats.xtab.too_few_levels'
  | 'stats.xtab.too_many_levels'
  | 'stats.xtab.suspect_missing_code'
  | 'stats.xtab.overflow'
  | 'stats.xtab.arithmetic_refused'
  | 'stats.xtab.distribution_out_of_range'
  | 'stats.xtab.expected_below_five'
  | 'stats.xtab.missing_row'
  | 'stats.xtab.missing_column'
  | 'stats.xtab.missing_code'
  | 'stats.xtab.no_layers'
  | 'stats.xtab.fisher_two_by_two_only'
  | 'stats.xtab.no_further_nominal_measures'
  | 'stats.xtab.no_weights'
  | 'stats.xtab.code_point_order';

export type XtabParamKey = 'column' | 'count' | 'index' | 'value' | 'limit' | 'expected' | 'found';

/** A finding: a closed code with numeric parameters. */
export interface XtabDiagnostic {
  code: XtabCode;
  params: Partial<Record<XtabParamKey, number>>;
  severity: Severity;
}

export interface XtabValidationView {
  status: ValidationStatus;
  errors: XtabDiagnostic[];
  warnings: XtabDiagnostic[];
  missingInformation: XtabDiagnostic[];
  estimatedResources: ResourceEstimate;
}

/** A crosstabs run's state, with its progress in steps: the check points done
 *  and their total, one. */
export interface XtabRunStateView {
  runId: number;
  state: RunState;
  terminal: boolean;
  elapsedMs: number;
  cancelRequested: boolean;
  stepsDone: number;
  stepsTotal: number;
  diagnostics: XtabDiagnostic[];
}

/** A number, or the closed reason it is not defined. */
export type XtabStat = { value: number } | { not_computable: XtabCode };

/** One level of a column, by its index in the table (code-point order among the
 *  included rows). The label is the user's own text. */
export interface LevelLabel {
  level: number;
  label: string;
}

/** One cell: its row level and column level by index, and its numbers. */
export interface Frequency {
  row: number;
  column: number;
  observed: number;
  expected: number;
  rowProportion: number;
  columnProportion: number;
  totalProportion: number;
  residual: number;
  pearsonResidual: number;
  adjustedResidual: number;
}

/** A chi-square statistic, its df and its upper-tail p. */
export interface ChiTest {
  statistic: number;
  df: number;
  p: number;
}

export type ChiOutcome = { computed: ChiTest } | { not_computable: XtabCode };

/** Fisher's exact test's three p values. */
export interface FisherTest {
  twoSided: number;
  less: number;
  greater: number;
}

export interface XtabExclusion {
  code: XtabCode;
  count: number;
  params: Partial<Record<XtabParamKey, number>>;
}

/** stats_xtab::CrosstabsOutcome. */
export interface CrosstabsResult {
  rowLevels: LevelLabel[];
  columnLevels: LevelLabel[];
  /** Every cell, row by row; found by (row, column), never by position. */
  frequencies: Frequency[];
  rowTotals: number[];
  columnTotals: number[];
  n: number;
  pearson: ChiOutcome;
  likelihoodRatio: ChiOutcome;
  /** For a 2 x 2 table only. */
  continuityCorrected: ChiOutcome | null;
  /** For a 2 x 2 table only. */
  fisher: FisherTest | null;
  phi: XtabStat;
  cramersV: XtabStat;
  contingencyCoefficient: XtabStat;
  expectedBelowFive: number;
  cellCount: number;
  smallestExpected: number;
  exclusions: XtabExclusion[];
  notes: XtabCode[];
  limitations: XtabCode[];
}

/** Hashes, versions and counts: no header, label, cell, name or path. */
export interface XtabRunManifest {
  schemaVersion: 1;
  capabilityId: string;
  capabilityVersion: number;
  engineVersion: number;
  inputs: string[];
  parametersHash: string;
  seed: number | null;
  rowsRead: number;
  rowsUsed: number;
  exclusions: XtabExclusion[];
  platform: Platform;
  elapsedMs: number;
  outputHash: string;
  reproducibility: Reproducibility;
}

export interface XtabResultView {
  runId: number;
  status: RunState;
  result: CrosstabsResult;
  diagnostics: XtabDiagnostic[];
  manifest: XtabRunManifest;
}

/* ---- what the screen sends -------------------------------------------- */

/** stats_xtab::CrosstabsParams: camelCase, deny_unknown_fields. */
export interface XtabParams {
  /** The alias of the column whose levels are the table's rows, e.g. "C1". */
  rows: string;
  /** The alias of the column whose levels are its columns. */
  columns: string;
  /** Ticked candidate missing codes, ascending, once each; they apply by value
   *  to both columns. */
  missingCodes: number[];
}

export interface XtabSpec {
  schemaVersion: 1;
  operationId: number;
  workspaceId: 'statistics';
  capabilityId: 'stats.analyze.crosstabs';
  capabilityVersion: 1;
  inputs: InputRef[];
  parameters: XtabParams;
  randomSeed: null;
  /** Crosstabs takes no confirmation. */
  confirmationRefs: [];
  /** Decision 490: what made a prepared input. Absent on a file's own table, never null. */
  preparation?: Preparation;
}
