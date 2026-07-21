// Rich editor Set 1 — the pins. The body is CANONICAL MARKDOWN; TipTap is a
// view over it. Headless pins use richExtensions() — the PRODUCTION config —
// so what passes here is what ships. No network, no models.
import React from 'react';
import { cleanup, fireEvent, render, screen, waitFor, act } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { Editor } from '@tiptap/core';

import { richExtensions, mdOf, pickPastedImage, isFileDrag, imageDropHint } from './RichBody';
import ProjectNoteEditor from './ProjectNoteEditor';
import { noteToMarkdown } from './noteExport';
import { imageRefsIn } from './noteImages';
import { Note } from './notesBridge';

afterEach(cleanup);

const headless = (md = '') => {
  const e = new Editor({ extensions: richExtensions() });
  if (md) e.commands.setContent(md); // tiptap-markdown parses md strings
  return e;
};

/* ------------------- (b) round-trip idempotence (pure) ------------------- */
describe('md round-trip — idempotence (pin b)', () => {
  const CASES = [
    'Sleep helps recall.',
    'line one\nline two\nline three',
    '# Heading\n\nbody text',
    '- alpha\n- beta',
    '> a quote',
    '**bold** and *italic*',
    'para one\n\npara two',
    '5 * 3 = 15 and a_var too',
  ];
  it('serialize(parse(x)) is stable: another parse/serialize pass changes nothing', () => {
    for (const x of CASES) {
      const e1 = headless(x);
      const md1 = mdOf(e1);
      e1.destroy();
      const e2 = headless(md1);
      const md2 = mdOf(e2);
      e2.destroy();
      expect(md2).toBe(md1);
    }
  });
  it('plain text and structure round-trip byte-stable (incl. multiline via hardBreak→\\n)', () => {
    for (const x of ['Sleep helps recall.', 'line one\nline two\nline three', '# Heading\n\nbody text', '- alpha\n- beta', '> a quote']) {
      const e = headless(x);
      expect(mdOf(e)).toBe(x);
      e.destroy();
    }
  });
});

/* --------------- (d) input rules live in the PRODUCTION config ----------- */
describe('input rules in production (pin d)', () => {
  const typeCharThenSpace = (ch: string) => {
    const e = headless();
    e.view.dispatch(e.state.tr.insertText(ch, 1));
    const pos = e.state.selection.from;
    // Simulate the space keystroke through ProseMirror's text-input path —
    // the same hook the real WKWebView typing goes through (spike-proven).
    e.view.someProp('handleTextInput', (f) => (f as (...a: unknown[]) => boolean)(e.view, pos, pos, ' '));
    const first = e.state.doc.firstChild;
    const name = first?.type.name;
    e.destroy();
    return name;
  };
  it('"- " at line start becomes a bullet list', () => {
    expect(typeCharThenSpace('-')).toBe('bulletList');
  });
  it('"# " at line start becomes a heading', () => {
    expect(typeCharThenSpace('#')).toBe('heading');
  });
  it('"> " at line start becomes a blockquote', () => {
    expect(typeCharThenSpace('>')).toBe('blockquote');
  });
});

/* ------------------------ component-level pins --------------------------- */
const existingNote = (body: string): Note => ({
  id: 'j1', note_type: 'project', paper_id: null, paper_title: '', title: 'Thesis idea',
  fields_json: '{}', body, tags: ['idea'], sync_status: 'local_only', created_at: 1, updated_at: 1,
});

describe('backward compat — open without editing never rewrites (pin a)', () => {
  it('save-after-open sends the EXACT original bytes (multiline + md specials)', async () => {
    const original = 'line one\nline two\n\n5 * 3 = 15 and a_var';
    const onSave = vi.fn();
    render(<ProjectNoteEditor id="j1" existing={existingNote(original)} onSave={onSave} onClose={() => {}} />);
    // wait for the editor to mount (content rendered)
    await waitFor(() => expect(screen.getByTestId('project-body').textContent).toContain('line one'));
    fireEvent.click(screen.getByTestId('project-save'));
    expect(onSave).toHaveBeenCalledTimes(1);
    expect(onSave.mock.calls[0][0].body).toBe(original); // byte-identical — no silent rewrite
  });

  it('editing appends cleanly — original content survives the edit (headless, production config)', () => {
    const original = 'line one\nline two';
    const e = headless(original);
    e.commands.focus('end');
    e.view.dispatch(e.state.tr.insertText(' — edited', e.state.selection.from));
    const out = mdOf(e);
    e.destroy();
    expect(out).toBe('line one\nline two — edited'); // nothing lost, clean md
  });
});

