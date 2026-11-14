// The confirmatory factor analysis's model editor (Research OS, Phase 9C-3, decisions 782 and
// 783): each numeric column put in one factor or left out by its select; the factors held by ids
// and numbered by their first column in the file; each factor's columns drawn in the file's
// order; the correlated checkbox; the codes over the model's columns; readiness; and the spec.
import React from 'react';
import { act, cleanup, fireEvent, render, screen, within } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';

import type { ColumnProfile, DatasetView } from './describeTypes';
import { CfaSettings, buildCfaSpec, useCfaSettings, type CfaSettingsState } from './CfaSettings';

afterEach(cleanup);

const col = (alias: string, header: string, over: Partial<ColumnProfile> = {}): ColumnProfile => ({
  alias,
  header,
  kind: 'numeric',
  n: 100,
  missing: 0,
  distinct: 50,
  levels: null,
  suspectMissingCodes: [],
  nonFinite: 0,
  ...over,
});

const inputRef = { contentHash: 'ab'.repeat(32), rows: 100, columns: 8, semanticRole: 'table' as const };
const view: DatasetView = {
  slot: 3,
  displayName: 'survey.csv',
  profile: {
    rows: 100,
    columns: [
      col('C1', 'id', { kind: 'categorical' }),
      col('C2', 'q1', { suspectMissingCodes: [{ code: -99, cells: 2 }] }),
      col('C3', 'q2', { suspectMissingCodes: [{ code: 99, cells: 1 }, { code: -99, cells: 1 }] }),
      col('C4', 'q3', { suspectMissingCodes: [{ code: 77, cells: 4 }] }),
      col('C5', 'q4'),
      col('C6', 'q5'),
      col('C7', 'q6'),
      col('C8', 'C8', { prepared: true, scoreName: 'wellbeing' }),
    ],
  },
  inputRef,
};

/** A view of n numeric columns, C1 to Cn. */
const wide = (n: number): DatasetView => ({
  ...view,
  profile: { rows: 100, columns: Array.from({ length: n }, (_, i) => col(`C${i + 1}`, `x${i + 1}`)) },
});

let state: CfaSettingsState | null = null;
const Harness: React.FC<{ v?: DatasetView }> = ({ v = view }) => {
  const s = useCfaSettings(v);
  state = s;
  return <CfaSettings view={v} settings={s} />;
};

const select = (alias: string) => screen.getByTestId(`cf-assign-${alias}`) as HTMLSelectElement;
const options = (alias: string) => Array.from(select(alias).options).map((o) => o.textContent);
const chosen = (alias: string) => select(alias).selectedOptions[0]?.textContent;
/** Puts a column where the option with this text puts it. */
const choose = (alias: string, text: string) => {
  const o = Array.from(select(alias).options).find((x) => x.textContent === text);
  if (!o) throw new Error(`no option ${text} for ${alias}: ${options(alias).join(' | ')}`);
  fireEvent.change(select(alias), { target: { value: o.value } });
};
/** The right pane: each factor's heading and its columns, as drawn. */
const pane = () =>
  screen.queryAllByTestId(/^cf-factor-\d+$/).map((f) => [
    f.querySelector('h5')?.textContent,
    within(f)
      .queryAllByRole('listitem')
      .map((li) => li.getAttribute('data-testid')?.replace(/^cf-factor-\d+-/, '')),
  ]);
const s = () => state as CfaSettingsState;
const params = () => s().spec(1).parameters;
const codeBox = (c: number) => screen.queryByTestId(`cf-code-${c}`) as HTMLInputElement | null;
const box = () => screen.getByTestId('cf-correlated') as HTMLInputElement;
const missing = () => screen.queryByTestId('cf-settings-missing')?.textContent ?? null;

describe('the left pane', () => {
  it("offers the numeric columns, scores included, in the file's order, each labelled by its name and starting out of the model", () => {
    render(<Harness />);
    expect(within(screen.getByTestId('cf-columns')).getByRole('heading').textContent).toBe('Numeric columns');
    const selects = within(screen.getByTestId('cf-columns')).getAllByRole('combobox');
    expect(selects.map((x) => x.getAttribute('data-testid'))).toEqual(['cf-assign-C2', 'cf-assign-C3', 'cf-assign-C4', 'cf-assign-C5', 'cf-assign-C6', 'cf-assign-C7', 'cf-assign-C8']);
    for (const a of ['C2', 'C3', 'C4', 'C5', 'C6', 'C7', 'C8']) {
      expect(chosen(a)).toBe('Not in the model');
      expect(options(a)).toEqual(['Not in the model', 'A new factor']);
      expect(select(a).getAttribute('aria-label')).toBeNull();
    }
    expect(select('C3').closest('label')?.textContent).toContain('q2');
    expect(select('C8').closest('label')?.textContent).toContain('wellbeing');
    expect(screen.getByTestId('cf-none').textContent).toBe('No factor yet. Choose a factor for a column on the left.');
    expect(within(screen.getByTestId('cf-factors')).getByRole('heading').textContent).toBe("Factors, each in the file's order");
  });
});

