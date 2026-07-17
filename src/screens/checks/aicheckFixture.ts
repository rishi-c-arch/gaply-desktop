// Gaply — AI Check test fixture (Set 5). Wire-shaped AiCheckResult used by the
// vitest suites only (never imported by app code). Note texts mirror the Rust
// consts' key phrases so the "verbatim, un-strippable" assertions are honest.
import { AiCheckPassage, AiCheckResult } from './agentTypes';

const DEEP_TEXT =
  'The results show that the model can do the work well. The system is fast.';
const HEUR_TEXT = 'The user can see the way to go now.';

const DISCLAIMER =
  'STATISTICAL SIGNAL ONLY — NOT proof of AI authorship. Unreliable on short texts, ' +
  'non-native English, and heavily edited writing; never sole or definitive evidence.';

const deepPassage: AiCheckPassage = {
  section: 'introduction',
  start_char: 58,
  end_char: 132,
  text: DEEP_TEXT,
  sentences: [
    { text: 'The results show that the model can do the work well.', perplexity: 4.2, tokens: 12 },
    { text: 'The system is fast.', perplexity: 4.8, tokens: 5 },
  ],
  mean_perplexity: 4.5,
  burstiness: 0.3,
  signal: 'leans_ai_like',
  strength: 'strong',
  uncertainty:
    'AI-associated SIGNAL, not a determination of authorship. High false-positive rates.',
  depth: 'deep_verified',
  depth_note:
    'Deep-verified: the full on-device model (7B) re-scored this passage, a stronger measurement than the fast pre-pass. Still a SIGNAL, not proof of AI authorship.',
  category: 'unclassified',
  category_strength: null,
  evidence_quote: null,
  gate_flags: [],
  category_note:
    'Paraphrase distinction unavailable — no supported local model currently distinguishes AI-generated from AI-paraphrased text reliably. The passage keeps its two-way AI-associated signal; no category was guessed.',
};

const heuristicPassage: AiCheckPassage = {
  ...deepPassage,
  start_char: 190,
  end_char: 225,
  text: HEUR_TEXT,
  sentences: [{ text: HEUR_TEXT, perplexity: 5.9, tokens: 9 }],
  mean_perplexity: 5.9,
  burstiness: 0,
  strength: 'weak',
  depth: 'heuristic_only',
  depth_note:
    'Heuristic-only: flagged by the fast pre-pass and NOT model-verified (analysis budget). A preliminary, lower-confidence signal — never treat as proof.',
  category_note:
    'Not classified: only deep-verified passages are sent to the classification model. This heuristic-only flag remains a preliminary signal.',
};

export const AICHECK_FIXTURE: AiCheckResult = {
  analysis: {
    fast_model: 'heuristic frequency proxy (fast pre-pass)',
    deep_model: 'candle-slm1-q3',
    classifier_model: null,
    passages: [deepPassage, heuristicPassage],
    total_chars: 260,
    flagged_chars: 109,
    ai_signal_proportion: 0.42,
    candidates_found: 3,
    deep_verified: 1,
    cleared_by_deep: 1,
    lm_perplexity: 11.5,
    lm_perplexity_signal: 'below_human_median',
    norms_provisional: true,
    document_score: {
      value: null,
      band: null,
      evidence: [
        { signal: 'Citation verification', status: 'measured', level: 'high', bias_tier: 'factual', detail: '2 of 14 references could not be verified (2 not found; 0 DOI mismatch)' },
        { signal: 'template phrasing', status: 'measured', level: 'high', bias_tier: 'structural', detail: '3.1 markers / 1000 words (≥2 distinct)' },
        { signal: 'citation density', status: 'unavailable', level: null, bias_tier: 'structural', detail: 'references present but in-text citations could not be attributed — density unreliable' },
        { signal: 'sentence-length burstiness', status: 'measured', level: 'moderate', bias_tier: 'stylometric', detail: 'CV 0.26 (human ≈ 0.43)' },
        { signal: 'Deep-verifier perplexity', status: 'measured', level: 'high', bias_tier: 'stylometric', detail: 'compact 1.5B on-device verifier re-scored 2 flagged passage(s); 2 fell below the human academic perplexity norm (the human range overlaps AI — a soft, down-weighted signal)' },
      ],
    },
    coverage_note:
      '3 candidate passage(s) from the fast pre-pass; deep-verified 1; cleared 1 as document-baseline; 1 remain heuristic-only (analysis budget: 16 passages / 2000 tokens)',
    classified: 0,
    ai_generated_chars: 0,
    ai_paraphrased_chars: 0,
    classification_note:
      '1 deep-verified passage(s); paraphrase distinction UNAVAILABLE — no supported local model distinguishes AI-generated from AI-paraphrased reliably, so passages carry the two-way signal (human-written vs AI-associated) and nothing was guessed',
    paraphrase_caution:
      'Category is a RESEMBLANCE SIGNAL, not a determination. Distinguishing AI-paraphrased from AI-generated text is EVEN LESS reliable than AI detection itself.',
    language: {
      detected: 'english',
      english_stopword_ratio: 0.38,
      calibration_reliable: true,
      note: 'Detected language: English — the calibrated thresholds apply. Non-native English writers are disproportionately over-flagged by AI detectors.',
    },
    disclaimer: DISCLAIMER,
  },
  sections: [
    {
      kind: 'introduction',
      heading: 'Introduction',
      text:
        `Her grandmother’s schematics were scrawled across napkins. ${DEEP_TEXT} ` +
        `Nobody anticipated theatrical flair from a toaster. ${HEUR_TEXT} It wheezed absurdly.`,
    },
  ],
};

/** Same report over a Spanish document — the language downgrade case. */
export const AICHECK_FIXTURE_SPANISH: AiCheckResult = {
  ...AICHECK_FIXTURE,
  analysis: {
    ...AICHECK_FIXTURE.analysis,
    language: {
      detected: 'spanish',
      english_stopword_ratio: 0.01,
      calibration_reliable: false,
      note: 'NON-ENGLISH (or unidentifiable) text detected. Every signal in this report is LOW-CONFIDENCE for this document and false positives are substantially more likely.',
    },
  },
};
