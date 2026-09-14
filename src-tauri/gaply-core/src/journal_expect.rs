//! **Expectations — what a reviewer is told to look for, and nothing else.**
//!
//! §7's third kind of knowledge: *"18 of 25 comparable papers include external
//! validation"* — a frequency with its evidence, *"never as a rule"*. Prompt 5
//! item 4: *"For journals with public reviewer guidelines or editor statements,
//! ingest them through the same guarded path. For the rest, nothing — an empty
//! expectations table is honest; an expectation without a source is not."*
//!
//! # THE CORPUS MAY NOT FILL THIS TABLE
//!
//! The tempting shortcut, when a journal publishes no reviewer guidance, is to
//! derive an expectation from the recent-paper corpus — *"18 of 25 comparable
//! papers include external validation"* is, after all, §7's own example.
//!
//! **That number is a CONVENTION and belongs in `journal_conventions`.** §7
//! keeps the three kinds apart precisely here: a convention is what the
//! journal's published papers DO, an expectation is what its reviewers are
//! ASKED to look for, and the two answer different questions. A corpus
//! frequency filed as an expectation would tell a researcher that a journal
//! expects external validation when what was measured is that most authors
//! happened to include it.
//!
//! **The schema makes it unstorable rather than merely discouraged**
//! (`migrations` v23): `journal_expectations` requires a non-empty `source_url`
//! AND a non-empty `source_span`. A corpus statistic has neither — no page and
//! no sentence — so it cannot be written through this path at all.
//!
//! # What deterministic ingestion can actually take
//!
//! A reviewer-guidance page states what reviewers assess, in obligation
//! language: *"Reviewers should consider whether the statistical analysis is
//! appropriate."* Each such sentence is one expectation, quoted verbatim.
//!
//! `frequency_k`/`frequency_n` stay `None`. A guidance page says what is looked
//! for, never how often it is found; a frequency would have to come from the
//! corpus, which is the boundary above.

use serde::Serialize;

use crate::journal_extract::GuidelineBlock;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ExtractedExpectation {
    /// The claim, as the journal stated it.
    pub claim: String,
    /// The exact sentence. Never empty — the schema refuses a row without one.
    pub source_span: String,
    pub source_heading: String,
    /// **Always `None` from this path.** See the module header.
    pub frequency_k: Option<u32>,
    pub frequency_n: Option<u32>,
}

const REVIEWER_SUBJECT: &[&str] =
    &["reviewer", "referee", "you should", "you are asked", "we ask you", "assessors"];

const ASSESSMENT_VERB: &[&str] = &[
    "should ", "must ", "are asked to", "consider", "assess", "evaluate", "comment on",
    "judge", "check ", "look for", "determine whether", "state whether",
];

/// Is this page reviewer guidance rather than author guidance?
///
/// **Both are guideline pages and only one populates this table.** An author
/// instruction filed as an expectation would be a requirement wearing the wrong
/// label — §7's separation lost at the first step.
/// **MEASURED AGAINST A REAL CRAWL AND IT OVER-FIRES. Do not gate anything on
/// this without reading the numbers below (§11 D163).**
///
/// Across Nature Medicine's 36 admitted pages it returns `true` for **20**:
/// all five genuine reviewer pages (`/for-reviewers/*` and
/// `editorial-policies/peer-review`) and **fifteen author pages with them** —
/// `preparing-your-submission`, `aip-and-formatting`, `matters-arising`,
/// `clinicalresearch`, `competing-interests`, `ethics-and-biosecurity`,
/// `aims/fasttrack` among them. Precision 5/20; recall 5/5. It is a filter
/// that catches everything.
///
/// The cause is the second condition: `mentions >= 2 && contains("review")`
/// over the WHOLE page text. Every page on a journal's site discusses peer
/// review somewhere, so the text half fires everywhere and only the url/title
/// half discriminates.
///
/// **This was nearly wired into the requirement path as a fix for reviewer
/// pages polluting `SectionRequired`, and it would have suppressed 14 of the
/// 18 rows the rule produces — twelve of them true.** The function's own tests
/// pass because they are hand-written fixtures built from the same premise as
/// the function; the crawl is its first independent vote (CLAUDE.md, "agreement
/// between a spec, its implementation and its test"). Reproduce with
/// `examples/journal_statement_audit.rs`, which prints the verdict for every
/// admitted page.
pub fn is_reviewer_guidance(url: &str, title: &str, text: &str) -> bool {
    let hay = format!("{url} {title}").to_lowercase();
    if ["reviewer", "referee", "peer-review", "peer review", "for-reviewers"]
        .iter()
        .any(|t| hay.contains(t))
    {
        return true;
    }
    let lower = text.to_lowercase();
    let mentions = REVIEWER_SUBJECT.iter().filter(|s| lower.contains(**s)).count();
    mentions >= 2 && lower.contains("review")
}

