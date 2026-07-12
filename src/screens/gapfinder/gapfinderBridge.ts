// Gaply — Research Gap Finder bridge (Set 7). Thin typed adapters over the
// Sets 2–6 Tauri commands. Papers travel PATH-ONLY over IPC (never file
// bytes); every cloud command carries the user's JWT so the proxy's
// server-side entitlement gate (Set 8) can enforce — the client checks are
// UX-only. No secrets, no prices here.
import { isTauri } from '../../utils/isTauri';

/* ------------------------- backend shapes (snake_case) ------------------- */

export interface PaperDigest {
  id: string;
  origin: string;
  title: string;
  headings: string[];
  summary: string;
  claims: string[];
  reference_count: number;
  truncated: boolean;
}

export type PaperIngest =
  | { status: 'ingested'; id: string; origin: string; rag: string }
  | { status: 'unavailable'; origin: string; reason: string };

export interface CorpusReport {
  session: string;
  results: PaperIngest[];
  digests: PaperDigest[];
  note: string;
}

export interface GroundedGap {
  description: string;
  rationale: string;
  paper_refs: string[];
  gate_flags: string[];
}

export interface Suggestion {
  description: string;
  note: string;
  /** ALWAYS true — set by the backend gate, never by the model. */
  ungrounded_suggestion: boolean;
}

export interface GapFindings {
  grounded_gaps: GroundedGap[];
  suggestions: Suggestion[];
  warnings: string[];
  available: boolean;
}

export interface GapFinderOutcome {
  findings: GapFindings;
  proxy_payload: unknown;
}

export interface ResearcherConstraints {
  funding_level: string;
  lab_access: string;
  available_facilities: string[];
  time_horizon: string;
  team_size: string;
  other_constraints: string[];
}

export const EMPTY_CONSTRAINTS: ResearcherConstraints = {
  funding_level: '',
  lab_access: '',
  available_facilities: [],
  time_horizon: '',
  team_size: '',
  other_constraints: [],
};

export interface QaTurn {
  constraints: ResearcherConstraints;
  follow_up_question: string;
  achievable_gaps: GroundedGap[];
  suggestions: Suggestion[];
  warnings: string[];
  kind: 'answered' | 'refused_ghostwriting' | 'unavailable';
  available: boolean;
}

export interface GapDraft {
  gap_ref: string;
  paper_refs: string[];
  objectives: string[];
  methodology_steps: string[];
  expected_outcomes: string[];
  feasibility_notes: string[];
  gate_flags: string[];
}

export interface DraftResult {
  drafts: GapDraft[];
  warnings: string[];
  kind: 'answered' | 'refused_ghostwriting' | 'unavailable';
  available: boolean;
}

export interface YearCount {
  year: number;
  works: number;
}

export interface JournalVerification {
  query: string;
  name: string | null;
  issn: string | null;
  doaj_registered: boolean | null;
  openalex_in_doaj: boolean | null;
  works_by_year: YearCount[];
  recent_activity: boolean | null;
  scope: string[];
  warning: string | null;
  reasons: string[];
  verified_sources: string[];
  unverified: string[];
}

export interface FitAssessment {
  gap_ref: string;
  journal_ref: string;
  verdict: 'good_fit' | 'possible_fit' | 'poor_fit' | 'unknown';
  reasoning: string;
  gate_flags: string[];
}

export interface FitResult {
  fits: FitAssessment[];
  warnings: string[];
  kind: string;
  available: boolean;
}

/* --------------------------------- bridge -------------------------------- */

export interface GapFinderBridge {
  buildCorpus(input: { session: string; paths: string[]; links: string[] }): Promise<CorpusReport>;
  findGaps(input: { session: string; corpus: CorpusReport; userToken?: string }): Promise<GapFinderOutcome>;
  qaTurn(input: {
    session: string;
    corpus: CorpusReport;
    groundedGaps: GroundedGap[];
    constraints: ResearcherConstraints;
    latestAnswer: string;
    userToken?: string;
  }): Promise<QaTurn>;
  draft(input: {
    session: string;
    corpus: CorpusReport;
    achievableGaps: GroundedGap[];
    constraints: ResearcherConstraints;
    userNote: string;
    userToken?: string;
  }): Promise<DraftResult>;
  verifyJournal(input: {
    issn: string;
    name?: string;
    localPredatorySignals?: string[];
  }): Promise<JournalVerification>;
  fit(input: {
    session: string;
    corpus: CorpusReport;
    achievableGaps: GroundedGap[];
    journalCard: JournalVerification;
    userToken?: string;
  }): Promise<FitResult>;
}

/** Production bridge: Tauri IPC, desktop-app only. */
export class TauriGapFinderBridge implements GapFinderBridge {
  private async invoke<T>(cmd: string, args: Record<string, unknown>): Promise<T> {
    if (!isTauri) {
      throw new Error('The Research Gap Finder runs in the Gaply desktop app.');
    }
    const { invoke } = await import('@tauri-apps/api/core');
    return (await invoke(cmd, args)) as T;
  }

