//! **Frequentist inference specialist — §4.2's first fan-out.**
//!
//! Tier 1 (rule), `model: None`. Every check below is a decision over text and
//! over already-extracted statistics; nothing here calls a model, and the
//! findings are reproducible from the manuscript alone.
//!
//! # WHAT IT DELIBERATELY DOES NOT CHECK
//!
//! `validate.rs` already ships five deterministic rules —
//! `TestGroupMismatch`, `PValueOverclaim`, `MissingEffectSize`,
//! `MissingConfidenceInterval`, `SmallSampleCausalClaim` — and it is the
//! HARD-CONSTRAINT lane (§4.4 Tier 0/1, `hard_constraint: true` in the graph).
//! A specialist restating any of them would put a soft, votable opinion beside
//! a verdict that overrides consensus, on the same evidence. That is worse than
//! a gap: it makes the override look contested. So the four checks here are the
//! ones `validate.rs` does not make.
//!
//! # THE THRESHOLD IS DERIVED, NOT CHOSEN
//!
//! [`FAMILYWISE_TRIGGER`] is 14 because that is where the familywise error rate
//! of independent tests at α = 0.05 passes one half: 1 − 0.95¹⁴ = 0.512. An
//! invented number in a design document is the thing this project exists not to
//! do, and a threshold is a number in a design document that happens to compile.

use crate::agent_graph::{Cluster, EvidencePolicy, EvidenceSource, TrustTier};
use crate::epistemic::EpistemicStatus;
use crate::extract::{sentence, stats::Stat, Location};
use crate::report::FindingSeverity;

use super::{span_at, Finding, Specialist, SpecialistInput};

/// The number of uncorrected comparisons at which the familywise error rate of
/// independent tests at α = 0.05 exceeds one half: 1 − 0.95¹⁴ = 0.512.
pub const FAMILYWISE_TRIGGER: usize = 14;

/// Terms that show a correction was applied. Any one of them suppresses the
/// multiple-comparisons finding.
const CORRECTION_TERMS: &[&str] = &[
    "bonferroni",
    "holm",
    "šidák",
    "sidak",
    "false discovery rate",
    "benjamini",
    "hochberg",
    "tukey",
    "scheffé",
    "scheffe",
    "dunnett",
    "familywise",
    "family-wise",
    "multiple comparison",
    "multiplicity",
    "corrected for multiple",
    "adjusted p",
    "critical difference",
];

/// Terms that show a distributional assumption was examined.
const ASSUMPTION_TERMS: &[&str] = &[
    "normality",
    "normally distributed",
    "shapiro",
    "kolmogorov",
    "anderson-darling",
    "levene",
    "homogeneity of variance",
    "homoscedastic",
    "homoskedastic",
    "heteroscedastic",
    "equal variance",
    "sphericity",
    "mauchly",
    "q-q plot",
    "qq plot",
    "box-tidwell",
    "linearity",
    "multicollinearity",
    "variance inflation",
    "vif",
    "durbin-watson",
];

/// Parametric tests whose validity rests on the assumptions above.
const PARAMETRIC_TESTS: &[&str] =
    &["t-test", "t test", "anova", "ancova", "manova", "regression", "pearson", "f-test"];

/// Language that reports a non-significant result as though it were nearly
/// significant.
const MARGINAL_PHRASES: &[&str] = &[
    "marginally significant",
    "approaching significance",
    "approached significance",
    "trend towards significance",
    "trend toward significance",
    "nearly significant",
    "borderline significant",
    "close to significance",
];

pub struct FrequentistSpecialist;

