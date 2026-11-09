// A project's recipes on its page (Research OS, Phase 8U-3, decisions 554 and
// 555), over mock bridges.
//
// * project_recipes is read after each runs read that finds a prepared run,
//   under its own liveness flag, never the page's counter; never with none.
// * The section after the saved results, a line per recipe; each prepared
//   row's scores, in an element of its own; the unread forms, for a refusal
//   and for an answer that is not a list; a run under two recipes named by
//   none.
// * A saved result opened while the recipes are read gets its names when
//   they answer.
// * The names are text only: a sweep with a sentinel finds them where these
//   decisions put them, and in no attribute.
import fs from 'node:fs';
import path from 'node:path';
import React from 'react';
import { act, cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { makeMockWorkspaceBridge } from '../../workspace/workspaceBridge';
import type { WorkspaceStateView } from '../../workspace/workspaceTypes';
import { makeMockDescribeBridge } from '../describeBridge';
import { makeMockTTestBridge } from '../ttestBridge';
import { makeMockProjectBridge } from './projectBridge';
import { openProjectStore } from './projectSession';
import type { ProjectCurrentView, RecipeView, RerunView, StoredRunView } from './projectTypes';
import {
  RECIPES_HEADING,
  RECIPES_UNREAD,
  recipeLine,
  recipeUnreadLine,
  RUN_SCORES_UNREAD,
  runScoresLine,
  scoreColumn,
} from './projectWording';
import { ProjectPage } from './ProjectPage';

vi.mock('../../../config/featureFlags', () => ({
  isFeatureEnabled: (name: string) => name === 'projects' || name === 'workspaces' || name === 'statsDescribe',
}));

beforeEach(() => openProjectStore.clear());
afterEach(cleanup);

const A = '0123456789abcdef0123456789abcdef';
const ACTIVE: WorkspaceStateView = { state: { phase: 'active', workspace: 'statistics' }, activeRun: null, latestRun: null };
const STORED = path.join(__dirname, '../../../../src-tauri/tests/fixtures/projects/stored');
const stored = (name: string) => JSON.parse(fs.readFileSync(path.join(STORED, name), 'utf8')) as unknown;

function storedRun(id: number, capabilityId: string, prepared: boolean, over: Partial<StoredRunView> = {}): StoredRunView {
  return {
    storedRunId: id,
    capabilityId,
    capabilityVersion: 1,
    state: 'succeeded',
    endCode: null,
    startedAt: 1792396000 + id * 10,
    endedAt: 1792396001 + id * 10,
    savedAt: 1792396002 + id * 10,
    agentVersion: '0.1.0',
    rerunOf: null,
    comparison: null,
    engineVersionDiffers: null,
    ...(prepared ? { prepared: true as const } : {}),
    ...over,
  };
}

/** Alpha (1) on the file's own table; a summary (2) on recipe a; the
 *  correlations (3) listed under a and b; a t-test (5) on recipe b. */
const RUNS: StoredRunView[] = [
  storedRun(1, 'stats.analyze.cronbach_alpha', false),
  storedRun(2, 'stats.describe', true),
  storedRun(3, 'stats.analyze.correlations', true),
  storedRun(5, 'stats.analyze.independent_t', true),
];

const recipesWith = (name: string): RecipeView[] => [
  { usedBy: [2, 3], sourceColumns: 2, scores: [{ position: 3, name, statistic: 'mean', items: 2 }] },
  {
    usedBy: [3, 5],
    sourceColumns: 4,
    scores: [
      { position: 5, name: 'Total', statistic: 'sum', items: 4 },
      { position: 6, name: 'Middle', statistic: null, items: 3 },
    ],
  },
];
const RECIPES = recipesWith('Average');

interface World {
  current: ProjectCurrentView | (() => ProjectCurrentView);
  runs: StoredRunView[];
  recipes: () => unknown;
}

function page(w: World) {
  const bridge = makeMockProjectBridge({
    project_current: () => (typeof w.current === 'function' ? w.current() : w.current),
    project_list: [{ projectId: A, name: 'Chapter 3', bytes: 40000, savedAt: 1792396351, runCount: 4, refused: null }],
    project_sizes: { totalBytes: 40000, projectCount: 1, projectCapBytes: 104857600, allProjectsCapBytes: 10737418240 },
    project_runs: () => w.runs,
    project_recipes: () => w.recipes(),
    project_run_result: ({ storedRunId }: Record<string, unknown>) =>
      stored(storedRunId === 2 ? 'summary_prepared.json' : storedRunId === 3 ? 'corr_pairwise.json' : 'ttest.json'),
  });
  render(
    <ProjectPage
      projectId={A}
      bridge={bridge}
      workspace={makeMockWorkspaceBridge({ workspace_state: ACTIVE })}
      describe={makeMockDescribeBridge({ stats_desc_cancel: { runId: 0 } })}
      ttest={makeMockTTestBridge({ stats_ttest_cancel: { runId: 0 } })}
      desktop
      save={async () => null}
      pollMs={5}
      onClosed={vi.fn()}
      onBack={vi.fn()}
    />
  );
  return bridge;
}

const world = (over: Partial<World> = {}): World => ({
  current: { projectId: A, rerun: null },
  runs: RUNS,
  recipes: () => RECIPES,
  ...over,
});
const names = (b: { calls: Array<[string, Record<string, unknown>]> }) => b.calls.map(([c]) => c);
const wait = (ms: number) => act(async () => new Promise((r) => setTimeout(r, ms)));
const scoresOf = (id: number) => screen.queryByTestId(`project-run-scores-${id}`)?.textContent ?? null;

function deferred<T>() {
  let resolve!: (v: T) => void;
  let reject!: (e: unknown) => void;
  const promise = new Promise<T>((res, rej) => {
    resolve = res;
    reject = rej;
  });
  return { promise, resolve, reject };
}

describe('the page reads the recipes', () => {
  it('once, after the runs, when a prepared run is listed', async () => {
    const b = page(world());
    await screen.findByTestId('project-recipes');
    await wait(20);
    expect(names(b)).toEqual(['project_current', 'project_list', 'project_sizes', 'project_runs', 'project_recipes']);
    expect(b.calls.at(-1)?.[1]).toEqual({ projectId: A });
  });

  it('never, when no run is prepared', async () => {
    const b = page(world({ runs: [storedRun(1, 'stats.analyze.cronbach_alpha', false), storedRun(2, 'stats.describe', false)] }));
    await screen.findByTestId('project-runs-list');
    await wait(30);
    expect(names(b)).not.toContain('project_recipes');
    expect(screen.queryByTestId('project-recipes')).toBeNull();
    expect(screen.queryByTestId('project-run-scores-2')).toBeNull();
  });

  it('again after a rerun is saved and the runs are read again', async () => {
    let n = 0;
    const going: RerunView = { runId: 7, rerunOf: 2, status: 'going', storedRunId: null, code: null };
    const w = world({
      current: () => {
        n += 1;
        return { projectId: A, rerun: n < 3 ? going : { ...going, status: 'saved', storedRunId: 6 } };
      },
    });
    const b = page(w);
    await screen.findByTestId('project-recipes');
    w.runs = [...RUNS, storedRun(6, 'stats.describe', true, { rerunOf: 2, comparison: 'identical', engineVersionDiffers: false })];
    w.recipes = () => [{ ...RECIPES[0], usedBy: [2, 3, 6] }, RECIPES[1]];
    await screen.findByTestId('project-rerun-outcome');
    await waitFor(() => expect(names(b).filter((c) => c === 'project_recipes')).toHaveLength(2));
    await waitFor(() => expect(scoresOf(6)).toBe(runScoresLine(RECIPES[0].scores!)));
    expect(screen.getByTestId('project-recipe-0').textContent).toBe(recipeLine(3, RECIPES[0].scores!));
  });
});

describe("the page reads the recipes after Part 3's reads too", () => {
  it('when it follows another rerun of this project, and reads the runs again', async () => {
    let next: RerunView = { runId: 7, rerunOf: 2, status: 'going', storedRunId: null, code: null };
    const b = page(world({ current: () => ({ projectId: A, rerun: next }) }));
    await screen.findByTestId('project-recipes');
    expect(names(b).filter((c) => c === 'project_recipes')).toHaveLength(1);
    next = { runId: 8, rerunOf: 3, status: 'going', storedRunId: null, code: null };
    await waitFor(() => expect(names(b).filter((c) => c === 'project_recipes')).toHaveLength(2));
  });
});

describe('what the page shows of them', () => {
  it("the section after the saved results, and each prepared row's scores", async () => {
    page(world());
    const section = await screen.findByTestId('project-recipes');
    expect(within(section).getByRole('heading').textContent).toBe(RECIPES_HEADING);
    expect(within(section).getAllByRole('listitem').map((l) => l.textContent)).toEqual([
      recipeLine(2, RECIPES[0].scores!),
      recipeLine(2, RECIPES[1].scores!),
    ]);
    // After the saved results, in the page's order.
    const runs = screen.getByTestId('project-runs');
    expect(runs.compareDocumentPosition(section) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
    expect(scoresOf(2)).toBe('Scale scores: C3 "Average".');
    // Listed under two recipes: named by none.
    expect(scoresOf(3)).toBe(RUN_SCORES_UNREAD);
    expect(scoresOf(5)).toBe(runScoresLine(RECIPES[1].scores!));
    // Not prepared: no element at all.
    expect(scoresOf(1)).toBeNull();
    // The row's facts are untouched, its scores in an element of their own.
    expect(screen.getByTestId('project-run-facts-2').textContent).not.toContain('Scale scores');
  });

  it('a recipe whose scores could not be read: its own line, and its run unread', async () => {
    page(world({ recipes: () => [{ usedBy: [2, 3], sourceColumns: null, scores: null }, RECIPES[1]] }));
    const section = await screen.findByTestId('project-recipes');
    expect(within(section).getAllByRole('listitem').map((l) => l.textContent)).toEqual([
      recipeUnreadLine(2),
      recipeLine(2, RECIPES[1].scores!),
    ]);
    expect(scoresOf(2)).toBe(RUN_SCORES_UNREAD);
    expect(scoresOf(5)).toBe(runScoresLine(RECIPES[1].scores!));
  });

  it.each([
    ['a refusal', () => {
      throw { code: 'validation', message: JSON.stringify({ code: 'project.db_failed' }) };
    }],
    ['an answer that is not a list', () => null],
  ])('%s: the unread forms', async (_n, recipes) => {
    page(world({ recipes }));
    const unread = await screen.findByTestId('project-recipes-unread');
    expect(unread.textContent).toBe(RECIPES_UNREAD);
    expect(screen.queryAllByTestId(/^project-recipe-\d+$/)).toEqual([]);
    expect(scoresOf(2)).toBe(RUN_SCORES_UNREAD);
    expect(scoresOf(3)).toBe(RUN_SCORES_UNREAD);
    expect(scoresOf(5)).toBe(RUN_SCORES_UNREAD);
    expect(scoresOf(1)).toBeNull();
  });

  it('while they are read, nothing new', async () => {
    const d = deferred<RecipeView[]>();
    page(world({ recipes: () => d.promise }));
    await screen.findByTestId('project-runs-list');
    await wait(20);
    expect(screen.queryByTestId('project-recipes')).toBeNull();
    expect(screen.queryByTestId('project-recipes-unread')).toBeNull();
    expect(scoresOf(2)).toBeNull();
    await act(async () => d.resolve(RECIPES));
    expect(scoresOf(2)).toBe(runScoresLine(RECIPES[0].scores!));
  });
});

describe('a saved result opened while the recipes are read', () => {
  it('gets its names when they answer, and the section hides with the list', async () => {
    const d = deferred<RecipeView[]>();
    page(world({ recipes: () => d.promise }));
    fireEvent.click(await screen.findByTestId('project-run-open-2'));
    const table = await screen.findByTestId('desc-numeric-table');
    expect(within(table).getAllByRole('rowheader').map((h) => h.textContent)).toEqual(['C3']);
    await act(async () => d.resolve(RECIPES));
    await waitFor(() =>
      expect(within(screen.getByTestId('desc-numeric-table')).getAllByRole('rowheader').map((h) => h.textContent)).toEqual([
        scoreColumn(3, 'Average'),
      ])
    );
    expect(screen.queryByTestId('project-recipes')).toBeNull();
    fireEvent.click(screen.getByTestId('saved-result-back'));
    expect(await screen.findByTestId('project-recipes')).toBeTruthy();
  });

  it('a run under two recipes opens with its columns by position alone', async () => {
    page(world());
    await screen.findByTestId('project-recipes');
    fireEvent.click(screen.getByTestId('project-run-open-3'));
    await screen.findByTestId('cr-descriptives');
    const text = screen.getByTestId('saved-result').textContent ?? '';
    for (const n of ['Average', 'Total', 'Middle']) expect(text).not.toContain(n);
    expect(within(screen.getByTestId('cr-descriptives')).getAllByRole('rowheader').map((h) => h.textContent)).toEqual(['C1', 'C2', 'C3']);
  });
});

describe('the names are text the user typed', () => {
  it('a sentinel shows in the section, its rows and an open saved result, and in no attribute', async () => {
    const NAME = 'NAME_SENTINEL_p71';
    page(world({ recipes: () => recipesWith(NAME) }));
    const section = await screen.findByTestId('project-recipes');
    const where = () => {
      const page = screen.getByTestId('project-page');
      const attrs = Array.from(page.querySelectorAll('*')).flatMap((el) => Array.from(el.attributes).map((a) => `${a.name}=${a.value}`));
      expect(attrs.filter((a) => a.includes(NAME))).toEqual([]);
      // Every text node that holds it, by the nearest element with a test id.
      const walker = document.createTreeWalker(page, NodeFilter.SHOW_TEXT);
      const owners: string[] = [];
      for (let n = walker.nextNode(); n; n = walker.nextNode()) {
        if (n.textContent?.includes(NAME)) owners.push(n.parentElement?.closest('[data-testid]')?.getAttribute('data-testid') ?? '?');
      }
      return owners;
    };
    expect(section.textContent).toContain(NAME);
    expect(where()).toEqual(['project-run-scores-2', 'project-recipe-0']);
    fireEvent.click(screen.getByTestId('project-run-open-2'));
    await waitFor(() => expect(screen.getByTestId('desc-numeric-table').textContent).toContain(NAME));
    expect(where().every((t) => t === 'desc-numeric-table')).toBe(true);
    expect(where().length).toBeGreaterThan(0);
  });
});
