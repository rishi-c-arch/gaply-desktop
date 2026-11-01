// Gaply — the one-way ANOVA's bridge (Research OS, Phase 8G-2, decision 318).
// Calls the thin Rust commands in src-tauri/src/stats_anova_commands.rs, and
// opens and closes the dataset with Phase 2B's stats_desc_open and
// stats_desc_close, unchanged, as the t-test's bridge does. The levels come
// from the method's own command, stats_anova_group_levels (decisions 312 and
// 329). Rust owns every decision: the webview passes a path once (to open),
// slots, run ids and the user's choices, and shows what comes back. Export
// returns bytes; the save dialog writes the one file the user names, and Rust
// writes nothing.
import { isTauri } from '../../utils/isTauri';
import { invoke, openWithProgress, recorder, type MockResponse } from './statsInvoke';
import type { DatasetView, ExportFormat, ExportView, RunStartedView } from './describeTypes';
import type { GroupLevelsView } from './ttestTypes';
import type { AnovaResultView, AnovaRunStateView, AnovaSpec, AnovaValidationView } from './anovaTypes';

export interface AnovaBridge {
  /** Reads the file through Level 1's ingest (stats_desc_open). The path is
   *  not kept. `onRowsRead` gets the open's rows read while it runs; a mock
   *  ignores it. */
  open(path: string, onRowsRead?: (rowsRead: number) => void): Promise<DatasetView>;
  /** A column's levels as the ANOVA indexes them. */
  groupLevels(slot: number, column: string): Promise<GroupLevelsView>;
  validate(slot: number, spec: AnovaSpec): Promise<AnovaValidationView>;
  /** Starts a detached run and returns its id at once. */
  run(slot: number, spec: AnovaSpec): Promise<RunStartedView>;
  status(runId: number): Promise<AnovaRunStateView>;
  result(runId: number): Promise<AnovaResultView>;
  cancel(runId: number): Promise<AnovaRunStateView>;
  export(runId: number, format: ExportFormat): Promise<ExportView>;
  close(slot: number): Promise<boolean>;
}

export class TauriAnovaBridge implements AnovaBridge {
  open(path: string, onRowsRead?: (rowsRead: number) => void) {
    return openWithProgress<DatasetView>(path, onRowsRead);
  }
  groupLevels(slot: number, column: string) {
    return invoke<GroupLevelsView>('stats_anova_group_levels', { slot, column });
  }
  validate(slot: number, spec: AnovaSpec) {
    return invoke<AnovaValidationView>('stats_anova_validate', { slot, spec });
  }
  run(slot: number, spec: AnovaSpec) {
    return invoke<RunStartedView>('stats_anova_run', { slot, spec });
  }
  status(runId: number) {
    return invoke<AnovaRunStateView>('stats_anova_status', { runId });
  }
  result(runId: number) {
    return invoke<AnovaResultView>('stats_anova_result', { runId });
  }
  cancel(runId: number) {
    return invoke<AnovaRunStateView>('stats_anova_cancel', { runId });
  }
  export(runId: number, format: ExportFormat) {
    return invoke<ExportView>('stats_anova_export', { runId, format });
  }
  close(slot: number) {
    return invoke<boolean>('stats_desc_close', { slot });
  }
}

/** Test/dev double: records every call and its arguments, and returns what
 *  `responses` holds for the command. A path is recorded as `<path>`. */
export function makeMockAnovaBridge(
  responses: Partial<Record<string, MockResponse>> = {}
): AnovaBridge & { calls: Array<[string, Record<string, unknown>]> } {
  const { calls, call } = recorder(responses);
  return {
    calls,
    open: (path) => call('stats_desc_open', { path }, { path: '<path>' }),
    groupLevels: (slot, column) => call('stats_anova_group_levels', { slot, column }),
    validate: (slot, spec) => call('stats_anova_validate', { slot, spec }),
    run: (slot, spec) => call('stats_anova_run', { slot, spec }),
    status: (runId) => call('stats_anova_status', { runId }),
    result: (runId) => call('stats_anova_result', { runId }),
    cancel: (runId) => call('stats_anova_cancel', { runId }),
    export: (runId, format) => call('stats_anova_export', { runId, format }),
    close: (slot) => call('stats_desc_close', { slot }),
  };
}

/** The bridge the screen uses: Tauri's in the desktop app; otherwise a mock
 *  that answers nothing, so a browser build shows the refusal, never a
 *  number. */
export function anovaBridge(): AnovaBridge {
  return isTauri ? new TauriAnovaBridge() : makeMockAnovaBridge();
}
