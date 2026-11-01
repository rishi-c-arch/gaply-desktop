// The correlations' words (Research OS, Phase 8H-2, decisions 355, 356 and
// 359). Every code the correlations' screen can show (corrCodes.json, pinned
// to Rust's closed sets by src-tauri/tests/stats_corr_wire.rs) has a
// sentence. The wording is neutral (decision 66): no sentence says
// "significant", carries a star, or tells the user which coefficient to use;
// the progress counts steps, never columns; the budget sentence names the
// pairs and the rows.
import { describe, expect, it } from 'vitest';

import corrCodes from './corrCodes.json';
import * as words from './corrWording';

const NUMBERS = { column: 1, count: 3, index: 1, value: 99, limit: 2, expected: 1, found: 2, lifecycle: 'planned' };
const WORDS = { workspace: 'statistics', expected: 'statistics', found: 'statistics', from: 'active', event: 'activate', run: 4 };
const paramsFor = (code: string) => (code.startsWith('workspace.') ? WORDS : NUMBERS);
const NAMES = { column: (a: number) => `"col${a}"` };

/** Every string the module exports, sentences built from every code included. */
function everySentence(): string[] {
  const out: string[] = [];
  const collect = (v: unknown) => {
    if (typeof v === 'string') out.push(v);
    else if (Array.isArray(v)) v.forEach(collect);
    else if (v && typeof v === 'object') Object.values(v).forEach(collect);
  };
  for (const [name, v] of Object.entries(words)) {
    if (name === 'CORR_WORDS') continue;
    if (typeof v !== 'function') collect(v);
  }
  for (const code of corrCodes) out.push(words.corrSentence({ code, params: paramsFor(code), count: 2 }, NAMES));
  for (const state of ['cancelled', 'timed_out', 'resource_limit', 'refused', 'needs_user_input', 'failed', 'succeeded']) {
    out.push(words.corrRunEndedSentence(state));
  }
  out.push(
    words.corrMissingCodeQuestion(99, [
      { header: 'Loyalty', cells: 3 },
      { header: 'Price', cells: 1 },
    ]),
    words.coefficientHeading('pearson', 10),
    words.pairsShownSentence(1226, 1770, 1770),
    words.stepsSentence(400, 1306),
    words.settingsMissingSentence(0, 0, false) ?? ''
  );
  return out;
}

describe('the correlations screen words every code it can show', () => {
  it('has a sentence for each code in corrCodes.json, and none for any other', () => {
    expect(corrCodes).toHaveLength(35 + 11 + 5 + 6);
    expect(Object.keys(words.CORR_WORDS).sort()).toEqual([...corrCodes].sort());
    for (const code of corrCodes) {
      const s = words.corrSentence({ code, params: paramsFor(code), count: 2 }, NAMES);
      expect(s, code).not.toMatch(/no wording/);
      expect(s, code).toMatch(/[.:]$/);
      expect(s, code).not.toContain('undefined');
      expect(s, code).not.toContain('?');
    }
  });

  it('no sentence says "significant", carries a star, or advises a coefficient', () => {
    const all = everySentence();
    expect(all.length).toBeGreaterThan(70);
    const bad = all.filter((s) => /significan/i.test(s) || /[*★☆]/.test(s) || /\b(should|recommend)/i.test(s));
    expect(bad).toEqual([]);
  });

  it('says the progress in steps, a coefficient by its pairs, and which pairs are shown', () => {
    expect(words.stepsSentence(400, 1306)).toBe('400 of 1306 steps');
    expect(words.coefficientHeading('pearson', 10)).toBe("Pearson's r: 10 pairs");
    expect(words.coefficientHeading('kendall', 1)).toBe("Kendall's tau-b: 1 pair");
    expect(words.coefficientHeading('spearman', 8128)).toBe("Spearman's rho: 8,128 pairs");
    expect(words.pairsShownSentence(1, 1225, 1770)).toBe('Pairs 1 to 1,225 of 1,770 are shown.');
    expect(words.PAIRS_PAGE).toBe(1225);
    expect(words.MATRIX_MAX_VARIABLES).toBe(20);
  });

  it('says what is missing before the settings can be checked, and nothing once all is set', () => {
    expect(words.settingsMissingSentence(0, 0, false)).toBe(
      'To check the settings, choose two variables, a coefficient and how missing values are handled.'
    );
    expect(words.settingsMissingSentence(1, 1, true)).toBe('To check the settings, choose one more variable.');
    expect(words.settingsMissingSentence(2, 0, true)).toBe('To check the settings, choose a coefficient.');
    expect(words.settingsMissingSentence(2, 1, true)).toBeNull();
  });

  it('says the budget with its pairs and rows, and an infinite t as its sentence', () => {
    expect(words.corrSentence({ code: 'stats.corr.over_budget', params: { count: 2340, found: 100000 } })).toBe(
      'These settings are refused before they run: 2,340 pairs over 100,000 rows are estimated to take longer than 15 seconds on this computer. Choose fewer variables or fewer coefficients.'
    );
    expect(words.BUDGET_SECONDS).toBe(15);
    expect(words.boundedText({ infinite: 'stats.corr.perfect_coefficient' })).toMatch(/^Infinite: /);
    expect(words.boundedText({ value: 8.123456789 })).toBe('8.12346');
    expect(words.corrStatText({ not_computable: 'stats.corr.no_values' })).toBe(words.corrSentence({ code: 'stats.corr.no_values' }));
  });

  it('words a refusal from the correlations, the run manager and the registry', () => {
    const refusal = (code: string, params: object = {}) => new Error(JSON.stringify({ code, params }));
    expect(words.corrRefusalSentence(refusal('stats.corr.no_coefficient'))).toBe('No coefficient is chosen. Tick at least one.');
    expect(words.corrRefusalSentence(refusal('run.wrong_kind'))).toMatch(/not a correlations run/);
    expect(words.corrRefusalSentence(refusal('capability.version_mismatch', { expected: 1, found: 2 }))).toMatch(/version 2/);
  });
});
