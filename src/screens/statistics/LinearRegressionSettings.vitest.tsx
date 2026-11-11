// The linear regression's settings (Research OS, Phase 9A-2, decisions 684 and 685): the
// outcome a select of the numeric columns, scores included; the predictors AlphaSettings.tsx's
// two panes as RelatedRanksSettings.tsx draws them, the order the user chooses kept, shown and
// sent, never sorted.
import React, { useState } from 'react';
import { act, cleanup, fireEvent, render, screen, within } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';

import type { ColumnProfile, DatasetView } from './describeTypes';
import { LinearRegressionSettings, useLinearRegressionSettings, type LinearRegressionSettingsState } from './LinearRegressionSettings';
import { ALPHA_ADD_ALL, ALPHA_COLUMNS_HEADING, ALPHA_ITEMS_NONE, ALPHA_REMOVE_ALL, ALPHA_REMOVE_ITEM } from './window/windowWording';

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

const inputRef = { contentHash: 'ab'.repeat(32), rows: 100, columns: 5, semanticRole: 'table' as const };
const view: DatasetView = {
  slot: 3,
  displayName: 'survey.csv',
  profile: {
    rows: 100,
    columns: [
      col('C1', 'id', { kind: 'categorical' }),
      col('C2', 'hours', { suspectMissingCodes: [{ code: -99, cells: 2 }] }),
      col('C3', 'sleep', { suspectMissingCodes: [{ code: 99, cells: 1 }, { code: -99, cells: 1 }] }),
      col('C4', 'age', { suspectMissingCodes: [{ code: 77, cells: 4 }] }),
      col('C5', 'C5', { prepared: true, scoreName: 'wellbeing' }),
    ],
  },
  inputRef,
};

let state: LinearRegressionSettingsState | null = null;
const Harness: React.FC<{ v?: DatasetView }> = ({ v = view }) => {
  const s = useLinearRegressionSettings(v);
  state = s;
  return <LinearRegressionSettings view={v} settings={s} />;
};
const Host: React.FC<{ v: DatasetView }> = ({ v }) => {
  const [shown] = useState(v);
  return <Harness v={shown} />;
};

const outcome = (alias: string) => fireEvent.change(screen.getByTestId('lr-outcome'), { target: { value: alias } });
const tick = (alias: string) => fireEvent.click(screen.getByTestId(`lr-column-${alias}`));
const left = () => within(screen.getByTestId('lr-columns')).getAllByRole('checkbox').map((b) => b.getAttribute('data-testid')?.replace('lr-column-', ''));
const list = () => screen.queryAllByTestId(/^lr-row-C/).map((r) => r.getAttribute('data-testid')?.replace('lr-row-', ''));
const rowText = (alias: string) => screen.getByTestId(`lr-row-${alias}`).querySelector('[data-testid="lr-row-name"]')?.textContent;
const codeBox = (c: number) => screen.queryByTestId(`lr-code-${c}`) as HTMLInputElement | null;
const spec = () => (state as LinearRegressionSettingsState).spec(1);
const up = (a: string) => screen.getByTestId(`lr-up-${a}`) as HTMLButtonElement;
const down = (a: string) => screen.getByTestId(`lr-down-${a}`) as HTMLButtonElement;

describe('the outcome', () => {
  it('offers every numeric column, a scale score among them, with nothing chosen', () => {
    render(<Harness />);
    const select = screen.getByTestId('lr-outcome') as HTMLSelectElement;
    expect(select.closest('label')?.firstChild?.textContent).toBe('Outcome (the column to explain)');
    expect(Array.from(select.options).map((o) => [o.value, o.textContent])).toEqual([
      ['', 'Choose a column…'],
      ['C2', 'C2 hours'],
      ['C3', 'C3 sleep'],
      ['C4', 'C4 age'],
      ['C5', 'C5 wellbeing'],
    ]);
    expect(select.value).toBe('');
  });

  it('leaves the left pane when chosen, and a score can be the outcome', () => {
    render(<Harness />);
    expect(left()).toEqual(['C2', 'C3', 'C4', 'C5']);
    outcome('C5');
    expect(left()).toEqual(['C2', 'C3', 'C4']);
    expect(spec().parameters.outcome).toBe('C5');
  });

  it('takes a predictor chosen as the outcome out of the list, and another outcome later does not put it back', () => {
    render(<Harness />);
    outcome('C2');
    tick('C4');
    tick('C3');
    expect(list()).toEqual(['C4', 'C3']);
    outcome('C3');
    expect(list()).toEqual(['C4']);
    expect(left()).toEqual(['C2', 'C4', 'C5']);
    outcome('C2');
    expect(list()).toEqual(['C4']);
    expect(spec().parameters).toEqual({ outcome: 'C2', predictors: ['C4'], missingCodes: [] });
  });

  it('clears every tick when it changes', () => {
    render(<Harness />);
    outcome('C2');
    tick('C3');
    fireEvent.click(codeBox(-99) as HTMLInputElement);
    fireEvent.click(codeBox(99) as HTMLInputElement);
    expect(spec().parameters.missingCodes).toEqual([-99, 99]);
    outcome('C4');
    expect((codeBox(-99) as HTMLInputElement).checked).toBe(false);
    expect((codeBox(99) as HTMLInputElement).checked).toBe(false);
    expect(spec().parameters.missingCodes).toEqual([]);
  });
});

