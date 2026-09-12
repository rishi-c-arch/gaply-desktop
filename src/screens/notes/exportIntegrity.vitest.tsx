// Research Paper Writer — what the .docx actually contains.
//
// The existing export pins prove the file is a valid zip and that each markdown
// construct becomes the right docx ELEMENT. These pin the three ways the export
// silently disagreed with the editor:
//
//   1. a dangling-only bibliography threw away the manual References text too,
//      so a manuscript exported with NO reference list and nothing said so;
//   2. a single newline became a space, collapsing stanzas and line-per-item
//      lists, because the editor parses with breaks:true and the exporter didn't;
//   3. `[text](url)` dropped the url entirely.
//
// Plus the gate: a dangling citation writes "[?]" into a file destined for a
// journal, so the editor now stops and names the sections to fix.
import React from 'react';
import { readFileSync, writeFileSync, mkdtempSync } from 'fs';
import { execFileSync } from 'child_process';
import { tmpdir } from 'os';
import { join } from 'path';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, beforeAll, describe, expect, it, vi } from 'vitest';

// The .docx write itself is the shell's job (native dialog + fs); the gate is
// what's under test, so capture the call instead of touching the disk.
const saveManuscriptDocx = vi.fn(async () => '/tmp/out.docx');
vi.mock('./manuscriptDocx', async (importOriginal) => {
  const actual = await importOriginal<typeof import('./manuscriptDocx')>();
  return { ...actual, saveManuscriptDocx: (...a: unknown[]) => saveManuscriptDocx(...(a as [])) };
});

vi.mock('./manuscriptScaffolds', async (importOriginal) => {
  const actual = await importOriginal<typeof import('./manuscriptScaffolds')>();
  const fs = await import('fs');
  const cat = JSON.parse(fs.readFileSync('public/manuscripts/scaffolds.json', 'utf8'));
  return { ...actual, loadScaffoldCatalog: async () => cat, loadScaffolds: async () => cat.scaffolds };
});

import { ensureCslEngine, registerStyleXml, registerLocaleXml } from '../citations/cslEngine';
import { CslItem } from '../citations/citationTypes';
import { renderManuscriptCitations } from './manuscriptCitations';
import { buildManuscriptDocx } from './manuscriptDocx';
import { newManuscript, Scaffold, Manuscript } from './manuscriptModel';
import ManuscriptEditor, { danglingBySection } from './ManuscriptEditor';
import { Note } from './notesBridge';
import { makeMockLocalLibrary } from '../citations/localLibrary';

const CAT = JSON.parse(readFileSync('public/manuscripts/scaffolds.json', 'utf8')) as { scaffolds: Scaffold[] };
const sc = (id: string) => CAT.scaffolds.find((s) => s.id === id)!;

const REF_A: CslItem = {
  id: 'refA', type: 'article-journal', title: 'Deep residual learning',
  author: [{ family: 'He' }], issued: { year: 2016 }, containerTitle: 'CVPR',
};

beforeAll(async () => {
  await ensureCslEngine();
  await registerLocaleXml('en-US', readFileSync('public/csl/locales/locales-en-US.xml', 'utf8'));
  await registerStyleXml('ieee', readFileSync('public/csl/styles/ieee.csl', 'utf8'));
});

afterEach(() => { cleanup(); saveManuscriptDocx.mockClear(); });

/** Build a real .docx and read back both the raw XML and its visible text. */
const build = async (bodies: Record<string, string>, library: Map<string, CslItem>) => {
  const m: Manuscript = newManuscript('m1', sc('ieee'));
  m.title = 'Probe';
  for (const [k, v] of Object.entries(bodies)) m.sections.find((s) => s.key === k)!.body = v;
  const cites = await renderManuscriptCitations(m.sections, 'ieee', library);
  const blob = await buildManuscriptDocx(m, sc('ieee').docxFormat, async () => null, cites);
  const dir = mkdtempSync(join(tmpdir(), 'x-'));
  writeFileSync(join(dir, 'o.docx'), new Uint8Array(await blob.arrayBuffer()));
  execFileSync('unzip', ['-o', '-q', join(dir, 'o.docx'), '-d', dir]);
  const xml = readFileSync(join(dir, 'word', 'document.xml'), 'utf8');
  return { cites, xml, text: xml.replace(/<\/w:p>/g, '\n').replace(/<[^>]+>/g, ''), dir };
};

/* ------------- 1. the References section is never silently lost ------------- */

describe('References never vanishes from the export', () => {
  const MANUAL = 'Smith, J. (2020). A manually typed reference entry. Journal of Things.';

  it('ALL citations dangling → the manual reference text is exported, not dropped', async () => {
    const { cites, text } = await build(
      { introduction: 'Prior work [[cite:deleted-ref]] said so.', references: MANUAL },
      new Map(), // the library no longer has it
    );
    // the precondition that used to trigger the bug
    expect(cites.hasCitations).toBe(true);
    expect(cites.bibliography).toBe('');
    expect(cites.dangling).toEqual(['deleted-ref']);

    // ...and the section survives with the author's own text
    expect(text).toMatch(/References/);
    expect(text).toContain('A manually typed reference entry');
  });

  it('citations that RESOLVE still auto-generate, and the manual text stays out of the way', async () => {
    const { cites, text } = await build(
      { introduction: 'Prior work [[cite:refA]] said so.', references: MANUAL },
      new Map([['refA', REF_A]]),
    );
    expect(cites.bibliography).not.toBe('');
    expect(text).toContain('Deep residual learning'); // the generated entry
    expect(text).not.toContain('A manually typed reference entry'); // superseded, as the banner says
    expect(text).toMatch(/Prior work \[1\]/);
  });

  it('an empty References body with all-dangling citations emits no bare heading', async () => {
    const { text } = await build({ introduction: 'Prior work [[cite:gone]].' }, new Map());
    // nothing to say → no orphan "References" heading with nothing under it
    expect(text).not.toMatch(/References/);
  });
});

