// Research Paper Writer — Set A pins. Storage is Option A (note_type='manuscript',
// sections+metadata in fields_json); the writing surface is the proven RichBody.
// No network, no models. The Word-opens-clean gate is a human hand-test; here we
// pin what's machine-verifiable: round-trip, backward-compat, stable-key reuse,
// and that the .docx builds (a valid zip) incl. empty/partial content.
import React from 'react';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { makeMockNotesBridge, Note } from './notesBridge';
import {
  IMRAD_SCAFFOLD, newManuscript, manuscriptToDraft, manuscriptFromNote, wordCount,
} from './manuscriptModel';
import { buildManuscriptDocx, segmentSectionBody, ImageResolver } from './manuscriptDocx';
import {
  switchScaffold, Scaffold, renameSection, addSection, deleteSection, moveSection, uniqueSectionKey, IMRAD_SCAFFOLD as GEN,
} from './manuscriptModel';
import ManuscriptEditor from './ManuscriptEditor';
import { readFileSync, existsSync } from 'fs';

// The real shipped scaffold catalog (read from disk — deterministic, no fetch).
const CATALOG = JSON.parse(readFileSync('public/manuscripts/scaffolds.json', 'utf8')) as {
  version: number; globalNotice: string; scaffolds: Scaffold[];
};

// Serve the real catalog to the async loader so component tests get all scaffolds
// (a bare fetch of a relative URL fails in jsdom → would fall back to generic only).
vi.mock('./manuscriptScaffolds', async (importOriginal) => {
  const actual = await importOriginal<typeof import('./manuscriptScaffolds')>();
  const fs = await import('fs');
  const cat = JSON.parse(fs.readFileSync('public/manuscripts/scaffolds.json', 'utf8'));
  return {
    ...actual,
    loadScaffoldCatalog: async () => cat,
    loadScaffolds: async () => cat.scaffolds,
  };
});

// A real 1x1 PNG — valid image bytes for the embed path.
const PNG_1x1 = Uint8Array.from(
  atob('iVBORw0KGgoAAAANSUhEUgAAAAgAAAAICAIAAABLbSncAAAAEUlEQVR42mPQ9qnDihiGlgQA2mE9QQx+vG8AAAAASUVORK5CYII='),
  (c) => c.charCodeAt(0),
);

afterEach(cleanup);

// Build a real stored Note from a manuscript via the mock bridge (create→get) —
// exercises the exact fields_json serialization the Tauri store would persist.
const storeManuscript = async (fill: (m: ReturnType<typeof newManuscript>) => void): Promise<Note> => {
  const m = newManuscript('m1');
  fill(m);
  const bridge = makeMockNotesBridge();
  await bridge.create(manuscriptToDraft(m));
  const note = await bridge.get('m1');
  if (!note) throw new Error('not stored');
  return note;
};

/* -------- (b) STORAGE ROUND-TRIP: N sections + metadata survive save→reopen -------- */
describe('storage round-trip (pin b)', () => {
  it('title, authors, scaffold, CSL style, and every section body reopen identically', async () => {
    const note = await storeManuscript((m) => {
      m.title = 'On Transformers';
      m.authors = 'Ada Lovelace, Alan Turing';
      m.cslStyleId = 'nature';
      m.sections.find((s) => s.key === 'abstract')!.body = 'We study attention.';
      m.sections.find((s) => s.key === 'methods')!.body = 'A double-blind trial.\n\nSecond paragraph.';
    });
    expect(note.note_type).toBe('manuscript');

    const back = manuscriptFromNote(note);
    expect(back.title).toBe('On Transformers');
    expect(back.authors).toBe('Ada Lovelace, Alan Turing');
    expect(back.scaffoldId).toBe(IMRAD_SCAFFOLD.id);
    expect(back.cslStyleId).toBe('nature');
    // Section ORDER preserved (scaffold order) + bodies mapped by key.
    expect(back.sections.map((s) => s.key)).toEqual(IMRAD_SCAFFOLD.sections.map((s) => s.key));
    expect(back.sections.find((s) => s.key === 'abstract')!.body).toBe('We study attention.');
    expect(back.sections.find((s) => s.key === 'methods')!.body).toBe('A double-blind trial.\n\nSecond paragraph.');
    // Untouched sections come back empty (never invented).
    expect(back.sections.find((s) => s.key === 'results')!.body).toBe('');
  });

  it('bodies map BY KEY, so section order/heading changes never misalign content', async () => {
    const note = await storeManuscript((m) => { m.sections.find((s) => s.key === 'conclusion')!.body = 'Done.'; });
    const back = manuscriptFromNote(note);
    expect(back.sections.find((s) => s.key === 'conclusion')!.body).toBe('Done.');
  });
});

