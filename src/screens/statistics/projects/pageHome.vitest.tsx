// Where the project page goes back to (Research OS, Phase 8U-1, decision 524):
// projectPageHome() is the Statistics window while it is shown, and the old
// home otherwise, and nothing else; the page screen's Close, its rail's
// Statistics item and Back to Statistics all go there, with no router state.
// One recorder stands for Tauri's invoke, as projectPageScreens.vitest.tsx's.
import React from 'react';
import { MemoryRouter, Route, Routes, useLocation } from 'react-router-dom';
import { cleanup, fireEvent, render, screen, within } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { ProjectPageScreen } from '../ProjectPageScreen';
import { projectPageHome, STATISTICS_HOME_PATH, STATISTICS_WINDOW_PATH } from '../statisticsRoute';
import { openProjectStore } from './projectSession';
import { projectPagePath, projectRoutes } from './projectRoute';

const env = vi.hoisted(() => ({ statsDescribe: true, statsLayout: true }));
vi.mock('../../../config/featureFlags', () => ({
  isFeatureEnabled: (name: string) =>
    name === 'projects' || name === 'workspaces' || (name === 'statsDescribe' && env.statsDescribe) || (name === 'statsLayout' && env.statsLayout),
}));
vi.mock('../../../utils/isTauri', () => ({ isTauri: true, default: true }));
vi.mock('../../../design-system/GaplyGlobe', () => ({ GaplyGlobe: () => <div /> }));

const tauri = vi.hoisted(() => {
  const t = {
    calls: [] as Array<[string, Record<string, unknown>]>,
    answers: {} as Record<string, unknown>,
    invoke(cmd: string, args: Record<string, unknown> = {}): Promise<unknown> {
      t.calls.push([cmd, args]);
      const a = t.answers[cmd];
      return typeof a === 'function' ? Promise.resolve().then(() => (a as (x: unknown) => unknown)(args)) : Promise.resolve(a ?? null);
    },
  };
  return t;
});
vi.mock('@tauri-apps/api/core', () => ({ invoke: (cmd: string, args?: Record<string, unknown>) => tauri.invoke(cmd, args) }));

const ID = '0123456789abcdef0123456789abcdef';
const ACTIVE = { state: { phase: 'active', workspace: 'statistics' }, activeRun: null, latestRun: null };
let openNow = true;

beforeEach(() => {
  Object.assign(env, { statsDescribe: true, statsLayout: true });
  openNow = true;
  tauri.calls = [];
  tauri.answers = {
    workspace_state: ACTIVE,
    project_current: () => ({ projectId: openNow ? ID : null, rerun: null }),
    project_list: [{ projectId: ID, name: 'Chapter 3', bytes: 4096, savedAt: 1792396351, runCount: 0, refused: null }],
    project_sizes: { totalBytes: 4096, projectCount: 1, projectCapBytes: 104857600, allProjectsCapBytes: 10737418240 },
    project_close: () => {
      openNow = false;
      return { closed: true };
    },
    project_runs: [],
  };
  openProjectStore.clear();
  (window as unknown as { __TAURI_INTERNALS__: unknown }).__TAURI_INTERNALS__ = {
    invoke: (cmd: string, args?: Record<string, unknown>) => tauri.invoke(cmd, args),
  };
});
afterEach(() => {
  cleanup();
  delete (window as unknown as { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__;
});

function page() {
  const seen: string[] = [];
  const Where: React.FC = () => {
    const l = useLocation();
    seen.push(`${l.pathname}${l.search}${l.hash}${JSON.stringify(l.state)}`);
    return null;
  };
  render(
    <MemoryRouter initialEntries={[projectPagePath(ID)]}>
      <Where />
      <Routes>
        {projectRoutes(<ProjectPageScreen />)}
        <Route path={STATISTICS_WINDOW_PATH} element={<div data-testid="window-here" />} />
        <Route path={STATISTICS_HOME_PATH} element={<div data-testid="old-home-here" />} />
      </Routes>
    </MemoryRouter>
  );
  return seen;
}

const railStatistics = () => within(screen.getByTestId('gds-rail')).getByText('Statistics').closest('button') as HTMLButtonElement;

describe('projectPageHome (decision 524)', () => {
  it('is the window while it is shown, and the old home otherwise: those two fixed paths alone', () => {
    const got = new Set<string>();
    for (const [statsDescribe, statsLayout, want] of [
      [true, true, STATISTICS_WINDOW_PATH],
      [true, false, STATISTICS_HOME_PATH],
      [false, true, STATISTICS_HOME_PATH],
      [false, false, STATISTICS_HOME_PATH],
    ] as const) {
      Object.assign(env, { statsDescribe, statsLayout });
      expect(projectPageHome(), `${statsDescribe} ${statsLayout}`).toBe(want);
      got.add(projectPageHome());
    }
    expect(Array.from(got).sort()).toEqual([STATISTICS_HOME_PATH, STATISTICS_WINDOW_PATH].sort());
    expect(STATISTICS_WINDOW_PATH).toBe('/app/statistics/window');
    expect(STATISTICS_HOME_PATH).toBe('/app/statistics');
  });
});

describe('the page screen goes home through it (decision 524)', () => {
  it("Close on an open project's page goes to the window while it is shown", async () => {
    const seen = page();
    fireEvent.click(await screen.findByTestId('project-page-close'));
    expect(await screen.findByTestId('window-here')).toBeTruthy();
    expect(seen[seen.length - 1]).toBe(`${STATISTICS_WINDOW_PATH}null`);
  });

  it("the rail's Statistics item goes to the window while it is shown, the project left open", async () => {
    const seen = page();
    await screen.findByTestId('project-page-close');
    fireEvent.click(railStatistics());
    expect(await screen.findByTestId('window-here')).toBeTruthy();
    expect(seen[seen.length - 1]).toBe(`${STATISTICS_WINDOW_PATH}null`);
    expect(tauri.calls.map(([c]) => c)).not.toContain('project_close');
  });

  it('Back to Statistics, on a page with no project open, goes to the window while it is shown', async () => {
    openNow = false;
    const seen = page();
    fireEvent.click(await screen.findByTestId('project-page-back'));
    expect(await screen.findByTestId('window-here')).toBeTruthy();
    expect(seen[seen.length - 1]).toBe(`${STATISTICS_WINDOW_PATH}null`);
  });

  it("without the window's flag, the rail's Statistics item goes to the old home, as before", async () => {
    env.statsLayout = false;
    const seen = page();
    await screen.findByTestId('project-page-close');
    fireEvent.click(railStatistics());
    expect(await screen.findByTestId('old-home-here')).toBeTruthy();
    expect(seen[seen.length - 1]).toBe(`${STATISTICS_HOME_PATH}null`);
  });
});
