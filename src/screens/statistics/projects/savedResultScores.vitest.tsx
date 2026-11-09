// A saved result names a score column by its position and its name
// (Research OS, Phase 8U-3, decision 554; ADR-011:600-602 as decision 558
// reads it): C{n} "name" wherever BY_POSITION named it, found by the
// column's position and never by a header's text. Every other column keeps
// its name, and a run with no scores is unchanged
// (savedResultUnchanged.vitest.tsx).
import fs from 'node:fs';
import path from 'node:path';
import React from 'react';
import { cleanup, render, screen, within } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { SavedResultBody } from './SavedResult';
import { checkStored } from './storedShape';
import type { RunResultView, ScoreView } from './projectTypes';
import { scoreColumn } from './projectWording';

vi.mock('../../../config/featureFlags', () => ({
  isFeatureEnabled: (name: string) => name === 'projects' || name === 'workspaces' || name === 'statsDescribe',
}));
vi.mock('../../../utils/isTauri', () => ({ isTauri: true, default: true }));

afterEach(cleanup);

const STORED = path.join(__dirname, '../../../../src-tauri/tests/fixtures/projects/stored');
const view = (f: string) => JSON.parse(fs.readFileSync(path.join(STORED, f), 'utf8')) as RunResultView;

/** A stored view whose bundle `edit` changes. */
function edited(f: string, edit: (bundle: Record<string, any>) => void): RunResultView {
  const v = view(f);
  const bundle = JSON.parse(v.bundleJson as string) as Record<string, any>;
  edit(bundle);
  return { ...v, bundleJson: JSON.stringify(bundle) };
}

const score = (position: number, name: string): ScoreView => ({ position, name, statistic: 'mean', items: 3 });

function body(v: RunResultView, scores: ScoreView[] | null) {
  const checked = checkStored(v);
  expect(['unknownCapability', 'unknownSchema', 'badShape']).not.toContain(checked.kind);
  return render(<SavedResultBody checked={checked} scores={scores} />);
}

describe("a saved result's score columns", () => {
  it("a summary's row of a score, found by the row's alias", () => {
    // summary_prepared.json: one numeric row, C3, a score, its header the alias word.
    body(view('summary_prepared.json'), [score(3, 'Average')]);
    const table = screen.getByTestId('desc-numeric-table');
    const header = within(table).getAllByRole('rowheader').map((h) => h.textContent);
    expect(header).toEqual([scoreColumn(3, 'Average')]);
    expect(scoreColumn(3, 'Average')).toBe('C3 "Average"');
  });

  it('a file column whose header reads like an alias is never taken for a score', () => {
    // Row C1's header is "C3", and the score is at position 3: the row is C1's.
    const v = edited('summary_prepared.json', (b) => {
      b.primaryOutputs.columns[0].numeric.alias = 'C1';
    });
    body(v, [score(3, 'Average')]);
    const header = within(screen.getByTestId('desc-numeric-table')).getAllByRole('rowheader').map((h) => h.textContent);
    expect(header).toEqual(['C3']);
    expect(document.body.textContent).not.toContain('Average');
  });

  it("the correlations' descriptives and matrix", () => {
    body(view('corr_pairwise.json'), [score(2, 'Average')]);
    const label = scoreColumn(2, 'Average');
    const text = document.body.textContent ?? '';
    // In the descriptives' row and the matrix's header and row cells.
    expect(text.split(label).length - 1).toBeGreaterThanOrEqual(3);
    expect(text).toContain('C1');
    expect(text).toContain('C3');
    expect(text).not.toMatch(/C2(?! ")/);
  });

  it("alpha's items", () => {
    body(view('alpha.json'), [score(1, 'Average')]);
    const label = scoreColumn(1, 'Average');
    expect(screen.getByTestId('al-item-row-C1').textContent).toContain(label);
    expect(screen.getByTestId('al-item-row-C3').textContent).not.toContain('Average');
  });

  it("a t-test's sentence that names the column", () => {
    const v = edited('ttest.json', (b) => {
      b.diagnostics = [{ code: 'stats.ttest.suspect_missing_code', params: { column: 3, value: -99, count: 2 }, severity: 'warning' }];
    });
    body(v, [score(3, 'Average')]);
    expect(screen.getByTestId('tt-result-notes').textContent).toContain(`The outcome ${scoreColumn(3, 'Average')} holds -99`);
    cleanup();
    body(v, null);
    expect(screen.getByTestId('tt-result-notes').textContent).toContain('The outcome C3 holds -99');
  });

  it("an ANOVA's sentence that names the column", () => {
    const v = edited('anova.json', (b) => {
      b.diagnostics = [{ code: 'stats.anova.suspect_missing_code', params: { column: 4, value: -99, count: 2 }, severity: 'warning' }];
    });
    body(v, [score(4, 'Total')]);
    expect(screen.getByTestId('an-result-notes').textContent).toContain(`The outcome ${scoreColumn(4, 'Total')} holds -99`);
  });

  it('a score at no column of the result changes nothing', () => {
    const { container } = body(view('corr_pairwise.json'), [score(40, 'Elsewhere')]);
    const withScores = container.innerHTML;
    cleanup();
    const { container: plain } = body(view('corr_pairwise.json'), null);
    expect(withScores).toBe(plain.innerHTML);
  });

  it('the name is text only: in no attribute of any element', () => {
    const NAME = 'NAME_SENTINEL_s29';
    for (const [f, p] of [
      ['summary_prepared.json', 3],
      ['corr_pairwise.json', 2],
      ['alpha.json', 1],
    ] as const) {
      body(view(f), [score(p, NAME)]);
      expect(document.body.textContent, f).toContain(NAME);
      const attrs = Array.from(document.body.querySelectorAll('*')).flatMap((el) => Array.from(el.attributes).map((a) => `${a.name}=${a.value}`));
      expect(attrs.filter((a) => a.includes(NAME)), f).toEqual([]);
      cleanup();
    }
  });
});
