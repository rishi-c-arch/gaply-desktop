//! **Machine-learning methodology specialist — §4.2's second fan-out.**
//!
//! Tier 1 (rule), `model: None`. The four checks are the ones a reviewer of an
//! ML paper raises first, and each is decidable from the manuscript's own
//! sentences: whether resampling was described as happening inside the training
//! fold, whether a baseline was compared against, whether the headline number
//! is a single run, and whether a held-out evaluation is named at all.
//!
//! # IT MUST DECIDE WHETHER THE PAPER IS AN ML PAPER FIRST
//!
//! Four of the six real manuscripts are a silkworm biochemistry paper, two lake
//! chapters and a health-economics survey. Run over them, a specialist that
//! looks only for a missing baseline reports *"no baseline comparison"* on all
//! four, correctly and uselessly — the finding a filter that catches everything
//! produces. [`is_machine_learning_paper`] is the gate, and
//! [`super::SpecialistReport::not_applicable`] is how a specialist that declined
//! to run says so, distinguishably from one that ran and found nothing.

use crate::agent_graph::{Cluster, EvidencePolicy, EvidenceSource, TrustTier};
use crate::epistemic::EpistemicStatus;
use crate::extract::{sentence, Location, SectionKind};
use crate::report::FindingSeverity;

use super::{Finding, Specialist, SpecialistInput};

/// Terms that mark a manuscript as reporting a trained model. Two distinct ones
/// are required — a single mention of "machine learning" in an introduction is
/// a literature reference, not a method.
const ML_TERMS: &[&str] = &[
    "machine learning",
    "deep learning",
    "neural network",
    "training set",
    "training data",
    "test set",
    "classifier",
    "lstm",
    "transformer",
    "random forest",
    "gradient boosting",
    "xgboost",
    "support vector",
    "convolutional",
    "embedding",
    "fine-tun",
    "epochs",
    "hyperparameter",
];

/// Resampling and rescaling steps that leak when applied before the split.
const RESAMPLING_TERMS: &[&str] = &[
    "smote",
    "adasyn",
    "oversampl",
    "over-sampl",
    "undersampl",
    "under-sampl",
    "class balancing",
    "class-balancing",
    "resampling",
    "data augmentation",
];

/// Phrasing that shows the resampling was confined to the training data.
const LEAKAGE_GUARDS: &[&str] = &[
    "training set only",
    "only the training",
    "within each fold",
    "inside the fold",
    "within the training",
    "on the training set",
    "after the split",
    "after splitting",
    "training fold",
    "training partition",
    "not applied to the test",
    "test set was left",
];

/// Evidence that a held-out evaluation exists.
const HELDOUT_TERMS: &[&str] = &[
    "train/test",
    "train-test",
    "training and test",
    "held-out",
    "holdout",
    "hold-out",
    "cross-validation",
    "cross validation",
    "k-fold",
    "stratified split",
    "validation set",
    "test split",
    "unseen data",
];

/// Evidence that the result was compared against something.
const BASELINE_TERMS: &[&str] = &[
    "baseline",
    "state-of-the-art",
    "state of the art",
    "compared with existing",
    "compared to existing",
    "benchmark model",
    "prior model",
    "ablation",
];

/// Evidence that variability across runs was reported.
const VARIANCE_TERMS: &[&str] = &[
    "standard deviation",
    "std dev",
    "±",
    "+/-",
    "averaged over",
    "mean over",
    "across seeds",
    "random seeds",
    "repeated runs",
    "confidence interval",
    "error bar",
    "five runs",
    "ten runs",
];

pub struct MachineLearningSpecialist;