describe('the two panes', () => {
  it("offers the numeric columns in the file's order, with nothing chosen, under decision 685's words", () => {
    render(<Harness />);
    expect(screen.getByTestId('lr-predictor-choices').querySelector('legend')?.textContent).toBe('Predictors, entered together');
    expect(within(screen.getByTestId('lr-columns')).getByRole('heading').textContent).toBe(ALPHA_COLUMNS_HEADING);
    expect(within(screen.getByTestId('lr-list')).getByRole('heading').textContent).toBe('Predictors, in this order');
    expect(screen.getByTestId('lr-none').textContent).toBe(ALPHA_ITEMS_NONE);
    expect(screen.getByTestId('lr-add-all').textContent).toBe(ALPHA_ADD_ALL);
    expect(screen.getByTestId('lr-remove-all').textContent).toBe(ALPHA_REMOVE_ALL);
    expect(within(screen.getByTestId('lr-columns')).getAllByRole('checkbox').every((b) => !(b as HTMLInputElement).checked)).toBe(true);
  });

  it("keeps the list in click order, each row numbered from 1, never the file's order", () => {
    render(<Harness />);
    outcome('C5');
    tick('C4');
    tick('C2');
    tick('C3');
    expect(list()).toEqual(['C4', 'C2', 'C3']);
    expect(rowText('C4')).toBe('1. C4 age');
    expect(rowText('C2')).toBe('2. C2 hours');
    expect(rowText('C3')).toBe('3. C3 sleep');
    expect(spec().parameters.predictors).toEqual(['C4', 'C2', 'C3']);
  });

  it('moves with ↑ and ↓, disabled at their ends, each labelled by its words and the alias', () => {
    render(<Harness />);
    tick('C2');
    tick('C3');
    tick('C4');
    expect(up('C2').disabled).toBe(true);
    expect(down('C4').disabled).toBe(true);
    expect(up('C3').getAttribute('aria-label')).toBe('Move up C3');
    expect(down('C3').getAttribute('aria-label')).toBe('Move down C3');
    fireEvent.click(up('C3'));
    expect(list()).toEqual(['C3', 'C2', 'C4']);
    fireEvent.click(down('C2'));
    expect(list()).toEqual(['C3', 'C4', 'C2']);
    expect(up('C3').disabled).toBe(true);
    expect(down('C2').disabled).toBe(true);
  });

  it('removes a row with Remove, and empties the list with Remove all', () => {
    render(<Harness />);
    tick('C2');
    tick('C3');
    tick('C4');
    const remove = screen.getByTestId('lr-remove-C3');
    expect(remove.textContent).toBe(ALPHA_REMOVE_ITEM);
    expect(remove.getAttribute('aria-label')).toBe(`${ALPHA_REMOVE_ITEM} C3`);
    fireEvent.click(remove);
    expect(list()).toEqual(['C2', 'C4']);
    fireEvent.click(screen.getByTestId('lr-remove-all'));
    expect(list()).toEqual([]);
    expect(screen.getByTestId('lr-none')).toBeTruthy();
  });

  it("adds every numeric column not chosen, other than the outcome, in the file's order, after a chosen list", () => {
    render(<Harness />);
    outcome('C3');
    tick('C4');
    fireEvent.click(screen.getByTestId('lr-add-all'));
    expect(list()).toEqual(['C4', 'C2', 'C5']);
    fireEvent.click(screen.getByTestId('lr-add-all'));
    expect(list()).toEqual(['C4', 'C2', 'C5']);
  });
});

