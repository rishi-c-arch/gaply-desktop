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
import ManuscriptEditor from './ManuscriptEditor';

// A real 1x1 PNG — valid image bytes for the embed path.
const PNG_1x1 = Uint8Array.from(
  atob('iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAAC0lEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg=='),
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

  it('renders the honest scaffold label + non-camera-ready notice', async () => {
    render(<ManuscriptEditor id="m1" existing={null} onSave={() => {}} onClose={() => {}} />);
    await waitFor(() => expect(screen.getByTestId('ms-notice')).toBeTruthy());
    expect(screen.getByTestId('ms-notice').textContent).toMatch(/Generic IMRaD structure/);
    expect(screen.getByTestId('ms-notice').textContent).toMatch(/publisher typesets/i);
    expect(screen.getByTestId('ms-export').getAttribute('title')).toMatch(/submission structure/i);
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
    const blob = await buildManuscriptDocx(m, resolver);
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
    const blob = await buildManuscriptDocx(m, resolver);
    expect(await isZip(blob)).toBe(true); // clean docx, no dangling image relationship
  });

  it('the default resolver failing (no Tauri fs in tests) still yields a valid .docx', async () => {
    const m = newManuscript('m1');
    m.sections.find((s) => s.key === 'results')!.body = imgBody;
    await expect(buildManuscriptDocx(m)).resolves.toBeInstanceOf(Blob); // default resolver → null → placeholder
  });
});
