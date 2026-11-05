// The alpha dialog (Research OS, Phase 8R, decisions 469 and 470; ADR-012 Part 6),
// over a mock bridge. The twins of AlphaFlow.vitest.tsx's six settings tests and
// its refusal: every assertion on al-item, al-reverse, al-scale, al-code,
// al-settings-missing and the spec is that file's, and only the lines that reach
// the settings differ: toSettings and al-stage-heading become the dialog's
// opening, al-check becomes its Run, and al-back-settings goes, since a dialog
// that ran stays mounted here until its parent closes it. Then MethodDialog's own
// rule: a warning's second press, a refusal kept, a full Output, and no dialog
// ever closing the shared slot.
import fs from 'node:fs';
import path from 'node:path';
import React from 'react';
import { cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { makeMockAlphaBridge } from '../alphaBridge';
import type { AlphaRunStateView, AlphaSpec, AlphaValidationView } from '../alphaTypes';
import type { ColumnProfile, DatasetView } from '../describeTypes';
import { AlphaDialog } from './AlphaDialog';
import { OUTPUT_FULL, RUN_WITH_WARNINGS } from './windowWording';

afterEach(cleanup);

const INPUT = { contentHash: 'ab'.repeat(32), rows: 250, columns: 5, semanticRole: 'table' as const };

const col = (alias: string, header: string, codes: Array<[number, number]> = [], kind: ColumnProfile['kind'] = 'numeric'): ColumnProfile => ({
  alias,
  header,
  kind,
  n: 250,
  missing: 0,
  distinct: kind === 'numeric' ? 5 : 3,
  levels: kind === 'numeric' ? null : [{ label: 'x', count: 250 }],
  suspectMissingCodes: codes.map(([code, cells]) => ({ code, cells })),
  nonFinite: 0,
});

const view: DatasetView = {
  slot: 3,
  displayName: 'scale.csv',
  profile: {
    rows: 250,
    columns: [
      col('C1', 'Q1', [[-9, 2]]),
      col('C2', 'Group', [], 'categorical'),
      col('C3', 'Q3', [[99, 4]]),
      col('C4', 'Q4', [
        [99, 1],
        [-99, 3],
      ]),
      col('C5', 'Q5'),
    ],
  },
  inputRef: INPUT,
};

const report = (status: AlphaValidationView['status'], errors: AlphaValidationView['errors'] = [], warnings: AlphaValidationView['warnings'] = []): AlphaValidationView => ({
  status,
  errors,
  warnings,
  missingInformation: [],
  estimatedResources: { rows: 250, columns: 4, cells: 1000 },
});

const state: AlphaRunStateView = { runId: 5, state: 'running', terminal: false, elapsedMs: 10, cancelRequested: false, stepsDone: 1, stepsTotal: 7, diagnostics: [] };

function setup(responses: Record<string, unknown> = {}, over: { outputFull?: boolean; view?: DatasetView } = {}) {
  const bridge = makeMockAlphaBridge({ stats_alpha_run: { runId: 5 }, stats_alpha_status: state, ...responses });
  const onStarted = vi.fn();
  const onCancel = vi.fn();
  const utils = render(<AlphaDialog view={over.view ?? view} slot={3} bridge={bridge} outputFull={over.outputFull ?? false} onStarted={onStarted} onCancel={onCancel} />);
  return { bridge, onStarted, onCancel, ...utils };
}

const specs = (bridge: ReturnType<typeof makeMockAlphaBridge>, cmd: string) => bridge.calls.filter(([c]) => c === cmd).map(([, a]) => a.spec as AlphaSpec);
const box = (id: string) => screen.getByTestId(id) as HTMLInputElement;
const run = () => screen.getByTestId('al-run') as HTMLButtonElement;
const type = (id: string, text: string) => fireEvent.change(screen.getByTestId(id), { target: { value: text } });

describe('the twins of AlphaFlow.vitest.tsx', () => {
  it('opens with no item and no Reverse-score box ticked and no scale field, and Run off with what is missing', () => {
    setup({});
    for (const a of ['C1', 'C3', 'C4', 'C5']) expect(box(`al-item-${a}`).checked, a).toBe(false);
    expect(screen.queryByTestId('al-item-C2'), 'a text column is not offered').toBeNull();
    expect(screen.queryAllByTestId(/^al-reverse-/), 'no Reverse-score box before an item is ticked').toHaveLength(0);
    expect(screen.queryByTestId('al-scale')).toBeNull();
    expect(run().disabled).toBe(true);
    expect(screen.getByTestId('al-settings-missing').textContent).toBe('To check the settings, choose two items.');
    fireEvent.click(screen.getByTestId('al-item-C3'));
    expect(box('al-reverse-C3').checked, 'beside a ticked item, unticked').toBe(false);
    expect(screen.getByTestId('al-settings-missing').textContent).toBe('To check the settings, choose one more item.');
    fireEvent.click(screen.getByTestId('al-item-C1'));
    expect(box('al-reverse-C1').checked).toBe(false);
    expect(screen.queryByTestId('al-settings-missing')).toBeNull();
    expect(run().disabled).toBe(false);
  });

  it('shows the scale fields empty when an item is reverse-scored, and Run waits for both ends, the lowest below', () => {
    setup({});
    fireEvent.click(screen.getByTestId('al-item-C1'));
    fireEvent.click(screen.getByTestId('al-item-C3'));
    fireEvent.click(screen.getByTestId('al-reverse-C3'));
    const scale = screen.getByTestId('al-scale');
    expect(box('al-scale-min').value, 'never filled in').toBe('');
    expect(box('al-scale-max').value, 'never filled in').toBe('');
    expect(within(scale).getByText('Lowest value on the scale')).toBeTruthy();
    expect(within(scale).getByText('Highest value on the scale')).toBeTruthy();
    expect(screen.getByTestId('al-reverse-sentence').textContent).toBe(
      'Reverse-scoring turns each value x into lowest + highest − x. Type the ends your questionnaire allows, for example 1 and 5, not the lowest and highest answers in your data. Every reverse-scored item uses these ends.'
    );
    expect(run().disabled).toBe(true);
    expect(screen.getByTestId('al-settings-missing').textContent).toBe(
      'To check the settings, choose a number for the lowest value on the scale and a number for the highest value on the scale.'
    );
    type('al-scale-min', '1');
    expect(run().disabled).toBe(true);
    type('al-scale-max', '1');
    expect(run().disabled).toBe(true);
    expect(screen.getByTestId('al-settings-missing').textContent).toBe('To check the settings, type a lowest value on the scale that is below the highest.');
    type('al-scale-max', '5');
    expect(run().disabled).toBe(false);
    // Unticking the item unticks its box, and the fields go with the last.
    fireEvent.click(screen.getByTestId('al-item-C3'));
    expect(screen.queryByTestId('al-reverse-C3')).toBeNull();
    expect(screen.queryByTestId('al-scale')).toBeNull();
    fireEvent.click(screen.getByTestId('al-item-C3'));
    expect(box('al-reverse-C3').checked, 'ticked again, the item is not reversed').toBe(false);
  });

  it('gives each scale field its own visible border, padding and width', () => {
    setup({});
    fireEvent.click(screen.getByTestId('al-item-C1'));
    fireEvent.click(screen.getByTestId('al-item-C3'));
    fireEvent.click(screen.getByTestId('al-reverse-C3'));
    for (const id of ['al-scale-min', 'al-scale-max']) {
      const s = box(id).style;
      expect(s.borderStyle, `${id} border style`).toBe('solid');
      expect(parseFloat(s.borderWidth), `${id} border width`).toBeGreaterThan(0);
      expect(s.borderColor, `${id} border colour`).not.toBe('');
      expect(parseFloat(s.paddingTop), `${id} padding`).toBeGreaterThan(0);
      expect(parseFloat(s.width), `${id} width`).toBeGreaterThan(0);
      expect(s.background || s.backgroundColor, `${id} background`).not.toBe('');
    }
  });

  it('ticks and unticks every numeric column only when asked, and sends one spec per choice', async () => {
    const { bridge } = setup({ stats_alpha_validate: report('valid') });
    fireEvent.click(screen.getByTestId('al-tick-all'));
    for (const a of ['C1', 'C3', 'C4', 'C5']) expect(box(`al-item-${a}`).checked, a).toBe(true);
    for (const a of ['C1', 'C3', 'C4', 'C5']) expect(box(`al-reverse-${a}`).checked, `${a} not reversed by Add every`).toBe(false);
    fireEvent.click(screen.getByTestId('al-untick-all'));
    for (const a of ['C1', 'C3', 'C4', 'C5']) expect(box(`al-item-${a}`).checked, a).toBe(false);
    // Ticked out of order: the spec is in the file's order and the codes
    // ascending.
    for (const a of ['C5', 'C1', 'C4']) fireEvent.click(screen.getByTestId(`al-item-${a}`));
    fireEvent.click(screen.getByTestId('al-reverse-C5'));
    fireEvent.click(screen.getByTestId('al-reverse-C4'));
    type('al-scale-min', '1');
    type('al-scale-max', '5');
    fireEvent.click(screen.getByTestId('al-code-99'));
    fireEvent.click(screen.getByTestId('al-code--9'));
    fireEvent.click(run());
    await waitFor(() => expect(specs(bridge, 'stats_alpha_validate')).toHaveLength(1));
    expect(specs(bridge, 'stats_alpha_validate')[0].parameters).toEqual({
      items: ['C1', 'C4', 'C5'],
      reversed: ['C4', 'C5'],
      scale: { min: 1, max: 5 },
      missingCodes: [-9, 99],
    });
  });

  it('sends no scale when nothing is reverse-scored, even after ends were typed', async () => {
    const { bridge } = setup({ stats_alpha_validate: report('valid') });
    fireEvent.click(screen.getByTestId('al-item-C1'));
    fireEvent.click(screen.getByTestId('al-item-C3'));
    fireEvent.click(screen.getByTestId('al-reverse-C3'));
    type('al-scale-min', '1');
    type('al-scale-max', '5');
    fireEvent.click(screen.getByTestId('al-reverse-C3'));
    expect(screen.queryByTestId('al-scale')).toBeNull();
    fireEvent.click(run());
    await waitFor(() => expect(specs(bridge, 'stats_alpha_validate')).toHaveLength(1));
    expect(specs(bridge, 'stats_alpha_validate')[0].parameters).toEqual({
      items: ['C1', 'C3'],
      reversed: [],
      scale: null,
      missingCodes: [],
    });
    // Reverse-scored again: the fields are empty, not the ends typed before.
    fireEvent.click(screen.getByTestId('al-reverse-C1'));
    expect(box('al-scale-min').value).toBe('');
    expect(box('al-scale-max').value).toBe('');
  });

  it('offers the candidate codes of the ticked items only, ascending, each with where it appears', () => {
    setup({});
    expect(screen.queryByTestId('al-missing-codes')).toBeNull();
    fireEvent.click(screen.getByTestId('al-item-C4'));
    fireEvent.click(screen.getByTestId('al-item-C3'));
    const offered = Array.from(screen.getByTestId('al-missing-codes').querySelectorAll('input')).map((i) => i.getAttribute('data-testid'));
    expect(offered).toEqual(['al-code--99', 'al-code-99']);
    expect(screen.getByTestId('al-missing-codes').textContent).toContain('It appears in "Q3" (4 cells), "Q4" (1 cell).');
    fireEvent.click(screen.getByTestId('al-code--99'));
    fireEvent.click(screen.getByTestId('al-item-C4'));
    expect(screen.queryByTestId('al-code--99')).toBeNull();
  });

  it('shows a value outside the scale as one sentence per column, with the header and the count, and never runs', async () => {
    const { bridge, onStarted } = setup({
      stats_alpha_validate: report('invalid', [
        { code: 'stats.alpha.reversed_outside_scale', params: { column: 3, count: 12 }, severity: 'error' },
        { code: 'stats.alpha.reversed_outside_scale', params: { column: 4, count: 1 }, severity: 'error' },
      ]),
    });
    fireEvent.click(screen.getByTestId('al-tick-all'));
    fireEvent.click(run());
    const list = await screen.findByTestId('al-validation-errors');
    const errors = Array.from(list.querySelectorAll('li')).map((li) => li.textContent);
    expect(errors).toEqual([
      "\"Q3\" has 12 values outside the scale's ends you typed, once the ticked missing codes are left out. Check the ends, or tick a missing code.",
      "\"Q4\" has 1 value outside the scale's ends you typed, once the ticked missing codes are left out. Check the ends, or tick a missing code.",
    ]);
    expect(run().disabled).toBe(true);
    fireEvent.click(run());
    expect(bridge.calls.filter(([c]) => c === 'stats_alpha_run')).toEqual([]);
    expect(onStarted).not.toHaveBeenCalled();
  });
});

describe('the list on the right', () => {
  it("says nothing is chosen, then lists the items in the file's order, whatever the click order, each with Remove", () => {
    setup({});
    expect(screen.getByTestId('al-items-list').textContent).toContain("Items, in the file's order");
    expect(screen.getByTestId('al-items-none').textContent).toBe('None yet. Nothing is chosen for you.');
    for (const a of ['C5', 'C1']) fireEvent.click(screen.getByTestId(`al-item-${a}`));
    expect(screen.queryByTestId('al-items-none')).toBeNull();
    const shown = Array.from(screen.getByTestId('al-items-list').querySelectorAll('[data-testid^="al-chosen-"]')).map((e) => e.getAttribute('data-testid'));
    expect(shown).toEqual(['al-chosen-C1', 'al-chosen-C5']);
  });

  it('Remove unticks the item and its reversal, and the scale goes with the last', () => {
    setup({});
    for (const a of ['C1', 'C3']) fireEvent.click(screen.getByTestId(`al-item-${a}`));
    fireEvent.click(screen.getByTestId('al-reverse-C3'));
    type('al-scale-min', '1');
    fireEvent.click(screen.getByTestId('al-remove-C3'));
    expect(box('al-item-C3').checked).toBe(false);
    expect(screen.queryByTestId('al-scale')).toBeNull();
    fireEvent.click(screen.getByTestId('al-item-C3'));
    expect(box('al-reverse-C3').checked, 'not reversed when added again').toBe(false);
  });

  it("the buttons' words, and a score by the name the user typed", () => {
    const withScore: DatasetView = {
      ...view,
      profile: { rows: 250, columns: [...view.profile.columns, { ...col('C6', 'C6'), prepared: true, scoreName: 'Total anxiety' }] },
    };
    setup({}, { view: withScore });
    expect(screen.getByTestId('al-tick-all').textContent).toBe('Add every numeric column');
    expect(screen.getByTestId('al-untick-all').textContent).toBe('Remove all');
    expect(screen.getByTestId('al-columns').textContent).toContain('C6 Total anxiety');
    fireEvent.click(screen.getByTestId('al-item-C6'));
    expect(screen.getByTestId('al-chosen-C6').textContent).toContain('Total anxiety');
  });
});

describe("MethodDialog's rule", () => {
  const twoItems = () => {
    for (const a of ['C1', 'C3']) fireEvent.click(screen.getByTestId(`al-item-${a}`));
  };

  it('valid: validates, runs and hands the run to its parent, once', async () => {
    const { bridge, onStarted } = setup({ stats_alpha_validate: report('valid') });
    twoItems();
    fireEvent.click(run());
    await waitFor(() => expect(onStarted).toHaveBeenCalledTimes(1));
    expect(specs(bridge, 'stats_alpha_run')).toHaveLength(1);
    expect(bridge.calls.filter(([c]) => c === 'stats_alpha_run')[0][1].slot).toBe(3);
    const started = onStarted.mock.calls[0][0];
    expect(started.runId).toBe(5);
    expect(started.run).toEqual(state);
    expect(started.settings).toContain('C1 Q1, C3 Q3');
  });

  it('a warning stays open and shows it, Run says so, and a second press runs it', async () => {
    const warned = report('valid_with_warnings', [], [{ code: 'stats.alpha.reversed_outside_scale', params: { column: 3, count: 2 }, severity: 'warning' }]);
    const { bridge, onStarted } = setup({ stats_alpha_validate: warned });
    twoItems();
    expect(run().textContent).toBe('Run');
    fireEvent.click(run());
    const list = await screen.findByTestId('al-validation-warnings');
    expect(list.querySelectorAll('li')).toHaveLength(1);
    expect(screen.getByTestId('al-validation-status').textContent).toBe('The settings can run. Read the notes below first.');
    await waitFor(() => expect(run().textContent).toBe(RUN_WITH_WARNINGS));
    expect(run().textContent).toBe('Run with these warnings');
    expect(specs(bridge, 'stats_alpha_run')).toEqual([]);
    expect(onStarted).not.toHaveBeenCalled();
    fireEvent.click(run());
    await waitFor(() => expect(onStarted).toHaveBeenCalledTimes(1));
    expect(specs(bridge, 'stats_alpha_run')).toHaveLength(1);
    expect(specs(bridge, 'stats_alpha_validate')).toHaveLength(1);
  });

  it('a changed setting takes the warning back: the next press checks again and does not run', async () => {
    const warned = report('valid_with_warnings', [], [{ code: 'stats.alpha.reversed_outside_scale', params: { column: 3, count: 2 }, severity: 'warning' }]);
    const { bridge, onStarted } = setup({ stats_alpha_validate: warned });
    twoItems();
    fireEvent.click(run());
    await screen.findByTestId('al-validation-warnings');
    await waitFor(() => expect(run().textContent).toBe(RUN_WITH_WARNINGS));
    fireEvent.click(screen.getByTestId('al-item-C4'));
    expect(run().textContent).toBe('Run');
    expect(screen.queryByTestId('al-validation-warnings')).toBeNull();
    fireEvent.click(run());
    await waitFor(() => expect(specs(bridge, 'stats_alpha_validate')).toHaveLength(2));
    expect(specs(bridge, 'stats_alpha_run')).toEqual([]);
    expect(onStarted).not.toHaveBeenCalled();
  });

  it('a refused run stays open with its sentence, and nothing is handed on', async () => {
    const busy = () => {
      throw new Error(JSON.stringify({ code: 'run.run_busy' }));
    };
    const { onStarted } = setup({ stats_alpha_validate: report('valid'), stats_alpha_run: busy });
    twoItems();
    fireEvent.click(run());
    const said = await screen.findByTestId('sw-dialog-refusal');
    expect(said.textContent).not.toBe('');
    expect(screen.getByTestId('sw-method-dialog')).toBeTruthy();
    expect(onStarted).not.toHaveBeenCalled();
  });

  it('with the Output full, Run says so and nothing is validated or run', async () => {
    const { bridge, onStarted } = setup({}, { outputFull: true });
    twoItems();
    fireEvent.click(run());
    expect((await screen.findByTestId('sw-dialog-refusal')).textContent).toBe(OUTPUT_FULL);
    expect(OUTPUT_FULL).toBe('The Output is full. Remove a result you have kept, then run this.');
    expect(bridge.calls).toEqual([]);
    expect(onStarted).not.toHaveBeenCalled();
  });

  it('Cancel and Escape each ask the parent to drop it', () => {
    const { onCancel } = setup({});
    fireEvent.click(screen.getByTestId('al-dialog-cancel'));
    expect(onCancel).toHaveBeenCalledTimes(1);
    fireEvent.keyDown(screen.getByTestId('sw-method-dialog'), { key: 'Escape' });
    expect(onCancel).toHaveBeenCalledTimes(2);
  });

  it('never closes the slot: not on mount, Cancel, Run or unmount', async () => {
    const { bridge, onStarted, unmount } = setup({ stats_alpha_validate: report('valid') });
    const closes = () => bridge.calls.filter(([c]) => c === 'stats_desc_close');
    expect(closes()).toEqual([]);
    twoItems();
    fireEvent.click(run());
    await waitFor(() => expect(onStarted).toHaveBeenCalled());
    expect(closes()).toEqual([]);
    fireEvent.click(screen.getByTestId('al-dialog-cancel'));
    expect(closes()).toEqual([]);
    unmount();
    expect(closes()).toEqual([]);
  });

  it('the Run and Cancel buttons sit outside the part that scrolls', () => {
    setup({});
    const scroller = screen.getByTestId('sw-dialog-scroll');
    expect(scroller.contains(screen.getByTestId('al-settings'))).toBe(true);
    expect(scroller.contains(run())).toBe(false);
    expect(scroller.contains(screen.getByTestId('al-dialog-cancel'))).toBe(false);
    const css = fs.readFileSync(path.join(__dirname, 'MethodDialog.css'), 'utf8');
    expect(css).toMatch(/\[data-testid='sw-method-dialog'\][^{]*\{[^}]*max-width:\s*min\(800px, 90vw\)/);
    expect(css).toMatch(/\[data-testid='sw-method-dialog'\][^{]*\{[^}]*max-height:\s*min\(640px, 90vh\)/);
    expect(css).toMatch(/\.sw-dialog__scroll[^{]*\{[^}]*overflow:\s*auto/);
  });
});

describe('one copy of the settings', () => {
  const src = (f: string) => fs.readFileSync(path.join(__dirname, f), 'utf8');

  it('the page and the dialog each render AlphaSettings, by import, and neither holds a control', () => {
    for (const f of ['../AlphaFlow.tsx', 'AlphaDialog.tsx']) {
      expect(src(f), f).toMatch(/import \{[^}]*\bAlphaSettings\b[^}]*\buseAlphaSettings\b[^}]*\} from '[^']*AlphaSettings'/);
      expect(src(f), f).toMatch(/<AlphaSettings\b/);
      expect(src(f), f).not.toMatch(/al-item-\$\{c\.|al-reverse-|al-scale|al-code-/);
    }
    expect(src('../AlphaSettings.tsx')).toMatch(/al-item-/);
  });

  it('its bridge type omits open and close, and it names no close', () => {
    const typed = src('AlphaDialog.tsx');
    expect(typed).toMatch(/Omit<AlphaBridge, 'open' \| 'close'>/);
    for (const f of ['AlphaDialog.tsx', 'MethodDialog.tsx']) expect(src(f), f).not.toMatch(/\.close\(|stats_desc_close/);
  });
});
