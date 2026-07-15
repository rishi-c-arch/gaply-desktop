//! Pure, deterministic CHEAP signals for the AI-Signal ensemble (Set C1). No
//! network, no model — text in, feature out, microseconds each.
//!
//! FAIRNESS is load-bearing: stylometric signals (burstiness, lexical diversity,
//! function-word usage) provably OVER-FLAG non-native English writers, so they
//! carry [`BiasTier::Stylometric`] and are down-weighted; citation/structural
//! signals are fairer. Every value here is ONE soft input to a future calibrated
//! ensemble — never a standalone verdict.

use std::collections::HashSet;

use crate::extract::citations::{Citation, CitationStyle, Reference};

// ---------------------------------------------------------------------------
// Stylometry (BiasTier::Stylometric — down-weighted)
// ---------------------------------------------------------------------------

/// Minimum sentences for a trustworthy length-CV (short text is noisy).
const MIN_SENTENCES_FOR_CV: usize = 3;

/// Sentence-length burstiness = coefficient of variation (σ/μ) of sentence
/// word-lengths. LOW/uniform → AI-leaning (measured human median ≈ 0.43).
/// `None` for < 3 sentences.
pub fn sentence_length_cv(sentences: &[&str]) -> Option<f64> {
    let lens: Vec<f64> = sentences
        .iter()
        .map(|s| s.split_whitespace().count() as f64)
        .filter(|&l| l > 0.0)
        .collect();
    if lens.len() < MIN_SENTENCES_FOR_CV {
        return None;
    }
    let mean = lens.iter().sum::<f64>() / lens.len() as f64;
    if mean == 0.0 {
        return None;
    }
    let var = lens.iter().map(|l| (l - mean).powi(2)).sum::<f64>() / lens.len() as f64;
    Some(var.sqrt() / mean)
}

/// Punctuation counts per 100 words. The em-dash (U+2014) is counted SEPARATELY
/// — a GPT-4o-era tell, but model-specific and trivially evadable (low weight,
/// do not hardcode a direction).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PunctProfile {
    pub comma_per100: f64,
    pub period_per100: f64,
    pub semicolon_per100: f64,
    pub em_dash_per100: f64,
}

pub fn punctuation_profile(text: &str) -> PunctProfile {
    let words = text.split_whitespace().count().max(1) as f64;
    let count = |c: char| text.chars().filter(|&x| x == c).count() as f64;
    let per100 = |n: f64| n / words * 100.0;
    PunctProfile {
        comma_per100: per100(count(',')),
        period_per100: per100(count('.')),
        semicolon_per100: per100(count(';')),
        em_dash_per100: per100(count('\u{2014}')),
    }
}

/// A conservative function-word set (English). AI academic prose tends to use
/// FEWER function words (higher lexical density) — but the ratio is L1-dependent,
/// so this is a SOFT, down-weighted signal compared against a human reference.
const FUNCTION_WORDS: &[&str] = &[
    "the", "of", "and", "a", "to", "in", "is", "that", "for", "it", "as", "with", "by", "on",
    "are", "this", "be", "an", "which", "or", "from", "at", "was", "were", "not", "but", "have",
    "has", "had", "their", "its", "can", "we", "our",
];

/// Fraction of tokens that are function words.
pub fn function_word_ratio(tokens: &[String]) -> Option<f64> {
    if tokens.is_empty() {
        return None;
    }
    let fw = tokens
        .iter()
        .filter(|t| FUNCTION_WORDS.contains(&t.to_lowercase().as_str()))
        .count();
    Some(fw as f64 / tokens.len() as f64)
}

// ---------------------------------------------------------------------------
// Lexical diversity — MTLD (BiasTier::Stylometric)
// ---------------------------------------------------------------------------

/// Below this token count MTLD is unstable (McCarthy & Jarvis: stabilises ~100).
pub const MTLD_MIN_TOKENS: usize = 100;
const MTLD_TTR_THRESHOLD: f64 = 0.720;

/// MTLD (Measure of Textual Lexical Diversity), forward+backward averaged —
/// length-ROBUST (unlike raw TTR). `None` below the ≥100-token gate. Direction
/// is genuinely two-sided (AI measured both higher AND lower than humans by
/// domain), so callers use [`two_sided_deviation`], never a one-sided threshold.
pub fn mtld(tokens: &[String]) -> Option<f64> {
    if tokens.len() < MTLD_MIN_TOKENS {
        return None;
    }
    let fwd = mtld_pass(tokens);
    let mut rev: Vec<String> = tokens.to_vec();
    rev.reverse();
    let bwd = mtld_pass(&rev);
    Some((fwd + bwd) / 2.0)
}

