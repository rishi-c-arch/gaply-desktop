// Scale scores' wire contract, from the TypeScript side (Research OS, Phase
// 8T-1b, decision 499). Each JSON fixture is the exact view the webview reads
// through IPC (src-tauri/tests/stats_prep_screen_wire.rs checks the bytes); here
// each is deep-compared with a copy typed `satisfies` the view's type. Also
// pinned: PrepCode's union is exactly prepCodes.json's stats.prep words, the
// bridge's command names and argument keys, and the screen's number rule.
import { beforeEach, describe, expect, it, vi } from 'vitest';

import prepCodes from './prepCodes.json';
import { typedNumber, type PrepCode, type Recipe } from './prepTypes';
import madeJson from './fixtures/prepMadeView.json';
import refusedJson from './fixtures/prepMadeViewRefused.json';
import recipeJson from './fixtures/prepRecipeView.json';
import * as typed from './fixtures/prepFixtures';
import { makeMockPrepBridge, TauriPrepBridge } from './prepBridge';

const invoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...a: unknown[]) => invoke(...a) }));
vi.mock('../../utils/isTauri', () => ({ isTauri: true, default: true }));

beforeEach(() => {
  invoke.mockReset();
  invoke.mockResolvedValue({});
});

const PAIRS: Array<[string, unknown, unknown]> = [
  ['PrepMadeView (made)', typed.prepMadeView, madeJson],
  ['PrepMadeView (refused)', typed.prepMadeViewRefused, refusedJson],
  ['PrepRecipeView', typed.prepRecipeView, recipeJson],
];

describe("scale scores' view fixtures", () => {
  it.each(PAIRS)('%s: the typed copy equals the JSON the webview reads', (_name, t, json) => {
    expect(t).toEqual(json);
  });

  it('fill what a dialog reads: a made view with its table, a refused one with every list', () => {
    expect(madeJson.dataset?.profile.columns.at(-1)).toMatchObject({ alias: 'C8', header: 'C8', prepared: true, scoreName: 'wellbeing' });
    expect(refusedJson.dataset).toBeNull();
    for (const k of ['errors', 'warnings', 'missingInformation'] as const) expect(refusedJson[k].length, k).toBeGreaterThan(0);
  });
});

/** Every code of the union, once: a missing one or an extra one fails tsc
 *  (as alphaTypes.vitest.ts:136-178 pins alpha's). */
const EVERY_CODE: Record<PrepCode, true> = {
  'stats.prep.recipe_unreadable': true,
  'stats.prep.unknown_capability': true,
  'stats.prep.capability_version_mismatch': true,
  'stats.prep.engine_version_mismatch': true,
  'stats.prep.schema_version_mismatch': true,
  'stats.prep.source_already_prepared': true,
  'stats.prep.source_columns_mismatch': true,
  'stats.prep.no_steps': true,
  'stats.prep.alias_not_next': true,
  'stats.prep.too_wide': true,
  'stats.prep.cells_over_cap': true,
  'stats.prep.name_not_trimmed': true,
  'stats.prep.name_length': true,
  'stats.prep.name_has_invisible_character': true,
  'stats.prep.name_equals_file_header': true,
  'stats.prep.name_equals_other_score': true,
  'stats.prep.name_is_an_alias_word': true,
  'stats.prep.too_few_items': true,
  'stats.prep.item_repeated': true,
  'stats.prep.items_out_of_order': true,
  'stats.prep.item_not_a_file_column': true,
  'stats.prep.item_not_numeric': true,
  'stats.prep.item_holds_text': true,
  'stats.prep.item_non_finite': true,
  'stats.prep.reversed_repeated': true,
  'stats.prep.reversed_out_of_order': true,
  'stats.prep.reversed_not_an_item': true,
  'stats.prep.scale_missing': true,
  'stats.prep.scale_without_reversed': true,
  'stats.prep.scale_not_finite': true,
  'stats.prep.scale_ends_out_of_order': true,
  'stats.prep.reversed_outside_scale': true,
  'stats.prep.at_least_out_of_range': true,
  'stats.prep.missing_code_not_finite': true,
  'stats.prep.missing_code_repeated': true,
  'stats.prep.missing_codes_out_of_order': true,
  'stats.prep.statistic_not_chosen': true,
  'stats.prep.rule_not_chosen': true,
  'stats.prep.negative_zero': true,
  'stats.prep.no_row_scored': true,
  'stats.prep.table_refused_column': true,
  'stats.prep.sum_over_fewer_items': true,
  'stats.prep.scored_from_every_item': true,
  'stats.prep.scored_from_fewer': true,
  'stats.prep.not_scored_too_few': true,
  'stats.prep.not_scored_overflow': true,
};