  buildCorpus({ session, paths, links }: { session: string; paths: string[]; links: string[] }) {
    return this.invoke<CorpusReport>('build_gapfinder_corpus', { session, paths, links });
  }
  findGaps({ session, corpus, userToken }: { session: string; corpus: CorpusReport; userToken?: string }) {
    return this.invoke<GapFinderOutcome>('run_gap_finder', {
      session,
      corpus,
      userToken: userToken ?? null,
    });
  }
  qaTurn(input: {
    session: string;
    corpus: CorpusReport;
    groundedGaps: GroundedGap[];
    constraints: ResearcherConstraints;
    latestAnswer: string;
    userToken?: string;
  }) {
    return this.invoke<QaTurn>('run_gapfinder_qa', {
      session: input.session,
      corpus: input.corpus,
      groundedGaps: input.groundedGaps,
      constraints: input.constraints,
      latestAnswer: input.latestAnswer,
      userToken: input.userToken ?? null,
    });
  }
  draft(input: {
    session: string;
    corpus: CorpusReport;
    achievableGaps: GroundedGap[];
    constraints: ResearcherConstraints;
    userNote: string;
    userToken?: string;
  }) {
    return this.invoke<DraftResult>('run_gapfinder_draft', {
      session: input.session,
      corpus: input.corpus,
      achievableGaps: input.achievableGaps,
      constraints: input.constraints,
      userNote: input.userNote,
      userToken: input.userToken ?? null,
    });
  }
  verifyJournal({ issn, name, localPredatorySignals }: { issn: string; name?: string; localPredatorySignals?: string[] }) {
    return this.invoke<JournalVerification>('verify_journal_registry', {
      issn,
      name: name ?? null,
      localPredatorySignals: localPredatorySignals ?? null,
    });
  }
  fit(input: {
    session: string;
    corpus: CorpusReport;
    achievableGaps: GroundedGap[];
    journalCard: JournalVerification;
    userToken?: string;
  }) {
    return this.invoke<FitResult>('run_gapfinder_fit', {
      session: input.session,
      corpus: input.corpus,
      achievableGaps: input.achievableGaps,
      journalCard: input.journalCard,
      userToken: input.userToken ?? null,
    });
  }
}

/** Test/dev double: scriptable per-method responders; records every call. */
export function makeGapFinderMock(overrides: Partial<GapFinderBridge> = {}): GapFinderBridge & {
  calls: Array<{ method: string; input: unknown }>;
} {
  const calls: Array<{ method: string; input: unknown }> = [];
  const record = <T,>(method: string, input: unknown, out: T): Promise<T> => {
    calls.push({ method, input });
    return Promise.resolve(out);
  };
  const mock: GapFinderBridge & { calls: typeof calls } = {
    calls,
    buildCorpus: (input) => {
      const origins = [...input.paths, ...input.links];
      return record('buildCorpus', input, {
        session: input.session,
        results: origins.map((o, i) => ({ status: 'ingested' as const, id: `p${i + 1}`, origin: o, rag: 'ingested' })),
        digests: origins.map((o, i) => ({
          id: `p${i + 1}`,
          origin: o,
          title: `Paper ${i + 1}`,
          headings: ['Abstract', 'Methods'],
          summary: 'A study of sleep and memory.',
          claims: ['p = 0.002'],
          reference_count: 3,
          truncated: false,
        })),
        note: `${origins.length}/${origins.length} paper(s) ingested`,
      });
    },
    findGaps: (input) =>
      record('findGaps', input, {
        findings: {
          grounded_gaps: [
            { description: 'No field study tests the dose-response curve', rationale: 'lab-only so far', paper_refs: ['p1'], gate_flags: [] },
          ],
          suggestions: [
            { description: 'Explore an adjacent modality entirely', note: 'broad hunch', ungrounded_suggestion: true },
          ],
          warnings: [],
          available: true,
        },
        proxy_payload: {},
      }),
    qaTurn: (input) =>
      record('qaTurn', input, {
        constraints: { ...input.constraints, funding_level: input.constraints.funding_level || 'small internal grant' },
        follow_up_question: 'Do you have wet-lab access?',
        achievable_gaps: input.groundedGaps.slice(0, 1),
        suggestions: [],
        warnings: [],
        kind: 'answered' as const,
        available: true,
      }),
    draft: (input) =>
      record('draft', input, {
        drafts: [
          {
            gap_ref: 'g1',
            paper_refs: ['p1'],
            objectives: ['Quantify the dose-response curve in home settings'],
            methodology_steps: ['Recruit 40 adults with wearables', 'Track sleep for 8 weeks', 'Fit mixed-effects model'],
            expected_outcomes: ['Estimated marginal benefit per 30min'],
            feasibility_notes: ['Fits a small grant'],
            gate_flags: [],
          },
        ],
        warnings: [],
        kind: 'answered' as const,
        available: true,
      }),
    verifyJournal: (input) =>
      record('verifyJournal', input, {
        query: `${input.name ?? ''} (${input.issn})`,
        name: 'Journal of Sleep Research',
        issn: input.issn,
        doaj_registered: true,
        openalex_in_doaj: true,
        works_by_year: [{ year: 2026, works: 180 }],
        recent_activity: true,
        scope: ['Sleep medicine', 'Cognitive psychology'],
        warning: null,
        reasons: [],
        verified_sources: ['openalex', 'doaj'],
        unverified: [],
      }),
    fit: (input) =>
      record('fit', input, {
        fits: [
          { gap_ref: 'g1', journal_ref: 'j1', verdict: 'good_fit' as const, reasoning: 'Inside the sleep-medicine scope.', gate_flags: [] },
        ],
        warnings: [],
        kind: 'answered',
        available: true,
      }),
    ...overrides,
  };
  return mock;
}
