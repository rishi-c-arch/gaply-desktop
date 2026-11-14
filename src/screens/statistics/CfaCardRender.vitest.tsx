// A confirmatory factor analysis of 128 columns on 16 correlated factors, timed (Research OS,
// Phase 9C-3, decision 782): the card's first render, printed on a line as
// FactorCardRender.vitest.tsx prints its time. 128 columns and 16 factors are the most a model
// takes (MAX_ITEMS, MAX_FACTORS), so it has the most rows every table can: 128 weights, 120
// covariances, 16 + 128 variances. jsdom draws no pixels, so this is React's work, not a frame's.
// The ceiling, 5,700 ms, is ten times the slowest render measured under load, rounded up:
// 563.3 ms in the whole suite at default workers (load 3.19 before, 18.07 after); 315.6 ms with
// Part 1's files (load 3.32); 252.3 ms in the whole suite at two workers (load 2.47 before, 4.66
// after) (14 Nov 2026, ~/gaply-corpus/os/phase9/9c3/final_vitest_default.log and the logs beside it).
import React from 'react';
import { cleanup, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';

import { cfaResultView } from './fixtures/cfaFixtures';
import type { CfaResultView, CfaStat } from './cfaTypes';
import { CfaCard } from './CfaCard';
import { MAX_FACTORS, MAX_ITEMS } from './cfaWording';

afterEach(cleanup, 120000);

const P = 128;
const K = 16;
const RENDER_CEILING_MS = 5700;

/** A wire shape of P columns on K correlated factors, built from the fixture's shapes, numbers
 *  varied so no two cells repeat. */
function wideResult(p: number, k: number): CfaResultView {
  const base = cfaResultView as unknown as CfaResultView;
  const x = (i: number): CfaStat => ({ value: Math.sin(i * 31 + 1) * 0.9 });
  const items = Array.from({ length: p }, (_, i) => `C${i + 1}`);
  const indicators = Array.from({ length: k }, (_, f) => items.filter((_, i) => i % k === f));
  const flat = indicators.flat();
  const factorOf = (a: string) => indicators.findIndex((f) => f.includes(a)) + 1;
  const pairs: Array<[number, number]> = [];
  for (let f = 1; f <= k; f += 1) for (let g = f + 1; g <= k; g += 1) pairs.push([f, g]);
  return {
    ...base,
    result: {
      ...base.result,
      items: flat,
      rowsUsed: 2000,
      indicators,
      loadings: flat.map((item, i) => ({ factor: factorOf(item), item, estimate: x(i), se: x(i + 1), cr: x(i + 2), p: x(i + 3), standardized: x(i + 4) })),
      factorVariances: Array.from({ length: k }, (_, f) => ({ factor: f + 1, estimate: x(f), se: x(f + 1), cr: x(f + 2), p: x(f + 3) })),
      factorCovariances: pairs.map((fg, i) => ({ factors: fg, estimate: x(i), se: x(i + 1), cr: x(i + 2), p: x(i + 3), correlation: x(i + 4) })),
      errorVariances: flat.map((item, i) => ({ item, estimate: x(i), se: x(i + 1), cr: x(i + 2), p: x(i + 3) })),
      smc: flat.map((item, i) => ({ item, value: x(i) })),
    },
  };
}

describe('a confirmatory factor analysis of 128 columns on 16 correlated factors', () => {
  it('renders within the ceiling', () => {
    expect([MAX_ITEMS, MAX_FACTORS]).toEqual([P, K]);
    const result = wideResult(P, K);
    const t0 = performance.now();
    render(<CfaCard result={result} names={{ column: (a) => `C${a}` }} />);
    const first = performance.now() - t0;
    expect(document.querySelectorAll('[data-testid^="cf-weight-C"]').length).toBe(P);
    expect(document.querySelectorAll('[data-testid^="cf-covariance-"]').length).toBe((K * (K - 1)) / 2);
    expect(document.querySelectorAll('[data-testid^="cf-variance-"]').length).toBe(K + P);
    expect(screen.getByTestId('cf-weight-C128').querySelectorAll('td')).toHaveLength(4);
    process.stdout.write(`CFA_RENDER first=${first.toFixed(1)}ms columns=${P} factors=${K}\n`);
    expect(first).toBeLessThan(RENDER_CEILING_MS);
  }, 120000);
});
