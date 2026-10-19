// The project screens inside the real screens (Research OS, Phase 5B-3a;
// decisions 105, 135, 136, 140 and 142). One spy stands for Tauri's invoke,
// so every project command any screen makes is seen in one place.
//
// * With the flags off, with each flag on alone, and in the browser build
//   with both on: no project screen, button or sentence anywhere (the
//   Statistics home, the summary's and the t-test's results and failure
//   screens, Settings), and no project command called.
// * With both on in the desktop app: the Save button on both result screens
//   and on both failure screens (decision 140), the panel, the Settings
//   sentence.
// * Leaving Statistics clears the open project.
// * A name holding HTML, Markdown and a right-to-left override is shown as
//   text, inside <bdi>.
// * Sentinels in a project's name never reach the console, a network call,
//   document.title, the route or history, localStorage or sessionStorage.
import React from 'react';
import { MemoryRouter, Route, Routes, useLocation } from 'react-router-dom';
import { act, cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import DescribeFlow from '../DescribeFlow';
import TTestFlow from '../TTestFlow';
import { StatisticsHome } from '../StatisticsHome';
import { makeMockDescribeBridge } from '../describeBridge';
import { makeMockTTestBridge } from '../ttestBridge';
import { datasetView, resultBundleView } from '../fixtures/describeFixtures';
import { ttestResultView } from '../fixtures/ttestFixtures';
import SettingsPage from '../../settings/SettingsPage';
import { GaplySessionProvider } from '../../session/SessionProvider';
import { WorkspaceShell } from '../../workspace/WorkspaceShell';
import { makeMockWorkspaceBridge } from '../../workspace/workspaceBridge';
import type { WorkspaceStateView } from '../../workspace/workspaceTypes';
import { openProjectStore } from './projectSession';
import { SETTINGS_SENTENCE } from './projectWording';

const env = vi.hoisted(() => ({ projects: true, workspaces: true, desktop: true }));
vi.mock('../../../config/featureFlags', () => ({
  isFeatureEnabled: (name: string) =>
    (name === 'projects' && env.projects) || (name === 'workspaces' && env.workspaces) || name === 'statsDescribe',
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

const ID = '0123456789abcdef0123456789abcdef';
const ACTIVE: WorkspaceStateView = { state: { phase: 'active', workspace: 'statistics' }, activeRun: null, latestRun: null };
const NONE: WorkspaceStateView = { state: { phase: 'no_workspace' }, activeRun: null, latestRun: null };

/** Tauri's invoke: every call recorded, and answered from this table.
 *
 *  Measured while writing this file: when two dynamic imports of
 *  @tauri-apps/api/core run at once (the panel reads the list and the sizes
 *  together), vitest's module mock reaches the first and the second resolves
 *  to the real module, whose invoke calls window.__TAURI_INTERNALS__.invoke.
 *  So both routes lead here, and no call can go unrecorded. */
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
const projectCalls = () => tauri.calls.filter(([c]) => c.startsWith('project_'));

beforeEach(() => {
  env.projects = true;
  env.workspaces = true;
  env.desktop = true;
  tauri.calls = [];
  tauri.answers = {
    workspace_state: ACTIVE,
    project_list: [{ projectId: ID, name: 'Chapter 3', bytes: 4096, savedAt: 1792396351, runCount: 1, refused: null }],
    project_sizes: { totalBytes: 4096, projectCount: 1, projectCapBytes: 104857600, allProjectsCapBytes: 10737418240 },
    project_save_run: { projectId: ID, storedRunId: 1, created: true },
  };
  openProjectStore.clear();
  localStorage.clear();
  sessionStorage.clear();
  (window as unknown as { __TAURI_INTERNALS__: unknown }).__TAURI_INTERNALS__ = {
    invoke: (cmd: string, args?: Record<string, unknown>) => tauri.invoke(cmd, args),
  };
});
afterEach(() => {
  cleanup();
  delete (window as unknown as { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__;
});

/* ---- driving the real flows ------------------------------------------------- */

const FAILED_TTEST = {
  runId: 5,
  state: 'failed',
  terminal: true,
  elapsedMs: 10,
  cancelRequested: false,
  diagnostics: [{ code: 'stats.ttest.both_groups_constant', params: {}, severity: 'error' }],
};

async function ttestTo(end: 'results' | 'failed' | 'cancelled') {
  const bridge = makeMockTTestBridge({
    stats_desc_open: datasetView,
    stats_desc_close: true,
    stats_ttest_group_levels: ({ slot, column }: Record<string, unknown>) => ({
      slot,
      column,
      levels: [
        { index: 0, label: 'a', count: 5 },
        { index: 1, label: 'b', count: 4 },
      ],
    }),
    stats_ttest_validate: { status: 'valid', errors: [], warnings: [], missingInformation: [], estimatedResources: { rows: 9, columns: 2, cells: 18 } },
    stats_ttest_run: { runId: 5 },
    stats_ttest_status:
      end === 'results'
        ? { runId: 5, state: 'succeeded', terminal: true, elapsedMs: 10, cancelRequested: false, diagnostics: [] }
        : end === 'failed'
          ? FAILED_TTEST
          : { runId: 5, state: 'cancelled', terminal: true, elapsedMs: 10, cancelRequested: true, diagnostics: [] },
    stats_ttest_result: ttestResultView,
  });
  render(<TTestFlow bridge={bridge} pickPath={async () => '/x/data.csv'} pollMs={5} />);
  fireEvent.click(screen.getByTestId('tt-choose'));
  fireEvent.click(await screen.findByTestId('tt-to-settings'));
  fireEvent.change(await screen.findByTestId('tt-outcome'), { target: { value: 'C2' } });
  fireEvent.change(screen.getByTestId('tt-group'), { target: { value: 'C1' } });
  await screen.findByTestId('tt-levels');
  fireEvent.click(screen.getByTestId('tt-variant-welch'));
  fireEvent.click(screen.getByTestId('tt-check'));
  fireEvent.click(await screen.findByTestId('tt-run'));
  if (end === 'results') {
    fireEvent.click(await screen.findByTestId('tt-see-results'));
    await screen.findByTestId('tt-result-card');
  } else {
    await screen.findByTestId('tt-run-ended');
  }
}

async function describeTo(end: 'results' | 'failed') {
  const bridge = makeMockDescribeBridge({
    stats_desc_open: datasetView,
    stats_desc_close: true,
    stats_desc_validate: { status: 'valid', errors: [], warnings: [], missingInformation: [], estimatedResources: { rows: 9, columns: 2, cells: 18 } },
    stats_desc_run: { runId: 4 },
    stats_desc_status: {
      runId: 4,
      state: end === 'results' ? 'succeeded' : 'failed',
      terminal: true,
      columnsDone: 2,
      columnsTotal: 2,
      elapsedMs: 10,
      cancelRequested: false,
      diagnostics: end === 'results' ? [] : [{ code: 'stats.desc.input_count', params: { count: 2 }, severity: 'error' }],
    },
    stats_desc_result: resultBundleView,
  });
  render(<DescribeFlow bridge={bridge} pickPath={() => Promise.resolve('/x/data.csv')} pollMs={5} />);
  fireEvent.click(screen.getByTestId('desc-choose'));
  fireEvent.click(await screen.findByTestId('desc-to-settings'));
  fireEvent.click(await screen.findByTestId('desc-check'));
  fireEvent.click(await screen.findByTestId('desc-run'));
  if (end === 'results') {
    fireEvent.click(await screen.findByTestId('desc-see-results'));
    await screen.findByTestId('desc-manifest');
  } else {
    await screen.findByTestId('desc-run-ended');
  }
}

function home() {
  render(
    <MemoryRouter>
      <StatisticsHome bridge={{ list: () => Promise.resolve([]) }} />
    </MemoryRouter>
  );
}

function settings() {
  const auth = {
    signUp: vi.fn(),
    signIn: vi.fn(),
    signInWithOAuth: vi.fn(),
    signOut: vi.fn(),
    getSession: vi.fn().mockResolvedValue({ session: null, offline: true }),
    onAuthStateChange: (cb: (s: unknown) => void) => {
      cb(null);
      return () => undefined;
    },
  };
  const profiles = { getOwn: vi.fn().mockResolvedValue({ data: null, error: null, offline: true }), upsertOwn: vi.fn() };
  render(
    <MemoryRouter>
      <GaplySessionProvider authService={auth as never}>
        <SettingsPage profileService={profiles as never} />
      </GaplySessionProvider>
    </MemoryRouter>
  );
}

/** Lets effects and their reads settle. */
const settle = () => act(async () => new Promise((r) => setTimeout(r, 20)));

/* ---- the flags ------------------------------------------------------------- */

describe('with the project screens hidden', () => {
  it.each([
    ['both flags off', { projects: false, workspaces: false, desktop: true }],
    ['only REACT_APP_FEATURE_PROJECTS', { projects: true, workspaces: false, desktop: true }],
    ['only REACT_APP_FEATURE_WORKSPACES', { projects: false, workspaces: true, desktop: true }],
    ['the browser build, both flags on', { projects: true, workspaces: true, desktop: false }],
  ])('%s: no screen, button or sentence, and no project command', async (_name, flags) => {
    Object.assign(env, flags);
    home();
    await settle();
    expect(screen.queryByTestId('projects-panel')).toBeNull();
    cleanup();
    await ttestTo('results');
    expect(screen.queryByTestId('tt-project-save')).toBeNull();
    cleanup();
    await ttestTo('failed');
    expect(screen.queryByTestId('tt-project-save')).toBeNull();
    cleanup();
    await describeTo('results');
    expect(screen.queryByTestId('desc-project-save')).toBeNull();
    cleanup();
    await describeTo('failed');
    expect(screen.queryByTestId('desc-project-save')).toBeNull();
    cleanup();
    settings();
    await settle();
    expect(screen.queryByTestId('settings-projects-sentence')).toBeNull();
    expect(document.body.textContent).not.toContain('Save to a project');
    expect(document.body.textContent).not.toContain(SETTINGS_SENTENCE);
    expect(projectCalls()).toEqual([]);
  });
});

describe('with the project screens shown', () => {
  it('Save is on both result screens and both failure screens, the panel on the home, the sentence in Settings', async () => {
    await ttestTo('results');
    expect(screen.getByTestId('tt-project-save-button')).toBeTruthy();
    cleanup();
    await ttestTo('failed');
    expect(screen.getByTestId('tt-project-save-button')).toBeTruthy();
    cleanup();
    await describeTo('results');
    expect(screen.getByTestId('desc-project-save-button')).toBeTruthy();
    cleanup();
    await describeTo('failed');
    expect(screen.getByTestId('desc-project-save-button')).toBeTruthy();
    cleanup();
    home();
    expect(await screen.findByTestId('projects-panel')).toBeTruthy();
    cleanup();
    settings();
    expect((await screen.findByTestId('settings-projects-sentence')).textContent).toBe(SETTINGS_SENTENCE);
    // Reading the list, the sizes and which project is open (decision 148)
    // is all that showing them calls.
    expect(projectCalls().every(([c]) => c === 'project_list' || c === 'project_sizes' || c === 'project_current')).toBe(true);
  });

  it('a cancelled t-test offers no Save (savableOnScreen covers every state; the screen agrees)', async () => {
    await ttestTo('cancelled');
    expect(screen.queryByTestId('tt-project-save')).toBeNull();
  });
});

/* ---- leaving Statistics ------------------------------------------------------ */

describe('leaving Statistics', () => {
  it('clears the open project, and the panel shows none as open', async () => {
    openProjectStore.set({ projectId: ID, name: 'Chapter 3' });
    const workspace = makeMockWorkspaceBridge({ workspace_state: ACTIVE, workspace_deactivate: NONE });
    render(
      <MemoryRouter>
        <WorkspaceShell bridge={workspace} runs={makeMockDescribeBridge()} ttestRuns={makeMockTTestBridge()} pollMs={100000} />
      </MemoryRouter>
    );
    await waitFor(() => expect((screen.getByTestId('ws-switch') as HTMLButtonElement).disabled).toBe(false));
    expect(openProjectStore.get()).not.toBeNull();
    fireEvent.click(screen.getByTestId('ws-switch'));
    await waitFor(() => expect(openProjectStore.get()).toBeNull());
    cleanup();
    tauri.answers.workspace_state = NONE;
    openProjectStore.set({ projectId: ID, name: 'Chapter 3' });
    home();
    await screen.findByTestId(`project-row-${ID}`);
    await waitFor(() => expect(screen.queryByTestId(`project-open-${ID}`)).toBeNull());
    expect(openProjectStore.get()).toBeNull();
  });

  it('the shell reading any state but Statistics active clears it', async () => {
    openProjectStore.set({ projectId: ID, name: 'Chapter 3' });
    render(
      <MemoryRouter>
        <WorkspaceShell
          bridge={makeMockWorkspaceBridge({ workspace_state: NONE })}
          runs={makeMockDescribeBridge()}
          ttestRuns={makeMockTTestBridge()}
          pollMs={100000}
        />
      </MemoryRouter>
    );
    await waitFor(() => expect(openProjectStore.get()).toBeNull());
  });
});

/* ---- names ------------------------------------------------------------------ */

const HOSTILE = '<img src=x onerror="window.__hit=1"> **bold** [a](javascript:x) ‮evil';

describe('a name holding HTML, Markdown and a right-to-left override', () => {
  it('is text inside <bdi> in the panel, the delete dialog and the Save dialog', async () => {
    tauri.answers.project_list = [{ projectId: ID, name: HOSTILE, bytes: 4096, savedAt: 1792396351, runCount: 1, refused: null }];
    // Rust has it open (decision 148): the screens read that, not memory.
    tauri.answers.project_current = { projectId: ID };
    openProjectStore.set({ projectId: ID, name: HOSTILE });
    home();
    const name = await screen.findByTestId(`project-name-${ID}`);
    expect(name.tagName).toBe('BDI');
    expect(name.textContent).toBe(HOSTILE);
    expect(name.children).toHaveLength(0);
    fireEvent.click(screen.getByTestId(`project-delete-${ID}`));
    const inDialog = screen.getByTestId('project-delete-name').querySelector('bdi');
    expect(inDialog?.textContent).toBe(HOSTILE);
    expect(inDialog?.children).toHaveLength(0);
    fireEvent.click(screen.getByTestId('project-delete-cancel'));
    cleanup();
    await ttestTo('results');
    fireEvent.click(screen.getByTestId('tt-project-save-button'));
    const open = screen.getByTestId('tt-project-save-open-name');
    expect(open.tagName).toBe('BDI');
    expect(open.textContent).toBe(HOSTILE);
    expect(document.querySelector('img')).toBeNull();
    expect(document.querySelector('strong a, a[href^="javascript"]')).toBeNull();
    expect((window as unknown as { __hit?: number }).__hit).toBeUndefined();
  });
});

/* ---- sentinels -------------------------------------------------------------- */

describe("a project's name goes nowhere it must not", () => {
  it('not the console, a network call, document.title, the route or history, or browser storage', async () => {
    const NAME = 'NAME_SENTINEL_q8 Chapter';
    const consoleSeen: string[] = [];
    const spies = (['log', 'info', 'warn', 'error', 'debug'] as const).map((k) =>
      vi.spyOn(console, k).mockImplementation((...a: unknown[]) => {
        consoleSeen.push(a.map(String).join(' '));
      })
    );
    const fetchSpy = vi.fn().mockResolvedValue(new Response('{}'));
    const realFetch = globalThis.fetch;
    globalThis.fetch = fetchSpy as typeof fetch;
    const xhrOpen = vi.spyOn(XMLHttpRequest.prototype, 'open');
    const xhrSend = vi.spyOn(XMLHttpRequest.prototype, 'send');
    const beacon = vi.fn();
    Object.defineProperty(navigator, 'sendBeacon', { value: beacon, configurable: true });
    const titleBefore = document.title;
    const historyBefore = window.history.length;
    const seen: string[] = [];
    const Where: React.FC = () => {
      const l = useLocation();
      seen.push(`${l.pathname}${l.search}${l.hash}${JSON.stringify(l.state)}`);
      return null;
    };
    try {
      tauri.answers.project_list = [{ projectId: ID, name: NAME, bytes: 4096, savedAt: 1792396351, runCount: 1, refused: null }];
      tauri.answers.project_delete = () => {
        throw { code: 'validation', message: JSON.stringify({ code: 'project.open_elsewhere' }) };
      };
      tauri.answers.project_prepare_delete = { projectId: ID, confirmation: 'ab'.repeat(32), bytes: 4096, expiresInMs: 300000 };
      // A save into a new project named with the sentinel, through the real flow.
      await ttestTo('results');
      fireEvent.click(screen.getByTestId('tt-project-save-button'));
      fireEvent.change(screen.getByTestId('tt-project-save-name'), { target: { value: NAME } });
      fireEvent.click(screen.getByTestId('tt-project-save-confirm'));
      await waitFor(() => expect(screen.getByTestId('tt-project-save-status').textContent).toContain(NAME));
      cleanup();
      // The panel, the delete dialog and a refusal, inside a router.
      render(
        <MemoryRouter initialEntries={['/app/statistics']}>
          <Routes>
            <Route
              path="*"
              element={
                <>
                  <Where />
                  <StatisticsHome bridge={{ list: () => Promise.resolve([]) }} />
                </>
              }
            />
          </Routes>
        </MemoryRouter>
      );
      fireEvent.click(await screen.findByTestId(`project-delete-${ID}`));
      fireEvent.click(screen.getByTestId('project-delete-confirm'));
      await screen.findByTestId('project-delete-refusal');
      await settle();
    } finally {
      spies.forEach((s) => s.mockRestore());
      globalThis.fetch = realFetch;
    }
    // The name did reach Rust, as the Save's argument, and only there.
    expect(projectCalls().some(([c, a]) => c === 'project_save_run' && JSON.stringify(a).includes('NAME_SENTINEL'))).toBe(true);
    expect(consoleSeen.join('\n')).not.toContain('NAME_SENTINEL');
    expect(fetchSpy).not.toHaveBeenCalled();
    expect(xhrOpen).not.toHaveBeenCalled();
    expect(xhrSend).not.toHaveBeenCalled();
    expect(beacon).not.toHaveBeenCalled();
    expect(document.title).toBe(titleBefore);
    expect(document.title).not.toContain('NAME_SENTINEL');
    expect(window.history.length).toBe(historyBefore);
    expect(JSON.stringify(window.history.state ?? null)).not.toContain('NAME_SENTINEL');
    expect(window.location.href).not.toContain('NAME_SENTINEL');
    expect(seen.length).toBeGreaterThan(0);
    expect(seen.join('\n')).not.toContain('NAME_SENTINEL');
    const stored = (s: Storage) => Array.from({ length: s.length }, (_, i) => `${s.key(i)}=${s.getItem(s.key(i) as string)}`).join('\n');
    expect(stored(localStorage)).not.toContain('NAME_SENTINEL');
    expect(stored(sessionStorage)).not.toContain('NAME_SENTINEL');
    xhrOpen.mockRestore();
    xhrSend.mockRestore();
  });
});
