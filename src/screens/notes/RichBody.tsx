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
import React, { useEffect, useReducer, useState } from 'react';
import { useEditor, EditorContent, Editor } from '@tiptap/react';
import StarterKit from '@tiptap/starter-kit';
import { Placeholder } from '@tiptap/extensions';
import { TableKit } from '@tiptap/extension-table';
import { Markdown } from 'tiptap-markdown';
import { GaplyImage } from './GaplyImageNode';
import { GaplyCiteLive } from './CitationContext';
import { GaplyMath } from './GaplyMathNode';
import { MathNodeView } from './MathView';
import { ReactNodeViewRenderer } from '@tiptap/react';
import CitationPicker from './CitationPicker';
import MathInput from './MathInput';
import { storeImageFile } from './noteImages';

/** The math node WITH its KaTeX NodeView — the manuscript writing surface only,
 *  exactly like GaplyCiteLive. The base node keeps the markdown round-trip. */
const GaplyMathLive = GaplyMath.extend({
  addNodeView() {
    return ReactNodeViewRenderer(MathNodeView);
  },
});

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
export const richExtensions = (placeholder = '', opts: { citations?: boolean } = {}) => [
  StarterKit,
  TableKit.configure({ table: { resizable: false } }),
  GaplyImage, // pasted images (Set 3) — renders gaply-image://<hash> refs via blob URL
  // In-text citations (Set B1) — manuscript surface only; renders [[cite:id]]
  // tokens as live, style-aware markers via a NodeView. Off for project notes.
  // Math rides the same flag: both are manuscript concerns, and a project note
  // stays a plain quick-capture surface.
  ...(opts.citations ? [GaplyCiteLive, GaplyMathLive] : []),
  Placeholder.configure({ placeholder }),
  Markdown.configure({ html: false, breaks: true }),
];

/** The image FILE in a paste, or null. FREE path: clipboardData.items →
 *  getAsFile(). Proven to deliver real bytes in the WKWebView. Exported for
 *  the pins. */
export const pickPastedImage = (data: DataTransfer | null): File | null => {
  const item = Array.from(data?.items ?? []).find((i) => i.kind === 'file' && i.type.startsWith('image/'));
  return item ? item.getAsFile() : null;
};

/** True when a drag carries FILES (not text/html). Drag-drop does NOT deliver
 *  image bytes in this webview, so we detect the intent and hint honestly
 *  instead of leaving a silent dead-zone. */
export const isFileDrag = (data: DataTransfer | null): boolean =>
  !!data && Array.from(data.types).includes('Files');

/** The honest drop message: a file drag gets the ⌘V hint (bytes don't arrive
 *  via drop in this webview); anything else returns null (behaves normally). */
export const imageDropHint = (data: DataTransfer | null): string | null =>
  isFileDrag(data) ? 'Drag-and-drop isn’t supported here — paste images with ⌘V.' : null;

/** Prevent the mousedown from blurring the editor (which would drop the
 *  in-table selection before the command runs). */
const hold = (fn: () => void) => (e: React.MouseEvent) => { e.preventDefault(); fn(); };

