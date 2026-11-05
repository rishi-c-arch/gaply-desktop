// The session's one open file (Research OS, Phase 8P, decision 450; ADR-012
// Part 2): a module-level store on openProjectStore's pattern. Memory only,
// and anything but Statistics active clears it.
import { afterEach, describe, expect, it, vi } from 'vitest';

import type { DatasetView } from '../describeTypes';
import LIMITS from '../describeLimits.json';
import type { WorkspaceStateView } from '../../workspace/workspaceTypes';
import { holdsUnsavedWork, MAX_OUTPUT, sessionSlot, SLOT_GONE, statisticsSession } from './statisticsSession';

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
    expect(statisticsSession.get()).toEqual({ slot: null, view: null, output: [], selected: null, gridPlace: null, busy: false, notice: null });
  });

  it("holds the slot's number and its view, and nothing else of the table", () => {
    const v = view(7);
    statisticsSession.setFile(v);
    expect(statisticsSession.get().slot).toBe(7);
    expect(statisticsSession.get().view).toBe(v);
    expect(sessionSlot.current).toBe(7);
    expect(Object.keys(statisticsSession.get()).sort()).toEqual(['busy', 'gridPlace', 'notice', 'output', 'selected', 'slot', 'view']);
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

describe("the Output's entries (decision 471)", () => {
  const entry = (runId: number, v = view(1)) => ({
    kind: 'cronbach_alpha' as const,
    view: v,
    slot: v.slot,
    settings: 'Items: C1, C2.',
    runId,
    run: { runId, state: 'running' as const, terminal: false, elapsedMs: 0, cancelRequested: false, stepsDone: 0, stepsTotal: 7, diagnostics: [] },
  });

  it('numbers them from 1, newest last, the newest selected, and holds the very view', () => {
    const v = view(1);
    expect(statisticsSession.addEntry(entry(5, v))).toBe(1);
    expect(statisticsSession.addEntry(entry(6, v))).toBe(2);
    const s = statisticsSession.get();
    expect(s.output.map((e) => [e.number, e.runId])).toEqual([
      [1, 5],
      [2, 6],
    ]);
    expect(s.selected).toBe(2);
    expect(s.output[0].view).toBe(v);
    expect(s.output[0]).toMatchObject({ result: null, error: null, saved: null, exportOpen: false, exportFormat: 'csv', exportNote: null });
  });

  it('holds at most 32, refuses the 33rd and drops none', () => {
    expect(MAX_OUTPUT).toBe(32);
    for (let i = 0; i < MAX_OUTPUT; i += 1) expect(statisticsSession.addEntry(entry(i + 1))).toBe(i + 1);
    expect(statisticsSession.addEntry(entry(99))).toBeNull();
    expect(statisticsSession.get().output).toHaveLength(32);
    expect(statisticsSession.get().output[0].runId).toBe(1);
    expect(statisticsSession.get().output[31].runId).toBe(32);
  });

  it('does not reuse a number after a Remove; a clear starts again', () => {
    statisticsSession.addEntry(entry(1));
    statisticsSession.addEntry(entry(2));
    statisticsSession.removeEntry(2);
    expect(statisticsSession.addEntry(entry(3))).toBe(3);
    statisticsSession.clear();
    expect(statisticsSession.addEntry(entry(4))).toBe(1);
  });

  it('an update of an entry that is gone changes nothing', () => {
    const before = statisticsSession.get();
    statisticsSession.updateEntry(9, { error: 'x' });
    expect(statisticsSession.get()).toBe(before);
  });

  it('Remove moves the selection to the one after it, or the last', () => {
    for (const r of [1, 2, 3]) statisticsSession.addEntry(entry(r));
    statisticsSession.selectEntry(2);
    statisticsSession.removeEntry(2);
    expect(statisticsSession.get().selected).toBe(3);
    statisticsSession.removeEntry(3);
    expect(statisticsSession.get().selected).toBe(1);
    statisticsSession.removeEntry(1);
    expect(statisticsSession.get().selected).toBeNull();
  });

  it('a close keeps the Output; a slot gone keeps it; leaving Statistics clears it', () => {
    statisticsSession.setFile(view(1));
    statisticsSession.addEntry(entry(1));
    statisticsSession.clearFile();
    expect(statisticsSession.get().output).toHaveLength(1);
    statisticsSession.setFile(view(2));
    statisticsSession.slotGone();
    expect(statisticsSession.get().output).toHaveLength(1);
    statisticsSession.noteWorkspaceState(state({ phase: 'no_workspace' }));
    expect(statisticsSession.get().output).toHaveLength(0);
  });

  it('holdsUnsavedWork: an unsaved entry, or a recipe', () => {
    expect(holdsUnsavedWork(statisticsSession.get())).toBe(false);
    statisticsSession.setFile(view(1));
    expect(holdsUnsavedWork(statisticsSession.get())).toBe(false);
    const n = statisticsSession.addEntry(entry(1)) as number;
    expect(holdsUnsavedWork(statisticsSession.get())).toBe(true);
    statisticsSession.updateEntry(n, { saved: { projectId: 'p', name: 'P' } });
    expect(holdsUnsavedWork(statisticsSession.get())).toBe(false);
    statisticsSession.setFile({ ...view(1), preparation: { recipeHash: 'a' } as unknown as DatasetView['preparation'] });
    expect(holdsUnsavedWork(statisticsSession.get())).toBe(true);
  });
});

describe('the records behind the Output (decision 468)', () => {
  it('outnumber its entries, so a going rerun never pushes out a live result', () => {
    expect(LIMITS.maxRuns).toBe(40);
    expect(LIMITS.maxRuns).toBeGreaterThan(MAX_OUTPUT);
  });
});
