// Cronbach's alpha's screen (Research OS, Phase 8I-2, decisions 380, 383
// and 384), over a mock bridge:
// * no item and no Reverse-score box ticked, and no scale field shown, when
//   the settings open; Check stays off, with a sentence saying what is
//   missing; "Tick every numeric column" and "Untick all" are the user's
//   actions; unticking an item unticks its Reverse-score box;
// * the scale's two fields appear, empty, with the reversal's sentence, when
//   an item is reverse-scored, and Check stays off until both are numbers,
//   the lowest below the highest; they are never filled in;
// * one choice sends one spec, in the column and code order, with no scale
//   when nothing is reverse-scored;
// * the candidate missing codes are the union over the ticked items,
//   ascending;
// * a refusal at Check the settings shows its sentence, one per column with
//   the header and the count, and never starts a run;
// * the run's progress in steps, never columns, and Cancel;
// * the neutral card: the summary, the rows left out, the negative-alpha
//   note, the item table with "Reversed", every not-computable statistic as
//   its sentence, and no verdict word anywhere;
// * the render time at 128 items, measured and printed;
// * no path anywhere in the DOM.
import React from 'react';
import { cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import AlphaFlow, { AlphaResultCard } from './AlphaFlow';
import { makeMockAlphaBridge } from './alphaBridge';
import type { ColumnProfile, DatasetView } from './describeTypes';
import type { AlphaResultView, AlphaRunStateView, AlphaSpec, AlphaValidationView, ItemStats } from './alphaTypes';
import { alphaResultView, alphaResultViewOther } from './fixtures/alphaFixtures';
import { alphaSentence } from './alphaWording';

vi.mock('../../design-system/GaplyGlobe', () => ({ GaplyGlobe: () => <div /> }));

afterEach(cleanup);

/** The words a verdict on alpha would use (decision 384). */
const VERDICT = /\b(acceptable|unacceptable|good|excellent|questionable|poor|reliable|unreliable|adequate|sufficient)\b|0\.7\b|[*★☆]|\b(should|recommend|drop|remove)\b/i;

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

const report = (status: AlphaValidationView['status'], errors: AlphaValidationView['errors'] = []): AlphaValidationView => ({
  status,
  errors,
  warnings: [],
  missingInformation: [],
  estimatedResources: { rows: 250, columns: 4, cells: 1000 },
});

const state = (s: AlphaRunStateView['state'], terminal: boolean, done: number, cancelRequested = false): AlphaRunStateView => ({
  runId: 5,
  state: s,
  terminal,
  elapsedMs: 1200,
  cancelRequested,
  stepsDone: done,
  stepsTotal: 7,
  diagnostics: [],
});

const PATH = '/Users/someone/PATH_SENTINEL_a8/scale.csv';

function setup(responses: Record<string, unknown>) {
  const save = vi.fn().mockResolvedValue('/chosen/out.csv');
  const bridge = makeMockAlphaBridge({ stats_desc_open: view, stats_desc_close: true, ...responses });
  render(<AlphaFlow bridge={bridge} pickPath={async () => PATH} save={save} pollMs={10} limits={async () => null} />);
  return { bridge, save };
}

const heading = () => screen.getByTestId('al-stage-heading');
const specs = (bridge: ReturnType<typeof makeMockAlphaBridge>, cmd: string) =>
  bridge.calls.filter(([c]) => c === cmd).map(([, a]) => a.spec as AlphaSpec);
const box = (id: string) => screen.getByTestId(id) as HTMLInputElement;
const check = () => screen.getByTestId('al-check') as HTMLButtonElement;
const type = (id: string, text: string) => fireEvent.change(screen.getByTestId(id), { target: { value: text } });

async function toSettings() {
  fireEvent.click(screen.getByTestId('al-choose'));
  await waitFor(() => expect(heading().textContent).toBe('Profile of the file'));
  fireEvent.click(screen.getByTestId('al-to-settings'));
  await waitFor(() => expect(heading().textContent).toBe('Choose the items, any to reverse-score, and the missing codes'));
}

async function chooseAndCheck(bridge: ReturnType<typeof makeMockAlphaBridge>) {
  await toSettings();
  fireEvent.click(screen.getByTestId('al-tick-all'));
  fireEvent.click(check());
  await waitFor(() => expect(specs(bridge, 'stats_alpha_validate')).toHaveLength(1));
}

describe('AlphaFlow settings', () => {
  it('opens with no item and no Reverse-score box ticked and no scale field, and Check off with what is missing', async () => {
    setup({});
    await toSettings();
    for (const a of ['C1', 'C3', 'C4', 'C5']) expect(box(`al-item-${a}`).checked, a).toBe(false);
    expect(screen.queryByTestId('al-item-C2'), 'a text column is not offered').toBeNull();
    expect(screen.queryAllByTestId(/^al-reverse-/), 'no Reverse-score box before an item is ticked').toHaveLength(0);
    expect(screen.queryByTestId('al-scale')).toBeNull();
    expect(check().disabled).toBe(true);
    expect(screen.getByTestId('al-settings-missing').textContent).toBe('To check the settings, choose two items.');
    fireEvent.click(screen.getByTestId('al-item-C3'));
    expect(box('al-reverse-C3').checked, 'beside a ticked item, unticked').toBe(false);
    expect(screen.getByTestId('al-settings-missing').textContent).toBe('To check the settings, choose one more item.');
    fireEvent.click(screen.getByTestId('al-item-C1'));
    expect(box('al-reverse-C1').checked).toBe(false);
    expect(screen.queryByTestId('al-settings-missing')).toBeNull();
    expect(check().disabled).toBe(false);
  });

  it('shows the scale fields empty when an item is reverse-scored, and Check waits for both ends, the lowest below', async () => {
    setup({});
    await toSettings();
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
    expect(check().disabled).toBe(true);
    expect(screen.getByTestId('al-settings-missing').textContent).toBe(
      'To check the settings, choose a number for the lowest value on the scale and a number for the highest value on the scale.'
    );
    type('al-scale-min', '1');
    expect(check().disabled).toBe(true);
    type('al-scale-max', '1');
    expect(check().disabled).toBe(true);
    expect(screen.getByTestId('al-settings-missing').textContent).toBe(
      'To check the settings, type a lowest value on the scale that is below the highest.'
    );
    type('al-scale-max', '5');
    expect(check().disabled).toBe(false);
    // Unticking the item unticks its box, and the fields go with the last.
    fireEvent.click(screen.getByTestId('al-item-C3'));
    expect(screen.queryByTestId('al-reverse-C3')).toBeNull();
    expect(screen.queryByTestId('al-scale')).toBeNull();
    fireEvent.click(screen.getByTestId('al-item-C3'));
    expect(box('al-reverse-C3').checked, 'ticked again, the item is not reversed').toBe(false);
  });

  // Real-app check, Step 10 (2 Nov 2026): the two fields showed no box. The
  // app's global Tailwind preflight (src/tailwind.css, in every build) sets
  // `border: 0` and `padding: 0` on every input, and nothing in the design
  // system gives a text input its border back, so an unstyled field is an
  // invisible box on the dark panel. Each field must carry its own visible
  // border, padding and width, inline, where no stylesheet can remove them.
  it('gives each scale field its own visible border, padding and width', async () => {
    setup({});
    await toSettings();
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
    await toSettings();
    fireEvent.click(screen.getByTestId('al-tick-all'));
    for (const a of ['C1', 'C3', 'C4', 'C5']) expect(box(`al-item-${a}`).checked, a).toBe(true);
    for (const a of ['C1', 'C3', 'C4', 'C5']) expect(box(`al-reverse-${a}`).checked, `${a} not reversed by Tick every`).toBe(false);
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
    fireEvent.click(check());
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
    await toSettings();
    fireEvent.click(screen.getByTestId('al-item-C1'));
    fireEvent.click(screen.getByTestId('al-item-C3'));
    fireEvent.click(screen.getByTestId('al-reverse-C3'));
    type('al-scale-min', '1');
    type('al-scale-max', '5');
    fireEvent.click(screen.getByTestId('al-reverse-C3'));
    expect(screen.queryByTestId('al-scale')).toBeNull();
    fireEvent.click(check());
    await waitFor(() => expect(specs(bridge, 'stats_alpha_validate')).toHaveLength(1));
    expect(specs(bridge, 'stats_alpha_validate')[0].parameters).toEqual({
      items: ['C1', 'C3'],
      reversed: [],
      scale: null,
      missingCodes: [],
    });
    // Reverse-scored again: the fields are empty, not the ends typed before.
    fireEvent.click(screen.getByTestId('al-back-settings'));
    fireEvent.click(screen.getByTestId('al-reverse-C1'));
    expect(box('al-scale-min').value).toBe('');
    expect(box('al-scale-max').value).toBe('');
  });

  it('offers the candidate codes of the ticked items only, ascending, each with where it appears', async () => {
    setup({});
    await toSettings();
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
    const { bridge } = setup({
      stats_alpha_validate: report('invalid', [
        { code: 'stats.alpha.reversed_outside_scale', params: { column: 3, count: 12 }, severity: 'error' },
        { code: 'stats.alpha.reversed_outside_scale', params: { column: 4, count: 1 }, severity: 'error' },
      ]),
    });
    await chooseAndCheck(bridge);
    await waitFor(() => expect(heading().textContent).toBe('Check the settings'));
    const errors = Array.from(screen.getByTestId('al-validation-errors').querySelectorAll('li')).map((li) => li.textContent);
    expect(errors).toEqual([
      "\"Q3\" has 12 values outside the scale's ends you typed, once the ticked missing codes are left out. Check the ends, or tick a missing code.",
      "\"Q4\" has 1 value outside the scale's ends you typed, once the ticked missing codes are left out. Check the ends, or tick a missing code.",
    ]);
    const run = screen.getByTestId('al-run') as HTMLButtonElement;
    expect(run.disabled).toBe(true);
    fireEvent.click(run);
    expect(bridge.calls.filter(([c]) => c === 'stats_alpha_run')).toEqual([]);
  });

  it('shows the progress in steps while it runs, cancels, and shows the card', async () => {
    let polls = 0;
    const { bridge } = setup({
      stats_alpha_validate: report('valid'),
      stats_alpha_run: { runId: 5 },
      stats_alpha_status: () => (polls++ < 2 ? state('running', false, 3) : state('succeeded', true, 7)),
      stats_alpha_cancel: state('running', false, 3, true),
      stats_alpha_result: alphaResultView,
    });
    await chooseAndCheck(bridge);
    await screen.findByTestId('al-run');
    fireEvent.click(screen.getByTestId('al-run'));
    expect((await screen.findByTestId('al-progress')).textContent).toBe('3 of 7 steps');
    expect(screen.getByTestId('al-run-status').textContent).not.toMatch(/column/i);
    fireEvent.click(screen.getByTestId('al-cancel'));
    await waitFor(() => expect(bridge.calls.some(([c]) => c === 'stats_alpha_cancel')).toBe(true));
    await waitFor(() => expect(screen.getByTestId('al-progress').textContent).toBe('7 of 7 steps'));
    fireEvent.click(screen.getByTestId('al-see-results'));
    await screen.findByTestId('al-result-card');
    // The live screen names the result's aliases by the file's headers.
    expect(screen.getByTestId('al-items').textContent).toContain('"Q1"');
    expect(document.body.innerHTML).not.toContain('PATH_SENTINEL_a8');
    expect(document.body.textContent ?? '').not.toMatch(VERDICT);
  });
});

const names = { column: (a: number) => `C${a}` };
const row = (alias: string) => screen.getByTestId(`al-item-row-${alias}`);
const cells = (alias: string) => Array.from(row(alias).querySelectorAll('th, td')).map((c) => c.textContent);

describe('AlphaResultCard', () => {
  it('shows the summary, the rows left out and the item table, "Reversed" beside a reversed item only', () => {
    render(<AlphaResultCard result={alphaResultView} names={names} />);
    const summary = screen.getByTestId('al-summary');
    expect(within(summary).getByTestId('al-n').textContent).toBe('9');
    expect(within(summary).getByTestId('al-k').textContent).toBe('3');
    expect(within(summary).getByTestId('al-alpha').textContent).toBe('0.8125');
    expect(within(summary).getByTestId('al-interval').textContent).toBe('0.5 to 0.9375');
    expect(within(summary).getByTestId('al-standardized').textContent).toBe('0.8');
    expect(within(summary).getByTestId('al-inter-item').textContent).toBe('0.5 (smallest 0.25, largest 0.75)');
    expect(screen.queryByTestId('al-note')).toBeNull();
    expect(cells('C1')).toEqual(['C1', '', '3.5', '1.25', '7.5', '4.75', '0.625', '0.75']);
    expect(cells('C2')).toEqual(['C2', 'Reversed', '2.25', '1.25', '7.5', '4.75', '0.625', '0.75']);
    expect(Array.from(screen.getByTestId('al-items').querySelectorAll('tbody tr')).map((r) => r.getAttribute('data-testid'))).toEqual([
      'al-item-row-C1',
      'al-item-row-C2',
      'al-item-row-C4',
    ]);
    expect(screen.getByTestId('al-exclusions').textContent).toContain(
      alphaSentence({ code: 'stats.alpha.missing_value', count: 2, params: { column: 1 } }, names)
    );
    expect(screen.getByTestId('al-exclusions').textContent).toContain(
      alphaSentence({ code: 'stats.alpha.missing_code', count: 1, params: { column: 4 } }, names)
    );
    expect(document.body.textContent ?? '').not.toMatch(VERDICT);
  });

  it('shows each not-computable statistic as its sentence where its number would be, and the negative-alpha note', () => {
    render(<AlphaResultCard result={alphaResultViewOther} names={names} />);
    const summary = screen.getByTestId('al-summary');
    expect(within(summary).getByTestId('al-alpha').textContent).toBe('-0.5');
    expect(within(summary).getByTestId('al-interval').textContent).toBe(alphaSentence({ code: 'stats.alpha.distribution_out_of_range' }));
    const constant = alphaSentence({ code: 'stats.alpha.item_constant' });
    expect(within(summary).getByTestId('al-standardized').textContent).toBe(constant);
    expect(within(summary).getByTestId('al-inter-item').textContent).toBe(constant);
    expect(screen.getByTestId('al-note').textContent).toBe(
      "Alpha is negative: the items' average covariance is negative. An item may need to be reverse-scored."
    );
    const two = alphaSentence({ code: 'stats.alpha.two_items' });
    expect(cells('C1')).toEqual(['C1', '', '3', '0', '7.5', '4.75', constant, two]);
    expect(cells('C2')[7]).toBe(two);
    expect(screen.getByTestId('al-exclusions').textContent).toBe('Rows left outNo row was left out.');
    expect(document.body.textContent ?? '').not.toMatch(VERDICT);
  });

  it('names the limitations, and says no verdict word, threshold or advice anywhere', () => {
    render(<AlphaResultCard result={alphaResultView} names={names} />);
    const limits = Array.from(screen.getByTestId('al-limitations').querySelectorAll('li')).map((li) => li.textContent);
    expect(limits).toEqual([
      'Alpha assumes the items measure one thing; Gaply does not check that.',
      "McDonald's omega and squared multiple correlations are not shown.",
    ]);
    const text = document.body.textContent ?? '';
    expect(text).not.toMatch(VERDICT);
    // The pattern catches a verdict on the rendered card.
    expect(`${text} acceptable`).toMatch(VERDICT);
  });
});

/** A result with `k` items, every statistic computed. */
function big(k: number): AlphaResultView {
  const alias = (i: number) => `C${i + 1}`;
  const stat = (v: number) => ({ value: v });
  const items: ItemStats[] = Array.from({ length: k }, (_, i) => ({
    item: alias(i),
    reversed: i % 5 === 0,
    mean: stat(3 + (i % 7) / 10),
    sd: stat(1.1),
    scaleMeanIfDeleted: stat(400 - i),
    scaleVarianceIfDeleted: stat(1200 + i),
    correctedItemTotal: stat(0.3 + (i % 9) / 100),
    alphaIfDeleted: stat(0.9),
  }));
  return {
    ...alphaResultView,
    result: { ...alphaResultView.result, k, items },
  };
}

describe('AlphaResultCard with many items', () => {
  it('measures the render at 128 items, one row each', () => {
    const t0 = performance.now();
    render(<AlphaResultCard result={big(128)} names={names} />);
    const ms = performance.now() - t0;
    expect(screen.getByTestId('al-items').querySelectorAll('tbody tr')).toHaveLength(128);
    expect(screen.getAllByText('Reversed')).toHaveLength(26);
    // eslint-disable-next-line no-console
    console.log(`ALPHA_RENDER_MS 128 items: ${ms.toFixed(0)}`);
    expect(ms).toBeLessThan(30000);
  });
});
