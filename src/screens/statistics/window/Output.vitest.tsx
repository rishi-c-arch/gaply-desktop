// The Output (Research OS, Phase 8R, decisions 471 and 472; ADR-012 Part 7), in the
// window, over mock bridges: a run from the dialog to its entry, a run's states,
// Save and Export, names and Save after a close, Remove and the cap, and the
// sentences before anything is lost. Nothing touches Rust, a disk or a network.
import React from 'react';
import { MemoryRouter, Route, Routes } from 'react-router-dom';
import { act, cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { makeMockWorkspaceBridge } from '../../workspace/workspaceBridge';
import type { WorkspaceStateView } from '../../workspace/workspaceTypes';
import { makeMockAlphaBridge } from '../alphaBridge';
import type { AlphaRunStateView, AlphaValidationView } from '../alphaTypes';
import { makeMockDescribeBridge } from '../describeBridge';
import type { ColumnProfile, DatasetView } from '../describeTypes';
import { alphaResultView } from '../fixtures/alphaFixtures';
import { METHODS } from '../methods';
import { makeMockProjectBridge, PROJECT_COMMANDS as C } from '../projects/projectBridge';
import { openProjectStore } from '../projects/projectSession';
import { PROJECT_TEXT, PROJECT_WORDS } from '../projects/projectWording';
import { makeMockCapabilityBridge } from '../ttestBridge';
import type { CapabilityView } from '../ttestTypes';
import { MAX_OUTPUT, statisticsSession } from './statisticsSession';
import { StatisticsWindow } from './StatisticsWindow';
import { SCALE_SCORE_CAPABILITY } from './windowMenus';
import { ASK_CLOSE, ASK_LEAVE, ASK_OPEN, ASK_REMOVE, LATER_STEP, NO_FILE, OUTPUT_FULL, RUN_GOING } from './windowWording';

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
});

const ACTIVE: WorkspaceStateView = { state: { phase: 'active', workspace: 'statistics' }, activeRun: null, latestRun: null };
const NONE: WorkspaceStateView = { state: { phase: 'no_workspace' }, activeRun: null, latestRun: null };

const capability = (id: string): CapabilityView => ({
  id,
  version: 1,
  lifecycle: 'beta',
  titleCode: 'stats.capability.describe.title',
  descriptionCode: 'stats.capability.describe.description',
  licenseNotes: [],
  knownLimitations: [],
  runnable: true,
});
const ALL = [...METHODS.map((m) => capability(m.capabilityId)), capability(SCALE_SCORE_CAPABILITY)];

const col = (alias: string, header: string): ColumnProfile => ({ alias, header, kind: 'numeric', n: 9, missing: 0, distinct: 5, levels: null, suspectMissingCodes: [], nonFinite: 0 });
const fileView = (slot: number, names = ['Q1', 'Q2', 'Q3']): DatasetView => ({
  slot,
  displayName: `file${slot}.csv`,
  profile: { rows: 9, columns: names.map((n, i) => col(`C${i + 1}`, n)) },
  inputRef: { contentHash: `${slot}`.repeat(64).slice(0, 64), rows: 9, columns: names.length, semanticRole: 'table' },
});

const report = (status: AlphaValidationView['status']): AlphaValidationView => ({ status, errors: [], warnings: [], missingInformation: [], estimatedResources: { rows: 9, columns: 2, cells: 18 } });
const runState = (runId: number, state: AlphaRunStateView['state'], terminal: boolean, over: Partial<AlphaRunStateView> = {}): AlphaRunStateView => ({
  runId,
  state,
  terminal,
  elapsedMs: 1200,
  cancelRequested: false,
  stepsDone: 3,
  stepsTotal: 7,
  diagnostics: [],
  ...over,
});

interface Over {
  status?: (a: Record<string, unknown>) => AlphaRunStateView;
  alpha?: Record<string, unknown>;
  files?: Record<string, unknown>;
  state?: () => WorkspaceStateView;
  deactivate?: unknown;
  opens?: DatasetView[];
  projects?: Record<string, unknown>;
}

