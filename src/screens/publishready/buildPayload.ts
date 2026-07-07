// Gaply — PublishReady privacy core. Builds the proxy payload from STRUCTURED
// FINDINGS ONLY. The manuscript never leaves the device: this reuses the
// backend's UntrustedText/llm_safe + structured-summary discipline — a finding's
// free-text `detail` (which may quote a manuscript excerpt, e.g. a plagiarism
// span) is DELIBERATELY EXCLUDED; only agent/tier/severity, the structured
// finding title, confidence, and structured provenance (rule ids / evidence
// refs / weights) are sent. A test asserts a manuscript sentinel never appears
// in the payload.
import { PublishReadyReport } from '../report/reportTypes';
import { ProxyReviewPayload, TargetJournal } from './publishReadyTypes';

/** Structured provenance items we allow through — never a raw excerpt. */
function structuredProvenance(provenance: string[]): string[] {
  return provenance.filter((p) =>
    /^(rule:|evidence:|swarm:|agent:|gate:|similarity:|match_type:|source:|signal:)/.test(p)
  );
}

export function buildProxyPayload(report: PublishReadyReport, journal: TargetJournal): ProxyReviewPayload {
  return {
    task: 'publishready_review',
    journal: { name: journal.name, quartile: journal.quartile },
    findings: report.findings.map((f) => ({
      agent: f.agent,
      tier: f.tier,
      severity: f.severity,
      title: f.title, // structured summary, not manuscript text
      confidence: f.confidence,
      evidence: structuredProvenance(f.provenance),
      // NOTE: f.detail is intentionally NOT included — it may contain excerpts.
    })),
    checklist: report.checklist.map((c) => ({ requirement: c.requirement, passed: c.passed })),
  };
}
