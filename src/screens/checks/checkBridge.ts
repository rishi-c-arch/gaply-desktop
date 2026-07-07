// Gaply — single-agent check bridge. Calls ONE existing Rust command per screen
// and returns its raw report. LOCAL only: passes a file PATH over Tauri IPC,
// never manuscript bytes, and makes no network call.
import {
  AiDetectionReport,
  PlagiarismReport,
  StatsValidityReport,
} from './agentTypes';

export interface CheckBridge {
  plagiarism(path: string): Promise<PlagiarismReport>;
  ai(path: string): Promise<AiDetectionReport>;
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
  ai(path: string) {
    return this.invoke<AiDetectionReport>('detect_ai', { path });
  }
  validation(path: string, title?: string) {
    return this.invoke<StatsValidityReport>('validate_manuscript', { path, title });
  }
}

/** Test/dev double — returns canned reports, records the paths it was handed
 *  (to prove only a path, never bytes, crosses the seam). */
export function makeMockCheckBridge(reports: {
  plagiarism?: PlagiarismReport;
  ai?: AiDetectionReport;
  validation?: StatsValidityReport;
  onCall?: (cmd: string, path: string) => void;
}): CheckBridge {
  return {
    async plagiarism(path) {
      reports.onCall?.('check_plagiarism', path);
      return reports.plagiarism!;
    },
    async ai(path) {
      reports.onCall?.('detect_ai', path);
      return reports.ai!;
    },
    async validation(path) {
      reports.onCall?.('validate_manuscript', path);
      return reports.validation!;
    },
  };
}
