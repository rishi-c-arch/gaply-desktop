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
import { aiBridge, errorText, ImportPreflight, LinkSourceEvent, OaFetchReport } from './aiBridge';
import { describeOaOutcome, describeOaPhase, isFetchSuccess, oaFetchDisclosure } from './oaOutcome';
import { pickManuscriptPath } from '../common/pickFile';
import { mayUseCloud } from '../settings/settingsStore';

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
  bridge?: Pick<
    typeof aiBridge,
    | 'citationDocument'
    | 'documentSource'
    | 'linkSourceDocument'
    | 'fetchOpenAccess'
    | 'importPreflight'
    // §11 D115. Stops an embed already running under `linkSourceDocument`.
    | 'cancelEmbedding'
  >;
  /** The citation's DOI. Without one there is nothing to look up, and the
   *  open-access action is not offered rather than offered and refused. */
  doi?: string | null;
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
  doi,
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
  /** §11 D115. Set once the user has asked to stop an embed. The flag is
   *  read between batches, so the current one finishes — saying "Stopping…"
   *  rather than "Stopped" is the honest report of that. */
  const [stopping, setStopping] = useState(false);
  /** Non-null while an open-access fetch is running. */
  const [fetching, setFetching] = useState(false);
  /** §11 D105. What the fetch is doing right now. A 37-page paper spends ~90
   *  seconds between the press and the outcome, and a button that says
   *  "Looking for a free copy…" for all of it is indistinguishable from a stuck
   *  one — which is how this feature came to be reported as broken. */
  const [phase, setPhase] = useState<string | null>(null);
  const [fetchNote, setFetchNote] = useState<string | null>(null);
  /** Set when the import guard wants an answer before spending the time. */
  const [pendingConfirm, setPendingConfirm] = useState<{ path: string; pre: ImportPreflight } | null>(
    null,
  );
  /** The estimate for an import that is allowed to proceed. */
  const [estimate, setEstimate] = useState<string | null>(null);

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

  /** The indexing half, once the guard has been satisfied. */
  const runLink = useCallback(
    async (path: string, confirmed: boolean) => {
    setLinkError(null);
    setPendingConfirm(null);
    setStopping(false);
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
      }, confirmed);
      // The backend enforces the gate too, so it can still come back asking —
      // and that answer is not an error, it is the question being put again.
      if (r.outcome === 'confirmationRequired' && r.preflight) {
        setPendingConfirm({ path, pre: r.preflight });
        return;
      }
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
      setStopping(false);
    }
    },
    [bridge, citationId, citedSource, onLinked],
  );

  const linkSource = useCallback(async () => {
    setLinkError(null);
    setEstimate(null);
    setPendingConfirm(null);
    const path = await pickSource();
    if (!path) return;
    // Ask what it will cost BEFORE committing to it. A scanned PDF is refused
    // here too, which is the point: the OCR advice arrives before any indexing
    // time is spent rather than after a progress bar has run.
    let pre: ImportPreflight;
    try {
      pre = await bridge.importPreflight(path);
    } catch (e) {
      setLinkError(errorText(e));
      return;
    }
    if (pre.verdict === 'refused') {
      setLinkError(pre.summary);
      return;
    }
    if (pre.verdict === 'confirmationRequired') {
      setPendingConfirm({ path, pre });
      return;
    }
    setEstimate(pre.summary);
    await runLink(path, false);
  }, [bridge, pickSource, runLink]);

  const fetchOpenAccess = useCallback(async () => {
    setFetchNote(null);
    setLinkError(null);
    // The SAME consent gate the batch action reads. An outbound lookup is an
    // outbound lookup: a user who turned this off in Settings did not turn it
    // off only for the toolbar button.
    if (!mayUseCloud('citation_verification')) {
      setFetchNote(
        'Citation verification is turned off in Settings → Sync & Privacy, so nothing was looked up.',
      );
      return;
    }
    setFetching(true);
    setPhase('Looking for a free copy…');
    try {
      const [report] = await bridge.fetchOpenAccess([{ kind: 'citation', citationId }], (ev) => {
        if (ev.kind === 'phase') setPhase(describeOaPhase(ev.phase));
      });
      if (!report) {
        setFetchNote('The fetch returned no result.');
        return;
      }
      // The sentence comes from the shared vocabulary, so this row and the
      // batch report never describe the same outcome two different ways.
      setFetchNote(describeOaOutcome(report));
      if (isFetchSuccess(report) && report.documentId != null) {
        const src = await bridge.documentSource(report.documentId);
        setState({
          kind: 'linked',
          documentId: report.documentId,
          path: src.path,
          exists: src.exists,
        });
        if (report.checkable) onLinked?.(report.documentId);
      }
    } catch (e) {
      setFetchNote(errorText(e));
    } finally {
      setFetching(false);
      setPhase(null);
    }
  }, [bridge, citationId, onLinked]);

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
          <div className="gds-audit__actions">
            <Button
              variant="primary"
              onClick={linkSource}
              disabled={linking !== null || fetching}
              data-testid="document-link"
            >
              {linking ? 'Linking…' : 'Link document'}
            </Button>
            {/* Offered only with a DOI: the lookup is BY DOI, and a button that
                can only fail is worse than one that is not there. */}
            {doi && (
              <Button
                variant="secondary"
                onClick={fetchOpenAccess}
                disabled={fetching || linking !== null}
                data-testid="document-fetch-oa"
              >
                {fetching ? 'Looking…' : 'Fetch open-access PDF'}
              </Button>
            )}
          </div>
          {doi && (
            <p className="gds-ai__hint" data-testid="document-fetch-oa-note">
              {oaFetchDisclosure(1)}
            </p>
          )}
        </>
      )}

      {fetchNote && (
        <p className="gds-ai__hint" data-testid="document-fetch-result">
          {fetchNote}
        </p>
      )}

      {estimate && !linking && !pendingConfirm && (
        <p className="gds-ai__hint" data-testid="document-estimate">
          {estimate}
        </p>
      )}

      {pendingConfirm && (
        <div data-testid="document-confirm">
          <p className="gds-ai__hint">{pendingConfirm.pre.summary}</p>
          <div className="gds-audit__actions">
            <Button
              variant="primary"
              onClick={() => void runLink(pendingConfirm.path, true)}
              data-testid="document-confirm-yes"
            >
              Index it
            </Button>
            <Button
              variant="secondary"
              onClick={() => setPendingConfirm(null)}
              data-testid="document-confirm-no"
            >
              Cancel
            </Button>
          </div>
        </div>
      )}

      {/* §11 D105. Same shape as the `linking` line below, for the same
          reason: the button says a state, this says what is happening in it. */}
      {fetching && phase && (
        <p className="gds-ai__hint" data-testid="document-fetching">
          {phase}
        </p>
      )}

      {/* §11 D115. Indexing and embedding a thesis runs for minutes and streams
          "Embedding 32 of 113 passages…" the whole time. There was no way to
          stop it: `ai_link_source_document` reads the shared cancel flag, but
          the only command that SETS it was not on the IPC surface and no
          surface offered the action. A live progress indicator with no stop is
          the defect; the unreferenced command was the symptom.

          Offered for LINKING only. The open-access fetch embeds too, but its
          loop never reads the flag, so a cancel there would be a button that
          cannot succeed (§11 D101). */}
      {linking && (
        <div className="gds-ai__row" data-testid="document-linking-row">
          <p className="gds-ai__hint" data-testid="document-linking">
            {linking}
          </p>
          <Button
            variant="ghost"
            disabled={stopping}
            data-testid="document-link-cancel"
            onClick={async () => {
              setStopping(true);
              try {
                await bridge.cancelEmbedding();
              } catch (e) {
                // Failing to STOP is worth saying: the run continues.
                setStopping(false);
                setLinkError(`Could not stop the run: ${errorText(e)}`);
              }
            }}
          >
            {stopping ? 'Stopping…' : 'Stop'}
          </Button>
        </div>
      )}
      {stopping && (
        <p className="gds-ai__hint" data-testid="document-stopping-note">
          Finishing the batch in progress, then stopping. What has already been
          embedded is kept, and the source stays linked — it just will not be
          checkable until the rest is embedded.
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
