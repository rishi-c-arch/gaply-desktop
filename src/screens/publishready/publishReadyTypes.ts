// Gaply — PublishReady (F10) types. The flagship paid output.
import { PublishReadyReport } from '../report/reportTypes';

export type Recommendation = 'reject' | 'major_revision' | 'minor_revision' | 'accept';

export interface JournalAlt {
  name: string;
  quartile: string;
}

export interface ReviewerLetter {
  recommendation: Recommendation;
  /** 0–100. */
  publicationProbability: number;
  novelty: { score: number; assessment: string };
  journalFit: { journal: string; quartile: string; fitScore: number; note: string };
  /** Same-or-higher-quartile options. */
  alternatives: JournalAlt[];
  /** Synthesized reviewer prose (cloud-generated in production). */
  body: string;
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
};

export const RECOMMENDATION_STATUS: Record<Recommendation, 'certain' | 'assessed' | 'flagged'> = {
  reject: 'flagged',
  major_revision: 'flagged',
  minor_revision: 'assessed',
  accept: 'certain',
};
