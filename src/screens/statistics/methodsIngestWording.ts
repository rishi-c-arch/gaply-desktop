// Gaply — the file ingest's refusals as the methods on the user's data say
// them (Research OS, Phase 8B, decision 243). The descriptive summary and the
// t-test read a file through the same ingest as Level 1, and get its codes;
// four of Level 1's sentences name Level 1, so these screens have their own
// for those four. Every other code, and archive_too_large's case of a file
// that unpacks past its own contents list, is said in Level 1's words
// (l1Wording's sentenceFor), with the same numbers and the same megabytes.
import { megabytes, nested, num, sentenceFor } from '../statsverifier/l1/l1Wording';
import type { WireCode } from '../statsverifier/statsL1Types';

type Words = (c: WireCode) => string;

const WORDS: Record<string, Words> = {
  file_too_large: (c) =>
    `The file has more than ${num(c, 'limit_rows') ?? '?'} rows. Gaply never computes on part of a file, so it was not opened.`,
  too_wide: (c) =>
    `The file has ${num(c, 'columns') ?? '?'} columns, more than the ${num(c, 'limit') ?? '?'} Gaply reads from one file, so it was not opened.`,
  archive_too_large: (c) => {
    const declared = num(c, 'declared');
    const limit = num(c, 'limit');
    if (declared !== undefined && limit !== undefined && declared <= limit) return sentenceFor(c);
    return `This file would take at least ${megabytes(declared)} once unpacked, more than the ${megabytes(limit)} Gaply opens, so it was not opened.`;
  },
  xls_not_readable: () =>
    'This .xls file is in a format Gaply cannot read, such as an older Excel version. Save it as .xlsx and open that.',
  // too_wide reaches these screens only inside dataset (codes.rs's
  // l1_ingest_refusal), so the nested refusal is said through this table too.
  dataset: (c) => methodsIngestSentence(nested(c, 'refusal')),
};

/** The sentence for an ingest refusal on the summary's and the t-test's
 *  screens: this table's, else Level 1's. */
export function methodsIngestSentence(c: WireCode | undefined): string {
  const f = c ? WORDS[c.code] : undefined;
  return c && f ? f(c) : sentenceFor(c);
}
