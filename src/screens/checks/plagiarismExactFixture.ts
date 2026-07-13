// Gaply — exact-match plagiarism test fixtures (Set 5). Wire-shaped
// ExactPlagiarismReport + LibraryPaper[] used by the vitest suites only.
// Disclosure text mirrors the Rust core's named-scope + Turnitin notice so the
// "verbatim, un-strippable" assertions are honest.
import { ExactPlagiarismReport, LibraryPaper } from './agentTypes';

const RECYCLED =
  'The mitochondrial membrane potential collapses during the early phase of apoptosis and ' +
  'triggers the release of cytochrome c into the cytosol.';

const NOT_TURNITIN =
  'This is NOT a comprehensive plagiarism check and does NOT replace Turnitin or iThenticate. ' +
  'It only compares against the documents listed here, and cannot detect matches against ' +
  'journals, the web, subscription databases, or any work not provided to it. The absence of ' +
  'matches here does NOT mean the text is original.';

export const PLAG_EXACT_FIXTURE: ExactPlagiarismReport = {
  shingle_size: 5,
  window_size: 4,
  library_matches: [
    {
      source: { start_char: 40, end_char: 40 + RECYCLED.length, text: RECYCLED },
      matched: { start_char: 12, end_char: 12 + RECYCLED.length, text: RECYCLED },
      similarity: 1.0,
      word_count: 20,
      match_kind: 'library_match',
      source_ref: 'Prior study (2019)',
    },
  ],
  self_matches: [
    {
      source: { start_char: 5, end_char: 5 + RECYCLED.length, text: RECYCLED },
      matched: { start_char: 600, end_char: 600 + RECYCLED.length, text: RECYCLED },
      similarity: 1.0,
      word_count: 20,
      match_kind: 'self_repeat',
      source_ref: 'this document',
    },
  ],
  compared_against: ['Prior study (2019)', 'Methods handbook'],
  total_source_words: 1200,
  duplication_ratio: 0.18,
  stats: {
    source_fingerprints: 240,
    comparison_fingerprints: 500,
    candidate_seeds: 12,
    library_papers_examined: 1,
  },
  disclosure:
    'Checked this document against itself and your 2 paper(s) in your library: Prior study (2019); ' +
    'Methods handbook. ' + NOT_TURNITIN,
};

/** An all-clear report (no matches) — still carries the disclosure. */
export const PLAG_EXACT_CLEAR: ExactPlagiarismReport = {
  ...PLAG_EXACT_FIXTURE,
  library_matches: [],
  self_matches: [],
  duplication_ratio: 0,
};

export const LIBRARY_FIXTURE: LibraryPaper[] = [
  { id: 1, title: 'Prior study (2019)', added_at: 1_700_000_000, source_label: '/papers/prior.pdf' },
  { id: 2, title: 'Methods handbook', added_at: 1_700_100_000, source_label: '/papers/methods.pdf' },
];