function setUp(o: Over = {}) {
  let runs = 0;
  let opened = 0;
  const opens = o.opens ?? [fileView(1), fileView(2, ['Other1', 'Other2', 'Other3'])];
  const workspace = makeMockWorkspaceBridge({ workspace_state: o.state ?? ACTIVE, workspace_deactivate: o.deactivate ?? NONE });
  const files = makeMockDescribeBridge({
    stats_desc_open: () => opens[Math.min(opened++, opens.length - 1)],
    stats_desc_close: true,
    stats_desc_forget_run: null,
    ...o.files,
  });
  const cells = makeMockDescribeBridge({
    stats_desc_cells: (a: Record<string, unknown>) => ({ slot: a.slot, inputHash: statisticsSession.get().view?.inputRef.contentHash, rows: 9, columns: 3, firstRow: a.firstRow, firstColumn: a.firstColumn, cells: [] }),
  });
  const alpha = makeMockAlphaBridge({
    stats_alpha_validate: report('valid'),
    stats_alpha_run: () => ({ runId: ++runs + 4 }),
    stats_alpha_status: o.status ?? ((a: Record<string, unknown>) => runState(a.runId as number, 'succeeded', true)),
    stats_alpha_result: (a: Record<string, unknown>) => ({ ...alphaResultView, runId: a.runId }),
    stats_alpha_cancel: (a: Record<string, unknown>) => runState(a.runId as number, 'running', false, { cancelRequested: true }),
    stats_alpha_export: { bytes: Array.from(new TextEncoder().encode('a,b\n')), sha256: 'cd'.repeat(32) },
    ...o.alpha,
  });
  const capabilities = makeMockCapabilityBridge({ capability_list: ALL });
  const projects = makeMockProjectBridge(
    o.projects ?? { [C.current]: { projectId: null, rerun: null }, [C.list]: [], [C.saveRun]: { projectId: 'p'.repeat(32), storedRunId: 1, created: true } }
  );
  const save = vi.fn().mockResolvedValue('/chosen/out.csv');
  render(
    <MemoryRouter initialEntries={['/w']}>
      <Routes>
        <Route
          path="/w"
          element={
            <StatisticsWindow
              workspace={workspace}
              files={files}
              forget={files}
              cells={cells}
              capabilities={capabilities}
              projects={projects}
              runs={{ alpha }}
              save={save}
              pickPath={() => Promise.resolve('/x/f.csv')}
              limits={() => Promise.resolve(null)}
              pollMs={15}
              runPollMs={5}
            />
          }
        />
        <Route path="/app/workspaces" element={<div data-testid="chooser-here" />} />
      </Routes>
    </MemoryRouter>
  );
  return { workspace, files, alpha, projects, save };
}

const openMenu = (id: string) => fireEvent.click(screen.getByTestId(`gds-menu-${id}`));
const menuItem = (id: string) => screen.getByTestId(`gds-menu-item-${id}`);
const description = (id: string) => {
  const by = menuItem(id).getAttribute('aria-describedby');
  return by ? document.getElementById(by)?.textContent : undefined;
};
const tab = (name: RegExp) => fireEvent.click(screen.getByRole('tab', { name }));
const tabNames = () => screen.getAllByRole('tab').map((t) => t.textContent);
const opened = async () => {
  fireEvent.click(await screen.findByTestId('sw-open'));
  await screen.findByTestId('sw-file-summary');
};
/** A menu item, read with its menu open and then closed again. Never inside a
 *  waitFor: toggling a menu changes the DOM, which re-fires the callback for ever. */
const readItem = (m: string, id: string) => {
  openMenu(m);
  const out = { off: menuItem(id).getAttribute('aria-disabled') === 'true', said: description(id) };
  openMenu(m);
  return out;
};
const untilItem = async (m: string, id: string, ok: (i: ReturnType<typeof readItem>) => boolean) => {
  for (let i = 0; i < 200; i += 1) {
    if (ok(readItem(m, id))) return;
    // eslint-disable-next-line no-await-in-loop
    await act(async () => {
      await new Promise((r) => setTimeout(r, 10));
    });
  }
  throw new Error(`${id} never reached its state: ${JSON.stringify(readItem(m, id))}`);
};
const openDialog = async () => {
  await untilItem('analyze', 'analyze-cronbach_alpha', (i) => !i.off);
  openMenu('analyze');
  fireEvent.click(menuItem('analyze-cronbach_alpha'));
  return screen.findByTestId('sw-method-dialog');
};
const runAlpha = async () => {
  await openDialog();
  fireEvent.click(screen.getByTestId('al-item-C1'));
  fireEvent.click(screen.getByTestId('al-item-C2'));
  fireEvent.click(screen.getByTestId('al-run'));
  await waitFor(() => expect(screen.queryByTestId('sw-method-dialog')).toBeNull());
};
const entry = (n: number) => screen.getByTestId(`sw-entry-${n}`);
const finished = async (n: number) => within(await screen.findByTestId(`sw-entry-${n}`)).findByTestId('al-result-card');

