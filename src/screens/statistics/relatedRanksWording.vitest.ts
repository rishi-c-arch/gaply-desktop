// The related rank tests' words (Research OS, Phase 8Z-2, decisions 653 and 654). Every
// code the screen can show (relatedRanksCodes.json, pinned to Rust's closed sets by
// src-tauri/tests/stats_related_ranks_wire.rs) has a sentence: its own 34, and the run's,
// the registry's and the workspace's as the rank tests give them, the two that name the
// method renamed. No sentence says "significant", carries a star, advises a test, or uses
// a word a verdict would use. "Remove all" and "Remove" are labels, not verdicts, and fail
// the list: the settings import them from windowWording, and this module never holds them.
import fs from 'node:fs';
import path from 'node:path';
import { describe, expect, it } from 'vitest';

import relatedRanksCodes from './relatedRanksCodes.json';
import LIMITS from './describeLimits.json';
import RELATED_LIMITS from './fixtures/relatedRanksLimits.json';
import { CAPABILITY_WORDS } from './capabilityWording';
import { alphaMissingCodeQuestion } from './alphaWording';
import { RANKS_WORDS } from './ranksWording';
import * as words from './relatedRanksWording';

/** ranksWording.vitest.ts's list, copied: importing a test file would run its tests here. */
const ALPHA_VERDICT = /\b(acceptable|unacceptable|good|excellent|questionable|poor|reliable|unreliable|adequate|sufficient)\b|0\.7\b|[*★☆]|\b(should|recommend|drop|remove)\b/i;
const VERDICT = (s: string) =>
  /significan/i.test(s) || /[*★☆]/.test(s) || /\b(should|recommend)/i.test(s) || ALPHA_VERDICT.test(s) || /\b(valid|accurate)\b/i.test(s);

const NUMBERS = { column: 1, count: 3, index: 1, value: 99, limit: 50, expected: 1, found: 2, lifecycle: 'planned' };
const WORKSPACE = { workspace: 'statistics', expected: 'statistics', found: 'statistics', from: 'active', event: 'activate', run: 4 };
const paramsFor = (code: string) => (code.startsWith('workspace.') ? WORKSPACE : NUMBERS);
const NAMES = { column: (a: number) => `"col${a}"` };

function everySentence(): string[] {
  const out: string[] = [];
  const collect = (v: unknown) => {
    if (typeof v === 'string') out.push(v);
    else if (Array.isArray(v)) v.forEach(collect);
    else if (v && typeof v === 'object') Object.values(v).forEach(collect);
  };
  for (const [name, v] of Object.entries(words)) {
    if (name === 'RELATED_RANKS_WORDS') continue;
    if (typeof v !== 'function') collect(v);
  }
  for (const count of [1, 2]) {
    for (const code of relatedRanksCodes) out.push(words.relatedRanksSentence({ code, params: paramsFor(code), count }, NAMES));
  }
  for (const state of ['cancelled', 'timed_out', 'resource_limit', 'refused', 'needs_user_input', 'failed', 'succeeded']) {
    out.push(words.relatedRanksRunEndedSentence(state));
  }
  out.push(words.relatedRanksDifferenceSentence('first', 'second'));
  out.push(words.relatedRanksWilcoxonSentence('first', 'second'));
  out.push(words.relatedRanksFriedmanSentence(3));
  out.push(words.relatedRanksMissingCodeQuestion(-99, [{ header: 'a', cells: 2 }]));
  out.push(words.relatedRanksTooManyColumns(51));
  return out;
}

const limit = (code: string) => (CAPABILITY_WORDS as Record<string, string>)[`stats.capability.limit.analyze.${code}`];

