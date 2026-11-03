// The one-way ANOVA's wire contract, from the TypeScript side (Research OS,
// Phase 8G-2, decisions 318 and 329). Each JSON fixture is the exact
// serialisation of a Rust view (src-tauri/tests/stats_anova_wire.rs checks
// the bytes); here each is deep-compared with a copy typed `satisfies` the
// view's type. Also pinned: the bridge's command names and argument keys, and
// the spec the screen builds: levels null when every level is ticked and
// otherwise the ticked indexes ascending, the post hoc tests in the method's
// order, so one choice always gives one spec (decision 329).
import { beforeEach, describe, expect, it, vi } from 'vitest';

import anovaCodes from './anovaCodes.json';
import type { AnovaCode } from './anovaTypes';

import validationJson from './fixtures/anovaValidationView.json';
import stateJson from './fixtures/anovaRunStateView.json';
import resultJson from './fixtures/anovaResultView.json';
import otherJson from './fixtures/anovaResultViewOther.json';
import * as typed from './fixtures/anovaFixtures';
import { datasetView } from './fixtures/describeFixtures';
import { TauriAnovaBridge } from './anovaBridge';
import { buildAnovaSpec } from './AnovaFlow';
import type { AnovaSpec } from './anovaTypes';
import type { GroupLevel } from './ttestTypes';

const invoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...a: unknown[]) => invoke(...a) }));
vi.mock('../../utils/isTauri', () => ({ isTauri: true, default: true }));

beforeEach(() => {
  invoke.mockReset();
  invoke.mockResolvedValue({});
});

const PAIRS: Array<[string, unknown, unknown]> = [
  ['AnovaValidationView', typed.anovaValidationView, validationJson],
  ['AnovaRunStateView', typed.anovaRunStateView, stateJson],
  ['AnovaResultView', typed.anovaResultView, resultJson],
  ['AnovaResultView (the other variants)', typed.anovaResultViewOther, otherJson],
];

describe('the ANOVA view fixtures', () => {
  it.each(PAIRS)('%s: the typed copy equals the JSON Rust serialises', (_name, t, json) => {
    expect(t).toEqual(json);
  });

  it('carry the progress in steps, never columns', () => {
    expect(Object.keys(stateJson)).toContain('stepsDone');
    expect(Object.keys(stateJson)).toContain('stepsTotal');
    expect(JSON.stringify(stateJson)).not.toContain('columns');
  });
});

const LEVELS: GroupLevel[] = [
  { index: 0, label: 'a', count: 4 },
  { index: 1, label: 'b', count: 4 },
  { index: 2, label: 'c', count: 4 },
  { index: 3, label: 'd', count: 4 },
];

describe('the spec the screen builds (decision 329)', () => {
  const build = (ticked: number[], postHoc: AnovaSpec['parameters']['postHoc']) =>
    buildAnovaSpec(datasetView, 'C1', 'C2', LEVELS, ticked, postHoc, [], 1).parameters;

  it('sends null when every level is ticked, in whatever order they were ticked', () => {
    expect(build([0, 1, 2, 3], []).levels).toBeNull();
    expect(build([3, 1, 0, 2], []).levels).toBeNull();
  });

  it('sends the ticked indexes ascending when one is left out, so one choice is one spec', () => {
    expect(build([3, 0, 2], []).levels).toEqual([0, 2, 3]);
    expect(build([2, 3, 0], []).levels).toEqual([0, 2, 3]);
    expect(JSON.stringify(build([3, 0, 2], ['bonferroni', 'tukey']))).toBe(
      JSON.stringify(build([0, 2, 3], ['tukey', 'bonferroni']))
    );
  });

  it('sends the post hoc tests in the method order, none by default', () => {
    expect(build([0, 1], []).postHoc).toEqual([]);
    expect(build([0, 1], ['bonferroni', 'games_howell', 'tukey']).postHoc).toEqual(['tukey', 'games_howell', 'bonferroni']);
  });

  it('names columns by alias and levels by index, never a label', () => {
    const s = buildAnovaSpec(datasetView, 'C1', 'C2', LEVELS, [0, 2], ['tukey'], [99], 7);
    expect(s.capabilityId).toBe('stats.analyze.one_way_anova');
    expect(Object.keys(s.parameters).sort()).toEqual(['group', 'levels', 'missingCodes', 'outcome', 'postHoc']);
    for (const l of LEVELS) expect(JSON.stringify(s)).not.toContain(`"${l.label}"`);
  });
});