describe('the alpha menu item', () => {
  it('no file, a run going, the Output full: each its sentence; otherwise live', async () => {
    let state = ACTIVE;
    setUp({ state: () => state });
    await screen.findByTestId('sw-open');
    await untilItem('analyze', 'analyze-cronbach_alpha', (i) => i.said === NO_FILE);
    await opened();
    state = { ...ACTIVE, activeRun: { runId: 9, kind: 'ttest' }, latestRun: { runId: 9, kind: 'ttest' } };
    await untilItem('analyze', 'analyze-cronbach_alpha', (i) => i.said === RUN_GOING);
    state = ACTIVE;
    await openDialog();
  });

  it('every method is live with a file (decision 485), and so is Transform\'s scale score (decision 506)', async () => {
    setUp();
    await opened();
    for (const m of METHODS) await untilItem('analyze', `analyze-${m.kind}`, (i) => !i.off);
    await untilItem('transform', 'transform-scale-score', (i) => !i.off && i.said === undefined);
  });
});

describe('a run from the dialog', () => {
  it("opens with nothing chosen: the mockup's alpha, over the data", async () => {
    setUp();
    await opened();
    await openDialog();
    expect(screen.getByTestId('sw-method-dialog').textContent).toContain("Reliability (Cronbach's alpha)");
    expect(screen.getByTestId('al-items-none').textContent).toBe('None yet. Nothing is chosen for you.');
    expect((screen.getByTestId('al-run') as HTMLButtonElement).disabled).toBe(true);
    expect(screen.getByTestId('sw-grid'), 'the data stays behind').toBeTruthy();
  });

  it('Cancel drops the settings, and reopening starts empty', async () => {
    const { alpha } = setUp();
    await opened();
    await openDialog();
    fireEvent.click(screen.getByTestId('al-item-C1'));
    fireEvent.click(screen.getByTestId('al-dialog-cancel'));
    expect(screen.queryByTestId('sw-method-dialog')).toBeNull();
    await openDialog();
    expect((screen.getByTestId('al-item-C1') as HTMLInputElement).checked).toBe(false);
    expect(alpha.calls).toEqual([]);
  });

  it('closes, shows the Output with its count, says so, and focuses the new heading', async () => {
    setUp();
    await opened();
    await runAlpha();
    await waitFor(() => expect(tabNames()).toEqual(['Data View', 'Variable View', 'Output (1)']));
    expect(screen.getByRole('tab', { name: /Output/ }).getAttribute('aria-selected')).toBe('true');
    expect(screen.getByTestId('sw-output-status').textContent).toBe('Result 1 is shown.');
    await finished(1);
    expect(document.activeElement).toBe(screen.getByTestId('sw-entry-1-heading'));
    expect(screen.getByTestId('sw-entry-1-heading').textContent).toBe("1 Cronbach's alpha");
    expect(screen.getByTestId('sw-entry-1-settings').textContent).toBe('Settings: Items: C1 Q1, C2 Q2. Reverse-score: none. Missing codes: none.');
  });

  it('the settings line reads the reversal, its ends and the codes', async () => {
    const withCode = fileView(1);
    withCode.profile.columns[0].suspectMissingCodes = [{ code: 99, cells: 2 }];
    setUp({ opens: [withCode] });
    await opened();
    await openDialog();
    fireEvent.click(screen.getByTestId('al-item-C1'));
    fireEvent.click(screen.getByTestId('al-item-C2'));
    fireEvent.click(screen.getByTestId('al-reverse-C2'));
    fireEvent.change(screen.getByTestId('al-scale-min'), { target: { value: '1' } });
    fireEvent.change(screen.getByTestId('al-scale-max'), { target: { value: '5' } });
    fireEvent.click(screen.getByTestId('al-code-99'));
    fireEvent.click(screen.getByTestId('al-run'));
    await finished(1);
    expect(screen.getByTestId('sw-entry-1-settings').textContent).toBe(
      'Settings: Items: C1 Q1, C2 Q2. Reverse-score: C2 Q2. Lowest value on the scale: 1. Highest value on the scale: 5. Missing codes: 99.'
    );
  });
});

