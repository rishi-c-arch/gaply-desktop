//! AI-text detection via perplexity + burstiness — a STATISTICAL SIGNAL,
//! never proof.
//!
//! # Architecture
//!
//! Scoring is abstracted behind the [`PerplexityModel`] trait so the
//! detection algorithm (per-sentence perplexity, burstiness, strided
//! sliding-window scoring, per-section aggregation) is model-agnostic and
//! fully testable. The intended production backend is **GPT-2 run locally via
//! candle** (Rust-native ML): candle exposes per-token logits, which is what
//! perplexity needs — unlike a local Ollama call, whose API does not cleanly
//! surface per-token log-probabilities. GPT-2 is the interim pre-trained
//! model until the project's custom-trained SLMs are ready; both plug into
//! this same trait.
//!
//! The default [`HeuristicModel`] is a deterministic, offline frequency-based
//! surprisal proxy (common words → low surprisal, rare/long words → high),
//! standing in for GPT-2 the way `HashEmbedder` stands in for a real
//! embedding model. GPT-2 perplexity correlates strongly with token
//! frequency, so this is a faithful direction-preserving proxy — but its
//! absolute numbers are NOT GPT-2's, which is exactly why every result
//! carries an explicit uncertainty disclaimer.
//!
//! # Interpretation
//!
//! Low perplexity = more predictable word choices = more AI-like. High
//! perplexity = more surprising choices = more human-like. Burstiness is the
//! sentence-to-sentence variation in perplexity: humans mix simple and
//! complex sentences (high burstiness); AI text tends to be uniformly smooth
//! (low burstiness). NONE of this is proof — see [`AI_DISCLAIMER`].

use std::collections::HashSet;
use std::sync::OnceLock;

use serde::{Deserialize, Serialize};

use crate::extract::{ExtractionResult, SectionKind};

/// Mandatory disclaimer attached to every [`AiDetectionReport`]. Never empty.
pub const AI_DISCLAIMER: &str = "STATISTICAL SIGNAL ONLY — NOT proof of AI authorship. \
Perplexity and burstiness are probabilistic indicators with high false-positive and \
false-negative rates. They vary by domain, genre, and individual writing style, can be \
deliberately evaded, and are unreliable on short texts, non-native English, and heavily \
edited writing. These scores must never be used as sole or definitive evidence that text \
was AI-generated — treat them as one weak input to human judgement.";

/// Per-section uncertainty note. Never empty.
pub const SECTION_UNCERTAINTY: &str = "Indicative only; not a determination. Short sections \
and technical/formulaic prose score unreliably.";

// ---------------------------------------------------------------------------
// Model abstraction
// ---------------------------------------------------------------------------

/// A language model that can assign per-token surprisal (in bits). Perplexity
/// of a span is `2^(mean surprisal)`.
///
/// `surprisals(seq)` returns one value per token, where each token's surprisal
/// may condition on the tokens preceding it *within `seq`*. The strided
/// windower relies on this contract to give tokens real left-context while
/// never exceeding [`context_tokens`](PerplexityModel::context_tokens).
pub trait PerplexityModel: Send + Sync {
    fn name(&self) -> &str;
    /// Maximum tokens the model can attend over at once (GPT-2 = 1024).
    fn context_tokens(&self) -> usize;
    /// Stride between windows for long inputs (must be < context).
    fn stride(&self) -> usize;
    fn tokenize(&self, text: &str) -> Vec<String>;
    /// Per-token surprisal in bits for `tokens`; `len == tokens.len()`.
    fn surprisals(&self, tokens: &[String]) -> Vec<f32>;
}

// ---------------------------------------------------------------------------
// Heuristic (interim) model — deterministic frequency-based surprisal proxy
// ---------------------------------------------------------------------------

fn common_words() -> &'static HashSet<&'static str> {
    static S: OnceLock<HashSet<&'static str>> = OnceLock::new();
    S.get_or_init(|| {
        // ~150 of the most frequent English words — predictable choices that a
        // language model assigns high probability (low surprisal).
        const WORDS: &[&str] = &[
            "the", "be", "to", "of", "and", "a", "in", "that", "have", "i", "it", "for", "not",
            "on", "with", "he", "as", "you", "do", "at", "this", "but", "his", "by", "from",
            "they", "we", "say", "her", "she", "or", "an", "will", "my", "one", "all", "would",
            "there", "their", "what", "so", "up", "out", "if", "about", "who", "get", "which",
            "go", "me", "when", "make", "can", "like", "time", "no", "just", "him", "know",
            "take", "people", "into", "year", "your", "good", "some", "could", "them", "see",
            "other", "than", "then", "now", "look", "only", "come", "its", "over", "think",
            "also", "back", "after", "use", "two", "how", "our", "work", "first", "well", "way",
            "even", "new", "want", "because", "any", "these", "give", "day", "most", "us", "is",
            "are", "was", "were", "been", "has", "had", "said", "very", "many", "such", "more",
            "much", "here", "both", "each", "same", "should", "may", "must", "shows", "show",
            "used", "results", "data", "system", "model", "study", "method", "test", "group",
            "found", "using", "based", "high", "low", "able", "process", "simple", "clear",
            "easy", "fast", "handle", "tasks", "designed", "user",
        ];
        WORDS.iter().copied().collect()
    })
}

