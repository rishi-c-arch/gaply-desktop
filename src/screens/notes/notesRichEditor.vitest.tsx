// Rich editor Set 1 — the pins. The body is CANONICAL MARKDOWN; TipTap is a
// view over it. Headless pins use richExtensions() — the PRODUCTION config —
// so what passes here is what ships. No network, no models.
import React from 'react';
import { cleanup, fireEvent, render, screen, waitFor, act } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { Editor } from '@tiptap/core';

import { richExtensions, mdOf } from './RichBody';
import ProjectNoteEditor from './ProjectNoteEditor';
import { noteToMarkdown } from './noteExport';
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
