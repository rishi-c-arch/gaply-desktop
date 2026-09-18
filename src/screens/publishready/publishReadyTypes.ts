// Gaply — PublishReady (F10) types. The flagship paid output.
import { PublishReadyReport } from '../report/reportTypes';

// 'unknown' is the gate's honest downgrade (e.g. an ungrounded reject) and the
// cloud-unavailable state — the backend can return it, so the UI must too.
export type Recommendation = 'reject' | 'major_revision' | 'minor_revision' | 'accept' | 'unknown';

export interface JournalAlt {
  name: string;
  quartile: string;
  /** The grounded reason this venue was suggested (advisory, from a finding). */
  reason?: string;
}

/** A gated reviewer issue, grounded in a finding the backend actually sent. */
export interface ReviewerIssue {
  findingRef: string;
  severity: string;
  rationale: string;
}

export interface ReviewerLetter {
  recommendation: Recommendation;
  /** 0–100. Weakly grounded — derived from findings/checklist the payload
   *  actually carries (unlike the removed novelty/fit scores). */
  /** ABSENT when no probability was computed — the cloud reviewer was
   *  unavailable, or the deterministic verdict was withheld. The backend omits
   *  the key entirely rather than sending 0, so this is structurally absent
   *  rather than a sentinel. */
  publicationProbability: number | null;
  /** `assessment` is '' whenever the backend gate could not ground it.
   *  There is deliberately NO numeric novelty score: the proxy payload carries
   *  no topic or subject matter, so nothing could ground one. A score must be
   *  the CONCLUSION of a real evidence pipeline (Evidence → Agent → Reviewer →
   *  Score), never an LLM's opening guess rendered as a ring. */
  novelty: { assessment: string };
  /** `note` is '' whenever the backend gate could not ground it. No `fitScore`
   *  for the same reason as `novelty` — and because ≥65 used to render a
   *  "certain" badge on a number with zero supporting evidence. */
  journalFit: { journal: string; quartile: string; note: string };
  /** Same-or-higher-quartile options; [] until the backend produces them. */
  alternatives: JournalAlt[];
  /** Synthesized reviewer prose (cloud-generated in production). */
  body: string;
  /** False = deep reasoning unavailable offline (proxy not live) — render the
   *  honest state, never a faked letter. Optional so the mock bridge (no field)
   *  reads as available. */
  available?: boolean;
  /** Gate-approved issues (hallucinated finding_refs already dropped). */
  issues?: ReviewerIssue[];
  /** Gate warnings (dropped hallucinations, downgrades) — shown for honesty. */
  warnings?: string[];
}

export interface TargetJournal {
  name: string;
  quartile: string; // 'Q1'..'Q4'
  /** **The crawler's key, present ONLY for a profiled journal (§11 D183).**
   *
   *  The bundled Scopus directory is keyed by title and carries no key; the ten
   *  journals Gaply has crawled are keyed in `config/journal-crawl.json`. When
   *  this is set the backend reads `journal_requirements` for that key and the
   *  checklist becomes the journal's own word limits, abstract limits and
   *  required statements. When it is absent the checklist is the four structural
   *  rows, which is what every journal gave before.
   *
   *  It is PASSED, never re-derived: only 4 of the 10 crawler names match a
   *  Scopus title exactly, so matching by name would wire four journals and fail
   *  silently for six — the failure mode `JOURNALS.every(x => !('guidelinesUrl'
   *  in x))` exists to prevent. */
  key?: string;
}

export interface PublishReadyResult {
  report: PublishReadyReport;
  reviewerLetter: ReviewerLetter;
  /** The EXACT structured payload sent to the proxy — structured findings only,
   *  never raw manuscript text. Exposed so the UI (and tests) can inspect it. */
  proxyPayload: ProxyReviewPayload;
  /** Identity of the run. The Rust-rendered PDF is held in session memory under
   *  this id; `export_publishready_pdf` fetches it. Optional because the
   *  dev/mock bridge derives a result without running the pipeline. */
  runId?: string;
  /** What Gaply deliberately does not do, and why — from
   *  `gaply_core::declined::DECLINED_LANES`. Rendered in place of the em-dash
   *  that used to stand where a declined lane's output would have been. */
  declined?: DeclinedLane[];
}

/** One lane Gaply does not run. Constant per build; see the Rust module. */
export interface DeclinedLane {
  lane: string;
  reason: string;
  record: string;
}

/** Structured-summary payload — the ONLY thing that leaves the device. */
export interface ProxyReviewPayload {
  task: 'publishready_review';
  journal: { name: string; quartile: string };
  findings: Array<{
    agent: string;
    tier: string;
    severity: string;
    /** A STRUCTURED finding title (e.g. "citation c1 REFUTED") — never an
     *  excerpt of the manuscript. */
    title: string;
    confidence: number;
    /** Structured provenance only: rule ids, evidence refs, weights. */
    evidence: string[];
  }>;
  checklist: Array<{ requirement: string; passed: boolean }>;
}

/** Mirrors `gaply_core::vocabulary::recommendation_label`, pinned by
 *  `vocabulary.vitest.ts` against the generated artifact.
 *
 *  SENTENCE CASE: caps are PRESENTATION, not vocabulary. `'MAJOR REVISION'` is
 *  unusable mid-sentence and the PDF needs it in one; a header wanting uppercase
 *  applies `text-transform` in CSS. */
export const RECOMMENDATION_LABEL: Record<Recommendation, string> = {
  reject: 'Reject',
  major_revision: 'Major revision',
  minor_revision: 'Minor revision',
  accept: 'Accept',
  unknown: 'Not determined',
};

export const RECOMMENDATION_STATUS: Record<Recommendation, 'certain' | 'assessed' | 'flagged'> = {
  reject: 'flagged',
  major_revision: 'flagged',
  minor_revision: 'assessed',
  accept: 'certain',
  unknown: 'assessed',
};
