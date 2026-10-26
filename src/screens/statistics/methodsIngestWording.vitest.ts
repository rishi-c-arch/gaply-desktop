// The ingest's refusals on the descriptive summary's and the t-test's
// screens (Research OS, Phase 8B, decision 243): four sentences of their own,
// none naming Level 1, and every other code in Level 1's words; Level 1's own
// screens keep every sentence they had.
import { describe, expect, it } from 'vitest';

import { refusalSentence } from './describeWording';
import { ttestRefusalSentence } from './ttestWording';
import { INGEST_REFUSALS, rejection } from './fixtures/ingestRefusals';
import { refusalSentence as l1RefusalSentence, sentenceFor } from '../statsverifier/l1/l1Wording';

const SCREENS = [
  { screen: 'the summary', say: refusalSentence },
  { screen: 'the t-test', say: ttestRefusalSentence },
];

/** Decision 243's four sentences, with the numbers INGEST_REFUSALS carries. */
const OWN: Record<string, string> = {
  file_too_large: 'The file has more than 5000 rows. Gaply never computes on part of a file, so it was not opened.',
  'dataset too_wide': 'The file has 129 columns, more than the 128 Gaply reads from one file, so it was not opened.',
  'archive_too_large, over the limit':
    'This file would take at least 1500 MB once unpacked, more than the 64 MB Gaply opens, so it was not opened.',
  xls_not_readable:
    'This .xls file is in a format Gaply cannot read, such as an older Excel version. Save it as .xlsx and open that.',
};

/** What Level 1's screens said for each code at 735116e, before 8B. */
const LEVEL_1: Record<string, string> = {
  not_readable: 'The data file could not be read.',
  'too_large, the ingest cap': 'The data file is just over the 10.0 MB limit.',
  'too_large, import_guard': 'The data file is just over the 100.0 MB limit.',
  unsupported_format: 'This data file format is not supported.',
  malformed: 'The data file is malformed.',
  no_table: 'The data file holds no table.',
  file_too_large: 'The file has more than 5000 rows, and Level 1 never computes on part of a file.',
  file_changed_while_reading: 'The data file changed while it was being read.',
  xls_not_readable:
    'This .xls file is in a format Level 1 cannot read, such as an older Excel version. Save it as .xlsx and open that.',
  'dataset too_wide': 'The file has 129 columns, more than the 128 Level 1 reads.',
  'dataset no_columns': 'The file has no columns.',
  not_a_regular_file:
    'The chosen data file is not a regular file (it may be a pipe, a device or a socket), so it was not read.',
  'archive_too_large, over the limit':
    'This file would take at least 1500 MB once unpacked, more than the 64 MB Level 1 opens, so it was not opened.',
  'archive_too_large, past its own list': 'This file unpacks to more than its own contents list says, so it was not opened.',
  content_too_large:
    'This file holds more than 2 MB of text across its cells, more than Gaply reads from one file, so it was not opened. A copy with only the columns you need may fit.',
  encoding_not_utf8:
    'This file is text saved in an encoding other than UTF-8, such as Latin-1 or UTF-16, so it was not opened. Save a copy in UTF-8 and open that copy instead.',
};

describe('the ingest refusals on the methods screens', () => {
  it('says decision 243 four sentences exactly, with their numbers, on both screens', () => {
    for (const { screen, say } of SCREENS) {
      for (const { name, code } of INGEST_REFUSALS) {
        if (name in OWN) expect(say(rejection(code)), `${screen}: ${name}`).toBe(OWN[name]);
      }
    }
  });

  it('names Level 1 in no sentence either screen gives an ingest refusal', () => {
    for (const { screen, say } of SCREENS) {
      for (const { name, code } of INGEST_REFUSALS) {
        const said = say(rejection(code));
        expect(said, `${screen}: ${name}`).not.toContain('Level 1');
        expect(said, `${screen}: ${name}`).not.toMatch(/no wording|No reason was recorded/);
      }
    }
  });

  it('says every other ingest code in Level 1 words, and archive_too_large past its own list as before', () => {
    for (const { screen, say } of SCREENS) {
      for (const { name, code } of INGEST_REFUSALS) {
        if (name in OWN) continue;
        expect(say(rejection(code)), `${screen}: ${name}`).toBe(LEVEL_1[name]);
      }
      expect(say(rejection({ code: 'archive_too_large', declared: 4097, limit: 67_108_864 }))).toBe(
        'This file unpacks to more than its own contents list says, so it was not opened.',
      );
    }
  });

  it("keeps every sentence Level 1's screens give these codes", () => {
    expect(Object.keys(LEVEL_1).sort()).toEqual(INGEST_REFUSALS.map((r) => r.name).sort());
    for (const { name, code } of INGEST_REFUSALS) {
      expect(sentenceFor(code as never), name).toBe(LEVEL_1[name]);
      // Level 1's open refuses a data file as data_refused, its reason the
      // ingest's code (stats_l1_commands.rs's open), and L1Flow words it
      // through l1Wording's refusalSentence.
      expect(l1RefusalSentence(rejection({ code: 'data_refused', reason: code })), name).toBe(LEVEL_1[name]);
    }
  });
});
