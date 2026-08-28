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

/// Bumped v1 -> v1.1 by §11 D26: the evidence header rendering changed, so a
/// report from either side of that change describes a different prompt.
pub const PROMPT_VERSION: &str = "citation_support-v1.3";

/// SPEC OVERRIDE — §11 D27. The spec pins `max_tokens: 400`; measurement
/// retired it.
///
/// Across the Phase 6 support cells, 13 of 28 first attempts stopped EXACTLY at
/// their ceiling — every 0.5B v2 attempt, and 4 of 5 1.5B v1 attempts. A reply
/// cut off mid-JSON is unparseable, and unparseable scored identically to
/// wrong, so the arm was measuring the ceiling rather than the models.
///
/// 768 is derived, not guessed: the largest COMPLETE v1 first attempt observed
/// was 309 tokens and the schema's own limits put a realistic three-chunk reply
/// near 420. It is NOT provisioned for the schema-legal maximum — with
/// `supporting_chunks` uncapped, twelve chunks would need ~1400 tokens, and
/// `TASK_N_CTX - max_tokens` would then leave less prompt budget than the
/// prompts actually measure.
const MAX_TOKENS: usize = 768;

/// Spec: `"why" <= 20 words`.
const MAX_WHY_WORDS: usize = 20;

/// SPEC ADDITION — §11 D32. The spec puts no ceiling on `supporting_chunks`.
///
/// Measured consequence: the 0.5B truncated 4 of 6 seeds at a 1024-token
/// ceiling by citing five, eight, twelve chunks with a full `why` on each. That
/// is not a length problem — raising the ceiling twice did not fix it — it is
/// unbounded enumeration, and the cap is the fix that bounds output size BY
/// CONSTRUCTION rather than by giving the pathology more room.
///
/// FATAL above this, not advisory: a citation card listing a dozen chunks is
/// not untidy, it is a claim that twelve passages support the sentence, and a
/// reader cannot check that. Four is enough to carry a real multi-passage
/// justification and few enough to read.
///
/// THE PROMPT STATES THE BOUND AND NOTHING ELSE (§11 D33). It briefly also said
/// "Cite only the chunks that actually carry the point. Listing every chunk you
/// were given is not evidence." Decoding is greedy, so that sentence was
/// measured — it moved the 3B off the planted chunk on two of four seeds
/// (planted-chunk citation 75% -> 25%) while every other metric held still. Do
/// not reintroduce guidance here: the bound belongs in the validator, and the
/// prompt's job is to state it, not to argue for it.
const MAX_SUPPORTING_CHUNKS: usize = 4;
/// v2's quote ceiling. Long enough to carry a finding, short enough that
/// "quote the whole chunk" is not a way to pass the check without reading.
const MAX_QUOTE_WORDS: usize = 25;
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
    /// v2 ONLY: a verbatim substring of the cited chunk (§11 D24).
    ///
    /// `Option` because v1 does not ask for it and must keep parsing. v2's
    /// validator requires it to be present AND to actually occur in the chunk
    /// text that was sent — that check is the whole point of the variant.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quote: Option<String>,
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
Each evidence line begins with a header: [CHUNK_ID=<id> PAGE=<n> SECTION=<name>].
Every chunk_id you output must be the CHUNK_ID VALUE ALONE - write \"c1\", never
\"c1 | p.8 | Results\" and never the whole bracket. It must be an id that appears
in <evidence>. Never invent one.
If the evidence does not cover the claim, say so — abstaining is correct, not failure.
Output raw JSON only. No markdown, no code fences, no commentary.";

const OUTPUT_SCHEMA: &str = r#"{
  "verdict": "strong|partial|weak|contradicts|insufficient_evidence",
  "confidence": 0.0-1.0,
  "supporting_chunks": [{"chunk_id": string, "page": integer, "why": string}],   // at most 4
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
- supporting_chunks: AT MOST 4 entries.
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
    "more than 4 supporting_chunks",
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
        validate_support(out, ctx, false)
    }
}

