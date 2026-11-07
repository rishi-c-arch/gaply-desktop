// Gaply — the one-way ANOVA's wire types (Research OS, Phase 8G-2, decision
// 318). TypeScript mirrors of the views src-tauri/src/stats_anova_commands.rs
// returns and of the spec it takes. Every view field is pinned by a JSON
// fixture in ./fixtures (anova*.json), which
// src-tauri/tests/stats_anova_wire.rs serialises byte for byte and
// anovaTypes.vitest.ts deep-compares with a typed copy.
//
// The dataset is opened with Phase 2B's stats_desc_open and closed with
// stats_desc_close, and a group's levels come in the t-test's GroupLevelsView
// shape, so those types are describe's and the t-test's, unchanged.
//
// What these hold: level labels and result numbers, for this device's
// screen only. The spec names columns by alias and levels by index (decision
// 329); never a label, a row or a path.
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

/** Every finding, outcome, exclusion, note and limitation of the ANOVA. */
export type AnovaCode =
  | 'stats.anova.unknown_capability'
  | 'stats.anova.capability_version_mismatch'
  | 'stats.anova.input_count'
  | 'stats.anova.input_mismatch'
  | 'stats.anova.unknown_column'
  | 'stats.anova.same_column'
  | 'stats.anova.outcome_holds_text'
  | 'stats.anova.outcome_non_finite'
  | 'stats.anova.group_one_level'
  | 'stats.anova.too_many_levels'
  | 'stats.anova.level_out_of_range'
  | 'stats.anova.level_repeated'
  | 'stats.anova.too_few_levels'
  | 'stats.anova.missing_code_not_finite'
  | 'stats.anova.suspect_missing_code'
  | 'stats.anova.group_too_small'
  | 'stats.anova.post_hoc_too_many_levels'
  | 'stats.anova.outcome_constant'
  | 'stats.anova.within_variance_zero'
  | 'stats.anova.welch_variance_zero'
  | 'stats.anova.levene_deviations_equal'
  | 'stats.anova.pair_both_constant'
  | 'stats.anova.distribution_out_of_range'
  | 'stats.anova.overflow'
  | 'stats.anova.underflow'
  | 'stats.anova.arithmetic_refused'
  | 'stats.anova.p_below_floor'
  | 'stats.anova.omega_squared_negative'
  | 'stats.anova.missing_group'
  | 'stats.anova.other_level'
  | 'stats.anova.missing_outcome'
  | 'stats.anova.missing_code'
  | 'stats.anova.normality_not_assessed'
  | 'stats.anova.effect_sizes_have_no_interval'
  | 'stats.anova.no_contrasts'
  | 'stats.anova.preparation_mismatch'
  | 'stats.anova.score_as_group'
  | 'stats.anova.code_equals_score';

export type AnovaParamKey = 'column' | 'count' | 'index' | 'value' | 'limit' | 'expected' | 'found';

/** A finding: a closed code with numeric parameters. */
export interface AnovaDiagnostic {
  code: AnovaCode;
  params: Partial<Record<AnovaParamKey, number>>;
  severity: Severity;
}

export interface AnovaValidationView {
  status: ValidationStatus;
  errors: AnovaDiagnostic[];
  warnings: AnovaDiagnostic[];
  missingInformation: AnovaDiagnostic[];
  estimatedResources: ResourceEstimate;
}

/** An ANOVA run's state, with its progress in steps: the check points done
 *  and their total (decision 328), never columns. */
export interface AnovaRunStateView {
  runId: number;
  state: RunState;
  terminal: boolean;
  elapsedMs: number;
  cancelRequested: boolean;
  stepsDone: number;
  stepsTotal: number;
  diagnostics: AnovaDiagnostic[];
}

export type PostHoc = 'tukey' | 'games_howell' | 'bonferroni';

/** Every post hoc test, in the order the method runs them. */
export const POST_HOC_TESTS: readonly PostHoc[] = ['tukey', 'games_howell', 'bonferroni'];

/** A number, or the closed reason it is not defined. */
export type AnovaStat = { value: number } | { not_computable: AnovaCode };

