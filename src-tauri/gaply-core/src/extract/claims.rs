//! Deterministic scientific-claim extraction.
//!
//! Stage 1 (implemented): rule-based claim detection using section awareness,
//! scientific cue phrases, reporting verbs, and statistical indicators.
//!
//! Stage 2 (planned): optional local-SLM refinement of candidate claims.
//! Stage 3 (planned): optional cloud refinement with bounded metadata only.
//!
//! Claims are NOT sentences. A sentence may contain zero, one, or many claims.
//! A claim may also be summarized across multiple sentences. This stage emits
//! one claim per detected cue, which future SLM/cloud stages can split, merge,
//! or normalize.

use std::collections::HashMap;
use std::sync::OnceLock;

use regex::Regex;

use crate::extract::{ExtractionResult, Location, SectionKind, sentence};
use crate::scientific_model::{
    ClaimCategory, ClaimId, ClaimNature, ClaimSource, ClaimVerificationStatus, CitationRef,
    ScientificClaim, SourceSpan, StatRef,
};

/// Maximum length of a claim statement stored in the model. Longer clauses are
/// truncated; the full text remains reachable through `source_span`.
const MAX_STATEMENT_LEN: usize = 500;

/// A detected cue phrase plus its metadata.
struct Cue {
    category: ClaimCategory,
    confidence: f64,
    re: Regex,
}

struct CueSet {
    cues: Vec<Cue>,
}

