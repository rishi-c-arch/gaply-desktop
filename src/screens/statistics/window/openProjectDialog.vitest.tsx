// Project, Open a project… (Research OS, Phase 8U-1, decision 525; ADR-012
// Part 9): the item live in the Project menu; a window dialog titled "Open a
// project" holding ProjectsPanel unchanged, its list, sizes and Delete with it;
// its Close; its Open closing the dialog and going to the project's page; and
// the panel's Delete dialog, nested inside it, keeping Tab inside itself.
// Every bridge is a mock; no project command is named as a string here.
import React from 'react';
import { MemoryRouter, Route, Routes, useLocation } from 'react-router-dom';
import { cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import type { MenuDefinition, MenuItem } from '../../../design-system';
import { makeMockWorkspaceBridge } from '../../workspace/workspaceBridge';
import type { WorkspaceStateView } from '../../workspace/workspaceTypes';
import { makeMockDescribeBridge } from '../describeBridge';
import limits8 from '../fixtures/methodLimitsView8gb.json';
import { makeMockProjectBridge, PROJECT_COMMANDS as C } from '../projects/projectBridge';
import { PROJECT_PAGE_PATH, projectPagePath } from '../projects/projectRoute';
import { openProjectStore } from '../projects/projectSession';
import { makeMockCapabilityBridge } from '../ttestBridge';
import { StatisticsWindow } from './StatisticsWindow';
import { statisticsSession } from './statisticsSession';
import { windowMenus, type WindowMenuContext } from './windowMenus';
import { OPEN_PROJECT_TITLE } from './windowWording';

vi.mock('../../../config/featureFlags', () => ({ isFeatureEnabled: (name: string) => name === 'workspaces' || name === 'projects' }));
vi.mock('../../../utils/isTauri', () => ({ isTauri: true, default: true }));
vi.mock('../../../design-system/GaplyGlobe', () => ({ GaplyGlobe: () => <div /> }));

afterEach(() => {
  cleanup();
  statisticsSession.clear();
  openProjectStore.clear();
});

/* ---- the item ------------------------------------------------------------------- */

describe("Project, Open a project… (decision 525)", () => {
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
  const openItem = (ms: MenuDefinition[]): MenuItem => {
    const i = ms.find((m) => m.id === 'project')?.groups.flatMap((g) => g.items).find((x) => x.id === 'project-open');
    if (!i) throw new Error('no Open a project item');
    return i;
  };

  it('is live, with or without a file and while a run goes, and calls openProject', () => {
    const openProject = vi.fn();
    for (const over of [{}, { fileOpen: true }, { fileOpen: true, runGoing: true }, { busy: true }]) {
      const i = openItem(windowMenus(context({ ...over, actions: { open: vi.fn(), close: vi.fn(), go: vi.fn(), switchWorkspace: vi.fn(), help: vi.fn(), openProject } })));
      expect(i.label).toBe('Open a project…');
      expect(i.disabled, JSON.stringify(over)).toBeFalsy();
      expect(i.description).toBeUndefined();
      i.onSelect?.();
    }
    expect(openProject).toHaveBeenCalledTimes(4);
  });

  it('is not there when projects are not shown, as the whole group', () => {
    expect(windowMenus(context({ projects: false })).find((m) => m.id === 'project')).toBeUndefined();
  });
});

/* ---- the dialog ----------------------------------------------------------------- */

const ACTIVE: WorkspaceStateView = { state: { phase: 'active', workspace: 'statistics' }, activeRun: null, latestRun: null };
const A = 'a'.repeat(32);
const B = 'b'.repeat(32);
const LIST = [
  { projectId: A, name: 'Chapter 3', bytes: 40000, savedAt: 1792396351, runCount: 2, refused: null },
  { projectId: B, name: 'Pilot', bytes: 20000, savedAt: 1792396300, runCount: 1, refused: null },
];

function setUp() {
  const seen: string[] = [];
  const Where: React.FC = () => {
    const l = useLocation();
    seen.push(`${l.pathname}${JSON.stringify(l.state)}`);
    return null;
  };
  const projects = makeMockProjectBridge({
    [C.current]: { projectId: null, rerun: null },
    [C.list]: LIST,
    [C.sizes]: { totalBytes: 60000, projectCount: 2, projectCapBytes: 104857600, allProjectsCapBytes: 10737418240 },
    [C.open]: (a: Record<string, unknown>) => ({ projectId: a.projectId, name: 'Chapter 3', createdAt: 1792396000, runCount: 2, bytes: 40000 }),
  });
  render(
    <MemoryRouter initialEntries={['/w']}>
      <Where />
      <Routes>
        <Route
          path="/w"
          element={
            <StatisticsWindow
              workspace={makeMockWorkspaceBridge({ workspace_state: ACTIVE })}
              files={makeMockDescribeBridge({ stats_desc_close: true })}
              capabilities={makeMockCapabilityBridge({ capability_list: [] })}
              projects={projects}
              limits={() => Promise.resolve(limits8)}
              pollMs={15}
            />
          }
        />
        <Route path={PROJECT_PAGE_PATH} element={<div data-testid="page-here" />} />
      </Routes>
    </MemoryRouter>
  );
  return { seen, projects };
}

const openDialog = async () => {
  fireEvent.click(screen.getByTestId('gds-menu-project'));
  fireEvent.click(screen.getByTestId('gds-menu-item-project-open'));
  return screen.findByTestId('sw-open-project-dialog');
};
const calls = (b: { calls: Array<[string, Record<string, unknown>]> }, cmd: string) => b.calls.filter(([c]) => c === cmd);

describe('the Open a project dialog (decision 525)', () => {
  it('is titled "Open a project" and holds the panel: its heading, list, sizes and totals', async () => {
    setUp();
    const dialog = await openDialog();
    expect(OPEN_PROJECT_TITLE).toBe('Open a project');
    expect(screen.getByRole('dialog', { name: 'Open a project' })).toBe(dialog);
    expect(within(dialog).getByTestId('projects-panel')).toBeTruthy();
    expect(within(dialog).getByTestId('projects-heading').textContent).toBe('Projects');
    expect(await within(dialog).findByTestId(`project-row-${A}`)).toBeTruthy();
    expect(within(dialog).getByTestId(`project-row-${B}`)).toBeTruthy();
    expect(within(dialog).getByTestId('projects-totals')).toBeTruthy();
    expect(within(dialog).getByTestId(`project-delete-${A}`)).toBeTruthy();
  });

  it('Close closes it, and nothing is opened', async () => {
    const { projects } = setUp();
    const dialog = await openDialog();
    await within(dialog).findByTestId(`project-row-${A}`);
    const close = within(dialog).getByTestId('sw-open-project-close');
    expect(close.textContent).toBe('Close');
    fireEvent.click(close);
    expect(screen.queryByTestId('sw-open-project-dialog')).toBeNull();
    expect(calls(projects, C.open)).toEqual([]);
  });

  it('Escape closes it too', async () => {
    setUp();
    const dialog = await openDialog();
    fireEvent.keyDown(dialog, { key: 'Escape' });
    expect(screen.queryByTestId('sw-open-project-dialog')).toBeNull();
  });

  it("Open opens the project once, closes the dialog and goes to its page, the line following", async () => {
    const { projects, seen } = setUp();
    const dialog = await openDialog();
    fireEvent.click(await within(dialog).findByTestId(`project-open-button-${A}`));
    expect(await screen.findByTestId('page-here')).toBeTruthy();
    expect(calls(projects, C.open)).toEqual([[C.open, { projectId: A }]]);
    expect(seen[seen.length - 1]).toBe(`${projectPagePath(A)}null`);
    expect(screen.queryByTestId('sw-open-project-dialog')).toBeNull();
    expect(openProjectStore.get()?.projectId).toBe(A);
  });
});

describe('the Delete dialog nested inside it keeps Tab to itself (decision 525)', () => {
  const inside = async () => {
    setUp();
    const outer = await openDialog();
    fireEvent.click(await within(outer).findByTestId(`project-delete-${A}`));
    const inner = await screen.findByTestId('project-delete-dialog');
    await waitFor(() => expect(inner.contains(document.activeElement)).toBe(true));
    return { outer, inner };
  };

  it('Tab from a control inside the Delete dialog stays inside it', async () => {
    const { inner } = await inside();
    const at = document.activeElement as HTMLElement;
    fireEvent.keyDown(at, { key: 'Tab' });
    expect(inner.contains(document.activeElement)).toBe(true);
  });

  it('Shift+Tab from its first control wraps to its last, not out to the Open dialog', async () => {
    const { inner } = await inside();
    const cancel = within(inner).getByTestId('project-delete-cancel');
    cancel.focus();
    fireEvent.keyDown(cancel, { key: 'Tab', shiftKey: true });
    expect(document.activeElement).toBe(within(inner).getByTestId('project-delete-confirm'));
  });

  it('Tab from its last control wraps to its first', async () => {
    const { inner } = await inside();
    const confirm = within(inner).getByTestId('project-delete-confirm');
    confirm.focus();
    fireEvent.keyDown(confirm, { key: 'Tab' });
    expect(inner.contains(document.activeElement)).toBe(true);
    expect(document.activeElement).not.toBe(confirm);
  });

  it('Escape in the Delete dialog closes it alone, the Open dialog staying', async () => {
    const { inner } = await inside();
    fireEvent.keyDown(inner, { key: 'Escape' });
    expect(screen.queryByTestId('project-delete-dialog')).toBeNull();
    expect(screen.getByTestId('sw-open-project-dialog')).toBeTruthy();
  });
});
