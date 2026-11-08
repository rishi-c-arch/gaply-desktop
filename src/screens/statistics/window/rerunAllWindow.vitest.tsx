// Run every saved result again… in the window (Research OS, Phase 8U-2,
// decisions 536, 538 and 540): the item opening the dialog; the list going on
// when the window unmounts, the dialog closed when it comes back, and the item
// opening it on the list; nothing started by itself; a session clear, by the
// window's poll or by the switch, ending the list; and, while it goes, the
// menus' sentence and the Output's Save greyed. Every bridge is a mock; no
// project command is named as a string here.
import React from 'react';
import { MemoryRouter, Route, Routes, useNavigate } from 'react-router-dom';
import { act, cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { makeMockWorkspaceBridge } from '../../workspace/workspaceBridge';
import type { WorkspaceStateView } from '../../workspace/workspaceTypes';
import { makeMockAlphaBridge } from '../alphaBridge';
import type { AlphaResultView } from '../alphaTypes';
import { makeMockDescribeBridge } from '../describeBridge';
import type { ColumnProfile, DatasetView } from '../describeTypes';
import { alphaResultView } from '../fixtures/alphaFixtures';
import limits8 from '../fixtures/methodLimitsView8gb.json';
import { makeMockProjectBridge, PROJECT_COMMANDS as C } from '../projects/projectBridge';
import { openProjectStore } from '../projects/projectSession';
import type { StoredRunView } from '../projects/projectTypes';
import { PROJECT_TEXT } from '../projects/projectWording';
import { makeMockCapabilityBridge } from '../ttestBridge';
import { rerunAllStore } from './rerunAll';
import { StatisticsWindow } from './StatisticsWindow';
import { statisticsSession } from './statisticsSession';
import { RERUNS_GOING } from './windowWording';

vi.mock('../../../config/featureFlags', () => ({ isFeatureEnabled: (name: string) => name === 'workspaces' || name === 'projects' }));
vi.mock('../../../utils/isTauri', () => ({ isTauri: true, default: true }));
vi.mock('../../../design-system/GaplyGlobe', () => ({ GaplyGlobe: () => <div /> }));

afterEach(() => {
  cleanup();
  statisticsSession.clear();
  openProjectStore.clear();
});

const P = 'p'.repeat(32);
const ACTIVE: WorkspaceStateView = { state: { phase: 'active', workspace: 'statistics' }, activeRun: null, latestRun: null };
const NONE: WorkspaceStateView = { state: { phase: 'no_workspace' }, activeRun: null, latestRun: null };
const stored = (id: number, savedAt: number): StoredRunView => ({
  storedRunId: id,
  capabilityId: 'stats.describe',
  capabilityVersion: 1,
  state: 'succeeded',
  endCode: null,
  startedAt: savedAt - 10,
  endedAt: savedAt - 9,
  savedAt,
  agentVersion: '0.1.0',
  rerunOf: null,
  comparison: null,
  engineVersionDiffers: null,
});
const TWO = [stored(1, 1792396100), stored(2, 1792396200)];
const col = (alias: string): ColumnProfile => ({ alias, header: alias, kind: 'numeric', n: 9, missing: 0, distinct: 5, levels: null, suspectMissingCodes: [], nonFinite: 0 });
const view: DatasetView = {
  slot: 1,
  displayName: 'items.csv',
  profile: { rows: 9, columns: [col('C1'), col('C2')] },
  inputRef: { contentHash: 'ab'.repeat(32), rows: 9, columns: 2, semanticRole: 'table' },
};

/** Rust, through the project bridge: each rerun stays going. */
function setUp(o: { state?: () => WorkspaceStateView; deactivate?: unknown } = {}) {
  let ids = 10;
  let going: { runId: number; rerunOf: number } | null = null;
  const projects = makeMockProjectBridge({
    [C.current]: () => ({ projectId: P, rerun: going ? { ...going, status: 'going', storedRunId: null, code: null } : null }),
    [C.list]: [{ projectId: P, name: 'A project', bytes: 1, savedAt: 1, runCount: 2, refused: null }],
    [C.runs]: TWO,
    [C.rerun]: (a: Record<string, unknown>) => {
      going = { runId: ++ids, rerunOf: a.runId as number };
      return { runId: ids, kind: 'describe' };
    },
  });
  const describe = makeMockDescribeBridge({ stats_desc_cancel: { runId: 0 }, stats_desc_close: true });
  const workspace = makeMockWorkspaceBridge({ workspace_state: o.state ?? ACTIVE, workspace_deactivate: o.deactivate ?? NONE });
  const Away: React.FC = () => {
    const navigate = useNavigate();
    return (
      <button type="button" data-testid="back-to-window" onClick={() => navigate('/w')}>
        back
      </button>
    );
  };
  const Leave: React.FC = () => {
    const navigate = useNavigate();
    return (
      <button type="button" data-testid="leave-window" onClick={() => navigate('/away')}>
        leave
      </button>
    );
  };
  render(
    <MemoryRouter initialEntries={['/w']}>
      <Leave />
      <Routes>
        <Route
          path="/w"
          element={
            <StatisticsWindow
              workspace={workspace}
              files={describe}
              forget={describe}
              cells={makeMockDescribeBridge({ stats_desc_cells: { slot: 1, inputHash: 'ab'.repeat(32), rows: 9, columns: 2, firstRow: 0, firstColumn: 0, cells: [] } })}
              capabilities={makeMockCapabilityBridge({ capability_list: [] })}
              projects={projects}
              runs={{ describe, alpha: makeMockAlphaBridge() }}
              limits={() => Promise.resolve(limits8)}
              pollMs={15}
              runPollMs={2}
            />
          }
        />
        <Route path="/away" element={<Away />} />
        <Route path="/app/workspaces" element={<div data-testid="chooser" />} />
      </Routes>
    </MemoryRouter>
  );
  return { projects, describe };
}

const reruns = (b: { calls: Array<[string, Record<string, unknown>]> }) => b.calls.filter(([c]) => c === C.rerun);
const openMenu = (id: string) => fireEvent.click(screen.getByTestId(`gds-menu-${id}`));
const menuItem = (id: string) => screen.getByTestId(`gds-menu-item-${id}`);
const chooseRerunAll = async () => {
  await screen.findByTestId('sw-project-open');
  openMenu('project');
  fireEvent.click(menuItem('project-rerun-all'));
  return screen.findByTestId('sw-rerun-all-dialog');
};
const describedBy = (el: HTMLElement) => document.getElementById(el.getAttribute('aria-describedby') ?? '')?.textContent;

describe('the item and its dialog in the window (decision 538)', () => {
  it('the window rendered and remounted, and the dialog opened, start nothing by themselves', async () => {
    const { projects } = setUp();
    await chooseRerunAll();
    await screen.findByTestId('sw-rerun-all-sentence');
    fireEvent.click(screen.getByTestId('sw-rerun-all-cancel'));
    fireEvent.click(screen.getByTestId('leave-window'));
    fireEvent.click(await screen.findByTestId('back-to-window'));
    await screen.findByTestId('statistics-window');
    await act(async () => {
      await new Promise((r) => setTimeout(r, 40));
    });
    expect(reruns(projects)).toEqual([]);
    expect(rerunAllStore.get().status).toBe('idle');
  });

  it('the list goes on when the window unmounts; back, the dialog is closed, and the item opens it on the list', async () => {
    const { projects } = setUp();
    await chooseRerunAll();
    fireEvent.click(await screen.findByTestId('sw-rerun-all-run'));
    await waitFor(() => expect(reruns(projects)).toHaveLength(1));
    fireEvent.click(screen.getByTestId('leave-window'));
    await screen.findByTestId('back-to-window');
    expect(rerunAllStore.get().status).toBe('going');
    fireEvent.click(screen.getByTestId('back-to-window'));
    await screen.findByTestId('statistics-window');
    expect(screen.queryByTestId('sw-rerun-all-dialog')).toBeNull();
    expect(rerunAllStore.get().status).toBe('going');
    openMenu('project');
    fireEvent.click(menuItem('project-rerun-all'));
    expect(await screen.findByTestId('sw-rerun-all-log')).toBeTruthy();
    expect(screen.queryByTestId('sw-rerun-all-run')).toBeNull();
    expect(reruns(projects)).toHaveLength(1);
  });

  it("the window's poll clearing the session ends the list: nothing more starts", async () => {
    let active = true;
    const { projects } = setUp({ state: () => (active ? ACTIVE : NONE) });
    await chooseRerunAll();
    fireEvent.click(await screen.findByTestId('sw-rerun-all-run'));
    await waitFor(() => expect(reruns(projects)).toHaveLength(1));
    active = false;
    await waitFor(() => expect(rerunAllStore.get().status).toBe('idle'));
    await act(async () => {
      await new Promise((r) => setTimeout(r, 40));
    });
    expect(reruns(projects)).toHaveLength(1);
  });

  it("the switch's own clear ends the list too", async () => {
    const { projects } = setUp();
    await chooseRerunAll();
    fireEvent.click(await screen.findByTestId('sw-rerun-all-run'));
    await waitFor(() => expect(reruns(projects)).toHaveLength(1));
    fireEvent.click(screen.getByTestId('sw-rerun-all-close'));
    // Gaply, Switch workspace: Rust lets the workspace go, and the session is cleared.
    openMenu('gaply');
    fireEvent.click(menuItem('gaply-switch'));
    await screen.findByTestId('chooser');
    expect(rerunAllStore.get().status).toBe('idle');
    await act(async () => {
      await new Promise((r) => setTimeout(r, 40));
    });
    expect(reruns(projects)).toHaveLength(1);
  });
});

describe('the window while the list goes (decision 538)', () => {
  it("Open a data file…, the Data View's Open and Open a project… say the list's sentence", async () => {
    setUp();
    await chooseRerunAll();
    fireEvent.click(await screen.findByTestId('sw-rerun-all-run'));
    await waitFor(() => expect(rerunAllStore.get().status).toBe('going'));
    fireEvent.click(screen.getByTestId('sw-rerun-all-close'));
    openMenu('file');
    expect(describedBy(menuItem('file-open'))).toBe(RERUNS_GOING);
    openMenu('file');
    openMenu('project');
    expect(menuItem('project-open').getAttribute('aria-disabled')).toBe('true');
    expect(describedBy(menuItem('project-open'))).toBe(RERUNS_GOING);
    openMenu('project');
    expect((screen.getByTestId('sw-open') as HTMLButtonElement).disabled).toBe(true);
    expect(screen.getByTestId('sw-open-blocked').textContent).toBe(RERUNS_GOING);
  });

  it("an entry's Save to a project… is greyed, with the list's sentence beside it", async () => {
    act(() => {
      statisticsSession.setFile(view);
      const n = statisticsSession.addEntry({
        kind: 'cronbach_alpha',
        view,
        slot: 1,
        settings: 'Items: C1, C2.',
        runId: 5,
        run: { runId: 5, state: 'succeeded', terminal: true, elapsedMs: 1, cancelRequested: false, stepsDone: 7, stepsTotal: 7, diagnostics: [] },
      });
      statisticsSession.updateEntry(n as number, { result: { ...alphaResultView, runId: 5 } as unknown as AlphaResultView });
    });
    setUp();
    fireEvent.click(screen.getByRole('tab', { name: /Output/ }));
    const e = await screen.findByTestId('sw-entry-1');
    expect(within(e).getByTestId('sw-entry-1-project-button')).toBeTruthy();
    await chooseRerunAll();
    fireEvent.click(await screen.findByTestId('sw-rerun-all-run'));
    await waitFor(() => expect(rerunAllStore.get().status).toBe('going'));
    fireEvent.click(screen.getByTestId('sw-rerun-all-close'));
    await waitFor(() => expect(within(e).queryByTestId('sw-entry-1-project-button')).toBeNull());
    expect((within(e).getByTestId('sw-entry-1-save-off') as HTMLButtonElement).disabled).toBe(true);
    expect(within(e).getByTestId('sw-entry-1-save-off').textContent).toBe(PROJECT_TEXT.saveButton);
    expect(within(e).getByTestId('sw-entry-1-save-why').textContent).toBe(RERUNS_GOING);
  });
});
