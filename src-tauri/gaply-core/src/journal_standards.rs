//! **Reporting-standard bindings, and the five evaluators that route on them.**
//!
//! Prompt 5 item 5: extract which standard the journal binds to which study
//! design, store it with its span, then build the five most common standards as
//! deterministic checklist evaluators over [`ResearchState`].
//!
//! # A standard NAMED is not a standard BOUND
//!
//! *"We encourage authors to follow the ARRIVE guidelines"* names a standard and
//! says nothing about when it applies. *"Observational studies (cohort,
//! case-control or cross-sectional designs) must be reported according to the
//! STROBE"* binds one. Only the second can route: an evaluator needs to know
//! which manuscripts it applies to, and a binding nobody stated is a judgement
//! nobody made.
//!
//! So an unbound mention produces NO binding. It is still stored as a
//! `reporting_standard` requirement with its span — the journal did say it —
//! but it cannot select an evaluator.
//!
//! [`ResearchState`]: crate::research_state::ResearchState

use serde::{Deserialize, Serialize};

use crate::journal_extract::{ExtractedRequirement, RequirementKind};

/// The standards this engine knows. Mirrors the schema CHECK.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum Standard {
    Consort,
    Prisma,
    Strobe,
    Arrive,
    Tripod,
    Cheers,
    Spirit,
    Stard,
}

impl Standard {
    pub fn as_str(&self) -> &'static str {
        match self {
            Standard::Consort => "CONSORT",
            Standard::Prisma => "PRISMA",
            Standard::Strobe => "STROBE",
            Standard::Arrive => "ARRIVE",
            Standard::Tripod => "TRIPOD",
            Standard::Cheers => "CHEERS",
            Standard::Spirit => "SPIRIT",
            Standard::Stard => "STARD",
        }
    }
    pub fn parse(s: &str) -> Option<Standard> {
        Some(match s.to_uppercase().as_str() {
            "CONSORT" => Standard::Consort,
            "PRISMA" => Standard::Prisma,
            "STROBE" => Standard::Strobe,
            "ARRIVE" => Standard::Arrive,
            "TRIPOD" => Standard::Tripod,
            "CHEERS" => Standard::Cheers,
            "SPIRIT" => Standard::Spirit,
            "STARD" => Standard::Stard,
            _ => return None,
        })
    }
}

/// Study designs a standard can be bound to. The phrase list is what journals
/// actually write; the canonical name is what a manuscript is matched against.
///
/// **This is a lexicon, with the same shape as the crawl's topic list and a
/// different failure direction.** A phrase missing here means a binding is NOT
/// made, so the cost is a standard that cannot route rather than one that
/// routes wrongly — and the unbound mention is still stored with its span, so
/// the loss is visible in the row.
///
/// Two entries came from the measurement rather than from guessing. Nature
/// Medicine writes *"cohort, case-control or cross-sectional designs"*, not
/// "cohort study"; PLOS Medicine writes *"Studies including animals"*, not
/// "animal research". Both were missed by the first list.
const DESIGN_PHRASES: &[(&str, &str)] = &[
    ("randomised controlled trial", "randomised trial"),
    ("randomized controlled trial", "randomised trial"),
    ("randomised trial", "randomised trial"),
    ("randomized trial", "randomised trial"),
    ("clinical trial", "clinical trial"),
    ("systematic review", "systematic review"),
    ("meta-analys", "meta-analysis"),
    ("observational stud", "observational study"),
    // Bare, because journals enumerate designs rather than name them in full:
    // "cohort, case-control or cross-sectional designs".
    ("cohort", "cohort study"),
    ("case-control", "case-control study"),
    ("cross-sectional", "cross-sectional study"),
    // "animal" also matches "animals", which is how PLOS Medicine writes it:
    // "Studies including animals must follow the ARRIVE guidelines".
    ("animal", "animal study"),
    ("in vivo", "animal study"),
    ("prediction model", "prediction model study"),
    ("diagnostic accuracy", "diagnostic accuracy study"),
    ("economic evaluation", "economic evaluation"),
    ("trial protocol", "trial protocol"),
    ("study protocol", "trial protocol"),
];

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub struct StandardBinding {
    pub standard: Standard,
    /// Canonical design name.
    pub design: String,
    /// The sentence that binds them. Never empty — the schema refuses a row
    /// without one, and an unquoted binding is an assertion.
    pub source_span: String,
}