const RichToolbar: React.FC<{ editor: Editor; onInsertCite?: () => void; onInsertMath?: () => void }> = ({ editor, onInsertCite, onInsertMath }) => {
  const inTable = editor.isActive('table');
  return (
    <div className="an-rb-toolbar" data-testid="rb-toolbar">
      {onInsertCite && !inTable && (
        <button className="an-rb-btn" data-testid="rb-insert-cite" title="Insert citation (⌘⇧C)"
          onMouseDown={hold(onInsertCite)}>
          ❝ Cite
        </button>
      )}
      {onInsertMath && !inTable && (
        <button className="an-rb-btn" data-testid="rb-insert-math" title="Insert formula (⌘⇧M)"
          onMouseDown={hold(onInsertMath)}>
          ∑ Formula
        </button>
      )}
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
  /** Manuscript surface only: enables the in-text citation node + insert picker
   *  (⌘⇧C / toolbar) AND the math node + formula dialog (⌘⇧M / toolbar).
   *  Project notes leave this off. */
  withCitations?: boolean;
}

const RichBody: React.FC<RichBodyProps> = ({ value, onChange, placeholder, testid, withCitations }) => {
  const [, force] = useReducer((x: number) => x + 1, 0);
  // One honest inline message channel: image errors (oversize/unsupported) and
  // the drag-drop hint. Cleared on the next successful paste or on dismiss.
  const [hint, setHint] = useState<string | null>(null);
  const [citeOpen, setCiteOpen] = useState(false);
  // null = closed. `pos` is set when EDITING an existing formula (replace in
  // place); undefined `pos` means insert at the selection.
  const [mathOpen, setMathOpen] = useState<null | { tex: string; display: boolean; pos?: number }>(null);

  const editor = useEditor({
    extensions: richExtensions(placeholder ?? '', { citations: withCitations }),
    content: value,
    editorProps: {
      // The spike-proven WKWebView fix — keep all three exactly as verified.
      attributes: { autocorrect: 'off', autocapitalize: 'off', spellcheck: 'false' },
      // ⌘⇧C opens the insert-citation picker (manuscript surface only).
      handleKeyDown: withCitations ? (_view, event) => {
        if ((event.metaKey || event.ctrlKey) && event.shiftKey && event.key.toLowerCase() === 'c') {
          event.preventDefault();
          setCiteOpen(true);
          return true;
        }
        if ((event.metaKey || event.ctrlKey) && event.shiftKey && event.key.toLowerCase() === 'm') {
          event.preventDefault();
          setMathOpen({ tex: '', display: false });
          return true;
        }
        return false;
      } : undefined,
      // PASTE an image (Set 3): store to disk by content-hash, then insert the
      // canonical gaply-image:// ref as an image node. Async, so we consume the
      // event now and dispatch the node when the write resolves.
      handlePaste: (view, event) => {
        const file = pickPastedImage(event.clipboardData);
        if (!file) return false; // not an image — let normal (text) paste run
        storeImageFile(file)
          .then((ref) => {
            setHint(null);
            const node = view.state.schema.nodes.image.create({ src: ref });
            // The dispatch runs through TipTap's dispatchTransaction, so onUpdate
            // fires and serializes the new body — no explicit onChange needed.
            view.dispatch(view.state.tr.replaceSelectionWith(node));
          })
          // Show the REAL reason (storeImageFile rethrows fs errors with context);
          // String(e) as a last resort so a non-Error rejection is never masked.
          .catch((e) => setHint(e instanceof Error ? e.message : String(e)));
        return true; // consume — we handled the image
      },
      // DROP does not deliver image bytes in this webview. Be honest: on a file
      // drag, show the paste hint instead of silently swallowing it.
      handleDrop: (_view, event) => {
        const msg = imageDropHint((event as DragEvent).dataTransfer);
        if (msg) {
          setHint(msg);
          event.preventDefault();
          return true;
        }
        return false; // text/other drags behave normally
      },
    },
    onUpdate: ({ editor: e }) => onChange(mdOf(e)),
  });

  // Re-render the toolbar on selection changes so table controls appear in-context.
  // The same transaction hook carries the math NodeView's double-click: it sets
  // a `gaplyMathEdit` meta rather than reaching into this component, so the node
  // stays a plain NodeView with no prop drilling.
  useEffect(() => {
    if (!editor) return;
    const onTx = ({ transaction }: { transaction: { getMeta: (k: string) => unknown } }) => {
      const edit = transaction.getMeta('gaplyMathEdit') as { pos: number; tex: string; display: boolean } | undefined;
      if (edit) setMathOpen({ tex: edit.tex, display: edit.display, pos: edit.pos });
      force();
    };
    editor.on('selectionUpdate', force);
    editor.on('transaction', onTx);
    return () => { editor.off('selectionUpdate', force); editor.off('transaction', onTx); };
  }, [editor]);

  const insertCite = (refId: string) => {
    editor?.chain().focus().insertContent({ type: 'gaplyCite', attrs: { refId } }).run();
    setCiteOpen(false);
  };

  /** Insert a new formula, or replace the one being edited IN PLACE so the
   *  surrounding prose and the document-order citation counter are untouched. */
  const applyMath = (tex: string, display: boolean) => {
    const at = mathOpen?.pos;
    const node = { type: 'gaplyMath', attrs: { tex, display } };
    if (editor) {
      if (typeof at === 'number') {
        editor.chain().focus().insertContentAt({ from: at, to: at + 1 }, node).run();
      } else {
        editor.chain().focus().insertContent(node).run();
      }
    }
    setMathOpen(null);
  };

  return (
    <div className="an-richbody" data-testid={testid}>
      {editor && (
        <RichToolbar
          editor={editor}
          onInsertCite={withCitations ? () => setCiteOpen(true) : undefined}
          onInsertMath={withCitations ? () => setMathOpen({ tex: '', display: false }) : undefined}
        />
      )}
      {hint && (
        <div className="an-rb-hint" role="status" data-testid="rb-image-hint">
          <span>{hint}</span>
          <button type="button" className="an-rb-hint-x" onClick={() => setHint(null)} aria-label="Dismiss">×</button>
        </div>
      )}
      {citeOpen && withCitations && <CitationPicker onPick={insertCite} onClose={() => setCiteOpen(false)} />}
      {mathOpen && withCitations && (
        <MathInput
          initialTex={mathOpen.tex}
          initialDisplay={mathOpen.display}
          onInsert={applyMath}
          onClose={() => setMathOpen(null)}
        />
      )}
      <EditorContent editor={editor} />
    </div>
  );
};

export default RichBody;
