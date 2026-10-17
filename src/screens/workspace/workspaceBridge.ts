// Gaply — the workspace bridge (Research OS, Phase 3). Calls the workspace_*
// commands in src-tauri/src/workspace.rs. Rust holds the state and makes
// every decision; the screens read what comes back. Polling only: there is
// no event channel (decision 24).
import { isTauri } from '../../utils/isTauri';
import type { WorkspaceDescriptorView, WorkspaceId, WorkspaceStateView } from './workspaceTypes';

export interface WorkspaceBridge {
  list(): Promise<WorkspaceDescriptorView[]>;
  state(): Promise<WorkspaceStateView>;
  /** The only way a workspace becomes active. */
  activate(id: WorkspaceId): Promise<WorkspaceStateView>;
  deactivate(): Promise<WorkspaceStateView>;
}

export class TauriWorkspaceBridge implements WorkspaceBridge {
  private async invoke<T>(cmd: string, args: Record<string, unknown> = {}): Promise<T> {
    if (!isTauri) throw new Error('Workspaces run in the Gaply desktop app.');
    const { invoke } = await import('@tauri-apps/api/core');
    return invoke<T>(cmd, args);
  }

  list() {
    return this.invoke<WorkspaceDescriptorView[]>('workspace_list');
  }
  state() {
    return this.invoke<WorkspaceStateView>('workspace_state');
  }
  activate(id: WorkspaceId) {
    return this.invoke<WorkspaceStateView>('workspace_activate', { id });
  }
  deactivate() {
    return this.invoke<WorkspaceStateView>('workspace_deactivate');
  }
}

/** The bridge the screens use: Tauri's in the desktop app; otherwise a mock
 *  that answers nothing, so a browser build shows the refusal, never a
 *  workspace. */
export function workspaceBridge(): WorkspaceBridge {
  return isTauri ? new TauriWorkspaceBridge() : makeMockWorkspaceBridge();
}

type MockResponse = unknown | ((args: Record<string, unknown>) => unknown);

/** Test/dev double: records every call, and answers with what `responses`
 *  holds for the command (a value, or a function of the arguments that may
 *  throw a refusal). */
export function makeMockWorkspaceBridge(
  responses: Partial<Record<string, MockResponse>> = {}
): WorkspaceBridge & { calls: Array<[string, Record<string, unknown>]> } {
  const calls: Array<[string, Record<string, unknown>]> = [];
  const call = <T>(cmd: string, args: Record<string, unknown> = {}): Promise<T> => {
    calls.push([cmd, args]);
    if (!(cmd in responses)) return Promise.reject(new Error(`no mock response for ${cmd}`));
    const r = responses[cmd];
    try {
      return Promise.resolve((typeof r === 'function' ? (r as (a: Record<string, unknown>) => unknown)(args) : r) as T);
    } catch (e) {
      return Promise.reject(e);
    }
  };
  return {
    calls,
    list: () => call('workspace_list'),
    state: () => call('workspace_state'),
    activate: (id) => call('workspace_activate', { id }),
    deactivate: () => call('workspace_deactivate'),
  };
}