describe('anti-substitution attributes on the production editor (pin c)', () => {
  it('.ProseMirror carries autocorrect/autocapitalize/spellcheck off', async () => {
    render(<ProjectNoteEditor id="j1" existing={existingNote('x')} onSave={() => {}} onClose={() => {}} />);
    await waitFor(() => expect(document.querySelector('.an-richbody .ProseMirror')).not.toBeNull());
    const pm = document.querySelector('.an-richbody .ProseMirror') as HTMLElement;
    expect(pm.getAttribute('autocorrect')).toBe('off');
    expect(pm.getAttribute('autocapitalize')).toBe('off');
    expect(pm.getAttribute('spellcheck')).toBe('false');
  });
});

/* -------------------------- Set 2: GFM tables --------------------------- */
describe('GFM tables round-trip losslessly (Set 2 key pin)', () => {
  const GFM = '| A | B |\n| --- | --- |\n| 1 | 2 |\n| 3 | 4 |';

  it('a table typed/parsed → md → reopened → IDENTICAL table', () => {
    const e1 = headless(GFM);
    expect(JSON.stringify(e1.getJSON())).toContain('"type":"table"'); // really a table node
    const md1 = mdOf(e1);
    e1.destroy();
    expect(md1).toBe(GFM); // serialize back = byte-identical

    const e2 = headless(md1); // reopen
    const md2 = mdOf(e2);
    e2.destroy();
    expect(md2).toBe(GFM); // round-trip stable
  });

  it('programmatic insertTable → serializes to a valid GFM table', () => {
    const e = new Editor({ extensions: richExtensions() });
    e.commands.insertTable({ rows: 2, cols: 2, withHeaderRow: true });
    const md = mdOf(e);
    e.destroy();
    // header row + separator row + one body row, all pipe-delimited
    const lines = md.trim().split('\n');
    expect(lines.length).toBe(3);
    expect(lines[1]).toMatch(/^\|\s*---\s*\|\s*---\s*\|$/);
    expect(lines.every((l) => l.startsWith('|') && l.endsWith('|'))).toBe(true);
  });

  it('idempotence with a table present: s(p(s(p(x)))) == s(p(x))', () => {
    const doc = `Intro paragraph.\n\n${GFM}\n\nOutro paragraph.`;
    const a = mdOf(headless(doc));
    const b = mdOf(headless(a));
    expect(b).toBe(a);
  });

  it('BACKWARD COMPAT: a note with NO table is untouched by the table extension', () => {
    for (const x of ['Sleep helps recall.', '# H\n\n- a\n- b', '> quote']) {
      const e = headless(x);
      expect(mdOf(e)).toBe(x); // table extension present, but plain notes unchanged
      e.destroy();
    }
  });
});

describe('export contract — a table body flows through noteToMarkdown as GFM (Set 2)', () => {
  it('noteToMarkdown embeds the GFM table verbatim', () => {
    const body = '| A | B |\n| --- | --- |\n| 1 | 2 |';
    const md = noteToMarkdown(
      { note_type: 'project', title: 'Data', paper_title: '', body, tags: [] },
      {},
    );
    expect(md).toBe(`# Data\n\n${body}`); // the export contract carries the table unchanged
  });
});

describe('export contract unchanged (pin e)', () => {
  it('typed plain text serializes to itself, and noteToMarkdown output is the pre-swap shape', () => {
    const e = headless();
    e.view.dispatch(e.state.tr.insertText('Sleep helps recall.', 1));
    const body = mdOf(e);
    e.destroy();
    expect(body).toBe('Sleep helps recall.');
    const md = noteToMarkdown(
      { note_type: 'project', title: 'Idea', paper_title: '', body, tags: ['x'] },
      {},
    );
    expect(md).toBe('# Idea\n\nSleep helps recall.\n\n_Tags: x_');
  });
});

