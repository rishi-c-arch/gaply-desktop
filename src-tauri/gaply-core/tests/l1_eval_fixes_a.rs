//! Phase 2 evaluation fixes, part A: Level 1 regressions from the joint
//! evaluation's own failing inputs (`~/gaply-corpus/eval2/joint_evaluation.md`,
//! Blocker B-1 and the Level 1 side of B-3).
//!
//! Every number is R's (`/usr/local/bin/Rscript`, R 4.6.1), quoted beside it.
//! The data are the evaluation's red-team cases V05b and V10, whole numbers
//! and counts, so the file's own precision plays no part.

use gaply_core::extract::{extract_from_text, SectionKind};
use gaply_core::stats_l0::input::DocumentContext;
use gaply_core::stats_l0::method::CouldNotCheckReason;
use gaply_core::stats_l0::number::Relation;
use gaply_core::stats_l0::result::{read_results, Family, TableStructure};
use gaply_core::stats_l1::binding::{BindingDraft, ProposalSource};
use gaply_core::stats_l1::catalogue::{L1MethodId, RoleName, VariantChoice, VariantKind};
use gaply_core::stats_l1::claim::{ClaimLocation, UserClaim, UserP};
use gaply_core::stats_l1::compute::ComputedVariant;
use gaply_core::stats_l1::dataset::{
    build_dataset, ColumnAlias, DataFingerprint, Dataset, RawTable, TableRef,
};
use gaply_core::stats_l1::reproduce::{reproduce, L1Note, L1Verdict, NotCheckedReason, NumberKind};
use gaply_core::stats_l1::run_state::L1RunState;

/// Red-team case V05b's data: group A eight scores, group B fifteen.
/// R: `t.test(score ~ grp)` -> t = -1.348226, df = 14.602457, p = 0.198128
/// (Welch); `var.equal = TRUE` -> t = -0.981543, df = 21, p = 0.337499.
const A: [&str; 8] = ["10", "11", "12", "10", "11", "12", "11", "10"];
const B: [&str; 15] = [
    "4", "18", "9", "22", "6", "15", "25", "3", "12", "20", "8", "16", "27", "5", "14",
];

fn t_data() -> Dataset {
    let rows = A
        .iter()
        .map(|v| vec![v.to_string(), "A".into()])
        .chain(B.iter().map(|v| vec![v.to_string(), "B".into()]))
        .collect();
    build_dataset(
        RawTable::from_rows(vec!["score".into(), "grp".into()], rows),
        DataFingerprint::of_bytes(b"v05b", "in-memory", TableRef::FirstTable),
    )
    .expect("dataset")
}

/// Red-team case V10's 2x2 table: yes/no by m/f = 19, 11 / 9, 16.
/// R: `chisq.test(tab, correct = FALSE)` -> 4.076499, p = 0.043484;
/// `correct = TRUE` -> 3.056162, p = 0.080431.
fn chi_data() -> Dataset {
    let mut rows = Vec::new();
    for (resp, sex, n) in [
        ("yes", "m", 19),
        ("yes", "f", 11),
        ("no", "m", 9),
        ("no", "f", 16),
    ] {
        for _ in 0..n {
            rows.push(vec![resp.to_string(), sex.to_string()]);
        }
    }
    build_dataset(
        RawTable::from_rows(vec!["resp".into(), "sex".into()], rows),
        DataFingerprint::of_bytes(b"v10", "in-memory", TableRef::FirstTable),
    )
    .expect("dataset")
}