impl Specialist for FrequentistSpecialist {
    fn id(&self) -> &'static str {
        "frequentist_stats"
    }
    fn cluster(&self) -> Cluster {
        Cluster::MethodologicalSoundness
    }
    fn trust_tier(&self) -> TrustTier {
        TrustTier::Rule
    }
    /// One manuscript span per finding, and every finding states what it could
    /// not determine — both checks below reason from ABSENCE of a term, which
    /// is exactly the reasoning that must declare its own limits.
    fn evidence_policy(&self) -> EvidencePolicy {
        EvidencePolicy {
            min_sources: 1,
            permitted_sources: vec![
                EvidenceSource::ManuscriptSpan,
                EvidenceSource::AnalysisRecordEntry,
            ],
            citation_required: true,
            uncertainty_required: true,
        }
    }

    fn assess(&self, input: &SpecialistInput<'_>) -> Vec<Finding> {
        let r = input.extraction;
        let mut out = Vec::new();
        let lower = lowercased_body(input);

        // --- 1. p = 0.000 is not a p-value ---------------------------------
        //
        // SPSS prints `.000` for any p below its display precision, and it is
        // copied into manuscripts verbatim. A probability of exactly zero is not
        // a thing a test can produce; the correct report is `p < 0.001`.
        for s in &r.statistics {
            let Stat::PValue { operator, value, raw } = &s.stat else { continue };
            if operator.contains('=') && *value == 0.0 {
                out.push(Finding {
                    specialist: self.id().into(),
                    code: "p_value_reported_as_exactly_zero".into(),
                    severity: FindingSeverity::Minor,
                    status: EpistemicStatus::Detected,
                    summary: format!(
                        "`{raw}` reports a probability of exactly zero. No test produces one; \
                         this is a statistics package's display precision (SPSS prints `.000`) \
                         copied verbatim. The reportable form is `p < 0.001`."
                    ),
                    span: span_at(r, &s.location),
                    location: Some(s.location.clone()),
                    evidence: vec![EvidenceSource::ManuscriptSpan],
                    uncertainty: Some(
                        "Reads the reported string only. It cannot tell a rounding artefact \
                         from a transcription error, and does not know the true p."
                            .into(),
                    ),
                });
            }
        }

        // --- 2. many comparisons, no correction named -----------------------
        let p_values: Vec<&crate::extract::stats::StatClaim> = r
            .statistics
            .iter()
            .filter(|s| matches!(s.stat, Stat::PValue { .. }))
            .collect();
        if p_values.len() >= FAMILYWISE_TRIGGER
            && !CORRECTION_TERMS.iter().any(|t| lower.contains(t))
        {
            // The row the pipeline would surface: the FIRST reported p-value,
            // with its paragraph. Not one chosen for reading well.
            let anchor = p_values[0];
            out.push(Finding {
                specialist: self.id().into(),
                code: "multiple_comparisons_uncorrected".into(),
                severity: FindingSeverity::Major,
                status: EpistemicStatus::Detected,
                summary: format!(
                    "{} p-values are reported and no correction for multiple comparisons is \
                     named anywhere in the manuscript. At α = 0.05 the familywise error rate \
                     of {} independent tests is {:.0}%.",
                    p_values.len(),
                    p_values.len(),
                    (1.0 - 0.95_f64.powi(p_values.len() as i32)) * 100.0
                ),
                span: span_at(r, &anchor.location),
                location: Some(anchor.location.clone()),
                evidence: vec![EvidenceSource::ManuscriptSpan],
                uncertainty: Some(format!(
                    "Counts every p-value the extractor found and assumes the tests are \
                     independent. Three things inflate the figure and are not separated: \
                     p-values quoted from prior work, per-coefficient p-values within one \
                     model, and goodness-of-fit diagnostics — a Hosmer–Lemeshow or \
                     Box–Tidwell p is not a comparison. It searches for {} correction terms \
                     and cannot see a correction described in words none of them match.",
                    CORRECTION_TERMS.len()
                )),
            });
        }

        // --- 3. a parametric test with no assumption check stated -----------
        if !ASSUMPTION_TERMS.iter().any(|t| lower.contains(t)) {
            if let Some((test_name, loc)) = first_parametric_test(r) {
                out.push(Finding {
                    specialist: self.id().into(),
                    code: "parametric_test_assumptions_unstated".into(),
                    severity: FindingSeverity::Major,
                    status: EpistemicStatus::Detected,
                    summary: format!(
                        "The manuscript reports a {test_name} and states no check of the \
                         distributional assumptions it rests on — no normality, variance \
                         homogeneity, sphericity or linearity check is named anywhere."
                    ),
                    span: span_at(r, &loc),
                    location: Some(loc),
                    evidence: vec![EvidenceSource::ManuscriptSpan],
                    uncertainty: Some(
                        "Absence of a term is not absence of the check: it may have been run \
                         and not reported, which is a reporting finding rather than a \
                         methodological one. The specialist cannot tell the two apart."
                            .into(),
                    ),
                });
            }
        }

        // --- 4. a non-significant result described as nearly significant ----
        for (phrase, loc, text) in marginal_phrases(input) {
            out.push(Finding {
                specialist: self.id().into(),
                code: "marginal_significance_language".into(),
                severity: FindingSeverity::Minor,
                status: EpistemicStatus::Detected,
                summary: format!(
                    "\"{phrase}\" describes a result that did not meet the stated threshold as \
                     though it partly did. Under a frequentist test a result is significant at \
                     the chosen α or it is not; the effect size and interval carry the degree."
                ),
                span: Some(text),
                location: Some(loc),
                evidence: vec![EvidenceSource::ManuscriptSpan],
                uncertainty: Some(
                    "Matches the phrase, not the p-value it refers to. A manuscript \
                     discussing this practice rather than using it would match identically."
                        .into(),
                ),
            });
        }

        // --- 5. a test claimed in the paper and absent from the uploaded
        //        analysis. THE DIFFERENTIATOR of §1, and it only runs when a
        //        record exists — which is no run today (see `analysis::`).
        if let Some(record) = input.analysis {
            out.extend(tests_absent_from_the_record(self.id(), input, record));
        }

        out
    }
}

