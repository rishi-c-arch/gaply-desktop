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
  /** Scope B: epoch (ms) of the CrossRef confirmation, persisted so a verified
   *  entry can show when it was verified. Absent = never verified online. */
  verifiedAt?: number | null;
  /** Axis C: the outcome of the last RETRACTION check — 'clear' (a registry
   *  answered and said no) vs 'check_failed' (we tried and couldn't reach it).
   *  ABSENT MEANS NEVER CHECKED, which is deliberately a different fact from
   *  'clear'. See the Axis C block below for why that distinction is the whole
   *  point. Never write 'clear' from a default or a falsy coercion. */
  retractionOutcome?: 'clear' | 'check_failed';
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

/* ======================= Axis C — RETRACTION ============================
 * THE TYPED ABSENCE. This axis exists because the code had no way to say "we
 * did not check", so it said "clear" instead — and three separate defects fell
 * out of that one gap:
 *
 *   • a DOI-less entry was skipped by the sweep and rendered identically to a
 *     checked-and-clean one;
 *   • `Boolean(v.retraction?.retracted || …)` turned a NULL retraction block
 *     (Retraction Watch unreachable) into a confident `retracted: false`;
 *   • one rejected lookup in a Promise.all discarded EVERY result, after which
 *     the sidebar read "Retracted Items (0)" over a library holding a
 *     retracted paper.
 *
 * All three are the same bug: unchecked collapsed into clean. A researcher can
 * submit a paper citing a retracted study because Gaply showed no badge, so
 * this axis is deliberately pessimistic — anything short of a registry actually
 * answering "not retracted" reads as NOT CHECKED.
 *
 * Completeness (computeStatus) and verification (Axis B) are separate facts and
 * still must not share a word with this one.
 *
 * DURABILITY: `retracted` IS persisted (localLibrary Scope B), so a confirmed
 * retraction survives a reload. `retractionOutcome` is NOT persisted yet — the
 * store's column set lives in gaply_core and is a separate workstream — so
 * after a reload a checked-and-clear entry honestly reverts to 'unchecked'
 * until re-swept. That is the safe direction, and the same Scope-A/Scope-B
 * staging Axis B went through: under-claiming is safe, over-claiming is not.
 *
 * THAT REVERT IS EXPECTED, NOT A BUG. It is a KNOWN, STAGED gap with a known
 * completion step: add a `retraction_outcome` column beside the existing Scope
 * B ones in gaply_core::citation_library, thread it through
 * citation_lib_upsert / StoredReference / storedToCitation, and the axis is
 * complete. Do that when the gaply_core workstream is next open — it is
 * deliberately not done from here. Until then, do NOT "fix" the revert by
 * defaulting a reloaded row to 'clear'; that reinstates the exact defect this
 * axis exists to remove. */
export type RetractionState = 'retracted' | 'clear' | 'unchecked' | 'check_failed';

/** An entry is 'clear' ONLY when a retraction registry actually answered "no".
 *  Absence of an outcome is 'unchecked' — never 'clear'. */
export function retractionState(c: Citation): RetractionState {
  if (c.retracted) return 'retracted';
  if (c.retractionOutcome === 'check_failed') return 'check_failed';
  if (c.retractionOutcome === 'clear') return 'clear';
  return 'unchecked';
}

/** True when this entry's retraction status has NOT been established — either
 *  never attempted or attempted and failed. Both mean the same thing to a
 *  researcher: you cannot rely on the absence of a badge. */
export const retractionUnsettled = (c: Citation): boolean => {
  const s = retractionState(c);
  return s === 'unchecked' || s === 'check_failed';
};

export const RETRACTION_LABEL: Record<RetractionState, string> = {
  retracted: 'RETRACTED',
  clear: 'no retraction found',
  unchecked: 'not checked for retraction',
  check_failed: 'retraction check failed — try again',
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
