// The confirmatory factor analysis's wire contract, from the TypeScript side (Research OS,
// Phase 9C-3, decision 782), on factorTypes.vitest.ts's pattern. Each JSON fixture is the exact
// serialisation of a Rust view (src-tauri/tests/stats_cfa_wire.rs writes and checks the bytes);
// here each is deep-compared with a copy typed `satisfies` the view's type. Also pinned: the
// four limits the screen reads, the bridge's command names and argument keys, and the spec the
// screen builds, whose parameters follow CfaParams' serde exactly (camelCase,
// deny_unknown_fields, no defaults).
import { beforeEach, describe, expect, it, vi } from 'vitest';

import cfaCodes from './cfaCodes.json';
import type { CfaCode, CfaSpec } from './cfaTypes';

import validationJson from './fixtures/cfaValidationView.json';
import stateJson from './fixtures/cfaRunStateView.json';
import resultJson from './fixtures/cfaResultView.json';
import otherJson from './fixtures/cfaResultViewOther.json';
import limitsJson from './fixtures/cfaLimits.json';
import * as typed from './fixtures/cfaFixtures';
import { datasetView, datasetViewPrepared } from './fixtures/describeFixtures';
import { TauriCfaBridge, makeMockCfaBridge } from './cfaBridge';
import { buildCfaSpec } from './CfaSettings';
import * as words from './cfaWording';

const invoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...a: unknown[]) => invoke(...a) }));
vi.mock('../../utils/isTauri', () => ({ isTauri: true, default: true }));

beforeEach(() => {
  invoke.mockReset();
  invoke.mockResolvedValue({});
});

const PAIRS: Array<[string, unknown, unknown]> = [
  ['CfaValidationView', typed.cfaValidationView, validationJson],
  ['CfaRunStateView', typed.cfaRunStateView, stateJson],
  ['CfaResultView (two correlated factors)', typed.cfaResultView, resultJson],
  ['CfaResultView (one factor, a Heywood case at 0 degrees of freedom)', typed.cfaResultViewOther, otherJson],
];

describe('the confirmatory factor analysis view fixtures', () => {
  it.each(PAIRS)('%s: the typed copy equals the JSON Rust serialises', (_name, t, json) => {
    expect(t).toEqual(json);
  });

  it('carry the progress in steps', () => {
    expect(Object.keys(stateJson)).toContain('stepsDone');
    expect(stateJson.stepsTotal).toBeGreaterThan(1);
  });

  it("give the first result two factors' covariance, and the other one factor with none", () => {
    expect(resultJson.result.indicators).toEqual([
      ['C1', 'C3', 'C5'],
      ['C2', 'C4', 'C6'],
    ]);
    expect(resultJson.result.factorCovariances.map((c) => c.factors)).toEqual([[1, 2]]);
    expect(otherJson.result.indicators).toEqual([['C1', 'C2', 'C3']]);
    expect(otherJson.result.factorCovariances).toEqual([]);
  });

  it('give each a first loading fixed at 1, and the other its notes and its df_zero cells', () => {
    for (const j of [resultJson, otherJson]) {
      const l = j.result.loadings[0];
      expect(l.estimate).toEqual({ value: 1 });
      expect([l.se, l.cr, l.p]).toEqual(Array(3).fill({ not_computable: 'stats.cfa.fixed_parameter' }));
    }
    expect(otherJson.result.notes).toEqual(['stats.cfa.negative_variance', 'stats.cfa.df_zero']);
    expect(otherJson.result.fit.model.df).toBe(0);
    expect(otherJson.result.fit.model.rmsea).toEqual({ not_computable: 'stats.cfa.df_zero' });
  });

  it('give each result the three fit rows of 38 measures', () => {
    for (const j of [resultJson, otherJson]) {
      expect(Object.keys(j.result.fit)).toEqual(['model', 'saturated', 'independence']);
      for (const row of Object.values(j.result.fit)) expect(Object.keys(row)).toHaveLength(38);
    }
  });

  it('share one manifest, so no test treats the Other fixture as a stored result', () => {
    expect(otherJson.manifest).toEqual(resultJson.manifest);
  });

  it('name only codes the type knows', () => {
    const codes = new Set<string>(cfaCodes);
    const known: CfaCode = 'stats.cfa.independence_cmin_zero';
    expect(codes.has(known)).toBe(true);
    for (const c of resultJson.result.limitations) expect(codes.has(c), c).toBe(true);
  });
});

