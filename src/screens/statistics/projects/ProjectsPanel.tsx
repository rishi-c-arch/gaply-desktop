// Gaply — the Projects panel on the Statistics home, and its delete dialog
// (Research OS, Phases 5B-3a and 5B-3b; decisions 103, 104, 107, 120, 124,
// 132 to 136, 148 and 149).
//
// * Shown only when projectsShown() says so: both flags on, desktop app.
// * Everything comes from project_list and project_sizes, read when the
//   panel appears and after a delete; both are exempt from the workspace
//   check (decision 120), so the panel works with no workspace active.
// * Each project: its name (as text, inside <bdi>), size, saved date and
//   number of saved runs; one that cannot be read shows its code's sentence
//   and its size, and can still be deleted. The total against both limits,
//   in decimal MB (decisions 163 and 174). Loading, empty and error states.
// * "Open now" marks the project Rust says is open: the panel asks
//   project_current when it appears (decision 148), and never marks one
//   while Statistics is not active (decision 135): it reads the workspace
//   state too.
// * Open (decision 149), on each project that can be read: project_open on
//   that click only, then the project's page. A refusal shows its sentence.
// * One generation counter covers every read and action here, the delete
//   dialog's included: a late answer never overwrites a newer one.
// * Delete opens a dialog: the name, the size and decision 133's sentence;
//   Cancel has the focus, Escape cancels. Its Delete calls prepare delete,
//   then delete, on that click only; a refusal shows its sentence (another
//   copy of the app having it open included) and the dialog stays open;
//   Cancel then reads Close and takes the focus (decision 175).
//   After a delete, focus moves to the panel's heading.
// * Phase 5C-1b (decision 199): when project_current, read with the rest,
//   says a rerun is going in a project, that project's Delete… is disabled,
//   with project.rerun_in_progress's sentence beside it, until the panel is
//   next read.
import React, { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { Button, Card } from '../../../design-system';
import { workspaceBridge, type WorkspaceBridge } from '../../workspace/workspaceBridge';
import { projectBridge, type ProjectBridge } from './projectBridge';
import { projectsShown } from './projectGate';
import { currentId, noteCurrent, noteDeleted, noteWorkspaceState, openProjectStore, useOpenProject } from './projectSession';
import type { ProjectListEntryView, ProjectSizesView } from './projectTypes';
import { DELETE_SENTENCE, PROJECT_TEXT, PROJECT_WORDS, projectSentence, savedOn, sizeSentence, totalsSentence } from './projectWording';
import ProjectDialog from './ProjectDialog';
import { useGeneration, type Generation } from './useGeneration';

export interface ProjectsPanelProps {
  /** Test seams. */
  bridge?: ProjectBridge;
  workspace?: WorkspaceBridge;
  desktop?: boolean;
  /** After an Open: where the project's page is (the Statistics home passes
   *  its navigation). Without it the panel only marks the project open. */
  onOpened?: (projectId: string) => void;
}

export const ProjectsPanel: React.FC<ProjectsPanelProps> = (props) =>
  projectsShown(props.desktop) ? <ProjectsPanelShown {...props} /> : null;

type Listed = { list: ProjectListEntryView[]; sizes: ProjectSizesView };

const ProjectsPanelShown: React.FC<ProjectsPanelProps> = ({ bridge: given, workspace: givenWorkspace, onOpened }) => {
  const bridge = useMemo(() => given ?? projectBridge(), [given]);
  const workspace = useMemo(() => givenWorkspace ?? workspaceBridge(), [givenWorkspace]);
  const open = useOpenProject();
  const gen = useGeneration();
  const [listed, setListed] = useState<Listed | null>(null);
  const [unread, setUnread] = useState(false);
  const [target, setTarget] = useState<ProjectListEntryView | null>(null);
  const [message, setMessage] = useState('');
  const [opening, setOpening] = useState<string | null>(null);
  // The project a rerun was going in when the panel was last read.
  const [rerunIn, setRerunIn] = useState<string | null>(null);
  const openInFlight = useRef(false);
  const heading = useRef<HTMLHeadingElement>(null);
  // The Delete button that opened the dialog, recorded from its click.
  const opener = useRef<HTMLElement | null>(null);

  // Reads only: the list, the sizes, which project Rust has open, and the
  // workspace state. A failed project_current means none is open.
  const read = useCallback(async () => {
    const g = gen.next();
    const [list, sizes, current, state] = await Promise.allSettled([
      bridge.list(),
      bridge.sizes(),
      bridge.current(),
      workspace.state(),
    ]);
    if (!gen.is(g)) return;
    const entries = list.status === 'fulfilled' ? list.value : null;
    const answer = current.status === 'fulfilled' ? current.value : null;
    noteCurrent(answer, entries);
    setRerunIn(answer && typeof answer === 'object' && answer.rerun?.status === 'going' ? currentId(answer) : null);
    if (state.status === 'fulfilled') noteWorkspaceState(state.value);
    if (entries !== null && sizes.status === 'fulfilled') {
      setListed({ list: entries, sizes: sizes.value });
      setUnread(false);
    } else {
      setUnread(true);
    }
  }, [bridge, workspace, gen]);

  useEffect(() => {
    void read();
  }, [read]);

  // Open, on its click only (decision 149): then the project's page.
  const openProject = async (projectId: string) => {
    if (openInFlight.current) return;
    openInFlight.current = true;
    const g = gen.next();
    setOpening(projectId);
    setMessage('');
    try {
      const r = await bridge.open(projectId);
      if (!gen.is(g)) return;
      openProjectStore.set({ projectId: r.projectId, name: r.name });
      onOpened?.(r.projectId);
    } catch (e) {
      if (gen.is(g)) setMessage(projectSentence(e));
    } finally {
      openInFlight.current = false;
      if (gen.is(g)) setOpening(null);
    }
  };

  const deleted = useCallback(() => {
    setTarget(null);
    setMessage(PROJECT_TEXT.deleted);
    void read();
  }, [read]);

  return (
    <Card>
      <section aria-labelledby="projects-heading" data-testid="projects-panel">
        <h3 id="projects-heading" ref={heading} tabIndex={-1} className="gds-card__title" data-testid="projects-heading">
          {PROJECT_TEXT.panelHeading}
        </h3>
        <p role="status" aria-live="polite" data-testid="projects-message" style={{ margin: 0 }}>
          {message}
        </p>
        {unread && (
          <p role="alert" data-testid="projects-unread">
            {PROJECT_TEXT.unread}
          </p>
        )}
        {!listed && !unread && <p data-testid="projects-loading">{PROJECT_TEXT.loading}</p>}
        {listed && (
          <>
            <p data-testid="projects-totals">
              {totalsSentence(listed.sizes.totalBytes)}
            </p>
            {listed.list.length === 0 ? (
              <p data-testid="projects-empty">{PROJECT_TEXT.empty}</p>
            ) : (
              <ul data-testid="projects-list" style={{ listStyle: 'none', padding: 0, margin: 0, display: 'grid', gap: 8 }}>
                {listed.list.map((p) => (
                  <li key={p.projectId} data-testid={`project-row-${p.projectId}`}>
                    {p.name !== null ? (
                      <strong>
                        <bdi data-testid={`project-name-${p.projectId}`}>{p.name}</bdi>
                      </strong>
                    ) : (
                      <span data-testid={`project-unreadable-${p.projectId}`}>
                        {PROJECT_TEXT.unreadable} {p.refused ? PROJECT_WORDS[p.refused]({ code: p.refused }) : ''}
                      </span>
                    )}
                    {open?.projectId === p.projectId && (
                      <span data-testid={`project-open-${p.projectId}`}> ({PROJECT_TEXT.openNow})</span>
                    )}
                    <br />
                    <span data-testid={`project-facts-${p.projectId}`}>
                      {sizeSentence(p.bytes)}
                      {p.savedAt !== null && ` ${PROJECT_TEXT.savedOn} ${savedOn(p.savedAt)}.`}
                      {p.runCount !== null && ` ${PROJECT_TEXT.results(p.runCount)}.`}
                    </span>{' '}
                    {p.name !== null && (
                      <Button
                        variant="secondary"
                        onClick={() => void openProject(p.projectId)}
                        disabled={opening !== null}
                        data-testid={`project-open-button-${p.projectId}`}
                      >
                        {opening === p.projectId ? PROJECT_TEXT.opening : PROJECT_TEXT.open}
                      </Button>
                    )}{' '}
                    <Button
                      variant="secondary"
                      onClick={(e) => {
                        opener.current = e.currentTarget;
                        setMessage('');
                        setTarget(p);
                      }}
                      disabled={rerunIn === p.projectId}
                      data-testid={`project-delete-${p.projectId}`}
                    >
                      {PROJECT_TEXT.delete}
                    </Button>
                    {rerunIn === p.projectId && (
                      <span data-testid={`project-delete-going-${p.projectId}`}>
                        {' '}
                        {PROJECT_WORDS['project.rerun_in_progress']({ code: 'project.rerun_in_progress' })}
                      </span>
                    )}
                  </li>
                ))}
              </ul>
            )}
          </>
        )}
      </section>
      {target && (
        <DeleteDialog
          project={target}
          bridge={bridge}
          gen={gen}
          headingAfter={() => heading.current}
          openerAfter={() => opener.current}
          onCancel={() => setTarget(null)}
          onDeleted={deleted}
        />
      )}
    </Card>
  );
};

interface DeleteDialogProps {
  project: ProjectListEntryView;
  bridge: ProjectBridge;
  /** The panel's counter: the dialog is part of the panel's screen. */
  gen: Generation;
  headingAfter: () => HTMLElement | null;
  openerAfter: () => HTMLElement | null;
  onCancel: () => void;
  onDeleted: () => void;
}

const DeleteDialog: React.FC<DeleteDialogProps> = ({ project, bridge, gen, headingAfter, openerAfter, onCancel, onDeleted }) => {
  const [pending, setPending] = useState(false);
  const [refusal, setRefusal] = useState<string | null>(null);
  const alive = useRef(true);
  const inFlight = useRef(false);
  const cancelButton = useRef<HTMLButtonElement>(null);
  // Focus goes back to the Delete button that opened it, unless the project
  // was deleted: then to the panel's heading.
  const done = useRef(false);
  const returnFocus = useRef<(() => HTMLElement | null) | null>(() => (done.current ? headingAfter() : openerAfter()));

  useEffect(() => {
    alive.current = true;
    return () => {
      alive.current = false;
    };
  }, []);

  // After a refusal the dialog stays open so it can be read, and its Cancel
  // reads Close and takes the focus once it is enabled again (decision 175).
  useEffect(() => {
    if (refusal !== null && !pending) cancelButton.current?.focus();
  }, [refusal, pending]);

  const cancel = () => {
    if (!pending) onCancel();
  };

  const remove = async () => {
    if (inFlight.current) return;
    inFlight.current = true;
    const g = gen.next();
    setPending(true);
    setRefusal(null);
    try {
      const c = await bridge.prepareDelete(project.projectId);
      if (!alive.current || !gen.is(g)) return;
      const r = await bridge.delete(project.projectId, c.confirmation);
      if (!alive.current || !gen.is(g)) return;
      noteDeleted(r.projectId);
      done.current = true;
      onDeleted();
    } catch (e) {
      if (!alive.current || !gen.is(g)) return;
      setRefusal(projectSentence(e));
    } finally {
      inFlight.current = false;
      if (alive.current) setPending(false);
    }
  };

  return (
    <ProjectDialog
      title={PROJECT_TEXT.deleteTitle}
      onClose={cancel}
      initialFocus={cancelButton}
      returnFocus={returnFocus}
      testId="project-delete-dialog"
    >
      <p data-testid="project-delete-name">
        {project.name !== null ? <bdi>{project.name}</bdi> : PROJECT_TEXT.unnamed}
      </p>
      <p data-testid="project-delete-size">{sizeSentence(project.bytes)}</p>
      <p data-testid="project-delete-sentence">{DELETE_SENTENCE}</p>
      {refusal && (
        <p role="alert" data-testid="project-delete-refusal">
          {refusal}
        </p>
      )}
      <p role="status" aria-live="polite" data-testid="project-delete-pending" style={{ margin: '4px 0' }}>
        {pending ? PROJECT_TEXT.deleting : ''}
      </p>
      <div style={{ display: 'flex', gap: 8, flexWrap: 'wrap' }}>
        <button
          ref={cancelButton}
          type="button"
          className="gds-btn gds-btn--secondary"
          onClick={cancel}
          disabled={pending}
          data-testid="project-delete-cancel"
        >
          {refusal !== null ? PROJECT_TEXT.closeDialog : PROJECT_TEXT.cancel}
        </button>
        <Button onClick={() => void remove()} disabled={pending} data-testid="project-delete-confirm">
          {PROJECT_TEXT.deleteConfirm}
        </Button>
      </div>
    </ProjectDialog>
  );
};

export default ProjectsPanel;
