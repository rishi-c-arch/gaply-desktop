// Cronbach's alpha's words (Research OS, Phase 8I-2, decisions 380, 381 and
// 384). Every code alpha's screen can show (alphaCodes.json, pinned to Rust's
// closed sets by src-tauri/tests/stats_alpha_wire.rs) has a sentence. The
// wording is neutral (decision 66): no sentence calls a value acceptable,
// good, excellent, questionable, poor or unacceptable, names a threshold,
// carries a star, or tells the user which item to drop or reverse; the
// progress counts steps, never columns; a reversed item's values outside the
// scale are said with the column and the count, singular at 1.
import { describe, expect, it } from 'vitest';

import alphaCodes from './alphaCodes.json';
import * as words from './alphaWording';

const NUMBERS = { column: 1, count: 3, index: 1, value: 99, expected: 1, found: 2, lifecycle: 'planned' };
const WORDS = { workspace: 'statistics', expected: 'statistics', found: 'statistics', from: 'active', event: 'activate', run: 4 };
const paramsFor = (code: string) => (code.startsWith('workspace.') ? WORDS : NUMBERS);
const NAMES = { column: (a: number) => `"col${a}"` };

/** The words a verdict on alpha would use (decision 384). */
export const VERDICT = /\b(acceptable|unacceptable|good|excellent|questionable|poor|reliable|unreliable|adequate|sufficient)\b|0\.7\b|[*★☆]|\b(should|recommend|drop|remove)\b/i;

/** Every string the module exports, sentences built from every code included. */
function everySentence(): string[] {
  const out: string[] = [];
  const collect = (v: unknown) => {
    if (typeof v === 'string') out.push(v);
    else if (Array.isArray(v)) v.forEach(collect);
    else if (v && typeof v === 'object') Object.values(v).forEach(collect);
  };
  for (const [name, v] of Object.entries(words)) {
    if (name === 'ALPHA_WORDS') continue;
    if (typeof v !== 'function') collect(v);
  }
  for (const code of alphaCodes) out.push(words.alphaSentence({ code, params: paramsFor(code), count: 2 }, NAMES));
  for (const state of ['cancelled', 'timed_out', 'resource_limit', 'refused', 'needs_user_input', 'failed', 'succeeded']) {
    out.push(words.alphaRunEndedSentence(state));
  }
  out.push(
    words.alphaMissingCodeQuestion(99, [
      { header: 'Q5', cells: 3 },
      { header: 'Q6', cells: 1 },
    ]),
    words.stepsSentence(400, 1276),
    words.settingsMissingSentence(0, 1, '', '') ?? ''
  );
  return out;
}

