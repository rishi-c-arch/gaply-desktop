// A saved factor analysis is drawn by FactorCard, through SavedResult's renderer (Research OS,
// Phase 9B-3, decision 732): on the stored fixtures, each number as Rust wrote it, promax's
// three tables and varimax's two, a rotation that did not converge with its note and no
// rotation table, a failed run in its own words, and a saved run's items named by position and
// a score by its name.
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
const tds = (id: string) => Array.from(screen.getByTestId(id).querySelectorAll('td')).map((c) => c.textContent);

describe('a saved factor analysis', () => {
  it('draws principal axis with promax: its numbers as stored, as factors, with the pattern, structure and correlations', () => {
    const r = bundleOf('factor.json');
    body(view('factor.json'));
    expect(screen.getByTestId('fa-result-card')).toBeTruthy();
    expect(screen.getByTestId('saved-data-file').textContent).toContain('CSV file');
    expect(tds('fa-kmo-overall')).toEqual([shownNumber(r.kmo.overall.value)]);
    expect(tds('fa-matrix-C1')).toEqual(r.extraction.loadings[0].map((s: { value: number }) => shownNumber(s.value)));
    expect(tds('fa-pattern-C1')).toEqual(r.rotation.loadings[0].map((s: { value: number }) => shownNumber(s.value)));
    expect(tds('fa-correlations-1')).toEqual(r.rotation.factorCorrelations[0].map((s: { value: number }) => shownNumber(s.value)));
    expect(screen.getByRole('heading', { name: 'Factor matrix' })).toBeTruthy();
    expect(screen.getByTestId('fa-exclusions').textContent).toContain('1 row had no value');
  });

  it('draws principal components with varimax as components, with the rotated and transformation matrices', () => {
    const r = bundleOf('factor_varimax.json');
    body(view('factor_varimax.json'));
    expect(screen.getByRole('heading', { name: 'Rotated component matrix' })).toBeTruthy();
    expect(tds('fa-transformation-1')).toEqual(r.rotation.transformation[0].map((s: { value: number }) => shownNumber(s.value)));
    expect(screen.queryByTestId('fa-pattern')).toBeNull();
  });

  it('draws no rotation table when the rotation did not converge, and its note says why', () => {
    body(view('factor_not_rotated.json'));
    expect(screen.queryByTestId('fa-rotated')).toBeNull();
    expect(screen.queryByTestId('fa-transformation')).toBeNull();
    expect(screen.getByTestId('fa-notes').textContent).toBe('The rotation did not converge within the maximum iterations, so no rotated solution is given.');
  });

  it('names the items by position in a saved result, and a score by its name', () => {
    const v = view('factor.json');
    body(v);
    expect(screen.getByTestId('fa-desc-C1').querySelector('th')?.textContent).toBe('C1');
    cleanup();
    body(v, [{ position: 1, name: 'Total', statistic: 'mean', items: 3 }]);
    expect(screen.getByTestId('fa-desc-C1').querySelector('th')?.textContent).toBe(scoreColumn(1, 'Total'));
  });

  it('shows the singular failed run in its own words, naming the item by position', () => {
    expect(checkStored(view('factor_failed.json')).kind).toBe('failed');
    const { container } = body(view('factor_failed.json'));
    expect(container.textContent).toContain(
      'C7 is a combination of the items before it, or within one part in a billion of its variance of being one, so the correlation matrix is singular and principal axis factoring cannot start. Take it out of the items, or use principal components.'
    );
  });
});
