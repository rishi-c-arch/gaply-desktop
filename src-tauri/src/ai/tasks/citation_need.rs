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
/// What a caller gets if it does not choose.
pub const PROMPT_VERSION: &str = PROMPT_VERSION_V2;

/// Which INPUT layout to use. The SPEC RULES are byte-identical across both;
/// only the arrangement of the input differs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PromptVariant {
    /// Spec order: preceding, target, following, section.
    V1,
    /// Context first and subordinate, then the target sentence LAST, labelled,
    /// immediately before the output instruction.
    V2,
}

impl PromptVariant {
    pub fn version(self) -> &'static str {
        match self {
            PromptVariant::V1 => PROMPT_VERSION_V1,
            PromptVariant::V2 => PROMPT_VERSION_V2,
        }
    }
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "v1" | PROMPT_VERSION_V1 => Some(PromptVariant::V1),
            "v2" | PROMPT_VERSION_V2 => Some(PromptVariant::V2),
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
    pub severity: Severity,
    pub reason: String,
    /// Spec: "only when needs_citation=true", null otherwise.
    #[serde(default)]
    pub search_query: Option<String>,
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

pub struct CitationNeedTask {
    pub input: CitationNeedInput,
    pub variant: PromptVariant,
}

impl CitationNeedTask {
    /// Default to v2 — the variant measured to be less prone to classifying the
    /// wrong sentence.
    pub fn new(input: CitationNeedInput) -> Self {
        Self { input, variant: PromptVariant::V2 }
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
    "reason empty — a required field with no content",
    "search_query present when needs_citation is false (fields contradict)",
    "search_query absent when needs_citation is true (fields contradict)",
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

        match (out.needs_citation, out.search_query.as_deref()) {
            // Spec: a query ONLY when a citation is needed.
            // FATAL: the two fields contradict each other, so at least one is
            // wrong and there is no way to tell which.
            (false, Some(q)) if !q.trim().is_empty() => errors.push(ValidationError::fatal(
                "search_query",
                "must be null when needs_citation is false",
            )),
            // FATAL: same contradiction, the other way round.
            (true, None) | (true, Some("")) => errors.push(ValidationError::fatal(
                "search_query",
                format!(
                    "is required when needs_citation is true \
                     ({MIN_QUERY_WORDS}-{MAX_QUERY_WORDS} keywords)"
                ),
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
            severity: Severity::High,
            reason: "Empirical claim about the world stated without attribution.".into(),
            search_query: Some("organic farming soil biodiversity species richness meta analysis".into()),
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
            severity: Severity::Low,
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

    #[test]
    fn a_query_outside_six_to_twelve_words_is_rejected() {
        let mut out = ok_output();
        out.search_query = Some("too short".into());
        assert!(CitationNeedTask::validate(&out, &ctx()).is_err());
        out.search_query = Some((0..13).map(|i| format!("w{i}")).collect::<Vec<_>>().join(" "));
        assert!(CitationNeedTask::validate(&out, &ctx()).is_err());
        // the boundaries themselves are legal
        out.search_query = Some((0..6).map(|i| format!("w{i}")).collect::<Vec<_>>().join(" "));
        assert!(CitationNeedTask::validate(&out, &ctx()).is_ok());
        out.search_query = Some((0..12).map(|i| format!("w{i}")).collect::<Vec<_>>().join(" "));
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
            out.search_query = Some(q.to_string());
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
        out.search_query = Some("organic farming soil biodiversity Smith et al review".into());
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
        out.search_query = Some("soil carbon sequestration 2021 survey data cropland".into());
        assert!(CitationNeedTask::validate(&out, &ctx()).is_ok(), "a bare year was rejected");
        out.search_query = Some("nitrogen (N) fixation legume rotation yield response".into());
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

        let mut o = ok_output();
        o.search_query = None;
        assert_eq!(tier_of(o, "search_query"), Some(Tier::Fatal));

        let mut o = ok_output();
        o.search_query = Some("organic farming soil biodiversity Smith et al review".into());
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
        o.search_query = Some("too short".into());
        assert_eq!(
            tier_of(o, "search_query"),
            Some(Tier::Advisory),
            "THIS rule caused every end-to-end failure in Phase 4b"
        );

        let mut o = ok_output();
        o.search_query = Some((0..14).map(|i| format!("w{i}")).collect::<Vec<_>>().join(" "));
        assert_eq!(tier_of(o, "search_query"), Some(Tier::Advisory));

        // the declared lists are non-empty and documented
        assert_eq!(FATAL_RULES.len(), 6);
        assert_eq!(ADVISORY_RULES.len(), 2);
    }

    #[test]
    fn a_short_query_no_longer_blocks_an_otherwise_good_answer() {
        // The exact shape that failed 8/8 in Phase 4b: valid JSON, right
        // classification, query 4 words instead of 6.
        let mut o = ok_output();
        o.search_query = Some("land degradation distribution survey".into());
        let errors = CitationNeedTask::validate(&o, &ctx()).unwrap_err();
        assert!(
            errors.iter().all(|e| e.tier == Tier::Advisory),
            "this output must be acceptable with advisories: {errors:?}"
        );
    }

    #[test]
    fn variant_parsing_accepts_short_and_full_names() {
        assert_eq!(PromptVariant::parse("v1"), Some(PromptVariant::V1));
        assert_eq!(PromptVariant::parse("citation_need-v2"), Some(PromptVariant::V2));
        assert_eq!(PromptVariant::parse("v3"), None);
        // the default must be a real variant, not a third string
        assert_eq!(PROMPT_VERSION, PROMPT_VERSION_V2);
    }
}
