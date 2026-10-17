// Where the workspace screens are reached (Research OS, Phase 3): with
// REACT_APP_FEATURE_WORKSPACES off, the chooser, the shell and the hub tile
// are hidden; with it on, they appear, and with the describe flag on too the
// descriptive summary moves from the hub into the Statistics workspace. The
// gate in front of the describe screen shows it only while Rust says
// Statistics is active, and opens Statistics only on the user's click.
import React from 'react';
import { MemoryRouter, Route, Routes } from 'react-router-dom';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { CHOOSER_PATH, STATISTICS_PATH, workspaceRoutes } from './workspaceRoute';
import { RequireWorkspace } from './RequireWorkspace';
import { makeMockWorkspaceBridge } from './workspaceBridge';
import AppHomeHero from '../home/AppHomeHero';
import { ThemeProvider } from '../../contexts/ThemeContext';
import type { WorkspaceStateView } from './workspaceTypes';

const flags = vi.hoisted(() => ({ workspaces: false, describe: false }));
vi.mock('../../config/featureFlags', () => ({
  isFeatureEnabled: (name: string) =>
    (name === 'workspaces' && flags.workspaces) || (name === 'statsDescribe' && flags.describe),
}));
vi.mock('../../design-system/GaplyGlobe', () => ({ GaplyGlobe: () => <div /> }));
vi.mock('../home/HeroAnimation', () => ({ default: () => <div /> }));

afterEach(cleanup);

function routeTo(path: string) {
  render(
    <MemoryRouter initialEntries={[path]}>
      <Routes>
        {workspaceRoutes(<div data-testid="chooser-here" />, <div data-testid="shell-here" />)}
        <Route path="*" element={<div data-testid="no-route" />} />
      </Routes>
    </MemoryRouter>
  );
}

function hub() {
  render(
    <MemoryRouter>
      <ThemeProvider>
        <AppHomeHero />
      </ThemeProvider>
    </MemoryRouter>
  );
}

describe('REACT_APP_FEATURE_WORKSPACES', () => {
  it('off: the chooser, the shell and the hub tile are hidden', () => {
    flags.workspaces = false;
    flags.describe = false;
    for (const path of [CHOOSER_PATH, STATISTICS_PATH]) {
      routeTo(path);
      expect(screen.queryByTestId('chooser-here')).toBeNull();
      expect(screen.queryByTestId('shell-here')).toBeNull();
      expect(screen.getByTestId('no-route')).toBeTruthy();
      cleanup();
    }
    hub();
    expect(screen.queryByText('Choose your workspace')).toBeNull();
    expect(screen.getAllByText(/Statistical Analysis Verifier/).length).toBeGreaterThan(0);
  });

  it('on: the chooser and the shell appear, and the hub links to the chooser', () => {
    flags.workspaces = true;
    flags.describe = false;
    expect(CHOOSER_PATH).toBe('/app/workspaces');
    routeTo(CHOOSER_PATH);
    expect(screen.getByTestId('chooser-here')).toBeTruthy();
    cleanup();
    routeTo(STATISTICS_PATH);
    expect(screen.getByTestId('shell-here')).toBeTruthy();
    cleanup();
    hub();
    const tiles = screen.getAllByText('Choose your workspace');
    expect(tiles[0].closest('a')?.getAttribute('href')).toBe(CHOOSER_PATH);
    // The Stats Check and the Stats Verifier stay reachable from the hub.
    expect(screen.getAllByText(/Statistical Analysis Verifier/).length).toBeGreaterThan(0);
  });

  it('with the describe flag on too, the descriptive summary leaves the hub for the workspace', () => {
    flags.workspaces = true;
    flags.describe = true;
    hub();
    expect(screen.queryByText('Descriptive Summary')).toBeNull();
    cleanup();
    flags.workspaces = false;
    hub();
    expect(screen.getAllByText('Descriptive Summary').length).toBeGreaterThan(0);
  });
});

describe('RequireWorkspace', () => {
  const NONE: WorkspaceStateView = { state: { phase: 'no_workspace' }, activeRun: null, latestRun: null };
  const ACTIVE: WorkspaceStateView = { state: { phase: 'active', workspace: 'statistics' }, activeRun: null, latestRun: null };

  function gate(bridge: ReturnType<typeof makeMockWorkspaceBridge>) {
    render(
      <MemoryRouter>
        <RequireWorkspace id="statistics" bridge={bridge}>
          <div data-testid="describe-here" />
        </RequireWorkspace>
      </MemoryRouter>
    );
  }

  it('shows the screen only while Rust says Statistics is active', async () => {
    gate(makeMockWorkspaceBridge({ workspace_state: ACTIVE }));
    expect(await screen.findByTestId('describe-here')).toBeTruthy();
  });

  it('otherwise says so, and activates only on the click, once', async () => {
    flags.workspaces = false;
    const bridge = makeMockWorkspaceBridge({ workspace_state: NONE, workspace_activate: ACTIVE });
    gate(bridge);
    const open = await screen.findByTestId('ws-gate-open');
    expect(screen.queryByTestId('describe-here')).toBeNull();
    expect(bridge.calls.map(([c]) => c)).toEqual(['workspace_state']);
    fireEvent.click(open);
    expect(await screen.findByTestId('describe-here')).toBeTruthy();
    await waitFor(() =>
      expect(bridge.calls.filter(([c]) => c === 'workspace_activate')).toEqual([['workspace_activate', { id: 'statistics' }]])
    );
  });
});
