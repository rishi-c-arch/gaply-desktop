// "and {n} more" counted from distinct (Research OS, Phase 8W, decision 567).
// The profile keeps at most 50 levels a column, its distinct counting them
// all, so the two profile tables count the rest from distinct less the twelve
// shown, and only for a column with a level list. Every column shows what it
// showed before: the twelve, and the same "and {n} more".
import React from 'react';
import { cleanup, fireEvent, render, screen, within } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import DescribeFlow from './DescribeFlow';
import { makeMockDescribeBridge } from './describeBridge';
import type { ColumnProfile, DatasetView } from './describeTypes';
import { ProfileTable } from './flowParts';

vi.mock('../../utils/isTauri', () => ({ isTauri: true, default: true }));

afterEach(cleanup);

const levels = (k: number) => Array.from({ length: k }, (_, i) => ({ label: `L${String(i).padStart(2, '0')}`, count: 1 }));
const twelve = levels(12)
  .map((l) => `${l.label} (${l.count})`)
  .join(', ');

const col = (alias: string, over: Partial<ColumnProfile>): ColumnProfile => ({
  alias,
  header: `h${alias}`,
  kind: 'categorical',
  n: 100,
  missing: 0,
  distinct: 0,
  levels: null,
  suspectMissingCodes: [],
  nonFinite: 0,
  ...over,
});

/** C1: 50 kept of 100,000; C2: 5 of 5; C3: 30 of 30; C4: 13 of 13; C5: a
 *  numeric column of distinct 15, with no list. */
const view: DatasetView = {
  slot: 1,
  displayName: 'levels.csv',
  profile: {
    rows: 100000,
    columns: [
      col('C1', { distinct: 100000, levels: levels(50) }),
      col('C2', { distinct: 5, levels: levels(5) }),
      col('C3', { distinct: 30, levels: levels(30) }),
      col('C4', { distinct: 13, levels: levels(13) }),
      col('C5', { kind: 'numeric', distinct: 15, levels: null }),
    ],
  },
  inputRef: { contentHash: '0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef', rows: 100000, columns: 5, semanticRole: 'table' },
};

const WANT: Record<string, string> = {
  C1: `${twelve}, and 99988 more`,
  C2: levels(5)
    .map((l) => `${l.label} (${l.count})`)
    .join(', '),
  C3: `${twelve}, and 18 more`,
  C4: `${twelve}, and 1 more`,
  C5: '',
};

/** Each row's Levels cell, by the row's alias, read at column `at`. */
function levelsCells(table: HTMLElement, at: number): Record<string, string> {
  const out: Record<string, string> = {};
  for (const row of within(table).getAllByRole('row').slice(1)) {
    const cells = row.querySelectorAll('td');
    const alias = cells[0].querySelector('.gds-mono')?.textContent ?? '?';
    out[alias] = cells[at].textContent ?? '';
  }
  return out;
}

describe('the profile tables count "and {n} more" from distinct', () => {
  it('ProfileTable', () => {
    render(<ProfileTable prefix="pt" view={view} />);
    expect(levelsCells(screen.getByTestId('pt-profile'), 4)).toEqual(WANT);
  });

  it('DescribeFlow, through a mocked stats_desc_open', async () => {
    const m = makeMockDescribeBridge({ stats_desc_open: view, stats_desc_close: true });
    render(<DescribeFlow bridge={m} pickPath={() => Promise.resolve('/x/levels.csv')} pollMs={1} />);
    fireEvent.click(screen.getByTestId('desc-choose'));
    const table = await screen.findByTestId('desc-profile');
    expect(levelsCells(table, 5)).toEqual(WANT);
    // Its distinct column shows the full count.
    expect(within(table).getAllByRole('row')[1].querySelectorAll('td')[4].textContent).toBe('100000');
  });
});