/* -------- (a) BACKWARD COMPAT: 'manuscript' is additive; other types untouched -------- */
describe('backward compat (pin a)', () => {
  it('adding manuscripts does not disturb project/paper listing or counts', async () => {
    const bridge = makeMockNotesBridge();
    await bridge.create({ id: 'p1', note_type: 'project', title: 'Idea', body: 'a thought' });
    await bridge.create({ id: 'pa1', note_type: 'paper', title: 'Paper note', paper_title: 'X' });
    await bridge.create(manuscriptToDraft(newManuscript('m1')));

    expect((await bridge.list('project')).length).toBe(1);
    expect((await bridge.list('paper')).length).toBe(1);
    expect((await bridge.list('manuscript')).length).toBe(1);
    // A project note's body is unchanged by the manuscript machinery.
    expect((await bridge.get('p1'))!.body).toBe('a thought');
  });

  it('manuscriptFromNote/ToDraft never touch a non-manuscript note', async () => {
    const project: Note = {
      id: 'p1', note_type: 'project', paper_id: null, paper_title: '', title: 'Idea',
      fields_json: '{}', body: 'original body', tags: [], sync_status: 'local_only', created_at: 1, updated_at: 1,
    };
    // Reading a project note as a manuscript yields empty sections (safe), and
    // does NOT mutate the source note object.
    const asMs = manuscriptFromNote(project);
    expect(asMs.sections.every((s) => s.body === '')).toBe(true);
    expect(project.body).toBe('original body');
  });
});

/* -------- (c) RICHBODY REUSE with STABLE keys: no content bleed on switch -------- */
describe('per-section RichBody with stable keys (pin c)', () => {
  it('switching sections shows each section its OWN content (stable-key remount)', async () => {
    const note = await storeManuscript((m) => {
      m.title = 'T';
      m.sections.find((s) => s.key === 'abstract')!.body = 'ABSTRACT_CONTENT';
      m.sections.find((s) => s.key === 'methods')!.body = 'METHODS_CONTENT';
    });
    render(<ManuscriptEditor id="m1" existing={note} onSave={() => {}} onClose={() => {}} />);

    // Abstract is active first (first scaffold section) — its body shows.
    await waitFor(() => expect(screen.getByTestId('ms-body-abstract').textContent).toContain('ABSTRACT_CONTENT'));

    // Switch to Methods → its own content, NOT the abstract's.
    fireEvent.click(screen.getByTestId('ms-nav-methods'));
    await waitFor(() => expect(screen.getByTestId('ms-body-methods').textContent).toContain('METHODS_CONTENT'));
    expect(screen.getByTestId('ms-body-methods').textContent).not.toContain('ABSTRACT_CONTENT');

    // Switch back → abstract content intact (no bleed).
    fireEvent.click(screen.getByTestId('ms-nav-abstract'));
    await waitFor(() => expect(screen.getByTestId('ms-body-abstract').textContent).toContain('ABSTRACT_CONTENT'));
  });

  it('per-section word counts reflect each section body', () => {
    expect(wordCount('one two three')).toBe(3);
    expect(wordCount('   ')).toBe(0);
    expect(wordCount('')).toBe(0);
  });

  it('a NEW manuscript opens the picker (honest notice + unofficial badges + verify links), then the editor', async () => {
    render(<ManuscriptEditor id="m1" existing={null} onSave={() => {}} onClose={() => {}} />);
    // Picker first — the persistent honesty notice + per-scaffold honesty.
    await waitFor(() => expect(screen.getByTestId('sp-notice')).toBeTruthy());
    expect(screen.getByTestId('sp-notice').textContent).toMatch(/not official publisher templates/i);
    expect(screen.getByTestId('sp-unofficial-ieee')).toBeTruthy();
    expect(screen.getByTestId('sp-verify-ieee').getAttribute('href')).toMatch(/ieeeauthorcenter\.ieee\.org/);
    // Pick the generic → the editing surface with its honest structure banner.
    fireEvent.click(screen.getByTestId('sp-pick-imrad-generic'));
    await waitFor(() => expect(screen.getByTestId('ms-notice')).toBeTruthy());
    expect(screen.getByTestId('ms-notice').textContent).toMatch(/Generic IMRaD/);
    expect(screen.getByTestId('ms-notice').textContent).toMatch(/publisher typesets/i);
    expect(screen.getByTestId('ms-export').getAttribute('title')).toMatch(/single-column submission/i);
    // ...and the .tex button names itself a reading layout, not a submission file
    expect(screen.getByTestId('ms-export-tex').getAttribute('title')).toMatch(/reading layout/i);
    expect(screen.getByTestId('ms-export-tex').getAttribute('title')).toMatch(/not a submission/i);
  });
});

