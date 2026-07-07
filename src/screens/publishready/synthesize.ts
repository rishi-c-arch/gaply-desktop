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

  // journal-fit: a higher-quartile target is harder → temper the probability
  const tr = RANK[journal.quartile] ?? 4;
  const fitScore = Math.max(0, Math.min(100, prob - (tr === 1 ? 10 : tr === 2 ? 4 : 0)));
  const journalProb = fitScore;

  // novelty (cloud-assessed in prod; here a stable heuristic placeholder)
  const noveltyScore = Math.round(55 + Math.min(30, report.findings.length * 2) - criticals * 5);

  const alternatives = suggestAlternatives(journal);

  const body =
    `Recommendation: ${RECOMMENDATION_LABEL[recommendation]}. ` +
    `Assessed for ${journal.name} (${journal.quartile}). ` +
    `${criticals} deterministic statistical issue(s), ${refuted} refuted citation(s), ` +
    `${failedChecklist} unmet guideline item(s). ` +
    `Estimated publication probability at this venue: ${journalProb}%. ` +
    `Novelty vs. recent literature: ${noveltyScore}/100. ` +
    `This assessment is model-assisted and non-definitive; deterministic (mathematically certain) ` +
    `findings must be corrected regardless of the overall recommendation.`;

  return {
    recommendation,
    publicationProbability: journalProb,
    novelty: {
      score: noveltyScore,
      assessment:
        noveltyScore >= 70
          ? 'Appears novel relative to indexed prior work.'
          : noveltyScore >= 50
          ? 'Moderate novelty; position the contribution clearly vs. recent work.'
          : 'Limited apparent novelty; strengthen the gap statement.',
    },
    journalFit: {
      journal: journal.name,
      quartile: journal.quartile,
      fitScore,
      note:
        fitScore >= 65
          ? `Competitive for this ${journal.quartile} venue.`
          : `A stretch for ${journal.quartile}; consider the alternatives below.`,
    },
    alternatives,
    body,
  };
}
