// Citation Manager dedupe (Set 2a) — the pure matcher. No React, no network.
import { describe, expect, it } from 'vitest';
import { normalizeDoi, normalizeTitle, findDuplicate, fieldDelta, applyEnrichment } from './dedupe';
import { Citation } from './citationTypes';

const cite = (over: Partial<Citation> & { title?: string; year?: number; doiRaw?: string | null }): Citation => ({
  id: over.id ?? Math.random().toString(36).slice(2),
  csl: {
    id: 's',
    type: 'article-journal',
    title: over.title ?? '',
    author: [],
    ...(over.year != null ? { issued: { year: over.year } } : {}),
    ...(over.doiRaw !== undefined && over.doiRaw !== null ? { DOI: over.doiRaw } : {}),
  },
  doi: over.doiRaw ?? null,
  retracted: false,
  source: 'manual',
});

describe('normalizeDoi', () => {
  it('strips the doi.org / dx.doi.org URL prefixes', () => {
    expect(normalizeDoi('https://doi.org/10.1/ABC')).toBe('10.1/abc');
    expect(normalizeDoi('http://dx.doi.org/10.1/XyZ')).toBe('10.1/xyz');
  });
  it('lowercases and strips the doi: prefix, trailing slash, and whitespace', () => {
    expect(normalizeDoi('DOI: 10.1/AbC')).toBe('10.1/abc');
    expect(normalizeDoi('10.1/abc/')).toBe('10.1/abc');
    expect(normalizeDoi('  10.1/abc  ')).toBe('10.1/abc');
  });
  it('all four surface forms of one DOI normalize identically', () => {
    const forms = ['10.1/abc', 'https://doi.org/10.1/ABC', '10.1/abc/', 'doi:10.1/ABC'];
    const norm = Array.from(new Set(forms.map(normalizeDoi)));
    expect(norm.length).toBe(1);
    expect(norm[0]).toBe('10.1/abc');
  });
  it('returns null for empty / missing', () => {
    expect(normalizeDoi(null)).toBeNull();
    expect(normalizeDoi(undefined)).toBeNull();
    expect(normalizeDoi('   ')).toBeNull();
  });
});

describe('normalizeTitle', () => {
  it('lowercases, strips punctuation, collapses whitespace', () => {
    expect(normalizeTitle('Nano-emulsion: A Review!')).toBe('nano emulsion a review');
    expect(normalizeTitle('  Deep   Learning\tfor  X  ')).toBe('deep learning for x');
  });
  it('is empty for null/blank', () => {
    expect(normalizeTitle(null)).toBe('');
    expect(normalizeTitle('   ')).toBe('');
  });
});

describe('findDuplicate', () => {
  it('Tier 1: same DOI in different surface forms is a duplicate', () => {
    const lib = [cite({ id: 'a', title: 'Some Paper', doiRaw: '10.1/abc' })];
    const cand = cite({ title: 'SOME paper (typed differently)', doiRaw: 'https://doi.org/10.1/ABC' });
    const d = findDuplicate(cand, lib);
    expect(d?.tier).toBe('doi');
    expect(d?.match.id).toBe('a');
  });

  it('Tier 2: same normalized title + same year, no DOI, is a duplicate', () => {
    const lib = [cite({ id: 'b', title: 'Nano-emulsion: A Review', year: 2021 })];
    const cand = cite({ title: 'nano emulsion a review', year: 2021 });
    const d = findDuplicate(cand, lib);
    expect(d?.tier).toBe('title-year');
    expect(d?.match.id).toBe('b');
  });

  it('same title but DIFFERENT years are NOT duplicates — both survive', () => {
    const lib = [cite({ id: 'c', title: 'Nanoemulsion drug delivery: a review', year: 2019 })];
    const cand = cite({ title: 'Nanoemulsion drug delivery: a review', year: 2023 });
    expect(findDuplicate(cand, lib)).toBeNull();
  });

  it('no DOI and no title+year match → not a duplicate (a manual entry still adds)', () => {
    const lib = [cite({ id: 'd', title: 'Existing Paper', year: 2020, doiRaw: '10.1/exist' })];
    const cand = cite({ title: 'Untitled — edit details' }); // no DOI, no year
    expect(findDuplicate(cand, lib)).toBeNull();
  });

  it('Tier 1 wins over Tier 2: a DOI match returns even when titles differ', () => {
    const lib = [cite({ id: 'e', title: 'Original Title', year: 2020, doiRaw: '10.1/z' })];
    const cand = cite({ title: 'Completely Different Title', year: 1999, doiRaw: '10.1/z' });
    expect(findDuplicate(cand, lib)?.tier).toBe('doi');
  });

  it('batch-internal: the same entry appearing twice in one list is caught', () => {
    const entry = cite({ id: 'x', title: 'Repeated', year: 2022, doiRaw: '10.1/rep' });
    // simulate an import accumulator: entry vs a batch that already accepted it
    expect(findDuplicate(entry, [entry])).not.toBeNull();
    expect(findDuplicate(entry, [entry])?.tier).toBe('doi');
  });
});