describe("a run's states", () => {
  it('while it runs: the state, the time, the steps and Cancel; Cancel asks, and a cancelled run has no Save', async () => {
    let cancelled = false;
    const { alpha } = setUp({
      status: (a) => (cancelled ? runState(a.runId as number, 'cancelled', true) : runState(a.runId as number, 'running', false)),
      alpha: {
        stats_alpha_cancel: (a: Record<string, unknown>) => {
          cancelled = true;
          return runState(a.runId as number, 'running', false, { cancelRequested: true });
        },
      },
    });
    await opened();
    await runAlpha();
    const e = await screen.findByTestId('sw-entry-1');
    expect(within(e).getByTestId('sw-entry-1-run-state').textContent).toBe('Running');
    expect(within(e).getByTestId('sw-entry-1-progress').textContent).toBe('3 of 7 steps');
    expect((screen.getByTestId('sw-outline-remove-1') as HTMLButtonElement).disabled).toBe(true);
    fireEvent.click(within(e).getByTestId('sw-entry-1-cancel'));
    await waitFor(() => expect(alpha.calls.some(([c]) => c === 'stats_alpha_cancel')).toBe(true));
    await within(e).findByTestId('sw-entry-1-run-ended');
    expect(within(e).getByTestId('sw-entry-1-run-state').textContent).toBe('Cancelled');
    expect(within(e).queryByTestId('sw-entry-1-project-button')).toBeNull();
    expect(within(e).queryByTestId('sw-entry-1-see-results'), 'no See the results').toBeNull();
    expect(within(e).queryByTestId('sw-entry-1-run-back'), 'no Back to settings').toBeNull();
    expect((screen.getByTestId('sw-outline-remove-1') as HTMLButtonElement).disabled).toBe(false);
  });

  it('a failure with its code: the sentence, the diagnostics, and Save to a project', async () => {
    const failed = (a: Record<string, unknown>) =>
      runState(a.runId as number, 'failed', true, { diagnostics: [{ code: 'stats.alpha.no_rows', params: {}, severity: 'error' }] as unknown as AlphaRunStateView['diagnostics'] });
    setUp({ status: failed });
    await opened();
    await runAlpha();
    const e = await screen.findByTestId('sw-entry-1');
    await within(e).findByTestId('sw-entry-1-run-ended');
    expect(within(e).getByTestId('sw-entry-1-diagnostics')).toBeTruthy();
    expect(within(e).getByTestId('sw-entry-1-project-button').textContent).toBe(PROJECT_TEXT.saveButton);
  });

  it('keeps reading a run while another tab is shown, so a finished run is not "going" for ever', async () => {
    let done = false;
    setUp({ status: (a) => (done ? runState(a.runId as number, 'succeeded', true) : runState(a.runId as number, 'running', false)) });
    await opened();
    await runAlpha();
    tab(/Data View/);
    done = true;
    await waitFor(() => expect(statisticsSession.get().output[0].result).not.toBeNull());
    tab(/Output/);
    await finished(1);
  });

  it('a status that cannot be read says so, and neither the menu nor Remove waits for ever', async () => {
    let reads = 0;
    setUp({
      status: (a) => {
        reads += 1;
        if (reads > 1) throw new Error(JSON.stringify({ code: 'run.unknown_run' }));
        return runState(a.runId as number, 'running', false);
      },
    });
    await opened();
    await runAlpha();
    expect((await screen.findByTestId('sw-entry-1-error')).textContent).not.toBe('');
    expect((screen.getByTestId('sw-outline-remove-1') as HTMLButtonElement).disabled).toBe(false);
    await untilItem('analyze', 'analyze-cronbach_alpha', (i) => !i.off);
  });
});

describe('Save and Export', () => {
  it('a Save is kept in its entry and draws the same after a remount', async () => {
    const { projects } = setUp();
    await opened();
    await runAlpha();
    const e = await screen.findByTestId('sw-entry-1');
    await finished(1);
    fireEvent.click(within(e).getByTestId('sw-entry-1-project-button'));
    fireEvent.change(await screen.findByTestId('sw-entry-1-project-name'), { target: { value: 'Chapter 3' } });
    fireEvent.click(screen.getByTestId('sw-entry-1-project-confirm'));
    await waitFor(() => expect(statisticsSession.get().output[0].saved).toEqual({ projectId: 'p'.repeat(32), name: 'Chapter 3' }));
    expect(projects.calls.filter(([c]) => c === C.saveRun)).toEqual([[C.saveRun, { runId: 5, target: { into: 'newProject', name: 'Chapter 3' } }]]);
    expect(screen.getByTestId('sw-entry-1-saved').textContent).toBe('Saved in the project Chapter 3.');
    tab(/Data View/);
    tab(/Output/);
    expect((await screen.findByTestId('sw-entry-1-saved')).textContent).toBe('Saved in the project Chapter 3.');
  });

  it('Export CSV… opens the card, CSV first, the manifest second, and keeps its note in the entry', async () => {
    const { alpha, save } = setUp();
    await opened();
    await runAlpha();
    await finished(1);
    fireEvent.click(screen.getByTestId('sw-entry-1-export'));
    expect((screen.getByTestId('sw-entry-1-export-csv') as HTMLInputElement).checked).toBe(true);
    expect((screen.getByTestId('sw-entry-1-export-manifestJson') as HTMLInputElement).checked).toBe(false);
    fireEvent.click(screen.getByTestId('sw-entry-1-save'));
    await screen.findByTestId('sw-entry-1-export-note');
    expect(alpha.calls.filter(([c]) => c === 'stats_alpha_export')).toEqual([['stats_alpha_export', { runId: 5, format: 'csv' }]]);
    expect(save).toHaveBeenCalledTimes(1);
    tab(/Data View/);
    tab(/Output/);
    expect((await screen.findByTestId('sw-entry-1-export-note')).textContent).toBe(statisticsSession.get().output[0].exportNote);
    expect(statisticsSession.get().output[0].exportNote).toContain('Saved.');
  });
});

