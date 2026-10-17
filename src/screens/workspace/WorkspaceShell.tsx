// Gaply — the slim shell around the Statistics workspace (Research OS,
// Phase 3; blueprint §4.1): the switcher, recent items (empty for now), a job
// status area and settings, around the Statistics tools.
//
// The shell reads Rust's state and never decides it. The job area polls
// workspace_state for the newest run's id and reads that run through Phase
// 2B's stats_desc_status (polling only, decision 24). Switching asks Rust to
// close the workspace; while a run is queued or running Rust refuses with the
// run's id, and the shell shows that run with Wait and Cancel. Nothing is
// abandoned silently.
//
// Behind REACT_APP_FEATURE_WORKSPACES, which hides unfinished UI and is not
// an authorization boundary.
import React, { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { AppShell, Button, Card, GaplyGlobe, HeaderBar, Modal, NavRail } from '../../design-system';
import { isFeatureEnabled } from '../../config/featureFlags';
import { describeBridge, type DescribeBridge } from '../statistics/describeBridge';
import { DESCRIBE_PATH } from '../statistics/describeRoute';
import { progressSentence, RUN_STATE_NAMES } from '../statistics/describeWording';
import type { RunStateView } from '../statistics/describeTypes';
import { workspaceBridge, type WorkspaceBridge } from './workspaceBridge';
import { isActive, workspaceRefusal, type WorkspaceStateView } from './workspaceTypes';
import { SHELL_TEXT, SWITCH_DIALOG, workspaceSentence } from './workspaceWording';
import { CHOOSER_PATH } from './workspaceRoute';

export interface WorkspaceShellProps {
  bridge?: WorkspaceBridge;
  /** Reads and cancels a run (Phase 2B's commands). */
  runs?: DescribeBridge;
  /** How often the state and the newest run are read. */
  pollMs?: number;
}

/** The Statistics tools the shell lists. Each keeps its own flag; the Stats
 *  Check and the Stats Verifier stay reachable without a workspace too
 *  (decision 27). */
function statisticsTools(): Array<{ label: string; href: string }> {
  return [
    ...(isFeatureEnabled('statsDescribe') ? [{ label: 'Descriptive Summary', href: DESCRIBE_PATH }] : []),
    ...(isFeatureEnabled('statsCheck') ? [{ label: 'Statistical Analysis Check', href: '/app/check/stats' }] : []),
    { label: 'Statistical Analysis Verifier ★', href: '/app/statsverifier' },
  ];
}

const JobArea: React.FC<{ run: RunStateView | null }> = ({ run }) => (
  <section aria-labelledby="ws-jobs-heading" data-testid="ws-jobs">
    <h2 id="ws-jobs-heading">{SHELL_TEXT.jobs}</h2>
    {run ? (
      <p role="status" aria-live="polite" data-testid="ws-job">
        Run {run.runId}: {RUN_STATE_NAMES[run.state]}. {progressSentence(run.columnsDone, run.columnsTotal)}
      </p>
    ) : (
      <p data-testid="ws-job-none">{SHELL_TEXT.jobsNone}</p>
    )}
  </section>
);

export const WorkspaceShell: React.FC<WorkspaceShellProps> = ({ bridge: given, runs: givenRuns, pollMs = 1000 }) => {
  const navigate = useNavigate();
  const bridge = useMemo(() => given ?? workspaceBridge(), [given]);
  const runs = useMemo(() => givenRuns ?? describeBridge(), [givenRuns]);
  const [view, setView] = useState<WorkspaceStateView | null>(null);
  const [run, setRun] = useState<RunStateView | null>(null);
  const [message, setMessage] = useState<string | null>(null);
  /** The run that blocked a switch: the dialog is open while this is set. */
  const [blocking, setBlocking] = useState<number | null>(null);
  const [blockingRun, setBlockingRun] = useState<RunStateView | null>(null);
  const [waiting, setWaiting] = useState(false);
  const alive = useRef(true);

  useEffect(() => {
    alive.current = true;
    return () => {
      alive.current = false;
    };
  }, []);

  const read = useCallback(async () => {
    try {
      const s = await bridge.state();
      if (!alive.current) return;
      setView(s);
      if (s.latestRun !== null) {
        const r = await runs.status(s.latestRun).catch(() => null);
        if (alive.current) setRun(r);
      } else {
        setRun(null);
      }
    } catch {
      if (alive.current) setMessage('The workspace state could not be read.');
    }
  }, [bridge, runs]);

  useEffect(() => {
    read();
    const t = setInterval(read, pollMs);
    return () => clearInterval(t);
  }, [read, pollMs]);

  const leave = useCallback(async () => {
    setMessage(null);
    try {
      await bridge.deactivate();
      setBlocking(null);
      navigate(CHOOSER_PATH);
    } catch (e) {
      const r = workspaceRefusal(e);
      if (r?.code === 'workspace.run_active' && typeof r.params.run === 'number') {
        setBlocking(r.params.run);
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
      const r = await runs.status(blocking).catch(() => null);
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
  }, [blocking, waiting, runs, pollMs, leave]);

  const cancelRun = useCallback(async () => {
    if (blocking === null) return;
    setWaiting(true);
    await runs.cancel(blocking).catch(() => null);
  }, [blocking, runs]);

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
                  <li key={t.href}>
                    <a href={t.href} onClick={(e) => { e.preventDefault(); navigate(t.href); }}>
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
            <a href="/app/settings" onClick={(e) => { e.preventDefault(); navigate('/app/settings'); }}>
              {SHELL_TEXT.settings}
            </a>
          </Card>
        </div>
      </AppShell>
      <Modal open={blocking !== null} title={SWITCH_DIALOG.title} onClose={stay}>
        <div data-testid="ws-switch-dialog">
          <p>{SWITCH_DIALOG.body}</p>
          <p role="status" aria-live="polite" data-testid="ws-switch-run">
            {blockingRun
              ? `Run ${blockingRun.runId}: ${RUN_STATE_NAMES[blockingRun.state]}. ${progressSentence(blockingRun.columnsDone, blockingRun.columnsTotal)}`
              : `Run ${blocking ?? ''}`}
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
