// Scale scores' words (Research OS, Phase 8T-1b, decision 499). Every code in
// prepCodes.json but the four count codes has a sentence and a row of its
// parameters; no sentence holds a verdict word; a column that is a score is
// said by the name typed for it, never its header; a step is said by its typed
// name, but in the name codes' sentences, which never quote it; no sentence
// echoes a scale end or a missing code; and a refusal's own text is never
// shown.
import { describe, expect, it } from 'vitest';

import prepCodes from './prepCodes.json';
import type { DatasetView, ScoreCounts } from './describeTypes';
import type { Recipe, ScaleScoreStep } from './prepTypes';
import * as words from './prepWording';

/** Alpha's verdict words, copied from alphaWording.vitest.ts:20 (decision 384),
 *  as AlphaFlow.vitest.tsx:38 copies it: importing a vitest file would run its
 *  tests again. It also forbids "should", "recommend", "drop" and "remove". */
const VERDICT = /\b(acceptable|unacceptable|good|excellent|questionable|poor|reliable|unreliable|adequate|sufficient)\b|0\.7\b|[*★☆]|\b(should|recommend|drop|remove)\b/i;

/** Decision 66's words, copied from PairedFlow.vitest.tsx:135. */
const VERDICT_66 = /significan|[*★☆]|\breject|\bconfirm(s|ed)? that|\bproves?\b|null hypothesis|\b(t-)?test (passed|failed)\b|\bfail(s|ed)? the test\b/i;

const COUNTS = new Set<string>(words.COUNT_CODES);
const WORDED = (prepCodes as string[]).filter((c) => !COUNTS.has(c));

const column = (alias: string, header: string, scoreName?: string) => ({
  alias,
  header,
  kind: 'numeric' as const,
  n: 10,
  missing: 0,
  distinct: 5,
  levels: null,
  suspectMissingCodes: [],
  nonFinite: 0,
  ...(scoreName === undefined ? {} : { prepared: true as const, scoreName }),
});

/** A file of three columns and one score, C4, whose header is its alias word. */
const VIEW: DatasetView = {
  slot: 3,
  displayName: 'survey.csv',
  profile: {
    rows: 10,
    columns: [column('C1', 'Calm'), column('C2', 'Tense'), column('C3', 'Rested'), column('C4', 'C4', 'Mood')],
  },
  inputRef: { contentHash: 'ab'.repeat(32), rows: 10, columns: 4, semanticRole: 'table' },
};

const step = (alias: string, name: string, over: Partial<ScaleScoreStep> = {}): ScaleScoreStep => ({
  kind: 'scale_score',
  alias,
  name,
  items: ['C1', 'C2'],
  reversed: [],
  scale: null,
  statistic: 'mean',
  rule: 'every_item',
  missingCodes: [],
  ...over,
});

/** Two scores: the one held, and the one the dialog adds, last, with ends and
 *  codes no sentence may echo. */
const RECIPE: Recipe = {
  schemaVersion: 1,
  capabilityId: 'stats.prepare.scale_score',
  capabilityVersion: 1,
  engineVersion: 1,
  sourceColumns: 3,
  steps: [
    step('C4', 'Mood'),
    step('C5', 'Wellbeing', { reversed: ['C2'], scale: { min: 1234.5, max: 6789.5 }, missingCodes: [-777, 8888] }),
  ],
};

const COUNTS_OF: ScoreCounts[] = [{ column: 'C5', scoredFromEveryItem: 0, scoredFromFewer: 0, notScoredTooFew: 10, notScoredOverflow: 0 }];

/** A value for each parameter a code carries, as the code that sends it does. */
const VALUE: Record<string, number | string> = {
  index: 1,
  column: 4,
  count: 3,
  expected: 7,
  found: 9,
  limit: 4,
  from: 0,
  workspace: 'statistics',
  run: 4,
  event: 'activate',
  lifecycle: 'planned',
};
const paramsOf = (code: string) => {
  const out: Record<string, number | string> = {};
  for (const k of words.PREP_PARAMS[code] ?? []) out[k] = VALUE[k];
  if (code.startsWith('workspace.')) {
    if ('expected' in out) out.expected = 'statistics';
    if ('found' in out) out.found = 'statistics';
    if ('from' in out) out.from = 'active';
  }
  return out;
};
const sentence = (code: string) => words.prepSentence({ code, params: paramsOf(code) }, VIEW, RECIPE, COUNTS_OF);

