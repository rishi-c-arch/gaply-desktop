// The rank tests' words (Research OS, Phase 8Y-2, decisions 619 and 620). Every code
// the screen can show (ranksCodes.json, pinned to Rust's closed sets by
// src-tauri/tests/stats_ranks_wire.rs) has a sentence: its own 37, and the run's, the
// registry's and the workspace's as the ANOVA's words give them, the two that name a
// method renamed. No sentence says "significant", carries a star, advises a test, or
// uses a word a verdict would use. The run has one check point, so no steps sentence.
import { describe, expect, it } from 'vitest';

import ranksCodes from './ranksCodes.json';
import LIMITS from './describeLimits.json';
import { ANOVA_WORDS } from './anovaWording';
import * as words from './ranksWording';

/** alphaWording.vitest.ts:20's words, copied: importing a test file would run its tests here. */
const ALPHA_VERDICT = /\b(acceptable|unacceptable|good|excellent|questionable|poor|reliable|unreliable|adequate|sufficient)\b|0\.7\b|[*★☆]|\b(should|recommend|drop|remove)\b/i;

const NUMBERS = { column: 1, count: 3, index: 1, value: 99, limit: 50, expected: 1, found: 2, lifecycle: 'planned' };
const WORDS = { workspace: 'statistics', expected: 'statistics', found: 'statistics', from: 'active', event: 'activate', run: 4 };
const paramsFor = (code: string) => (code.startsWith('workspace.') ? WORDS : NUMBERS);
const NAMES = { column: (a: number) => `"col${a}"`, level: (i: number) => `lev${i}` };

const VERDICT = (s: string) =>
  /significan/i.test(s) || /[*★☆]/.test(s) || /\b(should|recommend)/i.test(s) || ALPHA_VERDICT.test(s) || /\b(valid|accurate)\b/i.test(s);

function everySentence(): string[] {
  const out: string[] = [];
  const collect = (v: unknown) => {
    if (typeof v === 'string') out.push(v);
    else if (Array.isArray(v)) v.forEach(collect);
    else if (v && typeof v === 'object') Object.values(v).forEach(collect);
  };
  for (const [name, v] of Object.entries(words)) {
    if (name === 'RANKS_WORDS') continue;
    if (typeof v !== 'function') collect(v);
  }
  for (const count of [1, 2]) {
    for (const code of ranksCodes) out.push(words.ranksSentence({ code, params: paramsFor(code), count }, NAMES));
  }
  for (const state of ['cancelled', 'timed_out', 'resource_limit', 'refused', 'needs_user_input', 'failed', 'succeeded']) {
    out.push(words.ranksRunEndedSentence(state));
  }
  out.push(words.ranksFirstSecondSentence('first', 'second'));
  return out;
}