describe('the factors', () => {
  it("are drawn in the file's order and numbered by their first column, whatever order they were chosen in", () => {
    render(<Harness />);
    choose('C6', 'A new factor');
    expect(pane()).toEqual([['Factor 1', ['C6']]]);
    choose('C3', 'Factor 1');
    choose('C7', 'A new factor');
    expect(pane()).toEqual([
      ['Factor 1', ['C3', 'C6']],
      ['Factor 2', ['C7']],
    ]);
    choose('C2', 'Factor 2');
    // C2 now begins the second factor made, so it is Factor 1, and the first is Factor 2.
    choose('C4', 'Factor 2');
    // The first factor made (C6, then C3 and C4) begins at C3; the second (C7, then C2) at C2.
    expect(pane()).toEqual([
      ['Factor 1', ['C2', 'C7']],
      ['Factor 2', ['C3', 'C4', 'C6']],
    ]);
    expect(chosen('C2')).toBe('Factor 1');
    expect(chosen('C6')).toBe('Factor 2');
    expect(options('C5')).toEqual(['Not in the model', 'Factor 1', 'Factor 2', 'A new factor']);
    expect(within(screen.getByTestId('cf-factor-1')).getAllByRole('listitem')[0].textContent).toBe('C2 q1 (loading fixed at 1)');
    expect(within(screen.getByTestId('cf-factor-2')).getAllByRole('listitem').map((li) => li.textContent)).toEqual([
      'C3 q2 (loading fixed at 1)',
      'C4 q3',
      'C6 q5',
    ]);
  });

  it("renumber at once when a factor's first column leaves it, the right pane and every select alike", () => {
    render(<Harness />);
    choose('C2', 'A new factor');
    choose('C5', 'Factor 1');
    choose('C3', 'A new factor');
    choose('C4', 'Factor 2');
    expect(pane()).toEqual([
      ['Factor 1', ['C2', 'C5']],
      ['Factor 2', ['C3', 'C4']],
    ]);
    choose('C2', 'Not in the model');
    expect(pane()).toEqual([
      ['Factor 1', ['C3', 'C4']],
      ['Factor 2', ['C5']],
    ]);
    expect([chosen('C3'), chosen('C4'), chosen('C5'), chosen('C2')]).toEqual(['Factor 1', 'Factor 1', 'Factor 2', 'Not in the model']);
    expect(options('C2')).toEqual(['Not in the model', 'Factor 1', 'Factor 2', 'A new factor']);
    // A factor whose last column leaves it is gone.
    choose('C5', 'Not in the model');
    expect(pane()).toEqual([['Factor 1', ['C3', 'C4']]]);
    expect(options('C2')).toEqual(['Not in the model', 'Factor 1', 'A new factor']);
  });

  it('every column can be put back as not in the model', () => {
    render(<Harness />);
    for (const a of ['C2', 'C3', 'C4']) choose(a, 'A new factor');
    for (const a of ['C2', 'C3', 'C4']) choose(a, 'Not in the model');
    expect(pane()).toEqual([]);
    for (const a of ['C2', 'C3', 'C4', 'C5', 'C6', 'C7', 'C8']) expect(chosen(a)).toBe('Not in the model');
    expect(missing()).toBe('Put at least 3 columns in factors.');
  });

  it('offer no new factor at 16 factors', () => {
    render(<Harness v={wide(40)} />);
    act(() => {
      for (let i = 1; i <= 16; i += 1) s().assign(`C${i}`, 'new');
    });
    expect(s().factors).toHaveLength(16);
    const o = options('C17');
    expect(o).toHaveLength(1 + 16);
    expect(o).not.toContain('A new factor');
    expect(o[16]).toBe('Factor 16');
    act(() => s().assign('C1', 'none'));
    expect(options('C17')).toContain('A new factor');
  });

  it('take a column out when a view change removes it, and drop a factor left empty', () => {
    const without = (alias: string): DatasetView => ({ ...view, profile: { ...view.profile, columns: view.profile.columns.filter((c) => c.alias !== alias) } });
    const { rerender } = render(<Harness v={view} />);
    choose('C2', 'A new factor');
    choose('C3', 'Factor 1');
    choose('C4', 'A new factor');
    rerender(<Harness v={without('C4')} />);
    expect(pane()).toEqual([['Factor 1', ['C2', 'C3']]]);
    rerender(<Harness v={without('C2')} />);
    expect(pane()).toEqual([['Factor 1', ['C3']]]);
    rerender(<Harness v={view} />);
    expect(pane()).toEqual([['Factor 1', ['C3']]]);
    expect([chosen('C2'), chosen('C4')]).toEqual(['Not in the model', 'Not in the model']);
  });
});

