// Gaply — the project commands' wire types (Research OS, Phase 5B-3a; D282's
// command table, decisions 118 to 127 and 132). Each mirrors a Rust view in
// src-tauri/src/project_commands.rs, whose JSON is pinned byte for byte in
// src-tauri/tests/fixtures/projects/; projectTypes.vitest.ts reads those
// files and checks these types against them.
//
// A project's name arrives only in ProjectListEntryView and ProjectOpenView.
// Everything else is an id, a hash, a closed word or a number.
import type { RunState } from '../describeTypes';

/** Every project.* code, in Rust's order (ProjectCode::ALL; pinned to
 *  projectCodes.json by projectTypes.vitest.ts). */
export const PROJECT_CODES = [
  'project.bad_project_id',
  'project.bad_hash',
  'project.bad_title',
  'project.bad_agent_version',
  'project.bad_format',
  'project.root_unavailable',
  'project.not_found',
  'project.symlink_refused',
  'project.db_missing',
  'project.db_unreadable',
  'project.db_newer_schema',
  'project.busy',
  'project.still_open',
  'project.run_not_finished',
  'project.unknown_artifact',
  'project.hash_mismatch',
  'project.artifact_missing',
  'project.artifact_not_regular',
  'project.artifact_wrong_size',
  'project.artifact_wrong_hash',
  'project.prov_cycle',
  'project.prov_unlinked_derivation',
  'project.project_cap',
  'project.all_projects_cap',
  'project.disk_full',
  'project.io_failed',
  'project.db_failed',
  'project.run_interrupted',
  'project.not_open',
  'project.run_unknown',
  'project.run_evicted',
  'project.run_cancelled',
  'project.run_not_saveable',
  'project.run_failed_without_code',
  'project.run_already_saved',
  'project.data_unavailable',
  'project.data_mismatch',
  'project.confirmation_missing',
  'project.confirmation_unknown',
  'project.confirmation_used',
  'project.confirmation_replaced',
  'project.confirmation_expired',
  'project.confirmation_other_project',
  'project.stored_run_unknown',
  'project.result_too_large',
  'project.open_elsewhere',
  'project.being_deleted',
  'project.lock_failed',
] as const;

export type ProjectCode = (typeof PROJECT_CODES)[number];

/** A run's state: the contracts' RunState, typed once in describeTypes. */
export type StoredRunState = RunState;

/** project_list: one project. One that cannot be read has no name, and its
 *  code in `refused`; it can still be deleted. */
export interface ProjectListEntryView {
  projectId: string;
  name: string | null;
  bytes: number;
  /** Seconds since the epoch. */
  savedAt: number | null;
  runCount: number | null;
  refused: ProjectCode | null;
}

/** project_sizes: what all projects hold, and both caps (decision 107). */
export interface ProjectSizesView {
  totalBytes: number;
  projectCount: number;
  projectCapBytes: number;
  allProjectsCapBytes: number;
}

/** project_prepare_delete: a confirmation and the size; never the name. */
export interface DeleteConfirmationView {
  projectId: string;
  confirmation: string;
  bytes: number;
  expiresInMs: number;
}

/** project_delete. `closed`: it was open in this copy, and was closed first. */
export interface ProjectDeletedView {
  projectId: string;
  closed: boolean;
}

/** project_save_run. `created`: this Save made the project. */
export interface SavedRunView {
  projectId: string;
  storedRunId: number;
  created: boolean;
}

/** project_open (5B-3b). */
export interface ProjectOpenView {
  projectId: string;
  name: string;
  createdAt: number;
  runCount: number;
  bytes: number;
}

/** project_close. */
export interface ProjectClosedView {
  closed: boolean;
}

/** project_current (5B-3b, decision 148): the id of the project this copy of
 *  the app has open, or null. Never its name. */
export interface ProjectCurrentView {
  projectId: string | null;
}

/** project_runs (5B-3b): one saved run, as its row holds it. */
export interface StoredRunView {
  storedRunId: number;
  capabilityId: string;
  capabilityVersion: number;
  state: StoredRunState;
  endCode: ProjectCode | null;
  startedAt: number;
  endedAt: number | null;
  savedAt: number;
  agentVersion: string;
}

export interface StoredArtifactView {
  sha256: string;
  kind: 'data_file' | 'operation_spec' | 'result_bundle' | 'run_manifest' | 'run_failure';
  format: string;
  bytes: number;
}

/** project_run_result (5B-3b): the stored JSON, as text, for JSON.parse. */
export interface RunResultView {
  storedRunId: number;
  capabilityId: string;
  state: StoredRunState;
  dataFile: StoredArtifactView;
  specJson: string;
  bundleJson: string | null;
  manifestJson: string | null;
  failureJson: string | null;
}

/** project_report and project_provenance (5B-3b): bytes and a fixed name. */
export interface ProjectExportView {
  fileName: 'gaply_project_report.md' | 'gaply_project_provenance.json';
  bytes: number[];
  sha256: string;
}

/** A refusal: the code, and for a cap the bytes needed and the limit. */
export interface ProjectRefusalView {
  code: ProjectCode;
  needed?: number;
  limit?: number;
}

/** Where a Save goes: a new project named by the user, or the open one. */
export type SaveTarget = { into: 'newProject'; name: string } | { into: 'openProject'; projectId: string };