/* ---------------- 2. the editor blocks a [?] export ---------------- */

describe('export gate — a dangling citation stops the write', () => {
  const manuscriptNote = (body: string): Note => ({
    id: 'm1', note_type: 'manuscript', paper_id: null, paper_title: '', title: 'A paper',
    fields_json: JSON.stringify({
      scaffoldId: 'ieee', cslStyleId: 'ieee',
      sections: [{ key: 'introduction', heading: 'Introduction', body }],
    }),
    body, tags: [], sync_status: 'local_only', created_at: 1771065000, updated_at: 1771065000,
  });

  const open = async (body: string) => {
    render(
      <ManuscriptEditor
        id="m1"
        existing={manuscriptNote(body)}
        onSave={vi.fn()}
        onClose={vi.fn()}
        library={makeMockLocalLibrary()} // empty → every cited ref is dangling
      />
    );
    await screen.findByTestId('manuscript-editor');
  };

  it('warns instead of writing, and names the section to go and fix', async () => {
    await open('We build on [[cite:gone-1]] and [[cite:gone-2]].');
    fireEvent.click(screen.getByTestId('ms-export'));

    const warn = await screen.findByTestId('ms-danglingwarn');
    expect(warn.textContent).toMatch(/2 references you cite/);
    expect(warn.textContent).toMatch(/Introduction — 2 citations/);
    expect(saveManuscriptDocx).not.toHaveBeenCalled();
  });

  it('"Fix the references first" leaves the file unwritten', async () => {
    await open('We build on [[cite:gone-1]].');
    fireEvent.click(screen.getByTestId('ms-export'));
    fireEvent.click(await screen.findByTestId('ms-danglingwarn-cancel'));

    await waitFor(() => expect(screen.queryByTestId('ms-danglingwarn')).toBeNull());
    expect(saveManuscriptDocx).not.toHaveBeenCalled();
  });

  it('"Export anyway" is available and writes the file', async () => {
    await open('We build on [[cite:gone-1]].');
    fireEvent.click(screen.getByTestId('ms-export'));
    fireEvent.click(await screen.findByTestId('ms-danglingwarn-confirm'));

    await waitFor(() => expect(saveManuscriptDocx).toHaveBeenCalledTimes(1));
    expect(screen.queryByTestId('ms-danglingwarn')).toBeNull();
  });

  it('a manuscript with no dangling refs exports straight through — no gate', async () => {
    render(
      <ManuscriptEditor
        id="m1"
        existing={manuscriptNote('Nothing cited here at all.')}
        onSave={vi.fn()}
        onClose={vi.fn()}
        library={makeMockLocalLibrary()}
      />
    );
    await screen.findByTestId('manuscript-editor');
    fireEvent.click(screen.getByTestId('ms-export'));

    await waitFor(() => expect(saveManuscriptDocx).toHaveBeenCalledTimes(1));
    expect(screen.queryByTestId('ms-danglingwarn')).toBeNull();
  });

  it('danglingBySection counts per section and skips clean ones', () => {
    expect(danglingBySection(
      [
        { heading: 'Introduction', body: 'a [[cite:x]] b [[cite:y]]' },
        { heading: 'Methods', body: 'c [[cite:ok]]' },
        { heading: 'Results', body: 'no citations' },
      ],
      ['x', 'y'],
    )).toEqual([{ heading: 'Introduction', count: 2 }]);
  });
});

/* --------------- 3. the two parsers agree about the document --------------- */

describe('export matches what the editor stored', () => {
  it('single newlines stay separate lines (a real <w:br/>, not a space)', async () => {
    const { xml, text } = await build({ keywords: 'alpha\nbeta\ngamma' }, new Map());
    expect(xml).toContain('<w:br/>');
    // the three terms are no longer welded into one run of text
    expect(text).not.toContain('alpha beta gamma');
    for (const w of ['alpha', 'beta', 'gamma']) expect(text).toContain(w);
  });

  it('a blank line still starts a new paragraph (unchanged behaviour)', async () => {
    const { text } = await build({ introduction: 'First para.\n\nSecond para.' }, new Map());
    const lines = text.split('\n').map((l) => l.trim()).filter(Boolean);
    expect(lines).toContain('First para.');
    expect(lines).toContain('Second para.');
  });

  it('a link keeps its URL — as a real hyperlink AND in readable text', async () => {
    const { xml, text } = await build(
      { methods: 'Data are at [our dataset](https://example.org/data) in full.' },
      new Map(),
    );
    expect(text).toContain('our dataset');
    expect(text).toContain('https://example.org/data'); // visible on paper/PDF
    expect(xml).toContain('w:hyperlink');               // and clickable in Word
  });

  it('a bare URL as its own link text is not printed twice', async () => {
    const { text } = await build(
      { methods: 'See [https://example.org/data](https://example.org/data).' },
      new Map(),
    );
    expect(text.match(/https:\/\/example\.org\/data/g) ?? []).toHaveLength(1);
  });

  it('links inside formatting still resolve, and citations around them stay in order', async () => {
    const { text } = await build(
      { introduction: 'See **[the site](https://a.example) and [[cite:refA]]** next.' },
      new Map([['refA', REF_A]]),
    );
    expect(text).toContain('the site');
    expect(text).toContain('https://a.example');
    expect(text).toMatch(/\[1\]/); // the citation still numbered, not swallowed by the link
  });
});
