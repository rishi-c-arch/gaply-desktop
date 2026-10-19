// Gaply — the Projects panel on the Statistics home, and its delete dialog
// (Research OS, Phase 5B-3a; decisions 103, 104, 107, 120, 124, 132 to 136).
// Open comes in 5B-3b.
//
// * Shown only when projectsShown() says so: both flags on, desktop app.
// * Everything comes from project_list and project_sizes, read when the
//   panel appears and after a delete; both are exempt from the workspace
//   check (decision 120), so the panel works with no workspace active.
// * Each project: its name (as text, inside <bdi>), size, saved date and
//   number of saved runs; one that cannot be read shows its code's sentence
//   and its size, and can still be deleted. The total against both limits,
//   in MiB, the unit named once. Loading, empty and error states.
// * "Open now" marks the project this window saved into last (decision
//   142), and never while Statistics is not active (decision 135): the
//   panel reads the workspace state when it appears.
// * Delete opens a dialog: the name, the size and decision 133's sentence;
//   Cancel has the focus, Escape cancels. Its Delete calls prepare delete,
//   then delete, on that click only; a refusal shows its sentence (another
//   copy of the app having it open included) and the dialog stays open.
//   After a delete, focus moves to the panel's heading.
import React, { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { Button, Card } from '../../../design-system';
import { workspaceBridge, type WorkspaceBridge } from '../../workspace/workspaceBridge';
import { projectBridge, type ProjectBridge } from './projectBridge';
import { projectsShown } from './projectGate';
import { noteDeleted, noteWorkspaceState, useOpenProject } from './projectSession';
import type { ProjectListEntryView, ProjectSizesView } from './projectTypes';
import { DELETE_SENTENCE, PROJECT_TEXT, PROJECT_WORDS, projectSentence, savedOn, sizeSentence, totalsSentence } from './projectWording';
import ProjectDialog from './ProjectDialog';

export interface ProjectsPanelProps {
  /** Test seams. */
  bridge?: ProjectBridge;
  workspace?: WorkspaceBridge;
  desktop?: boolean;
}

export const ProjectsPanel: React.FC<ProjectsPanelProps> = (props) =>
  projectsShown(props.desktop) ? <ProjectsPanelShown {...props} /> : null;

type Listed = { list: ProjectListEntryView[]; sizes: ProjectSizesView };

const ProjectsPanelShown: React.FC<ProjectsPanelProps> = ({ bridge: given, workspace: givenWorkspace }) => {
  const bridge = useMemo(() => given ?? projectBridge(), [given]);
  const workspace = useMemo(() => givenWorkspace ?? workspaceBridge(), [givenWorkspace]);
  const open = useOpenProject();
  const [listed, setListed] = useState<Listed | null>(null);
  const [unread, setUnread] = useState(false);
  const [target, setTarget] = useState<ProjectListEntryView | null>(null);
  const [message, setMessage] = useState('');
  const alive = useRef(true);
  const heading = useRef<HTMLHeadingElement>(null);
  // The Delete button that opened the dialog, recorded from its click.
  const opener = useRef<HTMLElement | null>(null);

  useEffect(() => {
    alive.current = true;
    return () => {
      alive.current = false;
    };
  }, []);

  const read = useCallback(async () => {
    try {
      const [list, sizes] = await Promise.all([bridge.list(), bridge.sizes()]);
      if (!alive.current) return;
      setListed({ list, sizes });
      setUnread(false);
    } catch {
      if (alive.current) setUnread(true);
    }
  }, [bridge]);

  // Reads only: the list, the sizes and the workspace state.
  useEffect(() => {
    void read();
    workspace.state().then(
      (s) => noteWorkspaceState(s),
      () => undefined
    );
  }, [read, workspace]);

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
              {totalsSentence(listed.sizes.totalBytes, listed.sizes.projectCapBytes, listed.sizes.allProjectsCapBytes)}
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
                    <Button
                      variant="ghost"
                      onClick={(e) => {
                        opener.current = e.currentTarget;
                        setMessage('');
                        setTarget(p);
                      }}
                      data-testid={`project-delete-${p.projectId}`}
                    >
                      {PROJECT_TEXT.delete}
                    </Button>
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
  headingAfter: () => HTMLElement | null;
  openerAfter: () => HTMLElement | null;
  onCancel: () => void;
  onDeleted: () => void;
}

const DeleteDialog: React.FC<DeleteDialogProps> = ({ project, bridge, headingAfter, openerAfter, onCancel, onDeleted }) => {
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

  const cancel = () => {
    if (!pending) onCancel();
  };

  const remove = async () => {
    if (inFlight.current) return;
    inFlight.current = true;
    setPending(true);
    setRefusal(null);
    try {
      const c = await bridge.prepareDelete(project.projectId);
      if (!alive.current) return;
      const r = await bridge.delete(project.projectId, c.confirmation);
      if (!alive.current) return;
      noteDeleted(r.projectId);
      done.current = true;
      onDeleted();
    } catch (e) {
      if (!alive.current) return;
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
          {PROJECT_TEXT.cancel}
        </button>
        <Button onClick={() => void remove()} disabled={pending} data-testid="project-delete-confirm">
          {PROJECT_TEXT.deleteConfirm}
        </Button>
      </div>
    </ProjectDialog>
  );
};

export default ProjectsPanel;
