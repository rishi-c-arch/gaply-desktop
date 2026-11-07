// The Statistics window, wired (Research OS, Phase 8P, decisions 449 to 453;
// ADR-012 Parts 1, 2, 4 and 5): regions, tabs, Open and Close from the menu, the
// workspace's state, the project's line, and the session's life across a route
// and a Switch. Every bridge is a mock; nothing touches Rust, a disk or a network.
import fs from 'node:fs';
import path from 'node:path';
import React from 'react';
import { MemoryRouter, Route, Routes, useNavigate } from 'react-router-dom';
import { act, cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { makeMockWorkspaceBridge } from '../../workspace/workspaceBridge';
import type { WorkspaceStateView } from '../../workspace/workspaceTypes';
import { makeMockDescribeBridge } from '../describeBridge';
import type { DatasetView } from '../describeTypes';
import limits8 from '../fixtures/methodLimitsView8gb.json';
import preparedFixture from '../fixtures/datasetViewPrepared.json';
import { limitsSentence } from '../flowParts';
import type { MethodLimitsView } from '../describeTypes';
import { METHODS } from '../methods';
import { makeMockProjectBridge, PROJECT_COMMANDS as C } from '../projects/projectBridge';
import { openProjectStore } from '../projects/projectSession';
import { makeMockCapabilityBridge } from '../ttestBridge';
import type { CapabilityView } from '../ttestTypes';
import { DATA_EXTENSIONS, StatisticsWindow } from './StatisticsWindow';
import { SLOT_GONE, statisticsSession } from './statisticsSession';
import { SCALE_SCORE_CAPABILITY } from './windowMenus';
import { LATER_STEP, NOT_RUNNABLE, NO_FILE, RUN_GOING } from './windowWording';

const flags = vi.hoisted(() => ({ statsCheck: true, workspaces: true, projects: true }));
vi.mock('../../../config/featureFlags', () => ({
  isFeatureEnabled: (name: string) => (name === 'statsCheck' && flags.statsCheck) || (name === 'workspaces' && flags.workspaces) || (name === 'projects' && flags.projects),
}));
vi.mock('../../../utils/isTauri', () => ({ isTauri: true, default: true }));
vi.mock('../../../design-system/GaplyGlobe', () => ({ GaplyGlobe: () => <div /> }));

afterEach(() => {
  cleanup();
  statisticsSession.clear();
  openProjectStore.clear();
  flags.statsCheck = true;
  flags.workspaces = true;
  flags.projects = true;
});

const ACTIVE: WorkspaceStateView = { state: { phase: 'active', workspace: 'statistics' }, activeRun: null, latestRun: null };
const GOING: WorkspaceStateView = { ...ACTIVE, activeRun: { runId: 5, kind: 'describe' }, latestRun: { runId: 5, kind: 'describe' } };
const NONE: WorkspaceStateView = { state: { phase: 'no_workspace' }, activeRun: null, latestRun: null };

const capability = (id: string, runnable = true): CapabilityView => ({
  id,
  version: 1,
  lifecycle: 'beta',
  titleCode: 'stats.capability.describe.title',
  descriptionCode: 'stats.capability.describe.description',
  licenseNotes: [],
  knownLimitations: [],
  runnable,
});
const ALL = [...METHODS.map((m) => capability(m.capabilityId)), capability(SCALE_SCORE_CAPABILITY)];

const fileView = (slot: number, name = 'survey.csv'): DatasetView => ({
  slot,
  displayName: name,
  profile: { rows: 250, columns: [{ alias: 'C1', header: 'age', kind: 'numeric', n: 250, missing: 0, distinct: 40, levels: null, suspectMissingCodes: [], nonFinite: 0 }] },
  inputRef: { contentHash: 'ab'.repeat(32), rows: 250, columns: 1, semanticRole: 'table' },
});

interface Over {
  state?: () => WorkspaceStateView;
  deactivate?: unknown;
  files?: Record<string, unknown>;
  capabilities?: CapabilityView[] | (() => never);
  projects?: Record<string, unknown>;
  pick?: () => Promise<string | null>;
  limits?: () => Promise<unknown>;
  onRowsRead?: number;
}

function setUp(o: Over = {}, entries: string[] = ['/w']) {
  const workspace = makeMockWorkspaceBridge({ workspace_state: o.state ?? ACTIVE, workspace_deactivate: o.deactivate ?? NONE });
  const files = makeMockDescribeBridge({ stats_desc_open: fileView(1), stats_desc_close: true, ...o.files });
  // Decision 460: the grid's reads are its own, so files' recorded calls do not change.
  const cells = makeMockDescribeBridge({
    stats_desc_cells: (a: Record<string, unknown>) => ({ slot: a.slot, inputHash: 'ab'.repeat(32), rows: 250, columns: 1, firstRow: a.firstRow, firstColumn: a.firstColumn, cells: [] }),
  });
  const capabilities = makeMockCapabilityBridge({ capability_list: o.capabilities ?? ALL });
  const projects = makeMockProjectBridge(o.projects ?? { [C.current]: { projectId: null, rerun: null }, [C.list]: [] });
  const pick = o.pick ?? (() => Promise.resolve('/x/survey.csv'));
  const limits = o.limits ?? (() => Promise.resolve(limits8));
  const Nav: React.FC = () => {
    const navigate = useNavigate();
    return <button data-testid="back-to-window" onClick={() => navigate('/w')} />;
  };
  const ui = (
    <MemoryRouter initialEntries={entries}>
      <Routes>
        <Route path="/w" element={<StatisticsWindow workspace={workspace} files={files} cells={cells} capabilities={capabilities} projects={projects} pickPath={pick} limits={limits} pollMs={15} />} />
        <Route path="/app" element={<div data-testid="home-here"><Nav /></div>} />
        <Route path="/app/settings" element={<div data-testid="settings-here" />} />
        <Route path="/app/statsverifier" element={<div data-testid="verifier-here" />} />
        <Route path="/app/check/stats" element={<div data-testid="check-here" />} />
        <Route path="/app/workspaces" element={<div data-testid="chooser-here" />} />
      </Routes>
    </MemoryRouter>
  );
  const rendered = render(ui);
  return { workspace, files, capabilities, projects, rendered, commands: () => files.calls.map(([c]) => c) };
}

const openMenu = (id: string) => fireEvent.click(screen.getByTestId(`gds-menu-${id}`));
const choose = (menu: string, item: string) => {
  openMenu(menu);
  fireEvent.click(screen.getByTestId(`gds-menu-item-${item}`));
};
const description = (item: string) => {
  const el = screen.getByTestId(`gds-menu-item-${item}`);
  const id = el.getAttribute('aria-describedby');
  return id ? document.getElementById(id)?.textContent : undefined;
};
const tab = (name: RegExp) => fireEvent.click(screen.getByRole('tab', { name }));
const opened = async (view = fileView(1)) => {
  act(() => statisticsSession.setFile(view));
  await screen.findByTestId('sw-file-summary');
};

describe('the regions', () => {
  it('holds the top bar, the tabs, the line, the body and the footer, and sits in no shell', async () => {
    setUp();
    expect(await screen.findByTestId('statistics-window')).toBeTruthy();
    expect(screen.getByRole('heading', { name: 'Statistics' })).toBeTruthy();
    expect(screen.getByRole('menubar', { name: 'Statistics menus' })).toBeTruthy();
    expect(screen.getAllByRole('tab').map((t) => t.textContent)).toEqual(['Data View', 'Variable View', 'Output (0)']);
    expect(screen.getByTestId('sw-line')).toBeTruthy();
    expect(screen.getByTestId('sw-body')).toBeTruthy();
    expect(Array.from(screen.getByTestId('sw-foot').children).map((c) => c.textContent)).toEqual([
      'Statistics workspace',
      'Read-only grid: your file is never changed',
      'Everything stays on this computer',
    ]);
    expect(document.querySelector('.gds-shell')).toBeNull();
    expect(document.querySelector('.gds-rail')).toBeNull();
    expect(screen.getByTestId('sw-top').textContent).toContain('Gaply');
    expect(screen.getByTestId('sw-top').textContent).toContain('Statistics');
  });
});

describe('the line above the grid', () => {
  it('with no file: the permanent line alone', async () => {
    setUp();
    expect((await screen.findByTestId('sw-as-read')).textContent).toBe('As Gaply read it: numbers in full, missing cells empty, no value labels.');
    expect(screen.queryByTestId('sw-file-summary')).toBeNull();
  });

  it('with a file: its base name, rows and columns, and the permanent line', async () => {
    setUp();
    await opened();
    expect(screen.getByTestId('sw-file-summary').textContent).toBe('survey.csv: 250 rows, 1 column.');
    expect(screen.queryByTestId('sw-prepared-note')).toBeNull();
    expect(screen.getByTestId('sw-as-read')).toBeTruthy();
    expect(screen.getByTestId('sw-file-summary').closest('[title]')?.getAttribute('title')).toBe('survey.csv');
  });

  it('with prepared columns: the note, from the DatasetView', async () => {
    setUp();
    await opened(preparedFixture as DatasetView);
    expect(screen.getByTestId('sw-prepared-note').textContent).toContain('1 prepared column.');
  });
});

describe('the tabs, until the grid and the Output come', () => {
  it('Data View with no file: Open a data file and the limits sentence', async () => {
    setUp();
    const empty = await screen.findByTestId('sw-data-empty');
    expect(within(empty).getByRole('button', { name: 'Open a data file' })).toBeTruthy();
    await waitFor(() => expect(within(empty).getByTestId('sw-limits').textContent).toBe(limitsSentence(limits8 as MethodLimitsView)));
    expect(screen.queryByTestId('sw-grid')).toBeNull();
  });

  it('Data View with a file: the grid, in place of the sentence that held its place', async () => {
    setUp();
    await opened();
    expect(screen.getByRole('grid').getAttribute('aria-readonly')).toBe('true');
    expect(screen.queryByText('The grid comes in the next step of the build.')).toBeNull();
    expect(screen.queryByTestId('sw-data-empty')).toBeNull();
  });

  it('Variable View: its table with a file, and says to open one without', async () => {
    setUp();
    await screen.findByTestId('sw-data-empty');
    tab(/Variable View/);
    expect(screen.getByTestId('sw-variable-empty').textContent).toBe(NO_FILE);
    await opened();
    expect(screen.getByTestId('sw-var-C1')).toBeTruthy();
  });

  it('Output: the empty sentence, and its count 0', async () => {
    setUp();
    await screen.findByTestId('sw-data-empty');
    tab(/Output/);
    expect(screen.getByTestId('sw-output-empty').textContent).toBe('No output yet. Run an analysis from the Analyze menu; each result is added here, newest last.');
    expect(screen.getByRole('tab', { name: 'Output (0)' })).toBeTruthy();
  });
});

describe('Open and Close, from the File menu', () => {
  it('with no file, Close says why and Open opens, with no close sent', async () => {
    const { commands } = setUp();
    await screen.findByTestId('sw-data-empty');
    openMenu('file');
    expect(screen.getByTestId('gds-menu-item-file-close').getAttribute('aria-disabled')).toBe('true');
    expect(description('file-close')).toBe(NO_FILE);
    fireEvent.click(screen.getByTestId('gds-menu-item-file-open'));
    await screen.findByTestId('sw-file-summary');
    expect(statisticsSession.get().slot).toBe(1);
    expect(commands()).toEqual(['stats_desc_open']);
  });

  it('Open a data file from the empty state does the same', async () => {
    const { commands } = setUp();
    fireEvent.click(await screen.findByTestId('sw-open'));
    await screen.findByTestId('sw-file-summary');
    expect(commands()).toEqual(['stats_desc_open']);
  });

  it('opening another: the new file shows, and the old slot closes after it', async () => {
    const { commands, files } = setUp({ files: { stats_desc_open: fileView(2, 'other.csv') } });
    await opened();
    choose('file', 'file-open');
    await waitFor(() => expect(screen.getByTestId('sw-file-summary').textContent).toContain('other.csv'));
    expect(commands()).toEqual(['stats_desc_open', 'stats_desc_close']);
    expect(files.calls[1][1]).toEqual({ slot: 1 });
  });

  it('a refused open changes nothing, closes nothing, and says why', async () => {
    const refusal = JSON.stringify({ code: 'ingest.too_many_rows', params: {}, severity: 'error' });
    const { commands } = setUp({
      files: {
        stats_desc_open: () => {
          throw new Error(refusal);
        },
      },
    });
    await opened();
    choose('file', 'file-open');
    await screen.findByTestId('sw-error');
    expect(screen.getByTestId('sw-file-summary').textContent).toContain('survey.csv');
    expect(statisticsSession.get().slot).toBe(1);
    expect(commands()).toEqual(['stats_desc_open']);
  });

  it('a cancelled picker changes nothing and sends nothing', async () => {
    const { commands } = setUp({ pick: () => Promise.resolve(null) });
    await opened();
    choose('file', 'file-open');
    await act(async () => {
      await Promise.resolve();
    });
    expect(commands()).toEqual([]);
    expect(statisticsSession.get().slot).toBe(1);
    expect(screen.queryByTestId('sw-error')).toBeNull();
  });

  it('Close sends the close, and the empty state comes back', async () => {
    const { commands, files } = setUp();
    await opened();
    choose('file', 'file-close');
    await screen.findByTestId('sw-data-empty');
    expect(commands()).toEqual(['stats_desc_close']);
    expect(files.calls[0][1]).toEqual({ slot: 1 });
    expect(statisticsSession.get().view).toBeNull();
  });

  it('shows the open\'s rows read while it goes, by the shared line', async () => {
    const { files } = setUp();
    let release: () => void = () => undefined;
    files.open = (_p, on) => {
      on?.(1234);
      return new Promise((resolve) => {
        release = () => resolve(fileView(1));
      });
    };
    fireEvent.click(await screen.findByTestId('sw-open'));
    expect((await screen.findByTestId('sw-open-progress')).textContent).toContain('1234 rows read');
    release();
    await screen.findByTestId('sw-file-summary');
    expect(screen.queryByTestId('sw-open-progress')).toBeNull();
  });
});

describe('what the window knows of the workspace', () => {
  it('a run going disables Open and Close, with the sentence, and they come back after it', async () => {
    let state = GOING;
    setUp({ state: () => state });
    await opened();
    // The menu stays open: a waitFor callback must only read, since a change to
    // the DOM inside one wakes it again and starves the timers.
    openMenu('file');
    await waitFor(() => expect(description('file-open')).toBe(RUN_GOING));
    expect(screen.getByTestId('gds-menu-item-file-open').getAttribute('aria-disabled')).toBe('true');
    expect(description('file-close')).toBe(RUN_GOING);
    state = ACTIVE;
    await waitFor(() => expect(screen.getByTestId('gds-menu-item-file-open').getAttribute('aria-disabled')).toBeNull());
    expect(description('file-close')).toBeUndefined();
  });

  it('a run going disables the empty state\'s Open too, with the sentence', async () => {
    setUp({ state: () => GOING });
    await waitFor(() => expect((screen.getByTestId('sw-open') as HTMLButtonElement).disabled).toBe(true));
    expect(screen.getByTestId('sw-open-blocked').textContent).toBe(RUN_GOING);
  });

  it('asks every poll, and starts by asking', async () => {
    const { workspace } = setUp();
    await waitFor(() => expect(workspace.calls.filter(([c]) => c === 'workspace_state').length).toBeGreaterThan(2));
  });

  it('anything but Statistics active clears the session, and the file goes from the screen', async () => {
    let state = ACTIVE;
    setUp({ state: () => state });
    await opened();
    state = NONE;
    await screen.findByTestId('sw-data-empty');
    expect(statisticsSession.get().view).toBeNull();
    expect(screen.queryByTestId('sw-file-summary')).toBeNull();
  });

  it('a slot found gone clears the file and says the sentence', async () => {
    setUp();
    await opened();
    act(() => statisticsSession.slotGone());
    await screen.findByTestId('sw-data-empty');
    expect(screen.getByTestId('sw-notice').textContent).toBe(SLOT_GONE);
  });
});

describe('Analyze and Transform read Rust\'s list', () => {
  it('capability_list is read once, and a method Rust does not call runnable says so', async () => {
    const { capabilities } = setUp({ capabilities: ALL.map((c) => (c.id === 'stats.analyze.paired_t' ? { ...c, runnable: false } : c)) });
    await waitFor(() => expect(capabilities.calls).toHaveLength(1));
    await act(async () => {
      await Promise.resolve();
    });
    openMenu('analyze');
    expect(description('analyze-paired')).toBe(NOT_RUNNABLE);
    expect(description('analyze-ttest')).toBe(NO_FILE);
    openMenu('analyze');
    openMenu('transform');
    expect(description('transform-scale-score')).toBe(NO_FILE);
  });

  it('a list that could not be read leaves every method not runnable', async () => {
    const { capabilities } = setUp({
      capabilities: () => {
        throw new Error('no');
      },
    });
    await waitFor(() => expect(capabilities.calls).toHaveLength(1));
    openMenu('analyze');
    expect(description('analyze-ttest')).toBe(NOT_RUNNABLE);
  });
});

describe("Gaply's menu, and the file across a visit", () => {
  it.each([
    ['gaply-settings', 'settings-here'],
    ['gaply-verifier', 'verifier-here'],
    ['gaply-check', 'check-here'],
    ['gaply-home', 'home-here'],
  ])('%s leaves the route, and the file stays open: nothing is closed', async (item, here) => {
    const { commands } = setUp();
    await opened();
    choose('gaply', item);
    await screen.findByTestId(here);
    expect(commands()).toEqual([]);
    expect(statisticsSession.get().slot).toBe(1);
  });

  it('Home and back shows the same file', async () => {
    setUp();
    await opened();
    choose('gaply', 'gaply-home');
    fireEvent.click(await screen.findByTestId('back-to-window'));
    expect((await screen.findByTestId('sw-file-summary')).textContent).toContain('survey.csv');
  });

  it('Switch workspace asks Rust, drops the file and lands on the chooser', async () => {
    const { workspace, commands } = setUp();
    await opened();
    choose('gaply', 'gaply-switch');
    await screen.findByTestId('chooser-here');
    expect(workspace.calls.some(([c]) => c === 'workspace_deactivate')).toBe(true);
    expect(statisticsSession.get().view).toBeNull();
    expect(commands()).toEqual([]);
  });

  it("Switch workspace refused for a run shows today's Wait and Cancel dialog, and the file stays", async () => {
    const refusal = JSON.stringify({ code: 'workspace.run_active', params: { run: 5 }, severity: 'error' });
    setUp({
      state: () => GOING,
      deactivate: () => {
        throw new Error(refusal);
      },
      files: { stats_desc_status: { runId: 5, state: 'running', terminal: false, columnsDone: 0, columnsTotal: 1 } },
    });
    await opened();
    choose('gaply', 'gaply-switch');
    expect(await screen.findByTestId('ws-switch-dialog')).toBeTruthy();
    expect(screen.getByTestId('ws-switch-wait')).toBeTruthy();
    expect(screen.getByTestId('ws-switch-cancel')).toBeTruthy();
    expect(statisticsSession.get().slot).toBe(1);
  });

  it('Switch workspace is not offered while the workspaces flag is off', async () => {
    flags.workspaces = false;
    setUp();
    await screen.findByTestId('statistics-window');
    openMenu('gaply');
    expect(screen.queryByTestId('gds-menu-item-gaply-switch')).toBeNull();
  });
});

describe("the project's line", () => {
  it('says none open', async () => {
    const { projects } = setUp();
    expect((await screen.findByTestId('sw-project-line')).textContent).toBe('Project: none open');
    await waitFor(() => expect(projects.calls.map(([c]) => c).sort()).toEqual([C.current, C.list].sort()));
  });

  it('gives the open project\'s name, inside bdi, from Rust\'s answer', async () => {
    setUp({ projects: { [C.current]: { projectId: 'p1', rerun: null }, [C.list]: [{ projectId: 'p1', name: 'My thesis', bytes: 1, savedAt: 1, runCount: 1, refused: null }] } });
    await waitFor(() => expect(screen.getByTestId('sw-project-line').textContent).toBe('Project: My thesis'));
    expect(screen.getByTestId('sw-project-line').querySelector('bdi')?.textContent).toBe('My thesis');
    expect(screen.getByTestId('sw-project-line').getAttribute('title')).toBe('My thesis');
  });

  it('is not shown, and nothing is asked, when projectsShown says no', async () => {
    flags.projects = false;
    const { projects } = setUp();
    await screen.findByTestId('statistics-window');
    await act(async () => {
      await Promise.resolve();
    });
    expect(screen.queryByTestId('sw-project-line')).toBeNull();
    expect(projects.calls).toEqual([]);
    openMenu('help');
    expect(screen.queryByTestId('gds-menu-project')).toBeNull();
  });
});

describe('Help, from the menu', () => {
  it.each([
    ['help-methods', 'sw-help-methods'],
    ['help-file', 'sw-help-file'],
    ['help-keyboard', 'sw-help-keyboard'],
  ])('%s opens its dialog, and closing it returns the focus to the Help menu', async (item, id) => {
    setUp();
    await screen.findByTestId('statistics-window');
    choose('help', item);
    expect(await screen.findByTestId(id)).toBeTruthy();
    expect(screen.getByRole('dialog').contains(document.activeElement)).toBe(true);
    fireEvent.click(screen.getByTestId('sw-help-close'));
    expect(screen.queryByRole('dialog')).toBeNull();
    expect(document.activeElement).toBe(screen.getByTestId('gds-menu-help'));
  });
});

describe('the picker\'s list of extensions', () => {
  const FLOWS = ['TTestFlow', 'OneSampleFlow', 'PairedFlow', 'AnovaFlow', 'CorrFlow', 'AlphaFlow', 'DescribeFlow'];

  it.each(FLOWS)("equals %s's", (flow) => {
    const src = fs.readFileSync(path.join(__dirname, `../${flow}.tsx`), 'utf8');
    const m = src.match(/const DATA_EXTENSIONS = (\[[^\]]*\]);/);
    expect(m, flow).toBeTruthy();
    const list = JSON.parse((m as RegExpMatchArray)[1].replace(/'/g, '"'));
    expect(list.length).toBeGreaterThan(0);
    expect([...DATA_EXTENSIONS]).toEqual(list);
  });
});

// Research OS, Phase 8Q (decision 460): the grid follows the session's view, and
// the cache belongs to it.
describe('the grid and the session', () => {
  const hashed = (slot: number, hash: string): DatasetView => ({ ...fileView(slot), inputRef: { ...fileView(slot).inputRef, contentHash: hash } });
  const box = () => {
    Object.defineProperty(HTMLElement.prototype, 'clientHeight', { configurable: true, get: () => 560 });
    Object.defineProperty(HTMLElement.prototype, 'clientWidth', { configurable: true, get: () => 1000 });
  };
  afterEach(() => {
    delete (HTMLElement.prototype as unknown as Record<string, unknown>).clientHeight;
    delete (HTMLElement.prototype as unknown as Record<string, unknown>).clientWidth;
  });

  function withCells(answer: (a: Record<string, unknown>) => unknown) {
    box();
    const cells = makeMockDescribeBridge({ stats_desc_cells: answer });
    render(
      <MemoryRouter initialEntries={['/w']}>
        <Routes>
          <Route
            path="/w"
            element={
              <StatisticsWindow
                workspace={makeMockWorkspaceBridge({ workspace_state: ACTIVE, workspace_deactivate: NONE })}
                files={makeMockDescribeBridge({ stats_desc_close: true })}
                cells={cells}
                capabilities={makeMockCapabilityBridge({ capability_list: ALL })}
                projects={makeMockProjectBridge({ [C.current]: { projectId: null, rerun: null }, [C.list]: [] })}
                pickPath={() => Promise.resolve(null)}
                limits={() => Promise.resolve(limits8)}
                pollMs={1000}
              />
            }
          />
        </Routes>
      </MemoryRouter>
    );
    return cells;
  }
  const answer = (hash: string, value: string) => (a: Record<string, unknown>) => ({
    slot: a.slot,
    inputHash: hash,
    rows: 250,
    columns: 1,
    firstRow: a.firstRow,
    firstColumn: a.firstColumn,
    cells: Array.from({ length: Math.min(a.rowCount as number, 250 - (a.firstRow as number)) }, () => [value]),
  });

  it('a make: setFile with the same slot and a new inputHash empties the cache, and the grid reads again', async () => {
    let value = 'BEFORE';
    let hash = 'ab'.repeat(32);
    const cells = withCells((a) => answer(hash, value)(a));
    act(() => statisticsSession.setFile(hashed(1, hash)));
    await waitFor(() => expect(screen.getByTestId('sw-cell-0-0').textContent).toBe('BEFORE'));
    const asked = cells.calls.length;
    value = 'AFTER';
    hash = 'cd'.repeat(32);
    act(() => statisticsSession.setFile(hashed(1, hash)));
    expect(screen.getByTestId('sw-cell-0-0').textContent).not.toBe('BEFORE');
    await waitFor(() => expect(screen.getByTestId('sw-cell-0-0').textContent).toBe('AFTER'));
    expect(cells.calls.length).toBeGreaterThan(asked);
  });

  it.each([
    ['a close', () => statisticsSession.clearFile()],
    ['a clear', () => statisticsSession.clear()],
    ['a slot gone', () => statisticsSession.slotGone()],
  ])('%s: the grid goes, and the next file starts empty', async (_what, change) => {
    const hash = 'ab'.repeat(32);
    const cells = withCells((a) => answer(hash, 'FIRST')(a));
    act(() => statisticsSession.setFile(hashed(1, hash)));
    await waitFor(() => expect(screen.getByTestId('sw-cell-0-0').textContent).toBe('FIRST'));
    act(() => change());
    expect(screen.queryByTestId('sw-grid')).toBeNull();
    expect(document.body.textContent).not.toContain('FIRST');
    const asked = cells.calls.length;
    act(() => statisticsSession.setFile(hashed(1, hash)));
    expect(screen.getByTestId('sw-cell-0-0').textContent).toBe('');
    await waitFor(() => expect(cells.calls.length).toBeGreaterThan(asked));
  });

  it('an open: a new slot starts empty, and a window answered for the old one is not drawn', async () => {
    const cells = withCells((a) => answer(a.slot === 1 ? 'ab'.repeat(32) : 'cd'.repeat(32), a.slot === 1 ? 'ONE' : 'TWO')(a));
    act(() => statisticsSession.setFile(hashed(1, 'ab'.repeat(32))));
    await waitFor(() => expect(screen.getByTestId('sw-cell-0-0').textContent).toBe('ONE'));
    act(() => statisticsSession.setFile(hashed(2, 'cd'.repeat(32))));
    expect(screen.getByTestId('sw-cell-0-0').textContent).toBe('');
    await waitFor(() => expect(screen.getByTestId('sw-cell-0-0').textContent).toBe('TWO'));
    expect(cells.calls.some(([, a]) => a.slot === 2)).toBe(true);
  });

  it('a slot gone found by a window: the file goes, and the sentence shows', async () => {
    withCells(() => Promise.reject('{"code":"run.unknown_input"}'));
    act(() => statisticsSession.setFile(hashed(1, 'ab'.repeat(32))));
    await waitFor(() => expect(screen.getByTestId('sw-notice').textContent).toBe('This file is no longer held in memory. Open it again.'));
    expect(screen.queryByTestId('sw-grid')).toBeNull();
    expect(screen.getByTestId('sw-data-empty')).toBeTruthy();
  });

  it('the body does not scroll around the grid, and scrolls again with no file', async () => {
    withCells((a) => answer('ab'.repeat(32), 'x')(a));
    expect(screen.getByTestId('sw-body').className).toBe('sw-body');
    expect(screen.getByTestId('sw-body').getAttribute('tabindex')).toBe('0');
    act(() => statisticsSession.setFile(hashed(1, 'ab'.repeat(32))));
    await screen.findByTestId('sw-grid');
    expect(screen.getByTestId('sw-body').className).toBe('sw-body sw-body--grid');
    expect(screen.getByTestId('sw-body').getAttribute('tabindex')).toBeNull();
  });

  it('the Data View\'s reads go through the grid\'s prop, never the files bridge', async () => {
    const { files } = setUp();
    box();
    await opened();
    await act(async () => undefined);
    expect(files.calls.map(([c]) => c)).not.toContain('stats_desc_cells');
  });

  it('the counter is not there with statsTiming off', async () => {
    setUp();
    await opened();
    expect(screen.queryByTestId('sw-timing')).toBeNull();
  });
});
