// The t-test's and the method selector's words (Research OS, Phase 4B-2).
// Every code the t-test screen can show (ttestCodes.json, pinned to Rust's
// closed sets by src-tauri/tests/stats_ttest_wire.rs) has a sentence, and so
// does every capability wording code (capabilityCodes.json, pinned to
// StatsWording). The wording is neutral (decision 66): no sentence anywhere
// in these tables says "significant" or carries a star.
import { describe, expect, it } from 'vitest';

import ttestCodes from './ttestCodes.json';
import capabilityCodes from './capabilityCodes.json';
import * as words from './ttestWording';
import { CAPABILITY_WORDS, capabilitySentence, HOME_TEXT, LIFECYCLE_LABELS } from './capabilityWording';

/** Parameters every sentence can be asked with: numbers for the t-test's
 *  and the registry's codes, closed words for the workspace's. */
const NUMBERS = { column: 1, count: 3, index: 1, value: 99, limit: 2, expected: 1, found: 2, lifecycle: 'planned' };
const WORDS = { workspace: 'statistics', expected: 'statistics', found: 'statistics', from: 'active', event: 'activate', run: 4 };
const paramsFor = (code: string) => (code.startsWith('workspace.') ? WORDS : NUMBERS);
const NAMES = { column: (a: number) => `"col${a}"`, level: (i: number) => `lvl${i}` };

/** Every string a module exports, sentences built from every code included. */
function everySentence(): string[] {
  const out: string[] = [];
  const collect = (v: unknown) => {
    if (typeof v === 'string') out.push(v);
    else if (Array.isArray(v)) v.forEach(collect);
    else if (v && typeof v === 'object') Object.values(v).forEach(collect);
  };
  for (const [name, v] of Object.entries(words)) {
    if (name === 'TTEST_WORDS') continue;
    if (typeof v !== 'function') collect(v);
  }
  for (const code of ttestCodes) out.push(words.ttestSentence({ code, params: paramsFor(code), count: 2 }, NAMES));
  for (const state of ['cancelled', 'timed_out', 'resource_limit', 'refused', 'needs_user_input', 'failed', 'succeeded']) {
    out.push(words.ttestRunEndedSentence(state));
  }
  out.push(words.levelsQuestion(3), words.ttestMissingCodeQuestion(-99, 'score', 2));
  collect(CAPABILITY_WORDS);
  collect(HOME_TEXT);
  collect(LIFECYCLE_LABELS);
  return out;
}

describe('the t-test screen words every code it can show', () => {
  it('has a sentence for each code in ttestCodes.json, and none for any other', () => {
    expect(ttestCodes).toHaveLength(51);
    const missing = ttestCodes.filter((c) => !(c in words.TTEST_WORDS));
    expect(missing).toEqual([]);
    expect(Object.keys(words.TTEST_WORDS).sort()).toEqual([...ttestCodes].sort());
    for (const code of ttestCodes) {
      const s = words.ttestSentence({ code, params: paramsFor(code), count: 2 }, NAMES);
      expect(s, code).not.toMatch(/no wording/);
      expect(s, code).toMatch(/[.:]$/);
      expect(s, code).not.toContain('undefined');
    }
  });

  it('words a refusal from the run manager, the registry, the workspace and the ingest', () => {
    const err = (v: unknown) => ({ code: 'validation', message: JSON.stringify(v) });
    expect(words.ttestRefusalSentence(err({ code: 'run.wrong_kind' }))).toBe('This run is not a t-test, so this screen cannot read it.');
    expect(
      words.ttestRefusalSentence(err({ code: 'capability.version_mismatch', params: { expected: 1, found: 2 }, severity: 'error' }))
    ).toBe('These settings were made for version 2 of the method, and this is version 1.');
    expect(
      words.ttestRefusalSentence(err({ code: 'workspace.inactive', params: { expected: 'statistics' }, severity: 'error' }))
    ).toBe('This needs the Statistics workspace, which is not open.');
    // An ingest refusal is Level 1's own code, in Level 1's words.
    expect(words.ttestRefusalSentence(err({ code: 'encoding_not_utf8' }))).not.toMatch(/no wording/);
  });
});

describe('the method selector words every capability code', () => {
  it('has a sentence for each code in capabilityCodes.json, and none for any other', () => {
    expect(Object.keys(CAPABILITY_WORDS).sort()).toEqual([...capabilityCodes].sort());
    for (const code of capabilityCodes) expect(capabilitySentence(code), code).not.toBe(code);
  });

  it('gives the five codes Phase 4B-1 left without sentences their own', () => {
    for (const code of [
      'stats.capability.analyze.independent_t.title',
      'stats.capability.analyze.independent_t.description',
      'stats.capability.limit.analyze.levels_follow_level_1',
      'stats.capability.limit.analyze.normality_not_assessed',
      'stats.capability.limit.analyze.cohens_d_has_no_interval',
    ]) {
      expect(capabilityCodes).toContain(code);
      expect(capabilitySentence(code).length, code).toBeGreaterThan(10);
    }
  });
});

describe('neutral wording (decision 66)', () => {
  it('no sentence says "significant" or "not significant", and none carries a star', () => {
    const all = everySentence();
    expect(all.length).toBeGreaterThan(100);
    const bad = all.filter((s) => /significan/i.test(s) || /[*★☆]/.test(s));
    expect(bad).toEqual([]);
  });
});
