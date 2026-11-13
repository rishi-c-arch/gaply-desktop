// A 128-item factor analysis with 16 factors and promax, timed (Research OS, Phase 9B-3,
// decision 729): the card's first render, printed on a line as LinearRegressionCardRender.
// vitest.tsx prints its time. 128 is the most items a run takes (MAX_ITEMS). jsdom draws no
// pixels, so this is React's work, not a frame's. The ceiling, 10,300 ms, is ten times the
// slowest render measured under load, rounded up: 1,025.2 ms in the whole suite at default
// workers (load 16.12); 725.3 ms with Part 1's six new files (load 6.11); 671.4 ms in the
// statistics and workspace folders (load 20.56); 351.4 ms in the whole suite at two workers
// (13 Nov 2026, ~/gaply-corpus/os/phase9/9b3/final_vitest_default.log and the logs beside it). The widest
// result the spec allows, 128 items and 128 components with promax, drew in 1,422.0 ms once,
// by a probe never committed.
import React from 'react';
import { cleanup, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';

import { factorResultView } from './fixtures/factorFixtures';
import type { FactorResultView, FactorStat, FactorSums } from './factorTypes';
import { FactorCard } from './FactorCard';
import { MAX_ITEMS } from './factorWording';

afterEach(cleanup, 120000);

const P = 128;
const M = 16;
const RENDER_CEILING_MS = 10300;

/** A wire shape of P items and M factors, numbers varied so no two cells repeat. */
function wideResult(p: number, m: number): FactorResultView {
  const base = factorResultView as unknown as FactorResultView;
  const items = Array.from({ length: p }, (_, i) => `C${i + 1}`);
  const x = (i: number, j: number): FactorStat => ({ value: Math.sin(i * 31 + j * 7 + 1) * 0.9 });
  const sums = (k: number): FactorSums[] => Array.from({ length: k }, (_, j) => ({ total: { value: p / (j + 2) }, percent: { value: 100 / (j + 2) }, cumulative: { value: 50 + j / 3 } }));
  const matrix = (rows: number) => Array.from({ length: rows }, (_, i) => Array.from({ length: m }, (_, j) => x(i, j)));
  return {
    ...base,
    result: {
      ...base.result,
      items,
      rowsUsed: 2000,
      descriptives: items.map((item, i) => ({ item, mean: { value: i / 3 }, sd: { value: 1 + i / 9 } })),
      kmo: { overall: { value: 0.8 }, items: items.map((item, i) => ({ item, msa: { value: 0.5 + i / 400 } })) },
      bartlett: { ...base.result.bartlett, df: (p * (p - 1)) / 2 },
      eigenvalues: sums(p),
      extraction: {
        ...base.result.extraction,
        factors: m,
        communalities: items.map((item, i) => ({ item, initial: { value: 0.3 + i / 500 }, extraction: { value: 0.4 + i / 500 } })),
        sums: sums(m),
        loadings: matrix(p),
      },
      rotation: { method: 'promax', iterations: 7, loadings: matrix(p), transformation: null, sums: null, structure: matrix(p), factorCorrelations: matrix(m) },
    },
  };
}

describe('a 128-item factor analysis with 16 factors and promax', () => {
  it('renders within the ceiling', () => {
    expect(MAX_ITEMS).toBe(P);
    const result = wideResult(P, M);
    const t0 = performance.now();
    render(<FactorCard result={result} names={{ column: (a) => `C${a}` }} />);
    const first = performance.now() - t0;
    expect(document.querySelectorAll('[data-testid^="fa-pattern-C"]').length).toBe(P);
    expect(document.querySelectorAll('[data-testid^="fa-structure-C"]').length).toBe(P);
    expect(document.querySelectorAll('[data-testid^="fa-variance-"]').length).toBe(P);
    expect(screen.getByTestId('fa-pattern-C128').querySelectorAll('td')).toHaveLength(M);
    process.stdout.write(`FACTOR_RENDER first=${first.toFixed(1)}ms items=${P} factors=${M}\n`);
    expect(first).toBeLessThan(RENDER_CEILING_MS);
  }, 120000);
});
