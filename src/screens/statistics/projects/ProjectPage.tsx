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
// * Phase 5C-1b (decisions 191, 193 and 197 to 200): reruns.
//   - "Run again" on each saved result that finished opens a dialog with
//     decision 191's sentence; its Run again calls project_rerun on that
//     click only. A refusal shows its sentence, and Cancel then reads Close
//     and takes the focus, as the delete dialog does (decision 175).
//   - While project_current says a rerun of this project is going, whether
//     this page started it or found it when it appeared, a status line with
//     Cancel shows, and every Run again and Close the project are disabled,
//     with project.rerun_in_progress's sentence beside them. The page reads
//     project_current every pollMs, with its own liveness flag, never the
//     generation counter, so the poll neither waits on nor discards the
//     page's other reads and actions. Cancel calls the run kind's own cancel
//     (the describe or t-test bridge), never its status: a rerun is going
//     until its save at the end has returned, after its run has finished.
//   - When this page's poll sees the rerun end, it says whether the new run
//     was saved, with the code's sentence if not, and on a save reads the
//     runs once. An outcome project_current already held when the page
//     appeared shows nothing. A failed poll shows its sentence and stops.
//   - Each rerun, in the list and in its saved result view, says which saved
//     result it repeats and how its result compares (RerunWords).
import React, { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { Button, Card } from '../../../design-system';
import { saveTextFile } from '../../../utils/saveTextFile';
import { workspaceBridge, type WorkspaceBridge } from '../../workspace/workspaceBridge';
import { isActive } from '../../workspace/workspaceTypes';
import { describeBridge, type DescribeBridge } from '../describeBridge';
import { RUN_STATE_NAMES } from '../describeWording';
import { methodFor } from '../methods';
import { ttestBridge, type TTestBridge } from '../ttestBridge';
import { oneSampleBridge, type OneSampleBridge } from '../onesampleBridge';
import { projectBridge, type ProjectBridge } from './projectBridge';
import { EXPORTS, prepareExport, type ExportKind, type SaveFile } from './projectExport';
import { projectsShown } from './projectGate';
import { currentId, noteCurrent, noteWorkspaceState, openProjectStore } from './projectSession';
import type {
  ProjectCode,
  ProjectCurrentView,
  ProjectListEntryView,
  ProjectSizesView,
  RerunStartedView,
  StoredRunView,
} from './projectTypes';
import {
  capabilityName,
  EXPORT_WORDS,
  FALLBACK_SENTENCE,
  PROJECT_TEXT,
  PROJECT_WORDS,
  projectSentence,
  RERUN_SENTENCE,
  RERUN_WORDS,
  savedOn,
  sizeSentence,
} from './projectWording';
import ProjectDialog from './ProjectDialog';
import SavedResult, { RerunWords } from './SavedResult';
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
  /** A rerun's cancel, by its kind. */
  describe?: DescribeBridge;
  ttest?: TTestBridge;
  oneSample?: OneSampleBridge;
  /** How often project_current is read while a rerun is going. */
  pollMs?: number;
}

export const ProjectPage: React.FC<ProjectPageProps> = (props) =>
  projectsShown(props.desktop) ? <ProjectPageShown {...props} /> : null;

type Runs = StoredRunView[] | 'loading' | 'unread';

type RerunKind = RerunStartedView['kind'];

/** A rerun of the open project that is going: its in-memory run, the saved
 *  run it repeats, and its kind once known (from project_rerun's answer, or
 *  from the repeated run's capability once the runs have been read). */
interface Going {
  runId: number;
  rerunOf: number;
  kind: RerunKind | null;
}

/** The kind whose cancel stops a rerun of a run with this capability, from
 *  the methods table (Phase 8C); null for a capability no method has. */
function kindOf(capabilityId: string): RerunKind | null {
  return methodFor(capabilityId)?.kind ?? null;
}

/** The rerun project_current says is going in `projectId`, or null. */
function goingIn(answer: ProjectCurrentView | null, projectId: string): Going | null {
  const r = answer && typeof answer === 'object' ? answer.rerun : null;
  if (!r || currentId(answer) !== projectId || r.status !== 'going') return null;
  if (typeof r.runId !== 'number' || typeof r.rerunOf !== 'number') return null;
  return { runId: r.runId, rerunOf: r.rerunOf, kind: null };
}

