// The journal-styled .docx — two-column reading layout and real OMML math.
//
// THESE TESTS READ THE XML BACK OUT, deliberately. §13 of the Note Creator
// teardown established that Word accepts a corrupt PNG without a murmur and
// renders it as nothing, so "it opened in Word" proves well-formedness within
// Word's tolerance and not correctness. Opening one in Word is still required
// before shipping — it catches what XML cannot — but it is the second check,
// not the first.
import { describe, expect, it, beforeAll } from 'vitest';
import { readFileSync, writeFileSync, mkdtempSync } from 'fs';
import { execFileSync } from 'child_process';
import { tmpdir } from 'os';
import { join } from 'path';

import { ensureCslEngine, registerStyleXml, registerLocaleXml } from '../citations/cslEngine';
import { renderManuscriptCitations } from './manuscriptCitations';
import { buildManuscriptDocx } from './manuscriptDocx';
import { texToOmml, FALLBACK_NOTES } from './manuscriptOmml';
import { newManuscript, Manuscript, Scaffold } from './manuscriptModel';

const CAT = JSON.parse(readFileSync('public/manuscripts/scaffolds.json', 'utf8')) as { scaffolds: Scaffold[] };
const sc = (id: string) => CAT.scaffolds.find((s) => s.id === id)!;

beforeAll(async () => {
  await ensureCslEngine();
  await registerLocaleXml('en-US', readFileSync('public/csl/locales/locales-en-US.xml', 'utf8'));
  await registerStyleXml('ieee', readFileSync('public/csl/styles/ieee.csl', 'utf8'));
});

/** Build a .docx and return its document.xml. `reading` absent = the untouched
 *  single-column SUBMISSION export. */
const build = async (bodies: Record<string, string>, opts: { scaffoldId?: string; reading?: boolean } = {}) => {
  const scaffold = sc(opts.scaffoldId ?? 'ieee');
  const m: Manuscript = newManuscript('m1', scaffold);
  m.title = 'Reading Layout';
  m.authors = 'Ada Lovelace';
  m.affiliations = '¹Analytical Engine Lab';
  for (const [k, v] of Object.entries(bodies)) { const s = m.sections.find((x) => x.key === k); if (s) s.body = v; }
  const cites = await renderManuscriptCitations(m.sections, 'ieee', new Map());
  const fallbacks: Array<{ tex: string; reason: string }> = [];
  const blob = await buildManuscriptDocx(
    m, scaffold.docxFormat, async () => null, cites,
    opts.reading ? scaffold.readingFormat : undefined, fallbacks,
  );
  const dir = mkdtempSync(join(tmpdir(), 'rd-'));
  writeFileSync(join(dir, 'o.docx'), new Uint8Array(await blob.arrayBuffer()));
  execFileSync('unzip', ['-o', '-q', join(dir, 'o.docx'), '-d', dir]);
  return { xml: readFileSync(join(dir, 'word', 'document.xml'), 'utf8'), fallbacks, path: join(dir, 'o.docx') };
};

/* ------------------------- two-column section props ---------------------- */

describe('two-column reading layout', () => {
  it('a two-column venue gets w:cols num=2 and two continuous sections', async () => {
    const { xml } = await build({ introduction: 'x' }, { reading: true });
    expect(xml).toMatch(/<w:cols[^>]*w:num="2"/);
    expect((xml.match(/w:val="continuous"/g) ?? []).length).toBe(2);
  });

  it('the title block section is NOT two-column — it is full width above the body', async () => {
    const { xml } = await build({ introduction: 'x' }, { reading: true });
    // exactly one section carries the column count; the other (the title) does not
    expect((xml.match(/<w:cols[^>]*w:num="2"/g) ?? []).length).toBe(1);
  });

  it('a single-column venue gets no column count at all', async () => {
    const { xml } = await build({ introduction: 'x' }, { reading: true, scaffoldId: 'plos' });
    expect(xml).not.toMatch(/<w:cols[^>]*w:num="2"/);
  });

  it('line numbers follow the venue profile, on both sections', async () => {
    const on = await build({ introduction: 'x' }, { reading: true, scaffoldId: 'plos' });
    expect((on.xml.match(/<w:lnNumType/g) ?? []).length).toBe(2);
    const off = await build({ introduction: 'x' }, { reading: true, scaffoldId: 'ieee' });
    expect(off.xml).not.toContain('<w:lnNumType');
  });

  it('THE SUBMISSION EXPORT IS UNTOUCHED — no columns, no continuous break, one page break', async () => {
    const { xml } = await build({ introduction: 'x' });
    expect(xml).not.toMatch(/<w:cols[^>]*w:num="2"/);
    expect(xml).not.toContain('w:val="continuous"');
    expect(xml).toContain('w:type="page"'); // the title page break the reading layout drops
  });
});

/* -------------------------------- OMML ---------------------------------- */

