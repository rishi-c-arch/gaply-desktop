// Crosstabs' settings (Research OS, Phase 8X-2, decision 588), on AnovaSettings.tsx's:
// two pickers, rows and columns, each offering the columns that are not scale scores
// and have 2 to MAX_GROUP_LEVELS distinct values in the profile, as the ANOVA's group
// (AnovaSettings.tsx:27-28); ready when both are chosen and differ; the candidate
// missing codes the union of both columns' suspect codes, ascending and unique, as
// CorrSettings.tsx's (:63-75). A column whose 51 distinct values include a code is
// not offered, though Rust would take it (recorded, not built).
import React, { act } from 'react';
import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';

import type { ColumnProfile, DatasetView } from './describeTypes';
import { MAX_GROUP_LEVELS } from './anovaWording';
import { useXtabSettings, XtabSettings, type XtabSettingsState } from './XtabSettings';

afterEach(cleanup);

const col = (alias: string, header: string, distinct: number, over: Partial<ColumnProfile> = {}): ColumnProfile => ({
  alias,
  header,
  kind: 'categorical',
  n: 100,
  missing: 0,
  distinct,
  levels: null,
  suspectMissingCodes: [],
  nonFinite: 0,
  ...over,
});

const view: DatasetView = {
  slot: 3,
  displayName: 'survey.csv',
  profile: {
    rows: 100,
    columns: [
      col('C1', 'arm', 2, { suspectMissingCodes: [{ code: 99, cells: 2 }, { code: -9, cells: 1 }] }),
      col('C2', 'site', 4, { suspectMissingCodes: [{ code: -99, cells: 3 }, { code: 99, cells: 5 }] }),
      col('C3', 'one level', 1),
      col('C4', 'fifty', MAX_GROUP_LEVELS),
      col('C5', 'fifty-one', MAX_GROUP_LEVELS + 1, { suspectMissingCodes: [{ code: -99, cells: 1 }] }),
      col('C6', 'age', 40, { kind: 'numeric' }),
      col('C7', 'total', 9, { kind: 'numeric', prepared: true, scoreName: 'Total' }),
    ],
  },
  inputRef: { contentHash: 'ab'.repeat(32), rows: 100, columns: 7, semanticRole: 'table' },
};

let state: XtabSettingsState | null = null;
const Harness: React.FC<{ v: DatasetView }> = ({ v }) => {
  const s = useXtabSettings(v);
  state = s;
  return <XtabSettings view={v} settings={s} />;
};

const options = (id: string) => Array.from((screen.getByTestId(id) as HTMLSelectElement).options).map((o) => o.value);
const pick = (id: string, value: string) => fireEvent.change(screen.getByTestId(id), { target: { value } });

describe('the pickers', () => {
  it('offer, in each, the columns with 2 to 50 distinct values that are not scores', () => {
    render(<Harness v={view} />);
    for (const id of ['xt-rows', 'xt-columns']) expect(options(id), id).toEqual(['', 'C1', 'C2', 'C4', 'C6']);
  });

  it('say the limit, and open with nothing chosen and not ready', () => {
    render(<Harness v={view} />);
    expect(screen.getByTestId('xt-column-limit').textContent).toBe(`A column with more than ${MAX_GROUP_LEVELS} distinct values is not offered.`);
    expect((screen.getByTestId('xt-rows') as HTMLSelectElement).value).toBe('');
    expect((screen.getByTestId('xt-columns') as HTMLSelectElement).value).toBe('');
    expect(state?.ready).toBe(false);
  });
});

describe('readiness', () => {
  it('is ready only when both are chosen and differ, and says so when they are one column', () => {
    render(<Harness v={view} />);
    pick('xt-rows', 'C1');
    expect(state?.ready).toBe(false);
    pick('xt-columns', 'C1');
    expect(state?.ready).toBe(false);
    expect(screen.getByTestId('xt-same-column').textContent).toBe('The rows and the columns are the same column. Choose two different columns.');
    pick('xt-columns', 'C2');
    expect(state?.ready).toBe(true);
    expect(screen.queryByTestId('xt-same-column')).toBeNull();
    pick('xt-rows', '');
    expect(state?.ready).toBe(false);
  });
});

describe('the missing codes', () => {
  it("offer the union of both columns' suspect codes, ascending and once each", () => {
    render(<Harness v={view} />);
    expect(screen.queryByTestId('xt-missing-codes')).toBeNull();
    pick('xt-rows', 'C1');
    expect(state?.offered.map((o) => o.code)).toEqual([-9, 99]);
    pick('xt-columns', 'C2');
    expect(state?.offered.map((o) => o.code)).toEqual([-99, -9, 99]);
    expect(Array.from(screen.getByTestId('xt-missing-codes').querySelectorAll('input')).map((i) => i.getAttribute('data-testid'))).toEqual([
      'xt-code--99',
      'xt-code--9',
      'xt-code-99',
    ]);
    // 99 is in both columns: one box, naming both.
    expect(screen.getByTestId('xt-code-99').closest('label')?.textContent?.trim()).toBe(
      'Is 99 a code for a missing value? It appears in "arm" (2 cells), "site" (5 cells). If you tick it, it is treated as missing in both columns.'
    );
  });

  it('send the ticked codes still offered, ascending, and none unticked', () => {
    render(<Harness v={view} />);
    pick('xt-rows', 'C1');
    pick('xt-columns', 'C2');
    fireEvent.click(screen.getByTestId('xt-code-99'));
    fireEvent.click(screen.getByTestId('xt-code--99'));
    expect(state?.spec(1).parameters).toEqual({ rows: 'C1', columns: 'C2', missingCodes: [-99, 99] });
    // Decision 652: the codes are the pair's, so any change of a column clears every tick.
    pick('xt-columns', 'C4');
    expect(state?.spec(1).parameters).toEqual({ rows: 'C1', columns: 'C4', missingCodes: [] });
  });

  it('reset drops every choice', () => {
    render(<Harness v={view} />);
    pick('xt-rows', 'C1');
    pick('xt-columns', 'C2');
    fireEvent.click(screen.getByTestId('xt-code-99'));
    act(() => state?.reset());
    expect(state?.rows).toBe('');
    expect(state?.columns).toBe('');
    expect(state?.ticked).toEqual([]);
  });
});
