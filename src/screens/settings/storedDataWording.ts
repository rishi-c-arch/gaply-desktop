// Gaply — the Stored data section's words (Research OS, Phase 5D-1a;
// decision 207). Each category: its name, one plain sentence of what it is
// and which feature made it, and how its count reads. Sizes read as decisions
// 163 and 174 word them, through projectWording's mb(). The sentences are
// approved by decision 208, with category 9 named "Unused note images".
// Phase 5D-1b adds the deletes' words (decision 215) and the note under the
// section (decision 209).

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
  // Decision 209: always under the section, so it never reads as everything
  // Gaply keeps.
  note:
    'Some of what Gaply keeps is not listed here yet: the files you linked to citations and the papers Gaply fetched for citation checks, with their text, and the evidence cards of citation checks.',
};

/* ---- the deletes (Phase 5D-1b; decision 215) -------------------------------- */

/** What stops when each category is deleted, shown in its dialog. */
export const WHAT_STOPS: Record<StoredCategoryId, string> = {
  manuscript_analyses: 'Nothing on screen reads these now. Cached reports of these manuscripts are listed and deleted on their own.',
  gap_finder_papers: 'The Research Gap Finder, the full analysis and search will no longer find these papers. You can add them again.',
  thesis_audits: 'Past audits will no longer be listed, resumed or exported. Their evidence cards and fetched sources stay, as the note below says.',
  cached_reports: 'To see one of these reports again, run it again.',
  expired_cache: 'Nothing reads these any more.',
  comparison_log: 'Nothing in the app reads it. A new log starts with the next PublishReady run.',
  open_access_papers:
    "Gaply's viewer will no longer open these copies. Their text, already read for citation checks, stays, as the note below says.",
  temporary_exports: 'Nothing reads these any more.',
  orphan_note_images: 'No note uses these images. An image changed in the last 10 minutes is kept, in case its note is not saved yet.',
};

/** The five categories that live in Gaply's database: their delete is
 *  followed by a compaction, and only they can say it did not finish. */
export const DATABASE_CATEGORIES: readonly StoredCategoryId[] = [
  'manuscript_analyses',
  'gap_finder_papers',
  'thesis_audits',
  'cached_reports',
  'expired_cache',
];

export const DELETION_TEXT = {
  open: 'Delete…',
  // Decision 219: a leading "The" reads lowercase inside the sentence.
  title: (name: string) => `Delete ${name.replace(/^The /, 'the ')}?`,
  backups:
    "Deleting removes these from this app's folder. It cannot remove copies in Time Machine or other backups, in file-system snapshots, or anything you exported. The disk may keep traces until the space is reused.",
  cancel: 'Cancel',
  close: 'Close',
  confirm: 'Delete',
  deleted: (count: string) => `Deleted: ${count}.`,
  // Decision 224: the not-done line keeps the count (deletedLine prepends it),
  // then this sentence.
  notCompacted:
    'Gaply could not finish clearing the deleted text from its database files just now. It will try again the next time it starts.',
  // F9 (decision 226): the count prepare_delete returned differs from what
  // the dialog showed, so nothing was deleted; it now shows the new count.
  changed: 'These have changed since they were shown. Press Delete again to delete what is shown now.',
  // Drafted in 5D-1b, beyond decision 215's words, for approval:
  deleting: 'Deleting…',
  partlyRemoved: 'Gaply could not delete the rest just now. The counts show what is left.',
  failed: 'Gaply could not finish this delete. The counts show what is left.',
};

/** Each refusal's sentence, by its closed code (decision 214's are drafted in
 *  5D-1b, for approval). A code not listed here reads as DELETION_TEXT.failed. */
const NOT_CONFIRMED = 'This delete was not confirmed in time. Press Delete again.';
export const REFUSAL_WORDS: Record<string, string> = {
  'stored_data.analysis_running': 'A manuscript analysis is running. Delete these when it has finished.',
  'stored_data.gap_finder_running': 'The Research Gap Finder is reading papers. Delete these when it has finished.',
  'stored_data.papers_in_use': 'A citation check uses some of these papers, so none were deleted.',
  'stored_data.job_running': 'A thesis audit is running or paused. Delete these when it has finished or been cancelled.',
  'stored_data.fetch_running': 'Gaply is downloading open-access papers. Delete these when it has finished.',
  'stored_data.confirmation_missing': NOT_CONFIRMED,
  'stored_data.confirmation_unknown': NOT_CONFIRMED,
  'stored_data.confirmation_used': NOT_CONFIRMED,
  'stored_data.confirmation_replaced': NOT_CONFIRMED,
  'stored_data.confirmation_expired': NOT_CONFIRMED,
  'stored_data.confirmation_other_category': NOT_CONFIRMED,
};

/** A refusal's closed code, read from the error's message as the JSON Rust
 *  sends; never returns or keeps the message itself. */
export function refusalCode(e: unknown): string | null {
  const message =
    typeof e === 'string'
      ? e
      : e && typeof e === 'object' && typeof (e as { message?: unknown }).message === 'string'
        ? (e as { message: string }).message
        : null;
  if (message === null) return null;
  try {
    const v = JSON.parse(message) as { code?: unknown } | null;
    return v && typeof v.code === 'string' && v.code in REFUSAL_WORDS ? v.code : null;
  } catch {
    return null;
  }
}

/** One sentence for a failed prepare or delete: its refusal's, or the
 *  failure's. */
export function deleteSentence(e: unknown): string {
  const code = refusalCode(e);
  return code ? REFUSAL_WORDS[code] : DELETION_TEXT.failed;
}

/** "1 analysis", "1,204 rows". */
export function countWords(n: number, w: Pick<StoredCategoryWords, 'one' | 'many'>): string {
  return `${n.toLocaleString('en-US')} ${n === 1 ? w.one : w.many}`;
}
