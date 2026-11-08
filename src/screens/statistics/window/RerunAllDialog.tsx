// Gaply — Project, Run every saved result again… : its dialog (Research OS,
// Phase 8U-2, decisions 537, 538 and 546; ADR-012 Part 9), on the design
// system's ProjectDialog, as OpenProjectDialog is.
//
// * Opened while the list is idle, or done for another project, it reads the
//   open project's saved runs once and says what Run again would do. Nothing
//   runs until Run again is clicked, once.
// * While the list goes, and when it is done, it shows its lines as they come,
//   in a log. Stop cancels the going run and starts no more; Close only closes
//   the dialog, and the list goes on. Escape is Close, in every state.
// * The list itself lives in rerunAll.ts, outside any component, so closing
//   the dialog or leaving the window does not stop it.
import React, { useEffect, useMemo, useRef, useState } from 'react';
import { Button } from '../../../design-system';
import { ProjectDialog } from '../../../design-system/ProjectDialog';
import { runKinds, type RunBridges } from '../../workspace/useWorkspaceSwitch';
import type { ProjectBridge } from '../projects/projectBridge';
import type { StoredRunView } from '../projects/projectTypes';
import { codeSentence, PROJECT_TEXT } from '../projects/projectWording';
import { useGeneration } from '../projects/useGeneration';
import './MethodDialog.css';
import { goingLine, rerunAllStore, rerunList, useRerunAll, type RerunAllDeps } from './rerunAll';
import {
  RERUN_ALL_GOING_NOTE,
  RERUN_ALL_LEFT_OUT,
  RERUN_ALL_MEMORY,
  RERUN_ALL_PREPARED,
  RERUN_ALL_STOP,
  RERUN_ALL_TITLE,
  rerunAllSentence,
} from './windowWording';

export interface RerunAllDialogProps {
  /** The open project's id, from useOpenProject. */
  projectId: string;
  projects: ProjectBridge;
  /** The window's run bridges: each kind's cancel, for Stop. */
  runs: RunBridges;
  /** How often the going rerun is read: the window's runPollMs. */
  pollMs: number;
  /** A data file is open in the window (decision 546's sentence). */
  fileOpen: boolean;
  onClose: () => void;
}

type Read = { kind: 'loading' } | { kind: 'unread' } | { kind: 'ready'; runs: StoredRunView[] };