describe('after a close', () => {
  const closeFile = () => {
    openMenu('file');
    fireEvent.click(menuItem('file-close'));
  };
  const toContinue = () => fireEvent.click(screen.getByTestId('sw-ask-continue'));

  it("names survive a close and a new file's different names", async () => {
    setUp();
    await opened();
    await runAlpha();
    await finished(1);
    closeFile();
    toContinue();
    await waitFor(() => expect(statisticsSession.get().view).toBeNull());
    tab(/Data View/);
    fireEvent.click(await screen.findByTestId('sw-open'));
    await waitFor(() => expect(statisticsSession.get().view?.displayName).toBe('file2.csv'));
    tab(/Output/);
    const row = within(await finished(1)).getByTestId('al-item-row-C1');
    expect(row.textContent).toContain('"Q1"');
    expect(row.textContent).not.toContain('Other1');
    expect(screen.getByTestId('sw-entry-1-settings').textContent).toContain('C1 Q1');
  });

  it("Save is disabled with its sentence, Export still works, and the Output stays", async () => {
    const { alpha, projects } = setUp();
    await opened();
    await runAlpha();
    await finished(1);
    closeFile();
    toContinue();
    await waitFor(() => expect(statisticsSession.get().view).toBeNull());
    expect(tabNames()[2]).toBe('Output (1)');
    const off = (await screen.findByTestId('sw-entry-1-save-off')) as HTMLButtonElement;
    expect(off.disabled).toBe(true);
    expect(off.textContent).toBe(PROJECT_TEXT.saveButton);
    expect(screen.getByTestId('sw-entry-1-save-why').textContent).toBe(PROJECT_WORDS['project.data_unavailable']({ code: 'project.data_unavailable' } as never));
    expect(screen.queryByTestId('sw-entry-1-project-button')).toBeNull();
    fireEvent.click(screen.getByTestId('sw-entry-1-export'));
    fireEvent.click(screen.getByTestId('sw-entry-1-save'));
    await screen.findByTestId('sw-entry-1-export-note');
    expect(alpha.calls.filter(([c]) => c === 'stats_alpha_export')).toHaveLength(1);
    expect(projects.calls.filter(([c]) => c === C.saveRun)).toEqual([]);
  });

  it('the disabled Save is not drawn when projects are hidden', async () => {
    flags.projects = false;
    try {
      setUp();
      await opened();
      await runAlpha();
      await finished(1);
      closeFile();
      toContinue();
      await waitFor(() => expect(statisticsSession.get().view).toBeNull());
      expect(screen.queryByTestId('sw-entry-1-save-off')).toBeNull();
      expect(screen.queryByTestId('sw-entry-1-save-why')).toBeNull();
    } finally {
      flags.projects = true;
    }
  });
});

describe('the outline', () => {
  it('lists the entries in order, newest last; a click moves the pane and the focus', async () => {
    setUp();
    await opened();
    await runAlpha();
    await finished(1);
    await runAlpha();
    await finished(2);
    const lines = within(screen.getByTestId('sw-outline')).getAllByRole('listitem');
    expect(lines.map((l) => within(l).getByTestId(/^sw-outline-go-/).textContent)).toEqual(["1 Cronbach's alpha", "2 Cronbach's alpha"]);
    expect(entry(2).hidden).toBe(false);
    fireEvent.click(screen.getByTestId('sw-outline-go-1'));
    await waitFor(() => expect(document.activeElement).toBe(screen.getByTestId('sw-entry-1-heading')));
    expect(entry(1).hidden).toBe(false);
    expect(entry(2).hidden).toBe(true);
    expect(screen.getByTestId('sw-outline-go-1').getAttribute('aria-current')).toBe('true');
  });

  it('holds a tab count that follows the entries', async () => {
    setUp();
    await opened();
    await runAlpha();
    await runAlpha();
    await waitFor(() => expect(tabNames()[2]).toBe('Output (2)'));
  });
});