fn cue_set() -> &'static CueSet {
    static SET: OnceLock<CueSet> = OnceLock::new();
    SET.get_or_init(|| CueSet {
        cues: vec![
            // Main / result claims
            Cue {
                category: ClaimCategory::MainClaim,
                confidence: 0.95,
                re: Regex::new(r"(?i)\bthis study (shows?|demonstrates?|indicates?|suggests?|reveals?|found) that\b").unwrap(),
            },
            Cue {
                category: ClaimCategory::MainClaim,
                confidence: 0.95,
                re: Regex::new(r"(?i)\bour findings (show|demonstrate|indicate|suggest|reveal) that\b").unwrap(),
            },
            Cue {
                category: ClaimCategory::ResultClaim,
                confidence: 0.9,
                re: Regex::new(r"(?i)\bwe found that\b").unwrap(),
            },
            Cue {
                category: ClaimCategory::ResultClaim,
                confidence: 0.9,
                re: Regex::new(r"(?i)\bresults (show|demonstrate|indicate|suggest) that\b").unwrap(),
            },
            // Conclusion claims
            Cue {
                category: ClaimCategory::ConclusionClaim,
                confidence: 1.0,
                re: Regex::new(r"(?i)\bwe conclude that\b").unwrap(),
            },
            Cue {
                category: ClaimCategory::ConclusionClaim,
                confidence: 0.9,
                re: Regex::new(r"(?i)\btaken together[,.]?\b").unwrap(),
            },
            Cue {
                category: ClaimCategory::ConclusionClaim,
                confidence: 0.9,
                re: Regex::new(r"(?i)\bin conclusion\b").unwrap(),
            },
            Cue {
                category: ClaimCategory::ConclusionClaim,
                confidence: 0.85,
                re: Regex::new(r"(?i)\bcollectively[,.]?\b").unwrap(),
            },
            // Hypothesis claims
            Cue {
                category: ClaimCategory::HypothesisClaim,
                confidence: 0.95,
                re: Regex::new(r"(?i)\bwe hypothesized that\b").unwrap(),
            },
            Cue {
                category: ClaimCategory::HypothesisClaim,
                confidence: 0.95,
                re: Regex::new(r"(?i)\bwe predicted that\b").unwrap(),
            },
            Cue {
                category: ClaimCategory::HypothesisClaim,
                confidence: 0.9,
                re: Regex::new(r"(?i)\bour hypothesis\b").unwrap(),
            },
            // Method claims
            Cue {
                category: ClaimCategory::MethodClaim,
                confidence: 0.8,
                re: Regex::new(r"(?i)\bparticipants were\b").unwrap(),
            },
            Cue {
                category: ClaimCategory::MethodClaim,
                confidence: 0.75,
                re: Regex::new(r"(?i)\bwe used\b").unwrap(),
            },
            Cue {
                category: ClaimCategory::MethodClaim,
                confidence: 0.8,
                re: Regex::new(r"(?i)\bwe conducted\b").unwrap(),
            },
            Cue {
                category: ClaimCategory::MethodClaim,
                confidence: 0.85,
                re: Regex::new(r"(?i)\brandomized (controlled )?trial\b").unwrap(),
            },
            // Background claims
            Cue {
                category: ClaimCategory::BackgroundClaim,
                confidence: 0.75,
                re: Regex::new(r"(?i)\bprevious studies\b").unwrap(),
            },
            Cue {
                category: ClaimCategory::BackgroundClaim,
                confidence: 0.75,
                re: Regex::new(r"(?i)\bhas been shown\b").unwrap(),
            },
            Cue {
                category: ClaimCategory::BackgroundClaim,
                confidence: 0.8,
                re: Regex::new(r"(?i)\bit is (well )?known that\b").unwrap(),
            },
            // Supporting claims
            Cue {
                category: ClaimCategory::SupportingClaim,
                confidence: 0.85,
                re: Regex::new(r"(?i)\bthese results support\b").unwrap(),
            },
            // Comparative claims
            Cue {
                category: ClaimCategory::ComparativeClaim,
                confidence: 0.8,
                re: Regex::new(r"(?i)\bcompared to\b").unwrap(),
            },
            Cue {
                category: ClaimCategory::ComparativeClaim,
                confidence: 0.8,
                re: Regex::new(r"(?i)\bversus\b").unwrap(),
            },
            Cue {
                category: ClaimCategory::ComparativeClaim,
                confidence: 0.75,
                re: Regex::new(r"(?i)\b(in contrast to|higher than|lower than|greater than|less than)\b").unwrap(),
            },
            // Negative claims
            Cue {
                category: ClaimCategory::NegativeClaim,
                confidence: 0.8,
                re: Regex::new(r"(?i)\bno significant\b").unwrap(),
            },
            Cue {
                category: ClaimCategory::NegativeClaim,
                confidence: 0.8,
                re: Regex::new(r"(?i)\bdid not (find|show|observe|demonstrate)\b").unwrap(),
            },
            Cue {
                category: ClaimCategory::NegativeClaim,
                confidence: 0.75,
                re: Regex::new(r"(?i)\babsence of\b").unwrap(),
            },
            // Novelty claims
            Cue {
                category: ClaimCategory::NoveltyClaim,
                confidence: 0.9,
                re: Regex::new(r"(?i)\bfor the first time\b").unwrap(),
            },
            Cue {
                category: ClaimCategory::NoveltyClaim,
                confidence: 0.85,
                re: Regex::new(r"(?i)\bnovel finding\b").unwrap(),
            },
            Cue {
                category: ClaimCategory::NoveltyClaim,
                confidence: 0.9,
                re: Regex::new(r"(?i)\bthis is the first\b").unwrap(),
            },
            // Future work claims
            Cue {
                category: ClaimCategory::FutureWorkClaim,
                confidence: 0.9,
                re: Regex::new(r"(?i)\bfuture (studies|research|work)\b").unwrap(),
            },
            Cue {
                category: ClaimCategory::FutureWorkClaim,
                confidence: 0.9,
                re: Regex::new(r"(?i)\bfurther research\b").unwrap(),
            },
            // Limitation claims
            Cue {
                category: ClaimCategory::LimitationClaim,
                confidence: 0.85,
                re: Regex::new(r"(?i)\blimitation\b").unwrap(),
            },
            Cue {
                category: ClaimCategory::LimitationClaim,
                confidence: 0.85,
                re: Regex::new(r"(?i)\blimited by\b").unwrap(),
            },
            Cue {
                category: ClaimCategory::LimitationClaim,
                confidence: 0.75,
                re: Regex::new(r"(?i)\bcaution\b").unwrap(),
            },
        ],
    })
}

