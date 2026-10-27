// Gaply — the slim shell around the Statistics workspace (Research OS,
// Phase 3; blueprint §4.1): the switcher, recent items (empty for now), a job
// status area and settings, around the Statistics tools.
//
// The shell reads Rust's state and never decides it. The job area polls
// workspace_state for the newest run's id and kind, and reads that run
// through its kind's status command: Phase 2B's stats_desc_status for a
// descriptive summary, stats_ttest_status for a t-test (polling only,
// decision 24; Phase 4B-2, decision 80). Switching asks Rust to close the
// workspace; while a run is queued or running Rust refuses with the run's
// id, and the shell shows that run with Wait and Cancel, through its kind's
// commands. Nothing is abandoned silently.
//
// Behind REACT_APP_FEATURE_WORKSPACES, which hides unfinished UI and is not
// an authorization boundary.
import React, { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { AppShell, Button, Card, GaplyGlobe, HeaderBar, Modal, NavRail } from '../../design-system';
import { isFeatureEnabled } from '../../config/featureFlags';
import { describeBridge, type DescribeBridge } from '../statistics/describeBridge';
import { progressSentence, RUN_STATE_NAMES } from '../statistics/describeWording';
import type { RunState } from '../statistics/describeTypes';
import { STATISTICS_HOME_PATH, statisticsTools as selectorTools } from '../statistics/statisticsRoute';
import { FOCUS_PROJECTS } from '../statistics/StatisticsHome';
import { ttestBridge, type TTestBridge } from '../statistics/ttestBridge';
import { oneSampleBridge, pairedBridge, type OneSampleBridge, type PairedBridge } from '../statistics/onesampleBridge';
import { byKind } from '../statistics/methods';
import { projectsShown } from '../statistics/projects/projectGate';
import { noteWorkspaceState, openProjectStore } from '../statistics/projects/projectSession';
import { PROJECT_TEXT } from '../statistics/projects/projectWording';
import { workspaceBridge, type WorkspaceBridge } from './workspaceBridge';
import { isActive, workspaceRefusal, type RunEntry, type WorkspaceStateView } from './workspaceTypes';
import { SHELL_TEXT, SWITCH_DIALOG, workspaceSentence } from './workspaceWording';
import { CHOOSER_PATH } from './workspaceRoute';

export interface WorkspaceShellProps {
  bridge?: WorkspaceBridge;
  /** Reads and cancels a descriptive summary's run (Phase 2B's commands). */
  runs?: DescribeBridge;
  /** Reads and cancels a t-test's run (Phase 4B-2's commands). */
  ttestRuns?: TTestBridge;
  /** Reads and cancels a one-sample t-test's run (Phase 8D-2b's bridge). */
  oneSampleRuns?: OneSampleBridge;
  /** Reads and cancels a paired t-test's run (Phase 8D-2c, the same bridge). */
  pairedRuns?: PairedBridge;
  /** How often the state and the newest run are read. */
  pollMs?: number;
}

/** The Statistics tools the shell lists. Each keeps its own flag; the Stats
 *  Check and the Stats Verifier stay reachable without a workspace too
 *  (decision 27). */
function statisticsTools(): Array<{ key: string; label: string; href: string; state?: object; testId?: string }> {
  return [
    // The method selector replaced the descriptive summary's link here
    // (Phase 4B-2, decision 77); both flows are reached from it.
    ...selectorTools().map((t) => ({ key: t.href, ...t })),
    // The projects (decision 172), only where their pages can open: the
    // project screens show and the methods page they sit on exists. It leads
    // to that same page, which then brings its Projects panel into view.
    ...(projectsShown() && isFeatureEnabled('statsDescribe')
      ? [{ key: 'projects', label: PROJECT_TEXT.panelHeading, href: STATISTICS_HOME_PATH, state: FOCUS_PROJECTS, testId: 'ws-tools-projects' }]
      : []),
    ...(isFeatureEnabled('statsCheck') ? [{ key: '/app/check/stats', label: 'Statistical Analysis Check', href: '/app/check/stats' }] : []),
    { key: '/app/statsverifier', label: 'Statistical Analysis Verifier ★', href: '/app/statsverifier' },
  ];
}

/** A run of either kind, as the shell shows it. */
interface JobView {
  runId: number;
  state: RunState;
  terminal: boolean;
  /** The descriptive summary's column progress; a t-test has none. */
  progress: string | null;
  kind: RunEntry['kind'];
}

/** Each kind's job name, from the methods table (Phase 8C): none for the
 *  summary, whose sentence has no parenthesis. */
const KIND_NAMES: Record<RunEntry['kind'], string | null> = byKind((m) => m.jobName);

/** What the shell does with a run of one kind, through that kind's own
 *  commands (decision 80). */
interface KindCommands {
  status: (runId: number) => Promise<JobView | null>;
  cancel: (runId: number) => Promise<unknown>;
}

/** A kind's commands; a kind this screen does not know reads and cancels as
 *  the summary, as it did before Phase 8C. An own key only, so a word such
 *  as 'toString' is not taken from Object.prototype. */
function commandsFor(kinds: Record<RunEntry['kind'], KindCommands>, kind: string): KindCommands {
  return Object.prototype.hasOwnProperty.call(kinds, kind) ? kinds[kind as RunEntry['kind']] : kinds.describe;
}

export function jobSentence(run: JobView): string {
  const kind = KIND_NAMES[run.kind];
  return `Run ${run.runId}${kind ? ` (${kind})` : ''}: ${RUN_STATE_NAMES[run.state]}.${run.progress ? ` ${run.progress}` : ''}`;
}

const JobArea: React.FC<{ run: JobView | null }> = ({ run }) => (
  <section aria-labelledby="ws-jobs-heading" data-testid="ws-jobs">
    <h2 id="ws-jobs-heading">{SHELL_TEXT.jobs}</h2>
    {run ? (
      <p role="status" aria-live="polite" data-testid="ws-job">
        {jobSentence(run)}
      </p>
    ) : (
      <p data-testid="ws-job-none">{SHELL_TEXT.jobsNone}</p>
    )}
  </section>
);

export const WorkspaceShell: React.FC<WorkspaceShellProps> = ({
  bridge: given,
  runs: givenRuns,
  ttestRuns: givenTTest,
  oneSampleRuns: givenOneSample,
  pairedRuns: givenPaired,
  pollMs = 1000,
}) => {
  const navigate = useNavigate();
  const bridge = useMemo(() => given ?? workspaceBridge(), [given]);
  const runs = useMemo(() => givenRuns ?? describeBridge(), [givenRuns]);
  const ttest = useMemo(() => givenTTest ?? ttestBridge(), [givenTTest]);
  const oneSample = useMemo(() => givenOneSample ?? oneSampleBridge(), [givenOneSample]);
  const paired = useMemo(() => givenPaired ?? pairedBridge(), [givenPaired]);
  const [view, setView] = useState<WorkspaceStateView | null>(null);
  const [run, setRun] = useState<JobView | null>(null);
  const [message, setMessage] = useState<string | null>(null);
  /** The run that blocked a switch: the dialog is open while this is set. */
  const [blocking, setBlocking] = useState<RunEntry | null>(null);
  const [blockingRun, setBlockingRun] = useState<JobView | null>(null);
  const [waiting, setWaiting] = useState(false);
  const alive = useRef(true);

  useEffect(() => {
    alive.current = true;
    return () => {
      alive.current = false;
    };
  }, []);

  /** Each kind's status and cancel, from the bridges given (Phase 8C). */
  const kinds = useMemo<Record<RunEntry['kind'], KindCommands>>(
    () => ({
      describe: {
        status: async (runId) => {
          const r = await runs.status(runId).catch(() => null);
          return (
            r && {
              runId: r.runId,
              state: r.state,
              terminal: r.terminal,
              progress: progressSentence(r.columnsDone, r.columnsTotal),
              kind: 'describe',
            }
          );
        },
        cancel: (runId) => runs.cancel(runId).catch(() => null),
      },
      ttest: {
        status: async (runId) => {
          const r = await ttest.status(runId).catch(() => null);
          return r && { runId: r.runId, state: r.state, terminal: r.terminal, progress: null, kind: 'ttest' };
        },
        cancel: (runId) => ttest.cancel(runId).catch(() => null),
      },
      one_sample: {
        status: async (runId) => {
          const r = await oneSample.status(runId).catch(() => null);
          return r && { runId: r.runId, state: r.state, terminal: r.terminal, progress: null, kind: 'one_sample' };
        },
        cancel: (runId) => oneSample.cancel(runId).catch(() => null),
      },
      paired: {
        status: async (runId) => {
          const r = await paired.status(runId).catch(() => null);
          return r && { runId: r.runId, state: r.state, terminal: r.terminal, progress: null, kind: 'paired' };
        },
        cancel: (runId) => paired.cancel(runId).catch(() => null),
      },
    }),
    [runs, ttest, oneSample, paired]
  );

  /** A run's state, through its kind's status command (decision 80). */
  const statusOf = useCallback((e: RunEntry): Promise<JobView | null> => commandsFor(kinds, e.kind).status(e.runId), [kinds]);

  const read = useCallback(async () => {
    try {
      const s = await bridge.state();
      // Statistics not active: no project is open (decision 135).
      noteWorkspaceState(s);
      if (!alive.current) return;
      setView(s);
      if (s.latestRun !== null) {
        const r = await statusOf(s.latestRun);
        if (alive.current) setRun(r);
      } else {
        setRun(null);
      }
    } catch {
      if (alive.current) setMessage('The workspace state could not be read.');
    }
  }, [bridge, statusOf]);

  useEffect(() => {
    read();
    const t = setInterval(read, pollMs);
    return () => clearInterval(t);
  }, [read, pollMs]);

  const leave = useCallback(async () => {
    setMessage(null);
    try {
      await bridge.deactivate();
      // Leaving Statistics closed the open project in Rust (decision 135).
      openProjectStore.clear();
      setBlocking(null);
      navigate(CHOOSER_PATH);
    } catch (e) {
      const r = workspaceRefusal(e);
      if (r?.code === 'workspace.run_active' && typeof r.params.run === 'number') {
        // The refusal names the run; the state names its kind.
        const id = r.params.run;
        const s = await bridge.state().catch(() => null);
        const kind = s?.activeRun?.runId === id ? s.activeRun.kind : 'describe';
        setBlocking({ runId: id, kind });
      } else {
        setMessage(r ? workspaceSentence(r) : 'The workspace could not be closed.');
      }
    }
  }, [bridge, navigate]);

  // While the dialog is open, read the blocking run; once it has ended and
  // the user chose to wait (or cancelled it), switch.
  useEffect(() => {
    if (blocking === null) return;
    let stop = false;
    const tick = async () => {
      const r = await statusOf(blocking);
      if (stop || !alive.current) return;
      setBlockingRun(r);
      if (waiting && (r === null || r.terminal)) {
        stop = true;
        setWaiting(false);
        await leave();
      }
    };
    tick();
    const t = setInterval(tick, pollMs);
    return () => {
      stop = true;
      clearInterval(t);
    };
  }, [blocking, waiting, statusOf, pollMs, leave]);

  const cancelRun = useCallback(async () => {
    if (blocking === null) return;
    setWaiting(true);
    // Through the run's own kind's cancel (decision 80).
    await commandsFor(kinds, blocking.kind).cancel(blocking.runId);
  }, [blocking, kinds]);

  const stay = useCallback(() => {
    setBlocking(null);
    setBlockingRun(null);
    setWaiting(false);
  }, []);

  const statisticsOpen = view !== null && isActive(view.state, 'statistics');

  return (
    <div className="gds-root" style={{ height: '100vh', overflow: 'auto' }} data-testid="workspace-shell">
      <AppShell
        rail={
          <NavRail
            items={[
              { id: 'home', label: 'Home', icon: '◫', onSelect: () => navigate('/app') },
              { id: 'workspaces', label: SHELL_TEXT.choose, icon: '▦', onSelect: () => navigate(CHOOSER_PATH) },
              { id: 'settings', label: SHELL_TEXT.settings, icon: '⚙', onSelect: () => navigate('/app/settings') },
            ]}
            activeId="workspaces"
            brand={<GaplyGlobe scale="mark" />}
          />
        }
        header={
          <HeaderBar title="Statistics">
            <Button variant="secondary" data-testid="ws-switch" onClick={leave} disabled={!statisticsOpen}>
              {SHELL_TEXT.switch}
            </Button>
          </HeaderBar>
        }
      >
        <div style={{ display: 'grid', gap: 16, padding: 16 }}>
          <p role="status" aria-live="polite" data-testid="ws-shell-message">
            {message ?? ''}
          </p>
          {view !== null && !statisticsOpen ? (
            <Card title={SHELL_TEXT.notActive}>
              <Button data-testid="ws-shell-choose" onClick={() => navigate(CHOOSER_PATH)}>
                {SHELL_TEXT.choose}
              </Button>
            </Card>
          ) : (
            <Card title={SHELL_TEXT.tools}>
              <ul data-testid="ws-tools">
                {statisticsTools().map((t) => (
                  <li key={t.key}>
                    <a
                      href={t.href}
                      className="gds-link"
                      data-testid={t.testId}
                      onClick={(e) => {
                        e.preventDefault();
                        navigate(t.href, t.state ? { state: t.state } : undefined);
                      }}
                    >
                      {t.label}
                    </a>
                  </li>
                ))}
              </ul>
            </Card>
          )}
          <Card title={SHELL_TEXT.recent}>
            <p data-testid="ws-recent-empty">{SHELL_TEXT.recentEmpty}</p>
          </Card>
          <Card>
            <JobArea run={run} />
          </Card>
          <Card title={SHELL_TEXT.settings}>
            <a href="/app/settings" className="gds-link" onClick={(e) => { e.preventDefault(); navigate('/app/settings'); }}>
              {SHELL_TEXT.settings}
            </a>
          </Card>
        </div>
      </AppShell>
      <Modal open={blocking !== null} title={SWITCH_DIALOG.title} onClose={stay}>
        <div data-testid="ws-switch-dialog">
          <p>{SWITCH_DIALOG.body}</p>
          <p role="status" aria-live="polite" data-testid="ws-switch-run">
            {blockingRun ? jobSentence(blockingRun) : `Run ${blocking?.runId ?? ''}`}
          </p>
          {waiting && <p data-testid="ws-switch-waiting">{SWITCH_DIALOG.waiting}</p>}
          <div style={{ display: 'flex', gap: 8, flexWrap: 'wrap' }}>
            <Button data-testid="ws-switch-wait" autoFocus onClick={() => setWaiting(true)} disabled={waiting}>
              {SWITCH_DIALOG.wait}
            </Button>
            <Button variant="secondary" data-testid="ws-switch-cancel" onClick={cancelRun} disabled={waiting}>
              {SWITCH_DIALOG.cancel}
            </Button>
            <Button variant="ghost" data-testid="ws-switch-stay" onClick={stay}>
              {SWITCH_DIALOG.stay}
            </Button>
          </div>
        </div>
      </Modal>
    </div>
  );
};

export default WorkspaceShell;
