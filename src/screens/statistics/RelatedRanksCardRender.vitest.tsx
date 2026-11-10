// The 50-column Friedman result, timed (Research OS, Phase 8Z-2, decision 653): the card's
// first render, printed on a line as RanksCardRender.vitest.tsx prints its time. Fifty is
// the most columns a run takes (MAX_RELATED_COLUMNS). jsdom draws no pixels, so this is
// React's work, not a frame's. The ceiling, 1,600 ms, is ten times the slowest render
// measured under load (153.0 ms at default workers in 8Z-2's final run, load 4.17 8.60 11.82),
// rounded up.
import React from 'react';
import { cleanup, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';

import { relatedRanksResultViewOther } from './fixtures/relatedRanksFixtures';
import type { RelatedRanksResultView } from './relatedRanksTypes';
import { RelatedRanksCard } from './RelatedRanksCard';

afterEach(cleanup, 120000);

const FIFTY = 50;
const RENDER_CEILING_MS = 1600;

const result50: RelatedRanksResultView = (() => {
  const base = relatedRanksResultViewOther as unknown as RelatedRanksResultView;
  const aliases = Array.from({ length: FIFTY }, (_, i) => `C${i + 1}`);
  return {
    ...base,
    result: {
      ...base.result,
      columns: aliases.map((column, i) => ({ column, median: i / 3 })),
      rowsUsed: 300,
      test: {
        friedman: {
          columns: aliases.map((column, i) => ({ column, rankSum: 7000.5 + i, meanRank: 23.335 + i / 300 })),
          q: { value: 66.5 },
          df: 49,
          p: { value: 0.0482 },
          kendallsW: { value: 0.0045 },
        },
      },
    },
  };
})();

describe('a 50-column Friedman test', () => {
  it('renders within the ceiling', () => {
    const t0 = performance.now();
    render(<RelatedRanksCard result={result50} names={{ column: (a) => `C${a}` }} />);
    const first = performance.now() - t0;
    expect(document.querySelectorAll('[data-testid^="rr-col-C"]').length).toBe(FIFTY);
    expect(screen.getByTestId('rr-col-C50').querySelectorAll('td')[0].textContent).not.toBe('');
    process.stdout.write(`RELATED_RANKS_RENDER first=${first.toFixed(1)}ms columns=${FIFTY}\n`);
    expect(first).toBeLessThan(RENDER_CEILING_MS);
  }, 120000);
});
