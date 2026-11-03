// The session's one open file (Research OS, Phase 8P, decision 450; ADR-012
// Part 2): a module-level store on openProjectStore's pattern. Memory only,
// and anything but Statistics active clears it.
import { afterEach, describe, expect, it, vi } from 'vitest';

import type { DatasetView } from '../describeTypes';
import type { WorkspaceStateView } from '../../workspace/workspaceTypes';
import { sessionSlot, SLOT_GONE, statisticsSession } from './statisticsSession';

const view = (slot: number): DatasetView => ({
  slot,
  displayName: 'a.csv',
  profile: { rows: 3, columns: [{ alias: 'C1', header: 'x', kind: 'numeric', n: 3, missing: 0, distinct: 3, levels: null, suspectMissingCodes: [], nonFinite: 0 }] },
  inputRef: { contentHash: 'ab'.repeat(32), rows: 3, columns: 1, semanticRole: 'table' },
});
const state = (s: WorkspaceStateView['state']): WorkspaceStateView => ({ state: s, activeRun: null, latestRun: null });

afterEach(() => statisticsSession.clear());

describe('statisticsSession', () => {
  it('starts empty, with an empty list of Output entries', () => {
    expect(statisticsSession.get()).toEqual({ slot: null, view: null, output: [], busy: false, notice: null });
  });

  it("holds the slot's number and its view, and nothing else of the table", () => {
    const v = view(7);
    statisticsSession.setFile(v);
    expect(statisticsSession.get().slot).toBe(7);
    expect(statisticsSession.get().view).toBe(v);
    expect(sessionSlot.current).toBe(7);
    expect(Object.keys(statisticsSession.get()).sort()).toEqual(['busy', 'notice', 'output', 'slot', 'view']);
  });

  it('a new file replaces the one before it', () => {
    statisticsSession.setFile(view(1));
    statisticsSession.setFile(view(2));
    expect(statisticsSession.get().slot).toBe(2);
  });

  it('closing the file drops the file', () => {
    statisticsSession.setFile(view(1));
    statisticsSession.clearFile();
    expect(statisticsSession.get().slot).toBeNull();
    expect(statisticsSession.get().view).toBeNull();
    expect(sessionSlot.current).toBeNull();
  });

  it.each([
    ['no workspace', { phase: 'no_workspace' }],
    ['Statistics activating', { phase: 'activating', workspace: 'statistics' }],
    ['Statistics deactivating', { phase: 'deactivating', workspace: 'statistics' }],
    ['Compute active', { phase: 'active', workspace: 'compute' }],
    ['Design active', { phase: 'active', workspace: 'design' }],
  ] as Array<[string, WorkspaceStateView['state']]>)('%s clears the session', (_name, s) => {
    statisticsSession.setFile(view(1));
    statisticsSession.noteWorkspaceState(state(s));
    expect(statisticsSession.get().view).toBeNull();
  });

  it('Statistics active keeps the session', () => {
    const v = view(1);
    statisticsSession.setFile(v);
    statisticsSession.noteWorkspaceState(state({ phase: 'active', workspace: 'statistics' }));
    expect(statisticsSession.get().view).toBe(v);
  });

  it("a slot found gone clears the file and says the sentence; opening a file ends the sentence", () => {
    expect(SLOT_GONE).toBe('This file is no longer held in memory. Open it again.');
    statisticsSession.setFile(view(1));
    statisticsSession.slotGone();
    expect(statisticsSession.get().view).toBeNull();
    expect(statisticsSession.get().notice).toBe(SLOT_GONE);
    statisticsSession.setFile(view(2));
    expect(statisticsSession.get().notice).toBeNull();
  });

  it('a clear moves the generation on, so an open that began before it can tell', () => {
    const g = statisticsSession.generation();
    statisticsSession.clearFile();
    expect(statisticsSession.generation()).toBe(g);
    statisticsSession.clear();
    expect(statisticsSession.generation()).toBe(g + 1);
  });

  it('tells its listeners of a change and of nothing else, and gives a new snapshot only on a change', () => {
    const l = vi.fn();
    const off = statisticsSession.subscribe(l);
    const before = statisticsSession.get();
    statisticsSession.clear();
    expect(l).not.toHaveBeenCalled();
    expect(statisticsSession.get()).toBe(before);
    statisticsSession.setFile(view(1));
    expect(l).toHaveBeenCalledTimes(1);
    expect(statisticsSession.get()).not.toBe(before);
    off();
    statisticsSession.clear();
    expect(l).toHaveBeenCalledTimes(1);
  });
});
