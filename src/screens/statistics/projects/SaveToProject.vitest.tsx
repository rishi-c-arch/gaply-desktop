// Save to a project (Research OS, Phase 5B-3a; decisions 105, 121 to 123,
// 133, 136, 139, 140 and 142), over a mock project bridge: the two choices,
// the disabled states, every name refusal from the shared vectors, a
// success, every refusal code and the fallback, no call without a click on
// Save, one call for a double click, a response after the screen has gone
// dropped, the keyboard and focus, and React StrictMode.
import fs from 'node:fs';
import path from 'node:path';
import React from 'react';
import { act, cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { makeMockProjectBridge } from './projectBridge';
import { openProjectStore } from './projectSession';
import { PROJECT_CODES } from './projectTypes';
import { FALLBACK_SENTENCE, NAME_WORDS, PROJECT_WORDS, SAVE_SENTENCE } from './projectWording';
import { savableOnScreen, SaveToProject } from './SaveToProject';
import type { NameRefusal } from './projectName';

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

const ID = '0123456789abcdef0123456789abcdef';
const refusal = (v: unknown) => ({ code: 'validation', message: JSON.stringify(v) });

function deferred<T>() {
  let resolve!: (v: T) => void;
  let reject!: (e: unknown) => void;
  const promise = new Promise<T>((res, rej) => {
    resolve = res;
    reject = rej;
  });
  return { promise, resolve, reject };
}

function setup(saveRun: unknown = { projectId: ID, storedRunId: 1, created: true }, wrap = false, open: { projectId: string; name: string } | null = null) {
  const bridge = makeMockProjectBridge({
    project_save_run: saveRun,
    // Phase 5B-3b (decision 148): the dialog asks which project Rust has
    // open, and the list for its name, when it appears.
    project_current: { projectId: open ? open.projectId : null },
    project_list: open ? [{ projectId: open.projectId, name: open.name, bytes: 4096, savedAt: 1792396351, runCount: 1, refused: null }] : [],
  });
  const ui = <SaveToProject runId={5} bridge={bridge} desktop />;
  const view = render(wrap ? <React.StrictMode>{ui}</React.StrictMode> : ui);
  return { bridge, view };
}

const button = () => screen.getByTestId('project-save-button') as HTMLButtonElement;
const confirm = () => screen.getByTestId('project-save-confirm') as HTMLButtonElement;
const nameField = () => screen.getByTestId('project-save-name') as HTMLInputElement;
const saves = (b: ReturnType<typeof makeMockProjectBridge>) => b.calls.filter(([c]) => c === 'project_save_run');

describe('which runs offer Save (decision 136)', () => {
  it('a run that Succeeded, or Failed with its code', () => {
    expect(savableOnScreen({ state: 'succeeded', diagnostics: [] })).toBe(true);
    expect(savableOnScreen({ state: 'failed', diagnostics: [{ severity: 'warning' }, { severity: 'error' }] })).toBe(true);
    expect(savableOnScreen({ state: 'failed', diagnostics: [{ severity: 'warning' }] })).toBe(false);
    for (const s of ['cancelled', 'refused', 'timed_out', 'resource_limit', 'needs_user_input', 'running', 'queued']) {
      expect(savableOnScreen({ state: s, diagnostics: [{ severity: 'error' }] }), s).toBe(false);
    }
  });
});

describe('the Save dialog', () => {
  it('with no open project: one choice, a new project, focus in its name field, and the save sentence', async () => {
    const { bridge } = setup();
    fireEvent.click(button());
    const dialog = await screen.findByTestId('project-save-dialog');
    expect(dialog.getAttribute('role')).toBe('dialog');
    expect(dialog.getAttribute('aria-modal')).toBe('true');
    expect(screen.getByTestId('project-save-sentence').textContent).toBe(SAVE_SENTENCE);
    expect(screen.queryByTestId('project-save-choice-open')).toBeNull();
    expect((screen.getByTestId('project-save-choice-new') as HTMLInputElement).checked).toBe(true);
    expect(document.activeElement).toBe(nameField());
    expect(screen.getByLabelText('Name of the new project')).toBe(nameField());
    expect(confirm().disabled).toBe(true);
    fireEvent.change(nameField(), { target: { value: '  Chapter 3  ' } });
    expect(confirm().disabled).toBe(false);
    expect(saves(bridge)).toHaveLength(0);
  });

  it('with an open project: two choices, none chosen, Save disabled until one is', () => {
    openProjectStore.set({ projectId: ID, name: 'Thesis' });
    const { bridge } = setup({ projectId: ID, storedRunId: 2, created: false }, false, { projectId: ID, name: 'Thesis' });
    fireEvent.click(button());
    const open = screen.getByTestId('project-save-choice-open') as HTMLInputElement;
    const fresh = screen.getByTestId('project-save-choice-new') as HTMLInputElement;
    expect(open.checked || fresh.checked).toBe(false);
    expect(screen.getByTestId('project-save-open-name').tagName).toBe('BDI');
    expect(screen.getByTestId('project-save-open-name').textContent).toBe('Thesis');
    expect(confirm().disabled).toBe(true);
    fireEvent.click(fresh);
    expect(confirm().disabled).toBe(true);
    fireEvent.click(open);
    expect(confirm().disabled).toBe(false);
    expect(saves(bridge)).toHaveLength(0);
  });

  const VECTORS = (
    JSON.parse(
      fs.readFileSync(path.join(__dirname, '../../../../src-tauri/gaply-core/tests/fixtures/research_project/name_vectors.json'), 'utf8')
    ) as { vectors: Array<{ input: string; accepted: boolean; reason?: NameRefusal; why: string }> }
  ).vectors.filter((v) => !v.accepted);

  it.each(VECTORS.map((v) => [v.why, v] as const))('refuses %s with its sentence, before any call', (_why, v) => {
    const { bridge } = setup();
    fireEvent.click(button());
    // Typed first, so that an empty name is a change too.
    fireEvent.change(nameField(), { target: { value: 'x' } });
    fireEvent.change(nameField(), { target: { value: v.input } });
    const why = screen.getByTestId('project-save-name-why');
    expect(why.textContent).toBe(NAME_WORDS[v.reason as NameRefusal]);
    expect(why.getAttribute('role')).toBe('alert');
    expect(nameField().getAttribute('aria-invalid')).toBe('true');
    expect(confirm().disabled).toBe(true);
    fireEvent.click(confirm());
    expect(saves(bridge)).toHaveLength(0);
  });

  it('saves into a new project on the click, names it, and makes it the open one', async () => {
    const { bridge } = setup();
    fireEvent.click(button());
    fireEvent.change(nameField(), { target: { value: '  Chapter 3  ' } });
    fireEvent.click(confirm());
    await waitFor(() => expect(screen.queryByTestId('project-save-dialog')).toBeNull());
    expect(saves(bridge)).toEqual([['project_save_run', { runId: 5, target: { into: 'newProject', name: 'Chapter 3' } }]]);
    const status = screen.getByTestId('project-save-status');
    expect(status.getAttribute('aria-live')).toBe('polite');
    expect(status.textContent).toBe('Saved. This run is the first in the new project Chapter 3.');
    expect(screen.getByTestId('project-save-saved-name').tagName).toBe('BDI');
    expect(openProjectStore.get()).toEqual({ projectId: ID, name: 'Chapter 3' });
    expect(document.activeElement).toBe(button());
  });

  it('saves into the open project', async () => {
    openProjectStore.set({ projectId: ID, name: 'Thesis' });
    const { bridge } = setup({ projectId: ID, storedRunId: 2, created: false }, false, { projectId: ID, name: 'Thesis' });
    fireEvent.click(button());
    fireEvent.click(screen.getByTestId('project-save-choice-open'));
    fireEvent.click(confirm());
    await waitFor(() => expect(screen.getByTestId('project-save-status').textContent).toBe('Saved. This run was added to the project Thesis.'));
    expect(saves(bridge)).toEqual([['project_save_run', { runId: 5, target: { into: 'openProject', projectId: ID } }]]);
  });

  it.each(PROJECT_CODES.map((c) => [c]))('shows %s as its sentence and stays open', async (code) => {
    const { bridge } = setup(() => {
      throw refusal({ code, needed: 2, limit: 1 });
    });
    fireEvent.click(button());
    fireEvent.change(nameField(), { target: { value: 'Chapter 3' } });
    fireEvent.click(confirm());
    const shown = await screen.findByTestId('project-save-refusal');
    expect(shown.textContent).toBe(PROJECT_WORDS[code]({ code, needed: 2, limit: 1 }));
    expect(screen.getByTestId('project-save-dialog')).toBeTruthy();
    expect(saves(bridge)).toHaveLength(1);
    expect(openProjectStore.get()).toBeNull();
  });

  it('shows the fallback for a refusal it does not know, never the message', async () => {
    setup(() => {
      throw new Error('RAW_SENTINEL_x1 /Users/someone/a.csv');
    });
    fireEvent.click(button());
    fireEvent.change(nameField(), { target: { value: 'Chapter 3' } });
    fireEvent.click(confirm());
    expect((await screen.findByTestId('project-save-refusal')).textContent).toBe(FALLBACK_SENTENCE);
    expect(document.body.innerHTML).not.toContain('RAW_SENTINEL');
  });

  it('calls nothing but its reads without a click on Save: opening, typing, choosing, Escape and Cancel', () => {
    openProjectStore.set({ projectId: ID, name: 'Thesis' });
    const { bridge } = setup(undefined, false, { projectId: ID, name: 'Thesis' });
    fireEvent.click(button());
    fireEvent.click(screen.getByTestId('project-save-choice-new'));
    fireEvent.change(nameField(), { target: { value: 'Chapter 3' } });
    fireEvent.click(screen.getByTestId('project-save-choice-open'));
    fireEvent.keyDown(screen.getByTestId('project-save-dialog'), { key: 'Escape' });
    expect(screen.queryByTestId('project-save-dialog')).toBeNull();
    fireEvent.click(button());
    fireEvent.click(screen.getByTestId('project-save-cancel'));
    // Each time the dialog appeared it read which project is open, and the
    // list (decision 148); nothing else.
    expect(bridge.calls.map(([c]) => c)).toEqual(['project_current', 'project_list', 'project_current', 'project_list']);
  });

  it('makes one call for a double click, with Save disabled while it is pending', async () => {
    const d = deferred<unknown>();
    const { bridge } = setup(() => d.promise);
    fireEvent.click(button());
    fireEvent.change(nameField(), { target: { value: 'Chapter 3' } });
    fireEvent.click(confirm());
    fireEvent.click(confirm());
    expect(confirm().disabled).toBe(true);
    expect(screen.getByTestId('project-save-pending').textContent).toBe('Saving…');
    await act(async () => d.resolve({ projectId: ID, storedRunId: 1, created: true }));
    expect(saves(bridge)).toHaveLength(1);
  });

  it('makes one call for two clicks in one event turn, before Save re-renders as disabled', async () => {
    // The disabled button alone would stop a second click that comes after a
    // render; two clicks inside one act() land before it, so only the
    // in-flight check can hold (found by a deletion test that stayed green).
    const d = deferred<unknown>();
    const { bridge } = setup(() => d.promise);
    fireEvent.click(button());
    fireEvent.change(nameField(), { target: { value: 'Chapter 3' } });
    const b = confirm();
    act(() => {
      b.click();
      b.click();
    });
    await act(async () => d.resolve({ projectId: ID, storedRunId: 1, created: true }));
    expect(saves(bridge)).toHaveLength(1);
  });

  it('drops a response that arrives after the screen has gone', async () => {
    const d = deferred<unknown>();
    const { view } = setup(() => d.promise);
    fireEvent.click(button());
    fireEvent.change(nameField(), { target: { value: 'Chapter 3' } });
    fireEvent.click(confirm());
    view.unmount();
    await act(async () => d.resolve({ projectId: ID, storedRunId: 1, created: true }));
    expect(openProjectStore.get()).toBeNull();
  });

  it('keeps Tab inside the dialog, and Escape returns the focus to Save', () => {
    setup();
    fireEvent.click(button());
    const dialog = screen.getByTestId('project-save-dialog');
    fireEvent.change(nameField(), { target: { value: 'Chapter 3' } });
    const confirmButton = confirm();
    confirmButton.focus();
    fireEvent.keyDown(dialog, { key: 'Tab' });
    expect(dialog.contains(document.activeElement)).toBe(true);
    expect(document.activeElement).not.toBe(confirmButton);
    (document.activeElement as HTMLElement).focus();
    const first = document.activeElement;
    fireEvent.keyDown(dialog, { key: 'Tab', shiftKey: true });
    expect(document.activeElement).toBe(confirmButton);
    expect(first).not.toBe(confirmButton);
    fireEvent.keyDown(dialog, { key: 'Escape' });
    expect(screen.queryByTestId('project-save-dialog')).toBeNull();
    expect(document.activeElement).toBe(button());
  });

  it('under React StrictMode, one click makes one save call', async () => {
    const { bridge } = setup(undefined, true);
    fireEvent.click(button());
    fireEvent.change(nameField(), { target: { value: 'Chapter 3' } });
    fireEvent.click(confirm());
    await waitFor(() => expect(screen.queryByTestId('project-save-dialog')).toBeNull());
    expect(saves(bridge)).toHaveLength(1);
    // The rest are the dialog's reads (decision 148), made again by
    // StrictMode's second effect run.
    expect(bridge.calls.filter(([c]) => c !== 'project_current' && c !== 'project_list')).toHaveLength(1);
  });

  it('is not there when the helper says no', () => {
    flags.projects = false;
    const { bridge } = setup();
    expect(screen.queryByTestId('project-save')).toBeNull();
    expect(bridge.calls).toEqual([]);
  });
});
