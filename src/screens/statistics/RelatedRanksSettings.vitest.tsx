// The related rank tests' settings (Research OS, Phase 8Z-2, decision 653), on
// AlphaSettings.tsx's two panes, the order the user chooses kept and shown: the first
// less the second is what a Wilcoxon test computes.
import React, { useState } from 'react';
import { act, cleanup, fireEvent, render, screen, within } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';

import type { ColumnProfile, DatasetView } from './describeTypes';
import { addColumn, moveColumnDown, moveColumnUp, RelatedRanksSettings, removeColumn, useRelatedRanksSettings, type RelatedRanksSettingsState } from './RelatedRanksSettings';
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
      col('C2', 'before', { suspectMissingCodes: [{ code: -99, cells: 2 }] }),
      col('C3', 'after', { suspectMissingCodes: [{ code: 99, cells: 1 }, { code: -99, cells: 1 }] }),
      col('C4', 'follow up', { suspectMissingCodes: [{ code: 77, cells: 4 }] }),
      col('C5', 'C5', { prepared: true, scoreName: 'wellbeing' }),
    ],
  },
  inputRef,
};

let state: RelatedRanksSettingsState | null = null;
const Harness: React.FC<{ v?: DatasetView }> = ({ v = view }) => {
  const s = useRelatedRanksSettings(v);
  state = s;
  return <RelatedRanksSettings view={v} settings={s} />;
};
const Host: React.FC<{ v: DatasetView }> = ({ v }) => {
  const [shown] = useState(v);
  return <Harness v={shown} />;
};

const tick = (alias: string) => fireEvent.click(screen.getByTestId(`rr-column-${alias}`));
const list = () => screen.queryAllByTestId(/^rr-row-C/).map((r) => r.getAttribute('data-testid')?.replace('rr-row-', ''));
const rowText = (alias: string) => screen.getByTestId(`rr-row-${alias}`).querySelector('[data-testid="rr-row-name"]')?.textContent;
const codeBox = (c: number) => screen.queryByTestId(`rr-code-${c}`) as HTMLInputElement | null;
const spec = () => (state as RelatedRanksSettingsState).spec(1);
const up = (a: string) => screen.getByTestId(`rr-up-${a}`) as HTMLButtonElement;
const down = (a: string) => screen.getByTestId(`rr-down-${a}`) as HTMLButtonElement;

describe('the two panes', () => {
  it("offers every numeric column in the file's order, a scale score among them, and nothing chosen", () => {
    render(<Harness />);
    const left = within(screen.getByTestId('rr-columns'));
    expect(screen.getByTestId('rr-settings').querySelector('legend')?.textContent).toBe('Which columns do you want to compare? Each row is one case, measured in every chosen column.');
    expect(left.getByRole('heading').textContent).toBe(ALPHA_COLUMNS_HEADING);
    expect(left.getAllByRole('checkbox').map((b) => b.getAttribute('data-testid'))).toEqual(['rr-column-C2', 'rr-column-C3', 'rr-column-C4', 'rr-column-C5']);
    expect(left.getByLabelText(/C5 wellbeing/)).toBeTruthy();
    expect(left.getAllByRole('checkbox').every((b) => !(b as HTMLInputElement).checked)).toBe(true);
    const right = within(screen.getByTestId('rr-list'));
    expect(right.getByRole('heading').textContent).toBe('Compared, in this order');
    expect(screen.getByTestId('rr-none').textContent).toBe(ALPHA_ITEMS_NONE);
    expect(screen.queryByTestId('rr-code--99')).toBeNull();
  });

  it('keeps the list in click order, each row numbered from 1, never the file\'s order', () => {
    render(<Harness />);
    tick('C4');
    tick('C2');
    tick('C3');
    expect(list()).toEqual(['C4', 'C2', 'C3']);
    expect(rowText('C4')).toBe('1. C4 follow up');
    expect(rowText('C2')).toBe('2. C2 before');
    expect(rowText('C3')).toBe('3. C3 after');
    expect(screen.queryByTestId('rr-none')).toBeNull();
  });

  it('unticks by the left box, and the row goes', () => {
    render(<Harness />);
    tick('C4');
    tick('C2');
    tick('C4');
    expect(list()).toEqual(['C2']);
  });

  it('moves with ↑ and ↓, disabled at their ends, each labelled by its words and the alias', () => {
    render(<Harness />);
    tick('C2');
    tick('C3');
    tick('C4');
    expect(up('C2').disabled).toBe(true);
    expect(down('C4').disabled).toBe(true);
    expect(up('C3').disabled).toBe(false);
    expect(down('C3').disabled).toBe(false);
    expect(up('C3').textContent).toBe('↑');
    expect(down('C3').textContent).toBe('↓');
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
    const remove = screen.getByTestId('rr-remove-C2');
    expect(remove.textContent).toBe(ALPHA_REMOVE_ITEM);
    expect(remove.getAttribute('aria-label')).toBe(`${ALPHA_REMOVE_ITEM} C2`);
    fireEvent.click(remove);
    expect(list()).toEqual(['C3']);
    expect((screen.getByTestId('rr-column-C2') as HTMLInputElement).checked).toBe(false);
    expect(screen.getByTestId('rr-remove-all').textContent).toBe(ALPHA_REMOVE_ALL);
    fireEvent.click(screen.getByTestId('rr-remove-all'));
    expect(list()).toEqual([]);
    expect(screen.getByTestId('rr-none')).toBeTruthy();
  });

  it("adds the columns not yet chosen, in the file's order, after a chosen list", () => {
    render(<Harness />);
    tick('C4');
    tick('C2');
    expect(screen.getByTestId('rr-add-all').textContent).toBe(ALPHA_ADD_ALL);
    fireEvent.click(screen.getByTestId('rr-add-all'));
    expect(list()).toEqual(['C4', 'C2', 'C3', 'C5']);
  });

  it('never lists a column twice', () => {
    render(<Harness />);
    tick('C2');
    act(() => state?.toggleColumn('C2', true));
    expect(list()).toEqual(['C2']);
  });

  // Decision 668: with two chosen, adding the first again must not move it to the end.
  it('adding a column already chosen changes nothing, its place kept', () => {
    render(<Harness />);
    tick('C2');
    tick('C3');
    act(() => state?.toggleColumn('C2', true));
    expect(list()).toEqual(['C2', 'C3']);
    expect(spec().parameters.columns).toEqual(['C2', 'C3']);
  });
});

