// Gaply — what the SPSS reader noted about a .sav file, as the screens say it
// (Research OS, Phase 8I, decision 365). Rust maps each of the reader's
// findings to a closed code with numbers and keeps nothing of its text; each
// code's sentence is here, word for word, and says what the file declares
// and what Gaply read, never which is right.
import type { SavFinding } from './describeTypes';

const n = (f: SavFinding, key: keyof SavFinding['params']): string => String(f.params[key] ?? '?');

const WORDS: Record<SavFinding['code'], (f: SavFinding) => string> = {
  'sav.header_rows_mismatch': (f) =>
    `The file's header says it holds ${n(f, 'declared')} cases, but ${n(f, 'read')} were read. Gaply uses the ${n(f, 'read')} it read.`,
  'sav.case_size_mismatch': (f) =>
    `The file's header gives ${n(f, 'declared')} values per case, but its variables define ${n(f, 'used')}. Gaply reads ${n(f, 'used')}.`,
  'sav.case_size_missing': (f) =>
    `The file's header does not give its values per case. Gaply reads the ${n(f, 'used')} its variables define.`,
  'sav.dates_out_of_range': (f) =>
    `${n(f, 'count')} date or time values were outside the years 1 to 9999, and were read as missing.`,
  'sav.type_continuation': (f) =>
    `The file's variable record ${n(f, 'record')} is marked as part of the variable before it. Gaply reads it that way.`,
  'sav.compression_bias': (f) =>
    `The file stores its compressed whole numbers with an unusual offset (${n(f, 'bias')}). Gaply reads them with that offset, as SPSS would.`,
  'sav.very_long_string_ignored': (f) =>
    `${n(f, 'count')} long text variables declare a length the file cannot hold. Gaply reads each at the length of its own record, at most 255 bytes, so longer text in them may be cut.`,
  'sav.unrecognised_finding': (f) =>
    `The SPSS reader noted ${n(f, 'count')} other things about this file that Gaply cannot describe yet.`,
};

/** A finding's sentence. A code this screen does not know is said as one
 *  thing the reader noted that Gaply cannot describe. */
export function savFindingSentence(f: SavFinding): string {
  const words = WORDS[f.code];
  return words ? words(f) : WORDS['sav.unrecognised_finding']({ code: 'sav.unrecognised_finding', params: { count: 1 } });
}
