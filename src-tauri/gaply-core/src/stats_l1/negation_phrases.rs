//! The phrase table for the shared negation rule (Phase 2 evaluation fixes,
//! part A, section 1). Every row's expected answer was written before the
//! readers changed. Each row names the use it tests: the variant reader
//! (Level 1), the one-tailed reader or the multiple-testing reader (Level 0).
//! `cargo test -p gaply_core --lib negation_phrases -- --nocapture` prints
//! the table with the actual answers.

use super::catalogue::{ContinuityVariant, TVariant, Variant, VariantChoice, VariantKind};
use crate::stats_l0::input::{names_correction, states_one_tailed};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Expect {
    /// The variant reader: this variant stated, or not stated (`None`).
    Var(VariantKind, Option<Variant>),
    /// The one-tailed reader: stated or not.
    OneTailed(bool),
    /// The multiple-testing reader: the p values are adjusted (true) or raw.
    Adjusted(bool),
}

use self::Expect::*;

const T: VariantKind = VariantKind::T;
const C: VariantKind = VariantKind::Continuity;
const WELCH: Option<Variant> = Some(Variant::T(TVariant::Welch));
const STUDENT: Option<Variant> = Some(Variant::T(TVariant::Student));
const UNCORR: Option<Variant> = Some(Variant::Continuity(ContinuityVariant::No));
const YATES: Option<Variant> = Some(Variant::Continuity(ContinuityVariant::Yes));

