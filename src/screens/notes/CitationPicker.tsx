// Gaply — Research Paper Writer, Set B1: the insert-citation picker. Searches the
// user's citation_library (via the context's search) and inserts a gaplyCite node
// on pick. Opened by the toolbar button or ⌘⇧C. NO reference creation here —
// that stays the Citation Manager's job (linked out honestly).
import React, { useEffect, useRef, useState } from 'react';
import { useCitationCtx, CitationPickItem } from './CitationContext';

interface CitationPickerProps {
  onPick: (refId: string) => void;
  onClose: () => void;
}

const CitationPicker: React.FC<CitationPickerProps> = ({ onPick, onClose }) => {
  const { search } = useCitationCtx();
  const [query, setQuery] = useState('');
  const [results, setResults] = useState<CitationPickItem[]>([]);
  const [loading, setLoading] = useState(false);
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => { inputRef.current?.focus(); }, []);

  // Debounced search (empty query lists recent).
  useEffect(() => {
    let alive = true;
    setLoading(true);
    const t = window.setTimeout(() => {
      search(query).then((r) => { if (alive) { setResults(r); setLoading(false); } }).catch(() => { if (alive) { setResults([]); setLoading(false); } });
    }, 120);
    return () => { alive = false; window.clearTimeout(t); };
  }, [query, search]);

  return (
    <div className="an-cite-picker" role="dialog" data-testid="cite-picker" onKeyDown={(e) => { if (e.key === 'Escape') onClose(); }}>
      <div className="an-cite-picker-head">
        <input ref={inputRef} className="an-cite-search" placeholder="Search your citation library…" value={query} data-testid="cite-search" onChange={(e) => setQuery(e.target.value)} />
        <button className="an-rb-btn" data-testid="cite-picker-close" onClick={onClose}>Esc</button>
      </div>
      <div className="an-cite-results" data-testid="cite-results">
        {loading && <div className="an-cite-empty">Searching…</div>}
        {!loading && results.length === 0 && (
          <div className="an-cite-empty" data-testid="cite-noresults">
            No references match. Add references in <b>Citations</b>, then cite them here.
          </div>
        )}
        {!loading && results.map((r) => (
          <button key={r.id} className="an-cite-result" data-testid={`cite-pick-${r.id}`} onClick={() => onPick(r.id)}>
            <span className="an-cite-result-title">{r.title || '(untitled reference)'}</span>
            <span className="an-cite-result-meta">{[r.authors, r.year].filter(Boolean).join(' · ')}</span>
          </button>
        ))}
      </div>
    </div>
  );
};

export default CitationPicker;
