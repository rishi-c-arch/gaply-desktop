// Gaply — typed copies of the descriptive summary's view fixtures (Research
// OS, Phase 2B). Each is written `satisfies` its view's type, so tsc rejects
// a renamed, missing or extra key, and describeTypes.vitest.ts deep-compares
// each with the JSON fixture beside it, which a Rust test serialises byte
// for byte (src-tauri/tests/stats_desc_wire.rs). The screen's tests reuse
// them as the bridge's answers.
import type {
  DatasetView,
  Diagnostic,
  ExportView,
  ResultBundleView,
  RunStartedView,
  RunStateView,
  ValidationReportView,
} from '../describeTypes';

const INPUT = '0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef';

const suspectWarning = {
  code: 'stats.desc.suspect_missing_code',
  params: { column: 2, count: 2, value: 99 },
  severity: 'warning',
} satisfies Diagnostic;

export const datasetView = {
  slot: 3,
  displayName: 'survey.csv',
  profile: {
    rows: 9,
    columns: [
      {
        alias: 'C1',
        header: 'group',
        kind: 'categorical',
        n: 9,
        missing: 0,
        distinct: 2,
        levels: [
          { label: 'a', count: 5 },
          { label: 'b', count: 4 },
        ],
        suspectMissingCodes: [{ code: -99, cells: 1 }],
        nonFinite: 0,
      },
      {
        alias: 'C2',
        header: 'dose',
        kind: 'numeric',
        n: 8,
        missing: 1,
        distinct: 3,
        levels: [{ label: '1', count: 4 }],
        suspectMissingCodes: [{ code: 99, cells: 2 }],
        nonFinite: 1,
      },
    ],
  },
  inputRef: { contentHash: INPUT, rows: 9, columns: 2, semanticRole: 'table' },
} satisfies DatasetView;

export const validationReportView = {
  status: 'invalid',
  errors: [{ code: 'stats.desc.unknown_column', params: { column: 9 }, severity: 'error' }],
  warnings: [suspectWarning],
  missingInformation: [{ code: 'stats.desc.no_columns_selected', params: {}, severity: 'note' }],
  estimatedResources: { rows: 9, columns: 2, cells: 18 },
} satisfies ValidationReportView;

export const runStartedView = { runId: 4 } satisfies RunStartedView;

export const runStateView = {
  runId: 4,
  state: 'running',
  terminal: false,
  columnsDone: 1,
  columnsTotal: 2,
  elapsedMs: 12,
  cancelRequested: true,
  diagnostics: [suspectWarning],
} satisfies RunStateView;

export const resultBundleView = {
  runId: 4,
  status: 'succeeded',
  columns: [
    {
      numeric: {
        alias: 'C2',
        header: 'dose',
        n: 8,
        missing: 1,
        mean: { value: 52.5 },
        sd: { not_computable: 'stats.desc.overflow' },
        min: { value: 10.5 },
        q1: { value: 31 },
        median: { value: 50 },
        q3: { value: 70.75 },
        max: { value: 95.5 },
      },
    },
    {
      categorical: {
        alias: 'C1',
        header: 'group',
        n: 9,
        missing: 0,
        levels: [
          { label: 'a', count: 5 },
          { label: 'b', count: 4 },
        ],
      },
    },
  ],
  diagnostics: [suspectWarning],
  manifest: {
    schemaVersion: 1,
    capabilityId: 'stats.describe',
    capabilityVersion: 1,
    engineVersion: 1,
    inputs: [INPUT],
    parametersHash: 'fedcba9876543210fedcba9876543210fedcba9876543210fedcba9876543210',
    seed: 7,
    rowsRead: 9,
    rowsUsed: 9,
    exclusions: [{ code: 'stats.desc.missing_cell', count: 1, params: { column: 2 } }],
    platform: { os: 'macos', arch: 'aarch64' },
    elapsedMs: 3,
    outputHash: '5fd0af5daee4d8dfcb466411096e7f26e513bf2cadfb0e1605ff085026fe5b4a',
    reproducibility: 'bitwise',
  },
} satisfies ResultBundleView;

export const exportView = {
  bytes: [34, 97, 34, 13, 10],
  sha256: '8914567fcbb12f42f9bb0ba7d69cc1957f97c8410310bdf025351a3ebdf16d0c',
} satisfies ExportView;