/// Extract claims from a fully populated `ExtractionResult`.
///
/// This is Stage 1: deterministic, local, no AI. It never fails the pipeline;
/// on any internal error it returns whatever claims it managed to collect.
pub fn extract_claims(result: &ExtractionResult) -> Vec<ScientificClaim> {
    let mut claims = Vec::new();
    let mut next_id = 0usize;

    // Pre-index stats and citations by location for O(1) lookup per claim.
    let stat_index = index_stats_by_location(result);
    let cite_index = index_citations_by_location(result);

    for section in &result.sections {
        if section.kind == SectionKind::References {
            continue;
        }

        for (p_idx, paragraph) in section.paragraphs.iter().enumerate() {
            let loc = Location { section: section.kind, paragraph: p_idx };
            let sentences = sentence::sentences_in(paragraph);

            for (s_idx, sentence) in sentences.iter().enumerate() {
                let matches = find_cues(sentence);
                if matches.is_empty() {
                    continue;
                }

                // Slice the sentence into one claim text per cue.
                for i in 0..matches.len() {
                    let start = matches[i].start;
                    let end = matches.get(i + 1).map(|m| m.start).unwrap_or(sentence.len());
                    let text = sentence[start..end].trim();
                    if text.len() < 10 {
                        continue;
                    }
                    let bounded = truncate_statement(text);

                    next_id += 1;
                    claims.push(ScientificClaim {
                        id: ClaimId(format!("claim-{next_id}")),
                        category: matches[i].category,
                        statement: bounded.to_string(),
                        normalized: None,
                        nature: infer_nature(bounded),
                        source_span: SourceSpan::Point(loc.clone()),
                        sentence_indices: vec![s_idx],
                        confidence: matches[i].confidence,
                        source: ClaimSource::Deterministic,
                        evidence: Vec::new(),
                        statistics: associated_stats(&stat_index, &loc, bounded, result),
                        citations: associated_citations(&cite_index, &loc, bounded, result),
                        variables: Vec::new(),
                        methods: Vec::new(),
                        datasets: Vec::new(),
                        verification_status: ClaimVerificationStatus::Unverified,
                        reviewer_notes: Vec::new(),
                        novelty_links: Vec::new(),
                    });
                }
            }
        }
    }

    claims
}

/// Stage 2 placeholder: optional local-SLM refinement of candidate claims.
///
/// Receives only candidate paragraphs, never the full manuscript.
#[allow(dead_code)]
pub fn extract_claims_local_slm(
    _result: &ExtractionResult,
    _candidates: &[ScientificClaim],
) -> Result<Vec<ScientificClaim>, crate::GaplyError> {
    // Reserved for Milestone A, Step 4+.
    Ok(Vec::new())
}

/// Stage 3 placeholder: optional cloud refinement of a single claim.
///
/// Receives only the candidate claim, scientific metadata, section, and nearby
/// statistics/citations — never the full manuscript.
#[allow(dead_code)]
pub fn extract_claims_cloud(
    _candidate: &ScientificClaim,
    _section: SectionKind,
) -> Result<ScientificClaim, crate::GaplyError> {
    // Reserved for a future cloud-assisted refinement task.
    Err(crate::GaplyError::Internal(
        "cloud claim refinement is not implemented in this milestone".into(),
    ))
}

/// A cue match inside a sentence.
struct CueMatch {
    category: ClaimCategory,
    confidence: f64,
    start: usize,
    end: usize,
}

