// The factor analysis's settings (Research OS, Phase 9B-3, decisions 729 and 730): the items
// AlphaSettings.tsx's two panes, the right one in the file's order whatever order they were
// ticked in; the extraction, the number of factors and the rotation as radios; the iterations
// and the convergence typed; the codes over the drawn items.
import React from 'react';
import { act, cleanup, fireEvent, render, screen, within } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';

import type { ColumnProfile, DatasetView } from './describeTypes';
import { FactorSettings, buildFactorSpec, useFactorSettings, type FactorSettingsState } from './FactorSettings';
import { ALPHA_ADD_ALL, ALPHA_COLUMNS_HEADING, ALPHA_ITEMS_HEADING, ALPHA_ITEMS_NONE, ALPHA_REMOVE_ALL, ALPHA_REMOVE_ITEM } from './window/windowWording';

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

const inputRef = { contentHash: 'ab'.repeat(32), rows: 100, columns: 6, semanticRole: 'table' as const };
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
      col('C6', 'C6', { prepared: true, scoreName: 'wellbeing' }),
    ],
  },
  inputRef,
};

/** A view of n numeric items, C1 to Cn. */
const wide = (n: number): DatasetView => ({
  ...view,
  profile: { rows: 100, columns: Array.from({ length: n }, (_, i) => col(`C${i + 1}`, `x${i + 1}`)) },
});

let state: FactorSettingsState | null = null;
const Harness: React.FC<{ v?: DatasetView }> = ({ v = view }) => {
  const s = useFactorSettings(v);
  state = s;
  return <FactorSettings view={v} settings={s} />;
};

const tick = (alias: string) => fireEvent.click(screen.getByTestId(`fa-column-${alias}`));
const list = () => screen.queryAllByTestId(/^fa-row-C/).map((r) => r.getAttribute('data-testid')?.replace('fa-row-', ''));
const codeBox = (c: number) => screen.queryByTestId(`fa-code-${c}`) as HTMLInputElement | null;
const spec = () => (state as FactorSettingsState).spec(1);
const type = (id: string, text: string) => fireEvent.change(screen.getByTestId(id), { target: { value: text } });
const radio = (id: string) => screen.getByTestId(id) as HTMLInputElement;
const labelOf = (id: string) => (screen.getByTestId(id).closest('label')?.textContent ?? '').trim();

describe('the two panes', () => {
  it("offers the numeric columns in the file's order, with nothing chosen, under alpha's words", () => {
    render(<Harness />);
    expect(within(screen.getByTestId('fa-columns')).getByRole('heading').textContent).toBe(ALPHA_COLUMNS_HEADING);
    expect(within(screen.getByTestId('fa-list')).getByRole('heading').textContent).toBe(ALPHA_ITEMS_HEADING);
    expect(screen.getByTestId('fa-none').textContent).toBe(ALPHA_ITEMS_NONE);
    expect(screen.getByTestId('fa-add-all').textContent).toBe(ALPHA_ADD_ALL);
    expect(screen.getByTestId('fa-remove-all').textContent).toBe(ALPHA_REMOVE_ALL);
    const boxes = within(screen.getByTestId('fa-columns')).getAllByRole('checkbox');
    expect(boxes.map((b) => b.getAttribute('data-testid'))).toEqual(['fa-column-C2', 'fa-column-C3', 'fa-column-C4', 'fa-column-C5', 'fa-column-C6']);
    expect(boxes.every((b) => !(b as HTMLInputElement).checked)).toBe(true);
  });

  it("draws the chosen items in the file's order after ticks in another order, and sends them so", () => {
    render(<Harness />);
    tick('C5');
    tick('C2');
    tick('C6');
    tick('C3');
    expect(list()).toEqual(['C2', 'C3', 'C5', 'C6']);
    expect(spec().parameters.items).toEqual(['C2', 'C3', 'C5', 'C6']);
  });

  it('adds every numeric column, a score among them, and empties the list with Remove all or Remove', () => {
    render(<Harness />);
    fireEvent.click(screen.getByTestId('fa-add-all'));
    expect(list()).toEqual(['C2', 'C3', 'C4', 'C5', 'C6']);
    const remove = screen.getByTestId('fa-remove-C4');
    expect(remove.textContent).toBe(ALPHA_REMOVE_ITEM);
    expect(remove.getAttribute('aria-label')).toBe(`${ALPHA_REMOVE_ITEM} C4`);
    fireEvent.click(remove);
    expect(list()).toEqual(['C2', 'C3', 'C5', 'C6']);
    fireEvent.click(screen.getByTestId('fa-remove-all'));
    expect(list()).toEqual([]);
    expect(screen.getByTestId('fa-none')).toBeTruthy();
  });
});

