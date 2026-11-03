// Gaply — the Statistics window's session (Research OS, Phase 8P, decision
// 450; ADR-012 Part 2), on openProjectStore's pattern
// (projects/projectSession.ts). One open file: its slot's number and its
// DatasetView. The Output's entries join it in 8R.
//
// Memory only: never browser storage and never logged, and never a cell (a
// DatasetView holds none). It survives a visit to Home, Settings or the project
// page, because the file stays open in Rust. It is cleared whenever the window
// sees that Statistics is not active, and by the switch after a deactivate.
// Rust's own session decides what is held, whatever this says.
import { useSyncExternalStore } from 'react';
import { isActive, type WorkspaceStateView } from '../../workspace/workspaceTypes';
import type { DatasetView } from '../describeTypes';

/** The sentence for a slot the window finds gone (ADR-012 Part 2). */
export const SLOT_GONE = 'This file is no longer held in memory. Open it again.';

/** What 8R's Output will hold; nothing can yet. */
export type OutputEntry = never;

export interface StatisticsSessionState {
  /** The open slot's number; null for no file. */
  slot: number | null;
  /** What Rust said of that slot. */
  view: DatasetView | null;
  output: readonly OutputEntry[];
  /** A picker, an open or a close is going. */
  busy: boolean;
  /** Why the file went, when it did not go by the user's own act. */
  notice: string | null;
}

const EMPTY: StatisticsSessionState = { slot: null, view: null, output: [], busy: false, notice: null };

let current: StatisticsSessionState = EMPTY;
/** How many times the session has been cleared: an open that began before a
 *  clear can tell, since its file is not this session's. */
let cleared = 0;
const listeners = new Set<() => void>();

function replace(next: Partial<StatisticsSessionState>) {
  current = { ...current, ...next };
  listeners.forEach((l) => l());
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
    replace({ slot: view.slot, view, notice: null });
  },
  /** After a close. */
  clearFile() {
    if (current.slot === null && current.view === null) return;
    replace({ slot: null, view: null });
  },
  /** Rust no longer holds the slot: the file goes, and the screen says so.
   *  8Q's window command is the first caller. */
  slotGone() {
    replace({ slot: null, view: null, notice: SLOT_GONE });
  },
  /** Statistics is not active, or the switch has left it. */
  clear() {
    cleared += 1;
    if (current.slot === null && current.view === null && current.output.length === 0 && current.notice === null) return;
    replace({ slot: null, view: null, output: [], notice: null });
  },
  setBusy(busy: boolean) {
    if (current.busy !== busy) replace({ busy });
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
