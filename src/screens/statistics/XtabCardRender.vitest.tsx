// The 50 x 50 crosstab, timed (Research OS, Phase 8X-2, decision 588): the card's
// first render and one move of the cell statistic switch, each printed on a line as
// window/outputRender.vitest.tsx:123 prints its times. A 50 x 50 table is the largest
// a crosstab can be (decision 575: at most MAX_GROUP_LEVELS levels in each column).
// jsdom draws no pixels, so these are React's work, not a frame's. The ceiling, 10 s
// each, only catches an order-of-magnitude regression on a loaded Mac.
import React from 'react';
import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';

import { xtabResultView } from './fixtures/xtabFixtures';
import type { XtabResultView } from './xtabTypes';
import { XtabCard } from './XtabCard';

afterEach(cleanup, 120000);

const SIDE = 50;

const table50: XtabResultView = (() => {
  const base = xtabResultView as unknown as XtabResultView;
  const f = base.result.frequencies[0];
  const frequencies = [];
  for (let row = 0; row < SIDE; row += 1) {
    for (let column = 0; column < SIDE; column += 1) frequencies.push({ ...f, row, column, observed: 40 + row + column, expected: 40 + (row + column) / 3 });
  }
  const levels = Array.from({ length: SIDE }, (_, level) => ({ level, label: `level ${level}` }));
  return {
    ...base,
    result: {
      ...base.result,
      rowLevels: levels,
      columnLevels: levels,
      frequencies,
      rowTotals: Array.from({ length: SIDE }, (_, i) => 2000 + i),
      columnTotals: Array.from({ length: SIDE }, (_, i) => 2000 + i),
      n: 100000,
      continuityCorrected: null,
      fisher: null,
      cellCount: SIDE * SIDE,
    },
  };
})();

describe('a 50 x 50 crosstab', () => {
  it('renders, and moves the switch, within the ceiling', () => {
    let t0 = performance.now();
    render(<XtabCard result={table50} names={{ column: (a) => `C${a}` }} />);
    const first = performance.now() - t0;
    expect(screen.getByTestId('xt-cell-49-49').textContent).toBe('138');
    t0 = performance.now();
    fireEvent.click(screen.getByTestId('xt-show-expected'));
    const move = performance.now() - t0;
    expect(screen.getByTestId('xt-cell-49-49').textContent).toBe('72.6667');
    process.stdout.write(`XTAB_RENDER first=${first.toFixed(1)}ms switch=${move.toFixed(1)}ms cells=${document.querySelectorAll('[data-testid^="xt-cell-"]').length}\n`);
    expect(document.querySelectorAll('[data-testid^="xt-cell-"]').length).toBe(SIDE * SIDE);
    expect(first).toBeLessThan(10000);
    expect(move).toBeLessThan(10000);
  }, 120000);
});
