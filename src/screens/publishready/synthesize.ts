// Gaply — deterministic reviewer-letter synthesis from the compiled report +
// target journal. The RECOMMENDATION and PUBLICATION PROBABILITY are derived
// from the structured findings (so they're testable and explainable); the prose
// `body` is a template here and is cloud-synthesized (via the proxy) in
// production. Journal-fit alternatives come from the F9 directory.
import { PublishReadyReport } from '../report/reportTypes';
import { JOURNALS, Quartile } from '../journal/journalData';
import {
  JournalAlt,
  Recommendation,
  RECOMMENDATION_LABEL,
  ReviewerLetter,
  TargetJournal,
} from './publishReadyTypes';

const RANK: Record<string, number> = { Q1: 1, Q2: 2, Q3: 3, Q4: 4 };

/** Suggest same-or-higher-quartile alternatives to the target (excluding it). */
export function suggestAlternatives(target: TargetJournal, limit = 4): JournalAlt[] {
  const tr = RANK[target.quartile] ?? 4;
  const seen = new Set<string>();
  const out: JournalAlt[] = [];
  for (const j of JOURNALS) {
    if (!j.quartile || j.name === target.name) continue;
    if (RANK[j.quartile] > tr) continue; // must be same or higher quartile
    if (seen.has(j.name)) continue;
    seen.add(j.name);
    out.push({ name: j.name, quartile: j.quartile as Quartile });
    if (out.length >= limit) break;
  }
  return out;
}

export function synthesizeReviewerLetter(
  report: PublishReadyReport,
  journal: TargetJournal
): ReviewerLetter {
  const criticals = report.findings.filter((f) => f.severity === 'critical').length;
  const refuted = report.findings.filter(
    (f) => f.agent === 'verification' && /REFUTED/i.test(f.title)
  ).length;
  const flagged = report.findings.filter((f) => f.tier === 'reconsidered_after_peer_review' || f.severity === 'major').length;
  const failedChecklist = report.checklist.filter((c) => !c.passed).length;

  // recommendation + probability
  let recommendation: Recommendation;
  let prob: number;
  if (criticals > 0 || refuted > 0) {
    recommendation = criticals >= 2 || refuted >= 2 ? 'reject' : 'major_revision';
    prob = Math.max(5, 40 - criticals * 15 - refuted * 10);
  } else if (report.verdict === 'concern' || flagged >= 2) {
    recommendation = 'major_revision';
    prob = 45;
  } else if (flagged > 0 || failedChecklist > 0) {
    recommendation = 'minor_revision';
    prob = 68;
  } else {
    recommendation = 'accept';
    prob = 84;
  }

  // journal-fit tempering: a higher-quartile target is harder, so the publication
  // probability is reduced for it. This adjusts the PROBABILITY — it is no longer
  // also surfaced as a standalone "fit score" (see below).
  const tr = RANK[journal.quartile] ?? 4;
  const journalProb = Math.max(0, Math.min(100, prob - (tr === 1 ? 10 : tr === 2 ? 4 : 0)));

  const alternatives = suggestAlternatives(journal);

  const body =
    `Recommendation: ${RECOMMENDATION_LABEL[recommendation]}. ` +
    `Assessed for ${journal.name} (${journal.quartile}). ` +
    `${criticals} deterministic statistical issue(s), ${refuted} refuted citation(s), ` +
    `${failedChecklist} unmet guideline item(s). ` +
    `Estimated publication probability at this venue: ${journalProb}%. ` +
    `This assessment is model-assisted and non-definitive; deterministic (mathematically certain) ` +
    `findings must be corrected regardless of the overall recommendation.`;

  return {
    recommendation,
    publicationProbability: journalProb,
    // Novelty + fit carry NO synthesized text here. The old heuristic
    // (`55 + findings*2 - criticals*5`) produced a novelty score, and confident
    // prose off the back of it ("Appears novel relative to indexed prior
    // work."), from a formula that had never seen the manuscript's subject
    // matter — it was counting findings. Production now has no novelty or fit
    // score at all (the backend removed both as ungroundable), so a mock that
    // invented one would train the demo on behaviour the real app does not
    // have. Empty is what production returns when nothing is grounded, and the
    // panel renders '—' for it.
    novelty: { assessment: '' },
    journalFit: { journal: journal.name, quartile: journal.quartile, note: '' },
    alternatives,
    body,
  };
}