describe('the PrepCode union', () => {
  it('is exactly the stats.prep words of prepCodes.json', () => {
    const listed = (prepCodes as string[]).filter((c) => c.startsWith('stats.prep.'));
    expect(listed).toHaveLength(46);
    expect(Object.keys(EVERY_CODE).sort()).toEqual([...listed].sort());
  });

  it('sits in a list of 59 codes, sorted, with only the two run codes the prep commands give', () => {
    expect(prepCodes).toHaveLength(59);
    expect([...prepCodes].sort()).toEqual(prepCodes);
    expect((prepCodes as string[]).filter((c) => c.startsWith('run.'))).toEqual(['run.run_busy', 'run.unknown_input']);
  });
});

describe('the bridge', () => {
  it("calls scale scores' commands with the webview argument names", async () => {
    const b = new TauriPrepBridge();
    await b.make(3, typed.prepRecipeView.recipe);
    await b.clear(3);
    await b.recipe(3);
    await b.csv(3);
    expect(invoke.mock.calls.map((c) => [c[0], Object.keys(c[1] as object).sort()])).toEqual([
      ['stats_prep_make', ['recipe', 'slot']],
      ['stats_prep_clear', ['slot']],
      ['stats_prep_recipe', ['slot']],
      ['stats_prep_csv', ['slot']],
    ]);
    expect(invoke.mock.calls[0][1]).toEqual({ slot: 3, recipe: typed.prepRecipeView.recipe });
  });

  it('has a mock that records each call and refuses a command it was not given', async () => {
    const m = makeMockPrepBridge({ stats_prep_recipe: null });
    await expect(m.recipe(2)).resolves.toBeNull();
    await expect(m.clear(2)).rejects.toThrow(/no mock response for stats_prep_clear/);
    expect(m.calls).toEqual([
      ['stats_prep_recipe', { slot: 2 }],
      ['stats_prep_clear', { slot: 2 }],
    ]);
  });
});

describe("the screen's number rule (decision 413's answer 4)", () => {
  // JSON.stringify writes -0 as "0" and NaN or an infinity as null, so what
  // reaches Rust cannot show the rule: only the built value can.
  it('sends a typed -0 as 0', () => {
    for (const t of ['-0', '-0.0', ' -0 ', '-0e5', '0', '+0']) {
      const v = typedNumber(t);
      expect(Object.is(v, 0), t).toBe(true);
      expect(1 / (v as number), t).toBe(Infinity);
    }
  });

  it('lets no non-finite value into a recipe', () => {
    for (const t of ['NaN', 'Infinity', '-Infinity', '1e999', '-1e999', 'abc', '', '   ', '1,5']) {
      expect(typedNumber(t), t).toBeNull();
    }
    expect(typedNumber('1.5')).toBe(1.5);
    expect(typedNumber(' -3 ')).toBe(-3);
  });

  it('gives a recipe built from typed fields no -0, NaN or infinity', () => {
    const lowest = typedNumber('-0');
    const highest = typedNumber('5');
    const codes = ['-0', '99'].map(typedNumber).filter((v): v is number => v !== null);
    const recipe: Recipe = {
      ...typed.prepRecipeView.recipe,
      steps: [{ ...typed.prepRecipeView.recipe.steps[0], scale: { min: lowest as number, max: highest as number }, missingCodes: codes }],
    };
    const step = recipe.steps[0];
    const numbers = [step.scale?.min, step.scale?.max, ...step.missingCodes];
    for (const v of numbers) {
      expect(Number.isFinite(v), String(v)).toBe(true);
      expect(Object.is(v, -0), String(v)).toBe(false);
    }
  });
});
