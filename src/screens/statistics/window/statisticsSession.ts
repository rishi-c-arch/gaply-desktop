// Gaply — the Statistics window's session (Research OS, Phase 8P, decision
// 450; ADR-012 Part 2), on openProjectStore's pattern
// (projects/projectSession.ts). One open file: its slot's number and its
// DatasetView. The Output's entries (Phase 8R, decision 471), and the grid's
// place (decision 473), beside them.
//
// Memory only: never browser storage and never logged, and never a cell (a
// DatasetView holds none). It survives a visit to Home, Settings or the project
// page, because the file stays open in Rust. It is cleared whenever the window
// sees that Statistics is not active, and by the switch after a deactivate.
// Rust's own session decides what is held, whatever this says.
import { useSyncExternalStore } from 'react';
import { isActive, type WorkspaceStateView } from '../../workspace/workspaceTypes';
import type { AlphaResultView, AlphaRunStateView } from '../alphaTypes';
import type { DatasetView, ExportFormat } from '../describeTypes';
import type { GridCell } from './gridRange';

/** The sentence for a slot the window finds gone (ADR-012 Part 2). */
export const SLOT_GONE = 'This file is no longer held in memory. Open it again.';

/** The Output holds at most this many; none is ever dropped by itself. */
export const MAX_OUTPUT = 32;

/** One result, with all it shows, so a remount draws the same (decision 471). */
export interface OutputEntry {
  number: number;
  kind: 'cronbach_alpha';
  /** That DatasetView itself, so its names survive a close, an open or a new recipe. */
  view: DatasetView;
  /** The slot its run used: a Save needs that one still open. */
  slot: number;
  settings: string;
  runId: number;
  run: AlphaRunStateView;
  result: AlphaResultView | null;
  /** Why the run could not be read, or why an action on it was refused. */
  error: string | null;
  saved: { projectId: string; name: string } | null;
  exportOpen: boolean;
  exportFormat: ExportFormat;
  exportNote: string | null;
}

export type NewOutputEntry = Pick<OutputEntry, 'kind' | 'view' | 'slot' | 'settings' | 'runId' | 'run'>;

/** The grid's place: numbers only, never a cell (decision 473). */
export interface GridPlace {
  /** The slot and the hash, as the grid's own key. */
  key: string;
  top: number;
  left: number;
  active: GridCell | null;
}

export interface StatisticsSessionState {
  /** The open slot's number; null for no file. */
  slot: number | null;
  /** What Rust said of that slot. */
  view: DatasetView | null;
  output: readonly OutputEntry[];
  /** The entry the result pane shows. */
  selected: number | null;
  gridPlace: GridPlace | null;
  /** A picker, an open or a close is going. */
  busy: boolean;
  /** Why the file went, when it did not go by the user's own act. */
  notice: string | null;
}

const EMPTY: StatisticsSessionState = { slot: null, view: null, output: [], selected: null, gridPlace: null, busy: false, notice: null };

let current: StatisticsSessionState = EMPTY;
/** How many times the session has been cleared: an open that began before a
 *  clear can tell, since its file is not this session's. */
let cleared = 0;
/** The last entry's number. A Remove does not give a number back. */
let numbered = 0;
const listeners = new Set<() => void>();

function replace(next: Partial<StatisticsSessionState>) {
  current = { ...current, ...next };
  listeners.forEach((l) => l());
}

/** Something would be lost: an entry not saved, or a recipe (decision 472). */
export function holdsUnsavedWork(s: StatisticsSessionState): boolean {
  return s.output.some((e) => e.saved === null) || s.view?.preparation !== undefined;
}

/** Rust is still working on it, as far as this session can tell. */
export function runIsGoing(e: OutputEntry): boolean {
  return !e.run.terminal && e.error === null;
}

export const statisticsSession = {
  get(): StatisticsSessionState {
    return current;
  },
  generation(): number {
    return cleared;
  },
  /** After an open: this is the session's file. */
  setFile(view: DatasetView) {
    replace({ slot: view.slot, view, notice: null, gridPlace: null });
  },
  /** After a close. The Output stays. */
  clearFile() {
    if (current.slot === null && current.view === null) return;
    replace({ slot: null, view: null, gridPlace: null });
  },
  /** Rust no longer holds the slot: the file goes, and the screen says so.
   *  8Q's window command is the first caller. */
  slotGone() {
    replace({ slot: null, view: null, notice: SLOT_GONE, gridPlace: null });
  },
  /** Statistics is not active, or the switch has left it. */
  clear() {
    cleared += 1;
    numbered = 0;
    if (current.slot === null && current.view === null && current.output.length === 0 && current.notice === null) return;
    replace({ slot: null, view: null, output: [], selected: null, gridPlace: null, notice: null });
  },
  setBusy(busy: boolean) {
    if (current.busy !== busy) replace({ busy });
  },
  /** The new entry's number, newest last and selected; null when the Output is
   *  full, with nothing dropped. */
  addEntry(entry: NewOutputEntry): number | null {
    if (current.output.length >= MAX_OUTPUT) return null;
    numbered += 1;
    const number = numbered;
    const made: OutputEntry = { ...entry, number, result: null, error: null, saved: null, exportOpen: false, exportFormat: 'csv', exportNote: null };
    replace({ output: [...current.output, made], selected: number });
    return number;
  },
  updateEntry(number: number, patch: Partial<Omit<OutputEntry, 'number'>>) {
    if (!current.output.some((e) => e.number === number)) return;
    replace({ output: current.output.map((e) => (e.number === number ? { ...e, ...patch } : e)) });
  },
  /** The only thing that ever removes one entry. */
  removeEntry(number: number) {
    const at = current.output.findIndex((e) => e.number === number);
    if (at < 0) return;
    const output = current.output.filter((e) => e.number !== number);
    const neighbour = output[Math.min(at, output.length - 1)];
    replace({ output, selected: current.selected === number ? neighbour?.number ?? null : current.selected });
  },
  selectEntry(number: number) {
    if (current.selected !== number) replace({ selected: number });
  },
  setGridPlace(place: GridPlace) {
    replace({ gridPlace: place });
  },
  /** What the workspace state says: anything but Statistics active means no
   *  session, on projectSession.ts's rule. */
  noteWorkspaceState(view: WorkspaceStateView) {
    if (!isActive(view.state, 'statistics')) statisticsSession.clear();
  },
  subscribe(listener: () => void): () => void {
    listeners.add(listener);
    return () => {
      listeners.delete(listener);
    };
  },
};

/** The session's slot as flowHooks' fileActions reads it. A write does nothing:
 *  fileActions then calls onOpened or onClosed, and those tell the store. */
export const sessionSlot: { current: number | null } = {
  get current(): number | null {
    return current.slot;
  },
  set current(_slot: number | null) {
    // The store takes the slot from setFile and clearFile.
  },
};

export function useStatisticsSession(): StatisticsSessionState {
  return useSyncExternalStore(statisticsSession.subscribe, statisticsSession.get, statisticsSession.get);
}
