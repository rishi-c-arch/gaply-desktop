// Where the Statistics method selector and the t-test are reached (Research
// OS, Phase 4B-2, decision 77), under each combination of
// REACT_APP_FEATURE_STATS_DESCRIBE and REACT_APP_FEATURE_WORKSPACES:
//
// * describe off: the selector, the t-test and the describe flow are all
//   hidden, and nothing links to them;
// * describe on, workspaces off: the hub's tile opens the selector;
// * both on: the selector leaves the hub and the Statistics workspace's tools
//   open it instead.
//
// The t-test's route is behind RequireWorkspace id="statistics", with the
// explicit button the describe screen has, and its own sentence.
import fs from 'node:fs';
import path from 'node:path';
import React from 'react';
import { MemoryRouter, Route, Routes } from 'react-router-dom';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { STATISTICS_HOME_PATH, statisticsRoutes, statisticsTiles, TTEST_PATH } from './statisticsRoute';
import { DESCRIBE_PATH, describeRoute } from './describeRoute';
import AppHomeHero from '../home/AppHomeHero';
import { ThemeProvider } from '../../contexts/ThemeContext';
import { WorkspaceShell } from '../workspace/WorkspaceShell';
import { RequireWorkspace } from '../workspace/RequireWorkspace';
import { makeMockWorkspaceBridge } from '../workspace/workspaceBridge';
import { makeMockDescribeBridge } from './describeBridge';
import type { WorkspaceStateView } from '../workspace/workspaceTypes';

const flags = vi.hoisted(() => ({ describe: false, workspaces: false }));
vi.mock('../../config/featureFlags', () => ({
  isFeatureEnabled: (name: string) => (name === 'statsDescribe' && flags.describe) || (name === 'workspaces' && flags.workspaces),
}));
vi.mock('../../design-system/GaplyGlobe', () => ({ GaplyGlobe: () => <div /> }));
vi.mock('../home/HeroAnimation', () => ({ default: () => <div /> }));

afterEach(cleanup);

const ACTIVE: WorkspaceStateView = { state: { phase: 'active', workspace: 'statistics' }, activeRun: null, latestRun: null };
const NONE: WorkspaceStateView = { state: { phase: 'no_workspace' }, activeRun: null, latestRun: null };

function routeTo(p: string) {
  render(
    <MemoryRouter initialEntries={[p]}>
      <Routes>
        {statisticsRoutes(<div data-testid="home-here" />, <div data-testid="ttest-here" />)}
        {describeRoute(<div data-testid="describe-here" />)}
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

function shellTools() {
  render(
    <MemoryRouter initialEntries={['/app/workspaces/statistics']}>
      <WorkspaceShell bridge={makeMockWorkspaceBridge({ workspace_state: ACTIVE })} runs={makeMockDescribeBridge({})} pollMs={20} />
    </MemoryRouter>
  );
  return screen.findByTestId('ws-tools');
}

describe('the method selector and the t-test, under each flag', () => {
  it('describe flag off: the selector and both flows are hidden, and nothing links to them', async () => {
    flags.describe = false;
    flags.workspaces = false;
    for (const p of [STATISTICS_HOME_PATH, TTEST_PATH, DESCRIBE_PATH]) {
      routeTo(p);
      expect(screen.getByTestId('no-route'), p).toBeTruthy();
      cleanup();
    }
    expect(statisticsTiles()).toEqual([]);
    hub();
    expect(screen.queryByText('Statistical Methods')).toBeNull();
    cleanup();
    flags.workspaces = true;
    const tools = await shellTools();
    expect(tools.textContent).not.toContain('Statistical Methods');
    expect(tools.textContent).not.toContain('Descriptive Summary');
  });

  it('describe flag on, workspaces off: the routes appear and the hub tile opens the selector', () => {
    flags.describe = true;
    flags.workspaces = false;
    expect(STATISTICS_HOME_PATH).toBe('/app/statistics');
    expect(TTEST_PATH).toBe('/app/statistics/ttest');
    for (const [p, id] of [
      [STATISTICS_HOME_PATH, 'home-here'],
      [TTEST_PATH, 'ttest-here'],
      [DESCRIBE_PATH, 'describe-here'],
    ]) {
      routeTo(p);
      expect(screen.getByTestId(id), p).toBeTruthy();
      cleanup();
    }
    hub();
    const tiles = screen.getAllByText('Statistical Methods');
    expect(tiles[0].closest('a')?.getAttribute('href')).toBe(STATISTICS_HOME_PATH);
    expect(screen.queryByText('Descriptive Summary')).toBeNull();
  });

  it('both on: the selector leaves the hub, and the Statistics workspace opens it', async () => {
    flags.describe = true;
    flags.workspaces = true;
    hub();
    expect(screen.queryByText('Statistical Methods')).toBeNull();
    expect(screen.queryByText('Descriptive Summary')).toBeNull();
    cleanup();
    const tools = await shellTools();
    const link = await screen.findByText('Statistical Methods');
    expect(link.closest('a')?.getAttribute('href')).toBe(STATISTICS_HOME_PATH);
    expect(tools.textContent).not.toContain('Descriptive Summary');
  });
});

describe("the t-test's route", () => {
  it('is wrapped in RequireWorkspace id="statistics", as the describe route is', () => {
    const src = fs.readFileSync(path.join(__dirname, '../GaplyScreens.tsx'), 'utf8');
    const at = src.indexOf('export const TTestRoute');
    expect(at).toBeGreaterThan(0);
    const body = src.slice(at, src.indexOf(');', at));
    expect(body).toMatch(/<RequireWorkspace id="statistics"[^>]*>\s*<TTestPage \/>\s*<\/RequireWorkspace>/);
  });

  it("shows the gate's explicit button and its own sentence, and opens Statistics only on the click", async () => {
    flags.workspaces = false;
    const bridge = makeMockWorkspaceBridge({ workspace_state: NONE, workspace_activate: ACTIVE });
    render(
      <MemoryRouter>
        <RequireWorkspace id="statistics" bridge={bridge} body="Open the Statistics workspace to use the t-test.">
          <div data-testid="ttest-here" />
        </RequireWorkspace>
      </MemoryRouter>
    );
    const open = await screen.findByTestId('ws-gate-open');
    expect(screen.getByText('Open the Statistics workspace to use the t-test.')).toBeTruthy();
    expect(screen.queryByTestId('ttest-here')).toBeNull();
    expect(bridge.calls.map(([c]) => c)).toEqual(['workspace_state']);
    fireEvent.click(open);
    expect(await screen.findByTestId('ttest-here')).toBeTruthy();
    await waitFor(() => expect(bridge.calls.filter(([c]) => c === 'workspace_activate')).toHaveLength(1));
  });
});
