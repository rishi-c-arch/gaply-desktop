// Opening the Stats Check or the Stats Verifier opens the Statistics
// workspace (Research OS, Phase 4A, decision 49): one workspace_activate when
// Statistics is not active, none when it is, none in the browser build, one
// under StrictMode's double effect; the screen's body only once Statistics
// is Active; a refusal's sentence from the wording table. The two routes are
// rendered through GaplyScreens, with the session, the auth gate and the two
// pages stubbed and the Tauri core mocked.
import React from 'react';
import { MemoryRouter } from 'react-router-dom';
import { act, cleanup, render, screen, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { EnterStatistics } from './EnterStatistics';
import { makeMockWorkspaceBridge } from './workspaceBridge';
import type { WorkspaceStateView } from './workspaceTypes';
import { ENTER_TEXT } from './workspaceWording';

const invoke = vi.hoisted(() => vi.fn());
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...a: unknown[]) => invoke(...a) }));
vi.mock('../../utils/isTauri', () => ({ isTauri: true, default: true }));
vi.mock('../session/SessionProvider', () => ({
  GaplySessionProvider: ({ children }: { children: React.ReactNode }) => <>{children}</>,
  RequireSession: ({ children }: { children: React.ReactNode }) => <>{children}</>,
}));
vi.mock('../session/RequireAuth', () => ({ default: ({ children }: { children: React.ReactNode }) => <>{children}</> }));
vi.mock('../checks/StatsCheckPage', () => ({ default: () => <div data-testid="stats-check-body" /> }));
vi.mock('../statsverifier/StatsVerifierPage', () => ({ default: () => <div data-testid="stats-verifier-body" /> }));

afterEach(() => {
  cleanup();
  invoke.mockReset();
});

const none: WorkspaceStateView = { state: { phase: 'no_workspace' }, activeRun: null, latestRun: null };
const active: WorkspaceStateView = { state: { phase: 'active', workspace: 'statistics' }, activeRun: null, latestRun: null };

const Body: React.FC = () => <div data-testid="body" />;

function calls(b: { calls: Array<[string, Record<string, unknown>]> }, cmd: string) {
  return b.calls.filter(([c]) => c === cmd);
}

describe('EnterStatistics', () => {
  it('activates Statistics when it is not active, then shows the screen', async () => {
    const b = makeMockWorkspaceBridge({ workspace_state: none, workspace_activate: active });
    render(
      <EnterStatistics bridge={b} desktop>
        <Body />
      </EnterStatistics>
    );
    expect(await screen.findByTestId('body')).toBeTruthy();
    expect(calls(b, 'workspace_activate')).toEqual([['workspace_activate', { id: 'statistics' }]]);
  });

  it('makes no workspace_activate call when Statistics is already active', async () => {
    const b = makeMockWorkspaceBridge({ workspace_state: active, workspace_activate: active });
    render(
      <EnterStatistics bridge={b} desktop>
        <Body />
      </EnterStatistics>
    );
    expect(await screen.findByTestId('body')).toBeTruthy();
    expect(calls(b, 'workspace_state')).toHaveLength(1);
    expect(calls(b, 'workspace_activate')).toHaveLength(0);
  });

  it('makes no call at all in the browser build, and shows the screen at once', () => {
    const b = makeMockWorkspaceBridge({});
    render(
      <EnterStatistics bridge={b} desktop={false}>
        <Body />
      </EnterStatistics>
    );
    expect(screen.getByTestId('body')).toBeTruthy();
    expect(b.calls).toEqual([]);
  });

  it('makes one activation under StrictMode', async () => {
    const b = makeMockWorkspaceBridge({ workspace_state: none, workspace_activate: active });
    render(
      <React.StrictMode>
        <EnterStatistics bridge={b} desktop>
          <Body />
        </EnterStatistics>
      </React.StrictMode>
    );
    expect(await screen.findByTestId('body')).toBeTruthy();
    expect(calls(b, 'workspace_activate')).toHaveLength(1);
    expect(calls(b, 'workspace_state')).toHaveLength(1);
  });

  it('hides the body until Statistics is Active', async () => {
    let finish: (v: WorkspaceStateView) => void = () => {};
    const pending = new Promise<WorkspaceStateView>((resolve) => {
      finish = resolve;
    });
    const b = makeMockWorkspaceBridge({ workspace_state: none });
    b.activate = () => {
      b.calls.push(['workspace_activate', { id: 'statistics' }]);
      return pending;
    };
    render(
      <EnterStatistics bridge={b} desktop>
        <Body />
      </EnterStatistics>
    );
    await waitFor(() => expect(calls(b, 'workspace_activate')).toHaveLength(1));
    expect(screen.queryByTestId('body')).toBeNull();
    expect(screen.getByTestId('ws-enter-message').textContent).toBe(ENTER_TEXT.opening);
    await act(async () => finish(active));
    expect(await screen.findByTestId('body')).toBeTruthy();
  });

  it('shows the refusal sentence and never the body when activation is refused', async () => {
    const refusal = {
      code: 'workspace.illegal_transition',
      params: { from: 'active', event: 'activate', workspace: 'compute' },
      severity: 'error',
    };
    const b = makeMockWorkspaceBridge({
      workspace_state: none,
      workspace_activate: () => {
        throw { code: 'validation', message: JSON.stringify(refusal) };
      },
    });
    render(
      <EnterStatistics bridge={b} desktop>
        <Body />
      </EnterStatistics>
    );
    await waitFor(() =>
      expect(screen.getByTestId('ws-enter-message').textContent).toBe('Compute is open. Close it first.')
    );
    expect(screen.queryByTestId('body')).toBeNull();
  });
});

describe('the Stats Check and Stats Verifier routes', () => {
  async function open(route: 'StatsCheckRoute' | 'StatsVerifierRoute', testid: string) {
    const seen: string[] = [];
    let phase: 'none' | 'active' = 'none';
    invoke.mockImplementation(async (cmd: string) => {
      seen.push(cmd);
      if (cmd === 'workspace_state') return phase === 'active' ? active : none;
      if (cmd === 'workspace_activate') {
        phase = 'active';
        return active;
      }
      throw new Error(`unexpected command ${cmd}`);
    });
    const screens = await import('../GaplyScreens');
    const Route = screens[route];
    render(
      <MemoryRouter>
        <Route />
      </MemoryRouter>
    );
    expect(await screen.findByTestId(testid)).toBeTruthy();
    return seen;
  }

  it('opening the Stats Check opens Statistics with no extra click', async () => {
    expect(await open('StatsCheckRoute', 'stats-check-body')).toEqual(['workspace_state', 'workspace_activate']);
  });

  it('opening the Stats Verifier opens Statistics', async () => {
    expect(await open('StatsVerifierRoute', 'stats-verifier-body')).toEqual(['workspace_state', 'workspace_activate']);
  });
});
