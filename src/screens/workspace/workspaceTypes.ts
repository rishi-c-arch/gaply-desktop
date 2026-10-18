// Gaply — the workspace wire (Research OS, Phase 3): the views the
// workspace_* commands in src-tauri/src/workspace.rs return, and their
// refusals. Rust owns the state; the screens read it. Each view's exact JSON
// is pinned on both sides (fixtures/, workspaceTypes.vitest.ts and
// src-tauri/tests/workspace_wire.rs).
import type { EntitlementStatus } from '../subscription/entitlement';

export type WorkspaceId = 'statistics' | 'compute' | 'design';

/** Fixed per build: Statistics is available; Compute and Design are missing
 *  ("Coming later"). */
export type Availability = 'available' | 'missing' | 'incompatible';

export type DisplayNameCode = 'workspace.name.statistics' | 'workspace.name.compute' | 'workspace.name.design';

export type Os = 'macos' | 'windows' | 'linux' | 'other';

/** workspace_list's item. `entitled` is always null from Rust (it does not
 *  check entitlement in Phase 3); the chooser reads entitlement from
 *  useEntitlement and shows it as its own fact. */
export interface WorkspaceDescriptorView {
  id: WorkspaceId;
  version: number;
  displayNameCode: DisplayNameCode;
  availability: Availability;
  activationPolicy: 'single_active';
  capabilityIds: string[];
  supportsOffline: boolean;
  supportedPlatforms: Os[];
  entitled: null;
}

/** A descriptor with the entitlement the frontend read for it. */
export interface WorkspaceCard extends Omit<WorkspaceDescriptorView, 'entitled'> {
  entitled: EntitlementStatus;
}

export type WorkspacePhase = 'no_workspace' | 'activating' | 'active' | 'deactivating';

export type WorkspaceState =
  | { phase: 'no_workspace' }
  | { phase: 'activating' | 'active' | 'deactivating'; workspace: WorkspaceId };

/** Which kind of Statistics run a record holds (Research OS, Phase 4B-2,
 *  decision 80): the shell reads it through that kind's commands. */
export type RunKind = 'describe' | 'ttest';

/** A Statistics run, as the shell names it. */
export interface RunEntry {
  runId: number;
  kind: RunKind;
}

/** workspace_state, workspace_activate and workspace_deactivate return this. */
export interface WorkspaceStateView {
  state: WorkspaceState;
  /** The Statistics run that is queued or running, if any. */
  activeRun: RunEntry | null;
  /** The newest Statistics run held, finished or not. */
  latestRun: RunEntry | null;
}

export type WorkspaceCode =
  | 'workspace.unavailable'
  | 'workspace.not_ready'
  | 'workspace.inactive'
  | 'workspace.wrong_workspace'
  | 'workspace.run_active'
  | 'workspace.illegal_transition';

export type WorkspaceEventKind = 'activate' | 'activated' | 'activation_refused' | 'deactivate' | 'deactivated';

/** A workspace refusal: a closed code with closed-word or numeric parameters. */
export interface WorkspaceRefusal {
  code: WorkspaceCode;
  params: {
    workspace?: WorkspaceId;
    expected?: WorkspaceId;
    found?: WorkspaceId;
    from?: WorkspacePhase;
    event?: WorkspaceEventKind;
    run?: number;
  };
  severity: 'error';
}

/** The refusal inside a command's error, or null when it is not one. */
export function workspaceRefusal(err: unknown): WorkspaceRefusal | null {
  const message =
    typeof err === 'string'
      ? err
      : err && typeof err === 'object' && typeof (err as { message?: unknown }).message === 'string'
        ? (err as { message: string }).message
        : null;
  if (message === null) return null;
  try {
    const parsed = JSON.parse(message) as Partial<WorkspaceRefusal>;
    return typeof parsed.code === 'string' && parsed.code.startsWith('workspace.') ? (parsed as WorkspaceRefusal) : null;
  } catch {
    return null;
  }
}

export const isActive = (s: WorkspaceState, id: WorkspaceId): boolean => s.phase === 'active' && s.workspace === id;
