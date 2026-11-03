// The Variable View (Research OS, Phase 8P, decision 452; ADR-012 Part 4): one
// row per column of the profile, read-only, against the prepared wire fixture
// and against a view built here with a score and an SPSS reader's finding, on
// savFindings.vitest.tsx's pattern.
import React from 'react';
import { cleanup, render, screen, within } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import type { ColumnProfile, DatasetView, SavFinding } from '../describeTypes';
import preparedFixture from '../fixtures/datasetViewPrepared.json';
import { VariableView } from './VariableView';

vi.mock('../../../design-system/GaplyGlobe', () => ({ GaplyGlobe: () => <div /> }));

afterEach(cleanup);

const HEADERS = ['#', 'Alias', 'Name', 'Type', 'Distinct values', 'Empty cells', 'Possible missing codes', 'From'];
const READ_ONLY = 'Read-only. Gaply never changes your file; a prepared column is kept as a recipe in the project.';

const column = (over: Partial<ColumnProfile>): ColumnProfile => ({
  alias: 'C1',
  header: 'age',
  kind: 'numeric',
  n: 10,
  missing: 0,
  distinct: 5,
  levels: null,
  suspectMissingCodes: [],
  nonFinite: 0,
  ...over,
});

const built = (columns: ColumnProfile[], extra: Partial<DatasetView> = {}): DatasetView => ({
  slot: 4,
  displayName: 'built.sav',
  profile: { rows: 12, columns },
  inputRef: { contentHash: 'ab'.repeat(32), rows: 12, columns: columns.length, semanticRole: 'table' },
  ...extra,
});

const rowOf = (alias: string) => within(screen.getByTestId(`sw-var-${alias}`)).getAllByRole('cell').map((c) => c.textContent);

