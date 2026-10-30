// The one-sample and paired t-tests' wire contract, from the TypeScript side
// (Research OS, Phase 8D-2b, decision 268). Each JSON fixture is the exact
// serialisation of a Rust view (src-tauri/tests/stats_onesample_wire.rs
// checks the bytes); here each is deep-compared with a copy typed
// `satisfies` the view's type, so tsc catches a renamed or missing key and
// this run catches a value or shape that differs. Also pinned: the code
// union against onesampleCodes.json, both ways; the bridge's command names
// and argument keys for both tests; and the keys of the specs it sends.
import { beforeEach, describe, expect, it, vi } from 'vitest';

import validationJson from './fixtures/onesampleValidationView.json';
import stateJson from './fixtures/onesampleRunStateView.json';
import oneResultJson from './fixtures/onesampleResultView.json';
import pairedResultJson from './fixtures/pairedResultView.json';
import onesampleCodes from './onesampleCodes.json';
import { datasetView } from './fixtures/describeFixtures';
import { makeMockOneSampleBridge, makeMockPairedBridge, TauriOneSampleBridge, TauriPairedBridge } from './onesampleBridge';
import type {
  OneSampleCode,
  OneSampleResultView,
  OneSampleRunStateView,
  OneSampleSpec,
  OneSampleValidationView,
  PairedResultView,
  PairedSpec,
} from './onesampleTypes';

const invoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...a: unknown[]) => invoke(...a) }));
vi.mock('../../utils/isTauri', () => ({ isTauri: true, default: true }));

beforeEach(() => {
  invoke.mockReset();
  invoke.mockResolvedValue({});
});

const warning = { code: 'stats.onesample.suspect_missing_code', params: { column: 1, count: 2, value: 99 }, severity: 'warning' } as const;
const manifest = (capabilityId: string, code: OneSampleCode, column: number) => ({
  schemaVersion: 1 as const,
  capabilityId,
  capabilityVersion: 1,
  engineVersion: 1,
  inputs: ['0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef'],
  parametersHash: 'fedcba9876543210fedcba9876543210fedcba9876543210fedcba9876543210',
  seed: 7,
  rowsRead: 10,
  rowsUsed: 9,
  exclusions: [{ code, count: 1, params: { column } }],
  platform: { os: 'macos' as const, arch: 'aarch64' as const },
  elapsedMs: 3,
  outputHash: '5fd0af5daee4d8dfcb466411096e7f26e513bf2cadfb0e1605ff085026fe5b4a',
  reproducibility: 'bitwise' as const,
});

const typedValidation = {
  status: 'invalid',
  errors: [{ code: 'stats.onesample.too_few_values', params: { count: 1 }, severity: 'error' }],
  warnings: [warning],
  missingInformation: [{ code: 'stats.onesample.test_value_not_chosen', params: {}, severity: 'note' }],
  estimatedResources: { rows: 10, columns: 1, cells: 10 },
} satisfies OneSampleValidationView;

const typedState = {
  runId: 5,
  state: 'running',
  terminal: false,
  elapsedMs: 12,
  cancelRequested: true,
  diagnostics: [warning],
} satisfies OneSampleRunStateView;

const typedOne = {
  runId: 5,
  status: 'succeeded',
  result: {
    n: 9,
    mean: 63.125,
    sd: 29.25,
    testValue: 50.5,
    t: 1.5,
    df: 8,
    p: 0.125,
    meanDifference: 12.625,
    standardError: 9.75,
    confidenceLevel: 0.95,
    ciLower: -9.875,
    ciUpper: 35.125,
    cohensD: 0.4375,
    exclusions: [{ code: 'stats.onesample.missing_value', count: 1, params: { column: 1 } }],
    limitations: ['stats.onesample.values_normality_not_assessed', 'stats.onesample.cohens_d_has_no_interval'],
  },
  diagnostics: [warning],
  manifest: manifest('stats.analyze.one_sample_t', 'stats.onesample.missing_value', 1),
} satisfies OneSampleResultView;

