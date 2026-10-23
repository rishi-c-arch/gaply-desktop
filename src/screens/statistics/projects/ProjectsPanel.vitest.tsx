// The Projects panel and its delete dialog (Research OS, Phase 5B-3a;
// decisions 103, 104, 107, 124, 132 to 136 and 139), over mock project and
// workspace bridges: the list, an unreadable project, the totals, the
// loading, empty and error states, "Open now" only while Statistics is
// active, and the delete dialog: focus on Cancel, Escape, a delete, every
// refusal, "open in another copy", focus after a delete, and StrictMode.
import React from 'react';
import { act, cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { makeMockWorkspaceBridge } from '../../workspace/workspaceBridge';
import type { WorkspaceStateView } from '../../workspace/workspaceTypes';
import { makeMockProjectBridge } from './projectBridge';
import { openProjectStore } from './projectSession';
import { PROJECT_CODES, type ProjectListEntryView, type ProjectSizesView } from './projectTypes';
import { DELETE_SENTENCE, PROJECT_TEXT, PROJECT_WORDS, savedOn } from './projectWording';
import { ProjectsPanel } from './ProjectsPanel';

const flags = vi.hoisted(() => ({ projects: true, workspaces: true }));
vi.mock('../../../config/featureFlags', () => ({
  isFeatureEnabled: (name: string) => (name === 'projects' && flags.projects) || (name === 'workspaces' && flags.workspaces),
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
  { projectId: A, name: 'Chapter 3', bytes: 3 * 1048576, savedAt: 1792396351, runCount: 2, refused: null },
  { projectId: B, name: null, bytes: 4096, savedAt: null, runCount: null, refused: 'project.db_unreadable' },
];
const SIZES: ProjectSizesView = { totalBytes: 3 * 1048576 + 4096, projectCount: 2, projectCapBytes: 104857600, allProjectsCapBytes: 10737418240 };
const CONFIRMATION = 'ab'.repeat(32);
const refusal = (v: unknown) => ({ code: 'validation', message: JSON.stringify(v) });

function setup(responses: Record<string, unknown> = {}, state: WorkspaceStateView = ACTIVE, strict = false) {
  const bridge = makeMockProjectBridge({
    project_list: LIST,
    project_sizes: SIZES,
    project_prepare_delete: ({ projectId }: Record<string, unknown>) => ({ projectId, confirmation: CONFIRMATION, bytes: 4096, expiresInMs: 300000 }),
    project_delete: ({ projectId }: Record<string, unknown>) => ({ projectId, closed: false }),
    // Phase 5B-3b (decision 148): the panel asks which project is open.
    project_current: { projectId: null },
    ...responses,
  });
  const workspace = makeMockWorkspaceBridge({ workspace_state: state });
  const ui = <ProjectsPanel bridge={bridge} workspace={workspace} desktop />;
  const view = render(strict ? <React.StrictMode>{ui}</React.StrictMode> : ui);
  return { bridge, workspace, view };
}

const commands = (b: ReturnType<typeof makeMockProjectBridge>, cmd: string) => b.calls.filter(([c]) => c === cmd);

describe('the Projects panel', () => {
  it('lists each project: its name as text in <bdi>, size, saved date and saved runs; and the totals', async () => {
    setup();
    const row = await screen.findByTestId(`project-row-${A}`);
    const name = screen.getByTestId(`project-name-${A}`);
    expect(name.tagName).toBe('BDI');
    expect(name.textContent).toBe('Chapter 3');
    expect(screen.getByTestId(`project-facts-${A}`).textContent).toBe(`Size: 3.15 MB. Saved ${savedOn(1792396351)}. 2 saved runs.`);
    expect(within(row).getByTestId(`project-delete-${A}`)).toBeTruthy();
    expect(screen.getByTestId('projects-totals').textContent).toBe(
      'Your projects use 3.15 MB of their 10 GB limit. A project can hold up to 100 MB.'
    );
  });

  it('shows a project that cannot be read by its sentence and size, and it can still be deleted', async () => {
    setup();
    const shown = await screen.findByTestId(`project-unreadable-${B}`);
    expect(shown.textContent).toBe(`${PROJECT_TEXT.unreadable} ${PROJECT_WORDS['project.db_unreadable']({ code: 'project.db_unreadable' })}`);
    expect(screen.getByTestId(`project-facts-${B}`).textContent).toBe('Size: under 0.01 MB.');
    fireEvent.click(screen.getByTestId(`project-delete-${B}`));
    expect(screen.getByTestId('project-delete-name').textContent).toBe(PROJECT_TEXT.unnamed);
  });

  it('has a loading, an empty and an error state', async () => {
    let release: (v: unknown) => void = () => undefined;
    const pending = new Promise((r) => {
      release = r;
    });
    setup({ project_list: () => pending });
    expect(screen.getByTestId('projects-loading').textContent).toBe(PROJECT_TEXT.loading);
    await act(async () => release([]));
    expect((await screen.findByTestId('projects-empty')).textContent).toBe(PROJECT_TEXT.empty);
    cleanup();

    setup({
      project_list: () => {
        throw new Error('RAW_SENTINEL_p2');
      },
    });
    expect((await screen.findByTestId('projects-unread')).textContent).toBe(PROJECT_TEXT.unread);
    expect(document.body.innerHTML).not.toContain('RAW_SENTINEL');
  });

  it('marks the open project only while Statistics is active (decision 135)', async () => {
    setup({ project_current: { projectId: A } });
    expect(await screen.findByTestId(`project-open-${A}`)).toBeTruthy();
    cleanup();
    setup({ project_current: { projectId: A } }, NONE);
    await screen.findByTestId(`project-row-${A}`);
    await waitFor(() => expect(screen.queryByTestId(`project-open-${A}`)).toBeNull());
    expect(openProjectStore.get()).toBeNull();
  });

  it('reads the list, the sizes, which project is open and the workspace state, and calls nothing else, under StrictMode too', async () => {
    const { bridge } = setup({}, ACTIVE, true);
    await screen.findByTestId(`project-row-${A}`);
    const names = bridge.calls.map(([c]) => c);
    expect(names.every((c) => c === 'project_list' || c === 'project_sizes' || c === 'project_current')).toBe(true);
  });

  it('is not there when the helper says no, and calls nothing', () => {
    flags.workspaces = false;
    const { bridge, workspace } = setup();
    expect(screen.queryByTestId('projects-panel')).toBeNull();
    expect(bridge.calls).toEqual([]);
    expect(workspace.calls).toEqual([]);
  });
});

describe('the delete dialog', () => {
  async function openDialog(id = A) {
    fireEvent.click(await screen.findByTestId(`project-delete-${id}`));
    return screen.getByTestId('project-delete-dialog');
  }

  it('shows the name, the size and the sentence, with the focus on Cancel; Escape cancels and returns the focus', async () => {
    const { bridge } = setup();
    const dialog = await openDialog();
    expect(dialog.getAttribute('role')).toBe('dialog');
    expect(within(dialog).getByTestId('project-delete-name').querySelector('bdi')?.textContent).toBe('Chapter 3');
    expect(within(dialog).getByTestId('project-delete-size').textContent).toBe('Size: 3.15 MB.');
    expect(within(dialog).getByTestId('project-delete-sentence').textContent).toBe(DELETE_SENTENCE);
    expect(document.activeElement).toBe(screen.getByTestId('project-delete-cancel'));
    fireEvent.keyDown(dialog, { key: 'Escape' });
    expect(screen.queryByTestId('project-delete-dialog')).toBeNull();
    expect(document.activeElement).toBe(screen.getByTestId(`project-delete-${A}`));
    expect(commands(bridge, 'project_prepare_delete')).toHaveLength(0);
    expect(commands(bridge, 'project_delete')).toHaveLength(0);
  });

  it('deletes on its click: prepare, then delete with that confirmation; then the focus is on the heading', async () => {
    openProjectStore.set({ projectId: A, name: 'Chapter 3' });
    const { bridge } = setup({ project_delete: ({ projectId }: Record<string, unknown>) => ({ projectId, closed: true }) });
    await openDialog();
    fireEvent.click(screen.getByTestId('project-delete-confirm'));
    await waitFor(() => expect(screen.queryByTestId('project-delete-dialog')).toBeNull());
    expect(commands(bridge, 'project_prepare_delete')).toEqual([['project_prepare_delete', { projectId: A }]]);
    expect(commands(bridge, 'project_delete')).toEqual([['project_delete', { projectId: A, confirmation: CONFIRMATION }]]);
    expect(screen.getByTestId('projects-message').textContent).toBe(PROJECT_TEXT.deleted);
    expect(screen.getByTestId('projects-message').getAttribute('aria-live')).toBe('polite');
    expect(document.activeElement).toBe(screen.getByTestId('projects-heading'));
    expect(openProjectStore.get()).toBeNull();
    await waitFor(() => expect(commands(bridge, 'project_list')).toHaveLength(2));
  });

  it('says another copy has it open, and stays open', async () => {
    setup({
      project_delete: () => {
        throw refusal({ code: 'project.open_elsewhere' });
      },
    });
    await openDialog();
    fireEvent.click(screen.getByTestId('project-delete-confirm'));
    expect((await screen.findByTestId('project-delete-refusal')).textContent).toBe(
      'Another copy of Gaply has this project open, so it was not deleted. Close it there first.'
    );
    expect(screen.getByTestId('project-delete-dialog')).toBeTruthy();
    expect((screen.getByTestId('project-delete-confirm') as HTMLButtonElement).disabled).toBe(false);
  });

  // Decision 175 (Research OS, 5B-4b): after a refusal the dialog stays open
  // so the refusal can be read; nothing said it was finished. Cancel now
  // reads Close and takes the focus; Delete stays, to try again.
  describe('after a refusal', () => {
    const refused = async () => {
      setup({
        project_delete: () => {
          throw refusal({ code: 'project.open_elsewhere' });
        },
      });
      await openDialog();
      // As a keyboard user does: Tab to Delete, then press it. A click alone
      // moves no focus, so the focus would still be on Cancel from when the
      // dialog opened, and the check below could not tell whether the dialog
      // moved it (a deletion test of the focus move stayed green that way).
      const del = screen.getByTestId('project-delete-confirm');
      del.focus();
      fireEvent.click(del);
      await screen.findByTestId('project-delete-refusal');
    };

    it('Cancel reads Close and has the focus', async () => {
      await refused();
      const close = screen.getByTestId('project-delete-cancel') as HTMLButtonElement;
      await waitFor(() => expect(close.disabled).toBe(false));
      expect(close.textContent).toBe('Close');
      expect(PROJECT_TEXT.close).toBe('Close the project');
      await waitFor(() => expect(document.activeElement).toBe(close));
    });

    it('still shows the refusal and an enabled Delete', async () => {
      await refused();
      expect(screen.getByTestId('project-delete-refusal').textContent).toBe(
        'Another copy of Gaply has this project open, so it was not deleted. Close it there first.'
      );
      await waitFor(() => expect((screen.getByTestId('project-delete-confirm') as HTMLButtonElement).disabled).toBe(false));
    });

    it('Close closes it, and the focus goes back to the project\'s Delete button', async () => {
      await refused();
      await waitFor(() => expect((screen.getByTestId('project-delete-cancel') as HTMLButtonElement).disabled).toBe(false));
      fireEvent.click(screen.getByTestId('project-delete-cancel'));
      expect(screen.queryByTestId('project-delete-dialog')).toBeNull();
      expect(document.activeElement).toBe(screen.getByTestId(`project-delete-${A}`));
    });

    it('Escape closes it, and the focus goes back to the project\'s Delete button', async () => {
      await refused();
      await waitFor(() => expect((screen.getByTestId('project-delete-cancel') as HTMLButtonElement).disabled).toBe(false));
      fireEvent.keyDown(screen.getByTestId('project-delete-dialog'), { key: 'Escape' });
      expect(screen.queryByTestId('project-delete-dialog')).toBeNull();
      expect(document.activeElement).toBe(screen.getByTestId(`project-delete-${A}`));
    });
  });

  it.each(PROJECT_CODES.map((c) => [c]))('shows %s from either call as its sentence', async (code) => {
    for (const which of ['project_prepare_delete', 'project_delete']) {
      setup({
        [which]: () => {
          throw refusal({ code, needed: 2, limit: 1 });
        },
      });
      await openDialog();
      fireEvent.click(screen.getByTestId('project-delete-confirm'));
      expect((await screen.findByTestId('project-delete-refusal')).textContent).toBe(PROJECT_WORDS[code]({ code, needed: 2, limit: 1 }));
      cleanup();
    }
  });

  it('makes one prepare and one delete for a double click, under StrictMode', async () => {
    let release: (v: unknown) => void = () => undefined;
    const pending = new Promise((r) => {
      release = r;
    });
    const { bridge } = setup({ project_delete: () => pending }, ACTIVE, true);
    await openDialog();
    fireEvent.click(screen.getByTestId('project-delete-confirm'));
    fireEvent.click(screen.getByTestId('project-delete-confirm'));
    await waitFor(() => expect(commands(bridge, 'project_delete')).toHaveLength(1));
    expect((screen.getByTestId('project-delete-confirm') as HTMLButtonElement).disabled).toBe(true);
    expect((screen.getByTestId('project-delete-cancel') as HTMLButtonElement).disabled).toBe(true);
    await act(async () => release({ projectId: A, closed: false }));
    expect(commands(bridge, 'project_prepare_delete')).toHaveLength(1);
    expect(commands(bridge, 'project_delete')).toHaveLength(1);
  });

  it('makes one prepare for two clicks in one event turn, before Delete re-renders as disabled', async () => {
    let release: (v: unknown) => void = () => undefined;
    const pending = new Promise((r) => {
      release = r;
    });
    const { bridge } = setup({ project_prepare_delete: () => pending });
    await openDialog();
    const b = screen.getByTestId('project-delete-confirm');
    act(() => {
      b.click();
      b.click();
    });
    await act(async () => release({ projectId: A, confirmation: CONFIRMATION, bytes: 4096, expiresInMs: 300000 }));
    await waitFor(() => expect(commands(bridge, 'project_delete')).toHaveLength(1));
    expect(commands(bridge, 'project_prepare_delete')).toHaveLength(1);
  });

  it('drops a response that arrives after the panel has gone', async () => {
    openProjectStore.set({ projectId: A, name: 'Chapter 3' });
    let release: (v: unknown) => void = () => undefined;
    const pending = new Promise((r) => {
      release = r;
    });
    const { view } = setup({ project_delete: () => pending, project_current: { projectId: A } });
    await openDialog();
    fireEvent.click(screen.getByTestId('project-delete-confirm'));
    view.unmount();
    await act(async () => release({ projectId: A, closed: true }));
    expect(openProjectStore.get()).toEqual({ projectId: A, name: 'Chapter 3' });
  });
});