fn mtld_pass(tokens: &[String]) -> f64 {
    let mut factors = 0.0f64;
    let mut types: HashSet<String> = HashSet::new();
    let mut count = 0usize;
    for t in tokens {
        count += 1;
        types.insert(t.to_lowercase());
        if types.len() as f64 / count as f64 <= MTLD_TTR_THRESHOLD {
            factors += 1.0;
            types.clear();
            count = 0;
        }
    }
    if count > 0 {
        let ttr = types.len() as f64 / count as f64;
        factors += (1.0 - ttr) / (1.0 - MTLD_TTR_THRESHOLD);
    }
    if factors == 0.0 {
        return tokens.len() as f64;
    }
    tokens.len() as f64 / factors
}

/// TWO-SIDED deviation of a value from a human reference (absolute distance).
/// Used for MTLD so neither "too low" nor "too high" is hard-coded as AI.
pub fn two_sided_deviation(value: f64, human_reference: f64) -> f64 {
    (value - human_reference).abs()
}

// ---------------------------------------------------------------------------
// N-gram repetition (BiasTier::Structural)
// ---------------------------------------------------------------------------

/// Repetition density = 1 − distinct-n. HIGH → repetitive → AI-leaning
/// (for modern sampled prose). `0.0` for text shorter than one n-gram.
pub fn ngram_repetition(tokens: &[String], n: usize) -> f64 {
    if n == 0 || tokens.len() < n {
        return 0.0;
    }
    let mut seen: HashSet<Vec<String>> = HashSet::new();
    let mut total = 0usize;
    for w in tokens.windows(n) {
        seen.insert(w.iter().map(|t| t.to_lowercase()).collect());
        total += 1;
    }
    if total == 0 {
        return 0.0;
    }
    1.0 - (seen.len() as f64 / total as f64)
}

// ---------------------------------------------------------------------------
// Template-phrase density (BiasTier::Structural) — DE-BIASED
// ---------------------------------------------------------------------------

/// AI template/transition markers. REGIONALLY-LOADED tokens are deliberately
/// EXCLUDED — "delve"/"empower"/"realm"/"tapestry" are common in West-African
/// English (an RLHF-annotation artifact) and would import a dialect bias.
const TEMPLATE_MARKERS: &[&str] = &[
    "furthermore",
    "moreover",
    "it is important to note",
    "it is worth noting",
    "in conclusion",
    "consequently",
    "notably",
    "multifaceted",
    "underscores",
    "showcasing",
    "comprehensive examination",
    "paramount importance",
    "plays a crucial role",
    "in the realm of scholarly", // phrase, not the bare loaded word "realm"
];

/// Minimum DISTINCT co-occurring markers before the signal registers at all.
pub const TEMPLATE_MIN_DISTINCT_MARKERS: usize = 2;

/// De-biased template-phrase density (hits per 1000 words) — but ONLY when at
/// least [`TEMPLATE_MIN_DISTINCT_MARKERS`] DISTINCT markers co-occur. A single
/// marker returns `0.0`: the signal is NEVER standalone (one "furthermore" is
/// not evidence, and single loaded words penalise non-native writers).
pub fn template_density(text: &str) -> f64 {
    let lower = text.to_lowercase();
    let words = lower.split_whitespace().count().max(1);
    let mut distinct = 0usize;
    let mut hits = 0usize;
    for m in TEMPLATE_MARKERS {
        let c = lower.matches(m).count();
        if c > 0 {
            distinct += 1;
            hits += c;
        }
    }
    if distinct < TEMPLATE_MIN_DISTINCT_MARKERS {
        return 0.0;
    }
    hits as f64 / words as f64 * 1000.0
}

// ---------------------------------------------------------------------------
// Citation behaviour — LOCAL components (BiasTier::Structural; the fair anchor)
// ---------------------------------------------------------------------------

/// In-text citations per 1000 words. AI academic text is systematically LESS
/// citation-dense. Low bias.
pub fn citation_density(n_citations: usize, word_count: usize) -> f64 {
    if word_count == 0 {
        return 0.0;
    }
    n_citations as f64 / word_count as f64 * 1000.0
}

