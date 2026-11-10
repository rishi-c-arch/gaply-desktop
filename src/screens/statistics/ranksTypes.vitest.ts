// The rank tests' wire contract, from the TypeScript side (Research OS, Phase 8Y-2,
// decision 619), on xtabTypes.vitest.ts's pattern. Each JSON fixture is the exact
// serialisation of a Rust view (src-tauri/tests/stats_ranks_wire.rs writes and checks
// the bytes); here each is deep-compared with a copy typed `satisfies` the view's type.
// Also pinned: the bridge's command names and argument keys, groupLevels among them,
// and the spec the screen builds, whose parameters follow IndependentRanksParams' serde
// exactly (camelCase, deny_unknown_fields, stats_ranks/mod.rs): outcome, group, levels
// and missingCodes, nothing else.
import { beforeEach, describe, expect, it, vi } from 'vitest';

import ranksCodes from './ranksCodes.json';
import type { RanksCode, RanksSpec } from './ranksTypes';

import validationJson from './fixtures/ranksValidationView.json';
import stateJson from './fixtures/ranksRunStateView.json';
import resultJson from './fixtures/ranksResultView.json';
import otherJson from './fixtures/ranksResultViewOther.json';
import * as typed from './fixtures/ranksFixtures';
import { datasetView, datasetViewPrepared } from './fixtures/describeFixtures';
import { TauriRanksBridge, makeMockRanksBridge } from './ranksBridge';
import { buildRanksSpec } from './RanksSettings';

const invoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...a: unknown[]) => invoke(...a) }));
vi.mock('../../utils/isTauri', () => ({ isTauri: true, default: true }));

beforeEach(() => {
  invoke.mockReset();
  invoke.mockResolvedValue({});
});

const LEVELS = [
  { index: 0, label: 'a', count: 4 },
  { index: 1, label: 'b', count: 5 },
  { index: 2, label: 'c', count: 6 },
];

const PAIRS: Array<[string, unknown, unknown]> = [
  ['RanksValidationView', typed.ranksValidationView, validationJson],
  ['RanksRunStateView', typed.ranksRunStateView, stateJson],
  ['RanksResultView (Mann-Whitney)', typed.ranksResultView, resultJson],
  ['RanksResultView (Kruskal-Wallis)', typed.ranksResultViewOther, otherJson],
];

describe('the rank tests view fixtures', () => {
  it.each(PAIRS)('%s: the typed copy equals the JSON Rust serialises', (_name, t, json) => {
    expect(t).toEqual(json);
  });

  it('carry the progress in steps and one check point', () => {
    expect(Object.keys(stateJson)).toContain('stepsDone');
    expect(stateJson.stepsTotal).toBe(1);
  });

  it('give the Mann-Whitney form a number for exactP, and the Kruskal-Wallis form its not-computable values', () => {
    expect(Object.keys(resultJson.result.test)).toEqual(['mann_whitney']);
    expect(typeof resultJson.result.test.mann_whitney.exactP).toBe('number');
    expect(Object.keys(otherJson.result.test)).toEqual(['kruskal_wallis']);
    expect('not_computable' in otherJson.result.test.kruskal_wallis.h).toBe(true);
  });
});

describe('the spec the screen builds (decision 619)', () => {
  it('names the capability, its version and the view as its one input', () => {
    const s = buildRanksSpec(datasetView, 'C2', 'C1', LEVELS, [0, 1, 2], [], 7);
    expect(s.capabilityId).toBe('stats.analyze.independent_ranks');
    expect(s.capabilityVersion).toBe(1);
    expect(s.workspaceId).toBe('statistics');
    expect(s.operationId).toBe(7);
    expect(s.inputs).toEqual([datasetView.inputRef]);
    expect(s.randomSeed).toBeNull();
    expect(s.confirmationRefs).toEqual([]);
  });

  it("sends exactly IndependentRanksParams' camelCase fields", () => {
    const s = buildRanksSpec(datasetView, 'C2', 'C1', LEVELS, [0, 1, 2], [99], 1);
    expect(Object.keys(s.parameters).sort()).toEqual(['group', 'levels', 'missingCodes', 'outcome']);
    expect(JSON.parse(JSON.stringify(s)).parameters).toEqual({ outcome: 'C2', group: 'C1', levels: null, missingCodes: [99] });
  });

  it('sends levels null when every level is ticked, else the ticked indexes ascending', () => {
    expect(buildRanksSpec(datasetView, 'C2', 'C1', LEVELS, [2, 0, 1], [], 1).parameters.levels).toBeNull();
    expect(buildRanksSpec(datasetView, 'C2', 'C1', LEVELS, [2, 0], [], 1).parameters.levels).toEqual([0, 2]);
  });

  it('sends the codes ascending and once each, so one choice is one spec', () => {
    expect(buildRanksSpec(datasetView, 'C2', 'C1', LEVELS, [0, 1, 2], [99, -9, 99, -99], 1).parameters.missingCodes).toEqual([-99, -9, 99]);
  });

  it("carries the view's preparation exactly when it has one, the key absent otherwise (decision 490)", () => {
    expect(buildRanksSpec(datasetViewPrepared, 'C2', 'C1', LEVELS, [0, 1], [], 1).preparation).toEqual(datasetViewPrepared.preparation);
    expect('preparation' in buildRanksSpec(datasetView, 'C2', 'C1', LEVELS, [0, 1], [], 1)).toBe(false);
  });
});

