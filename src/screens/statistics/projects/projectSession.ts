// Gaply — which project is open, as the screens know it (Research OS, Phase
// 5B-3a; decisions 122, 135 and 142). Rust holds the open project; no
// command reports it, so until Open comes in 5B-3b the screens know it only
// from a Save's response, held here in memory.
//
// Memory only: never localStorage, sessionStorage or any persisted store, and
// never logged. It is cleared whenever the screens see that Statistics is
// not active (leaving Statistics closes the project in Rust, decision 135),
// and when the open project is deleted. A reload forgets it; Rust's own
// session decides what is open whatever this says.
import { useSyncExternalStore } from 'react';
import { isActive, type WorkspaceStateView } from '../../workspace/workspaceTypes';

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

/** What the workspace state says: anything but Statistics active means no
 *  project is open (decision 135). */
export function noteWorkspaceState(view: WorkspaceStateView): void {
  if (!isActive(view.state, 'statistics')) openProjectStore.clear();
}

/** After a delete: if it was the open project, none is open now. */
export function noteDeleted(projectId: string): void {
  if (current?.projectId === projectId) openProjectStore.clear();
}

export function useOpenProject(): OpenProject | null {
  return useSyncExternalStore(openProjectStore.subscribe, openProjectStore.get, openProjectStore.get);
}
