// Gaply — typed copies of the t-test's view fixtures (Research OS, Phase
// 4B-2). Each is written `satisfies` its view's type, so tsc rejects a
// renamed, missing or extra key, and ttestTypes.vitest.ts deep-compares each
// with the JSON fixture beside it, which a Rust test serialises byte for
// byte (src-tauri/tests/stats_ttest_wire.rs). The screens' tests reuse them
// as the bridges' answers.
import type {
  CapabilityView,
  TTestDiagnostic,
  TTestExclusion,
  TTestResultView,
  TTestRunStateView,
  TTestValidationReportView,
} from '../ttestTypes';

const INPUT = '0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef';

const suspectWarning = {
  code: 'stats.ttest.suspect_missing_code',
  params: { column: 1, count: 2, value: 99 },
  severity: 'warning',
} satisfies TTestDiagnostic;

const exclusion = {
  code: 'stats.ttest.missing_outcome',
  count: 1,
  params: { column: 1 },
} satisfies TTestExclusion;

export const ttestValidationReportView = {
  status: 'invalid',
  errors: [{ code: 'stats.ttest.group_too_small', params: { count: 1, index: 1 }, severity: 'error' }],
  warnings: [suspectWarning],
  missingInformation: [{ code: 'stats.ttest.variant_not_chosen', params: {}, severity: 'note' }],
  estimatedResources: { rows: 10, columns: 2, cells: 20 },
} satisfies TTestValidationReportView;

export const ttestRunStateView = {
  runId: 5,
  state: 'running',
  terminal: false,
  elapsedMs: 12,
  cancelRequested: true,
  diagnostics: [suspectWarning],
} satisfies TTestRunStateView;

export const ttestResultView = {
  runId: 5,
  status: 'succeeded',
  result: {
    variant: 'welch',
    t: 1.5,
    df: 7.25,
    p: 0.125,
    meanDifference: 30.5,
    standardError: 16.25,
    confidenceLevel: 0.95,
    ciLower: -8.5,
    ciUpper: 68.75,
    cohensD: 1.125,
    dStandardizer: 'root_mean_variance',
    dStandardizerValue: 26.5,
    groups: [
      { level: 0, label: 'ctrl', n: 5, mean: 63.125, sd: 29.25 },
      { level: 1, label: 'trt', n: 4, mean: 33.0625, sd: 22.75 },
    ],
    sdRatio: { value: 1.25 },
    exclusions: [exclusion],
    limitations: ['stats.ttest.normality_not_assessed', 'stats.ttest.cohens_d_has_no_interval'],
  },
  diagnostics: [suspectWarning],
  manifest: {
    schemaVersion: 1,
    capabilityId: 'stats.analyze.independent_t',
    capabilityVersion: 1,
    engineVersion: 1,
    inputs: [INPUT],
    parametersHash: 'fedcba9876543210fedcba9876543210fedcba9876543210fedcba9876543210',
    seed: 7,
    rowsRead: 10,
    rowsUsed: 9,
    exclusions: [exclusion],
    platform: { os: 'macos', arch: 'aarch64' },
    elapsedMs: 3,
    outputHash: '5fd0af5daee4d8dfcb466411096e7f26e513bf2cadfb0e1605ff085026fe5b4a',
    reproducibility: 'bitwise',
  },
} satisfies TTestResultView;

export const capabilityList = [
  {
    id: 'stats.analyze.independent_t',
    version: 1,
    lifecycle: 'beta',
    titleCode: 'stats.capability.analyze.independent_t.title',
    descriptionCode: 'stats.capability.analyze.independent_t.description',
    licenseNotes: ['stats.capability.license.statrs_mit'],
    knownLimitations: [
      'stats.capability.limit.analyze.levels_follow_level_1',
      'stats.capability.limit.analyze.normality_not_assessed',
      'stats.capability.limit.analyze.cohens_d_has_no_interval',
    ],
    runnable: true,
  },
] satisfies CapabilityView[];
