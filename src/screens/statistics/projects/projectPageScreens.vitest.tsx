// The project page inside the real screens (Research OS, Phase 5B-3b;
// decisions 148 to 151). One recorder stands for Tauri's invoke, so every
// project command any screen makes is seen in one place (the two routes to it
// are explained in projectScreens.vitest.tsx).
//
// * With both flags off, with each on alone, and in the browser build with
//   all on: no Open, no project page route, no page, and no project command.
// * Open on the Statistics home leads to the page by the project's id, inside
//   the real router; Close comes back to the home.
// * Under React StrictMode: no open, close or save call made twice, and no
//   command from an effect but the reads.
// * Sentinels in a project's name and in a stored result, wrong-shape stored
//   JSON included, never reach the console, a network call, document.title,
//   the route, history or history.state, localStorage, sessionStorage or
//   IndexedDB.
import fs from 'node:fs';
import path from 'node:path';
import React from 'react';
import { MemoryRouter, Route, Routes, useLocation } from 'react-router-dom';
import { act, cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { StatisticsHome } from '../StatisticsHome';
import { ProjectPageScreen } from '../ProjectPageScreen';
import { STATISTICS_HOME_PATH, statisticsRoutes } from '../statisticsRoute';
import { openProjectStore } from './projectSession';
import { PROJECT_PAGE_PATH, projectPagePath, projectRoutes } from './projectRoute';
import { ProjectPage } from './ProjectPage';

const env = vi.hoisted(() => ({ projects: true, workspaces: true, statsDescribe: true, desktop: true }));
vi.mock('../../../config/featureFlags', () => ({
  isFeatureEnabled: (name: string) =>
    (name === 'projects' && env.projects) || (name === 'workspaces' && env.workspaces) || (name === 'statsDescribe' && env.statsDescribe),
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

const ID = '0123456789abcdef0123456789abcdef';
const ACTIVE = { state: { phase: 'active', workspace: 'statistics' }, activeRun: null, latestRun: null };
const STORED = path.join(__dirname, '../../../../src-tauri/tests/fixtures/projects/stored');
const storedText = (name: string) => fs.readFileSync(path.join(STORED, name), 'utf8');
const ACTIONS = ['project_open', 'project_close', 'project_save_run', 'project_delete', 'project_prepare_delete'];

let openNow = false;

beforeEach(() => {
  Object.assign(env, { projects: true, workspaces: true, statsDescribe: true, desktop: true });
  openNow = false;
  tauri.calls = [];
  tauri.answers = {
    workspace_state: ACTIVE,
    capability_list: [],
    project_current: () => ({ projectId: openNow ? ID : null }),
    project_list: [{ projectId: ID, name: 'Chapter 3', bytes: 4096, savedAt: 1792396351, runCount: 2, refused: null }],
    project_sizes: { totalBytes: 4096, projectCount: 1, projectCapBytes: 104857600, allProjectsCapBytes: 10737418240 },
    project_open: ({ projectId }: Record<string, unknown>) => {
      openNow = true;
      return { projectId, name: 'Chapter 3', createdAt: 1792396000, runCount: 2, bytes: 4096 };
    },
    project_close: () => {
      openNow = false;
      return { closed: true };
    },
    project_runs: [
      { storedRunId: 1, capabilityId: 'stats.describe', capabilityVersion: 1, state: 'succeeded', endCode: null, startedAt: 1, endedAt: 2, savedAt: 1792396100, agentVersion: '0.1.0' },
      { storedRunId: 2, capabilityId: 'stats.analyze.independent_t', capabilityVersion: 1, state: 'succeeded', endCode: null, startedAt: 1, endedAt: 2, savedAt: 1792396200, agentVersion: '0.1.0' },
    ],
    project_run_result: ({ storedRunId }: Record<string, unknown>) => JSON.parse(storedText(storedRunId === 1 ? 'summary.json' : 'ttest.json')),
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

/** The home and the page, in a router, as App.tsx registers them. */
function app(at: string, strict = false, extra: React.ReactNode = null) {
  const seen: string[] = [];
  const Where: React.FC = () => {
    const l = useLocation();
    seen.push(`${l.pathname}${l.search}${l.hash}${JSON.stringify(l.state)}`);
    return null;
  };
  const ui = (
    <MemoryRouter initialEntries={[at]}>
      <Where />
      {extra}
      <Routes>
        {statisticsRoutes(<StatisticsHome bridge={{ list: () => Promise.resolve([]) }} />, <div />, <div />, <div />)}
        {projectRoutes(<ProjectPageScreen />)}
        <Route path="*" element={<div data-testid="no-route" />} />
      </Routes>
    </MemoryRouter>
  );
  render(strict ? <React.StrictMode>{ui}</React.StrictMode> : ui);
  return seen;
}

const settle = () => act(async () => new Promise((r) => setTimeout(r, 20)));

describe('with the project screens hidden', () => {
  it.each([
    ['all three flags off', { projects: false, workspaces: false, statsDescribe: false, desktop: true }],
    ['only REACT_APP_FEATURE_PROJECTS', { projects: true, workspaces: false, statsDescribe: false, desktop: true }],
    ['only REACT_APP_FEATURE_WORKSPACES', { projects: false, workspaces: true, statsDescribe: false, desktop: true }],
    ['only REACT_APP_FEATURE_STATS_DESCRIBE', { projects: false, workspaces: false, statsDescribe: true, desktop: true }],
    ['the projects and workspaces flags without the summary flag', { projects: true, workspaces: true, statsDescribe: false, desktop: true }],
    ['the browser build, all flags on', { projects: true, workspaces: true, statsDescribe: true, desktop: false }],
  ])('%s: no Open, no page, no route and no project command', async (_name, flags) => {
    Object.assign(env, flags);
    expect(projectRoutes(<div />)).toEqual([]);
    app(STATISTICS_HOME_PATH);
    await settle();
    expect(screen.queryByTestId(`project-open-button-${ID}`)).toBeNull();
    cleanup();
    app(projectPagePath(ID));
    await settle();
    expect(screen.queryByTestId('project-page')).toBeNull();
    expect(screen.queryByTestId('project-page-screen')).toBeNull();
    cleanup();
    // The component itself asks projectsShown(), as every project screen
    // does; the summary's flag is the route's (projectRoute.tsx).
    if (!(flags.projects && flags.workspaces && flags.desktop)) {
      render(<ProjectPage projectId={ID} onClosed={vi.fn()} onBack={vi.fn()} />);
      await settle();
      expect(screen.queryByTestId('project-page')).toBeNull();
    }
    expect(projectCalls()).toEqual([]);
  });

  it('with them shown, the route is the one path, holding only the id', () => {
    expect(projectRoutes(<div />)).toHaveLength(1);
    expect(PROJECT_PAGE_PATH).toBe('/app/statistics/projects/:projectId');
    expect(projectPagePath(ID)).toBe(`/app/statistics/projects/${ID}`);
  });
});

describe('Open, the page and Close, in the router', () => {
  it('Open on the home leads to the page by the id; Close comes back to the home', async () => {
    const seen = app(STATISTICS_HOME_PATH);
    fireEvent.click(await screen.findByTestId(`project-open-button-${ID}`));
    expect(await screen.findByTestId('project-runs-list')).toBeTruthy();
    expect(seen[seen.length - 1]).toBe(`/app/statistics/projects/${ID}null`);
    fireEvent.click(screen.getByTestId('project-page-close'));
    expect(await screen.findByTestId('projects-panel')).toBeTruthy();
    expect(seen[seen.length - 1]).toBe(`${STATISTICS_HOME_PATH}null`);
    expect(projectCalls().filter(([c]) => c === 'project_open')).toEqual([['project_open', { projectId: ID }]]);
    expect(projectCalls().filter(([c]) => c === 'project_close')).toEqual([['project_close', {}]]);
  });

  it('under StrictMode: no open, close or save twice, and only reads from effects', async () => {
    app(STATISTICS_HOME_PATH, true);
    await screen.findByTestId(`project-open-button-${ID}`);
    await settle();
    expect(projectCalls().filter(([c]) => ACTIONS.indexOf(c) >= 0)).toEqual([]);
    fireEvent.click(screen.getByTestId(`project-open-button-${ID}`));
    await screen.findByTestId('project-runs-list');
    await settle();
    expect(projectCalls().filter(([c]) => ACTIONS.indexOf(c) >= 0)).toEqual([['project_open', { projectId: ID }]]);
    fireEvent.click(screen.getByTestId('project-run-open-2'));
    await screen.findByTestId('tt-result-card');
    fireEvent.click(screen.getByTestId('project-page-close'));
    await screen.findByTestId('projects-panel');
    await settle();
    expect(projectCalls().filter(([c]) => ACTIONS.indexOf(c) >= 0)).toEqual([
      ['project_open', { projectId: ID }],
      ['project_close', {}],
    ]);
    const reads = ['project_current', 'project_list', 'project_sizes', 'project_runs', 'project_run_result'];
    expect(projectCalls().every(([c]) => reads.indexOf(c) >= 0 || ACTIONS.indexOf(c) >= 0)).toBe(true);
  });
});

describe('sentinels in a name and in stored results', () => {
  it('reach no console, network call, title, route, history, history.state, storage or IndexedDB', async () => {
    const NAME = 'NAME_SENTINEL_z4 Chapter';
    const HEADER = 'HEADER_SENTINEL_k2';
    const LABEL = 'LABEL_SENTINEL_m7';
    const summary = JSON.parse(storedText('summary.json'));
    const bundle = JSON.parse(summary.bundleJson);
    bundle.primaryOutputs.columns[0].numeric.header = HEADER;
    bundle.primaryOutputs.columns[1].categorical.levels[0].label = LABEL;
    summary.bundleJson = JSON.stringify(bundle);
    const ttest = JSON.parse(storedText('ttest.json'));
    const wrong = JSON.parse(ttest.bundleJson);
    wrong.primaryOutputs.groups[0].label = LABEL;
    wrong.primaryOutputs.groups[0].mean = null;
    wrong.primaryOutputs.t = `${HEADER} not a number`;
    ttest.bundleJson = JSON.stringify(wrong);
    const notJson = { ...JSON.parse(storedText('ttest.json')), bundleJson: `{"label":"${LABEL}", "x": ${HEADER}` };
    tauri.answers.project_list = [{ projectId: ID, name: NAME, bytes: 4096, savedAt: 1792396351, runCount: 3, refused: null }];
    tauri.answers.project_open = ({ projectId }: Record<string, unknown>) => {
      openNow = true;
      return { projectId, name: NAME, createdAt: 1, runCount: 3, bytes: 4096 };
    };
    tauri.answers.project_runs = [1, 2, 3].map((n) => ({
      storedRunId: n,
      capabilityId: n === 1 ? 'stats.describe' : 'stats.analyze.independent_t',
      capabilityVersion: 1,
      state: 'succeeded',
      endCode: null,
      startedAt: 1,
      endedAt: 2,
      savedAt: 1792396100 + n,
      agentVersion: '0.1.0',
    }));
    tauri.answers.project_run_result = ({ storedRunId }: Record<string, unknown>) => (storedRunId === 1 ? summary : storedRunId === 2 ? ttest : notJson);
    const report = new TextEncoder().encode(`# ${NAME}\n`);
    tauri.answers.project_report = {
      fileName: 'gaply_project_report.md',
      bytes: Array.from(report),
      sha256: Array.from(new Uint8Array(await crypto.subtle.digest('SHA-256', report)), (b) => b.toString(16).padStart(2, '0')).join(''),
    };

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
    const idbOpen = vi.fn();
    const hadIdb = 'indexedDB' in window;
    Object.defineProperty(window, 'indexedDB', { value: { open: idbOpen, deleteDatabase: idbOpen }, configurable: true });
    const titleBefore = document.title;
    const historyBefore = window.history.length;
    const pushState = vi.spyOn(window.history, 'pushState');
    const replaceState = vi.spyOn(window.history, 'replaceState');
    let seen: string[] = [];
    try {
      seen = app(STATISTICS_HOME_PATH);
      fireEvent.click(await screen.findByTestId(`project-open-button-${ID}`));
      await screen.findByTestId('project-runs-list');
      for (const [n, ready] of [
        [1, 'desc-manifest'],
        [2, 'saved-result-bad-shape'],
        [3, 'saved-result-bad-shape'],
      ] as const) {
        fireEvent.click(screen.getByTestId(`project-run-open-${n}`));
        await screen.findByTestId(ready);
        fireEvent.click(screen.getByTestId('saved-result-back'));
        await screen.findByTestId('project-runs-list');
      }
      fireEvent.click(screen.getByTestId('project-export-report'));
      fireEvent.click(screen.getByTestId('project-export-confirm'));
      // The browser build's export path is a Blob download; here it is the
      // desktop path, whose dialog plugin is not loaded in the test, so the
      // write fails, and says so.
      await waitFor(() => expect(screen.queryByTestId('project-export-pending')?.textContent ?? '').toBe(''));
      fireEvent.click(screen.getByTestId('project-page-close'));
      await screen.findByTestId('projects-panel');
      await settle();
    } finally {
      spies.forEach((s) => s.mockRestore());
      globalThis.fetch = realFetch;
      if (!hadIdb) delete (window as unknown as { indexedDB?: unknown }).indexedDB;
    }
    // The summary's header and label were on the user's own screen, as they
    // are on the live one; nowhere else.
    const all = [NAME.slice(0, 13), HEADER, LABEL];
    const said = consoleSeen.join('\n');
    for (const s of all) {
      expect(said, s).not.toContain(s);
      expect(document.title, s).not.toContain(s);
      expect(seen.join('\n'), s).not.toContain(s);
      expect(JSON.stringify(window.history.state ?? null), s).not.toContain(s);
      expect(window.location.href, s).not.toContain(s);
    }
    expect(document.title).toBe(titleBefore);
    expect(window.history.length).toBe(historyBefore);
    expect(pushState).not.toHaveBeenCalled();
    expect(replaceState).not.toHaveBeenCalled();
    expect(fetchSpy).not.toHaveBeenCalled();
    expect(xhrOpen).not.toHaveBeenCalled();
    expect(xhrSend).not.toHaveBeenCalled();
    expect(beacon).not.toHaveBeenCalled();
    expect(idbOpen).not.toHaveBeenCalled();
    expect(seen.length).toBeGreaterThan(1);
    expect(seen.every((s) => s.endsWith('null')), 'no router state').toBe(true);
    const stored = (s: Storage) => Array.from({ length: s.length }, (_, i) => `${s.key(i)}=${s.getItem(s.key(i) as string)}`).join('\n');
    for (const s of all) {
      expect(stored(localStorage), s).not.toContain(s);
      expect(stored(sessionStorage), s).not.toContain(s);
    }
    // And the name did reach the page, as text: the sweep saw real traffic.
    expect(projectCalls().some(([c]) => c === 'project_run_result')).toBe(true);
    expect(projectCalls().some(([c]) => c === 'project_report')).toBe(true);
    xhrOpen.mockRestore();
    xhrSend.mockRestore();
    pushState.mockRestore();
    replaceState.mockRestore();
  });
});