describe("Cronbach's alpha's screen words every code it can show", () => {
  it('has a sentence for each code in alphaCodes.json, and none for any other', () => {
    expect(alphaCodes).toHaveLength(37 + 11 + 5 + 6);
    expect(Object.keys(words.ALPHA_WORDS).sort()).toEqual([...alphaCodes].sort());
    for (const code of alphaCodes) {
      const s = words.alphaSentence({ code, params: paramsFor(code), count: 2 }, NAMES);
      expect(s, code).not.toMatch(/no wording/);
      expect(s, code).toMatch(/[.:]$/);
      expect(s, code).not.toContain('undefined');
      expect(s, code).not.toContain('?');
    }
  });

  it('no sentence passes a verdict, names a threshold, or advises an item', () => {
    const all = everySentence();
    expect(all.length).toBeGreaterThan(70);
    expect(all.filter((s) => VERDICT.test(s))).toEqual([]);
    // The pattern does catch a verdict.
    expect(VERDICT.test('Alpha is acceptable.')).toBe(true);
    expect(VERDICT.test('above 0.7')).toBe(true);
  });

  it('says a value outside the scale with its column and count, singular at 1', () => {
    expect(words.alphaSentence({ code: 'stats.alpha.reversed_outside_scale', params: { column: 3, count: 12 } }, { column: () => '"Q3"' })).toBe(
      "\"Q3\" has 12 values outside the scale's ends you typed, once the ticked missing codes are left out. Check the ends, or tick a missing code."
    );
    expect(words.alphaSentence({ code: 'stats.alpha.reversed_outside_scale', params: { column: 7, count: 1 } }, { column: () => '"Q7"' })).toBe(
      "\"Q7\" has 1 value outside the scale's ends you typed, once the ticked missing codes are left out. Check the ends, or tick a missing code."
    );
  });

  it("says the negative-alpha note in decision 367's words", () => {
    expect(words.alphaSentence({ code: 'stats.alpha.negative_alpha' })).toBe(
      "Alpha is negative: the items' average covariance is negative. An item may need to be reverse-scored."
    );
  });

  it('says the progress in steps and the reversal on the ends the user typed', () => {
    expect(words.stepsSentence(400, 1276)).toBe('400 of 1276 steps');
    expect(words.REVERSE_SENTENCE).toBe(
      'Reverse-scoring turns each value x into lowest + highest − x. Type the ends your questionnaire allows, for example 1 and 5, not the lowest and highest answers in your data. Every reverse-scored item uses these ends.'
    );
  });

  it('says what is missing before the settings can be checked, and nothing once all is set', () => {
    expect(words.settingsMissingSentence(0, 0, '', '')).toBe('To check the settings, choose two items.');
    expect(words.settingsMissingSentence(1, 0, '', '')).toBe('To check the settings, choose one more item.');
    expect(words.settingsMissingSentence(2, 0, '', '')).toBeNull();
    expect(words.settingsMissingSentence(2, 1, '', '')).toBe(
      'To check the settings, choose a number for the lowest value on the scale and a number for the highest value on the scale.'
    );
    expect(words.settingsMissingSentence(2, 1, '1', '')).toBe('To check the settings, choose a number for the highest value on the scale.');
    expect(words.settingsMissingSentence(2, 1, 'one', '5')).toBe('To check the settings, choose a number for the lowest value on the scale.');
    expect(words.settingsMissingSentence(2, 1, '5', '1')).toBe(
      'To check the settings, type a lowest value on the scale that is below the highest.'
    );
    expect(words.settingsMissingSentence(2, 1, '5', '5')).not.toBeNull();
    expect(words.settingsMissingSentence(2, 1, '1', '5')).toBeNull();
    expect(words.settingsMissingSentence(2, 1, ' 0.5 ', '7')).toBeNull();
  });

  it('reads a typed scale end as a finite number, or nothing', () => {
    expect(words.scaleEnd('1')).toBe(1);
    expect(words.scaleEnd(' -2.5 ')).toBe(-2.5);
    for (const t of ['', '  ', 'one', 'Infinity', 'NaN', '1e999']) expect(words.scaleEnd(t), t).toBeNull();
  });

  it('says each statistic as a number, or its sentence where it would be', () => {
    expect(words.alphaStatText({ value: 0.812345678 })).toBe('0.812346');
    expect(words.alphaStatText({ not_computable: 'stats.alpha.two_items' })).toBe(words.alphaSentence({ code: 'stats.alpha.two_items' }));
    expect(words.alphaIntervalText({ computed: { lower: 0.5, upper: 0.9375 } })).toBe('0.5 to 0.9375');
    expect(words.alphaIntervalText({ not_computable: 'stats.alpha.total_constant' })).toBe(
      words.alphaSentence({ code: 'stats.alpha.total_constant' })
    );
  });

  it("words a refusal from alpha, the run manager and the registry", () => {
    const refusal = (code: string, params: object = {}) => new Error(JSON.stringify({ code, params }));
    expect(words.alphaRefusalSentence(refusal('stats.alpha.scale_missing'))).toBe(
      "An item is reverse-scored, but the scale's lowest and highest values are not given."
    );
    expect(words.alphaRefusalSentence(refusal('run.wrong_kind'))).toMatch(/not a Cronbach's alpha run/);
    expect(words.alphaRefusalSentence(refusal('capability.version_mismatch', { expected: 1, found: 2 }))).toMatch(/version 2/);
  });
});
