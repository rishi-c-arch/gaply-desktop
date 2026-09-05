//! Spec Prompt 3 — Citation Need Detector.
//!
//! Decides whether one academic sentence requires a citation. It **does not
//! suggest sources**: the spec is explicit that this task "only classifies the
//! sentence", and `search_query` is a query for the user's LOCAL library, never
//! a reference.
//!
//! Runs only on sentences a deterministic pass has already flagged as carrying
//! no citation marker — this task never decides that question for itself.
//!
//! # No evidence block
//!
//! Prompt 3's INPUT is the sentence and its neighbours, not retrieved passages,
//! so the spec's shared rules 1–3 (only use `<evidence>`, echo a `chunk_id`,
//! abstain when evidence is insufficient) are inapplicable and are not injected
//! — see plan §11 D8. Rule 4, raw JSON only, IS injected and carries more weight
//! here than anywhere, since §9.1 removed grammar-constrained decoding.
//!
//! # HARD RULE
//!
//! This task never produces citation metadata: no author names, no years, no
//! DOIs, no formatted citations. `search_query` is keywords. The validator
//! rejects anything that looks like a reference rather than trusting the prompt
//! to have been obeyed — R3 made mechanical.

use serde::{Deserialize, Serialize};

use crate::ai::task::{AiTask, TaskContext, ValidationError};

/// v1 — the spec's INPUT block order, kept for comparison (plan §11 D10/D12).
pub const PROMPT_VERSION_V1: &str = "citation_need-v1";
/// v2 — same rules, target sentence last and explicitly labelled.
pub const PROMPT_VERSION_V2: &str = "citation_need-v2";
/// v3 — the own-work rule stated explicitly, with worked examples, and the
/// validator's `search_query` constraint written into the prompt.
const PROMPT_VERSION_V3: &str = "citation_need-v3";
/// v4 — the decision made on its own terms, with `sentence_type` emitted AFTER
/// the boolean as a description rather than before it as a determinant (D78).
pub const PROMPT_VERSION_V4: &str = "citation_need-v4";
/// What a caller gets if it does not choose.
/// §11 D78 ships v4 as the ADVISORY variant, so it is what `new()` builds.
///
/// This was v3 for one commit while the report already printed v4's measured
/// 82%/43% — the engine would have produced 0% recall while the page claimed
/// 82%. `advisory_figures_match_a_real_eval_of_the_shipped_prompt` now binds the
/// two, so the default and the printed numbers cannot drift apart again.
pub const PROMPT_VERSION: &str = PROMPT_VERSION_V4;

/// Which INPUT layout to use. The SPEC RULES are byte-identical across both;
/// only the arrangement of the input differs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PromptVariant {
    /// Spec order: preceding, target, following, section.
    V1,
    /// Context first and subordinate, then the target sentence LAST, labelled,
    /// immediately before the output instruction.
    V2,
    /// The decision made on its own terms, with `sentence_type` emitted AFTER
    /// the boolean as a description rather than before it as a determinant
    /// (§11 D78).
    V4,
    /// V2's layout with [`RULES_V3_ADDENDUM`]: the own-work rule stated
    /// explicitly with worked examples, and the validator's `search_query`
    /// constraint written into the prompt rather than left to be discovered.
    V3,
}

impl PromptVariant {
    pub fn version(self) -> &'static str {
        match self {
            PromptVariant::V1 => PROMPT_VERSION_V1,
            PromptVariant::V2 => PROMPT_VERSION_V2,
            PromptVariant::V3 => PROMPT_VERSION_V3,
            PromptVariant::V4 => PROMPT_VERSION_V4,
        }
    }
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "v1" | PROMPT_VERSION_V1 => Some(PromptVariant::V1),
            "v2" | PROMPT_VERSION_V2 => Some(PromptVariant::V2),
            "v3" | PROMPT_VERSION_V3 => Some(PromptVariant::V3),
            "v4" | PROMPT_VERSION_V4 => Some(PromptVariant::V4),
            _ => None,
        }
    }
}

/// Spec: `max_tokens: 200`.
const MAX_TOKENS: usize = 200;

/// Spec: `reason ≤ 25 words`.
const MAX_REASON_WORDS: usize = 25;

/// Spec: "6–12 keyword query". Enforced as a range, not a suggestion.
const MIN_QUERY_WORDS: usize = 6;
const MAX_QUERY_WORDS: usize = 12;

/// The ten values in the spec's `sentence_type` enum, in spec order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SentenceType {
    EmpiricalClaim,
    Statistic,
    Definition,
    PriorWork,
    MethodBorrowed,
    CommonKnowledge,
    AuthorOwnResult,
    Transition,
    Interpretation,
    HedgedSpeculation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    High,
    Medium,
    Low,
}

/// Exactly the spec's OUTPUT SCHEMA. No additions — in particular no
/// `confidence` field, which Prompts 1/2/5/7 have and this one deliberately
/// does not.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CitationNeedOutput {
    pub needs_citation: bool,
    pub sentence_type: SentenceType,
    /// How badly the sentence needs a citation — **only meaningful when it
    /// needs one** (§11 D66).
    ///
    /// The spec declared this unconditionally required, and the engine threw
    /// away every answer that omitted it. Reading the raw outputs showed those
    /// were not model failures: the model returned a complete, well-formed,
    /// CORRECT object and left out the one field that carries no information
    /// when `needs_citation` is false. A sentence that needs no citation has no
    /// severity-of-need to report.
    ///
    /// So the requirement is CONDITIONAL, mirroring `search_query`, which the
    /// spec already treats this way in the opposite direction: required when
    /// `needs_citation` is true, legal to omit when it is false.
    #[serde(default)]
    pub severity: Option<Severity>,
    pub reason: String,
    /// Spec: "only when needs_citation=true", null otherwise.
    ///
    /// **Two `None`s, deliberately** (§11 D81). The outer one means the reply
    /// did not contain the key at all; `Some(None)` means it contained
    /// `"search_query": null`. Those are different facts about what the model
    /// did and they point at different fixes — a dropped field is a schema or
    /// prompt problem, a deliberate `null` beside `needs_citation: true` is an
    /// incoherent judgement — and `Option<String>` could not tell them apart.
    ///
    /// It said "is required when needs_citation is true" for both, so the one
    /// case that actually shipped (the model supplying `null`, 5/5) was
    /// reported as an omission the model had not committed. D66's whole lesson
    /// is that this distinction decides who is at fault; the type now carries
    /// it.
    ///
    /// Both remain FATAL. Only the message changed.
    #[serde(default, deserialize_with = "present_but_maybe_null")]
    pub search_query: Option<Option<String>>,
}

