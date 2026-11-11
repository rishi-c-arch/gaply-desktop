// The linear regression's wire contract, from the TypeScript side (Research OS, Phase 9A-2,
// decision 684), on relatedRanksTypes.vitest.ts's pattern. Each JSON fixture is the exact
// serialisation of a Rust view (src-tauri/tests/stats_linear_regression_wire.rs writes and
// checks the bytes); here each is deep-compared with a copy typed `satisfies` the view's
// type. Also pinned: the bridge's command names and argument keys, and the spec the screen
// builds, whose parameters follow LinearRegressionParams' serde exactly (camelCase,
// deny_unknown_fields): outcome, predictors and missingCodes, nothing else.
import { beforeEach, describe, expect, it, vi } from 'vitest';

import linearRegressionCodes from './linearRegressionCodes.json';
import type { LinearRegressionCode, LinearRegressionSpec } from './linearRegressionTypes';

import validationJson from './fixtures/linearRegressionValidationView.json';
import stateJson from './fixtures/linearRegressionRunStateView.json';
import resultJson from './fixtures/linearRegressionResultView.json';
import otherJson from './fixtures/linearRegressionResultViewOther.json';
import * as typed from './fixtures/linearRegressionFixtures';
import { datasetView, datasetViewPrepared } from './fixtures/describeFixtures';
import { TauriLinearRegressionBridge, makeMockLinearRegressionBridge } from './linearRegressionBridge';
import { buildLinearRegressionSpec } from './LinearRegressionSettings';

const invoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...a: unknown[]) => invoke(...a) }));
vi.mock('../../utils/isTauri', () => ({ isTauri: true, default: true }));

beforeEach(() => {
  invoke.mockReset();
  invoke.mockResolvedValue({});
});

const PAIRS: Array<[string, unknown, unknown]> = [
  ['LinearRegressionValidationView', typed.linearRegressionValidationView, validationJson],
  ['LinearRegressionRunStateView', typed.linearRegressionRunStateView, stateJson],
  ['LinearRegressionResultView (three predictors)', typed.linearRegressionResultView, resultJson],
  ['LinearRegressionResultView (a perfect fit)', typed.linearRegressionResultViewOther, otherJson],
];

describe('the linear regression view fixtures', () => {
  it.each(PAIRS)('%s: the typed copy equals the JSON Rust serialises', (_name, t, json) => {
    expect(t).toEqual(json);
  });

  it('carry the progress in steps and one check point', () => {
    expect(Object.keys(stateJson)).toContain('stepsDone');
    expect(stateJson.stepsTotal).toBe(1);
  });

  it('give the columns as the outcome then the predictors, by alias, and an exclusion its column by number', () => {
    expect(resultJson.result.outcome).toBe('C1');
    expect(resultJson.result.columns.map((c) => c.column)).toEqual(['C1', ...resultJson.result.predictors.map((p) => p.column)]);
    expect(resultJson.result.exclusions[0].params.column).toBe(2);
    expect(Object.keys(resultJson.result.anova.total).sort()).toEqual(['df', 'sumOfSquares']);
    expect(Object.keys(resultJson.result.residuals.standardizedPredicted).sort()).toEqual(['maximum', 'minimum']);
  });

  it("give the perfect fit its not-computable values and its note, beta a number", () => {
    const p = otherJson.result.predictors[0];
    expect('not_computable' in p.standardError).toBe(true);
    expect('value' in p.beta).toBe(true);
    expect(otherJson.result.notes).toEqual(['stats.linear_regression.essentially_perfect_fit']);
  });

  it('share one manifest, so no test treats the Other fixture as a stored result', () => {
    expect(otherJson.manifest).toEqual(resultJson.manifest);
  });

  it('name only codes the type knows', () => {
    const codes = new Set<string>(linearRegressionCodes);
    const known: LinearRegressionCode = 'stats.linear_regression.predicted_no_spread';
    expect(codes.has(known)).toBe(true);
    for (const c of resultJson.result.limitations) expect(codes.has(c), c).toBe(true);
  });
});

describe('the spec the screen builds (decision 684)', () => {
  it('names the capability, its version and the view as its one input', () => {
    const s: LinearRegressionSpec = buildLinearRegressionSpec(datasetView, 'C2', ['C1'], [], 7);
    expect(s.capabilityId).toBe('stats.analyze.linear_regression');
    expect(s.capabilityVersion).toBe(1);
    expect(s.workspaceId).toBe('statistics');
    expect(s.operationId).toBe(7);
    expect(s.inputs).toEqual([datasetView.inputRef]);
    expect(s.randomSeed).toBeNull();
    expect(s.confirmationRefs).toEqual([]);
  });

  it("sends exactly LinearRegressionParams' camelCase fields, the predictors in the list's order, never sorted", () => {
    const s = buildLinearRegressionSpec(datasetView, 'C1', ['C4', 'C2', 'C3'], [99, -99, 99], 1);
    expect(Object.keys(s.parameters).sort()).toEqual(['missingCodes', 'outcome', 'predictors']);
    expect(JSON.parse(JSON.stringify(s)).parameters).toEqual({ outcome: 'C1', predictors: ['C4', 'C2', 'C3'], missingCodes: [-99, 99] });
  });

  it("carries the view's preparation exactly when it has one", () => {
    expect(buildLinearRegressionSpec(datasetViewPrepared, 'C1', ['C2'], [], 1).preparation).toBe(datasetViewPrepared.preparation);
    expect('preparation' in buildLinearRegressionSpec(datasetView, 'C1', ['C2'], [], 1)).toBe(false);
  });
});

describe('the bridge (decision 684)', () => {
  const spec = buildLinearRegressionSpec(datasetView, 'C1', ['C2'], [], 1);
  const bridge = new TauriLinearRegressionBridge();

  it.each<[string, () => Promise<unknown>, string, Record<string, unknown>]>([
    ['validate', () => bridge.validate(3, spec), 'stats_linear_regression_validate', { slot: 3, spec }],
    ['run', () => bridge.run(3, spec), 'stats_linear_regression_run', { slot: 3, spec }],
    ['status', () => bridge.status(5), 'stats_linear_regression_status', { runId: 5 }],
    ['result', () => bridge.result(5), 'stats_linear_regression_result', { runId: 5 }],
    ['cancel', () => bridge.cancel(5), 'stats_linear_regression_cancel', { runId: 5 }],
    ['export', () => bridge.export(5, 'csv'), 'stats_linear_regression_export', { runId: 5, format: 'csv' }],
    ['close', () => bridge.close(3), 'stats_desc_close', { slot: 3 }],
  ])('%s calls its one command with its argument keys', async (_name, call, command, args) => {
    await call();
    expect(invoke).toHaveBeenCalledTimes(1);
    expect(invoke.mock.calls[0][0]).toBe(command);
    expect(invoke.mock.calls[0][1]).toEqual(args);
  });

  it('has no groupLevels: the method has no levels', () => {
    expect('groupLevels' in bridge).toBe(false);
    expect('groupLevels' in makeMockLinearRegressionBridge()).toBe(false);
  });

  it('records every call on its mock, a path as <path>', async () => {
    const mock = makeMockLinearRegressionBridge({ stats_linear_regression_status: stateJson, stats_desc_open: datasetView });
    await mock.status(5);
    await mock.open('/synthetic/file.csv');
    expect(mock.calls).toEqual([
      ['stats_linear_regression_status', { runId: 5 }],
      ['stats_desc_open', { path: '<path>' }],
    ]);
  });
});
