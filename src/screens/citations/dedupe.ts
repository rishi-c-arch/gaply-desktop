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

/** The DOI of a citation, checking the top-level field then the CSL body. */
function doiOf(c: Citation): string | null {
  return normalizeDoi(c.doi ?? c.csl?.DOI ?? null);
}
function titleOf(c: Citation): string {
  return normalizeTitle(c.csl?.title ?? null);
}
function yearOf(c: Citation): number | null {
  const y = c.csl?.issued?.year;
  return typeof y === 'number' ? y : null;
}

export type DuplicateTier = 'doi' | 'title-year';
export interface DuplicateMatch {
  tier: DuplicateTier;
  match: Citation;
}

/** Find the first existing citation the candidate duplicates, or null.
 *  Tier 1 (DOI exact) is checked first and wins over Tier 2 (title+year) —
 *  a DOI match is certain even when titles were entered differently. */
export function findDuplicate(candidate: Citation, existing: Citation[]): DuplicateMatch | null {
  const cDoi = doiOf(candidate);
  if (cDoi) {
    const m = existing.find((e) => doiOf(e) === cDoi);
    if (m) return { tier: 'doi', match: m };
  }
  const cTitle = titleOf(candidate);
  const cYear = yearOf(candidate);
  // Tier 2 requires BOTH a title and a year — a title alone is too weak, and a
  // year alone is meaningless. (Two untitled manual stubs never collide.)
  if (cTitle && cYear != null) {
    const m = existing.find((e) => titleOf(e) === cTitle && yearOf(e) === cYear);
    if (m) return { tier: 'title-year', match: m };
  }
  return null;
}
