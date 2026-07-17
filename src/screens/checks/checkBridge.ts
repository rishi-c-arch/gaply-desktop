// Gaply — single-agent check bridge. Calls ONE existing Rust command per screen
// and returns its raw report. LOCAL only: passes a file PATH over Tauri IPC,
// never manuscript bytes, and makes no network call.
import {
  AiCheckEvent,
  AiCheckMemoryStatus,
  AiCheckResult,
  AiDetectionReport,
  ExactPlagiarismReport,
  LibraryPaper,
  PlagiarismReport,
  StatsValidityReport,
} from './agentTypes';

export interface CheckBridge {
  plagiarism(path: string): Promise<PlagiarismReport>;
  /** Deterministic exact-match check against the "my papers" library. */
  checkExact(path: string): Promise<ExactPlagiarismReport>;
  /** "My papers" library management (durable comparison set). `citationId` (M2
   *  Set 2B) is the OPTIONAL soft anchor → citation_library.id captured by the
   *  add-time picker; omitted → stored NULL (unchanged behavior). */
  libraryAdd(path: string, title?: string, citationId?: string): Promise<number>;
  libraryList(): Promise<LibraryPaper[]>;
  libraryRemove(id: number): Promise<boolean>;
  ai(path: string): Promise<AiDetectionReport>;
  /** Set 5: the two-way tiered AI Check (run_aicheck) — passages + honest %.
   *  `verifyCitations` (C2) is the AND of the AI-Check opt-in and the global
   *  cloud gate; omitted/false keeps AI Check fully on-device. `onEvent`
   *  (progress+cancel Set 2) receives streamed events over the IPC Channel. */
  aicheck(
    path: string,
    verifyCitations?: boolean,
    onEvent?: (ev: AiCheckEvent) => void,
  ): Promise<AiCheckResult>;
  /** Flip the AI Check cancel token — the running analysis stops promptly. */
  cancelAicheck(): Promise<void>;
  /** Pre-flight memory status (re-checkable) — free vs needed + attainable tier. */
  aicheckMemoryStatus(): Promise<AiCheckMemoryStatus>;
  validation(path: string, title?: string): Promise<StatsValidityReport>;
}

export class TauriCheckBridge implements CheckBridge {
  private async invoke<T>(cmd: string, args: Record<string, unknown>): Promise<T> {
    const { invoke } = await import('@tauri-apps/api/core');
    return invoke<T>(cmd, args);
  }
  plagiarism(path: string) {
    return this.invoke<PlagiarismReport>('check_plagiarism', { path });
  }
  checkExact(path: string) {
    return this.invoke<ExactPlagiarismReport>('check_plagiarism_exact', { path });
  }
  libraryAdd(path: string, title?: string, citationId?: string) {
    return this.invoke<number>('add_to_plagiarism_library', { path, title, citationId });
  }
  libraryList() {
    return this.invoke<LibraryPaper[]>('list_plagiarism_library', {});
  }
  libraryRemove(id: number) {
    return this.invoke<boolean>('remove_from_plagiarism_library', { id });
  }
  ai(path: string) {
    return this.invoke<AiDetectionReport>('detect_ai', { path });
  }
  async aicheck(path: string, verifyCitations?: boolean, onEvent?: (ev: AiCheckEvent) => void) {
    const { invoke, Channel } = await import('@tauri-apps/api/core');
    const ch = new Channel<AiCheckEvent>();
    if (onEvent) ch.onmessage = onEvent;
    return invoke<AiCheckResult>('run_aicheck', { path, verifyCitations, onEvent: ch });
  }
  cancelAicheck() {
    return this.invoke<void>('cancel_aicheck', {});
  }
  aicheckMemoryStatus() {
    return this.invoke<AiCheckMemoryStatus>('aicheck_memory_status', {});
  }
  validation(path: string, title?: string) {
    return this.invoke<StatsValidityReport>('validate_manuscript', { path, title });
  }
}

/** Test/dev double — returns canned reports, records the paths it was handed
 *  (to prove only a path, never bytes, crosses the seam). */
