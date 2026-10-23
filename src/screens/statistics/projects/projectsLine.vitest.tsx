// The "Projects" line on the Statistics workspace page (Research OS, Phase
// 5B-4b; decision 172). It lives beside the other project screen tests, the
// only folder whose tests may name project commands (projectGuards.vitest.ts). The hand test's rerun found the projects hard to find:
// the panel sits at the foot of the Statistical methods page.
//
// * The line is in the Tools card only where the project pages can open:
//   projectsShown() (both flags, desktop app) and the describe flag, which the
//   Statistical methods page sits behind.
// * Clicking it opens that page and, once its method list has loaded or
//   failed, moves the focus to the Projects panel's heading, which brings the
//   panel into view.
// * Opened any other way, the page moves no focus, as before.
import React from 'react';
import { MemoryRouter, Route, Routes } from 'react-router-dom';
import { cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { WorkspaceShell } from '../../workspace/WorkspaceShell';
import { makeMockWorkspaceBridge } from '../../workspace/workspaceBridge';
import type { WorkspaceStateView } from '../../workspace/workspaceTypes';
import { StatisticsHome } from '../StatisticsHome';
import { makeMockDescribeBridge } from '../describeBridge';
import { makeMockTTestBridge, type CapabilityBridge } from '../ttestBridge';
import { capabilityList } from '../fixtures/ttestFixtures';
import type { CapabilityView } from '../ttestTypes';
import { openProjectStore } from './projectSession';

const env = vi.hoisted(() => ({ projects: true, workspaces: true, statsDescribe: true, desktop: true }));
vi.mock('../../../config/featureFlags', () => ({
  isFeatureEnabled: (name: string) =>
    (name === 'projects' && env.projects) ||
    (name === 'workspaces' && env.workspaces) ||
    (name === 'statsDescribe' && env.statsDescribe),
}));
vi.mock('../../../utils/isTauri', () => ({
  get isTauri() {
    return env.desktop;
  },
  get default() {
    return env.desktop;
  },
}));
vi.mock('../../../design-system/GaplyGlobe', () => ({ GaplyGlobe: () => <div /> }));

const ACTIVE: WorkspaceStateView = { state: { phase: 'active', workspace: 'statistics' }, activeRun: null, latestRun: null };

/** Tauri's invoke, for the Projects panel's own bridges (the pattern of
 *  projectScreens.vitest.tsx: both the module mock and the window global). */
const tauri = vi.hoisted(() => {
  const t = {
    answers: {} as Record<string, unknown>,
    invoke(cmd: string): Promise<unknown> {
      return Promise.resolve(t.answers[cmd] ?? null);
    },
  };
  return t;
});
vi.mock('@tauri-apps/api/core', () => ({ invoke: (cmd: string) => tauri.invoke(cmd) }));

beforeEach(() => {
  Object.assign(env, { projects: true, workspaces: true, statsDescribe: true, desktop: true });
  tauri.answers = {
    workspace_state: ACTIVE,
    project_list: [],
    project_sizes: { totalBytes: 0, projectCount: 0, projectCapBytes: 104857600, allProjectsCapBytes: 10737418240 },
    project_current: { projectId: null },
  };
  openProjectStore.clear();
  (window as unknown as { __TAURI_INTERNALS__: unknown }).__TAURI_INTERNALS__ = {
    invoke: (cmd: string) => tauri.invoke(cmd),
  };
});
afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
  delete (window as unknown as { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__;
});

/** A capability list the test resolves or fails when it chooses. */
function heldList() {
  let settle: { resolve: (l: CapabilityView[]) => void; reject: (e: unknown) => void } | null = null;
  const bridge: CapabilityBridge = {
    list: () =>
      new Promise<CapabilityView[]>((resolve, reject) => {
        settle = { resolve, reject };
      }),
  };
  return {
    bridge,
    resolve: () => settle?.resolve(capabilityList as CapabilityView[]),
    reject: () => settle?.reject(new Error('unread')),
  };
}

function app(start: string, capabilities: CapabilityBridge) {
  render(
    <MemoryRouter initialEntries={[start]}>
      <Routes>
        <Route
          path="/app/workspaces/statistics"
          element={
            <WorkspaceShell
              bridge={makeMockWorkspaceBridge({ workspace_state: ACTIVE })}
              runs={makeMockDescribeBridge({})}
              ttestRuns={makeMockTTestBridge({})}
              pollMs={100000}
            />
          }
        />
        <Route path="/app/statistics" element={<StatisticsHome bridge={capabilities} />} />
      </Routes>
    </MemoryRouter>
  );
}

const projectsLine = () => screen.queryByTestId('ws-tools-projects');

describe('the Projects line in the Tools card', () => {
  it('shows with both flags on in the desktop app, beside the Statistical methods line and with its own key', async () => {
    const errors = vi.spyOn(console, 'error').mockImplementation(() => {});
    app('/app/workspaces/statistics', heldList().bridge);
    const tools = await screen.findByTestId('ws-tools');
    const line = await within(tools).findByTestId('ws-tools-projects');
    expect(line.textContent).toBe('Projects');
    expect(line.tagName).toBe('A');
    expect(line.classList.contains('gds-link')).toBe(true);
    expect(within(tools).getAllByRole('link').map((a) => a.textContent)).toContain('Statistical Methods');
    // Two lines lead to the same page: a shared key would make React warn.
    expect(errors.mock.calls.flat().join(' ')).not.toMatch(/same key/i);
  });

  it.each([
    ['the projects flag off', { projects: false }],
    ['the describe flag off', { statsDescribe: false }],
    ['the browser', { desktop: false }],
  ])('is absent with %s', async (_what, off) => {
    Object.assign(env, off);
    app('/app/workspaces/statistics', heldList().bridge);
    await screen.findByTestId('ws-tools');
    expect(projectsLine()).toBeNull();
  });
});

describe('where the Projects line leads', () => {
  it('opens the Statistical methods page and, once its methods have loaded, focuses the Projects heading', async () => {
    const list = heldList();
    app('/app/workspaces/statistics', list.bridge);
    fireEvent.click(await screen.findByTestId('ws-tools-projects'));
    const heading = await screen.findByTestId('projects-heading');
    expect(screen.getByTestId('stats-home')).toBeTruthy();
    // Not before the methods above the panel have been drawn.
    await new Promise((r) => setTimeout(r, 30));
    expect(document.activeElement).not.toBe(heading);
    list.resolve();
    await screen.findByTestId('stats-cards');
    await waitFor(() => expect(document.activeElement).toBe(heading));
  });

  it('also focuses the heading when the method list fails', async () => {
    const list = heldList();
    app('/app/workspaces/statistics', list.bridge);
    fireEvent.click(await screen.findByTestId('ws-tools-projects'));
    const heading = await screen.findByTestId('projects-heading');
    list.reject();
    await screen.findByTestId('stats-home-error');
    await waitFor(() => expect(document.activeElement).toBe(heading));
  });

  it('moves no focus when the page is opened directly', async () => {
    const list = heldList();
    app('/app/statistics', list.bridge);
    const heading = await screen.findByTestId('projects-heading');
    list.resolve();
    await screen.findByTestId('stats-cards');
    await new Promise((r) => setTimeout(r, 30));
    expect(document.activeElement).not.toBe(heading);
    expect(document.activeElement).toBe(document.body);
  });

  it('moves no focus when the page is opened from the Statistical methods line', async () => {
    const list = heldList();
    app('/app/workspaces/statistics', list.bridge);
    const tools = await screen.findByTestId('ws-tools');
    fireEvent.click(within(tools).getByText('Statistical Methods'));
    const heading = await screen.findByTestId('projects-heading');
    list.resolve();
    await screen.findByTestId('stats-cards');
    await new Promise((r) => setTimeout(r, 30));
    expect(document.activeElement).not.toBe(heading);
  });
});
