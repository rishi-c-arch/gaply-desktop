// The t-test's wire contract, from the TypeScript side (Research OS, Phase
// 4B-2). Each JSON fixture is the exact serialisation of a Rust view
// (src-tauri/tests/stats_ttest_wire.rs checks the bytes); here each is
// deep-compared with a copy typed `satisfies` the view's type, so tsc
// catches a renamed or missing key and this run catches a value or shape
// that differs. Also pinned: the bridge's command names and argument keys,
// and the keys of the spec it sends.
import { beforeEach, describe, expect, it, vi } from 'vitest';

import validationJson from './fixtures/ttestValidationReportView.json';
import stateJson from './fixtures/ttestRunStateView.json';
import resultJson from './fixtures/ttestResultView.json';
import capabilityJson from './fixtures/capabilityList.json';
import groupLevelsJson from './fixtures/groupLevelsView.json';
import * as typed from './fixtures/ttestFixtures';
import { datasetView } from './fixtures/describeFixtures';
import { makeMockTTestBridge, TauriCapabilityBridge, TauriTTestBridge } from './ttestBridge';
import type { TTestSpec } from './ttestTypes';

const invoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...a: unknown[]) => invoke(...a) }));
vi.mock('../../utils/isTauri', () => ({ isTauri: true, default: true }));

beforeEach(() => {
  invoke.mockReset();
  invoke.mockResolvedValue({});
});

const PAIRS: Array<[string, unknown, unknown]> = [
  ['TTestValidationReportView', typed.ttestValidationReportView, validationJson],
  ['TTestRunStateView', typed.ttestRunStateView, stateJson],
  ['TTestResultView', typed.ttestResultView, resultJson],
  ['CapabilityView[]', typed.capabilityList, capabilityJson],
  ['GroupLevelsView', typed.groupLevelsView, groupLevelsJson],
];

/** Every key path in a value, arrays read through. */
function keyPaths(v: unknown, at = ''): string[] {
  if (Array.isArray(v)) return v.flatMap((x) => keyPaths(x, `${at}[]`));
  if (v && typeof v === 'object') {
    return Object.entries(v as Record<string, unknown>).flatMap(([k, x]) => [`${at}.${k}`, ...keyPaths(x, `${at}.${k}`)]);
  }
  return [];
}
const keys = (v: unknown) => Array.from(new Set(keyPaths(v))).sort();

const spec: TTestSpec = {
  schemaVersion: 1,
  operationId: 1,
  workspaceId: 'statistics',
  capabilityId: 'stats.analyze.independent_t',
  capabilityVersion: 1,
  inputs: [datasetView.inputRef],
  parameters: { outcome: 'C2', group: 'C1', levels: [0, 1], variant: 'welch', missingCodes: [99] },
  randomSeed: null,
  confirmationRefs: [],
};

