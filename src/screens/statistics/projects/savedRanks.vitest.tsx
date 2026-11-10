// A saved rank test is drawn by RanksCard, through SavedResult's renderer (Research OS,
// Phase 8Y-2, decision 622): on both stored fixtures, each number and label as Rust
// wrote it, the form it is and no other, and a score as the outcome named as the
// ANOVA's is.
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
const value = (id: string) => screen.getByTestId(id).querySelectorAll('td')[0].textContent;

describe('a saved rank test', () => {
  it('draws a Mann-Whitney result, its labels and numbers as stored, with no Kruskal-Wallis line', () => {
    const r = bundleOf('ranks_mw.json');
    const t = r.test.mann_whitney;
    body(view('ranks_mw.json'));
    expect(screen.getByTestId('rk-result-card')).toBeTruthy();
    expect(screen.getByTestId('saved-data-file').textContent).toContain('CSV file');
    expect(screen.queryByTestId('rk-kw')).toBeNull();
    for (const g of r.groups as Array<Record<string, any>>) {
      expect(screen.getByTestId(`rk-group-label-${g.level}`).textContent).toBe(g.label);
    }
    expect(value('rk-mw-u1')).toBe(shownNumber(t.u1));
    expect(value('rk-mw-z')).toBe(shownNumber(t.z.value));
    expect(value('rk-mw-exact-p')).toBe(shownNumber(t.exactP));
    expect(value('rk-mw-rank-biserial')).toBe(shownNumber(t.rankBiserial));
    expect(screen.getByTestId('rk-mw-first-second').textContent).toContain(`The first group is ${r.groups[0].label}, the second ${r.groups[1].label}.`);
    expect(screen.getByTestId('rk-exclusions').textContent).toContain('1 row held a missing code you ticked in the outcome, and was left out.');
  });

  it('draws a Kruskal-Wallis result, with no Mann-Whitney line', () => {
    const r = bundleOf('ranks_kw.json');
    const t = r.test.kruskal_wallis;
    body(view('ranks_kw.json'));
    expect(screen.queryByTestId('rk-mw')).toBeNull();
    expect(value('rk-kw-h')).toBe(shownNumber(t.h.value));
    expect(value('rk-kw-df')).toBe(shownNumber(t.df));
    expect(value('rk-kw-p')).toBe(shownNumber(t.p.value));
    expect(value('rk-kw-epsilon-squared')).toBe(shownNumber(t.epsilonSquared.value));
    expect(screen.getAllByTestId(/^rk-group-label-/)).toHaveLength(r.groups.length);
  });

  it("names the columns by position in a saved result's sentences, and a score by its name", () => {
    const v = view('ranks_mw.json');
    const bundle = JSON.parse(v.bundleJson as string);
    bundle.diagnostics = [{ code: 'stats.ranks.suspect_missing_code', params: { column: 3, value: -99, count: 2 }, severity: 'warning' }];
    const edited = { ...v, bundleJson: JSON.stringify(bundle) };
    body(edited);
    expect(screen.getByTestId('rk-result-notes').textContent).toContain('The outcome C3 holds -99');
    cleanup();
    body(edited, [{ position: 3, name: 'Total', statistic: 'mean', items: 3 }]);
    expect(screen.getByTestId('rk-result-notes').textContent).toContain(`The outcome ${scoreColumn(3, 'Total')} holds -99`);
  });
});
