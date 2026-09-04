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
pub const PROMPT_VERSION: &str = "citation_support-v1.6";

/// SPEC OVERRIDE — §11 D27. The spec pins `max_tokens: 400`; measurement
/// retired it.
///
/// Across the Phase 6 support cells, 13 of 28 first attempts stopped EXACTLY at
/// their ceiling — every 0.5B v2 attempt, and 4 of 5 1.5B v1 attempts. A reply
/// cut off mid-JSON is unparseable, and unparseable scored identically to
/// wrong, so the arm was measuring the ceiling rather than the models.
///
/// 768 was derived, not guessed — and on the first real manuscript it was still
/// short. The Naidu 2023 run failed validation TWICE with
/// `EOF while parsing a list at line 67 column 5`: the reply was pretty-printed
/// across 67 lines and ran out of room inside `claim_elements`. "line 67" is
/// the tell — a compact object of the same content is one line.
///
/// v1.5 attacks both halves. The prompt now demands single-line JSON (see
/// `COMPACT_RULE`), which removes the newlines and indentation the model was
/// spending tokens on, and the ceiling rises to 1024.
///
/// THE ARITHMETIC, since the two budgets trade against each other:
/// `generative.rs` enforces `prompt_budget = TASK_N_CTX - max_tokens`, and
/// `TASK_N_CTX` is 4096. At 1024 that leaves **3072 tokens of prompt**. The v1
/// prompt measures 2035-2775 on real evidence (`a_full_prompt_fits_the_context`
/// pins the ceiling), plus ~50 for the labelled header, so the worst measured
/// case is 2825 against 3072 — it fits, with 247 to spare. Going higher would
/// not: 1280 would leave 2816, under the measured worst case.
const MAX_TOKENS: usize = 1024;

/// The longest thing that is still a CLAIM rather than a passage.
///
/// The first real-manuscript failure spent two 3B generations — minutes — on
/// 2,163 characters of the cited paper's own Methods section, pasted into the
/// claim box. The model behaved correctly: it decomposed a whole section into
/// `claim_elements` and ran out of room mid-array. No prompt or token ceiling
/// fixes that, because the input was never a claim.
///
/// Generous on purpose: a long academic sentence is ~400 characters, and the
/// measured real claim from `manuscript-nsi.pdf` was 141 characters / 33
/// tokens. This only catches input that is a different kind of thing.
pub const MAX_CLAIM_CHARS: usize = 800;

/// `Some(len)` when the claim is too long to be one, `None` when it is fine.
///
/// A function rather than an inline `if` at the call site so the bound sits
/// beside the other limits this task enforces, and so it is testable without
/// standing up a Tauri command.
pub fn claim_is_a_passage(claim: &str) -> Option<usize> {
    let n = claim.trim().chars().count();
    (n > MAX_CLAIM_CHARS).then_some(n)
}

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
/// THE PROMPT SAYS NOTHING ABOUT THIS BOUND AT ALL (§11 D34).
///
/// Three configurations were measured on the 3B under greedy decoding, so the
/// differences are causal rather than sampling:
///
/// | prompt | planted-chunk citation |
/// |---|---|
/// | bound + guidance (v1.2) | 25% |
/// | bound stated plainly (v1.3) | 50% |
/// | bound not mentioned (v1.1/v1.4) | 75% |
///
/// Stating the bound at all cost grounding; arguing for it cost more. The
/// validator does not need the model's cooperation to enforce a maximum — it
/// simply rejects — so the prompt buys nothing here and demonstrably charges
/// for it. **Do not add a line about this to the prompt.** If a future model
/// hits the cap often enough that the retry cost matters, measure that first
/// on the 50-case labeled set.
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
- Never say a claim is supported because it is plausible or well known.
- At most 5 claim_elements. Decompose to the checkable parts, not to every word."#;

