// The one-sample and paired t-tests' words (Research OS, Phase 8D-2b,
// decision 268). Every code either screen can show (onesampleCodes.json,
// pinned to Rust's closed sets by src-tauri/tests/stats_onesample_wire.rs)
// has a sentence, the paired codes included; decision 259's two sentences
// are word for word; and the wording is neutral (decision 66): no sentence
// says "significant", carries a star, or uses a verdict word.
import { describe, expect, it } from 'vitest';

import onesampleCodes from './onesampleCodes.json';
import * as words from './onesampleWording';

const NUMBERS = { column: 1, count: 3, index: 1, value: 99, limit: 2, expected: 1, found: 2, lifecycle: 'planned' };
const WORDS = { workspace: 'statistics', expected: 'statistics', found: 'statistics', from: 'active', event: 'activate', run: 4 };
const paramsFor = (code: string) => (code.startsWith('workspace.') ? WORDS : NUMBERS);
const NAMES = { column: (a: number) => `"col${a}"` };

/** Every string the module exports, and every sentence it can build. */
function everySentence(): string[] {
  const out: string[] = [];
  const collect = (v: unknown) => {
    if (typeof v === 'string') out.push(v);
    else if (Array.isArray(v)) v.forEach(collect);
    else if (v && typeof v === 'object') Object.values(v).forEach(collect);
  };
  for (const [name, v] of Object.entries(words)) {
    if (name === 'ONE_SAMPLE_WORDS') continue;
    if (typeof v !== 'function') collect(v);
  }
  for (const code of onesampleCodes) {
    for (const count of [1, 2]) out.push(words.oneSampleSentence({ code, params: { ...paramsFor(code), count }, count }, NAMES));
  }
  for (const state of ['cancelled', 'timed_out', 'resource_limit', 'refused', 'needs_user_input', 'failed', 'succeeded']) {
    out.push(words.oneSampleRunEndedSentence(state));
  }
  out.push(words.oneSampleMissingCodeQuestion(-99, 'weight', 2));
  return out;
}

describe('the one-sample and paired screens word every code they can show', () => {
  it('has a sentence for each code in onesampleCodes.json, and none for any other', () => {
    expect(onesampleCodes).toHaveLength(51);
    expect(Object.keys(words.ONE_SAMPLE_WORDS)).toHaveLength(onesampleCodes.length);
    expect(Object.keys(words.ONE_SAMPLE_WORDS).sort()).toEqual([...onesampleCodes].sort());
    for (const code of onesampleCodes) {
      const s = words.oneSampleSentence({ code, params: paramsFor(code), count: 2 }, NAMES);
      expect(s, code).not.toMatch(/no wording/);
      expect(s, code).toMatch(/[.:]$/);
      expect(s, code).not.toContain('undefined');
    }
  });

  it('uses decision 259’s two sentences and decision 268’s d sentence word for word', () => {
    expect(words.oneSampleSentence({ code: 'stats.onesample.values_essentially_constant' })).toBe(
      'The values vary so little, compared with their size, that the standard error is lost in rounding, so t is not computed. R refuses such data too: "data are essentially constant".'
    );
    expect(words.oneSampleSentence({ code: 'stats.onesample.differences_essentially_constant' })).toBe(
      'The differences vary so little, compared with the size of the values, that the standard error is lost in rounding, so t is not computed. This happens when every pair differs by the same amount, such as 0.1, which a computer stores with a tiny rounding error.'
    );
    expect(words.oneSampleSentence({ code: 'stats.onesample.cohens_d_has_no_interval' })).toBe(
      "Cohen's d is shown without a confidence interval: Gaply does not compute one for d yet."
    );
    expect(words.TEST_VALUE_LINE).toBe('The mean difference is the mean minus the test value.');
  });

  it('words a refusal from the run manager, the registry, the workspace and the ingest', () => {
    const err = (v: unknown) => ({ code: 'validation', message: JSON.stringify(v) });
    expect(words.oneSampleRefusalSentence(err({ code: 'run.run_busy' }))).toBe('Another run is going in this workspace. Wait for it to end, or cancel it.');
    expect(
      words.oneSampleRefusalSentence(err({ code: 'capability.version_mismatch', params: { expected: 1, found: 2 }, severity: 'error' }))
    ).toBe('These settings were made for version 2 of the method, and this is version 1.');
    expect(words.oneSampleRefusalSentence(err({ code: 'workspace.inactive', params: { expected: 'statistics' }, severity: 'error' }))).toBe(
      'This needs the Statistics workspace, which is not open.'
    );
    expect(words.oneSampleRefusalSentence(err({ code: 'encoding_not_utf8' }))).not.toMatch(/no wording/);
    // A word from Object.prototype is not a code.
    expect(words.oneSampleSentence({ code: 'toString' })).toBe('Gaply has no wording for the code toString.');
  });

  it('names both tests’ export files', () => {
    expect(words.ONE_SAMPLE_EXPORT_CHOICES.map((c) => c.file)).toEqual(['one_sample_t_test.csv', 'one_sample_t_test_manifest.json']);
    expect(words.PAIRED_EXPORT_CHOICES.map((c) => c.file)).toEqual(['paired_t_test.csv', 'paired_t_test_manifest.json']);
  });
});

describe('neutral wording (decision 66)', () => {
  it('no sentence says "significant", carries a star, or passes a verdict', () => {
    const all = everySentence();
    expect(all.length).toBeGreaterThan(100);
    const bad = all.filter(
      (s) =>
        /significan/i.test(s) ||
        /[*★☆]/.test(s) ||
        /\breject|\bconfirm(s|ed)? that|\bproves?\b|null hypothesis|\b(t-)?test (passed|failed)\b|\bfail(s|ed)? the test\b/i.test(s)
    );
    expect(bad).toEqual([]);
  });
});