/* ------------------------------ Set C pins ------------------------------ */
describe('scaffold catalog honesty (Set C)', () => {
  it('every scaffold is unofficial + has a publisher author URL + notice text + default style', () => {
    expect(CATALOG.scaffolds.length).toBeGreaterThanOrEqual(11);
    for (const s of CATALOG.scaffolds) {
      expect(s.unofficial).toBe(true);
      expect(s.publisherAuthorUrl).toMatch(/^https:\/\//);
      expect(s.noticeText.trim().length).toBeGreaterThan(0);
      expect(s.cslDefaultStyle.trim().length).toBeGreaterThan(0);
      expect(s.sections.length).toBeGreaterThan(0);
    }
  });

  it('non-generic scaffolds are labelled "…-style (unofficial)" — never "template"/"official"', () => {
    for (const s of CATALOG.scaffolds.filter((x) => x.id !== 'imrad-generic')) {
      expect(s.label).toMatch(/-style \(unofficial\)$/); // marked unofficial, never "template"/"official"
      expect(s.label.toLowerCase()).not.toMatch(/template/);
      expect(s.label.toLowerCase()).not.toMatch(/\bofficial\b/); // "unofficial" is fine; a bare "official" claim is not
    }
  });

  it('no link carries a utm_source tracking param', () => {
    for (const s of CATALOG.scaffolds) expect(s.publisherAuthorUrl).not.toMatch(/utm_source/);
  });

  it('EVERY scaffold\'s cslDefaultStyle EXISTS in the bundled CSL corpus (no missing style)', () => {
    for (const s of CATALOG.scaffolds) {
      expect(existsSync(`public/csl/styles/${s.cslDefaultStyle}.csl`)).toBe(true);
    }
  });
});

describe('per-venue docxFormat profiles (Set C+)', () => {
  const isZip = async (blob: Blob) => { const b = new Uint8Array(await blob.arrayBuffer()); return b[0] === 0x50 && b[1] === 0x4b; };
  const byId = (id: string) => CATALOG.scaffolds.find((s) => s.id === id)!;

  it('every scaffold has a valid docxFormat with an honest source note', () => {
    for (const s of CATALOG.scaffolds) {
      const f = s.docxFormat!;
      expect(f).toBeTruthy();
      expect(['single', '1.5', 'double']).toContain(f.lineSpacing);
      expect(['letter', 'a4']).toContain(f.pageSize);
      expect(['none', 'decimal', 'roman-upper']).toContain(f.sectionNumbering);
      expect(f.font.trim().length).toBeGreaterThan(0);
      expect(f.fontSizePt).toBeGreaterThan(0);
      expect(f.marginInch).toBeGreaterThan(0);
      expect(typeof f.lineNumbers).toBe('boolean');
      expect((f.source ?? '').trim().length).toBeGreaterThan(0); // grounded or marked a Gaply default
    }
  });

  it('the honesty stays intact: no docxFormat implies camera-ready/two-column reproduction', () => {
    // IEEE's true format is two-column — the source must say we do NOT reproduce it.
    expect(byId('ieee').docxFormat!.source!.toLowerCase()).toMatch(/not reproduced|two-column/);
    for (const s of CATALOG.scaffolds) {
      expect(s.docxFormat!.source!.toLowerCase()).not.toMatch(/camera-ready reproduction|official template/);
    }
  });

  it('PLOS profile reflects its fetched public guideline (double-spaced + continuous line numbers)', () => {
    const f = byId('plos').docxFormat!;
    expect(f.lineSpacing).toBe('double');
    expect(f.lineNumbers).toBe(true);
    expect(f.source!.toLowerCase()).toMatch(/plos/);
  });

  it('buildManuscriptDocx applies a profile and stays a valid .docx (IEEE letter/no-lines, Nature A4/lines)', async () => {
    const mk = (id: string) => {
      const sc = byId(id); const m = newManuscript('m1', sc);
      m.title = 'T'; m.sections.find((s) => s.key === 'introduction')!.body = 'Intro.';
      return { m, f: sc.docxFormat };
    };
    const ie = mk('ieee'); const na = mk('nature');
    expect(await isZip(await buildManuscriptDocx(ie.m, ie.f))).toBe(true);
    expect(await isZip(await buildManuscriptDocx(na.m, na.f))).toBe(true);
  });
});

describe('scaffold switching never silently drops content (Set C)', () => {
  const byId = (id: string) => CATALOG.scaffolds.find((s) => s.id === id)!;

  it('shared-key content is preserved; venue-absent sections are reported as dropped', () => {
    const generic = byId('imrad-generic');
    const nature = byId('nature'); // has no "keywords" or "conclusion" section
    let m = { id: 'm1', title: 'T', authors: '', affiliations: '', correspondingAuthor: '', scaffoldId: generic.id, cslStyleId: generic.cslDefaultStyle,
      sections: generic.sections.map((s) => ({ ...s, body: '' })) };
    m.sections.find((s) => s.key === 'introduction')!.body = 'INTRO';
    m.sections.find((s) => s.key === 'conclusion')!.body = 'CONCLUSION_TEXT';

    const { manuscript: switched, dropped } = switchScaffold(m, nature);
    // Introduction (shared key) carried over…
    expect(switched.sections.find((s) => s.key === 'introduction')!.body).toBe('INTRO');
    // …Conclusion (absent in Nature) reported as dropped, never silently lost.
    expect(dropped.map((d) => d.heading)).toContain('Conclusion');
    // Venue switch adopts the new default CSL style.
    expect(switched.cslStyleId).toBe(nature.cslDefaultStyle);
    expect(switched.scaffoldId).toBe('nature');
  });

  it('switching to a superset structure drops nothing', () => {
    const generic = byId('imrad-generic');
    const ieee = byId('ieee'); // has all generic keys + extras
    const m = { id: 'm1', title: 'T', authors: '', affiliations: '', correspondingAuthor: '', scaffoldId: generic.id, cslStyleId: 'apa',
      sections: generic.sections.map((s) => ({ ...s, body: s.key === 'methods' ? 'M' : '' })) };
    const { dropped } = switchScaffold(m, ieee);
    expect(dropped).toEqual([]);
  });

  it('the editor WARNS before dropping content on a structure switch, and preserves on cancel', async () => {
    // Start an existing generic manuscript with content in a section Nature lacks.
    const note = await storeManuscript((mm) => {
      mm.title = 'T';
      mm.sections.find((s) => s.key === 'conclusion')!.body = 'MY_CONCLUSION';
    });
    render(<ManuscriptEditor id="m1" existing={note} onSave={() => {}} onClose={() => {}} />);
    await waitFor(() => expect(screen.getByTestId('ms-change-structure')).toBeTruthy());

    fireEvent.click(screen.getByTestId('ms-change-structure'));         // open picker (switch)
    fireEvent.click(await screen.findByTestId('sp-pick-nature'));       // pick Nature (drops Conclusion)
    // A warning appears listing the dropped section — not a silent drop.
    await waitFor(() => expect(screen.getByTestId('ms-dropwarn').textContent).toMatch(/Conclusion/));
    // Cancel keeps the current structure (still generic, content intact).
    fireEvent.click(screen.getByTestId('ms-dropwarn-cancel'));
    fireEvent.click(screen.getByTestId('sp-cancel'));
    await waitFor(() => expect(screen.getByTestId('ms-nav-conclusion')).toBeTruthy());
  });
});

/* -------- editable sections: rename / add / delete / reorder (new feature) -------- */
describe('custom section editing helpers', () => {
  const base = () => { const m = newManuscript('m1'); m.sections.find((s) => s.key === 'methods')!.body = 'M-body'; return m; };

  it('rename keeps the key + body, changes only the heading', () => {
    const m = renameSection(base(), 'methods', 'Materials and Methods');
    const s = m.sections.find((x) => x.key === 'methods')!;
    expect(s.heading).toBe('Materials and Methods');
    expect(s.body).toBe('M-body');
  });

  it('addSection inserts a NEW custom section with a unique key at the index', () => {
    const m = addSection(base(), 2, 'Data Availability');
    expect(m.sections[2].heading).toBe('Data Availability');
    expect(m.sections[2].key).toBe('data-availability');
    expect(m.sections.length).toBe(base().sections.length + 1);
  });

  it('uniqueSectionKey disambiguates against existing keys', () => {
    expect(uniqueSectionKey('Methods', ['methods'])).toBe('methods-2');
    expect(uniqueSectionKey('New Section!', [])).toBe('new-section');
    expect(uniqueSectionKey('', [])).toBe('section');
  });

  it('deleteSection removes by key; moveSection swaps neighbors and no-ops at the ends', () => {
    expect(deleteSection(base(), 'methods').sections.some((s) => s.key === 'methods')).toBe(false);
    const m = base();
    const keys = m.sections.map((s) => s.key);
    const down = moveSection(m, keys[1], 1);
    expect(down.sections[1].key).toBe(keys[2]); // swapped with its neighbour
    expect(moveSection(m, keys[0], -1)).toBe(m); // no-op at the top
  });
});

describe('custom structure round-trips + backward compat', () => {
  it('renamed + added + reordered sections survive save→reopen (headings, order, keys, bodies)', async () => {
    let m = newManuscript('m1');
    m.title = 'T';
    m = renameSection(m, 'methods', 'Materials and Methods');
    m = addSection(m, 2, 'Data Availability');
    const dataKey = m.sections[2].key;
    m.sections.find((s) => s.key === dataKey)!.body = 'All data public.';
    m = moveSection(m, 'conclusion', -1);

    const bridge = makeMockNotesBridge();
    await bridge.create(manuscriptToDraft(m));
    const note = (await bridge.get('m1'))!;
    const back = manuscriptFromNote(note, GEN);

    expect(back.sections.map((s) => s.key)).toEqual(m.sections.map((s) => s.key));       // order + keys
    expect(back.sections.map((s) => s.heading)).toEqual(m.sections.map((s) => s.heading)); // renames persist
    expect(back.sections.find((s) => s.key === dataKey)!.body).toBe('All data public.');
    expect(back.sections.find((s) => s.key === 'methods')!.heading).toBe('Materials and Methods');
  });

  it('a legacy manuscript stored WITHOUT section headings reopens with scaffold headings', () => {
    const legacy: Note = {
      id: 'm1', note_type: 'manuscript', paper_id: null, paper_title: '', title: 'Old',
      fields_json: JSON.stringify({ scaffoldId: 'imrad-generic', sections: [{ key: 'introduction', body: 'Intro' }, { key: 'methods', body: 'M' }] }),
      body: '', tags: [], sync_status: 'local_only', created_at: 1, updated_at: 1,
    };
    const back = manuscriptFromNote(legacy, GEN);
    expect(back.sections.find((s) => s.key === 'methods')!.heading).toBe('Methods');   // derived
    expect(back.sections.find((s) => s.key === 'introduction')!.body).toBe('Intro');
  });
});

describe('editable sections in the editor never silently drop content', () => {
  it('rename a heading via the inline control', async () => {
    const note = await storeManuscript((m) => { m.title = 'T'; });
    render(<ManuscriptEditor id="m1" existing={note} onSave={() => {}} onClose={() => {}} />);
    await waitFor(() => expect(screen.getByTestId('ms-nav-abstract')).toBeTruthy()); // abstract active
    fireEvent.click(screen.getByTestId('ms-rename-abstract'));
    const input = screen.getByTestId('ms-rename-input-abstract') as HTMLInputElement;
    fireEvent.change(input, { target: { value: 'Summary' } });
    fireEvent.keyDown(input, { key: 'Enter' });
    await waitFor(() => expect(screen.getByTestId('ms-nav-abstract').textContent).toContain('Summary'));
  });

  it('add a section', async () => {
    const note = await storeManuscript((m) => { m.title = 'T'; });
    render(<ManuscriptEditor id="m1" existing={note} onSave={() => {}} onClose={() => {}} />);
    await waitFor(() => expect(screen.getByTestId('ms-add-section')).toBeTruthy());
    const before = screen.getByTestId('ms-nav').querySelectorAll('[data-testid^="ms-nav-"]').length;
    fireEvent.click(screen.getByTestId('ms-add-section'));
    await waitFor(() => expect(screen.getByTestId('ms-nav').querySelectorAll('[data-testid^="ms-nav-"], [data-testid^="ms-rename-input-"]').length).toBe(before + 1));
  });

  it('deleting a NON-EMPTY section warns; cancel preserves it', async () => {
    const note = await storeManuscript((m) => { m.title = 'T'; m.sections.find((s) => s.key === 'abstract')!.body = 'ABS'; });
    render(<ManuscriptEditor id="m1" existing={note} onSave={() => {}} onClose={() => {}} />);
    await waitFor(() => expect(screen.getByTestId('ms-del-abstract')).toBeTruthy()); // abstract active
    fireEvent.click(screen.getByTestId('ms-del-abstract'));
    await waitFor(() => expect(screen.getByTestId('ms-deletewarn').textContent).toMatch(/Abstract/));
    fireEvent.click(screen.getByTestId('ms-deletewarn-cancel'));
    await waitFor(() => expect(screen.getByTestId('ms-nav-abstract')).toBeTruthy()); // still there
  });

  it('shows the honest camera-ready note pointing to the publisher template / Overleaf', async () => {
    const note = await storeManuscript((m) => { m.title = 'T'; });
    render(<ManuscriptEditor id="m1" existing={note} onSave={() => {}} onClose={() => {}} />);
    await waitFor(() => expect(screen.getByTestId('ms-export-note')).toBeTruthy());
    expect(screen.getByTestId('ms-export-note').textContent).toMatch(/camera-ready.*publisher.*template|Overleaf/i);
  });
});

/* -------- (d)+(e) DOCX EXPORT builds a valid zip, incl. empty/partial -------- */
describe('docx export builds a valid .docx (pins d, e)', () => {
  const isZip = async (blob: Blob) => {
    const b = new Uint8Array(await blob.arrayBuffer());
    return b[0] === 0x50 && b[1] === 0x4b; // "PK" — the ZIP/OOXML magic
  };

  it('a filled manuscript exports a valid, non-empty .docx (pin d)', async () => {
    const m = newManuscript('m1');
    m.title = 'Paper';
    m.authors = 'A. Author';
    m.sections.find((s) => s.key === 'introduction')!.body = 'Intro paragraph.\n\nSecond.';
    const blob = await buildManuscriptDocx(m);
    expect(blob.size).toBeGreaterThan(0);
    expect(await isZip(blob)).toBe(true);
  });

  it('an ALL-EMPTY manuscript still exports a valid .docx (title page only) — no throw (pin e)', async () => {
    const blob = await buildManuscriptDocx(newManuscript('m1'));
    expect(blob.size).toBeGreaterThan(0);
    expect(await isZip(blob)).toBe(true);
  });

  it('a partial manuscript (some sections empty) exports without throwing (pin e)', async () => {
    const m = newManuscript('m1');
    m.title = 'Partial';
    m.sections.find((s) => s.key === 'results')!.body = 'Only results filled.';
    await expect(buildManuscriptDocx(m)).resolves.toBeInstanceOf(Blob);
  });
});

/* -------- image embedding: the .docx-corruption fix (pins d + honest fallback) -------- */
describe('pasted images embed into the .docx, never corrupt it', () => {
  const isZip = async (blob: Blob) => {
    const b = new Uint8Array(await blob.arrayBuffer());
    return b[0] === 0x50 && b[1] === 0x4b;
  };
  const imgBody = 'See the figure:\n\n![](gaply-image://deadbeefcafe0123.png)\n\nDiscussion.';

  it('segmentSectionBody splits text and image refs in order (text/image/text)', () => {
    const segs = segmentSectionBody(imgBody);
    expect(segs).toEqual([
      { text: 'See the figure:\n\n' },
      { ref: 'gaply-image://deadbeefcafe0123.png' },
      { text: '\n\nDiscussion.' },
    ]);
    expect(segmentSectionBody('plain text, no images')).toEqual([{ text: 'plain text, no images' }]);
    expect(segmentSectionBody('![](gaply-image://x.png)')).toEqual([{ ref: 'gaply-image://x.png' }]);
  });

  it('an image resolved to REAL bytes embeds via ImageRun → valid .docx (pin d)', async () => {
    const m = newManuscript('m1');
    m.title = 'With figure';
    m.sections.find((s) => s.key === 'results')!.body = imgBody;
    const resolver: ImageResolver = async () => ({ data: PNG_1x1, width: 640, height: 480 });
    const blob = await buildManuscriptDocx(m, undefined, resolver);
    expect(await isZip(blob)).toBe(true);
    // Dump for the headless media/r:embed verification (Bash step).
    const { writeFileSync } = await import('fs');
    writeFileSync('/tmp/ms_embed.docx', new Uint8Array(await blob.arrayBuffer()));
  });

  it('an UNRESOLVABLE image degrades to an honest placeholder — valid .docx, never corrupt', async () => {
    const m = newManuscript('m1');
    m.title = 'Missing figure';
    m.sections.find((s) => s.key === 'results')!.body = imgBody;
    const resolver: ImageResolver = async () => null; // bytes unavailable
    const blob = await buildManuscriptDocx(m, undefined, resolver);
    expect(await isZip(blob)).toBe(true); // clean docx, no dangling image relationship
  });

  it('the default resolver failing (no Tauri fs in tests) still yields a valid .docx', async () => {
    const m = newManuscript('m1');
    m.sections.find((s) => s.key === 'results')!.body = imgBody;
    await expect(buildManuscriptDocx(m)).resolves.toBeInstanceOf(Blob); // default resolver → null → placeholder
  });
});
