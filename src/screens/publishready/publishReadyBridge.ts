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
  DeclinedLane,
} from './publishReadyTypes';

/** Result of ingesting the target journal's guidelines into the local
 *  `journal_guideline` corpus (H4). Wire shape from Rust `GuidelinesReport`
 *  (snake_case). Never throws for a bad page — `any_ingested: false` + an honest
 *  `note` (unreachable / quarantined / no substantive content). */
export interface GuidelinesIngestResult {
  any_ingested: boolean;
  note: string;
}

/**
 * What can be said about the reviewer letter BEFORE a run starts.
 *
 * `available: false` means we are CERTAIN it cannot work (no proxy is
 * configured on this machine). `available: true` means nothing known ahead of
 * time rules it out — NOT that it will succeed; a configured remote proxy may
 * still be down, which the run's own honest degradation covers.
 *
 * Wire shape from Rust `ReviewerLetterAvailability` (camelCase).
 */
export interface ReviewerLetterAvailability {
  available: boolean;
  /** Present exactly when `available` is false. Already phrased for a user. */
  reason: string | null;
  proxyUrl: string;
}

/** Extensions the analysis picker offers. MIRRORS
 *  `analysis_ingest::ACCEPTED_EXTENSIONS`. Three of these have no parser and are
 *  offered anyway: the count of what arrives is the measurement that decides
 *  whether to build one, and a format the picker refuses can never be counted
 *  (§11 D191). */
export const ANALYSIS_EXTENSIONS = ['sps', 'jnl', 'py', 'R', 'r', 'ipynb', 'csv'];

/** One uploaded file and what was done with it. Wire shape from Rust
 *  `AnalysisFileReport`, whose `outcome` is a THREE-variant tag — parsed,
 *  stored-but-no-parser, unreadable — because "we kept your file and did nothing
 *  with it" is a sentence that has to be sayable and a bool cannot say it. */
export interface AnalysisFileReport {
  file_name: string;
  extension: string;
  size_bytes: number;
  outcome: 'parsed' | 'stored_not_parsed' | 'rejected';
  /** Present when `outcome === 'parsed'`. */
  commands?: number;
  statistical?: number;
  unparsed_lines?: number;
  /** Present for the other two. Already phrased for a user. */
  reason?: string;
}

export interface AnalysisIngestReport {
  files: AnalysisFileReport[];
  parsed: number;
  stored_not_parsed: number;
  rejected: number;
  commands: number;
  statistical: number;
  /** The sentence the screen shows, composed in Rust so the surfaces that
   *  render it cannot word it differently. */
  summary: string;
}

export interface PublishReadyBridge {
  /** `userToken` (Set 8): the signed-in user's Supabase JWT, forwarded so the
   *  proxy can run the REAL server-side entitlement check + consume a use.
   *  Optional — the UX gate already routed signed-out users to sign-in. */
  run(input: { manuscriptPath: string; journal: TargetJournal; userToken?: string; guidelinesUrl?: string }): Promise<PublishReadyResult>;
  /** H4: LOCAL, best-effort. Fetch + sanitize + embed the target journal's
   *  author-guidelines page into the `journal_guideline` corpus so the pipeline's
   *  checklist can cross-reference the manuscript against the REAL guidelines. No
   *  proxy, no JWT (RAG is local); a bad page degrades honestly (structural
   *  checks only) and never blocks the run. */
  ingestGuidelines(input: { journalUrl?: string; guidelinesUrl?: string }): Promise<GuidelinesIngestResult>;
  /** Read the chosen analysis files and report what was done with each. LOCAL:
   *  no proxy, no network. Parses SPSS; accepts and counts everything else.
   *  **Does NOT feed the specialist cross-check** — that check fired on 29 of 31
   *  claimed tests on the one pairing available and stays unwired (§11 D191). */
  ingestAnalysis(input: { paths: string[] }): Promise<AnalysisIngestReport>;
  /** Pre-flight, instant, no network and no keychain: is the configured proxy
   *  the loopback default? Asked BEFORE a run so a user is not told the
   *  reviewer letter was unavailable only after paying for a full analysis. */
  reviewerLetterAvailability(): Promise<ReviewerLetterAvailability>;
}

/** Backend `run_publishready` return shape (snake_case, as serialized by Rust).
 *  Adapted below to the frontend's PublishReadyResult (Option B: the frontend
 *  reconciles fields the backend does not produce yet). */
