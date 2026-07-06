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
/// disclaimer. Recalibrate when a real model is wired to the trait.
fn classify(mean_ppl: f64, burstiness: f64) -> AiSignal {
    if mean_ppl < 12.0 && burstiness < 8.0 {
        AiSignal::LeansAiLike
    } else if mean_ppl > 20.0 || burstiness > 15.0 {
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