describe('the correlated checkbox', () => {
  it('starts ticked, is disabled with fewer than two factors, and keeps its value', () => {
    render(<Harness />);
    expect(box().closest('label')?.textContent?.trim()).toBe('The factors are correlated');
    expect(screen.getByTestId('cf-correlated-line').textContent).toBe(
      'Ticked, every pair of factors has a covariance to estimate. Unticked, those covariances are fixed at 0.'
    );
    expect([box().checked, box().disabled]).toEqual([true, true]);
    choose('C2', 'A new factor');
    expect(box().disabled).toBe(true);
    choose('C3', 'A new factor');
    expect(box().disabled).toBe(false);
    fireEvent.click(box());
    expect(box().checked).toBe(false);
    expect(params().correlatedFactors).toBe(false);
    choose('C3', 'Factor 1');
    expect([box().checked, box().disabled]).toEqual([false, true]);
    expect(params().correlatedFactors).toBe(false);
    choose('C4', 'A new factor');
    expect([box().checked, box().disabled]).toEqual([false, false]);
  });
});

describe('the codes', () => {
  it("offers the union over the model's columns, ascending, each with where it appears", () => {
    render(<Harness />);
    choose('C3', 'A new factor');
    choose('C2', 'Factor 1');
    choose('C4', 'A new factor');
    expect(screen.getByTestId('cf-missing-codes').querySelector('legend')?.textContent).toBe('Candidate missing codes');
    const questions = within(screen.getByTestId('cf-missing-codes'))
      .getAllByRole('checkbox')
      .map((b) => (b.closest('label')?.textContent ?? '').trim());
    const after = 'If you tick it, it is treated as missing in every column in the model, and a row with it is left out.';
    expect(questions).toEqual([
      `Is -99 a code for a missing value? It appears in "q1" (2 cells), "q2" (1 cell). ${after}`,
      `Is 77 a code for a missing value? It appears in "q3" (4 cells). ${after}`,
      `Is 99 a code for a missing value? It appears in "q2" (1 cell). ${after}`,
    ]);
    fireEvent.click(codeBox(99) as HTMLInputElement);
    fireEvent.click(codeBox(-99) as HTMLInputElement);
    expect(params().missingCodes).toEqual([-99, 99]);
  });

  it('drops a tick with the last column that offered it', () => {
    render(<Harness />);
    choose('C2', 'A new factor');
    choose('C3', 'Factor 1');
    choose('C5', 'Factor 1');
    fireEvent.click(codeBox(-99) as HTMLInputElement);
    choose('C2', 'Not in the model');
    expect(params().missingCodes).toEqual([-99]);
    choose('C3', 'Not in the model');
    expect(codeBox(-99)).toBeNull();
    expect(s().ticked).toEqual([]);
    choose('C3', 'Factor 1');
    expect((codeBox(-99) as HTMLInputElement).checked).toBe(false);
  });

  it('prunes a tick when a view change takes its code away, and the code returns unticked', () => {
    const without = (alias: string): DatasetView => ({ ...view, profile: { ...view.profile, columns: view.profile.columns.filter((c) => c.alias !== alias) } });
    const { rerender } = render(<Harness v={view} />);
    choose('C2', 'A new factor');
    choose('C4', 'Factor 1');
    choose('C5', 'Factor 1');
    fireEvent.click(codeBox(77) as HTMLInputElement);
    expect(params().missingCodes).toEqual([77]);
    rerender(<Harness v={without('C4')} />);
    expect(params().missingCodes).toEqual([]);
    rerender(<Harness v={view} />);
    expect(codeBox(77)).toBeNull();
    choose('C4', 'Factor 1');
    expect((codeBox(77) as HTMLInputElement).checked).toBe(false);
  });
});

