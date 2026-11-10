// The 50-group rank test, timed (Research OS, Phase 8Y-2, decision 619): the card's
// first render, printed on a line as XtabCardRender.vitest.tsx prints its times. Fifty
// is the most groups a run takes (MAX_GROUP_LEVELS). jsdom draws no pixels, so this is
// React's work, not a frame's. The ceiling, 10 s, only catches an order-of-magnitude
// regression on a loaded Mac.
import React from 'react';
import { cleanup, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';

import { ranksResultViewOther } from './fixtures/ranksFixtures';
import type { RanksResultView } from './ranksTypes';
import { RanksCard } from './RanksCard';

afterEach(cleanup, 120000);

const FIFTY = 50;

const result50: RanksResultView = (() => {
  const base = ranksResultViewOther as unknown as RanksResultView;
  const groups = Array.from({ length: FIFTY }, (_, level) => ({ level, label: `level ${level}`, n: 40 + level, median: level / 3, rankSum: 1000.5 + level, meanRank: 20.25 + level }));
  return {
    ...base,
    result: {
      ...base.result,
      groups,
      n: 3225,
      test: { kruskal_wallis: { h: { value: 66.5 }, df: 49, p: { value: 0.0482 }, epsilonSquared: { value: 0.02 } } },
    },
  };
})();

describe('a 50-group rank test', () => {
  it('renders within the ceiling', () => {
    const t0 = performance.now();
    render(<RanksCard result={result50} names={{ column: (a) => `C${a}`, level: (i) => `level ${i + 1}` }} />);
    const first = performance.now() - t0;
    expect(document.querySelectorAll('[data-testid^="rk-group-label-"]').length).toBe(FIFTY);
    expect(screen.getByTestId('rk-group-49').querySelectorAll('td')[0].textContent).toBe('89');
    process.stdout.write(`RANKS_RENDER first=${first.toFixed(1)}ms groups=${FIFTY}\n`);
    expect(first).toBeLessThan(10000);
  }, 120000);
});