/** Decision 668: the list's changes, as pure functions, on three columns. */
describe("the list's changes", () => {
  const three = ['C2', 'C3', 'C4'];

  it('adds a column at the end, and an alias already in the list changes nothing', () => {
    expect(addColumn(three, 'C5')).toEqual(['C2', 'C3', 'C4', 'C5']);
    expect(addColumn(three, 'C2')).toEqual(['C2', 'C3', 'C4']);
    expect(addColumn(three, 'C3')).toEqual(['C2', 'C3', 'C4']);
    expect(addColumn([], 'C3')).toEqual(['C3']);
  });

  it('removes a column, and one not in the list changes nothing', () => {
    expect(removeColumn(three, 'C3')).toEqual(['C2', 'C4']);
    expect(removeColumn(three, 'C9')).toEqual(['C2', 'C3', 'C4']);
  });

  it('moves a column up and down by one, and not past either end', () => {
    expect(moveColumnUp(three, 'C3')).toEqual(['C3', 'C2', 'C4']);
    expect(moveColumnDown(three, 'C3')).toEqual(['C2', 'C4', 'C3']);
    expect(moveColumnUp(three, 'C2')).toEqual(three);
    expect(moveColumnDown(three, 'C4')).toEqual(three);
    expect(moveColumnUp(three, 'C9')).toEqual(three);
  });

  it('moves past a column that is not shown, swapping with the shown neighbour', () => {
    expect(moveColumnUp(three, 'C4', ['C2', 'C4'])).toEqual(['C4', 'C3', 'C2']);
    expect(moveColumnDown(three, 'C2', ['C2', 'C4'])).toEqual(['C4', 'C3', 'C2']);
  });

  it('leaves the list it is given as it was', () => {
    const list = ['C2', 'C3', 'C4'];
    addColumn(list, 'C5');
    removeColumn(list, 'C2');
    moveColumnUp(list, 'C3');
    moveColumnDown(list, 'C3');
    expect(list).toEqual(['C2', 'C3', 'C4']);
  });
});