/* ---- Enrichment (Set 2b-iv): fill-only merge -------------------------- */
describe('fieldDelta + applyEnrichment (fill-only)', () => {
  // builders with explicit csl so we can vary each enrichable field
  const withCsl = (over: Partial<Citation['csl']>, id = 'c'): Citation => ({
    id,
    csl: { id: 's', type: 'article-journal', title: 'Paper', author: [{ family: 'Doe' }], issued: { year: 2020 }, ...over },
    doi: '10.1/same',
    retracted: false,
    source: 'manual',
  });

  it('routing basis: rich incoming on a THIN existing → the missing fields', () => {
    const existing = withCsl({}); // title/author/year only — no journal/vol/issue/page/URL
    const incoming = withCsl({ containerTitle: 'J. Rest', volume: '12', issue: '3', page: '1-9', URL: 'https://x' });
    expect(fieldDelta(existing, incoming)).toEqual(['containerTitle', 'volume', 'issue', 'page', 'URL']);
  });

  it('routing basis: THIN incoming on a rich existing → EMPTY (no-op dup, will be skipped)', () => {
    const existing = withCsl({ containerTitle: 'J. Rest', volume: '12', page: '1-9' });
    const incoming = withCsl({}); // has nothing extra
    expect(fieldDelta(existing, incoming)).toEqual([]);
  });

  it('routing basis: identical entries → EMPTY delta', () => {
    const a = withCsl({ containerTitle: 'J. Rest', page: '1-9' });
    const b = withCsl({ containerTitle: 'J. Rest', page: '1-9' });
    expect(fieldDelta(a, b)).toEqual([]);
  });

  // THE GUARDRAIL — a conflicting field the existing ALREADY has is excluded
  // from the delta, so applyEnrichment leaves the hand-edited value untouched.
  it('GUARDRAIL: a populated field that DIFFERS from the import is NEVER in the delta or overwritten', () => {
    const existing = withCsl({ title: 'My Edited Title', containerTitle: 'My Journal' }); // no page
    const incoming = withCsl({ title: 'Other Title', containerTitle: 'Other Journal', page: '1-9' });

    // delta excludes title + journal (existing has them); includes only the gap: page
    expect(fieldDelta(existing, incoming)).toEqual(['page']);

    const merged = applyEnrichment(existing, incoming);
    expect(merged.csl.title).toBe('My Edited Title'); // hand-edit preserved EXACTLY
    expect(merged.csl.containerTitle).toBe('My Journal'); // populated field NOT clobbered
    expect(merged.csl.page).toBe('1-9'); // only the genuine gap was filled
    // identity preserved
    expect(merged.id).toBe(existing.id);
    expect(merged.doi).toBe(existing.doi);
  });

  it('fills an empty author list / missing year, but never replaces a populated one', () => {
    const existing = withCsl({ author: [], issued: undefined }); // stub: no authors, no year
    const incoming = withCsl({ author: [{ family: 'Roe', given: 'A' }], issued: { year: 2021 } });
    expect(fieldDelta(existing, incoming)).toEqual(['author', 'issued']);
    const merged = applyEnrichment(existing, incoming);
    expect(merged.csl.author).toEqual([{ family: 'Roe', given: 'A' }]);
    expect(merged.csl.issued?.year).toBe(2021);

    // but a populated author list is left alone
    const existing2 = withCsl({ author: [{ family: 'Keep' }] });
    const incoming2 = withCsl({ author: [{ family: 'Other' }] });
    expect(fieldDelta(existing2, incoming2)).toEqual([]);
    expect(applyEnrichment(existing2, incoming2).csl.author).toEqual([{ family: 'Keep' }]);
  });

  it('empty delta is a pure no-op (returns the existing unchanged)', () => {
    const existing = withCsl({ containerTitle: 'J' });
    expect(applyEnrichment(existing, withCsl({ containerTitle: 'J' }))).toBe(existing);
  });
});
