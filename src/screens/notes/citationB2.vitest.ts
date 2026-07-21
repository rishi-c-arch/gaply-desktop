// Set B2 pins — resolve-on-export + auto-bibliography. This is a research-
// integrity feature: the exported in-text [1] and the exported References [1]
// MUST be the same paper. We verify by unzipping the real .docx and reading the
// document.xml text. Styles registered from disk (headless).
import { describe, it, expect, beforeAll } from 'vitest';
import { readFileSync, writeFileSync, mkdtempSync } from 'fs';
import { execFileSync } from 'child_process';
import { tmpdir } from 'os';
import { join } from 'path';

import { ensureCslEngine, registerStyleXml, registerLocaleXml } from '../citations/cslEngine';
import { CslItem } from '../citations/citationTypes';
import { renderManuscriptCitations } from './manuscriptCitations';
import { buildManuscriptDocx } from './manuscriptDocx';
import { newManuscript, manuscriptToDraft, manuscriptFromNote, Scaffold, Manuscript } from './manuscriptModel';
import { Note } from './notesBridge';

const CAT = JSON.parse(readFileSync('public/manuscripts/scaffolds.json', 'utf8')) as { scaffolds: Scaffold[] };
const scaffoldById = (id: string) => CAT.scaffolds.find((s) => s.id === id)!;

const item = (id: string, family: string, year: number, title: string): CslItem =>
  ({ id, type: 'article-journal', title, author: [{ family }], issued: { year }, containerTitle: 'J' });
const LIB = new Map<string, CslItem>([
  ['refA', item('refA', 'He', 2016, 'Deep residual learning')],
  ['refB', item('refB', 'Vaswani', 2017, 'Attention is all you need')],
  ['refC', item('refC', 'Devlin', 2019, 'BERT pretraining')],
]);

beforeAll(async () => {
  await ensureCslEngine();
  await registerLocaleXml('en-US', readFileSync('public/csl/locales/locales-en-US.xml', 'utf8'));
  await registerStyleXml('ieee', readFileSync('public/csl/styles/ieee.csl', 'utf8'));
  await registerStyleXml('apa', readFileSync('public/csl/styles/apa.csl', 'utf8'));
});

// Build a manuscript on a scaffold, set section bodies by key.
const makeMs = (scaffoldId: string, bodies: Record<string, string>): Manuscript => {
  const m = newManuscript('m1', scaffoldById(scaffoldId));
  m.title = 'A Study';
  for (const [k, v] of Object.entries(bodies)) m.sections.find((s) => s.key === k)!.body = v;
  return m;
};

// Export → unzip → { body, refs } text (the References heading is unnumbered +
// not uppercased in every style, so it's a stable split point).
const exportParts = async (m: Manuscript, styleId = m.cslStyleId): Promise<{ full: string; body: string; refs: string }> => {
  const cites = await renderManuscriptCitations(m.sections, styleId, LIB);
  const blob = await buildManuscriptDocx(m, scaffoldById(m.scaffoldId).docxFormat, undefined, cites);
  const dir = mkdtempSync(join(tmpdir(), 'b2-'));
  writeFileSync(join(dir, 'o.docx'), new Uint8Array(await blob.arrayBuffer()));
  execFileSync('unzip', ['-o', '-q', join(dir, 'o.docx'), '-d', dir]);
  const xml = readFileSync(join(dir, 'word', 'document.xml'), 'utf8');
  const full = xml.replace(/<\/w:p>/g, '\n').replace(/<[^>]+>/g, '');
  const i = full.lastIndexOf('References');
  return { full, body: i >= 0 ? full.slice(0, i) : full, refs: i >= 0 ? full.slice(i) : '' };
};
const refCount = (refs: string) => (refs.match(/^\[\d+\]/gm) || []).length;

/* -------- (a) ⭐ IN-TEXT ↔ BIBLIOGRAPHY MATCH -------- */
describe('in-text markers match the References list, in document order (pin a)', () => {
  it('IEEE: scattered citations + a reuse → exported [n] and References [n] agree', async () => {
    const m = makeMs('ieee', {
      introduction: 'We build on [[cite:refB]] and [[cite:refA]].',
      methods: 'Following [[cite:refC]].',
      results: 'As in [[cite:refB]] again.', // reuse of refB
    });
    const { body, refs } = await exportParts(m);
    // References, in document order of first appearance (refB, refA, refC):
    expect(refs).toMatch(/\[1\][^\n]*Vaswani/); // [1] is refB
    expect(refs).toMatch(/\[2\][^\n]*He/);      // [2] is refA
    expect(refs).toMatch(/\[3\][^\n]*Devlin/);  // [3] is refC
    expect(refCount(refs)).toBe(3);             // reuse doesn't duplicate
    // in-text markers present in the body (refB→[1], refA→[2], refC→[3], reuse→[1])
    expect(body).toContain('[1]');
    expect(body).toContain('[2]');
    expect(body).toContain('[3]');
  });
});

