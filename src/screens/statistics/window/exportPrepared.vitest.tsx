// File, Export the prepared data (CSV)… (Research OS, Phase 8T-2b, decisions 514
// and 517; ADR-011 Part 8; ADR-012 Part 8). The item's states in their order, a
// run going never disabling it; then in the window: asked first with ADR-011
// Part 8's sentence word for word, Cancel calling nothing, Continue calling csv
// once and the save once with prepared_data.csv, a closed save dialog's note,
// run.unknown_input calling slotGone only under the generation-and-slot check,
// and any other refusal its sentence in sw-error. Every bridge is a mock.
import React from 'react';
import { MemoryRouter, Route, Routes } from 'react-router-dom';
import { act, cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import type { MenuDefinition, MenuItem } from '../../../design-system';
import { makeMockWorkspaceBridge } from '../../workspace/workspaceBridge';
import type { WorkspaceStateView } from '../../workspace/workspaceTypes';
import { makeMockDescribeBridge } from '../describeBridge';
import type { ColumnProfile, DatasetView, ExportView } from '../describeTypes';
import { shortHash } from '../describeWording';
import limits8 from '../fixtures/methodLimitsView8gb.json';
import { METHODS } from '../methods';
import { makeMockPrepBridge } from '../prepBridge';
import { prepRefusalSentence } from '../prepWording';
import { makeMockProjectBridge, PROJECT_COMMANDS as C } from '../projects/projectBridge';
import { openProjectStore } from '../projects/projectSession';
import { makeMockCapabilityBridge } from '../ttestBridge';
import type { CapabilityView } from '../ttestTypes';
import { StatisticsWindow } from './StatisticsWindow';
import { statisticsSession } from './statisticsSession';
import { SCALE_SCORE_CAPABILITY, windowMenus, type WindowMenuContext } from './windowMenus';
import { SLOT_GONE } from './statisticsSession';
import { ASK_EXPORT_PREPARED, EXPORT_PREPARED_NAME, FILE_BUSY, NOT_RUNNABLE, NO_FILE, NO_SCORE } from './windowWording';

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
const LABEL = 'Export the prepared data (CSV)…';

/* ---- the item ----------------------------------------------------------------- */

describe("File's export item (decision 514)", () => {
  const context = (over: Partial<WindowMenuContext> = {}): WindowMenuContext => ({
    fileOpen: true,
    prepared: true,
    runGoing: false,
    busy: false,
    capabilities: ALL,
    projects: true,
    statsCheck: true,
    workspaces: true,
    actions: { open: vi.fn(), close: vi.fn(), go: vi.fn(), switchWorkspace: vi.fn(), help: vi.fn() },
    ...over,
  });
  const exportItem = (ms: MenuDefinition[]): MenuItem => {
    const f = ms.find((m) => m.id === 'file');
    const i = f?.groups.flatMap((g) => g.items).find((x) => x.id === 'file-export');
    if (!i) throw new Error('no export item');
    return i;
  };
  const notRunnable = ALL.map((c) => (c.id === SCALE_SCORE_CAPABILITY ? { ...c, runnable: false } : c));

  it("ADR-012 Part 5's sentence, word for word", () => {
    expect(NO_SCORE).toBe('Make a scale score first.');
  });

  it('with a file that holds a score, it is live, keeps its label, and calls the export', () => {
    const exportPrepared = vi.fn();
    const i = exportItem(windowMenus(context({ actions: { open: vi.fn(), close: vi.fn(), go: vi.fn(), switchWorkspace: vi.fn(), help: vi.fn(), exportPrepared } })));
    expect(i.label).toBe(LABEL);
    expect(i.disabled).toBeFalsy();
    expect(i.description).toBeUndefined();
    i.onSelect?.();
    expect(exportPrepared).toHaveBeenCalledTimes(1);
  });

  it('says why not, in its order: not runnable, then no file, then no score, then busy', () => {
    expect(exportItem(windowMenus(context({ capabilities: notRunnable, fileOpen: false, prepared: false, busy: true }))).description).toBe(NOT_RUNNABLE);
    expect(exportItem(windowMenus(context({ capabilities: null }))).description).toBe(NOT_RUNNABLE);
    expect(exportItem(windowMenus(context({ capabilities: ALL.filter((c) => c.id !== SCALE_SCORE_CAPABILITY) }))).description).toBe(NOT_RUNNABLE);
    expect(exportItem(windowMenus(context({ fileOpen: false, prepared: false, busy: true }))).description).toBe(NO_FILE);
    expect(exportItem(windowMenus(context({ prepared: false, busy: true }))).description).toBe(NO_SCORE);
    expect(exportItem(windowMenus(context({ prepared: undefined }))).description).toBe(NO_SCORE);
    expect(exportItem(windowMenus(context({ busy: true }))).description).toBe(FILE_BUSY);
    for (const o of [{ capabilities: notRunnable }, { fileOpen: false }, { prepared: false }, { busy: true }]) {
      const i = exportItem(windowMenus(context(o)));
      expect(i.disabled).toBe(true);
      expect(i.onSelect).toBeUndefined();
    }
  });

  it('a run going does not disable it, nor a full Output: the command only reads', () => {
    for (const o of [{ runGoing: true }, { outputFull: true }, { runGoing: true, outputFull: true }]) {
      const i = exportItem(windowMenus(context(o)));
      expect(i.disabled).toBeFalsy();
      expect(i.description).toBeUndefined();
    }
  });

  it('with no export action given, choosing it does nothing', () => {
    expect(() => exportItem(windowMenus(context())).onSelect?.()).not.toThrow();
  });
});

/* ---- in the window ------------------------------------------------------------ */

const ACTIVE: WorkspaceStateView = { state: { phase: 'active', workspace: 'statistics' }, activeRun: null, latestRun: null };
const col = (alias: string, header: string, extra: Partial<ColumnProfile> = {}): ColumnProfile => ({ alias, header, kind: 'numeric', n: 9, missing: 0, distinct: 5, levels: null, suspectMissingCodes: [], nonFinite: 0, ...extra });
const fileView: DatasetView = {
  slot: 1,
  displayName: 'items.csv',
  profile: { rows: 9, columns: [col('C1', 'Q1'), col('C2', 'Q2'), col('C3', 'Q3')] },
  inputRef: { contentHash: 'ab'.repeat(32), rows: 9, columns: 3, semanticRole: 'table' },
};
const preparedView: DatasetView = {
  ...fileView,
  profile: { rows: 9, columns: [...fileView.profile.columns, col('C4', 'C4', { prepared: true, scoreName: 'Total' })] },
  inputRef: { ...fileView.inputRef, contentHash: 'cd'.repeat(32), columns: 4 },
  preparation: { sourceHash: 'ab'.repeat(32), recipeHash: 'ef'.repeat(32), sourceColumns: 3, capabilityVersion: 1, engineVersion: 1 },
  scoreCounts: [{ column: 'C4', scoredFromEveryItem: 9, scoredFromFewer: 0, notScoredTooFew: 0, notScoredOverflow: 0 }],
};
const TEXT = 'Q1,Q2,Q3,Total\r\n1,2,3,6\r\n-1.5,,2,\r\n';
const CSV: ExportView = { bytes: Array.from(new TextEncoder().encode(TEXT)), sha256: '0123456789abcdef'.repeat(4) };
const refusal = (code: string) => new Error(JSON.stringify({ code, params: {}, severity: 'error' }));

function deferred<T>() {
  let resolve: (v: T) => void = () => undefined;
  let reject: (e: unknown) => void = () => undefined;
  const promise = new Promise<T>((res, rej) => {
    resolve = res;
    reject = rej;
  });
  return { promise, resolve, reject };
}

function setUp(csv: unknown = CSV, saveAnswer: string | null = '/somewhere/prepared_data.csv') {
  const workspace = makeMockWorkspaceBridge({ workspace_state: ACTIVE });
  const files = makeMockDescribeBridge({ stats_desc_close: true });
  const cells = makeMockDescribeBridge({
    stats_desc_cells: (a: Record<string, unknown>) => ({ slot: a.slot, inputHash: 'cd'.repeat(32), rows: 9, columns: 4, firstRow: a.firstRow, firstColumn: a.firstColumn, cells: [] }),
  });
  const capabilities = makeMockCapabilityBridge({ capability_list: ALL });
  const projects = makeMockProjectBridge({ [C.current]: { projectId: null, rerun: null }, [C.list]: [] });
  const prep = makeMockPrepBridge({ stats_prep_csv: csv });
  const save = vi.fn((_name: string, _text: string, _mime: string) => Promise.resolve(saveAnswer));
  render(
    <MemoryRouter initialEntries={['/w']}>
      <Routes>
        <Route path="/w" element={<StatisticsWindow workspace={workspace} files={files} cells={cells} capabilities={capabilities} projects={projects} prep={prep} save={save} limits={() => Promise.resolve(limits8)} pollMs={15} />} />
      </Routes>
    </MemoryRouter>
  );
  act(() => statisticsSession.setFile(preparedView));
  return { prep, save, files };
}

const openMenu = (id: string) => fireEvent.click(screen.getByTestId(`gds-menu-${id}`));
const menuItem = (id: string) => screen.getByTestId(`gds-menu-item-${id}`);
const itemOff = (m: string, id: string) => {
  openMenu(m);
  const off = menuItem(id).getAttribute('aria-disabled') === 'true';
  openMenu(m);
  return off;
};
/** Chooses the item once the capability list has made it live. */
const chooseExport = async () => {
  for (let i = 0; i < 200 && itemOff('file', 'file-export'); i += 1) {
    // eslint-disable-next-line no-await-in-loop
    await act(async () => {
      await new Promise((r) => setTimeout(r, 10));
    });
  }
  openMenu('file');
  fireEvent.click(menuItem('file-export'));
  return screen.findByTestId('sw-ask-export');
};
const settle = () =>
  act(async () => {
    await new Promise((r) => setTimeout(r, 20));
  });

describe('the export in the window (decision 514)', () => {
  it('asks first, titled and with ADR-011 Part 8\'s sentence word for word, under a test id of its own', async () => {
    setUp();
    await chooseExport();
    expect(ASK_EXPORT_PREPARED.title).toBe('Export the prepared data');
    expect(ASK_EXPORT_PREPARED.sentence).toBe(
      "This file holds your data as Gaply read it, with your scores added. Missing answers are empty, SPSS's value labels are not included, and numbers are written in full. Your original file is not changed."
    );
    expect(screen.getByRole('dialog', { name: 'Export the prepared data' })).toBeTruthy();
    expect(screen.getByTestId('sw-ask-export-sentence').textContent).toBe(ASK_EXPORT_PREPARED.sentence);
    expect(screen.queryByTestId('sw-ask')).toBeNull();
  });

  it('Cancel writes nothing and calls nothing', async () => {
    const { prep, save } = setUp();
    await chooseExport();
    fireEvent.click(screen.getByTestId('sw-ask-export-cancel'));
    await settle();
    expect(screen.queryByTestId('sw-ask-export')).toBeNull();
    expect(prep.calls).toEqual([]);
    expect(save).not.toHaveBeenCalled();
    expect(screen.getByTestId('sw-message').textContent).toBe('');
  });

  it("Continue calls csv once for the slot, then the save once with prepared_data.csv, Rust's text and text/csv; the note goes to sw-message", async () => {
    const { prep, save } = setUp();
    await chooseExport();
    fireEvent.click(screen.getByTestId('sw-ask-export-continue'));
    await waitFor(() => expect(screen.getByTestId('sw-message').textContent).toBe(`Saved. SHA-256 of the file: ${shortHash(CSV.sha256)}`));
    expect(EXPORT_PREPARED_NAME).toBe('prepared_data.csv');
    expect(prep.calls).toEqual([['stats_prep_csv', { slot: 1 }]]);
    expect(save).toHaveBeenCalledTimes(1);
    expect(save).toHaveBeenCalledWith('prepared_data.csv', TEXT, 'text/csv');
    expect(screen.queryByTestId('sw-ask-export')).toBeNull();
    expect(screen.queryByTestId('sw-error')).toBeNull();
    expect(statisticsSession.get().busy).toBe(false);
  });

  it('a closed save dialog: its note, and nothing else', async () => {
    const { save } = setUp(CSV, null);
    await chooseExport();
    fireEvent.click(screen.getByTestId('sw-ask-export-continue'));
    await waitFor(() => expect(screen.getByTestId('sw-message').textContent).toBe('Not saved: the save dialog was closed.'));
    expect(save).toHaveBeenCalledTimes(1);
    expect(screen.queryByTestId('sw-error')).toBeNull();
    expect(statisticsSession.get().view).toBe(preparedView);
  });

  it('run.unknown_input from csv: slotGone, and no save dialog', async () => {
    const { save } = setUp(() => {
      throw refusal('run.unknown_input');
    });
    await chooseExport();
    fireEvent.click(screen.getByTestId('sw-ask-export-continue'));
    await waitFor(() => expect(statisticsSession.get().notice).toBe(SLOT_GONE));
    expect(statisticsSession.get().slot).toBeNull();
    expect(save).not.toHaveBeenCalled();
    expect(screen.queryByTestId('sw-error')).toBeNull();
  });

  it('run.unknown_input after the session moved on is dropped: no slotGone, no save, nothing said', async () => {
    const d = deferred<ExportView>();
    const { save } = setUp(() => d.promise);
    await chooseExport();
    fireEvent.click(screen.getByTestId('sw-ask-export-continue'));
    act(() => statisticsSession.clear());
    await act(async () => {
      d.reject(refusal('run.unknown_input'));
      await d.promise.catch(() => undefined);
    });
    await settle();
    expect(statisticsSession.get().notice).toBeNull();
    expect(save).not.toHaveBeenCalled();
    expect(screen.getByTestId('sw-message').textContent).toBe('');
  });

  it('a csv answer after another file was opened: no save dialog opens and nothing is said', async () => {
    const d = deferred<ExportView>();
    const { save } = setUp(() => d.promise);
    await chooseExport();
    fireEvent.click(screen.getByTestId('sw-ask-export-continue'));
    act(() => statisticsSession.setFile({ ...fileView, slot: 2 }));
    await act(async () => {
      d.resolve(CSV);
      await d.promise;
    });
    await settle();
    expect(save).not.toHaveBeenCalled();
    expect(screen.getByTestId('sw-message').textContent).toBe('');
    expect(screen.queryByTestId('sw-error')).toBeNull();
  });

  it("any other refusal: its sentence in sw-error, the one paragraph, and no save", async () => {
    const e = refusal('run.run_busy');
    const { save } = setUp(() => {
      throw e;
    });
    await chooseExport();
    fireEvent.click(screen.getByTestId('sw-ask-export-continue'));
    await waitFor(() => expect(screen.getByTestId('sw-error').textContent).toBe(prepRefusalSentence(e, preparedView)));
    expect(screen.getAllByRole('alert').filter((n) => n.getAttribute('data-testid') === 'sw-error')).toHaveLength(1);
    expect(save).not.toHaveBeenCalled();
    expect(statisticsSession.get().view).toBe(preparedView);
  });

  it('bytes that are not UTF-8: the uncoded sentence in sw-error, and no save', async () => {
    const { save } = setUp({ bytes: [0xff, 0xfe], sha256: CSV.sha256 });
    await chooseExport();
    fireEvent.click(screen.getByTestId('sw-ask-export-continue'));
    await waitFor(() => expect(screen.getByTestId('sw-error').textContent).toBe(prepRefusalSentence(new Error('not coded'))));
    expect(save).not.toHaveBeenCalled();
  });

  it('with no score, the item says so and nothing can be asked', async () => {
    setUp();
    act(() => statisticsSession.setFile(fileView));
    await settle();
    openMenu('file');
    expect(menuItem('file-export').getAttribute('aria-disabled')).toBe('true');
    expect(screen.getByText(NO_SCORE)).toBeTruthy();
  });
});
