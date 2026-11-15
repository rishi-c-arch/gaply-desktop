// The structural equation model's wire contract, from the TypeScript side (Research OS, Phase
// 9C-5, decision 814), on cfaTypes.vitest.ts's pattern. Each JSON fixture is the exact
// serialisation of a Rust view (src-tauri/tests/stats_sem_wire.rs writes and checks the bytes);
// here each is deep-compared with a copy typed `satisfies` the view's type. Also pinned: the seven
// limits the screen reads, the bridge's command names and argument keys, and the spec the screen
// builds, whose parameters follow SemParams' serde exactly (camelCase, deny_unknown_fields, no
// defaults). The spec's order rules are SemSettings.vitest.tsx's.
import { beforeEach, describe, expect, it, vi } from 'vitest';

import semCodes from './semCodes.json';
import type { SemCode, SemSpec } from './semTypes';

import validationJson from './fixtures/semValidationView.json';
import stateJson from './fixtures/semRunStateView.json';
import resultJson from './fixtures/semResultView.json';
import otherJson from './fixtures/semResultViewOther.json';
import limitsJson from './fixtures/semLimits.json';
import * as typed from './fixtures/semFixtures';
import { datasetView, datasetViewPrepared } from './fixtures/describeFixtures';
import { TauriSemBridge, makeMockSemBridge } from './semBridge';
import { buildSemSpec } from './SemSettings';
import * as words from './semWording';

const invoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...a: unknown[]) => invoke(...a) }));
vi.mock('../../utils/isTauri', () => ({ isTauri: true, default: true }));

beforeEach(() => {
  invoke.mockReset();
  invoke.mockResolvedValue({});
});

const PAIRS: Array<[string, unknown, unknown]> = [
  ['SemValidationView', typed.semValidationView, validationJson],
  ['SemRunStateView', typed.semRunStateView, stateJson],
  ['SemResultView (two factors, a path between them)', typed.semResultView, resultJson],
  ['SemResultView (a loop, two factors, covariances, notes)', typed.semResultViewOther, otherJson],
];

const MODEL = { indicators: [['C1', 'C2']], paths: [], covariances: [], equal: [] };

describe('the structural equation model view fixtures', () => {
  it.each(PAIRS)('%s: the typed copy equals the JSON Rust serialises', (_name, t, json) => {
    expect(t).toEqual(json);
  });

  it('carry the progress in steps', () => {
    expect(Object.keys(stateJson)).toContain('stepsDone');
    expect(stateJson.stepsTotal).toBeGreaterThan(1);
  });

  it('name each variable as a factor or a column, never both', () => {
    for (const j of [resultJson, otherJson]) {
      const vars = [
        ...j.result.weights.flatMap((w) => [w.to, w.from]),
        ...j.result.covariances.flatMap((c) => [c.a, c.b]),
        ...j.result.variances.map((v) => v.variable),
        ...j.result.smc.map((s) => s.variable),
        ...j.result.effects.flatMap((e) => [e.to, e.from]),
      ];
      expect(vars.length).toBeGreaterThan(10);
      for (const v of vars) expect(Object.keys(v).length === 1 && ('factor' in v || 'column' in v)).toBe(true);
    }
  });

  it("give each factor's first loading fixed at 1, a loop's notes, and the attempt", () => {
    for (const j of [resultJson, otherJson]) {
      const first = j.result.weights[0];
      expect(first.kind).toBe('loading');
      expect(first.estimate).toEqual({ value: 1 });
      expect([first.se, first.cr, first.p]).toEqual(Array(3).fill({ not_computable: 'stats.sem.fixed_parameter' }));
      expect([1, 2, 3]).toContain(j.result.attempt);
    }
    expect(otherJson.result.notes).toEqual(['stats.sem.negative_variance', 'stats.sem.nonrecursive']);
    expect(resultJson.result.covariances).toEqual([]);
    expect(otherJson.result.covariances.length).toBe(3);
  });

  it('give each result the three fit rows of 38 measures and the fifteen limitations', () => {
    for (const j of [resultJson, otherJson]) {
      expect(Object.keys(j.result.fit)).toEqual(['model', 'saturated', 'independence']);
      for (const row of Object.values(j.result.fit)) expect(Object.keys(row)).toHaveLength(38);
      expect(j.result.limitations).toHaveLength(15);
    }
  });

  it('share one manifest, so no test treats the Other fixture as a stored result', () => {
    expect(otherJson.manifest).toEqual(resultJson.manifest);
  });

  it('name only codes the type knows: the method has 73', () => {
    const codes = new Set<string>(semCodes);
    const known: SemCode = 'stats.sem.solution_not_identified';
    expect(codes.has(known)).toBe(true);
    expect(semCodes.filter((c) => c.startsWith('stats.sem.'))).toHaveLength(73);
    for (const c of [...resultJson.result.limitations, ...otherJson.result.notes]) expect(codes.has(c), c).toBe(true);
  });
});

