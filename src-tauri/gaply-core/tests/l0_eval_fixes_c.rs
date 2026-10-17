//! **Phase 2 evaluation fixes, part C: the Level 0 changes the deep
//! evaluation's findings required** (`~/gaply-corpus/eval3/deep_evaluation.md`).
//!
//! - B-2, Level 0's half: the report's 144 honest double-rounded p cases,
//!   printed as Level 0 reads them, gave 20 false Inconsistent at 5163aec
//!   (`~/gaply-corpus/eval3/partc/b2/`), so the double-rounding rule applies
//!   to Level 0's p comparison too, with the same note.
//! - m-3: two alpha phrasings the reader missed, and "P values below .01
//!   were considered significant".
//! - m-6: a clause of 70,000 statistics with no p took 58.5 s (quadratic).
//!
//! p ranges quoted from `/usr/local/bin/Rscript` (R 4.6.1).

use std::time::{Duration, Instant};

use gaply_core::extract::extract_from_text;
use gaply_core::stats_l0::method::{CheckNote, L0Verdict};
use gaply_core::stats_l0::outcome::check_reading;
use gaply_core::stats_l0::result::{read_results, Reason, TableStructure};

fn doc(methods: &str, result: &str) -> String {
    format!("A Study\n\nMethods\n\n{methods}\n\nResults\n\nGroups differed, {result}.\n")
}

fn verdict(text: &str) -> L0Verdict {
    let ex = extract_from_text(text);
    let reading = read_results(&ex, TableStructure::NotAvailable);
    let out = check_reading(&ex, reading);
    assert_eq!(out.claims.len(), 1, "one result read");
    out.claims[0].verdict.clone()
}

fn alpha(methods: &str) -> (Vec<String>, bool) {
    let ex = extract_from_text(&doc(methods, "t(40) = 2.10, p = .042"));
    let reading = read_results(&ex, TableStructure::NotAvailable);
    let out = check_reading(&ex, reading);
    (out.alpha_printed.clone(), out.alpha_defaulted)
}

/// Two of the 20 (eval3 cases c000105 and c000128). R: `2*pt(-1.825, 59)` =
/// .073065 and `2*pt(-1.815, 59)` = .074608, so the printed statistic allows
/// p up to .074608; `.08` stands for [.075, .085) by rounding, and a number
/// rounded first to 3 decimals reaches down to .0745. `t(13) = 2.81`:
/// [.014604, .014887] against `.02` rounded from .0145 or more.
#[test]
fn a_double_rounded_p_is_consistent_with_its_note_at_level_0() {
    for unit in ["t(59) = -1.82, p = .08", "t(13) = -2.81, p = .02"] {
        assert_eq!(
            verdict(&doc("Participants completed the task.", unit)),
            L0Verdict::Consistent(Some(CheckNote::DoubleRounded)),
            "{unit}"
        );
    }
    // Plain rounding still wins, with no note: R gives [.124363, .126621]
    // for t(23) = 1.59, which meets .13's own interval.
    assert_eq!(
        verdict(&doc(
            "Participants completed the task.",
            "t(23) = 1.59, p = .13"
        )),
        L0Verdict::Consistent(None)
    );
    // A last-digit typo past both readings stays Inconsistent.
    assert!(matches!(
        verdict(&doc(
            "Participants completed the task.",
            "t(59) = -1.82, p = .09"
        )),
        L0Verdict::Inconsistent { .. }
    ));
}

/// m-3. The report's P10 read only .05; P11 was not read; and the phrase
/// with "below" was not read either. Several thresholds leave the
/// conclusion unassessed, which is the safe reading of P10.
#[test]
fn the_reports_missed_alpha_phrasings_are_read() {
    let (p10, defaulted) =
        alpha("Significance was set at p < .05 for main effects and p < .01 for interactions.");
    assert_eq!(
        (p10, defaulted),
        (vec![".05".to_string(), ".01".to_string()], false)
    );
    let (p11, defaulted) = alpha("The Bonferroni-corrected threshold was .05/6.");
    assert_eq!((p11, defaulted), (vec![".05/6".to_string()], false));
    for phrase in [
        "P values below .01 were considered significant.",
        "p-values below 0.01 were considered statistically significant.",
    ] {
        let (a, defaulted) = alpha(phrase);
        assert!(!defaulted, "{phrase}");
        assert_eq!(a.len(), 1, "{phrase}: {a:?}");
        assert!(a[0].ends_with(".01"), "{phrase}: {a:?}");
    }
    // A reliability coefficient is still never a threshold.
    assert!(alpha("Cronbach's alpha was .82.").1);
}

/// m-6: statistics with no p, all in one clause, as a PDF table extracted as
/// one paragraph gives them. 10,000 took 1.56 s in release at 5163aec and
/// grow with the square; the reader is now linear. The bound is generous
/// for a debug build.
#[test]
fn a_long_clause_of_statistics_without_p_is_read_in_linear_time() {
    let units = 10_000;
    let text = format!(
        "A Study\n\nResults\n\nThe values were {}and that is all.\n",
        "t(28) = 2.10, ".repeat(units)
    );
    let started = Instant::now();
    let ex = extract_from_text(&text);
    let reading = read_results(&ex, TableStructure::NotAvailable);
    let took = started.elapsed();
    assert!(reading.results.is_empty());
    assert_eq!(
        reading
            .not_read
            .iter()
            .filter(|n| n.reason == Reason::StatisticWithoutP)
            .count(),
        units
    );
    assert!(took < Duration::from_secs(3), "{units} units took {took:?}");
}
