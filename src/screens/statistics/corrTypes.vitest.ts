// The correlations' wire contract, from the TypeScript side (Research OS,
// Phase 8H-2, decisions 338 and 356). Each JSON fixture is the exact
// serialisation of a Rust view (src-tauri/tests/stats_corr_wire.rs checks the
// bytes); here each is deep-compared with a copy typed `satisfies` the view's
// type. Also pinned: the bridge's command names and argument keys, and the
// spec the screen builds: the columns in the file's order, the coefficients
// in Pearson, Spearman, Kendall order and the codes ascending, so one choice
// always gives one spec (decision 338).
import { beforeEach, describe, expect, it, vi } from 'vitest';

import corrCodes from './corrCodes.json';
import type { CorrCode } from './corrTypes';

import validationJson from './fixtures/corrValidationView.json';
import stateJson from './fixtures/corrRunStateView.json';
import resultJson from './fixtures/corrResultView.json';
import otherJson from './fixtures/corrResultViewOther.json';
import * as typed from './fixtures/corrFixtures';
import type { DatasetView } from './describeTypes';
import { TauriCorrBridge } from './corrBridge';
import { buildCorrSpec } from './CorrFlow';
import type { Coefficient } from './corrTypes';

const invoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...a: unknown[]) => invoke(...a) }));
vi.mock('../../utils/isTauri', () => ({ isTauri: true, default: true }));

beforeEach(() => {
  invoke.mockReset();
  invoke.mockResolvedValue({});
});

const PAIRS: Array<[string, unknown, unknown]> = [
  ['CorrValidationView', typed.corrValidationView, validationJson],
  ['CorrRunStateView', typed.corrRunStateView, stateJson],
  ['CorrResultView', typed.corrResultView, resultJson],
  ['CorrResultView (the other variants)', typed.corrResultViewOther, otherJson],
];

describe('the correlations view fixtures', () => {
  it.each(PAIRS)('%s: the typed copy equals the JSON Rust serialises', (_name, t, json) => {
    expect(t).toEqual(json);
  });

  it('carry the progress in steps, never columns', () => {
    expect(Object.keys(stateJson)).toContain('stepsDone');
    expect(Object.keys(stateJson)).toContain('stepsTotal');
    expect(JSON.stringify(stateJson)).not.toContain('columns');
  });

  it('name columns by alias only', () => {
    const aliases = JSON.stringify(resultJson.result).match(/"(variable|first|second)":"[^"]*"/g) ?? [];
    expect(aliases.length).toBeGreaterThan(5);
    for (const a of aliases) expect(a).toMatch(/:"C[0-9]+"$/);
  });
});

const numeric = (alias: string, header: string) => ({
  alias,
  header,
  kind: 'numeric' as const,
  n: 10,
  missing: 0,
  distinct: 10,
  levels: null,
  suspectMissingCodes: [],
  nonFinite: 0,
});

const VIEW: DatasetView = {
  slot: 3,
  displayName: 'thesis.csv',
  profile: {
    rows: 10,
    columns: [numeric('C1', 'Trust'), numeric('C2', 'Satisfaction'), numeric('C3', 'Loyalty'), numeric('C4', 'Price')],
  },
  inputRef: { contentHash: 'ab'.repeat(32), rows: 10, columns: 4, semanticRole: 'table' },
};

describe('the spec the screen builds (decision 338)', () => {
  const build = (vars: string[], coefs: Coefficient[], codes: number[] = []) =>
    buildCorrSpec(VIEW, vars, coefs, 'pairwise', codes, 1).parameters;

  it('sends the columns in the file order, whatever order they were ticked in', () => {
    expect(build(['C3', 'C1', 'C4'], ['pearson']).variables).toEqual(['C1', 'C3', 'C4']);
    expect(build(['C4', 'C3', 'C2', 'C1'], ['pearson']).variables).toEqual(['C1', 'C2', 'C3', 'C4']);
  });

  it('sends the coefficients in the method order and the codes ascending, once each', () => {
    expect(build(['C1', 'C2'], ['kendall', 'pearson']).coefficients).toEqual(['pearson', 'kendall']);
    expect(build(['C1', 'C2'], ['kendall', 'spearman', 'pearson']).coefficients).toEqual(['pearson', 'spearman', 'kendall']);
    expect(build(['C1', 'C2'], ['pearson'], [99, -9, 99]).missingCodes).toEqual([-9, 99]);
  });

  it('gives one spec for one choice, however it was made', () => {
    expect(JSON.stringify(build(['C3', 'C1'], ['kendall', 'pearson'], [99, -9]))).toBe(
      JSON.stringify(build(['C1', 'C3'], ['pearson', 'kendall'], [-9, 99]))
    );
  });

  it('names columns by alias, never a header', () => {
    const s = buildCorrSpec(VIEW, ['C1', 'C3'], ['spearman'], 'listwise', [99], 7);
    expect(s.capabilityId).toBe('stats.analyze.correlations');
    expect(Object.keys(s.parameters).sort()).toEqual(['coefficients', 'missing', 'missingCodes', 'variables']);
    expect(s.parameters.missing).toBe('listwise');
    for (const h of ['Trust', 'Satisfaction', 'Loyalty', 'Price']) expect(JSON.stringify(s)).not.toContain(h);
  });
});

