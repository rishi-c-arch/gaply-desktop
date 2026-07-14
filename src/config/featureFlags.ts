// Gaply — feature flags. CRA build-time booleans (process.env.REACT_APP_*),
// typed and defaulting to OFF. A flag being off must never fabricate a result:
// gated features render an honest "coming soon" / disabled state instead.
//
// Set in .env(.local) or the hosting env, e.g. REACT_APP_FEATURE_PAYMENTS=true.

export type FeatureName =
  | 'deepPlagiarism'
  | 'payments'
  | 'orcid'
  | 'fullAnalysis'
  // Free-check visibility gates (default OFF — reversibly hide the UI without
  // touching the shared backend). statsCheck OFF hides the free Statistical
  // Analysis Check entirely (route + nav); plagiarismCheck OFF shows the
  // Plagiarism Check as an honest coming-soon (run disabled, library kept).
  | 'statsCheck'
  | 'plagiarismCheck';

/** Parse a CRA env string as a boolean (default false). Only 'true'/'1' enable. */
function envBool(v: string | undefined): boolean {
  return v === 'true' || v === '1';
}

/** The resolved flag map — evaluated once at module load (env is build-time). */
export const FEATURE_FLAGS: Record<FeatureName, boolean> = {
  deepPlagiarism: envBool(process.env.REACT_APP_FEATURE_DEEP_PLAGIARISM),
  payments: envBool(process.env.REACT_APP_FEATURE_PAYMENTS),
  orcid: envBool(process.env.REACT_APP_FEATURE_ORCID),
  fullAnalysis: envBool(process.env.REACT_APP_FEATURE_FULL_ANALYSIS),
  statsCheck: envBool(process.env.REACT_APP_FEATURE_STATS_CHECK),
  plagiarismCheck: envBool(process.env.REACT_APP_FEATURE_PLAGIARISM_CHECK),
};

export function isFeatureEnabled(name: FeatureName): boolean {
  return FEATURE_FLAGS[name] === true;
}
