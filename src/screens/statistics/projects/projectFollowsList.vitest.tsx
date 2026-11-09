// The project page follows the window's list of reruns (Research OS, Phase
// 8U-3, decision 556; ADR-012:647, decision 541), over mock bridges.
//
// * In the poll: a going answer about this project with another run id is
//   followed, its outcome cleared and the runs read again; an ended one is
//   read and no longer followed, with no outcome sentence; no rerun, or
//   another project, is unread, as before.
// * From the list's store: its going run is followed after the poll has
//   stopped; the page's Run again buttons and Close the project stay disabled
//   while the list goes in this project, also between two of its reruns; the
//   page's Cancel cancels the list's run; a follow moves no focus.
import React from 'react';
import { act, cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { makeMockWorkspaceBridge } from '../../workspace/workspaceBridge';
import type { WorkspaceStateView } from '../../workspace/workspaceTypes';
import { makeMockDescribeBridge } from '../describeBridge';
import { makeMockTTestBridge } from '../ttestBridge';
import { rerunAllStore } from '../window/rerunAll';
import { statisticsSession } from '../window/statisticsSession';
import { makeMockProjectBridge } from './projectBridge';
import { openProjectStore } from './projectSession';
import type { ProjectCurrentView, RerunView, StoredRunView } from './projectTypes';
import { PROJECT_TEXT, RERUN_WORDS } from './projectWording';
import { ProjectPage } from './ProjectPage';

vi.mock('../../../config/featureFlags', () => ({
  isFeatureEnabled: (name: string) => name === 'projects' || name === 'workspaces' || name === 'statsDescribe',
}));

const A = '0123456789abcdef0123456789abcdef';
const C = '00000000000000000000000000000c0c';
const ACTIVE: WorkspaceStateView = { state: { phase: 'active', workspace: 'statistics' }, activeRun: null, latestRun: null };

const stored_ = (id: number, capabilityId: string, over: Partial<StoredRunView> = {}): StoredRunView => ({
  storedRunId: id,
  capabilityId,
  capabilityVersion: 1,
  state: 'succeeded',
  endCode: null,
  startedAt: 1792396000 + id * 10,
  endedAt: 1792396001 + id * 10,
  savedAt: 1792396002 + id * 10,
  agentVersion: '0.1.0',
  rerunOf: null,
  comparison: null,
  engineVersionDiffers: null,
  ...over,
});
/** A summary (1) and a t-test (2). */
const RUNS: StoredRunView[] = [stored_(1, 'stats.describe'), stored_(2, 'stats.analyze.independent_t')];

const rv = (runId: number, rerunOf: number, status: RerunView['status'], extra: Partial<RerunView> = {}): RerunView => ({
  runId,
  rerunOf,
  status,
  storedRunId: null,
  code: null,
  ...extra,
});
const here = (rerun: RerunView | null): ProjectCurrentView => ({ projectId: A, rerun });

interface World {
  current: ProjectCurrentView;
  runs: StoredRunView[];
  /** When a list is going, Rust's one answer is the list's: the page reads it. */
  list?: { state: { answer: ProjectCurrentView } };
}

/** The page, its bridges answering from `w`, which a test changes. */
function page(w: World) {
  const bridge = makeMockProjectBridge({
    project_current: () => (w.list ? w.list.state.answer : w.current),
    project_list: [
      { projectId: A, name: 'Chapter 3', bytes: 40000, savedAt: 1792396351, runCount: 2, refused: null },
      { projectId: C, name: 'Chapter 4', bytes: 40000, savedAt: 1792396351, runCount: 1, refused: null },
    ],
    project_sizes: { totalBytes: 80000, projectCount: 2, projectCapBytes: 104857600, allProjectsCapBytes: 10737418240 },
    project_runs: () => w.runs,
    project_rerun: { runId: 7, kind: 'describe' },
  });
  const describe = makeMockDescribeBridge({ stats_desc_cancel: { runId: 0 } });
  const ttest = makeMockTTestBridge({ stats_ttest_cancel: { runId: 0 } });
  render(
    <ProjectPage
      projectId={A}
      bridge={bridge}
      workspace={makeMockWorkspaceBridge({ workspace_state: ACTIVE })}
      describe={describe}
      ttest={ttest}
      desktop
      save={async () => null}
      pollMs={5}
      onClosed={vi.fn()}
      onBack={vi.fn()}
    />
  );
  return { bridge, describe, ttest };
}

const calls = (b: { calls: Array<[string, Record<string, unknown>]> }, cmd: string) => b.calls.filter(([c]) => c === cmd);
const wait = (ms: number) => act(async () => new Promise((r) => setTimeout(r, ms)));
const isDisabled = (id: string) => (screen.getByTestId(id) as HTMLButtonElement).disabled;

/* ---- a list the test drives, through the store's own start ----------------- */

/** A list started in the store on its own bridge, which the test steps:
 *  `next` decides each project_current answer the driver reads. */
function listDriver(projectId: string) {
  const state: { answer: ProjectCurrentView; runs: StoredRunView[]; started: number[]; hold: boolean } = {
    answer: { projectId, rerun: null },
    runs: RUNS,
    started: [],
    // While true, the driver waits in its read of the runs after a save:
    // between two reruns, for as long as a test needs.
    hold: false,
  };
  let nextRun = 40;
  const bridge = {
    current: async () => state.answer,
    rerun: async (storedRunId: number) => {
      nextRun += 1;
      state.started.push(storedRunId);
      state.answer = { projectId, rerun: rv(nextRun, storedRunId, 'going') };
      return { runId: nextRun, kind: 'describe' as const };
    },
    runs: async () => {
      while (state.hold) {
        // eslint-disable-next-line no-await-in-loop
        await new Promise((r) => setTimeout(r, 2));
      }
      return state.runs;
    },
  };
  const cancel = vi.fn(async () => ({}));
  const started = rerunAllStore.start(projectId, RUNS, {
    bridge: bridge as never,
    kinds: { describe: { cancel }, ttest: { cancel } },
    pollMs: 5,
  });
  expect(started).toBe(true);
  return { state, cancel, last: () => nextRun };
}

beforeEach(() => {
  openProjectStore.clear();
});
afterEach(async () => {
  cleanup();
  rerunAllStore.stop();
  await new Promise((r) => setTimeout(r, 30));
  rerunAllStore.clear();
  statisticsSession.clear();
});

describe('the poll follows another rerun of this project', () => {
  it('a going answer with another run id, seen by the poll itself: followed, and no unread line', async () => {
    let n = 0;
    const w: World = { current: here(rv(7, 1, 'going')), runs: RUNS };
    const { bridge } = page(w);
    await screen.findByTestId('project-rerun-running');
    const before = calls(bridge, 'project_runs').length;
    w.current = here(rv(8, 2, 'going'));
    await waitFor(() => expect(calls(bridge, 'project_runs').length).toBe(before + 1));
    await wait(30);
    expect(screen.queryByTestId('project-rerun-unread')).toBeNull();
    expect(screen.getByTestId('project-rerun-running')).toBeTruthy();
    // It polls 8 now: 8 ends saved, with its outcome.
    w.runs = [...RUNS, stored_(3, 'stats.analyze.independent_t', { rerunOf: 2, comparison: 'identical', engineVersionDiffers: false })];
    w.current = here(rv(8, 2, 'saved', { storedRunId: 3 }));
    expect((await screen.findByTestId('project-rerun-outcome')).textContent).toBe(RERUN_WORDS.saved);
    n = calls(bridge, 'project_current').length;
    await wait(30);
    expect(calls(bridge, 'project_current')).toHaveLength(n);
  });

  it('an ended answer with another run id: the runs read again, no longer followed, and no outcome sentence', async () => {
    const w: World = { current: here(rv(7, 1, 'going')), runs: RUNS };
    const { bridge } = page(w);
    await screen.findByTestId('project-rerun-running');
    const before = calls(bridge, 'project_runs').length;
    w.current = here(rv(8, 2, 'saved', { storedRunId: 4 }));
    await waitFor(() => expect(screen.queryByTestId('project-rerun-running')).toBeNull());
    expect(calls(bridge, 'project_runs')).toHaveLength(before + 1);
    expect(screen.queryByTestId('project-rerun-outcome')).toBeNull();
    expect(screen.queryByTestId('project-rerun-unread')).toBeNull();
    expect(isDisabled('project-page-close')).toBe(false);
    const n = calls(bridge, 'project_current').length;
    await wait(30);
    expect(calls(bridge, 'project_current')).toHaveLength(n);
  });

  it.each([
    ['no rerun', here(null)],
    ['another project', { projectId: C, rerun: rv(8, 2, 'going') } as ProjectCurrentView],
  ])('%s: unread, as before', async (_n, answer) => {
    const w: World = { current: here(rv(7, 1, 'going')), runs: RUNS };
    page(w);
    await screen.findByTestId('project-rerun-running');
    w.current = answer;
    expect((await screen.findByTestId('project-rerun-unread')).textContent).toBe(PROJECT_TEXT.pageUnread);
  });
});

describe("the page follows the list's going run", () => {
  it('after its own poll has stopped, clearing the outcome; disabled throughout, also between two reruns', async () => {
    const w: World = { current: here(null), runs: RUNS };
    page(w);
    await screen.findByTestId('project-runs-list');
    expect(isDisabled('project-page-close')).toBe(false);
    const list = listDriver(A);
    w.list = list;
    // The first rerun, of run 1 (the oldest saved first).
    await waitFor(() => expect(list.state.started).toEqual([1]));
    expect(await screen.findByTestId('project-rerun-running')).toBeTruthy();
    expect(isDisabled('project-page-close')).toBe(true);
    expect(isDisabled('project-run-rerun-1')).toBe(true);
    // It ends saved: the page's poll sees it, and stops; the driver is held
    // in its read of the runs, so the list sits between two reruns.
    list.state.hold = true;
    list.state.answer = here(rv(list.last(), 1, 'saved', { storedRunId: 3 }));
    expect((await screen.findByTestId('project-rerun-outcome')).textContent).toBe(RERUN_WORDS.saved);
    // Between the two, still disabled.
    await wait(30);
    expect(screen.queryByTestId('project-rerun-running')).toBeNull();
    expect(isDisabled('project-page-close')).toBe(true);
    expect(isDisabled('project-run-rerun-1')).toBe(true);
    list.state.hold = false;
    await waitFor(() => expect(list.state.started).toEqual([1, 2]));
    expect(isDisabled('project-page-close')).toBe(true);
    expect(isDisabled('project-run-rerun-2')).toBe(true);
    // The second is followed from the store: the outcome cleared, Cancel back.
    await waitFor(() => expect(screen.queryByTestId('project-rerun-outcome')).toBeNull());
    expect(await screen.findByTestId('project-rerun-cancel')).toBeTruthy();
  });

  it('Run again stays disabled between two of its reruns, while nothing is going', async () => {
    const w: World = { current: here(null), runs: RUNS };
    page(w);
    await screen.findByTestId('project-runs-list');
    // A list whose next start never answers: going, with no run of its own.
    rerunAllStore.start(A, RUNS, {
      bridge: { current: async () => here(null), rerun: () => new Promise(() => undefined), runs: async () => RUNS } as never,
      kinds: {},
      pollMs: 5,
    });
    await wait(30);
    expect(rerunAllStore.get().going?.runId ?? null).toBeNull();
    expect(screen.queryByTestId('project-rerun-running')).toBeNull();
    expect(isDisabled('project-run-rerun-1')).toBe(true);
    expect(isDisabled('project-run-rerun-2')).toBe(true);
    expect(isDisabled('project-page-close')).toBe(true);
  });

  it("the page's Cancel cancels the list's run, by its kind", async () => {
    const w: World = { current: here(null), runs: RUNS };
    const { describe } = page(w);
    await screen.findByTestId('project-runs-list');
    const list = listDriver(A);
    w.list = list;
    await waitFor(() => expect(list.state.started).toEqual([1]));
    const cancel = await screen.findByTestId('project-rerun-cancel');
    await waitFor(() => expect((cancel as HTMLButtonElement).disabled).toBe(false));
    fireEvent.click(cancel);
    await waitFor(() => expect(describe.calls).toContainEqual(['stats_desc_cancel', { runId: list.last() }]));
  });

  it('a follow moves no focus', async () => {
    const w: World = { current: here(null), runs: RUNS };
    page(w);
    await screen.findByTestId('project-runs-list');
    const open = screen.getByTestId('project-run-open-2');
    open.focus();
    const list = listDriver(A);
    w.list = list;
    await waitFor(() => expect(list.state.started).toEqual([1]));
    await screen.findByTestId('project-rerun-running');
    await wait(20);
    expect(document.activeElement).toBe(open);
  });

  it("a list in another project is not followed, and disables nothing", async () => {
    const w: World = { current: here(null), runs: RUNS };
    page(w);
    await screen.findByTestId('project-runs-list');
    const list = listDriver(C);
    await waitFor(() => expect(list.state.started).toEqual([1]));
    await wait(20);
    expect(screen.queryByTestId('project-rerun-running')).toBeNull();
    expect(isDisabled('project-page-close')).toBe(false);
  });
});
