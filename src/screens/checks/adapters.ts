// Gaply — convert each local agent's raw report into a PublishReadyReport so
// the F6 viewer renders it scoped to that one agent. Certainty tiers are honest:
//   Validation/Maths  -> mathematically_certain (🟢, deterministic)
//   AI Detection      -> ai_assessed_moderate   (🟡, statistical signal only)
//   Plagiarism        -> ai_assessed_moderate   (🟡, embedding similarity)
import {
  Finding,
  FindingSeverity,
  PublishReadyReport,
} from '../report/reportTypes';
import {
  AiDetectionReport,
  MatchSpan,
  PlagiarismReport,
  StatsValidityReport,
} from './agentTypes';

const EMPTY_DEBATE = {
  rounds_run: 0,
  converged: true,
  overridden_by_constraint: false,
  rejected_agents: [] as never[],
  revised_agents: [] as never[],
};

/* ------------------------------ Plagiarism ------------------------------ */

// MIRROR of `match_type_label` in gaply-core/src/report.rs — the two must stay
// byte-identical; both sides are pinned by tests.
//
// WHY THESE WORDS — do not "improve" them back. `m.similarity` is cosine over
// HashEmbedder (gaply-core/src/embed.rs:33-53), a feature-hashing BAG-OF-WORDS
// encoder and the only Embedder in the tree. It measures WORD OVERLAP: it is
// word-order-blind (so "verbatim" was never verifiable) and has no semantic
// capability (so synonyms are invisible). "paraphrase" named the inverse of
// what the engine detects — a genuine paraphrase has LOW word overlap, falls
// below DEFAULT_THRESHOLD (0.80), and is never reported at all.
function matchTypeLabel(m: MatchSpan): string {
  // "(same manuscript)", not "(self-plagiarism)": the source enum establishes WHERE
  // the match is, never that the reuse was illegitimate — a determination the
  // isolation note explicitly disclaims.
  if (m.source.kind === 'self_manuscript') return 'internal duplication (same manuscript)';
  if (m.similarity >= 0.98) return 'near-identical wording';
  if (m.similarity >= 0.85) return 'high word overlap';
  return 'partial lexical overlap';
}

export function plagiarismToReport(r: PlagiarismReport): PublishReadyReport {
  const spans = [...r.self_matches, ...r.corpus_matches];
  const peak = spans.reduce((mx, m) => Math.max(mx, m.similarity), 0);
  const findings: Finding[] = spans.map((m) => {
    const severity: FindingSeverity = m.similarity >= r.threshold ? 'major' : 'minor';
    const sourceLabel =
      m.source.kind === 'corpus'
        ? `corpus: ${m.source.title}`
        : `self · chunk ${m.source.other_chunk_seq}`;
    return {
      severity,
      tier: 'ai_assessed_moderate',
      certainty_label: 'AI-assessed, moderate confidence',
      agent: 'plagiarism',
      // "word overlap", not "similarity" — see matchTypeLabel. Mirrors report.rs.
      title: `${matchTypeLabel(m)} — ${(m.similarity * 100).toFixed(0)}% word overlap`,
      detail: `“${m.manuscript_excerpt}” matches ${sourceLabel}.`,
      confidence: m.similarity,
      provenance: [
        `similarity:${m.similarity.toFixed(3)}`,
        `match_type:${matchTypeLabel(m)}`,
        `source:${m.source.kind}`,
        'store:per-session isolated (shared corpus read-only)',
      ],
      section: `chunk ${m.manuscript_chunk_seq}`,
    };
  });
  if (findings.length === 0) {
    findings.push({
      severity: 'info',
      tier: 'ai_assessed_moderate',
      certainty_label: 'AI-assessed, moderate confidence',
      agent: 'plagiarism',
      title: 'No local similarity matches',
      detail: r.note,
      confidence: 0.7,
      provenance: [`chunks:${r.chunk_count}`, `threshold:${r.threshold}`, 'store:per-session isolated'],
    });
  }
  return {
    verdict: peak >= r.threshold ? 'concern' : 'pass',
    combined_confidence: peak,
    findings,
    checklist: [],
    debate: EMPTY_DEBATE,
    disclaimer:
      'Local similarity is computed on-device against your own session and the shared corpus only. It is an indicator, not proof — broader deep plagiarism analysis is a separate capability (coming soon).',
  };
}

