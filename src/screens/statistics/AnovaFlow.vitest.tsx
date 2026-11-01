// The one-way ANOVA's screen (Research OS, Phase 8G-2, decisions 318, 328,
// 329 and 331), over a mock bridge:
// * the stages; every level ticked and no post hoc test ticked by default;
//   leaving a level out sends the ticked indexes ascending, and ticking tests
//   in any order sends them in the method's order; fewer than two levels
//   keeps Check off;
// * the run's progress in steps, never columns, and Cancel;
// * the neutral card: every not-computable statistic shows its sentence
//   where its number would be, a p below the floor shows as "below" the
//   floor's value from the result, and only the tests the result carries
//   (the ticked ones) have a table;
// * at 50 levels, each of the three tables shown in full, 1,225 rows, one at
//   a time under a heading with its row count, and the render time of all
//   three measured and printed;
// * no path anywhere in the DOM.
import React from 'react';
import { act, cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import AnovaFlow, { AnovaResultCard } from './AnovaFlow';
import { makeMockAnovaBridge } from './anovaBridge';
import type { DatasetView } from './describeTypes';
import type { AnovaResultView, AnovaRunStateView, AnovaSpec, AnovaValidationView, Pair, PostHoc } from './anovaTypes';
import { anovaResultView, anovaResultViewOther } from './fixtures/anovaFixtures';
import { anovaSentence } from './anovaWording';

vi.mock('../../design-system/GaplyGlobe', () => ({ GaplyGlobe: () => <div /> }));

afterEach(cleanup);

const INPUT = { contentHash: 'ab'.repeat(32), rows: 16, columns: 2, semanticRole: 'table' as const };

const view: DatasetView = {
  slot: 3,
  displayName: 'trial.csv',
  profile: {
    rows: 16,
    columns: [
      { alias: 'C1', header: 'score', kind: 'numeric', n: 16, missing: 0, distinct: 15, levels: null, suspectMissingCodes: [], nonFinite: 0 },
      {
        alias: 'C2',
        header: 'arm',
        kind: 'categorical',
        n: 16,
        missing: 0,
        distinct: 4,
        levels: [
          { label: 'a', count: 4 },
          { label: 'b', count: 4 },
          { label: 'c', count: 4 },
          { label: 'd', count: 4 },
        ],
        suspectMissingCodes: [],
        nonFinite: 0,
      },
    ],
  },
  inputRef: INPUT,
};

const report = (status: AnovaValidationView['status']): AnovaValidationView => ({
  status,
  errors: [],
  warnings: [],
  missingInformation: [],
  estimatedResources: { rows: 16, columns: 2, cells: 32 },
});

const state = (s: AnovaRunStateView['state'], terminal: boolean, done: number, cancelRequested = false): AnovaRunStateView => ({
  runId: 5,
  state: s,
  terminal,
  elapsedMs: 1200,
  cancelRequested,
  stepsDone: done,
  stepsTotal: 46,
  diagnostics: [],
});

const PATH = '/Users/someone/PATH_SENTINEL_a8/trial.csv';

const levels = ({ slot, column }: Record<string, unknown>) => ({
  slot,
  column,
  levels: ['a', 'b', 'c', 'd'].map((label, index) => ({ index, label, count: 4 })),
});

function setup(responses: Record<string, unknown>) {
  const save = vi.fn().mockResolvedValue('/chosen/out.csv');
  const bridge = makeMockAnovaBridge({ stats_desc_open: view, stats_desc_close: true, stats_anova_group_levels: levels, ...responses });
  render(<AnovaFlow bridge={bridge} pickPath={async () => PATH} save={save} pollMs={10} limits={async () => null} />);
  return { bridge, save };
}

const heading = () => screen.getByTestId('an-stage-heading');
const specs = (bridge: ReturnType<typeof makeMockAnovaBridge>, cmd: string) =>
  bridge.calls.filter(([c]) => c === cmd).map(([, a]) => a.spec as AnovaSpec);

async function toSettingsWithGroup() {
  fireEvent.click(screen.getByTestId('an-choose'));
  await waitFor(() => expect(heading().textContent).toBe('Profile of the file'));
  fireEvent.click(screen.getByTestId('an-to-settings'));
  await waitFor(() => expect(heading().textContent).toBe('Choose the outcome, the group, the levels and any post hoc tests'));
  fireEvent.change(screen.getByTestId('an-outcome'), { target: { value: 'C1' } });
  fireEvent.change(screen.getByTestId('an-group'), { target: { value: 'C2' } });
  await screen.findByTestId('an-level-3');
}

const box = (id: string) => screen.getByTestId(id) as HTMLInputElement;

describe('AnovaFlow', () => {
  it('ticks every level and no post hoc test, and sends one spec per choice', async () => {
    const { bridge } = setup({ stats_anova_validate: report('valid') });
    await toSettingsWithGroup();
    for (const i of [0, 1, 2, 3]) expect(box(`an-level-${i}`).checked, `level ${i}`).toBe(true);
    for (const t of ['tukey', 'games_howell', 'bonferroni']) expect(box(`an-post-hoc-${t}`).checked, t).toBe(false);

    fireEvent.click(screen.getByTestId('an-check'));
    await waitFor(() => expect(specs(bridge, 'stats_anova_validate')).toHaveLength(1));
    expect(specs(bridge, 'stats_anova_validate')[0].parameters).toEqual({
      outcome: 'C1',
      group: 'C2',
      levels: null,
      postHoc: [],
      missingCodes: [],
    });

    fireEvent.click(screen.getByTestId('an-back-settings'));
    await screen.findByTestId('an-level-1');
    fireEvent.click(screen.getByTestId('an-level-1'));
    fireEvent.click(screen.getByTestId('an-post-hoc-bonferroni'));
    fireEvent.click(screen.getByTestId('an-post-hoc-tukey'));
    fireEvent.click(screen.getByTestId('an-check'));
    await waitFor(() => expect(specs(bridge, 'stats_anova_validate')).toHaveLength(2));
    const p = specs(bridge, 'stats_anova_validate')[1].parameters;
    expect(p.levels).toEqual([0, 2, 3]);
    expect(p.postHoc).toEqual(['tukey', 'bonferroni']);
  });

  it('keeps Check off with fewer than two levels ticked, and says so', async () => {
    setup({});
    await toSettingsWithGroup();
    for (const i of [0, 1, 2]) fireEvent.click(screen.getByTestId(`an-level-${i}`));
    expect(screen.getByTestId('an-levels-too-few').textContent).toBe('1 level is ticked. Tick at least two.');
    expect((screen.getByTestId('an-check') as HTMLButtonElement).disabled).toBe(true);
    fireEvent.click(screen.getByTestId('an-level-0'));
    expect(screen.queryByTestId('an-levels-too-few')).toBeNull();
    expect((screen.getByTestId('an-check') as HTMLButtonElement).disabled).toBe(false);
  });

  it('shows the progress in steps while it runs, cancels, and shows the card', async () => {
    let polls = 0;
    const { bridge } = setup({
      stats_anova_validate: report('valid'),
      stats_anova_run: { runId: 5 },
      stats_anova_status: () => (polls++ < 2 ? state('running', false, 20) : state('succeeded', true, 46)),
      stats_anova_cancel: state('running', false, 20, true),
      stats_anova_result: anovaResultView,
    });
    await toSettingsWithGroup();
    fireEvent.click(screen.getByTestId('an-check'));
    await screen.findByTestId('an-run');
    fireEvent.click(screen.getByTestId('an-run'));
    expect((await screen.findByTestId('an-progress')).textContent).toBe('20 of 46 steps');
    expect(screen.getByTestId('an-run-status').textContent).not.toMatch(/column/i);
    fireEvent.click(screen.getByTestId('an-cancel'));
    await waitFor(() => expect(bridge.calls.some(([c]) => c === 'stats_anova_cancel')).toBe(true));
    await waitFor(() => expect(screen.getByTestId('an-progress').textContent).toBe('46 of 46 steps'));
    fireEvent.click(screen.getByTestId('an-see-results'));
    await screen.findByTestId('an-result-card');
    expect(document.body.innerHTML).not.toContain('PATH_SENTINEL_a8');
  });
});

const names = { column: (a: number) => `C${a}`, level: (i: number) => `level ${i + 1}` };

describe('AnovaResultCard', () => {
  it('shows each not-computable statistic as its sentence where its number would be, and a p below the floor as below it', () => {
    render(<AnovaResultCard result={anovaResultView} names={names} />);
    const welch = screen.getByTestId('an-welch');
    expect(within(welch).getByTestId('an-welch-not-computable').textContent).toBe(
      anovaSentence({ code: 'stats.anova.welch_variance_zero' })
    );
    expect(screen.getByTestId('an-levene-median-not-computable').textContent).toBe(
      anovaSentence({ code: 'stats.anova.levene_deviations_equal' })
    );
    expect(screen.getByTestId('an-omega').textContent).toBe(anovaSentence({ code: 'stats.anova.within_variance_zero' }));
    expect(screen.getByTestId('an-eta').textContent).toBe('0.806');
    // Tukey's table first, its p below the floor shown as below 1e-10.
    const tukey = screen.getByTestId('an-post-hoc-table-tukey');
    expect(within(tukey).getByTestId('an-post-hoc-heading-tukey').textContent).toBe("Tukey's HSD: 2 pairs");
    expect(tukey.textContent).toContain('below 1e-10');
    expect(tukey.textContent).toContain('ctrl minus dose_hi');
    // A pair not computable, and a test not computable, each in words.
    fireEvent.click(screen.getByTestId('an-show-games_howell'));
    expect(screen.getByTestId('an-post-hoc-table-games_howell').textContent).toContain(
      anovaSentence({ code: 'stats.anova.pair_both_constant' })
    );
    fireEvent.click(screen.getByTestId('an-show-bonferroni'));
    expect(screen.getByTestId('an-post-hoc-not-computable-bonferroni').textContent).toBe(
      anovaSentence({ code: 'stats.anova.within_variance_zero' })
    );
    expect(screen.getByTestId('an-notes').textContent).toContain(anovaSentence({ code: 'stats.anova.p_below_floor' }));
    expect(document.body.textContent).not.toMatch(/significan/i);
  });

  it('shows the other variants: a table not computable, F tests computed, omega as computed', () => {
    render(<AnovaResultCard result={anovaResultViewOther} names={names} />);
    expect(screen.getByTestId('an-anova-table-not-computable').textContent).toBe(
      anovaSentence({ code: 'stats.anova.outcome_constant' })
    );
    expect(screen.getByTestId('an-welch').textContent).toContain('3.5');
    expect(screen.getByTestId('an-omega').textContent).toBe('-0.05');
    expect(screen.getByTestId('an-eta').textContent).toBe(anovaSentence({ code: 'stats.anova.outcome_constant' }));
  });

  it('shows a table only for a test the result carries: one ticked, one table and no choice', () => {
    const onlyTukey: AnovaResultView = {
      ...anovaResultView,
      result: { ...anovaResultView.result, postHoc: anovaResultView.result.postHoc.filter((t) => t.test === 'tukey') },
    };
    render(<AnovaResultCard result={onlyTukey} names={names} />);
    expect(screen.getByTestId('an-post-hoc-table-tukey')).toBeTruthy();
    for (const t of ['games_howell', 'bonferroni']) {
      expect(screen.queryByTestId(`an-post-hoc-table-${t}`), t).toBeNull();
      expect(screen.queryByTestId(`an-show-${t}`), t).toBeNull();
    }
    cleanup();
    const none: AnovaResultView = { ...anovaResultView, result: { ...anovaResultView.result, postHoc: [] } };
    render(<AnovaResultCard result={none} names={names} />);
    expect(screen.queryByTestId('an-post-hoc-results')).toBeNull();
  });

  it('at 50 levels shows each test in full, 1,225 rows, one at a time, and measures the render', () => {
    const k = 50;
    const groups = Array.from({ length: k }, (_, i) => ({
      level: i,
      label: `g${String(i).padStart(2, '0')}`,
      n: 2000,
      mean: i,
      sd: 1,
      se: 0.02,
      ciLower: i - 0.04,
      ciUpper: i + 0.04,
      min: i - 3,
      max: i + 3,
    }));
    const pairs = (test: PostHoc): Pair[] => {
      const out: Pair[] = [];
      for (let i = 0; i < k; i++)
        for (let j = i + 1; j < k; j++)
          out.push({
            first: i,
            second: j,
            outcome: {
              computed: {
                difference: i - j,
                se: 0.03,
                statistic: 4.5,
                df: test === 'games_howell' ? 3997.5 : 99950,
                p: j - i > 2 ? { below: 1e-10 } : { value: 0.0123 },
                ciLower: i - j - 0.1,
                ciUpper: i - j + 0.1,
              },
            },
          });
      return out;
    };
    const big: AnovaResultView = {
      ...anovaResultView,
      result: {
        ...anovaResultView.result,
        groups,
        postHoc: (['tukey', 'games_howell', 'bonferroni'] as PostHoc[]).map((test) => ({ test, pairs: { computed: pairs(test) } })),
      },
    };
    const times: Record<string, number> = {};
    let t0 = performance.now();
    render(<AnovaResultCard result={big} names={names} />);
    times.tukey = performance.now() - t0;
    const rows = (test: string) => screen.getByTestId(`an-post-hoc-table-${test}`).querySelectorAll('tbody tr').length;
    expect(rows('tukey')).toBe(1225);
    expect(screen.getByTestId('an-post-hoc-heading-tukey').textContent).toBe("Tukey's HSD: 1225 pairs");
    for (const test of ['games_howell', 'bonferroni']) {
      t0 = performance.now();
      act(() => {
        fireEvent.click(screen.getByTestId(`an-show-${test}`));
      });
      times[test] = performance.now() - t0;
      expect(rows(test), test).toBe(1225);
      expect(screen.getByTestId(`an-post-hoc-heading-${test}`).textContent).toMatch(/: 1225 pairs$/);
      // One test at a time: the other tables are not in the DOM.
      expect(document.querySelectorAll('[data-testid^="an-post-hoc-table-"]')).toHaveLength(1);
    }
    // eslint-disable-next-line no-console
    console.log(
      `ANOVA_RENDER_MS 50 levels, 1225 rows each: tukey ${times.tukey.toFixed(0)} games_howell ${times.games_howell.toFixed(0)} bonferroni ${times.bonferroni.toFixed(0)}`
    );
    expect(times.tukey + times.games_howell + times.bonferroni).toBeLessThan(30000);
  });
});
