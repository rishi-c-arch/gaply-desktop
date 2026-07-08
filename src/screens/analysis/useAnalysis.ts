// Gaply — analysis controller: consumes an AnalysisBridge event stream into
// browsable state. Six lanes, determinate per-stage/per-section progress,
// section browsing while later stages run, and abort.
import { useCallback, useMemo, useRef, useState } from 'react';
import {
  AgentStage,
  AnalysisBridge,
  AnalysisEvent,
  DebateTurn,
  RunInput,
} from './bridge';

export type StageStatus = 'pending' | 'running' | 'done' | 'locked' | 'error';

export interface LaneState {
  stage: AgentStage;
  status: StageStatus;
  sectionIndex: number;
  sectionTotal: number;
  summary?: string;
  reason?: string; // locked reason / error message
  data?: unknown;
}

export const STAGE_ORDER: AgentStage[] = [
  'extraction',
  'validation',
  'ai',
  'plagiarism',
  'rag',
  'verification',
];

export const STAGE_LABEL: Record<AgentStage, string> = {
  extraction: 'Extraction',
  validation: 'Validation / Maths',
  ai: 'AI Check',
  plagiarism: 'Plagiarism',
  rag: 'RAG · Journal',
  verification: 'Verification (cloud)',
};

export interface SectionRisk {
  title: string;
  risk?: number;
}

export interface AnalysisState {
  running: boolean;
  complete: boolean;
  aborted: boolean;
  lanes: Record<AgentStage, LaneState>;
  /** Completed sections (from extraction) the user can browse mid-scan. */
  sections: SectionRisk[];
  debate: DebateTurn[];
  /** Set on completion — the id of the compiled report to load in the viewer. */
  reportId?: string;
}

function initialLanes(): Record<AgentStage, LaneState> {
  return STAGE_ORDER.reduce((acc, stage) => {
    acc[stage] = { stage, status: 'pending', sectionIndex: 0, sectionTotal: 0 };
    return acc;
  }, {} as Record<AgentStage, LaneState>);
}

export function useAnalysis(bridge: AnalysisBridge) {
  const [state, setState] = useState<AnalysisState>({
    running: false,
    complete: false,
    aborted: false,
    lanes: initialLanes(),
    sections: [],
    debate: [],
  });
  const abortRef = useRef<AbortController | null>(null);

  const apply = useCallback((e: AnalysisEvent) => {
    setState((prev) => {
      const lanes = { ...prev.lanes };
      let sections = prev.sections;
      let debate = prev.debate;
      let complete = prev.complete;
      let aborted = prev.aborted;
      let running = prev.running;
      let reportId = prev.reportId;

      switch (e.kind) {
        case 'stage-start':
          lanes[e.stage] = { ...lanes[e.stage], status: 'running' };
          break;
        case 'section':
          lanes[e.stage] = {
            ...lanes[e.stage],
            sectionIndex: e.index,
            sectionTotal: e.total,
          };
          // extraction builds the browsable section list; ai annotates risk
          if (e.stage === 'extraction' && e.title) {
            sections = [...sections];
            sections[e.index - 1] = { title: e.title, risk: sections[e.index - 1]?.risk };
          }
          if (e.stage === 'ai' && e.risk !== undefined) {
            sections = [...sections];
            const existing = sections[e.index - 1];
            sections[e.index - 1] = { title: existing?.title ?? e.title ?? `Section ${e.index}`, risk: e.risk };
          }
          break;
        case 'stage-done':
          lanes[e.stage] = { ...lanes[e.stage], status: 'done', summary: e.summary, data: e.data };
          break;
        case 'stage-locked':
          lanes[e.stage] = { ...lanes[e.stage], status: 'locked', reason: e.reason };
          break;
        case 'stage-error':
          lanes[e.stage] = { ...lanes[e.stage], status: 'error', reason: e.message };
          break;
        case 'debate-turn':
          debate = [...debate, e.turn];
          break;
        case 'complete':
          complete = true;
          running = false;
          reportId = e.reportId ?? reportId;
          break;
        case 'aborted':
          aborted = true;
          running = false;
          break;
      }
      return { ...prev, lanes, sections, debate, complete, aborted, running, reportId };
    });
  }, []);

  const start = useCallback(
    async (input: RunInput) => {
      const controller = new AbortController();
      abortRef.current = controller;
      setState({
        running: true,
        complete: false,
        aborted: false,
        lanes: initialLanes(),
        sections: [],
        debate: [],
        reportId: undefined,
      });
      try {
        await bridge.run(input, apply, controller.signal);
      } catch (err) {
        // e.g. the real Tauri bridge invoked outside the desktop app. Surface
        // it on the current lane instead of crashing the theater.
        apply({
          kind: 'stage-error',
          stage: 'extraction',
          message: err instanceof Error ? err.message : 'analysis failed',
        });
        setState((prev) => ({ ...prev, running: false }));
      }
    },
    [bridge, apply]
  );

  const abort = useCallback(() => abortRef.current?.abort(), []);

  const overallPct = useMemo(() => {
    const done = STAGE_ORDER.filter(
      (s) => ['done', 'locked'].includes(state.lanes[s].status)
    ).length;
    return Math.round((done / STAGE_ORDER.length) * 100);
  }, [state.lanes]);

  return { state, start, abort, overallPct };
}
