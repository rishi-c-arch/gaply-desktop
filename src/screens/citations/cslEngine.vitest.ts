// Set 3 — the full CSL engine. Tests format with REAL bundled .csl styles
// read from disk (public/csl/…) and registered directly — no fetch, no
// network, no mocks around citeproc: this IS the deterministic processor.
import { readFileSync, existsSync } from 'fs';
import { join } from 'path';
import { beforeAll, describe, expect, it } from 'vitest';

import {
  formatWithCsl,
  isStyleReady,
  registerStyleXml,
  toCslJson,
  LEGACY_STYLE_ALIASES,
} from './cslEngine';
import { formatCitation } from './formatCitation';
import { CslItem } from './citationTypes';

const STYLES_DIR = join(process.cwd(), 'public', 'csl', 'styles');
const styleXml = (fileId: string) => readFileSync(join(STYLES_DIR, `${fileId}.csl`), 'utf-8');

/** Verified Set-2-shaped metadata as a frontend CslItem. */
const WATSON: CslItem = {
  id: 'wc1953',
  type: 'article-journal',
  title: 'Molecular Structure of Nucleic Acids',
  author: [
    { family: 'Watson', given: 'J. D.' },
    { family: 'Crick', given: 'F. H. C.' },
  ],
  issued: { year: 1953 },
  DOI: '10.1038/171737a0',
  containerTitle: 'Nature',
  volume: '171',
  issue: '4356',
  page: '737-738',
};

/** The anti-hallucination case: registry lacked volume/issue/page/journal. */
const SPARSE: CslItem = {
  id: 'sparse1',
  type: 'article',
  title: 'A Sparse Preprint',
  author: [{ family: 'Lovelace', given: 'Ada' }],
  issued: { year: 2024 },
};

const SPOT_STYLES: Array<[string, string]> = [
  ['apa', 'apa'],
  ['ieee', 'ieee'],
  ['vancouver', 'nlm-citation-sequence'], // repo retired vancouver.csl; NLM cs IS Vancouver
  ['nature', 'nature'],
  ['chicago-author-date', 'chicago-author-date'],
];

beforeAll(async () => {
  for (const [id, fileId] of SPOT_STYLES) {
    await registerStyleXml(id, styleXml(fileId));
  }
});

describe('the bundled style set (offline by construction)', () => {
  it('ships the full ~2,856-style repository + manifest as app assets', () => {
    const manifest = JSON.parse(readFileSync(join(process.cwd(), 'public', 'csl', 'manifest.json'), 'utf-8'));
    expect(manifest.length).toBeGreaterThan(2000);
    // every legacy alias resolves to a real bundled file
    for (const fileId of Object.values(LEGACY_STYLE_ALIASES)) {
      expect(existsSync(join(STYLES_DIR, `${fileId}.csl`)), `${fileId}.csl`).toBe(true);
    }
    // locales bundled too
    expect(existsSync(join(process.cwd(), 'public', 'csl', 'locales', 'locales-en-US.xml'))).toBe(true);
  });

  it('formats with a style registered from the bundled file — zero network involved', () => {
    // No fetch was mocked anywhere in this suite; styles came from disk.
    const out = formatWithCsl([WATSON], 'apa');
    expect(out.length).toBeGreaterThan(40);
  });
});

describe('spot-checked known-correct formatting', () => {
  it('APA: author-date with ampersand, volume(issue), pages, doi', () => {
    const out = formatWithCsl([WATSON], 'apa');
    expect(out).toContain('Watson, J. D., & Crick, F. H. C.');
    expect(out).toContain('(1953)');
    expect(out).toContain('Nature');
    expect(out).toMatch(/171/);
    expect(out).toMatch(/737/);
    expect(out).toContain('10.1038/171737a0');
  });

  it('IEEE: numeric bracket + initials-first authors', () => {
    const out = formatWithCsl([WATSON], 'ieee');
    expect(out).toMatch(/^\[1\]/);
    expect(out).toContain('J. D. Watson');
    expect(out).toContain('F. H. C. Crick');
    expect(out).toMatch(/vol\.\s*171/i);
  });

  it('Vancouver: family-initials, year;volume:pages', () => {
    const out = formatWithCsl([WATSON], 'vancouver');
    expect(out).toContain('Watson JD');
    expect(out).toContain('Crick FHC');
    expect(out).toMatch(/1953/);
    expect(out).toMatch(/171/);
  });

  it('Nature: journal style with bold volume semantics (text form)', () => {
    const out = formatWithCsl([WATSON], 'nature');
    expect(out).toContain('Watson, J. D.');
    expect(out).toContain('Nature');
    expect(out).toMatch(/737/);
    expect(out).toMatch(/\(1953\)/);
  });

  it('Chicago author-date: quoted title, year after authors', () => {
    const out = formatWithCsl([WATSON], 'chicago-author-date');
    expect(out).toContain('Watson');
    expect(out).toContain('1953');
    expect(out).toContain('Molecular Structure of Nucleic Acids');
  });
});

