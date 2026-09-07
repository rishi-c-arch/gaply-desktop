// Set B1 pins — insert-citation + live style-aware display in the manuscript
// editor. Styles are registered from disk (headless) so the marker map computes;
// the library is an injected mock. B1 is display-only (export is B2).
import React from 'react';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, beforeAll, describe, expect, it, vi } from 'vitest';
import { readFileSync } from 'fs';

import { ensureCslEngine, registerStyleXml, registerLocaleXml } from '../citations/cslEngine';
import { StoredReference, LocalLibrary } from '../citations/localLibrary';
import { CslItem } from '../citations/citationTypes';
import { signatureOf, computeMarkers } from './manuscriptCitations';
import { manuscriptToDraft, newManuscript, manuscriptFromNote, Scaffold } from './manuscriptModel';
import { makeMockNotesBridge, Note } from './notesBridge';
import ManuscriptEditor from './ManuscriptEditor';

// Serve the real scaffold catalog to the async loader (fetch fails in jsdom).
vi.mock('./manuscriptScaffolds', async (importOriginal) => {
  const actual = await importOriginal<typeof import('./manuscriptScaffolds')>();
  const fs = await import('fs');
  const cat = JSON.parse(fs.readFileSync('public/manuscripts/scaffolds.json', 'utf8'));
  return { ...actual, loadScaffoldCatalog: async () => cat, loadScaffolds: async () => cat.scaffolds };
});

const CATALOG = JSON.parse(readFileSync('public/manuscripts/scaffolds.json', 'utf8')) as { scaffolds: Scaffold[] };
const scaffoldById = (id: string) => CATALOG.scaffolds.find((s) => s.id === id)!;

const ref = (id: string, family: string, given: string, year: number, title: string): StoredReference => ({
  id, csl_json: JSON.stringify({ id, type: 'article-journal', title, author: [{ family, given }], issued: { 'date-parts': [[year]] }, 'container-title': 'J' }),
  doi: null, title, authors: `${family}, ${given}`, year, tags: [], retracted: false, source: null,
  verify_provenance: [], verify_outcome: null, verified_at: null,
  retraction_outcome: null,
  retraction_checked_at: null, sync_status: 'local_only', created_at: 1, updated_at: 1,
});
const REFS = [ref('refA', 'He', 'K', 2016, 'Deep residual learning'), ref('refB', 'Vaswani', 'A', 2017, 'Attention is all you need')];
const cslById = new Map<string, CslItem>(REFS.map((r) => [r.id, { id: r.id, type: 'article-journal', title: r.title, author: [{ family: r.authors.split(',')[0] }], issued: { year: r.year! } }]));

const mockLib = (refs: StoredReference[] = REFS): LocalLibrary => ({
  list: async () => refs,
  search: async (q: string) => refs.filter((r) => !q || r.title.toLowerCase().includes(q.toLowerCase())),
  upsert: async () => refs[0], setTags: async () => refs[0], remove: async () => {}, markSync: async () => {},
});

// Build a stored manuscript note on a given scaffold, with a section body.
const noteWith = async (scaffoldId: string, sectionKey: string, body: string): Promise<Note> => {
  const sc = scaffoldById(scaffoldId);
  const m = newManuscript('m1', sc);
  m.title = 'T';
  m.sections.find((s) => s.key === sectionKey)!.body = body;
  const bridge = makeMockNotesBridge();
  await bridge.create(manuscriptToDraft(m));
  return (await bridge.get('m1'))!;
};

beforeAll(async () => {
  // jsdom lacks the layout APIs ProseMirror's scroll-into-view uses on insert.
  const rects = () => ({ length: 0, item: () => null, [Symbol.iterator]: function* () { /* empty */ } });
  const box = () => ({ top: 0, left: 0, bottom: 0, right: 0, width: 0, height: 0, x: 0, y: 0, toJSON: () => ({}) });
  for (const proto of [Range.prototype, Element.prototype]) {
    (proto as unknown as { getClientRects: unknown }).getClientRects = rects;
    (proto as unknown as { getBoundingClientRect: unknown }).getBoundingClientRect = box;
  }
  (Element.prototype as unknown as { scrollIntoView: unknown }).scrollIntoView = () => {};

  await ensureCslEngine();
  await registerLocaleXml('en-US', readFileSync('public/csl/locales/locales-en-US.xml', 'utf8'));
  await registerStyleXml('ieee', readFileSync('public/csl/styles/ieee.csl', 'utf8'));
  await registerStyleXml('apa', readFileSync('public/csl/styles/apa.csl', 'utf8'));
});
afterEach(cleanup);

