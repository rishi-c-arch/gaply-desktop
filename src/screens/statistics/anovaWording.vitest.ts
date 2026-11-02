// The one-way ANOVA's words (Research OS, Phase 8G-2, decisions 318, 328 and
// 331). Every code the ANOVA screen can show (anovaCodes.json, pinned to
// Rust's closed sets by src-tauri/tests/stats_anova_wire.rs) has a sentence.
// The wording is neutral (decision 66): no sentence says "significant",
// carries a star, or tells the user which test to use; the progress counts
// steps, never columns; a p below the floor is said as below the floor's
// value, from the result.
import { describe, expect, it } from 'vitest';

import anovaCodes from './anovaCodes.json';
import * as words from './anovaWording';

const NUMBERS = { column: 1, count: 3, index: 1, value: 99, limit: 2, expected: 1, found: 2, lifecycle: 'planned' };
const WORDS = { workspace: 'statistics', expected: 'statistics', found: 'statistics', from: 'active', event: 'activate', run: 4 };
const paramsFor = (code: string) => (code.startsWith('workspace.') ? WORDS : NUMBERS);
const NAMES = { column: (a: number) => `"col${a}"`, level: (i: number) => `lvl${i}` };

/** Every string the module exports, sentences built from every code included. */
function everySentence(): string[] {
  const out: string[] = [];
  const collect = (v: unknown) => {
    if (typeof v === 'string') out.push(v);
    else if (Array.isArray(v)) v.forEach(collect);
    else if (v && typeof v === 'object') Object.values(v).forEach(collect);
  };
  for (const [name, v] of Object.entries(words)) {
    if (name === 'ANOVA_WORDS') continue;
    if (typeof v !== 'function') collect(v);
  }
  for (const code of anovaCodes) out.push(words.anovaSentence({ code, params: paramsFor(code), count: 2 }, NAMES));
  for (const state of ['cancelled', 'timed_out', 'resource_limit', 'refused', 'needs_user_input', 'failed', 'succeeded']) {
    out.push(words.anovaRunEndedSentence(state));
  }
  out.push(
    words.anovaMissingCodeQuestion(-99, 'score', 2),
    words.levelsTooFewSentence(1),
    words.postHocHeading('tukey', 1225),
    words.stepsSentence(20, 46)
  );
  return out;
}

describe('the ANOVA screen words every code it can show', () => {
  it('has a sentence for each code in anovaCodes.json, and none for any other', () => {
    expect(anovaCodes).toHaveLength(38 + 11 + 5 + 6);
    expect(Object.keys(words.ANOVA_WORDS).sort()).toEqual([...anovaCodes].sort());
    for (const code of anovaCodes) {
      const s = words.anovaSentence({ code, params: paramsFor(code), count: 2 }, NAMES);
      expect(s, code).not.toMatch(/no wording/);
      expect(s, code).toMatch(/[.:]$/);
      expect(s, code).not.toContain('undefined');
      expect(s, code).not.toContain('?');
    }
  });

  it('no sentence says "significant", carries a star, or advises a test', () => {
    const all = everySentence();
    expect(all.length).toBeGreaterThan(70);
    const bad = all.filter((s) => /significan/i.test(s) || /[*★☆]/.test(s) || /\b(should|recommend)/i.test(s));
    expect(bad).toEqual([]);
  });

  it('says the progress in steps (decision 328) and a table by its row count (decision 331)', () => {
    expect(words.stepsSentence(20, 46)).toBe('20 of 46 steps');
    expect(words.stepsSentence(0, 1)).not.toMatch(/column/);
    expect(words.postHocHeading('games_howell', 1225)).toBe('Games–Howell: 1225 pairs');
    expect(words.postHocHeading('bonferroni', 1)).toBe('Bonferroni: 1 pair');
  });

  it('shows a p below the floor as below the floor from the result, and a not-computable statistic as its sentence', () => {
    expect(words.postHocPText({ below: 1e-10 })).toBe('below 1e-10');
    expect(words.postHocPText({ value: 0.0123456789 })).toBe('0.0123457');
    expect(words.anovaStatText({ not_computable: 'stats.anova.within_variance_zero' })).toBe(
      words.anovaSentence({ code: 'stats.anova.within_variance_zero' })
    );
    expect(words.anovaStatText({ value: -0.0512345678 })).toBe('-0.0512346');
  });

  it('words a refusal from the ANOVA, the run manager and the registry', () => {
    const refusal = (code: string, params: object = {}) => new Error(JSON.stringify({ code, params }));
    expect(words.anovaRefusalSentence(refusal('stats.anova.too_many_levels', { count: 51, limit: 50 }))).toBe(
      'This column has 51 distinct values, more than the 50 a group may have, so its values are not listed.'
    );
    expect(words.anovaRefusalSentence(refusal('run.wrong_kind'))).toMatch(/not a one-way ANOVA/);
    expect(words.anovaRefusalSentence(refusal('capability.version_mismatch', { expected: 1, found: 2 }))).toMatch(/version 2/);
  });
});

// Research OS, Phase 8M-2 (decisions 417 and 418): a score's refusals, shown by
// no screen yet, name the column and the count, singular at 1.
describe('the words for a prepared table', () => {
  it('says a ticked code equals a score, with the column and the count', () => {
    const say = (count: number) => words.anovaSentence({ code: 'stats.anova.code_equals_score', params: { column: 8, count }, count }, NAMES);
    expect(say(1)).toBe('Column "col8" is a score, and it holds a missing code you ticked in 1 cell. Untick the code, or leave this score out.');
    expect(say(3)).toBe('Column "col8" is a score, and it holds a missing code you ticked in 3 cells. Untick the code, or leave this score out.');
  });

  it("says a preparation does not match, in one sentence that names nothing of the user's", () => {
    expect(words.anovaSentence({ code: 'stats.anova.preparation_mismatch', params: {}, count: 0 }, NAMES)).toBe(
      'These settings do not match how the open data was prepared. Choose the settings again.'
    );
  });

  it('says a score cannot be the group', () => {
    expect(words.anovaSentence({ code: 'stats.anova.score_as_group', params: { column: 8 }, count: 0 }, NAMES)).toBe(
      'Column "col8" is a score, so it cannot be the group. Choose a column from your file.'
    );
  });
});
