// Gaply — Statistical Analysis Verifier (premium) wire types. These MIRROR the
// Rust serialization of gaply-core's stats_verdict (Set 3) + stats_chat (Set 4)
// EXACTLY — snake_case tags, serde renames — so the bridge is a pure pass-through.
// The verified numbers are ALWAYS the engine's; nothing here recomputes them.

/** serde rename_all = "snake_case" on TestKind. */
export type TestKind =
  | 't_test_welch'
  | 't_test_student'
  | 'anova'
  | 'pearson'
  | 'spearman'
  | 'chi_square'
  | 'ols';

/** serde enum tag = "layout", rename_all = "snake_case". */
export type ColumnRoles =
  | { layout: 'samples'; columns: string[] }
  | { layout: 'grouped'; value: string; group: string }
  | { layout: 'paired'; x: string; y: string }
  | { layout: 'contingency'; count_columns: string[] }
  | { layout: 'regression'; outcome: string; predictors: string[] };

export interface ReportedStatistic {
  statistic: number | null;
  p_value: number | null;
  coefficient_index: number | null;
}

export interface Tolerance {
  statistic_abs: number;
  statistic_rel: number;
  p_abs: number;
}

export interface AnalysisSpec {
  test_kind: TestKind;
  roles: ColumnRoles;
  reported: ReportedStatistic;
  tolerance: Tolerance;
}

/** The always-shown engine ground truth. */
export interface Recomputed {
  test_kind: TestKind;
  statistic_name: string; // "t" | "F" | "r" | "rho" | "chi_square" | "beta"
  statistic: number;
  p_value: number;
  detail: string;
  rows_dropped: number;
}

export type Verdict = 'match' | 'mismatch';

/** serde CertaintyTier (report.rs) — both match AND mismatch are certain. */
export type CertaintyTier = 'mathematically_certain' | 'ai_assessed_moderate' | 'reconsidered_after_peer_review';

export interface VerifiedResult {
  verdict: Verdict;
  tier: CertaintyTier;
  recomputed: Recomputed;
  reported: ReportedStatistic;
  statistic_delta: number | null;
  p_delta: number | null;
  tolerance: Tolerance;
  explanation: string;
}

export interface AdvisoryNote {
  rule: string; // validate.rs RuleId (snake_case)
  severity: 'CRITICAL' | 'MAJOR';
  location: { section: string; paragraph: number };
  observation: string;
  /** Required, never empty — advice, not a verification. */
  disclaimer: string;
}

/** The Set 3 two-lane verdict report. */
export interface VerificationReport {
  verified: VerifiedResult | null;
  recompute_error: string | null;
  advisory: AdvisoryNote[];
  /** Required, never empty — the lane-distinguishing disclosure. */
  disclosure: string;
}

/** The Set 4 scoped-chat turn (advisory by TYPE — no verified-lane fields). */
export interface StatsChatTurn {
  kind: 'answered' | 'refused_ghostwriting' | 'blocked_ghostwriting' | 'unavailable';
  answer: string;
  refs: string[];
  advisory: boolean;
  disclaimer: string;
  warnings: string[];
  available: boolean;
}

/** run_stats_preview result — columns + optional extraction pre-fill. */
export interface StatsPreview {
  headers: string[];
  row_count: number;
  prefill_p_value: number | null;
}

/** The TestKind catalogue for the builder, with the roles each needs. */
export interface TestKindInfo {
  kind: TestKind;
  label: string;
  /** which ColumnRoles layout this test consumes. */
  layout: ColumnRoles['layout'];
  /** the statistic the user reports (for the label). */
  statistic: string;
}

export const TEST_KINDS: TestKindInfo[] = [
  { kind: 't_test_welch', label: "t-test (Welch's, unequal variances)", layout: 'samples', statistic: 't' },
  { kind: 't_test_student', label: "t-test (Student's, pooled)", layout: 'samples', statistic: 't' },
  { kind: 'anova', label: 'One-way ANOVA', layout: 'samples', statistic: 'F' },
  { kind: 'pearson', label: 'Pearson correlation', layout: 'paired', statistic: 'r' },
  { kind: 'spearman', label: 'Spearman correlation', layout: 'paired', statistic: 'rho' },
  { kind: 'chi_square', label: 'Chi-square (independence)', layout: 'contingency', statistic: 'χ²' },
  { kind: 'ols', label: 'OLS regression', layout: 'regression', statistic: 'β' },
];

export const DEFAULT_TOLERANCE: Tolerance = { statistic_abs: 0.005, statistic_rel: 0.01, p_abs: 0.005 };