/** Decision 668: what is sent is what is drawn. */
describe('a chosen column that leaves the view', () => {
  const without = (alias: string): DatasetView => ({ ...view, profile: { ...view.profile, columns: view.profile.columns.filter((c) => c.alias !== alias) } });

  it('is neither drawn nor sent, readiness counts without it, and it is drawn again when the view brings it back', () => {
    const { rerender } = render(<Harness v={view} />);
    tick('C2');
    tick('C3');
    expect(list()).toEqual(['C2', 'C3']);
    expect(state?.stillMissing).toBeNull();
    rerender(<Harness v={without('C3')} />);
    expect(list()).toEqual(['C2']);
    expect(state?.columns).toEqual(['C2']);
    expect(state?.stillMissing).toBe('Choose at least two columns.');
    expect(spec().parameters.columns).toEqual(['C2']);
    expect(screen.queryByTestId('rr-difference-line')).toBeNull();
    rerender(<Harness v={view} />);
    expect(list()).toEqual(['C2', 'C3']);
    expect(spec().parameters.columns).toEqual(['C2', 'C3']);
  });

  it('drops its codes from those offered and sent while it is gone', () => {
    const { rerender } = render(<Harness v={view} />);
    tick('C2');
    tick('C3');
    tick('C4');
    fireEvent.click(codeBox(99) as HTMLInputElement);
    expect(spec().parameters.missingCodes).toEqual([99]);
    rerender(<Harness v={without('C3')} />);
    expect(codeBox(99)).toBeNull();
    expect(spec().parameters.missingCodes).toEqual([]);
  });

  // Research OS, Phase 9B-1 (decision 698; the review of 9e4b7ad9, finding 6).
  it('drops a tick when it goes, and the code returns unticked when the view brings the column back', () => {
    const { rerender } = render(<Harness v={view} />);
    tick('C2');
    tick('C3');
    tick('C4');
    fireEvent.click(codeBox(99) as HTMLInputElement);
    expect(spec().parameters.missingCodes).toEqual([99]);
    rerender(<Harness v={without('C3')} />);
    expect(codeBox(99)).toBeNull();
    rerender(<Harness v={view} />);
    expect(list()).toEqual(['C2', 'C3', 'C4']);
    expect((codeBox(99) as HTMLInputElement).checked).toBe(false);
    expect(spec().parameters.missingCodes).toEqual([]);
  });

  it('moves over the drawn list, never swapping with a column that is not shown', () => {
    const { rerender } = render(<Harness v={view} />);
    tick('C2');
    tick('C3');
    tick('C4');
    rerender(<Harness v={without('C3')} />);
    fireEvent.click(up('C4'));
    expect(list()).toEqual(['C4', 'C2']);
    expect(spec().parameters.columns).toEqual(['C4', 'C2']);
  });
});

describe('the codes', () => {
  it("offers the union over the chosen columns, ascending, each with where it appears", () => {
    render(<Harness />);
    tick('C4');
    tick('C2');
    tick('C3');
    const fieldset = within(screen.getByTestId('rr-missing-codes'));
    const questions = fieldset.getAllByRole('checkbox').map((b) => (b.closest('label')?.textContent ?? '').trim());
    expect(questions).toEqual([
      'Is -99 a code for a missing value? It appears in "before" (2 cells), "after" (1 cell). If you tick it, it is treated as missing in every chosen column, and a row with it is left out.',
      'Is 77 a code for a missing value? It appears in "follow up" (4 cells). If you tick it, it is treated as missing in every chosen column, and a row with it is left out.',
      'Is 99 a code for a missing value? It appears in "after" (1 cell). If you tick it, it is treated as missing in every chosen column, and a row with it is left out.',
    ]);
  });

  it('keeps a tick while a chosen column offers it', () => {
    render(<Harness />);
    tick('C2');
    tick('C3');
    fireEvent.click(codeBox(-99) as HTMLInputElement);
    fireEvent.click(screen.getByTestId('rr-remove-C2'));
    expect((codeBox(-99) as HTMLInputElement).checked).toBe(true);
    expect(spec().parameters.missingCodes).toEqual([-99]);
  });

  it('drops a tick with the last column that offered it, and does not restore it on re-adding the column', () => {
    render(<Harness />);
    tick('C4');
    tick('C2');
    fireEvent.click(codeBox(77) as HTMLInputElement);
    expect(spec().parameters.missingCodes).toEqual([77]);
    fireEvent.click(screen.getByTestId('rr-remove-C4'));
    expect(codeBox(77)).toBeNull();
    expect(spec().parameters.missingCodes).toEqual([]);
    tick('C4');
    expect((codeBox(77) as HTMLInputElement).checked).toBe(false);
    expect(spec().parameters.missingCodes).toEqual([]);
  });

  it('drops every tick with Remove all and with the left box', () => {
    render(<Harness />);
    tick('C4');
    tick('C2');
    fireEvent.click(codeBox(77) as HTMLInputElement);
    fireEvent.click(screen.getByTestId('rr-remove-all'));
    tick('C4');
    expect((codeBox(77) as HTMLInputElement).checked).toBe(false);
    fireEvent.click(codeBox(77) as HTMLInputElement);
    tick('C4');
    tick('C4');
    expect((codeBox(77) as HTMLInputElement).checked).toBe(false);
  });

  it('sends the ticked codes ascending and once each', () => {
    render(<Harness />);
    tick('C4');
    tick('C3');
    fireEvent.click(codeBox(99) as HTMLInputElement);
    fireEvent.click(codeBox(77) as HTMLInputElement);
    fireEvent.click(codeBox(-99) as HTMLInputElement);
    expect(spec().parameters.missingCodes).toEqual([-99, 77, 99]);
  });
});

