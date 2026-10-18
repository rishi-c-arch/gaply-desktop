// Research OS evaluation of Phases 2 and 3, F10 (decision 52): a CSV or TSV
// saved in Latin-1 or UTF-16 has its own sentence, which says what to do,
// and is not "malformed". The descriptive summary words ingest refusals with
// this table too, so the sentence names no level.
import { describe, expect, it } from 'vitest';

import codes from './l1Codes.json';
import { sentenceFor } from './l1Wording';
import { refusalSentence } from '../../statistics/describeWording';

describe('encoding_not_utf8', () => {
  it('is a code Rust pins, with its own sentence', () => {
    expect(codes).toContain('encoding_not_utf8');
    const s = sentenceFor({ code: 'encoding_not_utf8' });
    expect(s).toBe(
      'This file is text saved in an encoding other than UTF-8, such as Latin-1 or UTF-16, so it was not opened. Save a copy in UTF-8 and open that copy instead.'
    );
    expect(s).not.toBe(sentenceFor({ code: 'malformed' }));
    expect(s).not.toMatch(/Level 1|no wording/);
  });

  it('reads the same on the descriptive summary', () => {
    // As stats_desc_open refuses it: a validation error whose message is the code.
    const refused = { code: 'validation', message: JSON.stringify({ code: 'encoding_not_utf8' }) };
    expect(refusalSentence(refused)).toBe(sentenceFor({ code: 'encoding_not_utf8' }));
  });
});
