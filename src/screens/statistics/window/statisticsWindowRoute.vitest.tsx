// Where the Statistics window is reached (Research OS, Phase 8P, decision
// 449), under each combination of REACT_APP_FEATURE_STATS_LAYOUT and
// REACT_APP_FEATURE_STATS_DESCRIBE: its route and its one link exist only with
// both on, and every older route is the same either way. The flag hides
// unfinished UI: the gate and Rust are what stand (decision 454).
import fs from 'node:fs';
import path from 'node:path';
import React from 'react';
import { MemoryRouter, Route, Routes } from 'react-router-dom';
import { cleanup, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import {
  ALPHA_PATH,
  ANOVA_PATH,
  CORR_PATH,
  ONE_SAMPLE_PATH,
  PAIRED_PATH,
  STATISTICS_HOME_PATH,
  STATISTICS_WINDOW_PATH,
  statisticsRoutes,
  statisticsTools,
  statisticsWindowRoute,
  TTEST_PATH,
} from '../statisticsRoute';
import { RequireWorkspace } from '../../workspace/RequireWorkspace';
import { WorkspaceShell } from '../../workspace/WorkspaceShell';
import { makeMockWorkspaceBridge } from '../../workspace/workspaceBridge';
import type { WorkspaceStateView } from '../../workspace/workspaceTypes';
import { makeMockDescribeBridge } from '../describeBridge';
import { StatisticsWindow } from './StatisticsWindow';

const flags = vi.hoisted(() => ({ describe: false, layout: false, workspaces: false }));
vi.mock('../../../config/featureFlags', () => ({
  isFeatureEnabled: (name: string) =>
    (name === 'statsDescribe' && flags.describe) || (name === 'statsLayout' && flags.layout) || (name === 'workspaces' && flags.workspaces),
}));
vi.mock('../../../design-system/GaplyGlobe', () => ({ GaplyGlobe: () => <div /> }));

afterEach(cleanup);

const ACTIVE: WorkspaceStateView = { state: { phase: 'active', workspace: 'statistics' }, activeRun: null, latestRun: null };
const NONE: WorkspaceStateView = { state: { phase: 'no_workspace' }, activeRun: null, latestRun: null };
const WINDOW_TOOL = { label: 'Statistics window (in development)', href: '/app/statistics/window' };

function routeTo(p: string) {
  render(
    <MemoryRouter initialEntries={[p]}>
      <Routes>
        {statisticsRoutes(
          <div data-testid="home-here" />,
          <div data-testid="old-page" />,
          <div data-testid="old-page" />,
          <div data-testid="old-page" />,
          <div data-testid="old-page" />,
          <div data-testid="old-page" />,
          <div data-testid="old-page" />
        )}
        {statisticsWindowRoute(<div data-testid="window-here" />)}
        <Route path="*" element={<div data-testid="no-route" />} />
      </Routes>
    </MemoryRouter>
  );
}

describe("the Statistics window's route and its way in, under each pair of flags", () => {
  const COMBINATIONS: Array<[boolean, boolean, boolean]> = [
    [false, false, false],
    [false, true, false],
    [true, false, false],
    [true, true, true],
  ];

  it.each(COMBINATIONS)('describe %s, layout %s: the route exists only with both on', (describeOn, layoutOn, shown) => {
    flags.describe = describeOn;
    flags.layout = layoutOn;
    expect(STATISTICS_WINDOW_PATH).toBe('/app/statistics/window');
    routeTo(STATISTICS_WINDOW_PATH);
    expect(screen.queryByTestId('window-here') !== null).toBe(shown);
    expect(screen.queryByTestId('no-route') !== null).toBe(!shown);
  });

  it.each(COMBINATIONS)('describe %s, layout %s: the tools list has its entry only with both on', (describeOn, layoutOn, shown) => {
    flags.describe = describeOn;
    flags.layout = layoutOn;
    const tools = statisticsTools();
    expect(tools.some((t) => t.href === STATISTICS_WINDOW_PATH)).toBe(shown);
    if (shown) expect(tools).toContainEqual(WINDOW_TOOL);
  });

  it.each(COMBINATIONS)('describe %s, layout %s: every older route is as it was', (describeOn, layoutOn) => {
    flags.describe = describeOn;
    flags.layout = layoutOn;
    routeTo(STATISTICS_HOME_PATH);
    expect(screen.queryByTestId('home-here') !== null).toBe(describeOn);
    cleanup();
    for (const p of [TTEST_PATH, ONE_SAMPLE_PATH, PAIRED_PATH, ANOVA_PATH, CORR_PATH, ALPHA_PATH]) {
      routeTo(p);
      expect(screen.queryByTestId('old-page') !== null, p).toBe(describeOn);
      cleanup();
    }
  });

  it("the workspace page's tools list links to it, and it comes after the selector", async () => {
    flags.describe = true;
    flags.layout = true;
    flags.workspaces = true;
    render(
      <MemoryRouter initialEntries={['/app/workspaces/statistics']}>
        <WorkspaceShell bridge={makeMockWorkspaceBridge({ workspace_state: ACTIVE })} runs={makeMockDescribeBridge({})} pollMs={20} />
      </MemoryRouter>
    );
    const link = await screen.findByText(WINDOW_TOOL.label);
    expect(link.closest('a')?.getAttribute('href')).toBe(STATISTICS_WINDOW_PATH);
    const items = Array.from((await screen.findByTestId('ws-tools')).querySelectorAll('a')).map((a) => a.textContent);
    expect(items.indexOf(WINDOW_TOOL.label)).toBe(items.indexOf('Statistical Methods') + 1);
  });

  it('App.tsx mounts it lazily through the route function, so a flag off leaves no route', () => {
    const src = fs.readFileSync(path.join(__dirname, '../../../App.tsx'), 'utf8');
    expect(src).toMatch(/const GaplyStatisticsWindowRoute = React\.lazy\(\(\) => import\('\.\/screens\/GaplyScreens'\)\.then\(\(m\) => \(\{ default: m\.StatisticsWindowRoute \}\)\)\)/);
    expect(src).toMatch(/\{statisticsWindowRoute\(<Suspense fallback=\{null\}><GaplyStatisticsWindowRoute \/><\/Suspense>\)\}/);
  });
});

describe('the gate in front of the window (ADR-012 Part 11)', () => {
  it('the route is wrapped in RequireWorkspace id="statistics", with its own sentence', () => {
    const src = fs.readFileSync(path.join(__dirname, '../../GaplyScreens.tsx'), 'utf8');
    const at = src.indexOf('export const StatisticsWindowRoute');
    expect(at).toBeGreaterThan(0);
    const body = src.slice(at, src.indexOf(');', at));
    expect(body).toMatch(/<GaplySessionProvider>\s*<RequireAuth>\s*<RequireWorkspace id="statistics" body="Open the Statistics workspace to use the Statistics window\.">\s*<StatisticsWindow \/>\s*<\/RequireWorkspace>\s*<\/RequireAuth>\s*<\/GaplySessionProvider>/);
  });

  it('with both flags on and the workspace inactive, the gate shows and workspace_state is the only command sent', async () => {
    flags.describe = true;
    flags.layout = true;
    const bridge = makeMockWorkspaceBridge({ workspace_state: NONE });
    render(
      <MemoryRouter>
        <RequireWorkspace id="statistics" bridge={bridge} body="Open the Statistics workspace to use the Statistics window.">
          <StatisticsWindow workspace={bridge} />
        </RequireWorkspace>
      </MemoryRouter>
    );
    expect(await screen.findByTestId('ws-gate-open')).toBeTruthy();
    expect(screen.getByText('Open the Statistics workspace to use the Statistics window.')).toBeTruthy();
    expect(screen.queryByTestId('statistics-window')).toBeNull();
    expect(bridge.calls.map(([c]) => c)).toEqual(['workspace_state']);
  });
});
