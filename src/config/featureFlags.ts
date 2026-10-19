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
  | 'plagiarismCheck'
  // Level 1 of the premium Statistical Analysis Verifier, "Check a paper
  // against its data" (Phase 2). Default OFF; behind the page's own gate too.
  | 'statsL1'
  // The Research OS descriptive summary (Phase 2B): its route and hub tile.
  // Default OFF. Hides unfinished UI; not an authorization boundary.
  | 'statsDescribe'
  // The Research OS workspace chooser and shell (Phase 3). Default OFF. Hides
  // unfinished UI; not an authorization boundary (Rust checks the active
  // workspace on every stats_desc_* command whatever this says).
  | 'workspaces'
  // The Research OS project screens (Phase 5B-3a, decisions 119 and 136):
  // Save, the Projects panel, the delete dialog and the Settings sentence.
  // Default OFF, and shown only with 'workspaces' on too and only in the
  // desktop app (projects/projectGate.ts). Hides unfinished UI; not an
  // authorization boundary (Rust checks every project_* command).
  | 'projects';

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
  statsL1: envBool(process.env.REACT_APP_FEATURE_STATS_L1),
  statsDescribe: envBool(process.env.REACT_APP_FEATURE_STATS_DESCRIBE),
  workspaces: envBool(process.env.REACT_APP_FEATURE_WORKSPACES),
  projects: envBool(process.env.REACT_APP_FEATURE_PROJECTS),
};

export function isFeatureEnabled(name: FeatureName): boolean {
  return FEATURE_FLAGS[name] === true;
}
