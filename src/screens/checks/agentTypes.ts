// Gaply — TypeScript mirrors of the three local agents' report shapes (serde-
// exact), consumed by the F7 single-agent screens. All produced by the existing
// Rust commands: check_plagiarism / detect_ai / validate_manuscript.

/* --------------- Plagiarism — deterministic exact match (Set 2/3) --------------- */
// Serde-exact mirrors of gaply-core's plagiarism_exact / plagiarism_library.
// The DETERMINISTIC lane: real verbatim overlaps you can point to in both
// places. `check_plagiarism_exact` returns ExactPlagiarismReport; the library
// commands manage the durable "my papers" comparison set.

export type MatchKind = 'self_repeat' | 'library_match';

/** A span of a document. start_char/end_char are Rust BYTE offsets — the UI
 *  renders the `text` field directly (already the exact slice) rather than
 *  indexing JS strings with them. */
export interface TextSpan {
  start_char: number;
  end_char: number;
  text: string;
}

export interface MatchedPassage {
  /** The overlapping span in the document under test (the upload). */
  source: TextSpan;
  /** The same text in the OTHER location: elsewhere in the upload
   *  (self_repeat) or in the matched library paper (library_match). */
  matched: TextSpan;
  /** Jaccard over shared shingles — 1.0 for a fully verbatim run. A computed
   *  overlap measure, never a fabricated score. */
  similarity: number;
  word_count: number;
  match_kind: MatchKind;
  /** "this document" for self_repeat, else the matched paper's title. */
  source_ref: string;
}

export interface MatchStats {
  source_fingerprints: number;
  comparison_fingerprints: number;
  candidate_seeds: number;
  library_papers_examined: number;
}

export interface ExactPlagiarismReport {
  shingle_size: number;
  window_size: number;
  self_matches: MatchedPassage[];
  library_matches: MatchedPassage[];
  /** The library papers the upload was compared against (scope, by title). */
  compared_against: string[];
  total_source_words: number;
  /** Fraction of the upload's text covered by ANY match — a real proportion,
   *  NOT a plagiarism score/verdict. */
  duplication_ratio: number;
  stats: MatchStats;
  /** REQUIRED, never empty: the exact named scope + the Turnitin limitation. */
  disclosure: string;
}

/** A paper in the durable "my papers" library (metadata only). */
export interface LibraryPaper {
  id: number;
  title: string;
  added_at: number; // epoch seconds
  source_label: string;
  /** M2 Set 2B: the OPTIONAL soft anchor → citation_library.id captured at add
   *  time (NULL/undefined when unlinked). Carried for 2C's reliable side-by-side
   *  and the id-based "full text available" badge; not yet consumed. */
  citation_id?: string;
}

/* ----------- Plagiarism — embedding "similar meaning" lane (existing) ----------- */

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

/* ------------------- AI Check (Set 5) — two-way tiered ------------------ */
// Serde-exact mirrors of gaply-core's ClassifiedAnalysis wire (run_aicheck).
// TWO-WAY by design (the Set-4 live-probe decision): passages are human-
// written vs AI-associated; `category` is always 'unclassified' in shipped
// output and the paraphrase lane reports itself unavailable. Everything is a
// SIGNAL level — the wire carries no verdicts and no probabilities.

export type PassageStrength = 'strong' | 'moderate' | 'weak';
export type AnalysisDepth = 'deep_verified' | 'heuristic_only';
export type PassageCategory =
  | 'leans_ai_generated'
  | 'leans_ai_paraphrased'
  | 'unclear_signal'
  | 'unclassified';

export interface SentenceScore {
  text: string;
  perplexity: number;
  tokens: number;
}

/** ClassifiedPassage — flattened on the wire (FlaggedPassage + tier + Set-4). */
export interface AiCheckPassage {
  section: string; // SectionKind snake_case
  start_char: number; // Rust byte offsets — do NOT index JS strings with these
  end_char: number;
  text: string;
  sentences: SentenceScore[];
  mean_perplexity: number;
  burstiness: number;
  signal: AiSignal;
  strength: PassageStrength;
  uncertainty: string; // REQUIRED, never empty
  depth: AnalysisDepth;
  depth_note: string; // REQUIRED, never empty
  category: PassageCategory;
  category_strength: PassageStrength | null;
  evidence_quote: string | null;
  gate_flags: string[];
  category_note: string; // REQUIRED, never empty
}

export interface LanguageAssessment {
  detected: string;
  english_stopword_ratio: number;
  /** FALSE downgrades the entire report's confidence (non-English input). */
  calibration_reliable: boolean;
  note: string; // REQUIRED, never empty
}

export interface AiCheckAnalysis {
  fast_model: string;
  deep_model: string | null;
  classifier_model: string | null;
  passages: AiCheckPassage[];
  total_chars: number;
  flagged_chars: number;
  /** flagged_chars / total_chars — a deterministic PROPORTION of flagged
   *  text, NOT a probability of AI authorship. */
  ai_signal_proportion: number;
  candidates_found: number;
  deep_verified: number;
  cleared_by_deep: number;
  coverage_note: string; // ALWAYS present
  classified: number;
  ai_generated_chars: number;
  ai_paraphrased_chars: number;
  classification_note: string; // ALWAYS present
  paraphrase_caution: string; // REQUIRED, never empty
  language: LanguageAssessment;
  disclaimer: string; // MANDATORY, never empty
}

/** One analyzed section, exactly as the Rust flow saw it (`paragraphs.join(" ")`). */
export interface AiCheckSection {
  kind: string;
  heading: string;
  text: string;
}

export interface AiCheckResult {
  analysis: AiCheckAnalysis;
  sections: AiCheckSection[];
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