/* -------- (b) SECTION REORDER RENUMBERS -------- */
describe('section reorder renumbers in-text AND bibliography (pin b)', () => {
  it('moving Methods before Introduction flips the numbering consistently', async () => {
    const base = makeMs('ieee', { introduction: 'See [[cite:refA]].', methods: 'And [[cite:refB]].' });
    const before = (await exportParts(base)).refs;
    expect(before).toMatch(/\[1\][^\n]*He/);   // refA first
    expect(before).toMatch(/\[2\][^\n]*Vaswani/);

    // reorder: methods now before introduction
    const mi = base.sections.findIndex((s) => s.key === 'methods');
    const ii = base.sections.findIndex((s) => s.key === 'introduction');
    const reordered: Manuscript = { ...base, sections: (() => { const a = [...base.sections]; const [m] = a.splice(mi, 1); a.splice(a.indexOf(base.sections[ii]) , 0, m); return a; })() };
    const after = (await exportParts(reordered)).refs;
    expect(after).toMatch(/\[1\][^\n]*Vaswani/); // refB now first
    expect(after).toMatch(/\[2\][^\n]*He/);
  });
});

/* -------- (c) STYLE FIDELITY -------- */
describe('style fidelity: IEEE numbered vs APA author-date (pin c)', () => {
  const m = () => makeMs('ieee', { introduction: 'Text [[cite:refA]] and [[cite:refB]].' });
  it('IEEE → numbered in-text + numbered References', async () => {
    const { body, refs } = await exportParts(m(), 'ieee');
    expect(body).toContain('[1]');
    expect(refs).toMatch(/\[1\][^\n]*He/);
  });
  it('APA → author-date in-text + alphabetical References', async () => {
    const { body, refs } = await exportParts(m(), 'apa');
    expect(body).toMatch(/\(He, 2016\)/);          // author-date in-text
    expect(refs).not.toMatch(/^\[1\]/m);           // NOT numbered
    expect(refs.indexOf('He')).toBeLessThan(refs.indexOf('Vaswani')); // alphabetical
  });
});

/* -------- (d) DANGLING doesn't break numbering -------- */
describe('a dangling citation exports [?] without breaking numbers (pin d)', () => {
  it('[A, dangling, B] → [1] [?] [2]', async () => {
    const m = makeMs('ieee', { introduction: '[[cite:refA]] [[cite:ghost]] [[cite:refB]].' });
    const { body, refs } = await exportParts(m);
    expect(body).toMatch(/\[1\]\s*\[\?\]\s*\[2\]/); // A=[1], ghost=[?], B=[2]
    expect(refCount(refs)).toBe(2);               // ghost excluded from References
  });
});

/* -------- (e) BACKWARD COMPAT: no citations → unchanged -------- */
describe('a manuscript with NO citations exports as before (pin e)', () => {
  it('manual References text is preserved; nothing auto-generated', async () => {
    const m = makeMs('ieee', { introduction: 'Plain intro, no citations.', references: '[1] A manually typed reference.' });
    const { full } = await exportParts(m);
    expect(full).toContain('A manually typed reference.'); // manual text kept
    expect(full).toContain('Plain intro, no citations.');
  });
  it('renderManuscriptCitations reports hasCitations:false for a citation-free doc', async () => {
    const r = await renderManuscriptCitations(makeMs('ieee', { introduction: 'No cites.' }).sections, 'ieee', LIB);
    expect(r).toMatchObject({ hasCitations: false, perToken: [], bibliography: '' });
  });
});

/* -------- round-trip: insert → save → export → reopen → markers unchanged -------- */
describe('resolved citations are stable across save/reopen (round-trip)', () => {
  it('perToken + bibliography identical before save and after reopen', async () => {
    const m = makeMs('ieee', { introduction: '[[cite:refB]] [[cite:refA]]', results: '[[cite:refB]]' });
    const before = await renderManuscriptCitations(m.sections, 'ieee', LIB);
    const draft = manuscriptToDraft(m);
    const note = { id: 'm1', note_type: 'manuscript', paper_id: null, paper_title: '', title: draft.title ?? '', fields_json: JSON.stringify(draft.fields), body: '', tags: [], sync_status: 'local_only', created_at: 1, updated_at: 1 } as Note;
    const reopened = manuscriptFromNote(note, scaffoldById('ieee'));
    const after = await renderManuscriptCitations(reopened.sections, 'ieee', LIB);
    expect(after.perToken).toEqual(before.perToken);
    expect(after.bibliography).toBe(before.bibliography);
  });
});

/* -------- live/export parity (a, restated at the data layer) -------- */
describe('live display and export use ONE path (parity)', () => {
  it('the markers the editor shows equal the markers export writes', async () => {
    const m = makeMs('ieee', { introduction: '[[cite:refB]] [[cite:refA]]', methods: '[[cite:refB]]' });
    const r = await renderManuscriptCitations(m.sections, 'ieee', LIB);
    // live markers (refId map) and export perToken agree for every token position
    const ids = ['refB', 'refA', 'refB'];
    ids.forEach((id, k) => expect(r.perToken[k]).toBe(r.markers.get(id)!.marker));
  });
});