impl Specialist for MachineLearningSpecialist {
    fn id(&self) -> &'static str {
        "ml_methodology"
    }
    fn cluster(&self) -> Cluster {
        Cluster::MethodologicalSoundness
    }
    fn trust_tier(&self) -> TrustTier {
        TrustTier::Rule
    }
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

    /// Four of the six real manuscripts are not ML papers. Declining is a
    /// different answer from finding nothing, and this is where it is said.
    fn applies_to(&self, input: &SpecialistInput<'_>) -> Option<String> {
        let sentences = body_sentences(input);
        let lower = lowercased(&sentences);
        if is_machine_learning_paper(&lower) {
            None
        } else {
            Some(format!(
                "fewer than two of {} machine-learning terms appear — this is not a paper \
                 reporting a trained model, and every check below would fire correctly and \
                 uselessly",
                ML_TERMS.len()
            ))
        }
    }

    fn assess(&self, input: &SpecialistInput<'_>) -> Vec<Finding> {
        let sentences = body_sentences(input);
        let lower = lowercased(&sentences);
        let mut out = Vec::new();

        // --- 1. resampling with no statement that it followed the split -----
        //
        // The single commonest leak in an applied ML paper: SMOTE over the whole
        // dataset before splitting puts synthetic neighbours of test rows into
        // training, and the reported accuracy is then partly memorised.
        if let Some((term, loc, text)) = first_match(&sentences, RESAMPLING_TERMS) {
            if !LEAKAGE_GUARDS.iter().any(|g| lower.contains(g)) {
                out.push(Finding {
                    specialist: self.id().into(),
                    code: "resampling_not_stated_as_post_split".into(),
                    severity: FindingSeverity::Major,
                    status: EpistemicStatus::Detected,
                    summary: format!(
                        "The manuscript applies {term} and never states that it was confined to \
                         the training data. Resampling the full dataset before the split places \
                         synthetic neighbours of test rows into training, and the reported \
                         performance then includes what the model memorised."
                    ),
                    span: Some(text),
                    location: Some(loc),
                    evidence: vec![EvidenceSource::ManuscriptSpan],
                    uncertainty: Some(
                        "Reads the manuscript's description, not the code. The split may have \
                         come first and gone unstated, which is a reporting finding rather than \
                         a leak. Only the analysis record could separate them, and none is \
                         uploaded."
                            .into(),
                    ),
                });
            }
        }

        // --- 2. no held-out evaluation named --------------------------------
        if !HELDOUT_TERMS.iter().any(|t| lower.contains(t)) {
            if let Some((_, loc, text)) = first_match(&sentences, ML_TERMS) {
                out.push(Finding {
                    specialist: self.id().into(),
                    code: "no_heldout_evaluation_named".into(),
                    severity: FindingSeverity::Major,
                    status: EpistemicStatus::Detected,
                    summary: "The manuscript reports a trained model and names no held-out \
                              evaluation — no train/test split, cross-validation, validation set \
                              or unseen data. Performance measured on the data a model was fitted \
                              to is not an estimate of its performance."
                        .into(),
                    span: Some(text),
                    location: Some(loc),
                    evidence: vec![EvidenceSource::ManuscriptSpan],
                    uncertainty: Some(format!(
                        "Searches for {} phrasings of a held-out evaluation. A split described \
                         in words none of them match would be missed.",
                        HELDOUT_TERMS.len()
                    )),
                });
            }
        }

        // --- 3. no baseline ------------------------------------------------
        if !BASELINE_TERMS.iter().any(|t| lower.contains(t)) {
            if let Some((_, loc, text)) = first_match(&sentences, ML_TERMS) {
                out.push(Finding {
                    specialist: self.id().into(),
                    code: "no_baseline_comparison".into(),
                    severity: FindingSeverity::Minor,
                    status: EpistemicStatus::Detected,
                    summary: "No baseline, prior model or ablation is named. A reported accuracy \
                              with nothing to compare it against does not establish that the \
                              contribution is responsible for it."
                        .into(),
                    span: Some(text),
                    location: Some(loc),
                    evidence: vec![EvidenceSource::ManuscriptSpan],
                    uncertainty: Some(
                        "A comparison against named prior systems, without the word `baseline`, \
                         would be missed."
                            .into(),
                    ),
                });
            }
        }

        // --- 4. a headline number from a single run -------------------------
        if !VARIANCE_TERMS.iter().any(|t| lower.contains(t)) {
            if let Some((_, loc, text)) = first_match(&sentences, &["accuracy", "f1-score", "f1 score", "macro-f1"]) {
                out.push(Finding {
                    specialist: self.id().into(),
                    code: "no_variance_across_runs".into(),
                    severity: FindingSeverity::Minor,
                    status: EpistemicStatus::Detected,
                    summary: "Performance is reported as a single number with no variability — \
                              no standard deviation, interval, or mean across seeds or folds. A \
                              difference smaller than run-to-run variance is not a difference."
                        .into(),
                    span: Some(text),
                    location: Some(loc),
                    evidence: vec![EvidenceSource::ManuscriptSpan],
                    uncertainty: Some(
                        "Detects the ABSENCE of variability language over the whole manuscript. \
                         Variance reported only inside a table the extractor flattened would be \
                         missed."
                            .into(),
                    ),
                });
            }
        }

        out
    }
}