/// Bind standards to designs from already-extracted requirements.
///
/// **Reads the span, not the value.** The value is just `"CONSORT"`; which
/// design it applies to is only in the sentence.
pub fn bindings_from(reqs: &[ExtractedRequirement]) -> Vec<StandardBinding> {
    let mut out: Vec<StandardBinding> = Vec::new();
    for r in reqs.iter().filter(|r| r.kind == RequirementKind::ReportingStandard) {
        let Some(standard) = Standard::parse(&r.value) else { continue };
        let lower = r.source_span.to_lowercase();
        // A sentence may bind one standard to several designs — STROBE to
        // "cohort, case-control or cross-sectional" — and each is a binding.
        let mut designs: Vec<&str> = Vec::new();
        for (phrase, canonical) in DESIGN_PHRASES {
            if lower.contains(phrase) && !designs.contains(canonical) {
                designs.push(canonical);
            }
        }
        for d in designs {
            out.push(StandardBinding {
                standard,
                design: d.to_string(),
                source_span: r.source_span.clone(),
            });
        }
    }
    out.sort();
    out.dedup();
    out
}

// ---------------------------------------------------------------------------
// The evaluators
// ---------------------------------------------------------------------------

/// One checklist item a standard requires, and what in the research state it
/// is checked against.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct StandardItem {
    /// The standard's own item label, as its published checklist numbers it.
    pub item: &'static str,
    pub requirement: &'static str,
    /// The `ResearchState` field(s) this reads. **Named, so a reader can see
    /// what the verdict rests on** — and so an item with nothing to read is
    /// visibly unevaluable rather than silently passing.
    pub reads: &'static [&'static str],
}

/// The five most common standards, as item lists.
///
/// **These are NOT the full published checklists.** CONSORT 2010 has 25 items
/// and 37 sub-items; PRISMA 2020 has 27. What is here is the subset whose
/// evidence `ResearchState` actually carries — and the count is stated beside
/// each so nobody reads a passing evaluator as a passed checklist.
pub fn items_for(standard: Standard) -> &'static [StandardItem] {
    match standard {
        Standard::Consort => CONSORT,
        Standard::Prisma => PRISMA,
        Standard::Strobe => STROBE,
        Standard::Arrive => ARRIVE,
        Standard::Tripod => TRIPOD,
        // The three beyond the five Prompt 5 asks for have no evaluator yet;
        // an empty list is visible where a wrong one would not be.
        Standard::Cheers | Standard::Spirit | Standard::Stard => &[],
    }
}

const CONSORT: &[StandardItem] = &[
    StandardItem { item: "1b", requirement: "Structured summary of trial design, methods, results, and conclusions", reads: &["structure", "sections"] },
    StandardItem { item: "6a", requirement: "Completely defined pre-specified primary and secondary outcome measures", reads: &["science.variables"] },
    StandardItem { item: "7a", requirement: "How sample size was determined", reads: &["science.methods", "statistics"] },
    StandardItem { item: "12a", requirement: "Statistical methods used to compare groups for primary and secondary outcomes", reads: &["science.methods", "statistics"] },
    StandardItem { item: "17a", requirement: "For each outcome, results for each group and the estimated effect size and its precision", reads: &["statistics"] },
];

const PRISMA: &[StandardItem] = &[
    StandardItem { item: "5", requirement: "Specify the inclusion and exclusion criteria for the review", reads: &["science.methods"] },
    StandardItem { item: "6", requirement: "Specify all databases and registers searched, with the date last searched", reads: &["science.datasets", "science.methods"] },
    StandardItem { item: "7", requirement: "Present the full search strategy for at least one database", reads: &["science.methods"] },
    StandardItem { item: "16a", requirement: "Describe the results of the search and selection process", reads: &["structure", "statistics"] },
    StandardItem { item: "20b", requirement: "Present results of all statistical syntheses conducted", reads: &["statistics"] },
];