/* ------------------------------ AI Detection ---------------------------- */

const AI_RISK: Record<string, number> = { leans_ai_like: 0.7, inconclusive: 0.45, leans_human_like: 0.2 };

export function aiToReport(r: AiDetectionReport): PublishReadyReport {
  const findings: Finding[] = r.sections.map((s) => {
    const risk = AI_RISK[s.signal] ?? 0.45;
    const severity: FindingSeverity = s.signal === 'leans_ai_like' ? 'major' : 'info';
    return {
      severity,
      tier: 'ai_assessed_moderate',
      certainty_label: 'AI-assessed, moderate confidence',
      agent: 'ai_detection',
      title: `${s.section}: ${s.signal.replace(/_/g, ' ')} (${Math.round(risk * 100)}% risk)`,
      detail: `mean perplexity ${s.mean_perplexity.toFixed(1)}, burstiness ${s.burstiness.toFixed(1)}. ${s.uncertainty}`,
      confidence: risk,
      // provenance ALWAYS includes the mandatory uncertainty note
      provenance: [
        `signal:${s.signal}`,
        `mean_perplexity:${s.mean_perplexity.toFixed(2)}`,
        `burstiness:${s.burstiness.toFixed(2)}`,
        `sentences:${s.sentence_count}`,
        'model:HeuristicModel (interim; swappable PerplexityModel seam)',
      ],
      section: s.section,
    };
  });
  return {
    verdict: r.signal === 'leans_ai_like' ? 'concern' : 'pass',
    combined_confidence: AI_RISK[r.signal] ?? 0.45,
    findings,
    checklist: [],
    debate: EMPTY_DEBATE,
    // r.disclaimer is the Rust agent's MANDATORY disclaimer — surfaced verbatim.
    disclaimer: r.disclaimer,
  };
}

/* --------------------------- Validation / Maths ------------------------- */

export function validationToReport(r: StatsValidityReport): PublishReadyReport {
  const findings: Finding[] = r.flags.map((f) => ({
    severity: f.severity === 'CRITICAL' ? 'critical' : 'major',
    tier: 'mathematically_certain',
    certainty_label: 'mathematically certain',
    agent: 'validation_maths',
    title: `rule failed: ${f.rule.replace(/_/g, ' ')}`,
    detail: f.explanation,
    confidence: 1.0,
    provenance: [
      `rule:${f.rule} (${f.severity})`,
      // 1-BASED, matching every other surface a reader sees (gaply_core's
      // `report_model::human_paragraph`). This mirror showed "paragraph 0"
      // while the exported PDF showed "paragraph 1" for the same finding.
      `location:${f.location.section} paragraph ${f.location.paragraph + 1}`,
      'agent:validation_maths (deterministic)',
    ],
    section: f.location.section,
  }));
  if (findings.length === 0) {
    findings.push({
      severity: 'info',
      tier: 'mathematically_certain',
      certainty_label: 'mathematically certain',
      agent: 'validation_maths',
      title: 'All deterministic statistical rules passed',
      detail: `${r.checks.length} rules evaluated; none fired.`,
      confidence: 1.0,
      provenance: ['agent:validation_maths (deterministic)'],
    });
  }
  return {
    verdict: r.passed ? 'pass' : 'concern',
    combined_confidence: 1.0,
    findings,
    checklist: [],
    debate: { ...EMPTY_DEBATE, overridden_by_constraint: !r.passed },
    disclaimer:
      'These are deterministic, mathematically-certain checks of statistical reporting — not probabilistic estimates. Each finding requires correction, not interpretation.',
  };
}
