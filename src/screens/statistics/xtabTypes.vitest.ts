// Crosstabs' wire contract, from the TypeScript side (Research OS, Phase 8X-2,
// decision 588). Each JSON fixture is the exact serialisation of a Rust view
// (src-tauri/tests/stats_xtab_wire.rs writes and checks the bytes); here each is
// deep-compared with a copy typed `satisfies` the view's type. Also pinned: the
// bridge's command names and argument keys (no levels command: the table takes
// every level, decision 575), and the spec the screen builds, whose parameters
// follow CrosstabsParams' serde exactly (camelCase, deny_unknown_fields,
// stats_xtab/mod.rs:174-180): rows, columns and missingCodes, nothing else.
import { beforeEach, describe, expect, it, vi } from 'vitest';

import xtabCodes from './xtabCodes.json';
import type { XtabCode, XtabSpec } from './xtabTypes';

import validationJson from './fixtures/xtabValidationView.json';
import stateJson from './fixtures/xtabRunStateView.json';
import resultJson from './fixtures/xtabResultView.json';
import otherJson from './fixtures/xtabResultViewOther.json';
import * as typed from './fixtures/xtabFixtures';
import { datasetView, datasetViewPrepared } from './fixtures/describeFixtures';
import { TauriXtabBridge, makeMockXtabBridge } from './xtabBridge';
import { buildXtabSpec } from './XtabSettings';

const invoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...a: unknown[]) => invoke(...a) }));
vi.mock('../../utils/isTauri', () => ({ isTauri: true, default: true }));

beforeEach(() => {
  invoke.mockReset();
  invoke.mockResolvedValue({});
});

const PAIRS: Array<[string, unknown, unknown]> = [
  ['XtabValidationView', typed.xtabValidationView, validationJson],
  ['XtabRunStateView', typed.xtabRunStateView, stateJson],
  ['XtabResultView', typed.xtabResultView, resultJson],
  ['XtabResultView (the other variants)', typed.xtabResultViewOther, otherJson],
];

describe('the crosstabs view fixtures', () => {
  it.each(PAIRS)('%s: the typed copy equals the JSON Rust serialises', (_name, t, json) => {
    expect(t).toEqual(json);
  });

  it('carry the progress in steps, never columns, and one check point', () => {
    expect(Object.keys(stateJson)).toContain('stepsDone');
    expect(Object.keys(stateJson)).toContain('stepsTotal');
    expect(stateJson.stepsTotal).toBe(1);
  });

  it('give the 2 x 2 every field, and the other fixture its nulls and not-computable forms', () => {
    expect(resultJson.result.continuityCorrected).not.toBeNull();
    expect(resultJson.result.fisher).not.toBeNull();
    expect(otherJson.result.continuityCorrected).toBeNull();
    expect(otherJson.result.fisher).toBeNull();
    expect('not_computable' in otherJson.result.pearson).toBe(true);
    // A wire-shape fixture, not a consistent table: three row levels, frequencies
    // for rows 0 and 1 only, and two row totals (decision 588).
    expect(otherJson.result.rowLevels).toHaveLength(3);
    expect(otherJson.result.rowTotals).toHaveLength(2);
    expect(otherJson.result.frequencies.every((f) => f.row < 2)).toBe(true);
  });
});

describe('the spec the screen builds (decision 588)', () => {
  it('names the capability, its version and the view as its one input', () => {
    const s = buildXtabSpec(datasetView, 'C1', 'C2', [], 7);
    expect(s.capabilityId).toBe('stats.analyze.crosstabs');
    expect(s.capabilityVersion).toBe(1);
    expect(s.workspaceId).toBe('statistics');
    expect(s.operationId).toBe(7);
    expect(s.inputs).toEqual([datasetView.inputRef]);
    expect(s.randomSeed).toBeNull();
    expect(s.confirmationRefs).toEqual([]);
  });

  it("sends exactly CrosstabsParams' camelCase fields: rows, columns and missingCodes", () => {
    const s = buildXtabSpec(datasetView, 'C1', 'C2', [99], 1);
    expect(Object.keys(s.parameters).sort()).toEqual(['columns', 'missingCodes', 'rows']);
    expect(JSON.parse(JSON.stringify(s)).parameters).toEqual({ rows: 'C1', columns: 'C2', missingCodes: [99] });
  });

  it('sends the codes ascending and once each, so one choice is one spec', () => {
    expect(buildXtabSpec(datasetView, 'C1', 'C2', [99, -9, 99, -99], 1).parameters.missingCodes).toEqual([-99, -9, 99]);
    expect(JSON.stringify(buildXtabSpec(datasetView, 'C1', 'C2', [99, -9], 1))).toBe(JSON.stringify(buildXtabSpec(datasetView, 'C1', 'C2', [-9, 99], 1)));
  });

  it("carries the view's preparation exactly when it has one, the key absent otherwise (decision 490)", () => {
    expect(buildXtabSpec(datasetViewPrepared, 'C1', 'C2', [], 1).preparation).toEqual(datasetViewPrepared.preparation);
    expect('preparation' in buildXtabSpec(datasetView, 'C1', 'C2', [], 1)).toBe(false);
  });
});