describe('the radios (decision 730)', () => {
  it('start at principal components, Kaiser\'s rule and no rotation, under their words', () => {
    render(<Harness />);
    expect(screen.getByTestId('fa-extraction').querySelector('legend')?.textContent).toBe('Extraction');
    expect(labelOf('fa-extraction-principal_components')).toBe('Principal components');
    expect(labelOf('fa-extraction-principal_axis')).toBe('Principal axis factoring');
    expect(screen.getByTestId('fa-factors').querySelector('legend')?.textContent).toBe('Number of factors');
    expect(labelOf('fa-factors-kaiser')).toBe('Every factor whose eigenvalue is above 1');
    expect(labelOf('fa-factors-fixed')).toBe('A fixed number');
    expect(screen.getByTestId('fa-rotation').querySelector('legend')?.textContent).toBe('Rotation');
    expect(['none', 'varimax', 'promax'].map((r) => labelOf(`fa-rotation-${r}`))).toEqual(['None', 'Varimax', 'Promax (power 4)']);
    expect(radio('fa-extraction-principal_components').checked).toBe(true);
    expect(radio('fa-factors-kaiser').checked).toBe(true);
    expect(radio('fa-rotation-none').checked).toBe(true);
    expect(screen.queryByTestId('fa-factors-number')).toBeNull();
    const p = spec().parameters;
    expect([p.extraction, p.factors, p.rotation, p.iterations, p.convergence]).toEqual(['principal_components', null, 'none', 25, 0.001]);
  });

  it('send what each radio says', () => {
    render(<Harness />);
    fireEvent.click(radio('fa-extraction-principal_axis'));
    fireEvent.click(radio('fa-rotation-promax'));
    expect(spec().parameters.extraction).toBe('principal_axis');
    expect(spec().parameters.rotation).toBe('promax');
    fireEvent.click(radio('fa-rotation-varimax'));
    expect(spec().parameters.rotation).toBe('varimax');
    fireEvent.click(radio('fa-factors-fixed'));
    expect(labelOf('fa-factors-number')).toBe('Fixed number of factors');
    type('fa-factors-number', '2');
    expect(spec().parameters.factors).toBe(2);
    fireEvent.click(radio('fa-factors-kaiser'));
    expect(spec().parameters.factors).toBeNull();
  });
});

