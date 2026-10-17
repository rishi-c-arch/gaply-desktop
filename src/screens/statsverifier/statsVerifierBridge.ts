// Gaply — Statistical Analysis Verifier bridge. Calls the two thin Rust
// commands (run_stats_preview / run_stats_verify). LOCAL: a file PATH crosses
// the IPC seam, never data bytes. Fully deterministic and offline. The chat
// command (run_stats_chat) was removed in Research OS Phase 3 (decision 8).
import { isTauri } from '../../utils/isTauri';
import { AnalysisSpec, StatsPreview, VerificationReport } from './statsVerifierTypes';

export interface StatsVerifierBridge {
  /** Columns (+ optional manuscript p-value pre-fill) for the spec builder. */
  preview(path: string, manuscriptPath?: string): Promise<StatsPreview>;
  /** Deterministic recompute + reported-vs-recomputed verdict (Set 3). */
  verify(path: string, spec: AnalysisSpec, manuscriptPath?: string): Promise<VerificationReport>;
}

export class TauriStatsVerifierBridge implements StatsVerifierBridge {
  private async invoke<T>(cmd: string, args: Record<string, unknown>): Promise<T> {
    if (!isTauri) throw new Error('The Statistical Analysis Verifier runs in the Gaply desktop app.');
    const { invoke } = await import('@tauri-apps/api/core');
    return invoke<T>(cmd, args);
  }

  preview(path: string, manuscriptPath?: string) {
    return this.invoke<StatsPreview>('run_stats_preview', { path, manuscriptPath: manuscriptPath ?? null });
  }

  verify(path: string, spec: AnalysisSpec, manuscriptPath?: string) {
    return this.invoke<VerificationReport>('run_stats_verify', {
      path,
      spec,
      manuscriptPath: manuscriptPath ?? null,
    });
  }
}

/** Test/dev double — records the calls it was handed (to prove path-only + the
 *  spec that crossed the seam) and returns canned reports. */
export function makeMockStatsVerifierBridge(opts: {
  preview?: StatsPreview;
  report?: VerificationReport;
  onCall?: (cmd: string, detail: unknown) => void;
}): StatsVerifierBridge & { calls: Array<[string, unknown]> } {
  const calls: Array<[string, unknown]> = [];
  return {
    calls,
    async preview(path, manuscriptPath) {
      calls.push(['run_stats_preview', { path, manuscriptPath }]);
      opts.onCall?.('run_stats_preview', path);
      return opts.preview ?? { headers: [], row_count: 0, prefill_p_value: null };
    },
    async verify(path, spec, manuscriptPath) {
      calls.push(['run_stats_verify', { path, spec, manuscriptPath }]);
      opts.onCall?.('run_stats_verify', path);
      return opts.report!;
    },
  };
}