describe('Remove', () => {
  it('asks first if not saved, then forgets its record and removes the entry', async () => {
    const { files } = setUp();
    await opened();
    await runAlpha();
    await finished(1);
    fireEvent.click(screen.getByTestId('sw-outline-remove-1'));
    expect(screen.getByTestId('sw-ask-sentence').textContent).toBe(ASK_REMOVE.sentence);
    expect(files.calls.filter(([c]) => c === 'stats_desc_forget_run')).toEqual([]);
    fireEvent.click(screen.getByTestId('sw-ask-cancel'));
    expect(statisticsSession.get().output).toHaveLength(1);
    fireEvent.click(screen.getByTestId('sw-outline-remove-1'));
    fireEvent.click(screen.getByTestId('sw-ask-continue'));
    await waitFor(() => expect(statisticsSession.get().output).toHaveLength(0));
    expect(files.calls.filter(([c]) => c === 'stats_desc_forget_run')).toEqual([['stats_desc_forget_run', { runId: 5 }]]);
    expect(await screen.findByTestId('sw-output-empty')).toBeTruthy();
  });

  it('asks nothing for a saved entry', async () => {
    const { files } = setUp();
    await opened();
    await runAlpha();
    await finished(1);
    act(() => statisticsSession.updateEntry(1, { saved: { projectId: 'p', name: 'P' } }));
    fireEvent.click(screen.getByTestId('sw-outline-remove-1'));
    expect(screen.queryByTestId('sw-ask-sentence')).toBeNull();
    await waitFor(() => expect(statisticsSession.get().output).toHaveLength(0));
    expect(files.calls.filter(([c]) => c === 'stats_desc_forget_run')).toHaveLength(1);
  });

  it('a record already gone is treated as done', async () => {
    const gone = () => {
      throw new Error(JSON.stringify({ code: 'run.unknown_run' }));
    };
    setUp({ files: { stats_desc_forget_run: gone } });
    await opened();
    await runAlpha();
    await finished(1);
    fireEvent.click(screen.getByTestId('sw-outline-remove-1'));
    fireEvent.click(screen.getByTestId('sw-ask-continue'));
    await waitFor(() => expect(statisticsSession.get().output).toHaveLength(0));
  });

  it('any other refusal keeps the entry and says why', async () => {
    const busy = () => {
      throw new Error(JSON.stringify({ code: 'run.not_finished' }));
    };
    setUp({ files: { stats_desc_forget_run: busy } });
    await opened();
    await runAlpha();
    await finished(1);
    fireEvent.click(screen.getByTestId('sw-outline-remove-1'));
    fireEvent.click(screen.getByTestId('sw-ask-continue'));
    await screen.findByTestId('sw-output-refusal');
    expect(statisticsSession.get().output).toHaveLength(1);
  });

  it('moves the pane to a neighbour', async () => {
    setUp();
    await opened();
    await runAlpha();
    await finished(1);
    await runAlpha();
    await finished(2);
    act(() => statisticsSession.updateEntry(2, { saved: { projectId: 'p', name: 'P' } }));
    fireEvent.click(screen.getByTestId('sw-outline-remove-2'));
    await waitFor(() => expect(statisticsSession.get().selected).toBe(1));
    expect(entry(1).hidden).toBe(false);
  });
});

describe('the cap', () => {
  const add = (n: number, v: DatasetView) => {
    for (let i = 0; i < n; i += 1) {
      statisticsSession.addEntry({ kind: 'cronbach_alpha', view: v, slot: v.slot, settings: 's', runId: 100 + i, run: runState(100 + i, 'cancelled', true) });
    }
  };

  it('holds 32; the menu says it is full and a 33rd is refused, with nothing dropped', async () => {
    expect(MAX_OUTPUT).toBe(32);
    setUp();
    await opened();
    const v = statisticsSession.get().view as DatasetView;
    act(() => add(32, v));
    expect(statisticsSession.get().output).toHaveLength(32);
    await untilItem('analyze', 'analyze-cronbach_alpha', (i) => i.said === OUTPUT_FULL);
    let added: number | null = 0;
    act(() => {
      added = statisticsSession.addEntry({ kind: 'cronbach_alpha', view: v, slot: 1, settings: 's', runId: 999, run: runState(999, 'cancelled', true) });
    });
    expect(added).toBeNull();
    expect(statisticsSession.get().output).toHaveLength(32);
    expect(statisticsSession.get().output[0].runId).toBe(100);
    expect(statisticsSession.get().output.map((e) => e.runId)).not.toContain(999);
  });
});