/* -------------------------- Set 3: pasted images ------------------------ */
describe('image ref round-trips through markdown-canonical storage (Set 3, pin b)', () => {
  const REF = 'gaply-image://abc123def.png';

  it('a body with an image parses to an image node → serializes back with the ref → idempotent reopen', () => {
    const body = `Notes before.\n\n![](${REF})\n\nNotes after.`;
    const e1 = headless(body);
    expect(JSON.stringify(e1.getJSON())).toContain('"type":"image"'); // really an image node
    const md1 = mdOf(e1);
    e1.destroy();
    expect(md1).toContain(REF); // the canonical ref survives serialization (persistence)

    const e2 = headless(md1); // reopen the stored body
    const md2 = mdOf(e2);
    e2.destroy();
    expect(md2).toBe(md1); // reopen is stable — the image persists across save/open
    expect(imageRefsIn(md2)).toContain(REF); // GC + persistence can still find it
  });

  it('BACKWARD COMPAT: the image extension leaves an image-FREE note byte-identical', () => {
    for (const x of ['Sleep helps recall.', '# H\n\n- a\n- b', '> quote', 'line one\nline two']) {
      const e = headless(x);
      expect(mdOf(e)).toBe(x);
      e.destroy();
    }
  });
});

describe('paste/drop plumbing is honest (Set 3, pin e)', () => {
  it('pickPastedImage returns the clipboard image FILE (free path), else null', () => {
    const imgFile = { type: 'image/png', size: 10 } as File;
    const withImg = { items: [{ kind: 'file', type: 'image/png', getAsFile: () => imgFile }] } as unknown as DataTransfer;
    expect(pickPastedImage(withImg)).toBe(imgFile);
    const textOnly = { items: [{ kind: 'string', type: 'text/plain', getAsFile: () => null }] } as unknown as DataTransfer;
    expect(pickPastedImage(textOnly)).toBeNull();
    expect(pickPastedImage(null)).toBeNull();
  });

  it('isFileDrag detects a file drag so we can HINT, not silently swallow it', () => {
    expect(isFileDrag({ types: ['Files'] } as unknown as DataTransfer)).toBe(true);
    expect(isFileDrag({ types: ['text/plain'] } as unknown as DataTransfer)).toBe(false);
    expect(isFileDrag(null)).toBe(false);
  });

  it('imageDropHint gives the honest ⌘V message on a file drag, null otherwise (the exact wiring handleDrop uses)', () => {
    expect(imageDropHint({ types: ['Files'] } as unknown as DataTransfer)).toMatch(/paste images with ⌘V/i);
    expect(imageDropHint({ types: ['text/plain'] } as unknown as DataTransfer)).toBeNull();
    expect(imageDropHint(null)).toBeNull();
  });
});

describe('two-step delete (pin f)', () => {
  it('first click arms, 3s disarms, second click within the window deletes', async () => {
    vi.useFakeTimers();
    try {
      const onDelete = vi.fn();
      render(<ProjectNoteEditor id="j1" existing={existingNote('x')} onSave={() => {}} onDelete={onDelete} onClose={() => {}} />);
      const btn = screen.getByTestId('project-delete');

      fireEvent.click(btn); // arm
      expect(onDelete).not.toHaveBeenCalled();
      expect(btn.textContent).toMatch(/Really delete\?/);

      act(() => { vi.advanceTimersByTime(3100); }); // timeout disarms
      expect(screen.getByTestId('project-delete').textContent).not.toMatch(/Really delete\?/);

      fireEvent.click(screen.getByTestId('project-delete')); // re-arm
      expect(onDelete).not.toHaveBeenCalled();
      fireEvent.click(screen.getByTestId('project-delete')); // second click within window
      expect(onDelete).toHaveBeenCalledTimes(1);
    } finally {
      vi.useRealTimers();
    }
  });
});
