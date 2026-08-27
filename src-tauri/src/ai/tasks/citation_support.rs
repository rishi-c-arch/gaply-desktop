//! Spec Prompt 2 — Citation Support Checker. "Does the cited paper actually
//! say this?"
//!
//! The first EVIDENCE-GROUNDED task: it reads retrieved chunks from one cited
//! source and judges whether they support the author's claim. Unlike
//! citation_need, every field it produces must trace to a chunk that was
//! actually sent — so the spec's four shared rules all apply here, and
//! `TaskContext`'s generic chunk-id check does real work.
//!
//! # The dangerous output is a false "strong"
//!
//! The spec is explicit that this task is "deliberately conservative" and that
//! "partial topical overlap is NOT support". A wrong `weak` costs the author a
//! second look; a wrong `strong` tells them a citation is sound when it is not,
//! and that is the failure that reaches a thesis. The validator is asymmetric
//! for that reason: `strong` must be earned against `claim_elements`, while the
//! cautious verdicts are not second-guessed.
//!
//! # v2-style layout
//!
//! Phase 4b measured that this model classifies whatever appears FIRST. So the
//! rules and schema come first, evidence next, and the CLAIM UNDER TEST last,
//! immediately before the output instruction.

use serde::{Deserialize, Serialize};

use crate::ai::task::{AiTask, TaskContext, ValidationError};

pub const PROMPT_VERSION: &str = "citation_support-v1";

/// Spec: `max_tokens: 400`.
const MAX_TOKENS: usize = 400;

/// Spec: `"why" <= 20 words`.
const MAX_WHY_WORDS: usize = 20;
/// Not in the spec as a number; a rationale far past this is untidy, not wrong.
const MAX_EXPLANATION_WORDS: usize = 120;
/// The spec names five checkable element kinds (subject, direction, magnitude,
/// population, condition). Outside this range the decomposition is not useful.
const MIN_CLAIM_ELEMENTS: usize = 1;
const MAX_CLAIM_ELEMENTS: usize = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Verdict {
    Strong,
    Partial,
    Weak,
    Contradicts,
    InsufficientEvidence,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ElementStatus {
    Found,
    Absent,
    Different,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SupportingChunk {
    pub chunk_id: String,
    /// Echoed back by the model and checked against the STORE (plan §11 D16).
    pub page: Option<u32>,
    pub why: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClaimElement {
    pub element: String,
    pub status: ElementStatus,
}

/// Exactly the spec's OUTPUT SCHEMA.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CitationSupportOutput {
    pub verdict: Verdict,
    pub confidence: f64,
    #[serde(default)]
    pub supporting_chunks: Vec<SupportingChunk>,
    #[serde(default)]
    pub claim_elements: Vec<ClaimElement>,
    pub explanation: String,
    #[serde(default)]
    pub suggested_rewrite: Option<String>,
}

pub struct CitationSupportTask {
    pub claim: String,
    /// The spec's `<cited_source>` line: "{author} ({year}) — {title}".
    pub cited_source: String,
    /// Pre-rendered `<evidence>` block, in the spec's chunk format.
    pub evidence: String,
}

const SYSTEM: &str = "You are a citation verification engine for academic writing.
You judge ONE thing: does the evidence from the cited source support the author's claim?
You are deliberately conservative. Partial topical overlap is NOT support.
A source that discusses the same topic but does not report the claimed finding is \"weak\".
You may only use text inside <evidence>. You have no other knowledge.
Every chunk_id you output must be one that appears in <evidence>. Never invent one.
If the evidence does not cover the claim, say so — abstaining is correct, not failure.
Output raw JSON only. No markdown, no code fences, no commentary.";

const OUTPUT_SCHEMA: &str = r#"{
  "verdict": "strong|partial|weak|contradicts|insufficient_evidence",
  "confidence": 0.0-1.0,
  "supporting_chunks": [{"chunk_id": string, "page": integer, "why": string}],
  "claim_elements": [
    {"element": string, "status": "found|absent|different"}
  ],
  "explanation": string,
  "suggested_rewrite": string|null
}"#;

const RULES: &str = r#"- Decompose the claim into its checkable elements first (subject, direction of effect,
  magnitude, population, condition) and fill claim_elements before choosing a verdict.