/// The manuscript's body text, lowercased once. Built once per assessment
/// rather than per check: four passes over a 400-page paper for the same string
/// is the regex-compilation mistake in a different costume.
fn lowercased_body(input: &SpecialistInput<'_>) -> String {
    let mut s = String::new();
    for sec in &input.extraction.sections {
        if sec.kind == crate::extract::SectionKind::References {
            continue;
        }
        for p in &sec.paragraphs {
            s.push_str(&p.to_lowercase());
            s.push('\n');
        }
    }
    s
}

/// The first parametric test named, with where it was named.
fn first_parametric_test(
    r: &crate::extract::ExtractionResult,
) -> Option<(String, Location)> {
    for s in &r.statistics {
        let name = match &s.stat {
            Stat::Test { name, .. } => name.clone(),
            Stat::TestStatistic { name, .. } => name.clone(),
            _ => continue,
        };
        let lower = name.to_lowercase();
        if PARAMETRIC_TESTS.iter().any(|t| lower.contains(t)) {
            return Some((name, s.location.clone()));
        }
    }
    None
}

/// Every marginal-significance phrase, with the SENTENCE it sits in.
///
/// The sentence, not the paragraph: this finding is about a form of words, and
/// quoting 400 words around it makes the reader hunt for the eight that matter.
fn marginal_phrases(input: &SpecialistInput<'_>) -> Vec<(String, Location, String)> {
    let mut out = Vec::new();
    for (sec_idx, sec) in input.extraction.sections.iter().enumerate() {
        if sec.kind == crate::extract::SectionKind::References {
            continue;
        }
        for (p_idx, para) in sec.paragraphs.iter().enumerate() {
            for sent in sentence::sentences_in(para) {
                let lower = sent.to_lowercase();
                if let Some(phrase) = MARGINAL_PHRASES.iter().find(|p| lower.contains(**p)) {
                    out.push((
                        (*phrase).to_string(),
                        Location::in_section(sec.kind, sec_idx, p_idx),
                        sent.trim().to_string(),
                    ));
                }
            }
        }
    }
    out
}

/// A test the manuscript names that no procedure in the uploaded analysis
/// performs.
///
/// **UNREACHABLE TODAY, and saying so is the point.** `input.analysis` is `None`
/// on every run because no upload path accepts an analysis file. The check is
/// written because it is the §1 differentiator — *"a reviewer who can see the
/// code checks the statistics against what was actually run"* — and because the
/// shape of the record is decided by what this needs from it. It has never run
/// on a real pair, and nothing here should be read as though it had.
fn tests_absent_from_the_record(
    id: &str,
    input: &SpecialistInput<'_>,
    record: &crate::analysis::AnalysisRecord,
) -> Vec<Finding> {
    use crate::analysis::ProcedureKind;
    let mut out = Vec::new();
    let kinds = record.kinds();
    for s in &input.extraction.statistics {
        let Stat::Test { name, raw } = &s.stat else { continue };
        let lower = name.to_lowercase();
        let expected = if lower.contains("anova") {
            ProcedureKind::OneWayAnova
        } else if lower.contains("regression") {
            ProcedureKind::LinearRegression
        } else if lower.contains("t-test") || lower.contains("t test") {
            ProcedureKind::TTest
        } else if lower.contains("factor") {
            ProcedureKind::FactorAnalysis
        } else if lower.contains("chi") {
            ProcedureKind::CrossTabulation
        } else {
            continue;
        };
        let also_glm = expected == ProcedureKind::OneWayAnova
            && kinds.contains(&ProcedureKind::GeneralLinearModel);
        if kinds.contains(&expected) || also_glm {
            continue;
        }
        out.push(Finding {
            specialist: id.into(),
            code: "test_claimed_but_absent_from_analysis".into(),
            severity: FindingSeverity::Major,
            status: EpistemicStatus::Detected,
            summary: format!(
                "The manuscript reports a {name} (`{raw}`), and no {expected:?} appears among \
                 the {} procedure(s) in the uploaded analysis.",
                record.statistical().count()
            ),
            span: span_at(input.extraction, &s.location),
            location: Some(s.location.clone()),
            evidence: vec![
                EvidenceSource::ManuscriptSpan,
                EvidenceSource::AnalysisRecordEntry,
            ],
            uncertainty: Some(
                "The upload may be a subset of the work, and the same test can be run under a \
                 procedure this mapping does not name. Absence from the record is not proof \
                 the test was not run."
                    .into(),
            ),
        });
    }
    out
}