const STROBE: &[StandardItem] = &[
    StandardItem { item: "4", requirement: "Present key elements of study design early in the paper", reads: &["science.methods"] },
    StandardItem { item: "6a", requirement: "Give the eligibility criteria, and the sources and methods of selection of participants", reads: &["science.methods"] },
    StandardItem { item: "7", requirement: "Clearly define all outcomes, exposures, predictors, and potential confounders", reads: &["science.variables"] },
    StandardItem { item: "12a", requirement: "Describe all statistical methods, including those used to control for confounding", reads: &["science.methods", "statistics"] },
    StandardItem { item: "16a", requirement: "Give unadjusted and confounder-adjusted estimates and their precision", reads: &["statistics"] },
];

const ARRIVE: &[StandardItem] = &[
    StandardItem { item: "1", requirement: "The study design, including the number of experimental and control groups", reads: &["science.methods"] },
    StandardItem { item: "2a", requirement: "The experimental unit and the total number of animals used", reads: &["science.methods", "statistics"] },
    StandardItem { item: "3", requirement: "Inclusion and exclusion criteria for animals and data points", reads: &["science.methods"] },
    StandardItem { item: "5", requirement: "The statistical methods used for each analysis", reads: &["science.methods", "statistics"] },
    StandardItem { item: "7", requirement: "Details of the animals used, including species, strain, sex and age", reads: &["science.variables"] },
];

const TRIPOD: &[StandardItem] = &[
    StandardItem { item: "4a", requirement: "Describe the study design or source of data", reads: &["science.datasets", "science.methods"] },
    StandardItem { item: "5a", requirement: "Specify key elements of the study setting", reads: &["science.methods"] },
    StandardItem { item: "7a", requirement: "Clearly define all predictors used in the model", reads: &["science.variables"] },
    StandardItem { item: "10b", requirement: "Specify the type of model, all model-building procedures, and the method for internal validation", reads: &["science.methods"] },
    StandardItem { item: "16", requirement: "Report performance measures for the prediction model, with confidence intervals", reads: &["statistics"] },
];

