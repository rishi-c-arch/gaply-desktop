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

pub const PROMPT_VERSION: &str = "citation_need-v1";

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
}

impl CitationNeedTask {
    pub fn new(input: CitationNeedInput) -> Self {
        Self { input }
    }
}

impl AiTask for CitationNeedTask {
    type Output = CitationNeedOutput;

    fn prompt_version() -> &'static str {
        PROMPT_VERSION
    }

    fn max_tokens() -> usize {
        MAX_TOKENS
    }

    fn build_prompt(&self) -> String {
        // Qwen2.5-Instruct chat template around the spec's blocks.
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

    fn validate(out: &Self::Output, _ctx: &TaskContext) -> Result<(), Vec<ValidationError>> {
        // Note on enums: serde has already rejected any value outside the ten
        // spec variants at PARSE time, so an out-of-range enum surfaces as a
        // parse error and earns a retry. That is a stronger guarantee than a
        // post-hoc range check, and the test asserts it holds.
        let mut errors = Vec::new();

        let reason_words = out.reason.split_whitespace().count();
        if reason_words > MAX_REASON_WORDS {
            errors.push(ValidationError {
                field: "reason".into(),
                problem: format!("is {reason_words} words; the limit is {MAX_REASON_WORDS}"),
            });
        }
        if out.reason.trim().is_empty() {
            errors.push(ValidationError {
                field: "reason".into(),
                problem: "must not be empty".into(),
            });
        }

        match (out.needs_citation, out.search_query.as_deref()) {
            // Spec: a query ONLY when a citation is needed.
            (false, Some(q)) if !q.trim().is_empty() => errors.push(ValidationError {
                field: "search_query".into(),
                problem: "must be null when needs_citation is false".into(),
            }),
            (true, None) | (true, Some("")) => errors.push(ValidationError {
                field: "search_query".into(),
                problem: format!(
                    "is required when needs_citation is true \
                     ({MIN_QUERY_WORDS}-{MAX_QUERY_WORDS} keywords)"
                ),
            }),
            (true, Some(q)) => {
                let words = q.split_whitespace().count();
                if !(MIN_QUERY_WORDS..=MAX_QUERY_WORDS).contains(&words) {
                    errors.push(ValidationError {
                        field: "search_query".into(),
                        problem: format!(
                            "is {words} words; the spec requires {MIN_QUERY_WORDS}-{MAX_QUERY_WORDS} keywords"
                        ),
                    });
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
    let flag = |problem: &str| ValidationError {
        field: "search_query".into(),
        problem: format!(
            "{problem} — this task never produces citation metadata; search_query is \
             keywords for the local library only"
        ),
    };

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

    #[test]
    fn the_prompt_reproduces_the_spec_blocks_verbatim() {
        let t = CitationNeedTask::new(CitationNeedInput {
            sentence: "Organic farming increases soil biodiversity.".into(),
            preceding_sentence: "Soil health is a growing concern.".into(),
            following_sentence: "This has implications for policy.".into(),
            section: "Introduction".into(),
        });
        let p = t.build_prompt();
        // spec SYSTEM
        assert!(p.contains("You decide whether an academic sentence requires a citation."));
        assert!(p.contains("You do not suggest sources. You only classify the sentence."));
        // shared rule 4 (the only addition — see plan §11 D8)
        assert!(p.contains("Output raw JSON only."));
        // spec INPUT block, all four fields
        assert!(p.contains("<preceding_sentence>Soil health is a growing concern.</preceding_sentence>"));
        assert!(p.contains("<sentence>Organic farming increases soil biodiversity.</sentence>"));
        assert!(p.contains("<following_sentence>This has implications for policy.</following_sentence>"));
        assert!(p.contains("<section>Introduction</section>"));
        // spec RULES, load-bearing lines
        assert!(p.contains("covered by preceding citation"));
        assert!(p.contains("6-12 keyword query for the library search, only when needs_citation=true"));
        assert!(p.contains("reason <= 25 words"));
        // NO evidence block — this task has none (plan §11 D8)
        assert!(!p.contains("<evidence>"), "citation_need must not claim to have evidence");
        assert_eq!(CitationNeedTask::max_tokens(), 200, "spec pins max_tokens: 200");
        assert_eq!(CitationNeedTask::prompt_version(), "citation_need-v1");
    }
}
