// Gaply — the independent-samples t-test's bridge (Research OS, Phase 4B-2).
// Calls the thin Rust commands in src-tauri/src/stats_ttest_commands.rs, and
// opens and closes the dataset with Phase 2B's stats_desc_open and
// stats_desc_close, unchanged (decision 74). Rust owns every decision: the
// webview passes a path once (to open), slots, run ids and the user's
// choices, and shows what comes back. Export returns bytes; the save dialog
// writes the one file the user names, and Rust writes nothing.
import { isTauri } from '../../utils/isTauri';
import { invoke, openWithProgress, recorder, type MockResponse } from './statsInvoke';
import type { DatasetView, ExportFormat, ExportView, RunStartedView } from './describeTypes';
import type {
  CapabilityView,
  GroupLevelsView,
  TTestResultView,
  TTestRunStateView,
  TTestSpec,
  TTestValidationReportView,
} from './ttestTypes';

export interface TTestBridge {
  /** Reads the file through Level 1's ingest (stats_desc_open). The path is
   *  not kept. `onRowsRead` gets the open's rows read while it runs
   *  (Research OS, Phase 8F-2, decision 290); a mock ignores it. */
  open(path: string, onRowsRead?: (rowsRead: number) => void): Promise<DatasetView>;
  /** A column's levels as the t-test indexes them (decision 81). */
  groupLevels(slot: number, column: string): Promise<GroupLevelsView>;
  validate(slot: number, spec: TTestSpec): Promise<TTestValidationReportView>;
  /** Starts a detached run and returns its id at once. */
  run(slot: number, spec: TTestSpec): Promise<RunStartedView>;
  status(runId: number): Promise<TTestRunStateView>;
  result(runId: number): Promise<TTestResultView>;
  cancel(runId: number): Promise<TTestRunStateView>;
  export(runId: number, format: ExportFormat): Promise<ExportView>;
  close(slot: number): Promise<boolean>;
}

/** The method selector's one call: needs no workspace (decision 76). */
export interface CapabilityBridge {
  list(): Promise<CapabilityView[]>;
}

export class TauriTTestBridge implements TTestBridge {
  open(path: string, onRowsRead?: (rowsRead: number) => void) {
    return openWithProgress<DatasetView>(path, onRowsRead);
  }
  groupLevels(slot: number, column: string) {
    return invoke<GroupLevelsView>('stats_ttest_group_levels', { slot, column });
  }
  validate(slot: number, spec: TTestSpec) {
    return invoke<TTestValidationReportView>('stats_ttest_validate', { slot, spec });
  }
  run(slot: number, spec: TTestSpec) {
    return invoke<RunStartedView>('stats_ttest_run', { slot, spec });
  }
  status(runId: number) {
    return invoke<TTestRunStateView>('stats_ttest_status', { runId });
  }
  result(runId: number) {
    return invoke<TTestResultView>('stats_ttest_result', { runId });
  }
  cancel(runId: number) {
    return invoke<TTestRunStateView>('stats_ttest_cancel', { runId });
  }
  export(runId: number, format: ExportFormat) {
    return invoke<ExportView>('stats_ttest_export', { runId, format });
  }
  close(slot: number) {
    return invoke<boolean>('stats_desc_close', { slot });
  }
}

export class TauriCapabilityBridge implements CapabilityBridge {
  list() {
    return invoke<CapabilityView[]>('capability_list');
  }
}

/** Test/dev double: records every call and its arguments, and returns what
 *  `responses` holds for the command. A path is recorded as `<path>`. */
export function makeMockTTestBridge(
  responses: Partial<Record<string, MockResponse>> = {}
): TTestBridge & { calls: Array<[string, Record<string, unknown>]> } {
  const { calls, call } = recorder(responses);
  return {
    calls,
    open: (path) => call('stats_desc_open', { path }, { path: '<path>' }),
    groupLevels: (slot, column) => call('stats_ttest_group_levels', { slot, column }),
    validate: (slot, spec) => call('stats_ttest_validate', { slot, spec }),
    run: (slot, spec) => call('stats_ttest_run', { slot, spec }),
    status: (runId) => call('stats_ttest_status', { runId }),
    result: (runId) => call('stats_ttest_result', { runId }),
    cancel: (runId) => call('stats_ttest_cancel', { runId }),
    export: (runId, format) => call('stats_ttest_export', { runId, format }),
    close: (slot) => call('stats_desc_close', { slot }),
  };
}

export function makeMockCapabilityBridge(
  responses: Partial<Record<string, MockResponse>> = {}
): CapabilityBridge & { calls: Array<[string, Record<string, unknown>]> } {
  const { calls, call } = recorder(responses);
  return { calls, list: () => call('capability_list') };
}

/** The bridges the screens use: Tauri's in the desktop app; otherwise mocks
 *  that answer nothing, so a browser build shows the refusal, never a
 *  number and never a card. */
export function ttestBridge(): TTestBridge {
  return isTauri ? new TauriTTestBridge() : makeMockTTestBridge();
}

export function capabilityBridge(): CapabilityBridge {
  return isTauri ? new TauriCapabilityBridge() : makeMockCapabilityBridge();
}