#[cfg(test)]
mod tests {
    //! **The discriminating cases are taken from the real corpus, not invented.**
    //!
    //! The fixtures below are the shapes the six real manuscripts actually
    //! have — measured, `examples/specialist_probe.rs` — because a hand-written
    //! fixture inherits its author's premise and a classifier that fires on
    //! everything passes every test its author writes. The corpus row that
    //! matters most is `final final L.pdf`: **74 p-values, the highest count in
    //! the set, and the check stays silent because the paper names Holm.** A
    //! filter that caught everything would fire there.

    use super::*;
    use crate::extract;
    use crate::specialist::{run, SpecialistInput};

    fn assess(text: &str) -> Vec<Finding> {
        let r = extract::extract_from_text(text);
        let input = SpecialistInput { extraction: &r, science: None, analysis: None };
        FrequentistSpecialist.assess(&input)
    }

    fn codes(text: &str) -> Vec<String> {
        assess(text).into_iter().map(|f| f.code).collect()
    }

    fn many_p_values(n: usize) -> String {
        let body: String =
            (0..n).map(|i| format!("Group {i} differed (p = 0.0{}). ", i % 5 + 1)).collect();
        format!("Results\n\n{body}\n")
    }

    /// **The corpus's discriminating negative.** `final final L.pdf` reports 74
    /// p-values and names Holm; it must stay silent, or the check is a filter
    /// that catches everything.
    #[test]
    fn a_correction_that_is_named_suppresses_the_multiple_comparisons_finding() {
        let many = many_p_values(30);
        assert!(
            codes(&many).contains(&"multiple_comparisons_uncorrected".to_string()),
            "the positive control: 30 uncorrected p-values must fire"
        );
        let corrected = format!("{many}\nP-values were adjusted by the Holm procedure.\n");
        assert!(
            !codes(&corrected).contains(&"multiple_comparisons_uncorrected".to_string()),
            "naming a correction must suppress it — this is the `final final L` row"
        );
    }

    /// The trigger is derived, so the boundary is worth pinning: one below is
    /// silent, the trigger fires.
    #[test]
    fn the_trigger_is_the_familywise_half_point_and_the_boundary_holds() {
        assert!(
            (1.0 - 0.95_f64.powi(FAMILYWISE_TRIGGER as i32)) > 0.5,
            "the constant must be where the familywise rate passes one half"
        );
        assert!(
            (1.0 - 0.95_f64.powi(FAMILYWISE_TRIGGER as i32 - 1)) < 0.5,
            "and one fewer must be below it, or the constant is not that point"
        );
        let below = many_p_values(FAMILYWISE_TRIGGER - 1);
        assert!(!codes(&below).contains(&"multiple_comparisons_uncorrected".to_string()));
        let at = many_p_values(FAMILYWISE_TRIGGER);
        assert!(codes(&at).contains(&"multiple_comparisons_uncorrected".to_string()));
    }

    /// **The `IJAS` row.** ANOVA reported, no distributional check named.
    /// The sentence is the manuscript's own.
    #[test]
    fn a_parametric_test_with_no_assumption_check_is_found() {
        let text = "Methods\n\nThe data of each season were subjected to analysis of variance \
                    appropriate to a three-factor completely randomised design and treatment \
                    means were compared by the critical difference at p ≤ 0.05.\n";
        assert!(codes(text).contains(&"parametric_test_assumptions_unstated".to_string()));

        // And naming ONE assumption check suppresses it — the Health Economics
        // row, where VIF and multicollinearity are stated.
        let checked = format!("{text}\nMulticollinearity was assessed by variance inflation factors.\n");
        assert!(
            !codes(&checked).contains(&"parametric_test_assumptions_unstated".to_string()),
            "a stated check must suppress the finding"
        );
    }

