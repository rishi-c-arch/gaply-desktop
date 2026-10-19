// Gaply — which project is open, as the screens know it (Research OS, Phases
// 5B-3a and 5B-3b; decisions 122, 135, 142 and 148). Rust holds the open
// project. Since 5B-3b every project screen asks Rust when it appears
// (project_current, with project_list for the name) and writes the answer
// here through noteCurrent, so no screen treats as open a project Rust has
// closed or deleted. Between those reads it holds what the last answer, or a
// Save, an Open, a Close or a delete, said.
//
// Memory only: never localStorage, sessionStorage, IndexedDB or any persisted
// store, and never logged. It is cleared whenever the screens see that
// Statistics is not active (leaving Statistics closes the project in Rust,
// decision 135), and when the open project is deleted. A reload forgets it;
// Rust's own session decides what is open whatever this says.
import { useSyncExternalStore } from 'react';
import { isActive, type WorkspaceStateView } from '../../workspace/workspaceTypes';
import type { ProjectListEntryView } from './projectTypes';

export interface OpenProject {
  projectId: string;
  /** The name the user typed for it, shown only as text inside <bdi>. */
  name: string;
}

let current: OpenProject | null = null;
const listeners = new Set<() => void>();

function emit() {
  listeners.forEach((l) => l());
}

export const openProjectStore = {
  get(): OpenProject | null {
    return current;
  },
  /** After a Save: the project saved into is the open one (decision 122). */
  set(p: OpenProject) {
    current = { projectId: p.projectId, name: p.name };
    emit();
  },
  clear() {
    if (current === null) return;
    current = null;
    emit();
  },
  subscribe(listener: () => void): () => void {
    listeners.add(listener);
    return () => {
      listeners.delete(listener);
    };
  },
};

/** The id in a project_current answer, or null for none, a failed read or
 *  anything that is not the answer's shape. */
export function currentId(answer: unknown): string | null {
  if (!answer || typeof answer !== 'object') return null;
  const id = (answer as { projectId?: unknown }).projectId;
  return typeof id === 'string' ? id : null;
}

/** What the workspace state says: anything but Statistics active means no
 *  project is open (decision 135). */
export function noteWorkspaceState(view: WorkspaceStateView): void {
  if (!isActive(view.state, 'statistics')) openProjectStore.clear();
}

/** What project_current answered, with the list for the name (decision
 *  148). `answer` is the command's answer as it came, or null when the read
 *  failed: an id that is not a string, or a failed read, means none is open
 *  as far as the screens can show. An open project whose name the list does
 *  not give (not listed, or unreadable) is not offered by name, so the
 *  screens treat it as none too; Rust refuses whatever they send. */
export function noteCurrent(answer: unknown, list: ProjectListEntryView[] | null): void {
  const id = currentId(answer);
  const entry = id === null || list === null ? undefined : list.find((e) => e.projectId === id && typeof e.name === 'string');
  if (id === null) {
    openProjectStore.clear();
  } else if (entry && entry.name !== null) {
    if (current?.projectId !== id || current.name !== entry.name) openProjectStore.set({ projectId: id, name: entry.name });
  } else if (current?.projectId !== id) {
    openProjectStore.clear();
  }
}

/** After a delete: if it was the open project, none is open now. */
export function noteDeleted(projectId: string): void {
  if (current?.projectId === projectId) openProjectStore.clear();
}

export function useOpenProject(): OpenProject | null {
  return useSyncExternalStore(openProjectStore.subscribe, openProjectStore.get, openProjectStore.get);
}