describe('the bridge', () => {
  it('calls the rank tests commands with the webview argument names, groupLevels among them', async () => {
    const b = new TauriRanksBridge();
    const spec: RanksSpec = buildRanksSpec(datasetView, 'C2', 'C1', LEVELS, [0, 1, 2], [], 1);
    await b.groupLevels(3, 'C1');
    await b.validate(3, spec);
    await b.run(3, spec);
    await b.status(5);
    await b.result(5);
    await b.cancel(5);
    await b.export(5, 'csv');
    await b.close(3);
    expect(invoke.mock.calls.map((c) => [c[0], Object.keys(c[1] as object).sort()])).toEqual([
      ['stats_ranks_group_levels', ['column', 'slot']],
      ['stats_ranks_validate', ['slot', 'spec']],
      ['stats_ranks_run', ['slot', 'spec']],
      ['stats_ranks_status', ['runId']],
      ['stats_ranks_result', ['runId']],
      ['stats_ranks_cancel', ['runId']],
      ['stats_ranks_export', ['format', 'runId']],
      ['stats_desc_close', ['slot']],
    ]);
  });

  it('the mock records the same command names', async () => {
    const m = makeMockRanksBridge({ stats_ranks_status: typed.ranksRunStateView, stats_ranks_group_levels: { slot: 3, column: 'C1', levels: [] } });
    await m.status(5);
    await m.groupLevels(3, 'C1');
    await m.open('/some/where/file.csv').catch(() => undefined);
    expect(m.calls).toEqual([
      ['stats_ranks_status', { runId: 5 }],
      ['stats_ranks_group_levels', { slot: 3, column: 'C1' }],
      ['stats_desc_open', { path: '<path>' }],
    ]);
  });
});

/** Every code of the union, once: a missing one or an extra one fails tsc. */
const EVERY_CODE: Record<RanksCode, true> = {
  'stats.ranks.unknown_capability': true,
  'stats.ranks.capability_version_mismatch': true,
  'stats.ranks.input_count': true,
  'stats.ranks.input_mismatch': true,
  'stats.ranks.preparation_mismatch': true,
  'stats.ranks.missing_code_not_finite': true,
  'stats.ranks.unknown_column': true,
  'stats.ranks.score_as_group': true,
  'stats.ranks.code_equals_score': true,
  'stats.ranks.same_column': true,
  'stats.ranks.outcome_non_finite': true,
  'stats.ranks.outcome_holds_text': true,
  'stats.ranks.group_one_level': true,
  'stats.ranks.too_many_levels': true,
  'stats.ranks.level_out_of_range': true,
  'stats.ranks.level_repeated': true,
  'stats.ranks.too_few_levels': true,
  'stats.ranks.group_empty': true,
  'stats.ranks.suspect_missing_code': true,
  'stats.ranks.all_values_tied': true,
  'stats.ranks.distribution_out_of_range': true,
  'stats.ranks.overflow': true,
  'stats.ranks.arithmetic_refused': true,
  'stats.ranks.exact_not_given_ties': true,
  'stats.ranks.exact_not_given_size': true,
  'stats.ranks.group_below_five': true,
  'stats.ranks.missing_group': true,
  'stats.ranks.other_level': true,
  'stats.ranks.missing_outcome': true,
  'stats.ranks.missing_code': true,
  'stats.ranks.no_pairwise_comparisons': true,
  'stats.ranks.two_sided_only': true,
  'stats.ranks.no_location_shift_estimate': true,
  'stats.ranks.exact_without_ties_only': true,
  'stats.ranks.no_further_k_sample_tests': true,
  'stats.ranks.no_weights': true,
  'stats.ranks.code_point_order': true,
};

describe('the RanksCode union', () => {
  it('is exactly the stats.ranks words of ranksCodes.json', () => {
    const listed = (ranksCodes as string[]).filter((c) => c.startsWith('stats.ranks.'));
    expect(listed).toHaveLength(37);
    expect(Object.keys(EVERY_CODE).sort()).toEqual([...listed].sort());
  });
});