describe('the sentences before anything is lost', () => {
  const closeItem = () => {
    openMenu('file');
    fireEvent.click(menuItem('file-close'));
  };
  const openItem = () => {
    openMenu('file');
    fireEvent.click(menuItem('file-open'));
  };
  const switchItem = () => {
    openMenu('gaply');
    fireEvent.click(menuItem('gaply-switch'));
  };
  const saveIt = () => act(() => statisticsSession.updateEntry(1, { saved: { projectId: 'p', name: 'P' } }));
  const closes = (f: { calls: Array<[string, unknown]> }) => f.calls.filter(([c]) => c === 'stats_desc_close');
  const prepared = (slot: number): DatasetView => ({ ...fileView(slot), preparation: { recipeHash: 'a'.repeat(64) } as unknown as DatasetView['preparation'] });

  it('are ADR-012 Part 7, word for word', () => {
    expect(ASK_CLOSE.sentence).toBe(
      'Close this file? Your results stay here and can still be exported, but those you have not saved to a project can no longer be saved. Any scores you made go with the file; only a saved run keeps their recipe.'
    );
    expect(ASK_OPEN.sentence).toBe(
      'Open another file? Your results stay here and can still be exported, but those you have not saved to a project can no longer be saved. Any scores you made go with the file; only a saved run keeps their recipe.'
    );
    expect(ASK_LEAVE.sentence).toBe('Leave Statistics? Your results and scores go when you leave, unless you saved them to a project. Exported files are not affected.');
  });

  it('Close with an unsaved entry asks; Cancel keeps the file; Continue closes it and the Output stays', async () => {
    const { files } = setUp();
    await opened();
    await runAlpha();
    await finished(1);
    closeItem();
    expect(screen.getByTestId('sw-ask-sentence').textContent).toBe(ASK_CLOSE.sentence);
    fireEvent.click(screen.getByTestId('sw-ask-cancel'));
    expect(closes(files)).toEqual([]);
    expect(statisticsSession.get().view).not.toBeNull();
    closeItem();
    fireEvent.click(screen.getByTestId('sw-ask-continue'));
    await waitFor(() => expect(closes(files)).toHaveLength(1));
    await waitFor(() => expect(statisticsSession.get().view).toBeNull());
    expect(statisticsSession.get().output).toHaveLength(1);
  });

  it('Open another with an unsaved entry asks first; Cancel opens nothing', async () => {
    const { files } = setUp();
    await opened();
    await runAlpha();
    await finished(1);
    const opensBefore = files.calls.filter(([c]) => c === 'stats_desc_open').length;
    openItem();
    expect(screen.getByTestId('sw-ask-sentence').textContent).toBe(ASK_OPEN.sentence);
    fireEvent.click(screen.getByTestId('sw-ask-cancel'));
    expect(files.calls.filter(([c]) => c === 'stats_desc_open')).toHaveLength(opensBefore);
    openItem();
    fireEvent.click(screen.getByTestId('sw-ask-continue'));
    await waitFor(() => expect(files.calls.filter(([c]) => c === 'stats_desc_open')).toHaveLength(opensBefore + 1));
    await waitFor(() => expect(statisticsSession.get().view?.displayName).toBe('file2.csv'));
  });

  it('nothing is asked when everything is saved, or there is nothing', async () => {
    const { files } = setUp();
    await opened();
    closeItem();
    expect(screen.queryByTestId('sw-ask-sentence')).toBeNull();
    await waitFor(() => expect(closes(files)).toHaveLength(1));
    await opened();
    await runAlpha();
    await finished(1);
    saveIt();
    closeItem();
    expect(screen.queryByTestId('sw-ask-sentence')).toBeNull();
    await waitFor(() => expect(closes(files)).toHaveLength(2));
  });

  it('a recipe asks, with every entry saved or none', async () => {
    setUp({ opens: [prepared(1)] });
    await opened();
    closeItem();
    expect(screen.getByTestId('sw-ask-sentence').textContent).toBe(ASK_CLOSE.sentence);
  });

  it('Switch asks with an unsaved entry, then leaves, and clears the session', async () => {
    const { workspace } = setUp();
    await opened();
    await runAlpha();
    await finished(1);
    switchItem();
    expect(screen.getByTestId('sw-ask-sentence').textContent).toBe(ASK_LEAVE.sentence);
    expect(workspace.calls.filter(([c]) => c === 'workspace_deactivate')).toEqual([]);
    fireEvent.click(screen.getByTestId('sw-ask-cancel'));
    expect(workspace.calls.filter(([c]) => c === 'workspace_deactivate')).toEqual([]);
    switchItem();
    fireEvent.click(screen.getByTestId('sw-ask-continue'));
    await screen.findByTestId('chooser-here');
    expect(statisticsSession.get().output).toHaveLength(0);
  });

  it('Switch with nothing unsaved goes straight to the deactivate', async () => {
    const { workspace } = setUp();
    await opened();
    switchItem();
    expect(screen.queryByTestId('sw-ask-sentence')).toBeNull();
    await screen.findByTestId('chooser-here');
    expect(workspace.calls.filter(([c]) => c === 'workspace_deactivate')).toHaveLength(1);
  });

  it("Switch's Wait path does not ask twice", async () => {
    let refuse = true;
    let state = ACTIVE;
    const { workspace } = setUp({
      state: () => state,
      deactivate: () => {
        if (refuse) throw new Error(JSON.stringify({ code: 'workspace.run_active', params: { run: 5 } }));
        return NONE;
      },
    });
    await opened();
    await runAlpha();
    await finished(1);
    state = { ...ACTIVE, activeRun: { runId: 5, kind: 'cronbach_alpha' }, latestRun: { runId: 5, kind: 'cronbach_alpha' } };
    openMenu('gaply');
    fireEvent.click(menuItem('gaply-switch'));
    fireEvent.click(screen.getByTestId('sw-ask-continue'));
    await screen.findByTestId('ws-switch-dialog');
    refuse = false;
    state = ACTIVE;
    fireEvent.click(screen.getByTestId('ws-switch-wait'));
    await screen.findByTestId('chooser-here');
    expect(workspace.calls.filter(([c]) => c === 'workspace_deactivate').length).toBeGreaterThanOrEqual(2);
    expect(screen.queryByTestId('sw-ask-sentence')).toBeNull();
  });
});