describe('readiness (decision 782)', () => {
  it('says what is missing until there is a factor, every factor has 2 columns and the model 3', () => {
    render(<Harness />);
    expect(missing()).toBe('Put at least 3 columns in factors.');
    choose('C2', 'A new factor');
    expect(missing()).toBe('Put at least 3 columns in factors.');
    expect(screen.getByTestId('cf-factor-1-short').textContent).toBe('Factor 1 has 1 column; each factor needs at least 2.');
    choose('C3', 'Factor 1');
    expect(missing()).toBe('Put at least 3 columns in factors.');
    expect(screen.queryByTestId('cf-factor-1-short')).toBeNull();
    choose('C4', 'Factor 1');
    expect(missing()).toBeNull();
    expect(s().stillMissing).toBeNull();
    choose('C5', 'A new factor');
    expect(missing()).toBe('Factor 2 has 1 column; each factor needs at least 2.');
    choose('C6', 'Factor 2');
    expect(missing()).toBeNull();
  });

  it('at 128 columns is ready, and a 129th column offers only "Not in the model", with the reason beside the selects', () => {
    render(<Harness v={wide(129)} />);
    // C1 to C16 each make a factor (ids 1 to 16, in order); C17 to C128 go round them, so
    // each factor has 8 columns.
    act(() => {
      for (let i = 1; i <= 16; i += 1) s().assign(`C${i}`, 'new');
      for (let i = 17; i <= 128; i += 1) s().assign(`C${i}`, ((i - 17) % 16) + 1);
    });
    expect(s().columns).toHaveLength(128);
    expect(s().factors).toHaveLength(16);
    expect(s().stillMissing).toBeNull();
    expect(s().atCap).toBe(true);
    expect(options('C129')).toEqual(['Not in the model']);
    expect(options('C1')).toContain('Factor 16');
    expect(screen.getByTestId('cf-at-cap').textContent).toBe('The model has 128 columns, the most Gaply fits; take one out to add another.');
    act(() => s().assign('C128', 'none'));
    expect(screen.queryByTestId('cf-at-cap')).toBeNull();
    expect(options('C129')).toEqual(['Not in the model', ...Array.from({ length: 16 }, (_, i) => `Factor ${i + 1}`)]);
  });
});

describe('the spec (decision 782)', () => {
  it("lists the factors by their first column, each in the file's order, the codes once each and ascending", () => {
    const s = buildCfaSpec(view, [['C7', 'C3'], ['C5', 'C2', 'C6']], false, [99, -99, 99], 7);
    expect(s.capabilityId).toBe('stats.analyze.cfa');
    expect(s.capabilityVersion).toBe(1);
    expect(s.operationId).toBe(7);
    expect(s.inputs).toEqual([view.inputRef]);
    expect(JSON.parse(JSON.stringify(s)).parameters).toEqual({
      indicators: [
        ['C2', 'C5', 'C6'],
        ['C3', 'C7'],
      ],
      correlatedFactors: false,
      missingCodes: [-99, 99],
    });
  });

  it('is what the editor gives, after choices in another order', () => {
    render(<Harness />);
    choose('C7', 'A new factor');
    choose('C4', 'A new factor');
    choose('C3', 'Factor 2');
    choose('C2', 'Factor 1');
    choose('C6', 'Factor 2');
    expect(params()).toEqual({
      indicators: [
        ['C2', 'C3', 'C7'],
        ['C4', 'C6'],
      ],
      correlatedFactors: true,
      missingCodes: [],
    });
  });

  it("carries the view's preparation on a score, and none on a file's own table", () => {
    const preparation = { kind: 'scale_score' as const, sourceContentHash: 'cd'.repeat(32), operations: [] };
    const prepared = { ...view, preparation } as unknown as DatasetView;
    render(<Harness v={prepared} />);
    choose('C8', 'A new factor');
    expect(s().spec(1).preparation).toBe(preparation);
    cleanup();
    render(<Harness v={view} />);
    choose('C2', 'A new factor');
    expect('preparation' in s().spec(1)).toBe(false);
  });

  it('reset drops every choice and restores the starting values', () => {
    render(<Harness />);
    choose('C2', 'A new factor');
    choose('C3', 'A new factor');
    fireEvent.click(box());
    act(() => s().reset());
    expect(s().factors).toEqual([]);
    expect(s().correlated).toBe(true);
    expect(s().ticked).toEqual([]);
  });
});

describe('a column name', () => {
  it('appears only as text, never in a key, an attribute, a test id or an aria-label', () => {
    const sentinel = 'LABEL_SENTINEL_9c3';
    const v: DatasetView = {
      ...view,
      profile: { rows: 100, columns: [col('C1', sentinel), col('C2', 'C2', { suspectMissingCodes: [{ code: -99, cells: 1 }] }), col('C3', 'x'), col('C4', 'y')] },
    };
    const { container } = render(<Harness v={v} />);
    choose('C1', 'A new factor');
    choose('C2', 'Factor 1');
    choose('C3', 'A new factor');
    expect(container.textContent).toContain(sentinel);
    for (const el of Array.from(container.querySelectorAll('*'))) {
      for (const attr of Array.from(el.attributes)) expect(attr.value, attr.name).not.toContain(sentinel);
    }
  });
});
