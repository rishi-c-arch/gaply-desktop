// Gaply — the confirmatory factor analysis's bridge (Research OS, Phase 9C-3, decision 782), on
// factorBridge.ts's pattern, with no levels command. Calls the thin Rust commands in
// src-tauri/src/stats_cfa_commands.rs, and opens and closes the dataset with Phase 2B's
// stats_desc_open and stats_desc_close, unchanged. Rust owns every decision: the webview
// passes a path once (to open), slots, run ids and the user's choices, and shows what comes
// back. Export returns bytes; the save dialog writes the one file the user names, and Rust
// writes nothing.
import { isTauri } from '../../utils/isTauri';
import { invoke, openWithProgress, recorder, type MockResponse } from './statsInvoke';
import type { DatasetView, ExportFormat, ExportView, RunStartedView } from './describeTypes';
import type { CfaResultView, CfaRunStateView, CfaSpec, CfaValidationView } from './cfaTypes';

export interface CfaBridge {
  /** Reads the file through Level 1's ingest (stats_desc_open). The path is not kept.
   *  `onRowsRead` gets the open's rows read while it runs; a mock ignores it. */
  open(path: string, onRowsRead?: (rowsRead: number) => void): Promise<DatasetView>;
  validate(slot: number, spec: CfaSpec): Promise<CfaValidationView>;
  /** Starts a detached run and returns its id at once. */
  run(slot: number, spec: CfaSpec): Promise<RunStartedView>;
  status(runId: number): Promise<CfaRunStateView>;
  result(runId: number): Promise<CfaResultView>;
  cancel(runId: number): Promise<CfaRunStateView>;
  export(runId: number, format: ExportFormat): Promise<ExportView>;
  close(slot: number): Promise<boolean>;
}

export class TauriCfaBridge implements CfaBridge {
  open(path: string, onRowsRead?: (rowsRead: number) => void) {
    return openWithProgress<DatasetView>(path, onRowsRead);
  }
  validate(slot: number, spec: CfaSpec) {
    return invoke<CfaValidationView>('stats_cfa_validate', { slot, spec });
  }
  run(slot: number, spec: CfaSpec) {
    return invoke<RunStartedView>('stats_cfa_run', { slot, spec });
  }
  status(runId: number) {
    return invoke<CfaRunStateView>('stats_cfa_status', { runId });
  }
  result(runId: number) {
    return invoke<CfaResultView>('stats_cfa_result', { runId });
  }
  cancel(runId: number) {
    return invoke<CfaRunStateView>('stats_cfa_cancel', { runId });
  }
  export(runId: number, format: ExportFormat) {
    return invoke<ExportView>('stats_cfa_export', { runId, format });
  }
  close(slot: number) {
    return invoke<boolean>('stats_desc_close', { slot });
  }
}

/** Test/dev double: records every call and its arguments, and returns what
 *  `responses` holds for the command. A path is recorded as `<path>`. */
export function makeMockCfaBridge(
  responses: Partial<Record<string, MockResponse>> = {}
): CfaBridge & { calls: Array<[string, Record<string, unknown>]> } {
  const { calls, call } = recorder(responses);
  return {
    calls,
    open: (path) => call('stats_desc_open', { path }, { path: '<path>' }),
    validate: (slot, spec) => call('stats_cfa_validate', { slot, spec }),
    run: (slot, spec) => call('stats_cfa_run', { slot, spec }),
    status: (runId) => call('stats_cfa_status', { runId }),
    result: (runId) => call('stats_cfa_result', { runId }),
    cancel: (runId) => call('stats_cfa_cancel', { runId }),
    export: (runId, format) => call('stats_cfa_export', { runId, format }),
    close: (slot) => call('stats_desc_close', { slot }),
  };
}

/** The bridge the window uses: Tauri's in the desktop app; otherwise a mock that answers
 *  nothing, so a browser build shows the refusal, never a number. */
export function cfaBridge(): CfaBridge {
  return isTauri ? new TauriCfaBridge() : makeMockCfaBridge();
}
