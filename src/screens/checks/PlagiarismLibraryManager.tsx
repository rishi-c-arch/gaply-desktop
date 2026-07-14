// Gaply — the "my papers" library manager (Set 5). The user CURATES the
// durable set of their own papers that the exact-match check compares against:
// add a paper (path → add_to_plagiarism_library), list them, remove them.
// Path-only over IPC; the text never crosses the seam here.
import React, { useCallback, useEffect, useRef, useState } from 'react';
import { Button, Card } from '../../design-system';
import './plagiarism.css';
import { CheckBridge } from './checkBridge';
import { LibraryPaper } from './agentTypes';
import { LocalLibrary, StoredReference, TauriLocalLibrary } from '../citations/localLibrary';

// Module-level singleton default (stable identity → no per-render churn / effect
// loops), mirroring TauriPaperSource's citation source.
const defaultCitations = new TauriLocalLibrary();

export interface PlagiarismLibraryManagerProps {
  bridge: CheckBridge;
  /** Notified after add/remove so the checker can note the scope changed. */
  onChange?: () => void;
  /** M2 Set 2B: source for the OPTIONAL add-time citation picker (citation_library).
   *  Defaults to the real local library; injectable for tests. */
  citations?: LocalLibrary;
}

const PlagiarismLibraryManager: React.FC<PlagiarismLibraryManagerProps> = ({ bridge, onChange, citations = defaultCitations }) => {
  const [papers, setPapers] = useState<LibraryPaper[] | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const inputRef = useRef<HTMLInputElement>(null);
  // The OPTIONAL citation to associate with the NEXT added paper. '' = "— none —"
  // → no link (citation_id stays NULL, exactly as before). Read at add time.
  const [refs, setRefs] = useState<StoredReference[]>([]);
  const [linkCitationId, setLinkCitationId] = useState('');

  const refresh = useCallback(async () => {
    try {
      setPapers(await bridge.libraryList());
    } catch (e) {
      setError(e instanceof Error ? e.message : 'could not load library');
    }
  }, [bridge]);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  // Load the citation library once for the optional picker (best-effort — an
  // empty/failed list just leaves the picker with only "— none —").
  useEffect(() => {
    citations.list().then(setRefs).catch(() => setRefs([]));
  }, [citations]);

  const onPick = async (file: File) => {
    setBusy(true);
    setError(null);
    try {
      const path = (file as any).path ?? file.name;
      // Explicit optional link: pass the picked citation id, or undefined when
      // "— none —" (→ Set-2A's optional param → NULL, unchanged behavior).
      await bridge.libraryAdd(path, undefined, linkCitationId || undefined);
      setLinkCitationId(''); // reset so the link is a per-paper deliberate choice
      await refresh();
      onChange?.();
    } catch (e) {
      setError(e instanceof Error ? e.message : 'could not add paper');
    } finally {
      setBusy(false);
    }
  };

  const onRemove = async (id: number) => {
    setBusy(true);
    setError(null);
    try {
      await bridge.libraryRemove(id);
      await refresh();
      onChange?.();
    } catch (e) {
      setError(e instanceof Error ? e.message : 'could not remove paper');
    } finally {
      setBusy(false);
    }
  };

  return (
    <Card title="Your papers" data-testid="plag-library">
      <p style={{ margin: '0 0 12px', color: 'var(--g-text-3)', fontSize: 13 }}>
        The papers a plagiarism check compares against — add your past work to catch reuse.
        Parsed on your device; the text never leaves it.
      </p>
      <div style={{ display: 'flex', gap: 8, alignItems: 'center', flexWrap: 'wrap', marginBottom: 12 }}>
        {/* Optional add-time link: associate the next added paper with a citation
            from your library (grounded — YOU declare the match, no auto-infer).
            Leaving it "— none —" adds the paper with no link, exactly as before. */}
        <select
          className="gds-jc__input"
          data-testid="library-citation-select"
          value={linkCitationId}
          onChange={(e) => setLinkCitationId(e.target.value)}
          disabled={busy}
          style={{ minWidth: 240 }}
          aria-label="Link to a citation (optional)"
        >
          <option value="">Link to a citation (optional) — none —</option>
          {refs.map((r) => (
            <option key={r.id} value={r.id}>
              {r.title}{r.authors ? ` — ${r.authors}` : ''}{r.year ? ` (${r.year})` : ''}
            </option>
          ))}
        </select>
        <Button variant="secondary" onClick={() => inputRef.current?.click()} disabled={busy} data-testid="library-pick">
          {busy ? 'Working…' : 'Add a paper'}
        </Button>
        <input
          ref={inputRef}
          type="file"
          accept=".pdf,.docx,.txt,.md"
          style={{ display: 'none' }}
          data-testid="library-add-input"
          onChange={(e) => e.target.files?.[0] && void onPick(e.target.files[0])}
        />
      </div>
      {error && <p style={{ color: 'var(--g-flagged)', fontSize: 13 }} role="alert" data-testid="library-error">{error}</p>}

      {papers === null ? (
        <p className="gds-plag-lib__empty">Loading…</p>
      ) : papers.length === 0 ? (
        <p className="gds-plag-lib__empty" data-testid="library-empty">
          No papers in your library yet — add your past work to check for reuse.
        </p>
      ) : (
        <div className="gds-plag-lib" data-testid="library-list">
          {papers.map((p) => (
            <div key={p.id} className="gds-plag-lib__row" data-testid={`library-paper-${p.id}`}>
              <div className="gds-plag-lib__meta">
                <p className="gds-plag-lib__title">{p.title}</p>
                <span className="gds-plag-lib__date">
                  added {new Date(p.added_at * 1000).toLocaleDateString()}
                </span>
              </div>
              <Button
                variant="ghost"
                onClick={() => onRemove(p.id)}
                disabled={busy}
                data-testid={`library-remove-${p.id}`}
              >
                Remove
              </Button>
            </div>
          ))}
        </div>
      )}
    </Card>
  );
};

export default PlagiarismLibraryManager;
