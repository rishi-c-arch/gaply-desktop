// The scale score's dialog (Research OS, Phase 8T-2a, decisions 504 to 508;
// ADR-012 Part 8, ADR-011 Parts 10 and 14), over a mock prep bridge and the
// window's own session. Nothing is filled in; Make waits for every piece and
// says what is missing; the recipe sent is the held steps, then this one; an
// answer is taken only for the session's slot of the same generation; a
// deletion asks first and renumbers; and no path here closes the slot.
import fs from 'node:fs';
import path from 'node:path';
import React from 'react';
import { act, cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import type { ColumnProfile, DatasetView, ScoreCounts } from '../describeTypes';
import { makeMockPrepBridge } from '../prepBridge';
import type { PrepMadeView, PrepRecipeView, Recipe, ScaleScoreStep } from '../prepTypes';
import { prepRefusalSentence, prepSentence } from '../prepWording';
import { ScoreDialog } from './ScoreDialog';
import { SLOT_GONE, statisticsSession } from './statisticsSession';
import { ASK_DELETE_ALL, ASK_DELETE_SCORE, scoreDeletedSentence, scoreMadeSentence, SCORES_DELETED } from './windowWording';

afterEach(() => {
  cleanup();
  statisticsSession.clear();
});

const col = (alias: string, header: string, codes: Array<[number, number]> = [], kind: ColumnProfile['kind'] = 'numeric', extra: Partial<ColumnProfile> = {}): ColumnProfile => ({
  alias,
  header,
  kind,
  n: 250,
  missing: 0,
  distinct: kind === 'numeric' ? 5 : 3,
  levels: kind === 'numeric' ? null : [{ label: 'x', count: 250 }],
  suspectMissingCodes: codes.map(([code, cells]) => ({ code, cells })),
  nonFinite: 0,
  ...extra,
});

const FILE_COLUMNS = [
  col('C1', 'Q1', [[-99, 2]]),
  col('C2', 'Group', [], 'categorical'),
  col('C3', 'Q3', [[99, 4]]),
  col('C4', 'Q4', [
    [99, 1],
    [-99, 3],
  ]),
  col('C5', 'Q5'),
];

const fileView: DatasetView = {
  slot: 3,
  displayName: 'scale.csv',
  profile: { rows: 250, columns: FILE_COLUMNS },
  inputRef: { contentHash: 'ab'.repeat(32), rows: 250, columns: 5, semanticRole: 'table' },
};

const counts = (column: string, every = 240): ScoreCounts => ({ column, scoredFromEveryItem: every, scoredFromFewer: 6, notScoredTooFew: 4, notScoredOverflow: 0 });
const PREPARATION = { sourceHash: 'ab'.repeat(32), recipeHash: 'cd'.repeat(32), sourceColumns: 5, capabilityVersion: 1, engineVersion: 1 };

/** A view holding `names` as scores C6, C7, … */
function preparedView(names: string[], hash = 'ef'): DatasetView {
  const scores = names.map((n, i) => col(`C${6 + i}`, `C${6 + i}`, [], 'numeric', { prepared: true, scoreName: n }));
  return {
    slot: 3,
    displayName: 'scale.csv',
    profile: { rows: 250, columns: [...FILE_COLUMNS, ...scores] },
    inputRef: { contentHash: hash.repeat(32), rows: 250, columns: 5 + names.length, semanticRole: 'table' },
    preparation: PREPARATION,
    scoreCounts: names.map((_, i) => counts(`C${6 + i}`)),
  };
}

const heldStep = (alias: string, name: string, items: string[]): ScaleScoreStep => ({
  kind: 'scale_score',
  alias,
  name,
  items,
  reversed: [],
  scale: null,
  statistic: 'sum',
  rule: 'every_item',
  missingCodes: [],
});

const recipeOf = (steps: ScaleScoreStep[]): Recipe => ({ schemaVersion: 1, capabilityId: 'stats.prepare.scale_score', capabilityVersion: 1, engineVersion: 1, sourceColumns: 5, steps });
const held = (steps: ScaleScoreStep[]): PrepRecipeView => ({ recipe: recipeOf(steps), counts: steps.map((s) => counts(s.alias)) });

const made = (dataset: DatasetView | null, over: Partial<PrepMadeView> = {}): PrepMadeView => ({
  status: dataset ? 'valid' : 'invalid',
  errors: [],
  warnings: [],
  missingInformation: [],
  estimatedResources: { rows: 250, columns: dataset ? dataset.profile.columns.length : 6, cells: 1500 },
  counts: dataset ? (dataset.scoreCounts ?? []) : [],
  dataset,
  ...over,
});

const refusal = (code: string) => new Error(JSON.stringify({ code, params: {}, severity: 'error' }));

function setup(responses: Record<string, unknown> = {}, view: DatasetView = fileView) {
  act(() => statisticsSession.setFile(view));
  const prep = makeMockPrepBridge({ stats_prep_make: made(preparedView(['Wellbeing'])), ...responses });
  const onDone = vi.fn();
  const onCancel = vi.fn();
  const utils = render(<ScoreDialog view={view} slot={3} prep={prep} onDone={onDone} onCancel={onCancel} />);
  return { prep, onDone, onCancel, ...utils };
}

const box = (id: string) => screen.getByTestId(id) as HTMLInputElement;
const make = () => screen.getByTestId('sc-make') as HTMLButtonElement;
const type = (id: string, text: string) => fireEvent.change(screen.getByTestId(id), { target: { value: text } });
const click = (id: string) => fireEvent.click(screen.getByTestId(id));
const missing = () => screen.queryByTestId('sc-settings-missing')?.textContent ?? null;
const sent = (prep: ReturnType<typeof makeMockPrepBridge>) => prep.calls.filter(([c]) => c === 'stats_prep_make').map(([, a]) => a.recipe as Recipe);
const commands = (prep: ReturnType<typeof makeMockPrepBridge>) => prep.calls.map(([c]) => c);

/** Every piece ready: a name, two items with one reversed and its ends, the
 *  mean, and "at least 1". */
function fill() {
  type('sc-name', 'Wellbeing');
  click('sc-item-C1');
  click('sc-item-C3');
  click('sc-reverse-C3');
  type('sc-scale-min', '1');
  type('sc-scale-max', '5');
  click('sc-statistic-mean');
  click('sc-rule-at-least');
  type('sc-at-least', '1');
}

describe('nothing is filled in (decisions 504 and 508)', () => {
  it('on opening: the name is empty', () => {
    setup();
    expect(box('sc-name').value).toBe('');
  });

  it('on opening: the name takes the focus', () => {
    setup();
    expect(document.activeElement).toBe(box('sc-name'));
  });

  it('on opening: no item is chosen', () => {
    setup();
    for (const a of ['C1', 'C3', 'C4', 'C5']) expect(box(`sc-item-${a}`).checked, a).toBe(false);
    expect(screen.queryAllByTestId(/^sc-chosen-/)).toHaveLength(0);
    expect(screen.getByTestId('sc-items-none')).toBeTruthy();
  });

  it('on opening: neither the sum nor the mean is chosen', () => {
    setup();
    expect(box('sc-statistic-sum').checked).toBe(false);
    expect(box('sc-statistic-mean').checked).toBe(false);
  });

  it('on opening: neither rule is chosen, and n is not shown', () => {
    setup();
    expect(box('sc-rule-every').checked).toBe(false);
    expect(box('sc-rule-at-least').checked).toBe(false);
    expect(screen.queryByTestId('sc-at-least')).toBeNull();
  });

  it('after one item holding a suspect code is added: its reverse box is unticked', () => {
    setup();
    click('sc-item-C1');
    expect(box('sc-reverse-C1').checked).toBe(false);
  });

  it('after one item holding a suspect code is added: no ends are shown', () => {
    setup();
    click('sc-item-C1');
    expect(screen.queryByTestId('sc-scale')).toBeNull();
  });

  it('after one item holding a suspect code is added: its code is offered, unticked', () => {
    setup();
    click('sc-item-C1');
    expect(box('sc-code--99').checked).toBe(false);
    expect(screen.getByTestId('sc-missing-codes').textContent).toContain('Is -99 a code for a missing value? It appears in "Q1" (2 cells).');
  });

  it('after its reversal is ticked: both ends are empty, beside the reverse-scoring sentence', () => {
    setup();
    click('sc-item-C1');
    click('sc-reverse-C1');
    expect(box('sc-scale-min').value).toBe('');
    expect(box('sc-scale-max').value).toBe('');
    expect(screen.getByTestId('sc-reverse-sentence').textContent).toContain('Reverse-scoring turns each value x into lowest + highest − x.');
  });

  it('after "at least" is chosen: n is empty', () => {
    setup();
    click('sc-rule-at-least');
    expect(box('sc-at-least').value).toBe('');
  });

  it('Make is disabled on opening, with one sentence listing every piece in the form\'s order', () => {
    setup();
    expect(make().disabled).toBe(true);
    expect(missing()).toBe('To make the column, choose a name for the score, two items, the sum or the mean and when a person gets a score.');
  });

  it('taking an item out unticks its reversal, and the ends go with the last one', () => {
    setup();
    click('sc-item-C1');
    click('sc-reverse-C1');
    type('sc-scale-min', '1');
    click('sc-remove-C1');
    click('sc-item-C1');
    expect(box('sc-reverse-C1').checked).toBe(false);
    click('sc-reverse-C1');
    expect(box('sc-scale-min').value).toBe('');
  });

  it('a ticked code stays only while an item still offers it', () => {
    setup();
    click('sc-item-C1');
    click('sc-item-C3');
    click('sc-code--99');
    click('sc-code-99');
    click('sc-remove-C1');
    expect(screen.queryByTestId('sc-code--99')).toBeNull();
    click('sc-item-C1');
    expect(box('sc-code--99').checked).toBe(false);
    expect(box('sc-code-99').checked).toBe(true);
  });
});

describe('Make waits for every piece (decision 504)', () => {
  it('is enabled exactly when every piece is ready, with no sentence', () => {
    setup();
    fill();
    expect(missing()).toBeNull();
    expect(make().disabled).toBe(false);
  });

  const UNDO: Array<[string, () => void, string]> = [
    ['the name empty', () => type('sc-name', ''), 'a name for the score'],
    ['the name only spaces', () => type('sc-name', '   '), 'a name for the score'],
    ['the name one character too long', () => type('sc-name', 'x'.repeat(129)), 'a name of at most 128 characters'],
    ['one item', () => click('sc-remove-C1'), 'one more item'],
    ['the lowest end empty', () => type('sc-scale-min', ''), 'a number for the lowest value on the scale'],
    ['the highest end not a number', () => type('sc-scale-max', 'five'), 'a number for the highest value on the scale'],
    ['n empty', () => type('sc-at-least', ''), 'a whole number of items from 1 to 1'],
    ['n the number of items', () => type('sc-at-least', '2'), 'a whole number of items from 1 to 1'],
    ['n not whole', () => type('sc-at-least', '0.5'), 'a whole number of items from 1 to 1'],
  ];
  it.each(UNDO)('is disabled with %s, and its sentence says so', (_what, undo, words) => {
    setup();
    fill();
    act(undo);
    expect(make().disabled).toBe(true);
    expect(missing()).toBe(`To make the column, choose ${words}.`);
  });

  it('is disabled with no statistic, and its sentence says so', () => {
    setup();
    type('sc-name', 'Wellbeing');
    click('sc-item-C1');
    click('sc-item-C3');
    click('sc-rule-every');
    expect(make().disabled).toBe(true);
    expect(missing()).toBe('To make the column, choose the sum or the mean.');
  });

  it('is disabled with no rule, and its sentence says so', () => {
    setup();
    type('sc-name', 'Wellbeing');
    click('sc-item-C1');
    click('sc-item-C3');
    click('sc-statistic-sum');
    expect(make().disabled).toBe(true);
    expect(missing()).toBe('To make the column, choose when a person gets a score.');
  });

  it('counts the name by code point, as Rust does: 128 emoji are ready, 129 are not', () => {
    setup();
    fill();
    type('sc-name', '😀'.repeat(128));
    expect(make().disabled).toBe(false);
    type('sc-name', '😀'.repeat(129));
    expect(make().disabled).toBe(true);
  });

  it('leaves the ends\' order to Rust: 5 and 1 are ready', () => {
    setup();
    fill();
    type('sc-scale-min', '5');
    type('sc-scale-max', '1');
    expect(make().disabled).toBe(false);
  });
});

describe('the recipe sent (decision 505)', () => {
  it('one score on a file\'s own table: no recipe read, the name trimmed, items, reversals and codes in order, -0 as 0', async () => {
    const { prep } = setup();
    expect(commands(prep)).toEqual([]);
    type('sc-name', '  Wellbeing  ');
    click('sc-item-C4');
    click('sc-item-C1');
    click('sc-reverse-C4');
    click('sc-reverse-C1');
    type('sc-scale-min', '-0');
    type('sc-scale-max', '5');
    click('sc-code-99');
    click('sc-code--99');
    click('sc-statistic-mean');
    click('sc-rule-at-least');
    type('sc-at-least', '1');
    click('sc-make');
    await waitFor(() => expect(sent(prep)).toHaveLength(1));
    const r = sent(prep)[0];
    expect(r).toEqual({
      schemaVersion: 1,
      capabilityId: 'stats.prepare.scale_score',
      capabilityVersion: 1,
      engineVersion: 1,
      sourceColumns: 5,
      steps: [
        {
          kind: 'scale_score',
          alias: 'C6',
          name: 'Wellbeing',
          items: ['C1', 'C4'],
          reversed: ['C1', 'C4'],
          scale: { min: 0, max: 5 },
          statistic: 'mean',
          rule: { at_least: 1 },
          missingCodes: [-99, 99],
        },
      ],
    });
    expect(Object.is(r.steps[0].scale?.min, 0)).toBe(true);
    expect(prep.calls[0][1].slot).toBe(3);
  });

  it('with no item reversed, the scale is null, and "every item" is its word', async () => {
    const { prep } = setup();
    type('sc-name', 'Total');
    click('sc-item-C1');
    click('sc-item-C5');
    click('sc-statistic-sum');
    click('sc-rule-every');
    click('sc-make');
    await waitFor(() => expect(sent(prep)).toHaveLength(1));
    expect(sent(prep)[0].steps[0]).toMatchObject({ scale: null, reversed: [], statistic: 'sum', rule: 'every_item', missingCodes: [] });
  });

  it('a second score: the held recipe read once, its steps first, then this one as the next column', async () => {
    const first = heldStep('C6', 'Total', ['C1', 'C3']);
    const view = preparedView(['Total']);
    const { prep } = setup({ stats_prep_recipe: held([first]), stats_prep_make: made(preparedView(['Total', 'Average'], 'aa')) }, view);
    await waitFor(() => expect(commands(prep)).toEqual(['stats_prep_recipe']));
    expect(prep.calls[0][1]).toEqual({ slot: 3 });
    type('sc-name', 'Average');
    click('sc-item-C1');
    click('sc-item-C5');
    click('sc-statistic-mean');
    click('sc-rule-every');
    click('sc-make');
    await waitFor(() => expect(sent(prep)).toHaveLength(1));
    const r = sent(prep)[0];
    expect(r.sourceColumns).toBe(5);
    expect(r.steps[0]).toEqual(first);
    expect(r.steps[1]).toMatchObject({ alias: 'C7', name: 'Average', items: ['C1', 'C5'] });
    expect(commands(prep)).toEqual(['stats_prep_recipe', 'stats_prep_make']);
  });

  it('a prepared column is never offered as an item, nor added by Add every numeric column', async () => {
    const { prep } = setup({ stats_prep_recipe: held([heldStep('C6', 'Total', ['C1', 'C3'])]) }, preparedView(['Total']));
    await waitFor(() => expect(commands(prep)).toEqual(['stats_prep_recipe']));
    expect(screen.queryByTestId('sc-item-C6')).toBeNull();
    expect(screen.queryByTestId('sc-item-C2')).toBeNull();
    click('sc-tick-all');
    expect(screen.queryAllByTestId(/^sc-chosen-/).map((e) => e.getAttribute('data-testid'))).toEqual(['sc-chosen-C1', 'sc-chosen-C3', 'sc-chosen-C4', 'sc-chosen-C5']);
  });
});

describe("the sum's sentence (decision 504, item 7)", () => {
  const COMBOS: Array<[string, string, boolean]> = [
    ['sc-statistic-sum', 'sc-rule-at-least', true],
    ['sc-statistic-sum', 'sc-rule-every', false],
    ['sc-statistic-mean', 'sc-rule-at-least', false],
    ['sc-statistic-mean', 'sc-rule-every', false],
  ];
  it.each(COMBOS)('%s with %s: shown %s', (statistic, rule, shown) => {
    setup();
    type('sc-name', 'Wellbeing');
    click('sc-item-C1');
    click('sc-item-C3');
    click(statistic);
    click(rule);
    const note = screen.queryByTestId('sc-sum-note');
    if (!shown) {
      expect(note).toBeNull();
      return;
    }
    const recipe = recipeOf([{ ...heldStep('C6', 'Wellbeing', ['C1', 'C3']) }]);
    expect(note?.textContent).toBe(prepSentence({ code: 'stats.prep.sum_over_fewer_items', params: { index: 0 } }, fileView, recipe));
    expect(note?.textContent).toContain('For "Wellbeing", a person scored from fewer items has a sum over fewer items');
  });

  it('on a second score, it names the new score, not the held one', async () => {
    const { prep } = setup({ stats_prep_recipe: held([heldStep('C6', 'Total', ['C1', 'C3'])]) }, preparedView(['Total']));
    await waitFor(() => expect(commands(prep)).toEqual(['stats_prep_recipe']));
    type('sc-name', 'Average');
    click('sc-statistic-sum');
    click('sc-rule-at-least');
    expect(screen.getByTestId('sc-sum-note').textContent).toContain('For "Average",');
  });
});

describe("the make's answer (decision 505)", () => {
  it('a made score: the session takes the new view, and onDone has its counts', async () => {
    const after = preparedView(['Wellbeing']);
    const { onDone } = setup({ stats_prep_make: made(after) });
    fill();
    click('sc-make');
    await waitFor(() => expect(onDone).toHaveBeenCalledTimes(1));
    expect(statisticsSession.get().view).toBe(after);
    expect(statisticsSession.get().slot).toBe(3);
    expect(onDone).toHaveBeenCalledWith(scoreMadeSentence('Wellbeing', counts('C6')));
    expect(onDone.mock.calls[0][0]).toBe('The score was made. "Wellbeing": 240 rows scored from every item, 6 from fewer; 4 not scored, too few items; 0 not scored, too large to hold.');
  });

  it('a made score with warnings: taken the same, and no warning repeated', async () => {
    const after = preparedView(['Wellbeing']);
    const { onDone } = setup({ stats_prep_make: made(after, { status: 'valid_with_warnings', warnings: [{ code: 'stats.prep.sum_over_fewer_items', params: { index: 0 }, severity: 'warning' }] }) });
    fill();
    click('sc-make');
    await waitFor(() => expect(onDone).toHaveBeenCalledTimes(1));
    expect(statisticsSession.get().view).toBe(after);
    expect(onDone.mock.calls[0][0]).not.toContain('fewer items has a sum');
  });

  it('a refused make keeps the dialog and the form, each error and question in its sentence', async () => {
    const answer = made(null, {
      status: 'invalid',
      errors: [{ code: 'stats.prep.name_equals_file_header', params: { index: 0, column: 1 }, severity: 'error' }],
      missingInformation: [{ code: 'stats.prep.no_row_scored', params: { column: 6 }, severity: 'error' }],
      counts: [{ column: 'C6', scoredFromEveryItem: 0, scoredFromFewer: 0, notScoredTooFew: 250, notScoredOverflow: 0 }],
    });
    const { onDone, onCancel } = setup({ stats_prep_make: answer });
    fill();
    type('sc-name', 'Q1');
    click('sc-make');
    await screen.findByTestId('sc-errors');
    expect(screen.getByTestId('sc-errors').textContent).toBe('The name is already a column of the file. Give the score another name.');
    expect(screen.getByTestId('sc-questions').textContent).toContain('No row would get a score for "Q1", so it was not made. "Q1": 0 rows scored from every item');
    expect(box('sc-name').value).toBe('Q1');
    expect(box('sc-item-C1').checked).toBe(true);
    expect(onDone).not.toHaveBeenCalled();
    expect(onCancel).not.toHaveBeenCalled();
    expect(statisticsSession.get().view).toBe(fileView);
  });

  function deferred() {
    let resolve: (v: unknown) => void = () => undefined;
    let reject: (e: unknown) => void = () => undefined;
    const promise = new Promise((res, rej) => {
      resolve = res;
      reject = rej;
    });
    return { promise, resolve, reject };
  }

  it.each([
    ['a clear', () => statisticsSession.clear()],
    ['slotGone', () => statisticsSession.slotGone()],
    ['another file', () => statisticsSession.setFile({ ...fileView, slot: 9 })],
  ])('an answer after %s is dropped', async (_what, change) => {
    const d = deferred();
    const { onDone } = setup({ stats_prep_make: () => d.promise });
    fill();
    click('sc-make');
    // The file's part of the session; busy changes as the make ends, so it is left out.
    const file = () => {
      const s = statisticsSession.get();
      return { slot: s.slot, view: s.view, notice: s.notice };
    };
    const before = file();
    act(change);
    const changed = file();
    expect(changed).not.toEqual(before);
    await act(async () => {
      d.resolve(made(preparedView(['Wellbeing'])));
      await d.promise;
    });
    expect(file().view).toBe(changed.view);
    expect(file()).toEqual(changed);
    expect(onDone).not.toHaveBeenCalled();
  });

  it('run.unknown_input from the make: slotGone, and the dialog closes', async () => {
    const { onDone } = setup({
      stats_prep_make: () => {
        throw refusal('run.unknown_input');
      },
    });
    fill();
    click('sc-make');
    await waitFor(() => expect(onDone).toHaveBeenCalledWith(null));
    expect(statisticsSession.get().notice).toBe(SLOT_GONE);
    expect(statisticsSession.get().slot).toBeNull();
  });

  it('run.unknown_input after a clear is dropped: no slotGone', async () => {
    const d = deferred();
    setup({ stats_prep_make: () => d.promise });
    fill();
    click('sc-make');
    act(() => statisticsSession.clear());
    await act(async () => {
      d.reject(refusal('run.unknown_input'));
      await d.promise.catch(() => undefined);
    });
    expect(statisticsSession.get().notice).toBeNull();
  });

  it('run.unknown_input from the opening recipe read: slotGone, and the dialog closes', async () => {
    const { onDone } = setup(
      {
        stats_prep_recipe: () => {
          throw refusal('run.unknown_input');
        },
      },
      preparedView(['Total'])
    );
    await waitFor(() => expect(onDone).toHaveBeenCalledWith(null));
    expect(statisticsSession.get().notice).toBe(SLOT_GONE);
  });

  it("run.run_busy: its sentence in the dialog, which stays", async () => {
    const { onDone } = setup({
      stats_prep_make: () => {
        throw refusal('run.run_busy');
      },
    });
    fill();
    click('sc-make');
    const said = await screen.findByTestId('sc-dialog-refusal');
    expect(said.textContent).toBe(prepRefusalSentence(refusal('run.run_busy')));
    expect(said.textContent).toBe('The scores cannot change while a run is going, or while another change to them is being made. Wait for it to end, then try again.');
    expect(onDone).not.toHaveBeenCalled();
    expect(statisticsSession.get().view).toBe(fileView);
  });

  it('while a make goes: the session is busy, and Cancel and Escape do nothing', async () => {
    const d = deferred();
    const { onCancel } = setup({ stats_prep_make: () => d.promise });
    fill();
    click('sc-make');
    expect(statisticsSession.get().busy).toBe(true);
    expect((screen.getByTestId('sc-dialog-cancel') as HTMLButtonElement).disabled).toBe(true);
    fireEvent.keyDown(screen.getByTestId('sw-score-dialog'), { key: 'Escape' });
    expect(onCancel).not.toHaveBeenCalled();
    await act(async () => {
      d.resolve(made(preparedView(['Wellbeing'])));
      await d.promise;
    });
    expect(statisticsSession.get().busy).toBe(false);
  });

  it('Cancel and Escape each ask the parent to drop it', () => {
    const { onCancel } = setup();
    click('sc-dialog-cancel');
    expect(onCancel).toHaveBeenCalledTimes(1);
    fireEvent.keyDown(screen.getByTestId('sw-score-dialog'), { key: 'Escape' });
    expect(onCancel).toHaveBeenCalledTimes(2);
  });
});

describe('deleting a score (decision 507)', () => {
  const total = heldStep('C6', 'Total', ['C1', 'C3']);
  const average = { ...heldStep('C7', 'Average', ['C1', 'C4', 'C5']), statistic: 'mean' as const, rule: { at_least: 2 }, missingCodes: [-99] };
  const third = { ...heldStep('C8', 'Third', ['C3', 'C5']), reversed: ['C5'], scale: { min: 1, max: 5 } };

  it("lists nothing on a file's own table", () => {
    setup();
    expect(screen.queryByTestId('sc-scores')).toBeNull();
  });

  it('lists the held scores by name, each Delete naming its score, and Delete every score', async () => {
    setup({ stats_prep_recipe: held([total, average]) }, preparedView(['Total', 'Average']));
    await screen.findByTestId('sc-scores');
    expect(screen.getByRole('button', { name: 'Delete the score "Total"' })).toBeTruthy();
    expect(screen.getByRole('button', { name: 'Delete the score "Average"' })).toBeTruthy();
    expect(screen.getByTestId('sc-delete-all').textContent).toBe('Delete every score');
  });

  it('Delete asks first, beside the dialog, not inside it, and Cancel changes nothing', async () => {
    const { prep, onDone } = setup({ stats_prep_recipe: held([total, average]) }, preparedView(['Total', 'Average']));
    await screen.findByTestId('sc-scores');
    fireEvent.click(screen.getByRole('button', { name: 'Delete the score "Total"' }));
    expect(screen.getByTestId('sc-ask-sentence').textContent).toBe(ASK_DELETE_SCORE.sentence('Total'));
    expect(screen.getByTestId('sw-score-dialog').contains(screen.getByTestId('sc-ask'))).toBe(false);
    click('sc-ask-cancel');
    expect(screen.queryByTestId('sc-ask')).toBeNull();
    expect(commands(prep)).toEqual(['stats_prep_recipe']);
    expect(onDone).not.toHaveBeenCalled();
    expect(screen.getByTestId('sw-score-dialog')).toBeTruthy();
  });

  it('deleting the first of three sends the others unchanged but their aliases, renumbered', async () => {
    const after = preparedView(['Average', 'Third'], '12');
    const { prep, onDone } = setup({ stats_prep_recipe: held([total, average, third]), stats_prep_make: made(after) }, preparedView(['Total', 'Average', 'Third']));
    await screen.findByTestId('sc-scores');
    fireEvent.click(screen.getByRole('button', { name: 'Delete the score "Total"' }));
    click('sc-ask-continue');
    await waitFor(() => expect(onDone).toHaveBeenCalledWith(scoreDeletedSentence('Total')));
    expect(sent(prep)).toEqual([{ ...recipeOf([]), steps: [{ ...average, alias: 'C6' }, { ...third, alias: 'C7' }] }]);
    expect(commands(prep)).not.toContain('stats_prep_clear');
    expect(statisticsSession.get().view).toBe(after);
  });

  it('deleting the last of two keeps the first as it is', async () => {
    const { prep, onDone } = setup({ stats_prep_recipe: held([total, average]), stats_prep_make: made(preparedView(['Total'], '34')) }, preparedView(['Total', 'Average']));
    await screen.findByTestId('sc-scores');
    fireEvent.click(screen.getByRole('button', { name: 'Delete the score "Average"' }));
    click('sc-ask-continue');
    await waitFor(() => expect(onDone).toHaveBeenCalled());
    expect(sent(prep)[0].steps).toEqual([total]);
  });

  it('deleting the only one clears', async () => {
    const { prep, onDone } = setup({ stats_prep_recipe: held([total]), stats_prep_clear: fileView }, preparedView(['Total']));
    await screen.findByTestId('sc-scores');
    fireEvent.click(screen.getByRole('button', { name: 'Delete the score "Total"' }));
    click('sc-ask-continue');
    await waitFor(() => expect(onDone).toHaveBeenCalledWith(scoreDeletedSentence('Total')));
    expect(prep.calls.filter(([c]) => c === 'stats_prep_clear')).toEqual([['stats_prep_clear', { slot: 3 }]]);
    expect(sent(prep)).toEqual([]);
    expect(statisticsSession.get().view).toBe(fileView);
  });

  it('Delete every score asks first, then clears', async () => {
    const { prep, onDone } = setup({ stats_prep_recipe: held([total, average]), stats_prep_clear: fileView }, preparedView(['Total', 'Average']));
    await screen.findByTestId('sc-scores');
    click('sc-delete-all');
    expect(screen.getByTestId('sc-ask-sentence').textContent).toBe(ASK_DELETE_ALL.sentence);
    click('sc-ask-continue');
    await waitFor(() => expect(onDone).toHaveBeenCalledWith(SCORES_DELETED));
    expect(commands(prep)).toEqual(['stats_prep_recipe', 'stats_prep_clear']);
  });
});

describe('the dialog never closes the slot (decision 508)', () => {
  it('the slot is unchanged after Make, Cancel and Delete, and no close is sent', async () => {
    const view = preparedView(['Total', 'Average']);
    const { prep, onDone, onCancel } = setup({ stats_prep_recipe: held([heldStep('C6', 'Total', ['C1', 'C3']), heldStep('C7', 'Average', ['C1', 'C5'])]), stats_prep_make: made(preparedView(['Total', 'Average', 'Wellbeing'], '56')) }, view);
    await screen.findByTestId('sc-scores');
    fill();
    click('sc-make');
    await waitFor(() => expect(onDone).toHaveBeenCalledTimes(1));
    expect(statisticsSession.get().slot).toBe(3);
    click('sc-dialog-cancel');
    expect(onCancel).toHaveBeenCalled();
    fireEvent.click(screen.getByRole('button', { name: 'Delete the score "Total"' }));
    click('sc-ask-continue');
    await waitFor(() => expect(onDone).toHaveBeenCalledTimes(2));
    expect(statisticsSession.get().slot).toBe(3);
    expect(commands(prep).filter((c) => /close/.test(c))).toEqual([]);
  });

  it("a cancelled dialog calls no make and no clear, and on a file's own table no bridge at all", async () => {
    const own = setup();
    click('sc-dialog-cancel');
    expect(own.prep.calls).toEqual([]);
    cleanup();
    const prepared = setup({ stats_prep_recipe: held([heldStep('C6', 'Total', ['C1', 'C3'])]) }, preparedView(['Total']));
    await screen.findByTestId('sc-scores');
    click('sc-dialog-cancel');
    expect(commands(prepared.prep)).toEqual(['stats_prep_recipe']);
  });

  it('its source names no close and no clearFile', () => {
    const src = (f: string) => fs.readFileSync(path.join(__dirname, f), 'utf8');
    for (const f of ['ScoreDialog.tsx', 'ScoreSettings.tsx']) {
      expect(src(f), f).not.toMatch(/\.close\(|stats_desc_close|clearFile\(/);
      expect(src(f).length, f).toBeGreaterThan(500);
    }
  });

  it('its frame is sized by the method dialog\'s two rules, and Make and Cancel sit outside the part that scrolls', () => {
    setup();
    const scroller = screen.getByTestId('sc-dialog-scroll');
    expect(scroller.contains(screen.getByTestId('sc-name'))).toBe(true);
    expect(scroller.contains(make())).toBe(false);
    const css = fs.readFileSync(path.join(__dirname, 'MethodDialog.css'), 'utf8');
    expect(css).toMatch(/\[data-testid='sw-score-dialog'\][^{]*\{[^}]*max-width:\s*min\(800px, 90vw\)/);
    expect(css).toMatch(/\[data-testid='sw-score-dialog'\][^{]*\{[^}]*max-height:\s*min\(640px, 90vh\)/);
    expect(css).toMatch(/\[data-testid='sw-score-dialog'\] \.gds-modal__body/);
    expect(screen.getByRole('dialog', { name: 'Scale score' })).toBeTruthy();
  });
});

describe('127 items (decision 508)', () => {
  it('measures the render of 127 numeric columns, Add every numeric column pressed', () => {
    const many: DatasetView = {
      ...fileView,
      profile: { rows: 250, columns: Array.from({ length: 127 }, (_, i) => col(`C${i + 1}`, `item${i + 1}`, i % 10 === 0 ? [[-99, 2]] : [])) },
      inputRef: { ...fileView.inputRef, columns: 127 },
    };
    act(() => statisticsSession.setFile(many));
    const prep = makeMockPrepBridge({});
    const t0 = performance.now();
    render(<ScoreDialog view={many} slot={3} prep={prep} onDone={vi.fn()} onCancel={vi.fn()} />);
    fireEvent.click(screen.getByTestId('sc-tick-all'));
    const ms = performance.now() - t0;
    expect(screen.queryAllByTestId(/^sc-chosen-/)).toHaveLength(127);
    expect(screen.getByTestId('sc-code--99')).toBeTruthy();
    // eslint-disable-next-line no-console
    console.log(`SCORE_RENDER_MS 127 items: ${ms.toFixed(0)}`);
    expect(ms).toBeLessThan(CEILING_MS);
  });
});

/** Above the worst of five measured runs. Measured 7 Nov 2026 on this Mac, the
 *  test alone (-t), load average 3.3: 85, 116, 89, 86 and 88 ms; 27 ms with the
 *  file's other tests before it. The ceiling is 86 times the worst, since
 *  decision 480 measured a render 40 times slower in the whole suite at a load
 *  near 20; no per-test timeout (decision 488). */
const CEILING_MS = 10000;