/* -------- (b) LIVE STYLE: same doc, different style → different marker -------- */
describe('style drives the marker (pin b)', () => {
  const sections = [{ body: 'Intro [[cite:refA]] and [[cite:refB]].' }, { body: 'More [[cite:refA]] reuse.' }];
  it('IEEE renders numbered [1][2]; APA renders author-date; reuse is stable', async () => {
    const ieee = await computeMarkers(sections, 'ieee', cslById);
    expect(ieee.get('refA')).toEqual({ marker: '[1]', missing: false });
    expect(ieee.get('refB')).toEqual({ marker: '[2]', missing: false });
    const apa = await computeMarkers(sections, 'apa', cslById);
    expect(apa.get('refA')!.marker).toMatch(/He, 2016/);
    expect(apa.get('refB')!.marker).toMatch(/Vaswani, 2017/);
  });
});

/* -------- (c) PERFORMANCE: the signature gate -------- */
describe('markers recompute ONLY on citation/style/library change, not prose (pin c)', () => {
  const libIds = ['refA', 'refB'];
  it('stable across prose; changes on citation add, style switch, and library presence', () => {
    const a = [{ body: 'Intro [[cite:refA]] here.' }];
    const proseEdited = [{ body: 'Intro [[cite:refA]] here, expanded with more prose.' }];
    const citationAdded = [{ body: 'Intro [[cite:refA]] here [[cite:refB]].' }];
    expect(signatureOf(proseEdited, 'ieee', libIds)).toBe(signatureOf(a, 'ieee', libIds)); // prose → NO recompute
    expect(signatureOf(citationAdded, 'ieee', libIds)).not.toBe(signatureOf(a, 'ieee', libIds)); // new citation
    expect(signatureOf(a, 'apa', libIds)).not.toBe(signatureOf(a, 'ieee', libIds)); // style switch
    expect(signatureOf(a, 'ieee', ['refA'])).not.toBe(signatureOf(a, 'ieee', [])); // LIBRARY presence change
    // library id ORDER doesn't matter (sorted); a metadata-only edit (same ids) is a no-op.
    expect(signatureOf(a, 'ieee', ['refB', 'refA'])).toBe(signatureOf(a, 'ieee', ['refA', 'refB']));
  });
});

/* -------- (f)+(g) LIBRARY gate reacts both directions -------- */
describe('the gate reacts to library presence (pins f, g)', () => {
  const sections = [{ body: 'x [[cite:refA]] y' }];
  const withA = new Map([['refA', cslById.get('refA')!]]);
  const withoutA = new Map<string, CslItem>();

  it('(f) dangling → resolved: add the ref to the library, document unchanged', async () => {
    expect((await computeMarkers(sections, 'ieee', withoutA)).get('refA')).toEqual({ marker: '', missing: true });
    expect((await computeMarkers(sections, 'ieee', withA)).get('refA')).toEqual({ marker: '[1]', missing: false });
    expect(signatureOf(sections, 'ieee', withA.keys())).not.toBe(signatureOf(sections, 'ieee', withoutA.keys()));
  });

  it('(g) resolved → dangling: remove the ref from the library, document unchanged', async () => {
    const resolved = await computeMarkers(sections, 'ieee', withA);
    expect(resolved.get('refA')).toEqual({ marker: '[1]', missing: false });
    const gone = await computeMarkers(sections, 'ieee', withoutA);
    expect(gone.get('refA')).toEqual({ marker: '', missing: true });
    expect(signatureOf(sections, 'ieee', withoutA.keys())).not.toBe(signatureOf(sections, 'ieee', withA.keys()));
  });
});

