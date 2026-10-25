// Gaply — Save to a project (Research OS, Phases 5B-3a and 5B-3b; decisions
// 105, 121 to 123, 133, 136, 140, 142 and 148). A button on the summary's and
// the t-test's screens, for a run that Succeeded or Failed with its code, and
// the dialog it opens.
//
// * Shown only when projectsShown() says so: both flags on, desktop app.
// * Two choices: a new project, named here, or the open project. When the
//   dialog appears it asks Rust which project is open (project_current, and
//   project_list for its name; decision 148) and offers that one, or none.
//   Save stays disabled until the choice is complete; a name the rule
//   refuses shows its sentence before any call.
// * One generation counter covers the dialog's read and the Save, so a late
//   answer to the read never undoes a Save.
// * Nothing is saved without a click on Save (decision 105): no call from an
//   effect or a cleanup, one call per click (a second click while one is
//   pending does nothing), and a response that arrives after the screen has
//   gone is dropped.
// * On success the dialog closes and a status line names the project; on a
//   refusal it shows the code's sentence and stays open.
// * The project's name is shown only as text, inside <bdi>.
import React, { useEffect, useMemo, useRef, useState } from 'react';
import { Button } from '../../../design-system';
import { checkName } from './projectName';
import { projectBridge, type ProjectBridge } from './projectBridge';
import { projectsShown } from './projectGate';
import { noteCurrent, openProjectStore, useOpenProject } from './projectSession';
import { NAME_WORDS, PROJECT_TEXT, projectSentence, SAVE_SENTENCE } from './projectWording';
import ProjectDialog from './ProjectDialog';
import { useGeneration } from './useGeneration';

/** A run the screens offer to save: one that Succeeded, or Failed with its
 *  code (decision 123). A failed run's state view ends with its failure, the
 *  one error a run that started can carry; Rust checks again on Save
 *  (project.run_failed_without_code). */
export function savableOnScreen(view: { state: string; diagnostics: Array<{ severity: string }> }): boolean {
  if (view.state === 'succeeded') return true;
  return view.state === 'failed' && view.diagnostics.some((d) => d.severity === 'error');
}

export interface SaveToProjectProps {
  runId: number;
  /** Test seam: the bridge. */
  bridge?: ProjectBridge;
  /** Test seam: whether this is the desktop app. */
  desktop?: boolean;
  testId?: string;
}

type Choice = 'new' | 'open' | null;

interface Saved {
  created: boolean;
  name: string;
}

export const SaveToProject: React.FC<SaveToProjectProps> = (props) =>
  projectsShown(props.desktop) ? <SaveToProjectShown {...props} /> : null;

