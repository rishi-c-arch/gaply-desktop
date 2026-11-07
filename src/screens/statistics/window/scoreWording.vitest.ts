// The scale score's dialog's words (Research OS, Phase 8T-2a, decisions 504,
// 507 and 508): every new sentence and label in windowWording.ts, read for the
// words a verdict would use, with alpha's and decision 66's lists copied, and
// for remove, drop and should, which a deletion's words never use. And the
// name's limit: the screen's SCORE_NAME_MAX_CHARS, which a Rust test pins to
// stats_prep::NAME_MAX_CHARS, is the number name_length's sentence gives.
import { describe, expect, it } from 'vitest';

import { SCORE_NAME_MAX_CHARS } from '../prepTypes';
import { prepSentence } from '../prepWording';
import {
  ASK_DELETE_ALL,
  ASK_DELETE_SCORE,
  SCORE_AT_LEAST_LABEL,
  SCORE_CODES_LEGEND,
  SCORE_DELETE,
  SCORE_DELETE_ALL,
  scoreDeleteLabel,
  scoreDeletedSentence,
  SCORE_DIALOG_TITLE,
  SCORE_HELD_READING,
  SCORE_ITEMS_LEGEND,
  SCORE_MAKE,
  scoreMadeSentence,
  SCORE_MISSING,
  scoreMissingSentence,
  SCORE_NAME_LABEL,
  SCORE_RULE_AT_LEAST,
  SCORE_RULE_EVERY,
  SCORE_RULE_LEGEND,
  SCORE_SCALE_LEGEND,
  SCORE_STATISTIC_CHOICES,
  SCORE_STATISTIC_LEGEND,
  SCORES_DELETED,
  SCORES_HEADING,
} from './windowWording';

/** alphaWording.vitest.ts:20, copied: the words a verdict on alpha would use (decision 384). */
const VERDICT = /\b(acceptable|unacceptable|good|excellent|questionable|poor|reliable|unreliable|adequate|sufficient)\b|0\.7\b|[*★☆]|\b(should|recommend|drop|remove)\b/i;

/** PairedFlow.vitest.tsx:135, copied: decision 66's words. */
const VERDICT_66 = /significan|[*★☆]|\breject|\bconfirm(s|ed)? that|\bproves?\b|null hypothesis|\b(t-)?test (passed|failed)\b|\bfail(s|ed)? the test\b/i;

const COUNTS = { scoredFromEveryItem: 280, scoredFromFewer: 12, notScoredTooFew: 8, notScoredOverflow: 0 };

/** Every new string, each function called. */
function everyNewString(): string[] {
  return [
    SCORE_DIALOG_TITLE,
    SCORE_NAME_LABEL,
    SCORE_ITEMS_LEGEND,
    SCORE_SCALE_LEGEND,
    SCORE_STATISTIC_LEGEND,
    ...SCORE_STATISTIC_CHOICES.map((c) => c.label),
    SCORE_RULE_LEGEND,
    SCORE_RULE_EVERY,
    SCORE_RULE_AT_LEAST,
    SCORE_AT_LEAST_LABEL,
    SCORE_CODES_LEGEND,
    SCORE_MAKE,
    SCORE_HELD_READING,
    SCORE_MISSING.name,
    SCORE_MISSING.nameLong,
    SCORE_MISSING.twoItems,
    SCORE_MISSING.oneMoreItem,
    SCORE_MISSING.lowest,
    SCORE_MISSING.highest,
    SCORE_MISSING.statistic,
    SCORE_MISSING.rule,
    SCORE_MISSING.atLeast(1),
    SCORE_MISSING.atLeast(6),
    scoreMissingSentence([SCORE_MISSING.name]) ?? '',
    scoreMissingSentence([SCORE_MISSING.name, SCORE_MISSING.twoItems, SCORE_MISSING.statistic, SCORE_MISSING.rule]) ?? '',
    SCORES_HEADING,
    SCORE_DELETE,
    SCORE_DELETE_ALL,
    scoreDeleteLabel('Total'),
    ASK_DELETE_SCORE.title,
    ASK_DELETE_SCORE.sentence('Total'),
    ASK_DELETE_ALL.title,
    ASK_DELETE_ALL.sentence,
    scoreMadeSentence('Total', COUNTS),
    scoreDeletedSentence('Total'),
    SCORES_DELETED,
  ];
}

describe("the score dialog's words", () => {
  it('hold no verdict word, by either list, and the lists do catch one', () => {
    const all = everyNewString();
    expect(all.length).toBe(37);
    for (const s of all) expect(s.length, s).toBeGreaterThan(2);
    expect(all.filter((s) => VERDICT.test(s))).toEqual([]);
    expect(all.filter((s) => VERDICT_66.test(s))).toEqual([]);
    expect(VERDICT.test('Remove the score.')).toBe(true);
    expect(VERDICT_66.test('The test failed.')).toBe(true);
  });

  it('use none of remove, drop or should, and say Delete', () => {
    for (const s of everyNewString()) expect(s, s).not.toMatch(/\b(remove|drop|should)\b/i);
    expect(SCORE_DELETE).toBe('Delete');
    expect(SCORE_DELETE_ALL).toBe('Delete every score');
  });

  it('say, before a deletion, that the file is not changed, that the Output keeps it, and that it can be made again', () => {
    for (const s of [ASK_DELETE_SCORE.sentence('Total'), ASK_DELETE_ALL.sentence]) {
      expect(s).toMatch(/deleted from the table/);
      expect(s).toMatch(/file on disk is not changed/);
      expect(s).toMatch(/results already in the Output keep/);
      expect(s).toMatch(/make (it|them) again/);
    }
  });

  it('give a title that names no score, and a Delete label that does', () => {
    expect(ASK_DELETE_SCORE.title).not.toContain('Total');
    expect(scoreDeleteLabel('Total')).toBe('Delete the score "Total"');
  });

  it("say a made score's four counts, through scoreCountsSentence", () => {
    expect(scoreMadeSentence('Total', COUNTS)).toBe('The score was made. "Total": 280 rows scored from every item, 12 from fewer; 8 not scored, too few items; 0 not scored, too large to hold.');
  });

  it('list what is missing in one sentence, or say nothing', () => {
    expect(scoreMissingSentence([])).toBeNull();
    expect(scoreMissingSentence([SCORE_MISSING.rule])).toBe('To make the column, choose when a person gets a score.');
    expect(scoreMissingSentence([SCORE_MISSING.twoItems, SCORE_MISSING.rule])).toBe('To make the column, choose two items and when a person gets a score.');
  });
});

describe("the name's limit (decision 508; 8T-1b's question 2)", () => {
  it('is 128, the number name_length\'s sentence gives', () => {
    expect(SCORE_NAME_MAX_CHARS).toBe(128);
    expect(prepSentence({ code: 'stats.prep.name_length', params: { index: 0, count: 200 } })).toContain(String(SCORE_NAME_MAX_CHARS));
  });

  it("is the number the form's sentence gives", () => {
    expect(SCORE_MISSING.nameLong).toContain(String(SCORE_NAME_MAX_CHARS));
  });
});
