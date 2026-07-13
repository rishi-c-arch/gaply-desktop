// Gaply — Statistical Analysis Verifier bridge. Calls the three thin Rust
// commands (run_stats_preview / run_stats_verify / run_stats_chat). LOCAL: a
// file PATH crosses the IPC seam, never data bytes. The verify path is fully
// deterministic and offline; only the chat hits the cloud (JWT at the proxy).
import { isTauri } from '../../utils/isTauri';
import { AnalysisSpec, StatsChatTurn, StatsPreview, VerificationReport } from './statsVerifierTypes';

export interface StatsVerifierBridge {
  /** Columns (+ optional manuscript p-value pre-fill) for the spec builder. */
  preview(path: string, manuscriptPath?: string): Promise<StatsPreview>;
  /** Deterministic recompute + reported-vs-recomputed verdict (Set 3). */
  verify(path: string, spec: AnalysisSpec, manuscriptPath?: string): Promise<VerificationReport>;
  /** One analysis-scoped interpretive-chat turn (Set 4). */
  chat(args: {
    path: string;
    spec: AnalysisSpec;
    manuscriptPath?: string;
    question: string;
    language: string;
    userToken?: string;
  }): Promise<StatsChatTurn>;
}

export class TauriStatsVerifierBridge implements StatsVerifierBridge {
  constructor(private getUserToken?: () => string | undefined) {}

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

  chat(args: {
    path: string;
    spec: AnalysisSpec;
    manuscriptPath?: string;
    question: string;
    language: string;
    userToken?: string;
  }) {
    return this.invoke<StatsChatTurn>('run_stats_chat', {
      path: args.path,
      spec: args.spec,
      manuscriptPath: args.manuscriptPath ?? null,
      question: args.question,
      language: args.language,
      userToken: args.userToken ?? this.getUserToken?.() ?? null,
    });
  }
}

/** Test/dev double — records the calls it was handed (to prove path-only + the
 *  spec that crossed the seam) and returns canned reports/turns. */
export function makeMockStatsVerifierBridge(opts: {
  preview?: StatsPreview;
  report?: VerificationReport;
  chat?: (question: string) => StatsChatTurn;
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
    async chat(args) {
      calls.push(['run_stats_chat', args]);
      opts.onCall?.('run_stats_chat', args.question);
      return (
        opts.chat?.(args.question) ?? {
          kind: 'answered',
          answer: `[${args.language}] advisory interpretation`,
          refs: ['v1'],
          advisory: true,
          disclaimer: 'This is an ADVISORY interpretation of your already-computed results, NOT a verification.',
          warnings: [],
          available: true,
        }
      );
    },
  };
}