/// How many items the published standard actually has, so a report can say
/// what fraction is being evaluated. **A passing evaluator is not a passed
/// checklist**, and the only defence against reading it as one is the ratio.
pub fn published_item_count(standard: Standard) -> Option<usize> {
    Some(match standard {
        Standard::Consort => 25,
        Standard::Prisma => 27,
        Standard::Strobe => 22,
        Standard::Arrive => 21,
        Standard::Tripod => 22,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::journal_extract::RequirementKind;

    fn req(value: &str, span: &str) -> ExtractedRequirement {
        ExtractedRequirement {
            kind: RequirementKind::ReportingStandard,
            value: value.into(),
            article_type: None,
            source_heading: "Reporting standards".into(),
            source_span: span.into(),
        }
    }

    /// **Nature Medicine's spans, verbatim.** Each names the design it applies
    /// to, which is what makes the binding mechanical rather than a judgement.
    #[test]
    fn a_span_that_names_its_design_binds() {
        let got = bindings_from(&[
            req("STROBE", "Observational studies (cohort, case-control or cross-sectional designs) must be reported according to the STROBE guidelines."),
            req("PRISMA", "Systematic reviews and meta-analyses must follow the PRISMA guidelines."),
            req("TRIPOD", "Studies developing, validating, or updating prediction models for diagnosis or prognosis must follow the TRIPOD guidelines."),
        ]);
        let pairs: Vec<(&str, &str)> =
            got.iter().map(|b| (b.standard.as_str(), b.design.as_str())).collect();

        // One sentence, three designs — each is a binding.
        assert!(pairs.contains(&("STROBE", "observational study")), "{pairs:?}");
        assert!(pairs.contains(&("STROBE", "cohort study")), "{pairs:?}");
        assert!(pairs.contains(&("STROBE", "case-control study")), "{pairs:?}");
        assert!(pairs.contains(&("STROBE", "cross-sectional study")), "{pairs:?}");
        assert!(pairs.contains(&("PRISMA", "systematic review")), "{pairs:?}");
        assert!(pairs.contains(&("PRISMA", "meta-analysis")), "{pairs:?}");
        assert!(pairs.contains(&("TRIPOD", "prediction model study")), "{pairs:?}");
        assert!(got.iter().all(|b| !b.source_span.is_empty()));
    }

    /// **A standard NAMED is not a standard BOUND.** PLOS ONE's ARRIVE
    /// sentence, verbatim: it names the standard and says nothing about when it
    /// applies, so there is nothing for an evaluator to route on.
    #[test]
    fn a_span_that_names_no_design_binds_nothing() {
        let got = bindings_from(&[req(
            "ARRIVE",
            "To maximize reproducibility and potential for re-use of data, we encourage authors \
             to follow the ARRIVE guidelines.",
        )]);
        assert!(got.is_empty(), "a mention is not a binding: {got:#?}");
    }

    /// …and the same standard IS bound where a journal says when it applies.
    #[test]
    fn the_same_standard_binds_when_the_journal_states_the_design() {
        let got = bindings_from(&[req(
            "ARRIVE",
            "Studies including animals must follow the ARRIVE guidelines and include a completed \
             ARRIVE checklist.",
        )]);
        assert_eq!(got.len(), 1, "{got:#?}");
        assert_eq!(got[0].design, "animal study");
    }

    #[test]
    fn a_requirement_that_is_not_a_reporting_standard_is_ignored() {
        let mut r = req("CONSORT", "Trials must follow CONSORT.");
        r.kind = RequirementKind::WordLimit;
        assert!(bindings_from(&[r]).is_empty());
    }

    /// **A passing evaluator is not a passed checklist.** Every evaluator
    /// carries fewer items than the published standard, and the ratio has to be
    /// available or a report will imply full compliance.
    #[test]
    fn every_evaluator_is_a_subset_and_says_by_how_much() {
        for s in [Standard::Consort, Standard::Prisma, Standard::Strobe, Standard::Arrive,
                  Standard::Tripod] {
            let items = items_for(s);
            let published = published_item_count(s).expect("a published count");
            assert!(!items.is_empty(), "{} has no evaluator", s.as_str());
            assert!(
                items.len() < published,
                "{}: {} items claimed against {published} published — an evaluator must not \
                 claim to be the whole checklist",
                s.as_str(),
                items.len()
            );
            // Every item names what it reads, so a verdict's basis is visible.
            for i in items {
                assert!(!i.reads.is_empty(), "{} item {} reads nothing", s.as_str(), i.item);
                assert!(!i.requirement.is_empty());
            }
        }
    }

    /// A standard with no evaluator returns an EMPTY list, not a wrong one.
    #[test]
    fn a_standard_without_an_evaluator_is_visibly_empty() {
        for s in [Standard::Cheers, Standard::Spirit, Standard::Stard] {
            assert!(items_for(s).is_empty(), "{} has an unvetted evaluator", s.as_str());
            assert_eq!(published_item_count(s), None);
        }
    }

    #[test]
    fn the_standard_names_match_the_schema_check() {
        let schema = include_str!("migrations.rs");
        let check = schema.split("standard     TEXT NOT NULL CHECK (standard IN (").nth(1)
            .expect("the journal_standard_bindings CHECK");
        let check: String = check.chars().take(300).collect();
        for s in [Standard::Consort, Standard::Prisma, Standard::Strobe, Standard::Arrive,
                  Standard::Tripod, Standard::Cheers, Standard::Spirit, Standard::Stard] {
            assert!(check.contains(&format!("'{}'", s.as_str())), "{} missing", s.as_str());
        }
    }
}
