// Gaply — PublishReady orchestration bridge. Runs ALL SIX agents via the Report
// Compiler + ReConcile debate; the Verification Agent (cloud) does citation-
// hallucination checking through the Railway proxy. The manuscript stays on the
// device — only the structured payload from buildProxyPayload() is sent.
//
// The full local-agents → structured-summary → proxy → compile_report pipeline
// is a backend concern (compile_report / verify aren't Tauri commands yet — same
// deferral as F6/F8). The real TauriPublishReadyBridge is documented; tests use
// makePublishReadyMock.
import { isTauri } from '../../utils/isTauri';
import { PublishReadyReport } from '../report/reportTypes';
import { buildProxyPayload } from './buildPayload';
import { synthesizeReviewerLetter } from './synthesize';
import {
  ProxyReviewPayload,
  PublishReadyResult,
  Recommendation,
  ReviewerLetter,
  TargetJournal,
} from './publishReadyTypes';

/** Result of ingesting the target journal's guidelines into the local
 *  `journal_guideline` corpus (H4). Wire shape from Rust `GuidelinesReport`
 *  (snake_case). Never throws for a bad page — `any_ingested: false` + an honest
 *  `note` (unreachable / quarantined / no substantive content). */
export interface GuidelinesIngestResult {
  any_ingested: boolean;
  note: string;
}

export interface PublishReadyBridge {
  /** `userToken` (Set 8): the signed-in user's Supabase JWT, forwarded so the
   *  proxy can run the REAL server-side entitlement check + consume a use.
   *  Optional — the UX gate already routed signed-out users to sign-in. */
  run(input: { manuscriptPath: string; journal: TargetJournal; userToken?: string }): Promise<PublishReadyResult>;
  /** H4: LOCAL, best-effort. Fetch + sanitize + embed the target journal's
   *  author-guidelines page into the `journal_guideline` corpus so the pipeline's
   *  checklist can cross-reference the manuscript against the REAL guidelines. No
   *  proxy, no JWT (RAG is local); a bad page degrades honestly (structural
   *  checks only) and never blocks the run. */
  ingestGuidelines(input: { journalUrl?: string; guidelinesUrl?: string }): Promise<GuidelinesIngestResult>;
}

/** Backend `run_publishready` return shape (snake_case, as serialized by Rust).
 *  Adapted below to the frontend's PublishReadyResult (Option B: the frontend
 *  reconciles fields the backend does not produce yet). */
interface BackendReviewer {
  recommendation: Recommendation;
  publication_probability: number;
  novelty_score: number;
  /** Grounded text; '' when the gate dropped it as ungrounded (Set 4-A). */
  novelty_assessment?: string;
  journal_fit_score: number;
  /** Grounded text; '' when the gate dropped it as ungrounded (Set 4-A). */
  journal_fit_note?: string;
  body: string;
  issues: Array<{ finding_ref: string; severity: string; rationale: string }>;
  /** Gated alternatives; ungrounded/fabricated ones already dropped (Set 4-A). */
  alternatives?: Array<{ journal: string; quartile: string; reason: string; evidence_ref: string }>;
  warnings: string[];
  available: boolean;
}
interface PublishReadyOutcome {
  report: PublishReadyReport; // shape aligns 1:1 (verdict/combined_confidence/findings/checklist/debate/disclaimer)
  reviewer: BackendReviewer;
  proxy_payload: unknown; // { task, instruction, summary: { journal, findings, checklist, … } }
}

/** Map the backend outcome → the frontend result. The three fields the backend
 *  doesn't produce yet (novelty.assessment, journalFit.note, alternatives) are
 *  left EMPTY — never faked — as clean slots for Set 4-A. */
export function adaptOutcome(o: PublishReadyOutcome, journal: TargetJournal): PublishReadyResult {
  const r = o.reviewer;
  const reviewerLetter: ReviewerLetter = {
    recommendation: r.recommendation,
    publicationProbability: r.publication_probability,
    // The three Set 4-A fields — populated when the backend grounded them,
    // left EMPTY (never faked) when the gate dropped them as ungrounded.
    novelty: { score: r.novelty_score, assessment: r.novelty_assessment ?? '' },
    journalFit: {
      journal: journal.name,
      quartile: journal.quartile,
      fitScore: r.journal_fit_score,
      note: r.journal_fit_note ?? '',
    },
    alternatives: (r.alternatives ?? []).map((a) => ({
      name: a.journal,
      quartile: a.quartile,
      reason: a.reason,
    })),
    body: r.body,
    available: r.available,
    issues: (r.issues ?? []).map((i) => ({
      findingRef: i.finding_ref,
      severity: i.severity,
      rationale: i.rationale,
    })),
    warnings: r.warnings ?? [],
  };
  return { report: o.report, reviewerLetter, proxyPayload: adaptPayload(o.proxy_payload, journal) };
}