/// Fraction of in-text citations in the DOMINANT style (1.0 = fully consistent).
/// AI reference lists mix styles; humans are consistent. `None` if no citations.
pub fn citation_style_consistency(citations: &[Citation]) -> Option<f64> {
    if citations.is_empty() {
        return None;
    }
    let paren = citations
        .iter()
        .filter(|c| c.style == CitationStyle::Parenthetical)
        .count();
    let dominant = paren.max(citations.len() - paren);
    Some(dominant as f64 / citations.len() as f64)
}

/// A syntactically-valid DOI: `10.NNNN/...` (registrant + suffix). Pure syntax —
/// existence is the NETWORK lane (C2).
pub fn is_valid_doi(doi: &str) -> bool {
    let d = doi.trim();
    let Some(rest) = d.strip_prefix("10.") else {
        return false;
    };
    match rest.split_once('/') {
        Some((registrant, suffix)) => {
            registrant.len() >= 4
                && registrant.chars().all(|c| c.is_ascii_digit())
                && !suffix.is_empty()
        }
        None => false,
    }
}

/// Fraction of DOI-bearing references whose DOI is syntactically valid. `None`
/// if no references carry a DOI.
pub fn doi_syntax_validity(references: &[Reference]) -> Option<f64> {
    let with_doi: Vec<&str> = references.iter().filter_map(|r| r.doi.as_deref()).collect();
    if with_doi.is_empty() {
        return None;
    }
    let valid = with_doi.iter().filter(|d| is_valid_doi(d)).count();
    Some(valid as f64 / with_doi.len() as f64)
}

// ---------------------------------------------------------------------------
// Provisional human-academic stylometry reference (bundled, like stage1_norms)
// ---------------------------------------------------------------------------

const STYLO_JSON: &str = include_str!("../calibration/stylo_norms.json");

#[derive(Debug, Clone, PartialEq, serde::Deserialize)]
pub struct StyloHumanNorm {
    pub sentence_length_cv_median: f64,
    pub function_word_ratio_median: f64,
    pub comma_per100_median: f64,
    pub mtld_median: f64,
}

#[derive(Debug, Clone, PartialEq, serde::Deserialize)]
pub struct StyloNorms {
    pub version: u32,
    pub provisional: bool,
    pub human_academic: StyloHumanNorm,
}

impl StyloNorms {
    /// The compile-time-bundled provisional stylometry reference.
    pub fn bundled() -> Self {
        serde_json::from_str(STYLO_JSON).expect("bundled stylo_norms.json must parse")
    }
}

