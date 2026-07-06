//! Memory-poisoning defense, stage 2: a perplexity-based anomaly filter.
//!
//! Natural language has low character-level entropy — a handful of letters
//! and spaces dominate, so the per-character distribution is far from
//! uniform. Injected payloads (base64/hex blobs, encoded shellcode,
//! adversarial token-soup, zalgo) approach a uniform distribution over a
//! large symbol set. We score text by the perplexity of an order-0 character
//! model — `2^H` where `H` is the Shannon entropy (bits) of the character
//! distribution. English prose (incl. spaces) lands near 2^4.1 ≈ 17; a blob
//! using all 64 base64 symbols uniformly approaches 2^6 = 64.
//!
//! Order-0 is deliberate: a bigram model over a 64-symbol alphabet has 4096
//! cells, so from a few hundred characters its self-estimate is badly biased
//! (sparse rows look deterministic and *under*-estimate entropy — exactly
//! backwards for short random strings). The unigram estimate needs far fewer
//! samples and is stable at document-chunk scale.
//!
//! This is a statistical proxy, not an LLM: cheap, deterministic, offline.
//! It complements (never replaces) the pattern scan in [`crate::sanitize`].

use std::collections::HashMap;

/// Texts scoring above this character perplexity are anomalous. English
/// prose stays under ~22 even with technical vocabulary and digits; uniform
/// encoded payloads score 2.5–4× higher.
pub const PERPLEXITY_THRESHOLD: f64 = 30.0;

/// Below this many characters there isn't enough signal to score reliably;
/// short texts pass (they are also too small to hide a meaningful payload).
pub const MIN_SCORABLE_CHARS: usize = 120;

/// Order-0 (unigram) character perplexity of `text`: `2^H` where `H` is the
/// Shannon entropy of the character distribution. Case-folded; whitespace
/// runs collapse to a single space.
pub fn char_perplexity(text: &str) -> f64 {
    let mut counts: HashMap<char, f64> = HashMap::new();
    let mut total = 0.0;
    let mut last_space = false;
    for c in text.to_lowercase().chars() {
        let c = if c.is_whitespace() {
            if last_space {
                continue;
            }
            last_space = true;
            ' '
        } else {
            last_space = false;
            c
        };
        *counts.entry(c).or_default() += 1.0;
        total += 1.0;
    }
    if total < 2.0 {
        return 1.0;
    }

    let mut entropy = 0.0;
    for count in counts.values() {
        let p = count / total;
        entropy -= p * p.log2();
    }
    2f64.powf(entropy)
}

/// Returns `Some(score)` when `text` is anomalous, `None` when it looks like
/// natural language (or is too short to score).
pub fn anomaly_score(text: &str) -> Option<f64> {
    if text.chars().count() < MIN_SCORABLE_CHARS {
        return None;
    }
    let score = char_perplexity(text);
    (score > PERPLEXITY_THRESHOLD).then_some(score)
}

#[cfg(test)]
mod tests {
    use super::*;

    const GUIDELINE: &str = "Authors should ensure that all references are cited in the text \
        and listed at the end of the manuscript. The journal follows the Vancouver style for \
        citations. Tables must be numbered consecutively and referred to in the body of the \
        article. Statistical methods should be described with enough detail to enable a \
        knowledgeable reader with access to the original data to verify the reported results.";

    fn pseudo_base64(len: usize, seed: u64) -> String {
        let alphabet: Vec<char> =
            "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/".chars().collect();
        let mut x = seed;
        (0..len)
            .map(|_| {
                x = x.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
                alphabet[(x >> 33) as usize % alphabet.len()]
            })
            .collect()
    }

    #[test]
    fn english_prose_is_not_anomalous() {
        let ppl = char_perplexity(GUIDELINE);
        assert!(ppl < PERPLEXITY_THRESHOLD, "prose scored {ppl}");
        assert!(anomaly_score(GUIDELINE).is_none());
    }

    #[test]
    fn base64_blob_is_anomalous() {
        for seed in [42u64, 0x9E3779B97F4A7C15, 12345] {
            let blob = pseudo_base64(600, seed);
            let ppl = char_perplexity(&blob);
            assert!(ppl > PERPLEXITY_THRESHOLD, "payload (seed {seed}) scored only {ppl}");
            assert!(anomaly_score(&blob).is_some());
        }
    }

    #[test]
    fn short_text_passes_without_scoring() {
        assert!(anomaly_score("Qm90aA==").is_none());
    }

    #[test]
    fn prose_with_digits_and_tables_stays_natural() {
        let text = "Table 3 reports 1240 patients across 5 sites over 24 months, with p < 0.05 \
                    for the primary endpoint and a 95% confidence interval of 1.2 to 3.4 percent.";
        assert!(char_perplexity(text) < PERPLEXITY_THRESHOLD);
    }
}