/** A code's sentence, or the fallback for a code this screen does not know. */
const codeSentence = (code: ProjectCode | null) =>
  (code && PROJECT_WORDS[code]?.({ code })) || FALLBACK_SENTENCE;

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
  describe: givenDescribe,
  ttest: givenTtest,
  oneSample: givenOneSample,
  pollMs = 250,
}) => {
  const bridge = useMemo(() => given ?? projectBridge(), [given]);
  const workspace = useMemo(() => givenWorkspace ?? workspaceBridge(), [givenWorkspace]);
  const describe = useMemo(() => givenDescribe ?? describeBridge(), [givenDescribe]);
  const ttest = useMemo(() => givenTtest ?? ttestBridge(), [givenTtest]);
  const oneSample = useMemo(() => givenOneSample ?? oneSampleBridge(), [givenOneSample]);
  // Each kind's cancel, from the bridges given (Phase 8C).
  const cancels = useMemo<Record<RerunKind, (runId: number) => Promise<unknown>>>(
    () => ({
      describe: (runId) => describe.cancel(runId),
      ttest: (runId) => ttest.cancel(runId),
      one_sample: (runId) => oneSample.cancel(runId),
    }),
    [describe, ttest, oneSample]
  );
  const gen = useGeneration();
  const [view, setView] = useState<View>({ kind: 'loading' });
  const [pending, setPending] = useState<'open' | 'close' | null>(null);
  const [message, setMessage] = useState('');
  const [selected, setSelected] = useState<StoredRunView | null>(null);
  const [exporting, setExporting] = useState<ExportKind | null>(null);
  // Reruns (5C-1b): the one going, the dialog's run, what this page's poll
  // saw at the end, whether a poll failed, and a tick that re-arms the poll.
  const [going, setGoing] = useState<Going | null>(null);
  const [rerunning, setRerunning] = useState<StoredRunView | null>(null);
  const [outcome, setOutcome] = useState<string[] | null>(null);
  const [pollFailed, setPollFailed] = useState(false);
  const [tick, setTick] = useState(0);
  const [cancelling, setCancelling] = useState(false);
  const alive = useRef(true);
  const cancelInFlight = useRef(false);
  const busy = useRef(false);
  const heading = useRef<HTMLHeadingElement>(null);
  // The button that opened a dialog or a saved result, recorded from its click.
  const opener = useRef<HTMLElement | null>(null);
  // Decision 232: where the focus goes next across a rerun started here, set
  // when the start, the end or a failed poll is seen and done after the render
  // that shows it.
  const focusNext = useRef<FocusNext | null>(null);
  const pageBox = useRef<HTMLElement>(null);

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
    // A rerun going in it shows again; an outcome it already holds does not.
    setGoing(goingIn(answer, projectId));
    setOutcome(null);
    setPollFailed(false);
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

  useEffect(() => {
    alive.current = true;
    return () => {
      alive.current = false;
    };
  }, []);

  // While a rerun is going, project_current is read every `pollMs`, under
  // this effect's own liveness flag (as DescribeFlow's poll), never the
  // page's counter. Its end is seen here only.
  useEffect(() => {
    if (!going || pollFailed) return;
    let live = true;
    // Decision 232: the focus follows only from Cancel or from nothing.
    const followed = (next: FocusNext) => {
      if (focusOnCancelOrNothing(pageBox.current)) focusNext.current = next;
    };
    const failed = () => {
      followed({ to: 'retry' });
      setPollFailed(true);
    };
    const t = setTimeout(async () => {
      let answer: ProjectCurrentView;
      try {
        answer = await bridge.current();
      } catch {
        if (live) failed();
        return;
      }
      if (!live) return;
      const r = answer && typeof answer === 'object' ? answer.rerun : null;
      if (!r || currentId(answer) !== projectId || r.runId !== going.runId) {
        // Not an answer about this rerun: as unread as a failed read.
        failed();
        return;
      }
      if (r.status === 'going') {
        setTick((n) => n + 1);
        return;
      }
      if (r.status === 'saved') {
        let runs: Runs;
        try {
          runs = newestFirst(await bridge.runs(projectId));
        } catch {
          runs = 'unread';
        }
        if (!live) return;
        setView((v) => (v.kind === 'open' ? { ...v, runs } : v));
        setOutcome([RERUN_WORDS.saved]);
      } else if (r.status === 'not_saved') {
        setOutcome([RERUN_WORDS.notSaved, codeSentence(r.code)]);
      } else {
        failed();
        return;
      }
      followed({ to: 'end', rerunOf: going.rerunOf });
      setGoing(null);
      setCancelling(false);
    }, pollMs);
    return () => {
      live = false;
      clearTimeout(t);
    };
  }, [going, tick, pollFailed, bridge, projectId, pollMs]);

  // Decision 232, after the render that shows the start, the end or Try
  // again. A saved result open keeps the focus where it is.
  useEffect(() => {
    const next = focusNext.current;
    if (next === null) return;
    const find = (id: string) => pageBox.current?.querySelector<HTMLElement>(`[data-testid="${id}"]`) ?? null;
    if (next.to === 'cancel') {
      if (going === null || pollFailed) return;
      focusNext.current = null;
      find('project-rerun-cancel')?.focus();
      return;
    }
    if (next.to === 'retry') {
      if (!pollFailed) return;
      focusNext.current = null;
      if (selected === null) find('project-rerun-retry')?.focus();
      return;
    }
    if (going !== null) return;
    focusNext.current = null;
    if (selected !== null) return;
    const again = find(`project-run-rerun-${next.rerunOf}`) as HTMLButtonElement | null;
    if (again && !again.disabled) again.focus();
    else find('project-runs-heading')?.focus();
  }, [going, pollFailed, view, selected]);

  // The kind whose cancel stops the going rerun: as project_rerun said, or
  // from the run it repeats, once the runs are read. Until then, none.
  const runs = view.kind === 'open' && Array.isArray(view.runs) ? view.runs : null;
  const goingKind: RerunKind | null =
    going === null
      ? null
      : (going.kind ?? (runs ? kindOf(runs.find((r) => r.storedRunId === going.rerunOf)?.capabilityId ?? '') : null));

  // Cancel, on its click only: the kind's own cancel. Its end is seen by the
  // poll, which then says the new run was not saved.
  const cancelRerun = async () => {
    if (going === null || goingKind === null || cancelInFlight.current) return;
    cancelInFlight.current = true;
    setCancelling(true);
    setMessage('');
    try {
      // A word other than 'describe' cancels through the t-test, as before
      // Phase 8C; an own key only, never one from Object.prototype.
      const own = Object.prototype.hasOwnProperty.call(cancels, goingKind);
      await (own ? cancels[goingKind] : cancels.ttest)(going.runId);
    } catch (e) {
      if (alive.current) {
        setMessage(projectSentence(e));
        setCancelling(false);
      }
    } finally {
      cancelInFlight.current = false;
    }
  };

  const rerunStarted = (run: StoredRunView, started: RerunStartedView) => {
    setRerunning(null);
    setMessage('');
    setOutcome(null);
    setPollFailed(false);
    setCancelling(false);
    // Decision 232: a rerun started here puts the focus on its Cancel.
    focusNext.current = { to: 'cancel' };
    setGoing({ runId: started.runId, rerunOf: run.storedRunId, kind: started.kind });
  };

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
          rerun={{
            going: going !== null,
            canCancel: goingKind !== null && !cancelling,
            cancelling,
            outcome,
            pollFailed,
            onRunAgain: (run, from) => {
              opener.current = from;
              setMessage('');
              setRerunning(run);
            },
            onCancel: () => void cancelRerun(),
            onRetry: () => void load(),
          }}
        />
      );
      break;
  }

  return (
    <Card>
      <section ref={pageBox} aria-labelledby="project-page-heading" data-testid="project-page" style={{ display: 'grid', gap: 8 }}>
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
      {rerunning && view.kind === 'open' && (
        <RerunDialog
          run={rerunning}
          bridge={bridge}
          gen={gen}
          openerAfter={() => opener.current}
          onCancel={() => setRerunning(null)}
          onStarted={(started) => rerunStarted(rerunning, started)}
        />
      )}
    </Card>
  );
};

