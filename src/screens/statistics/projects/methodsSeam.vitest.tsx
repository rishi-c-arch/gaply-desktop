// What differs per method on screen, pinned before the seam (Research OS,
// Phase 8C; decision 249). Each case pins one difference of the step's list
// that no earlier test pinned:
//
// * 6, the stored-result checks: the summary's diagnostic params allow
//   strings and the keys from and event, the t-test's do not; a failed
//   t-test's case names its kind 'ttest'; only a summary sits behind the
//   summary flag's gate.
// * 7 and 13, the shell: a kind that is neither word reads, and cancels, as
//   the summary does; a switch refused for a run the state does not name
//   takes it for a summary.
// * 13, the project page: a rerun of a run whose capability no method has
//   keeps its Cancel disabled, after the runs are read too; and the
//   stored-result check answers an unknown capability before it reads the
//   data file.
//
// No capability id but registered ones is written: the unknown one is
// stats.check, which is registered and is no method a project runs.
import fs from 'node:fs';
import path from 'node:path';
import React from 'react';
import { MemoryRouter, Route, Routes } from 'react-router-dom';
import { act, cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { WorkspaceShell } from '../../workspace/WorkspaceShell';
import { makeMockWorkspaceBridge } from '../../workspace/workspaceBridge';
import type { RunKind, WorkspaceStateView } from '../../workspace/workspaceTypes';
import { makeMockDescribeBridge } from '../describeBridge';
import type { RunStateView } from '../describeTypes';
import { makeMockTTestBridge } from '../ttestBridge';
import DESCRIBE_CODES from '../describeCodes.json';
import TTEST_CODES from '../ttestCodes.json';
import { makeMockProjectBridge } from './projectBridge';
import { openProjectStore } from './projectSession';
import type { ProjectListEntryView, StoredRunView } from './projectTypes';
import { ProjectPage } from './ProjectPage';
import { SavedResult } from './SavedResult';
import { checkStored } from './storedShape';
import { useGeneration } from './useGeneration';

const flags = vi.hoisted(() => ({ on: new Set(['projects', 'workspaces', 'statsDescribe']) }));
vi.mock('../../../config/featureFlags', () => ({ isFeatureEnabled: (name: string) => flags.on.has(name) }));
vi.mock('../../../design-system/GaplyGlobe', () => ({ GaplyGlobe: () => <div /> }));

beforeEach(() => {
  flags.on = new Set(['projects', 'workspaces', 'statsDescribe']);
  openProjectStore.clear();
});
afterEach(cleanup);

const STORED = path.join(__dirname, '../../../../src-tauri/tests/fixtures/projects/stored');
const stored = (name: string) => JSON.parse(fs.readFileSync(path.join(STORED, name), 'utf8')) as Record<string, unknown>;

/** A snapshot with one diagnostic added to its stored bundle. */
function withNote(name: string, note: Record<string, unknown>): Record<string, unknown> {
  const s = stored(name);
  const bundle = JSON.parse(s.bundleJson as string) as { diagnostics: unknown[] };
  bundle.diagnostics.push(note);
  return { ...s, bundleJson: JSON.stringify(bundle) };
}

const DESC_CODE = (DESCRIBE_CODES as string[]).find((c) => c.startsWith('stats.desc.')) as string;
const T_CODE = (TTEST_CODES as string[]).find((c) => c.startsWith('stats.ttest.')) as string;

/* ---- 6: the stored-result checks ----------------------------------------------- */

describe('difference 6: each kind keeps its own stored-result rules', () => {
  it("the summary's params allow strings and from and event; the t-test's allow neither", () => {
    const note = (code: string, params: Record<string, unknown>) => ({ code, params, severity: 'note' });
    expect(checkStored(withNote('summary.json', note(DESC_CODE, { from: 'x', event: 'y' }))).kind).toBe('summary');
    expect(checkStored(withNote('summary.json', note(DESC_CODE, { column: 'C1' }))).kind).toBe('summary');
    expect(checkStored(withNote('ttest.json', note(T_CODE, { column: 1 }))).kind).toBe('ttest');
    expect(checkStored(withNote('ttest.json', note(T_CODE, { column: 'C1' }))).kind).toBe('badShape');
    expect(checkStored(withNote('ttest.json', note(T_CODE, { from: 1 }))).kind).toBe('badShape');
    expect(checkStored(withNote('ttest.json', note(T_CODE, { event: 1 }))).kind).toBe('badShape');
  });

  it("a failed t-test's case names its kind 'ttest'", () => {
    const failed = checkStored(stored('ttest_failed.json'));
    expect(failed.kind === 'failed' && failed.capability).toBe('ttest');
  });

  it("with the summary's flag off, a saved t-test is still read and shown", async () => {
    flags.on.delete('statsDescribe');
    const bridge = makeMockProjectBridge({ project_run_result: stored('ttest.json') });
    const run: StoredRunView = {
      storedRunId: 3,
      capabilityId: 'stats.analyze.independent_t',
      capabilityVersion: 1,
      state: 'succeeded',
      endCode: null,
      startedAt: 1,
      endedAt: 2,
      savedAt: 1792396300,
      agentVersion: '0.1.0',
      rerunOf: null,
      comparison: null,
      engineVersionDiffers: null,
    };
    const Host: React.FC = () => <SavedResult projectId={A} run={run} bridge={bridge} gen={useGeneration()} />;
    render(<Host />);
    await screen.findByTestId('tt-result-card');
    expect(screen.queryByTestId('saved-result-hidden')).toBeNull();
    expect(bridge.calls.filter(([c]) => c === 'project_run_result')).toHaveLength(1);
  });
});

/* ---- 7 and 13: the shell ------------------------------------------------------ */

const runView = (state: RunStateView['state'], terminal: boolean): RunStateView => ({
  runId: 4,
  state,
  terminal,
  columnsDone: 2,
  columnsTotal: 3,
  elapsedMs: 120,
  cancelRequested: false,
  diagnostics: [],
});

function renderShell(ws: ReturnType<typeof makeMockWorkspaceBridge>, runs: ReturnType<typeof makeMockDescribeBridge>, ttestRuns = makeMockTTestBridge({})) {
  render(
    <MemoryRouter initialEntries={['/app/workspaces/statistics']}>
      <Routes>
        <Route path="/app/workspaces/statistics" element={<WorkspaceShell bridge={ws} runs={runs} ttestRuns={ttestRuns} pollMs={20} />} />
        <Route path="/app/workspaces" element={<div data-testid="chooser-here" />} />
      </Routes>
    </MemoryRouter>
  );
}

describe('differences 7 and 13: the shell reads a kind it does not know as the summary', () => {
  it('reads it through the summary\'s status, with the summary\'s sentence', async () => {
    const other = 'other' as unknown as RunKind;
    const ws = makeMockWorkspaceBridge({
      workspace_state: { state: { phase: 'active', workspace: 'statistics' }, activeRun: { runId: 4, kind: other }, latestRun: { runId: 4, kind: other } },
    });
    const runs = makeMockDescribeBridge({ stats_desc_status: runView('running', false) });
    const ttestRuns = makeMockTTestBridge({});
    renderShell(ws, runs, ttestRuns);
    await waitFor(() => expect(screen.getByTestId('ws-job').textContent).toBe('Run 4: Running. 2 of 3 columns summarised.'));
    expect(ttestRuns.calls).toEqual([]);
  });

  it('a switch refused for a run the state does not name reads and cancels it as a summary', async () => {
    let ended = false;
    const refused = Object.assign(
      new Error(JSON.stringify({ code: 'workspace.run_active', params: { workspace: 'statistics', run: 4 }, severity: 'error' })),
      { code: 'validation' }
    );
    const state: WorkspaceStateView = { state: { phase: 'active', workspace: 'statistics' }, activeRun: null, latestRun: null };
    const ws = makeMockWorkspaceBridge({
      workspace_state: state,
      workspace_deactivate: () => {
        if (!ended) throw refused;
        return { state: { phase: 'no_workspace' }, activeRun: null, latestRun: null };
      },
    });
    const runs = makeMockDescribeBridge({
      stats_desc_status: () => (ended ? runView('cancelled', true) : runView('running', false)),
      stats_desc_cancel: () => {
        ended = true;
        return runView('running', false);
      },
    });
    const ttestRuns = makeMockTTestBridge({});
    renderShell(ws, runs, ttestRuns);
    await waitFor(() => expect((screen.getByTestId('ws-switch') as HTMLButtonElement).disabled).toBe(false));
    fireEvent.click(screen.getByTestId('ws-switch'));
    await waitFor(() => expect(screen.getByTestId('ws-switch-run').textContent).toBe('Run 4: Running. 2 of 3 columns summarised.'));
    fireEvent.click(screen.getByTestId('ws-switch-cancel'));
    await screen.findByTestId('chooser-here');
    expect(runs.calls.filter(([c]) => c === 'stats_desc_cancel')).toEqual([['stats_desc_cancel', { runId: 4 }]]);
    expect(ttestRuns.calls).toEqual([]);
  });
});

/* ---- 13: the project page and the stored-result check -------------------------- */

const A = '0123456789abcdef0123456789abcdef';
const LIST: ProjectListEntryView[] = [{ projectId: A, name: 'Chapter 3', bytes: 40000, savedAt: 1792396351, runCount: 1, refused: null }];
const SIZES = { totalBytes: 40000, projectCount: 1, projectCapBytes: 104857600, allProjectsCapBytes: 10737418240 };
const UNKNOWN: StoredRunView = {
  storedRunId: 5,
  capabilityId: 'stats.check',
  capabilityVersion: 1,
  state: 'succeeded',
  endCode: null,
  startedAt: 1792396090,
  endedAt: 1792396091,
  savedAt: 1792396100,
  agentVersion: '0.1.0',
  rerunOf: null,
  comparison: null,
  engineVersionDiffers: null,
};

describe('difference 13: an unknown capability on the project page', () => {
  it("a going rerun of it keeps Cancel disabled, before and after the runs are read", async () => {
    let resolveRuns!: (v: StoredRunView[]) => void;
    const runsRead = new Promise<StoredRunView[]>((r) => {
      resolveRuns = r;
    });
    const bridge = makeMockProjectBridge({
      project_current: { projectId: A, rerun: { runId: 9, rerunOf: 5, status: 'going', storedRunId: null, code: null } },
      project_list: LIST,
      project_sizes: SIZES,
      project_runs: () => runsRead,
      project_close: { closed: true },
    });
    const describe_ = makeMockDescribeBridge({ stats_desc_cancel: { runId: 0 } });
    const ttest = makeMockTTestBridge({ stats_ttest_cancel: { runId: 0 } });
    render(
      <ProjectPage
        projectId={A}
        bridge={bridge}
        workspace={makeMockWorkspaceBridge({ workspace_state: { state: { phase: 'active', workspace: 'statistics' }, activeRun: null, latestRun: null } })}
        describe={describe_}
        ttest={ttest}
        desktop
        save={async () => '/chosen/place'}
        pollMs={5}
        onClosed={vi.fn()}
        onBack={vi.fn()}
      />
    );
    await screen.findByTestId('project-rerun-running');
    const disabled = () => (screen.getByTestId('project-rerun-cancel') as HTMLButtonElement).disabled;
    expect(disabled()).toBe(true);
    await act(async () => resolveRuns([UNKNOWN]));
    await screen.findByTestId('project-runs-list');
    await act(async () => new Promise((r) => setTimeout(r, 30)));
    expect(disabled()).toBe(true);
    fireEvent.click(screen.getByTestId('project-rerun-cancel'));
    expect(describe_.calls.filter(([c]) => c === 'stats_desc_cancel')).toEqual([]);
    expect(ttest.calls.filter(([c]) => c === 'stats_ttest_cancel')).toEqual([]);
  });

  it('the stored-result check names it before it reads the data file', () => {
    expect(checkStored({ ...stored('summary.json'), capabilityId: 'stats.check', dataFile: null }).kind).toBe('unknownCapability');
    expect(checkStored({ ...stored('summary.json'), dataFile: null }).kind).toBe('badShape');
  });
});
