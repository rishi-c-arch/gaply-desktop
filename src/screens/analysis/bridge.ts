// Gaply — analysis bridge: the seam between the theater UI and the EXISTING
// Rust agents (Tauri commands). Everything here is LOCAL: the Rust core reads
// the manuscript from a file PATH and returns structured reports. This bridge
// only ever passes a PATH over Tauri IPC — never the manuscript bytes — and
// makes NO network calls itself.
//
// THE VERIFICATION LANE IS NOT LOCAL, and this file used to say it was. It
// sends each reference to CrossRef/OpenAlex — by TITLE when the reference has
// no DOI (`refverify.rs:474`, `query.bibliographic=`) — and it did so with no
// consent check anywhere in the path. Seven other screens gated their cloud
// work on `mayUseCloud`; this one did not, and a measured run on a two-DOI
// manuscript reported "2 reference(s) checked via public APIs" with the toggle
// off.
//
// The gate below is now read here and PASSED to Rust, where the lane refuses
// (`pipeline.rs`, `NetworkConsent`). Reading it here alone would have made the
// screens agree and left the boundary nowhere: localStorage is a preference the
// backend cannot see and the user can edit.
//
// The bridge PROTOCOL supports true per-section streaming (fully exercised by
// makeMockBridge in tests). The current Rust commands are one-shot, so the real
// TauriAnalysisBridge emits real stage results plus the real section list, and
// streams per-section AI risk from the real report. A future event-emitting
// Rust command would let the real path stream sections live during a stage;
// that wiring is intentionally deferred (same pattern as the rest of the build).

import { mayUseCloud } from '../settings/settingsStore';

export type AgentStage =
  | 'extraction'
  | 'validation'
  | 'ai'
  | 'plagiarism'
  | 'rag'
  | 'verification';

export interface DebateTurn {
  speaker: string;
  text: string;
  verdict?: string;
}

export type AnalysisEvent =
  | { kind: 'stage-start'; stage: AgentStage }
  | { kind: 'section'; stage: AgentStage; index: number; total: number; title?: string; risk?: number }
  | { kind: 'stage-done'; stage: AgentStage; summary: string; data?: unknown }
  | { kind: 'stage-locked'; stage: AgentStage; reason: string }
  | { kind: 'debate-turn'; turn: DebateTurn }
  | { kind: 'stage-error'; stage: AgentStage; message: string }
  | { kind: 'complete'; reportId?: string }
  | { kind: 'aborted' };

/** Wire shape of the Rust `AnalysisEvent` (serde tag = "type", camelCase). */
type RustAnalysisEvent =
  | { type: 'stageStarted'; stage: string; index: number; total: number }
  | { type: 'stageProgress'; stage: string; pct: number }
  | { type: 'section'; stage: string; index: number; total: number; title: string }
  | { type: 'stageCompleted'; stage: string; summary: string }
  | { type: 'finished'; reportId: string }
  | { type: 'failed'; stage: string; message: string };

const KNOWN_STAGES: readonly AgentStage[] = ['extraction', 'validation', 'ai', 'plagiarism', 'rag', 'verification'];
const isLane = (s: string): s is AgentStage => (KNOWN_STAGES as readonly string[]).includes(s);

export interface RunInput {
  path: string;
  title?: string;
  tier: 'free' | 'premium';
}

export interface AnalysisBridge {
  run(input: RunInput, emit: (e: AnalysisEvent) => void, signal?: AbortSignal): Promise<void>;
}

export class AnalysisAbortError extends Error {
  constructor() {
    super('aborted');
    this.name = 'AnalysisAbortError';
  }
}

/* ------------------------------------------------------------------ */
/* Real bridge — calls the existing Rust Tauri commands (LOCAL only). */
/* ------------------------------------------------------------------ */

