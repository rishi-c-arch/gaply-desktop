// Crosstabs' words (Research OS, Phase 8X-2, decisions 588 and 589). Every code
// the crosstabs screen can show (xtabCodes.json, pinned to Rust's closed sets by
// src-tauri/tests/stats_xtab_wire.rs) has a sentence: its own 26, and the run's,
// the registry's and the workspace's as the ANOVA's words give them, the two that
// name a method renamed. The wording is neutral (decision 66): no sentence says
// "significant", carries a star, advises a test, or uses a word a verdict on a
// table would use (the ANOVA's list, :56-61, with alphaWording.vitest.ts:20's).
// A crosstab has one check point, so no steps sentence is shown.
import { describe, expect, it } from 'vitest';

import xtabCodes from './xtabCodes.json';
import LIMITS from './describeLimits.json';
import { ANOVA_WORDS } from './anovaWording';
import * as words from './xtabWording';

/** alphaWording.vitest.ts:20's words, copied: importing a test file would run its tests here. */
const ALPHA_VERDICT = /\b(acceptable|unacceptable|good|excellent|questionable|poor|reliable|unreliable|adequate|sufficient)\b|0\.7\b|[*★☆]|\b(should|recommend|drop|remove)\b/i;

const NUMBERS = { column: 1, count: 3, index: 1, value: 99, limit: 50, expected: 1, found: 2, lifecycle: 'planned' };
const WORDS = { workspace: 'statistics', expected: 'statistics', found: 'statistics', from: 'active', event: 'activate', run: 4 };
const paramsFor = (code: string) => (code.startsWith('workspace.') ? WORDS : NUMBERS);
const NAMES = { column: (a: number) => `"col${a}"` };

/** The ANOVA's verdict test (anovaWording.vitest.ts:59), alpha's list, and decision
 *  575's whole words "valid" and "accurate". */
const VERDICT = (s: string) =>
  /significan/i.test(s) || /[*★☆]/.test(s) || /\b(should|recommend)/i.test(s) || ALPHA_VERDICT.test(s) || /\b(valid|accurate)\b/i.test(s);

/** Every string the module exports, sentences built from every code included. */
function everySentence(): string[] {
  const out: string[] = [];
  const collect = (v: unknown) => {
    if (typeof v === 'string') out.push(v);
    else if (Array.isArray(v)) v.forEach(collect);
    else if (v && typeof v === 'object') Object.values(v).forEach(collect);
  };
  for (const [name, v] of Object.entries(words)) {
    if (name === 'XTAB_WORDS') continue;
    if (typeof v !== 'function') collect(v);
  }
  for (const code of xtabCodes) out.push(words.xtabSentence({ code, params: paramsFor(code), count: 2 }, NAMES));
  for (const code of xtabCodes) out.push(words.xtabSentence({ code, params: paramsFor(code), count: 1 }, NAMES));
  for (const state of ['cancelled', 'timed_out', 'resource_limit', 'refused', 'needs_user_input', 'failed', 'succeeded']) {
    out.push(words.xtabRunEndedSentence(state));
  }
  out.push(
    words.xtabMissingCodeQuestion(-99, [
      { header: 'arm', cells: 3 },
      { header: 'site', cells: 1 },
    ]),
    words.xtabCellsBelowFive(0, 4, 5.25),
    words.xtabCellsBelowFive(3, 2500, 0.012345678)
  );
  return out;
}

