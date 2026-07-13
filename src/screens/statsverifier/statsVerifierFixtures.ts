// Gaply — Statistical Analysis Verifier test fixtures. Wire-shaped exactly like
// the Rust serialization of stats_verdict (Set 3) + stats_chat (Set 4). The
// disclosure/disclaimer text mirrors the core constants so the "un-strippable,
// from-a-required-field" assertions are honest.
import { StatsPreview, VerificationReport } from './statsVerifierTypes';

const LANE_DISCLOSURE =
  'VERIFIED results are deterministic recomputations from your data. ADVISORY notes are ' +
  'methodology observations, NOT verifications — consult a statistician.';

const ADVISORY_DISCLAIMER =
  'This is ADVICE about methodology, NOT a verification — a human statistician should judge.';

export const PREVIEW_FIXTURE: StatsPreview = {
  headers: ['A', 'B', 'group'],
  row_count: 10,
  prefill_p_value: 0.03,
};

/** A MISMATCH: user reported t = 2.41; the data gives t ≈ -1.90. */
export const MISMATCH_REPORT: VerificationReport = {
  verified: {
    verdict: 'mismatch',
    tier: 'mathematically_certain',
    recomputed: {
      test_kind: 't_test_student',
      statistic_name: 't',
      statistic: -1.8973665961,
      p_value: 0.094286,
      detail: 'student t-test: df = 8, mean(A) = 3.000000, mean(B) = 6.000000',
      rows_dropped: 0,
    },
    reported: { statistic: 2.41, p_value: null, coefficient_index: null },
    statistic_delta: 2.41 - -1.8973665961,
    p_delta: null,
    tolerance: { statistic_abs: 0.005, statistic_rel: 0.01, p_abs: 0.005 },
    explanation:
      'you reported t = 2.41; recomputing gives t = -1.8974 (Δ = 4.3074, beyond tolerance) → MISMATCH. ' +
      'Tolerance: statistic within ±0.005 (+0.01·|value|), p within ±0.005 — sized for values rounded to ~2 decimals.',
  },
  recompute_error: null,
  advisory: [
    {
      rule: 'missing_effect_size',
      severity: 'MAJOR',
      location: { section: 'Results', paragraph: 0 },
      observation:
        'A p-value is reported without an accompanying effect size (e.g. Cohen’s d, eta-squared, r, odds ratio).',
      disclaimer: ADVISORY_DISCLAIMER,
    },
  ],
  disclosure: LANE_DISCLOSURE,
};

/** A MATCH: reported r = 1.00 matches recomputed r = 1.0. */
export const MATCH_REPORT: VerificationReport = {
  verified: {
    verdict: 'match',
    tier: 'mathematically_certain',
    recomputed: {
      test_kind: 'pearson',
      statistic_name: 'r',
      statistic: 1.0,
      p_value: 0.0,
      detail: 'pearson correlation: df = 3, t = inf',
      rows_dropped: 0,
    },
    reported: { statistic: 1.0, p_value: 0.0, coefficient_index: null },
    statistic_delta: 0,
    p_delta: 0,
    tolerance: { statistic_abs: 0.005, statistic_rel: 0.01, p_abs: 0.005 },
    explanation:
      'you reported r = 1; recomputing gives r = 1.0000 (Δ = 0.0000) → MATCH. Tolerance: statistic within ±0.005.',
  },
  recompute_error: null,
  advisory: [],
  disclosure: LANE_DISCLOSURE,
};

/** An honest recompute ERROR (non-numeric data) — no fake result. */
export const ERROR_REPORT: VerificationReport = {
  verified: null,
  recompute_error:
    "column 'A' has a non-numeric value \"oops\" at row 2 — refusing to guess",
  advisory: [],
  disclosure: LANE_DISCLOSURE,
};