/** Re-shape the backend payload (nested under `summary`) into the frontend's
 *  flat ProxyReviewPayload for inspection. Structured fields only. */
function adaptPayload(payload: unknown, journal: TargetJournal): ProxyReviewPayload {
  const s = ((payload as { summary?: Record<string, unknown> })?.summary ?? {}) as Record<string, unknown>;
  const findings = Array.isArray(s.findings) ? s.findings : [];
  const checklist = Array.isArray(s.checklist) ? s.checklist : [];
  return {
    task: 'publishready_review',
    journal: (s.journal as { name: string; quartile: string }) ?? {
      name: journal.name,
      quartile: journal.quartile,
    },
    findings: findings.map((f) => {
      const o = f as Record<string, unknown>;
      return {
        agent: String(o.agent ?? ''),
        tier: String(o.tier ?? ''),
        severity: String(o.severity ?? ''),
        title: String(o.title ?? ''),
        confidence: Number(o.confidence ?? 0),
        evidence: Array.isArray(o.evidence) ? o.evidence.map(String) : [],
      };
    }),
    checklist: checklist.map((c) => {
      const o = c as Record<string, unknown>;
      return { requirement: String(o.requirement ?? ''), passed: Boolean(o.passed) };
    }),
  };
}

/** Production bridge: invoke `run_publishready` (path-only; the manuscript text
 *  never leaves the device) and adapt the result. Only in the Tauri desktop
 *  app — a plain browser throws a clear message the page surfaces. */
export class TauriPublishReadyBridge implements PublishReadyBridge {
  async run({ manuscriptPath, journal, userToken }: { manuscriptPath: string; journal: TargetJournal; userToken?: string }): Promise<PublishReadyResult> {
    if (!isTauri) {
      throw new Error('PublishReady runs in the Gaply desktop app.');
    }
    const { invoke } = await import('@tauri-apps/api/core');
    const outcome = (await invoke('run_publishready', {
      path: manuscriptPath,
      journalName: journal.name,
      journalQuartile: journal.quartile,
      userToken: userToken ?? null,
    })) as PublishReadyOutcome;
    return adaptOutcome(outcome, journal);
  }

  async ingestGuidelines({ journalUrl, guidelinesUrl }: { journalUrl?: string; guidelinesUrl?: string }): Promise<GuidelinesIngestResult> {
    if (!isTauri) {
      throw new Error('PublishReady runs in the Gaply desktop app.');
    }
    const { invoke } = await import('@tauri-apps/api/core');
    // camelCase → Rust snake_case (journal_url / guidelines_url), same auto-map
    // as run_publishready. Local command — no proxy, no JWT.
    return (await invoke('ingest_guidelines', {
      journalUrl: journalUrl ?? null,
      guidelinesUrl: guidelinesUrl ?? null,
    })) as GuidelinesIngestResult;
  }
}

/** Test/dev bridge: given a compiled report, derive the reviewer letter and the
 *  exact proxy payload deterministically. Records the payload so tests can prove
 *  it carries no manuscript text. */
export function makePublishReadyMock(
  report: PublishReadyReport,
  opts?: { ingestResult?: GuidelinesIngestResult }
): PublishReadyBridge & {
  lastPayload?: unknown;
  /** Recorded ingest calls + call order, so tests can prove ingest-before-run. */
  ingestCalls: Array<{ journalUrl?: string; guidelinesUrl?: string }>;
  callOrder: string[];
} {
  const ingestCalls: Array<{ journalUrl?: string; guidelinesUrl?: string }> = [];
  const callOrder: string[] = [];
  const bridge: PublishReadyBridge & {
    lastPayload?: unknown;
    ingestCalls: Array<{ journalUrl?: string; guidelinesUrl?: string }>;
    callOrder: string[];
  } = {
    ingestCalls,
    callOrder,
    async ingestGuidelines(input) {
      ingestCalls.push(input);
      callOrder.push('ingest');
      return opts?.ingestResult ?? { any_ingested: true, note: 'guidelines ingested (mock)' };
    },
    async run({ journal }) {
      callOrder.push('run');
      const proxyPayload = buildProxyPayload(report, journal);
      bridge.lastPayload = proxyPayload;
      const reviewerLetter = synthesizeReviewerLetter(report, journal);
      return { report, reviewerLetter, proxyPayload };
    },
  };
  return bridge;
}