/// Distinguish "key absent" from `"key": null` (§11 D81).
///
/// serde calls this ONLY when the key is present, so reaching it at all proves
/// presence; `#[serde(default)]` supplies the outer `None` when it is not.
fn present_but_maybe_null<'de, D>(d: D) -> Result<Option<Option<String>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::Deserialize;
    Option::<String>::deserialize(d).map(Some)
}

impl CitationNeedOutput {
    /// The query the model actually offered, flattening both kinds of absence.
    ///
    /// Every consumer wants this; only the validator cares which `None` it was.
    pub fn query(&self) -> Option<&str> {
        self.search_query.as_ref().and_then(|q| q.as_deref())
    }
}

/// Exactly the spec's INPUT block.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CitationNeedInput {
    pub sentence: String,
    #[serde(default)]
    pub preceding_sentence: String,
    #[serde(default)]
    pub following_sentence: String,
    pub section: String,
}

/// SYSTEM + RULES, verbatim from spec Prompt 3.
///
/// The ARCHITECTURE OVERRIDE covers runtime and decoding only; the prompt text
/// is authoritative and is reproduced without paraphrase. The one addition is
/// shared rule 4, which is marked where it appears.
const SYSTEM: &str = "You decide whether an academic sentence requires a citation.
You do not suggest sources. You only classify the sentence.
Output raw JSON only. No markdown, no code fences, no commentary.";

const OUTPUT_SCHEMA: &str = r#"{
  "needs_citation": true|false,
  "sentence_type": "empirical_claim|statistic|definition|prior_work|method_borrowed|
                    common_knowledge|author_own_result|transition|interpretation|hedged_speculation",
  "severity": "high|medium|low",
  "reason": string,
  "search_query": string|null
}"#;

const RULES: &str = r#"- needs_citation = true for: empirical claims about the world, statistics, numbers,
  definitions attributable to a source, descriptions of prior work, borrowed methods.
- needs_citation = false for: the author's own results (Results/Discussion sections),
  transitions, research questions, statements about the thesis's own structure,
  and genuinely common knowledge.
- Section matters: an empirical claim in Introduction/Literature Review is high severity.
  The same sentence in Results is probably the author's own finding -> false.
- If the preceding sentence carries a citation and this sentence continues the same
  attributed idea, needs_citation = false, reason = "covered by preceding citation".
- search_query: 6-12 keyword query for the library search, only when needs_citation=true.
- reason <= 25 words."#;

/// What v3 ADDS to [`RULES`]. Kept separate so v1 and v2 stay byte-identical to
/// the variants that were measured — a version that silently changed its rules
/// would make every earlier eval number a claim about a prompt that no longer
/// exists.
/// v4's schema. THE FIELD ORDER IS THE EXPERIMENT (§11 D78).
///
/// Generation is left to right, so a schema that emits `sentence_type` before
/// `needs_citation` makes the boolean a FUNCTION of the type — and D77 measured
/// type agreement at 25-37%, with v2 and v3 failing as exact mirror images
/// (`empirical_claim` over-used 12x / under-used 12x). The boolean was downstream
/// of a misclassification in both.
///
/// Here the decision and its reason come FIRST, on their own terms. The type is
/// emitted afterwards as a DESCRIPTION of a decision already made, so it can no
/// longer cause the answer.
const OUTPUT_SCHEMA_V4: &str = r#"{
  "needs_citation": true|false,
  "reason": string,
  "search_query": string|null,
  "sentence_type": "empirical_claim|statistic|definition|prior_work|method_borrowed|
                    common_knowledge|author_own_result|transition|interpretation|hedged_speculation",
  "severity": "high|medium|low"
}"#;

/// v4's rules. The SAME judgements as v1-v3, asked as one question rather than
/// routed through a taxonomy.
///
/// v3's own-work content is kept — D58 measured it working (18 own-work false
/// positives -> 2) — but stated as a fact about the SENTENCE rather than as a
/// type to assign. D77 showed v3 overshot into blanket suppression; the wording
/// here removes the "classify, then decide" step without removing the rule.
const RULES_V4: &str = r#"ONE QUESTION: would a reader need a source to check this sentence?

Answer it directly. Do not classify the sentence first.

Say TRUE when the sentence asserts something a reader could look up and verify
in someone else's work: a fact about the world, a number taken from elsewhere,
someone else's finding, method or definition, or a claim about what is known in
the field.

Say FALSE when there is nothing external to check against: the authors' own
results, numbers they measured themselves, their own method, algorithm or
experimental setup, their own hardware, a pointer to their own figure or table,
a transition, or a statement so ordinary that no source would be offered for it.

A NUMBER IS NOT A REASON TO SAY TRUE. A number the authors measured is theirs;
only a number taken FROM someone else needs a source.

BEING IN THE INTRODUCTION IS NOT A REASON TO SAY TRUE, and being in Results is
not a reason to say false. Ask what the sentence asserts, not where it sits.

- reason: <= 25 words, and it must state WHAT would be checked and against whom.
- search_query: 6-12 keywords, only when needs_citation is true; otherwise null.
- sentence_type and severity DESCRIBE the answer you have already given. They do
  not decide it. Severity may be omitted when needs_citation is false."#;

const RULES_V3_ADDENDUM: &str = r#"

THE TWO RULES THAT ARE MOST OFTEN GOT WRONG. Apply these last, and let them
override anything above.

(1) THE AUTHORS' OWN WORK NEVER NEEDS A CITATION. A paper does not cite itself.
    This covers their own results and numbers, their own method and algorithm,
    their own experimental setup, and the hardware or software they ran on.
    A number is not a reason to say true: a number the authors MEASURED is
    their own, and only a number they took FROM someone else needs a source.
    If your own reason would say "this is the author's own finding", the answer
    is false. Say false.

    Sentence: "I carried out all the experiments on an Intel Core i7-11800H CPU
    with 16 GB RAM."
    -> needs_citation: false, sentence_type: "author_own_result",
       reason: "the authors' own experimental setup", search_query: null

    Sentence: "The highest F1-scores are for joy (97.0%) and sadness (96.1%)."
    -> needs_citation: false, sentence_type: "author_own_result",
       reason: "the authors' own measured results", search_query: null

    Sentence: "HEFCSO-BiLSTM outperforms all eight baselines on the combined set."
    -> needs_citation: false, sentence_type: "author_own_result",
       reason: "a claim about the authors' own system", search_query: null

