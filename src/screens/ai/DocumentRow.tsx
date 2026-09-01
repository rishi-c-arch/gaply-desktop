// Gaply — the source document behind a citation, in the citation's own pane.
//
// The evidence rows could already open a page of the cited PDF, but only as a
// consequence of an AI check. A reference IS a document; being able to look at
// it should not require asking a model something first.
//
// The row owns BOTH its lookups on purpose. The Citation Manager's own
// `aiDocumentId` effect is gated on `aiInstalled`, which is right for the AI
// panel and wrong here: whether a citation has a linked document is a fact in
// the store, and reading "No document linked" because the model happens to be
// absent would be a false statement rather than a missing feature.
import './ai.css';
import React, { useCallback, useEffect, useState } from 'react';
import { Button, Card } from '../../design-system/primitives';
import { aiBridge, errorText, LinkSourceEvent } from './aiBridge';
import { pickManuscriptPath } from '../common/pickFile';

const PdfViewer = React.lazy(() => import('./PdfViewer'));

/** What the row knows once both lookups have answered. */
type Linked =
  | { kind: 'loading' }
  | { kind: 'none' }
  | { kind: 'linked'; documentId: number; path: string; exists: boolean }
  | { kind: 'error'; message: string };

/** Last path segment, for either separator. The store holds absolute paths. */
export function fileName(path: string): string {
  const parts = path.split(/[\\/]/).filter(Boolean);
  return parts[parts.length - 1] ?? path;
}

export interface DocumentRowProps {
  citationId: string;
  /** Window title for the viewer; falls back to the file name. */
  citedSource?: string;
  bridge?: Pick<typeof aiBridge, 'citationDocument' | 'documentSource' | 'linkSourceDocument'>;
  /** Test seam for the native file dialog. */
  pickSource?: () => Promise<string | null>;
  /** Called once a source becomes checkable, so the panel can offer a re-check. */
  onLinked?: (documentId: number) => void;
  /** Test seam, forwarded to the viewer. */
  loadBytes?: (documentId: number) => Promise<Uint8Array>;
  /** Test seam for the native reveal. */
  reveal?: (path: string) => Promise<void>;
}

/** Show the file where it lives. Dynamic import so a browser build never
 *  evaluates the Tauri plugin. */
async function revealInFinder(path: string): Promise<void> {
  const { revealItemInDir } = await import('@tauri-apps/plugin-opener');
  await revealItemInDir(path);
}