/* -------- (h)+(i) document-order renumbering -------- */
describe('renumbering follows document order (pins h, i)', () => {
  const lib = new Map([['refA', cslById.get('refA')!], ['refB', cslById.get('refB')!]]);

  it('(h) duplicate-then-delete: [A, B, A] → remove first A → B renumbers [2]→[1], A [1]→[2]', async () => {
    const before = [{ body: '[[cite:refA]] [[cite:refB]] [[cite:refA]]' }];
    const mb = await computeMarkers(before, 'ieee', lib);
    expect(mb.get('refA')!.marker).toBe('[1]');
    expect(mb.get('refB')!.marker).toBe('[2]');
    const after = [{ body: '[[cite:refB]] [[cite:refA]]' }]; // first A token removed
    const ma = await computeMarkers(after, 'ieee', lib);
    expect(ma.get('refB')!.marker).toBe('[1]');
    expect(ma.get('refA')!.marker).toBe('[2]');
    expect(signatureOf(after, 'ieee', lib.keys())).not.toBe(signatureOf(before, 'ieee', lib.keys()));
  });

  it('(i) cross-section move: a token moved from section 2 to section 1 renumbers by section order', async () => {
    const before = [{ body: 'Intro.' }, { body: 'M [[cite:refA]] and [[cite:refB]].' }]; // order A, B
    const mb = await computeMarkers(before, 'ieee', lib);
    expect(mb.get('refA')!.marker).toBe('[1]');
    expect(mb.get('refB')!.marker).toBe('[2]');
    const after = [{ body: 'Intro [[cite:refB]].' }, { body: 'M [[cite:refA]].' }]; // B now in section 1
    const ma = await computeMarkers(after, 'ieee', lib);
    expect(ma.get('refB')!.marker).toBe('[1]'); // section order × token order → B first
    expect(ma.get('refA')!.marker).toBe('[2]');
    expect(signatureOf(after, 'ieee', lib.keys())).not.toBe(signatureOf(before, 'ieee', lib.keys()));
  });
});

/* -------- (e) DANGLING: deleted-from-library ref shows ⚠, never crashes -------- */
describe('dangling reference (pin e)', () => {
  it('computeMarkers flags a refId not in the library as missing', async () => {
    const map = await computeMarkers([{ body: 'x [[cite:ghost]] y' }], 'ieee', cslById);
    expect(map.get('ghost')).toEqual({ marker: '', missing: true });
  });
  it('the editor renders a ⚠ missing-reference chip, not a crash', async () => {
    render(<ManuscriptEditor id="m1" existing={await noteWith('ieee', 'abstract', 'See [[cite:ghost]].')} onSave={() => {}} onClose={() => {}} library={mockLib()} />);
    await waitFor(() => expect(screen.getByTestId('cite-ghost')).toBeTruthy());
    expect(screen.getByTestId('cite-ghost').getAttribute('data-missing')).toBe('1');
    expect(screen.getByTestId('cite-ghost').textContent).toMatch(/missing/i);
  });
});

/* -------- PHASE 0: library-change liveness (remount) + metadata fingerprint -------- */
describe('library changes flip the ⚠ chip on return (Phase 0)', () => {
  it('re-opening the manuscript after a library edit refreshes markers BOTH directions', async () => {
    const note = await noteWith('ieee', 'abstract', 'See [[cite:refA]].');
    // (1) refA NOT in the library → dangling ⚠
    const a = render(<ManuscriptEditor id="m1" existing={note} onSave={() => {}} onClose={() => {}} library={mockLib([])} />);
    await waitFor(() => expect(screen.getByTestId('cite-refA').getAttribute('data-missing')).toBe('1'));
    a.unmount();
    // (2) user adds refA in the Citation Manager → re-open manuscript (remount) → resolved
    const b = render(<ManuscriptEditor id="m1" existing={note} onSave={() => {}} onClose={() => {}} library={mockLib([REFS[0]])} />);
    await waitFor(() => expect(screen.getByTestId('cite-refA').getAttribute('data-missing')).toBe('0'));
    await waitFor(() => expect(screen.getByTestId('cite-refA').textContent).toBe('[1]'));
    b.unmount();
    // (3) reverse — user deletes refA → re-open → back to ⚠
    render(<ManuscriptEditor id="m1" existing={note} onSave={() => {}} onClose={() => {}} library={mockLib([])} />);
    await waitFor(() => expect(screen.getByTestId('cite-refA').getAttribute('data-missing')).toBe('1'));
  });

  it('the signature fingerprint reacts to a metadata edit (updated_at bump), prose still does not', () => {
    const sections = [{ body: 'x [[cite:refA]] y' }];
    // presence-only fingerprint (ids) is unchanged by prose; a metadata bump differs
    expect(signatureOf(sections, 'ieee', ['refA@1'])).not.toBe(signatureOf(sections, 'ieee', ['refA@2']));
    const prose = [{ body: 'x [[cite:refA]] y, more words' }];
    expect(signatureOf(prose, 'ieee', ['refA@2'])).toBe(signatureOf(sections, 'ieee', ['refA@2'])); // prose → NO recompute
  });
});

