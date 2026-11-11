// Gaply — leaving the Statistics workspace (Research OS, Phase 8P, decision
// 449), moved here from WorkspaceShell.tsx, not copied, so the workspace page
// and the Statistics window reach one switch. Switching asks Rust to close
// the workspace; while a run is queued or running Rust refuses with the run's
// id, and this shows that run with Wait and Cancel, through its kind's
// commands (decision 80). Nothing is abandoned silently, and the hook decides
// nothing: Rust's answer does.
import React, { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { Button, Modal } from '../../design-system';
import { describeBridge, type DescribeBridge } from '../statistics/describeBridge';
import { progressSentence, RUN_STATE_NAMES } from '../statistics/describeWording';
import type { RunState } from '../statistics/describeTypes';
import { ttestBridge, type TTestBridge } from '../statistics/ttestBridge';
import { oneSampleBridge, pairedBridge, type OneSampleBridge, type PairedBridge } from '../statistics/onesampleBridge';
import { anovaBridge, type AnovaBridge } from '../statistics/anovaBridge';
import { stepsSentence } from '../statistics/anovaWording';
import { corrBridge, type CorrBridge } from '../statistics/corrBridge';
import { alphaBridge, type AlphaBridge } from '../statistics/alphaBridge';
import { xtabBridge, type XtabBridge } from '../statistics/xtabBridge';
import { ranksBridge, type RanksBridge } from '../statistics/ranksBridge';
import { relatedRanksBridge, type RelatedRanksBridge } from '../statistics/relatedRanksBridge';
import { linearRegressionBridge, type LinearRegressionBridge } from '../statistics/linearRegressionBridge';
import { byKind } from '../statistics/methods';
import { openProjectStore } from '../statistics/projects/projectSession';
import { AskFirst } from '../statistics/window/MethodDialog';
import { holdsUnsavedWork, statisticsSession } from '../statistics/window/statisticsSession';
import { ASK_LEAVE } from '../statistics/window/windowWording';
import type { WorkspaceBridge } from './workspaceBridge';
import { workspaceRefusal, type RunEntry } from './workspaceTypes';
import { SWITCH_DIALOG, workspaceSentence } from './workspaceWording';
import { CHOOSER_PATH } from './workspaceRoute';

/** A run of either kind, as the shell shows it. */
export interface JobView {
  runId: number;
  state: RunState;
  terminal: boolean;
  /** The descriptive summary's column progress, or the one-way ANOVA's or
   *  the correlations' or Cronbach's alpha's steps (decisions 328, 355 and
   *  380); a t-test has none, nor a crosstab, whose one check point makes a
   *  steps sentence say nothing (decision 588). */
  progress: string | null;
  kind: RunEntry['kind'];
}

/** The eleven methods' bridges: each kind's run is read and cancelled through
 *  its own (decision 80); crosstabs' since Research OS Phase 8X-2 (decision 591),
 *  the rank tests' since 8Y-2 (decision 622), the related rank tests' since 8Z-2
 *  (decision 656), the linear regression's since 9A-2 (decision 687). */
export interface RunBridges {
  describe: DescribeBridge;
  ttest: TTestBridge;
  oneSample: OneSampleBridge;
  paired: PairedBridge;
  anova: AnovaBridge;
  corr: CorrBridge;
  alpha: AlphaBridge;
  xtab: XtabBridge;
  ranks: RanksBridge;
  relatedRanks: RelatedRanksBridge;
  linearRegression: LinearRegressionBridge;
}

/** The bridges the app uses, for any a caller does not give. */
export function defaultRunBridges(given: Partial<RunBridges> = {}): RunBridges {
  return {
    describe: given.describe ?? describeBridge(),
    ttest: given.ttest ?? ttestBridge(),
    oneSample: given.oneSample ?? oneSampleBridge(),
    paired: given.paired ?? pairedBridge(),
    anova: given.anova ?? anovaBridge(),
    corr: given.corr ?? corrBridge(),
    alpha: given.alpha ?? alphaBridge(),
    xtab: given.xtab ?? xtabBridge(),
    ranks: given.ranks ?? ranksBridge(),
    relatedRanks: given.relatedRanks ?? relatedRanksBridge(),
    linearRegression: given.linearRegression ?? linearRegressionBridge(),
  };
}

/** Each kind's job name, from the methods table (Phase 8C): none for the
 *  summary, whose sentence has no parenthesis. */
const KIND_NAMES: Record<RunEntry['kind'], string | null> = byKind((m) => m.jobName);

/** What the shell does with a run of one kind, through that kind's own
 *  commands (decision 80). */
export interface KindCommands {
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

/** Each kind's status and cancel, from the bridges given (Phase 8C). */
export function runKinds({ describe: runs, ttest, oneSample, paired, anova, corr, alpha, xtab, ranks, relatedRanks, linearRegression }: RunBridges): Record<RunEntry['kind'], KindCommands> {
  return {
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
    anova: {
      status: async (runId) => {
        const r = await anova.status(runId).catch(() => null);
        return (
          r && {
            runId: r.runId,
            state: r.state,
            terminal: r.terminal,
            progress: `${stepsSentence(r.stepsDone, r.stepsTotal)}.`,
            kind: 'anova',
          }
        );
      },
      cancel: (runId) => anova.cancel(runId).catch(() => null),
    },
    correlations: {
      status: async (runId) => {
        const r = await corr.status(runId).catch(() => null);
        return (
          r && {
            runId: r.runId,
            state: r.state,
            terminal: r.terminal,
            progress: `${stepsSentence(r.stepsDone, r.stepsTotal)}.`,
            kind: 'correlations',
          }
        );
      },
      cancel: (runId) => corr.cancel(runId).catch(() => null),
    },
    cronbach_alpha: {
      status: async (runId) => {
        const r = await alpha.status(runId).catch(() => null);
        return (
          r && {
            runId: r.runId,
            state: r.state,
            terminal: r.terminal,
            progress: `${stepsSentence(r.stepsDone, r.stepsTotal)}.`,
            kind: 'cronbach_alpha',
          }
        );
      },
      cancel: (runId) => alpha.cancel(runId).catch(() => null),
    },
    crosstabs: {
      status: async (runId) => {
        const r = await xtab.status(runId).catch(() => null);
        return r && { runId: r.runId, state: r.state, terminal: r.terminal, progress: null, kind: 'crosstabs' };
      },
      cancel: (runId) => xtab.cancel(runId).catch(() => null),
    },
    independent_ranks: {
      status: async (runId) => {
        const r = await ranks.status(runId).catch(() => null);
        return r && { runId: r.runId, state: r.state, terminal: r.terminal, progress: null, kind: 'independent_ranks' };
      },
      cancel: (runId) => ranks.cancel(runId).catch(() => null),
    },
    related_ranks: {
      status: async (runId) => {
        const r = await relatedRanks.status(runId).catch(() => null);
        return r && { runId: r.runId, state: r.state, terminal: r.terminal, progress: null, kind: 'related_ranks' };
      },
      cancel: (runId) => relatedRanks.cancel(runId).catch(() => null),
    },
    // Research OS, Phase 9A-2 (decision 687): one check point, so no progress.
    linear_regression: {
      status: async (runId) => {
        const r = await linearRegression.status(runId).catch(() => null);
        return r && { runId: r.runId, state: r.state, terminal: r.terminal, progress: null, kind: 'linear_regression' };
      },
      cancel: (runId) => linearRegression.cancel(runId).catch(() => null),
    },
  };
}

export interface WorkspaceSwitchOptions {
  bridge: WorkspaceBridge;
  runs: RunBridges;
  /** How often the blocking run is read while the dialog is open. */
  pollMs: number;
  /** Where a refusal's sentence goes; null clears it. */
  setMessage: (message: string | null) => void;
}

export interface WorkspaceSwitch {
  /** Asks Rust to close the workspace; on a yes, goes to the chooser. */
  leave: () => Promise<void>;
  /** A run's state, through its kind's status command (decision 80). */
  statusOf: (e: RunEntry) => Promise<JobView | null>;
  /** The Wait and Cancel dialog: the caller places it, once. */
  dialog: React.ReactElement;
}

export function useWorkspaceSwitch({ bridge, runs, pollMs, setMessage }: WorkspaceSwitchOptions): WorkspaceSwitch {
  const navigate = useNavigate();
  /** The run that blocked a switch: the dialog is open while this is set. */
  const [blocking, setBlocking] = useState<RunEntry | null>(null);
  const [blockingRun, setBlockingRun] = useState<JobView | null>(null);
  const [waiting, setWaiting] = useState(false);
  /** The sentence before leaving is up (decision 472). */
  const [asking, setAsking] = useState(false);
  const alive = useRef(true);

  useEffect(() => {
    alive.current = true;
    return () => {
      alive.current = false;
    };
  }, []);

  const kinds = useMemo(() => runKinds(runs), [runs]);

  const statusOf = useCallback((e: RunEntry): Promise<JobView | null> => commandsFor(kinds, e.kind).status(e.runId), [kinds]);

  const deactivate = useCallback(async () => {
    setMessage(null);
    try {
      await bridge.deactivate();
      // Leaving Statistics closed the open project in Rust (decision 135).
      openProjectStore.clear();
      // It dropped the engine and its slots too (decision 450).
      statisticsSession.clear();
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
  }, [bridge, navigate, setMessage]);

  // Asked once, here, before the first deactivate; the Wait path's own deactivate
  // does not ask again. With an empty session it goes straight on, as ever.
  const leave = useCallback(async () => {
    if (holdsUnsavedWork(statisticsSession.get())) {
      setAsking(true);
      return;
    }
    await deactivate();
  }, [deactivate]);

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
        await deactivate();
      }
    };
    tick();
    const t = setInterval(tick, pollMs);
    return () => {
      stop = true;
      clearInterval(t);
    };
  }, [blocking, waiting, statusOf, pollMs, deactivate]);

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

  const dialog = (
    <>
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
    {asking && (
      <AskFirst
        title={ASK_LEAVE.title}
        sentence={ASK_LEAVE.sentence}
        testId="sw-ask"
        onCancel={() => setAsking(false)}
        onContinue={() => {
          setAsking(false);
          void deactivate();
        }}
      />
    )}
    </>
  );

  return { leave, statusOf, dialog };
}
