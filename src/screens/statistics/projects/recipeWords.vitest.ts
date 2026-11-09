// The words for a project's recipes (Research OS, Phase 8U-3, decision 555),
// each word for word, and none of them a verdict, by the three lists decision
// 557 names, copied here with their lines.
import { describe, expect, it } from 'vitest';

import type { ScoreView } from './projectTypes';
import {
  RECIPES_HEADING,
  RECIPES_UNREAD,
  recipeLine,
  recipeUnreadLine,
  RUN_SCORES_UNREAD,
  runScoresLine,
  scoreColumn,
} from './projectWording';

/** alphaWording.vitest.ts:20, decision 384's words. */
const ALPHA_VERDICT = /\b(acceptable|unacceptable|good|excellent|questionable|poor|reliable|unreliable|adequate|sufficient)\b|0\.7\b|[*★☆]|\b(should|recommend|drop|remove)\b/i;
/** PairedFlow.vitest.tsx:135, decision 66's words. */
const TEST_VERDICT = /significan|[*★☆]|\breject|\bconfirm(s|ed)? that|\bproves?\b|null hypothesis|\b(t-)?test (passed|failed)\b|\bfail(s|ed)? the test\b/i;
/** projectWording.vitest.ts:47. */
const PROJECT_VERDICT = /\b(significant|correct|incorrect|accurate|inaccurate|reliable|unreliable|trustworthy|wrong|invalid|suspicious|fraud)\b|★|✓/i;

const AVERAGE: ScoreView = { position: 51, name: 'Average', statistic: 'mean', items: 6 };
const TOTAL: ScoreView = { position: 52, name: 'Total', statistic: 'sum', items: 6 };
const OPEN: ScoreView = { position: 53, name: 'Open', statistic: null, items: 4 };

describe('the recipe words, word for word', () => {
  it('the heading and the two unread sentences', () => {
    expect(RECIPES_HEADING).toBe('Scale score recipes');
    expect(RECIPES_UNREAD).toBe("This project's scale score recipes could not be read.");
    expect(RUN_SCORES_UNREAD).toBe('Scale scores: their names could not be read.');
  });

  it('a score column: its C-number and the name, quoted', () => {
    expect(scoreColumn(51, 'Average')).toBe('C51 "Average"');
  });

  it('a recipe line, for one saved result and for more, each statistic', () => {
    expect(recipeLine(1, [AVERAGE])).toBe('Used by 1 saved result: C51 "Average", the mean of 6 items.');
    expect(recipeLine(3, [AVERAGE, TOTAL, OPEN])).toBe(
      'Used by 3 saved results: C51 "Average", the mean of 6 items; C52 "Total", the sum of 6 items; C53 "Open", from 4 items.'
    );
  });

  it('a recipe whose scores could not be read', () => {
    expect(recipeUnreadLine(1)).toBe('Used by 1 saved result. Its scores could not be read.');
    expect(recipeUnreadLine(2)).toBe('Used by 2 saved results. Its scores could not be read.');
  });

  it("a run's scores", () => {
    expect(runScoresLine([AVERAGE])).toBe('Scale scores: C51 "Average".');
    expect(runScoresLine([AVERAGE, TOTAL])).toBe('Scale scores: C51 "Average", C52 "Total".');
  });
});

describe('no recipe word is a verdict', () => {
  const all = [
    RECIPES_HEADING,
    RECIPES_UNREAD,
    RUN_SCORES_UNREAD,
    scoreColumn(5, 'x'),
    recipeLine(1, [AVERAGE]),
    recipeLine(2, [AVERAGE, TOTAL, OPEN]),
    recipeUnreadLine(1),
    recipeUnreadLine(4),
    runScoresLine([AVERAGE, TOTAL]),
  ];

  it.each([
    ['alphaWording.vitest.ts:20', ALPHA_VERDICT],
    ['PairedFlow.vitest.tsx:135', TEST_VERDICT],
    ['projectWording.vitest.ts:47', PROJECT_VERDICT],
  ])('by %s', (_where, list) => {
    expect(all.filter((s) => typeof s !== 'string' || list.test(s))).toEqual([]);
  });

  it('each list catches a verdict word', () => {
    expect(ALPHA_VERDICT.test('a good scale')).toBe(true);
    expect(TEST_VERDICT.test('significant')).toBe(true);
    expect(PROJECT_VERDICT.test('reliable')).toBe(true);
  });
});