- verdict mapping:
    strong    = every element found in evidence
    partial   = direction/subject found, but magnitude, population or condition differs
    weak      = topic present, claimed finding absent
    contradicts = evidence states the opposite direction or a null result
    insufficient_evidence = retrieved chunks do not cover the claim's topic at all
- "why" <= 20 words, must paraphrase the chunk, never quote more than 10 words.
- suggested_rewrite: only for "partial" - rewrite the author's sentence so it becomes
  accurate for this source. null for every other verdict.
- Never say a claim is supported because it is plausible or well known."#;

/* ------------------- how this task maps spec rules to tiers ---------------- *
 * Beside the validator so the mapping is reviewable (plan §9.11). THE SPEC'S
 * PROMPT TEXT IS UNCHANGED — every rule is still stated with equal force. These
 * decide only what the ENGINE does when the model misses.
 *
 * The asymmetry is deliberate: anything that could make a citation look sound
 * when it is not is Fatal; anything that only makes the output untidy is
 * Advisory. `every_rule_has_a_declared_tier` keeps these honest. */

/// Violations that could make an unsound citation look sound, or point a reader
/// at text that does not exist. One retry, then failure.
pub const FATAL_RULES: &[&str] = &[
    "supporting_chunks references a chunk_id that was not in the evidence sent",
    "supporting_chunks page does not match the page stored for that chunk",
    "supporting_chunks page is non-null for a chunk whose stored page is unknown",
    "verdict 'strong' while some claim_element is not 'found'",
    "verdict 'contradicts' with no element marked 'different'",
    "suggested_rewrite is non-null when verdict is not 'partial'",
    "suggested_rewrite is null when verdict IS 'partial'",
    "confidence outside 0.0-1.0",
    "verdict or element status outside the spec's enums (rejected by serde at parse)",
    "a verdict other than insufficient_evidence with no supporting_chunks",
];

/// Violations that make an output untidy. Accepted; reported as advisories.
pub const ADVISORY_RULES: &[&str] = &[
    "a 'why' longer than 20 words",
    "an explanation longer than 120 words",
    "claim_elements count outside 1-8",
];

impl AiTask for CitationSupportTask {
    type Output = CitationSupportOutput;