/// **Is this a paper about a trained model at all?**
///
/// Two DISTINCT terms, not two occurrences of one: a single mention of
/// "machine learning" in a literature review is a citation, and a specialist
/// that fires on it is `is_reviewer_guidance` again — the classifier that
/// returned `true` for 20 of 36 pages because one clause matched everywhere.
pub fn is_machine_learning_paper(lowercased_body: &str) -> bool {
    ML_TERMS.iter().filter(|t| lowercased_body.contains(**t)).count() >= 2
}

fn lowercased(sentences: &[(Location, String)]) -> String {
    sentences.iter().map(|(_, t)| t.to_lowercase()).collect::<Vec<_>>().join("\n")
}

/// Body sentences with their locations. References are excluded: a reference
/// list names every method in the field.
fn body_sentences(input: &SpecialistInput<'_>) -> Vec<(Location, String)> {
    let mut out = Vec::new();
    for (sec_idx, sec) in input.extraction.sections.iter().enumerate() {
        if sec.kind == SectionKind::References {
            continue;
        }
        for (p_idx, para) in sec.paragraphs.iter().enumerate() {
            for s in sentence::sentences_in(para) {
                out.push((Location::in_section(sec.kind, sec_idx, p_idx), s.trim().to_string()));
            }
        }
    }
    out
}