const SaveToProjectShown: React.FC<SaveToProjectProps> = ({ runId, bridge: given, testId = 'project-save' }) => {
  const bridge = useMemo(() => given ?? projectBridge(), [given]);
  const open = useOpenProject();
  const [dialog, setDialog] = useState(false);
  const [choice, setChoice] = useState<Choice>(null);
  const [name, setName] = useState('');
  const [typed, setTyped] = useState(false);
  const [pending, setPending] = useState(false);
  const [refusal, setRefusal] = useState<string | null>(null);
  const [saved, setSaved] = useState<Saved | null>(null);
  const gen = useGeneration();
  const inFlight = useRef(false);
  const nameField = useRef<HTMLInputElement>(null);
  // The Save button, recorded from its click: focus returns to it.
  const opener = useRef<HTMLElement | null>(null);
  const returnFocus = useRef<(() => HTMLElement | null) | null>(() => opener.current);

  // Reads only, when the dialog appears: which project Rust has open, and the
  // list for its name (decision 148). A failed read means none is offered.
  useEffect(() => {
    if (!dialog) return;
    const g = gen.next();
    void Promise.allSettled([bridge.current(), bridge.list()]).then(([current, list]) => {
      if (!gen.is(g)) return;
      noteCurrent(current.status === 'fulfilled' ? current.value : null, list.status === 'fulfilled' ? list.value : null);
    });
  }, [dialog, bridge, gen]);

  // The open project went away while the dialog was up: a new project is the
  // one choice left.
  useEffect(() => {
    if (dialog && open === null && choice !== 'new') setChoice('new');
  }, [dialog, open, choice]);

  const checked = checkName(name);
  const nameSentence = choice === 'new' && typed && !checked.ok ? NAME_WORDS[checked.reason] : null;
  const ready = !pending && ((choice === 'new' && checked.ok) || (choice === 'open' && open !== null));

  const start = (e: React.MouseEvent<HTMLButtonElement>) => {
    opener.current = e.currentTarget;
    setChoice(open ? null : 'new');
    setName('');
    setTyped(false);
    setRefusal(null);
    setSaved(null);
    setDialog(true);
  };

  const cancel = () => {
    if (pending) return;
    setDialog(false);
  };

  const save = async () => {
    if (inFlight.current || !ready) return;
    const target =
      choice === 'open' && open
        ? { into: 'openProject' as const, projectId: open.projectId }
        : checked.ok
          ? { into: 'newProject' as const, name: checked.name }
          : null;
    if (!target) return;
    const shownName = target.into === 'openProject' ? (open as { name: string }).name : target.name;
    inFlight.current = true;
    const g = gen.next();
    setPending(true);
    setRefusal(null);
    try {
      const r = await bridge.saveRun(runId, target);
      if (!gen.is(g)) return;
      openProjectStore.set({ projectId: r.projectId, name: shownName });
      setSaved({ created: r.created, name: shownName });
      setDialog(false);
    } catch (e) {
      if (!gen.is(g)) return;
      setRefusal(projectSentence(e));
    } finally {
      inFlight.current = false;
      if (gen.is(g)) setPending(false);
    }
  };

  return (
    <div data-testid={testId}>
      <Button variant="secondary" onClick={start} data-testid={`${testId}-button`}>
        {PROJECT_TEXT.saveButton}
      </Button>
      <p role="status" aria-live="polite" data-testid={`${testId}-status`} style={{ margin: '4px 0 0' }}>
        {saved && (
          <>
            {saved.created ? PROJECT_TEXT.savedNew : PROJECT_TEXT.savedInto} <bdi data-testid={`${testId}-saved-name`}>{saved.name}</bdi>.
          </>
        )}
      </p>
      {dialog && (
        <ProjectDialog
          title={PROJECT_TEXT.saveTitle}
          onClose={cancel}
          initialFocus={open ? undefined : nameField}
          returnFocus={returnFocus}
          testId={`${testId}-dialog`}
        >
          <p data-testid={`${testId}-sentence`}>{SAVE_SENTENCE}</p>
          <fieldset style={{ border: 0, padding: 0, margin: '8px 0' }}>
            <legend>{PROJECT_TEXT.whereToSave}</legend>
            {open && (
              <label style={{ display: 'block' }}>
                <input
                  type="radio"
                  name={`${testId}-choice`}
                  checked={choice === 'open'}
                  onChange={() => setChoice('open')}
                  disabled={pending}
                  data-testid={`${testId}-choice-open`}
                />{' '}
                {PROJECT_TEXT.openProject} <bdi data-testid={`${testId}-open-name`}>{open.name}</bdi>
              </label>
            )}
            <label style={{ display: 'block' }}>
              <input
                type="radio"
                name={`${testId}-choice`}
                checked={choice === 'new'}
                onChange={() => setChoice('new')}
                disabled={pending}
                data-testid={`${testId}-choice-new`}
              />{' '}
              {PROJECT_TEXT.newProject}
            </label>
          </fieldset>
          {choice === 'new' && (
            <div style={{ display: 'grid', gap: 4 }}>
              <label htmlFor={`${testId}-name`}>{PROJECT_TEXT.nameLabel}</label>
              <input
                id={`${testId}-name`}
                ref={nameField}
                type="text"
                autoComplete="off"
                spellCheck={false}
                value={name}
                onChange={(e) => {
                  setName(e.target.value);
                  setTyped(true);
                }}
                disabled={pending}
                aria-invalid={nameSentence !== null}
                aria-describedby={nameSentence ? `${testId}-name-why` : undefined}
                data-testid={`${testId}-name`}
              />
              {nameSentence && (
                <p id={`${testId}-name-why`} role="alert" data-testid={`${testId}-name-why`} style={{ margin: 0 }}>
                  {nameSentence}
                </p>
              )}
            </div>
          )}
          {refusal && (
            <p role="alert" data-testid={`${testId}-refusal`}>
              {refusal}
            </p>
          )}
          <p role="status" aria-live="polite" data-testid={`${testId}-pending`} style={{ margin: '4px 0' }}>
            {pending ? PROJECT_TEXT.saving : ''}
          </p>
          <div style={{ display: 'flex', gap: 8, flexWrap: 'wrap' }}>
            <Button variant="secondary" onClick={cancel} disabled={pending} data-testid={`${testId}-cancel`}>
              {PROJECT_TEXT.cancel}
            </Button>
            <Button onClick={() => void save()} disabled={!ready} data-testid={`${testId}-confirm`}>
              {PROJECT_TEXT.saveConfirm}
            </Button>
          </div>
        </ProjectDialog>
      )}
    </div>
  );
};

export default SaveToProject;
