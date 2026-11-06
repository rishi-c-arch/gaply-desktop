// The correlations' screen (Research OS, Phase 8H-2, decisions 355, 358 and
// 359), over a mock bridge:
// * no column, no coefficient and no missing handling chosen when the
//   settings open; Check stays off, with a sentence saying what is missing;
//   "Tick every numeric column" and "Untick all" are the user's actions; one
//   choice sends one spec, in the column, coefficient and code order;
// * the candidate missing codes are the union over the ticked columns,
//   ascending;
// * a choice over the budget shows its sentence, with the pairs and the rows,
//   at Check the settings, and never starts a run;
// * the run's progress in steps, never columns, and Cancel;
// * the neutral card: the descriptives; one button per coefficient with its
//   pair count; a lower triangle for 20 variables or fewer, none above; the
//   pairs in pages of 1,225 rows with Next, Previous and which rows are
//   shown; every not-computable statistic as its sentence; a perfect Pearson
//   or Spearman t as its sentence and its p 0, a perfect Kendall z computed;
// * the render time at 50 variables (1,225 pairs) and at 128 (8,128 pairs,
//   the first page and the last), measured and printed;
// * no path, and no word "significant", anywhere in the DOM.
import React from 'react';
import { act, cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import CorrFlow, { CorrResultCard } from './CorrFlow';
import { makeMockCorrBridge } from './corrBridge';
import type { ColumnProfile, DatasetView } from './describeTypes';
import type { Coefficient, CorrResultView, CorrRunStateView, CorrSpec, CorrValidationView, Pair } from './corrTypes';
import { corrResultView, corrResultViewOther } from './fixtures/corrFixtures';
import { corrSentence } from './corrWording';

vi.mock('../../design-system/GaplyGlobe', () => ({ GaplyGlobe: () => <div /> }));

afterEach(cleanup);

const INPUT = { contentHash: 'ab'.repeat(32), rows: 200, columns: 5, semanticRole: 'table' as const };

const col = (alias: string, header: string, codes: Array<[number, number]> = [], kind: ColumnProfile['kind'] = 'numeric'): ColumnProfile => ({
  alias,
  header,
  kind,
  n: 200,
  missing: 0,
  distinct: kind === 'numeric' ? 150 : 3,
  levels: kind === 'numeric' ? null : [{ label: 'x', count: 200 }],
  suspectMissingCodes: codes.map(([code, cells]) => ({ code, cells })),
  nonFinite: 0,
});

const view: DatasetView = {
  slot: 3,
  displayName: 'thesis.csv',
  profile: {
    rows: 200,
    columns: [
      col('C1', 'Trust', [[-9, 2]]),
      col('C2', 'Group', [], 'categorical'),
      col('C3', 'Loyalty', [[99, 4]]),
      col('C4', 'Price', [
        [99, 1],
        [-99, 3],
      ]),
      col('C5', 'Quality'),
    ],
  },
  inputRef: INPUT,
};

const report = (status: CorrValidationView['status'], errors: CorrValidationView['errors'] = []): CorrValidationView => ({
  status,
  errors,
  warnings: [],
  missingInformation: [],
  estimatedResources: { rows: 200, columns: 4, cells: 800 },
});

const state = (s: CorrRunStateView['state'], terminal: boolean, done: number, cancelRequested = false): CorrRunStateView => ({
  runId: 5,
  state: s,
  terminal,
  elapsedMs: 1200,
  cancelRequested,
  stepsDone: done,
  stepsTotal: 19,
  diagnostics: [],
});

const PATH = '/Users/someone/PATH_SENTINEL_c9/thesis.csv';

function setup(responses: Record<string, unknown>) {
  const save = vi.fn().mockResolvedValue('/chosen/out.csv');
  const bridge = makeMockCorrBridge({ stats_desc_open: view, stats_desc_close: true, ...responses });
  render(<CorrFlow bridge={bridge} pickPath={async () => PATH} save={save} pollMs={10} limits={async () => null} />);
  return { bridge, save };
}

const heading = () => screen.getByTestId('cr-stage-heading');
const specs = (bridge: ReturnType<typeof makeMockCorrBridge>, cmd: string) =>
  bridge.calls.filter(([c]) => c === cmd).map(([, a]) => a.spec as CorrSpec);
const box = (id: string) => screen.getByTestId(id) as HTMLInputElement;
const check = () => screen.getByTestId('cr-check') as HTMLButtonElement;

async function toSettings() {
  fireEvent.click(screen.getByTestId('cr-choose'));
  await waitFor(() => expect(heading().textContent).toBe('Profile of the file'));
  fireEvent.click(screen.getByTestId('cr-to-settings'));
  await waitFor(() => expect(heading().textContent).toBe('Choose the variables, the coefficients and how missing values are handled'));
}

async function chooseAndCheck(bridge: ReturnType<typeof makeMockCorrBridge>) {
  await toSettings();
  fireEvent.click(screen.getByTestId('cr-tick-all'));
  for (const c of ['pearson', 'spearman', 'kendall']) fireEvent.click(screen.getByTestId(`cr-coefficient-${c}`));
  fireEvent.click(screen.getByTestId('cr-missing-pairwise'));
  fireEvent.click(check());
  await waitFor(() => expect(specs(bridge, 'stats_corr_validate')).toHaveLength(1));
}

describe('CorrFlow settings', () => {
  it('opens with no column, no coefficient and no handling chosen, and Check off with what is missing', async () => {
    setup({});
    await toSettings();
    for (const a of ['C1', 'C3', 'C4', 'C5']) expect(box(`cr-variable-${a}`).checked, a).toBe(false);
    expect(screen.queryByTestId('cr-variable-C2'), 'a text column is not offered').toBeNull();
    for (const c of ['pearson', 'spearman', 'kendall']) expect(box(`cr-coefficient-${c}`).checked, c).toBe(false);
    for (const m of ['pairwise', 'listwise']) expect(box(`cr-missing-${m}`).checked, m).toBe(false);
    expect(check().disabled).toBe(true);
    expect(screen.getByTestId('cr-settings-missing').textContent).toBe(
      'To check the settings, choose two variables, a coefficient and how missing values are handled.'
    );
    expect(screen.getByTestId('cr-missing').textContent).toContain('Each pair uses every row in which both of its columns have a value');
    expect(screen.getByTestId('cr-missing').textContent).toContain('Every pair uses only the rows in which every ticked column has a value');
    fireEvent.click(screen.getByTestId('cr-variable-C3'));
    expect(screen.getByTestId('cr-settings-missing').textContent).toBe(
      'To check the settings, choose one more variable, a coefficient and how missing values are handled.'
    );
    fireEvent.click(screen.getByTestId('cr-variable-C1'));
    fireEvent.click(screen.getByTestId('cr-coefficient-kendall'));
    expect(check().disabled).toBe(true);
    fireEvent.click(screen.getByTestId('cr-missing-listwise'));
    expect(screen.queryByTestId('cr-settings-missing')).toBeNull();
    expect(check().disabled).toBe(false);
  });

  it('ticks and unticks every numeric column only when asked, and sends one spec per choice', async () => {
    const { bridge } = setup({ stats_corr_validate: report('valid') });
    await toSettings();
    fireEvent.click(screen.getByTestId('cr-tick-all'));
    for (const a of ['C1', 'C3', 'C4', 'C5']) expect(box(`cr-variable-${a}`).checked, a).toBe(true);
    fireEvent.click(screen.getByTestId('cr-untick-all'));
    for (const a of ['C1', 'C3', 'C4', 'C5']) expect(box(`cr-variable-${a}`).checked, a).toBe(false);
    // Ticked out of order: the spec is in the file's, coefficient and code
    // order.
    for (const a of ['C5', 'C1', 'C3']) fireEvent.click(screen.getByTestId(`cr-variable-${a}`));
    for (const c of ['kendall', 'pearson']) fireEvent.click(screen.getByTestId(`cr-coefficient-${c}`));
    fireEvent.click(screen.getByTestId('cr-missing-pairwise'));
    fireEvent.click(screen.getByTestId('cr-code-99'));
    fireEvent.click(screen.getByTestId('cr-code--9'));
    fireEvent.click(check());
    await waitFor(() => expect(specs(bridge, 'stats_corr_validate')).toHaveLength(1));
    expect(specs(bridge, 'stats_corr_validate')[0].parameters).toEqual({
      variables: ['C1', 'C3', 'C5'],
      coefficients: ['pearson', 'kendall'],
      missing: 'pairwise',
      missingCodes: [-9, 99],
    });
  });

  it('offers the candidate codes of the ticked columns only, ascending, each with where it appears', async () => {
    setup({});
    await toSettings();
    expect(screen.queryByTestId('cr-missing-codes')).toBeNull();
    fireEvent.click(screen.getByTestId('cr-variable-C4'));
    fireEvent.click(screen.getByTestId('cr-variable-C3'));
    const offered = Array.from(screen.getByTestId('cr-missing-codes').querySelectorAll('input')).map((i) => i.getAttribute('data-testid'));
    expect(offered).toEqual(['cr-code--99', 'cr-code-99']);
    expect(screen.getByTestId('cr-missing-codes').textContent).toContain('It appears in "Loyalty" (4 cells), "Price" (1 cell).');
    // A ticked code of a column that is then unticked is not sent.
    fireEvent.click(screen.getByTestId('cr-code--99'));
    fireEvent.click(screen.getByTestId('cr-variable-C4'));
    expect(screen.queryByTestId('cr-code--99')).toBeNull();
  });

  it('shows the budget sentence with the pairs and the rows at Check the settings, and never runs', async () => {
    const { bridge } = setup({
      stats_corr_validate: report('invalid', [{ code: 'stats.corr.over_budget', params: { count: 2340, found: 100000 }, severity: 'error' }]),
    });
    await chooseAndCheck(bridge);
    await waitFor(() => expect(heading().textContent).toBe('Check the settings'));
    expect(screen.getByTestId('cr-validation-errors').textContent).toContain(
      'These settings are refused before they run: 2,340 pairs over 100,000 rows are estimated to take longer than 15 seconds on this computer. Choose fewer variables or fewer coefficients.'
    );
    const run = screen.getByTestId('cr-run') as HTMLButtonElement;
    expect(run.disabled).toBe(true);
    fireEvent.click(run);
    expect(bridge.calls.filter(([c]) => c === 'stats_corr_run')).toEqual([]);
  });

  it('shows the progress in steps while it runs, cancels, and shows the card', async () => {
    let polls = 0;
    const { bridge } = setup({
      stats_corr_validate: report('valid'),
      stats_corr_run: { runId: 5 },
      stats_corr_status: () => (polls++ < 2 ? state('running', false, 7) : state('succeeded', true, 19)),
      stats_corr_cancel: state('running', false, 7, true),
      stats_corr_result: corrResultView,
    });
    await chooseAndCheck(bridge);
    await screen.findByTestId('cr-run');
    fireEvent.click(screen.getByTestId('cr-run'));
    expect((await screen.findByTestId('cr-progress')).textContent).toBe('7 of 19 steps');
    expect(screen.getByTestId('cr-run-status').textContent).not.toMatch(/column/i);
    fireEvent.click(screen.getByTestId('cr-cancel'));
    await waitFor(() => expect(bridge.calls.some(([c]) => c === 'stats_corr_cancel')).toBe(true));
    await waitFor(() => expect(screen.getByTestId('cr-progress').textContent).toBe('19 of 19 steps'));
    fireEvent.click(screen.getByTestId('cr-see-results'));
    await screen.findByTestId('cr-result-card');
    // The live screen names the result's aliases by the file's headers.
    expect(screen.getByTestId('cr-descriptives').textContent).toContain('"Trust"');
    expect(document.body.innerHTML).not.toContain('PATH_SENTINEL_c9');
  });
});

const names = { column: (a: number) => `C${a}` };

describe('CorrResultCard', () => {
  it('shows each not-computable statistic as its sentence, a perfect t as its sentence with p 0, and a perfect z computed', () => {
    render(<CorrResultCard result={corrResultView} names={names} />);
    const desc = screen.getByTestId('cr-descriptives');
    expect(desc.textContent).toContain(corrSentence({ code: 'stats.corr.too_few_values' }));
    expect(desc.textContent).toContain('4.25');
    // One button per coefficient, with its pair count.
    expect(screen.getByTestId('cr-show-pearson').textContent).toBe("Pearson's r: 3 pairs");
    expect(screen.getByTestId('cr-show-spearman').textContent).toBe("Spearman's rho: 1 pair");
    expect(screen.getByTestId('cr-show-kendall').textContent).toBe("Kendall's tau-b: 1 pair");
    // Pearson first: a pair not computable, in words, in the table and the
    // matrix.
    const pearson = screen.getByTestId('cr-pairs-pearson');
    expect(within(pearson).getAllByTestId('cr-pair-not-computable')[0].textContent).toBe(
      corrSentence({ code: 'stats.corr.pair_too_few_rows' })
    );
    expect(screen.getByTestId('cr-matrix-C1-C4').textContent).toBe(corrSentence({ code: 'stats.corr.pair_too_few_rows' }));
    expect(screen.getByTestId('cr-matrix-C1-C3').textContent).toBe('0.61');
    // Spearman: perfect, so its t is its sentence and its p is 0.
    fireEvent.click(screen.getByTestId('cr-show-spearman'));
    const spearman = screen.getByTestId('cr-pairs-spearman');
    expect(within(spearman).getByTestId('cr-t').textContent).toBe(corrSentence({ code: 'stats.corr.perfect_coefficient' }));
    expect(within(spearman).getByTestId('cr-t').textContent).toMatch(/^Infinite/);
    expect(within(spearman).getByTestId('cr-p').textContent).toBe('0');
    expect(within(spearman).getByTestId('cr-interval').textContent).toBe('1 to 1');
    // Kendall: z and p computed.
    fireEvent.click(screen.getByTestId('cr-show-kendall'));
    expect(within(screen.getByTestId('cr-pairs-kendall')).getByTestId('cr-z').textContent).toBe('1.732');
    expect(screen.queryByTestId('cr-pairs-pearson'), 'one coefficient at a time').toBeNull();
    expect(screen.getByTestId('cr-exclusions').textContent).toContain(
      corrSentence({ code: 'stats.corr.missing_value', count: 2, params: { column: 1 } }, names)
    );
    expect(document.body.textContent).not.toMatch(/significan/i);
    expect(document.body.textContent).not.toContain('*');
  });

  it('shows the other variants: a mean not computable, a perfect Kendall with its interval in words, a pairwise exclusion', () => {
    render(<CorrResultCard result={corrResultViewOther} names={names} />);
    expect(screen.getByTestId('cr-descriptives').textContent).toContain(corrSentence({ code: 'stats.corr.no_values' }));
    const kendall = screen.getByTestId('cr-pairs-kendall');
    expect(within(kendall).getByTestId('cr-z').textContent).toBe('-2.04');
    expect(within(kendall).getByTestId('cr-p').textContent).toBe('0.0415');
    expect(within(kendall).getByTestId('cr-interval').textContent).toBe(corrSentence({ code: 'stats.corr.interval_too_few_rows' }));
    expect(screen.getByTestId('cr-exclusions').textContent).toContain(
      corrSentence({ code: 'stats.corr.fewer_than_two_values', count: 3, params: {} })
    );
  });
});

/* ---- many variables ---------------------------------------------------------- */

/** A result with `k` variables and each coefficient's every pair computed. */
function big(k: number, coefficients: Coefficient[] = ['pearson']): CorrResultView {
  const alias = (i: number) => `C${i + 1}`;
  const pairs = (c: Coefficient): Pair[] => {
    const out: Pair[] = [];
    for (let i = 0; i < k; i++)
      for (let j = i + 1; j < k; j++)
        out.push({
          first: alias(i),
          second: alias(j),
          n: 1000,
          outcome: {
            computed: {
              estimate: ((i * 7 + j * 3) % 199) / 200 - 0.45,
              test: c === 'kendall' ? { z: 1.5 } : { t: { t: { value: 2.25 }, df: 998 } },
              p: 0.0123,
              interval: { computed: { lower: -0.1, upper: 0.2 } },
            },
          },
        });
    return out;
  };
  return {
    ...corrResultView,
    result: {
      descriptives: Array.from({ length: k }, (_, i) => ({ variable: alias(i), n: 1000, mean: { value: i }, sd: { value: 1 } })),
      coefficients: coefficients.map((coefficient) => ({ coefficient, pairs: pairs(coefficient) })),
      rowsUsed: 1000,
      exclusions: [],
      limitations: ['stats.corr.pearson_assumes_normality', 'stats.corr.two_tailed_no_partial'],
    },
  };
}

const bodyRows = (c: string) => screen.getByTestId(`cr-pairs-${c}`).querySelectorAll('tbody tr').length;

describe('CorrResultCard with many variables', () => {
  it('draws the matrix for 20 variables, a lower triangle of 190 cells, and none for 21', () => {
    render(<CorrResultCard result={big(20)} names={names} />);
    const m = screen.getByTestId('cr-matrix');
    expect(m.querySelectorAll('td[data-testid^="cr-matrix-"]')).toHaveLength(190);
    expect(screen.getByTestId('cr-matrix-C1-C20')).toBeTruthy();
    expect(screen.queryByTestId('cr-matrix-C20-C1')).toBeNull();
    cleanup();
    render(<CorrResultCard result={big(21)} names={names} />);
    expect(screen.queryByTestId('cr-matrix')).toBeNull();
    expect(bodyRows('pearson')).toBe(210);
  });

  it('shows 1,770 pairs in pages of 1,225, with Next, Previous and which pairs are shown', () => {
    render(<CorrResultCard result={big(60)} names={names} />);
    expect(screen.queryByTestId('cr-matrix')).toBeNull();
    expect(screen.getByTestId('cr-show-pearson').textContent).toBe("Pearson's r: 1,770 pairs");
    expect(bodyRows('pearson')).toBe(1225);
    expect(screen.getByTestId('cr-pairs-shown').textContent).toBe('Pairs 1 to 1,225 of 1,770 are shown.');
    expect((screen.getByTestId('cr-pairs-previous') as HTMLButtonElement).disabled).toBe(true);
    fireEvent.click(screen.getByTestId('cr-pairs-next'));
    expect(bodyRows('pearson')).toBe(545);
    expect(screen.getByTestId('cr-pairs-shown').textContent).toBe('Pairs 1,226 to 1,770 of 1,770 are shown.');
    expect((screen.getByTestId('cr-pairs-next') as HTMLButtonElement).disabled).toBe(true);
    expect(screen.getByTestId('cr-pair-C59-C60')).toBeTruthy();
    fireEvent.click(screen.getByTestId('cr-pairs-previous'));
    expect(bodyRows('pearson')).toBe(1225);
    expect(screen.getByTestId('cr-pair-C1-C2')).toBeTruthy();
    cleanup();
    // 435 pairs: one page, no Next.
    render(<CorrResultCard result={big(30)} names={names} />);
    expect(bodyRows('pearson')).toBe(435);
    expect(screen.queryByTestId('cr-pairs-next')).toBeNull();
    expect(screen.queryByTestId('cr-pairs-shown')).toBeNull();
  });

  it('measures the render at 50 variables (one page) and at 128 (the first page and the last)', () => {
    const times: Record<string, number> = {};
    let t0 = performance.now();
    render(<CorrResultCard result={big(50)} names={names} />);
    times.k50 = performance.now() - t0;
    expect(bodyRows('pearson')).toBe(1225);
    expect(screen.queryByTestId('cr-pairs-next')).toBeNull();
    cleanup();
    t0 = performance.now();
    render(<CorrResultCard result={big(128)} names={names} />);
    times.k128first = performance.now() - t0;
    expect(bodyRows('pearson')).toBe(1225);
    expect(screen.getByTestId('cr-pairs-shown').textContent).toBe('Pairs 1 to 1,225 of 8,128 are shown.');
    for (let page = 1; page < 7; page++) {
      act(() => {
        fireEvent.click(screen.getByTestId('cr-pairs-next'));
      });
    }
    t0 = performance.now();
    act(() => {
      fireEvent.click(screen.getByTestId('cr-pairs-next'));
    });
    times.k128last = performance.now() - t0;
    expect(bodyRows('pearson')).toBe(8128 - 6 * 1225);
    expect(screen.getByTestId('cr-pairs-shown').textContent).toBe('Pairs 7,351 to 8,128 of 8,128 are shown.');
    // eslint-disable-next-line no-console
    console.log(
      `CORR_RENDER_MS 50 variables (1225 pairs): ${times.k50.toFixed(0)}; 128 variables (8128 pairs): first page ${times.k128first.toFixed(0)}, last page ${times.k128last.toFixed(0)}`
    );
    expect(times.k50 + times.k128first + times.k128last).toBeLessThan(30000);
    // Its own timeout, above the 30,000 ms ceiling, so that ceiling decides rather than the
    // global 20 s (vitest.config.ts:26). Measured 5 Nov 2026 (decision 480): alone, five runs,
    // 2,119 to 2,227 ms; in the whole suite at default workers, five runs, 6,209 to 8,619 ms.
    // On a loaded Mac: 22,422 ms in 8R, which is what died at 20 s; and 90,189 ms on 5 Nov at a
    // load average near 20 with 5 GB of swap in use, its renders 12 s of that, so the ceiling held.
  }, 180000);
});
