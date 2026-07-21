// Set B SPIKE — the one hard question, locked: numbered-style (IEEE) in-text
// citations must number in DOCUMENT order and produce a MATCHING ordered
// bibliography; a reused citation keeps its first number; author-date (APA)
// works too. This runs headless (reads the real .csl from disk) — the unknown
// was citeproc's cross-document numbering, which is pure logic, not WKWebView.
import { describe, it, expect, beforeAll } from 'vitest';
import { readFileSync } from 'fs';
import { ensureCslEngine, registerStyleXml, registerLocaleXml, renderCitations } from './cslEngine';
import { CslItem } from './citationTypes';

const LIB: CslItem[] = [
  { id: 'A', type: 'article-journal', title: 'Alpha', author: [{ family: 'Vaswani', given: 'A' }], issued: { year: 2017 }, containerTitle: 'NeurIPS' },
  { id: 'B', type: 'article-journal', title: 'Bravo', author: [{ family: 'He', given: 'K' }], issued: { year: 2016 }, containerTitle: 'CVPR' },
  { id: 'C', type: 'article-journal', title: 'Charlie', author: [{ family: 'Devlin', given: 'J' }], issued: { year: 2019 }, containerTitle: 'NAACL' },
];
// Document order: B, then A, then C, then B again (reuse).
const CLUSTERS = [['B'], ['A'], ['C'], ['B']];

beforeAll(async () => {
  await ensureCslEngine();
  await registerLocaleXml('en-US', readFileSync('public/csl/locales/locales-en-US.xml', 'utf8'));
  await registerStyleXml('ieee', readFileSync('public/csl/styles/ieee.csl', 'utf8'));
  await registerStyleXml('apa', readFileSync('public/csl/styles/apa.csl', 'utf8'));
});

describe('numbered-style ordering (IEEE) — the spike', () => {
  it('in-text numbers follow document order; a reuse keeps its number', () => {
    const { inText, citationOrder } = renderCitations(LIB, CLUSTERS, 'ieee');
    expect(citationOrder).toEqual(['B', 'A', 'C']);           // first-appearance order
    expect(inText).toEqual(['[1]', '[2]', '[3]', '[1]']);     // B=1, A=2, C=3, B reused=1
  });

  it('the bibliography is ordered to MATCH the in-text numbers', () => {
    const { bibliography } = renderCitations(LIB, CLUSTERS, 'ieee');
    const lines = bibliography.split('\n');
    expect(lines[0]).toMatch(/^\[1\].*Bravo/);   // [1] == B (first cited)
    expect(lines[1]).toMatch(/^\[2\].*Alpha/);   // [2] == A
    expect(lines[2]).toMatch(/^\[3\].*Charlie/); // [3] == C
  });

  it('only CITED references appear (uncited library items are excluded)', () => {
    const { citationOrder, bibliography } = renderCitations(LIB, [['A']], 'ieee');
    expect(citationOrder).toEqual(['A']);
    expect(bibliography).toMatch(/Alpha/);
    expect(bibliography).not.toMatch(/Bravo|Charlie/);
  });
});

describe('author-date style (APA) also works', () => {
  it('in-text markers are author-year, consistent on reuse; bibliography is alphabetical', () => {
    const { inText, bibliography } = renderCitations(LIB, CLUSTERS, 'apa');
    expect(inText[0]).toMatch(/He, 2016/);
    expect(inText[3]).toBe(inText[0]);            // reuse renders identically
    const lines = bibliography.split('\n');
    expect(lines[0]).toMatch(/Devlin/);           // alphabetical (correct for author-date)
    expect(lines[2]).toMatch(/Vaswani/);
  });
});

describe('honest edge cases', () => {
  it('no citations → empty in-text list + empty bibliography', () => {
    expect(renderCitations(LIB, [], 'ieee')).toEqual({ inText: [], bibliography: '', citationOrder: [], dangling: [] });
  });
  it('a dangling id (not in the library) is dropped + reported, never crashes citeproc', () => {
    const r = renderCitations(LIB, [['A'], ['ghost'], ['B']], 'ieee');
    expect(r.dangling).toEqual(['ghost']);
    expect(r.inText).toEqual(['[1]', '', '[2]']); // ghost → '' placeholder; A/B still number
    expect(r.citationOrder).toEqual(['A', 'B']);
  });
  it('an unregistered style throws (no silent APA fallback)', () => {
    expect(() => renderCitations(LIB, [['A']], 'not-a-real-style-xyz')).toThrow(/not registered/);
  });
});