/// Extract expectations from a reviewer-guidance page.
///
/// Deterministic: sentences in, sentences out. **No model** — Prompt 5 item 4
/// says this phase is deterministic ingestion only, and the cloud research
/// agent is Phase 4.
pub fn extract_expectations(blocks: &[GuidelineBlock]) -> Vec<ExtractedExpectation> {
    let mut out: Vec<ExtractedExpectation> = Vec::new();
    for block in blocks {
        for sentence in block.text.split_inclusive(['.', '!', '?']) {
            let s = sentence.trim();
            let n = s.chars().count();
            if !(40..400).contains(&n) {
                continue;
            }
            let lower = s.to_lowercase();
            let addresses_reviewer = REVIEWER_SUBJECT.iter().any(|r| lower.contains(r));
            let asks_for_judgement = ASSESSMENT_VERB.iter().any(|v| lower.contains(v));
            if !(addresses_reviewer && asks_for_judgement) {
                continue;
            }
            out.push(ExtractedExpectation {
                claim: s.chars().take(240).collect(),
                source_span: s.chars().take(400).collect(),
                source_heading: block.heading.clone(),
                frequency_k: None,
                frequency_n: None,
            });
        }
    }
    out.sort_by(|a, b| a.source_span.cmp(&b.source_span));
    out.dedup();
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn b(heading: &str, text: &str) -> GuidelineBlock {
        GuidelineBlock { heading: heading.into(), text: text.into() }
    }

    #[test]
    fn a_reviewer_guidance_sentence_becomes_an_expectation_with_its_span() {
        let got = extract_expectations(&[b(
            "Guidelines for Reviewers",
            "Reviewers should consider whether the statistical analysis is appropriate for the \
             study design. The journal publishes twice monthly.",
        )]);
        assert_eq!(got.len(), 1, "{got:#?}");
        assert!(got[0].source_span.contains("statistical analysis"));
        assert_eq!(got[0].source_heading, "Guidelines for Reviewers");
        assert_eq!((got[0].frequency_k, got[0].frequency_n), (None, None));
    }

    /// **An author instruction is a REQUIREMENT, not an expectation.**
    #[test]
    fn an_author_instruction_is_not_an_expectation() {
        let got = extract_expectations(&[b(
            "Submission guidelines",
            "Authors must declare all competing interests at submission, and should format \
             references in the Vancouver style throughout the manuscript.",
        )]);
        assert!(got.is_empty(), "{got:#?}");
    }

    #[test]
    fn reviewer_guidance_is_recognised_by_url_title_or_by_who_the_page_addresses() {
        assert!(is_reviewer_guidance("https://j.test/s/reviewer-guidelines", "", ""));
        assert!(is_reviewer_guidance("https://j.test/x", "Guidelines for Reviewers", ""));
        assert!(is_reviewer_guidance(
            "https://j.test/x",
            "Editorial process",
            "Reviewers are asked to comment on novelty. Each referee receives the full text \
             for review."
        ));
    /// The over-firing is PINNED, not merely described: an author-facing page
    /// whose prose mentions reviewers and review is classified as reviewer
    /// guidance. If someone narrows the rule, this test tells them they changed
    /// the behaviour the doc comment measures.
    #[test]
    fn an_author_page_that_discusses_review_is_currently_misclassified() {
        assert!(
            is_reviewer_guidance(
                "https://www.nature.com/nm/submission-guidelines/aip-and-formatting",
                "Formatting your submission | Nature Medicine",
                "Any relevant funding should be declared in a separate funding statement. \
                 Manuscripts sent out for review are assessed by referees, and the reviewer \
                 reports are returned to the editor who handles the review.",
            ),
            "the measured over-firing has changed — update the doc comment's numbers"
        );
    }

        assert!(!is_reviewer_guidance(
            "https://j.test/s/submission-guidelines",
            "Submission Guidelines",
            "Authors must declare competing interests."
        ));
    }

    /// **THE BOUNDARY §7 EXISTS FOR.** A corpus frequency is a CONVENTION. It
    /// has no page and no sentence, so it cannot be written through this path.
    #[test]
    fn an_expectation_cannot_be_manufactured_from_a_corpus_statistic() {
        let got = extract_expectations(&[b(
            "",
            "18 of 25 comparable papers include external validation of the model.",
        )]);
        assert!(got.is_empty(), "a corpus statistic is not reviewer guidance: {got:#?}");

        let real = extract_expectations(&[b(
            "Reviewers",
            "Reviewers should assess whether the conclusions are supported by the data presented.",
        )]);
        assert_eq!(real.len(), 1);
        assert!(!real[0].source_span.is_empty());
        assert!(!real[0].claim.is_empty());
    }

    #[test]
    fn very_short_and_very_long_fragments_are_not_expectations() {
        let got = extract_expectations(&[b("Reviewers", "Reviewers should.")]);
        assert!(got.is_empty(), "too short to be a claim: {got:#?}");
        let long = format!("Reviewers should consider {}", "the analysis ".repeat(60));
        assert!(extract_expectations(&[b("Reviewers", &long)]).is_empty());
    }
}
