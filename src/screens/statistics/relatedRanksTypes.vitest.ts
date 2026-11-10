// The related rank tests' wire contract, from the TypeScript side (Research OS, Phase 8Z-2,
// decision 653), on ranksTypes.vitest.ts's pattern. Each JSON fixture is the exact
// serialisation of a Rust view (src-tauri/tests/stats_related_ranks_wire.rs writes and
// checks the bytes); here each is deep-compared with a copy typed `satisfies` the view's
// type. Also pinned: the bridge's command names and argument keys, and the spec the
// screen builds, whose parameters follow RelatedRanksParams' serde exactly (camelCase,
// deny_unknown_fields): columns and missingCodes, nothing else.
import { beforeEach, describe, expect, it, vi } from 'vitest';

import relatedRanksCodes from './relatedRanksCodes.json';
import type { RelatedRanksCode, RelatedRanksSpec } from './relatedRanksTypes';

import validationJson from './fixtures/relatedRanksValidationView.json';
import stateJson from './fixtures/relatedRanksRunStateView.json';
import resultJson from './fixtures/relatedRanksResultView.json';
import otherJson from './fixtures/relatedRanksResultViewOther.json';
import * as typed from './fixtures/relatedRanksFixtures';
import { datasetView, datasetViewPrepared } from './fixtures/describeFixtures';
import { TauriRelatedRanksBridge, makeMockRelatedRanksBridge } from './relatedRanksBridge';
import { buildRelatedRanksSpec } from './RelatedRanksSettings';

const invoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...a: unknown[]) => invoke(...a) }));
vi.mock('../../utils/isTauri', () => ({ isTauri: true, default: true }));

beforeEach(() => {
  invoke.mockReset();
  invoke.mockResolvedValue({});
});

const PAIRS: Array<[string, unknown, unknown]> = [
  ['RelatedRanksValidationView', typed.relatedRanksValidationView, validationJson],
  ['RelatedRanksRunStateView', typed.relatedRanksRunStateView, stateJson],
  ['RelatedRanksResultView (Wilcoxon)', typed.relatedRanksResultView, resultJson],
  ['RelatedRanksResultView (Friedman)', typed.relatedRanksResultViewOther, otherJson],
];

describe('the related rank tests view fixtures', () => {
  it.each(PAIRS)('%s: the typed copy equals the JSON Rust serialises', (_name, t, json) => {
    expect(t).toEqual(json);
  });

  it('carry the progress in steps and one check point', () => {
    expect(Object.keys(stateJson)).toContain('stepsDone');
    expect(stateJson.stepsTotal).toBe(1);
  });

  it('give the Wilcoxon form its exact p and its sides, and the Friedman form its not-computable values', () => {
    expect(Object.keys(resultJson.result.test)).toEqual(['wilcoxon']);
    expect(typeof resultJson.result.test.wilcoxon.exactP).toBe('number');
    expect(resultJson.result.test.wilcoxon.first).toBe('C1');
    expect(Object.keys(otherJson.result.test)).toEqual(['friedman']);
    expect('not_computable' in otherJson.result.test.friedman.q).toBe(true);
    // An exclusion's column is a number; a result's column is its alias.
    expect(resultJson.result.exclusions[0].params.column).toBe(2);
    expect(resultJson.result.columns[0].column).toBe('C1');
  });

  it('name only codes the type knows', () => {
    const codes = new Set<string>(relatedRanksCodes);
    const known: RelatedRanksCode = 'stats.related_ranks.column_order';
    expect(codes.has(known)).toBe(true);
    for (const c of resultJson.result.limitations) expect(codes.has(c), c).toBe(true);
  });
});

describe('the spec the screen builds (decision 653)', () => {
  it('names the capability, its version and the view as its one input', () => {
    const s: RelatedRanksSpec = buildRelatedRanksSpec(datasetView, ['C2', 'C1'], [], 7);
    expect(s.capabilityId).toBe('stats.analyze.related_ranks');
    expect(s.capabilityVersion).toBe(1);
    expect(s.workspaceId).toBe('statistics');
    expect(s.operationId).toBe(7);
    expect(s.inputs).toEqual([datasetView.inputRef]);
    expect(s.randomSeed).toBeNull();
    expect(s.confirmationRefs).toEqual([]);
  });

  it("sends exactly RelatedRanksParams' camelCase fields, the columns in the list's order, never sorted", () => {
    const s = buildRelatedRanksSpec(datasetView, ['C2', 'C1'], [99, -99, 99], 1);
    expect(Object.keys(s.parameters).sort()).toEqual(['columns', 'missingCodes']);
    expect(JSON.parse(JSON.stringify(s)).parameters).toEqual({ columns: ['C2', 'C1'], missingCodes: [-99, 99] });
  });

  it("carries the view's preparation exactly when it has one", () => {
    expect(buildRelatedRanksSpec(datasetViewPrepared, ['C1', 'C2'], [], 1).preparation).toBe(datasetViewPrepared.preparation);
    expect('preparation' in buildRelatedRanksSpec(datasetView, ['C1', 'C2'], [], 1)).toBe(false);
  });
});

describe('the bridge (decision 653)', () => {
  const spec = buildRelatedRanksSpec(datasetView, ['C1', 'C2'], [], 1);
  const bridge = new TauriRelatedRanksBridge();

  it.each<[string, () => Promise<unknown>, string, Record<string, unknown>]>([
    ['validate', () => bridge.validate(3, spec), 'stats_related_ranks_validate', { slot: 3, spec }],
    ['run', () => bridge.run(3, spec), 'stats_related_ranks_run', { slot: 3, spec }],
    ['status', () => bridge.status(5), 'stats_related_ranks_status', { runId: 5 }],
    ['result', () => bridge.result(5), 'stats_related_ranks_result', { runId: 5 }],
    ['cancel', () => bridge.cancel(5), 'stats_related_ranks_cancel', { runId: 5 }],
    ['export', () => bridge.export(5, 'csv'), 'stats_related_ranks_export', { runId: 5, format: 'csv' }],
    ['close', () => bridge.close(3), 'stats_desc_close', { slot: 3 }],
  ])('%s calls its one command with its argument keys', async (_name, call, command, args) => {
    await call();
    expect(invoke).toHaveBeenCalledTimes(1);
    expect(invoke.mock.calls[0][0]).toBe(command);
    expect(invoke.mock.calls[0][1]).toEqual(args);
  });

  it('has no groupLevels: the method has no levels', () => {
    expect('groupLevels' in bridge).toBe(false);
    expect('groupLevels' in makeMockRelatedRanksBridge()).toBe(false);
  });

  it('records every call on its mock, a path as <path>', async () => {
    const mock = makeMockRelatedRanksBridge({ stats_related_ranks_status: stateJson });
    await mock.status(5);
    expect(mock.calls).toEqual([['stats_related_ranks_status', { runId: 5 }]]);
  });
});