describe('the typed numbers and readiness (decision 730)', () => {
  it('needs 3 to 128 items', () => {
    const { rerender } = render(<Harness v={wide(129)} />);
    expect(state?.stillMissing).toBe('Choose at least 3 items.');
    tick('C1');
    tick('C2');
    expect(state?.stillMissing).toBe('Choose at least 3 items.');
    tick('C3');
    expect(state?.stillMissing).toBeNull();
    fireEvent.click(screen.getByTestId('fa-add-all'));
    expect(state?.items).toHaveLength(129);
    expect(state?.stillMissing).toBe('Choose 128 items or fewer; 129 are chosen.');
    tick('C129');
    expect(state?.items).toHaveLength(128);
    expect(state?.stillMissing).toBeNull();
    rerender(<Harness v={wide(129)} />);
  });

  it("follows the extraction and the drawn items in the fixed number's range", () => {
    render(<Harness />);
    tick('C2');
    tick('C3');
    tick('C4');
    tick('C5');
    fireEvent.click(radio('fa-factors-fixed'));
    expect(state?.stillMissing).toBe('Enter a whole number of factors from 1 to 4.');
    type('fa-factors-number', '4');
    expect(state?.stillMissing).toBeNull();
    fireEvent.click(radio('fa-extraction-principal_axis'));
    expect(state?.stillMissing).toBe('Enter a whole number of factors from 1 to 3.');
    type('fa-factors-number', '3');
    expect(state?.stillMissing).toBeNull();
    tick('C6');
    type('fa-factors-number', '4');
    expect(state?.stillMissing).toBeNull();
    for (const bad of ['0', '5', '2.5', '-1', 'two', '']) {
      type('fa-factors-number', bad);
      expect(state?.stillMissing, bad).toBe('Enter a whole number of factors from 1 to 4.');
    }
    fireEvent.click(radio('fa-factors-kaiser'));
    expect(state?.stillMissing).toBeNull();
  });

  it('takes iterations from 1 to 1000 and a convergence from 1e-12 to 0.1', () => {
    render(<Harness />);
    tick('C2');
    tick('C3');
    tick('C4');
    expect(labelOf('fa-iterations')).toBe('Maximum iterations, for the extraction and the rotation');
    expect(labelOf('fa-convergence')).toBe('Convergence, for principal axis factoring');
    expect((screen.getByTestId('fa-iterations') as HTMLInputElement).value).toBe('25');
    expect((screen.getByTestId('fa-convergence') as HTMLInputElement).value).toBe('0.001');
    for (const [text, ok] of [['0', false], ['1', true], ['1000', true], ['1001', false], ['2.5', false], ['', false]] as const) {
      type('fa-iterations', text);
      expect(state?.stillMissing, text).toBe(ok ? null : 'Enter a whole number of iterations from 1 to 1000.');
    }
    type('fa-iterations', '1000');
    expect(spec().parameters.iterations).toBe(1000);
    for (const [text, ok] of [['1e-12', true], ['0.1', true], ['1e-13', false], ['0.2', false], ['0', false], ['x', false], ['', false]] as const) {
      type('fa-convergence', text);
      expect(state?.stillMissing, text).toBe(ok ? null : 'Enter a convergence from 1e-12 to 0.1.');
    }
    type('fa-convergence', '0.1');
    expect(spec().parameters.convergence).toBe(0.1);
  });
});

describe('the codes', () => {
  it('offers the union over the drawn items, ascending, each with where it appears', () => {
    render(<Harness />);
    tick('C3');
    tick('C2');
    tick('C4');
    expect(screen.getByTestId('fa-missing-codes').querySelector('legend')?.textContent).toBe('Candidate missing codes');
    const questions = within(screen.getByTestId('fa-missing-codes'))
      .getAllByRole('checkbox')
      .map((b) => (b.closest('label')?.textContent ?? '').trim());
    const after = 'If you tick it, it is treated as missing in every item, and a row with it is left out.';
    expect(questions).toEqual([
      `Is -99 a code for a missing value? It appears in "q1" (2 cells), "q2" (1 cell). ${after}`,
      `Is 77 a code for a missing value? It appears in "q3" (4 cells). ${after}`,
      `Is 99 a code for a missing value? It appears in "q2" (1 cell). ${after}`,
    ]);
    fireEvent.click(codeBox(99) as HTMLInputElement);
    fireEvent.click(codeBox(-99) as HTMLInputElement);
    expect(spec().parameters.missingCodes).toEqual([-99, 99]);
  });

  it('drops a tick with the last item that offered it', () => {
    render(<Harness />);
    tick('C2');
    tick('C3');
    tick('C5');
    fireEvent.click(codeBox(-99) as HTMLInputElement);
    tick('C2');
    expect(spec().parameters.missingCodes).toEqual([-99]);
    tick('C3');
    expect(codeBox(-99)).toBeNull();
    expect(state?.ticked).toEqual([]);
    tick('C3');
    expect((codeBox(-99) as HTMLInputElement).checked).toBe(false);
  });

  it('prunes a tick when a view change takes its code away, and the code returns unticked', () => {
    const without = (alias: string): DatasetView => ({ ...view, profile: { ...view.profile, columns: view.profile.columns.filter((c) => c.alias !== alias) } });
    const { rerender } = render(<Harness v={view} />);
    tick('C2');
    tick('C4');
    tick('C5');
    fireEvent.click(codeBox(77) as HTMLInputElement);
    expect(spec().parameters.missingCodes).toEqual([77]);
    rerender(<Harness v={without('C4')} />);
    expect(list()).toEqual(['C2', 'C5']);
    expect(spec().parameters.missingCodes).toEqual([]);
    rerender(<Harness v={view} />);
    expect((codeBox(77) as HTMLInputElement).checked).toBe(false);
    expect(spec().parameters.missingCodes).toEqual([]);
  });
});

