// Open and an open project's page (Research OS, Phase 5B-3b; decisions 148
// and 149), over mock project and workspace bridges.
//
// * The panel: Open on each project that can be read, on its click only,
//   once for a double click, then the page; none on an unreadable project; a
//   refusal as its sentence.
// * The page: the open project (name in <bdi>, size, saved results newest
//   first, each with its kind, status, date and closed code); a listed
//   project that is not open, and Open from there; an unknown id and an id
//   that is not an id; an unreadable project; loading, empty and error
//   states; Close; Statistics not active, and going back after leaving it.
// * Under StrictMode: no open, close or save from an effect, and a
//   project_current that answers after an Open does not undo it; the same for
//   the Save dialog's read and a Save.
import React from 'react';
import { act, cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { makeMockWorkspaceBridge } from '../../workspace/workspaceBridge';
import type { WorkspaceStateView } from '../../workspace/workspaceTypes';
import { RUN_STATE_NAMES } from '../describeWording';
import { makeMockProjectBridge } from './projectBridge';
import { openProjectStore } from './projectSession';
import type { ProjectListEntryView, StoredRunView } from './projectTypes';
import { FALLBACK_SENTENCE, PROJECT_TEXT, PROJECT_WORDS, savedOn } from './projectWording';
import { workspaceSentence } from '../../workspace/workspaceWording';
import { ProjectPage } from './ProjectPage';
import { ProjectsPanel } from './ProjectsPanel';
import { SaveToProject } from './SaveToProject';

const flags = vi.hoisted(() => ({ projects: true, workspaces: true }));
vi.mock('../../../config/featureFlags', () => ({
  isFeatureEnabled: (name: string) =>
    (name === 'projects' && flags.projects) || (name === 'workspaces' && flags.workspaces) || name === 'statsDescribe',
}));

beforeEach(() => {
  flags.projects = true;
  flags.workspaces = true;
  openProjectStore.clear();
});
afterEach(cleanup);

const A = '0123456789abcdef0123456789abcdef';
const B = 'fedcba9876543210fedcba9876543210';
const ACTIVE: WorkspaceStateView = { state: { phase: 'active', workspace: 'statistics' }, activeRun: null, latestRun: null };
const NONE: WorkspaceStateView = { state: { phase: 'no_workspace' }, activeRun: null, latestRun: null };

const LIST: ProjectListEntryView[] = [
  { projectId: A, name: 'Chapter 3', bytes: 3 * 1048576, savedAt: 1792396351, runCount: 3, refused: null },
  { projectId: B, name: null, bytes: 4096, savedAt: null, runCount: null, refused: 'project.db_unreadable' },
];
const SIZES = { totalBytes: 3 * 1048576 + 4096, projectCount: 2, projectCapBytes: 104857600, allProjectsCapBytes: 10737418240 };
const RUNS: StoredRunView[] = [
  {
    storedRunId: 1,
    capabilityId: 'stats.describe',
    capabilityVersion: 1,
    state: 'succeeded',
    endCode: null,
    startedAt: 1792396000,
    endedAt: 1792396001,
    savedAt: 1792396100,
    agentVersion: '0.1.0',
  },
  {
    storedRunId: 2,
    capabilityId: 'stats.analyze.independent_t',
    capabilityVersion: 1,
    state: 'failed',
    endCode: 'project.run_interrupted',
    startedAt: 1792396200,
    endedAt: 1792396201,
    savedAt: 1792396300,
    agentVersion: '0.1.0',
  },
  {
    storedRunId: 3,
    capabilityId: 'stats.analyze.independent_t',
    capabilityVersion: 1,
    state: 'succeeded',
    endCode: null,
    startedAt: 1792396200,
    endedAt: 1792396201,
    savedAt: 1792396300,
    agentVersion: '0.1.0',
  },
];
const refusal = (v: unknown) => ({ code: 'validation', message: JSON.stringify(v) });

function deferred<T>() {
  let resolve!: (v: T) => void;
  const promise = new Promise<T>((res) => {
    resolve = res;
  });
  return { promise, resolve };
}

type Responses = Record<string, unknown>;

function pageBridges(responses: Responses = {}, state: WorkspaceStateView = ACTIVE) {
  const bridge = makeMockProjectBridge({
    project_current: { projectId: A },
    project_list: LIST,
    project_sizes: SIZES,
    project_runs: RUNS,
    project_open: ({ projectId }: Record<string, unknown>) => ({ projectId, name: 'Chapter 3', createdAt: 1792396000, runCount: 3, bytes: 3 * 1048576 }),
    project_close: { closed: true },
    ...responses,
  });
  const workspace = makeMockWorkspaceBridge({ workspace_state: state });
  return { bridge, workspace };
}

function page(id = A, responses: Responses = {}, state: WorkspaceStateView = ACTIVE, strict = false) {
  const { bridge, workspace } = pageBridges(responses, state);
  const onClosed = vi.fn();
  const onBack = vi.fn();
  const ui = <ProjectPage projectId={id} bridge={bridge} workspace={workspace} desktop onClosed={onClosed} onBack={onBack} />;
  const view = render(strict ? <React.StrictMode>{ui}</React.StrictMode> : ui);
  return { bridge, workspace, onClosed, onBack, view };
}

const commands = (b: ReturnType<typeof makeMockProjectBridge>, cmd: string) => b.calls.filter(([c]) => c === cmd);
const ACTIONS = ['project_open', 'project_close', 'project_save_run', 'project_delete', 'project_prepare_delete'];

/* ---- Open on the panel -------------------------------------------------------- */

describe('Open on the Projects panel (decision 149)', () => {
  function panel(responses: Responses = {}, strict = false) {
    const { bridge, workspace } = pageBridges({ project_current: { projectId: null }, ...responses });
    const onOpened = vi.fn();
    const ui = <ProjectsPanel bridge={bridge} workspace={workspace} desktop onOpened={onOpened} />;
    render(strict ? <React.StrictMode>{ui}</React.StrictMode> : ui);
    return { bridge, onOpened };
  }

  it('is on a project that can be read and not on one that cannot', async () => {
    panel();
    expect((await screen.findByTestId(`project-open-button-${A}`)).textContent).toBe(PROJECT_TEXT.open);
    expect(screen.queryByTestId(`project-open-button-${B}`)).toBeNull();
  });

  it('opens on its click only, then goes to the page by the id', async () => {
    const { bridge, onOpened } = panel();
    await screen.findByTestId(`project-open-button-${A}`);
    expect(commands(bridge, 'project_open')).toHaveLength(0);
    fireEvent.click(screen.getByTestId(`project-open-button-${A}`));
    await waitFor(() => expect(onOpened).toHaveBeenCalledWith(A));
    expect(commands(bridge, 'project_open')).toEqual([['project_open', { projectId: A }]]);
    expect(openProjectStore.get()).toEqual({ projectId: A, name: 'Chapter 3' });
  });

  it('makes one call for two clicks in one event turn, with Open disabled while pending, under StrictMode', async () => {
    const d = deferred<unknown>();
    const { bridge, onOpened } = panel({ project_open: () => d.promise }, true);
    const b = await screen.findByTestId(`project-open-button-${A}`);
    act(() => {
      b.click();
      b.click();
    });
    expect((screen.getByTestId(`project-open-button-${A}`) as HTMLButtonElement).disabled).toBe(true);
    expect(screen.getByTestId(`project-open-button-${A}`).textContent).toBe(PROJECT_TEXT.opening);
    await act(async () => d.resolve({ projectId: A, name: 'Chapter 3', createdAt: 1, runCount: 3, bytes: 1 }));
    expect(commands(bridge, 'project_open')).toHaveLength(1);
    expect(onOpened).toHaveBeenCalledTimes(1);
    expect(bridge.calls.filter(([c]) => ACTIONS.indexOf(c) >= 0 && c !== 'project_open')).toEqual([]);
  });

  it('shows a refusal as its sentence, and goes nowhere', async () => {
    const { onOpened } = panel({
      project_open: () => {
        throw refusal({ code: 'project.being_deleted' });
      },
    });
    fireEvent.click(await screen.findByTestId(`project-open-button-${A}`));
    await waitFor(() =>
      expect(screen.getByTestId('projects-message').textContent).toBe(PROJECT_WORDS['project.being_deleted']({ code: 'project.being_deleted' }))
    );
    expect(onOpened).not.toHaveBeenCalled();
    expect((screen.getByTestId(`project-open-button-${A}`) as HTMLButtonElement).disabled).toBe(false);
  });
});

/* ---- the page ----------------------------------------------------------------- */

describe('the page of the open project', () => {
  it('shows its name as text in <bdi>, its size, and its saved results newest first with kind, status, date and code', async () => {
    const { bridge } = page();
    const name = await screen.findByTestId('project-page-name');
    expect(name.tagName).toBe('BDI');
    expect(name.textContent).toBe('Chapter 3');
    expect(screen.getByTestId('project-page-size').textContent).toBe('Size: 3 MiB.');
    const list = await screen.findByTestId('project-runs-list');
    const rows = within(list).getAllByRole('listitem');
    expect(rows.map((r) => r.getAttribute('data-testid'))).toEqual(['project-run-3', 'project-run-2', 'project-run-1']);
    expect(screen.getByTestId('project-run-facts-1').textContent).toBe(
      `Descriptive summary. ${RUN_STATE_NAMES.succeeded}. Saved ${savedOn(1792396100)}.`
    );
    expect(screen.getByTestId('project-run-facts-2').textContent).toBe(
      `Independent-samples t-test. ${RUN_STATE_NAMES.failed}. Saved ${savedOn(1792396300)}. ${PROJECT_WORDS['project.run_interrupted']({
        code: 'project.run_interrupted',
      })}`
    );
    expect(commands(bridge, 'project_runs')).toEqual([['project_runs', { projectId: A }]]);
    expect(bridge.calls.filter(([c]) => ACTIONS.indexOf(c) >= 0)).toEqual([]);
  });

  it('has a loading, an empty and an error state for its saved results', async () => {
    const d = deferred<unknown>();
    page(A, { project_runs: () => d.promise });
    expect((await screen.findByTestId('project-runs-loading')).textContent).toBe(PROJECT_TEXT.runsLoading);
    await act(async () => d.resolve([]));
    expect((await screen.findByTestId('project-runs-empty')).textContent).toBe(PROJECT_TEXT.runsEmpty);
    cleanup();
    page(A, {
      project_runs: () => {
        throw new Error('RAW_SENTINEL_r1');
      },
    });
    expect((await screen.findByTestId('project-runs-unread')).textContent).toBe(PROJECT_TEXT.runsUnread);
    expect(document.body.innerHTML).not.toContain('RAW_SENTINEL');
  });

  it('has a loading and an error state, with Try again', async () => {
    const d = deferred<unknown>();
    const { bridge } = page(A, { project_list: () => d.promise });
    expect(screen.getByTestId('project-page-loading').textContent).toBe(PROJECT_TEXT.pageLoading);
    await act(async () => d.resolve(LIST));
    await screen.findByTestId('project-page-name');
    cleanup();
    let fail = true;
    const second = page(A, {
      project_list: () => {
        if (fail) throw new Error('RAW_SENTINEL_l1');
        return LIST;
      },
    });
    expect((await screen.findByTestId('project-page-unread')).textContent).toBe(PROJECT_TEXT.pageUnread);
    expect(document.body.innerHTML).not.toContain('RAW_SENTINEL');
    fail = false;
    fireEvent.click(screen.getByTestId('project-page-retry'));
    await screen.findByTestId('project-page-name');
    expect(commands(second.bridge, 'project_list')).toHaveLength(2);
    expect(bridge.calls.filter(([c]) => ACTIONS.indexOf(c) >= 0)).toEqual([]);
  });

  it('Close calls project_close on its click, once, and returns to the Statistics home', async () => {
    const d = deferred<unknown>();
    const { bridge, onClosed } = page(A, { project_close: () => d.promise });
    const b = await screen.findByTestId('project-page-close');
    act(() => {
      b.click();
      b.click();
    });
    expect((screen.getByTestId('project-page-close') as HTMLButtonElement).disabled).toBe(true);
    expect(screen.getByTestId('project-page-close').textContent).toBe(PROJECT_TEXT.closing);
    await act(async () => d.resolve({ closed: true }));
    expect(commands(bridge, 'project_close')).toEqual([['project_close', {}]]);
    expect(onClosed).toHaveBeenCalledTimes(1);
    expect(openProjectStore.get()).toBeNull();
  });

  it('shows a refusal of Close as its sentence and stays', async () => {
    // Rust's workspace refusal, in its diagnostic shape.
    const inactive = { code: 'workspace.inactive', params: { expected: 'statistics' }, severity: 'error' };
    const { onClosed } = page(A, {
      project_close: () => {
        throw refusal(inactive);
      },
    });
    fireEvent.click(await screen.findByTestId('project-page-close'));
    await waitFor(() => expect(screen.getByTestId('project-page-message').textContent).toBe(workspaceSentence(inactive as never)));
    expect(onClosed).not.toHaveBeenCalled();
    expect(screen.getByTestId('project-page-name').textContent).toBe('Chapter 3');
  });

  it('a workspace refusal without its parameters gives the fallback sentence, never a throw', async () => {
    const { onClosed } = page(A, {
      project_close: () => {
        throw refusal({ code: 'workspace.inactive' });
      },
    });
    fireEvent.click(await screen.findByTestId('project-page-close'));
    await waitFor(() => expect(screen.getByTestId('project-page-message').textContent).toBe(FALLBACK_SENTENCE));
    expect(onClosed).not.toHaveBeenCalled();
  });
});

describe('the page of a project that is not open', () => {
  it('says so, offers Open and never opens by itself', async () => {
    const { bridge } = page(A, { project_current: { projectId: null } }, ACTIVE, true);
    expect((await screen.findByTestId('project-page-not-open')).textContent).toBe(PROJECT_TEXT.pageNotOpen);
    expect(screen.getByTestId('project-page-name').textContent).toBe('Chapter 3');
    expect(screen.getByTestId('project-page-open')).toBeTruthy();
    expect(screen.queryByTestId('project-runs')).toBeNull();
    expect(commands(bridge, 'project_runs')).toEqual([]);
    expect(bridge.calls.filter(([c]) => ACTIONS.indexOf(c) >= 0)).toEqual([]);
  });

  it('Open from the page opens it on the click, then shows its saved results', async () => {
    let open = false;
    const { bridge } = page(A, {
      project_current: () => ({ projectId: open ? A : null }),
      project_open: ({ projectId }: Record<string, unknown>) => {
        open = true;
        return { projectId, name: 'Chapter 3', createdAt: 1, runCount: 3, bytes: 1 };
      },
    });
    fireEvent.click(await screen.findByTestId('project-page-open'));
    await screen.findByTestId('project-runs-list');
    expect(commands(bridge, 'project_open')).toEqual([['project_open', { projectId: A }]]);
    expect(openProjectStore.get()).toEqual({ projectId: A, name: 'Chapter 3' });
  });

  it('shows a refusal of Open as its sentence', async () => {
    page(A, {
      project_current: { projectId: null },
      project_open: () => {
        throw refusal({ code: 'project.being_deleted' });
      },
    });
    fireEvent.click(await screen.findByTestId('project-page-open'));
    await waitFor(() =>
      expect(screen.getByTestId('project-page-message').textContent).toBe(
        PROJECT_WORDS['project.being_deleted']({ code: 'project.being_deleted' })
      )
    );
    expect(screen.getByTestId('project-page-not-open')).toBeTruthy();
  });

  it('a project_current that answers after an Open does not undo it (StrictMode)', async () => {
    // StrictMode runs the page's reads twice; the first run's project_current
    // is held back and answers "none" only after the user has opened the
    // project from the second run's page.
    const late = deferred<unknown>();
    let n = 0;
    let open = false;
    page(
      A,
      {
        project_current: () => {
          n += 1;
          return n === 1 ? late.promise : { projectId: open ? A : null };
        },
        project_open: ({ projectId }: Record<string, unknown>) => {
          open = true;
          return { projectId, name: 'Chapter 3', createdAt: 1, runCount: 3, bytes: 1 };
        },
      },
      ACTIVE,
      true
    );
    fireEvent.click(await screen.findByTestId('project-page-open'));
    await screen.findByTestId('project-runs-list');
    await act(async () => late.resolve({ projectId: null }));
    expect(screen.getByTestId('project-runs-list')).toBeTruthy();
    expect(screen.queryByTestId('project-page-not-open')).toBeNull();
    expect(openProjectStore.get()).toEqual({ projectId: A, name: 'Chapter 3' });
  });
});

describe('a page with nothing to show of a project', () => {
  it('an id that is not listed: a sentence, no Open, no read of runs', async () => {
    const { bridge } = page('aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa');
    expect((await screen.findByTestId('project-page-not-found')).textContent).toBe(PROJECT_TEXT.pageNotFound);
    expect(screen.queryByTestId('project-page-open')).toBeNull();
    expect(commands(bridge, 'project_runs')).toEqual([]);
  });

  it('an id that is not an id: the same sentence, and the id is never sent', async () => {
    const { bridge } = page('../x');
    await screen.findByTestId('project-page-not-found');
    expect(bridge.calls.some(([, a]) => JSON.stringify(a).includes('../x'))).toBe(false);
  });

  it('a project that cannot be read: its sentence, no Open', async () => {
    page(B, { project_current: { projectId: null } });
    expect((await screen.findByTestId('project-page-unreadable')).textContent).toBe(
      `${PROJECT_TEXT.unreadable} ${PROJECT_WORDS['project.db_unreadable']({ code: 'project.db_unreadable' })}`
    );
    expect(screen.queryByTestId('project-page-open')).toBeNull();
  });

  it('Statistics not active: a sentence and nothing of the project', async () => {
    openProjectStore.set({ projectId: A, name: 'Chapter 3' });
    const { bridge, onBack } = page(A, {}, NONE);
    expect((await screen.findByTestId('project-page-inactive')).textContent).toBe(PROJECT_TEXT.pageInactive);
    expect(document.body.textContent).not.toContain('Chapter 3');
    expect(commands(bridge, 'project_runs')).toEqual([]);
    expect(openProjectStore.get()).toBeNull();
    fireEvent.click(screen.getByTestId('project-page-back'));
    expect(onBack).toHaveBeenCalledTimes(1);
  });

  it('leaving Statistics ends the page, and going back to it shows nothing of the project', async () => {
    page();
    await screen.findByTestId('project-runs-list');
    cleanup();
    // Rust closed the project when Statistics was left (decision 135).
    page(A, { project_current: { projectId: null } }, NONE);
    await screen.findByTestId('project-page-inactive');
    expect(document.body.textContent).not.toContain('Chapter 3');
    expect(screen.queryByTestId('project-runs')).toBeNull();
  });

  it('is not there when the helper says no, and calls nothing', () => {
    flags.workspaces = false;
    const { bridge, workspace } = page();
    expect(screen.queryByTestId('project-page')).toBeNull();
    expect(bridge.calls).toEqual([]);
    expect(workspace.calls).toEqual([]);
  });
});

describe('the Save dialog and a late answer', () => {
  it("a project_current that answers after a Save does not undo the Save's open project", async () => {
    const late = deferred<unknown>();
    const bridge = makeMockProjectBridge({
      project_current: () => late.promise,
      project_list: [],
      project_save_run: { projectId: A, storedRunId: 1, created: true },
    });
    render(<SaveToProject runId={5} bridge={bridge} desktop />);
    fireEvent.click(screen.getByTestId('project-save-button'));
    fireEvent.change(screen.getByTestId('project-save-name'), { target: { value: 'Chapter 3' } });
    fireEvent.click(screen.getByTestId('project-save-confirm'));
    await waitFor(() => expect(openProjectStore.get()).toEqual({ projectId: A, name: 'Chapter 3' }));
    await act(async () => late.resolve({ projectId: null }));
    expect(openProjectStore.get()).toEqual({ projectId: A, name: 'Chapter 3' });
  });
});