describe('the codes', () => {
  it('offers the union over the outcome and the predictors, ascending, each with where it appears', () => {
    render(<Harness />);
    outcome('C4');
    expect(screen.getByTestId('lr-missing-codes').querySelector('legend')?.textContent).toBe('Candidate missing codes');
    tick('C2');
    tick('C3');
    const questions = within(screen.getByTestId('lr-missing-codes'))
      .getAllByRole('checkbox')
      .map((b) => (b.closest('label')?.textContent ?? '').trim());
    const after = 'If you tick it, it is treated as missing in the outcome and every predictor, and a row with it is left out.';
    expect(questions).toEqual([
      `Is -99 a code for a missing value? It appears in "hours" (2 cells), "sleep" (1 cell). ${after}`,
      `Is 77 a code for a missing value? It appears in "age" (4 cells). ${after}`,
      `Is 99 a code for a missing value? It appears in "sleep" (1 cell). ${after}`,
    ]);
  });

  it("offers the outcome's codes before any predictor is chosen", () => {
    render(<Harness />);
    outcome('C4');
    expect(codeBox(77)).not.toBeNull();
    expect(codeBox(-99)).toBeNull();
  });

  it('drops a tick with the last column that offered it, and does not restore it on re-adding the column', () => {
    render(<Harness />);
    outcome('C5');
    tick('C4');
    tick('C2');
    fireEvent.click(codeBox(77) as HTMLInputElement);
    expect(spec().parameters.missingCodes).toEqual([77]);
    fireEvent.click(screen.getByTestId('lr-remove-C4'));
    expect(codeBox(77)).toBeNull();
    expect(spec().parameters.missingCodes).toEqual([]);
    tick('C4');
    expect((codeBox(77) as HTMLInputElement).checked).toBe(false);
  });

  it('keeps a tick while a chosen column, the outcome included, offers it', () => {
    render(<Harness />);
    outcome('C2');
    tick('C3');
    fireEvent.click(codeBox(-99) as HTMLInputElement);
    fireEvent.click(screen.getByTestId('lr-remove-C3'));
    expect((codeBox(-99) as HTMLInputElement).checked).toBe(true);
    expect(spec().parameters.missingCodes).toEqual([-99]);
  });

  // Research OS, Phase 9B-1 (decision 698; the review of 9e4b7ad9, finding 6).
  it('drops a tick when a view change takes its code away, and the code returns unticked', () => {
    const without = (alias: string): DatasetView => ({ ...view, profile: { ...view.profile, columns: view.profile.columns.filter((c) => c.alias !== alias) } });
    const { rerender } = render(<Harness v={view} />);
    outcome('C5');
    tick('C4');
    tick('C2');
    fireEvent.click(codeBox(77) as HTMLInputElement);
    expect(spec().parameters.missingCodes).toEqual([77]);
    rerender(<Harness v={without('C4')} />);
    expect(codeBox(77)).toBeNull();
    rerender(<Harness v={view} />);
    expect(list()).toEqual(['C4', 'C2']);
    expect((codeBox(77) as HTMLInputElement).checked).toBe(false);
    expect(spec().parameters.missingCodes).toEqual([]);
  });

  it('sends the ticked codes ascending and once each', () => {
    render(<Harness />);
    outcome('C5');
    tick('C4');
    tick('C3');
    fireEvent.click(codeBox(99) as HTMLInputElement);
    fireEvent.click(codeBox(77) as HTMLInputElement);
    fireEvent.click(codeBox(-99) as HTMLInputElement);
    expect(spec().parameters.missingCodes).toEqual([-99, 77, 99]);
  });
});