(2) search_query MUST be null when needs_citation is false. This is enforced,
    not advisory: an answer with needs_citation false and a non-null
    search_query is REJECTED and the whole judgement is thrown away. When you
    answer false, write "search_query": null."#;


pub struct CitationNeedTask {
    pub input: CitationNeedInput,
    pub variant: PromptVariant,
}

impl CitationNeedTask {
    /// Default to v2 — the variant measured to be less prone to classifying the
    /// wrong sentence.
    pub fn new(input: CitationNeedInput) -> Self {
        // v4 by default (§11 D78). v3 measured 0% recall on 41 cold labelled
        // cases — it answered "no citation needed" to every one — so it cannot
        // be what ships behind an advisory the report describes as catching 82%.
        Self { input, variant: PromptVariant::V4 }
    }
    pub fn with_variant(input: CitationNeedInput, variant: PromptVariant) -> Self {
        Self { input, variant }
    }

    /// v1: the spec's INPUT block, in the spec's order.
    fn build_v1(&self) -> String {
        format!(
            "<|im_start|>system\n{SYSTEM}\n<|im_end|>\n\
             <|im_start|>user\n\
             INPUT\n\
             <preceding_sentence>{prev}</preceding_sentence>\n\
             <sentence>{target}</sentence>\n\
             <following_sentence>{next}</following_sentence>\n\
             <section>{section}</section>\n\n\
             OUTPUT SCHEMA\n{OUTPUT_SCHEMA}\n\n\
             RULES\n{RULES}\n\
             <|im_end|>\n\
             <|im_start|>assistant\n",
            prev = self.input.preceding_sentence,
            target = self.input.sentence,
            next = self.input.following_sentence,
            section = self.input.section,
        )
    }

    /// v2: same SYSTEM, same OUTPUT SCHEMA, same RULES — byte-identical consts.
    ///
    /// Only the INPUT arrangement changes, to fix a measured failure: under v1
    /// the model's `reason` repeatedly described "the preceding sentence"
    /// rather than the target, i.e. it classified the wrong sentence. v1 opens
    /// with the preceding sentence, so that is what a small model's attention
    /// lands on first and, apparently, keeps.
    ///
    /// v2 demotes the neighbours to explicitly-labelled CONTEXT that is stated
    /// NOT to be classified, then puts the target LAST under its own heading,
    /// immediately before the output instruction — the position a decoder
    /// weights most heavily.
    /// v4: the question asked once, the type demoted to a description.
    ///
    /// Deliberately keeps v2's LAYOUT — context first, target last — so the only
    /// difference from the measured variants is the schema order and the rules,
    /// not where the sentence sits. "SENTENCE TO JUDGE", not "CLASSIFY".
    fn build_v4(&self) -> String {
        format!(
            "<|im_start|>system\n{SYSTEM}\n<|im_end|>\n\
             <|im_start|>user\n\
             OUTPUT SCHEMA\n{OUTPUT_SCHEMA_V4}\n\n\
             {RULES_V4}\n\n\
             CONTEXT (background only — do NOT judge these)\n\
             section: {section}\n\
             previous sentence: {prev}\n\
             next sentence: {next}\n\n\
             SENTENCE TO JUDGE\n\
             {target}\n\n\
             Judge ONLY the sentence above. Decide needs_citation FIRST, then \
             write the reason that justifies it. \
             Output the JSON object now, beginning with {{\n\
             <|im_end|>\n\
             <|im_start|>assistant\n",
            prev = self.input.preceding_sentence,
            target = self.input.sentence,
            next = self.input.following_sentence,
            section = self.input.section,
        )
    }

    fn build_v2(&self) -> String {
        format!(
            "<|im_start|>system\n{SYSTEM}\n<|im_end|>\n\
             <|im_start|>user\n\
             OUTPUT SCHEMA\n{OUTPUT_SCHEMA}\n\n\
             RULES\n{RULES}\n\n\
             CONTEXT (background only — do NOT classify these)\n\
             section: {section}\n\
             previous sentence: {prev}\n\
             next sentence: {next}\n\n\
             SENTENCE TO CLASSIFY\n\
             {target}\n\n\
             Classify ONLY the sentence above, under SENTENCE TO CLASSIFY. \
             Output the JSON object now, beginning with {{\n\
             <|im_end|>\n\
             <|im_start|>assistant\n",
            prev = self.input.preceding_sentence,
            target = self.input.sentence,
            next = self.input.following_sentence,
            section = self.input.section,
        )
    }
}

/* ------------------- how this task maps spec rules to tiers ---------------- *
 * Enumerated here, beside the validator, so the mapping is reviewable rather
 * than buried in control flow (plan §9.11). THE SPEC'S PROMPT TEXT IS UNCHANGED
 * — the model is still asked for every rule with equal force. What these decide
 * is only what the ENGINE does when the model misses.
 *
 * The test `every_rule_has_a_declared_tier` keeps these lists honest against
 * the validator's actual behaviour. */

/// Violations that make an output actively MISLEADING. One retry, then failure.
pub const FATAL_RULES: &[&str] = &[
    "sentence_type outside the ten spec values (rejected by serde at parse time)",
    "severity outside high|medium|low (rejected by serde at parse time)",
    "severity absent when needs_citation is true (the field that grades the need)",
    "reason empty — a required field with no content",
    "search_query present when needs_citation is false (fields contradict)",
    "search_query missing from the reply when needs_citation is true (a dropped field)",
    "search_query supplied as null when needs_citation is true (fields contradict) — §11 D81",
    "search_query shaped like a reference: parenthesised year, 'et al', DOI, or URL",
];

/// Violations that make an output UNTIDY. Accepted; reported as advisories.
pub const ADVISORY_RULES: &[&str] = &[
    "reason longer than 25 words",
    "search_query outside the 6-12 word range",
];

impl AiTask for CitationNeedTask {
    type Output = CitationNeedOutput;