describe('the Variable View', () => {
  it("has decision 441's columns, in order", () => {
    render(<VariableView view={preparedFixture as DatasetView} />);
    expect(within(screen.getByTestId('sw-vars')).getAllByRole('columnheader').map((h) => h.textContent)).toEqual(HEADERS);
  });

  it('against the prepared wire fixture: one row per column, the score by its typed name, from a recipe', () => {
    render(<VariableView view={preparedFixture as DatasetView} />);
    expect(rowOf('C1')).toEqual(['1', 'C1', 'item', 'Numeric', '3', '1', '', 'The file']);
    expect(rowOf('C2')).toEqual(['2', 'C2', 'wellbeing', 'Numeric', '4', '1', '', 'A recipe']);
    expect(within(screen.getByTestId('sw-vars')).getAllByRole('row')).toHaveLength(3);
    expect(screen.getByTestId('sw-var-name-C2').tagName).toBe('BDI');
  });

  it("says decision 441's sentence, under the table", () => {
    render(<VariableView view={preparedFixture as DatasetView} />);
    expect(screen.getByTestId('sw-vars-note').textContent).toBe(READ_ONLY);
  });

  it("gives each score's counts, in the prepared fixture's numbers", () => {
    render(<VariableView view={preparedFixture as DatasetView} />);
    expect(screen.getByTestId('sw-score-counts-C2').textContent).toBe(
      'C2 wellbeing: 6 rows scored from every item, 2 from fewer; 1 not scored, too few items; 0 not scored, too large to hold.'
    );
  });

  it('a score shows its typed name, never its header, which is its alias word', () => {
    render(<VariableView view={built([column({ alias: 'C2', header: 'C2', prepared: true, scoreName: 'total' })])} />);
    expect(screen.getByTestId('sw-var-name-C2').textContent).toBe('total');
    expect(rowOf('C2')[6 + 1]).toBe('A recipe');
  });

  it('a column of the file is named by its header, inside bdi, as text and never as markup', () => {
    render(<VariableView view={built([column({ header: '<img src=x onerror=alert(1)>' })])} />);
    const name = screen.getByTestId('sw-var-name-C1');
    expect(name.tagName).toBe('BDI');
    expect(name.textContent).toBe('<img src=x onerror=alert(1)>');
    expect(name.querySelector('img')).toBeNull();
  });

  it('names each kind as the profile table does', () => {
    render(
      <VariableView
        view={built([
          column({ alias: 'C1', kind: 'numeric' }),
          column({ alias: 'C2', kind: 'categorical', levels: [{ label: 'SECRET_LEVEL', count: 3 }] as ColumnProfile['levels'] }),
          column({ alias: 'C3', kind: 'empty', distinct: 0, missing: 12 }),
        ])}
      />
    );
    expect(rowOf('C1')[3]).toBe('Numeric');
    expect(rowOf('C2')[3]).toBe('Categorical');
    expect(rowOf('C3')[3]).toBe('Empty');
    expect(rowOf('C3')[5]).toBe('12');
  });

  it("does not show the profile table's levels", () => {
    render(<VariableView view={built([column({ kind: 'categorical', levels: [{ label: 'SECRET_LEVEL', count: 3 }] as ColumnProfile['levels'] })])} />);
    expect(screen.getByTestId('sw-vars').textContent).not.toContain('SECRET_LEVEL');
  });

  it('gives possible missing codes with their cells, one and many, and none as blank', () => {
    render(
      <VariableView
        view={built([
          column({ alias: 'C1', suspectMissingCodes: [{ code: 99, cells: 3 }] }),
          column({ alias: 'C2', suspectMissingCodes: [{ code: -9, cells: 1 }, { code: 99, cells: 2 }] }),
          column({ alias: 'C3' }),
        ])}
      />
    );
    expect(rowOf('C1')[6]).toBe('99 (3 cells)');
    expect(rowOf('C2')[6]).toBe('-9 (1 cell), 99 (2 cells)');
    expect(rowOf('C3')[6]).toBe('');
  });

  it('a view built with a score and an SPSS finding shows the sentence, the finding and the counts', () => {
    const finding: SavFinding = { code: 'sav.dates_out_of_range', params: { count: 2 } };
    render(
      <VariableView
        view={built(
          [
            column({ alias: 'C1', header: 'q1' }),
            column({ alias: 'C2', header: 'C2', prepared: true, scoreName: 'mean score' }),
          ],
          {
            fileFindings: [finding],
            scoreCounts: [{ column: 'C2', scoredFromEveryItem: 1, scoredFromFewer: 1, notScoredTooFew: 1, notScoredOverflow: 1 }],
          }
        )}
      />
    );
    expect(screen.getByTestId('sw-file-findings').textContent).toBe('2 date or time values were outside the years 1 to 9999, and were read as missing.');
    expect(screen.getByTestId('sw-score-counts-C2').textContent).toBe(
      'C2 mean score: 1 row scored from every item, 1 from fewer; 1 not scored, too few items; 1 not scored, too large to hold.'
    );
  });

  it('no finding, no score: neither line', () => {
    render(<VariableView view={built([column({})])} />);
    expect(screen.queryByTestId('sw-file-findings')).toBeNull();
    expect(screen.queryByTestId('sw-score-counts-C1')).toBeNull();
  });

  it('a column with non-finite cells says so, in its own numbers, alias and name', () => {
    render(
      <VariableView
        view={built([
          column({ alias: 'C3', header: 'height', nonFinite: 2 }),
          column({ alias: 'C4', header: 'weight', nonFinite: 1 }),
          column({ alias: 'C5', header: 'clean', nonFinite: 0 }),
        ])}
      />
    );
    expect(screen.getByTestId('sw-non-finite-C3').textContent).toBe('C3 height has 2 cells that are not finite numbers; a method refuses this column.');
    expect(screen.getByTestId('sw-non-finite-C4').textContent).toBe('C4 weight has 1 cell that is not a finite number; a method refuses this column.');
    expect(screen.queryByTestId('sw-non-finite-C5')).toBeNull();
  });

  it('is read-only: no control in it', () => {
    render(<VariableView view={preparedFixture as DatasetView} />);
    const root = screen.getByTestId('sw-variable-view');
    expect(root.querySelectorAll('input, select, textarea, button, a').length).toBe(0);
  });
});