describe('the related rank tests screen words every code it can show', () => {
  it('has a sentence for each code in relatedRanksCodes.json, and none for any other', () => {
    expect(relatedRanksCodes).toHaveLength(34 + 11 + 5 + 6);
    expect(Object.keys(words.RELATED_RANKS_WORDS).sort()).toEqual([...relatedRanksCodes].sort());
    for (const code of relatedRanksCodes) {
      for (const count of [1, 2]) {
        const s = words.relatedRanksSentence({ code, params: paramsFor(code), count }, NAMES);
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
    for (const bad of ['The columns differ significantly.', 'You should use Nemenyi.', 'An adequate test.', 'This is valid.', 'p < 0.05 *', 'Remove all']) {
      expect(VERDICT(bad), bad).toBe(true);
    }
  });

  it("does not hold the labels the verdict list would refuse, which the settings take from windowWording", () => {
    const text = JSON.stringify(Object.values(words).filter((v) => typeof v !== 'function'));
    expect(text).not.toContain('Remove');
  });

  it("takes the run, registry and workspace sentences from the rank tests' where they name no method", () => {
    const renamed = ['run.unknown_run', 'run.wrong_kind'];
    const shared = relatedRanksCodes.filter((c) => !c.startsWith('stats.related_ranks.') && !renamed.includes(c));
    expect(shared).toHaveLength(11 + 5 + 6 - 2);
    for (const code of shared) expect(words.RELATED_RANKS_WORDS[code], code).toBe(RANKS_WORDS[code]);
    expect(words.relatedRanksSentence({ code: 'run.unknown_run' })).toBe(
      `This run is no longer held. Gaply keeps the last ${LIMITS.maxRuns} runs in memory only; run the related-samples rank test again.`
    );
    expect(LIMITS.maxRuns).toBe(40);
    expect(words.relatedRanksSentence({ code: 'run.wrong_kind' })).toBe('This run is not a related-samples rank test, so this screen cannot read it.');
  });

  it('says the codes the rank tests share as the rank tests say them', () => {
    const same = ['unknown_capability', 'capability_version_mismatch', 'input_count', 'input_mismatch', 'preparation_mismatch', 'missing_code_not_finite', 'unknown_column', 'code_equals_score', 'distribution_out_of_range', 'overflow', 'no_weights'];
    const params = { column: 1, count: 3, index: 1, value: 99, limit: 50, expected: 1, found: 2 };
    for (const suffix of same) {
      const ours = words.relatedRanksSentence({ code: `stats.related_ranks.${suffix}`, params, count: 3 }, NAMES);
      const theirs = (RANKS_WORDS as Record<string, (d: unknown, n: unknown) => string>)[`stats.ranks.${suffix}`]({ code: `stats.ranks.${suffix}`, params, count: 3 }, { ...NAMES, level: () => '' });
      expect(ours, suffix).toBe(theirs);
    }
  });

  it("says decision 654's notes and not-computable sentences word for word", () => {
    const say = (c: string) => words.relatedRanksSentence({ code: `stats.related_ranks.${c}` });
    expect(say('exact_not_given_ties')).toBe('The exact p value is not given, because two or more differences are equal in size.');
    expect(say('exact_not_given_zeros')).toBe('The exact p value is not given, because one or more pairs have no difference.');
    expect(say('exact_not_given_size')).toBe('The exact p value is not given, because 50 or more pairs have a difference.');
    expect(say('zero_differences_dropped')).toBe('Pairs with no difference are left out of the ranks; their count is given above.');
    expect(say('rows_below_five')).toBe('Fewer than 5 pairs with a difference, or fewer than 5 complete rows, are used, where the normal and chi-square approximations are rougher.');
    expect(say('all_differences_zero')).toBe('Not computable: no pair has a difference, so there is nothing to rank.');
    expect(say('all_values_tied')).toBe('Not computable: in every row the chosen columns\' values are equal, so the ranks do not differ between columns.');
  });

  it("says each limitation as decision 640's capability limit with the same meaning says it", () => {
    const pairs: Array<[string, string]> = [
      ['no_pairwise_comparisons', 'no_pairwise_related_comparisons'],
      ['two_sided_only', 'related_tests_two_sided_only'],
      ['no_location_estimate', 'no_differences_location_estimate'],
      ['exact_without_ties_or_zeros_only', 'signed_rank_exact_p_rule'],
      ['pratt_not_offered', 'zero_differences_dropped'],
      ['no_further_related_tests', 'no_further_related_tests'],
      ['column_order', 'related_columns_in_chosen_order'],
    ];
    for (const [ours, cap] of pairs) {
      expect(limit(cap), cap).toBeTruthy();
      expect(words.relatedRanksSentence({ code: `stats.related_ranks.${ours}` }), ours).toBe(limit(cap));
    }
  });

  it("says decision 654's settings and card words word for word", () => {
    expect(words.RELATED_RANKS_LEGEND).toBe('Which columns do you want to compare? Each row is one case, measured in every chosen column.');
    expect(words.RELATED_RANKS_LIST_HEADING).toBe('Compared, in this order');
    expect(words.RELATED_RANKS_RULE).toBe('Two columns give the Wilcoxon signed-rank test; three or more give the Friedman test. A row is used only when every chosen column has a value.');
    expect(words.relatedRanksDifferenceSentence('Before', 'After')).toBe('The differences are Before less After.');
    expect(words.RELATED_RANKS_TOO_FEW).toBe('Choose at least two columns.');
    expect(words.relatedRanksTooManyColumns(51)).toBe('Choose 50 columns or fewer; 51 are chosen.');
    expect(words.relatedRanksMoveUp('C3')).toBe('Move up C3');
    expect(words.relatedRanksMoveDown('C3')).toBe('Move down C3');
    expect(words.RELATED_RANKS_HEADINGS).toEqual({ columns: 'Columns', ranks: 'Ranks of the differences', wilcoxon: 'Wilcoxon signed-rank test', friedman: 'Friedman test' });
    expect(words.RELATED_COLUMN_HEADS).toEqual(['Column', 'Median']);
    expect(words.RELATED_FRIEDMAN_HEADS).toEqual(['Mean rank', 'Sum of ranks']);
    expect(words.RELATED_RANK_HEADS).toEqual(['N', 'Mean rank', 'Sum of ranks']);
    expect(words.RELATED_RANK_ROWS).toEqual({ negative: 'Negative differences', positive: 'Positive differences', zero: 'No difference (left out)' });
    expect(words.RELATED_NO_PAIRS).toBe('No pairs.');
    expect(words.WILCOXON_ROW_LABELS).toEqual({ v: 'V', z: 'z', p: 'p (normal approximation)', exactP: 'Exact p', rankBiserial: 'Matched-pairs rank-biserial correlation', medianDifference: 'Median of the differences' });
    expect(words.RELATED_EXACT_NOT_GIVEN_CELL).toBe('Not given (see the note below).');
    expect(words.FRIEDMAN_ROW_LABELS).toEqual({ q: 'Q (chi-square)', df: 'df', p: 'p', kendallsW: "Kendall's W" });
    expect(words.relatedRanksWilcoxonSentence('A', 'B')).toBe(
      "The differences are A less B; a pair with no difference is left out of the ranks. V is the sum of the positive differences' ranks. z and the matched-pairs rank-biserial correlation are positive when A's values tend to be the larger."
    );
    expect(words.relatedRanksFriedmanSentence(4)).toBe("Within each row, the columns' values are ranked from 1 to 4, tied values sharing their mean rank.");
    expect(words.RELATED_RANKS_EXPORT_CHOICES).toEqual([
      { format: 'csv', label: 'The result tables (CSV)', file: 'related_ranks.csv', mime: 'text/csv' },
      { format: 'manifestJson', label: 'The run manifest (JSON)', file: 'related_ranks_manifest.json', mime: 'application/json' },
    ]);
  });

  it("writes a code's question from alpha's places, and what ticking it does", () => {
    const where = [{ header: 'a', cells: 1 }, { header: 'b', cells: 2 }];
    const alpha = alphaMissingCodeQuestion(-99, where);
    const ours = words.relatedRanksMissingCodeQuestion(-99, where);
    expect(ours.split('. If you tick')[0]).toBe(alpha.split('. If you tick')[0]);
    expect(ours).toBe('Is -99 a code for a missing value? It appears in "a" (1 cell), "b" (2 cells). If you tick it, it is treated as missing in every chosen column, and a row with it is left out.');
  });

  it('reads its column limit from the wire fixture, not typed twice', () => {
    expect(RELATED_LIMITS).toEqual({ maxColumns: 50 });
    expect(words.MAX_RELATED_COLUMNS).toBe(RELATED_LIMITS.maxColumns);
    const source = fs.readFileSync(path.join(__dirname, 'relatedRanksWording.ts'), 'utf8');
    expect(source).toContain("from './fixtures/relatedRanksLimits.json'");
    expect(source).toMatch(/MAX_RELATED_COLUMNS: number = RELATED_LIMITS\.maxColumns/);
    expect(source).not.toMatch(/MAX_RELATED_COLUMNS[^\n]*= *50/);
  });
});
