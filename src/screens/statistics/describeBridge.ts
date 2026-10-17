// Gaply — the descriptive summary's bridge (Research OS, Phase 2B). Calls the
// thin Rust commands in src-tauri/src/stats_desc_commands.rs. Rust owns every
// decision: the webview passes a path (once, to open), slots, run ids and the
// user's choices, and shows what comes back. Export returns bytes; the save
// dialog writes the one file the user names, and Rust writes nothing.
import { isTauri } from '../../utils/isTauri';
import type {
  DatasetView,
  ExportFormat,
  ExportView,
  OperationSpec,
  ResultBundleView,
  RunStartedView,
  RunStateView,
  ValidationReportView,
} from './describeTypes';

export interface DescribeBridge {
  /** Reads the file through Level 1's ingest. The path is not kept. */
  open(path: string): Promise<DatasetView>;
  validate(slot: number, spec: OperationSpec): Promise<ValidationReportView>;
  /** Starts a detached run and returns its id at once. */
  run(slot: number, spec: OperationSpec): Promise<RunStartedView>;
  status(runId: number): Promise<RunStateView>;
  result(runId: number): Promise<ResultBundleView>;
  cancel(runId: number): Promise<RunStateView>;
  export(runId: number, format: ExportFormat): Promise<ExportView>;
  close(slot: number): Promise<boolean>;
}

export class TauriDescribeBridge implements DescribeBridge {
  private async invoke<T>(cmd: string, args: Record<string, unknown>): Promise<T> {
    if (!isTauri) throw new Error('The descriptive summary runs in the Gaply desktop app.');
    const { invoke } = await import('@tauri-apps/api/core');
    return invoke<T>(cmd, args);
  }

  open(path: string) {
    return this.invoke<DatasetView>('stats_desc_open', { path });
  }
  validate(slot: number, spec: OperationSpec) {
    return this.invoke<ValidationReportView>('stats_desc_validate', { slot, spec });
  }
  run(slot: number, spec: OperationSpec) {
    return this.invoke<RunStartedView>('stats_desc_run', { slot, spec });
  }
  status(runId: number) {
    return this.invoke<RunStateView>('stats_desc_status', { runId });
  }
  result(runId: number) {
    return this.invoke<ResultBundleView>('stats_desc_result', { runId });
  }
  cancel(runId: number) {
    return this.invoke<RunStateView>('stats_desc_cancel', { runId });
  }
  export(runId: number, format: ExportFormat) {
    return this.invoke<ExportView>('stats_desc_export', { runId, format });
  }
  close(slot: number) {
    return this.invoke<boolean>('stats_desc_close', { slot });
  }
}

/** A canned response, or a function of the call's arguments (a sequence of
 *  status polls, a refusal). */
type MockResponse = unknown | ((args: Record<string, unknown>) => unknown);

/** Test/dev double: records every call and its arguments, and returns what
 *  `responses` holds for the command. A path is recorded as `<path>`. */
export function makeMockDescribeBridge(
  responses: Partial<Record<string, MockResponse>> = {}
): DescribeBridge & { calls: Array<[string, Record<string, unknown>]> } {
  const calls: Array<[string, Record<string, unknown>]> = [];
  const call = <T>(cmd: string, args: Record<string, unknown>, recorded = args): Promise<T> => {
    calls.push([cmd, recorded]);
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
    open: (path) => call('stats_desc_open', { path }, { path: '<path>' }),
    validate: (slot, spec) => call('stats_desc_validate', { slot, spec }),
    run: (slot, spec) => call('stats_desc_run', { slot, spec }),
    status: (runId) => call('stats_desc_status', { runId }),
    result: (runId) => call('stats_desc_result', { runId }),
    cancel: (runId) => call('stats_desc_cancel', { runId }),
    export: (runId, format) => call('stats_desc_export', { runId, format }),
    close: (slot) => call('stats_desc_close', { slot }),
  };
}

/** The bridge the screen uses: Tauri's in the desktop app; otherwise a mock
 *  that answers nothing, so a browser build shows the refusal, never a
 *  number. */
export function describeBridge(): DescribeBridge {
  return isTauri ? new TauriDescribeBridge() : makeMockDescribeBridge();
}