export function makeMockCheckBridge(reports: {
  plagiarism?: PlagiarismReport;
  exact?: ExactPlagiarismReport;
  /** Stateful mock library (add/list/remove mutate it). */
  library?: LibraryPaper[];
  ai?: AiDetectionReport;
  aicheck?: AiCheckResult;
  /** Events the mock replays to `onEvent` before resolving (Set 2). If they
   *  include a `cancelled` event, the run stays in-flight until `cancelAicheck`
   *  is called, then emits it and rejects with code `cancelled` (mirrors the
   *  backend). */
  aicheckEvents?: AiCheckEvent[];
  /** Keep the run pending after replaying events (to assert the mid-run UI). */
  aicheckPending?: boolean;
  memoryStatus?: AiCheckMemoryStatus;
  validation?: StatsValidityReport;
  onCall?: (cmd: string, path: string, verifyCitations?: boolean) => void;
}): CheckBridge {
  const library: LibraryPaper[] = reports.library ? [...reports.library] : [];
  let nextId = library.reduce((m, p) => Math.max(m, p.id), 0) + 1;
  let resolveCancelGate: (() => void) | null = null;
  return {
    async plagiarism(path) {
      reports.onCall?.('check_plagiarism', path);
      return reports.plagiarism!;
    },
    async checkExact(path) {
      reports.onCall?.('check_plagiarism_exact', path);
      return reports.exact!;
    },
    async libraryAdd(path, title, citationId) {
      reports.onCall?.('add_to_plagiarism_library', path);
      const id = nextId++;
      const name = title ?? (path.split('/').pop() || 'Untitled paper').replace(/\.[^.]+$/, '');
      // Record citation_id so tests can prove the picked link crossed the seam
      // (undefined when the picker was left "— none —", i.e. stored NULL).
      library.unshift({ id, title: name, added_at: Math.floor(Date.now() / 1000), source_label: path, citation_id: citationId });
      return id;
    },
    async libraryList() {
      reports.onCall?.('list_plagiarism_library', '');
      return [...library];
    },
    async libraryRemove(id) {
      reports.onCall?.('remove_from_plagiarism_library', String(id));
      const i = library.findIndex((p) => p.id === id);
      if (i === -1) return false;
      library.splice(i, 1);
      return true;
    },
    async ai(path) {
      reports.onCall?.('detect_ai', path);
      return reports.ai!;
    },
    async aicheck(path, verifyCitations, onEvent) {
      reports.onCall?.('run_aicheck', path, verifyCitations);
      const evs = reports.aicheckEvents ?? [];
      const cancelIdx = evs.findIndex((e) => e.type === 'cancelled');
      if (cancelIdx !== -1) {
        evs.slice(0, cancelIdx).forEach((ev) => onEvent?.(ev));
        await new Promise<void>((res) => {
          resolveCancelGate = res;
        });
        onEvent?.(evs[cancelIdx]);
        // eslint-disable-next-line no-throw-literal
        throw { code: 'cancelled', message: 'cancelled' };
      }
      evs.forEach((ev) => onEvent?.(ev));
      if (reports.aicheckPending) return new Promise<AiCheckResult>(() => {});
      return reports.aicheck!;
    },
    async cancelAicheck() {
      reports.onCall?.('cancel_aicheck', '');
      resolveCancelGate?.();
    },
    async aicheckMemoryStatus() {
      reports.onCall?.('aicheck_memory_status', '');
      return (
        reports.memoryStatus ?? {
          free_mb: 4096,
          total_gb: 8,
          stage1_fits: true,
          stage1_available: true,
          deep_fits: true,
          deep_need_mb: 2400,
          tier_attainable: 'compact_1_5b',
          tier_label: 'the compact 1.5B deep verifier',
          hint: 'Ready: the compact 1.5B deep verifier will run.',
        }
      );
    },
    async validation(path) {
      reports.onCall?.('validate_manuscript', path);
      return reports.validation!;
    },
  };
}