const typedPaired = {
  runId: 6,
  status: 'succeeded',
  result: {
    nPairs: 9,
    first: { mean: 63.125, sd: 29.25 },
    second: { mean: 33.0625, sd: 22.75 },
    t: 2.5,
    df: 8,
    p: 0.0375,
    meanDifference: 30.0625,
    sdDifferences: 36,
    standardError: 12,
    confidenceLevel: 0.95,
    ciLower: 2.375,
    ciUpper: 57.75,
    cohensDz: 0.8125,
    exclusions: [{ code: 'stats.onesample.missing_second', count: 1, params: { column: 2 } }],
    limitations: ['stats.onesample.differences_normality_not_assessed', 'stats.onesample.cohens_d_has_no_interval'],
  },
  diagnostics: [warning],
  manifest: manifest('stats.analyze.paired_t', 'stats.onesample.missing_second', 2),
} satisfies PairedResultView;

const PAIRS: Array<[string, unknown, unknown]> = [
  ['OneSampleValidationView', typedValidation, validationJson],
  ['OneSampleRunStateView', typedState, stateJson],
  ['OneSampleResultView', typedOne, oneResultJson],
  ['PairedResultView', typedPaired, pairedResultJson],
];

/** Every code of the union, once: a missing one or an extra one fails tsc. */
const EVERY_CODE: Record<OneSampleCode, true> = {
  'stats.onesample.unknown_capability': true,
  'stats.onesample.capability_version_mismatch': true,
  'stats.onesample.input_count': true,
  'stats.onesample.input_mismatch': true,
  'stats.onesample.unknown_column': true,
  'stats.onesample.same_column': true,
  'stats.onesample.column_holds_text': true,
  'stats.onesample.column_non_finite': true,
  'stats.onesample.missing_code_not_finite': true,
  'stats.onesample.test_value_not_chosen': true,
  'stats.onesample.test_value_not_finite': true,
  'stats.onesample.suspect_missing_code': true,
  'stats.onesample.too_few_values': true,
  'stats.onesample.too_few_pairs': true,
  'stats.onesample.values_constant': true,
  'stats.onesample.differences_constant': true,
  'stats.onesample.overflow': true,
  'stats.onesample.underflow': true,
  'stats.onesample.arithmetic_refused': true,
  'stats.onesample.quantile_out_of_range': true,
  'stats.onesample.missing_value': true,
  'stats.onesample.missing_first': true,
  'stats.onesample.missing_second': true,
  'stats.onesample.missing_code': true,
  'stats.onesample.values_normality_not_assessed': true,
  'stats.onesample.differences_normality_not_assessed': true,
  'stats.onesample.cohens_d_has_no_interval': true,
  'stats.onesample.values_essentially_constant': true,
  'stats.onesample.differences_essentially_constant': true,
};

const oneSpec: OneSampleSpec = {
  schemaVersion: 1,
  operationId: 1,
  workspaceId: 'statistics',
  capabilityId: 'stats.analyze.one_sample_t',
  capabilityVersion: 1,
  inputs: [datasetView.inputRef],
  parameters: { outcome: 'C1', testValue: 2500, missingCodes: [99] },
  randomSeed: null,
  confirmationRefs: [],
};

const pairedSpec: PairedSpec = {
  schemaVersion: 1,
  operationId: 2,
  workspaceId: 'statistics',
  capabilityId: 'stats.analyze.paired_t',
  capabilityVersion: 1,
  inputs: [datasetView.inputRef],
  parameters: { first: 'C1', second: 'C2', missingCodes: [] },
  randomSeed: null,
  confirmationRefs: [],
};