interface BackendReviewer {
  recommendation: Recommendation;
  /** OMITTED by the backend when nothing computed one (unavailable / withheld). */
  publication_probability?: number;
  /** Grounded text; '' when the gate dropped it as ungrounded (Set 4-A).
   *  There is no `novelty_score`/`journal_fit_score` — the backend removed both
   *  because the payload carries nothing that could ground them. */
  novelty_assessment?: string;
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
  run_id?: string;
}

/** Map the backend outcome → the frontend result. The three fields the backend
 *  doesn't produce yet (novelty.assessment, journalFit.note, alternatives) are
 *  left EMPTY — never faked — as clean slots for Set 4-A. */
export function adaptOutcome(o: PublishReadyOutcome, journal: TargetJournal): PublishReadyResult {
  const r = o.reviewer;
  const reviewerLetter: ReviewerLetter = {
    recommendation: r.recommendation,
    publicationProbability: r.publication_probability ?? null,
    // The Set 4-A grounded fields — populated when the backend grounded them,
    // left EMPTY (never faked) when the gate dropped them as ungrounded. The
    // numeric novelty/fit scores that used to sit beside them are GONE: they
    // were unreadable by the gate and rendered as confident numbers.
    novelty: { assessment: r.novelty_assessment ?? '' },
    journalFit: {
      journal: journal.name,
      quartile: journal.quartile,
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
  return {
    report: o.report,
    reviewerLetter,
    proxyPayload: adaptPayload(o.proxy_payload, journal),
    // Needed by `export_publishready_pdf`: the Rust renderer's bytes are held
    // in session memory under this id. Dropped before, so the button had
    // nothing to ask for.
    runId: o.run_id,
    // Straight through: the backend owns these strings (gaply_core::declined).
    declined: (o as unknown as { declined?: DeclinedLane[] }).declined ?? [],
  };
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
  async run({ manuscriptPath, journal, userToken, guidelinesUrl }: { manuscriptPath: string; journal: TargetJournal; userToken?: string; guidelinesUrl?: string }): Promise<PublishReadyResult> {
    if (!isTauri) {
      throw new Error('PublishReady runs in the Gaply desktop app.');
    }
    const { invoke } = await import('@tauri-apps/api/core');
    const outcome = (await invoke('run_publishready', {
      path: manuscriptPath,
      journalName: journal.name,
      journalQuartile: journal.quartile,
      userToken: userToken ?? null,
      // The guideline document the user asked for. Passing it keeps IDENTITY
      // propagated; letting the backend re-derive "which journal" from corpus
      // state would reconstruct identity from write order.
      guidelinesUrl: guidelinesUrl ?? null,
      // The crawler's key for a PROFILED journal, so the backend can read that
      // journal's extracted requirements. `null` for a Scopus-directory pick,
      // which is every journal Gaply has not crawled. §11 D183.
      journalKey: journal.key ?? null,
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

  async ingestAnalysis({ paths }: { paths: string[] }): Promise<AnalysisIngestReport> {
    if (!isTauri) {
      throw new Error('Analysis upload runs in the Gaply desktop app.');
    }
    const { invoke } = await import('@tauri-apps/api/core');
    return (await invoke('analysis_ingest', { paths })) as AnalysisIngestReport;
  }

  async reviewerLetterAvailability(): Promise<ReviewerLetterAvailability> {
    if (!isTauri) {
      // The web build has no proxy of its own; saying "unavailable" here is the
      // honest answer and matches what a run would do.
      return {
        available: false,
        reason: 'The reviewer letter is available in the Gaply desktop app.',
        proxyUrl: '',
      };
    }
    const { invoke } = await import('@tauri-apps/api/core');
    return (await invoke('reviewer_letter_availability')) as ReviewerLetterAvailability;
  }
}

/** Test/dev bridge: given a compiled report, derive the reviewer letter and the
 *  exact proxy payload deterministically. Records the payload so tests can prove
 *  it carries no manuscript text. */
export function makePublishReadyMock(
  report: PublishReadyReport,
  opts?: { ingestResult?: GuidelinesIngestResult; availability?: ReviewerLetterAvailability }
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
    async ingestAnalysis({ paths }: { paths: string[] }) {
      return {
        files: [], parsed: 0, stored_not_parsed: 0, rejected: 0, commands: 0, statistical: 0,
        summary: `${paths.length} file(s) (mock).`,
      };
    },
    async reviewerLetterAvailability() {
      // Deliberately NOT recorded in `callOrder`. That array exists to prove
      // ingest-happens-before-run; this pre-flight is unordered with respect to
      // both, and pushing it turned every `callOrder` assertion into a test of
      // when React happens to fire an effect.
      return (
        opts?.availability ?? { available: true, reason: null, proxyUrl: 'https://proxy.test' }
      );
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