export class TauriAnalysisBridge implements AnalysisBridge {
  async run(input: RunInput, emit: (e: AnalysisEvent) => void, signal?: AbortSignal): Promise<void> {
    // Lazy import so the browser/test bundle never needs Tauri.
    const { invoke, Channel } = await import('@tauri-apps/api/core');
    const { path, title } = input;
    // THE gate, read once per run and sent with the call. The suite is
    // `citation_verification` because that is exactly what the verification
    // lane does — reference metadata to CrossRef/OpenAlex — and it is the same
    // suite AI Check and the Citation Manager gate the same work on.
    const allowNetwork = mayUseCloud('citation_verification');

    // One real command (run_full_analysis) drives all six lanes; per-stage
    // progress streams back over a typed IPC Channel. We map the Rust
    // AnalysisEvent onto the theater's event vocabulary. Only a PATH crosses
    // IPC — never the manuscript bytes.
    let done = false;
    const channel = new Channel<RustAnalysisEvent>();
    channel.onmessage = (ev) => {
      // Cooperative abort: once aborted, stop forwarding events. (Server-side
      // cancellation of an in-flight run is a later enhancement.)
      if (signal?.aborted) return;
      switch (ev.type) {
        case 'stageStarted':
          if (isLane(ev.stage)) emit({ kind: 'stage-start', stage: ev.stage });
          break;
        case 'stageProgress':
          // Lane is already "running"; no per-section data to attach. (The
          // pipeline emits this during verification's per-reference loop.)
          break;
        case 'section':
          if (isLane(ev.stage)) {
            emit({ kind: 'section', stage: ev.stage, index: ev.index, total: ev.total, title: ev.title });
          }
          break;
        case 'stageCompleted':
          if (isLane(ev.stage)) emit({ kind: 'stage-done', stage: ev.stage, summary: ev.summary });
          break;
        case 'finished':
          done = true;
          emit({ kind: 'complete', reportId: ev.reportId });
          break;
        case 'failed':
          done = true;
          // Synthesis (debate/compile) has no lane; surface it on the last lane
          // so the user sees the real error rather than a silent stall.
          emit({ kind: 'stage-error', stage: isLane(ev.stage) ? ev.stage : 'verification', message: ev.message });
          break;
      }
    };

    try {
      await invoke('run_full_analysis', { path, title, allowNetwork, onEvent: channel });
    } catch (err) {
      if (signal?.aborted) {
        emit({ kind: 'aborted' });
        return;
      }
      // The command rejected without a terminal Channel event (e.g. it failed
      // before emitting). Never fabricate completion — surface the real error.
      if (!done) {
        const message = err instanceof Error ? err.message : String(err);
        emit({ kind: 'stage-error', stage: 'extraction', message });
      }
    }
  }
}

const PREVIEW_DEBATE: DebateTurn[] = [
  { speaker: 'Verification', text: 'Citation [12] resolves cleanly on CrossRef; DOI and title match.', verdict: 'SUPPORTED' },
  { speaker: 'Plagiarism', text: 'That passage shares 0.86 similarity with a retracted 2019 paper.' },
  { speaker: 'RevisingAgent', text: 'Reconsidering — the peer finding is grounded; downgrading.', verdict: 'UNKNOWN' },
];

/* ------------------------------------------------------------------ */
/* Mock bridge — scripted event stream for tests / browser preview.   */
/* ------------------------------------------------------------------ */

export interface MockScript {
  sections: string[];
  tier?: 'free' | 'premium';
  /** ms between events (0 in tests). */
  stepMs?: number;
  debate?: DebateTurn[];
}

export function makeMockBridge(script: MockScript): AnalysisBridge {
  return {
    async run(input, emit, signal) {
      const stepMs = script.stepMs ?? 0;
      const wait = () => new Promise<void>((r) => setTimeout(r, stepMs));
      const aborted = () => signal?.aborted;

      const localStages: AgentStage[] = ['extraction', 'validation', 'ai', 'plagiarism', 'rag'];
      const N = script.sections.length;

      for (const stage of localStages) {
        if (aborted()) return emit({ kind: 'aborted' });
        emit({ kind: 'stage-start', stage });
        // per-section streaming (extraction + ai carry section data)
        if (stage === 'extraction' || stage === 'ai') {
          for (let i = 0; i < N; i++) {
            if (aborted()) return emit({ kind: 'aborted' });
            await wait();
            emit({
              kind: 'section',
              stage,
              index: i + 1,
              total: N,
              title: script.sections[i],
              risk: stage === 'ai' ? (i % 3) / 3 : undefined,
            });
          }
        }
        await wait();
        emit({
          kind: 'stage-done',
          stage,
          summary:
            stage === 'extraction'
              ? `Parsed ${N} sections, 318 citations, 27 stat claims`
              : `${stage} complete`,
          data:
            stage === 'extraction'
              ? { sections: script.sections }
              : stage === 'ai'
              ? { disclaimer: 'Statistical signal only — never proof of AI authorship.' }
              : undefined,
        });
      }

      // verification lane. The script's tier defines the SCENARIO (the page's
      // tier prop is hardcoded free until F6 wires the subscription); the real
      // TauriAnalysisBridge uses input.tier.
      const tier = script.tier ?? input.tier ?? 'free';
      if (tier !== 'premium') {
        emit({ kind: 'stage-locked', stage: 'verification', reason: 'premium feature' });
      } else {
        emit({ kind: 'stage-start', stage: 'verification' });
        for (const turn of script.debate ?? PREVIEW_DEBATE) {
          if (aborted()) return emit({ kind: 'aborted' });
          await wait();
          emit({ kind: 'debate-turn', turn });
        }
        emit({ kind: 'stage-done', stage: 'verification', summary: 'verification complete' });
      }
      emit({ kind: 'complete' });
    },
  };
}
