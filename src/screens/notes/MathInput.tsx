// Gaply — the insert/edit-math dialog. A LaTeX field with a live KaTeX preview.
//
// DELIBERATELY A LATEX FIELD, not a WYSIWYG equation builder. Researchers
// already write LaTeX, it is what both exporters carry, and what you type is
// exactly what is stored — there is no lossy middle representation to explain.
// A visual builder is a much larger dependency and a different product
// decision; this keeps the promise small and true.
//
// The preview renders through `renderTex`, the SAME function the document uses,
// so the dialog can never show something the page will not.
import React, { useEffect, useMemo, useRef, useState } from 'react';
import { renderTex, texError } from './MathView';
import { isStorableTex } from './GaplyMathNode';
import './notes.css';

export interface MathInputProps {
  /** Prefill when editing an existing formula. */
  initialTex?: string;
  initialDisplay?: boolean;
  onInsert: (tex: string, display: boolean) => void;
  onClose: () => void;
}

const EXAMPLES: Array<{ label: string; tex: string }> = [
  { label: 'fraction', tex: '\\frac{a}{b}' },
  { label: 'integral', tex: '\\int_0^1 x^2\\,dx' },
  { label: 'sum', tex: '\\sum_{i=1}^{n} x_i' },
  { label: 'matrix', tex: '\\begin{pmatrix} a & b \\\\ c & d \\end{pmatrix}' },
  { label: 'cases', tex: '\\begin{cases} x & x \\ge 0 \\\\ -x & x < 0 \\end{cases}' },
];

const MathInput: React.FC<MathInputProps> = ({ initialTex = '', initialDisplay = false, onInsert, onClose }) => {
  const [tex, setTex] = useState(initialTex);
  const [display, setDisplay] = useState(initialDisplay);
  const ref = useRef<HTMLTextAreaElement>(null);
  useEffect(() => { ref.current?.focus(); ref.current?.select(); }, []);

  const trimmed = tex.trim();
  const preview = useMemo(() => renderTex(trimmed || '\\;', display), [trimmed, display]);
  const parseError = useMemo(() => (trimmed ? texError(trimmed, display) : null), [trimmed, display]);
  // The storage limit, enforced at the door rather than discovered on reopen.
  const tokenError = trimmed && !isStorableTex(trimmed)
    ? 'A formula can’t contain “]]” — that sequence ends the stored token. Rewrite it, e.g. with \\right] \\bigr].'
    : null;
  const blocked = !trimmed || !!parseError || !!tokenError;

  const submit = () => { if (!blocked) onInsert(trimmed, display); };

  return (
    <div
      className="an-cite-picker an-math-picker"
      role="dialog"
      aria-label="Insert formula"
      data-testid="math-input"
      onKeyDown={(e) => {
        if (e.key === 'Escape') { e.stopPropagation(); onClose(); }
        // ⌘/Ctrl+Enter inserts — plain Enter stays a newline, since TeX spans lines.
        if (e.key === 'Enter' && (e.metaKey || e.ctrlKey)) { e.preventDefault(); submit(); }
      }}
    >
      <div className="an-cite-picker-head">
        <strong className="an-math-title">{initialTex ? 'Edit formula' : 'Insert formula'}</strong>
        <label className="an-math-toggle">
          <input type="checkbox" checked={display} data-testid="math-display-toggle" onChange={(e) => setDisplay(e.target.checked)} />
          Display (own line)
        </label>
        <button className="an-rb-btn" data-testid="math-close" onClick={onClose}>Esc</button>
      </div>

      <textarea
        ref={ref}
        className="an-math-field"
        rows={3}
        spellCheck={false}
        autoCorrect="off"
        autoCapitalize="off"
        placeholder="LaTeX — e.g. \frac{1}{x} or \int_0^1 x^2\,dx"
        value={tex}
        data-testid="math-tex"
        onChange={(e) => setTex(e.target.value)}
      />

      <div className="an-math-preview" data-testid="math-preview">
        {trimmed ? <span dangerouslySetInnerHTML={{ __html: preview.html }} /> : <span className="an-math-hint">Preview appears here.</span>}
      </div>

      {(parseError || tokenError) && (
        <p className="an-error an-math-error" role="alert" data-testid="math-error">{tokenError ?? parseError}</p>
      )}

      <div className="an-math-examples">
        {EXAMPLES.map((ex) => (
          <button key={ex.label} className="an-rb-btn" data-testid={`math-example-${ex.label}`} onClick={() => setTex(ex.tex)}>
            {ex.label}
          </button>
        ))}
      </div>

      <div className="an-math-actions">
        <span className="an-math-hint">Everything KaTeX renders. ⌘↵ to insert.</span>
        <button className="an-savebtn" disabled={blocked} data-testid="math-insert" onClick={submit}>
          {initialTex ? 'Update' : 'Insert'}
        </button>
      </div>
    </div>
  );
};

export default MathInput;
