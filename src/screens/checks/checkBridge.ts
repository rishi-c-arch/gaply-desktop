// Gaply — single-agent check bridge. Calls ONE existing Rust command per screen
// and returns its raw report. LOCAL only: passes a file PATH over Tauri IPC,
// never manuscript bytes, and makes no network call.
import {
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
  /** "My papers" library management (durable comparison set). */
  libraryAdd(path: string, title?: string): Promise<number>;
  libraryList(): Promise<LibraryPaper[]>;
  libraryRemove(id: number): Promise<boolean>;
  ai(path: string): Promise<AiDetectionReport>;
  /** Set 5: the two-way tiered AI Check (run_aicheck) — passages + honest %. */
  aicheck(path: string): Promise<AiCheckResult>;
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
  libraryAdd(path: string, title?: string) {
    return this.invoke<number>('add_to_plagiarism_library', { path, title });
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
  aicheck(path: string) {
    return this.invoke<AiCheckResult>('run_aicheck', { path });
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
  validation?: StatsValidityReport;
  onCall?: (cmd: string, path: string) => void;
}): CheckBridge {
  const library: LibraryPaper[] = reports.library ? [...reports.library] : [];
  let nextId = library.reduce((m, p) => Math.max(m, p.id), 0) + 1;
  return {
    async plagiarism(path) {
      reports.onCall?.('check_plagiarism', path);
      return reports.plagiarism!;
    },
    async checkExact(path) {
      reports.onCall?.('check_plagiarism_exact', path);
      return reports.exact!;
    },
    async libraryAdd(path, title) {
      reports.onCall?.('add_to_plagiarism_library', path);
      const id = nextId++;
      const name = title ?? (path.split('/').pop() || 'Untitled paper').replace(/\.[^.]+$/, '');
      library.unshift({ id, title: name, added_at: Math.floor(Date.now() / 1000), source_label: path });
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
    async aicheck(path) {
      reports.onCall?.('run_aicheck', path);
      return reports.aicheck!;
    },
    async validation(path) {
      reports.onCall?.('validate_manuscript', path);
      return reports.validation!;
    },
  };
}