describe('the bridge', () => {
  it('calls the correlations commands with the webview argument names', async () => {
    const b = new TauriCorrBridge();
    const spec = buildCorrSpec(VIEW, ['C1', 'C2'], ['pearson'], 'pairwise', [], 1);
    await b.validate(3, spec);
    await b.run(3, spec);
    await b.status(5);
    await b.result(5);
    await b.cancel(5);
    await b.export(5, 'csv');
    await b.close(3);
    expect(invoke.mock.calls.map((c) => [c[0], Object.keys(c[1] as object).sort()])).toEqual([
      ['stats_corr_validate', ['slot', 'spec']],
      ['stats_corr_run', ['slot', 'spec']],
      ['stats_corr_status', ['runId']],
      ['stats_corr_result', ['runId']],
      ['stats_corr_cancel', ['runId']],
      ['stats_corr_export', ['format', 'runId']],
      ['stats_desc_close', ['slot']],
    ]);
  });
});

/** Every code of the union, once: a missing one or an extra one fails tsc
 * (Research OS, Phase 8M-3, decision 428, as onesampleTypes.vitest.ts pins its own). */
const EVERY_CODE: Record<CorrCode, true> = {
  'stats.corr.unknown_capability': true,
  'stats.corr.capability_version_mismatch': true,
  'stats.corr.input_count': true,
  'stats.corr.input_mismatch': true,
  'stats.corr.unknown_column': true,
  'stats.corr.too_few_variables': true,
  'stats.corr.variable_repeated': true,
  'stats.corr.variables_out_of_order': true,
  'stats.corr.no_coefficient': true,
  'stats.corr.coefficient_repeated': true,
  'stats.corr.coefficients_out_of_order': true,
  'stats.corr.missing_code_not_finite': true,
  'stats.corr.missing_code_repeated': true,
  'stats.corr.missing_codes_out_of_order': true,
  'stats.corr.variable_holds_text': true,
  'stats.corr.variable_non_finite': true,
  'stats.corr.suspect_missing_code': true,
  'stats.corr.too_few_complete_rows': true,
  'stats.corr.no_pair_of_three': true,
  'stats.corr.over_budget': true,
  'stats.corr.pair_too_few_rows': true,
  'stats.corr.variable_constant': true,
  'stats.corr.distribution_out_of_range': true,
  'stats.corr.overflow': true,
  'stats.corr.underflow': true,
  'stats.corr.arithmetic_refused': true,
  'stats.corr.interval_too_few_rows': true,
  'stats.corr.no_values': true,
  'stats.corr.too_few_values': true,
  'stats.corr.perfect_coefficient': true,
  'stats.corr.missing_value': true,
  'stats.corr.missing_code': true,
  'stats.corr.fewer_than_two_values': true,
  'stats.corr.pearson_assumes_normality': true,
  'stats.corr.two_tailed_no_partial': true,
  'stats.corr.preparation_mismatch': true,
  'stats.corr.code_equals_score': true,
};

describe('the CorrCode union', () => {
  it('is exactly the stats.corr words of corrCodes.json', () => {
    const listed = (corrCodes as string[]).filter((c) => c.startsWith('stats.corr.'));
    expect(listed).toHaveLength(37);
    expect(Object.keys(EVERY_CODE).sort()).toEqual([...listed].sort());
  });
});