describe('the bridge', () => {
  it('calls the crosstabs commands with the webview argument names, and has no levels command', async () => {
    const b = new TauriXtabBridge();
    const spec: XtabSpec = buildXtabSpec(datasetView, 'C1', 'C2', [], 1);
    await b.validate(3, spec);
    await b.run(3, spec);
    await b.status(5);
    await b.result(5);
    await b.cancel(5);
    await b.export(5, 'csv');
    await b.close(3);
    expect(invoke.mock.calls.map((c) => [c[0], Object.keys(c[1] as object).sort()])).toEqual([
      ['stats_xtab_validate', ['slot', 'spec']],
      ['stats_xtab_run', ['slot', 'spec']],
      ['stats_xtab_status', ['runId']],
      ['stats_xtab_result', ['runId']],
      ['stats_xtab_cancel', ['runId']],
      ['stats_xtab_export', ['format', 'runId']],
      ['stats_desc_close', ['slot']],
    ]);
    expect('groupLevels' in b).toBe(false);
    expect('groupLevels' in makeMockXtabBridge()).toBe(false);
  });

  it('the mock records the same command names', async () => {
    const m = makeMockXtabBridge({ stats_xtab_status: typed.xtabRunStateView, stats_xtab_cancel: typed.xtabRunStateView });
    await m.status(5);
    await m.cancel(5);
    await m.open('/some/where/file.csv').catch(() => undefined);
    expect(m.calls).toEqual([
      ['stats_xtab_status', { runId: 5 }],
      ['stats_xtab_cancel', { runId: 5 }],
      ['stats_desc_open', { path: '<path>' }],
    ]);
  });
});

/** Every code of the union, once: a missing one or an extra one fails tsc. */
const EVERY_CODE: Record<XtabCode, true> = {
  'stats.xtab.unknown_capability': true,
  'stats.xtab.capability_version_mismatch': true,
  'stats.xtab.input_count': true,
  'stats.xtab.input_mismatch': true,
  'stats.xtab.preparation_mismatch': true,
  'stats.xtab.missing_code_not_finite': true,
  'stats.xtab.missing_code_repeated': true,
  'stats.xtab.missing_codes_out_of_order': true,
  'stats.xtab.unknown_column': true,
  'stats.xtab.same_column': true,
  'stats.xtab.score_as_column': true,
  'stats.xtab.too_few_levels': true,
  'stats.xtab.too_many_levels': true,
  'stats.xtab.suspect_missing_code': true,
  'stats.xtab.overflow': true,
  'stats.xtab.arithmetic_refused': true,
  'stats.xtab.distribution_out_of_range': true,
  'stats.xtab.expected_below_five': true,
  'stats.xtab.missing_row': true,
  'stats.xtab.missing_column': true,
  'stats.xtab.missing_code': true,
  'stats.xtab.no_layers': true,
  'stats.xtab.fisher_two_by_two_only': true,
  'stats.xtab.no_further_nominal_measures': true,
  'stats.xtab.no_weights': true,
  'stats.xtab.code_point_order': true,
};

describe('the XtabCode union', () => {
  it('is exactly the stats.xtab words of xtabCodes.json', () => {
    const listed = (xtabCodes as string[]).filter((c) => c.startsWith('stats.xtab.'));
    expect(listed).toHaveLength(26);
    expect(Object.keys(EVERY_CODE).sort()).toEqual([...listed].sort());
  });
});
