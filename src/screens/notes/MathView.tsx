// Gaply — the math NodeView: KaTeX rendering inside the writing surface.
//
// KaTeX, not MathJax, and bundled rather than fetched: it is synchronous,
// deterministic (same TeX in, same HTML out — no model, no network), and it
// ships its own fonts as webpack assets served from the app's own origin, which
// the CSP already allows (`font-src 'self' data:`). The offline promise holds.
//
// A render FAILURE is shown honestly, in place, with KaTeX's own message — the
// same rule the rest of this lane follows. Math that does not parse must never
// silently vanish or render as something else, because the author cannot tell
// the difference from a glance at a formula they just wrote.
import React, { useCallback, useMemo } from 'react';
import katex from 'katex';
import 'katex/dist/katex.min.css';
import { NodeViewWrapper, NodeViewProps } from '@tiptap/react';
import './notes.css';

export interface MathRender { html: string; error: string | null }

/** Render TeX to HTML. Never throws: KaTeX's `throwOnError: false` puts its own
 *  error text in the output, and we surface the message alongside. Exported so
 *  the insert dialog previews through the EXACT path the document renders. */
export function renderTex(tex: string, display: boolean): MathRender {
  try {
    const html = katex.renderToString(tex, {
      displayMode: display,
      throwOnError: false,
      errorColor: 'var(--an-error)',
      strict: false,
      output: 'html',
      trust: false, // no \href, no \includegraphics — nothing that reaches out
    });
    return { html, error: null };
  } catch (e) {
    return { html: '', error: e instanceof Error ? e.message : String(e) };
  }
}

/** True when KaTeX could not parse the TeX at all (as opposed to rendering it
 *  with an inline error marker). Used by the dialog to gate Insert.
 *
 *  `display` IS LOAD-BEARING, not a detail. KaTeX accepts `equation`, `align`,
 *  `gather` and `\tag` ONLY in display mode — measured — so validating inline
 *  regardless would reject perfectly good display math and tell the author their
 *  formula was broken when it was not. Validate in the mode it will be used in. */
export function texError(tex: string, display = false): string | null {
  try {
    katex.renderToString(tex, { displayMode: display, throwOnError: true, strict: false, trust: false });
    return null;
  } catch (e) {
    return e instanceof Error ? e.message.replace(/^KaTeX parse error:\s*/, '') : String(e);
  }
}

export const MathNodeView: React.FC<NodeViewProps> = ({ node, editor, getPos, selected }) => {
  const tex = (node.attrs.tex as string) ?? '';
  const display = !!node.attrs.display;
  const { html } = useMemo(() => renderTex(tex, display), [tex, display]);

  // Click to edit: hand the node's position to the surface, which opens the
  // same dialog that inserted it.
  const onEdit = useCallback(() => {
    if (typeof getPos !== 'function') return;
    editor.commands.command(({ tr, dispatch }) => {
      if (dispatch) tr.setMeta('gaplyMathEdit', { pos: getPos(), tex, display });
      return true;
    });
  }, [editor, getPos, tex, display]);

  return (
    <NodeViewWrapper
      as="span"
      className={`an-math${display ? ' an-math--display' : ''}${selected ? ' an-math--selected' : ''}`}
      data-testid={`math-${display ? 'block' : 'inline'}`}
      data-tex={tex}
      contentEditable={false}
      onDoubleClick={onEdit}
      title="Double-click to edit"
    >
      <span dangerouslySetInnerHTML={{ __html: html }} />
    </NodeViewWrapper>
  );
};