describe('the one-sample and paired view fixtures', () => {
  it.each(PAIRS)('%s: the typed copy equals the JSON Rust serialises', (_name, t, json) => {
    expect(t).toEqual(json);
  });

  it('the code union is exactly the stats.onesample words of onesampleCodes.json', () => {
    const listed = (onesampleCodes as string[]).filter((c) => c.startsWith('stats.onesample.'));
    expect(listed).toHaveLength(29);
    expect(Object.keys(EVERY_CODE).sort()).toEqual([...listed].sort());
    expect(onesampleCodes).toHaveLength(51);
  });

  it('no manifest key can hold a path, a name, a header or a label', () => {
    for (const json of [oneResultJson, pairedResultJson]) {
      for (const k of Object.keys(json.manifest)) expect(k).not.toMatch(/path|file|name|header|label|dir|url/i);
    }
  });
});

describe('the one-sample and paired bridges', () => {
  it('invoke each command with the argument keys the Rust signature takes', async () => {
    const one = new TauriOneSampleBridge();
    await one.open('/Users/x/data.csv');
    await one.validate(3, oneSpec);
    await one.run(3, oneSpec);
    await one.status(4);
    await one.result(4);
    await one.cancel(4);
    await one.export(4, 'csv');
    await one.close(3);
    const paired = new TauriPairedBridge();
    await paired.validate(3, pairedSpec);
    await paired.run(3, pairedSpec);
    await paired.status(4);
    await paired.result(4);
    await paired.cancel(4);
    await paired.export(4, 'manifestJson');
    const seen = invoke.mock.calls.map(([cmd, args]) => [cmd, Object.keys(args ?? {}).sort()]);
    expect(seen).toEqual([
      // The dataset is opened and closed with Phase 2B's commands.
      // Phase 8F-2 (decision 290): the open carries its progress id.
      ['stats_desc_open', ['path', 'progressId']],
      ['stats_onesample_validate', ['slot', 'spec']],
      ['stats_onesample_run', ['slot', 'spec']],
      ['stats_onesample_status', ['runId']],
      ['stats_onesample_result', ['runId']],
      ['stats_onesample_cancel', ['runId']],
      ['stats_onesample_export', ['format', 'runId']],
      ['stats_desc_close', ['slot']],
      ['stats_paired_validate', ['slot', 'spec']],
      ['stats_paired_run', ['slot', 'spec']],
      ['stats_paired_status', ['runId']],
      ['stats_paired_result', ['runId']],
      ['stats_paired_cancel', ['runId']],
      ['stats_paired_export', ['format', 'runId']],
    ]);
    expect(invoke.mock.calls[6][1].format).toBe('csv');
    expect(invoke.mock.calls[13][1].format).toBe('manifestJson');
  });

  it('send specs with exactly the keys Rust reads, at every level', async () => {
    await new TauriOneSampleBridge().validate(3, oneSpec);
    await new TauriPairedBridge().validate(3, pairedSpec);
    const top = ['capabilityId', 'capabilityVersion', 'confirmationRefs', 'inputs', 'operationId', 'parameters', 'randomSeed', 'schemaVersion', 'workspaceId'];
    const one = invoke.mock.calls[0][1].spec as OneSampleSpec;
    const paired = invoke.mock.calls[1][1].spec as PairedSpec;
    expect(Object.keys(one).sort()).toEqual(top);
    expect(Object.keys(paired).sort()).toEqual(top);
    expect(Object.keys(one.parameters).sort()).toEqual(['missingCodes', 'outcome', 'testValue']);
    expect(Object.keys(paired.parameters).sort()).toEqual(['first', 'missingCodes', 'second']);
  });

  it('the mocks record calls, never a path, and refuse a command they were not given', async () => {
    for (const m of [makeMockOneSampleBridge({ stats_desc_open: datasetView }), makeMockPairedBridge({ stats_desc_open: datasetView })]) {
      await expect(m.open('/Users/secret/PATH_SENTINEL/data.csv')).resolves.toEqual(datasetView);
      expect(JSON.stringify(m.calls)).not.toContain('PATH_SENTINEL');
      expect(m.calls[0]).toEqual(['stats_desc_open', { path: '<path>' }]);
      await expect(m.status(3)).rejects.toThrow('no mock response');
    }
  });
});
