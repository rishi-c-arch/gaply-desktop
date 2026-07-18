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
  /** Set 2b-iii: the outcome of the last online verify for an imported entry
   *  that CrossRef could NOT confirm — 'not_found' (the DOI doesn't resolve) vs
   *  'check_failed' (a network error; retriable). Absent = never attempted, or
   *  verified (verification adds CrossRef provenance instead). */
  verifyOutcome?: 'not_found' | 'check_failed';
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
  ok: 'complete',
  orphan: 'orphan (not cited in text)',
  unused: 'unused',
  malformed: 'malformed metadata',
  retracted: 'RETRACTED',
};

/* ======================= Axis B — VERIFICATION ==========================
 * Completeness (computeStatus, above) and verification are DIFFERENT facts and
 * must never share a word. computeStatus answers "is this entry well-formed?";
 * this axis answers "was it confirmed to exist against an external authority
 * (CrossRef)?". A complete-but-unchecked hand-typed entry is 'complete' AND
 * 'unverified' — never 'verified'.
 *
 * Source-agnostic ON PURPOSE: applies to manual / extracted / imported / doi
 * entries alike (the old imported-only check let a hand-typed entry read
 * "verified").
 *
 * SESSION-ONLY (Scope A): the signal lives in `provenance` / `verifyOutcome`,
 * which are NOT persisted yet (see localLibrary.storedToCitation, which rebuilds
 * every reloaded row as source:'manual', provenance:undefined). So after a
 * reload a previously-verified entry honestly reverts to 'unverified' until
 * re-checked. Under-claiming verification is safe; over-claiming is not.
 * Scope B (REQUIRED follow-up) persists verified_at / verify_provenance /
 * verify_outcome — and, its highest-priority element, `retracted` — so neither
 * verification NOR retraction state vanishes on reload. */
export type VerificationState = 'verified' | 'unverified' | 'not_found' | 'check_failed';

/** An entry is 'verified' ONLY when its provenance shows a CrossRef confirmation
 *  (the online verify pass or add-by-DOI). Keyed on 'crossref' to keep the
 *  "· CrossRef" label truthful — an OpenAlex-only match stays the safe
 *  'unverified' rather than claiming a CrossRef confirmation it doesn't have. */
export function verificationState(c: Citation): VerificationState {
  const crossRefConfirmed = (c.provenance ?? []).some((p) => p.toLowerCase().includes('crossref'));
  if (crossRefConfirmed) return 'verified';
  if (c.verifyOutcome === 'not_found') return 'not_found';
  if (c.verifyOutcome === 'check_failed') return 'check_failed';
  return 'unverified';
}

export const VERIFY_LABEL: Record<VerificationState, string> = {
  verified: 'verified · CrossRef',
  unverified: 'not verified online',
  not_found: 'not found on CrossRef',
  check_failed: 'check failed — try again',
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