export const DocumentRow: React.FC<DocumentRowProps> = ({
  citationId,
  citedSource,
  bridge = aiBridge,
  loadBytes,
  reveal = revealInFinder,
  pickSource = () => pickManuscriptPath(['pdf', 'docx', 'txt', 'md'], 'Source document'),
  onLinked,
}) => {
  const [state, setState] = useState<Linked>({ kind: 'loading' });
  const [viewing, setViewing] = useState(false);
  const [revealError, setRevealError] = useState<string | null>(null);
  /** Non-null while a file is being indexed and embedded into a source. */
  const [linking, setLinking] = useState<string | null>(null);
  const [linkError, setLinkError] = useState<string | null>(null);
  const [justLinked, setJustLinked] = useState<number | null>(null);

  useEffect(() => {
    let alive = true;
    setState({ kind: 'loading' });
    setRevealError(null);
    (async () => {
      try {
        const link = await bridge.citationDocument(citationId);
        if (!alive) return;
        if (!link) {
          setState({ kind: 'none' });
          return;
        }
        // Two calls because they answer two questions: is anything linked, and
        // is the linked thing still on disk. The second can change without the
        // first, which is the whole point of the not-found state.
        const src = await bridge.documentSource(link.documentId);
        if (!alive) return;
        setState({
          kind: 'linked',
          documentId: link.documentId,
          path: src.path,
          exists: src.exists,
        });
      } catch (e) {
        if (alive) setState({ kind: 'error', message: errorText(e) });
      }
    })();
    return () => {
      alive = false;
    };
  }, [bridge, citationId]);

  const linkSource = useCallback(async () => {
    setLinkError(null);
    const path = await pickSource();
    if (!path) return;
    setLinking('Reading the file…');
    try {
      const r = await bridge.linkSourceDocument(citationId, path, citedSource, (ev: LinkSourceEvent) => {
        switch (ev.kind) {
          case 'parsing':
            setLinking('Reading the file…');
            break;
          case 'indexed':
            setLinking(`Indexed ${ev.chunks} passages. Embedding…`);
            break;
          case 'embedding':
            // Embedding a thesis is not instant, and a spinner that says
            // nothing is how a working step gets mistaken for a stuck one.
            setLinking(`Embedding ${ev.done} of ${ev.total} passages…`);
            break;
          case 'linked':
            setLinking('Linking…');
            break;
          default:
            break;
        }
      });
      setState({ kind: 'linked', documentId: r.documentId, path, exists: true });
      setJustLinked(r.checkable ? r.documentId : null);
      if (r.checkable) onLinked?.(r.documentId);
      if (!r.checkable) {
        // Linked but not checkable is a real state, not a success: retrieval
        // needs the vectors, and saying "done" here would send the user back
        // to a check that still cannot run.
        setLinkError(
          `Linked, but ${r.chunksPending} passages are not embedded yet — a support check cannot run until they are.`,
        );
      }
    } catch (e) {
      setLinkError(errorText(e));
    } finally {
      setLinking(null);
    }
  }, [bridge, citationId, citedSource, pickSource, onLinked]);

  return (
    <Card title="Document" data-testid="citation-document-row">
      {state.kind === 'loading' && (
        <p className="gds-ai__hint" data-testid="document-loading">
          Looking for the source document…
        </p>
      )}

      {state.kind === 'error' && (
        <p className="gds-ai__hint" data-testid="document-error" style={{ color: 'var(--g-flagged)' }}>
          {state.message}
        </p>
      )}

      {state.kind === 'none' && (
        <>
          <p className="gds-ai__hint" data-testid="document-none">
            No document linked. Citation support and the source viewer both read
            the cited document, so neither can run until one is.
          </p>
          <Button
            variant="primary"
            onClick={linkSource}
            disabled={linking !== null}
            data-testid="document-link"
          >
            {linking ? 'Linking…' : 'Link document'}
          </Button>
        </>
      )}

      {linking && (
        <p className="gds-ai__hint" data-testid="document-linking">
          {linking}
        </p>
      )}

      {linkError && (
        <p className="gds-ai__hint" data-testid="document-link-error" style={{ color: 'var(--g-flagged)' }}>
          {linkError}
        </p>
      )}

      {justLinked !== null && (
        <p className="gds-ai__hint" data-testid="document-linked-ok">
          Linked and indexed. The sentences citing this source can be checked now.
        </p>
      )}

      {state.kind === 'linked' && (
        <>
          <p className="gds-ai__value" data-testid="document-name">
            {fileName(state.path)}
          </p>
          {!state.exists && (
            // The evidence rows' wording, verbatim. One fact should not have two
            // phrasings depending on which screen noticed it.
            <p className="gds-evidence__missing" data-testid="document-missing">
              file not found — it was moved or renamed since indexing
            </p>
          )}
          <div className="gds-audit__actions">
            <Button
              variant="secondary"
              onClick={() => setViewing(true)}
              disabled={!state.exists}
              data-testid="document-open"
            >
              Open document
            </Button>
            <Button
              variant="secondary"
              onClick={() => {
                setRevealError(null);
                void Promise.resolve(reveal(state.path)).catch((e: unknown) =>
                  setRevealError(errorText(e)),
                );
              }}
              disabled={!state.exists}
              data-testid="document-reveal"
            >
              Reveal in Finder
            </Button>
          </div>
          {revealError && (
            <p className="gds-ai__hint" data-testid="document-reveal-error" style={{ color: 'var(--g-flagged)' }}>
              {revealError}
            </p>
          )}
          {viewing && (
            <React.Suspense fallback={null}>
              <PdfViewer
                open
                documentId={state.documentId}
                // Page 1: opened from the citation, not from a passage, so
                // there is no page to land on and pretending otherwise would
                // put the reader somewhere arbitrary.
                initialPage={1}
                title={citedSource || fileName(state.path)}
                onClose={() => setViewing(false)}
                loadBytes={loadBytes}
              />
            </React.Suspense>
          )}
        </>
      )}
    </Card>
  );
};

export default DocumentRow;