/// Shared validator. `require_quote` is the ONLY difference between v1 and v2 —
/// keeping one body means the two variants cannot drift apart on the rules they
/// are supposed to share, which is what makes the bake-off comparison mean
/// anything.
fn validate_support(
    out: &CitationSupportOutput,
    ctx: &TaskContext,
    require_quote: bool,
) -> Result<(), Vec<ValidationError>> {
    {
        let mut errors = Vec::new();

        if !(0.0..=1.0).contains(&out.confidence) {
            errors.push(ValidationError::fatal(
                "confidence",
                format!("is {}; the spec requires 0.0-1.0", out.confidence),
            ));
        }

        // --- §11 D32: the citation list is bounded ---
        if out.supporting_chunks.len() > MAX_SUPPORTING_CHUNKS {
            errors.push(ValidationError::fatal(
                "supporting_chunks".to_string(),
                format!(
                    "has {} entries; at most {MAX_SUPPORTING_CHUNKS} may be cited. Cite the chunks \
                     that carry the point, not every chunk that was sent",
                    out.supporting_chunks.len()
                ),
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

            // --- v2: the quote must actually be IN the chunk (§11 D24) ---
            //
            // This is the first check in the engine that ties PROSE to the
            // evidence rather than an identifier. D18 recorded that a model can
            // cite a real chunk, get its page right, and still describe it as
            // saying the opposite of what it says. A quote cannot do that and
            // survive: either the words are in the chunk or they are not.
            if require_quote {
                let field = format!("supporting_chunks[{i}].quote");
                match sc.quote.as_deref().map(str::trim) {
                    None | Some("") => errors.push(ValidationError::fatal(
                        &field,
                        "is missing — v2 requires a verbatim quote from the chunk, because a \
                         chunk_id alone does not show that the model read the text",
                    )),
                    Some(q) => {
                        let words = q.split_whitespace().count();
                        if words > MAX_QUOTE_WORDS {
                            errors.push(ValidationError::advisory(
                                &field,
                                format!("is {words} words; the limit is {MAX_QUOTE_WORDS}"),
                            ));
                        }
                        // Only checkable against a chunk we actually sent. An
                        // unknown chunk_id is already Fatal above; adding a
                        // second error for it would just be noise.
                        if let Some(sent) = ctx.get(&sc.chunk_id) {
                            if !normalize_ws(&sent.text).contains(&normalize_ws(q)) {
                                errors.push(ValidationError::fatal(
                                    &field,
                                    format!(
                                        "is not a verbatim substring of chunk {}. The quote must \
                                         be copied from the evidence, not recalled or paraphrased",
                                        sc.chunk_id
                                    ),
                                ));
                            }
                        }
                    }
                }
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

/* ======================= v2 — the D18 mitigation ========================== *
 * Phase 5 measured that the grounding guarantee covers IDENTIFIERS only: a
 * model can cite a real chunk, report its page correctly, and still describe it
 * as saying the opposite of what it says (D18). Nothing in v1 can catch that,
 * because nothing in v1 ties the model's PROSE to the evidence text.
 *
 * v2 asks for one more field per cited chunk: a verbatim quote. A quote is
 * checkable by string comparison — no entailment model, no second pass. It does
 * not make the explanation faithful, and it is not claimed to; it forces the
 * model to have located the supporting words, and it gives a reader the exact
 * text a verdict rests on.
 *
 * Both variants run in the bake-off. Whether the extra field helps, costs
 * latency, or simply produces a new failure mode is a question for the data. */

/// v2's prompt version. v1's string is untouched, so reports from the two
/// variants can never be conflated.
pub const PROMPT_VERSION_V2: &str = "citation_support-v2.3";

const OUTPUT_SCHEMA_V2: &str = r#"{
  "verdict": "strong|partial|weak|contradicts|insufficient_evidence",
  "confidence": 0.0-1.0,
  "supporting_chunks": [{"chunk_id": string, "page": integer, "quote": string, "why": string}],   // at most 4
  "claim_elements": [
    {"element": string, "status": "found|absent|different"}
  ],
  "explanation": string,
  "suggested_rewrite": string|null
}"#;

const QUOTE_RULE: &str = r#"- "quote" is MANDATORY for every supporting_chunk: copy 5-25 words WORD FOR WORD from
  that chunk's text in <evidence>. Copy, do not summarise, do not correct, do not
  rephrase. It must appear character for character in the chunk. If you cannot find
  words in the chunk that carry the point, that chunk does not support the claim -
  leave it out.
- "why" then explains, in your own words, what the quote shows."#;

/// Same task, same output type, one extra required field.
pub struct CitationSupportV2Task {
    pub claim: String,
    pub cited_source: String,
    pub evidence: String,
}

impl AiTask for CitationSupportV2Task {
    type Output = CitationSupportOutput;

    fn prompt_version(&self) -> &'static str {
        PROMPT_VERSION_V2
    }

    fn max_tokens() -> usize {
        // Larger than v1's: every cited chunk now carries up to 25 more words.
        //
        // §11 D27 raises this 600 -> 1024. D24's 600 was itself a raise, and it
        // was still short: every 0.5B v2 first attempt and one 1.5B v2 attempt
        // stopped exactly at 600, while the largest COMPLETE v2 reply measured
        // 558. 1024 is 1.8x that.
        //
        // The cost is paid in prompt room — `generative.rs` enforces
        // `budget = TASK_N_CTX - max_tokens`, so this leaves 3072 for a prompt
        // that measures 2035-2775 plus ~50 for D26's labelled header.
        1024
    }

    fn build_prompt(&self) -> String {
        format!(
            "<|im_start|>system\n{SYSTEM}\n<|im_end|>\n\
             <|im_start|>user\n\
             OUTPUT SCHEMA\n{OUTPUT_SCHEMA_V2}\n\n\
             RULES\n{RULES}\n{QUOTE_RULE}\n\n\
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
        validate_support(out, ctx, true)
    }
}

/// v2 adds exactly two rules; everything in [`FATAL_RULES`] still applies.
pub const FATAL_RULES_V2: &[&str] = &[
    "a supporting_chunk with no quote",
    "a quote that is not a verbatim substring of that chunk's text (whitespace-normalised)",
];

pub const ADVISORY_RULES_V2: &[&str] = &["a quote longer than 25 words"];

/// Collapse all runs of whitespace to single spaces and trim.
///
/// Whitespace ONLY. A quote check that also normalised case or punctuation
/// would start accepting paraphrases, which is precisely what it exists to
/// reject — the model must have copied the text, not recalled its gist.
fn normalize_ws(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
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
                quote: None,
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

    /// §11 D26. The 3B's actual Phase 6 failure, pinned so the rendering fix
    /// can never be "helped along" by loosening the validator instead. Both
    /// the old display form and the new labelled form are composites, and both
    /// must stay fatal — the fix belongs in what the model is SHOWN, never in
    /// what it is allowed to say.
    #[test]
    fn a_composite_display_chunk_id_is_still_fatal() {
        for composite in [
            "c1 | p.8 | Results",       // the old header, verbatim — what the 3B sent
            "CHUNK_ID=c1 PAGE=8 SECTION=Results", // the new header, verbatim
            "[CHUNK_ID=c1 PAGE=8 SECTION=Results]",
            "CHUNK_ID=c1",              // the key kept, which is still not the id
            "c1 ",                      // trailing whitespace is not the same id
        ] {
            let mut o = strong();
            o.supporting_chunks[0].chunk_id = composite.into();
            let errors = match CitationSupportTask::validate(&o, &ctx()) {
                Ok(()) => panic!("{composite:?} was ACCEPTED as a chunk_id"),
                Err(e) => e,
            };
            let e = errors
                .iter()
                .find(|e| e.field.contains("chunk_id"))
                .unwrap_or_else(|| panic!("{composite:?} did not flag the chunk_id field"));
            assert!(e.is_fatal(), "{composite:?} must be fatal, got {e}");
        }
    }

    /// §11 D32. Four is the cap, and four must PASS — a bound that also
    /// rejects the legal maximum would quietly become a bound of three.
    #[test]
    fn four_supporting_chunks_pass() {
        let mut o = strong();
        // c1 four times: the cap is about COUNT, and repeating a known-good
        // chunk isolates that from every other rule in the validator.
        let one = o.supporting_chunks[0].clone();
        o.supporting_chunks = vec![one.clone(), one.clone(), one.clone(), one];
        assert_eq!(o.supporting_chunks.len(), 4);
        assert!(
            CitationSupportTask::validate(&o, &ctx()).is_ok(),
            "four cited chunks must be accepted"
        );
    }

    /// The 0.5B's actual pathology: cite everything it was handed. Fatal, not
    /// advisory — a card listing a dozen chunks claims a dozen passages support
    /// the sentence, and a reader cannot check that.
    #[test]
    fn five_supporting_chunks_are_fatal() {
        let mut o = strong();
        let one = o.supporting_chunks[0].clone();
        o.supporting_chunks = vec![one.clone(), one.clone(), one.clone(), one.clone(), one];
        let errors = CitationSupportTask::validate(&o, &ctx())
            .expect_err("five cited chunks must be rejected");
        let e = errors
            .iter()
            .find(|e| e.field == "supporting_chunks")
            .expect("the cap was not the reported reason");
        assert!(e.is_fatal(), "the cap must be FATAL, not advisory: {e}");
        assert!(e.problem.contains('5') && e.problem.contains('4'), "{e}");
    }

    /// The cap is in `validate_support`, so v2 inherits it rather than needing
    /// its own copy.
    #[test]
    fn the_cap_applies_to_v2_as_well() {
        let mut o = strong();
        let one = SupportingChunk {
            quote: Some("Species richness rose 31%".into()),
            ..o.supporting_chunks[0].clone()
        };
        o.supporting_chunks = vec![one.clone(), one.clone(), one.clone(), one.clone(), one];
        let errors = CitationSupportV2Task::validate(&o, &ctx())
            .expect_err("v2 must inherit the cap");
        assert!(errors
            .iter()
            .any(|e| e.field == "supporting_chunks" && e.is_fatal()));
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
            quote: None,
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
            SupportingChunk { chunk_id: "c2".into(), page: None, why: "Null effect.".into(), quote: None };
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

        // §11 D32 — the cap is FATAL, and the tier test proves it rather than
        // trusting the table.
        let mut o = strong();
        let one = o.supporting_chunks[0].clone();
        o.supporting_chunks = vec![one.clone(), one.clone(), one.clone(), one.clone(), one];
        assert_eq!(tier_of(&o, "supporting_chunks"), Some(Tier::Fatal));

        assert_eq!(FATAL_RULES.len(), 11);
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


    /* ------------------------- v2: the quote check ------------------------ */

    fn v2_out(quote: Option<&str>) -> CitationSupportOutput {
        let mut o = strong();
        o.supporting_chunks[0].quote = quote.map(str::to_string);
        o
    }

    #[test]
    fn a_verbatim_quote_passes() {
        let o = v2_out(Some("Species richness rose 31% under organic management"));
        validate_support(&o, &ctx(), true).expect("a copied quote must pass");
    }

    #[test]
    fn a_quote_differing_only_in_whitespace_is_accepted() {
        // Line wrapping and double spaces are formatting, not paraphrase. The
        // normalisation exists so a correctly copied quote is not failed for
        // having been re-wrapped.
        let o = v2_out(Some("Species   richness\n  rose 31%\tunder organic management"));
        validate_support(&o, &ctx(), true).expect("whitespace must be normalised");
    }

    #[test]
    fn a_paraphrased_quote_is_fatal() {
        // The D18 failure in miniature: plausible, faithful in gist, and NOT in
        // the chunk. v1 has nothing that catches this.
        let o = v2_out(Some("Species richness increased by about a third under organic farming"));
        let errs = validate_support(&o, &ctx(), true).unwrap_err();
        let e = errs
            .iter()
            .find(|e| e.field.contains("quote"))
            .expect("the paraphrase must be caught");
        assert_eq!(e.tier, Tier::Fatal);
        assert!(e.problem.contains("verbatim substring"), "{}", e.problem);
        // and v1 must still accept it — that contrast is the point of running
        // both variants in the bake-off.
        validate_support(&o, &ctx(), false).expect("v1 does not check quotes");
    }

    #[test]
    fn an_invented_quote_is_fatal() {
        let o = v2_out(Some("Earthworm biomass doubled in every plot"));
        let errs = validate_support(&o, &ctx(), true).unwrap_err();
        assert!(errs.iter().any(|e| e.field.contains("quote") && e.tier == Tier::Fatal));
    }

    #[test]
    fn a_missing_quote_is_fatal_in_v2_and_fine_in_v1() {
        let o = v2_out(None);
        let errs = validate_support(&o, &ctx(), true).unwrap_err();
        assert!(errs.iter().any(|e| e.field.contains("quote") && e.tier == Tier::Fatal));
        validate_support(&o, &ctx(), false).expect("v1 never required a quote");
    }

    #[test]
    fn an_empty_quote_is_treated_as_missing_not_as_a_valid_substring() {
        // "" is a substring of everything. Without the explicit empty check
        // this would silently pass and the whole mitigation would be a no-op.
        let o = v2_out(Some("   "));
        let errs = validate_support(&o, &ctx(), true).unwrap_err();
        assert!(errs.iter().any(|e| e.field.contains("quote") && e.tier == Tier::Fatal));
    }

    #[test]
    fn a_quote_from_the_wrong_chunk_is_fatal() {
        // Real text, real chunk_id, wrong pairing — the model quoting c2's
        // sentence while citing c1.
        let o = v2_out(Some("No significant effect was observed for soil fauna"));
        let errs = validate_support(&o, &ctx(), true).unwrap_err();
        assert!(errs.iter().any(|e| e.field.contains("quote") && e.tier == Tier::Fatal));
    }

    #[test]
    fn an_over_long_quote_is_advisory_not_fatal() {
        // It is copied text; it is merely too much of it.
        let long = "Species richness rose 31% under organic management (p<0.01).";
        let mut o = v2_out(Some(long));
        o.supporting_chunks[0].quote = Some(long.to_string());
        // 9 words — under the limit, so construct the over-limit case from a
        // chunk long enough to exceed it.
        let ctx2 = TaskContext::new(vec![EvidenceChunk {
            chunk_id: "c1".into(),
            page: Some(8),
            section: None,
            text: (0..40).map(|i| format!("w{i}")).collect::<Vec<_>>().join(" "),
        }]);
        o.supporting_chunks[0].quote =
            Some((0..30).map(|i| format!("w{i}")).collect::<Vec<_>>().join(" "));
        o.supporting_chunks.truncate(1);
        o.supporting_chunks[0].page = Some(8);
        let errs = validate_support(&o, &ctx2, true).unwrap_err();
        let q = errs.iter().find(|e| e.field.contains("quote")).expect("length advisory");
        assert_eq!(q.tier, Tier::Advisory, "an over-long but REAL quote is untidy, not misleading");
    }

    #[test]
    fn the_v2_prompt_demands_the_quote_and_keeps_v1s_rules() {
        let t = CitationSupportV2Task {
            claim: "C".into(),
            cited_source: "S".into(),
            evidence: "<evidence>\n[CHUNK_ID=c1 PAGE=8 SECTION=Results] x\n</evidence>".into(),
        };
        let p = t.build_prompt();
        assert!(p.contains("\"quote\": string"), "the schema must show the quote field");
        assert!(p.contains("WORD FOR WORD"));
        assert!(p.contains("verdict mapping:"), "v1's rules must still be present");
        assert_eq!(t.prompt_version(), "citation_support-v2.3");
        assert_ne!(PROMPT_VERSION, PROMPT_VERSION_V2, "the two variants must be distinguishable");
        // the claim still comes last (the Phase 4b finding)
        let claim_at = p.rfind("CLAIM UNDER TEST").unwrap();
        assert!(claim_at > p.find("</evidence>").unwrap());
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
        assert!(p.contains("[CHUNK_ID=c1 PAGE=8 SECTION=Results]"), "evidence rendering drifted: {p}");
        // the claim is LAST (Phase 4b finding)
        let claim_at = p.rfind("Organic farming increases soil biodiversity").unwrap();
        let ev_at = p.rfind("[CHUNK_ID=c1 PAGE=8 SECTION=Results]").unwrap();
        assert!(claim_at > ev_at, "the claim must come after the evidence");
        assert!(p[claim_at..].contains("beginning with {"));
        assert_eq!(CitationSupportTask::max_tokens(), 768, "§11 D27 pins max_tokens: 768");
        assert_eq!(t.prompt_version(), "citation_support-v1.3");
    }
}
