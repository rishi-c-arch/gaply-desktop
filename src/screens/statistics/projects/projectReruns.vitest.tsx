// Reruns on screen (Research OS, Phase 5C-1b; decisions 191 and 197 to 200),
// over mock project, workspace, summary and t-test bridges. project_current's
// answers are driven over time through a value each test changes.
//
// * Run again: on each saved result that finished and on no other; its
//   dialog's words; one project_rerun per click; nothing on Cancel; a
//   refusal's sentence, then Close with the focus; nothing in the browser
//   build.
// * While it runs: the status line; going then saved (the runs read once, the
//   saved sentence) and going then not saved (two sentences); an outcome the
//   page finds when it appears says nothing; Cancel by the right kind,
//   disabled until the kind is known; the page opened while a rerun is going;
//   the disabled controls and their one sentence; the panel's Delete…; a poll
//   beside an open saved result and an open export dialog; a failed poll.
// * A rerun's words in the list and in its saved result view.
// * The design system's secondary button on Open this result, Run again and
//   the panel's Delete….
import { createHash } from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import React from 'react';
import { act, cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { makeMockWorkspaceBridge } from '../../workspace/workspaceBridge';
import type { WorkspaceStateView } from '../../workspace/workspaceTypes';
import { makeMockDescribeBridge } from '../describeBridge';
import { makeMockTTestBridge } from '../ttestBridge';
import { makeMockProjectBridge } from './projectBridge';
import type { SaveFile } from './projectExport';
import { openProjectStore } from './projectSession';
import type { ProjectCurrentView, ProjectListEntryView, RerunView, StoredRunView } from './projectTypes';
import { EXPORT_WORDS, PROJECT_TEXT, PROJECT_WORDS, RERUN_SENTENCE, RERUN_WORDS, savedOn } from './projectWording';
import { ProjectPage } from './ProjectPage';
import { ProjectsPanel } from './ProjectsPanel';

vi.mock('../../../config/featureFlags', () => ({
  isFeatureEnabled: (name: string) => name === 'projects' || name === 'workspaces' || name === 'statsDescribe',
}));

beforeEach(() => openProjectStore.clear());
afterEach(cleanup);

const A = '0123456789abcdef0123456789abcdef';
const C = '00000000000000000000000000000c0c';
const ACTIVE: WorkspaceStateView = { state: { phase: 'active', workspace: 'statistics' }, activeRun: null, latestRun: null };
const LIST: ProjectListEntryView[] = [
  { projectId: A, name: 'Chapter 3', bytes: 40000, savedAt: 1792396351, runCount: 3, refused: null },
  { projectId: C, name: 'Chapter 4', bytes: 40000, savedAt: 1792396351, runCount: 1, refused: null },
];
const SIZES = { totalBytes: 80000, projectCount: 2, projectCapBytes: 104857600, allProjectsCapBytes: 10737418240 };
const STORED = path.join(__dirname, '../../../../src-tauri/tests/fixtures/projects/stored');
const stored = (name: string) => JSON.parse(fs.readFileSync(path.join(STORED, name), 'utf8')) as unknown;
const IN_PROGRESS = PROJECT_WORDS['project.rerun_in_progress']({ code: 'project.rerun_in_progress' });

const SUMMARY = 'stats.describe';
const TTEST = 'stats.analyze.independent_t';

function stored_(id: number, capabilityId: string, state: StoredRunView['state'], savedAt: number, over: Partial<StoredRunView> = {}): StoredRunView {
  return {
    storedRunId: id,
    capabilityId,
    capabilityVersion: 1,
    state,
    endCode: state === 'failed' ? 'project.run_interrupted' : null,
    startedAt: savedAt - 10,
    endedAt: savedAt - 9,
    savedAt,
    agentVersion: '0.1.0',
    rerunOf: null,
    comparison: null,
    engineVersionDiffers: null,
    ...over,
  };
}

/** A summary (1), a failed t-test (2), a t-test (3) and a cancelled summary
 *  (9): only 1 and 3 finished. */
const RUNS: StoredRunView[] = [
  stored_(1, SUMMARY, 'succeeded', 1792396100),
  stored_(2, TTEST, 'failed', 1792396200),
  stored_(3, TTEST, 'succeeded', 1792396300),
  stored_(9, SUMMARY, 'cancelled', 1792396050),
];

const rerunView = (runId: number, rerunOf: number, status: RerunView['status'], extra: Partial<RerunView> = {}): RerunView => ({
  runId,
  rerunOf,
  status,
  storedRunId: null,
  code: null,
  ...extra,
});
const currentWith = (rerun: RerunView | null): ProjectCurrentView => ({ projectId: A, rerun });

const refusal = (v: unknown) => ({ code: 'validation', message: JSON.stringify(v) });

function deferred<T>() {
  let resolve!: (v: T) => void;
  const promise = new Promise<T>((res) => {
    resolve = res;
  });
  return { promise, resolve };
}

/** The values the mocks answer with, which a test changes over time. */
interface World {
  current: ProjectCurrentView | (() => ProjectCurrentView);
  runs: StoredRunView[] | Promise<StoredRunView[]>;
  rerun: (args: Record<string, unknown>) => unknown;
  runResult: (args: Record<string, unknown>) => unknown;
  report: unknown;
}

function world(over: Partial<World> = {}): World {
  return {
    current: currentWith(null),
    runs: RUNS,
    rerun: () => ({ runId: 7, kind: 'describe' }),
    runResult: ({ storedRunId }) => stored(storedRunId === 1 ? 'summary.json' : 'ttest.json'),
    report: null,
    ...over,
  };
}

function page(w: World, opts: { desktop?: boolean; save?: SaveFile } = {}) {
  const bridge = makeMockProjectBridge({
    project_current: () => (typeof w.current === 'function' ? w.current() : w.current),
    project_list: LIST,
    project_sizes: SIZES,
    project_runs: () => w.runs,
    project_rerun: (a: Record<string, unknown>) => w.rerun(a),
    project_run_result: (a: Record<string, unknown>) => w.runResult(a),
    project_report: () => w.report,
    project_close: { closed: true },
  });
  const workspace = makeMockWorkspaceBridge({ workspace_state: ACTIVE });
  const describe = makeMockDescribeBridge({ stats_desc_cancel: { runId: 0 } });
  const ttest = makeMockTTestBridge({ stats_ttest_cancel: { runId: 0 } });
  const save: SaveFile = opts.save ?? (async () => '/chosen/place');
  render(
    <ProjectPage
      projectId={A}
      bridge={bridge}
      workspace={workspace}
      describe={describe}
      ttest={ttest}
      desktop={opts.desktop ?? true}
      save={save}
      pollMs={5}
      onClosed={vi.fn()}
      onBack={vi.fn()}
    />
  );
  return { bridge, describe, ttest, save };
}

const calls = (b: { calls: Array<[string, Record<string, unknown>]> }, cmd: string) => b.calls.filter(([c]) => c === cmd);
const wait = (ms: number) => act(async () => new Promise((r) => setTimeout(r, ms)));
const isDisabled = (id: string) => (screen.getByTestId(id) as HTMLButtonElement).disabled;

/* ---- Run again and its dialog (decisions 191 and 199) --------------------- */

describe('Run again and its dialog', () => {
  it('is on each saved result that finished, and on no other', async () => {
    page(world());
    await screen.findByTestId('project-runs-list');
    expect(screen.getByTestId('project-run-rerun-1').textContent).toBe('Run again');
    expect(screen.getByTestId('project-run-rerun-3').textContent).toBe('Run again');
    expect(screen.queryByTestId('project-run-rerun-2')).toBeNull();
    expect(screen.queryByTestId('project-run-rerun-9')).toBeNull();
    // Every row keeps its Open this result.
    for (const id of [1, 2, 3, 9]) expect(screen.getByTestId(`project-run-open-${id}`)).toBeTruthy();
  });

  it("shows decision 191's words; one project_rerun per click, with that row's id; nothing on Cancel", async () => {
    const { bridge } = page(world());
    await screen.findByTestId('project-runs-list');
    fireEvent.click(screen.getByTestId('project-run-rerun-3'));
    const dialog = screen.getByTestId('project-rerun-dialog');
    expect(screen.getByTestId('project-rerun-sentence').textContent).toBe(RERUN_SENTENCE);
    expect(within(dialog).getByTestId('project-rerun-dialog-cancel').textContent).toBe('Cancel');
    expect(within(dialog).getByTestId('project-rerun-confirm').textContent).toBe('Run again');
    expect(document.activeElement).toBe(within(dialog).getByTestId('project-rerun-dialog-cancel'));
    fireEvent.click(screen.getByTestId('project-rerun-dialog-cancel'));
    expect(screen.queryByTestId('project-rerun-dialog')).toBeNull();
    await wait(20);
    expect(calls(bridge, 'project_rerun')).toEqual([]);

    // Two clicks in one event turn: one call.
    fireEvent.click(screen.getByTestId('project-run-rerun-3'));
    const confirm = screen.getByTestId('project-rerun-confirm');
    fireEvent.click(confirm);
    fireEvent.click(confirm);
    await screen.findByTestId('project-rerun-running');
    expect(calls(bridge, 'project_rerun')).toEqual([['project_rerun', { runId: 3 }]]);
    // A start closes the dialog and shows the status line.
    expect(screen.queryByTestId('project-rerun-dialog')).toBeNull();
    expect(screen.getByTestId('project-rerun-running').textContent).toBe('Running again…');
  });

  it("shows a refusal by its code's sentence; Cancel then reads Close, has the focus, and closes it", async () => {
    const w = world({
      rerun: () => {
        throw refusal({ code: 'project.rerun_busy' });
      },
    });
    const { bridge } = page(w);
    await screen.findByTestId('project-runs-list');
    fireEvent.click(screen.getByTestId('project-run-rerun-1'));
    fireEvent.click(screen.getByTestId('project-rerun-confirm'));
    const said = await screen.findByTestId('project-rerun-refusal');
    expect(said.textContent).toBe(PROJECT_WORDS['project.rerun_busy']({ code: 'project.rerun_busy' }));
    const close = screen.getByTestId('project-rerun-dialog-cancel');
    expect(close.textContent).toBe('Close');
    await waitFor(() => expect(document.activeElement).toBe(close));
    expect(screen.getByTestId('project-rerun-confirm')).toBeTruthy();
    expect(screen.queryByTestId('project-rerun-running')).toBeNull();
    fireEvent.click(close);
    expect(screen.queryByTestId('project-rerun-dialog')).toBeNull();
    await waitFor(() => expect(document.activeElement).toBe(screen.getByTestId('project-run-rerun-1')));
    expect(calls(bridge, 'project_rerun')).toHaveLength(1);
  });

  it('shows no Run again in the browser build, and makes no project call there', async () => {
    const { bridge } = page(world({ current: currentWith(rerunView(7, 1, 'going')) }), { desktop: false });
    await wait(30);
    expect(screen.queryByTestId('project-page')).toBeNull();
    expect(screen.queryByText('Run again')).toBeNull();
    expect(screen.queryByTestId('project-rerun-running')).toBeNull();
    expect(bridge.calls).toEqual([]);
  });
});

/* ---- while it runs, and its outcome (decisions 198 and 199) --------------- */

describe('while a rerun runs, and its outcome', () => {
  async function started(w: World, row = 1) {
    const p = page(w);
    await screen.findByTestId('project-runs-list');
    w.current = currentWith(rerunView(7, row, 'going'));
    fireEvent.click(screen.getByTestId(`project-run-rerun-${row}`));
    fireEvent.click(screen.getByTestId('project-rerun-confirm'));
    await screen.findByTestId('project-rerun-running');
    return p;
  }

  it('going, then saved: the status line, then the runs read once and the saved sentence', async () => {
    const w = world();
    const { bridge } = await started(w);
    await waitFor(() => expect(calls(bridge, 'project_current').length).toBeGreaterThanOrEqual(4));
    expect(screen.getByTestId('project-rerun-running').textContent).toBe(PROJECT_TEXT.runningAgain);
    expect(calls(bridge, 'project_runs')).toHaveLength(1);
    expect(screen.queryByTestId('project-rerun-outcome')).toBeNull();

    w.runs = [...RUNS, stored_(4, SUMMARY, 'succeeded', 1792396400, { rerunOf: 1, comparison: 'identical', engineVersionDiffers: false })];
    w.current = currentWith(rerunView(7, 1, 'saved', { storedRunId: 4 }));
    await waitFor(() => expect(screen.getByTestId('project-rerun-outcome').textContent).toBe(RERUN_WORDS.saved));
    expect(screen.getByTestId('project-rerun-outcome').textContent).toBe('The new run is saved in this project.');
    expect(calls(bridge, 'project_runs')).toHaveLength(2);
    expect(screen.getByTestId('project-run-4')).toBeTruthy();
    expect(screen.queryByTestId('project-rerun-running')).toBeNull();
    expect(isDisabled('project-run-rerun-1')).toBe(false);
    expect(isDisabled('project-page-close')).toBe(false);
    expect(screen.queryByText(IN_PROGRESS)).toBeNull();
    // The poll has stopped, and reading the runs read nothing else.
    const n = calls(bridge, 'project_current').length;
    await wait(40);
    expect(calls(bridge, 'project_current')).toHaveLength(n);
    expect(calls(bridge, 'project_runs')).toHaveLength(2);
  });

  it("going, then not saved with a code: the two sentences, and the runs are not read again", async () => {
    const w = world();
    const { bridge } = await started(w);
    await waitFor(() => expect(calls(bridge, 'project_current').length).toBeGreaterThanOrEqual(3));
    w.current = currentWith(rerunView(7, 1, 'not_saved', { code: 'project.disk_full' }));
    await waitFor(() =>
      expect(screen.getByTestId('project-rerun-outcome').textContent).toBe(
        "The new run was not saved. This computer's disk is full, so nothing was saved."
      )
    );
    expect(screen.queryByTestId('project-rerun-running')).toBeNull();
    expect(calls(bridge, 'project_runs')).toHaveLength(1);
    expect(isDisabled('project-page-close')).toBe(false);
  });

  it.each([
    ['saved', rerunView(7, 1, 'saved', { storedRunId: 4 })],
    ['not saved', rerunView(7, 1, 'not_saved', { code: 'project.run_cancelled' })],
  ])('a page that first appears with a %s outcome shows no sentence and does not poll', async (_name, rerun) => {
    const { bridge } = page(world({ current: currentWith(rerun) }));
    await screen.findByTestId('project-runs-list');
    await wait(40);
    expect(screen.queryByTestId('project-rerun-outcome')).toBeNull();
    expect(screen.queryByTestId('project-rerun-running')).toBeNull();
    expect(calls(bridge, 'project_current')).toHaveLength(1);
    expect(isDisabled('project-run-rerun-1')).toBe(false);
  });

  it("Cancel calls the kind's own cancel, by the kind project_rerun returned", async () => {
    const w = world({ rerun: () => ({ runId: 7, kind: 'ttest' }) });
    const { describe, ttest } = await started(w, 3);
    const cancel = screen.getByTestId('project-rerun-cancel');
    expect(cancel.textContent).toBe('Cancel');
    expect((cancel as HTMLButtonElement).disabled).toBe(false);
    fireEvent.click(cancel);
    fireEvent.click(cancel);
    await waitFor(() => expect(calls(ttest, 'stats_ttest_cancel')).toEqual([['stats_ttest_cancel', { runId: 7 }]]));
    expect(describe.calls).toEqual([]);
    expect(ttest.calls.map(([c]) => c)).toEqual(['stats_ttest_cancel']);
    expect(screen.getByTestId('project-rerun-cancel').textContent).toBe(PROJECT_TEXT.cancelling);
    // Its end comes through the poll.
    w.current = currentWith(rerunView(7, 3, 'not_saved', { code: 'project.run_cancelled' }));
    await waitFor(() =>
      expect(screen.getByTestId('project-rerun-outcome').textContent).toBe(
        `${RERUN_WORDS.notSaved} ${PROJECT_WORDS['project.run_cancelled']({ code: 'project.run_cancelled' })}`
      )
    );
  });

  it('a page opened while a rerun is going shows the status line; Cancel is disabled until the runs name its kind', async () => {
    const runs = deferred<StoredRunView[]>();
    const { describe, ttest } = page(world({ current: currentWith(rerunView(9, 1, 'going')), runs: runs.promise }));
    const line = await screen.findByTestId('project-rerun-running');
    expect(line.textContent).toBe('Running again…');
    expect(isDisabled('project-rerun-cancel')).toBe(true);
    expect(isDisabled('project-page-close')).toBe(true);
    fireEvent.click(screen.getByTestId('project-rerun-cancel'));
    await act(async () => runs.resolve(RUNS));
    await screen.findByTestId('project-runs-list');
    await waitFor(() => expect(isDisabled('project-rerun-cancel')).toBe(false));
    fireEvent.click(screen.getByTestId('project-rerun-cancel'));
    await waitFor(() => expect(calls(describe, 'stats_desc_cancel')).toEqual([['stats_desc_cancel', { runId: 9 }]]));
    expect(ttest.calls).toEqual([]);
  });

  it('the same for a repeated t-test: its own cancel', async () => {
    const { describe, ttest } = page(world({ current: currentWith(rerunView(9, 3, 'going')) }));
    await screen.findByTestId('project-runs-list');
    await waitFor(() => expect(isDisabled('project-rerun-cancel')).toBe(false));
    fireEvent.click(screen.getByTestId('project-rerun-cancel'));
    await waitFor(() => expect(calls(ttest, 'stats_ttest_cancel')).toEqual([['stats_ttest_cancel', { runId: 9 }]]));
    expect(describe.calls).toEqual([]);
  });

  it('disables every Run again and Close the project, with the sentence beside Close and once for the list', async () => {
    page(world({ current: currentWith(rerunView(9, 1, 'going')) }));
    await screen.findByTestId('project-runs-list');
    expect(isDisabled('project-run-rerun-1')).toBe(true);
    expect(isDisabled('project-run-rerun-3')).toBe(true);
    expect(isDisabled('project-page-close')).toBe(true);
    expect(isDisabled('project-run-open-1')).toBe(false);
    expect(screen.getByTestId('project-close-going').textContent).toBe(IN_PROGRESS);
    expect(screen.getByTestId('project-runs-going').textContent).toBe(IN_PROGRESS);
    expect(screen.getAllByText(IN_PROGRESS)).toHaveLength(2);
  });

  it('a poll beside an open saved result leaves it working', async () => {
    const result = deferred<unknown>();
    const w = world({ current: currentWith(rerunView(9, 1, 'going')), runResult: () => result.promise });
    const { bridge } = page(w);
    await screen.findByTestId('project-runs-list');
    fireEvent.click(screen.getByTestId('project-run-open-1'));
    await screen.findByTestId('saved-result-loading');
    const before = calls(bridge, 'project_current').length;
    await waitFor(() => expect(calls(bridge, 'project_current').length).toBeGreaterThanOrEqual(before + 3));
    await act(async () => result.resolve(stored('summary.json')));
    expect(await screen.findByTestId('desc-numeric-table')).toBeTruthy();
    expect(screen.getByTestId('project-rerun-running')).toBeTruthy();
  });

  it('a poll beside an open export dialog leaves it working', async () => {
    const text = '# Chapter 3\n';
    const bytes = Array.from(new TextEncoder().encode(text));
    const sha256 = createHash('sha256').update(Uint8Array.from(bytes)).digest('hex');
    const w = world({ current: currentWith(rerunView(9, 1, 'going')), report: { fileName: 'gaply_project_report.md', bytes, sha256 } });
    const save = vi.fn(async (): Promise<string | null> => '/chosen/place');
    const { bridge } = page(w, { save });
    await screen.findByTestId('project-runs-list');
    fireEvent.click(screen.getByTestId('project-export-report'));
    await screen.findByTestId('project-export-dialog');
    const before = calls(bridge, 'project_current').length;
    await waitFor(() => expect(calls(bridge, 'project_current').length).toBeGreaterThanOrEqual(before + 3));
    fireEvent.click(screen.getByTestId('project-export-confirm'));
    await waitFor(() => expect(screen.getByTestId('project-page-message').textContent).toBe(EXPORT_WORDS.saved(sha256)));
    expect(save).toHaveBeenCalledTimes(1);
    expect(screen.getByTestId('project-rerun-running')).toBeTruthy();
  });

  it('a failed poll shows its sentence, polls no more, and leaves the controls disabled', async () => {
    let n = 0;
    const w = world({
      current: () => {
        n += 1;
        if (n > 1) throw refusal({ code: 'project.busy' });
        return currentWith(rerunView(9, 1, 'going'));
      },
    });
    const { bridge } = page(w);
    const unread = await screen.findByTestId('project-rerun-unread');
    expect(unread.textContent).toBe(PROJECT_TEXT.pageUnread);
    expect(screen.queryByTestId('project-rerun-running')).toBeNull();
    const after = calls(bridge, 'project_current').length;
    expect(after).toBe(2);
    await wait(40);
    expect(calls(bridge, 'project_current')).toHaveLength(after);
    expect(isDisabled('project-page-close')).toBe(true);
    expect(isDisabled('project-run-rerun-1')).toBe(true);
    // Until the page is read again: then what project_current says stands,
    // and an outcome it holds shows nothing.
    w.current = currentWith(rerunView(9, 1, 'saved', { storedRunId: 4 }));
    fireEvent.click(screen.getByTestId('project-rerun-retry'));
    await waitFor(() => expect(isDisabled('project-page-close')).toBe(false));
    await screen.findByTestId('project-runs-list');
    expect(screen.queryByTestId('project-rerun-unread')).toBeNull();
    expect(screen.queryByTestId('project-rerun-outcome')).toBeNull();
    expect(isDisabled('project-run-rerun-1')).toBe(false);
  });
});

/* ---- the Projects panel's Delete… (decision 199) -------------------------- */

describe("the panel's Delete… while a rerun is going", () => {
  function panel(current: ProjectCurrentView) {
    const bridge = makeMockProjectBridge({ project_current: current, project_list: LIST, project_sizes: SIZES });
    const workspace = makeMockWorkspaceBridge({ workspace_state: ACTIVE });
    render(<ProjectsPanel bridge={bridge} workspace={workspace} desktop />);
    return bridge;
  }

  it('is disabled for that project on the read that says going, with the sentence beside it, and enabled on the next read', async () => {
    panel(currentWith(rerunView(9, 1, 'going')));
    await screen.findByTestId('projects-list');
    expect(isDisabled(`project-delete-${A}`)).toBe(true);
    expect(screen.getByTestId(`project-delete-going-${A}`).textContent?.trim()).toBe(IN_PROGRESS);
    expect(isDisabled(`project-delete-${C}`)).toBe(false);
    expect(screen.queryByTestId(`project-delete-going-${C}`)).toBeNull();
    cleanup();
    panel(currentWith(rerunView(9, 1, 'saved', { storedRunId: 4 })));
    await screen.findByTestId('projects-list');
    expect(isDisabled(`project-delete-${A}`)).toBe(false);
    expect(screen.queryByTestId(`project-delete-going-${A}`)).toBeNull();
  });
});

/* ---- a rerun's words (decisions 191, 197 and 198) ------------------------- */

describe("a rerun's words, in the list and in its saved result view", () => {
  const RERUNS: StoredRunView[] = [
    ...RUNS,
    stored_(4, SUMMARY, 'succeeded', 1792396400, { rerunOf: 1, comparison: 'identical', engineVersionDiffers: false }),
    stored_(5, TTEST, 'succeeded', 1792396500, { rerunOf: 3, comparison: 'different', engineVersionDiffers: true }),
    stored_(6, SUMMARY, 'succeeded', 1792396600, { rerunOf: 1, comparison: 'not_compared', engineVersionDiffers: null }),
    stored_(7, SUMMARY, 'succeeded', 1792396700, { rerunOf: 99, comparison: 'identical', engineVersionDiffers: false }),
    stored_(8, TTEST, 'failed', 1792396800, { rerunOf: 3 }),
  ];
  const on1 = `Repeats the result saved on ${savedOn(1792396100)}.`;
  const on3 = `Repeats the result saved on ${savedOn(1792396300)}.`;
  const WANT: Record<number, string> = {
    4: `${on1} Identical, byte for byte, to that result.`,
    5: `${on3} Not identical, byte for byte, to that result. Both are kept; Gaply does not judge which is right. It ran on a different version of Gaply's engine.`,
    6: `${on1} Gaply could not compare it with that result.`,
    7: 'Repeats an earlier result. Identical, byte for byte, to that result.',
    8: on3,
  };

  it('each comparison, the engine line, a repeated run no longer listed, and a rerun that did not succeed', async () => {
    page(world({ runs: RERUNS }));
    await screen.findByTestId('project-runs-list');
    for (const [id, words] of Object.entries(WANT)) {
      const row = screen.getByTestId(`project-run-${id}`);
      expect(within(row).getByTestId(`project-run-repeats-${id}`).textContent, `row ${id}`).toBe(words);
    }
    // A run that is not a rerun says none of it.
    for (const id of [1, 2, 3, 9]) expect(screen.queryByTestId(`project-run-repeats-${id}`)).toBeNull();
    // Under its own line: after the row's facts.
    const row4 = screen.getByTestId('project-run-4');
    const facts = within(row4).getByTestId('project-run-facts-4');
    const words = within(row4).getByTestId('project-run-repeats-4');
    expect(facts.compareDocumentPosition(words) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
    expect(words.tagName).toBe('P');
  });

  it.each(Object.keys(WANT).map(Number))('the saved result view of run %i says the same, under its "Saved result" line', async (id) => {
    page(world({ runs: RERUNS }));
    await screen.findByTestId('project-runs-list');
    fireEvent.click(screen.getByTestId(`project-run-open-${id}`));
    const view = await screen.findByTestId('saved-result');
    const line = within(view).getByTestId('saved-result-line');
    const words = within(view).getByTestId('saved-result-repeats');
    expect(words.textContent).toBe(WANT[id]);
    expect(line.nextElementSibling).toBe(words);
    // Still read-only (decision 150).
    expect(within(view).queryAllByRole('button')).toHaveLength(0);
  });

  it('the saved result view of a run that is not a rerun says none of it', async () => {
    page(world({ runs: RERUNS }));
    await screen.findByTestId('project-runs-list');
    fireEvent.click(screen.getByTestId('project-run-open-1'));
    await screen.findByTestId('saved-result');
    expect(screen.queryByTestId('saved-result-repeats')).toBeNull();
  });
});

/* ---- the polish (decisions 183 and 200) ------------------------------------ */

describe("the design system's secondary button", () => {
  it('on Open this result, Run again and the panel\'s Delete…', async () => {
    page(world());
    await screen.findByTestId('project-runs-list');
    for (const id of ['project-run-open-1', 'project-run-open-2', 'project-run-rerun-1']) {
      const b = screen.getByTestId(id);
      expect(b.tagName, id).toBe('BUTTON');
      expect(b.classList.contains('gds-btn--secondary'), id).toBe(true);
      expect(b.classList.contains('gds-btn--ghost'), id).toBe(false);
    }
    cleanup();
    const bridge = makeMockProjectBridge({ project_current: currentWith(null), project_list: LIST, project_sizes: SIZES });
    render(<ProjectsPanel bridge={bridge} workspace={makeMockWorkspaceBridge({ workspace_state: ACTIVE })} desktop />);
    const del = await screen.findByTestId(`project-delete-${A}`);
    expect(del.textContent).toBe('Delete…');
    expect(del.classList.contains('gds-btn--secondary')).toBe(true);
    expect(del.classList.contains('gds-btn--ghost')).toBe(false);
  });
});
