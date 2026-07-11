// Gaply — PublishReady (F10) types. The flagship paid output.
import { PublishReadyReport } from '../report/reportTypes';

// 'unknown' is the gate's honest downgrade (e.g. an ungrounded reject) and the
// cloud-unavailable state — the backend can return it, so the UI must too.
export type Recommendation = 'reject' | 'major_revision' | 'minor_revision' | 'accept' | 'unknown';

export interface JournalAlt {
  name: string;
  quartile: string;
}

/** A gated reviewer issue, grounded in a finding the backend actually sent. */
export interface ReviewerIssue {
  findingRef: string;
  severity: string;
  rationale: string;
}

export interface ReviewerLetter {
  recommendation: Recommendation;
  /** 0–100. */
  publicationProbability: number;
  /** `assessment` is '' when the backend doesn't produce it yet (Set 4-A). */
  novelty: { score: number; assessment: string };
  /** `note` is '' when the backend doesn't produce it yet (Set 4-A). */
  journalFit: { journal: string; quartile: string; fitScore: number; note: string };
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
}

export interface PublishReadyResult {
  report: PublishReadyReport;
  reviewerLetter: ReviewerLetter;
  /** The EXACT structured payload sent to the proxy — structured findings only,
   *  never raw manuscript text. Exposed so the UI (and tests) can inspect it. */
  proxyPayload: ProxyReviewPayload;
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

export const RECOMMENDATION_LABEL: Record<Recommendation, string> = {
  reject: 'REJECT',
  major_revision: 'MAJOR REVISION',
  minor_revision: 'MINOR REVISION',
  accept: 'ACCEPT',
  unknown: 'UNKNOWN',
};

export const RECOMMENDATION_STATUS: Record<Recommendation, 'certain' | 'assessed' | 'flagged'> = {
  reject: 'flagged',
  major_revision: 'flagged',
  minor_revision: 'assessed',
  accept: 'certain',
  unknown: 'assessed',
};
