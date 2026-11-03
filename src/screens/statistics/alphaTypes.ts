// Gaply — Cronbach's alpha's wire types (Research OS, Phase 8I-2, decision
// 381). TypeScript mirrors of the views src-tauri/src/stats_alpha_commands.rs
// returns and of the spec it takes. Every view field is pinned by a JSON
// fixture in ./fixtures (alpha*.json), which src-tauri/tests/stats_alpha_wire.rs
// serialises byte for byte and alphaTypes.vitest.ts deep-compares with a typed
// copy.
//
// The dataset is opened with Phase 2B's stats_desc_open and closed with
// stats_desc_close, so those types are describe's, unchanged.
//
// What these hold: aliases, numbers and closed codes. The result names columns
// by alias (C1, C2, …), never by header; the screen names them from the
// dataset's own profile, a saved result by position. The spec names columns by
// alias, and the scale's ends are numbers the user typed; never a header, a row
// or a path.
import type {
  InputRef,
  Platform,
  Reproducibility,
  ResourceEstimate,
  RunState,
  Severity,
  ValidationStatus,
} from './describeTypes';

/** Every finding, outcome, exclusion, note and limitation of Cronbach's
 *  alpha. */
export type AlphaCode =
  | 'stats.alpha.unknown_capability'
  | 'stats.alpha.capability_version_mismatch'
  | 'stats.alpha.input_count'
  | 'stats.alpha.input_mismatch'
  | 'stats.alpha.unknown_column'
  | 'stats.alpha.too_few_items'
  | 'stats.alpha.item_repeated'
  | 'stats.alpha.items_out_of_order'
  | 'stats.alpha.reversed_repeated'
  | 'stats.alpha.reversed_out_of_order'
  | 'stats.alpha.reversed_not_an_item'
  | 'stats.alpha.scale_without_reversed'
  | 'stats.alpha.scale_missing'
  | 'stats.alpha.scale_not_finite'
  | 'stats.alpha.scale_ends_out_of_order'
  | 'stats.alpha.missing_code_not_finite'
  | 'stats.alpha.missing_code_repeated'
  | 'stats.alpha.missing_codes_out_of_order'
  | 'stats.alpha.item_holds_text'
  | 'stats.alpha.item_non_finite'
  | 'stats.alpha.suspect_missing_code'
  | 'stats.alpha.reversed_outside_scale'
  | 'stats.alpha.too_few_complete_rows'
  | 'stats.alpha.total_constant'
  | 'stats.alpha.item_constant'
  | 'stats.alpha.standardized_not_defined'
  | 'stats.alpha.others_constant'
  | 'stats.alpha.two_items'
  | 'stats.alpha.distribution_out_of_range'
  | 'stats.alpha.overflow'
  | 'stats.alpha.underflow'
  | 'stats.alpha.arithmetic_refused'
  | 'stats.alpha.missing_value'
  | 'stats.alpha.missing_code'
  | 'stats.alpha.negative_alpha'
  | 'stats.alpha.assumes_one_dimension'
  | 'stats.alpha.no_omega'
  | 'stats.alpha.preparation_mismatch'
  | 'stats.alpha.code_equals_score';

export type AlphaParamKey = 'column' | 'count' | 'index' | 'value' | 'expected' | 'found';

/** A finding: a closed code with numeric parameters. */
export interface AlphaDiagnostic {
  code: AlphaCode;
  params: Partial<Record<AlphaParamKey, number>>;
  severity: Severity;
}

export interface AlphaValidationView {
  status: ValidationStatus;
  errors: AlphaDiagnostic[];
  warnings: AlphaDiagnostic[];
  missingInformation: AlphaDiagnostic[];
  estimatedResources: ResourceEstimate;
}

/** An alpha run's state, with its progress in steps: the check points done
 *  and their total (decision 380), never columns. */
export interface AlphaRunStateView {
  runId: number;
  state: RunState;
  terminal: boolean;
  elapsedMs: number;
  cancelRequested: boolean;
  stepsDone: number;
  stepsTotal: number;
  diagnostics: AlphaDiagnostic[];
}

/** A number, or the closed reason it is not defined. */
export type AlphaStat = { value: number } | { not_computable: AlphaCode };

/** Alpha's 95% interval, or why there is none. */
export type AlphaInterval = { computed: { lower: number; upper: number } } | { not_computable: AlphaCode };

/** The inter-item correlations' mean, smallest and largest. */
export interface InterItem {
  mean: AlphaStat;
  min: AlphaStat;
  max: AlphaStat;
}

/** One item's statistics, after reversing. */
export interface ItemStats {
  item: string;
  reversed: boolean;
  mean: AlphaStat;
  sd: AlphaStat;
  scaleMeanIfDeleted: AlphaStat;
  scaleVarianceIfDeleted: AlphaStat;
  correctedItemTotal: AlphaStat;
  alphaIfDeleted: AlphaStat;
}

export interface AlphaExclusion {
  code: AlphaCode;
  count: number;
  params: Partial<Record<AlphaParamKey, number>>;
}

export interface AlphaResult {
  /** The items. */
  k: number;
  /** The rows used, n. */
  rowsUsed: number;
  alpha: AlphaStat;
  interval: AlphaInterval;
  standardized: AlphaStat;
  interItem: InterItem;
  /** In the items' order. */
  items: ItemStats[];
  exclusions: AlphaExclusion[];
  /** negative_alpha when alpha is below 0, otherwise empty. */
  notes: AlphaCode[];
  limitations: AlphaCode[];
}

/** Hashes, versions and counts: no header, cell, name or path. */
export interface AlphaRunManifest {
  schemaVersion: 1;
  capabilityId: string;
  capabilityVersion: number;
  engineVersion: number;
  inputs: string[];
  parametersHash: string;
  seed: number | null;
  rowsRead: number;
  rowsUsed: number;
  exclusions: AlphaExclusion[];
  platform: Platform;
  elapsedMs: number;
  outputHash: string;
  reproducibility: Reproducibility;
}

export interface AlphaResultView {
  runId: number;
  status: RunState;
  result: AlphaResult;
  diagnostics: AlphaDiagnostic[];
  manifest: AlphaRunManifest;
}

/* ---- what the screen sends -------------------------------------------- */

/** The lowest and highest values the scale allows, as the user typed them. */
export interface ScaleEnds {
  min: number;
  max: number;
}

export interface AlphaParams {
  /** The ticked columns' aliases, e.g. "C1", in the dataset's column order;
   *  two or more, none by default. */
  items: string[];
  /** The reverse-scored items, among the items, in the same order; none by
   *  default. */
  reversed: string[];
  /** The scale's ends, only when an item is reverse-scored; otherwise null. */
  scale: ScaleEnds | null;
  /** Ticked candidate missing codes, ascending; they apply to every item. */
  missingCodes: number[];
}

export interface AlphaSpec {
  schemaVersion: 1;
  operationId: number;
  workspaceId: 'statistics';
  capabilityId: 'stats.analyze.cronbach_alpha';
  capabilityVersion: 1;
  inputs: InputRef[];
  parameters: AlphaParams;
  randomSeed: null;
  /** Alpha takes no confirmation. */
  confirmationRefs: [];
}
