//! Phase 2 evaluation fixes, part A: Level 0 regressions from the joint
//! evaluation's own failing inputs (`~/gaply-corpus/eval2/joint_evaluation.md`,
//! Blocker B-3 and Major M-5), through the reader and the verdict rule.
//!
//! The one result in every document is `t(28) = 2.10`. R
//! (`/usr/local/bin/Rscript`, R 4.6.1): `2*pt(-2.10, 28)` = 0.044863, over
//! the printed statistic's interval [0.044391, 0.045340]; one-tailed
//! `pt(-2.10, 28)` = 0.022432.

use gaply_core::extract::extract_from_text;
use gaply_core::stats_l0::method::{CheckNote, ConcernReason, CouldNotCheckReason, L0Verdict};
use gaply_core::stats_l0::outcome::check_reading;
use gaply_core::stats_l0::result::{read_results, TableStructure};

/// Level 0's verdict on the single result in a document whose Methods holds
/// `methods` and whose Results holds `result`.
fn verdict(methods: &str, result: &str) -> L0Verdict {
    let text = format!("A Study\n\nMethods\n\nParticipants completed two sessions. {methods}\n\nResults\n\n{result}\n");
    let ex = extract_from_text(&text);
    let reading = read_results(&ex, TableStructure::NotAvailable);
    let out = check_reading(&ex, reading);
    assert_eq!(out.claims.len(), 1, "one result read in {text:?}");
    out.claims[0].verdict.clone()
}

/// B-3: an adjusted p (.27, larger than R's .0449) in a document naming the
/// correction in words outside the old list. Each was Inconsistent with
/// `conclusion_changes: true` (report C03, C06, C07).
#[test]
fn the_reports_correction_words_make_a_larger_p_an_adjusted_p() {
    for methods in [
        "P values were FDR-corrected.",
        "P values were corrected for multiple comparisons.",
        "We used Hochberg's step-up procedure.",
    ] {
        assert_eq!(
            verdict(methods, "Scores rose, t(28) = 2.10, p = .27."),
            L0Verdict::CouldNotCheck(CouldNotCheckReason::AdjustedP),
            "{methods:?}"
        );
    }
}

/// The controls. A sphericity correction alone, a correction clearly negated,
/// and a surname that is only a citation leave the p raw: R's two-tailed p
/// [0.044391, 0.045340] is below .05 and the printed .27 is not, so the result
/// is Inconsistent and the conclusion changes (Phase 1's fix, unchanged).
#[test]
fn a_raw_p_stays_inconsistent_when_no_correction_applies() {
    for methods in [
        "Sphericity was violated, so Greenhouse-Geisser corrected degrees of freedom are reported.",
        "P values were not corrected for multiple comparisons.",
        "The approach follows earlier work (Benjamini & Hochberg, 1995).",
    ] {
        assert_eq!(
            verdict(methods, "Scores rose, t(28) = 2.10, p = .27."),
            L0Verdict::Inconsistent {
                conclusion_changes: Some(true)
            },
            "{methods:?}"
        );
    }
}

/// M-5: a halved p (.023 printed; R's one-tailed .022432) is a statement of a
/// one-tailed test only when the paper says one was used.
#[test]
fn negated_one_tailed_wording_leaves_a_halved_p_as_a_concern() {
    for methods in [
        "We did not use one-tailed tests.",
        "No one-tailed tests were used.",
        "One-tailed tests were not used.",
    ] {
        assert_eq!(
            verdict(methods, "Scores rose, t(28) = 2.10, p = .023."),
            L0Verdict::Concern(ConcernReason::OneTailedUnstated),
            "{methods:?}"
        );
    }
    for methods in [
        "One-tailed tests were used.",
        "Tests were one-tailed, not two-tailed.",
    ] {
        assert_eq!(
            verdict(methods, "Scores rose, t(28) = 2.10, p = .023."),
            L0Verdict::Consistent(Some(CheckNote::OneTailedStated)),
            "{methods:?}"
        );
    }
}

/// Review follow-up, item 2 (report B09): "one-sided" or "one-tailed"
/// followed within three words by an interval word describes an interval,
/// not a test, so a halved p (.023 printed; R's two-tailed 0.044863) stays a
/// Concern.
#[test]
fn a_one_sided_interval_is_not_a_one_tailed_test() {
    for methods in [
        "One-sided 95% confidence intervals were computed; tests were two-sided.",
        "We report a one-sided 95% CI for each difference.",
        "We computed one-sided confidence bounds.",
    ] {
        assert_eq!(
            verdict(methods, "Scores rose, t(28) = 2.10, p = .023."),
            L0Verdict::Concern(ConcernReason::OneTailedUnstated),
            "{methods:?}"
        );
    }
}
