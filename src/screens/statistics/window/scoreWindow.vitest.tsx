// Transform, Scale score… in the window (Research OS, Phase 8T-2a, decisions 505
// to 507): the menu item's order, methodItem's without the Output's cap, and the
// dialog reached from it: a make's counts in sw-message, the grid's and the
// Variable View's new view, a deletion's sentence, and no close sent. Every
// bridge is a mock.
import React from 'react';
import { MemoryRouter, Route, Routes } from 'react-router-dom';
import { act, cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import type { MenuDefinition, MenuItem } from '../../../design-system';
import { makeMockWorkspaceBridge } from '../../workspace/workspaceBridge';
import type { WorkspaceStateView } from '../../workspace/workspaceTypes';
import { makeMockDescribeBridge } from '../describeBridge';
import type { ColumnProfile, DatasetView } from '../describeTypes';
import limits8 from '../fixtures/methodLimitsView8gb.json';
import { METHODS } from '../methods';
import { makeMockPrepBridge } from '../prepBridge';
import type { PrepMadeView } from '../prepTypes';
import { makeMockProjectBridge, PROJECT_COMMANDS as C } from '../projects/projectBridge';
import { openProjectStore } from '../projects/projectSession';
import { makeMockCapabilityBridge } from '../ttestBridge';
import type { CapabilityView } from '../ttestTypes';
import { StatisticsWindow } from './StatisticsWindow';
import { statisticsSession } from './statisticsSession';
import { SCALE_SCORE_CAPABILITY, windowMenus, type WindowMenuContext } from './windowMenus';
import { FILE_BUSY, FROM_A_RECIPE, NOT_RUNNABLE, NO_FILE, RUN_GOING, scoreDeletedSentence } from './windowWording';

vi.mock('../../../config/featureFlags', () => ({ isFeatureEnabled: (name: string) => name === 'workspaces' || name === 'projects' }));
vi.mock('../../../utils/isTauri', () => ({ isTauri: true, default: true }));
vi.mock('../../../design-system/GaplyGlobe', () => ({ GaplyGlobe: () => <div /> }));

afterEach(() => {
  cleanup();
  statisticsSession.clear();
  openProjectStore.clear();
});

const capability = (id: string, runnable = true): CapabilityView => ({
  id,
  version: 1,
  lifecycle: 'beta',
  titleCode: `${id}.title`,
  descriptionCode: `${id}.description`,
  licenseNotes: [],
  knownLimitations: [],
  runnable,
});
const ALL = [...METHODS.map((m) => capability(m.capabilityId)), capability(SCALE_SCORE_CAPABILITY)];

describe("Transform's item (decision 506)", () => {
  const context = (over: Partial<WindowMenuContext> = {}): WindowMenuContext => ({
    fileOpen: true,
    runGoing: false,
    busy: false,
    capabilities: ALL,
    projects: true,
    statsCheck: true,
    workspaces: true,
    actions: { open: vi.fn(), close: vi.fn(), go: vi.fn(), switchWorkspace: vi.fn(), help: vi.fn() },
    ...over,
  });
  const scoreItem = (ms: MenuDefinition[]): MenuItem => {
    const t = ms.find((m) => m.id === 'transform');
    const i = t?.groups.flatMap((g) => g.items).find((x) => x.id === 'transform-scale-score');
    if (!i) throw new Error('no scale score item');
    return i;
  };
  const notRunnable = ALL.map((c) => (c.id === SCALE_SCORE_CAPABILITY ? { ...c, runnable: false } : c));

  it('with a file, it is live and opens the score dialog', () => {
    const score = vi.fn();
    const i = scoreItem(windowMenus(context({ actions: { open: vi.fn(), close: vi.fn(), go: vi.fn(), switchWorkspace: vi.fn(), help: vi.fn(), score } })));
    expect(i.disabled).toBeFalsy();
    i.onSelect?.();
    expect(score).toHaveBeenCalledTimes(1);
  });

  it("says why not, in methodItem's order: not runnable, then no file, then a run, then busy", () => {
    expect(scoreItem(windowMenus(context({ capabilities: notRunnable, fileOpen: false, runGoing: true, busy: true }))).description).toBe(NOT_RUNNABLE);
    expect(scoreItem(windowMenus(context({ capabilities: null }))).description).toBe(NOT_RUNNABLE);
    expect(scoreItem(windowMenus(context({ fileOpen: false, runGoing: true, busy: true }))).description).toBe(NO_FILE);
    expect(scoreItem(windowMenus(context({ runGoing: true, busy: true }))).description).toBe(RUN_GOING);
    expect(scoreItem(windowMenus(context({ busy: true }))).description).toBe(FILE_BUSY);
    for (const o of [{ capabilities: notRunnable }, { fileOpen: false }, { runGoing: true }, { busy: true }]) {
      const i = scoreItem(windowMenus(context(o)));
      expect(i.disabled).toBe(true);
      expect(i.onSelect).toBeUndefined();
    }
  });

  it("a full Output does not stop it: a score adds no entry", () => {
    expect(scoreItem(windowMenus(context({ outputFull: true }))).disabled).toBeFalsy();
  });

  it('with no score action given, choosing it does nothing', () => {
    expect(() => scoreItem(windowMenus(context())).onSelect?.()).not.toThrow();
  });

  it("Transform's shape stays one item, then Coming later", () => {
    const t = windowMenus(context()).find((m) => m.id === 'transform');
    expect(t?.groups[0].items.map((i) => i.label)).toEqual(['Scale score…']);
    expect(t?.groups).toHaveLength(2);
  });
});

/* ---- the dialog in the window ---------------------------------------------------- */

const ACTIVE: WorkspaceStateView = { state: { phase: 'active', workspace: 'statistics' }, activeRun: null, latestRun: null };

const col = (alias: string, header: string, extra: Partial<ColumnProfile> = {}): ColumnProfile => ({ alias, header, kind: 'numeric', n: 9, missing: 0, distinct: 5, levels: null, suspectMissingCodes: [], nonFinite: 0, ...extra });
const fileView: DatasetView = {
  slot: 1,
  displayName: 'items.csv',
  profile: { rows: 9, columns: [col('C1', 'Q1'), col('C2', 'Q2'), col('C3', 'Q3')] },
  inputRef: { contentHash: 'ab'.repeat(32), rows: 9, columns: 3, semanticRole: 'table' },
};
const scoreCounts = { column: 'C4', scoredFromEveryItem: 7, scoredFromFewer: 0, notScoredTooFew: 2, notScoredOverflow: 0 };
const preparedView: DatasetView = {
  ...fileView,
  profile: { rows: 9, columns: [...fileView.profile.columns, col('C4', 'C4', { prepared: true, scoreName: 'Total' })] },
  inputRef: { ...fileView.inputRef, contentHash: 'cd'.repeat(32), columns: 4 },
  preparation: { sourceHash: 'ab'.repeat(32), recipeHash: 'ef'.repeat(32), sourceColumns: 3, capabilityVersion: 1, engineVersion: 1 },
  scoreCounts: [scoreCounts],
};
const made: PrepMadeView = { status: 'valid', errors: [], warnings: [], missingInformation: [], estimatedResources: { rows: 9, columns: 4, cells: 36 }, counts: [scoreCounts], dataset: preparedView };
const totalStep = { kind: 'scale_score' as const, alias: 'C4', name: 'Total', items: ['C1', 'C2'], reversed: [], scale: null, statistic: 'sum' as const, rule: 'every_item' as const, missingCodes: [] };

function setUp(responses: Record<string, unknown> = {}) {
  const workspace = makeMockWorkspaceBridge({ workspace_state: ACTIVE });
  const files = makeMockDescribeBridge({ stats_desc_close: true });
  const cells = makeMockDescribeBridge({
    stats_desc_cells: (a: Record<string, unknown>) => ({ slot: a.slot, inputHash: 'ab'.repeat(32), rows: 9, columns: 3, firstRow: a.firstRow, firstColumn: a.firstColumn, cells: [] }),
  });
  const capabilities = makeMockCapabilityBridge({ capability_list: ALL });
  const projects = makeMockProjectBridge({ [C.current]: { projectId: null, rerun: null }, [C.list]: [] });
  const prep = makeMockPrepBridge({ stats_prep_make: made, ...responses });
  render(
    <MemoryRouter initialEntries={['/w']}>
      <Routes>
        <Route path="/w" element={<StatisticsWindow workspace={workspace} files={files} cells={cells} capabilities={capabilities} projects={projects} prep={prep} limits={() => Promise.resolve(limits8)} pollMs={15} />} />
      </Routes>
    </MemoryRouter>
  );
  return { files, prep };
}

const openMenu = (id: string) => fireEvent.click(screen.getByTestId(`gds-menu-${id}`));
const menuItem = (id: string) => screen.getByTestId(`gds-menu-item-${id}`);
/** Output.vitest.tsx's wait for an item's state, as the capability list arrives. */
const itemOff = (m: string, id: string) => {
  openMenu(m);
  const off = menuItem(id).getAttribute('aria-disabled') === 'true';
  openMenu(m);
  return off;
};
const openScore = async () => {
  for (let i = 0; i < 200 && itemOff('transform', 'transform-scale-score'); i += 1) {
    // eslint-disable-next-line no-await-in-loop
    await act(async () => {
      await new Promise((r) => setTimeout(r, 10));
    });
  }
  openMenu('transform');
  fireEvent.click(menuItem('transform-scale-score'));
  return screen.findByTestId('sw-score-dialog');
};

describe('the score dialog in the window (decisions 505 and 507)', () => {
  it('opens from Transform with a file, titled Scale score; Cancel closes it, sending nothing', async () => {
    const { prep } = setUp();
    act(() => statisticsSession.setFile(fileView));
    await openScore();
    expect(screen.getByRole('dialog', { name: 'Scale score' })).toBeTruthy();
    fireEvent.click(screen.getByTestId('sc-dialog-cancel'));
    expect(screen.queryByTestId('sw-score-dialog')).toBeNull();
    expect(prep.calls).toEqual([]);
  });

  it('a make: the dialog closes, sw-message has the counts, the Variable View shows the score, and nothing is closed', async () => {
    const { files } = setUp();
    act(() => statisticsSession.setFile(fileView));
    await openScore();
    fireEvent.change(screen.getByTestId('sc-name'), { target: { value: 'Total' } });
    fireEvent.click(screen.getByTestId('sc-item-C1'));
    fireEvent.click(screen.getByTestId('sc-item-C2'));
    fireEvent.click(screen.getByTestId('sc-statistic-sum'));
    fireEvent.click(screen.getByTestId('sc-rule-every'));
    fireEvent.click(screen.getByTestId('sc-make'));
    await waitFor(() => expect(screen.queryByTestId('sw-score-dialog')).toBeNull());
    expect(screen.getByTestId('sw-message').textContent).toBe('The score was made. "Total": 7 rows scored from every item, 0 from fewer; 2 not scored, too few items; 0 not scored, too large to hold.');
    expect(statisticsSession.get().view).toBe(preparedView);
    expect(screen.getByTestId('sw-prepared-note').textContent).toContain('1 prepared column.');
    fireEvent.click(screen.getByRole('tab', { name: /Variable View/ }));
    expect(screen.getByText(FROM_A_RECIPE)).toBeTruthy();
    expect(files.calls.map(([c]) => c)).not.toContain('stats_desc_close');
  });

  it("a deletion: the dialog closes, and sw-message names the score", async () => {
    const { files, prep } = setUp({ stats_prep_recipe: { recipe: { schemaVersion: 1, capabilityId: SCALE_SCORE_CAPABILITY, capabilityVersion: 1, engineVersion: 1, sourceColumns: 3, steps: [totalStep] }, counts: [scoreCounts] }, stats_prep_clear: fileView });
    act(() => statisticsSession.setFile(preparedView));
    await openScore();
    fireEvent.click(await screen.findByRole('button', { name: 'Delete the score "Total"' }));
    fireEvent.click(screen.getByTestId('sc-ask-continue'));
    await waitFor(() => expect(screen.queryByTestId('sw-score-dialog')).toBeNull());
    expect(screen.getByTestId('sw-message').textContent).toBe(scoreDeletedSentence('Total'));
    expect(statisticsSession.get().view).toBe(fileView);
    expect(prep.calls.map(([c]) => c)).toEqual(['stats_prep_recipe', 'stats_prep_clear']);
    expect(files.calls.map(([c]) => c)).not.toContain('stats_desc_close');
  });
});