/** Every sentence the module can give, with every code's parameters. */
function everySentence(): string[] {
  const out = WORDED.map(sentence);
  out.push(words.PREP_UNCODED);
  out.push(words.prepSentence({ code: 'stats.prep.name_length', params: { index: 0, count: 0 } }));
  out.push(words.prepSentence({ code: 'stats.prep.at_least_out_of_range', params: { index: 0, found: 1, limit: 0 } }));
  out.push(words.prepSentence({ code: 'stats.prep.no_row_scored', params: { column: 5 } }, VIEW, RECIPE));
  for (const code of WORDED) out.push(words.prepSentence({ code, params: paramsOf(code) }));
  return out;
}

describe("scale scores' words", () => {
  it('have a sentence and a row of parameters for every code in prepCodes.json but the four count codes', () => {
    expect(prepCodes).toHaveLength(59);
    expect(words.COUNT_CODES).toHaveLength(4);
    for (const c of words.COUNT_CODES) expect(prepCodes, c).toContain(c);
    expect(WORDED).toHaveLength(55);
    expect(Object.keys(words.PREP_PARAMS).sort()).toEqual([...WORDED].sort());
    expect(Object.keys(words.PREP_WORDS).sort()).toEqual([...WORDED].sort());
    for (const code of WORDED) {
      const s = sentence(code);
      expect(s, code).not.toMatch(/no wording/);
      expect(s, code).not.toMatch(/undefined|NaN|unknown number|\?/);
      expect(s, code).toMatch(/\.$/);
    }
  });

  it('give none of the count codes a sentence of its own', () => {
    for (const c of words.COUNT_CODES) {
      expect(words.PREP_WORDS[c], c).toBeUndefined();
      expect(words.prepSentence({ code: c }), c).toBe(`Gaply has no wording for the code ${c}.`);
    }
  });

  it('hold no verdict word', () => {
    const all = everySentence();
    expect(all.length).toBeGreaterThan(110);
    expect(all.filter((s) => VERDICT.test(s))).toEqual([]);
    expect(all.filter((s) => VERDICT_66.test(s))).toEqual([]);
    // The patterns do catch a verdict.
    expect(VERDICT.test('Remove the item.')).toBe(true);
    expect(VERDICT_66.test('The test failed.')).toBe(true);
  });

  it('use straight quotes, and echo no scale end or missing code from the recipe', () => {
    for (const s of everySentence()) {
      expect(s).not.toMatch(/[‘’“”]/);
      for (const v of ['1234.5', '6789.5', '777', '8888', '1,234', '6,789', '8,888']) expect(s, s).not.toContain(v);
    }
  });

  it("say a score column by the name typed for it, never its header", () => {
    const withColumn = WORDED.filter((c) => (words.PREP_PARAMS[c] ?? []).includes('column') && c.startsWith('stats.prep.'));
    expect(withColumn.length).toBeGreaterThan(10);
    for (const code of withColumn) {
      if (code === 'stats.prep.name_equals_file_header') continue; // the name codes name no column
      const s = words.prepSentence({ code, params: { ...paramsOf(code), column: 4 } }, VIEW, RECIPE, COUNTS_OF);
      expect(s, code).toContain('"Mood"');
      expect(s, code).not.toContain('C4');
    }
    // A file's column is said by its header.
    expect(words.prepSentence({ code: 'stats.prep.item_not_numeric', params: { index: 1, column: 2 } }, VIEW, RECIPE)).toBe(
      '"Tense" holds no numbers, so it cannot be an item of "Wellbeing".'
    );
    // With no view, a column is its alias.
    expect(words.prepSentence({ code: 'stats.prep.item_not_numeric', params: { index: 1, column: 2 } }, null, RECIPE)).toBe(
      'C2 holds no numbers, so it cannot be an item of "Wellbeing".'
    );
  });

  it('say a step by its typed name, but never quote it in a name code', () => {
    const nameCodes = WORDED.filter((c) => c.startsWith('stats.prep.name_'));
    expect(nameCodes).toHaveLength(6);
    for (const code of nameCodes) {
      const s = sentence(code);
      expect(s, code).toMatch(/^The name /);
      expect(s, code).not.toContain('Wellbeing');
      expect(s, code).not.toContain('Mood');
    }
    const stepCodes = WORDED.filter((c) => (words.PREP_PARAMS[c] ?? []).includes('index') && c.startsWith('stats.prep.') && !nameCodes.includes(c));
    expect(stepCodes.length).toBeGreaterThan(20);
    for (const code of stepCodes) expect(sentence(code), code).toContain('"Wellbeing"');
    // A name that is not plain is said by its place.
    const odd: Recipe = { ...RECIPE, steps: [RECIPE.steps[0], step('C5', ' Well​being')] };
    expect(words.prepSentence({ code: 'stats.prep.too_few_items', params: { index: 1, count: 1 } }, VIEW, odd)).toBe(
      'Score 2 needs at least 2 items. It has 1.'
    );
    expect(words.prepSentence({ code: 'stats.prep.too_few_items', params: { index: 1, count: 1 } })).toBe(
      'This score needs at least 2 items. It has 1.'
    );
  });

  it("say a missing code's place in the last step's codes", () => {
    expect(words.prepSentence({ code: 'stats.prep.missing_code_repeated', params: { index: 1 } }, VIEW, RECIPE)).toBe(
      'For "Wellbeing", missing code 2 is listed twice. List it once.'
    );
    expect(words.prepSentence({ code: 'stats.prep.missing_code_not_finite', params: { index: 0 } }, VIEW, RECIPE)).toBe(
      'For "Wellbeing", missing code 1 must be a finite number.'
    );
  });

  it("say no_row_scored with the step whose alias it is, and the counts", () => {
    expect(words.prepSentence({ code: 'stats.prep.no_row_scored', params: { column: 5 } }, VIEW, RECIPE, COUNTS_OF)).toBe(
      'No row would get a score for "Wellbeing", so it was not made. "Wellbeing": 0 rows scored from every item, 0 from fewer; 10 not scored, too few items; 0 not scored, too large to hold.'
    );
  });

  it("give decision 499's three examples word for word", () => {
    const one: Recipe = { ...RECIPE, steps: [step('C4', 'Wellbeing', { items: ['C1'] })] };
    expect(words.prepSentence({ code: 'stats.prep.too_few_items', params: { index: 0, count: 1 } }, VIEW, one)).toBe(
      '"Wellbeing" needs at least 2 items. It has 1.'
    );
    expect(words.prepSentence({ code: 'stats.prep.name_equals_file_header', params: { index: 0, column: 1 } }, VIEW, one)).toBe(
      'The name is already a column of the file. Give the score another name.'
    );
    expect(words.prepSentence({ code: 'stats.prep.at_least_out_of_range', params: { index: 0, found: 5, limit: 4 } }, VIEW, one)).toBe(
      'For "Wellbeing", the number of items a person needs must be from 1 to 4.'
    );
  });

  it('say a thrown refusal, and never show the text of an error with no code', () => {
    const refusal = (code: string, params?: object) => new Error(JSON.stringify(params === undefined ? { code } : { code, params }));
    expect(words.prepRefusalSentence(refusal('run.run_busy'))).toBe(words.PREP_WORDS['run.run_busy']({ code: 'run.run_busy' }, {}));
    expect(words.prepRefusalSentence(refusal('run.run_busy'))).toMatch(/while a run is going, or while another change to them is being made/);
    expect(words.prepRefusalSentence(refusal('run.unknown_input'))).toMatch(/no longer open, or it has no scores to export yet/);
    expect(words.prepRefusalSentence(refusal('workspace.inactive', { expected: 'statistics', from: 'no_workspace' }))).toMatch(/Statistics/);
    expect(words.prepRefusalSentence(refusal('capability.not_runnable', { lifecycle: 'planned' }))).toBe(
      'Scale scores are listed as Planned in this version of Gaply, so they cannot be made here.'
    );
    expect(words.prepRefusalSentence(new Error('invalid args `recipe`: "Wellbeing" 1234.5'))).toBe(words.PREP_UNCODED);
    expect(words.prepRefusalSentence('a string')).toBe(words.PREP_UNCODED);
    expect(words.prepRefusalSentence(refusal('stats.prep.not_a_code'))).toBe('Gaply has no wording for the code stats.prep.not_a_code.');
  });
});
