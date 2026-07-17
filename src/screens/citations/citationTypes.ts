// Gaply — Citation Manager types. Citation METADATA only (CSL-JSON + a few
// index fields). The manuscript itself is NEVER stored here or synced.

/** CSL-JSON subset we render + persist (in citation_library.csl_json). */
export interface CslItem {
  id: string;
  type: string; // "article-journal", ...
  title: string;
  author: Array<{ family: string; given?: string }>;
  issued?: { year?: number };
  DOI?: string;
  URL?: string;
  containerTitle?: string; // journal
  volume?: string;
  issue?: string;
  page?: string;
}

export type CitationStatus = 'ok' | 'orphan' | 'unused' | 'malformed' | 'retracted';

export type CitationSource = 'extracted' | 'doi' | 'manual' | 'imported';

export interface Citation {
  /** Local id (uuid-ish); mirrors citation_library.id once persisted. */
  id: string;
  csl: CslItem;
  doi: string | null;
  retracted: boolean;
  /** True when the extraction found no in-text use (orphan/unused). */
  cited?: boolean;
  source: CitationSource;
  collectionId?: string;
  /** Provenance strings from refverify (sources consulted). */
  provenance?: string[];
  /** Retraction notice URL, when retracted. */
  noticeUrl?: string;
  /** Local-first (Set 4): tags on this reference. */
  tags?: string[];
  /** Local-first (Set 4): honest sync marker (local sqlite is the truth). */
  syncStatus?: 'local_only' | 'pending' | 'synced';
}

/** Status precedence: retracted 🔴 > malformed > orphan/unused ⚠ > ok. */
export function computeStatus(c: Citation): CitationStatus {
  if (c.retracted) return 'retracted';
  const m = c.csl;
  if (!m.title || m.author.length === 0 || !m.issued?.year) return 'malformed';
  if (c.cited === false) return 'orphan';
  return 'ok';
}

export const STATUS_ICON: Record<CitationStatus, string> = {
  ok: '•',
  orphan: '⚠',
  unused: '⚠',
  malformed: '⚠',
  retracted: '🔴',
};

export const STATUS_LABEL: Record<CitationStatus, string> = {
  ok: 'verified',
  orphan: 'orphan (not cited in text)',
  unused: 'unused',
  malformed: 'malformed metadata',
  retracted: 'RETRACTED',
};

/** Map a Citation → the citation_library row shape (metadata only). */
export function citationToRow(userId: string, c: Citation) {
  return {
    user_id: userId,
    doi: c.doi,
    title: c.csl.title || null,
    authors: c.csl.author.map((a) => [a.family, a.given].filter(Boolean).join(', ')).join('; ') || null,
    year: c.csl.issued?.year ?? null,
    journal: c.csl.containerTitle ?? null,
    csl_json: c.csl as unknown as Record<string, unknown>,
    retracted_flag: c.retracted,
  };
}

/** Parse a DOI out of a raw DOI or URL string. */
export function parseDoi(input: string): string | null {
  const s = input.trim();
  // DOIs are 10.<registrant>/<suffix>; be lenient on the registrant length.
  const m = s.match(/10\.\d{1,9}\/[^\s"'<>]+/i);
  return m ? m[0].replace(/[).,;]+$/, '') : null;
}
