// Phase 3a pins — within-section markdown → real docx elements, verified in the
// document.xml. Bold/italic runs, bullet + numbered lists, GFM tables, sub-
// headings, blockquotes, inline code — NOT literal markdown text. And the B2
// citation↔References match must survive citations nested inside formatting.
import { describe, it, expect, beforeAll } from 'vitest';
import { readFileSync, writeFileSync, mkdtempSync } from 'fs';
import { execFileSync } from 'child_process';
import { tmpdir } from 'os';
import { join } from 'path';

import { ensureCslEngine, registerStyleXml, registerLocaleXml } from '../citations/cslEngine';
import { CslItem } from '../citations/citationTypes';
import { renderManuscriptCitations } from './manuscriptCitations';
import { buildManuscriptDocx } from './manuscriptDocx';
import { newManuscript, Scaffold, Manuscript } from './manuscriptModel';

const CAT = JSON.parse(readFileSync('public/manuscripts/scaffolds.json', 'utf8')) as { scaffolds: Scaffold[] };
const sc = (id: string) => CAT.scaffolds.find((s) => s.id === id)!;
const LIB = new Map<string, CslItem>([
  ['refA', { id: 'refA', type: 'article-journal', title: 'Deep residual learning', author: [{ family: 'He' }], issued: { year: 2016 }, containerTitle: 'CVPR' }],
  ['refB', { id: 'refB', type: 'article-journal', title: 'Attention is all you need', author: [{ family: 'Vaswani' }], issued: { year: 2017 }, containerTitle: 'NeurIPS' }],
]);

beforeAll(async () => {
  await ensureCslEngine();
  await registerLocaleXml('en-US', readFileSync('public/csl/locales/locales-en-US.xml', 'utf8'));
  await registerStyleXml('ieee', readFileSync('public/csl/styles/ieee.csl', 'utf8'));
  await registerStyleXml('apa', readFileSync('public/csl/styles/apa.csl', 'utf8'));
});

const build = async (bodies: Record<string, string>, styleId = 'ieee'): Promise<{ xml: string; text: string }> => {
  const m: Manuscript = newManuscript('m1', sc('ieee'));
  m.title = 'X';
  for (const [k, v] of Object.entries(bodies)) m.sections.find((s) => s.key === k)!.body = v;
  const cites = await renderManuscriptCitations(m.sections, styleId, LIB);
  const blob = await buildManuscriptDocx(m, sc('ieee').docxFormat, undefined, cites);
  const dir = mkdtempSync(join(tmpdir(), 'r-'));
  writeFileSync(join(dir, 'o.docx'), new Uint8Array(await blob.arrayBuffer()));
  execFileSync('unzip', ['-o', '-q', join(dir, 'o.docx'), '-d', dir]);
  const xml = readFileSync(join(dir, 'word', 'document.xml'), 'utf8');
  return { xml, text: xml.replace(/<\/w:p>/g, '\n').replace(/<[^>]+>/g, '') };
};

/* -------- (b) each formatting type → the right docx element (not literal md) -------- */
describe('formatting exports as real docx elements (pin b)', () => {
  it('bold + italic → bold/italic runs (no literal ** or *)', async () => {
    const { xml, text } = await build({ introduction: 'A **bold** and *italic* word.' });
    expect(xml).toContain('<w:b/>');       // bold run
    expect(xml).toContain('<w:i/>');       // italic run
    expect(text).not.toContain('**');      // no literal markdown
    expect(text).toContain('bold');
  });

  it('bullet list → real list paragraphs (numPr), not "- " text', async () => {
    const { xml, text } = await build({ methods: '- first step\n- second step' });
    expect(xml).toContain('<w:numPr>');            // list numbering property
    expect(text).toContain('first step');
    expect(text).not.toMatch(/^-\s/m);             // no literal "- " bullet text
  });

  it('numbered list → real ordered list (numPr + a numbering instance)', async () => {
    const { xml, text } = await build({ methods: '1. alpha\n2. beta' });
    expect(xml).toContain('<w:numPr>');
    expect(text).toContain('alpha');
    expect(text).not.toMatch(/^1\.\s/m);           // not literal "1. "
  });

  it('GFM table → a real docx table (w:tbl), not pipe text', async () => {
    const { xml, text } = await build({ results: '| A | B |\n| --- | --- |\n| 1 | 2 |' });
    expect(xml).toContain('<w:tbl>');
    expect(xml).toContain('<w:tc>');               // table cells
    expect(text).not.toContain('| A |');           // no literal pipe row
  });

  it('## subheading → Heading2 (below the section H1)', async () => {
    const { xml } = await build({ discussion: '## A subheading\n\nBody text.' });
    expect(xml).toContain('w:val="Heading2"');
  });

  it('> blockquote → a bordered/indented paragraph (not "> " text)', async () => {
    const { xml, text } = await build({ discussion: '> a quoted line' });
    expect(xml).toMatch(/w:pBdr|w:ind/);           // border or indent
    expect(text).not.toMatch(/^>\s/m);
    expect(text).toContain('a quoted line');
  });

  it('`code` → a monospace run', async () => {
    const { xml } = await build({ methods: 'Run the `deploy` script.' });
    expect(xml).toMatch(/Courier New/);
  });
});

/* -------- (a) ⭐ citations INSIDE formatting still match References -------- */
describe('citations nested in formatting still match References (pin a + e)', () => {
  it('a citation in a bold span and one in a list item resolve + match, in order', async () => {
    const { xml, text } = await build({
      introduction: 'A **bold term [[cite:refB]]** here.',
      methods: '- a step citing [[cite:refA]]',
    });
    // document order: refB (bold, intro) then refA (list, methods) → [1], [2]
    expect(text).toContain('[1]');
    expect(text).toContain('[2]');
    // and the References list matches: [1]=Vaswani(refB), [2]=He(refA)
    const refs = text.slice(text.lastIndexOf('References'));
    expect(refs).toMatch(/\[1\][^\n]*Vaswani/);
    expect(refs).toMatch(/\[2\][^\n]*He/);
    // the bold citation is really bold (formatting preserved around the marker)
    expect(xml).toContain('<w:b/>');
    // exactly 2 references
    expect((refs.match(/^\[\d+\]/gm) || []).length).toBe(2);
  });

  it('a citation in a TABLE CELL resolves and matches', async () => {
    const { text } = await build({ results: '| Method | Ref |\n| --- | --- |\n| ResNet | [[cite:refA]] |' });
    const refs = text.slice(text.lastIndexOf('References'));
    expect(text).toContain('[1]');
    expect(refs).toMatch(/\[1\][^\n]*He/);
  });
});

/* -------- (c) BACKWARD COMPAT: plain text unchanged -------- */
describe('plain-text manuscripts export cleanly (pin c)', () => {
  it('plain paragraphs have no stray markdown and read as written', async () => {
    const { xml, text } = await build({ introduction: 'Just plain prose.\n\nA second paragraph.' });
    expect(text).toContain('Just plain prose.');
    expect(text).toContain('A second paragraph.');
    expect(xml).not.toContain('<w:numPr>');   // no accidental lists
    expect(xml).not.toContain('<w:tbl>');     // no accidental tables
  });
});
