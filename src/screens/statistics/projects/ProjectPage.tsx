// Gaply — an open project's page (Research OS, Phase 5B-3b; decisions 122,
// 125, 133, 135, 136 and 148 to 151). Reached from Open on the Projects
// panel, at a route that holds the project's id only.
//
// * Shown only when projectsShown() says so, like every project screen, and
//   only while Statistics is active: the page reads the workspace state when
//   it appears and shows nothing of the project otherwise.
// * When it appears it reads which project Rust has open (project_current),
//   the list and the sizes, and for the open project its saved results,
//   newest first. Reads only; it never opens, closes or saves by itself.
//   - The open project: its name (as text, inside <bdi>), its size, and its
//     saved results, each with its kind, status, saved date and closed code.
//   - A listed project that is not open: it says so and offers Open.
//   - An id that is not listed, or a project that cannot be read: a sentence,
//     and no Open.
//   Loading, empty and error states.
// * Open, Close, opening a saved result and each export happen on a click
//   only, with their buttons disabled while the call is pending. Close calls
//   project_close and returns to the Statistics home.
// * A saved result opens read only, in SavedResult.tsx.
// * Export report and Export provenance each open a small dialog with the
//   export's decision 133 sentence. Export renders it, checks its name and
//   its SHA-256 (projectExport.ts) and hands the bytes, unchanged, to today's
//   export path. A refusal shows its sentence; a closed save dialog writes
//   nothing.
// * One generation counter covers every read and action on this page, so a
//   late answer, such as a project_current after an Open, never undoes a
//   newer state.
// * Navigation is the caller's: the page names no route and touches no
//   history (projectGuards.vitest.ts).
import React, { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { Button, Card } from '../../../design-system';
import { saveTextFile } from '../../../utils/saveTextFile';
import { workspaceBridge, type WorkspaceBridge } from '../../workspace/workspaceBridge';
import { isActive } from '../../workspace/workspaceTypes';
import { RUN_STATE_NAMES } from '../describeWording';
import { projectBridge, type ProjectBridge } from './projectBridge';
import { EXPORTS, prepareExport, type ExportKind, type SaveFile } from './projectExport';
import { projectsShown } from './projectGate';
import { currentId, noteCurrent, noteWorkspaceState, openProjectStore } from './projectSession';
import type { ProjectCode, ProjectListEntryView, ProjectSizesView, StoredRunView } from './projectTypes';
import {
  capabilityName,
  EXPORT_WORDS,
  PROJECT_TEXT,
  PROJECT_WORDS,
  projectSentence,
  savedOn,
  sizeSentence,
} from './projectWording';
import ProjectDialog from './ProjectDialog';
import SavedResult from './SavedResult';
import { useGeneration, type Generation } from './useGeneration';

const PROJECT_ID = /^[0-9a-f]{32}$/;

export interface ProjectPageProps {
  /** From the route: the project's id, and nothing else. */
  projectId: string;
  /** After Close: back to the Statistics home. */
  onClosed: () => void;
  /** "Back to Statistics", from a page that shows no open project. */
  onBack: () => void;
  /** Test seams. */
  bridge?: ProjectBridge;
  workspace?: WorkspaceBridge;
  desktop?: boolean;
  save?: SaveFile;
}

export const ProjectPage: React.FC<ProjectPageProps> = (props) =>
  projectsShown(props.desktop) ? <ProjectPageShown {...props} /> : null;

type Runs = StoredRunView[] | 'loading' | 'unread';

type View =
  | { kind: 'loading' }
  | { kind: 'unread' }
  | { kind: 'inactive' }
  | { kind: 'notFound' }
  | { kind: 'unreadable'; code: ProjectCode | null }
  | { kind: 'closed'; entry: ProjectListEntryView }
  | { kind: 'open'; entry: ProjectListEntryView; sizes: ProjectSizesView | null; runs: Runs };

/** Newest first: by saved time, then by number. */
const newestFirst = (runs: StoredRunView[]) =>
  runs.slice().sort((a, b) => b.savedAt - a.savedAt || b.storedRunId - a.storedRunId);

const ProjectPageShown: React.FC<ProjectPageProps> = ({
  projectId,
  onClosed,
  onBack,
  bridge: given,
  workspace: givenWorkspace,
  save = saveTextFile,
}) => {
  const bridge = useMemo(() => given ?? projectBridge(), [given]);
  const workspace = useMemo(() => givenWorkspace ?? workspaceBridge(), [givenWorkspace]);
  const gen = useGeneration();
  const [view, setView] = useState<View>({ kind: 'loading' });
  const [pending, setPending] = useState<'open' | 'close' | null>(null);
  const [message, setMessage] = useState('');
  const [selected, setSelected] = useState<StoredRunView | null>(null);
  const [exporting, setExporting] = useState<ExportKind | null>(null);
  const busy = useRef(false);
  const heading = useRef<HTMLHeadingElement>(null);
  // The button that opened a dialog or a saved result, recorded from its click.
  const opener = useRef<HTMLElement | null>(null);

  // Reads only.
  const load = useCallback(async () => {
    const g = gen.next();
    setView({ kind: 'loading' });
    const [state, current, list, sizes] = await Promise.allSettled([
      workspace.state(),
      bridge.current(),
      bridge.list(),
      bridge.sizes(),
    ]);
    if (!gen.is(g)) return;
    if (state.status !== 'fulfilled' || !isActive(state.value.state, 'statistics')) {
      if (state.status === 'fulfilled') noteWorkspaceState(state.value);
      setView({ kind: 'inactive' });
      return;
    }
    if (list.status !== 'fulfilled') {
      setView({ kind: 'unread' });
      return;
    }
    const answer = current.status === 'fulfilled' ? current.value : null;
    noteCurrent(answer, list.value);
    const entry = PROJECT_ID.test(projectId) ? list.value.find((e) => e.projectId === projectId) : undefined;
    if (!entry) {
      setView({ kind: 'notFound' });
      return;
    }
    if (entry.name === null) {
      setView({ kind: 'unreadable', code: entry.refused });
      return;
    }
    if (currentId(answer) !== projectId) {
      setView({ kind: 'closed', entry });
      return;
    }
    const sized = sizes.status === 'fulfilled' ? sizes.value : null;
    setView({ kind: 'open', entry, sizes: sized, runs: 'loading' });
    try {
      const runs = await bridge.runs(projectId);
      if (gen.is(g)) setView({ kind: 'open', entry, sizes: sized, runs: newestFirst(runs) });
    } catch {
      if (gen.is(g)) setView({ kind: 'open', entry, sizes: sized, runs: 'unread' });
    }
  }, [bridge, workspace, gen, projectId]);

  useEffect(() => {
    void load();
  }, [load]);

  const openIt = async () => {
    if (busy.current) return;
    busy.current = true;
    const g = gen.next();
    setPending('open');
    setMessage('');
    try {
      const r = await bridge.open(projectId);
      if (!gen.is(g)) return;
      openProjectStore.set({ projectId: r.projectId, name: r.name });
      setPending(null);
      void load();
    } catch (e) {
      if (gen.is(g)) setMessage(projectSentence(e));
    } finally {
      busy.current = false;
      if (gen.is(g)) setPending(null);
    }
  };

  const closeIt = async () => {
    if (busy.current) return;
    busy.current = true;
    const g = gen.next();
    setPending('close');
    setMessage('');
    try {
      await bridge.close();
      if (!gen.is(g)) return;
      openProjectStore.clear();
      onClosed();
    } catch (e) {
      if (gen.is(g)) setMessage(projectSentence(e));
    } finally {
      busy.current = false;
      if (gen.is(g)) setPending(null);
    }
  };

  const status = (
    <p role="status" aria-live="polite" data-testid="project-page-message" style={{ margin: 0 }}>
      {message}
    </p>
  );
  const back = (
    <Button variant="secondary" onClick={onBack} data-testid="project-page-back">
      {PROJECT_TEXT.backToStatistics}
    </Button>
  );
  const title = (
    <h3 id="project-page-heading" className="gds-card__title" ref={heading} tabIndex={-1} data-testid="project-page-heading">
      {PROJECT_TEXT.pageHeading}
    </h3>
  );

  let body: React.ReactNode;
  switch (view.kind) {
    case 'loading':
      body = <p data-testid="project-page-loading">{PROJECT_TEXT.pageLoading}</p>;
      break;
    case 'unread':
      body = (
        <>
          <p role="alert" data-testid="project-page-unread">
            {PROJECT_TEXT.pageUnread}
          </p>
          <Button variant="secondary" onClick={() => void load()} data-testid="project-page-retry">
            {PROJECT_TEXT.tryAgain}
          </Button>
          {back}
        </>
      );
      break;
    case 'inactive':
      body = (
        <>
          <p data-testid="project-page-inactive">{PROJECT_TEXT.pageInactive}</p>
          {back}
        </>
      );
      break;
    case 'notFound':
      body = (
        <>
          <p data-testid="project-page-not-found">{PROJECT_TEXT.pageNotFound}</p>
          {back}
        </>
      );
      break;
    case 'unreadable':
      body = (
        <>
          <p data-testid="project-page-unreadable">
            {PROJECT_TEXT.unreadable} {view.code ? PROJECT_WORDS[view.code]({ code: view.code }) : ''}
          </p>
          {back}
        </>
      );
      break;
    case 'closed':
      body = (
        <>
          <p>
            <strong>
              <bdi data-testid="project-page-name">{view.entry.name}</bdi>
            </strong>
          </p>
          <p data-testid="project-page-not-open">{PROJECT_TEXT.pageNotOpen}</p>
          <div style={{ display: 'flex', gap: 8, flexWrap: 'wrap' }}>
            <Button onClick={() => void openIt()} disabled={pending !== null} data-testid="project-page-open">
              {pending === 'open' ? PROJECT_TEXT.opening : PROJECT_TEXT.open}
            </Button>
            {back}
          </div>
        </>
      );
      break;
    case 'open':
      body = (
        <OpenProject
          view={view}
          projectId={projectId}
          bridge={bridge}
          gen={gen}
          pending={pending}
          selected={selected}
          onSelect={(run, from) => {
            opener.current = from;
            setSelected(run);
          }}
          onDeselect={() => {
            setSelected(null);
            // Focus back to the result's own Open button, once the list shows.
            window.setTimeout(() => {
              if (opener.current && document.contains(opener.current)) opener.current.focus();
            }, 0);
          }}
          onExport={(kind, from) => {
            opener.current = from;
            setMessage('');
            setExporting(kind);
          }}
          onClose={() => void closeIt()}
        />
      );
      break;
  }

  return (
    <Card>
      <section aria-labelledby="project-page-heading" data-testid="project-page" style={{ display: 'grid', gap: 8 }}>
        {title}
        {status}
        {body}
      </section>
      {exporting && view.kind === 'open' && (
        <ExportDialog
          kind={exporting}
          projectId={projectId}
          bridge={bridge}
          gen={gen}
          save={save}
          openerAfter={() => opener.current}
          onDone={(said) => {
            setExporting(null);
            if (said !== null) setMessage(said);
          }}
        />
      )}
    </Card>
  );
};

interface OpenProjectProps {
  view: Extract<View, { kind: 'open' }>;
  projectId: string;
  bridge: ProjectBridge;
  gen: Generation;
  pending: 'open' | 'close' | null;
  selected: StoredRunView | null;
  onSelect: (run: StoredRunView, from: HTMLElement) => void;
  onDeselect: () => void;
  onExport: (kind: ExportKind, from: HTMLElement) => void;
  onClose: () => void;
}

const OpenProject: React.FC<OpenProjectProps> = ({
  view,
  projectId,
  bridge,
  gen,
  pending,
  selected,
  onSelect,
  onDeselect,
  onExport,
  onClose,
}) => (
  <>
    <p style={{ margin: 0 }}>
      <strong>
        <bdi data-testid="project-page-name">{view.entry.name}</bdi>
      </strong>
    </p>
    <p data-testid="project-page-size" style={{ margin: 0 }}>
      {sizeSentence(view.entry.bytes)}
    </p>
    <div style={{ display: 'flex', gap: 8, flexWrap: 'wrap' }}>
      {(['report', 'provenance'] as const).map((k) => (
        <Button
          key={k}
          variant="secondary"
          onClick={(e) => onExport(k, e.currentTarget)}
          disabled={pending !== null}
          data-testid={`project-export-${k}`}
        >
          {EXPORTS[k].button}
        </Button>
      ))}
      <Button variant="secondary" onClick={onClose} disabled={pending !== null} data-testid="project-page-close">
        {pending === 'close' ? PROJECT_TEXT.closing : PROJECT_TEXT.close}
      </Button>
    </div>
    {selected ? (
      <>
        <SavedResult projectId={projectId} run={selected} bridge={bridge} gen={gen} />
        <div>
          <Button variant="secondary" onClick={onDeselect} data-testid="saved-result-back">
            {PROJECT_TEXT.backToResults}
          </Button>
        </div>
      </>
    ) : (
      <section aria-label={PROJECT_TEXT.savedResults} data-testid="project-runs">
        <h4 style={{ margin: '8px 0 4px' }}>{PROJECT_TEXT.savedResults}</h4>
        {view.runs === 'loading' && <p data-testid="project-runs-loading">{PROJECT_TEXT.runsLoading}</p>}
        {view.runs === 'unread' && (
          <p role="alert" data-testid="project-runs-unread">
            {PROJECT_TEXT.runsUnread}
          </p>
        )}
        {Array.isArray(view.runs) && view.runs.length === 0 && <p data-testid="project-runs-empty">{PROJECT_TEXT.runsEmpty}</p>}
        {Array.isArray(view.runs) && view.runs.length > 0 && (
          <ol data-testid="project-runs-list" style={{ margin: 0, paddingLeft: 20, display: 'grid', gap: 6 }}>
            {view.runs.map((r) => (
              <li key={r.storedRunId} data-testid={`project-run-${r.storedRunId}`}>
                <span data-testid={`project-run-facts-${r.storedRunId}`}>
                  {capabilityName(r.capabilityId)}. {RUN_STATE_NAMES[r.state] ?? PROJECT_TEXT.unknownState}. {PROJECT_TEXT.savedOn}{' '}
                  {savedOn(r.savedAt)}.{r.endCode ? ` ${PROJECT_WORDS[r.endCode]?.({ code: r.endCode }) ?? ''}` : ''}
                </span>{' '}
                <Button
                  variant="ghost"
                  onClick={(e) => onSelect(r, e.currentTarget)}
                  data-testid={`project-run-open-${r.storedRunId}`}
                >
                  {PROJECT_TEXT.openResult}
                </Button>
              </li>
            ))}
          </ol>
        )}
      </section>
    )}
  </>
);

interface ExportDialogProps {
  kind: ExportKind;
  projectId: string;
  bridge: ProjectBridge;
  gen: Generation;
  save: SaveFile;
  openerAfter: () => HTMLElement | null;
  /** Closes the dialog; `said` is the page's status line, or null. */
  onDone: (said: string | null) => void;
}

const ExportDialog: React.FC<ExportDialogProps> = ({ kind, projectId, bridge, gen, save, openerAfter, onDone }) => {
  const [pending, setPending] = useState(false);
  const [refusal, setRefusal] = useState<string | null>(null);
  const inFlight = useRef(false);
  const cancelButton = useRef<HTMLButtonElement>(null);
  const returnFocus = useRef<(() => HTMLElement | null) | null>(openerAfter);
  const e = EXPORTS[kind];

  const cancel = () => {
    if (!pending) onDone(null);
  };

  // On the Export click only: render, check, then today's export path.
  const run = async () => {
    if (inFlight.current) return;
    inFlight.current = true;
    const g = gen.next();
    setPending(true);
    setRefusal(null);
    try {
      let rendered: unknown;
      try {
        rendered = kind === 'report' ? await bridge.report(projectId) : await bridge.provenance(projectId);
      } catch (err) {
        if (gen.is(g)) setRefusal(projectSentence(err));
        return;
      }
      if (!gen.is(g)) return;
      const ready = await prepareExport(rendered, kind);
      if (!gen.is(g)) return;
      if (!ready.ok) {
        setRefusal(ready.sentence);
        return;
      }
      let where: string | null;
      try {
        where = await save(ready.fileName, ready.text, e.mime);
      } catch {
        if (gen.is(g)) setRefusal(EXPORT_WORDS.writeFailed);
        return;
      }
      if (!gen.is(g)) return;
      onDone(where === null ? PROJECT_TEXT.exportCancelled : EXPORT_WORDS.saved(ready.sha256));
    } finally {
      inFlight.current = false;
      if (gen.is(g)) setPending(false);
    }
  };

  return (
    <ProjectDialog title={e.title} onClose={cancel} initialFocus={cancelButton} returnFocus={returnFocus} testId={`project-export-dialog`}>
      <p data-testid="project-export-sentence">{e.sentence}</p>
      {refusal && (
        <p role="alert" data-testid="project-export-refusal">
          {refusal}
        </p>
      )}
      <p role="status" aria-live="polite" data-testid="project-export-pending" style={{ margin: '4px 0' }}>
        {pending ? PROJECT_TEXT.exporting : ''}
      </p>
      <div style={{ display: 'flex', gap: 8, flexWrap: 'wrap' }}>
        <button
          ref={cancelButton}
          type="button"
          className="gds-btn gds-btn--secondary"
          onClick={cancel}
          disabled={pending}
          data-testid="project-export-cancel"
        >
          {PROJECT_TEXT.cancel}
        </button>
        <Button onClick={() => void run()} disabled={pending} data-testid="project-export-confirm">
          {PROJECT_TEXT.exportConfirm}
        </Button>
      </div>
    </ProjectDialog>
  );
};

export default ProjectPage;