/// One claim with `sentence`, bound by hand the way the screen binds it: the
/// variant read from the claim's sentence by the closed list.
fn run(
    data: fn() -> Dataset,
    family: Family,
    stat: &str,
    df: &str,
    p: &str,
    sentence: &str,
    ctx: Option<DocumentContext>,
) -> (L1Verdict, Vec<L1Note>, VariantChoice) {
    let mut s = L1RunState::new();
    s.load_dataset(data());
    if let Some(c) = ctx {
        s.set_context(c);
    }
    let claim = s
        .add_claim(&UserClaim {
            family,
            statistic: stat,
            df: vec![df],
            n: None,
            p: UserP::Reported {
                raw: p,
                relation: Relation::Eq,
            },
            location: ClaimLocation {
                section_index: 0,
                section: SectionKind::Results,
                paragraph: 0,
                sentence: 0,
            },
            sentence: Some(sentence),
        })
        .unwrap();
    let (method, kind) = match family {
        Family::T => (L1MethodId::IndependentT, VariantKind::T),
        _ => (L1MethodId::ChiSquareIndependence, VariantKind::Continuity),
    };
    let variant = VariantChoice::from_wording(kind, &s.claim(claim).unwrap().sentence, None);
    let draft = match family {
        Family::T => BindingDraft::new(method, variant)
            .role(RoleName::Outcome, &[ColumnAlias(1)])
            .role(RoleName::Group, &[ColumnAlias(2)])
            .levels(RoleName::Group, &["A", "B"]),
        _ => BindingDraft::new(method, variant)
            .role(RoleName::RowVariable, &[ColumnAlias(1)])
            .role(RoleName::ColumnVariable, &[ColumnAlias(2)])
            .levels(RoleName::RowVariable, &["yes", "no"])
            .levels(RoleName::ColumnVariable, &["m", "f"]),
    };
    let pid = s
        .add_proposal(claim, draft, ProposalSource::Manual { edited_from: None })
        .unwrap();
    let token = s.confirm(pid).unwrap().1;
    let out = reproduce(
        &s,
        &[(s.claim(claim).unwrap().revision.clone(), token)],
        move || Ok(data()),
    );
    let c = &out.claims[0];
    (c.verdict.clone(), c.notes.clone(), variant)
}

fn reproduced_under(v: &L1Verdict, variant: ComputedVariant) -> bool {
    matches!(v, L1Verdict::Reproduced(notes) if notes.contains(&L1Note::VariantMatched(variant)))
}

/// B-1: each of the report's sentences, on correct Welch numbers (t(15) =
/// -1.35 is R's -1.348226 with its df 14.602457 rounded; p .198 is R's
/// 0.198128), must not read as Student and must reproduce.
#[test]
fn the_reports_variant_traps_reproduce_the_correct_welch_result() {
    for sentence in [
        "Equal variances were not assumed; the groups differed, t(15) = -1.35, p = .198.",
        "Student participants in group A differed from group B, t(15) = -1.35, p = .198.",
        "With data pooled across sites, the groups differed, t(15) = -1.35, p = .198.",
        "Without a pooled variance estimate, the groups differed, t(15) = -1.35, p = .198.",
    ] {
        let (v, _, choice) = run(t_data, Family::T, "-1.35", "15", ".198", sentence, None);
        assert!(
            reproduced_under(&v, ComputedVariant::Welch),
            "{sentence:?}: {v:?} (variant {choice:?})"
        );
    }
}

/// B-1, chi-square: "Without Yates' correction" names the uncorrected test.
/// R: chisq.test(tab, correct = FALSE) -> 4.076499, p = 0.043484.
#[test]
fn without_yates_correction_reproduces_the_uncorrected_chi_square() {
    let (v, _, choice) = run(
        chi_data,
        Family::ChiSquare,
        "4.08",
        "1",
        ".043",
        "Without Yates' correction, response differed by sex, \u{3c7}2(1) = 4.08, p = .043.",
        None,
    );
    assert!(
        reproduced_under(&v, ComputedVariant::ChiSquareUncorrected),
        "{v:?} (variant {choice:?})"
    );
}

/// B-3 at Level 1: an FDR-adjusted p (Student's t(21) = 0.98, R's raw p
/// 0.337499, printed p = .67) is not compared, so the claim reproduces on its
/// statistic and df with the p not checked as adjusted.
#[test]
fn an_fdr_adjusted_p_is_not_compared_at_level_1() {
    let text = "A Study\n\nMethods\n\nP values were FDR-corrected.\n\nResults\n\nScores differed, t(21) = 0.98, p = .67.\n";
    let ex = extract_from_text(text);
    let reading = read_results(&ex, TableStructure::NotAvailable);
    let ctx = DocumentContext::read(&ex, &reading);
    assert!(
        ctx.correction_at.is_some(),
        "the document names a correction"
    );
    let (v, _, _) = run(
        t_data,
        Family::T,
        "0.98",
        "21",
        ".67",
        "Assuming equal variances, scores differed, t(21) = 0.98, p = .67.",
        Some(ctx),
    );
    let L1Verdict::Reproduced(notes) = &v else {
        panic!("expected Reproduced, got {v:?}")
    };
    assert!(
        notes.contains(&L1Note::VariantMatched(ComputedVariant::StudentPooled)),
        "{notes:?}"
    );
    assert!(
        notes.contains(&L1Note::NotChecked {
            number: NumberKind::P,
            why: NotCheckedReason::PCouldNotCheckAtLevel0(CouldNotCheckReason::AdjustedP)
        }),
        "{notes:?}"
    );
}
