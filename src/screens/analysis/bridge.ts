// Gaply — analysis bridge: the seam between the theater UI and the EXISTING
// Rust agents (Tauri commands). Everything here is LOCAL: the Rust core reads
// the manuscript from a file PATH and returns structured reports. This bridge
// only ever passes a PATH over Tauri IPC — never the manuscript bytes — and
// makes NO network calls. The verification lane is the sole cloud step and is
// gated to premium (and still proxy-bound, structured-summaries-only).
//
// The bridge PROTOCOL supports true per-section streaming (fully exercised by
// makeMockBridge in tests). The current Rust commands are one-shot, so the real
// TauriAnalysisBridge emits real stage results plus the real section list, and
// streams per-section AI risk from the real report. A future event-emitting
// Rust command would let the real path stream sections live during a stage;
// that wiring is intentionally deferred (same pattern as the rest of the build).

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
  | { kind: 'complete' }
  | { kind: 'aborted' };

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
    const { invoke } = await import('@tauri-apps/api/core');
    const { path, title, tier } = input;
    const checkAbort = () => {
      if (signal?.aborted) throw new AnalysisAbortError();
    };

    try {
      // 1) Extraction ------------------------------------------------------
      emit({ kind: 'stage-start', stage: 'extraction' });
      checkAbort();
      // NOTE: only the PATH crosses IPC — never the file bytes.
      const ex = (await invoke('extract_manuscript', { path, title })) as any;
      const sections: string[] = (ex.sections ?? []).map(
        (s: any) => s.heading || s.kind || 'section'
      );
      sections.forEach((t, i) =>
        emit({ kind: 'section', stage: 'extraction', index: i + 1, total: sections.length, title: t })
      );
      emit({
        kind: 'stage-done',
        stage: 'extraction',
        summary: `Parsed ${sections.length} sections, ${(ex.citations ?? []).length} citations, ${(ex.statistics ?? []).length} stat claims`,
        data: { sections },
      });

      // 2) Validation / Maths (deterministic) ------------------------------
      emit({ kind: 'stage-start', stage: 'validation' });
      checkAbort();
      const v = (await invoke('validate_manuscript', { path, title })) as any;
      emit({
        kind: 'stage-done',
        stage: 'validation',
        summary: v.passed
          ? 'All deterministic statistical rules passed'
          : `${(v.flags ?? []).length} deterministic flag(s) — mathematically certain`,
        data: { passed: v.passed, flags: v.flags ?? [] },
      });

      // 3) AI Check (per-section risk + mandatory disclaimer) --------------
      emit({ kind: 'stage-start', stage: 'ai' });
      checkAbort();
      const ai = (await invoke('detect_ai', { path })) as any;
      const aiSections: any[] = ai.sections ?? [];
      aiSections.forEach((s, i) =>
        emit({
          kind: 'section',
          stage: 'ai',
          index: i + 1,
          total: aiSections.length,
          title: String(s.section),
          risk: riskFromSignal(s.signal),
        })
      );
      emit({
        kind: 'stage-done',
        stage: 'ai',
        summary: `Signal: ${ai.signal} · ${ai.confidence} confidence`,
        data: { signal: ai.signal, disclaimer: ai.disclaimer, sections: aiSections },
      });

      // 4) Plagiarism (per-session isolated store) -------------------------
      emit({ kind: 'stage-start', stage: 'plagiarism' });
      checkAbort();
      const pl = (await invoke('check_plagiarism', { path })) as any;
      const maxSim = Math.max(
        0,
        ...[...(pl.corpus_matches ?? []), ...(pl.self_matches ?? [])].map((m: any) => m.similarity)
      );
      emit({
        kind: 'stage-done',
        stage: 'plagiarism',
        summary: `Peak similarity ${(maxSim * 100).toFixed(0)}% · ${(pl.corpus_matches ?? []).length} corpus / ${(pl.self_matches ?? []).length} self`,
        data: { maxSimilarity: maxSim, note: pl.note },
      });

      // 5) RAG (journal match + international quartile) ---------------------
      emit({ kind: 'stage-start', stage: 'rag' });
      checkAbort();
      const hits = (await invoke('rag_search', {
        query: title || 'reporting standards and reference style',
        topK: 3,
        sourceFilter: 'journal_guideline',
      })) as any[];
      const top = hits?.[0];
      emit({
        kind: 'stage-done',
        stage: 'rag',
        summary: top ? `Matched to ${top.title} · SJR ${top.quartile ?? 'Q?'}` : 'No journal guideline matched (local corpus)',
        data: { matched: Boolean(top), journal: top?.title, quartile: top?.quartile },
      });

      // 6) Verification (cloud, premium only) ------------------------------
      if (tier !== 'premium') {
        emit({
          kind: 'stage-locked',
          stage: 'verification',
          reason: 'Cloud citation verification is a PublishReady feature — unlock to run the ReConcile debate.',
        });
      } else {
        // Premium: the real cloud verification runs through the Tailscale proxy
        // (structured summaries only, never the manuscript). That proxy command
        // is not wired as a Tauri command yet, so we present the ReConcile
        // debate as a live preview until it lands.
        emit({ kind: 'stage-start', stage: 'verification' });
        for (const turn of PREVIEW_DEBATE) {
          checkAbort();
          emit({ kind: 'debate-turn', turn });
        }
        emit({
          kind: 'stage-done',
          stage: 'verification',
          summary: 'Debate preview — live cloud verification wiring pending',
          data: { preview: true },
        });
      }

      emit({ kind: 'complete' });
    } catch (err) {
      if (err instanceof AnalysisAbortError) {
        emit({ kind: 'aborted' });
        return;
      }
      throw err;
    }
  }
}

function riskFromSignal(signal: string): number {
  switch (signal) {
    case 'leans_ai_like':
    case 'LeansAiLike':
      return 0.7;
    case 'leans_human_like':
    case 'LeansHumanLike':
      return 0.2;
    default:
      return 0.45;
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
