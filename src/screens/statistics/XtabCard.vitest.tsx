// Crosstabs' result card (Research OS, Phase 8X-2, decisions 588 and 589), on both
// result fixtures Rust writes (src-tauri/tests/stats_xtab_wire.rs). The table's cells
// are found by (row, column), never by position: xtabResultViewOther.json is a
// wire-shape fixture, three row levels and frequencies for rows 0 and 1 only, and it
// is drawn here as written and with its frequencies reversed. One cell statistic shows
// at a time, chosen by aria-pressed buttons; the continuity correction and Fisher's
// test show for a 2 x 2 only; a not-computable value shows its sentence; the expected
// counts' line always shows; a level's label is text only, never in an attribute.
import React from 'react';
import { cleanup, fireEvent, render, screen, within } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { xtabResultView, xtabResultViewOther } from './fixtures/xtabFixtures';
import type { XtabResultView } from './xtabTypes';
import { XtabCard } from './XtabCard';
import { CELL_STATISTICS, xtabSentence } from './xtabWording';

afterEach(cleanup);

const NAMES = { column: (a: number) => `"col${a}"` };
const two = xtabResultView as unknown as XtabResultView;
const other = xtabResultViewOther as unknown as XtabResultView;
const reversed: XtabResultView = { ...other, result: { ...other.result, frequencies: [...other.result.frequencies].reverse() } };

const cell = (r: number, c: number) => screen.getByTestId(`xt-cell-${r}-${c}`).textContent;
const grid = (rows: number, cols: number) => Array.from({ length: rows }, (_, r) => Array.from({ length: cols }, (_, c) => cell(r, c)));

describe('the table', () => {
  it('draws the 2 x 2 by level, its totals and n', () => {
    render(<XtabCard result={two} names={NAMES} />);
    const t = screen.getByTestId('xt-table');
    expect(within(t).getAllByRole('rowheader').map((h) => h.textContent)).toEqual(['no', 'yes', 'Total']);
    expect(within(t).getAllByRole('columnheader').map((h) => h.textContent)).toEqual(['a', 'b', 'Total']);
    expect(grid(2, 2)).toEqual([
      ['5', '3'],
      ['4', '4'],
    ]);
    expect([0, 1].map((r) => screen.getByTestId(`xt-row-total-${r}`).textContent)).toEqual(['8', '8']);
    expect([0, 1].map((c) => screen.getByTestId(`xt-column-total-${c}`).textContent)).toEqual(['9', '7']);
    expect(screen.getByTestId('xt-n').textContent).toBe('16');
  });

  it.each([
    ['as written', other],
    ['with its frequencies reversed', reversed],
  ])('finds each cell of xtabResultViewOther.json by (row, column), %s, and leaves a missing one empty', (_how, result) => {
    expect(() => render(<XtabCard result={result} names={NAMES} />)).not.toThrow();
    expect(grid(3, 2)).toEqual([
      ['5', '3'],
      ['4', '4'],
      ['', ''],
    ]);
    // Three row levels and two row totals: the third total is empty.
    expect([0, 1, 2].map((r) => screen.getByTestId(`xt-row-total-${r}`).textContent)).toEqual(['8', '8', '']);
  });
});

describe('the cell statistic switch', () => {
  const pressed = () =>
    CELL_STATISTICS.filter((c) => screen.getByTestId(`xt-show-${c.stat}`).getAttribute('aria-pressed') === 'true').map((c) => c.stat);

  it('shows one statistic at a time, the count first, in decision 589\'s words', () => {
    render(<XtabCard result={two} names={NAMES} />);
    const group = screen.getByRole('group', { name: 'Which cell statistic to show' });
    expect(within(group).getAllByRole('button').map((b) => b.textContent)).toEqual(CELL_STATISTICS.map((c) => c.label));
    expect(pressed()).toEqual(['count']);
  });

  it.each([
    ['expected', [['4.5', '3.5'], ['4.5', '3.5']]],
    ['row_percent', [['62.5', '62.5'], ['62.5', '62.5']]],
    ['column_percent', [['50', '50'], ['50', '50']]],
    ['total_percent', [['25', '25'], ['25', '25']]],
    ['residual', [['0.5', '-0.5'], ['-0.5', '0.5']]],
    ['pearson_residual', [['0.25', '0.25'], ['0.25', '0.25']]],
    ['adjusted_residual', [['-1.5', '-1.5'], ['-1.5', '-1.5']]],
  ])('%s: one number a cell, the percentages as the proportions times 100', (stat, want) => {
    render(<XtabCard result={two} names={NAMES} />);
    fireEvent.click(screen.getByTestId(`xt-show-${stat}`));
    expect(pressed()).toEqual([stat]);
    expect(grid(2, 2)).toEqual(want);
    // The totals stay counts.
    expect(screen.getByTestId('xt-n').textContent).toBe('16');
  });

  it('names the statistic shown in the caption', () => {
    render(<XtabCard result={two} names={NAMES} />);
    fireEvent.click(screen.getByTestId('xt-show-pearson_residual'));
    expect(screen.getByTestId('xt-table').querySelector('caption')?.textContent).toContain('Pearson residual');
  });
});

