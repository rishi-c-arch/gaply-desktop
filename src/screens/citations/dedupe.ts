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
import { Citation, CslItem } from './citationTypes';

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

/* ---- Enrichment (Set 2b-iv): fill-only merge for DOI-exact duplicates --- *
 * A Tier-1 DOI match is a CERTAIN same-paper identity, so when the incoming
 * entry carries fields the existing one lacks, we can safely OFFER to fill them.
 * The rule is FILL-ONLY: enrichment fills fields the existing entry is MISSING
 * and NEVER overwrites a field it already has. That exclusion is the whole
 * safety story — a hand-edited title/journal that differs from the import is
 * left exactly as-is, because it is never in the delta. Pure + deterministic;
 * the page decides WHEN to apply (never silent, never automatic). */

/** The enrichable CSL fields, in a stable order (deterministic delta + UI). */
export type EnrichField = 'title' | 'author' | 'issued' | 'containerTitle' | 'volume' | 'issue' | 'page' | 'URL';
const ENRICH_FIELDS: EnrichField[] = ['title', 'author', 'issued', 'containerTitle', 'volume', 'issue', 'page', 'URL'];

/** Human-readable names for the enrichment offer UI ("missing: journal, pages"). */
export const ENRICH_FIELD_LABEL: Record<EnrichField, string> = {
  title: 'title',
  author: 'authors',
  issued: 'year',
  containerTitle: 'journal',
  volume: 'volume',
  issue: 'issue',
  page: 'pages',
  URL: 'URL',
};

/** Is an enrichable field empty (missing / blank) on this citation? */
function isFieldEmpty(c: Citation, f: EnrichField): boolean {
  const m = c.csl;
  switch (f) {
    case 'author':
      return (m.author?.length ?? 0) === 0;
    case 'issued':
      return m.issued?.year == null;
    case 'title':
      return !m.title;
    default:
      return !m[f]; // containerTitle | volume | issue | page | URL
  }
}

/** Fields the EXISTING entry lacks but the INCOMING one has. Never includes a
 *  field the existing already holds — that exclusion is what keeps fill-only
 *  enrichment from clobbering hand-edits. */
export function fieldDelta(existing: Citation, incoming: Citation): EnrichField[] {
  return ENRICH_FIELDS.filter((f) => isFieldEmpty(existing, f) && !isFieldEmpty(incoming, f));
}

/** Fill-only merge: a NEW citation = existing with ONLY its missing fields
 *  filled from incoming. Keeps existing's id / doi / source / tags / retracted;
 *  touches ONLY the csl gaps in fieldDelta. A populated field is NEVER changed. */
export function applyEnrichment(existing: Citation, incoming: Citation): Citation {
  const delta = fieldDelta(existing, incoming);
  if (delta.length === 0) return existing;
  const csl: CslItem = { ...existing.csl };
  for (const f of delta) {
    switch (f) {
      case 'author':
        csl.author = incoming.csl.author;
        break;
      case 'issued':
        csl.issued = { year: incoming.csl.issued?.year };
        break;
      case 'title':
        if (incoming.csl.title) csl.title = incoming.csl.title;
        break;
      case 'containerTitle':
        csl.containerTitle = incoming.csl.containerTitle;
        break;
      case 'volume':
        csl.volume = incoming.csl.volume;
        break;
      case 'issue':
        csl.issue = incoming.csl.issue;
        break;
      case 'page':
        csl.page = incoming.csl.page;
        break;
      case 'URL':
        csl.URL = incoming.csl.URL;
        break;
    }
  }
  return { ...existing, csl };
}
