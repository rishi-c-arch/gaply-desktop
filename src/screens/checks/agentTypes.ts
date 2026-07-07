// Gaply — TypeScript mirrors of the three local agents' report shapes (serde-
// exact), consumed by the F7 single-agent screens. All produced by the existing
// Rust commands: check_plagiarism / detect_ai / validate_manuscript.

/* ----------------------------- Plagiarism ------------------------------- */

export type MatchSource =
  | { kind: 'corpus'; document_id: number; chunk_id: number; title: string; source_url: string; source_type: string; excerpt: string }
  | { kind: 'self_manuscript'; other_chunk_seq: number; excerpt: string };

export interface MatchSpan {
  manuscript_chunk_seq: number;
  manuscript_excerpt: string;
  similarity: number; // cosine [0,1]
  source: MatchSource;
}

export interface PlagiarismReport {
  chunk_count: number;
  threshold: number;
  corpus_matches: MatchSpan[];
  self_matches: MatchSpan[];
  note: string;
}

/* ----------------------------- AI Detection ----------------------------- */

export type AiSignal = 'leans_ai_like' | 'inconclusive' | 'leans_human_like';

export interface SectionAiScore {
  section: string; // SectionKind snake_case
  sentence_count: number;
  mean_perplexity: number;
  burstiness: number;
  signal: AiSignal;
  uncertainty: string;
}

export interface AiDetectionReport {
  model: string;
  overall_mean_perplexity: number;
  overall_burstiness: number;
  signal: AiSignal;
  sections: SectionAiScore[];
  confidence: string; // "low"
  disclaimer: string; // MANDATORY, never empty
}

/* --------------------------- Validation / Maths ------------------------- */

export type RuleId =
  | 'test_group_mismatch'
  | 'p_value_overclaim'
  | 'missing_effect_size'
  | 'missing_confidence_interval'
  | 'small_sample_causal_claim';

export type Severity = 'CRITICAL' | 'MAJOR';

export interface StatFlag {
  rule: RuleId;
  severity: Severity;
  location: { section: string; paragraph: number };
  explanation: string;
}

export interface RuleOutcome {
  rule: RuleId;
  passed: boolean;
  flags: number;
}

export interface StatsValidityReport {
  passed: boolean;
  checks: RuleOutcome[];
  flags: StatFlag[];
}
