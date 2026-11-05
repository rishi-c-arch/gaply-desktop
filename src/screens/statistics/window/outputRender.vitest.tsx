// The webview's half of the Output's budget (Research OS, Phase 8R, decision 474):
// 32 alpha entries at 128 items, timed, the first render and a move through the
// outline, each printed. jsdom draws no pixels, so these are React's work, not a
// frame's; the real-app checklist reads the app's memory. The ceiling only catches
// an order-of-magnitude regression under a loaded Mac.
import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import React from 'react';
import { act } from 'react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { makeMockAlphaBridge } from '../alphaBridge';
import type { AlphaResultView } from '../alphaTypes';
import { makeMockDescribeBridge } from '../describeBridge';
import type { ColumnProfile, DatasetView } from '../describeTypes';
import { alphaResultView } from '../fixtures/alphaFixtures';
import { makeMockProjectBridge } from '../projects/projectBridge';
import { Output } from './Output';
import { statisticsSession } from './statisticsSession';

vi.mock('../../../utils/isTauri', () => ({ isTauri: false, default: false }));

afterEach(() => {
  cleanup();
  statisticsSession.clear();
});

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

describe('32 entries at 128 items', () => {
  it('renders, and moves through the outline, within the ceiling', () => {
    act(() => {
      statisticsSession.setFile(view);
      for (let i = 0; i < 32; i += 1) {
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
    });
    const session = () => statisticsSession.get();
    const alpha = makeMockAlphaBridge();
    const forget = makeMockDescribeBridge();
    const projects = makeMockProjectBridge();
    const save = async () => null;
    const Harness: React.FC = () => {
      const [, force] = React.useState(0);
      React.useEffect(() => statisticsSession.subscribe(() => force((x) => x + 1)), []);
      return <Output session={session()} bridge={alpha} forget={forget} projects={projects} save={save} status={null} />;
    };
    const t0 = performance.now();
    render(<Harness />);
    const first = performance.now() - t0;
    const moves: number[] = [];
    for (let n = 1; n <= 32; n += 1) {
      const t = performance.now();
      fireEvent.click(screen.getByTestId(`sw-outline-go-${n}`));
      moves.push(performance.now() - t);
    }
    moves.sort((a, b) => a - b);
    const mounted = document.querySelectorAll('[data-testid="al-result-card"]').length;
    process.stdout.write(`OUTPUT_RENDER first=${first.toFixed(1)}ms move_median=${moves[16].toFixed(1)}ms move_worst=${moves[31].toFixed(1)}ms mounted=${mounted}\n`);
    expect(mounted).toBe(32);
    expect(screen.getByTestId('sw-entry-32').hidden).toBe(false);
    expect(first).toBeLessThan(20000);
  }, 120000);
});
