// Gaply — the rich writing surface (rich-editor Sets 1–2). TipTap v3 +
// StarterKit + TABLES over MARKDOWN-CANONICAL storage: the note's body stays a
// plain md string in sqlite (zero migration, total backward compat); this
// component is a VIEW over it — parse on open, serialize on change.
//
// Serializer: tiptap-markdown@0.9.0 (v3 peer-clean) + one normalization:
// hardBreak serializes as "\\\n"; we normalize to "\n" so multiline plain-text
// notes keep their historical shape. GFM TABLES round-trip losslessly through
// v3's TableKit native md serialization (probe-verified) — no custom serializer.
//
// The anti-substitution attributes (autocorrect/autocapitalize/spellcheck off)
// are the Set 0 spike's PROVEN fix — macOS smart-substitution in WKWebView was
// eating ProseMirror input rules. NO AI anywhere ("Gaply never writes your
// notes for you").
import React, { useEffect, useReducer } from 'react';
import { useEditor, EditorContent, Editor } from '@tiptap/react';
import StarterKit from '@tiptap/starter-kit';
import { Placeholder } from '@tiptap/extensions';
import { TableKit } from '@tiptap/extension-table';
import { Markdown } from 'tiptap-markdown';

/** Serialize the editor back to canonical markdown. Two normalizations:
 *  hardBreak → plain "\n" (keeps multiline plain-text notes byte-stable), and
 *  strip trailing newlines (tiptap-markdown appends one after a block-level
 *  table; stripping keeps table bodies byte-identical to input and the export
 *  clean — plain notes have no trailing newline, so they're unaffected). */
export const mdOf = (editor: Editor): string => {
  const raw = (editor.storage as { markdown?: { getMarkdown: () => string } }).markdown?.getMarkdown() ?? '';
  return raw.replace(/\\\n/g, '\n').replace(/\n+$/, '');
};

/** THE production extension set — exported so tests pin the real config
 *  (input rules, GFM tables, md round-trip), not a test-local copy. Table
 *  resizing is off (kept simple; tab navigation + toolbar controls edit it). */
export const richExtensions = (placeholder = '') => [
  StarterKit,
  TableKit.configure({ table: { resizable: false } }),
  Placeholder.configure({ placeholder }),
  Markdown.configure({ html: false, breaks: true }),
];

/** Prevent the mousedown from blurring the editor (which would drop the
 *  in-table selection before the command runs). */
const hold = (fn: () => void) => (e: React.MouseEvent) => { e.preventDefault(); fn(); };

const RichToolbar: React.FC<{ editor: Editor }> = ({ editor }) => {
  const inTable = editor.isActive('table');
  return (
    <div className="an-rb-toolbar" data-testid="rb-toolbar">
      {!inTable ? (
        <button className="an-rb-btn" data-testid="rb-insert-table"
          onMouseDown={hold(() => editor.chain().focus().insertTable({ rows: 3, cols: 3, withHeaderRow: true }).run())}>
          ⊞ Table
        </button>
      ) : (
        <>
          <button className="an-rb-btn" data-testid="rb-row-add" onMouseDown={hold(() => editor.chain().focus().addRowAfter().run())}>+ Row</button>
          <button className="an-rb-btn" data-testid="rb-col-add" onMouseDown={hold(() => editor.chain().focus().addColumnAfter().run())}>+ Col</button>
          <button className="an-rb-btn" data-testid="rb-row-del" onMouseDown={hold(() => editor.chain().focus().deleteRow().run())}>− Row</button>
          <button className="an-rb-btn" data-testid="rb-col-del" onMouseDown={hold(() => editor.chain().focus().deleteColumn().run())}>− Col</button>
          <button className="an-rb-btn an-rb-btn--danger" data-testid="rb-table-del" onMouseDown={hold(() => editor.chain().focus().deleteTable().run())}>Delete table</button>
        </>
      )}
    </div>
  );
};

export interface RichBodyProps {
  /** The note body, canonical markdown. Parsed once on mount; NOT re-synced on
   *  prop change (the editor is the source of truth while open). */
  value: string;
  /** Fired ONLY on real user edits — an untouched note never re-serializes,
   *  so opening a note without editing can never rewrite its bytes. */
  onChange: (md: string) => void;
  placeholder?: string;
  testid?: string;
}

const RichBody: React.FC<RichBodyProps> = ({ value, onChange, placeholder, testid }) => {
  const [, force] = useReducer((x: number) => x + 1, 0);
  const editor = useEditor({
    extensions: richExtensions(placeholder ?? ''),
    content: value,
    editorProps: {
      // The spike-proven WKWebView fix — keep all three exactly as verified.
      attributes: { autocorrect: 'off', autocapitalize: 'off', spellcheck: 'false' },
    },
    onUpdate: ({ editor: e }) => onChange(mdOf(e)),
  });

  // Re-render the toolbar on selection changes so table controls appear in-context.
  useEffect(() => {
    if (!editor) return;
    editor.on('selectionUpdate', force);
    editor.on('transaction', force);
    return () => { editor.off('selectionUpdate', force); editor.off('transaction', force); };
  }, [editor]);

  return (
    <div className="an-richbody" data-testid={testid}>
      {editor && <RichToolbar editor={editor} />}
      <EditorContent editor={editor} />
    </div>
  );
};

export default RichBody;
