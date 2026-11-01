// Gaply — the correlations' bridge (Research OS, Phase 8H-2, decision 356).
// Calls the thin Rust commands in src-tauri/src/stats_corr_commands.rs, and
// opens and closes the dataset with Phase 2B's stats_desc_open and
// stats_desc_close, unchanged, as the ANOVA's bridge does. Rust owns every
// decision: the webview passes a path once (to open), slots, run ids and the
// user's choices, and shows what comes back. Export returns bytes; the save
// dialog writes the one file the user names, and Rust writes nothing.
import { isTauri } from '../../utils/isTauri';
import { invoke, openWithProgress, recorder, type MockResponse } from './statsInvoke';
import type { DatasetView, ExportFormat, ExportView, RunStartedView } from './describeTypes';
import type { CorrResultView, CorrRunStateView, CorrSpec, CorrValidationView } from './corrTypes';

export interface CorrBridge {
  /** Reads the file through Level 1's ingest (stats_desc_open). The path is
   *  not kept. `onRowsRead` gets the open's rows read while it runs; a mock
   *  ignores it. */
  open(path: string, onRowsRead?: (rowsRead: number) => void): Promise<DatasetView>;
  validate(slot: number, spec: CorrSpec): Promise<CorrValidationView>;
  /** Starts a detached run and returns its id at once. */
  run(slot: number, spec: CorrSpec): Promise<RunStartedView>;
  status(runId: number): Promise<CorrRunStateView>;
  result(runId: number): Promise<CorrResultView>;
  cancel(runId: number): Promise<CorrRunStateView>;
  export(runId: number, format: ExportFormat): Promise<ExportView>;
  close(slot: number): Promise<boolean>;
}

export class TauriCorrBridge implements CorrBridge {
  open(path: string, onRowsRead?: (rowsRead: number) => void) {
    return openWithProgress<DatasetView>(path, onRowsRead);
  }
  validate(slot: number, spec: CorrSpec) {
    return invoke<CorrValidationView>('stats_corr_validate', { slot, spec });
  }
  run(slot: number, spec: CorrSpec) {
    return invoke<RunStartedView>('stats_corr_run', { slot, spec });
  }
  status(runId: number) {
    return invoke<CorrRunStateView>('stats_corr_status', { runId });
  }
  result(runId: number) {
    return invoke<CorrResultView>('stats_corr_result', { runId });
  }
  cancel(runId: number) {
    return invoke<CorrRunStateView>('stats_corr_cancel', { runId });
  }
  export(runId: number, format: ExportFormat) {
    return invoke<ExportView>('stats_corr_export', { runId, format });
  }
  close(slot: number) {
    return invoke<boolean>('stats_desc_close', { slot });
  }
}

/** Test/dev double: records every call and its arguments, and returns what
 *  `responses` holds for the command. A path is recorded as `<path>`. */
export function makeMockCorrBridge(
  responses: Partial<Record<string, MockResponse>> = {}
): CorrBridge & { calls: Array<[string, Record<string, unknown>]> } {
  const { calls, call } = recorder(responses);
  return {
    calls,
    open: (path) => call('stats_desc_open', { path }, { path: '<path>' }),
    validate: (slot, spec) => call('stats_corr_validate', { slot, spec }),
    run: (slot, spec) => call('stats_corr_run', { slot, spec }),
    status: (runId) => call('stats_corr_status', { runId }),
    result: (runId) => call('stats_corr_result', { runId }),
    cancel: (runId) => call('stats_corr_cancel', { runId }),
    export: (runId, format) => call('stats_corr_export', { runId, format }),
    close: (slot) => call('stats_desc_close', { slot }),
  };
}

/** The bridge the screen uses: Tauri's in the desktop app; otherwise a mock
 *  that answers nothing, so a browser build shows the refusal, never a
 *  number. */
export function corrBridge(): CorrBridge {
  return isTauri ? new TauriCorrBridge() : makeMockCorrBridge();
}
