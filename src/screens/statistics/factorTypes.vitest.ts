// The factor analysis's wire contract, from the TypeScript side (Research OS, Phase 9B-3,
// decision 729), on linearRegressionTypes.vitest.ts's pattern. Each JSON fixture is the exact
// serialisation of a Rust view (src-tauri/tests/stats_factor_wire.rs writes and checks the
// bytes); here each is deep-compared with a copy typed `satisfies` the view's type. Also
// pinned: the bridge's command names and argument keys, and the spec the screen builds, whose
// parameters follow FactorParams' serde exactly (camelCase, deny_unknown_fields, no defaults).
import { beforeEach, describe, expect, it, vi } from 'vitest';

import factorCodes from './factorCodes.json';
import type { FactorCode, FactorSpec } from './factorTypes';

import validationJson from './fixtures/factorValidationView.json';
import stateJson from './fixtures/factorRunStateView.json';
import resultJson from './fixtures/factorResultView.json';
import otherJson from './fixtures/factorResultViewOther.json';
import * as typed from './fixtures/factorFixtures';
import { datasetView, datasetViewPrepared } from './fixtures/describeFixtures';
import { TauriFactorBridge, makeMockFactorBridge } from './factorBridge';
import { buildFactorSpec } from './FactorSettings';

const invoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...a: unknown[]) => invoke(...a) }));
vi.mock('../../utils/isTauri', () => ({ isTauri: true, default: true }));

beforeEach(() => {
  invoke.mockReset();
  invoke.mockResolvedValue({});
});

const PAIRS: Array<[string, unknown, unknown]> = [
  ['FactorValidationView', typed.factorValidationView, validationJson],
  ['FactorRunStateView', typed.factorRunStateView, stateJson],
  ['FactorResultView (principal axis, promax)', typed.factorResultView, resultJson],
  ['FactorResultView (principal components, varimax, singular)', typed.factorResultViewOther, otherJson],
];

describe('the factor analysis view fixtures', () => {
  it.each(PAIRS)('%s: the typed copy equals the JSON Rust serialises', (_name, t, json) => {
    expect(t).toEqual(json);
  });

  it('carry the progress in steps', () => {
    expect(Object.keys(stateJson)).toContain('stepsDone');
    expect(stateJson.stepsTotal).toBeGreaterThan(1);
  });

  it("give promax its structure and factor correlations, and varimax its transformation and sums, the other fields null", () => {
    const p = resultJson.result.rotation;
    expect(p.method).toBe('promax');
    expect([p.transformation, p.sums]).toEqual([null, null]);
    expect(p.structure).toHaveLength(6);
    expect(p.factorCorrelations).toHaveLength(2);
    const v = otherJson.result.rotation;
    expect(v.method).toBe('varimax');
    expect([v.structure, v.factorCorrelations]).toEqual([null, null]);
    expect(v.transformation).toHaveLength(2);
    expect(v.sums).toHaveLength(2);
  });

  it('give the singular fixture its not-computable values and its note', () => {
    expect(otherJson.result.determinant.value).toEqual({ not_computable: 'stats.factor.correlations_singular' });
    expect(otherJson.result.notes).toEqual(['stats.factor.eigenvalues_not_distinct']);
  });

  it('share one manifest, so no test treats the Other fixture as a stored result', () => {
    expect(otherJson.manifest).toEqual(resultJson.manifest);
  });

  it('name only codes the type knows', () => {
    const codes = new Set<string>(factorCodes);
    const known: FactorCode = 'stats.factor.rotation_singular';
    expect(codes.has(known)).toBe(true);
    for (const c of resultJson.result.limitations) expect(codes.has(c), c).toBe(true);
  });
});

describe('the spec the screen builds (decision 729)', () => {
  it('names the capability, its version and the view as its one input', () => {
    const s: FactorSpec = buildFactorSpec(datasetView, ['C1', 'C2', 'C3'], 'principal_components', null, 'none', 25, 0.001, [], 7);
    expect(s.capabilityId).toBe('stats.analyze.factor');
    expect(s.capabilityVersion).toBe(1);
    expect(s.workspaceId).toBe('statistics');
    expect(s.operationId).toBe(7);
    expect(s.inputs).toEqual([datasetView.inputRef]);
    expect(s.randomSeed).toBeNull();
    expect(s.confirmationRefs).toEqual([]);
    expect(JSON.parse(JSON.stringify(s)).parameters.factors).toBeNull();
  });

  it("carries the view's preparation exactly when it has one", () => {
    expect(buildFactorSpec(datasetViewPrepared, ['C1'], 'principal_components', null, 'none', 25, 0.001, [], 1).preparation).toBe(datasetViewPrepared.preparation);
    expect('preparation' in buildFactorSpec(datasetView, ['C1'], 'principal_components', null, 'none', 25, 0.001, [], 1)).toBe(false);
  });
});

describe('the bridge (decision 729)', () => {
  const spec = buildFactorSpec(datasetView, ['C1', 'C2', 'C3'], 'principal_components', null, 'none', 25, 0.001, [], 1);
  const bridge = new TauriFactorBridge();

  it.each<[string, () => Promise<unknown>, string, Record<string, unknown>]>([
    ['validate', () => bridge.validate(3, spec), 'stats_factor_validate', { slot: 3, spec }],
    ['run', () => bridge.run(3, spec), 'stats_factor_run', { slot: 3, spec }],
    ['status', () => bridge.status(5), 'stats_factor_status', { runId: 5 }],
    ['result', () => bridge.result(5), 'stats_factor_result', { runId: 5 }],
    ['cancel', () => bridge.cancel(5), 'stats_factor_cancel', { runId: 5 }],
    ['export', () => bridge.export(5, 'csv'), 'stats_factor_export', { runId: 5, format: 'csv' }],
    ['close', () => bridge.close(3), 'stats_desc_close', { slot: 3 }],
  ])('%s calls its one command with its argument keys', async (_name, call, command, args) => {
    await call();
    expect(invoke).toHaveBeenCalledTimes(1);
    expect(invoke.mock.calls[0][0]).toBe(command);
    expect(invoke.mock.calls[0][1]).toEqual(args);
  });

  it('has no groupLevels: the method has no levels', () => {
    expect('groupLevels' in bridge).toBe(false);
    expect('groupLevels' in makeMockFactorBridge()).toBe(false);
  });

  it('records every call on its mock, a path as <path>', async () => {
    const mock = makeMockFactorBridge({ stats_factor_status: stateJson, stats_desc_open: datasetView });
    await mock.status(5);
    await mock.open('/synthetic/file.csv');
    expect(mock.calls).toEqual([
      ['stats_factor_status', { runId: 5 }],
      ['stats_desc_open', { path: '<path>' }],
    ]);
  });
});
