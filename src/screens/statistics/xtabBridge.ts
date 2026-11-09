// Gaply — crosstabs' bridge (Research OS, Phase 8X-2, decision 588), on
// anovaBridge.ts's pattern. Calls the thin Rust commands in
// src-tauri/src/stats_xtab_commands.rs, and opens and closes the dataset with
// Phase 2B's stats_desc_open and stats_desc_close, unchanged. There is no levels
// command: the table takes every level of both columns (decision 575). Rust owns
// every decision: the webview passes a path once (to open), slots, run ids and the
// user's choices, and shows what comes back. Export returns bytes; the save dialog
// writes the one file the user names, and Rust writes nothing.
import { isTauri } from '../../utils/isTauri';
import { invoke, openWithProgress, recorder, type MockResponse } from './statsInvoke';
import type { DatasetView, ExportFormat, ExportView, RunStartedView } from './describeTypes';
import type { XtabResultView, XtabRunStateView, XtabSpec, XtabValidationView } from './xtabTypes';

export interface XtabBridge {
  /** Reads the file through Level 1's ingest (stats_desc_open). The path is
   *  not kept. `onRowsRead` gets the open's rows read while it runs; a mock
   *  ignores it. */
  open(path: string, onRowsRead?: (rowsRead: number) => void): Promise<DatasetView>;
  validate(slot: number, spec: XtabSpec): Promise<XtabValidationView>;
  /** Starts a detached run and returns its id at once. */
  run(slot: number, spec: XtabSpec): Promise<RunStartedView>;
  status(runId: number): Promise<XtabRunStateView>;
  result(runId: number): Promise<XtabResultView>;
  cancel(runId: number): Promise<XtabRunStateView>;
  export(runId: number, format: ExportFormat): Promise<ExportView>;
  close(slot: number): Promise<boolean>;
}

export class TauriXtabBridge implements XtabBridge {
  open(path: string, onRowsRead?: (rowsRead: number) => void) {
    return openWithProgress<DatasetView>(path, onRowsRead);
  }
  validate(slot: number, spec: XtabSpec) {
    return invoke<XtabValidationView>('stats_xtab_validate', { slot, spec });
  }
  run(slot: number, spec: XtabSpec) {
    return invoke<RunStartedView>('stats_xtab_run', { slot, spec });
  }
  status(runId: number) {
    return invoke<XtabRunStateView>('stats_xtab_status', { runId });
  }
  result(runId: number) {
    return invoke<XtabResultView>('stats_xtab_result', { runId });
  }
  cancel(runId: number) {
    return invoke<XtabRunStateView>('stats_xtab_cancel', { runId });
  }
  export(runId: number, format: ExportFormat) {
    return invoke<ExportView>('stats_xtab_export', { runId, format });
  }
  close(slot: number) {
    return invoke<boolean>('stats_desc_close', { slot });
  }
}

/** Test/dev double: records every call and its arguments, and returns what
 *  `responses` holds for the command. A path is recorded as `<path>`. */
export function makeMockXtabBridge(
  responses: Partial<Record<string, MockResponse>> = {}
): XtabBridge & { calls: Array<[string, Record<string, unknown>]> } {
  const { calls, call } = recorder(responses);
  return {
    calls,
    open: (path) => call('stats_desc_open', { path }, { path: '<path>' }),
    validate: (slot, spec) => call('stats_xtab_validate', { slot, spec }),
    run: (slot, spec) => call('stats_xtab_run', { slot, spec }),
    status: (runId) => call('stats_xtab_status', { runId }),
    result: (runId) => call('stats_xtab_result', { runId }),
    cancel: (runId) => call('stats_xtab_cancel', { runId }),
    export: (runId, format) => call('stats_xtab_export', { runId, format }),
    close: (slot) => call('stats_desc_close', { slot }),
  };
}

/** The bridge the window uses: Tauri's in the desktop app; otherwise a mock
 *  that answers nothing, so a browser build shows the refusal, never a
 *  number. */
export function xtabBridge(): XtabBridge {
  return isTauri ? new TauriXtabBridge() : makeMockXtabBridge();
}