describe('the crosstabs screen words every code it can show', () => {
  it('has a sentence for each code in xtabCodes.json, and none for any other', () => {
    expect(xtabCodes).toHaveLength(26 + 11 + 5 + 6);
    expect(Object.keys(words.XTAB_WORDS).sort()).toEqual([...xtabCodes].sort());
    for (const code of xtabCodes) {
      for (const count of [1, 2]) {
        const s = words.xtabSentence({ code, params: paramsFor(code), count }, NAMES);
        expect(s, code).not.toMatch(/no wording/);
        expect(s, code).toMatch(/[.:]$/);
        expect(s, code).not.toContain('undefined');
        expect(s, code).not.toContain('?');
      }
    }
  });

  it('no sentence passes a verdict, carries a star, or advises a test', () => {
    const all = everySentence();
    expect(all.length).toBeGreaterThan(110);
    expect(all.filter(VERDICT)).toEqual([]);
    // The pattern does catch a verdict.
    for (const bad of ['The association is significant.', 'You should use Fisher.', 'An adequate table.', 'This is valid.', 'p < 0.05 *']) {
      expect(VERDICT(bad), bad).toBe(true);
    }
  });

  it("takes the run, registry and workspace sentences from the ANOVA's words where they name no method", () => {
    const renamed = ['run.unknown_run', 'run.wrong_kind'];
    const shared = xtabCodes.filter((c) => !c.startsWith('stats.xtab.') && !renamed.includes(c));
    expect(shared).toHaveLength(11 + 5 + 6 - 2);
    for (const code of shared) expect(words.XTAB_WORDS[code], code).toBe(ANOVA_WORDS[code]);
    expect(words.xtabSentence({ code: 'run.unknown_run' })).toBe(
      `This run is no longer held. Gaply keeps the last ${LIMITS.maxRuns} runs in memory only; run crosstabs again.`
    );
    expect(words.xtabSentence({ code: 'run.wrong_kind' })).toBe('This run is not a crosstabs run, so this screen cannot read it.');
    for (const code of renamed) expect(words.xtabSentence({ code })).not.toMatch(/ANOVA/);
  });

  it("says the expected counts' line and note in decision 589's words, 0 included", () => {
    expect(words.xtabCellsBelowFive(0, 4, 5.25)).toBe('0 of 4 cells have an expected count below 5; the smallest is 5.25.');
    expect(words.xtabCellsBelowFive(4, 4, 3.5)).toBe('4 of 4 cells have an expected count below 5; the smallest is 3.5.');
    expect(words.xtabCellsBelowFive(3, 2500, 0.012345678)).toBe('3 of 2500 cells have an expected count below 5; the smallest is 0.0123457.');
    expect(words.xtabSentence({ code: 'stats.xtab.expected_below_five' })).toBe(
      "Some expected counts are below 5, where the chi-square p values rest on a rougher approximation; for a table of two rows and two columns, Fisher's exact test does not rely on it."
    );
  });

  it("names the card's switch, tests and measures in decision 589's words", () => {
    expect(words.CELL_STATISTICS.map((c) => c.label)).toEqual([
      'Count',
      'Expected count',
      '% of row',
      '% of column',
      '% of total',
      'Residual',
      'Pearson residual',
      'Adjusted residual',
    ]);
    expect(words.CELL_STATISTICS[0].stat).toBe('count');
    expect(Object.values(words.XTAB_TEST_LABELS)).toEqual([
      'Pearson chi-square',
      'Likelihood ratio',
      'Continuity correction',
      "Fisher's exact test (two-sided)",
      "Fisher's exact test (less)",
      "Fisher's exact test (greater)",
    ]);
    expect(Object.values(words.XTAB_MEASURE_LABELS)).toEqual(['Phi', "Cramer's V", 'Contingency coefficient']);
  });

  it('says an exclusion with its column and count, singular at 1, each of the two columns its own', () => {
    const say = (code: string, column: number, count: number) => words.xtabSentence({ code, params: { column }, count }, NAMES);
    expect(say('stats.xtab.missing_row', 1, 2)).toBe(`2 rows had no value (an empty cell or a missing-value token) in "col1", the rows' column, and were left out.`);
    expect(say('stats.xtab.missing_column', 2, 1)).toBe(`1 row had no value (an empty cell or a missing-value token) in "col2", the columns' column, and was left out.`);
    expect(say('stats.xtab.missing_code', 1, 1)).toBe('1 row held a missing code you ticked in "col1", and was left out.');
    expect(say('stats.xtab.missing_code', 2, 3)).toBe('3 rows held a missing code you ticked in "col2", and were left out.');
  });

  it('says a level count with its column, never a label', () => {
    expect(words.xtabSentence({ code: 'stats.xtab.too_many_levels', params: { column: 3, count: 51, limit: 50 } }, NAMES)).toBe(
      'Column "col3" has 51 levels among the rows included, more than the 50 a table takes in each column.'
    );
    expect(words.xtabSentence({ code: 'stats.xtab.too_few_levels', params: { column: 3, count: 1 } }, NAMES)).toBe(
      'Column "col3" has 1 level among the rows included, and a table needs at least two in each column.'
    );
  });

  it('shows no steps sentence: a crosstab has one check point (anovaWording.ts:97)', () => {
    expect('stepsSentence' in words).toBe(false);
  });

  it('says each statistic as a number, or its sentence where it would be', () => {
    expect(words.xtabStatText({ value: 0.126123456 })).toBe('0.126123');
    expect(words.xtabStatText({ not_computable: 'stats.xtab.arithmetic_refused' })).toBe(words.xtabSentence({ code: 'stats.xtab.arithmetic_refused' }));
  });

  it('words a refusal from crosstabs, the run manager and the registry', () => {
    const refusal = (code: string, params: object = {}) => new Error(JSON.stringify({ code, params }));
    expect(words.xtabRefusalSentence(refusal('stats.xtab.same_column'))).toBe('The rows and the columns are the same column. Choose two different columns.');
    expect(words.xtabRefusalSentence(refusal('run.wrong_kind'))).toMatch(/not a crosstabs run/);
    expect(words.xtabRefusalSentence(refusal('capability.version_mismatch', { expected: 1, found: 2 }))).toMatch(/version 2/);
  });

  it('names its export files and choices on the ANOVA\'s pattern (anovaWording.ts:294-297)', () => {
    expect(words.XTAB_EXPORT_CHOICES.map((c) => [c.format, c.file, c.mime])).toEqual([
      ['csv', 'crosstabs.csv', 'text/csv'],
      ['manifestJson', 'crosstabs_manifest.json', 'application/json'],
    ]);
  });
});