describe("the screen's limits (decision 814)", () => {
  it('are the seven the wire writes, read from the fixture and never typed', () => {
    expect(Object.keys(limitsJson).sort()).toEqual(['maxColumns', 'maxCovariances', 'maxEqualMembers', 'maxFactors', 'maxPaths', 'minColumns', 'minFactorItems']);
    expect(typed.semLimits).toBe(limitsJson);
    expect([words.MAX_COLUMNS, words.MAX_COVARIANCES, words.MAX_EQUAL_MEMBERS, words.MAX_FACTORS, words.MAX_PATHS, words.MIN_COLUMNS, words.MIN_FACTOR_ITEMS]).toEqual([
      limitsJson.maxColumns,
      limitsJson.maxCovariances,
      limitsJson.maxEqualMembers,
      limitsJson.maxFactors,
      limitsJson.maxPaths,
      limitsJson.minColumns,
      limitsJson.minFactorItems,
    ]);
  });

  it('pin MAX_ITERATIONS by value against its Rust twin, stats_sem::MAX_ITERATIONS', () => {
    expect(words.MAX_ITERATIONS).toBe(500);
  });
});

describe('the spec the screen builds (decision 814)', () => {
  it('names the capability, its version and the view as its one input', () => {
    const s: SemSpec = buildSemSpec(datasetView, MODEL, [], 7);
    expect(s.capabilityId).toBe('stats.analyze.sem');
    expect(s.capabilityVersion).toBe(1);
    expect(s.workspaceId).toBe('statistics');
    expect(s.operationId).toBe(7);
    expect(s.inputs).toEqual([datasetView.inputRef]);
    expect(s.randomSeed).toBeNull();
    expect(s.confirmationRefs).toEqual([]);
    expect(Object.keys(s.parameters).sort()).toEqual(['covariances', 'equal', 'indicators', 'missingCodes', 'paths']);
  });

  it("carries the view's preparation exactly when it has one", () => {
    expect(buildSemSpec(datasetViewPrepared, MODEL, [], 1).preparation).toBe(datasetViewPrepared.preparation);
    expect('preparation' in buildSemSpec(datasetView, MODEL, [], 1)).toBe(false);
  });
});

describe('the bridge (decision 814)', () => {
  const spec = buildSemSpec(datasetView, MODEL, [], 1);
  const bridge = new TauriSemBridge();

  it.each<[string, () => Promise<unknown>, string, Record<string, unknown>]>([
    ['validate', () => bridge.validate(3, spec), 'stats_sem_validate', { slot: 3, spec }],
    ['run', () => bridge.run(3, spec), 'stats_sem_run', { slot: 3, spec }],
    ['status', () => bridge.status(5), 'stats_sem_status', { runId: 5 }],
    ['result', () => bridge.result(5), 'stats_sem_result', { runId: 5 }],
    ['cancel', () => bridge.cancel(5), 'stats_sem_cancel', { runId: 5 }],
    ['export', () => bridge.export(5, 'csv'), 'stats_sem_export', { runId: 5, format: 'csv' }],
    ['close', () => bridge.close(3), 'stats_desc_close', { slot: 3 }],
  ])('%s calls its one command with its argument keys', async (_name, call, command, args) => {
    await call();
    expect(invoke).toHaveBeenCalledTimes(1);
    expect(invoke.mock.calls[0][0]).toBe(command);
    expect(invoke.mock.calls[0][1]).toEqual(args);
  });

  it('has no groupLevels: the method has no levels', () => {
    expect('groupLevels' in bridge).toBe(false);
    expect('groupLevels' in makeMockSemBridge()).toBe(false);
  });

  it('records every call on its mock, a path as <path>', async () => {
    const mock = makeMockSemBridge({ stats_sem_status: stateJson, stats_desc_open: datasetView });
    await mock.status(5);
    await mock.open('/synthetic/file.csv');
    expect(mock.calls).toEqual([
      ['stats_sem_status', { runId: 5 }],
      ['stats_desc_open', { path: '<path>' }],
    ]);
  });
});