/// Find all non-overlapping cue matches in a sentence, sorted by start offset.
fn find_cues(sentence: &str) -> Vec<CueMatch> {
    let mut raw: Vec<CueMatch> = Vec::new();
    for cue in &cue_set().cues {
        for m in cue.re.find_iter(sentence) {
            raw.push(CueMatch {
                category: cue.category,
                confidence: cue.confidence,
                start: m.start(),
                end: m.end(),
            });
        }
    }

    // Sort by start, then by descending confidence, then by shorter match first.
    raw.sort_by(|a, b| {
        a.start
            .cmp(&b.start)
            .then_with(|| b.confidence.partial_cmp(&a.confidence).unwrap_or(std::cmp::Ordering::Equal))
            .then_with(|| a.end.cmp(&b.end))
    });

    // Drop overlapping matches, keeping the higher-confidence (earlier) one.
    let mut kept: Vec<CueMatch> = Vec::new();
    for m in raw {
        if let Some(last) = kept.last() {
            if m.start < last.end {
                continue; // overlaps a stronger cue
            }
        }
        kept.push(m);
    }

    kept
}

fn truncate_statement(text: &str) -> &str {
    if text.len() <= MAX_STATEMENT_LEN {
        text
    } else {
        // Snap back to the last whitespace so we do not cut a word.
        let prefix = &text[..MAX_STATEMENT_LEN];
        match prefix.rfind(|c: char| c.is_whitespace()) {
            Some(i) if i > 0 => &text[..i],
            _ => prefix,
        }
    }
}

fn infer_nature(text: &str) -> ClaimNature {
    let lower = text.to_lowercase();
    if lower.contains("cause") || lower.contains("effect") || lower.contains("affect")
        || lower.contains("increase") || lower.contains("decrease") || lower.contains("reduce")
        || lower.contains("improve") || lower.contains("worsen")
    {
        ClaimNature::Causal
    } else if lower.contains("associated") || lower.contains("correlation")
        || lower.contains("related to") || lower.contains("linked to")
    {
        ClaimNature::Associational
    } else if lower.contains("compared") || lower.contains("higher than")
        || lower.contains("lower than") || lower.contains("greater than")
    {
        ClaimNature::Comparative
    } else {
        ClaimNature::Descriptive
    }
}

fn index_stats_by_location(result: &ExtractionResult) -> HashMap<(SectionKind, usize), Vec<usize>> {
    let mut map: HashMap<(SectionKind, usize), Vec<usize>> = HashMap::new();
    for (i, claim) in result.statistics.iter().enumerate() {
        map.entry((claim.location.section, claim.location.paragraph))
            .or_default()
            .push(i);
    }
    map
}

fn index_citations_by_location(
    result: &ExtractionResult,
) -> HashMap<(SectionKind, usize), Vec<usize>> {
    let mut map: HashMap<(SectionKind, usize), Vec<usize>> = HashMap::new();
    for (i, cite) in result.citations.iter().enumerate() {
        map.entry((cite.location.section, cite.location.paragraph))
            .or_default()
            .push(i);
    }
    map
}

fn associated_stats(
    index: &HashMap<(SectionKind, usize), Vec<usize>>,
    loc: &Location,
    claim_text: &str,
    result: &ExtractionResult,
) -> Vec<StatRef> {
    let mut out = Vec::new();
    let claim_lower = claim_text.to_lowercase();
    if let Some(indices) = index.get(&(loc.section, loc.paragraph)) {
        for &i in indices {
            let raw = result.statistics[i].stat.raw().to_lowercase();
            if claim_lower.contains(&raw) {
                out.push(StatRef {
                    index: i,
                    location: result.statistics[i].location.clone(),
                });
            }
        }
    }
    out
}

