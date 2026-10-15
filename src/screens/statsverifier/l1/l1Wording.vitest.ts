// The Level 1 wording table against the shared code set (Phase 2, Step 7).
// `l1Codes.json` is pinned on the Rust side by gaply-core's
// `tests/stats_l1_code_set.rs` to exactly the codes `stats_l1::codes` emits;
// this file requires one entry per code there, and no other.
import { describe, expect, it } from 'vitest';

import CODES from './l1Codes.json';
import { NOT_REPRODUCED_TEXT, refusalSentence, sentenceFor, WORDS } from './l1Wording';

const ctx = { column: (a: number) => `C${a} anxiety_post` };

/** Every code with the parameters it can carry, so each sentence renders in
 *  full: numbers for the numeric ones, a nested code for the wrappers. */
function sample(code: string) {
  return {
    code,
    index: 0,
    n: 2,
    minimum: 3,
    groups: 1,
    empty_rows: 1,
    empty_columns: 0,
    column: 3,
    columns: 200,
    limit: 128,
    bytes: 10,
    limit_rows: 5000,
    printed: 1,
    expected: 2,
    note: 'one_tailed_stated',
    reason: 'invalid_input',
    why: 'spearman_p_may_be_exact',
    number: 'p',
    variant: 'welch',
    term: 'intercept',
    gap: 'several_alphas',
    because: 'claim_revised',
    field: 'fingerprint',
    rejection: 'unknown_token',
    refusal: 'zero_variance',
    at: 'input',
    source: 'claim_sentence',
  };
}

describe('the Level 1 wording table', () => {
  it('has exactly one entry per code in the shared code set', () => {
    expect(CODES.length).toBeGreaterThan(100);
    expect(Object.keys(WORDS).sort()).toEqual([...CODES].sort());
  });

  it('words every code without falling back, and says nothing forbidden', () => {
    for (const code of CODES) {
      const s = sentenceFor(sample(code), ctx);
      expect(s, code).not.toMatch(/no wording/);
      expect(s.length, code).toBeGreaterThan(2);
      // Phase 1's wording rules: no score, percentage, gauge or probability,
      // and no claim that the paper is wrong.
      // "confidence interval" is the statistic's name, not a confidence.
      const words = s.toLowerCase().replace(/confidence interval/g, 'interval');
      expect(words, code).not.toMatch(/score|percent|%|gauge|probab|confiden|\bwrong\b|\berror\b|incorrect/);
    }
  });

  it('a code it does not know is shown as itself, never hidden', () => {
    expect(sentenceFor({ code: 'brand_new_code' })).toBe('Gaply has no wording for the code brand_new_code.');
  });

  it('nested reasons read through to the leaf', () => {
    expect(sentenceFor({ code: 'refused', refusal: 'zero_variance' })).toBe('A chosen column does not vary, so the statistic is undefined.');
    expect(sentenceFor({ code: 'token_stale', rejection: { code: 'revoked', because: 'claim_revised' } })).toBe(
      'The claim was edited after its binding was confirmed.'
    );
    expect(sentenceFor({ code: 'not_numeric', column: 3 }, ctx)).toBe('Column C3 anxiety_post holds values that are not numbers.');
  });

  it('Not reproduced says the data gives a different value and lists the usual causes', () => {
    expect(NOT_REPRODUCED_TEXT).toMatch(/data under this binding gives a different value/);
    for (const cause of ['subset of rows', 'version of the data', 'different method', 'typo']) expect(NOT_REPRODUCED_TEXT).toContain(cause);
    expect(NOT_REPRODUCED_TEXT.toLowerCase()).not.toMatch(/wrong|error|incorrect|false/);
  });

  it('a command refusal is read from its JSON message', () => {
    expect(refusalSentence(new Error('{"code":"ai_help_off"}'))).toBe('AI help is off in Settings, so nothing was sent.');
    expect(refusalSentence({ code: 'Validation', message: '{"code":"budget_spent","max":20}' })).toBe(
      'This check has used its 20 requests for this task.'
    );
    expect(refusalSentence(new Error('{"code":"transport","error":"timeout"}'))).toBe('The request timed out.');
    expect(refusalSentence(new Error('not json'))).toBe('Something went wrong, and nothing was changed.');
  });
});
