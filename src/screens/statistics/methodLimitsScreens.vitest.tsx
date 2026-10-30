// The limits line and the open's progress line on the four methods' screens
// (Research OS, Phase 8F-2, decisions 289 and 290): the summary's, the
// independent t-test's, the one-sample t-test's and the paired t-test's.
//
// * The limits line is read once per screen through its `limits` prop, shown
//   from each tier's wire fixture, with the memory sentence when memoryKnown
//   is false, and not shown for a refusal, a rejection, null or anything
//   without MethodLimitsView's shape.
// * The progress line shows while an open is in flight and a count has
//   arrived, ignores a count after the open has settled, and goes when the
//   open returns and when it is refused.
// * Both lines' words pass the neutral-word check.
import React from 'react';
import { act, cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import DescribeFlow from './DescribeFlow';
import TTestFlow from './TTestFlow';
import OneSampleFlow from './OneSampleFlow';
import PairedFlow from './PairedFlow';
import { makeMockDescribeBridge } from './describeBridge';
import { makeMockTTestBridge } from './ttestBridge';
import { makeMockOneSampleBridge, makeMockPairedBridge } from './onesampleBridge';
import { limitsSentence, openProgressSentence } from './flowParts';
import type { DatasetView, MethodLimitsView } from './describeTypes';
import { datasetView } from './fixtures/describeFixtures';
import limits8 from './fixtures/methodLimitsView8gb.json';
import limits16 from './fixtures/methodLimitsView16gb.json';
import limits32 from './fixtures/methodLimitsView32gb.json';

vi.mock('../../design-system/GaplyGlobe', () => ({ GaplyGlobe: () => <div /> }));
vi.mock('../../utils/isTauri', () => ({ isTauri: true, default: true }));

afterEach(cleanup);

type Open = (path: string, onRowsRead?: (rows: number) => void) => Promise<DatasetView>;

interface Screen {
  name: string;
  prefix: string;
  render: (limits: () => Promise<unknown>, open?: Open) => void;
}

const PATH = '/Users/someone/PATH_SENTINEL_m8/data.csv';
const RESPONSES = { stats_desc_open: datasetView, stats_desc_close: true };

/** A mock bridge, with its open replaced by `open` when one is given. */
function withOpen<B extends { open: Open }>(bridge: B, open?: Open): B {
  if (open) bridge.open = open;
  return bridge;
}

const SCREENS: Screen[] = [
  {
    name: 'the summary',
    prefix: 'desc',
    render: (limits, open) =>
      render(<DescribeFlow bridge={withOpen(makeMockDescribeBridge(RESPONSES), open)} pickPath={async () => PATH} limits={limits} />),
  },
  {
    name: 'the independent t-test',
    prefix: 'tt',
    render: (limits, open) =>
      render(<TTestFlow bridge={withOpen(makeMockTTestBridge(RESPONSES), open)} pickPath={async () => PATH} limits={limits} />),
  },
  {
    name: 'the one-sample t-test',
    prefix: 'os',
    render: (limits, open) =>
      render(<OneSampleFlow bridge={withOpen(makeMockOneSampleBridge(RESPONSES), open)} pickPath={async () => PATH} limits={limits} />),
  },
  {
    name: 'the paired t-test',
    prefix: 'pt',
    render: (limits, open) =>
      render(<PairedFlow bridge={withOpen(makeMockPairedBridge(RESPONSES), open)} pickPath={async () => PATH} limits={limits} />),
  },
];

const known = (v: unknown, memoryKnown: boolean) => ({ ...(v as MethodLimitsView), memoryKnown });

const EIGHT =
  'On this computer, Gaply opens a file of up to 100000 rows, 128 columns and 5000000 cells (rows times columns), and of up to 64 MB. A file near these limits takes a few seconds to open.';
const SIXTEEN =
  'On this computer, Gaply opens a file of up to 100000 rows, 128 columns and 12800000 cells (rows times columns), and of up to 96 MB. A file near these limits takes a few seconds to open.';
const MEMORY = " Gaply could not read this computer's memory, so it uses its smallest limits.";

const PROGRESS = (n: number) =>
  `Opening the file: ${n} rows read. After the last row, Gaply builds the table and its profile, which takes a few more seconds.`;

describe.each(SCREENS)('$name: the limits line', ({ prefix, render: show }) => {
  it.each([
    ['8gb, memory known', known(limits8, true), EIGHT],
    ['16gb', known(limits16, true), SIXTEEN],
    ['32gb', known(limits32, true), SIXTEEN],
    ['8gb, memory not known', known(limits8, false), EIGHT + MEMORY],
  ])('shows %s from its view', async (_label, view, words) => {
    const limits = vi.fn().mockResolvedValue(view);
    show(limits);
    expect((await screen.findByTestId(`${prefix}-limits`)).textContent).toBe(words);
    expect(limits).toHaveBeenCalledTimes(1);
  });

  it.each([
    ['a refusal', () => Promise.reject(new Error('{"code":"workspace.inactive"}'))],
    ['null', () => Promise.resolve(null)],
    ['a refusal as an answer', () => Promise.resolve({ code: 'workspace.inactive' })],
    ['a view with a field missing', () => Promise.resolve({ ...(limits8 as object), maxCells: undefined })],
    ['a view with a word for a number', () => Promise.resolve({ ...(limits8 as object), maxRows: '100000' })],
    ['a throw', () => {
      throw new Error('not in the desktop app');
    }],
  ])('shows no line for %s', async (_label, limits) => {
    show(limits as () => Promise<unknown>);
    expect(await screen.findByTestId(`${prefix}-choose`)).toBeTruthy();
    await act(async () => {
      await new Promise((r) => setTimeout(r, 20));
    });
    expect(screen.queryByTestId(`${prefix}-limits`)).toBeNull();
  });
});

describe.each(SCREENS)('$name: the open\'s progress line', ({ prefix, render: show }) => {
  /** An open that reports `rows` and stays pending until settled. */
  function pending(rows: number) {
    let settle: (ok: boolean) => void = () => undefined;
    let report: (n: number) => void = () => undefined;
    const open: Open = (_path, onRowsRead) => {
      report = (n) => onRowsRead?.(n);
      report(rows);
      return new Promise<DatasetView>((resolve, reject) => {
        settle = (ok) => (ok ? resolve(datasetView) : reject(new Error('{"code":"malformed"}')));
      });
    };
    return { open, settle: (ok: boolean) => settle(ok), report: (n: number) => report(n) };
  }
  const noLimits = () => Promise.resolve(null);

  it('shows the rows read while the open is in flight, and goes when it returns', async () => {
    const o = pending(62000);
    show(noLimits, o.open);
    fireEvent.click(screen.getByTestId(`${prefix}-choose`));
    expect((await screen.findByTestId(`${prefix}-open-progress`)).textContent).toBe(PROGRESS(62000));
    act(() => o.report(64000));
    expect(screen.getByTestId(`${prefix}-open-progress`).textContent).toBe(PROGRESS(64000));
    await act(async () => o.settle(true));
    await waitFor(() => expect(screen.queryByTestId(`${prefix}-open-progress`)).toBeNull());
    // A count after the open has settled is ignored.
    act(() => o.report(99000));
    expect(screen.queryByTestId(`${prefix}-open-progress`)).toBeNull();
  });

  it('goes when the open is refused', async () => {
    const o = pending(3000);
    show(noLimits, o.open);
    fireEvent.click(screen.getByTestId(`${prefix}-choose`));
    expect((await screen.findByTestId(`${prefix}-open-progress`)).textContent).toBe(PROGRESS(3000));
    await act(async () => o.settle(false));
    await waitFor(() => expect(screen.queryByTestId(`${prefix}-open-progress`)).toBeNull());
    act(() => o.report(4000));
    expect(screen.queryByTestId(`${prefix}-open-progress`)).toBeNull();
  });

  it('shows no line for an open that reports no count', async () => {
    show(noLimits);
    fireEvent.click(screen.getByTestId(`${prefix}-choose`));
    await waitFor(() => expect(screen.queryByTestId(`${prefix}-choose`)).toBeNull());
    expect(screen.queryByTestId(`${prefix}-open-progress`)).toBeNull();
  });
});

describe('the two lines are neutral (decision 66), and hold no path', () => {
  it('no sentence says "significant", carries a star, or passes a verdict', () => {
    const all = [
      limitsSentence(known(limits8, true)),
      limitsSentence(known(limits8, false)),
      limitsSentence(known(limits16, true)),
      openProgressSentence(62000),
    ];
    const bad = all.filter(
      (s) =>
        /significan/i.test(s) ||
        /[*★☆]/.test(s) ||
        /\breject|\bconfirm(s|ed)? that|\bproves?\b|null hypothesis|\b(t-)?test (passed|failed)\b|\bfail(s|ed)? the test\b/i.test(s)
    );
    expect(bad).toEqual([]);
    expect(all.join(' ')).not.toContain('PATH_SENTINEL');
  });
});
