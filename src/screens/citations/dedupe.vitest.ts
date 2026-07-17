// Citation Manager dedupe (Set 2a) — the pure matcher. No React, no network.
import { describe, expect, it } from 'vitest';
import { normalizeDoi, normalizeTitle, findDuplicate } from './dedupe';
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
