// Gaply — the Stored data section's words (Research OS, Phase 5D-1a;
// decision 207). Each category: its name, one plain sentence of what it is
// and which feature made it, and how its count reads. Sizes read as decisions
// 163 and 174 word them, through projectWording's mb(). The sentences are
// approved by decision 208, with category 9 named "Unused note images".

export type StoredCategoryId =
  | 'manuscript_analyses'
  | 'gap_finder_papers'
  | 'thesis_audits'
  | 'cached_reports'
  | 'expired_cache'
  | 'comparison_log'
  | 'open_access_papers'
  | 'temporary_exports'
  | 'orphan_note_images';

export interface StoredCategoryWords {
  id: StoredCategoryId;
  name: string;
  sentence: string;
  /** The count's noun, singular and plural. */
  one: string;
  many: string;
}

/** The nine categories, in the order Rust counts and the section shows them. */
export const STORED_CATEGORIES: readonly StoredCategoryWords[] = [
  {
    id: 'manuscript_analyses',
    name: 'Manuscript analyses',
    sentence:
      'The text of each manuscript Gaply analysed, as Gaply read it, with what it found. Made by the full analysis, PublishReady and the Stats Check.',
    one: 'analysis',
    many: 'analyses',
  },
  {
    id: 'gap_finder_papers',
    name: 'Gap Finder papers',
    sentence: 'The text of the papers you gave the Research Gap Finder, kept so they can be searched. Made by the Research Gap Finder.',
    one: 'paper',
    many: 'papers',
  },
  {
    id: 'thesis_audits',
    name: 'Thesis audits',
    sentence:
      "Each citation audit of a manuscript: the sentences it checked, their results and the manuscript's reference list. Made by the thesis citation audit.",
    one: 'audit',
    many: 'audits',
  },
  {
    id: 'cached_reports',
    name: 'Cached reports',
    sentence:
      'Copies of reports finished in the last 30 days, kept so a report opens again without a new run. Made by the full analysis and PublishReady.',
    one: 'report',
    many: 'reports',
  },
  {
    id: 'expired_cache',
    name: 'Expired cache rows',
    sentence:
      'Saved copies past their time, which nothing reads any more: older reports, and older answers from scholarly databases and journal lookups.',
    one: 'row',
    many: 'rows',
  },
  {
    id: 'comparison_log',
    name: 'The comparison log',
    sentence:
      "One line for each PublishReady run, kept to check how Gaply's review behaves. It holds a fingerprint of each manuscript, not its text.",
    one: 'file',
    many: 'files',
  },
  // Decision 208: only the features traced to the writes in oa_papers/. The
  // Citation Manager (CitationManagerPage.tsx, DocumentRow.tsx) and the thesis
  // citation audit (ThesisAuditScreen.tsx, its staged references included) call
  // aiBridge.fetchOpenAccess -> citation_fetch_oa -> oa_fetch::fetch_one, which
  // writes the PDF or the abstract. The vitest pins this list to the callers.
  {
    id: 'open_access_papers',
    name: 'Downloaded open-access papers',
    sentence:
      'Free copies of cited papers, or of their abstracts, that Gaply downloaded to check your citations against. Made by the Citation Manager and the thesis citation audit.',
    one: 'file',
    many: 'files',
  },
  {
    id: 'temporary_exports',
    name: 'Temporary exports',
    sentence:
      "The last AI Check report and the last PublishReady PDF that Gaply opened for you, kept in this computer's temporary folder until the next export replaces them.",
    one: 'file',
    many: 'files',
  },
  {
    id: 'orphan_note_images',
    name: 'Unused note images',
    sentence: 'Images pasted into notes that no note uses any more. Made by the Note Creator.',
    one: 'image',
    many: 'images',
  },
];

export const STORED_DATA_TEXT = {
  heading: 'Stored data',
  reading: 'Reading what is stored…',
  failed: 'Gaply could not read what it keeps on this computer.',
};

/** "1 analysis", "1,204 rows". */
export function countWords(n: number, w: Pick<StoredCategoryWords, 'one' | 'many'>): string {
  return `${n.toLocaleString('en-US')} ${n === 1 ? w.one : w.many}`;
}
