// The webview's half of the Output's budget (Research OS, Phase 8R, decision 474):
// 31 alpha entries at 128 items and a correlations entry at 128 variables (decision
// 479), timed, the first render and a move through the outline, each printed. jsdom draws no pixels, so these are React's work, not a
// frame's; the real-app checklist reads the app's memory. The ceiling only catches
// an order-of-magnitude regression under a loaded Mac.
import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import React from 'react';
import { act } from 'react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { defaultRunBridges } from '../../workspace/useWorkspaceSwitch';
import { makeMockAlphaBridge } from '../alphaBridge';
import type { AlphaResultView } from '../alphaTypes';
import { makeMockDescribeBridge } from '../describeBridge';
import type { ColumnProfile, DatasetView } from '../describeTypes';
import type { CorrResultView } from '../corrTypes';
import { alphaResultView } from '../fixtures/alphaFixtures';
import { corrResultView } from '../fixtures/corrFixtures';
import { makeMockProjectBridge } from '../projects/projectBridge';
import { Output } from './Output';
import { statisticsSession } from './statisticsSession';

vi.mock('../../../utils/isTauri', () => ({ isTauri: false, default: false }));

// Unmounting 32 result tables took over 10 s, the hook's limit, on a loaded Mac (5 Nov 2026).
afterEach(() => {
  cleanup();
  statisticsSession.clear();
}, 120000);

const col = (i: number): ColumnProfile => ({ alias: `C${i + 1}`, header: `Question ${i + 1}`, kind: 'numeric', n: 9, missing: 0, distinct: 5, levels: null, suspectMissingCodes: [], nonFinite: 0 });
const view: DatasetView = {
  slot: 1,
  displayName: 'wide.csv',
  profile: { rows: 9, columns: Array.from({ length: 128 }, (_, i) => col(i)) },
  inputRef: { contentHash: 'a'.repeat(64), rows: 9, columns: 128, semanticRole: 'table' },
};
const wide = (runId: number): AlphaResultView => {
  const one = alphaResultView.result.items[0];
  return {
    ...alphaResultView,
    runId,
    result: { ...alphaResultView.result, k: 128, items: Array.from({ length: 128 }, (_, i) => ({ ...one, item: `C${i + 1}` })) },
  } as unknown as AlphaResultView;
};

/** A correlations result at 128 variables, on CorrFlow.vitest.tsx's shape: ADR-012
 *  Part 7's worst case (:541-542), since it is the result a render test timed out on. */
const corr128 = (runId: number): CorrResultView => {
  const pairRow = corrResultView.result.coefficients[0].pairs[0];
  const pairs: unknown[] = [];
  for (let i = 0; i < 128; i += 1) for (let j = i + 1; j < 128; j += 1) pairs.push({ ...pairRow, first: `C${i + 1}`, second: `C${j + 1}` });
  return {
    ...corrResultView,
    runId,
    result: {
      ...corrResultView.result,
      descriptives: Array.from({ length: 128 }, (_, i) => ({ variable: `C${i + 1}`, n: 1000, mean: { value: i }, sd: { value: 1 } })),
      coefficients: [{ coefficient: 'pearson', pairs }],
      rowsUsed: 1000,
      exclusions: [],
    },
  } as unknown as CorrResultView;
};

describe('32 entries at 128 items', () => {
  it('renders, and moves through the outline, within the ceiling', () => {
    act(() => {
      statisticsSession.setFile(view);
      for (let i = 0; i < 31; i += 1) {
        const n = statisticsSession.addEntry({
          kind: 'cronbach_alpha',
          view,
          slot: 1,
          settings: 'Items: all.',
          runId: 10 + i,
          run: { runId: 10 + i, state: 'succeeded', terminal: true, elapsedMs: 1, cancelRequested: false, stepsDone: 7, stepsTotal: 7, diagnostics: [] },
        });
        statisticsSession.updateEntry(n as number, { result: wide(10 + i) });
      }
      // The newest, so a window that has just run it draws it first (decision 479).
      const c = statisticsSession.addEntry({
        kind: 'correlations',
        view,
        slot: 1,
        settings: 'Variables: all.',
        runId: 99,
        run: { runId: 99, state: 'succeeded', terminal: true, elapsedMs: 1, cancelRequested: false, stepsDone: 7, stepsTotal: 7, diagnostics: [] },
      });
      statisticsSession.updateEntry(c as number, { result: corr128(99) });
    });
    const session = () => statisticsSession.get();
    const bridges = defaultRunBridges({ alpha: makeMockAlphaBridge() });
    const forget = makeMockDescribeBridge();
    const projects = makeMockProjectBridge();
    const save = async () => null;
    const Harness: React.FC = () => {
      const [, force] = React.useState(0);
      React.useEffect(() => statisticsSession.subscribe(() => force((x) => x + 1)), []);
      return <Output session={session()} bridges={bridges} forget={forget} projects={projects} save={save} status={null} />;
    };
    let t0 = performance.now();
    const drawn = render(<Harness />);
    const corrFirst = performance.now() - t0;
    expect(screen.getByTestId('cr-result-card')).toBeTruthy();
    drawn.unmount();
    act(() => statisticsSession.selectEntry(1));
    t0 = performance.now();
    render(<Harness />);
    const first = performance.now() - t0;
    const moves: number[] = [];
    for (let n = 2; n <= 31; n += 1) {
      const t = performance.now();
      fireEvent.click(screen.getByTestId(`sw-outline-go-${n}`));
      moves.push(performance.now() - t);
    }
    t0 = performance.now();
    fireEvent.click(screen.getByTestId('sw-outline-go-32'));
    const corrMove = performance.now() - t0;
    moves.sort((a, b) => a - b);
    const mounted = document.querySelectorAll('[data-testid="al-result-card"], [data-testid="cr-result-card"]').length;
    process.stdout.write(
      `OUTPUT_RENDER first=${first.toFixed(1)}ms move_median=${moves[15].toFixed(1)}ms move_worst=${moves[29].toFixed(1)}ms corr_first=${corrFirst.toFixed(1)}ms corr_move=${corrMove.toFixed(1)}ms mounted=${mounted}\n`
    );
    expect(mounted).toBe(32);
    expect(screen.getByTestId('sw-entry-32').hidden).toBe(false);
    expect(first).toBeLessThan(20000);
  }, 300000);
});