/// Formatting, kept OUT of `RULES` so the bake-off's rule set is unchanged.
///
/// This is not a style preference. The first real-manuscript failure was a
/// truncated reply 67 pretty-printed lines long: newlines and indentation are
/// generation tokens like any other, and the ones spent on layout are the ones
/// missing from the end of the array. The reader never sees this JSON.
/// The FATAL rules the validator enforced and the prompt never stated.
///
/// Exactly the defect shape §11 D58 fixed for `citation_need`'s `search_query`:
/// a rule the engine rejects on, that the model was never told. On the v1.5
/// six-seed run every validation failure on both devices was one of these — 17%
/// of Metal cases and 33% of CPU cases — and a failure costs a full second
/// attempt, the largest single decode cost in the profile (§11 D60).
///
/// Placed at the END, where this model weights hardest — the placement D58
/// found necessary rather than merely tidy.
///
/// # The chunk bound is DELIBERATELY NOT STATED HERE
///
/// The obvious third rule — "at most 4 supporting_chunks" — is absent on
/// purpose, and `the_prompt_never_mentions_the_chunk_bound` guards its absence.
/// §11 D32 measured that exact line on this exact model: the cap **never fired**
/// (the 3B's cited counts were 2, 4, 2, 3, 0) and stating it moved
/// **planted-chunk citation 75% -> 25%** — a grounding regression invisible in
/// every other metric. D34 removed it for that reason. Adding it back to save a
/// retry would trade the property this engine exists to provide for latency.
///
/// v1.5 DID produce two 5-chunk rejections where D32 saw none, so the bound now
/// fires where it did not. That is an argument for MEASURING it again as its own
/// variant, not for quietly re-adding a line already shown to cost grounding.
///
/// These rules state constraints that ALREADY EXIST. Nothing legal becomes
/// illegal, so a v1.5 answer is still a v1.6 answer; only how often the model
/// produces one should change.
const FATAL_RULES_STATED: &str = "\
TWO RULES THAT ARE REJECTED OUTRIGHT. An answer breaking either is discarded and \
you are asked again, so read them last and check them before you answer.
1. suggested_rewrite MUST be null unless verdict is exactly \"partial\". For \
strong, weak, contradicts and insufficient_evidence it is null. No exceptions.
2. verdict \"contradicts\" REQUIRES at least one claim_element with \
status \"different\". If nothing in the decomposition differs, the verdict is \
\"weak\", not \"contradicts\".";

const COMPACT_RULE: &str = "\
- Output the JSON on ONE line, with no newlines, no indentation and no spaces \
between tokens. Every character of layout is a character you cannot spend on \
the answer, and a reply that stops mid-array is discarded entirely.";

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
             RULES\n{RULES}\n{COMPACT_RULE}\n\n\
             {FATAL_RULES_STATED}\n\n\
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

/* ============ v1.7 — the chunk bound, RE-TESTED under changed conditions ==== *
 * EXPERIMENT ONLY. Not selected by `PROMPT_VERSION`, not reachable from the
 * app — `--support-variant v1c` in the eval harness is the only caller. The
 * shipped v1.6 prompt is byte-unchanged, and
 * `the_prompt_never_mentions_the_chunk_bound` still guards it.
 *
 * WHY THIS EXISTS AT ALL, given D32 measured it harmful. D32's cells were:
 *
 *   no mention      planted-chunk citation 75%
 *   stated plainly                         50%
 *   stated + guidance                      25%
 *
 * so D34 removed it. Two things have changed since. The cap NEVER FIRED on the
 * 3B then (cited counts 2, 4, 2, 3, 0); under v1.5 and v1.6 it fires on CPU,
 * where seeds 02 and 05 are rejected for 5 entries and are now the ONLY
 * remaining CPU failures. And D62/D63 changed what else is in the prompt.
 *
 * The narrow question: does stating the bound fix seeds 02 and 05 WITHOUT
 * moving cited-planted the way D32 saw? Cited-planted is the metric that killed
 * it before and it is the metric that decides it now — if it drops on either
 * device this variant does not ship, whatever the failure rate does. Grounding
 * over latency, the same rule that refused the 1.5B in D62.
 *
 * It states the bound PLAINLY and adds no guidance. D32's own numbers say the
 * guidance phrasing was the worse of the two (25% vs 50%), so re-running the
 * more harmful wording would answer a question nobody asked. */

/// The experiment's version string. Distinct so its reports can never be
/// conflated with a shipped v1.6 cell.
pub const PROMPT_VERSION_V17: &str = "citation_support-v1.7-chunkbound";

/// Bare statement of the bound. No "cite only the chunks that carry the point",
/// no "listing every chunk is not evidence" — that is D32's 25% cell.
const CHUNK_BOUND_RULE: &str =
    "3. supporting_chunks MUST contain AT MOST 4 entries.";

/// v1.6's task with the chunk bound appended to the fatal-rule block.
#[derive(Debug, Clone)]
pub struct CitationSupportV17Task {
    pub claim: String,
    pub cited_source: String,
    pub evidence: String,
}

impl AiTask for CitationSupportV17Task {
    type Output = CitationSupportOutput;