/// A minimal sentence splitter (on . ! ? boundaries) for document-level stylometry.
fn split_sents(text: &str) -> Vec<String> {
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

fn word_tokens(text: &str) -> Vec<String> {
    text.split_whitespace()
        .map(|w| w.trim_matches(|c: char| !c.is_alphanumeric()).to_lowercase())
        .filter(|w| !w.is_empty())
        .collect()
}

/// Compute all the document-level cheap signals over an extraction. Pure, µs.
pub fn document_features(
    result: &crate::extract::ExtractionResult,
    norms: &StyloNorms,
) -> crate::ai_features::DocumentFeatures {
    use crate::ai_features::DocumentFeatures;
    let doc_text: String = result
        .sections
        .iter()
        .map(|s| s.paragraphs.join(" "))
        .collect::<Vec<_>>()
        .join(" ");
    let word_count = doc_text.split_whitespace().count();
    let tokens = word_tokens(&doc_text);
    let sentences = split_sents(&doc_text);
    let sent_refs: Vec<&str> = sentences.iter().map(|s| s.as_str()).collect();
    let h = &norms.human_academic;
    let mtld_val = mtld(&tokens);

    DocumentFeatures {
        sentence_length_cv: sentence_length_cv(&sent_refs),
        function_word_ratio: function_word_ratio(&tokens),
        em_dash_per100: Some(punctuation_profile(&doc_text).em_dash_per100),
        mtld: mtld_val,
        mtld_deviation: mtld_val.map(|m| two_sided_deviation(m, h.mtld_median)),
        ngram_repetition: Some(ngram_repetition(&tokens, 3)),
        template_density: Some(template_density(&doc_text)),
        citation_density: Some(citation_density(result.citations.len(), word_count)),
        citation_style_consistency: citation_style_consistency(&result.citations),
        doi_syntax_validity: doi_syntax_validity(&result.references),
    }
}

/// Turn document features into per-signal Evidence rows (Set D renders these).
/// PROVISIONAL qualitative levels — no numeric score (honesty gate). Every row
/// carries its `BiasTier` so a future ensemble weighs it correctly.
pub fn document_evidence(
    f: &crate::ai_features::DocumentFeatures,
    norms: &StyloNorms,
) -> Vec<crate::ai_features::SignalEvidence> {
    use crate::ai_features::{BiasTier, SignalEvidence, SignalLevel};
    let h = &norms.human_academic;
    let mut ev = Vec::new();
    let row = |signal: &str, level: SignalLevel, bias: BiasTier, detail: String| SignalEvidence {
        signal: signal.to_string(),
        level,
        bias_tier: bias,
        detail,
    };

    // Sentence-length burstiness — low = AI-leaning (stylometric, soft).
    if let Some(cv) = f.sentence_length_cv {
        let lvl = if cv < h.sentence_length_cv_median * 0.6 {
            SignalLevel::High
        } else if cv < h.sentence_length_cv_median * 0.85 {
            SignalLevel::Moderate
        } else {
            SignalLevel::Low
        };
        ev.push(row("sentence-length burstiness", lvl, BiasTier::Stylometric,
            format!("CV {cv:.2} (human ≈ {:.2})", h.sentence_length_cv_median)));
    }
    // Lexical diversity (MTLD) — two-sided deviation (stylometric, soft).
    if let (Some(m), Some(dev)) = (f.mtld, f.mtld_deviation) {
        let lvl = if dev > h.mtld_median * 0.35 { SignalLevel::Moderate } else { SignalLevel::Low };
        ev.push(row("lexical diversity (MTLD)", lvl, BiasTier::Stylometric,
            format!("MTLD {m:.0}, deviates {dev:.0} from human ≈ {:.0}", h.mtld_median)));
    }
    // Template phrasing — structural.
    if let Some(td) = f.template_density {
        if td > 0.0 {
            ev.push(row("template phrasing", SignalLevel::High, BiasTier::Structural,
                format!("{td:.1} markers / 1000 words (≥2 distinct)")));
        }
    }
    // Repetition — structural.
    if let Some(r) = f.ngram_repetition {
        let lvl = if r > 0.15 { SignalLevel::Moderate } else { SignalLevel::Low };
        ev.push(row("n-gram repetition", lvl, BiasTier::Structural, format!("{:.0}% repeated 3-grams", r * 100.0)));
    }
    // Citation density — the fair anchor's local component (structural).
    if let Some(d) = f.citation_density {
        let lvl = if d < 1.0 { SignalLevel::High } else if d < 5.0 { SignalLevel::Moderate } else { SignalLevel::Low };
        ev.push(row("citation density", lvl, BiasTier::Structural, format!("{d:.1} citations / 1000 words")));
    }
    // DOI syntax validity — structural.
    if let Some(v) = f.doi_syntax_validity {
        let lvl = if v < 0.9 { SignalLevel::Moderate } else { SignalLevel::Low };
        ev.push(row("DOI syntax validity", lvl, BiasTier::Structural, format!("{:.0}% of DOIs well-formed", v * 100.0)));
    }
    ev
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::extract::Location;

    fn toks(t: &str) -> Vec<String> {
        t.split_whitespace().map(|w| w.to_string()).collect()
    }

    #[test]
    fn burstiness_cv_low_for_uniform_high_for_varied() {
        let uniform = ["one two three four", "five six seven eight", "nine ten eleven twelve"];
        let varied = ["a", "one two three four five six seven eight nine ten", "b c"];
        let u = sentence_length_cv(&uniform).unwrap();
        let v = sentence_length_cv(&varied).unwrap();
        assert!(u < v, "uniform sentence lengths → lower burstiness: {u} vs {v}");
        assert!(sentence_length_cv(&["only one"]).is_none(), "too few sentences → None");
    }

    #[test]
    fn punctuation_counts_em_dash_separately() {
        let p = punctuation_profile("a, b; c \u{2014} d. e, f");
        assert!(p.comma_per100 > 0.0 && p.semicolon_per100 > 0.0);
        assert!(p.em_dash_per100 > 0.0, "U+2014 counted separately");
        // a hyphen-minus is NOT an em-dash
        assert_eq!(punctuation_profile("a-b c-d").em_dash_per100, 0.0);
    }

    #[test]
    fn mtld_is_gated_below_100_tokens_and_two_sided() {
        assert!(mtld(&toks("short text")).is_none(), "≥100-token gate");
        let long: Vec<String> = (0..150).map(|i| format!("w{}", i % 40)).collect();
        assert!(mtld(&long).is_some());
        // two-sided: both directions deviate from a human reference of 70.
        assert_eq!(two_sided_deviation(50.0, 70.0), two_sided_deviation(90.0, 70.0));
    }

    #[test]
    fn ngram_repetition_rises_with_repeats() {
        let unique = toks("alpha beta gamma delta epsilon zeta eta theta");
        let repeated = toks("go go go go go go go go");
        assert!(ngram_repetition(&repeated, 2) > ngram_repetition(&unique, 2));
    }

    #[test]
    fn template_debiasing_all_three_rules() {
        // RULE: single marker → NO flag (never standalone).
        assert_eq!(template_density("Furthermore, the results are clear and useful today."), 0.0,
            "one marker is not evidence");
        // RULE: >=2 distinct markers → registers.
        assert!(template_density("Furthermore, this is notably a comprehensive examination of it.") > 0.0);
        // RULE: regionally-loaded tokens are IGNORED (not in the lexicon).
        assert_eq!(template_density("We delve into the realm and empower the tapestry of ideas here."), 0.0,
            "delve/realm/empower/tapestry stripped — no dialect penalty");
    }

    #[test]
    fn doi_syntax_validity_and_citation_signals() {
        assert!(is_valid_doi("10.1126/sciadv.adt3813"));
        assert!(!is_valid_doi("10.12/x"), "registrant too short");
        assert!(!is_valid_doi("not-a-doi"));
        assert!(!is_valid_doi("10.1234/"), "empty suffix");

        let refs = vec![
            Reference { raw: "".into(), authors: "".into(), year: None, title: None, doi: Some("10.1000/valid".into()) },
            Reference { raw: "".into(), authors: "".into(), year: None, title: None, doi: Some("bogus".into()) },
            Reference { raw: "".into(), authors: "".into(), year: None, title: None, doi: None },
        ];
        assert_eq!(doi_syntax_validity(&refs), Some(0.5), "1 of 2 DOI-bearing refs valid");

        assert_eq!(citation_density(4, 2000), 2.0);
        let loc = Location { section: crate::extract::SectionKind::Other, paragraph: 0 };
        let cites = vec![
            Citation { style: CitationStyle::Parenthetical, authors: "A".into(), year: 2020, raw: "".into(), location: loc.clone() },
            Citation { style: CitationStyle::Parenthetical, authors: "B".into(), year: 2021, raw: "".into(), location: loc.clone() },
            Citation { style: CitationStyle::Narrative, authors: "C".into(), year: 2022, raw: "".into(), location: loc },
        ];
        assert_eq!(citation_style_consistency(&cites), Some(2.0 / 3.0));
        assert!(citation_style_consistency(&[]).is_none());
    }

    #[test]
    fn stylo_norms_bundled_provisional_and_document_features_compute() {
        let norms = StyloNorms::bundled();
        assert!(norms.provisional, "Phase-1 stylo reference is provisional");
        assert!((norms.human_academic.sentence_length_cv_median - 0.434).abs() < 1e-6);

        let ex = crate::extract::extract_from_text(
            "Introduction\n\nFurthermore, this comprehensive examination underscores a notably \
             multifaceted phenomenon. The results are consequently significant and clear (Smith, 2020). \
             Moreover, the findings are paramount importance to the field of study examined here.\n",
        );
        let f = document_features(&ex, &norms);
        assert!(f.sentence_length_cv.is_some());
        assert!(f.template_density.unwrap() > 0.0, "multiple distinct markers registered");
        assert!(f.citation_density.unwrap() > 0.0, "one (Smith, 2020) citation counted");
        // Evidence rows carry bias tiers; no numeric score is produced (honesty gate).
        let ev = document_evidence(&f, &norms);
        assert!(!ev.is_empty());
        assert!(ev.iter().any(|e| e.signal.contains("template")));
        assert!(ev.iter().any(|e| matches!(e.bias_tier, crate::ai_features::BiasTier::Stylometric)));
        assert!(ev.iter().any(|e| matches!(e.bias_tier, crate::ai_features::BiasTier::Structural)));
    }
}
