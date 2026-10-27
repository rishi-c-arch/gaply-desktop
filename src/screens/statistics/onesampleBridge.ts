// Gaply — the one-sample and paired-samples t-tests' bridge (Research OS,
// Phase 8D-2b, decision 268). Calls the thin Rust commands in
// src-tauri/src/stats_onesample_commands.rs (stats_onesample_* and
// stats_paired_*), and opens and closes the dataset with Phase 2B's
// stats_desc_open and stats_desc_close, unchanged, as the t-test's bridge
// does. The two tests' commands differ only in their prefix, so one class
// serves both. Rust owns every decision: the webview passes a path once (to
// open), slots, run ids and the user's choices, and shows what comes back.
// Export returns bytes; the save dialog writes the one file the user names.
// The method selector's list is the t-test bridge's capability bridge, shared.
import { isTauri } from '../../utils/isTauri';
import { invoke, recorder, type MockResponse } from './statsInvoke';
import type { DatasetView, ExportFormat, ExportView, RunStartedView } from './describeTypes';
import type {
  OneSampleResultView,
  OneSampleRunStateView,
  OneSampleSpec,
  OneSampleValidationView,
  PairedResultView,
  PairedSpec,
} from './onesampleTypes';

/** One test's commands: the file's open and close, and its six own. */
export interface MethodBridge<S, R> {
  /** Reads the file through Level 1's ingest (stats_desc_open). The path is
   *  not kept. */
  open(path: string): Promise<DatasetView>;
  validate(slot: number, spec: S): Promise<OneSampleValidationView>;
  /** Starts a detached run and returns its id at once. */
  run(slot: number, spec: S): Promise<RunStartedView>;
  status(runId: number): Promise<OneSampleRunStateView>;
  result(runId: number): Promise<R>;
  cancel(runId: number): Promise<OneSampleRunStateView>;
  export(runId: number, format: ExportFormat): Promise<ExportView>;
  close(slot: number): Promise<boolean>;
}

export type OneSampleBridge = MethodBridge<OneSampleSpec, OneSampleResultView>;
export type PairedBridge = MethodBridge<PairedSpec, PairedResultView>;

/** Each test's command prefix, as Rust registers them. */
export const ONE_SAMPLE_PREFIX = 'stats_onesample';
export const PAIRED_PREFIX = 'stats_paired';

class TauriMethodBridge<S, R> implements MethodBridge<S, R> {
  constructor(private readonly prefix: string) {}
  open(path: string) {
    return invoke<DatasetView>('stats_desc_open', { path });
  }
  validate(slot: number, spec: S) {
    return invoke<OneSampleValidationView>(`${this.prefix}_validate`, { slot, spec });
  }
  run(slot: number, spec: S) {
    return invoke<RunStartedView>(`${this.prefix}_run`, { slot, spec });
  }
  status(runId: number) {
    return invoke<OneSampleRunStateView>(`${this.prefix}_status`, { runId });
  }
  result(runId: number) {
    return invoke<R>(`${this.prefix}_result`, { runId });
  }
  cancel(runId: number) {
    return invoke<OneSampleRunStateView>(`${this.prefix}_cancel`, { runId });
  }
  export(runId: number, format: ExportFormat) {
    return invoke<ExportView>(`${this.prefix}_export`, { runId, format });
  }
  close(slot: number) {
    return invoke<boolean>('stats_desc_close', { slot });
  }
}

export class TauriOneSampleBridge extends TauriMethodBridge<OneSampleSpec, OneSampleResultView> {
  constructor() {
    super(ONE_SAMPLE_PREFIX);
  }
}

export class TauriPairedBridge extends TauriMethodBridge<PairedSpec, PairedResultView> {
  constructor() {
    super(PAIRED_PREFIX);
  }
}

/** Test/dev double: records every call and its arguments, and returns what
 *  `responses` holds for the command. A path is recorded as `<path>`. */
function mockOf<S, R>(
  prefix: string,
  responses: Partial<Record<string, MockResponse>>
): MethodBridge<S, R> & { calls: Array<[string, Record<string, unknown>]> } {
  const { calls, call } = recorder(responses);
  return {
    calls,
    open: (path) => call('stats_desc_open', { path }, { path: '<path>' }),
    validate: (slot, spec) => call(`${prefix}_validate`, { slot, spec }),
    run: (slot, spec) => call(`${prefix}_run`, { slot, spec }),
    status: (runId) => call(`${prefix}_status`, { runId }),
    result: (runId) => call(`${prefix}_result`, { runId }),
    cancel: (runId) => call(`${prefix}_cancel`, { runId }),
    export: (runId, format) => call(`${prefix}_export`, { runId, format }),
    close: (slot) => call('stats_desc_close', { slot }),
  };
}

export function makeMockOneSampleBridge(responses: Partial<Record<string, MockResponse>> = {}) {
  return mockOf<OneSampleSpec, OneSampleResultView>(ONE_SAMPLE_PREFIX, responses);
}

export function makeMockPairedBridge(responses: Partial<Record<string, MockResponse>> = {}) {
  return mockOf<PairedSpec, PairedResultView>(PAIRED_PREFIX, responses);
}

/** The bridges the screens use: Tauri's in the desktop app; otherwise mocks
 *  that answer nothing, so a browser build shows the refusal, never a
 *  number. */
export function oneSampleBridge(): OneSampleBridge {
  return isTauri ? new TauriOneSampleBridge() : makeMockOneSampleBridge();
}

export function pairedBridge(): PairedBridge {
  return isTauri ? new TauriPairedBridge() : makeMockPairedBridge();
}