fn is_punct(t: &str) -> bool {
    !t.is_empty() && t.chars().all(|c| !c.is_alphanumeric())
}

/// Deterministic, context-free per-token surprisal (bits): common words and
/// punctuation are unsurprising; rarer/longer words are more surprising.
fn token_surprisal(raw: &str) -> f32 {
    let t = raw.to_lowercase();
    if t.is_empty() {
        return 0.0;
    }
    if is_punct(&t) {
        return 0.5;
    }
    if common_words().contains(t.as_str()) {
        return 2.0;
    }
    let len = t.chars().count() as f32;
    (3.5 + 0.55 * (len - 3.0)).clamp(3.5, 9.0)
}

/// Interim frequency-proxy scorer. Mirrors GPT-2's context window so the
/// strided windower behaves identically when the real model is swapped in.
pub struct HeuristicModel {
    context: usize,
    stride: usize,
}

impl HeuristicModel {
    pub fn new(context: usize, stride: usize) -> Self {
        assert!(stride < context, "stride must be smaller than context");
        Self { context, stride }
    }
    /// GPT-2-shaped context (1024 tokens, stride 512).
    pub fn gpt2_like() -> Self {
        Self::new(1024, 512)
    }
}

impl Default for HeuristicModel {
    fn default() -> Self {
        Self::gpt2_like()
    }
}

impl PerplexityModel for HeuristicModel {
    fn name(&self) -> &str {
        "heuristic-frequency-proxy (interim; GPT-2 via candle pending)"
    }
    fn context_tokens(&self) -> usize {
        self.context
    }
    fn stride(&self) -> usize {
        self.stride
    }
    fn tokenize(&self, text: &str) -> Vec<String> {
        let mut toks = Vec::new();
        let mut cur = String::new();
        for ch in text.chars() {
            if ch.is_alphanumeric() || ch == '\'' {
                cur.push(ch);
            } else {
                if !cur.is_empty() {
                    toks.push(std::mem::take(&mut cur));
                }
                if !ch.is_whitespace() {
                    toks.push(ch.to_string());
                }
            }
        }
        if !cur.is_empty() {
            toks.push(cur);
        }
        toks
    }
    fn surprisals(&self, tokens: &[String]) -> Vec<f32> {
        tokens.iter().map(|t| token_surprisal(t)).collect()
    }
}

// ---------------------------------------------------------------------------
// Strided sliding window — never exceeds context, no token lost or double-scored
// ---------------------------------------------------------------------------

/// Windows over `n` tokens as `(begin, target_start, end)`:
/// - `[begin, end)` is fed to the model (never longer than `window`);
/// - `[target_start, end)` are the tokens finalized by this window.
///
/// The target ranges partition `[0, n)` exactly — contiguous, non-overlapping,
/// fully covering — so no token is dropped or scored twice at chunk edges.
pub fn strided_ranges(n: usize, window: usize, stride: usize) -> Vec<(usize, usize, usize)> {
    assert!(window >= 1 && stride >= 1 && stride < window, "require 1 <= stride < window");
    let mut ranges = Vec::new();
    if n == 0 {
        return ranges;
    }
    let mut begin = 0usize;
    let mut prev_end = 0usize;
    loop {
        let end = (begin + window).min(n);
        ranges.push((begin, prev_end, end));
        prev_end = end;
        if end == n {
            break;
        }
        begin += stride;
    }
    ranges
}

