// The 50-predictor regression, timed (Research OS, Phase 9A-2, decision 684): the card's first
// render, printed on a line as RelatedRanksCardRender.vitest.tsx prints its time. Fifty is the
// most predictors a run takes (MAX_PREDICTORS). jsdom draws no pixels, so this is React's
// work, not a frame's. The ceiling, 2,700 ms, is ten times the slowest render measured under
// load, rounded up: 265.8 ms in the whole suite at default workers (load 4.34 4.59 7.45). The
// others: 171.3 ms in the statistics suite at default workers (load 14.77 10.07 18.69), 82.8 ms
// in the whole suite at two workers, 69.3 to 73.7 ms alone.
import React from 'react';
import { cleanup, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';

import { linearRegressionResultView } from './fixtures/linearRegressionFixtures';
import type { LinearRegressionResultView, PredictorEstimate } from './linearRegressionTypes';
import { LinearRegressionCard } from './LinearRegressionCard';
import { MAX_PREDICTORS } from './linearRegressionWording';

afterEach(cleanup, 120000);

const FIFTY = 50;
const RENDER_CEILING_MS = 2700;

const result50: LinearRegressionResultView = (() => {
  const base = linearRegressionResultView as unknown as LinearRegressionResultView;
  const one = base.result.predictors[0];
  const aliases = Array.from({ length: FIFTY }, (_, i) => `C${i + 2}`);
  const predictors: PredictorEstimate[] = aliases.map((column, i) => ({ ...one, column, b: { value: 0.5 + i / 7 }, t: { value: 1.25 - i / 11 } }));
  return {
    ...base,
    result: {
      ...base.result,
      rowsUsed: 2000,
      columns: [base.result.columns[0], ...aliases.map((column, i) => ({ column, mean: { value: i / 3 }, sd: { value: 1 + i / 9 } }))],
      predictors,
      anova: { ...base.result.anova, regression: { ...base.result.anova.regression, df: FIFTY } },
    },
  };
})();

describe('a 50-predictor regression', () => {
  it('renders within the ceiling', () => {
    expect(MAX_PREDICTORS).toBe(FIFTY);
    const t0 = performance.now();
    render(<LinearRegressionCard result={result50} names={{ column: (a) => `C${a}` }} />);
    const first = performance.now() - t0;
    expect(document.querySelectorAll('[data-testid^="lr-coef-C"]').length).toBe(FIFTY);
    expect(document.querySelectorAll('[data-testid^="lr-corr-C"]').length).toBe(FIFTY);
    expect(document.querySelectorAll('[data-testid^="lr-desc-C"]').length).toBe(FIFTY + 1);
    expect(screen.getByTestId('lr-coef-C51').querySelectorAll('td')[0].textContent).not.toBe('');
    process.stdout.write(`LINEAR_REGRESSION_RENDER first=${first.toFixed(1)}ms predictors=${FIFTY}\n`);
    expect(first).toBeLessThan(RENDER_CEILING_MS);
  }, 120000);
});