    fn prompt_version(&self) -> &'static str {
        self.variant.version()
    }

    fn max_tokens() -> usize {
        MAX_TOKENS
    }

    fn build_prompt(&self) -> String {
        match self.variant {
            PromptVariant::V1 => self.build_v1(),
            PromptVariant::V2 => self.build_v2(),
            // Same layout as v2; the difference is the rules it carries.
            PromptVariant::V3 => self.build_v2().replace(
                &format!("RULES\n{RULES}\n"),
                &format!("RULES\n{RULES}{RULES_V3_ADDENDUM}\n"),
            ),
            PromptVariant::V4 => self.build_v4(),
        }
    }

    fn validate(out: &Self::Output, _ctx: &TaskContext) -> Result<(), Vec<ValidationError>> {
        // Note on enums: serde has already rejected any value outside the ten
        // spec variants at PARSE time, so an out-of-range enum surfaces as a
        // parse error and earns a retry. That is a stronger guarantee than a
        // post-hoc range check, and the test asserts it holds.
        let mut errors = Vec::new();

        let reason_words = out.reason.split_whitespace().count();
        if reason_words > MAX_REASON_WORDS {
            // ADVISORY: a rationale two words over the limit is untidy, not
            // misleading. The classification it explains is unaffected.
            errors.push(ValidationError::advisory(
                "reason",
                format!("is {reason_words} words; the limit is {MAX_REASON_WORDS}"),
            ));
        }
        if out.reason.trim().is_empty() {
            // FATAL: a required field with no content is a schema violation,
            // not a style one.
            errors.push(ValidationError::fatal("reason", "must not be empty"));
        }

        match (out.needs_citation, out.severity) {
            // FATAL: the field that GRADES the need is missing while a need is
            // asserted. Unlike its absence below, this loses information the
            // report shows.
            (true, None) => errors.push(ValidationError::fatal(
                "severity",
                "must be present when needs_citation is true — it grades the need",
            )),
            // ACCEPTED SILENTLY, and this is a reversal worth recording. The
            // first draft made it an advisory — "a grade for a need that does
            // not exist is noise". `a_false_verdict_with_a_null_query_passes`
            // failed, and it was right to: the SPEC declares `severity` present
            // unconditionally, so a model that emits it here is doing exactly
            // what it was asked. D66 widens what is ACCEPTED; it must not
            // simultaneously start complaining about the compliant shape.
            // An advisory would penalise correct behaviour and inflate
            // `advisoryRate`, which the bake-off reads as a quality signal.
            (true, Some(_)) | (false, None) | (false, Some(_)) => {}
        }

        match (out.needs_citation, out.query()) {
            // Spec: a query ONLY when a citation is needed.
            // FATAL: the two fields contradict each other, so at least one is
            // wrong and there is no way to tell which.
            (false, Some(q)) if !q.trim().is_empty() => errors.push(ValidationError::fatal(
                "search_query",
                "must be null when needs_citation is false",
            )),
            // FATAL, both ways — but they are different events and the message
            // now says which (§11 D81). The old text asserted an omission for
            // both, and the case that actually reached users was the second:
            // the model SUPPLIED the field, as null, and was told it had not.
            (true, None) | (true, Some("")) => errors.push(ValidationError::fatal(
                "search_query",
                match &out.search_query {
                    // The reply did not carry the key. A dropped field.
                    None => format!(
                        "was not in the reply at all; it is required when needs_citation \
                         is true ({MIN_QUERY_WORDS}-{MAX_QUERY_WORDS} keywords)"
                    ),
                    // The reply carried it, empty or null. Not an omission: the
                    // model asserted a citation is needed and then declined to
                    // say what would be searched for, which is the judgement
                    // contradicting itself rather than a missing field.
                    Some(_) => format!(
                        "was supplied as null while needs_citation is true — the reply \
                         asserts a citation is needed and gives nothing to search for \
                         ({MIN_QUERY_WORDS}-{MAX_QUERY_WORDS} keywords expected)"
                    ),
                },
            )),
            (true, Some(q)) => {
                let words = q.split_whitespace().count();
                if !(MIN_QUERY_WORDS..=MAX_QUERY_WORDS).contains(&words) {
                    // ADVISORY: a 4-word or 14-word query still searches. This
                    // single rule caused every end-to-end failure in Phase 4b.
                    errors.push(ValidationError::advisory(
                        "search_query",
                        format!(
                            "is {words} words; the spec requires {MIN_QUERY_WORDS}-{MAX_QUERY_WORDS} keywords"
                        ),
                    ));
                }
                errors.extend(reject_citation_shaped(q));
            }
            (false, _) => {}
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}

/// THE HARD RULE, made mechanical.
///
/// `search_query` is a keyword query against the user's own library. A model
/// that emits "Smith et al. (2021)" or a DOI has produced citation metadata,
/// which this task must never do — and a fabricated reference is the single
/// most damaging thing this product could output. Checked rather than trusted.
fn reject_citation_shaped(q: &str) -> Vec<ValidationError> {
    let mut errors = Vec::new();
    let lower = q.to_lowercase();
    // FATAL, always: a fabricated reference is the most damaging thing this
    // product can emit, and no amount of tidiness makes it acceptable.
    let flag = |problem: &str| ValidationError::fatal(
        "search_query",
        format!(
            "{problem} — this task never produces citation metadata; search_query is \
             keywords for the local library only"
        ),
    );

    // A 4-digit year in parentheses: "(2021)", "(2021a)".
    if has_parenthesised_year(q) {
        errors.push(flag("contains a parenthesised year, which reads as a citation"));
    }
    if lower.contains("et al") {
        errors.push(flag("contains \"et al\""));
    }
    if lower.contains("10.") && lower.contains('/') {
        errors.push(flag("contains what looks like a DOI"));
    }
    if lower.contains("doi:") {
        errors.push(flag("contains a DOI prefix"));
    }
    if lower.contains("http") {
        errors.push(flag("contains a URL"));
    }
    errors
}

/// `(` then four digits then an optional letter then `)`.
fn has_parenthesised_year(q: &str) -> bool {
    let b: Vec<char> = q.chars().collect();
    for (i, c) in b.iter().enumerate() {
        if *c != '(' {
            continue;
        }
        let digits: String = b[i + 1..].iter().take(4).collect();
        if digits.len() == 4 && digits.chars().all(|d| d.is_ascii_digit()) {
            // allow "(2021)" and "(2021a)"
            let after: Vec<char> = b[i + 5..].iter().take(2).copied().collect();
            if after.first() == Some(&')')
                || (after.first().is_some_and(|c| c.is_ascii_alphabetic())
                    && after.get(1) == Some(&')'))
            {
                return true;
            }
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::task::Tier;

    fn ok_output() -> CitationNeedOutput {
        CitationNeedOutput {
            needs_citation: true,
            sentence_type: SentenceType::EmpiricalClaim,
            severity: Some(Severity::High),
            reason: "Empirical claim about the world stated without attribution.".into(),
            search_query: Some(Some("organic farming soil biodiversity species richness meta analysis".into())),
        }
    }

    fn ctx() -> TaskContext {
        TaskContext::default()
    }

    #[test]
    fn a_well_formed_output_passes() {
        assert!(CitationNeedTask::validate(&ok_output(), &ctx()).is_ok());
    }

    #[test]
    fn a_false_verdict_with_a_null_query_passes() {
        let out = CitationNeedOutput {
            needs_citation: false,
            sentence_type: SentenceType::AuthorOwnResult,
            severity: Some(Severity::Low),
            reason: "The author's own result, reported in Results.".into(),
            search_query: None,
        };
        assert!(CitationNeedTask::validate(&out, &ctx()).is_ok());
    }

    #[test]
    fn an_out_of_range_enum_is_rejected_at_parse_time() {
        // serde is the enforcement: a value outside the ten spec variants never
        // becomes a CitationNeedOutput at all, so it earns a retry rather than
        // reaching the validator as a legal-but-wrong value.
        let json = r#"{"needs_citation":true,"sentence_type":"vibes","severity":"high",
                       "reason":"x","search_query":"a b c d e f"}"#;
        let err = serde_json::from_str::<CitationNeedOutput>(json).unwrap_err();
        assert!(
            err.to_string().contains("unknown variant") && err.to_string().contains("vibes"),
            "serde must name the offending variant so the retry can quote it: {err}"
        );

        let bad_sev = r#"{"needs_citation":true,"sentence_type":"statistic","severity":"critical",
                          "reason":"x","search_query":"a b c d e f"}"#;
        assert!(serde_json::from_str::<CitationNeedOutput>(bad_sev).is_err());
    }

    #[test]
    fn all_ten_spec_sentence_types_round_trip() {
        for name in [
            "empirical_claim", "statistic", "definition", "prior_work", "method_borrowed",
            "common_knowledge", "author_own_result", "transition", "interpretation",
            "hedged_speculation",
        ] {
            let json = format!(
                r#"{{"needs_citation":false,"sentence_type":"{name}","severity":"low","reason":"r","search_query":null}}"#
            );
            let parsed: CitationNeedOutput =
                serde_json::from_str(&json).unwrap_or_else(|e| panic!("{name}: {e}"));
            assert_eq!(serde_json::to_value(parsed.sentence_type).unwrap(), name);
        }
    }

    #[test]
    fn a_reason_over_twenty_five_words_is_rejected() {
        let mut out = ok_output();
        out.reason = (0..26).map(|i| format!("w{i}")).collect::<Vec<_>>().join(" ");
        let errors = CitationNeedTask::validate(&out, &ctx()).unwrap_err();
        assert!(errors.iter().any(|e| e.field == "reason" && e.problem.contains("26 words")), "{errors:?}");
        // exactly 25 is fine — the spec says "<= 25"
        out.reason = (0..25).map(|i| format!("w{i}")).collect::<Vec<_>>().join(" ");
        assert!(CitationNeedTask::validate(&out, &ctx()).is_ok());
    }

    #[test]
    fn a_query_present_with_needs_citation_false_is_rejected() {
        let mut out = ok_output();
        out.needs_citation = false;
        let errors = CitationNeedTask::validate(&out, &ctx()).unwrap_err();
        assert!(
            errors.iter().any(|e| e.field == "search_query" && e.problem.contains("must be null")),
            "{errors:?}"
        );
    }

    #[test]
    fn a_query_absent_with_needs_citation_true_is_rejected() {
        let mut out = ok_output();
        out.search_query = None;
        let errors = CitationNeedTask::validate(&out, &ctx()).unwrap_err();
        assert!(
            errors.iter().any(|e| e.field == "search_query" && e.problem.contains("required")),
            "{errors:?}"
        );
    }

    /// §11 D81. THE DISTINCTION THAT DECIDES WHO IS AT FAULT.
    ///
    /// Both are FATAL and always were. What changed is that the engine can now
    /// tell them apart, because it could not, and so it reported the one that
    /// actually shipped — the model supplying `null`, 5 times out of 5 — as an
    /// omission the model had not committed. A reader who trusts that message
    /// concludes the model dropped a field and reaches for D66's fix.
    #[test]
    fn an_absent_query_and_a_null_query_are_reported_as_different_events() {
        let problem = |sq| {
            let mut out = ok_output();
            out.search_query = sq;
            let errors = CitationNeedTask::validate(&out, &ctx()).unwrap_err();
            let e = errors
                .iter()
                .find(|e| e.field == "search_query")
                .unwrap_or_else(|| panic!("no search_query error: {errors:?}"))
                .clone();
            assert_eq!(e.tier, Tier::Fatal, "both cases must stay FATAL");
            e.problem
        };

        // The key was not in the reply: a dropped field.
        let absent = problem(None);
        assert!(absent.contains("not in the reply at all"), "{absent}");

        // The key WAS in the reply, as null. Not an omission.
        let null = problem(Some(None));
        assert!(null.contains("supplied as null"), "{null}");
        assert!(
            !null.contains("not in the reply"),
            "a supplied null is still being described as absent: {null}"
        );

        assert_ne!(absent, null, "the two events still report identically");
    }

    /// And the distinction survives DESERIALISATION, which is the only place it
    /// can be observed — by the time validation runs, the reply is gone.
    #[test]
    fn serde_preserves_whether_the_reply_carried_the_key() {
        let with_null = r#"{"needs_citation":true,"sentence_type":"empirical_claim",
                            "severity":"high","reason":"r","search_query":null}"#;
        let omitted = r#"{"needs_citation":true,"sentence_type":"empirical_claim",
                          "severity":"high","reason":"r"}"#;

        let a: CitationNeedOutput = serde_json::from_str(with_null).expect("null parses");
        assert_eq!(a.search_query, Some(None), "an explicit null read as absent");

        let b: CitationNeedOutput = serde_json::from_str(omitted).expect("omission parses");
        assert_eq!(b.search_query, None, "an omitted key read as present");

        // Consumers see one thing, as they did before.
        assert_eq!(a.query(), None);
        assert_eq!(b.query(), None);
    }

    /// The exact reply from the D80 mis-run, verbatim, 5/5 of which produced
    /// this shape. Kept as a fixture so the message that describes it cannot
    /// drift back into claiming an omission.
    #[test]
    fn the_shape_that_actually_shipped_is_named_correctly() {
        let raw = r#"{
              "needs_citation": true,
              "reason": "PublishReady 3 Sentences that may need a citation These sentences carry no citation.",
              "search_query": null,
              "sentence_type": "common_knowledge",
              "severity": "high"
            }"#;
        let out: CitationNeedOutput = serde_json::from_str(raw).expect("the real reply parses");
        // It parsed. The model produced a well-formed object; the JUDGEMENT is
        // what is wrong, and D81 refuses to widen the schema to accept it.
        let errors = CitationNeedTask::validate(&out, &ctx()).unwrap_err();
        let e = errors.iter().find(|e| e.field == "search_query").expect("no search_query error");
        assert_eq!(e.tier, Tier::Fatal);
        assert!(e.problem.contains("supplied as null"), "{}", e.problem);
    }

    #[test]
    fn a_query_outside_six_to_twelve_words_is_rejected() {
        let mut out = ok_output();
        out.search_query = Some(Some("too short".into()));
        assert!(CitationNeedTask::validate(&out, &ctx()).is_err());
        out.search_query = Some(Some((0..13).map(|i| format!("w{i}")).collect::<Vec<_>>().join(" ")));
        assert!(CitationNeedTask::validate(&out, &ctx()).is_err());
        // the boundaries themselves are legal
        out.search_query = Some(Some((0..6).map(|i| format!("w{i}")).collect::<Vec<_>>().join(" ")));
        assert!(CitationNeedTask::validate(&out, &ctx()).is_ok());
        out.search_query = Some(Some((0..12).map(|i| format!("w{i}")).collect::<Vec<_>>().join(" ")));
        assert!(CitationNeedTask::validate(&out, &ctx()).is_ok());
    }

    /// THE HARD RULE: a query that looks like a citation is rejected, not
    /// trusted. Each of these is a way a model tries to be helpful and produces
    /// exactly the fabricated reference this product must never emit.
    #[test]
    fn a_citation_shaped_query_is_rejected() {
        for (label, q) in [
            ("parenthesised year", "soil biodiversity Smith (2021) organic farming yield effects"),
            ("year with suffix", "soil biodiversity Smith (2021a) organic farming yield effects"),
            ("et al", "organic farming soil biodiversity Smith et al meta analysis review"),
            ("et al.", "organic farming soil biodiversity Smith et al. meta analysis review"),
            ("DOI", "organic farming biodiversity 10.1038/s41586-020-2649-2 richness effects"),
            ("doi prefix", "organic farming biodiversity doi:10.1038 richness effects study"),
            ("url", "organic farming biodiversity https://example.org richness effects study"),
        ] {
            let mut out = ok_output();
            out.search_query = Some(Some(q.to_string()));
            let errors = CitationNeedTask::validate(&out, &ctx())
                .expect_err(&format!("{label}: a citation-shaped query was ACCEPTED"));
            assert!(
                errors.iter().any(|e| e.field == "search_query"),
                "{label}: rejected, but not for the search_query: {errors:?}"
            );
        }
    }

    #[test]
    fn citation_shaped_rejections_name_the_hard_rule() {
        let mut out = ok_output();
        out.search_query = Some(Some("organic farming soil biodiversity Smith et al review".into()));
        let errors = CitationNeedTask::validate(&out, &ctx()).unwrap_err();
        let e = errors.iter().find(|e| e.field == "search_query").expect("flagged");
        assert!(e.problem.contains("et al"), "{e}");
        assert!(
            e.problem.contains("never produces citation metadata"),
            "the retry must be told WHY, not just that it failed: {e}"
        );
    }

    #[test]
    fn an_ordinary_number_in_a_query_is_not_mistaken_for_a_year() {
        // "(2021)" is a citation; "2021" as a keyword, or a bare parenthesis,
        // is not. Over-rejecting would push the model toward worse queries.
        let mut out = ok_output();
        out.search_query = Some(Some("soil carbon sequestration 2021 survey data cropland".into()));
        assert!(CitationNeedTask::validate(&out, &ctx()).is_ok(), "a bare year was rejected");
        out.search_query = Some(Some("nitrogen (N) fixation legume rotation yield response".into()));
        assert!(CitationNeedTask::validate(&out, &ctx()).is_ok(), "a non-year parenthesis was rejected");
    }

    fn sample() -> CitationNeedInput {
        CitationNeedInput {
            sentence: "Organic farming increases soil biodiversity.".into(),
            preceding_sentence: "Soil health is a growing concern.".into(),
            following_sentence: "This has implications for policy.".into(),
            section: "Introduction".into(),
        }
    }

    #[test]
    fn v1_reproduces_the_spec_blocks_verbatim() {
        let t = CitationNeedTask::with_variant(sample(), PromptVariant::V1);
        let p = t.build_prompt();
        assert!(p.contains("You decide whether an academic sentence requires a citation."));
        assert!(p.contains("You do not suggest sources. You only classify the sentence."));
        assert!(p.contains("Output raw JSON only."));
        assert!(p.contains("<preceding_sentence>Soil health is a growing concern.</preceding_sentence>"));
        assert!(p.contains("<sentence>Organic farming increases soil biodiversity.</sentence>"));
        assert!(p.contains("<following_sentence>This has implications for policy.</following_sentence>"));
        assert!(p.contains("<section>Introduction</section>"));
        assert!(p.contains("covered by preceding citation"));
        assert!(p.contains("6-12 keyword query for the library search, only when needs_citation=true"));
        assert!(p.contains("reason <= 25 words"));
        assert!(!p.contains("<evidence>"), "citation_need must not claim to have evidence");
        assert_eq!(t.prompt_version(), "citation_need-v1");
        assert_eq!(CitationNeedTask::max_tokens(), 200, "spec pins max_tokens: 200");
    }

    #[test]
    fn v2_keeps_the_spec_rules_byte_identical_and_only_moves_the_input() {
        let v1 = CitationNeedTask::with_variant(sample(), PromptVariant::V1).build_prompt();
        let v2 = CitationNeedTask::with_variant(sample(), PromptVariant::V2).build_prompt();

        // The parts the spec owns are the SAME STRING in both.
        for spec_text in [SYSTEM, OUTPUT_SCHEMA, RULES] {
            assert!(v1.contains(spec_text), "v1 lost a spec block");
            assert!(v2.contains(spec_text), "v2 altered a spec block — only the INPUT may move");
        }
        assert_eq!(
            CitationNeedTask::with_variant(sample(), PromptVariant::V2).prompt_version(),
            "citation_need-v2"
        );
    }

    #[test]
    fn v2_puts_the_target_sentence_last_and_labels_it() {
        let t = CitationNeedTask::with_variant(sample(), PromptVariant::V2);
        let p = t.build_prompt();
        let target = "Organic farming increases soil biodiversity.";
        let prev = "Soil health is a growing concern.";

        assert!(p.contains("SENTENCE TO CLASSIFY"), "the target must be labelled: {p}");
        assert!(p.contains("do NOT classify these"), "context must be marked subordinate");

        // The target appears AFTER both neighbours — the position a decoder
        // weights most heavily, and the fix for v1 classifying the wrong one.
        let t_at = p.rfind(target).expect("target present");
        let p_at = p.rfind(prev).expect("preceding present");
        let n_at = p.rfind("This has implications for policy.").expect("following present");
        assert!(t_at > p_at && t_at > n_at, "the target must come last in v2");

        // and the output instruction is the very last thing before the turn ends
        let tail = &p[t_at..];
        assert!(tail.contains("Classify ONLY the sentence above"));
        assert!(tail.contains("beginning with {"));
    }

    /// §11 D66. `severity` is required ONLY when a citation is needed. All four
    /// combinations, because a conditional rule that is only tested on the two
    /// convenient sides is a rule nobody has checked.
    #[test]
    fn severity_is_required_only_when_a_citation_is_needed() {
        let out = |needs: bool, sev: Option<Severity>| CitationNeedOutput {
            needs_citation: needs,
            sentence_type: SentenceType::EmpiricalClaim,
            severity: sev,
            reason: "r".into(),
            search_query: needs.then(|| Some("a query of about eight words here now".to_string())),
        };
        let tier_of = |o: CitationNeedOutput| -> Option<Tier> {
            CitationNeedTask::validate(&o, &ctx())
                .err()?
                .into_iter()
                .find(|e| e.field == "severity")
                .map(|e| e.tier)
        };

        // 1. needs a citation, graded — the ordinary case.
        assert_eq!(tier_of(out(true, Some(Severity::High))), None);
        // 2. needs a citation, UNGRADED — fatal: the grade is the information.
        assert_eq!(tier_of(out(true, None)), Some(Tier::Fatal));
        // 3. no citation needed, absent — LEGAL. This is the whole point: the
        //    model returned a complete correct object and the engine used to
        //    discard it over a field grading a need that does not exist.
        assert_eq!(tier_of(out(false, None)), None);
        // 4. no citation needed, but graded anyway — ACCEPTED. The spec asks
        //    for severity unconditionally, so this is the COMPLIANT shape and
        //    must not be penalised while D66 widens what is accepted.
        assert_eq!(tier_of(out(false, Some(Severity::Low))), None);
    }

    /// The exact payload from the live diagnostic, byte for byte: a complete,
    /// correct, own-work answer that the engine used to throw away.
    #[test]
    fn the_real_discarded_output_now_parses_and_validates() {
        let raw = r#"{
  "needs_citation": false,
  "sentence_type": "author_own_result",
  "reason": "the authors' own pre-trained vectors",
  "search_query": null
}"#;
        let out: CitationNeedOutput =
            serde_json::from_str(raw).expect("this is what the model actually returns");
        assert!(!out.needs_citation);
        assert_eq!(out.severity, None);
        assert_eq!(out.sentence_type, SentenceType::AuthorOwnResult);
        assert!(
            CitationNeedTask::validate(&out, &ctx()).is_ok(),
            "the engine still rejects an answer that is correct"
        );
    }

    /// Keeps FATAL_RULES / ADVISORY_RULES honest against what the validator
    /// actually does. A doc comment that drifts from the code is worse than no
    /// doc comment, and this mapping is the reviewable record of a decision.
    #[test]
    fn every_rule_has_a_declared_tier_and_the_validator_agrees() {
        let tier_of = |out: CitationNeedOutput, field: &str| -> Option<Tier> {
            CitationNeedTask::validate(&out, &ctx())
                .err()?
                .into_iter()
                .find(|e| e.field == field)
                .map(|e| e.tier)
        };

        // --- FATAL ---
        let mut o = ok_output();
        o.reason = "   ".into();
        assert_eq!(tier_of(o, "reason"), Some(Tier::Fatal), "an empty reason must be fatal");

        let mut o = ok_output();
        o.needs_citation = false;
        assert_eq!(
            tier_of(o, "search_query"),
            Some(Tier::Fatal),
            "contradicting fields must be fatal"
        );

        // BOTH kinds of missing query are fatal, and they are two declared
        // rules rather than one because they are two different events (§11 D81).
        let mut o = ok_output();
        o.search_query = None; // not in the reply at all
        assert_eq!(tier_of(o, "search_query"), Some(Tier::Fatal));

        let mut o = ok_output();
        o.search_query = Some(None); // in the reply, as null
        assert_eq!(tier_of(o, "search_query"), Some(Tier::Fatal));

        let mut o = ok_output();
        o.search_query = Some(Some("organic farming soil biodiversity Smith et al review".into()));
        assert_eq!(
            tier_of(o, "search_query"),
            Some(Tier::Fatal),
            "a reference-shaped query must ALWAYS be fatal — a fabricated citation is the \
             worst thing this product can emit"
        );

        // --- ADVISORY ---
        let mut o = ok_output();
        o.reason = (0..30).map(|i| format!("w{i}")).collect::<Vec<_>>().join(" ");
        assert_eq!(tier_of(o, "reason"), Some(Tier::Advisory), "a long reason is untidy, not wrong");

        let mut o = ok_output();
        o.search_query = Some(Some("too short".into()));
        assert_eq!(
            tier_of(o, "search_query"),
            Some(Tier::Advisory),
            "THIS rule caused every end-to-end failure in Phase 4b"
        );

        let mut o = ok_output();
        o.search_query = Some(Some((0..14).map(|i| format!("w{i}")).collect::<Vec<_>>().join(" ")));
        assert_eq!(tier_of(o, "search_query"), Some(Tier::Advisory));

        // the declared lists are non-empty and documented
        assert_eq!(FATAL_RULES.len(), 8);
        assert_eq!(ADVISORY_RULES.len(), 2);
    }

    #[test]
    fn a_short_query_no_longer_blocks_an_otherwise_good_answer() {
        // The exact shape that failed 8/8 in Phase 4b: valid JSON, right
        // classification, query 4 words instead of 6.
        let mut o = ok_output();
        o.search_query = Some(Some("land degradation distribution survey".into()));
        let errors = CitationNeedTask::validate(&o, &ctx()).unwrap_err();
        assert!(
            errors.iter().all(|e| e.tier == Tier::Advisory),
            "this output must be acceptable with advisories: {errors:?}"
        );
    }

    #[test]
    fn v3_states_the_own_work_rule_with_worked_examples() {
        // The first real audit flagged the authors' own hardware, their own F1
        // scores and a claim about their own system as needing citations —
        // sometimes with a reason that SAID it was the author's own finding.
        // The rule existed; it was mid-prompt and keyed on a section the audit
        // never passed. v3 states it last and shows it.
        let t = CitationNeedTask::with_variant(
            CitationNeedInput {
                sentence: "The highest F1-scores are for joy (97.0%).".into(),
                preceding_sentence: String::new(),
                following_sentence: String::new(),
                section: "RESULTS".into(),
            },
            PromptVariant::V3,
        );
        let p = t.build_prompt();
        assert!(p.contains("THE AUTHORS' OWN WORK NEVER NEEDS A CITATION"), "{p}");
        assert!(p.contains("Intel Core i7-11800H"), "worked example missing");
        assert!(p.contains("outperforms all eight baselines"), "worked example missing");
        // A number is not itself a reason to say true.
        assert!(p.contains("a number the authors MEASURED is"), "{p}");
        // And the rule comes AFTER the general rules it overrides.
        let general = p.find("needs_citation = true for").unwrap();
        let own = p.find("THE AUTHORS' OWN WORK").unwrap();
        assert!(general < own, "the override rule must come last");
    }

    #[test]
    fn v3_states_the_search_query_constraint_the_validator_enforces() {
        // Five sentences failed validation TWICE on exactly this, so the
        // constraint stopped being a detail worth leaving implicit.
        let t = CitationNeedTask::with_variant(
            CitationNeedInput {
                sentence: "A sentence.".into(),
                preceding_sentence: String::new(),
                following_sentence: String::new(),
                section: String::new(),
            },
            PromptVariant::V3,
        );
        let p = t.build_prompt();
        assert!(p.contains("search_query MUST be null when needs_citation is false"), "{p}");
        assert!(p.contains("REJECTED"), "the prompt must say it is enforced: {p}");
    }

    #[test]
    fn v1_and_v2_are_untouched_by_v3() {
        // A version that silently changed its rules would make every earlier
        // eval number a claim about a prompt that no longer exists.
        let input = || CitationNeedInput {
            sentence: "A sentence.".into(),
            preceding_sentence: String::new(),
            following_sentence: String::new(),
            section: String::new(),
        };
        for v in [PromptVariant::V1, PromptVariant::V2] {
            let p = CitationNeedTask::with_variant(input(), v).build_prompt();
            assert!(!p.contains("THE AUTHORS' OWN WORK"), "{v:?} gained v3's rules");
        }
        let v3 = CitationNeedTask::with_variant(input(), PromptVariant::V3).build_prompt();
        let v2 = CitationNeedTask::with_variant(input(), PromptVariant::V2).build_prompt();
        // v3 IS v2 plus the addendum — same layout, more rules.
        assert!(v3.len() > v2.len());
        assert!(v3.contains("TARGET SENTENCE") == v2.contains("TARGET SENTENCE"));
    }

    /// §11 D78. The field ORDER is the experiment: the boolean and its reason
    /// must precede `sentence_type`, because generation is left to right and a
    /// type emitted first makes the answer a function of a classification that
    /// D77 measured at 25-37% agreement.
    #[test]
    fn v4_decides_before_it_classifies() {
        let t = CitationNeedTask::with_variant(
            CitationNeedInput {
                sentence: "S".into(),
                preceding_sentence: String::new(),
                following_sentence: String::new(),
                section: "Introduction".into(),
            },
            PromptVariant::V4,
        );
        let p = t.build_prompt();
        let needs = p.find("\"needs_citation\"").expect("no needs_citation in schema");
        let reason = p.find("\"reason\"").expect("no reason in schema");
        let stype = p.find("\"sentence_type\"").expect("no sentence_type in schema");
        assert!(needs < reason, "the reason must follow the decision it justifies");
        assert!(
            reason < stype,
            "sentence_type precedes the boolean — the answer is a function of the \
             classification again, which is the defect D78 exists to remove"
        );
        // And it must not ask the model to classify first.
        assert!(!p.contains("SENTENCE TO CLASSIFY"), "still framed as classification");
        assert!(p.contains("SENTENCE TO JUDGE"));
        assert!(p.contains("Do not classify the sentence first"));
        // v3 is untouched — its measured numbers must stay claims about it.
        let v3 = CitationNeedTask::with_variant(
            CitationNeedInput {
                sentence: "S".into(),
                preceding_sentence: String::new(),
                following_sentence: String::new(),
                section: "Introduction".into(),
            },
            PromptVariant::V3,
        )
        .build_prompt();
        assert!(v3.contains("SENTENCE TO CLASSIFY"), "v3 drifted");
    }

    #[test]
    fn variant_parsing_accepts_short_and_full_names() {
        assert_eq!(PromptVariant::parse("v1"), Some(PromptVariant::V1));
        assert_eq!(PromptVariant::parse("citation_need-v2"), Some(PromptVariant::V2));
        assert_eq!(PromptVariant::parse("v3"), Some(PromptVariant::V3));
        assert_eq!(PromptVariant::parse("v4"), Some(PromptVariant::V4));
        assert_eq!(PromptVariant::parse("v5"), None);
        // The default must be a real variant, not a third string — and it is
        // now v4 (§11 D78/D79): v3 measured 0% recall on 41 cold labelled
        // cases, so it cannot be what ships behind an advisory the report
        // describes as catching 82%.
        assert_eq!(PROMPT_VERSION, PROMPT_VERSION_V4);
        assert_eq!(
            CitationNeedTask::new(CitationNeedInput {
                sentence: "S".into(),
                preceding_sentence: String::new(),
                following_sentence: String::new(),
                section: String::new(),
            })
            .prompt_version(),
            PROMPT_VERSION,
            "new() builds a different variant than PROMPT_VERSION names"
        );
    }
}