describe("the grid's place, through the window's tabs (decision 473)", () => {
  it('stays across a change of tab and across a run, and goes with a new file', async () => {
    setUp();
    await opened();
    const grid = () => screen.getByTestId('sw-grid');
    fireEvent.scroll(grid(), { target: { scrollTop: 240, scrollLeft: 112 } });
    fireEvent.click(await screen.findByTestId('sw-cell-0-0').catch(() => grid()));
    tab(/Variable View/);
    expect(screen.queryByTestId('sw-grid')).toBeNull();
    tab(/Data View/);
    expect(grid().scrollTop).toBe(240);
    expect(grid().scrollLeft).toBe(112);
    await runAlpha();
    await finished(1);
    tab(/Data View/);
    expect(grid().scrollTop).toBe(240);
    act(() => statisticsSession.setFile(fileView(7)));
    await waitFor(() => expect(grid().scrollTop).toBe(0));
  });
});

describe('Remove asks only for what could be saved (decision 480)', () => {
  it('asks nothing for a cancelled run', async () => {
    const { files } = setUp({ status: (a) => runState(a.runId as number, 'cancelled', true) });
    await opened();
    await runAlpha();
    await screen.findByTestId('sw-entry-1-run-ended');
    fireEvent.click(screen.getByTestId('sw-outline-remove-1'));
    expect(screen.queryByTestId('sw-ask-sentence')).toBeNull();
    await waitFor(() => expect(files.calls.filter(([c]) => c === 'stats_desc_forget_run')).toHaveLength(1));
  });

  it('asks nothing for a result whose file has closed, which can no longer be saved', async () => {
    const { files } = setUp();
    await opened();
    await runAlpha();
    await finished(1);
    openMenu('file');
    fireEvent.click(menuItem('file-close'));
    fireEvent.click(screen.getByTestId('sw-ask-continue'));
    await waitFor(() => expect(statisticsSession.get().view).toBeNull());
    fireEvent.click(await screen.findByTestId('sw-outline-remove-1'));
    expect(screen.queryByTestId('sw-ask-sentence')).toBeNull();
    await waitFor(() => expect(files.calls.filter(([c]) => c === 'stats_desc_forget_run')).toHaveLength(1));
  });

  it('Close asks nothing when the only entry was cancelled', async () => {
    const { files } = setUp({ status: (a) => runState(a.runId as number, 'cancelled', true) });
    await opened();
    await runAlpha();
    await screen.findByTestId('sw-entry-1-run-ended');
    openMenu('file');
    fireEvent.click(menuItem('file-close'));
    expect(screen.queryByTestId('sw-ask-sentence')).toBeNull();
    await waitFor(() => expect(files.calls.filter(([c]) => c === 'stats_desc_close')).toHaveLength(1));
  });
});
