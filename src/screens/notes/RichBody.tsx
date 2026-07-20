// Gaply — the rich writing surface (rich-editor Set 1). TipTap v3 + StarterKit
// over MARKDOWN-CANONICAL storage: the note's body stays a plain md string in
// sqlite (zero migration, total backward compat); this component is a VIEW over
// it — parse on open, serialize on change. Serializer: tiptap-markdown@0.9.0
// (v3 peer-clean, probe-verified round-trips) + one normalization: hardBreak
// serializes as "\\\n"; we normalize to "\n" so multiline plain-text notes keep
// their historical shape and our MarkdownRenderer/preview render them cleanly.
// (Safe: ONLY hardBreak emits backslash-newline — a literal "\" in text gets
// double-escaped by the serializer.)
//
// The anti-substitution attributes (autocorrect/autocapitalize/spellcheck off)
// are the Set 0 spike's PROVEN fix — macOS smart-substitution in WKWebView was
// eating ProseMirror input rules ("- " → bullet stayed dead until these).
// NO AI anywhere: typing is the only way words appear ("Gaply never writes
// your notes for you").
import React from 'react';
import { useEditor, EditorContent, Editor } from '@tiptap/react';
import StarterKit from '@tiptap/starter-kit';
import { Placeholder } from '@tiptap/extensions';
import { Markdown } from 'tiptap-markdown';

/** Serialize the editor back to canonical markdown (hardBreak → plain \n). */
export const mdOf = (editor: Editor): string => {
  const raw = (editor.storage as { markdown?: { getMarkdown: () => string } }).markdown?.getMarkdown() ?? '';
  return raw.replace(/\\\n/g, '\n');
};

/** THE production extension set — exported so tests pin the real config
 *  (input rules, md round-trip), not a test-local copy. */
export const richExtensions = (placeholder = '') => [
  StarterKit,
  Placeholder.configure({ placeholder }),
  Markdown.configure({ html: false, breaks: true }),
];

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
  const editor = useEditor({
    extensions: richExtensions(placeholder ?? ''),
    content: value,
    editorProps: {
      // The spike-proven WKWebView fix — keep all three exactly as verified.
      attributes: { autocorrect: 'off', autocapitalize: 'off', spellcheck: 'false' },
    },
    onUpdate: ({ editor: e }) => onChange(mdOf(e)),
  });

  return (
    <div className="an-richbody" data-testid={testid}>
      <EditorContent editor={editor} />
    </div>
  );
};

export default RichBody;