describe('readiness', () => {
  const many = (n: number): DatasetView => ({
    ...view,
    profile: { rows: 100, columns: Array.from({ length: n }, (_, i) => col(`C${i + 1}`, `h${i + 1}`)) },
  });

  it('says what is missing at none and at one column', () => {
    render(<Harness />);
    expect(state?.stillMissing).toBe('Choose at least two columns.');
    expect(screen.getByTestId('rr-settings-missing').textContent).toBe('Choose at least two columns.');
    tick('C2');
    expect(state?.stillMissing).toBe('Choose at least two columns.');
  });

  it('is ready at two columns and at 50', () => {
    render(<Host v={many(50)} />);
    tick('C1');
    tick('C2');
    expect(state?.stillMissing).toBeNull();
    expect(screen.queryByTestId('rr-settings-missing')).toBeNull();
    fireEvent.click(screen.getByTestId('rr-add-all'));
    expect(state?.columns).toHaveLength(50);
    expect(state?.stillMissing).toBeNull();
  });

  it('says how many are chosen at 51', () => {
    render(<Host v={many(51)} />);
    fireEvent.click(screen.getByTestId('rr-add-all'));
    expect(state?.columns).toHaveLength(51);
    expect(state?.stillMissing).toBe('Choose 50 columns or fewer; 51 are chosen.');
    expect(screen.getByTestId('rr-settings-missing').textContent).toBe('Choose 50 columns or fewer; 51 are chosen.');
  });
});

describe('the difference line', () => {
  it('names the two columns in their order, follows a move, and is gone at three', () => {
    render(<Harness />);
    expect(screen.queryByTestId('rr-difference-line')).toBeNull();
    tick('C3');
    expect(screen.queryByTestId('rr-difference-line')).toBeNull();
    tick('C2');
    expect(screen.getByTestId('rr-difference-line').textContent).toBe('The differences are after less before.');
    fireEvent.click(up('C2'));
    expect(screen.getByTestId('rr-difference-line').textContent).toBe('The differences are before less after.');
    tick('C4');
    expect(screen.queryByTestId('rr-difference-line')).toBeNull();
  });

  it("names a score by its name, a label sentinel only as text", () => {
    const sentinel = 'LBL_SENT"<b>';
    const v: DatasetView = { ...view, profile: { rows: 100, columns: [col('C1', sentinel), col('C2', 'C2', { prepared: true, scoreName: 'wellbeing' })] } };
    const { container } = render(<Harness v={v} />);
    tick('C1');
    tick('C2');
    expect(screen.getByTestId('rr-difference-line').textContent).toBe(`The differences are ${sentinel} less wellbeing.`);
    for (const el of Array.from(container.querySelectorAll('*'))) {
      for (const a of Array.from(el.attributes)) expect(a.value, a.name).not.toContain('LBL_SENT');
    }
  });
});

describe('the spec', () => {
  it("lists the columns in the list's order after a move", () => {
    render(<Harness />);
    tick('C2');
    tick('C3');
    tick('C4');
    fireEvent.click(down('C2'));
    expect(spec().parameters.columns).toEqual(['C3', 'C2', 'C4']);
  });

  it("carries the view's preparation on a score, and none on a file's own table", () => {
    const preparation = { kind: 'recipe' } as never;
    const prepared: DatasetView = { ...view, preparation };
    render(<Harness v={prepared} />);
    tick('C2');
    tick('C5');
    expect(spec().preparation).toBe(preparation);
    cleanup();
    render(<Harness />);
    tick('C2');
    tick('C3');
    expect('preparation' in spec()).toBe(false);
  });

  it('reset drops every choice', () => {
    render(<Harness />);
    tick('C2');
    tick('C3');
    fireEvent.click(codeBox(-99) as HTMLInputElement);
    act(() => state?.reset());
    expect(state?.columns).toEqual([]);
    expect(state?.ticked).toEqual([]);
  });
});
