// Project, Export the project report… in the window (Research OS, Phase 8U-1,
// decision 526; ADR-012 Parts 5 and 9): disabled with "Open a project first."
// while no project is open, live with one; the page's one dialog, report kind
// only, for the open project; its Export saving the report through the window's
// save with its own mime, and its sentence in sw-message; Cancel calling
// nothing. Then every new sentence and label of this step read for a verdict's
// words. Every bridge is a mock; no project command is named as a string here.
import { createHash } from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import React from 'react';
import { MemoryRouter, Route, Routes } from 'react-router-dom';
import { act, cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import type { MenuDefinition, MenuItem } from '../../../design-system';
import { makeMockWorkspaceBridge } from '../../workspace/workspaceBridge';
import type { WorkspaceStateView } from '../../workspace/workspaceTypes';
import { makeMockDescribeBridge } from '../describeBridge';
import limits8 from '../fixtures/methodLimitsView8gb.json';
import { makeMockProjectBridge, PROJECT_COMMANDS as C } from '../projects/projectBridge';
import { EXPORTS } from '../projects/projectExport';
import { openProjectStore } from '../projects/projectSession';
import { EXPORT_WORDS, PROJECT_TEXT, REPORT_EXPORT_SENTENCE } from '../projects/projectWording';
import { makeMockCapabilityBridge } from '../ttestBridge';
import { StatisticsWindow } from './StatisticsWindow';
import { statisticsSession } from './statisticsSession';
import { LATER_STEP } from './windowWording';
import { windowMenus, type WindowMenuContext } from './windowMenus';
import { NO_PROJECT_OPEN, OPEN_PROJECT_TITLE } from './windowWording';

vi.mock('../../../config/featureFlags', () => ({ isFeatureEnabled: (name: string) => name === 'workspaces' || name === 'projects' }));
vi.mock('../../../utils/isTauri', () => ({ isTauri: true, default: true }));
vi.mock('../../../design-system/GaplyGlobe', () => ({ GaplyGlobe: () => <div /> }));

afterEach(() => {
  cleanup();
  statisticsSession.clear();
  openProjectStore.clear();
});

/* ---- the item ------------------------------------------------------------------- */

describe('Project, Export the project report… (decision 526)', () => {
  const context = (over: Partial<WindowMenuContext> = {}): WindowMenuContext => ({
    fileOpen: false,
    runGoing: false,
    busy: false,
    capabilities: null,
    projects: true,
    statsCheck: true,
    workspaces: true,
    actions: { open: vi.fn(), close: vi.fn(), go: vi.fn(), switchWorkspace: vi.fn(), help: vi.fn() },
    ...over,
  });
  const projectItem = (ms: MenuDefinition[], id: string): MenuItem => {
    const i = ms.find((m) => m.id === 'project')?.groups.flatMap((g) => g.items).find((x) => x.id === id);
    if (!i) throw new Error(`no ${id}`);
    return i;
  };

  it("ADR-012 Part 5's sentence, word for word", () => {
    expect(NO_PROJECT_OPEN).toBe('Open a project first.');
  });

  it('with no project open: disabled, with that sentence, and no handler', () => {
    for (const over of [{}, { projectOpen: false }, { projectOpen: undefined, fileOpen: true }]) {
      const i = projectItem(windowMenus(context(over)), 'project-report');
      expect(i.label).toBe('Export the project report…');
      expect(i.disabled).toBe(true);
      expect(i.description).toBe(NO_PROJECT_OPEN);
      expect(i.onSelect).toBeUndefined();
    }
  });

  it('with a project open: live, with or without a file and while a run goes, calling exportReport', () => {
    const exportReport = vi.fn();
    for (const over of [{ projectOpen: true }, { projectOpen: true, fileOpen: true, runGoing: true }]) {
      const i = projectItem(windowMenus(context({ ...over, actions: { open: vi.fn(), close: vi.fn(), go: vi.fn(), switchWorkspace: vi.fn(), help: vi.fn(), exportReport } })), 'project-report');
      expect(i.disabled).toBeFalsy();
      expect(i.description).toBeUndefined();
      i.onSelect?.();
    }
    expect(exportReport).toHaveBeenCalledTimes(2);
  });

  it('Run every saved result again… now: grey with no project open, live with one (decision 538)', () => {
    const none = projectItem(windowMenus(context({})), 'project-rerun-all');
    expect([none.disabled, none.description, none.onSelect]).toEqual([true, NO_PROJECT_OPEN, undefined]);
    const open = projectItem(windowMenus(context({ projectOpen: true })), 'project-rerun-all');
    expect([open.disabled, open.description, typeof open.onSelect]).toEqual([undefined, undefined, 'function']);
    expect(open.description).not.toBe(LATER_STEP);
  });
});

/* ---- in the window -------------------------------------------------------------- */

const ACTIVE: WorkspaceStateView = { state: { phase: 'active', workspace: 'statistics' }, activeRun: null, latestRun: null };
const ID = 'p'.repeat(32);
// projectReport.json's name and bytes; its sha256 is the Rust test's placeholder,
// so the view sent here carries the bytes' own, as Rust's would.
const FIXTURE = JSON.parse(fs.readFileSync(path.join(__dirname, '../../../../src-tauri/tests/fixtures/projects/projectReport.json'), 'utf8')) as { fileName: string; bytes: number[] };
const REPORT = { fileName: FIXTURE.fileName, bytes: FIXTURE.bytes, sha256: createHash('sha256').update(Buffer.from(FIXTURE.bytes)).digest('hex') };
const TEXT = new TextDecoder().decode(Uint8Array.from(REPORT.bytes));

function setUp(open: boolean, saveAnswer: string | null = '/chosen/report.md') {
  const projects = makeMockProjectBridge(
    open
      ? { [C.current]: { projectId: ID, rerun: null }, [C.list]: [{ projectId: ID, name: 'Chapter 3', bytes: 1, savedAt: 1, runCount: 1, refused: null }], [C.report]: REPORT }
      : { [C.current]: { projectId: null, rerun: null }, [C.list]: [] }
  );
  const save = vi.fn((_n: string, _t: string, _m: string) => Promise.resolve(saveAnswer));
  render(
    <MemoryRouter initialEntries={['/w']}>
      <Routes>
        <Route
          path="/w"
          element={
            <StatisticsWindow
              workspace={makeMockWorkspaceBridge({ workspace_state: ACTIVE })}
              files={makeMockDescribeBridge({ stats_desc_close: true })}
              capabilities={makeMockCapabilityBridge({ capability_list: [] })}
              projects={projects}
              save={save}
              limits={() => Promise.resolve(limits8)}
              pollMs={15}
            />
          }
        />
      </Routes>
    </MemoryRouter>
  );
  return { projects, save };
}

const openMenu = () => fireEvent.click(screen.getByTestId('gds-menu-project'));
const reportItem = () => screen.getByTestId('gds-menu-item-project-report');
const chooseReport = async () => {
  await screen.findByTestId('sw-project-open');
  openMenu();
  fireEvent.click(reportItem());
  return screen.findByTestId('project-export-dialog');
};
const calls = (b: { calls: Array<[string, Record<string, unknown>]> }, cmd: string) => b.calls.filter(([c]) => c === cmd);

describe("the window's report export (decision 526)", () => {
  it('with no project open, the item is grey with its sentence', async () => {
    setUp(false);
    await screen.findByTestId('sw-project-line');
    await act(async () => {
      await new Promise((r) => setTimeout(r, 20));
    });
    openMenu();
    expect(reportItem().getAttribute('aria-disabled')).toBe('true');
    // The item's own description (8U-2: Run every saved result again… says the same sentence now).
    expect(document.getElementById(reportItem().getAttribute('aria-describedby') ?? '')?.textContent).toBe(NO_PROJECT_OPEN);
  });

  it("with one open: the page's report dialog, its title and sentence, and nothing of provenance", async () => {
    setUp(true);
    const dialog = await chooseReport();
    expect(screen.getByRole('dialog', { name: PROJECT_TEXT.exportReportTitle })).toBe(dialog);
    expect(screen.getByTestId('project-export-sentence').textContent).toBe(REPORT_EXPORT_SENTENCE);
    expect(screen.queryByText(EXPORTS.provenance.sentence)).toBeNull();
    expect(screen.queryByText(EXPORTS.provenance.title)).toBeNull();
    expect(screen.queryByTestId('project-export-provenance')).toBeNull();
  });

  it('Cancel closes it, calling nothing and saving nothing', async () => {
    const { projects, save } = setUp(true);
    await chooseReport();
    fireEvent.click(screen.getByTestId('project-export-cancel'));
    expect(screen.queryByTestId('project-export-dialog')).toBeNull();
    expect(calls(projects, C.report)).toEqual([]);
    expect(calls(projects, C.provenance)).toEqual([]);
    expect(save).not.toHaveBeenCalled();
  });

  it("Export asks Rust for the open project's report once, saves it with the report's name, text and mime, and puts the note in sw-message", async () => {
    const { projects, save } = setUp(true);
    await chooseReport();
    fireEvent.click(screen.getByTestId('project-export-confirm'));
    await waitFor(() => expect(screen.getByTestId('sw-message').textContent).toBe(EXPORT_WORDS.saved(REPORT.sha256)));
    expect(calls(projects, C.report)).toEqual([[C.report, { projectId: ID }]]);
    expect(calls(projects, C.provenance)).toEqual([]);
    expect(save).toHaveBeenCalledTimes(1);
    expect(save).toHaveBeenCalledWith(EXPORTS.report.fileName, TEXT, EXPORTS.report.mime);
    expect(screen.queryByTestId('project-export-dialog')).toBeNull();
  });

  it("a closed save dialog: the page's sentence for it in sw-message", async () => {
    setUp(true, null);
    await chooseReport();
    fireEvent.click(screen.getByTestId('project-export-confirm'));
    await waitFor(() => expect(screen.getByTestId('sw-message').textContent).toBe(PROJECT_TEXT.exportCancelled));
  });
});

/* ---- the step's words ----------------------------------------------------------- */

/** alphaWording.vitest.ts:20, copied: the words a verdict on alpha would use (decision 384). */
const VERDICT = /\b(acceptable|unacceptable|good|excellent|questionable|poor|reliable|unreliable|adequate|sufficient)\b|0\.7\b|[*★☆]|\b(should|recommend|drop|remove)\b/i;

/** PairedFlow.vitest.tsx:135, copied: decision 66's words. */
const VERDICT_66 = /significan|[*★☆]|\breject|\bconfirm(s|ed)? that|\bproves?\b|null hypothesis|\b(t-)?test (passed|failed)\b|\bfail(s|ed)? the test\b/i;

describe("this step's new words (decision 528)", () => {
  it('carry no verdict word: alpha\'s list or decision 66\'s', () => {
    const all = [OPEN_PROJECT_TITLE, NO_PROJECT_OPEN];
    // Each is pinned word for word by its own test; here only their count, so a
    // verdict word reaches the lists below (mutation u11 first met this pin).
    expect(all).toHaveLength(2);
    for (const s of all) {
      expect(s, s).not.toMatch(VERDICT);
      expect(s, s).not.toMatch(VERDICT_66);
    }
  });
});