export const RerunAllDialog: React.FC<RerunAllDialogProps> = ({ projectId, projects, runs, pollMs, fileOpen, onClose }) => {
  const list = useRerunAll();
  const gen = useGeneration();
  const deps = useMemo<RerunAllDeps>(() => ({ bridge: projects, kinds: runKinds(runs), pollMs }), [projects, runs, pollMs]);
  const showsList = list.status === 'going' || (list.status === 'done' && list.projectId === projectId);
  const [read, setRead] = useState<Read | null>(null);
  const [clicked, setClicked] = useState(false);
  const closing = useRef(false);
  const cancelButton = useRef<HTMLButtonElement>(null);
  const stopButton = useRef<HTMLButtonElement>(null);
  const closeButton = useRef<HTMLButtonElement>(null);
  // ProjectDialog reads its initial focus once, after it mounts: this one
  // names the button the state shows at that moment.
  const shown = useRef({ status: list.status, showsList });
  shown.current = { status: list.status, showsList };
  const initialFocus = useMemo(
    () => ({
      get current(): HTMLElement | null {
        const s = shown.current;
        if (s.status === 'going') return stopButton.current;
        if (s.status === 'done' && s.showsList) return closeButton.current;
        return cancelButton.current ?? closeButton.current;
      },
    }),
    []
  );

  // The saved runs, read once, when the dialog has no list to show.
  useEffect(() => {
    if (showsList || read !== null || closing.current) return;
    setRead({ kind: 'loading' });
    const g = gen.next();
    projects.runs(projectId).then(
      (r) => {
        if (gen.is(g)) setRead({ kind: 'ready', runs: r });
      },
      () => {
        if (gen.is(g)) setRead({ kind: 'unread' });
      }
    );
  }, [showsList, read, gen, projects, projectId]);

  // The focus follows the list: to Stop when it starts, to Close when it ends.
  const was = useRef(list.status);
  useEffect(() => {
    if (was.current === list.status) return;
    was.current = list.status;
    if (list.status === 'going') stopButton.current?.focus();
    else if (list.status === 'done') closeButton.current?.focus();
  }, [list.status]);

  const closeDone = () => {
    closing.current = true;
    rerunAllStore.clear();
    onClose();
  };
  const close = () => {
    if (list.status === 'done' && showsList) closeDone();
    else onClose();
  };

  const runAgain = (all: StoredRunView[]) => {
    if (clicked) return;
    setClicked(true);
    rerunAllStore.start(projectId, all, deps);
  };

  let body: React.ReactNode;
  let actions: React.ReactNode;
  if (showsList) {
    body = (
      <>
        <ol className="sw-rerun-all__log" role="log" aria-live="polite" data-testid="sw-rerun-all-log" style={{ listStyle: 'none', margin: 0, padding: 0, display: 'grid', gap: 4 }}>
          {list.lines.map((line, i) => (
            // eslint-disable-next-line react/no-array-index-key
            <li key={i}>{line}</li>
          ))}
          {list.going && <li data-testid="sw-rerun-all-going">{goingLine(list.going)}</li>}
        </ol>
        {list.status === 'going' && (
          <p style={{ margin: '8px 0 0' }} data-testid="sw-rerun-all-going-note">
            {RERUN_ALL_GOING_NOTE}
          </p>
        )}
      </>
    );
    actions = (
      <>
        {list.status === 'going' && (
          <button ref={stopButton} type="button" className="gds-btn gds-btn--secondary" onClick={() => rerunAllStore.stop()} disabled={list.stopped} data-testid="sw-rerun-all-stop">
            {RERUN_ALL_STOP}
          </button>
        )}
        <button ref={closeButton} type="button" className="gds-btn gds-btn--secondary" onClick={close} data-testid="sw-rerun-all-close">
          {PROJECT_TEXT.closeDialog}
        </button>
      </>
    );
  } else if (read === null || read.kind === 'loading') {
    body = <p data-testid="sw-rerun-all-message">{PROJECT_TEXT.runsLoading}</p>;
    actions = (
      <button ref={cancelButton} type="button" className="gds-btn gds-btn--secondary" onClick={onClose} data-testid="sw-rerun-all-cancel">
        {PROJECT_TEXT.cancel}
      </button>
    );
  } else {
    const all = read.kind === 'ready' ? read.runs : [];
    const chosen = rerunList(all);
    const said = read.kind === 'unread' ? PROJECT_TEXT.runsUnread : all.length === 0 ? PROJECT_TEXT.runsEmpty : chosen.length === 0 ? codeSentence('project.rerun_not_succeeded') : null;
    if (said !== null) {
      body = <p data-testid="sw-rerun-all-message">{said}</p>;
      actions = (
        <button ref={closeButton} type="button" className="gds-btn gds-btn--secondary" onClick={onClose} data-testid="sw-rerun-all-close">
          {PROJECT_TEXT.closeDialog}
        </button>
      );
    } else {
      body = (
        <>
          <p data-testid="sw-rerun-all-sentence">{rerunAllSentence(chosen.length)}</p>
          {chosen.some((r) => r.prepared === true) && <p data-testid="sw-rerun-all-prepared">{RERUN_ALL_PREPARED}</p>}
          {chosen.length < all.length && <p data-testid="sw-rerun-all-left-out">{RERUN_ALL_LEFT_OUT}</p>}
          {fileOpen && <p data-testid="sw-rerun-all-memory">{RERUN_ALL_MEMORY}</p>}
        </>
      );
      actions = (
        <>
          <button ref={cancelButton} type="button" className="gds-btn gds-btn--secondary" onClick={onClose} data-testid="sw-rerun-all-cancel">
            {PROJECT_TEXT.cancel}
          </button>
          <Button onClick={() => runAgain(all)} disabled={clicked} data-testid="sw-rerun-all-run">
            {PROJECT_TEXT.runAgain}
          </Button>
        </>
      );
    }
  }

  return (
    <ProjectDialog title={RERUN_ALL_TITLE} onClose={close} initialFocus={initialFocus} testId="sw-rerun-all-dialog">
      {body}
      <div className="sw-dialog__actions">{actions}</div>
    </ProjectDialog>
  );
};

export default RerunAllDialog;
