// Set B SPIKE — the ONE unknown: does a custom inline gaplyCite node survive
// markdown-canonical storage? Headless, like the Set 2 table / Set 3 image
// round-trip probes (tiptap-markdown parsing is pure JS — no WKWebView needed).
import { describe, it, expect } from 'vitest';
import { Editor } from '@tiptap/core';
import StarterKit from '@tiptap/starter-kit';
import { Markdown } from 'tiptap-markdown';
import { GaplyCite, CITE_TOKEN_RE } from './GaplyCiteNode';

const ext = [StarterKit, GaplyCite, Markdown.configure({ html: false, breaks: true })];
const md = (e: Editor) => (e.storage as unknown as { markdown: { getMarkdown: () => string } }).markdown.getMarkdown();
const open = (source: string) => { const e = new Editor({ extensions: ext }); e.commands.setContent(source); return e; };
const hasCite = (e: Editor, refId: string) => JSON.stringify(e.getJSON()).includes(`"refId":"${refId}"`);

describe('gaplyCite token round-trip through markdown-canonical storage', () => {
  it('(2) the FIDDLY direction: raw [[cite:X]] in a loaded body becomes a real node, not text', () => {
    const e = open('See [[cite:ref-42]] here.');
    expect(hasCite(e, 'ref-42')).toBe(true);               // it's a gaplyCite node
    expect(JSON.stringify(e.getJSON())).toContain('"type":"gaplyCite"');
    e.destroy();
  });

  it('(1) node → markdown → node: serialize is a clean stable token; idempotent', () => {
    const e1 = open('Intro [[cite:abc-123]] end.');
    const m1 = md(e1); e1.destroy();
    expect(m1).toContain('[[cite:abc-123]]');              // clean, stable token
    const e2 = open(m1);                                    // reparse
    const m2 = md(e2); e2.destroy();
    expect(m2).toBe(m1);                                    // parse→serialize→parse stable
    expect(hasCite(open(m1), 'abc-123')).toBe(true);
  });

  it('(3a) a token ADJACENT to text (no spaces) still parses', () => {
    const e = open('see[[cite:X]]here');
    expect(hasCite(e, 'X')).toBe(true);
    expect(md(e)).toContain('[[cite:X]]');
    e.destroy();
  });

  it('(3b) real library-id shapes (uuid/hyphens) round-trip', () => {
    for (const id of ['a1b2c3', 'ref-2024-0001', '9f8e7d6c-1234-4abc-9def-000111222333']) {
      const e = open(`x [[cite:${id}]] y`);
      expect(hasCite(e, id)).toBe(true);
      expect(md(e)).toContain(`[[cite:${id}]]`);
      e.destroy();
    }
  });

  it('(3c) MALFORMED / does-not-trigger cases stay literal text (no node)', () => {
    for (const bad of ['[[cite:]]', '[[cite: spaced]]', '[cite:X]', '[[cite:X]']) {
      const e = open(`before ${bad} after`);
      expect(JSON.stringify(e.getJSON())).not.toContain('"type":"gaplyCite"');
      e.destroy();
    }
  });

  it('(3d) inside a CODE SPAN it must NOT become a node (stays literal)', () => {
    const e = open('use `[[cite:X]]` verbatim');
    expect(JSON.stringify(e.getJSON())).not.toContain('"type":"gaplyCite"');
    e.destroy();
  });

  it('(4) survives the full note path (parse → mdOf → store → reopen) + is regex-extractable for B2', () => {
    // parse → serialize (what save writes to fields_json)
    const stored = md(open('A [[cite:ref-A]] and [[cite:ref-B]] and again [[cite:ref-A]].'));
    // B2's orderedRefIds regex must find every token in document order:
    const ids = (stored.match(CITE_TOKEN_RE) ?? []).map((t) => t.replace(/\[\[cite:|\]\]/g, ''));
    expect(ids).toEqual(['ref-A', 'ref-B', 'ref-A']);      // order + reuse preserved
    // reopen from the stored string → still nodes
    const reopened = open(stored);
    expect(hasCite(reopened, 'ref-A')).toBe(true);
    expect(hasCite(reopened, 'ref-B')).toBe(true);
    expect(md(reopened)).toBe(stored);                     // byte-stable across the full path
    reopened.destroy();
  });

  it('BACKWARD COMPAT: a body with NO citations is untouched by the extension', () => {
    for (const x of ['Plain prose.', '# H\n\n- a\n- b', '> quote']) {
      const e = open(x);
      expect(md(e)).toBe(x);
      e.destroy();
    }
  });
});
