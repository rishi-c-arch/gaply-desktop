// A saved linear regression is drawn by LinearRegressionCard, through SavedResult's renderer
// (Research OS, Phase 9A-2, decision 687): on the stored fixtures, each number as Rust wrote
// it, a perfect fit with its spanning sentence and its note, a failed run in its own words, a
// saved run's columns named by position and a score by its name.
import fs from 'node:fs';
import path from 'node:path';
import React from 'react';
import { cleanup, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { SavedResultBody } from './SavedResult';
import { checkStored } from './storedShape';
import type { RunResultView, ScoreView } from './projectTypes';
import { scoreColumn } from './projectWording';
import { shownNumber } from '../ttestWording';

vi.mock('../../../config/featureFlags', () => ({
  isFeatureEnabled: (name: string) => name === 'projects' || name === 'workspaces',
}));
vi.mock('../../../utils/isTauri', () => ({ isTauri: true, default: true }));

afterEach(cleanup);

const STORED = path.join(__dirname, '../../../../src-tauri/tests/fixtures/projects/stored');
const view = (f: string) => JSON.parse(fs.readFileSync(path.join(STORED, f), 'utf8')) as RunResultView;
const bundleOf = (f: string) => JSON.parse(view(f).bundleJson as string).primaryOutputs as Record<string, any>;
const body = (v: RunResultView, scores: ScoreView[] | null = null) => render(<SavedResultBody checked={checkStored(v)} scores={scores} />);
const cells = (id: string) => Array.from(screen.getByTestId(id).querySelectorAll('th, td')).map((c) => c.textContent);
const MODEL = 'linear_regression.json';
const PERFECT = 'linear_regression_perfect.json';
const FAILED = 'linear_regression_failed.json';

describe('a saved linear regression', () => {
  it('draws the six tables, its numbers as stored, the predictors in the order chosen', () => {
    const r = bundleOf(MODEL);
    body(view(MODEL));
    expect(screen.getByTestId('lr-result-card')).toBeTruthy();
    expect(screen.getByTestId('saved-data-file').textContent).toContain('CSV file');
    expect(cells('lr-model-rSquared')).toEqual(['R²', shownNumber(r.model.rSquared.value)]);
    expect(screen.getByTestId('lr-anova-f').textContent).toBe(shownNumber(r.anova.f.value));
    expect(screen.getAllByTestId(/^lr-coef-C/).map((e) => e.getAttribute('data-testid'))).toEqual(r.predictors.map((p: { column: string }) => `lr-coef-${p.column}`));
    expect(cells('lr-coef-C2')[1]).toBe(shownNumber(r.predictors[0].b.value));
    expect(screen.getByTestId('lr-coefficients-sentence').textContent).toContain('B is the change in C1 for a one-unit change');
    expect(screen.getByTestId('lr-exclusions').textContent).toContain('1 row had no value in C3 (an empty cell or a missing-value token), and was left out.');
    expect(screen.queryByTestId('lr-notes')).toBeNull();
  });

  it('draws a perfect fit with its spanning sentence and its note', () => {
    body(view(PERFECT));
    expect(screen.getByTestId('lr-coef-intercept-not-computable').textContent).toBe(
      'Not computable: the model fits every row exactly, so there is no residual variation.'
    );
    expect(screen.getByTestId('lr-notes').textContent).toContain('essentially perfect fit');
  });

  it('names the columns by position in a saved result, and a score by its name', () => {
    const v = view(MODEL);
    body(v);
    expect(screen.getByTestId('lr-desc-C1').querySelector('th')?.textContent).toBe('C1');
    cleanup();
    body(v, [{ position: 1, name: 'Total', statistic: 'mean', items: 3 }]);
    expect(screen.getByTestId('lr-desc-C1').querySelector('th')?.textContent).toBe(scoreColumn(1, 'Total'));
  });

  it('shows the collinear failed run in its own words, naming the predictor by position', () => {
    expect(checkStored(view(FAILED)).kind).toBe('failed');
    const { container } = body(view(FAILED));
    expect(container.textContent).toContain(
      "C5 is, within R's tolerance, a linear combination of the intercept and the predictors before it, so its effect cannot be separated from theirs. Take it out of the predictors."
    );
  });
});
