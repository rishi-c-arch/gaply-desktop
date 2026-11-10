// A saved related rank test is drawn by RelatedRanksCard, through SavedResult's renderer
// (Research OS, Phase 8Z-2, decision 656): on both stored fixtures, each number as Rust
// wrote it, the form it is and no other, a saved run's columns named by position and a
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
const value = (id: string) => screen.getByTestId(id).querySelectorAll('td')[0].textContent;
const W = 'related_ranks_w.json';
const F = 'related_ranks_f.json';

describe('a saved related rank test', () => {
  it('draws a Wilcoxon result, its numbers as stored, with no Friedman line', () => {
    const r = bundleOf(W);
    const t = r.test.wilcoxon;
    body(view(W));
    expect(screen.getByTestId('rr-result-card')).toBeTruthy();
    expect(screen.getByTestId('saved-data-file').textContent).toContain('CSV file');
    expect(screen.queryByTestId('rr-friedman')).toBeNull();
    expect(value('rr-v')).toBe(shownNumber(t.v));
    expect(value('rr-z')).toBe(shownNumber(t.z.value));
    expect(value('rr-exact-p')).toBe(shownNumber(t.exactP));
    expect(value('rr-rank-biserial')).toBe(shownNumber(t.rankBiserial.value));
    expect(screen.getByTestId('rr-first-second').textContent).toContain('The differences are C1 less C2;');
    expect(screen.getByTestId('rr-exclusions').textContent).toContain('1 row had no value in C2 (an empty cell or a missing-value token), and was left out.');
    expect(screen.getByTestId('rr-exclusions').textContent).toContain('1 row held a missing code you ticked in C1, and was left out.');
  });

  it('draws a Friedman result, with no Wilcoxon line', () => {
    const r = bundleOf(F);
    const t = r.test.friedman;
    body(view(F));
    expect(screen.queryByTestId('rr-wilcoxon')).toBeNull();
    expect(value('rr-q')).toBe(shownNumber(t.q.value));
    expect(value('rr-df')).toBe(shownNumber(t.df));
    expect(value('rr-friedman-p')).toBe(shownNumber(t.p.value));
    expect(value('rr-kendalls-w')).toBe(shownNumber(t.kendallsW.value));
    expect(screen.getAllByTestId(/^rr-col-C/)).toHaveLength(r.columns.length);
  });

  it("names the columns by position in a saved result, and a score by its name", () => {
    const v = view(F);
    body(v);
    expect(screen.getByTestId('rr-col-C1').querySelector('th')?.textContent).toBe('C1');
    cleanup();
    body(v, [{ position: 1, name: 'Total', statistic: 'mean', items: 3 }]);
    expect(screen.getByTestId('rr-col-C1').querySelector('th')?.textContent).toBe(scoreColumn(1, 'Total'));
  });

  it('shows a failed run in its own words', () => {
    const v = view(W);
    const failed: RunResultView = {
      ...v,
      state: 'failed',
      bundleJson: null,
      manifestJson: null,
      failureJson: JSON.stringify({ schemaVersion: 1, status: 'failed', failure: { code: 'stats.related_ranks.no_complete_rows', params: {}, severity: 'error' } }),
    };
    expect(checkStored(failed).kind).toBe('failed');
    const { container } = body(failed);
    expect(container.textContent).toContain('No row has a value in every chosen column, so there is nothing to compare.');
  });
});
