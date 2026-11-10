// Gaply — the rank tests' bridge (Research OS, Phase 8Y-2, decision 619), on
// anovaBridge.ts's pattern. Calls the thin Rust commands in
// src-tauri/src/stats_ranks_commands.rs, and opens and closes the dataset with Phase
// 2B's stats_desc_open and stats_desc_close, unchanged. Rust owns every decision: the
// webview passes a path once (to open), slots, run ids and the user's choices, and
// shows what comes back. Export returns bytes; the save dialog writes the one file the
// user names, and Rust writes nothing.
import { isTauri } from '../../utils/isTauri';
import { invoke, openWithProgress, recorder, type MockResponse } from './statsInvoke';
import type { DatasetView, ExportFormat, ExportView, RunStartedView } from './describeTypes';
import type { RanksResultView, RanksRunStateView, RanksSpec, RanksValidationView } from './ranksTypes';
import type { GroupLevelsView } from './ttestTypes';

export interface RanksBridge {
  /** Reads the file through Level 1's ingest (stats_desc_open). The path is not
   *  kept. `onRowsRead` gets the open's rows read while it runs; a mock ignores it. */
  open(path: string, onRowsRead?: (rowsRead: number) => void): Promise<DatasetView>;
  /** A column's levels as the rank tests index them. */
  groupLevels(slot: number, column: string): Promise<GroupLevelsView>;
  validate(slot: number, spec: RanksSpec): Promise<RanksValidationView>;
  /** Starts a detached run and returns its id at once. */
  run(slot: number, spec: RanksSpec): Promise<RunStartedView>;
  status(runId: number): Promise<RanksRunStateView>;
  result(runId: number): Promise<RanksResultView>;
  cancel(runId: number): Promise<RanksRunStateView>;
  export(runId: number, format: ExportFormat): Promise<ExportView>;
  close(slot: number): Promise<boolean>;
}

export class TauriRanksBridge implements RanksBridge {
  open(path: string, onRowsRead?: (rowsRead: number) => void) {
    return openWithProgress<DatasetView>(path, onRowsRead);
  }
  groupLevels(slot: number, column: string) {
    return invoke<GroupLevelsView>('stats_ranks_group_levels', { slot, column });
  }
  validate(slot: number, spec: RanksSpec) {
    return invoke<RanksValidationView>('stats_ranks_validate', { slot, spec });
  }
  run(slot: number, spec: RanksSpec) {
    return invoke<RunStartedView>('stats_ranks_run', { slot, spec });
  }
  status(runId: number) {
    return invoke<RanksRunStateView>('stats_ranks_status', { runId });
  }
  result(runId: number) {
    return invoke<RanksResultView>('stats_ranks_result', { runId });
  }
  cancel(runId: number) {
    return invoke<RanksRunStateView>('stats_ranks_cancel', { runId });
  }
  export(runId: number, format: ExportFormat) {
    return invoke<ExportView>('stats_ranks_export', { runId, format });
  }
  close(slot: number) {
    return invoke<boolean>('stats_desc_close', { slot });
  }
}

/** Test/dev double: records every call and its arguments, and returns what
 *  `responses` holds for the command. A path is recorded as `<path>`. */
export function makeMockRanksBridge(
  responses: Partial<Record<string, MockResponse>> = {}
): RanksBridge & { calls: Array<[string, Record<string, unknown>]> } {
  const { calls, call } = recorder(responses);
  return {
    calls,
    open: (path) => call('stats_desc_open', { path }, { path: '<path>' }),
    groupLevels: (slot, column) => call('stats_ranks_group_levels', { slot, column }),
    validate: (slot, spec) => call('stats_ranks_validate', { slot, spec }),
    run: (slot, spec) => call('stats_ranks_run', { slot, spec }),
    status: (runId) => call('stats_ranks_status', { runId }),
    result: (runId) => call('stats_ranks_result', { runId }),
    cancel: (runId) => call('stats_ranks_cancel', { runId }),
    export: (runId, format) => call('stats_ranks_export', { runId, format }),
    close: (slot) => call('stats_desc_close', { slot }),
  };
}

/** The bridge the window uses: Tauri's in the desktop app; otherwise a mock that
 *  answers nothing, so a browser build shows the refusal, never a number. */
export function ranksBridge(): RanksBridge {
  return isTauri ? new TauriRanksBridge() : makeMockRanksBridge();
}