/** A studentized-range or Bonferroni p, or below the floor it is computed
 *  to (the floor's value), never a number of its own. */
export type PostHocP = { value: number } | { below: number };

/** One included group: its level's index and label (the user's own text). */
export interface GroupRow {
  level: number;
  label: string;
  n: number;
  mean: number;
  sd: number;
  se: number;
  ciLower: number;
  ciUpper: number;
  min: number;
  max: number;
}

export interface AnovaTable {
  ssBetween: number;
  ssWithin: number;
  ssTotal: number;
  dfBetween: number;
  dfWithin: number;
  msBetween: number;
  msWithin: number;
  f: AnovaStat;
  p: AnovaStat;
}

export type AnovaTableOutcome = { computed: AnovaTable } | { not_computable: AnovaCode };

export interface FTest {
  f: number;
  df1: number;
  df2: number;
  p: number;
}

export type FOutcome = { computed: FTest } | { not_computable: AnovaCode };

/** One pair's numbers: the first level minus the second. */
export interface PairStats {
  difference: number;
  se: number;
  /** q for Tukey and Games–Howell, t for Bonferroni. */
  statistic: number;
  df: number;
  p: PostHocP;
  ciLower: number;
  ciUpper: number;
}

export type PairOutcome = { computed: PairStats } | { not_computable: AnovaCode };

/** One pair, by the levels' indexes, first before second in the level order. */
export interface Pair {
  first: number;
  second: number;
  outcome: PairOutcome;
}

export type PostHocPairs = { computed: Pair[] } | { not_computable: AnovaCode };

export interface PostHocTable {
  test: PostHoc;
  pairs: PostHocPairs;
}

export interface AnovaExclusion {
  code: AnovaCode;
  count: number;
  params: Partial<Record<AnovaParamKey, number>>;
}

export interface AnovaResult {
  groups: GroupRow[];
  table: AnovaTableOutcome;
  welch: FOutcome;
  leveneMean: FOutcome;
  leveneMedian: FOutcome;
  etaSquared: AnovaStat;
  omegaSquared: AnovaStat;
  /** The chosen tests, in POST_HOC_TESTS' order. */
  postHoc: PostHocTable[];
  exclusions: AnovaExclusion[];
  notes: AnovaCode[];
  limitations: AnovaCode[];
}

/** Hashes, versions and counts: no header, label, cell, name or path. */
export interface AnovaRunManifest {
  schemaVersion: 1;
  capabilityId: string;
  capabilityVersion: number;
  engineVersion: number;
  inputs: string[];
  parametersHash: string;
  seed: number | null;
  rowsRead: number;
  rowsUsed: number;
  exclusions: AnovaExclusion[];
  platform: Platform;
  elapsedMs: number;
  outputHash: string;
  reproducibility: Reproducibility;
}

export interface AnovaResultView {
  runId: number;
  status: RunState;
  result: AnovaResult;
  diagnostics: AnovaDiagnostic[];
  manifest: AnovaRunManifest;
}

/* ---- what the screen sends -------------------------------------------- */

export interface AnovaParams {
  /** The outcome's alias, e.g. "C1". */
  outcome: string;
  /** The group's alias. */
  group: string;
  /** null when every level is ticked; otherwise the ticked indexes,
   *  ascending (decision 329). */
  levels: number[] | null;
  /** The ticked tests, in POST_HOC_TESTS' order; none by default. */
  postHoc: PostHoc[];
  /** Ticked candidate missing codes; the outcome only. */
  missingCodes: number[];
}

export interface AnovaSpec {
  schemaVersion: 1;
  operationId: number;
  workspaceId: 'statistics';
  capabilityId: 'stats.analyze.one_way_anova';
  capabilityVersion: 1;
  inputs: InputRef[];
  parameters: AnovaParams;
  randomSeed: null;
  /** The ANOVA takes no confirmation. */
  confirmationRefs: [];
  /** 8T-1a (decision 490): what made a prepared input. Absent on a file's own table, never null. */
  preparation?: Preparation;
}
