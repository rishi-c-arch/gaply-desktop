// The descriptive summary's words (Research OS, Phase 2B): every code in
// the closed set (describeCodes.json, pinned to Rust's DescCode and RunCode
// by src-tauri/tests/stats_desc_wire.rs) has a sentence; an ingest refusal
// is said in Level 1's words; the tone is neutral.
import { describe, expect, it } from 'vitest';

import CODES from './describeCodes.json';
import LIMITS from './describeLimits.json';
import {
  COULD_NOT_COMPUTE,
  diagnosticSentence,
  LEVELS_RULE,
  missingCodeQuestion,
  notComputableReason,
  refusalSentence,
  RUN_STATE_NAMES,
  runEndedSentence,
  shortHash,
  shown,
  STAGES,
  WORDS,
} from './describeWording';
import { sentenceFor } from '../statsverifier/l1/l1Wording';
import type { Diagnostic } from './describeTypes';

const refusal = (code: Record<string, unknown>) => new Error(JSON.stringify(code));

describe('describeWording', () => {
  // Evaluation of Phases 2 and 3, F11: the ceiling in the sentence is read
  // from describeLimits.json, which a Rust test pins to runs::RUN_CEILING_MS.
  it('says the run ceiling the run manager enforces, not a literal', () => {
    expect(runEndedSentence('timed_out')).toBe(
      `This run passed its ${LIMITS.runCeilingMs / 1000}-second limit and was stopped, so it has no result.`
    );
  });

  it('has a sentence for every code, and no code it does not know', () => {
    // 18 DescCode + 11 RunCode (run.wrong_kind joined in Phase 4B-2).
    expect(CODES.length).toBe(29);
    for (const code of CODES) {
      const d = { code, params: { column: 2, count: 3, value: -99, index: 0, expected: 1, found: 2 }, severity: 'error' } as Diagnostic;
      const s = diagnosticSentence(d, (a) => `"col${a}"`);
      expect(s, code).not.toMatch(/no wording/);
      expect(s, code).toMatch(/\.$/);
      expect(s, code).not.toContain('undefined');
    }
    expect(Object.keys(WORDS).sort()).toEqual([...CODES].sort());
  });

  it('names the column by its header and the value in a suspect-code sentence', () => {
    const d: Diagnostic = { code: 'stats.desc.suspect_missing_code', params: { column: 2, count: 3, value: -99 }, severity: 'warning' };
    expect(diagnosticSentence(d, () => '"dose"')).toBe(
      'Column "dose" holds -99 in 3 cells, which may be a code for a missing value. It is counted as a number unless you tick it in Settings.'
    );
  });

  it('says how levels are read (decision 17)', () => {
    expect(LEVELS_RULE).toContain('1 and 1.0 are one level');
    expect(LEVELS_RULE).toContain('trimmed');
  });

  it('asks about a candidate missing code by its value, and says what a tick does', () => {
    const q = missingCodeQuestion(-99, [{ header: 'dose', cells: 2 }, { header: 'age', cells: 1 }]);
    expect(q).toBe(
      'Is -99 a code for a missing value? It appears in "dose" (2 cells), "age" (1 cell). If you tick it, every -99 in the columns you summarise as numeric is left out.'
    );
  });

  it('says why a statistic could not be computed, in a neutral sentence', () => {
    expect(COULD_NOT_COMPUTE).toBe('Could not compute');
    expect(notComputableReason('stats.desc.single_value')).toBe('A standard deviation needs at least two values.');
    expect(notComputableReason('stats.desc.no_values')).toBe('The column has no values.');
  });

  it('words a run refusal itself and an ingest refusal in Level 1 words', () => {
    expect(refusalSentence(refusal({ code: 'run.run_busy' }))).toBe('A summary is already running. Wait for it to end, or cancel it.');
    const ingest = { code: 'file_too_large', limit_rows: 5000 };
    expect(refusalSentence(refusal(ingest))).toBe(sentenceFor(ingest as never));
    expect(refusalSentence(refusal(ingest))).toContain('5000');
    expect(refusalSentence(new Error('plain'))).toBe('This could not be done: plain');
    // F4: the cell text cap, as stats_desc_open refuses it.
    const cap = { code: 'content_too_large', limit_bytes: 2_097_152 };
    expect(refusalSentence(refusal(cap))).toBe(sentenceFor(cap as never));
    expect(refusalSentence(refusal(cap))).toMatch(/^This file holds more than 2 MB of text across its cells/);
  });

  it('is neutral: no state is a success, and an ended run says why it has no result', () => {
    for (const name of Object.values(RUN_STATE_NAMES)) expect(name).not.toMatch(/success|pass|great|well done/i);
    expect(RUN_STATE_NAMES.succeeded).toBe('Finished');
    expect(runEndedSentence('cancelled')).toBe('This run was cancelled, so it has no result.');
    expect(runEndedSentence('timed_out')).toContain('30-second');
  });

  it('lists the stages in order and formats numbers and hashes for the screen', () => {
    expect(STAGES.map((s) => s.label)).toEqual(['Import', 'Profile', 'Settings', 'Validate', 'Run', 'Results', 'Export']);
    expect(shown(52.104288888888888)).toBe('52.1043');
    expect(shown(10.5)).toBe('10.5');
    expect(shortHash('5fd0af5daee4d8dfcb466411096e7f26e513bf2cadfb0e1605ff085026fe5b4a')).toBe('5fd0af5daee4…');
  });
});