    fn prompt_version(&self) -> &'static str {
        PROMPT_VERSION
    }

    fn max_tokens() -> usize {
        MAX_TOKENS
    }

    fn build_prompt(&self) -> String {
        // Rules and schema first, evidence next, the CLAIM LAST — Phase 4b
        // measured that this model judges whatever it read first.
        format!(
            "<|im_start|>system\n{SYSTEM}\n<|im_end|>\n\
             <|im_start|>user\n\
             OUTPUT SCHEMA\n{OUTPUT_SCHEMA}\n\n\
             RULES\n{RULES}\n\n\
             <cited_source>\n{source}\n</cited_source>\n\n\
             {evidence}\n\n\
             CLAIM UNDER TEST\n<claim>\n{claim}\n</claim>\n\n\
             Judge ONLY the claim above against the evidence above. \
             Output the JSON object now, beginning with {{\n\
             <|im_end|>\n\
             <|im_start|>assistant\n",
            source = self.cited_source,
            evidence = self.evidence,
            claim = self.claim,
        )
    }

    fn validate(out: &Self::Output, ctx: &TaskContext) -> Result<(), Vec<ValidationError>> {
        let mut errors = Vec::new();

        if !(0.0..=1.0).contains(&out.confidence) {
            errors.push(ValidationError::fatal(
                "confidence",
                format!("is {}; the spec requires 0.0-1.0", out.confidence),
            ));
        }

        // --- grounding: every cited chunk was sent, and its page matches ---
        for (i, sc) in out.supporting_chunks.iter().enumerate() {
            let field = format!("supporting_chunks[{i}].chunk_id");
            match ctx.get(&sc.chunk_id) {
                None => {
                    // The generic Phase 3 check — an id we never sent.
                    if let Err(e) = ctx.require_known_chunk(&sc.chunk_id, &field) {
                        errors.push(e);
                    }
                }
                Some(sent) => {
                    // D16: the page is checked against the STORE, not for
                    // internal consistency. A card claiming page 8 for a chunk
                    // stored on page 12 sends a reader to the wrong page.
                    match (sent.page, sc.page) {
                        (Some(actual), Some(claimed)) if actual != claimed => {
                            errors.push(ValidationError::fatal(
                                format!("supporting_chunks[{i}].page"),
                                format!(
                                    "says page {claimed}, but chunk {} is stored on page {actual}",
                                    sc.chunk_id
                                ),
                            ));
                        }
                        (None, Some(claimed)) => {
                            errors.push(ValidationError::fatal(
                                format!("supporting_chunks[{i}].page"),
                                format!(
                                    "says page {claimed}, but chunk {} has no recorded page — \
                                     an unverifiable page must not be asserted",
                                    sc.chunk_id
                                ),
                            ));
                        }
                        _ => {}
                    }
                }
            }
            let why_words = sc.why.split_whitespace().count();
            if why_words > MAX_WHY_WORDS {
                errors.push(ValidationError::advisory(
                    format!("supporting_chunks[{i}].why"),
                    format!("is {why_words} words; the limit is {MAX_WHY_WORDS}"),
                ));
            }
        }

        // --- verdict must be earned against claim_elements ---
        let all_found = out
            .claim_elements
            .iter()
            .all(|e| e.status == ElementStatus::Found);
        let any_different = out
            .claim_elements
            .iter()
            .any(|e| e.status == ElementStatus::Different);

        match out.verdict {
            // THE dangerous output. "strong = every element found in evidence".
            Verdict::Strong if !out.claim_elements.is_empty() && !all_found => {
                let unmet: Vec<&str> = out
                    .claim_elements
                    .iter()
                    .filter(|e| e.status != ElementStatus::Found)
                    .map(|e| e.element.as_str())
                    .collect();
                errors.push(ValidationError::fatal(
                    "verdict",
                    format!(
                        "is 'strong', but these claim elements are not 'found': {}. \
                         The spec defines strong as every element found — a false 'strong' \
                         tells an author a citation is sound when it is not",
                        unmet.join(", ")
                    ),
                ));
            }
            Verdict::Strong if out.claim_elements.is_empty() => {
                errors.push(ValidationError::fatal(
                    "verdict",
                    "is 'strong' but claim_elements is empty — the spec requires the claim to be \
                     decomposed and checked BEFORE a verdict is chosen",
                ));
            }
            Verdict::Contradicts if !out.claim_elements.is_empty() && !any_different => {
                errors.push(ValidationError::fatal(
                    "verdict",
                    "is 'contradicts' but no claim element is marked 'different' — a \
                     contradiction must be visible in the decomposition",
                ));
            }
            _ => {}
        }

        // --- suggested_rewrite is exclusively partial's ---
        match (out.verdict, out.suggested_rewrite.as_deref()) {
            (Verdict::Partial, None) | (Verdict::Partial, Some("")) => {
                errors.push(ValidationError::fatal(
                    "suggested_rewrite",
                    "is required when the verdict is 'partial' — the spec asks for the author's \
                     sentence rewritten so it becomes accurate for this source",
                ));
            }
            (v, Some(r)) if v != Verdict::Partial && !r.trim().is_empty() => {
                errors.push(ValidationError::fatal(
                    "suggested_rewrite",
                    format!("must be null when the verdict is '{}'", verdict_name(v)),
                ));
            }
            _ => {}
        }

        // --- a supported verdict must point at something ---
        if out.verdict != Verdict::InsufficientEvidence && out.supporting_chunks.is_empty() {
            errors.push(ValidationError::fatal(
                "supporting_chunks",
                format!(
                    "is empty for verdict '{}' — a judgement about the evidence must say WHICH \
                     evidence; only insufficient_evidence may cite nothing",
                    verdict_name(out.verdict)
                ),
            ));
        }

        // --- advisories ---
        let expl_words = out.explanation.split_whitespace().count();
        if expl_words > MAX_EXPLANATION_WORDS {
            errors.push(ValidationError::advisory(
                "explanation",
                format!("is {expl_words} words; the limit is {MAX_EXPLANATION_WORDS}"),
            ));
        }
        let n = out.claim_elements.len();
        if !(MIN_CLAIM_ELEMENTS..=MAX_CLAIM_ELEMENTS).contains(&n) {
            errors.push(ValidationError::advisory(
                "claim_elements",
                format!("has {n} entries; {MIN_CLAIM_ELEMENTS}-{MAX_CLAIM_ELEMENTS} is the useful range"),
            ));
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}

fn verdict_name(v: Verdict) -> &'static str {
    match v {
        Verdict::Strong => "strong",
        Verdict::Partial => "partial",
        Verdict::Weak => "weak",
        Verdict::Contradicts => "contradicts",
        Verdict::InsufficientEvidence => "insufficient_evidence",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::task::{EvidenceChunk, Tier};

    fn ctx() -> TaskContext {
        TaskContext::new(vec![
            EvidenceChunk {
                chunk_id: "c1".into(),
                page: Some(8),
                section: Some("Results".into()),
                text: "Species richness rose 31% under organic management (p<0.01).".into(),
            },
            EvidenceChunk {
                chunk_id: "c2".into(),
                page: None,
                section: None,
                text: "No significant effect was observed for soil fauna.".into(),
            },
        ])
    }

    fn strong() -> CitationSupportOutput {
        CitationSupportOutput {
            verdict: Verdict::Strong,
            confidence: 0.9,
            supporting_chunks: vec![SupportingChunk {
                chunk_id: "c1".into(),
                page: Some(8),
                why: "Reports the richness increase the claim describes.".into(),
            }],
            claim_elements: vec![
                ClaimElement { element: "subject: organic management".into(), status: ElementStatus::Found },
                ClaimElement { element: "direction: increase".into(), status: ElementStatus::Found },
            ],
            explanation: "The evidence reports the same finding.".into(),
            suggested_rewrite: None,
        }
    }

    fn tier_of(out: &CitationSupportOutput, field_prefix: &str) -> Option<Tier> {
        CitationSupportTask::validate(out, &ctx())
            .err()?
            .into_iter()
            .find(|e| e.field.starts_with(field_prefix))
            .map(|e| e.tier)
    }

    #[test]
    fn a_well_formed_strong_verdict_passes() {
        assert!(CitationSupportTask::validate(&strong(), &ctx()).is_ok());
    }

    #[test]
    fn an_invented_chunk_id_is_fatal() {
        let mut o = strong();
        o.supporting_chunks[0].chunk_id = "c99".into();
        let errors = CitationSupportTask::validate(&o, &ctx()).unwrap_err();
        let e = errors.iter().find(|e| e.field.contains("chunk_id")).expect("flagged");
        assert!(e.is_fatal());
        assert!(e.problem.contains("c99") && e.problem.contains("c1, c2"), "{e}");
    }

    #[test]
    fn a_page_that_disagrees_with_the_store_is_fatal() {
        // D16: c1 is stored on page 8; claiming 12 would send a reader to the
        // wrong page of a real PDF.
        let mut o = strong();
        o.supporting_chunks[0].page = Some(12);
        let e = CitationSupportTask::validate(&o, &ctx())
            .unwrap_err()
            .into_iter()
            .find(|e| e.field.contains("page"))
            .expect("flagged");
        assert!(e.is_fatal());
        assert!(e.problem.contains("stored on page 8"), "{e}");
    }

    #[test]
    fn a_page_asserted_for_a_chunk_with_no_recorded_page_is_fatal() {
        let mut o = strong();
        o.supporting_chunks[0] = SupportingChunk {
            chunk_id: "c2".into(),
            page: Some(3),
            why: "Reports a null effect.".into(),
        };
        let e = CitationSupportTask::validate(&o, &ctx())
            .unwrap_err()
            .into_iter()
            .find(|e| e.field.contains("page"))
            .expect("flagged");
        assert!(e.is_fatal());
        assert!(e.problem.contains("no recorded page"), "{e}");
    }

    #[test]
    fn a_null_page_for_an_unpaginated_chunk_is_fine() {
        let mut o = strong();
        o.supporting_chunks[0] =
            SupportingChunk { chunk_id: "c2".into(), page: None, why: "Null effect.".into() };
        assert!(CitationSupportTask::validate(&o, &ctx()).is_ok());
    }

    #[test]
    fn strong_without_every_element_found_is_fatal_and_names_the_unmet_ones() {
        // THE dangerous output: a false "strong" tells an author a citation is
        // sound when it is not.
        let mut o = strong();
        o.claim_elements[1].status = ElementStatus::Different;
        let e = CitationSupportTask::validate(&o, &ctx())
            .unwrap_err()
            .into_iter()
            .find(|e| e.field == "verdict")
            .expect("flagged");
        assert!(e.is_fatal());
        assert!(e.problem.contains("direction: increase"), "must name what was unmet: {e}");
    }

    #[test]
    fn strong_with_no_decomposition_at_all_is_fatal() {
        let mut o = strong();
        o.claim_elements.clear();
        assert_eq!(tier_of(&o, "verdict"), Some(Tier::Fatal));
    }

    #[test]
    fn contradicts_needs_a_different_element() {
        let mut o = strong();
        o.verdict = Verdict::Contradicts;
        assert_eq!(tier_of(&o, "verdict"), Some(Tier::Fatal));
        o.claim_elements[0].status = ElementStatus::Different;
        assert!(
            CitationSupportTask::validate(&o, &ctx())
                .err()
                .map(|e| !e.iter().any(|x| x.field == "verdict"))
                .unwrap_or(true),
            "a contradiction visible in the decomposition must be accepted"
        );
    }

    #[test]
    fn suggested_rewrite_belongs_only_to_partial() {
        let mut o = strong();
        o.suggested_rewrite = Some("Rewritten.".into());
        assert_eq!(tier_of(&o, "suggested_rewrite"), Some(Tier::Fatal));

        // ... and is REQUIRED for partial
        let mut o = strong();
        o.verdict = Verdict::Partial;
        o.claim_elements[1].status = ElementStatus::Different;
        o.suggested_rewrite = None;
        assert_eq!(tier_of(&o, "suggested_rewrite"), Some(Tier::Fatal));

        o.suggested_rewrite = Some("Organic management increased richness in this reservoir.".into());
        assert!(CitationSupportTask::validate(&o, &ctx()).is_ok());
    }

    #[test]
    fn a_verdict_other_than_insufficient_must_cite_something() {
        let mut o = strong();
        o.verdict = Verdict::Weak;
        o.supporting_chunks.clear();
        assert_eq!(tier_of(&o, "supporting_chunks"), Some(Tier::Fatal));

        // insufficient_evidence may cite nothing — that is its whole meaning.
        let mut o = strong();
        o.verdict = Verdict::InsufficientEvidence;
        o.supporting_chunks.clear();
        o.claim_elements[0].status = ElementStatus::Absent;
        o.claim_elements[1].status = ElementStatus::Absent;
        assert!(CitationSupportTask::validate(&o, &ctx()).is_ok());
    }

    #[test]
    fn confidence_outside_zero_to_one_is_fatal() {
        let mut o = strong();
        o.confidence = 1.4;
        assert_eq!(tier_of(&o, "confidence"), Some(Tier::Fatal));
    }

    #[test]
    fn every_rule_has_a_declared_tier_and_the_validator_agrees() {
        // ADVISORY: a long "why" is untidy, not misleading.
        let mut o = strong();
        o.supporting_chunks[0].why = (0..25).map(|i| format!("w{i}")).collect::<Vec<_>>().join(" ");
        assert_eq!(tier_of(&o, "supporting_chunks[0].why"), Some(Tier::Advisory));

        let mut o = strong();
        o.explanation = (0..130).map(|i| format!("w{i}")).collect::<Vec<_>>().join(" ");
        assert_eq!(tier_of(&o, "explanation"), Some(Tier::Advisory));

        let mut o = strong();
        o.claim_elements = (0..9)
            .map(|i| ClaimElement { element: format!("e{i}"), status: ElementStatus::Found })
            .collect();
        assert_eq!(tier_of(&o, "claim_elements"), Some(Tier::Advisory));

        assert_eq!(FATAL_RULES.len(), 10);
        assert_eq!(ADVISORY_RULES.len(), 3);
    }

    #[test]
    fn out_of_range_enums_are_rejected_at_parse_time() {
        let bad = r#"{"verdict":"supported","confidence":0.5,"supporting_chunks":[],
                      "claim_elements":[],"explanation":"x","suggested_rewrite":null}"#;
        assert!(serde_json::from_str::<CitationSupportOutput>(bad).is_err());
        let bad_status = r#"{"verdict":"strong","confidence":0.5,"supporting_chunks":[],
                             "claim_elements":[{"element":"e","status":"maybe"}],
                             "explanation":"x","suggested_rewrite":null}"#;
        assert!(serde_json::from_str::<CitationSupportOutput>(bad_status).is_err());
    }

    #[test]
    fn all_five_spec_verdicts_round_trip() {
        for v in ["strong", "partial", "weak", "contradicts", "insufficient_evidence"] {
            let json = format!(
                r#"{{"verdict":"{v}","confidence":0.5,"supporting_chunks":[],
                     "claim_elements":[],"explanation":"x","suggested_rewrite":null}}"#
            );
            let p: CitationSupportOutput =
                serde_json::from_str(&json).unwrap_or_else(|e| panic!("{v}: {e}"));
            assert_eq!(serde_json::to_value(p.verdict).unwrap(), v);
        }
    }

    #[test]
    fn the_prompt_carries_the_spec_blocks_and_puts_the_claim_last() {
        let t = CitationSupportTask {
            claim: "Organic farming increases soil biodiversity by a third.".into(),
            cited_source: "Sharma et al. (2021) — Organic systems and soil life".into(),
            evidence: ctx().render_evidence(),
        };
        let p = t.build_prompt();
        assert!(p.contains("You are a citation verification engine for academic writing."));
        assert!(p.contains("Partial topical overlap is NOT support."));
        assert!(p.contains("strong    = every element found in evidence"));
        assert!(p.contains("\"why\" <= 20 words"));
        assert!(p.contains("Never say a claim is supported because it is plausible"));
        assert!(p.contains("<cited_source>"));
        // the spec's chunk format survives into the prompt
        assert!(p.contains("[c1 | p.8 | Results]"), "evidence not in the spec format: {p}");
        // the claim is LAST (Phase 4b finding)
        let claim_at = p.rfind("Organic farming increases soil biodiversity").unwrap();
        let ev_at = p.rfind("[c1 | p.8 | Results]").unwrap();
        assert!(claim_at > ev_at, "the claim must come after the evidence");
        assert!(p[claim_at..].contains("beginning with {"));
        assert_eq!(CitationSupportTask::max_tokens(), 400, "spec pins max_tokens: 400");
        assert_eq!(t.prompt_version(), "citation_support-v1");
    }
}