/* -------- (B2 pin f) honest auto-References banner + live preview -------- */
describe('References section auto-generates honestly when citations exist (B2 pin f)', () => {
  it('shows the "Auto-generated from your citations" banner + a live bibliography preview', async () => {
    const note = await noteWith('ieee', 'introduction', 'Intro [[cite:refA]].');
    render(<ManuscriptEditor id="m1" existing={note} onSave={() => {}} onClose={() => {}} library={mockLib()} />);
    await waitFor(() => expect(screen.getByTestId('ms-nav-references')).toBeTruthy());
    fireEvent.click(screen.getByTestId('ms-nav-references'));
    // the editable box is REPLACED by the honest banner (never a silent overwrite)
    await waitFor(() => expect(screen.getByTestId('ms-refs-banner')).toBeTruthy());
    expect(screen.getByTestId('ms-refs-banner').textContent).toMatch(/Auto-generated from your citations/i);
    expect(screen.queryByTestId('ms-body-references')).toBeNull(); // no editable RichBody here now
    await waitFor(() => expect(screen.getByTestId('ms-refs-preview').textContent).toMatch(/He|\[1\]/));
  });

  it('with NO citations the References section stays a normal editable box (backward compat)', async () => {
    const note = await noteWith('ieee', 'references', 'My manual reference list.');
    render(<ManuscriptEditor id="m1" existing={note} onSave={() => {}} onClose={() => {}} library={mockLib()} />);
    await waitFor(() => expect(screen.getByTestId('ms-nav-references')).toBeTruthy());
    fireEvent.click(screen.getByTestId('ms-nav-references'));
    await waitFor(() => expect(screen.getByTestId('ms-body-references')).toBeTruthy()); // editable
    expect(screen.queryByTestId('ms-refs-banner')).toBeNull();
  });
});

/* -------- (a) INSERT + live marker in the real editor -------- */
describe('insert a citation → live marker (pin a)', () => {
  it('pick a reference from the picker → gaplyCite node inserts and renders [1]', async () => {
    render(<ManuscriptEditor id="m1" existing={await noteWith('ieee', 'abstract', 'Start. ')} onSave={() => {}} onClose={() => {}} library={mockLib()} />);
    await waitFor(() => expect(screen.getByTestId('rb-insert-cite')).toBeTruthy()); // manuscript surface active
    fireEvent.mouseDown(screen.getByTestId('rb-insert-cite'));                       // open picker
    const pick = await screen.findByTestId('cite-pick-refA');                        // debounced search resolved
    fireEvent.click(pick);
    // node inserted + live marker rendered in the manuscript's IEEE style
    await waitFor(() => expect(screen.getByTestId('cite-refA')).toBeTruthy());
    await waitFor(() => expect(screen.getByTestId('cite-refA').textContent).toBe('[1]'));
  });
});

/* -------- (d) ROUND-TRIP: insert → save → reopen keeps the citation -------- */
describe('insert → save → reopen (pin d)', () => {
  it('a citation inserted in the editor survives the fields_json save/reopen path', async () => {
    const onSave = vi.fn();
    render(<ManuscriptEditor id="m1" existing={await noteWith('ieee', 'abstract', 'Start. ')} onSave={onSave} onClose={() => {}} library={mockLib()} />);
    await waitFor(() => expect(screen.getByTestId('rb-insert-cite')).toBeTruthy());
    fireEvent.mouseDown(screen.getByTestId('rb-insert-cite'));
    fireEvent.click(await screen.findByTestId('cite-pick-refB'));
    await waitFor(() => expect(screen.getByTestId('cite-refB')).toBeTruthy());
    fireEvent.click(screen.getByTestId('ms-save'));

    const draft = onSave.mock.calls[0][0];
    const intro = (draft.fields as { sections: Array<{ key: string; body: string }> }).sections.find((s) => s.key === 'abstract')!;
    expect(intro.body).toContain('[[cite:refB]]');                 // token stored in fields_json
    // reopen → still present as a token (extractable + re-rendered)
    const reopened = manuscriptFromNote({ ...(draft as unknown as Note), fields_json: JSON.stringify(draft.fields) } as Note, scaffoldById('ieee'));
    expect(reopened.sections.find((s) => s.key === 'abstract')!.body).toContain('[[cite:refB]]');
  });
});