/// The first sentence containing any of `terms`, with the term that matched.
fn first_match(
    sentences: &[(Location, String)],
    terms: &[&str],
) -> Option<(String, Location, String)> {
    for (loc, text) in sentences {
        let lower = text.to_lowercase();
        if let Some(t) = terms.iter().find(|t| lower.contains(**t)) {
            return Some(((*t).to_string(), loc.clone(), text.clone()));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    //! The fixtures are the `R PAPER .docx` shapes, measured: SMOTE appears six
    //! times, "split" and "training set" zero times, `baseline` four times and
    //! `ablation` seven. Two of those are what the checks fire on and two are
    //! what they must stay silent about — a specialist that only fired would
    //! have looked identical on this manuscript.

    use super::*;
    use crate::extract;
    use crate::specialist::{run, SpecialistInput};

    const ML_BODY: &str = "Methods\n\nWe train a bidirectional LSTM classifier with GloVe \
                           embeddings over 30 epochs, tuning hyperparameters on the training \
                           data.";

    fn codes(text: &str) -> Vec<String> {
        let r = extract::extract_from_text(text);
        let input = SpecialistInput { extraction: &r, science: None, analysis: None };
        MachineLearningSpecialist.assess(&input).into_iter().map(|f| f.code).collect()
    }

    /// **The `R PAPER` finding.** SMOTE named, no statement that it followed
    /// the split.
    #[test]
    fn resampling_with_no_post_split_statement_is_found() {
        let text = format!("{ML_BODY} We applied SMOTE-Text class balancing to the corpus.\n");
        assert!(codes(&text).contains(&"resampling_not_stated_as_post_split".to_string()));
    }

    /// And the guard that must silence it. Without this, the check would be
    /// "does the paper mention SMOTE", which every paper using SMOTE does.
    #[test]
    fn stating_that_resampling_followed_the_split_suppresses_it() {
        let text = format!(
            "{ML_BODY} We applied SMOTE within each fold, on the training set only.\n"
        );
        assert!(
            !codes(&text).contains(&"resampling_not_stated_as_post_split".to_string()),
            "{:?}",
            codes(&text)
        );
    }

    /// **The two correct negatives measured on `R PAPER`**: it names a baseline
    /// and a cross-validation, so neither check may fire. These are the rows
    /// that separate a specialist from a filter.
    #[test]
    fn a_named_baseline_and_a_named_split_suppress_their_findings() {
        let bare = format!("{ML_BODY}\n");
        let c = codes(&bare);
        assert!(c.contains(&"no_baseline_comparison".to_string()), "positive control: {c:?}");
        assert!(c.contains(&"no_heldout_evaluation_named".to_string()), "positive control: {c:?}");

        let full = format!(
            "{ML_BODY} Results are reported under 5-fold cross-validation against the best \
             baseline, with an ablation over each component.\n"
        );
        let c = codes(&full);
        assert!(!c.contains(&"no_baseline_comparison".to_string()), "{c:?}");
        assert!(!c.contains(&"no_heldout_evaluation_named".to_string()), "{c:?}");
    }

    /// **The `R PAPER` variance finding**, and the form that suppresses it.
    #[test]
    fn a_single_number_fires_and_a_reported_spread_does_not() {
        let single =
            format!("{ML_BODY} The model achieved 96.42% accuracy and 95.00% F1-score.\n");
        assert!(codes(&single).contains(&"no_variance_across_runs".to_string()));

        let spread = format!(
            "{ML_BODY} The model achieved 96.42% accuracy, averaged over five random seeds \
             with a standard deviation of 0.31.\n"
        );
        assert!(!codes(&spread).contains(&"no_variance_across_runs".to_string()));
    }

    /// **Four of the six real manuscripts are not ML papers.** One mention of
    /// the phrase in a literature review is a citation, not a method — the
    /// `is_reviewer_guidance` failure, where one clause matched every page.
    #[test]
    fn one_mention_is_not_an_ml_paper_and_two_distinct_terms_are() {
        assert!(!is_machine_learning_paper(
            "prior work has applied machine learning to this problem."
        ));
        assert!(is_machine_learning_paper(
            "we train a neural network and evaluate it on the test set."
        ));
    }

    /// Declining is a distinct answer, and the report says which one it gave.
    #[test]
    fn a_non_ml_manuscript_yields_not_applicable_rather_than_four_findings() {
        let r = extract::extract_from_text(
            "Methods\n\nHaemolymph was collected and total protein estimated \
             spectrophotometrically.\n",
        );
        let input = SpecialistInput { extraction: &r, science: None, analysis: None };
        let report = run(&MachineLearningSpecialist, &input);
        assert!(report.admitted.is_empty());
        assert!(report.rejected.is_empty());
        assert!(
            report.not_applicable.is_some(),
            "without this, four true-but-useless findings land on a silkworm paper"
        );
    }

    #[test]
    fn every_admitted_finding_carries_its_span_and_its_uncertainty() {
        let r = extract::extract_from_text(&format!(
            "{ML_BODY} We applied SMOTE and achieved 96.42% accuracy.\n"
        ));
        let input = SpecialistInput { extraction: &r, science: None, analysis: None };
        let report = run(&MachineLearningSpecialist, &input);
        assert!(report.admitted.len() >= 3, "{report:?}");
        for f in &report.admitted {
            assert!(f.span.is_some(), "{} carries no span", f.code);
            assert!(f.uncertainty.is_some(), "{} states no uncertainty", f.code);
        }
    }
}