describe("the screen's limits (decision 781)", () => {
  it('are the four the wire writes, read from the fixture and never typed', () => {
    expect(Object.keys(limitsJson).sort()).toEqual(['maxFactors', 'maxItems', 'minFactorItems', 'minItems']);
    expect(typed.cfaLimits).toBe(limitsJson);
    expect([words.MAX_ITEMS, words.MAX_FACTORS, words.MIN_FACTOR_ITEMS, words.MIN_ITEMS]).toEqual([
      limitsJson.maxItems,
      limitsJson.maxFactors,
      limitsJson.minFactorItems,
      limitsJson.minItems,
    ]);
  });

  it('pin MAX_ITERATIONS by value against its Rust twin, stats_cfa::MAX_ITERATIONS', () => {
    expect(words.MAX_ITERATIONS).toBe(500);
  });
});

describe('the spec the screen builds (decision 782)', () => {
  it('names the capability, its version and the view as its one input', () => {
    const s: CfaSpec = buildCfaSpec(datasetView, [['C1', 'C2']], true, [], 7);
    expect(s.capabilityId).toBe('stats.analyze.cfa');
    expect(s.capabilityVersion).toBe(1);
    expect(s.workspaceId).toBe('statistics');
    expect(s.operationId).toBe(7);
    expect(s.inputs).toEqual([datasetView.inputRef]);
    expect(s.randomSeed).toBeNull();
    expect(s.confirmationRefs).toEqual([]);
    expect(Object.keys(s.parameters).sort()).toEqual(['correlatedFactors', 'indicators', 'missingCodes']);
  });

  it("carries the view's preparation exactly when it has one", () => {
    expect(buildCfaSpec(datasetViewPrepared, [['C1', 'C2']], true, [], 1).preparation).toBe(datasetViewPrepared.preparation);
    expect('preparation' in buildCfaSpec(datasetView, [['C1', 'C2']], true, [], 1)).toBe(false);
  });
});

describe('the bridge (decision 782)', () => {
  const spec = buildCfaSpec(datasetView, [['C1', 'C2']], true, [], 1);
  const bridge = new TauriCfaBridge();

  it.each<[string, () => Promise<unknown>, string, Record<string, unknown>]>([
    ['validate', () => bridge.validate(3, spec), 'stats_cfa_validate', { slot: 3, spec }],
    ['run', () => bridge.run(3, spec), 'stats_cfa_run', { slot: 3, spec }],
    ['status', () => bridge.status(5), 'stats_cfa_status', { runId: 5 }],
    ['result', () => bridge.result(5), 'stats_cfa_result', { runId: 5 }],
    ['cancel', () => bridge.cancel(5), 'stats_cfa_cancel', { runId: 5 }],
    ['export', () => bridge.export(5, 'csv'), 'stats_cfa_export', { runId: 5, format: 'csv' }],
    ['close', () => bridge.close(3), 'stats_desc_close', { slot: 3 }],
  ])('%s calls its one command with its argument keys', async (_name, call, command, args) => {
    await call();
    expect(invoke).toHaveBeenCalledTimes(1);
    expect(invoke.mock.calls[0][0]).toBe(command);
    expect(invoke.mock.calls[0][1]).toEqual(args);
  });

  it('has no groupLevels: the method has no levels', () => {
    expect('groupLevels' in bridge).toBe(false);
    expect('groupLevels' in makeMockCfaBridge()).toBe(false);
  });

  it('records every call on its mock, a path as <path>', async () => {
    const mock = makeMockCfaBridge({ stats_cfa_status: stateJson, stats_desc_open: datasetView });
    await mock.status(5);
    await mock.open('/synthetic/file.csv');
    expect(mock.calls).toEqual([
      ['stats_cfa_status', { runId: 5 }],
      ['stats_desc_open', { path: '<path>' }],
    ]);
  });
});