/// (phrase, expected). Rows marked `report` are the evaluation report's own
/// failing inputs.
const TABLE: &[(&str, Expect)] = &[
    // Variant words, independent t.
    (
        "Equal variances were not assumed; the groups differed, t(15) = -1.35, p = .198.",
        Var(T, WELCH),
    ), // report V05b
    (
        "Student participants in group A differed from group B, t(15) = -1.35, p = .198.",
        Var(T, None),
    ), // report V06b
    (
        "With data pooled across sites, the groups differed, t(15) = -1.35, p = .198.",
        Var(T, None),
    ), // report V07b
    (
        "Without a pooled variance estimate, the groups differed, t(15) = -1.35, p = .198.",
        Var(T, None),
    ), // report V15
    (
        "Assuming unequal variances, the groups differed, t(15) = -1.35, p = .198.",
        Var(T, WELCH),
    ), // report V14
    (
        "A Student's t-test showed a difference, t(21) = 0.98, p = .34.",
        Var(T, STUDENT),
    ),
    (
        "A Student\u{2019}s t test showed a difference, t(21) = 0.98, p = .34.",
        Var(T, STUDENT),
    ),
    (
        "A Student t-test showed a difference, t(21) = 0.98, p = .34.",
        Var(T, STUDENT),
    ),
    (
        "A Students' t-test showed a difference, t(21) = 0.98, p = .34.",
        Var(T, STUDENT),
    ),
    (
        "The students differed by group, t(21) = 0.98, p = .34.",
        Var(T, None),
    ),
    (
        "The student sample differed by group, t(21) = 0.98, p = .34.",
        Var(T, None),
    ),
    (
        "Each student's score was higher in group A, t(21) = 0.98, p = .34.",
        Var(T, None),
    ),
    (
        "A pooled-variance t-test gave t(21) = 0.98, p = .34.",
        Var(T, STUDENT),
    ),
    (
        "A pooled t-test gave t(21) = 0.98, p = .34.",
        Var(T, STUDENT),
    ),
    (
        "A t-test with pooled variances gave t(21) = 0.98, p = .34.",
        Var(T, None),
    ),
    (
        "The pooled SD was 2.1, and the groups differed, t(21) = 0.98, p = .34.",
        Var(T, None),
    ),
    (
        "Cohen's d used the pooled standard deviation; t(21) = 0.98, p = .34.",
        Var(T, None),
    ),
    (
        "Equal variances not assumed, t(15) = -1.35, p = .198.",
        Var(T, WELCH),
    ),
    (
        "Not assuming equal variances, the groups differed, t(15) = -1.35, p = .198.",
        Var(T, WELCH),
    ),
    (
        "We did not use Welch's correction; t(21) = 0.98, p = .34.",
        Var(T, None),
    ),
    (
        "Welch's correction was not applied, t(21) = 0.98, p = .34.",
        Var(T, None),
    ),
    (
        "We used Welch's t-test rather than Student's t-test, t(15) = -1.35, p = .198.",
        Var(T, WELCH),
    ),
    (
        "Assuming equal variances, the groups differed, t(21) = 0.98, p = .34.",
        Var(T, STUDENT),
    ),
    (
        "We could not confirm that the groups had equal variances, t(21) = 0.98, p = .34.",
        Var(T, None),
    ),
    (
        "Welch's t-test, which does not assume equal variances, gave t(15) = -1.35, p = .198.",
        Var(T, WELCH),
    ),
    (
        "Student's t-test and Welch's t-test both gave t(21) = 0.98, p = .34.",
        Var(T, None),
    ),
    // Variant words, 2x2 chi-square.
    (
        "Without Yates' correction, response differed by sex, \u{3c7}2(1) = 4.08, p = .043.",
        Var(C, UNCORR),
    ), // report V10
    (
        "Without continuity correction, response differed by sex, \u{3c7}2(1) = 4.08, p = .043.",
        Var(C, UNCORR),
    ),
    (
        "An uncorrected chi-square test gave \u{3c7}2(1) = 4.08, p = .043.",
        Var(C, UNCORR),
    ),
    (
        "Uncorrected p values are reported; \u{3c7}2(1) = 4.08, p = .043.",
        Var(C, None),
    ), // report ledger F1
    (
        "With Yates' correction, response differed by sex, \u{3c7}2(1) = 3.06, p = .080.",
        Var(C, YATES),
    ),
    (
        "No continuity correction was applied, \u{3c7}2(1) = 4.08, p = .043.",
        Var(C, None),
    ),
    (
        "A continuity-corrected chi-square gave \u{3c7}2(1) = 3.06, p = .080.",
        Var(C, YATES),
    ),
    (
        "Yates' correction was not applied, \u{3c7}2(1) = 4.08, p = .043.",
        Var(C, None),
    ),
    // One-tailed words.
    ("We did not use one-tailed tests.", OneTailed(false)), // report B04
    ("No one-tailed tests were used.", OneTailed(false)),
    ("One-tailed tests were not used.", OneTailed(false)),
    ("One-tailed tests were used.", OneTailed(true)),
    ("Tests were one-tailed, not two-tailed.", OneTailed(true)),
    ("We tested a directional hypothesis.", OneTailed(false)), // report B01
    ("All tests were two-tailed.", OneTailed(false)),          // report B03
    // Report B09: "one-sided" describes an interval, not a test (review
    // follow-up, item 2).
    (
        "One-sided 95% confidence intervals were computed; tests were two-sided.",
        OneTailed(false),
    ),
    (
        "Although no outliers were found, one-tailed tests were used.",
        OneTailed(false),
    ),
    (
        "Rather than one-tailed tests, two-tailed tests were used.",
        OneTailed(false),
    ),
    // Multiple-testing words.
    ("P values were FDR-corrected.", Adjusted(true)), // report C03
    (
        "P values were corrected for multiple comparisons.",
        Adjusted(true),
    ), // report C06
    ("We used Hochberg's step-up procedure.", Adjusted(true)), // report C07
    (
        "Sphericity was violated, so Greenhouse-Geisser corrected degrees of freedom are reported.",
        Adjusted(false),
    ),
    (
        "P values were not corrected for multiple comparisons.",
        Adjusted(false),
    ),
    (
        "The procedure is described elsewhere (Benjamini & Hochberg, 1995).",
        Adjusted(false),
    ),
    (
        "A Benjamini\u{2013}Hochberg correction was applied.",
        Adjusted(true),
    ),
    (
        "We used the Benjamini and Hochberg procedure.",
        Adjusted(true),
    ),
    (
        "The Benjamini\u{2013}Yekutieli procedure controlled the FDR.",
        Adjusted(true),
    ),
    (
        "The Holm\u{2013}Bonferroni method was used.",
        Adjusted(true),
    ),
    (
        "Holmes et al. (2010) reported a similar effect.",
        Adjusted(false),
    ),
    ("Holm (1979) proposed a sequential method.", Adjusted(false)),
    ("Tukey's HSD post hoc tests followed.", Adjusted(true)),
    (
        "As Tukey (1977) argued, the data were plotted first.",
        Adjusted(false),
    ),
    ("Post hoc Tukey comparisons followed.", Adjusted(true)),
    (
        "Games\u{2013}Howell post hoc tests followed.",
        Adjusted(true),
    ),
    ("Dunn's test compared the groups.", Adjusted(true)),
    ("A \u{160}id\u{e1}k correction was applied.", Adjusted(true)),
    ("Sidak-adjusted p values are shown.", Adjusted(true)),
    ("The family-wise error rate was controlled.", Adjusted(true)),
    ("We report q values.", Adjusted(true)),
    ("Genes with padj < .05 were kept.", Adjusted(true)),
    (
        "No significant effects survived Bonferroni correction.",
        Adjusted(true),
    ),
    ("Uncorrected p values are reported.", Adjusted(false)),
    ("Unadjusted p values are reported.", Adjusted(false)),
    ("A Bonferroni correction was not applied.", Adjusted(false)),
    (
        "We report bias-corrected bootstrap intervals.",
        Adjusted(false),
    ),
    ("We used Welch's correction.", Adjusted(false)),
    ("Yates' continuity correction was applied.", Adjusted(false)),
    ("The Brown\u{2013}Forsythe test was used.", Adjusted(false)),
    ("The Huynh-Feldt correction was applied.", Adjusted(false)),
    (
        "Dunnett's test compared each dose with control.",
        Adjusted(true),
    ),
    (
        "Scheff\u{e9} post hoc comparisons followed.",
        Adjusted(true),
    ),
    ("Adjusted p-values are shown.", Adjusted(true)),
    ("The table gives p-adj for each contrast.", Adjusted(true)),
    (
        "We applied a multiple-comparison correction.",
        Adjusted(true),
    ),
    ("The false discovery rate was set at 5%.", Adjusted(true)),
    (
        "P values were adjusted for multiple testing.",
        Adjusted(true),
    ),
    ("Hommel's method was used.", Adjusted(true)),
    ("Bonferroni correction was used.", Adjusted(true)),
    // Review follow-up, item 1: a negator after the phrase that does not
    // negate a listed verb leaves the p values adjusted.
    (
        "Bonferroni correction did not change the results.",
        Adjusted(true),
    ),
    (
        "The FDR correction did not alter any conclusion.",
        Adjusted(true),
    ),
    (
        "Results were similar with and without Bonferroni correction.",
        Adjusted(true),
    ),
    ("No Bonferroni correction was applied.", Adjusted(false)),
    // Item 1, refined: a negator past a comma that negates a listed verb is
    // still clear; a bare one past a comma negates something else.
    (
        "One-tailed tests, however, were not used.",
        OneTailed(false),
    ),
    (
        "A Bonferroni correction, as planned, was not applied.",
        Adjusted(false),
    ),
    // Review follow-up, item 2: an interval described as one-sided.
    (
        "We report a one-sided 95% CI for the difference.",
        OneTailed(false),
    ),
    ("We computed one-sided confidence bounds.", OneTailed(false)),
];

fn actual(phrase: &str, e: Expect) -> Expect {
    match e {
        Var(kind, _) => Var(
            kind,
            Some(VariantChoice::from_wording(kind, phrase, None).variant())
                .filter(|v| v.is_stated()),
        ),
        OneTailed(_) => OneTailed(states_one_tailed(phrase)),
        Adjusted(_) => Adjusted(names_correction(phrase)),
    }
}

#[test]
fn every_phrase_gives_its_expected_answer() {
    assert!(TABLE.len() >= 40, "the step asks for at least 40 phrases");
    let mut wrong = Vec::new();
    println!("| # | phrase | expected | actual |\n|---|---|---|---|");
    for (i, (phrase, expected)) in TABLE.iter().enumerate() {
        let got = actual(phrase, *expected);
        println!(
            "| {} | {phrase} | {expected:?} | {got:?}{} |",
            i + 1,
            if got == *expected { "" } else { " **WRONG**" }
        );
        if got != *expected {
            wrong.push((phrase, expected, got));
        }
    }
    assert!(
        wrong.is_empty(),
        "{} of {} rows wrong:\n{wrong:#?}",
        wrong.len(),
        TABLE.len()
    );
}
