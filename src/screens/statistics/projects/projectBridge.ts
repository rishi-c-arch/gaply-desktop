// Gaply — the project bridge (Research OS, Phases 5B-3a and 5B-3b): the one
// wrapper that calls the twelve project_* commands in
// src-tauri/src/project_commands.rs (D282's table, and project_current,
// decision 148). Rust owns every decision and every check; the screens show
// what comes back. No other file may name a project command
// (projectGuards.vitest.ts).
//
// The webview passes ids, a run's number, a confirmation and, for a new
// project, the name the user typed. Never a path or a file name.
import { isTauri } from '../../../utils/isTauri';
import type {
  DeleteConfirmationView,
  ProjectClosedView,
  ProjectCurrentView,
  ProjectDeletedView,
  ProjectExportView,
  ProjectListEntryView,
  ProjectOpenView,
  ProjectSizesView,
  RunResultView,
  SavedRunView,
  SaveTarget,
  StoredRunView,
} from './projectTypes';

export interface ProjectBridge {
  /** Exempt from the workspace check (decision 120). */
  list(): Promise<ProjectListEntryView[]>;
  sizes(): Promise<ProjectSizesView>;
  prepareDelete(projectId: string): Promise<DeleteConfirmationView>;
  delete(projectId: string, confirmation: string): Promise<ProjectDeletedView>;
  /** Which project this copy has open, by id (decision 148). */
  current(): Promise<ProjectCurrentView>;
  /** Need Statistics active. */
  saveRun(runId: number, target: SaveTarget): Promise<SavedRunView>;
  open(projectId: string): Promise<ProjectOpenView>;
  close(): Promise<ProjectClosedView>;
  runs(projectId: string): Promise<StoredRunView[]>;
  runResult(projectId: string, storedRunId: number): Promise<RunResultView>;
  report(projectId: string): Promise<ProjectExportView>;
  provenance(projectId: string): Promise<ProjectExportView>;
}

/** The command names, by method: the guard reads this list. */
export const PROJECT_COMMANDS = {
  list: 'project_list',
  sizes: 'project_sizes',
  prepareDelete: 'project_prepare_delete',
  delete: 'project_delete',
  current: 'project_current',
  saveRun: 'project_save_run',
  open: 'project_open',
  close: 'project_close',
  runs: 'project_runs',
  runResult: 'project_run_result',
  report: 'project_report',
  provenance: 'project_provenance',
} as const;

const C = PROJECT_COMMANDS;

export class TauriProjectBridge implements ProjectBridge {
  private async invoke<T>(cmd: string, args: Record<string, unknown> = {}): Promise<T> {
    if (!isTauri) throw new Error('Projects are kept by the Gaply desktop app.');
    const { invoke } = await import('@tauri-apps/api/core');
    return invoke<T>(cmd, args);
  }

  list() {
    return this.invoke<ProjectListEntryView[]>(C.list);
  }
  sizes() {
    return this.invoke<ProjectSizesView>(C.sizes);
  }
  prepareDelete(projectId: string) {
    return this.invoke<DeleteConfirmationView>(C.prepareDelete, { projectId });
  }
  delete(projectId: string, confirmation: string) {
    return this.invoke<ProjectDeletedView>(C.delete, { projectId, confirmation });
  }
  current() {
    return this.invoke<ProjectCurrentView>(C.current);
  }
  saveRun(runId: number, target: SaveTarget) {
    return this.invoke<SavedRunView>(C.saveRun, { runId, target });
  }
  open(projectId: string) {
    return this.invoke<ProjectOpenView>(C.open, { projectId });
  }
  close() {
    return this.invoke<ProjectClosedView>(C.close);
  }
  runs(projectId: string) {
    return this.invoke<StoredRunView[]>(C.runs, { projectId });
  }
  runResult(projectId: string, storedRunId: number) {
    return this.invoke<RunResultView>(C.runResult, { projectId, storedRunId });
  }
  report(projectId: string) {
    return this.invoke<ProjectExportView>(C.report, { projectId });
  }
  provenance(projectId: string) {
    return this.invoke<ProjectExportView>(C.provenance, { projectId });
  }
}

type MockResponse = unknown | ((args: Record<string, unknown>) => unknown);

/** Test double: records every call and its arguments, and answers with what
 *  `responses` holds for the command (a value, a function of the arguments
 *  that may throw a refusal, or a promise the test settles). */
export function makeMockProjectBridge(
  responses: Partial<Record<string, MockResponse>> = {}
): ProjectBridge & { calls: Array<[string, Record<string, unknown>]> } {
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
    list: () => call(C.list),
    sizes: () => call(C.sizes),
    prepareDelete: (projectId) => call(C.prepareDelete, { projectId }),
    delete: (projectId, confirmation) => call(C.delete, { projectId, confirmation }),
    current: () => call(C.current),
    saveRun: (runId, target) => call(C.saveRun, { runId, target }),
    open: (projectId) => call(C.open, { projectId }),
    close: () => call(C.close),
    runs: (projectId) => call(C.runs, { projectId }),
    runResult: (projectId, storedRunId) => call(C.runResult, { projectId, storedRunId }),
    report: (projectId) => call(C.report, { projectId }),
    provenance: (projectId) => call(C.provenance, { projectId }),
  };
}

/** The bridge the screens use: Tauri's in the desktop app; otherwise a mock
 *  that answers nothing. The screens never reach this in the browser build:
 *  projectsShown() is false there. */
export function projectBridge(): ProjectBridge {
  return isTauri ? new TauriProjectBridge() : makeMockProjectBridge();
}