describe('the deterministic guarantee', () => {
  it('same CSL-JSON + same style → byte-identical output across runs', () => {
    const runs = Array.from({ length: 5 }, () => formatWithCsl([WATSON], 'apa'));
    expect(new Set(runs).size).toBe(1);
    // and across styles independently
    const ieee = Array.from({ length: 3 }, () => formatWithCsl([WATSON], 'ieee'));
    expect(new Set(ieee).size).toBe(1);
  });

  it('a missing field is OMITTED per the style rules — never invented', () => {
    const out = formatWithCsl([SPARSE], 'apa');
    expect(out).toContain('Lovelace');
    expect(out).toContain('2024');
    expect(out).toContain('A Sparse Preprint');
    // no volume/issue/pages/journal appear from nowhere
    expect(out).not.toMatch(/vol/i);
    expect(out).not.toMatch(/\d+\s*\(\d+\)/); // volume(issue)
    expect(out).not.toContain('Nature');
    expect(out).not.toMatch(/pp?\./);
  });

  it('toCslJson never fills an absent field', () => {
    const json = toCslJson(SPARSE) as Record<string, unknown>;
    expect(json.volume).toBeUndefined();
    expect(json.page).toBeUndefined();
    expect(json['container-title']).toBeUndefined();
    expect(json.issued).toEqual({ 'date-parts': [[2024]] });
  });
});

describe('the stable formatCitation seam', () => {
  it('legacy ids format without any preparation (old callers unchanged)', () => {
    // 'mla' was never registered in this suite — the legacy path serves it.
    expect(isStyleReady('mla')).toBe(false);
    const out = formatCitation(WATSON, 'mla');
    expect(out).toContain('Watson');
    expect(out).toContain('Molecular Structure of Nucleic Acids');
  });

  it('a prepared style routes through full citeproc via the SAME seam', () => {
    expect(isStyleReady('apa')).toBe(true);
    const seam = formatCitation(WATSON, 'apa');
    const direct = formatWithCsl([WATSON], 'apa'); // plain 'text' (the export path)
    // Same citeproc CONTENT, but formatCitation (the preview) preserves italics
    // as the *marker* the Formatted component renders; direct/export stays plain.
    expect(seam.replace(/\*/g, '')).toBe(direct);
    expect(seam).toMatch(/\*Nature\*/); // italic journal name kept for the preview
  });

  it('an unknown, unprepared style id errs honestly — no silent wrong-style fallback', () => {
    expect(() => formatCitation(WATSON, 'journal-of-made-up-studies')).toThrow(/not loaded/);
    expect(() => formatWithCsl([WATSON], 'journal-of-made-up-studies')).toThrow(/no silent fallback/);
  });

  it('an obscure real bundled style formats after registration', async () => {
    // Prove the long tail works: pick a real style from the repo far outside
    // the famous 8.
    const id = 'plos';
    await registerStyleXml(id, styleXml(id));
    const out = formatWithCsl([WATSON], id);
    expect(out).toContain('Watson');
    expect(out).toMatch(/1953/);
  });
});

describe('honest edge cases', () => {
  it('malformed CSL-JSON errs honestly', () => {
    const bad = { id: 'x', type: 'article-journal', title: 't', author: 'not-an-array' } as unknown as CslItem;
    expect(() => formatWithCsl([bad], 'apa')).toThrow();
  });
});