describe('readiness (decision 685)', () => {
  const many = (n: number): DatasetView => ({
    ...view,
    profile: { rows: 100, columns: Array.from({ length: n }, (_, i) => col(`C${i + 1}`, `h${i + 1}`)) },
  });
  const missing = () => screen.queryByTestId('lr-settings-missing')?.textContent ?? null;

  it('asks for the outcome first, then a predictor', () => {
    render(<Harness />);
    expect(state?.stillMissing).toBe('Choose the outcome.');
    expect(missing()).toBe('Choose the outcome.');
    tick('C2');
    expect(missing()).toBe('Choose the outcome.');
    outcome('C3');
    expect(missing()).toBeNull();
    fireEvent.click(screen.getByTestId('lr-remove-all'));
    expect(missing()).toBe('Choose at least one predictor.');
  });

  it('is ready at 1 and at 50 predictors, and says how many are chosen at 51', () => {
    render(<Host v={many(52)} />);
    outcome('C52');
    tick('C1');
    expect(state?.stillMissing).toBeNull();
    for (let i = 2; i <= 50; i += 1) tick(`C${i}`);
    expect(state?.predictors).toHaveLength(50);
    expect(state?.stillMissing).toBeNull();
    tick('C51');
    expect(state?.stillMissing).toBe('Choose 50 predictors or fewer; 51 are chosen.');
    expect(missing()).toBe('Choose 50 predictors or fewer; 51 are chosen.');
  });

  it('asks for the outcome again when the view has lost it, and neither draws nor sends it', () => {
    const without = (alias: string): DatasetView => ({ ...view, profile: { ...view.profile, columns: view.profile.columns.filter((c) => c.alias !== alias) } });
    const { rerender } = render(<Harness v={view} />);
    outcome('C4');
    tick('C2');
    expect(state?.stillMissing).toBeNull();
    rerender(<Harness v={without('C4')} />);
    expect(state?.outcome).toBe('');
    expect(state?.stillMissing).toBe('Choose the outcome.');
    expect((screen.getByTestId('lr-outcome') as HTMLSelectElement).value).toBe('');
    rerender(<Harness v={view} />);
    expect(state?.outcome).toBe('C4');
    expect(state?.stillMissing).toBeNull();
  });

  it('draws, sends and counts only the predictors still in the view', () => {
    const without = (alias: string): DatasetView => ({ ...view, profile: { ...view.profile, columns: view.profile.columns.filter((c) => c.alias !== alias) } });
    const { rerender } = render(<Harness v={view} />);
    outcome('C5');
    tick('C3');
    tick('C2');
    rerender(<Harness v={without('C3')} />);
    expect(list()).toEqual(['C2']);
    expect(spec().parameters.predictors).toEqual(['C2']);
    const neither: DatasetView = { ...view, profile: { ...view.profile, columns: view.profile.columns.filter((c) => c.alias !== 'C2' && c.alias !== 'C3') } };
    rerender(<Harness v={neither} />);
    expect(list()).toEqual([]);
    expect(state?.stillMissing).toBe('Choose at least one predictor.');
  });
});

describe('the spec (decision 684)', () => {
  it("lists the predictors in the list's order after a move", () => {
    render(<Harness />);
    outcome('C5');
    tick('C2');
    tick('C3');
    tick('C4');
    fireEvent.click(up('C4'));
    expect(spec().parameters.predictors).toEqual(['C2', 'C4', 'C3']);
    expect(spec().capabilityId).toBe('stats.analyze.linear_regression');
  });

  it("carries the view's preparation on a score, and none on a file's own table", () => {
    const preparation = { kind: 'scale_score' as const, sourceContentHash: 'cd'.repeat(32), operations: [] };
    const prepared = { ...view, preparation } as unknown as DatasetView;
    render(<Harness v={prepared} />);
    outcome('C5');
    tick('C2');
    expect(spec().preparation).toBe(preparation);
    cleanup();
    render(<Harness v={view} />);
    outcome('C5');
    tick('C2');
    expect('preparation' in spec()).toBe(false);
  });

  it('reset drops every choice', () => {
    render(<Harness />);
    outcome('C5');
    tick('C2');
    act(() => state?.reset());
    expect(state?.outcome).toBe('');
    expect(state?.predictors).toEqual([]);
    expect(state?.ticked).toEqual([]);
  });
});

describe('a column name', () => {
  it('appears only as text, never in a key, an attribute or a test id', () => {
    const sentinel = 'LABEL_SENTINEL_9a2';
    const v: DatasetView = { ...view, profile: { rows: 100, columns: [col('C1', sentinel), col('C2', 'C2', { suspectMissingCodes: [{ code: -99, cells: 1 }] }), col('C3', 'x')] } };
    const { container } = render(<Harness v={v} />);
    outcome('C2');
    tick('C1');
    tick('C3');
    expect(container.textContent).toContain(sentinel);
    for (const el of Array.from(container.querySelectorAll('*'))) {
      for (const attr of Array.from(el.attributes)) expect(attr.value, attr.name).not.toContain(sentinel);
    }
  });
});
