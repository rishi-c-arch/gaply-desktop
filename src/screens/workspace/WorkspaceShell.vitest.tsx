// The slim Statistics shell (Research OS, Phase 3): the job area reads Phase
// 2B's run status; switching while a run is queued or running shows that run
// with Wait and Cancel, and nothing is abandoned: the switch happens only
// after Rust lets the workspace go.
import React from 'react';
import { MemoryRouter, Route, Routes } from 'react-router-dom';
import { act, cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { WorkspaceShell } from './WorkspaceShell';
import { makeMockWorkspaceBridge } from './workspaceBridge';
import { makeMockDescribeBridge } from '../statistics/describeBridge';
import type { WorkspaceStateView } from './workspaceTypes';
import type { RunStateView } from '../statistics/describeTypes';

vi.mock('../../design-system/GaplyGlobe', () => ({ GaplyGlobe: () => <div /> }));

afterEach(cleanup);

const ACTIVE = (run: number | null): WorkspaceStateView => ({
  state: { phase: 'active', workspace: 'statistics' },
  activeRun: run,
  latestRun: run,
});
const NONE: WorkspaceStateView = { state: { phase: 'no_workspace' }, activeRun: null, latestRun: null };

const runView = (state: RunStateView['state'], terminal: boolean, done = 1): RunStateView => ({
  runId: 4,
  state,
  terminal,
  columnsDone: done,
  columnsTotal: 3,
  elapsedMs: 120,
  cancelRequested: false,
  diagnostics: [],
});

const RUN_ACTIVE = Object.assign(
  new Error(JSON.stringify({ code: 'workspace.run_active', params: { workspace: 'statistics', run: 4 }, severity: 'error' })),
  { code: 'validation' }
);

function renderShell(ws: ReturnType<typeof makeMockWorkspaceBridge>, runs: ReturnType<typeof makeMockDescribeBridge>) {
  render(
    <MemoryRouter initialEntries={['/app/workspaces/statistics']}>
      <Routes>
        <Route path="/app/workspaces/statistics" element={<WorkspaceShell bridge={ws} runs={runs} pollMs={20} />} />
        <Route path="/app/workspaces" element={<div data-testid="chooser-here" />} />
      </Routes>
    </MemoryRouter>
  );
}

describe('WorkspaceShell', () => {
  it('shows the tools, empty recent items, settings and the newest run in the job area', async () => {
    const ws = makeMockWorkspaceBridge({ workspace_state: ACTIVE(4) });
    const runs = makeMockDescribeBridge({ stats_desc_status: runView('running', false, 2) });
    renderShell(ws, runs);
    await waitFor(() => expect(screen.getByTestId('ws-job').textContent).toBe('Run 4: Running. 2 of 3 columns summarised.'));
    expect(screen.getByTestId('ws-recent-empty')).toBeTruthy();
    expect(screen.getByTestId('ws-tools').textContent).toContain('Statistical Analysis Verifier');
    expect(screen.getAllByText('Settings').length).toBeGreaterThan(0);
    expect(runs.calls.every(([c]) => c === 'stats_desc_status')).toBe(true);
  });

  it('switching with no run asks Rust once and goes to the chooser', async () => {
    const ws = makeMockWorkspaceBridge({ workspace_state: ACTIVE(null), workspace_deactivate: NONE });
    renderShell(ws, makeMockDescribeBridge({}));
    await waitFor(() => expect((screen.getByTestId('ws-switch') as HTMLButtonElement).disabled).toBe(false));
    fireEvent.click(screen.getByTestId('ws-switch'));
    await screen.findByTestId('chooser-here');
    expect(ws.calls.filter(([c]) => c === 'workspace_deactivate')).toHaveLength(1);
    // ADR-001 decision 7 as corrected (evaluation of Phases 2 and 3, F13):
    // the shell never activates a workspace; the chooser does, on a click.
    expect(ws.calls.some(([c]) => c === 'workspace_activate')).toBe(false);
  });

  it('switching during a run shows the run with Wait and Cancel; Cancel cancels it, then switches', async () => {
    let ended = false;
    let deactivations = 0;
    const ws = makeMockWorkspaceBridge({
      workspace_state: () => ACTIVE(ended ? null : 4),
      workspace_deactivate: () => {
        deactivations += 1;
        if (!ended) throw RUN_ACTIVE;
        return NONE;
      },
    });
    const runs = makeMockDescribeBridge({
      stats_desc_status: () => (ended ? runView('cancelled', true) : runView('running', false)),
      stats_desc_cancel: () => {
        ended = true;
        return runView('running', false);
      },
    });
    renderShell(ws, runs);
    await waitFor(() => expect((screen.getByTestId('ws-switch') as HTMLButtonElement).disabled).toBe(false));
    fireEvent.click(screen.getByTestId('ws-switch'));

    const dialog = await screen.findByTestId('ws-switch-dialog');
    expect(dialog.closest('[role="dialog"]')).not.toBeNull();
    expect(screen.getByTestId('ws-switch-wait').textContent).toBe('Wait, then switch');
    expect(screen.getByTestId('ws-switch-cancel').textContent).toBe('Cancel the run, then switch');
    await waitFor(() => expect(screen.getByTestId('ws-switch-run').textContent).toBe('Run 4: Running. 1 of 3 columns summarised.'));
    expect(screen.queryByTestId('chooser-here')).toBeNull();
    expect(deactivations).toBe(1);

    fireEvent.click(screen.getByTestId('ws-switch-cancel'));
    await screen.findByTestId('chooser-here');
    expect(runs.calls.filter(([c]) => c === 'stats_desc_cancel')).toEqual([['stats_desc_cancel', { runId: 4 }]]);
    expect(deactivations).toBe(2);
  });

  it('Wait leaves the run alone and switches once it has ended', async () => {
    let ended = false;
    const ws = makeMockWorkspaceBridge({
      workspace_state: () => ACTIVE(ended ? null : 4),
      workspace_deactivate: () => {
        if (!ended) throw RUN_ACTIVE;
        return NONE;
      },
    });
    const runs = makeMockDescribeBridge({
      stats_desc_status: () => (ended ? runView('succeeded', true, 3) : runView('running', false)),
    });
    renderShell(ws, runs);
    await waitFor(() => expect((screen.getByTestId('ws-switch') as HTMLButtonElement).disabled).toBe(false));
    fireEvent.click(screen.getByTestId('ws-switch'));
    fireEvent.click(await screen.findByTestId('ws-switch-wait'));
    expect(await screen.findByTestId('ws-switch-waiting')).toBeTruthy();
    await new Promise((r) => setTimeout(r, 80));
    expect(screen.queryByTestId('chooser-here')).toBeNull();
    await act(async () => {
      ended = true;
    });
    await screen.findByTestId('chooser-here');
    expect(runs.calls.some(([c]) => c === 'stats_desc_cancel')).toBe(false);
  });

  it('Stay closes the dialog and keeps the workspace', async () => {
    const ws = makeMockWorkspaceBridge({
      workspace_state: ACTIVE(4),
      workspace_deactivate: () => {
        throw RUN_ACTIVE;
      },
    });
    renderShell(ws, makeMockDescribeBridge({ stats_desc_status: runView('running', false) }));
    await waitFor(() => expect((screen.getByTestId('ws-switch') as HTMLButtonElement).disabled).toBe(false));
    fireEvent.click(screen.getByTestId('ws-switch'));
    fireEvent.click(await screen.findByTestId('ws-switch-stay'));
    await waitFor(() => expect(screen.queryByTestId('ws-switch-dialog')).toBeNull());
    expect(screen.queryByTestId('chooser-here')).toBeNull();
  });

  it('reads the state and does not decide it: with Statistics not open, it says so', async () => {
    const ws = makeMockWorkspaceBridge({ workspace_state: NONE });
    renderShell(ws, makeMockDescribeBridge({}));
    expect(await screen.findByText('Statistics is not open in this window.')).toBeTruthy();
    expect((screen.getByTestId('ws-switch') as HTMLButtonElement).disabled).toBe(true);
    expect(ws.calls.every(([c]) => c === 'workspace_state')).toBe(true);
  });
});