    /// **The `chapter3` row.** Kruskal-Wallis is non-parametric; the assumption
    /// finding must not fire on it. A check that fired on every test named
    /// would be right about parametric papers by accident.
    #[test]
    fn a_nonparametric_test_does_not_trigger_the_assumption_finding() {
        let text = "Methods\n\nDifferences between stations were tested with the \
                    Kruskal-Wallis test.\n";
        assert!(
            !codes(text).contains(&"parametric_test_assumptions_unstated".to_string()),
            "Kruskal-Wallis assumes no distribution: {:?}",
            codes(text)
        );
    }

    #[test]
    fn a_p_value_of_exactly_zero_is_found_and_p_less_than_is_not() {
        assert!(codes("Results\n\nThe effect was significant (p = 0.000).\n")
            .contains(&"p_value_reported_as_exactly_zero".to_string()));
        assert!(
            !codes("Results\n\nThe effect was significant (p < 0.001).\n")
                .contains(&"p_value_reported_as_exactly_zero".to_string()),
            "`p < 0.001` is the CORRECT form and must never be flagged"
        );
    }

    #[test]
    fn marginal_significance_language_is_found_with_its_sentence_not_its_paragraph() {
        let text = "Results\n\nThe first comparison was clear. The second was marginally \
                    significant (p = 0.061). A third sentence follows.\n";
        let f = assess(text)
            .into_iter()
            .find(|f| f.code == "marginal_significance_language")
            .expect("must be found");
        let span = f.span.expect("citation required");
        assert!(span.contains("marginally significant"));
        assert!(
            !span.contains("A third sentence"),
            "the span is the sentence, not the paragraph — 400 words around eight that \
             matter makes the reader hunt: {span}"
        );
    }

    /// **Every finding carries a span and a stated uncertainty, or the gate
    /// drops it.** Both are `true` in this specialist's policy, so a check added
    /// without them is silently disabled — which is why this asserts over the
    /// ADMITTED set of a real run rather than over `assess`.
    #[test]
    fn every_admitted_finding_carries_its_span_and_its_uncertainty() {
        let text = format!(
            "{}\nMethods\n\nMeans were compared by analysis of variance. The difference was \
             marginally significant. One estimate gave p = 0.000.\n",
            many_p_values(20)
        );
        let r = extract::extract_from_text(&text);
        let input = SpecialistInput { extraction: &r, science: None, analysis: None };
        let report = run(&FrequentistSpecialist, &input);
        assert!(report.admitted.len() >= 3, "several checks must fire: {report:?}");
        for f in &report.admitted {
            assert!(f.span.is_some(), "{} carries no span", f.code);
            assert!(f.uncertainty.is_some(), "{} states no uncertainty", f.code);
            assert!(!f.evidence.is_empty(), "{} carries no evidence kind", f.code);
        }
    }

    /// The record-backed check is UNREACHABLE on a real run and must be
    /// exercised somewhere, or its first execution is in front of a researcher.
    #[test]
    fn a_test_named_in_the_paper_and_absent_from_the_upload_is_found() {
        use crate::analysis::spss;
        let record = spss::parse("s.sps", "FACTOR\n  /VARIABLES V1 V2\n  /ROTATION VARIMAX.\n");
        let r = extract::extract_from_text(
            "Methods\n\nGroup means were compared by one-way ANOVA.\n",
        );
        let input =
            SpecialistInput { extraction: &r, science: None, analysis: Some(&record) };
        let codes: Vec<String> =
            FrequentistSpecialist.assess(&input).into_iter().map(|f| f.code).collect();
        assert!(
            codes.contains(&"test_claimed_but_absent_from_analysis".to_string()),
            "an ANOVA claimed against an upload containing only a factor analysis: {codes:?}"
        );

        // And it must NOT fire when the record does contain the test.
        let with = spss::parse("s.sps", "ONEWAY V1 BY GROUP\n  /STATISTICS DESCRIPTIVES.\n");
        let input = SpecialistInput { extraction: &r, science: None, analysis: Some(&with) };
        let codes: Vec<String> =
            FrequentistSpecialist.assess(&input).into_iter().map(|f| f.code).collect();
        assert!(
            !codes.contains(&"test_claimed_but_absent_from_analysis".to_string()),
            "the ONEWAY is right there: {codes:?}"
        );
    }
}