/** Where the focus goes next across a rerun (decision 232). */
type FocusNext = { to: 'cancel' } | { to: 'retry' } | { to: 'end'; rerunOf: number };

/** Whether the focus is on the rerun's Cancel, or on nothing. */
function focusOnCancelOrNothing(box: HTMLElement | null): boolean {
  const at = document.activeElement;
  if (at === null || at === document.body) return true;
  return box !== null && at === box.querySelector('[data-testid="project-rerun-cancel"]');
}

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
  rerun: RerunControls;
}

/** What the open project's page shows and does for reruns. */
interface RerunControls {
  /** A rerun of this project is going (decision 199). */
  going: boolean;
  /** Its kind is known and no cancel is pending. */
  canCancel: boolean;
  cancelling: boolean;
  /** What this page's poll saw at the rerun's end, or null. */
  outcome: string[] | null;
  pollFailed: boolean;
  onRunAgain: (run: StoredRunView, from: HTMLElement) => void;
  onCancel: () => void;
  onRetry: () => void;
}

const IN_PROGRESS = PROJECT_WORDS['project.rerun_in_progress']({ code: 'project.rerun_in_progress' });

/** The saved run a rerun repeats, if the list holds it. */
const repeatedIn = (runs: StoredRunView[], run: StoredRunView) =>
  run.rerunOf === null ? null : (runs.find((r) => r.storedRunId === run.rerunOf) ?? null);

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
  rerun,
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
    <div style={{ display: 'flex', gap: 8, flexWrap: 'wrap', alignItems: 'center' }}>
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
      <Button variant="secondary" onClick={onClose} disabled={pending !== null || rerun.going} data-testid="project-page-close">
        {pending === 'close' ? PROJECT_TEXT.closing : PROJECT_TEXT.close}
      </Button>
      {rerun.going && <span data-testid="project-close-going">{IN_PROGRESS}</span>}
    </div>
    <div data-testid="project-rerun-status" style={{ display: 'grid', gap: 4 }}>
      {rerun.going && !rerun.pollFailed && (
        <div style={{ display: 'flex', gap: 8, flexWrap: 'wrap', alignItems: 'center' }}>
          <p role="status" data-testid="project-rerun-running" style={{ margin: 0 }}>
            {PROJECT_TEXT.runningAgain}
          </p>
          <Button variant="secondary" onClick={rerun.onCancel} disabled={!rerun.canCancel} data-testid="project-rerun-cancel">
            {rerun.cancelling ? PROJECT_TEXT.cancelling : PROJECT_TEXT.cancel}
          </Button>
        </div>
      )}
      {rerun.pollFailed && (
        <div style={{ display: 'flex', gap: 8, flexWrap: 'wrap', alignItems: 'center' }}>
          <p role="alert" data-testid="project-rerun-unread" style={{ margin: 0 }}>
            {PROJECT_TEXT.pageUnread}
          </p>
          <Button variant="secondary" onClick={rerun.onRetry} data-testid="project-rerun-retry">
            {PROJECT_TEXT.tryAgain}
          </Button>
        </div>
      )}
      {rerun.outcome && (
        <p role="status" data-testid="project-rerun-outcome" style={{ margin: 0 }}>
          {rerun.outcome.join(' ')}
        </p>
      )}
    </div>
    {selected ? (
      <>
        <SavedResult
          projectId={projectId}
          run={selected}
          repeated={Array.isArray(view.runs) ? repeatedIn(view.runs, selected) : null}
          bridge={bridge}
          gen={gen}
        />
        <div>
          <Button variant="secondary" onClick={onDeselect} data-testid="saved-result-back">
            {PROJECT_TEXT.backToResults}
          </Button>
        </div>
      </>
    ) : (
      <section aria-label={PROJECT_TEXT.savedResults} data-testid="project-runs">
        {/* tabIndex -1: the focus comes here when a rerun's row is gone (decision 232). */}
        <h4 tabIndex={-1} style={{ margin: '8px 0 4px' }} data-testid="project-runs-heading">
          {PROJECT_TEXT.savedResults}
        </h4>
        {view.runs === 'loading' && <p data-testid="project-runs-loading">{PROJECT_TEXT.runsLoading}</p>}
        {view.runs === 'unread' && (
          <p role="alert" data-testid="project-runs-unread">
            {PROJECT_TEXT.runsUnread}
          </p>
        )}
        {Array.isArray(view.runs) && view.runs.length === 0 && <p data-testid="project-runs-empty">{PROJECT_TEXT.runsEmpty}</p>}
        {Array.isArray(view.runs) && view.runs.length > 0 && rerun.going && (
          <p data-testid="project-runs-going" style={{ margin: 0 }}>
            {IN_PROGRESS}
          </p>
        )}
        {Array.isArray(view.runs) && view.runs.length > 0 && (
          <ol data-testid="project-runs-list" style={{ margin: 0, paddingLeft: 20, display: 'grid', gap: 6 }}>
            {view.runs.map((r, _i, all) => (
              <li key={r.storedRunId} data-testid={`project-run-${r.storedRunId}`}>
                <span data-testid={`project-run-facts-${r.storedRunId}`}>
                  {capabilityName(r.capabilityId)}. {RUN_STATE_NAMES[r.state] ?? PROJECT_TEXT.unknownState}. {PROJECT_TEXT.savedOn}{' '}
                  {savedOn(r.savedAt)}.{r.endCode ? ` ${PROJECT_WORDS[r.endCode]?.({ code: r.endCode }) ?? ''}` : ''}
                </span>{' '}
                <Button
                  variant="secondary"
                  onClick={(e) => onSelect(r, e.currentTarget)}
                  data-testid={`project-run-open-${r.storedRunId}`}
                >
                  {PROJECT_TEXT.openResult}
                </Button>
                {r.state === 'succeeded' && (
                  <>
                    {' '}
                    <Button
                      variant="secondary"
                      onClick={(e) => rerun.onRunAgain(r, e.currentTarget)}
                      disabled={pending !== null || rerun.going}
                      data-testid={`project-run-rerun-${r.storedRunId}`}
                    >
                      {PROJECT_TEXT.runAgain}
                    </Button>
                  </>
                )}
                <RerunWords run={r} repeated={repeatedIn(all, r)} testId={`project-run-repeats-${r.storedRunId}`} />
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

interface RerunDialogProps {
  run: StoredRunView;
  bridge: ProjectBridge;
  gen: Generation;
  openerAfter: () => HTMLElement | null;
  onCancel: () => void;
  onStarted: (started: RerunStartedView) => void;
}

/** Decision 191's dialog, in the export dialog's pattern. After a refusal it
 *  stays open so the refusal can be read, and its Cancel reads Close and
 *  takes the focus, as the delete dialog does (decision 175). */
const RerunDialog: React.FC<RerunDialogProps> = ({ run, bridge, gen, openerAfter, onCancel, onStarted }) => {
  const [pending, setPending] = useState(false);
  const [refusal, setRefusal] = useState<string | null>(null);
  const inFlight = useRef(false);
  const cancelButton = useRef<HTMLButtonElement>(null);
  const returnFocus = useRef<(() => HTMLElement | null) | null>(openerAfter);

  useEffect(() => {
    if (refusal !== null && !pending) cancelButton.current?.focus();
  }, [refusal, pending]);

  const cancel = () => {
    if (!pending) onCancel();
  };

  // On the Run again click only.
  const start = async () => {
    if (inFlight.current) return;
    inFlight.current = true;
    const g = gen.next();
    setPending(true);
    setRefusal(null);
    try {
      const started = await bridge.rerun(run.storedRunId);
      if (gen.is(g)) onStarted(started);
    } catch (err) {
      if (gen.is(g)) setRefusal(projectSentence(err));
    } finally {
      inFlight.current = false;
      if (gen.is(g)) setPending(false);
    }
  };

  return (
    <ProjectDialog title={PROJECT_TEXT.runAgainTitle} onClose={cancel} initialFocus={cancelButton} returnFocus={returnFocus} testId="project-rerun-dialog">
      <p data-testid="project-rerun-sentence">{RERUN_SENTENCE}</p>
      {refusal && (
        <p role="alert" data-testid="project-rerun-refusal">
          {refusal}
        </p>
      )}
      <p role="status" aria-live="polite" data-testid="project-rerun-pending" style={{ margin: '4px 0' }}>
        {pending ? PROJECT_TEXT.starting : ''}
      </p>
      <div style={{ display: 'flex', gap: 8, flexWrap: 'wrap' }}>
        <button
          ref={cancelButton}
          type="button"
          className="gds-btn gds-btn--secondary"
          onClick={cancel}
          disabled={pending}
          data-testid="project-rerun-dialog-cancel"
        >
          {refusal !== null ? PROJECT_TEXT.closeDialog : PROJECT_TEXT.cancel}
        </button>
        <Button onClick={() => void start()} disabled={pending} data-testid="project-rerun-confirm">
          {PROJECT_TEXT.runAgain}
        </Button>
      </div>
    </ProjectDialog>
  );
};

export default ProjectPage;
