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
import { AppShell, Button, Card, GaplyGlobe, HeaderBar, NavRail } from '../../design-system';
import { isFeatureEnabled } from '../../config/featureFlags';
import { describeBridge, type DescribeBridge } from '../statistics/describeBridge';
import { STATISTICS_HOME_PATH, statisticsTools as selectorTools } from '../statistics/statisticsRoute';
import { FOCUS_PROJECTS } from '../statistics/StatisticsHome';
import { ttestBridge, type TTestBridge } from '../statistics/ttestBridge';
import { oneSampleBridge, pairedBridge, type OneSampleBridge, type PairedBridge } from '../statistics/onesampleBridge';
import { anovaBridge, type AnovaBridge } from '../statistics/anovaBridge';
import { corrBridge, type CorrBridge } from '../statistics/corrBridge';
import { alphaBridge, type AlphaBridge } from '../statistics/alphaBridge';
import { xtabBridge, type XtabBridge } from '../statistics/xtabBridge';
import { ranksBridge, type RanksBridge } from '../statistics/ranksBridge';
import { relatedRanksBridge, type RelatedRanksBridge } from '../statistics/relatedRanksBridge';
import { linearRegressionBridge, type LinearRegressionBridge } from '../statistics/linearRegressionBridge';
import { factorBridge, type FactorBridge } from '../statistics/factorBridge';
import { projectsShown } from '../statistics/projects/projectGate';
import { noteWorkspaceState } from '../statistics/projects/projectSession';
import { PROJECT_TEXT } from '../statistics/projects/projectWording';
import { workspaceBridge, type WorkspaceBridge } from './workspaceBridge';
import { isActive, type WorkspaceStateView } from './workspaceTypes';
import { SHELL_TEXT } from './workspaceWording';
import { CHOOSER_PATH } from './workspaceRoute';
import { jobSentence, useWorkspaceSwitch, type JobView } from './useWorkspaceSwitch';

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
  /** Reads and cancels a one-way ANOVA's run (Phase 8G-2's bridge). */
  anovaRuns?: AnovaBridge;
  /** Reads and cancels correlations' runs (Phase 8H-2's bridge). */
  corrRuns?: CorrBridge;
  /** Reads and cancels Cronbach's alpha's runs (Phase 8I-2's bridge). */
  alphaRuns?: AlphaBridge;
  /** Reads and cancels crosstabs' runs (Research OS, Phase 8X-2's bridge, decision 591). */
  xtabRuns?: XtabBridge;
  /** Reads and cancels the rank tests' runs (Research OS, Phase 8Y-2's bridge, decision 622). */
  ranksRuns?: RanksBridge;
  /** Research OS, Phase 8Z-2 (decision 656). */
  relatedRanksRuns?: RelatedRanksBridge;
  /** Research OS, Phase 9A-2 (decision 687). */
  linearRegressionRuns?: LinearRegressionBridge;
  /** Research OS, Phase 9B-3 (decision 732). */
  factorRuns?: FactorBridge;
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
  anovaRuns: givenAnova,
  corrRuns: givenCorr,
  alphaRuns: givenAlpha,
  xtabRuns: givenXtab,
  ranksRuns: givenRanks,
  relatedRanksRuns: givenRelatedRanks,
  linearRegressionRuns: givenLinearRegression,
  factorRuns: givenFactor,
  pollMs = 1000,
}) => {
  const navigate = useNavigate();
  const bridge = useMemo(() => given ?? workspaceBridge(), [given]);
  const runs = useMemo(() => givenRuns ?? describeBridge(), [givenRuns]);
  const ttest = useMemo(() => givenTTest ?? ttestBridge(), [givenTTest]);
  const oneSample = useMemo(() => givenOneSample ?? oneSampleBridge(), [givenOneSample]);
  const paired = useMemo(() => givenPaired ?? pairedBridge(), [givenPaired]);
  const anova = useMemo(() => givenAnova ?? anovaBridge(), [givenAnova]);
  const corr = useMemo(() => givenCorr ?? corrBridge(), [givenCorr]);
  const alpha = useMemo(() => givenAlpha ?? alphaBridge(), [givenAlpha]);
  const xtab = useMemo(() => givenXtab ?? xtabBridge(), [givenXtab]);
  const ranks = useMemo(() => givenRanks ?? ranksBridge(), [givenRanks]);
  const relatedRanks = useMemo(() => givenRelatedRanks ?? relatedRanksBridge(), [givenRelatedRanks]);
  const linearRegression = useMemo(() => givenLinearRegression ?? linearRegressionBridge(), [givenLinearRegression]);
  const factor = useMemo(() => givenFactor ?? factorBridge(), [givenFactor]);
  const [view, setView] = useState<WorkspaceStateView | null>(null);
  const [run, setRun] = useState<JobView | null>(null);
  const [message, setMessage] = useState<string | null>(null);
  const alive = useRef(true);

  useEffect(() => {
    alive.current = true;
    return () => {
      alive.current = false;
    };
  }, []);

  const runBridges = useMemo(
    () => ({ describe: runs, ttest, oneSample, paired, anova, corr, alpha, xtab, ranks, relatedRanks, linearRegression, factor }),
    [runs, ttest, oneSample, paired, anova, corr, alpha, xtab, ranks, relatedRanks, linearRegression, factor]
  );
  // The switch, and each kind's commands, are one hook's (decision 449).
  const { leave, statusOf, dialog: switchDialog } = useWorkspaceSwitch({ bridge, runs: runBridges, pollMs, setMessage });

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
      {switchDialog}
    </div>
  );
};

export default WorkspaceShell;