describe('the t-test view fixtures', () => {
  it.each(PAIRS)('%s: the typed copy equals the JSON Rust serialises', (_name, t, json) => {
    expect(t).toEqual(json);
  });

  it('fill every optional field and hold no empty array', () => {
    for (const [name, , json] of PAIRS) {
      const walk = (v: unknown, at: string) => {
        expect(v, `${name}${at} is null`).not.toBeNull();
        if (Array.isArray(v)) {
          expect(v.length, `${name}${at} is empty`).toBeGreaterThan(0);
          v.forEach((x, i) => walk(x, `${at}[${i}]`));
        } else if (v && typeof v === 'object') {
          for (const [k, x] of Object.entries(v as Record<string, unknown>)) if (k !== 'params') walk(x, `${at}.${k}`);
        }
      };
      walk(json, '');
    }
  });

  it('pin every key of every view', () => {
    const diag = (at: string) => [`${at}`, `${at}[].code`, `${at}[].params`, `${at}[].severity`];
    expect(keys(validationJson)).toEqual(
      [
        ...diag('.errors'),
        '.errors[].params.count',
        '.errors[].params.index',
        '.estimatedResources',
        '.estimatedResources.cells',
        '.estimatedResources.columns',
        '.estimatedResources.rows',
        ...diag('.missingInformation'),
        '.status',
        ...diag('.warnings'),
        '.warnings[].params.column',
        '.warnings[].params.count',
        '.warnings[].params.value',
      ].sort()
    );
    expect(keys(stateJson)).toEqual(
      [
        '.cancelRequested',
        ...diag('.diagnostics'),
        '.diagnostics[].params.column',
        '.diagnostics[].params.count',
        '.diagnostics[].params.value',
        '.elapsedMs',
        '.runId',
        '.state',
        '.terminal',
      ].sort()
    );
    // No column progress: a t-test has none (decision 74's view).
    expect(keys(stateJson).some((k) => k.startsWith('.columns'))).toBe(false);
    const excl = (at: string) => [at, `${at}[].code`, `${at}[].count`, `${at}[].params`, `${at}[].params.column`];
    expect(keys(resultJson)).toEqual(
      [
        ...diag('.diagnostics'),
        '.diagnostics[].params.column',
        '.diagnostics[].params.count',
        '.diagnostics[].params.value',
        '.manifest',
        '.manifest.capabilityId',
        '.manifest.capabilityVersion',
        '.manifest.elapsedMs',
        '.manifest.engineVersion',
        ...excl('.manifest.exclusions'),
        '.manifest.inputs',
        '.manifest.outputHash',
        '.manifest.parametersHash',
        '.manifest.platform',
        '.manifest.platform.arch',
        '.manifest.platform.os',
        '.manifest.reproducibility',
        '.manifest.rowsRead',
        '.manifest.rowsUsed',
        '.manifest.schemaVersion',
        '.manifest.seed',
        '.result',
        '.result.ciLower',
        '.result.ciUpper',
        '.result.cohensD',
        '.result.confidenceLevel',
        '.result.dStandardizer',
        '.result.dStandardizerValue',
        '.result.df',
        ...excl('.result.exclusions'),
        '.result.groups',
        '.result.groups[].label',
        '.result.groups[].level',
        '.result.groups[].mean',
        '.result.groups[].n',
        '.result.groups[].sd',
        '.result.limitations',
        '.result.meanDifference',
        '.result.p',
        '.result.sdRatio',
        '.result.sdRatio.value',
        '.result.standardError',
        '.result.t',
        '.result.variant',
        '.runId',
        '.status',
      ].sort()
    );
    expect(keys(capabilityJson)).toEqual(
      [
        '[].descriptionCode',
        '[].id',
        '[].knownLimitations',
        '[].licenseNotes',
        '[].lifecycle',
        '[].runnable',
        '[].titleCode',
        '[].version',
      ].sort()
    );
  });

  it('pins the group-levels view (decision 81)', () => {
    expect(keys(groupLevelsJson)).toEqual(['.column', '.levels', '.levels[].count', '.levels[].index', '.levels[].label', '.slot']);
  });

  it('no manifest key can hold a path, a name, a header or a label', () => {
    for (const k of Object.keys(resultJson.manifest)) expect(k).not.toMatch(/path|file|name|header|label|dir|url/i);
  });
});

describe('TauriTTestBridge', () => {
  it('invokes each command with the argument keys the Rust signature takes', async () => {
    const b = new TauriTTestBridge();
    await b.open('/Users/x/data.csv');
    await b.groupLevels(3, 'C1');
    await b.validate(3, spec);
    await b.run(3, spec);
    await b.status(4);
    await b.result(4);
    await b.cancel(4);
    await b.export(4, 'csv');
    await b.export(4, 'manifestJson');
    await b.close(3);
    await new TauriCapabilityBridge().list();

    const seen = invoke.mock.calls.map(([cmd, args]) => [cmd, Object.keys(args ?? {}).sort()]);
    expect(seen).toEqual([
      // The dataset is opened and closed with Phase 2B's commands (decision 74).
      ['stats_desc_open', ['path']],
      ['stats_ttest_group_levels', ['column', 'slot']],
      ['stats_ttest_validate', ['slot', 'spec']],
      ['stats_ttest_run', ['slot', 'spec']],
      ['stats_ttest_status', ['runId']],
      ['stats_ttest_result', ['runId']],
      ['stats_ttest_cancel', ['runId']],
      ['stats_ttest_export', ['format', 'runId']],
      ['stats_ttest_export', ['format', 'runId']],
      ['stats_desc_close', ['slot']],
      ['capability_list', []],
    ]);
    expect(invoke.mock.calls[7][1].format).toBe('csv');
    expect(invoke.mock.calls[8][1].format).toBe('manifestJson');
  });

  it('sends a spec with exactly the keys Rust reads, at every level', async () => {
    await new TauriTTestBridge().validate(3, spec);
    const sent = invoke.mock.calls[0][1].spec as TTestSpec;
    expect(Object.keys(sent).sort()).toEqual([
      'capabilityId', 'capabilityVersion', 'confirmationRefs', 'inputs', 'operationId', 'parameters', 'randomSeed', 'schemaVersion',
      'workspaceId',
    ]);
    expect(Object.keys(sent.parameters).sort()).toEqual(['group', 'levels', 'missingCodes', 'outcome', 'variant']);
  });
});

describe('makeMockTTestBridge', () => {
  it('records calls, never a path, and refuses a command it was not given', async () => {
    const m = makeMockTTestBridge({ stats_desc_open: datasetView });
    await expect(m.open('/Users/secret/PATH_SENTINEL/data.csv')).resolves.toEqual(datasetView);
    expect(JSON.stringify(m.calls)).not.toContain('PATH_SENTINEL');
    expect(m.calls[0]).toEqual(['stats_desc_open', { path: '<path>' }]);
    await expect(m.status(3)).rejects.toThrow('no mock response');
  });
});