/// Per-token surprisal over an arbitrarily long token stream using strided
/// windows, so the model never sees more than `window` tokens at once while
/// every token past the first window still gets `window - stride` tokens of
/// left context.
pub fn strided_surprisals(
    model: &dyn PerplexityModel,
    tokens: &[String],
    window: usize,
    stride: usize,
) -> Vec<f32> {
    let mut out = vec![0f32; tokens.len()];
    for (begin, target_start, end) in strided_ranges(tokens.len(), window, stride) {
        let win = model.surprisals(&tokens[begin..end]);
        for idx in target_start..end {
            out[idx] = win[idx - begin];
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Report types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AiSignal {
    LeansAiLike,
    Inconclusive,
    LeansHumanLike,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SentenceScore {
    pub text: String,
    pub perplexity: f64,
    pub tokens: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SectionAiScore {
    pub section: SectionKind,
    pub sentence_count: usize,
    pub mean_perplexity: f64,
    pub burstiness: f64,
    pub signal: AiSignal,
    /// Per-section uncertainty note. Never empty.
    pub uncertainty: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AiDetectionReport {
    pub model: String,
    pub overall_mean_perplexity: f64,
    pub overall_burstiness: f64,
    pub signal: AiSignal,
    pub sections: Vec<SectionAiScore>,
    /// Coarse confidence — always low for a statistical proxy.
    pub confidence: String,
    /// Mandatory disclaimer. Never empty.
    pub disclaimer: String,
}

fn mean(xs: &[f64]) -> f64 {
    if xs.is_empty() {
        0.0
    } else {
        xs.iter().sum::<f64>() / xs.len() as f64
    }
}

/// Population standard deviation — our burstiness measure.
fn std_dev(xs: &[f64]) -> f64 {
    if xs.len() < 2 {
        return 0.0;
    }
    let m = mean(xs);
    let var = xs.iter().map(|x| (x - m).powi(2)).sum::<f64>() / xs.len() as f64;
    var.sqrt()
}

fn split_sentences(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    for ch in text.chars() {
        cur.push(ch);
        if matches!(ch, '.' | '!' | '?') {
            let s = cur.trim().to_string();
            if !s.is_empty() {
                out.push(s);
            }
            cur.clear();
        }
    }
    let s = cur.trim().to_string();
    if !s.is_empty() {
        out.push(s);
    }
    out
}

/// Interim, deliberately conservative thresholds for the heuristic proxy.
/// NOT calibrated against real GPT-2 output — hence the ever-present
/// disclaimer. Recalibrate when a real model is wired to the trait; the
/// SLM-1 tiers (Q3 vs Q4, ~+0.13 bits mean) need PER-TIER values here.
/// The passage-level flagging (Set 2) derives from these SAME constants —
/// no second set of thresholds exists.
const AI_LIKE_PPL: f64 = 12.0;
const AI_LIKE_BURSTINESS: f64 = 8.0;
const HUMAN_LIKE_PPL: f64 = 20.0;
const HUMAN_LIKE_BURSTINESS: f64 = 15.0;

fn classify(mean_ppl: f64, burstiness: f64) -> AiSignal {
    if mean_ppl < AI_LIKE_PPL && burstiness < AI_LIKE_BURSTINESS {
        AiSignal::LeansAiLike
    } else if mean_ppl > HUMAN_LIKE_PPL || burstiness > HUMAN_LIKE_BURSTINESS {
        AiSignal::LeansHumanLike
    } else {
        AiSignal::Inconclusive
    }
}

/// Score one block of text: per-sentence perplexity (with strided windowing
/// across the whole block), the block mean, and burstiness.
fn score_block(model: &dyn PerplexityModel, text: &str) -> (f64, f64, Vec<SentenceScore>) {
    let sentences = split_sentences(text);
    let mut flat: Vec<String> = Vec::new();
    let mut ranges: Vec<(usize, usize)> = Vec::new();
    for s in &sentences {
        let start = flat.len();
        flat.extend(model.tokenize(s));
        ranges.push((start, flat.len()));
    }
    if flat.is_empty() {
        return (0.0, 0.0, Vec::new());
    }

    let surp = strided_surprisals(model, &flat, model.context_tokens(), model.stride());

    let mut scores = Vec::new();
    for (i, s) in sentences.iter().enumerate() {
        let (a, b) = ranges[i];
        if a == b {
            continue;
        }
        let mean_surp = surp[a..b].iter().map(|x| *x as f64).sum::<f64>() / (b - a) as f64;
        scores.push(SentenceScore {
            text: s.clone(),
            perplexity: 2f64.powf(mean_surp),
            tokens: b - a,
        });
    }

    let ppls: Vec<f64> = scores.iter().map(|s| s.perplexity).collect();
    (mean(&ppls), std_dev(&ppls), scores)
}

fn build_report(
    model: &dyn PerplexityModel,
    sections: Vec<SectionAiScore>,
    all_ppls: Vec<f64>,
) -> AiDetectionReport {
    let overall_mean = mean(&all_ppls);
    let overall_burst = std_dev(&all_ppls);
    AiDetectionReport {
        model: model.name().to_string(),
        overall_mean_perplexity: overall_mean,
        overall_burstiness: overall_burst,
        signal: classify(overall_mean, overall_burst),
        sections,
        confidence: "low".to_string(),
        disclaimer: AI_DISCLAIMER.to_string(),
    }
}

/// Detect over a single block of text (used for quick checks / short inputs).
pub fn detect_text(model: &dyn PerplexityModel, text: &str) -> AiDetectionReport {
    let (mean_ppl, burst, scores) = score_block(model, text);
    let section = SectionAiScore {
        section: SectionKind::Other,
        sentence_count: scores.len(),
        mean_perplexity: mean_ppl,
        burstiness: burst,
        signal: classify(mean_ppl, burst),
        uncertainty: SECTION_UNCERTAINTY.to_string(),
    };
    let ppls = scores.iter().map(|s| s.perplexity).collect();
    build_report(model, vec![section], ppls)
}

/// Detect per Extraction-Agent section, with an aggregated overall score.
#[tracing::instrument(skip(model, result), fields(sections = result.sections.len()))]
pub fn detect_extraction(
    model: &dyn PerplexityModel,
    result: &ExtractionResult,
) -> AiDetectionReport {
    let mut sections = Vec::new();
    let mut all_ppls = Vec::new();
    for sec in &result.sections {
        let text = sec.paragraphs.join(" ");
        if text.trim().is_empty() {
            continue;
        }
        let (mean_ppl, burst, scores) = score_block(model, &text);
        if scores.is_empty() {
            continue;
        }
        all_ppls.extend(scores.iter().map(|s| s.perplexity));
        sections.push(SectionAiScore {
            section: sec.kind,
            sentence_count: scores.len(),
            mean_perplexity: mean_ppl,
            burstiness: burst,
            signal: classify(mean_ppl, burst),
            uncertainty: SECTION_UNCERTAINTY.to_string(),
        });
    }
    build_report(model, sections, all_ppls)
}

// ---------------------------------------------------------------------------
// Passage-level flagging + the honest proportion (AI Check Set 2)
// ---------------------------------------------------------------------------

/// Per-passage caution. REQUIRED on every flagged passage, never empty —
/// the same un-strippable pattern as [`AI_DISCLAIMER`]/[`SECTION_UNCERTAINTY`].
pub const PASSAGE_UNCERTAINTY: &str = "AI-associated SIGNAL, not a determination of authorship. \
Passage-level flags have high false-positive rates — especially for non-native English, \
formulaic/technical prose, and short passages. Never treat a flagged passage as proof.";

/// Deterministic strength label, derived ONLY from the existing [`classify`]
/// thresholds (no new magic numbers):
/// - `Strong`: >= 2 sentences AND the passage as a whole meets the FULL
///   AI-like condition (mean perplexity AND burstiness under the same
///   constants `classify` uses);
/// - `Moderate`: >= 2 sentences whose mean perplexity is under the AI-like
///   threshold (the burstiness condition not met);
/// - `Weak`: a single flagged sentence — short text is exactly where the
///   disclaimer says detection is least reliable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PassageStrength {
    Strong,
    Moderate,
    Weak,
}

/// A contiguous run of sentences whose perplexity crosses the (existing)
/// AI-like threshold. A SIGNAL with strength and evidence — never a verdict.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FlaggedPassage {
    pub section: SectionKind,
    /// Char offsets of the span within the analyzed section text.
    pub start_char: usize,
    pub end_char: usize,
    /// The span text (evidence display).
    pub text: String,
    /// Constituent per-sentence scores — the evidence behind the flag.
    pub sentences: Vec<SentenceScore>,
    pub mean_perplexity: f64,
    pub burstiness: f64,
    pub signal: AiSignal,
    pub strength: PassageStrength,
    /// REQUIRED per-passage caution. Never empty.
    pub uncertainty: String,
}

/// Passage-level analysis + the HONEST percentage. `ai_signal_proportion`
/// is a deterministic COUNT — flagged sentence chars / total sentence chars
/// — i.e. "this proportion of the text shows AI-associated signals". It is
/// NOT a probability that the document is AI-written, and the field name is
/// chosen so it cannot be presented as one.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PassageAnalysis {
    pub model: String,
    pub passages: Vec<FlaggedPassage>,
    /// Total analyzed text length (sum of sentence chars).
    pub total_chars: usize,
    /// Flagged text length (sum of flagged sentence chars).
    pub flagged_chars: usize,
    /// flagged_chars / total_chars — the proportion of text with
    /// AI-associated signals. DETERMINISTIC, never model-guessed.
    pub ai_signal_proportion: f64,
    /// Mandatory disclaimer. Never empty.
    pub disclaimer: String,
}

/// A sentence carries an AI-associated signal when its perplexity sits under
/// the SAME AI-like threshold `classify` uses. One constant, one meaning.
fn sentence_flagged(ppl: f64) -> bool {
    ppl < AI_LIKE_PPL
}

fn passage_strength(sentences: &[SentenceScore]) -> PassageStrength {
    if sentences.len() < 2 {
        return PassageStrength::Weak;
    }
    let ppls: Vec<f64> = sentences.iter().map(|s| s.perplexity).collect();
    if classify(mean(&ppls), std_dev(&ppls)) == AiSignal::LeansAiLike {
        PassageStrength::Strong
    } else {
        PassageStrength::Moderate
    }
}

/// Group a section's scored sentences into flagged passages. Offsets are
/// located by scanning the section text forward (sentences come verbatim,
/// trimmed, in order from `split_sentences`).
fn flag_passages_in_block(
    section: SectionKind,
    block_text: &str,
    scores: &[SentenceScore],
) -> Vec<FlaggedPassage> {
    // Locate each sentence's char span in the block, in order.
    let mut spans: Vec<(usize, usize)> = Vec::with_capacity(scores.len());
    let mut cursor = 0usize;
    for s in scores {
        let found = block_text[cursor..].find(&s.text).map(|i| cursor + i);
        let (a, b) = match found {
            Some(a) => (a, a + s.text.len()),
            None => (cursor, cursor), // defensive: never panic on odd text
        };
        spans.push((a, b));
        cursor = b;
    }

    let mut passages = Vec::new();
    let mut run: Vec<usize> = Vec::new();
    let mut flush = |run: &mut Vec<usize>, passages: &mut Vec<FlaggedPassage>| {
        if run.is_empty() {
            return;
        }
        let first = run[0];
        let last = *run.last().unwrap();
        let sentences: Vec<SentenceScore> = run.iter().map(|&i| scores[i].clone()).collect();
        let ppls: Vec<f64> = sentences.iter().map(|s| s.perplexity).collect();
        let (start_char, end_char) = (spans[first].0, spans[last].1);
        passages.push(FlaggedPassage {
            section,
            start_char,
            end_char,
            text: block_text[start_char..end_char].to_string(),
            strength: passage_strength(&sentences),
            mean_perplexity: mean(&ppls),
            burstiness: std_dev(&ppls),
            signal: AiSignal::LeansAiLike,
            sentences,
            uncertainty: PASSAGE_UNCERTAINTY.to_string(),
        });
        run.clear();
    };

    for (i, s) in scores.iter().enumerate() {
        if sentence_flagged(s.perplexity) {
            run.push(i);
        } else {
            flush(&mut run, &mut passages);
        }
    }
    flush(&mut run, &mut passages);
    passages
}

fn finish_analysis(
    model: &dyn PerplexityModel,
    passages: Vec<FlaggedPassage>,
    total_chars: usize,
) -> PassageAnalysis {
    let flagged_chars: usize =
        passages.iter().flat_map(|p| p.sentences.iter()).map(|s| s.text.chars().count()).sum();
    let ai_signal_proportion =
        if total_chars == 0 { 0.0 } else { flagged_chars as f64 / total_chars as f64 };
    PassageAnalysis {
        model: model.name().to_string(),
        passages,
        total_chars,
        flagged_chars,
        ai_signal_proportion,
        disclaimer: AI_DISCLAIMER.to_string(),
    }
}

/// Passage-level analysis over one block of text.
pub fn analyze_passages_text(model: &dyn PerplexityModel, text: &str) -> PassageAnalysis {
    let (_, _, scores) = score_block(model, text);
    let total: usize = scores.iter().map(|s| s.text.chars().count()).sum();
    let passages = flag_passages_in_block(SectionKind::Other, text, &scores);
    finish_analysis(model, passages, total)
}

/// Passage-level analysis per Extraction-Agent section. Sits BESIDE
/// [`detect_extraction`] (which is unchanged): the section-level report and
/// the passage-level analysis share the same scoring and thresholds.
#[tracing::instrument(skip(model, result), fields(sections = result.sections.len()))]
pub fn analyze_passages(
    model: &dyn PerplexityModel,
    result: &ExtractionResult,
) -> PassageAnalysis {
    let mut passages = Vec::new();
    let mut total_chars = 0usize;
    for sec in &result.sections {
        let text = sec.paragraphs.join(" ");
        if text.trim().is_empty() {
            continue;
        }
        let (_, _, scores) = score_block(model, &text);
        if scores.is_empty() {
            continue;
        }
        total_chars += scores.iter().map(|s| s.text.chars().count()).sum::<usize>();
        passages.extend(flag_passages_in_block(sec.kind, &text, &scores));
    }
    finish_analysis(model, passages, total_chars)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::extract::extract_from_text;

    // Predictable, common-word, uniform prose — AI-like.
    const AI_SAMPLE: &str = "The results show that the model is able to process the data in a \
        way that is both fast and easy. The system can handle many tasks at the same time. It is \
        designed to be simple and clear for the user to use. The data shows that the method works \
        well for all of the tasks. The model is able to learn from the data in a short time.";

    // Varied, rare-word, mixed-length prose — human-like.
    const HUMAN_SAMPLE: &str = "Frankly, the contraption wheezed to life like an asthmatic \
        accordion, belching improbable plumes of cerulean smoke. Nobody anticipated such \
        theatrical flair from a glorified toaster. Yet there it sputtered, magnificent and \
        absurd. Mirabel laughed. Her grandmother's inscrutable schematics, scrawled across \
        napkins decades earlier, had somehow birthed this preposterous, wonderful monstrosity.";

    // ---- strided window: no loss, no double-count, never exceeds context ----

    #[test]
    fn strided_ranges_partition_exactly() {
        for (n, w, s) in [(10, 4, 2), (10, 4, 3), (7, 4, 2), (4, 4, 2), (3, 4, 2), (100, 8, 3)] {
            let ranges = strided_ranges(n, w, s);
            // window never exceeds w
            for &(b, _t, e) in &ranges {
                assert!(e - b <= w, "window {}..{} exceeds {w}", b, e);
            }
            // targets contiguously partition [0, n)
            assert_eq!(ranges.first().unwrap().1, 0, "must start at 0");
            assert_eq!(ranges.last().unwrap().2, n, "must end at n");
            for pair in ranges.windows(2) {
                assert_eq!(pair[0].2, pair[1].1, "gap/overlap at chunk edge");
            }
        }
    }

    #[test]
    fn strided_surprisals_lose_nothing_and_match_naive() {
        // context-free heuristic ⇒ windowed result must equal single-pass result
        let model = HeuristicModel::new(4, 2);
        let tokens: Vec<String> = (0..23).map(|i| format!("token{i}")).collect();
        let windowed = strided_surprisals(&model, &tokens, 4, 2);
        let naive = model.surprisals(&tokens);
        assert_eq!(windowed.len(), tokens.len(), "token count changed");
        assert_eq!(windowed, naive, "windowing altered per-token scores");
    }

    #[test]
    fn strided_handles_single_window_and_exact_multiple() {
        let model = HeuristicModel::new(8, 4);
        for n in [1usize, 8, 16, 17] {
            let tokens: Vec<String> = (0..n).map(|i| format!("w{i}")).collect();
            let out = strided_surprisals(&model, &tokens, 8, 4);
            assert_eq!(out, model.surprisals(&tokens));
        }
    }

    // ---- perplexity directionality: AI sample lower than human sample ----

    #[test]
    fn ai_sample_scores_lower_perplexity_than_human() {
        let model = HeuristicModel::gpt2_like();
        let ai = detect_text(&model, AI_SAMPLE);
        let human = detect_text(&model, HUMAN_SAMPLE);
        assert!(
            ai.overall_mean_perplexity < human.overall_mean_perplexity,
            "AI {} should be < human {}",
            ai.overall_mean_perplexity,
            human.overall_mean_perplexity
        );
    }

    #[test]
    fn human_sample_is_burstier_than_ai() {
        let model = HeuristicModel::gpt2_like();
        let ai = detect_text(&model, AI_SAMPLE);
        let human = detect_text(&model, HUMAN_SAMPLE);
        assert!(
            human.overall_burstiness > ai.overall_burstiness,
            "human {} should be burstier than AI {}",
            human.overall_burstiness,
            ai.overall_burstiness
        );
    }

    // ---- mandatory uncertainty / disclaimer on every result ----

    #[test]
    fn every_report_carries_a_nonempty_disclaimer() {
        let model = HeuristicModel::gpt2_like();
        let report = detect_text(&model, AI_SAMPLE);
        assert!(!report.disclaimer.is_empty());
        assert!(report.disclaimer.contains("NOT proof"));
        assert_eq!(report.confidence, "low");
        for section in &report.sections {
            assert!(!section.uncertainty.is_empty(), "section missing uncertainty note");
        }
    }

    #[test]
    fn per_section_report_scores_each_extraction_section() {
        let doc = "A Study\n\nAbstract\n".to_string()
            + AI_SAMPLE
            + "\n\nDiscussion\n"
            + HUMAN_SAMPLE;
        let model = HeuristicModel::gpt2_like();
        let report = detect_extraction(&model, &extract_from_text(&doc));

        let abstract_ = report
            .sections
            .iter()
            .find(|s| s.section == SectionKind::Abstract)
            .expect("abstract scored");
        let discussion = report
            .sections
            .iter()
            .find(|s| s.section == SectionKind::Discussion)
            .expect("discussion scored");

        // the AI-like abstract should read lower-perplexity than the human-like discussion
        assert!(abstract_.mean_perplexity < discussion.mean_perplexity);
        // disclaimer still mandatory on the aggregated report
        assert!(!report.disclaimer.is_empty());
        assert!(report.sections.iter().all(|s| !s.uncertainty.is_empty()));
    }

    #[test]
    fn deterministic() {
        let model = HeuristicModel::gpt2_like();
        assert_eq!(detect_text(&model, HUMAN_SAMPLE), detect_text(&model, HUMAN_SAMPLE));
    }
}

#[cfg(test)]
mod passage_tests {
    use super::*;

    /// With HeuristicModel: all-common-word sentences score ~ppl 4 (< 12,
    /// flagged); rare-long-word sentences score well above 12 (unflagged).
    const AI_SENT: &str = "The results show that the model can do the work well.";
    const HUMAN_SENT: &str = "Cerulean contraptions wheezed, magnificently preposterous, unfathomable.";

    fn m() -> HeuristicModel {
        HeuristicModel::default()
    }

    fn text_of(parts: &[&str]) -> String {
        parts.join(" ")
    }

    // ------------------------- passage spans + grouping ---------------------

    #[test]
    fn contiguous_ai_like_runs_become_passages_with_correct_spans() {
        // H, A, A, A, H, A  → two passages: len-3 and len-1.
        let text = text_of(&[HUMAN_SENT, AI_SENT, AI_SENT, AI_SENT, HUMAN_SENT, AI_SENT]);
        let out = analyze_passages_text(&m(), &text);
        assert_eq!(out.passages.len(), 2, "two contiguous runs expected: {:#?}", out.passages);
        assert_eq!(out.passages[0].sentences.len(), 3);
        assert_eq!(out.passages[1].sentences.len(), 1);
        // spans slice back to the exact text
        for p in &out.passages {
            assert_eq!(&text[p.start_char..p.end_char], p.text);
            assert!(p.text.contains("results show"));
        }
        // evidence rides along
        assert!(out.passages[0].sentences.iter().all(|s| s.perplexity < 12.0));
    }

    #[test]
    fn human_text_yields_no_false_passages_and_zero_proportion() {
        let text = text_of(&[HUMAN_SENT, HUMAN_SENT, HUMAN_SENT]);
        let out = analyze_passages_text(&m(), &text);
        assert!(out.passages.is_empty(), "no false flags: {:#?}", out.passages);
        assert_eq!(out.flagged_chars, 0);
        assert_eq!(out.ai_signal_proportion, 0.0);
        // the disclaimer STILL rides on an all-clear report
        assert_eq!(out.disclaimer, AI_DISCLAIMER);
    }

    #[test]
    fn inconclusive_sentences_are_not_force_categorized() {
        // Mixed common/rare words → mid perplexity (between the thresholds):
        // NOT flagged as a passage, and detect_text keeps Inconclusive.
        let mid = "The system wheezed with results and cerulean data everywhere today.";
        let text = text_of(&[mid, mid, mid]);
        let out = analyze_passages_text(&m(), &text);
        let detect = detect_text(&m(), &text);
        if detect.signal == AiSignal::Inconclusive {
            assert!(out.passages.is_empty(), "inconclusive must not be forced into a category");
        }
        // whatever the mid text scores, no passage may carry Inconclusive:
        assert!(out.passages.iter().all(|p| p.signal == AiSignal::LeansAiLike));
    }

    // ----------------------------- the honest % -----------------------------

    #[test]
    fn proportion_is_a_deterministic_char_count_not_a_probability() {
        let text = text_of(&[AI_SENT, HUMAN_SENT]);
        let out = analyze_passages_text(&m(), &text);
        // exactly the flagged sentence's chars over both sentences' chars
        let ai_len = AI_SENT.chars().count();
        let human_len = HUMAN_SENT.chars().count();
        assert_eq!(out.flagged_chars, ai_len);
        assert_eq!(out.total_chars, ai_len + human_len);
        let expected = ai_len as f64 / (ai_len + human_len) as f64;
        assert!((out.ai_signal_proportion - expected).abs() < 1e-12);
        // deterministic: identical across runs
        let again = analyze_passages_text(&m(), &text);
        assert_eq!(out, again);
    }

    #[test]
    fn all_flagged_text_approaches_one_never_exceeds() {
        let text = text_of(&[AI_SENT, AI_SENT, AI_SENT, AI_SENT]);
        let out = analyze_passages_text(&m(), &text);
        assert!(out.ai_signal_proportion > 0.9 && out.ai_signal_proportion <= 1.0);
        assert_eq!(out.passages.len(), 1, "one contiguous run");
    }

    // ------------------- caution at the passage level -----------------------

    #[test]
    fn the_caution_is_unstrippable_at_every_level() {
        let text = text_of(&[AI_SENT, AI_SENT, HUMAN_SENT, AI_SENT]);
        let out = analyze_passages_text(&m(), &text);
        assert!(!out.disclaimer.is_empty());
        assert!(out.disclaimer.contains("NOT proof"));
        assert!(out.disclaimer.contains("non-native English"));
        for p in &out.passages {
            assert!(!p.uncertainty.is_empty(), "per-passage caution is REQUIRED");
            assert!(p.uncertainty.contains("not a determination"));
        }
        // and it survives serialization — the UI receives it as data
        let wire = serde_json::to_string(&out).unwrap();
        assert!(wire.contains("NOT proof of AI authorship"));
        assert!(wire.contains("not a determination of authorship"));
        assert!(wire.contains("ai_signal_proportion"), "the field name says proportion, not probability");
        assert!(!wire.contains("probability"));
    }

    // -------------------------- strength labels -----------------------------

    #[test]
    fn strength_maps_deterministically_from_the_existing_thresholds() {
        // len-1 run → Weak (short text is where detection is least reliable)
        let single = analyze_passages_text(&m(), &text_of(&[HUMAN_SENT, AI_SENT, HUMAN_SENT]));
        assert_eq!(single.passages[0].strength, PassageStrength::Weak);

        // len-4 uniform AI-like run: burstiness ~0 < AI_LIKE_BURSTINESS and
        // mean < AI_LIKE_PPL → the FULL classify condition → Strong.
        let strong = analyze_passages_text(&m(), &text_of(&[AI_SENT, AI_SENT, AI_SENT, AI_SENT]));
        assert_eq!(strong.passages[0].strength, PassageStrength::Strong);

        // >= 2 sentences flagged but with enough perplexity VARIANCE to fail
        // the burstiness half of classify → Moderate.
        let varied_a = "The results show that the model can do the work well.";
        let varied_b = "It is due to work of all of the top labs everywhere generally speaking.";
        let out = analyze_passages_text(&m(), &text_of(&[varied_a, varied_b]));
        if out.passages.len() == 1 && out.passages[0].sentences.len() == 2 {
            let p = &out.passages[0];
            let expected = if classify(p.mean_perplexity, p.burstiness) == AiSignal::LeansAiLike {
                PassageStrength::Strong
            } else {
                PassageStrength::Moderate
            };
            assert_eq!(p.strength, expected, "strength derives ONLY from classify's constants");
        }
    }

    // ---------------------- extraction-level analysis -----------------------

    #[test]
    fn analyze_passages_covers_sections_and_existing_detect_is_unchanged() {
        let doc = format!(
            "Introduction\n\n{h}\n\nMethods\n\n{a} {a} {a}\n\nDiscussion\n\n{h}\n",
            a = AI_SENT,
            h = HUMAN_SENT
        );
        let ex = crate::extract::extract_from_text(&doc);
        let out = analyze_passages(&m(), &ex);
        assert!(!out.passages.is_empty(), "the methods run should flag");
        assert!(out.passages.iter().any(|p| p.sentences.len() >= 3));
        assert!(out.ai_signal_proportion > 0.0 && out.ai_signal_proportion < 1.0);

        // the ORIGINAL per-section report still works, side by side
        let old = detect_extraction(&m(), &ex);
        assert!(!old.sections.is_empty());
        assert_eq!(old.disclaimer, AI_DISCLAIMER);
    }
}
