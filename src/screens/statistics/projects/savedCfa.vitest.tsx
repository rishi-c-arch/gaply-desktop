// A saved confirmatory factor analysis is drawn by CfaCard, through SavedResult's renderer
// (Research OS, Phase 9C-3, decision 785): on the stored fixtures, each number as Rust wrote it,
// two correlated factors with their covariance and correlation, three uncorrelated factors with
// none and the line that says so, a Heywood case at 0 degrees of freedom with its notes and its
// "–" cells, a failed run in its own words, and a saved run's columns named by position and a
// score by its name.
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
const shown = (s: { value: number }) => shownNumber(s.value);

describe('a saved confirmatory factor analysis', () => {
  it('draws two correlated factors: its numbers as stored, the covariance and the correlation, and the fit tables', () => {
    const r = bundleOf('cfa.json');
    body(view('cfa.json'));
    expect(screen.getByTestId('cf-result-card')).toBeTruthy();
    expect(screen.getByTestId('saved-data-file').textContent).toContain('CSV file');
    expect(tds('cf-moments')).toEqual(['21']);
    const l = r.loadings[1];
    expect(tds('cf-weight-C4')).toEqual([shown(l.estimate), shown(l.se), shown(l.cr), shown(l.p)]);
    expect(tds('cf-correlation-1-2')).toEqual([shown(r.factorCovariances[0].correlation)]);
    expect(tds('cf-fit-cmin-model')).toEqual(['13', shown(r.fit.model.cmin), '8', shown(r.fit.model.p), shown(r.fit.model.cminDf)]);
    expect(screen.getByTestId('cf-exclusions').textContent).toContain('1 row held a missing code you ticked');
  });

  it('draws three uncorrelated factors with no covariance table, and says they are fixed at 0', () => {
    body(view('cfa_uncorrelated.json'));
    expect(screen.queryByTestId('cf-covariances')).toBeNull();
    expect(screen.getByTestId('cf-no-covariances').textContent).toBe("The factors' covariances are fixed at 0.");
    expect(screen.getAllByTestId(/^cf-variance-factor-/)).toHaveLength(3);
  });

  it('draws the Heywood case at 0 degrees of freedom with its notes and "–" cells', () => {
    const r = bundleOf('cfa_heywood.json');
    body(view('cfa_heywood.json'));
    expect(tds('cf-fit-rmsea-model')).toEqual(['–', '–', '–', '–']);
    expect(screen.getByTestId('cf-notes').textContent).toContain('At least one variance estimate is below 0');
    expect(screen.getByTestId('cf-notes').textContent).toContain('The model has 0 degrees of freedom');
    expect(r.errorVariances.some((e: { estimate: { value: number } }) => e.estimate.value < 0)).toBe(true);
  });

  it('names the columns by position in a saved result, and a score by its name', () => {
    const v = view('cfa.json');
    body(v);
    expect(screen.getByTestId('cf-weight-C1').querySelector('th')?.textContent).toBe('C1 ← Factor 1');
    cleanup();
    body(v, [{ position: 1, name: 'Total', statistic: 'mean', items: 3 }]);
    expect(screen.getByTestId('cf-weight-C1').querySelector('th')?.textContent).toBe(`${scoreColumn(1, 'Total')} ← Factor 1`);
  });

  it('shows the singular failed run in its own words, naming the column by position', () => {
    expect(checkStored(view('cfa_failed.json')).kind).toBe('failed');
    const { container } = body(view('cfa_failed.json'));
    expect(container.textContent).toContain(
      'C10 is, to within one part in a billion of its variance, a combination of the columns before it in the model, so their covariance matrix is singular and the model cannot be fitted. Take it out of the model.'
    );
  });
});
