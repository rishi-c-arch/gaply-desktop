// Gaply — what the SPSS reader noted about a .sav file, as the screens say it
// (Research OS, Phase 8I, decision 365). Rust maps each of the reader's
// findings to a closed code with numbers and keeps nothing of its text; each
// code's sentence is here, word for word, and says what the file declares
// and what Gaply read, never which is right. A count of exactly 1 takes the
// singular, every other number the plural (8I-2, decision 376).
import type { SavFinding } from './describeTypes';

const n = (f: SavFinding, key: keyof SavFinding['params']): string => String(f.params[key] ?? '?');

/** True when the parameter is exactly 1, so its sentence takes the singular. */
const one = (f: SavFinding, key: keyof SavFinding['params']): boolean => f.params[key] === 1;

const WORDS: Record<SavFinding['code'], (f: SavFinding) => string> = {
  'sav.header_rows_mismatch': (f) =>
    `The file's header says it holds ${n(f, 'declared')} ${one(f, 'declared') ? 'case' : 'cases'}, but ${n(f, 'read')} ${one(f, 'read') ? 'was' : 'were'} read. Gaply uses the ${n(f, 'read')} it read.`,
  'sav.case_size_mismatch': (f) =>
    `The file's header gives ${n(f, 'declared')} ${one(f, 'declared') ? 'value' : 'values'} per case, but its variables define ${n(f, 'used')}. Gaply reads ${n(f, 'used')}.`,
  'sav.case_size_missing': (f) =>
    `The file's header does not give its values per case. Gaply reads the ${n(f, 'used')} its variables define.`,
  'sav.dates_out_of_range': (f) =>
    one(f, 'count')
      ? '1 date or time value was outside the years 1 to 9999, and was read as missing.'
      : `${n(f, 'count')} date or time values were outside the years 1 to 9999, and were read as missing.`,
  'sav.type_continuation': (f) =>
    `The file's variable record ${n(f, 'record')} is marked as part of the variable before it. Gaply reads it that way.`,
  'sav.compression_bias': (f) =>
    `The file's header gives an offset of ${n(f, 'bias')} for its compressed whole numbers, not the usual 100. Gaply reads them with that offset; SPSS reads such a file assuming 100, so these numbers may differ from SPSS's.`,
  'sav.zsav_trailer_bias': (f) =>
    `The file's compressed-data trailer gives an offset of ${n(f, 'bias')}, not the usual 100. Gaply reads the numbers with the header's offset, so this changes nothing Gaply reads.`,
  'sav.very_long_string_ignored': (f) =>
    one(f, 'count')
      ? '1 long text variable declares a length the file cannot hold. Gaply reads it at the length of its own record, at most 255 bytes, so longer text in it may be cut.'
      : `${n(f, 'count')} long text variables declare a length the file cannot hold. Gaply reads each at the length of its own record, at most 255 bytes, so longer text in them may be cut.`,
  'sav.unrecognised_finding': (f) =>
    one(f, 'count')
      ? 'The SPSS reader noted 1 other thing about this file that Gaply cannot describe yet.'
      : `The SPSS reader noted ${n(f, 'count')} other things about this file that Gaply cannot describe yet.`,
};

/** A finding's sentence. A code this screen does not know is said as one
 *  thing the reader noted that Gaply cannot describe. */
export function savFindingSentence(f: SavFinding): string {
  const words = WORDS[f.code];
  return words ? words(f) : WORDS['sav.unrecognised_finding']({ code: 'sav.unrecognised_finding', params: { count: 1 } });
}
