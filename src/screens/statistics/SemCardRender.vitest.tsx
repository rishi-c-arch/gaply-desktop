// A structural equation model of the widest shape 9C-4 measured, timed (Research OS, Phase 9C-5,
// decision 814): the card's first render, printed on a line as CfaCardRender.vitest.tsx prints its
// time. 128 columns (16 factors of five columns and 48 columns on their own), 64 paths in one loop
// through the 64 structural variables, 256 covariances and so 4,032 effects, every pair of the 64
// in each of the six matrices. jsdom draws no pixels, so this is React's work, not a frame's.
// The ceiling, 41,600 ms, is ten times the slowest render measured under load, rounded up as
// CfaCardRender.vitest.tsx rounds: 4,156.6 ms in the statistics and workspace files at default
// workers (load 11.80 before, 19.21 after); 3,957.1 ms in the same run before it (load 2.92
// before, 11.80 after); 1,226.6 to 1,293.4 ms alone (load about 3) (15 Nov 2026,
// ~/gaply-corpus/os/phase9/9c5/timing/p1_stats_dirs_*.log and render_alone_*.log).
import React from 'react';
import { cleanup, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';

import { semResultView } from './fixtures/semFixtures';
import type { SemResultView, SemStat, SemVariable } from './semTypes';
import { SemCard } from './SemCard';
import { MAX_COLUMNS, MAX_COVARIANCES, MAX_FACTORS, MAX_PATHS } from './semWording';

afterEach(cleanup, 120000);

const K = 16;
const PER_FACTOR = 5;
const ALONE = 48;
const RENDER_CEILING_MS = 41600;

/** The widest shape, built from the fixture's shapes, numbers varied so no two cells repeat. */
function widest(): SemResultView {
  const base = semResultView as unknown as SemResultView;
  const x = (i: number): SemStat => ({ value: Math.sin(i * 31 + 1) * 0.9 });
  const tested = (i: number) => ({ estimate: x(i), se: x(i + 1), cr: x(i + 2), p: x(i + 3) });
  const factorColumns = Array.from({ length: K }, (_, f) => Array.from({ length: PER_FACTOR }, (_, j) => `C${f * PER_FACTOR + j + 1}`));
  const alone = Array.from({ length: ALONE }, (_, j) => `C${K * PER_FACTOR + j + 1}`);
  const structural: SemVariable[] = [...Array.from({ length: K }, (_, f) => ({ factor: f + 1 })), ...alone.map((column) => ({ column }))];
  const loadings = factorColumns.flatMap((cols, f) => cols.map((column, j) => ({ to: { column }, from: { factor: f + 1 }, kind: 'loading' as const, ...tested(f * 10 + j), standardized: x(f + j) })));
  const paths = structural.map((from, i) => ({ to: structural[(i + 1) % structural.length], from, kind: 'path' as const, ...tested(500 + i), standardized: x(i) }));
  const covariances = Array.from({ length: MAX_COVARIANCES }, (_, i) => ({ a: { column: `C${(i % 80) + 1}` }, b: { column: `C${81 + Math.floor(i / 80)}` }, ...tested(900 + i), correlation: x(i) }));
  const columns = [...factorColumns.flat(), ...alone];
  // Every variable is endogenous: each structural one has a path into it, and each factor column
  // measures its factor.
  const variances = [...structural, ...factorColumns.flat().map((column) => ({ column }))].map((variable, i) => ({ variable, error: true, ...tested(1300 + i) }));
  const smc = [...structural, ...factorColumns.flat().map((column) => ({ column }))].map((variable, i) => ({ variable, value: x(1500 + i) }));
  const effects = structural.flatMap((to, i) =>
    structural.flatMap((from, j) =>
      i === j ? [] : [{ to, from, total: x(i * 64 + j), direct: x(i + j), indirect: x(i * j), totalStandardized: x(i - j), directStandardized: x(j), indirectStandardized: x(i) }]
    )
  );
  return { ...base, result: { ...base.result, columns, rowsUsed: 2000, indicators: factorColumns, weights: [...loadings, ...paths], covariances, variances, smc, effects } };
}

describe('a structural equation model of 128 columns, 64 paths in a loop, 256 covariances and 4,032 effects', () => {
  it('renders within the ceiling', () => {
    expect([MAX_COLUMNS, MAX_FACTORS, MAX_PATHS, MAX_COVARIANCES]).toEqual([K * PER_FACTOR + ALONE, K, 64, 256]);
    const result = widest();
    expect(result.result.effects).toHaveLength(4032);
    const t0 = performance.now();
    render(<SemCard result={result} names={{ column: (a) => `C${a}` }} />);
    const first = performance.now() - t0;
    expect(document.querySelectorAll('[data-testid^="sm-weight-"]').length).toBe(K * PER_FACTOR + 64);
    expect(document.querySelectorAll('[data-testid^="sm-covariance-"]').length).toBe(256);
    expect(screen.getByTestId('sm-effects-total').querySelectorAll('tbody tr')).toHaveLength(64);
    expect(screen.getByTestId('sm-effects-total').querySelectorAll('tbody td')).toHaveLength(64 * 64);
    process.stdout.write(`SEM_RENDER first=${first.toFixed(1)}ms columns=128 paths=64 covariances=256 effects=4032\n`);
    expect(first).toBeLessThan(RENDER_CEILING_MS);
  }, 120000);
});