describe('the spec (decision 729)', () => {
  it("names the capability, its version and the view, the items in the file's order and the codes once each, ascending", () => {
    const s = buildFactorSpec(view, ['C5', 'C2', 'C3'], 'principal_axis', 2, 'promax', 25, 0.001, [99, -99, 99], 7);
    expect(s.capabilityId).toBe('stats.analyze.factor');
    expect(s.capabilityVersion).toBe(1);
    expect(s.operationId).toBe(7);
    expect(s.inputs).toEqual([view.inputRef]);
    expect(JSON.parse(JSON.stringify(s)).parameters).toEqual({
      items: ['C2', 'C3', 'C5'],
      extraction: 'principal_axis',
      factors: 2,
      rotation: 'promax',
      iterations: 25,
      convergence: 0.001,
      missingCodes: [-99, 99],
    });
    expect(Object.keys(s.parameters).sort()).toEqual(['convergence', 'extraction', 'factors', 'items', 'iterations', 'missingCodes', 'rotation']);
  });

  it("carries the view's preparation on a score, and none on a file's own table", () => {
    const preparation = { kind: 'scale_score' as const, sourceContentHash: 'cd'.repeat(32), operations: [] };
    const prepared = { ...view, preparation } as unknown as DatasetView;
    render(<Harness v={prepared} />);
    tick('C2');
    expect(spec().preparation).toBe(preparation);
    cleanup();
    render(<Harness v={view} />);
    tick('C2');
    expect('preparation' in spec()).toBe(false);
  });

  it('reset drops every choice and restores the starting values', () => {
    render(<Harness />);
    tick('C2');
    fireEvent.click(radio('fa-extraction-principal_axis'));
    fireEvent.click(radio('fa-factors-fixed'));
    type('fa-iterations', '9');
    act(() => state?.reset());
    expect(state?.items).toEqual([]);
    expect([state?.extraction, state?.fixed, state?.rotation, state?.iterationsText, state?.convergenceText]).toEqual(['principal_components', false, 'none', '25', '0.001']);
  });
});

describe('a column name', () => {
  it('appears only as text, never in a key, an attribute or a test id', () => {
    const sentinel = 'LABEL_SENTINEL_9b3';
    const v: DatasetView = { ...view, profile: { rows: 100, columns: [col('C1', sentinel), col('C2', 'C2', { suspectMissingCodes: [{ code: -99, cells: 1 }] }), col('C3', 'x')] } };
    const { container } = render(<Harness v={v} />);
    tick('C1');
    tick('C2');
    tick('C3');
    expect(container.textContent).toContain(sentinel);
    for (const el of Array.from(container.querySelectorAll('*'))) {
      for (const attr of Array.from(el.attributes)) expect(attr.value, attr.name).not.toContain(sentinel);
    }
  });
});