    fn prompt_version(&self) -> &'static str {
        PROMPT_VERSION_V17
    }

    fn max_tokens() -> usize {
        MAX_TOKENS
    }

    fn build_prompt(&self) -> String {
        // v1.6's prompt with ONE line added, so any difference measured is
        // attributable to that line and nothing else.
        let base = CitationSupportTask {
            claim: self.claim.clone(),
            cited_source: self.cited_source.clone(),
            evidence: self.evidence.clone(),
        }
        .build_prompt();
        // The fatal-rule block says TWO; with the bound it says three.
        base.replace("TWO RULES THAT ARE REJECTED", "THREE RULES THAT ARE REJECTED")
            .replace(
                "\"weak\", not \"contradicts\".",
                &format!("\"weak\", not \"contradicts\".\n{CHUNK_BOUND_RULE}"),
            )
    }

    fn validate(out: &Self::Output, ctx: &TaskContext) -> Result<(), Vec<ValidationError>> {
        // IDENTICAL validation to v1. The bound was always enforced; the only
        // question is whether SAYING it changes what the model produces.
        validate_support(out, ctx, false)
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
pub const PROMPT_VERSION_V2: &str = "citation_support-v2.4";

const OUTPUT_SCHEMA_V2: &str = r#"{
  "verdict": "strong|partial|weak|contradicts|insufficient_evidence",
  "confidence": 0.0-1.0,
  "supporting_chunks": [{"chunk_id": string, "page": integer, "quote": string, "why": string}],
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
    #[test]
    fn a_sentence_is_a_claim_and_a_pasted_section_is_not() {
        use super::{claim_is_a_passage, MAX_CLAIM_CHARS};
        // The measured real claim from the test manuscript: 141 chars.
        let real = "A three year audit at a tertiary hospital in western India recorded an \
                    incidence of 10.4 needlestick injuries per 100 occupied beds per year.";
        assert_eq!(claim_is_a_passage(real), None, "a real citing sentence was refused");

        // Two long sentences are still a claim — the bound must not catch these.
        let two_sentences = "x".repeat(MAX_CLAIM_CHARS);
        assert_eq!(claim_is_a_passage(&two_sentences), None, "the bound itself must be allowed");

        // The shape that actually failed: a block of the source's own prose.
        let pasted = "y".repeat(2163);
        assert_eq!(claim_is_a_passage(&pasted), Some(2163));
    }

    #[test]
    fn the_bound_is_measured_on_trimmed_text_not_raw_length() {
        use super::claim_is_a_passage;
        // A short claim surrounded by whitespace is a short claim.
        let padded = format!("{}{}{}", " ".repeat(2000), "A short claim.", " ".repeat(2000));
        assert_eq!(claim_is_a_passage(&padded), None);
    }

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
    /// §11 D34. The prompt must say NOTHING about the chunk bound.
    ///
    /// Measured on the 3B under greedy decoding: mentioning it cost grounding
    /// (planted-chunk citation 75% with no mention, 50% stated plainly, 25%
    /// stated with guidance). The validator enforces the maximum without the
    /// model's cooperation, so a prompt line buys nothing and charges for it.
    /// This test exists to stop a well-meaning future edit reintroducing one.
    /// The experiment variant must differ from v1.6 by EXACTLY the bound line —
    /// otherwise its cell measures more than the thing under test.
    #[test]
    fn v17_is_v16_plus_the_bound_and_nothing_else() {
        let base = CitationSupportTask {
            claim: "C".into(),
            cited_source: "S".into(),
            evidence: ctx().render_evidence(),
        }
        .build_prompt();
        let v17 = CitationSupportV17Task {
            claim: "C".into(),
            cited_source: "S".into(),
            evidence: ctx().render_evidence(),
        }
        .build_prompt();

        assert!(v17.contains("AT MOST 4 entries"), "the bound is missing: {v17}");
        assert!(!base.contains("AT MOST 4 entries"), "v1.6 leaked the bound: {base}");
        assert!(v17.contains("THREE RULES THAT ARE REJECTED"), "count not updated: {v17}");
        // Bare statement only. D32 measured the guidance phrasing at 25% planted
        // citation against 50% for the plain one; re-running the worse wording
        // would answer a question nobody asked.
        let lower = v17.to_lowercase();
        assert!(!lower.contains("not evidence"), "the D32 guidance wording came back: {v17}");
        assert!(!lower.contains("carry the point"), "the D32 guidance wording came back: {v17}");

        // EXACTLY one added line, so the cell isolates it.
        let extra: Vec<&str> = v17.lines().filter(|l| !base.contains(*l)).collect();
        assert_eq!(extra.len(), 2, "expected the bound line + the reworded header, got {extra:?}");

        // And its reports can never be confused with a shipped cell.
        assert_ne!(PROMPT_VERSION_V17, PROMPT_VERSION);
    }

    /// §11 D69. THE RETRY INVARIANT, pinned against the configured constants.
    ///
    /// `retry_prompt` quotes the rejected reply verbatim (D10), so a retry costs
    /// the original prompt PLUS a reply of up to `max_tokens`. `max_tokens` is
    /// therefore subtracted TWICE — once for the reply being generated, once for
    /// the reply being quoted:
    ///
    /// ```text
    /// original <= TASK_N_CTX - 2*MAX_TOKENS - RETRY_OVERHEAD_TOKENS
    /// ```
    ///
    /// At 4096/1024 that ceiling was 1981 and REAL prompts measured 1919-2155,
    /// so a real check that failed validation could not be retried at all — it
    /// was rejected before generation with `prompt is 3246 tokens`. Changing
    /// either constant without the other must fail here rather than in a user's
    /// audit.
    #[test]
    fn a_retryable_prompt_fits_the_configured_context() {
        use crate::ai::generative::{max_retryable_prompt_tokens, RETRY_OVERHEAD_TOKENS, TASK_N_CTX};
        let max_tokens = <CitationSupportTask as AiTask>::max_tokens();
        let ceiling = max_retryable_prompt_tokens(max_tokens);

        // The arithmetic itself, stated so a future edit cannot quietly drop the
        // factor of two.
        assert_eq!(ceiling, TASK_N_CTX - 2 * max_tokens - RETRY_OVERHEAD_TOKENS);

        // MEASURED against real open-access sources fetched from OpenAlex, NOT
        // against the six synthetic seeds — §11 D69: the seeds' "worst measured
        // 2825" was a property of the fixtures, and real sources exceed the
        // budget those fixtures justified. 2155 is the largest real prompt
        // observed (GoEmotions, 6 chunks, 1390 evidence tokens).
        const LARGEST_REAL_PROMPT: usize = 2155;
        assert!(
            ceiling >= LARGEST_REAL_PROMPT,
            "a real fetched-source prompt of {LARGEST_REAL_PROMPT} tokens could not be \
             RETRIED: ceiling is {ceiling} (TASK_N_CTX {TASK_N_CTX}, max_tokens \
             {max_tokens}). Raise TASK_N_CTX or lower max_tokens — but lowering \
             max_tokens re-opens D58's truncation class and needs its own cell."
        );

        // And the first attempt must fit too, which is the weaker of the two.
        assert!(TASK_N_CTX - max_tokens >= LARGEST_REAL_PROMPT);
    }

    #[test]
    fn the_prompt_never_mentions_the_chunk_bound() {
        let v1 = CitationSupportTask {
            claim: "C".into(),
            cited_source: "S".into(),
            evidence: ctx().render_evidence(),
        }
        .build_prompt();
        let v2 = CitationSupportV2Task {
            claim: "C".into(),
            cited_source: "S".into(),
            evidence: ctx().render_evidence(),
        }
        .build_prompt();
        for (label, p) in [("v1", &v1), ("v2", &v2)] {
            let lower = p.to_lowercase();
            for banned in ["at most 4", "at most four", "no more than 4", "maximum of 4"] {
                assert!(
                    !lower.contains(banned),
                    "{label} prompt reintroduced the bound ({banned:?}) — see §11 D34"
                );
            }
            // the cap's own error text must not leak into the prompt either
            assert!(!lower.contains("may be cited"), "{label} prompt leaked the validator message");
        }
    }

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
        assert_eq!(t.prompt_version(), "citation_support-v2.4");
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
        // §11 D58 raises v1 768 -> 1024 after a real manuscript truncated
        // mid-array twice. The number is not free: `TASK_N_CTX - max_tokens`
        // is the prompt budget, so 1024 leaves 3072 against a worst measured
        // prompt of 2825.
        assert_eq!(CitationSupportTask::max_tokens(), 1024, "§11 D58 pins max_tokens: 1024");
        assert!(
            crate::ai::generative::TASK_N_CTX - CitationSupportTask::max_tokens() >= 2825,
            "the token raise ate the prompt budget the measured prompts need"
        );
        assert_eq!(t.prompt_version(), "citation_support-v1.6");
        // §11 D63: the three FATAL rules the validator enforces must be STATED.
        // Each is pinned by the phrase a reader would grep for, not by the
        // whole block, so wording can improve without the test rotting.
        assert!(p.contains("suggested_rewrite MUST be null"), "rule 1 missing: {p}");
        assert!(p.contains("REQUIRES at least one claim_element"), "rule 2 missing: {p}");
        // And they sit at the END, after the evidence and the claim — the
        // placement D58 measured as necessary for this model.
        let rules_at = p.rfind("TWO RULES THAT ARE REJECTED").expect("block absent");
        let schema_at = p.find("OUTPUT SCHEMA").expect("schema absent");
        assert!(rules_at > schema_at, "the fatal-rule block drifted above the schema");
        // The compact-output rule reaches the model, not just the source.
        assert!(p.contains("ONE line"), "the compact-JSON rule is missing: {p}");
        assert!(p.contains("At most 5 claim_elements"), "the element cap is missing: {p}");
    }
}
