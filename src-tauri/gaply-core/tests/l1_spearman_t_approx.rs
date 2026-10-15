//! Spearman's p at Level 1 (Phase 2 evaluation fixes, part A, section 4).
//!
//! Level 1 computes Spearman's p by the t approximation: R's
//! `cor.test(x, y, method = "spearman", exact = FALSE)`, which is what SPSS
//! reports. The reference is `tests/fixtures/stats_l1/spearman_tapprox.csv`,
//! written by `/usr/local/bin/Rscript make_l1_spearman_fixtures.R`, with and
//! without ties.
//!
//! - A printed p that matches the t approximation is matched, and the claim
//!   carries a note naming the method.
//! - A printed p that does not match is NOT CHECKED (`spearman_p_may_be_exact`):
//!   the paper may have used the exact p, which Level 1 does not compute yet.
//!   A p alone never makes a Spearman claim NotReproduced, with or without
//!   ties (before this step, a tied sample's mismatched p did).

use gaply_core::extract::SectionKind;
use gaply_core::stats_l0::number::Relation;
use gaply_core::stats_l0::result::Family;
use gaply_core::stats_l1::binding::{BindingDraft, ProposalSource};
use gaply_core::stats_l1::catalogue::{
    L1MethodId, RoleName, SpearmanPVariant, Variant, VariantChoice, VariantKind,
};
use gaply_core::stats_l1::claim::{ClaimLocation, UserClaim, UserP};
use gaply_core::stats_l1::dataset::{
    build_dataset, ColumnAlias, DataFingerprint, Dataset, RawTable, TableRef,
};
use gaply_core::stats_l1::reproduce::{
    reproduce, ClaimReproduction, L1CouldNotCheck, L1Note, L1Verdict, NotCheckedReason, NumberKind,
    NumberStatus, VariantGap,
};
use gaply_core::stats_l1::run_state::L1RunState;

struct Case {
    name: String,
    x: Vec<String>,
    y: Vec<String>,
    ties: bool,
    rho: f64,
    df: usize,
    p: f64,
}

fn cases() -> Vec<Case> {
    let text = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/stats_l1/spearman_tapprox.csv"
    ))
    .expect("fixture");
    assert!(
        text.lines()
            .any(|l| l.starts_with('#') && l.contains("/usr/local/bin/Rscript")),
        "the fixture names its generator"
    );
    let out: Vec<Case> = text
        .lines()
        .filter(|l| !l.starts_with('#') && !l.starts_with("case,"))
        .map(|l| {
            let f: Vec<&str> = l.split(',').collect();
            Case {
                name: f[0].into(),
                x: f[1].split(';').map(String::from).collect(),
                y: f[2].split(';').map(String::from).collect(),
                ties: f[3] == "TRUE",
                rho: f[4].parse().unwrap(),
                df: f[5].parse().unwrap(),
                p: f[6].parse().unwrap(),
            }
        })
        .collect();
    assert_eq!(out.len(), 4, "four R cases");
    assert!(
        out.iter().any(|c| c.ties) && out.iter().any(|c| !c.ties),
        "with and without ties"
    );
    out
}

fn data(c: &Case) -> Dataset {
    let rows =
        c.x.iter()
            .zip(&c.y)
            .map(|(a, b)| vec![a.clone(), b.clone()])
            .collect();
    build_dataset(
        RawTable::from_rows(vec!["x".into(), "y".into()], rows),
        DataFingerprint::of_bytes(c.name.as_bytes(), "in-memory", TableRef::FirstTable),
    )
    .expect("dataset")
}

/// rho printed to 2 decimals; p as printed by `p`; no variant stated.
fn run(c: &Case, p_raw: &str, relation: Relation) -> ClaimReproduction {
    run_with(
        c,
        p_raw,
        relation,
        VariantChoice::not_stated(VariantKind::SpearmanP),
    )
}

/// The same, under a given variant choice.
fn run_with(
    c: &Case,
    p_raw: &str,
    relation: Relation,
    variant: VariantChoice,
) -> ClaimReproduction {
    let rho = format!("{:.2}", c.rho);
    let df = c.df.to_string();
    let mut s = L1RunState::new();
    s.load_dataset(data(c));
    let claim = s
        .add_claim(&UserClaim {
            family: Family::R,
            statistic: &rho,
            df: vec![&df],
            n: None,
            p: UserP::Reported {
                raw: p_raw,
                relation,
            },
            location: ClaimLocation {
                section_index: 0,
                section: SectionKind::Results,
                paragraph: 0,
                sentence: 0,
            },
            sentence: None,
        })
        .unwrap();
    let draft = BindingDraft::new(L1MethodId::Spearman, variant)
        .role(RoleName::X, &[ColumnAlias(1)])
        .role(RoleName::Y, &[ColumnAlias(2)]);
    let pid = s
        .add_proposal(claim, draft, ProposalSource::Manual { edited_from: None })
        .unwrap();
    let token = s.confirm(pid).unwrap().1;
    let d = data(c);
    reproduce(
        &s,
        &[(s.claim(claim).unwrap().revision.clone(), token)],
        move || Ok(d),
    )
    .claims
    .remove(0)
}