fn associated_citations(
    index: &HashMap<(SectionKind, usize), Vec<usize>>,
    loc: &Location,
    claim_text: &str,
    result: &ExtractionResult,
) -> Vec<CitationRef> {
    let mut out = Vec::new();
    let claim_lower = claim_text.to_lowercase();
    if let Some(indices) = index.get(&(loc.section, loc.paragraph)) {
        for &i in indices {
            let raw = result.citations[i].raw.to_lowercase();
            if claim_lower.contains(&raw) {
                out.push(CitationRef(i));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::extract::extract_from_text;

    #[test]
    fn no_claims_in_generic_text() {
        let text = "The weather was pleasant. The conference took place in June.";
        let ex = extract_from_text(text);
        let claims = extract_claims(&ex);
        assert!(claims.is_empty(), "generic prose must yield no claims");
    }

    #[test]
    fn detects_main_claim_in_abstract() {
        let text = "\
A Study\n\n\
Abstract\n\
This study demonstrates that sleep improves memory consolidation in older adults.\n\
";
        let ex = extract_from_text(text);
        let claims = extract_claims(&ex);
        let main = claims
            .iter()
            .find(|c| matches!(c.category, ClaimCategory::MainClaim));
        assert!(main.is_some(), "expected a main claim; got {:?}", claims);
        assert!(main.unwrap().statement.contains("sleep improves memory"));
    }

    #[test]
    fn detects_result_claim_and_associates_statistics() {
        let text = "\
A Study\n\nAbstract\nA trial.\n\nResults\nWe found that treatment improved recall (p < 0.001, n = 120).\n";
        let ex = extract_from_text(text);
        let claims = extract_claims(&ex);
        let result = claims
            .iter()
            .find(|c| matches!(c.category, ClaimCategory::ResultClaim));
        assert!(result.is_some(), "expected a result claim; got {:?}", claims);
        let result = result.unwrap();
        assert!(!result.statistics.is_empty(), "result claim should link its statistics");
    }

    #[test]
    fn multiple_claims_per_sentence() {
        let text = "\nA Study\n\nIntroduction\nWe hypothesized that X affects Y, and we found that Z mediates the relationship.\n";
        let ex = extract_from_text(text);
        let claims = extract_claims(&ex);
        let has_hypothesis = claims
            .iter()
            .any(|c| matches!(c.category, ClaimCategory::HypothesisClaim));
        let has_result = claims
            .iter()
            .any(|c| matches!(c.category, ClaimCategory::ResultClaim));
        assert!(has_hypothesis, "expected a hypothesis claim");
        assert!(has_result, "expected a result claim");
    }

    #[test]
    fn detects_limitation_claim() {
        let text = "\nA Study\n\nDiscussion\nA limitation of this study is the small sample size.\n";
        let ex = extract_from_text(text);
        let claims = extract_claims(&ex);
        assert!(claims.iter().any(|c| matches!(c.category, ClaimCategory::LimitationClaim)));
    }

    #[test]
    fn detects_novelty_and_future_work() {
        let text = "\nA Study\n\nDiscussion\nThis is the first study to show X. Future research should examine Y.\n";
        let ex = extract_from_text(text);
        let claims = extract_claims(&ex);
        assert!(claims.iter().any(|c| matches!(c.category, ClaimCategory::NoveltyClaim)));
        assert!(claims.iter().any(|c| matches!(c.category, ClaimCategory::FutureWorkClaim)));
    }

    #[test]
    fn claims_reference_local_spans_not_full_text() {
        let text = "\nA Study\n\nResults\nWe found that X increased (p = 0.02).\n";
        let ex = extract_from_text(text);
        let claims = extract_claims(&ex);
        assert!(!claims.is_empty());
        for claim in &claims {
            assert!(
                !claim.statement.contains("\n\n"),
                "claim statement must not contain paragraph breaks"
            );
        }
    }

    #[test]
    fn large_manuscript_scales_without_panic() {
        let mut paragraphs = vec!["A Study".to_string(), "Abstract".to_string()];
        for i in 0..500 {
            paragraphs.push(format!(
                "Results\nWe found that intervention {i} improved outcome {i} (p < 0.05)."
            ));
        }
        let text = paragraphs.join("\n\n");
        let ex = extract_from_text(&text);
        let claims = extract_claims(&ex);
        assert!(!claims.is_empty());
        // Deterministic extraction should remain fast for a few hundred paragraphs.
        assert!(claims.len() >= 400);
    }

    #[test]
    fn claim_id_uniqueness() {
        let text = "\nA Study\n\nResults\nWe found that A increased. We found that B decreased.\n";
        let ex = extract_from_text(text);
        let claims = extract_claims(&ex);
        let ids: std::collections::HashSet<_> = claims.iter().map(|c| &c.id.0).collect();
        assert_eq!(ids.len(), claims.len(), "claim ids must be unique");
    }
}
