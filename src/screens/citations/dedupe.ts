// Gaply — Citation Manager dedupe (Set 2a). A pure, deterministic matcher used
// by every add-path so the library doesn't accumulate duplicates.
//
// TWO TIERS, by confidence:
//   Tier 1 — normalized DOI exact match. UNAMBIGUOUS → callers auto-skip.
//   Tier 2 — normalized title + same year (no fuzzy distance yet — academia is
//            full of legitimately near-identical titles across years/authors, so
//            a guessed similarity threshold would cost the user a decision on
//            every false flag). UNCERTAIN → a deliberate single add keeps both;
//            batch import (Set 2b) surfaces these for the user to decide.
//
// SCOPE / HONEST LIMITATION: this protects the paths that CALL it (the four
// add-ways in CitationManagerPage). It is a UI-layer property, NOT a DB
// invariant — a future caller invoking `citation_lib_upsert` directly bypasses
// it. The robust long-term guarantee is a DOI-normalized guard in
// `gaply_core::citation_library::upsert` (recorded as optional hardening); until
// then, dedupe lives here and only here.
import { Citation } from './citationTypes';

/** Normalize a DOI for comparison: lowercase, strip the doi.org / dx.doi.org
 *  URL and `doi:` prefixes, a trailing slash, and surrounding whitespace.
 *  Returns null when there is effectively no DOI. */
export function normalizeDoi(doi: string | null | undefined): string | null {
  if (!doi) return null;
  let d = doi.trim().toLowerCase();
  d = d.replace(/^https?:\/\/(dx\.)?doi\.org\//, '');
  d = d.replace(/^doi:\s*/, '');
  d = d.replace(/\/+$/, '');
  d = d.trim();
  return d.length > 0 ? d : null;
}

/** Normalize a title for comparison: lowercase, strip punctuation (unicode-aware),
 *  collapse whitespace. Conservative — exact-equality after normalization only. */
export function normalizeTitle(title: string | null | undefined): string {
  if (!title) return '';
  return title
    .toLowerCase()
    .replace(/[^\p{L}\p{N}\s]/gu, ' ') // punctuation → space (keeps letters/digits)
    .replace(/\s+/g, ' ')
    .trim();
}

/** The Tier-1 key: the normalized DOI (top-level field, else the CSL body), or
 *  null when there's no DOI. Two citations with the same key are DOI-duplicates. */
export function doiKeyOf(c: Citation): string | null {
  return normalizeDoi(c.doi ?? c.csl?.DOI ?? null);
}
/** The Tier-2 key: `normalizedTitle::year`, or null unless BOTH exist — a title
 *  alone is too weak, a year alone meaningless (two untitled stubs never collide). */
export function titleYearKeyOf(c: Citation): string | null {
  const t = normalizeTitle(c.csl?.title ?? null);
  const y = c.csl?.issued?.year;
  return t && typeof y === 'number' ? `${t}::${y}` : null;
}

export type DuplicateTier = 'doi' | 'title-year';
export interface DuplicateMatch {
  tier: DuplicateTier;
  match: Citation;
}

/** Find the first existing citation the candidate duplicates, or null.
 *  Tier 1 (DOI exact) is checked first and wins over Tier 2 (title+year) —
 *  a DOI match is certain even when titles were entered differently.
 *  O(n) scan — used by the single add-paths. For batch import, prefer the
 *  index below (O(1) lookups). */
export function findDuplicate(candidate: Citation, existing: Citation[]): DuplicateMatch | null {
  const cDoi = doiKeyOf(candidate);
  if (cDoi) {
    const m = existing.find((e) => doiKeyOf(e) === cDoi);
    if (m) return { tier: 'doi', match: m };
  }
  const cKey = titleYearKeyOf(candidate);
  if (cKey) {
    const m = existing.find((e) => titleYearKeyOf(e) === cKey);
    if (m) return { tier: 'title-year', match: m };
  }
  return null;
}

/* ---- Index (Set 2b): O(1) dedupe for batch import ----------------------- *
 * findDuplicate is O(n); a 5,000-entry import against a growing accumulator
 * would be O(n²). The index gives the identical semantics via two maps keyed by
 * the SAME key functions, so batch import stays linear. First-writer-wins so the
 * `match` returned is the earliest existing entry. */
export interface DedupeIndex {
  byDoi: Map<string, Citation>;
  byTitleYear: Map<string, Citation>;
}
export function buildDedupeIndex(cites: Citation[]): DedupeIndex {
  const index: DedupeIndex = { byDoi: new Map(), byTitleYear: new Map() };
  for (const c of cites) addToDedupeIndex(index, c);
  return index;
}
export function lookupDuplicate(index: DedupeIndex, candidate: Citation): DuplicateMatch | null {
  const dk = doiKeyOf(candidate);
  if (dk) {
    const m = index.byDoi.get(dk);
    if (m) return { tier: 'doi', match: m };
  }
  const tk = titleYearKeyOf(candidate);
  if (tk) {
    const m = index.byTitleYear.get(tk);
    if (m) return { tier: 'title-year', match: m };
  }
  return null;
}
export function addToDedupeIndex(index: DedupeIndex, c: Citation): void {
  const dk = doiKeyOf(c);
  if (dk && !index.byDoi.has(dk)) index.byDoi.set(dk, c);
  const tk = titleYearKeyOf(c);
  if (tk && !index.byTitleYear.has(tk)) index.byTitleYear.set(tk, c);
}