fn p_status(r: &ClaimReproduction) -> NumberStatus {
    r.shown()
        .and_then(|c| c.numbers.iter().find(|n| n.kind == NumberKind::P))
        .map(|n| n.status)
        .expect("a p row")
}

/// R's t-approximation p, printed the APA way: three decimals, or `< .001`.
fn printed(p: f64) -> (String, Relation) {
    if p < 0.001 {
        (".001".into(), Relation::Lt)
    } else {
        (
            format!("{p:.3}").trim_start_matches('0').to_string(),
            Relation::Eq,
        )
    }
}

#[test]
fn a_p_that_matches_the_t_approximation_is_matched_and_named() {
    for c in cases() {
        let (p, rel) = printed(c.p);
        let r = run(&c, &p, rel);
        let L1Verdict::Reproduced(notes) = &r.verdict else {
            panic!("{}: {:?}", c.name, r.verdict)
        };
        assert!(
            matches!(p_status(&r), NumberStatus::Matched(_)),
            "{}: p {:?}",
            c.name,
            p_status(&r)
        );
        assert!(
            notes.contains(&L1Note::SpearmanPByTApproximation),
            "{}: {notes:?}",
            c.name
        );
    }
}

/// A p that matches neither: not checked, never NotReproduced. The report's
/// own case is `no_ties_report_a01`: r(10) = -.97 printed with p = .200,
/// where R's t-approximation p is 3.88e-7 and its exact p 0.
#[test]
fn a_p_that_does_not_match_is_not_checked_with_or_without_ties() {
    for c in cases() {
        let r = run(&c, ".200", Relation::Eq);
        assert!(
            c.p < 0.15 || c.p > 0.25,
            "{}: .200 must really miss R's {}",
            c.name,
            c.p
        );
        assert_eq!(
            p_status(&r),
            NumberStatus::NotChecked(NotCheckedReason::SpearmanPMayBeExact),
            "{}",
            c.name
        );
        let L1Verdict::Reproduced(notes) = &r.verdict else {
            panic!("{} (ties {}): {:?}", c.name, c.ties, r.verdict)
        };
        assert!(
            notes.contains(&L1Note::NotChecked {
                number: NumberKind::P,
                why: NotCheckedReason::SpearmanPMayBeExact
            }),
            "{}: {notes:?}",
            c.name
        );
        assert!(
            !notes.contains(&L1Note::SpearmanPByTApproximation),
            "{}: an unmatched p names no method",
            c.name
        );
    }
}

/// Review follow-up, item 3: when the paper's words or the user's tick state
/// the t approximation, the exact p is not a candidate, so the p is compared
/// normally, with or without ties, and a mismatch makes the claim
/// NotReproduced. A stated exact p stays not compared.
#[test]
fn a_stated_t_approximation_compares_the_p_normally() {
    let t_approx =
        || VariantChoice::user_ticked(Variant::SpearmanP(SpearmanPVariant::TApproximation));
    for c in cases() {
        let (p, rel) = printed(c.p);
        let r = run_with(&c, &p, rel, t_approx());
        assert!(
            matches!(r.verdict, L1Verdict::Reproduced(_)),
            "{}: {:?}",
            c.name,
            r.verdict
        );
        assert!(
            matches!(p_status(&r), NumberStatus::Matched(_)),
            "{}",
            c.name
        );
        // .200 misses R's t-approximation p in every case.
        let r = run_with(&c, ".200", Relation::Eq, t_approx());
        assert_eq!(
            p_status(&r),
            NumberStatus::NotMatched,
            "{} (ties {})",
            c.name,
            c.ties
        );
        assert!(
            matches!(r.verdict, L1Verdict::NotReproduced { .. }),
            "{} (ties {}): {:?}",
            c.name,
            c.ties,
            r.verdict
        );
    }
    let exact = VariantChoice::user_ticked(Variant::SpearmanP(SpearmanPVariant::Exact));
    let c = &cases()[0];
    let r = run_with(c, ".200", Relation::Eq, exact);
    assert_eq!(
        r.verdict,
        L1Verdict::CouldNotCheck(L1CouldNotCheck::VariantNotAvailable(
            VariantGap::SpearmanExactP
        ))
    );
}