describe('the rank tests screen words every code it can show', () => {
  it('has a sentence for each code in ranksCodes.json, and none for any other', () => {
    expect(ranksCodes).toHaveLength(37 + 11 + 5 + 6);
    expect(Object.keys(words.RANKS_WORDS).sort()).toEqual([...ranksCodes].sort());
    for (const code of ranksCodes) {
      for (const count of [1, 2]) {
        const s = words.ranksSentence({ code, params: paramsFor(code), count }, NAMES);
        expect(s, code).not.toMatch(/no wording/);
        expect(s, code).toMatch(/[.:]$/);
        expect(s, code).not.toContain('undefined');
        expect(s, code).not.toContain('?');
      }
    }
  });

  it('no sentence passes a verdict, carries a star, or advises a test', () => {
    const all = everySentence();
    expect(all.length).toBeGreaterThan(120);
    expect(all.filter(VERDICT)).toEqual([]);
    for (const bad of ['The groups differ significantly.', 'You should use Dunn.', 'An adequate test.', 'This is valid.', 'p < 0.05 *']) {
      expect(VERDICT(bad), bad).toBe(true);
    }
  });

  it("takes the run, registry and workspace sentences from the ANOVA's words where they name no method", () => {
    const renamed = ['run.unknown_run', 'run.wrong_kind'];
    const shared = ranksCodes.filter((c) => !c.startsWith('stats.ranks.') && !renamed.includes(c));
    expect(shared).toHaveLength(11 + 5 + 6 - 2);
    for (const code of shared) expect(words.RANKS_WORDS[code], code).toBe(ANOVA_WORDS[code]);
    expect(words.ranksSentence({ code: 'run.unknown_run' })).toBe(
      `This run is no longer held. Gaply keeps the last ${LIMITS.maxRuns} runs in memory only; run the rank test again.`
    );
    expect(words.ranksSentence({ code: 'run.wrong_kind' })).toBe('This run is not a rank test, so this screen cannot read it.');
  });

  it("says the 25 codes whose suffix the ANOVA shares as the ANOVA says them, naming the method where the ANOVA's does", () => {
    const sameWords = [
      'unknown_capability',
      'input_mismatch',
      'preparation_mismatch',
      'code_equals_score',
      'score_as_group',
      'unknown_column',
      'same_column',
      'outcome_holds_text',
      'outcome_non_finite',
      'too_many_levels',
      'level_out_of_range',
      'level_repeated',
      'missing_code_not_finite',
      'suspect_missing_code',
      'overflow',
      'arithmetic_refused',
      'missing_group',
      'other_level',
      'missing_outcome',
      'missing_code',
    ];
    const renamed = ['capability_version_mismatch', 'input_count', 'group_one_level', 'too_few_levels'];
    const ownDistribution = ['distribution_out_of_range'];
    expect(sameWords.length + renamed.length + ownDistribution.length).toBe(25);
    const params = { column: 1, count: 3, index: 1, value: 99, limit: 50, expected: 1, found: 2 };
    for (const suffix of sameWords) {
      const ours = words.ranksSentence({ code: `stats.ranks.${suffix}`, params, count: 3 }, NAMES);
      const anova = (ANOVA_WORDS as Record<string, (d: unknown, n: unknown) => string>)[`stats.anova.${suffix}`]({ code: `stats.anova.${suffix}`, params, count: 3 }, NAMES);
      expect(ours, suffix).toBe(anova);
    }
    expect(words.ranksSentence({ code: 'stats.ranks.capability_version_mismatch', params })).toBe('These settings were made for version 2 of the rank tests, and this is version 1.');
    expect(words.ranksSentence({ code: 'stats.ranks.input_count', params })).toBe('These settings name 3 files, and a rank test reads exactly one.');
    expect(words.ranksSentence({ code: 'stats.ranks.group_one_level', params }, NAMES)).toBe('The group "col1" has 3 levels, and a rank test needs at least two.');
    expect(words.ranksSentence({ code: 'stats.ranks.too_few_levels', params })).toBe('3 levels included, and a rank test needs at least two.');
    expect(words.ranksSentence({ code: 'stats.ranks.distribution_out_of_range' })).toBe(
      'Not computable: this needs the chi-square distribution at more than 2,401 degrees of freedom, where Gaply has not measured its accuracy.'
    );
  });

  it("says decision 620's notes and the not-computable sentence word for word", () => {
    expect(words.ranksSentence({ code: 'stats.ranks.exact_not_given_ties' })).toBe(
      'The exact p value is not given, because two or more included values are tied.'
    );
    expect(words.ranksSentence({ code: 'stats.ranks.exact_not_given_size' })).toBe(
      'The exact p value is not given, because a group has 50 values or more.'
    );
    expect(words.ranksSentence({ code: 'stats.ranks.group_below_five' })).toBe('A group has fewer than 5 values, where the normal and chi-square approximations are rougher.');
    expect(words.ranksSentence({ code: 'stats.ranks.all_values_tied' })).toBe('Not computable: every included value is equal, so the ranks do not differ between groups.');
  });

  it("says decision 630's words in the absent exact p's cell", () => {
    expect(words.RANKS_EXACT_NOT_GIVEN_CELL).toBe('Not given (see the note below).');
  });

  it("says decision 620's sentence under the Mann-Whitney table, the groups as text", () => {
    expect(words.ranksFirstSecondSentence('a', 'b')).toBe(
      "The first group is a, the second b. U for the first group counts the pairs in which its value is the larger, a tie counting one half; z and the rank-biserial correlation are positive when the first group's values tend to be the larger."
    );
  });

  it("names the card's headings, columns and rows in decision 620's words", () => {
    expect(words.RANKS_HEADINGS).toEqual({ ranks: 'Ranks', mannWhitney: 'Mann-Whitney U test', kruskalWallis: 'Kruskal-Wallis H test' });
    expect(words.RANKS_GROUP_COLUMNS).toEqual(['Group', 'N', 'Median', 'Mean rank', 'Sum of ranks']);
    expect(Object.values(words.MW_ROW_LABELS)).toEqual(['U, first group', 'U, second group', 'z', 'p (normal approximation)', 'Exact p', 'Rank-biserial correlation']);
    expect(Object.values(words.KW_ROW_LABELS)).toEqual(['H', 'df', 'p', 'Epsilon-squared']);
  });

  it('says an exclusion with its count, singular at 1', () => {
    expect(words.ranksSentence({ code: 'stats.ranks.missing_code', count: 1 })).toBe('1 row held a missing code you ticked in the outcome, and was left out.');
    expect(words.ranksSentence({ code: 'stats.ranks.other_level', count: 3 })).toBe('3 rows were in levels left out, and were left out.');
  });

  it('names an empty group by its level, and only as the sentence on its own screen', () => {
    expect(words.ranksSentence({ code: 'stats.ranks.group_empty', params: { index: 1 } }, NAMES)).toBe('The group "lev1" has no value after the exclusions. Leave it out, or choose another group.');
  });

  it('shows no steps sentence: a run has one check point', () => {
    expect('stepsSentence' in words).toBe(false);
  });

  it('says each statistic as a number, or its sentence where it would be', () => {
    expect(words.ranksStatText({ value: 0.126123456 })).toBe('0.126123');
    expect(words.ranksStatText({ not_computable: 'stats.ranks.all_values_tied' })).toBe(words.ranksSentence({ code: 'stats.ranks.all_values_tied' }));
  });

  it('words a refusal from the rank tests, the run manager and the registry', () => {
    const refusal = (code: string, params: object = {}) => new Error(JSON.stringify({ code, params }));
    expect(words.ranksRefusalSentence(refusal('stats.ranks.same_column'))).toBe('The outcome and the group are the same column. Choose two different columns.');
    expect(words.ranksRefusalSentence(refusal('run.wrong_kind'))).toMatch(/not a rank test/);
    expect(words.ranksRefusalSentence(refusal('capability.version_mismatch', { expected: 1, found: 2 }))).toMatch(/version 2/);
  });

  it("names its export files and choices with the ANOVA's words (anovaWording.ts:294-297)", () => {
    expect(words.RANKS_EXPORT_CHOICES.map((c) => [c.format, c.label, c.file, c.mime])).toEqual([
      ['csv', 'The result tables (CSV)', 'independent_ranks.csv', 'text/csv'],
      ['manifestJson', 'The run manifest (JSON)', 'independent_ranks_manifest.json', 'application/json'],
    ]);
  });
});