describe('math becomes real OMML', () => {
  const om = (tex: string) => texToOmml(tex).omml ?? '';

  it('fractions, roots, scripts and n-ary operators', () => {
    expect(om('\\frac{1}{x}')).toContain('<m:f>');
    expect(om('\\sqrt{x}')).toContain('<m:rad>');
    expect(om('\\sqrt[3]{x}')).toMatch(/<m:deg>.*<\/m:deg>/);
    expect(om('x^2')).toContain('<m:sSup>');
    expect(om('x_i')).toContain('<m:sSub>');
    expect(om('x_i^2')).toContain('<m:sSubSup>');
    expect(om('\\sum_{i=1}^{n} x_i')).toContain('<m:nary>');
    expect(om('\\int_0^1 x\\,dx')).toContain('<m:nary>');
  });

  it('matrices, cases and aligned — the ones docx has no builder for', () => {
    expect(om('\\begin{pmatrix} a & b \\\\ c & d \\end{pmatrix}')).toContain('<m:m>');
    expect(om('\\begin{bmatrix} 1 \\\\ 2 \\end{bmatrix}')).toMatch(/m:begChr m:val="\["/);
    expect(om('\\begin{cases} x & x>0 \\\\ -x & x\\le 0 \\end{cases}')).toMatch(/m:begChr m:val="\{"/);
    expect(om('\\begin{aligned} a &= b \\\\ c &= d \\end{aligned}')).toContain('<m:m>');
  });

  it('accents, delimiters, Greek and upright function names', () => {
    expect(om('\\hat{\\beta}')).toContain('<m:acc>');
    expect(om('\\left( x \\right)')).toContain('<m:d>');
    expect(om('\\alpha')).toContain('α');
    expect(om('\\sin')).toContain('<m:sty m:val="p"/>'); // upright, not italic
  });

  it('XML-hostile characters in a formula are escaped, not emitted raw', () => {
    const x = om('a < b');
    expect(x).toContain('&lt;');
    expect(x).not.toMatch(/<m:t[^>]*>a < b/);
  });

  // THE ASSERTION THAT WOULD HAVE CAUGHT IT. docx 7.8.2's
  // ImportedXmlComponent.fromXmlString yields rootKey `undefined`, so the packer
  // wrote every formula inside a literal <undefined> element. That is well-formed
  // XML and invalid OOXML: xmllint passed it, `xml.includes('m:oMath')` passed it
  // — the OMML really was in there — and Word refused the entire file. Only
  // opening it in Word found it. This pin is the cheap version of that check.
  it('NO bogus wrapper element — the OOXML is valid, not merely well-formed', async () => {
    const { xml } = await build(
      { results: '[[math-block:\\begin{pmatrix} a & b \\\\ c & d \\end{pmatrix}]] and [[math:\\frac{1}{x}]]' },
      { reading: true },
    );
    expect(xml).not.toContain('<undefined>');
    expect(xml).not.toContain('</undefined>');
    // every oMath must sit directly in a run-level context, not in a wrapper
    expect(xml).toMatch(/<w:p>(?:(?!<\/w:p>).)*<m:oMath/);
  });

  it('a formula reaches document.xml as OMML, not as text', async () => {
    const { xml, fallbacks } = await build(
      { results: 'Estimator [[math:\\hat{\\beta}]] and\n\n[[math-block:\\int_0^1 x^2\\,dx = \\frac{1}{3}]]' },
      { reading: true },
    );
    expect((xml.match(/<m:oMath/g) ?? []).length).toBe(2);
    expect(xml).toContain('<m:nary>');
    expect(xml).toContain('<m:acc>');
    expect(fallbacks).toEqual([]);
    // the LaTeX source must NOT also be sitting there as literal text
    expect(xml).not.toContain('\\frac{1}{3}');
  });
});

/* ------------------------------ the fallback ----------------------------- */

describe('the fallback is a limit, not a loss', () => {
  it('what falls back is exactly what FALLBACK_NOTES says', () => {
    expect(texToOmml('\\begin{align} a &= b \\end{align}').reason).toMatch(/align/);
    expect(texToOmml('\\newcommand{\\x}{y}\\x').reason).toMatch(/newcommand/);
    expect(texToOmml('\\ce{H2O}').reason).toMatch(/ce/);
    expect(FALLBACK_NOTES.length).toBeGreaterThan(0);
    for (const n of FALLBACK_NOTES) { expect(n.construct).toBeTruthy(); expect(n.why).toBeTruthy(); }
  });

  it('a fallback formula still SHIPS, as its LaTeX source, and is reported', async () => {
    const { xml, fallbacks } = await build(
      { results: 'A [[math:\\begin{align} a &= b \\end{align}]] here.' },
      { reading: true },
    );
    expect(fallbacks).toHaveLength(1);
    expect(fallbacks[0].tex).toContain('align');
    // the mathematics is still on the page — never silently dropped
    expect(xml).toContain('Cambria Math');
    expect(xml).toContain('begin{align}');
  });

  it('texToOmml never throws, whatever it is given', () => {
    for (const junk of ['', '\\', '{', '}', '^', '\\frac{1}', '\\begin{nope}x\\end{nope}', '\\left(']) {
      expect(() => texToOmml(junk)).not.toThrow();
      expect(texToOmml(junk).omml).toBeNull();
    }
  });
});