describe('the tests and measures', () => {
  it('give the 2 x 2 every row, each with its value, df and p, and Fisher\'s three p', () => {
    render(<XtabCard result={two} names={NAMES} />);
    const row = (id: string) => Array.from(screen.getByTestId(id).querySelectorAll('th, td')).map((c) => c.textContent);
    expect(row('xt-test-pearson')).toEqual(['Pearson chi-square', '0.254', '1', '0.614']);
    expect(row('xt-test-likelihood')).toEqual(['Likelihood ratio', '0.255', '1', '0.613']);
    expect(row('xt-test-continuity')).toEqual(['Continuity correction', '0', '1', '1']);
    expect(row('xt-test-fisher-two-sided')).toEqual(["Fisher's exact test (two-sided)", '', '', '1']);
    expect(row('xt-test-fisher-less')).toEqual(["Fisher's exact test (less)", '', '', '0.807']);
    expect(row('xt-test-fisher-greater')).toEqual(["Fisher's exact test (greater)", '', '', '0.5']);
    expect(screen.getByTestId('xt-phi').textContent).toBe('0.126');
    expect(screen.getByTestId('xt-cramers-v').textContent).toBe('0.126');
    expect(screen.getByTestId('xt-contingency').textContent).toBe('0.125');
  });

  it('draws no continuity correction and no Fisher row for a larger table', () => {
    render(<XtabCard result={other} names={NAMES} />);
    for (const id of ['xt-test-continuity', 'xt-test-fisher-two-sided', 'xt-test-fisher-less', 'xt-test-fisher-greater']) {
      expect(screen.queryByTestId(id), id).toBeNull();
    }
    expect(screen.getByTestId('xt-test-likelihood')).toBeTruthy();
  });

  it("shows a not-computable value's sentence where its number would be", () => {
    render(<XtabCard result={other} names={NAMES} />);
    const nc = xtabSentence({ code: 'stats.xtab.arithmetic_refused' });
    expect(screen.getByTestId('xt-test-pearson-not-computable').textContent).toBe(nc);
    for (const id of ['xt-phi', 'xt-cramers-v', 'xt-contingency']) expect(screen.getByTestId(id).textContent, id).toBe(nc);
  });
});

describe('the lines below the table', () => {
  it("shows the expected counts' line always, 0 included, and the note when there is one", () => {
    render(<XtabCard result={two} names={NAMES} />);
    expect(screen.getByTestId('xt-cells-below-five').textContent).toBe('4 of 4 cells have an expected count below 5; the smallest is 3.5.');
    expect(screen.getByTestId('xt-notes').textContent).toBe(xtabSentence({ code: 'stats.xtab.expected_below_five' }));
    cleanup();
    render(<XtabCard result={other} names={NAMES} />);
    expect(screen.getByTestId('xt-cells-below-five').textContent).toBe('0 of 4 cells have an expected count below 5; the smallest is 3.5.');
    expect(screen.queryByTestId('xt-notes')).toBeNull();
  });

  it('lists every exclusion, missing_code twice, each with its own key', () => {
    const errors = vi.spyOn(console, 'error').mockImplementation(() => undefined);
    try {
      render(<XtabCard result={two} names={NAMES} />);
      const items = within(screen.getByTestId('xt-exclusions')).getAllByRole('listitem').map((li) => li.textContent);
      expect(items).toEqual([
        `2 rows had no value (an empty cell or a missing-value token) in "col1", the rows' column, and were left out.`,
        '1 row held a missing code you ticked in "col1", and was left out.',
        `1 row had no value (an empty cell or a missing-value token) in "col2", the columns' column, and was left out.`,
        '3 rows held a missing code you ticked in "col2", and were left out.',
      ]);
      expect(errors.mock.calls.filter((c) => String(c[0]).includes('same key'))).toEqual([]);
    } finally {
      errors.mockRestore();
    }
  });

  it('lists the limitations, the digits note and the manifest', () => {
    render(<XtabCard result={two} names={NAMES} />);
    expect(within(screen.getByTestId('xt-limitations')).getAllByRole('listitem')).toHaveLength(5);
    expect(screen.getByTestId('xt-digits-note').textContent).toMatch(/six figures/);
    expect(screen.getByTestId('xt-manifest-capability').textContent).toBe('stats.analyze.crosstabs, version 1');
    expect(screen.getByTestId('xt-result-notes').textContent).toContain('may be a code for a missing value');
  });
});

describe("a level's label is text only", () => {
  it('a sentinel shows in the headers and in no attribute', () => {
    const LVL = 'LVL_SENTINEL_x82';
    const result: XtabResultView = {
      ...two,
      result: {
        ...two.result,
        rowLevels: two.result.rowLevels.map((l) => ({ ...l, label: `${LVL}r${l.level}` })),
        columnLevels: two.result.columnLevels.map((l) => ({ ...l, label: `${LVL}c${l.level}` })),
      },
    };
    const { container } = render(<XtabCard result={result} names={NAMES} />);
    for (const stat of CELL_STATISTICS) {
      fireEvent.click(screen.getByTestId(`xt-show-${stat.stat}`));
      const attrs = Array.from(container.querySelectorAll('*')).flatMap((el) => Array.from(el.attributes).map((a) => `${a.name}=${a.value}`));
      expect(attrs.filter((a) => a.includes(LVL))).toEqual([]);
    }
    const walker = document.createTreeWalker(container, NodeFilter.SHOW_TEXT);
    const owners: string[] = [];
    for (let n = walker.nextNode(); n; n = walker.nextNode()) {
      if (n.textContent?.includes(LVL)) owners.push(n.parentElement?.closest('[data-testid]')?.getAttribute('data-testid') ?? '?');
    }
    expect(owners.sort()).toEqual(['xt-column-label-0', 'xt-column-label-1', 'xt-row-label-0', 'xt-row-label-1']);
  });
});