describe('the bridge', () => {
  it('calls the ANOVA commands with the webview argument names', async () => {
    const b = new TauriAnovaBridge();
    const spec = buildAnovaSpec(datasetView, 'C1', 'C2', LEVELS, [0, 1, 2, 3], [], [], 1);
    await b.groupLevels(3, 'C2');
    await b.validate(3, spec);
    await b.run(3, spec);
    await b.status(5);
    await b.result(5);
    await b.cancel(5);
    await b.export(5, 'csv');
    await b.close(3);
    expect(invoke.mock.calls.map((c) => [c[0], Object.keys(c[1] as object).sort()])).toEqual([
      ['stats_anova_group_levels', ['column', 'slot']],
      ['stats_anova_validate', ['slot', 'spec']],
      ['stats_anova_run', ['slot', 'spec']],
      ['stats_anova_status', ['runId']],
      ['stats_anova_result', ['runId']],
      ['stats_anova_cancel', ['runId']],
      ['stats_anova_export', ['format', 'runId']],
      ['stats_desc_close', ['slot']],
    ]);
  });
});

/** Every code of the union, once: a missing one or an extra one fails tsc
 * (Research OS, Phase 8M-3, decision 428, as onesampleTypes.vitest.ts pins its own). */
const EVERY_CODE: Record<AnovaCode, true> = {
  'stats.anova.unknown_capability': true,
  'stats.anova.capability_version_mismatch': true,
  'stats.anova.input_count': true,
  'stats.anova.input_mismatch': true,
  'stats.anova.unknown_column': true,
  'stats.anova.same_column': true,
  'stats.anova.outcome_holds_text': true,
  'stats.anova.outcome_non_finite': true,
  'stats.anova.group_one_level': true,
  'stats.anova.too_many_levels': true,
  'stats.anova.level_out_of_range': true,
  'stats.anova.level_repeated': true,
  'stats.anova.too_few_levels': true,
  'stats.anova.missing_code_not_finite': true,
  'stats.anova.suspect_missing_code': true,
  'stats.anova.group_too_small': true,
  'stats.anova.post_hoc_too_many_levels': true,
  'stats.anova.outcome_constant': true,
  'stats.anova.within_variance_zero': true,
  'stats.anova.welch_variance_zero': true,
  'stats.anova.levene_deviations_equal': true,
  'stats.anova.pair_both_constant': true,
  'stats.anova.distribution_out_of_range': true,
  'stats.anova.overflow': true,
  'stats.anova.underflow': true,
  'stats.anova.arithmetic_refused': true,
  'stats.anova.p_below_floor': true,
  'stats.anova.omega_squared_negative': true,
  'stats.anova.missing_group': true,
  'stats.anova.other_level': true,
  'stats.anova.missing_outcome': true,
  'stats.anova.missing_code': true,
  'stats.anova.normality_not_assessed': true,
  'stats.anova.effect_sizes_have_no_interval': true,
  'stats.anova.no_contrasts': true,
  'stats.anova.preparation_mismatch': true,
  'stats.anova.score_as_group': true,
  'stats.anova.code_equals_score': true,
};

describe('the AnovaCode union', () => {
  it('is exactly the stats.anova words of anovaCodes.json', () => {
    const listed = (anovaCodes as string[]).filter((c) => c.startsWith('stats.anova.'));
    expect(listed).toHaveLength(38);
    expect(Object.keys(EVERY_CODE).sort()).toEqual([...listed].sort());
  });
});
