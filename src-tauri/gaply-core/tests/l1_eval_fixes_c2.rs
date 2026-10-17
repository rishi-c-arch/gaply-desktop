//! **Phase 2 evaluation fixes, part C follow-up: a printed p that the printed
//! statistic rules out** (part C's open question 1, the joint evaluation's
//! red-team case A01).
//!
//! When the data's precision alone leaves a claim's p undecided, the printed
//! p is compared with what the PRINTED statistic and df allow, by Level 0's
//! own `PConsistency`. It is called on every print Level 1 itself accepts for
//! those numbers (the print, and one unit in the last place either side, which
//! hold a truncated or double-rounded statistic and Welch's rounded or
//! truncated df), and with the p one unit up (a truncated p). Only when every
//! one of them is Inconsistent is the p not matched. Spearman with no variant
//! stated widens each range to part C's band; with ties nothing changes.
//!
//! The honest cases below are each a print Level 1 accepts that Level 0's
//! range for the printed statistic alone does not hold, found with
//! `/usr/local/bin/Rscript ~/gaply-corpus/eval3/partc2/neighbours.R` and
//! `neighbours_b.R`. Every number is from `/usr/local/bin/Rscript
//! ~/gaply-corpus/eval3/partc2/item1_values.R` (R 4.6.1).

use gaply_core::extract::SectionKind;
use gaply_core::stats_l0::input::DocumentContext;
use gaply_core::stats_l0::method::{ConcernReason, TextPosition};
use gaply_core::stats_l0::number::Relation;
use gaply_core::stats_l0::p_consistency::alpha_of;
use gaply_core::stats_l0::result::Family;
use gaply_core::stats_l1::binding::{BindingDraft, ProposalSource};
use gaply_core::stats_l1::catalogue::{
    L1MethodId, RoleName, SpearmanPVariant, TVariant, Variant, VariantChoice, VariantKind,
};
use gaply_core::stats_l1::claim::{ClaimId, ClaimLocation, ClaimOrigin, L1Claim, UserClaim, UserP};
use gaply_core::stats_l1::codes;
use gaply_core::stats_l1::compute::{
    ComputedResult, ComputedVariant, InputCounts, L1Computation, Precision, StatisticKind, Tail,
};
use gaply_core::stats_l1::dataset::{
    build_dataset, ColumnAlias, DataFingerprint, Dataset, RawTable, TableRef,
};
use gaply_core::stats_l1::reproduce::{
    compare_claim, decide, reproduce, BindingFacts, CandidateComparison, L1CouldNotCheck, L1Note,
    L1Verdict, NotCheckedReason, NumberKind, NumberStatus,
};
use gaply_core::stats_l1::run_state::L1RunState;
use gaply_core::stats_l1::ENGINE_VERSION;
use serde_json::{json, Value};

/* ---- helpers -------------------------------------------------------------- */

fn loc() -> ClaimLocation {
    ClaimLocation {
        section_index: 0,
        section: SectionKind::Results,
        paragraph: 0,
        sentence: 0,
    }
}

fn claim(family: Family, statistic: &str, df: &[&str], p: &str) -> L1Claim {
    let u = UserClaim {
        family,
        statistic,
        df: df.to_vec(),
        n: None,
        p: UserP::Reported {
            raw: p,
            relation: Relation::Eq,
        },
        location: loc(),
        sentence: None,
    };
    L1Claim::user_entered(ClaimId(0), &u, ClaimOrigin::UserAdded).expect("numbers read")
}

fn ctx() -> DocumentContext {
    DocumentContext {
        thresholds: Vec::new(),
        one_tailed_at: None,
        correction_at: None,
    }
}

#[allow(clippy::too_many_arguments)]
fn result(
    variant: ComputedVariant,
    statistic: StatisticKind,
    value: f64,
    df: f64,
    p: f64,
    n: usize,
    precision: Precision,
) -> ComputedResult {
    ComputedResult {
        variant,
        statistic,
        value,
        df1: df,
        df2: None,
        p,
        tail: Tail::TwoSided,
        n,
        precision,
    }
}

fn computation(method: L1MethodId, r: ComputedResult, ties: Option<bool>) -> L1Computation {
    L1Computation {
        claim: ClaimId(0),
        method,
        counts: InputCounts {
            rows_read: r.n,
            excluded: Vec::new(),
            n_used: r.n,
        },
        results: vec![r],
        ties,
        engine_version: ENGINE_VERSION,
    }
}

/// The verdict, its notes and the candidates, under `ctx`.
fn run(
    c: &L1Claim,
    ctx: &DocumentContext,
    facts: BindingFacts,
    comp: &L1Computation,
) -> (L1Verdict, Vec<L1Note>, Vec<CandidateComparison>) {
    match compare_claim(&c.result, ctx, &facts, comp) {
        Err(why) => (L1Verdict::CouldNotCheck(why), Vec::new(), Vec::new()),
        Ok((cands, _)) => {
            let alpha = alpha_of(ctx, &mut Vec::new());
            let (v, notes) = decide(&cands, &alpha);
            (v, notes, cands)
        }
    }
}

fn p_status(cands: &[CandidateComparison]) -> NumberStatus {
    cands[0]
        .numbers
        .iter()
        .find(|n| n.kind == NumberKind::P)
        .expect("a p row")
        .status
}

fn wire(s: NumberStatus) -> Value {
    serde_json::to_value(codes::number_status(s)).unwrap()
}

/// The p row's status as the screen and the explanation receive it.
fn ruled_out() -> Value {
    json!({"code": "not_matched", "why": "p_ruled_out_by_printed_statistic"})
}

fn student() -> BindingFacts {
    BindingFacts {
        method: L1MethodId::IndependentT,
        variant: Variant::T(TVariant::Student),
        target: None,
    }
}

/// A Student t at df 20 whose stored cells were rounded to whole numbers:
/// the data's statistic could lie anywhere in `stat`, its p in `p`.
fn t20(value: f64, p: f64, stat: (f64, f64), p_range: (f64, f64)) -> L1Computation {
    let precision = Precision::Range {
        decimals: 0,
        statistic: stat,
        df: None,
        p: p_range,
    };
    computation(
        L1MethodId::IndependentT,
        result(
            ComputedVariant::StudentPooled,
            StatisticKind::T,
            value,
            20.0,
            p,
            22,
            precision,
        ),
        None,
    )
}

fn has_precision_note(notes: &[L1Note]) -> bool {
    notes.contains(&L1Note::NotChecked {
        number: NumberKind::P,
        why: NotCheckedReason::DataPrecision { decimals: 0 },
    })
}

/* ---- the rule ------------------------------------------------------------- */

/// `t(20) = 2.10, p = .500` on whole-number data. The data's t, 2.1032,
/// matches; its p, .0483073, does not, but the stored cells' rounding could
/// put the p anywhere in [.0323474, .6225318], so the p was "not decided".
/// No t from 2.09 to 2.11 at df 20 gives a p near .500 (Level 0: .048134 to
/// .049106 for 2.10): the printed p cannot follow from the printed t.
#[test]
fn a_p_the_printed_t_rules_out_is_not_matched_and_the_claim_not_reproduced() {
    let comp = t20(
        2.1032,
        0.048_307_301_6,
        (0.5, 2.3),
        (0.032_347_423_9, 0.622_531_842_3),
    );
    let (v, notes, cands) = run(
        &claim(Family::T, "2.10", &["20"], ".500"),
        &ctx(),
        student(),
        &comp,
    );
    assert_eq!(
        v,
        L1Verdict::NotReproduced {
            conclusion_changes: None
        },
        "{notes:?}"
    );
    assert_eq!(wire(p_status(&cands)), ruled_out());
    // The data's own p was not decided, so its side of alpha is not compared:
    // the conclusion gap says the p was judged on the printed numbers.
    let gaps: Vec<Value> = notes
        .iter()
        .map(|n| serde_json::to_value(codes::l1_note(*n)).unwrap())
        .collect();
    assert_eq!(
        gaps,
        vec![json!({"code": "conclusion_not_assessed", "gap": "p_judged_on_printed_numbers"})]
    );
}

/// The same data with `p = .0490`, which the printed t allows (.048134 to
/// .049106): the p stays "not decided", and the claim Reproduced.
#[test]
fn a_p_the_printed_t_allows_stays_not_decided() {
    let comp = t20(
        2.1032,
        0.048_307_301_6,
        (0.5, 2.3),
        (0.032_347_423_9, 0.622_531_842_3),
    );
    let (v, _, cands) = run(
        &claim(Family::T, "2.10", &["20"], ".0490"),
        &ctx(),
        student(),
        &comp,
    );
    let L1Verdict::Reproduced(notes) = &v else {
        panic!("{v:?}")
    };
    assert!(has_precision_note(notes), "{notes:?}");
    assert_eq!(
        p_status(&cands),
        NumberStatus::NotChecked(NotCheckedReason::DataPrecision { decimals: 0 })
    );
}

/// Honest prints Level 1 accepts whose p lies OUTSIDE Level 0's range for
/// the printed statistic alone. Each must stay "not decided": calling Level 0
/// on the literal print only would make every one a false "Not reproduced".
#[test]
fn honest_prints_level_1_accepts_are_never_ruled_out() {
    // Truncated t: truth 1.5095 printed 1.50, p .146808 printed .147. Level 0
    // for 1.50 gives .147954 to .150527; for 1.51, .145418 to .147954. The
    // data (rounded cells) give t 1.5040, p .1482097.
    let truncated_t = t20(1.5040, 0.148_209_662_3, (1.40, 1.60), (0.125, 0.177));
    // A double-rounded t at df 5: truth .6346 -> .635 -> printed .64, p
    // .553587 printed .554. Level 0 for .64: .547351 to .553346 (rule a2
    // reaches down only to .55345); for .63: .553346 to .559383. Data: t
    // .6380, p .551543.
    let double_t = computation(
        L1MethodId::IndependentT,
        result(
            ComputedVariant::StudentPooled,
            StatisticKind::T,
            0.6380,
            5.0,
            0.551_543,
            7,
            Precision::Range {
                decimals: 0,
                statistic: (0.3, 0.9),
                df: None,
                p: (0.40, 0.79),
            },
        ),
        None,
    );
    // A truncated p: t(20) = 1.800, p .086965 printed .086 (truncated). Level
    // 0 for 1.800: .086883 to .087046; .086 misses, .087 does not. Data: t
    // 1.7996, p .0870299 (outside the truncation of .086 too).
    let truncated_p = t20(1.7996, 0.087_029_872_2, (1.6, 2.0), (0.06, 0.13));
    for (name, printed, comp) in [
        (
            "truncated t",
            claim(Family::T, "1.50", &["20"], ".147"),
            truncated_t,
        ),
        (
            "double-rounded t",
            claim(Family::T, ".64", &["5"], ".554"),
            double_t,
        ),
        (
            "truncated p",
            claim(Family::T, "1.800", &["20"], ".086"),
            truncated_p,
        ),
    ] {
        let (v, _, cands) = run(&printed, &ctx(), student(), &comp);
        let L1Verdict::Reproduced(notes) = &v else {
            panic!("{name}: {v:?}")
        };
        assert!(has_precision_note(notes), "{name}: {notes:?}");
        assert_ne!(wire(p_status(&cands)), ruled_out(), "{name}");
    }

    // Welch's df printed as a whole number, truncated: truth df 5.95
    // printed 5, t 2.00, p .092827 printed .093. Level 0 at df 5: .101291 to
    // .102593; Welch's df read as 6 (5.5 to 6.5): .088128 to .097402. Data:
    // t 2.004, df 5.60, p .0953133.
    let welch = computation(
        L1MethodId::IndependentT,
        result(
            ComputedVariant::Welch,
            StatisticKind::T,
            2.004,
            5.60,
            0.095_313_289_6,
            8,
            Precision::Range {
                decimals: 0,
                statistic: (1.8, 2.2),
                df: Some((5.0, 6.6)),
                p: (0.066_041, 0.131_758),
            },
        ),
        None,
    );
    let facts = BindingFacts {
        method: L1MethodId::IndependentT,
        variant: Variant::T(TVariant::Welch),
        target: None,
    };
    let (v, _, cands) = run(
        &claim(Family::T, "2.00", &["5"], ".093"),
        &ctx(),
        facts,
        &welch,
    );
    let L1Verdict::Reproduced(notes) = &v else {
        panic!("welch: {v:?}")
    };
    assert!(has_precision_note(notes), "welch: {notes:?}");
    assert_ne!(wire(p_status(&cands)), ruled_out());
}

/// What Level 0 does not call Inconsistent, the rule does not decide either.
/// `t(20) = 2.10, p = .024` fits only as one-tailed: half of 2.09 to 2.11's p
/// is .023589 to .025048. The data: t 2.20, p .0397286, half .019864 (no
/// one-tailed match on the exact p). Unstated, Level 0 raises a Concern, so
/// the p is left with Level 0's reason before the data's precision is
/// reached; stated, Level 0 is Consistent and the p stays "not decided".
#[test]
fn a_print_level_0_does_not_call_inconsistent_changes_nothing() {
    let comp = t20(2.20, 0.039_728_589_6, (0.5, 2.6), (0.017_127, 0.622_532));
    let printed = claim(Family::T, "2.10", &["20"], ".024");
    let stated = DocumentContext {
        one_tailed_at: Some(TextPosition {
            section: 0,
            paragraph: Some(0),
        }),
        ..ctx()
    };
    let concern = NumberStatus::NotChecked(NotCheckedReason::PConcernAtLevel0(
        ConcernReason::OneTailedUnstated,
    ));
    let precision = NumberStatus::NotChecked(NotCheckedReason::DataPrecision { decimals: 0 });
    for (name, context, p) in [("unstated", ctx(), concern), ("stated", stated, precision)] {
        let (v, _, cands) = run(&printed, &context, student(), &comp);
        assert_eq!(
            v,
            L1Verdict::CouldNotCheck(L1CouldNotCheck::DataPrecision { decimals: 0 }),
            "{name}"
        );
        assert_eq!(p_status(&cands), p, "{name}");
    }
}

/* ---- Spearman --------------------------------------------------------------- */

/// A01's numbers: rho -.965035 on n = 12 (df 10), t-approximation p
/// 3.880966e-07. Rounding can reorder ranks, so the range is unbounded.
fn a01(
    printed_p: &str,
    ties: Option<bool>,
    variant: SpearmanPVariant,
) -> (L1Verdict, Vec<CandidateComparison>) {
    let comp = computation(
        L1MethodId::Spearman,
        result(
            ComputedVariant::SpearmanTApproximation,
            StatisticKind::Rho,
            -0.965_035,
            10.0,
            3.880_966e-7,
            12,
            Precision::Unbounded { decimals: 0 },
        ),
        ties,
    );
    let facts = BindingFacts {
        method: L1MethodId::Spearman,
        variant: Variant::SpearmanP(variant),
        target: None,
    };
    let (v, _, cands) = run(
        &claim(Family::R, "-.97", &["10"], printed_p),
        &ctx(),
        facts,
        &comp,
    );
    (v, cands)
}

/// No variant stated: Level 0's range for rho from -.98 to -.96 at df 10 is
/// 5.8320e-09 to 1.3473e-06, and part C's band widens it to about 2.9e-09 to
/// 1.65e-05. `p = .200` is outside: not matched. `p = .000012` is outside
/// the t approximation's range but inside the band, where the exact p could
/// be: not decided. With ties nothing changes (part C: other software's exact
/// p with ties is not bounded). A stated t approximation compares with
/// Level 0's range itself, so .000012 is then ruled out. (Not .000010: a
/// power of ten is Level 0's floor print, matched with its note.)
#[test]
fn spearman_uses_the_band_unless_the_t_approximation_is_stated_and_ties_change_nothing() {
    let precision = NumberStatus::NotChecked(NotCheckedReason::DataPrecision { decimals: 0 });

    let (v, cands) = a01(".200", Some(false), SpearmanPVariant::NotStated);
    assert!(matches!(v, L1Verdict::NotReproduced { .. }), "{v:?}");
    assert_eq!(wire(p_status(&cands)), ruled_out());

    let (v, cands) = a01(".000012", Some(false), SpearmanPVariant::NotStated);
    assert!(matches!(v, L1Verdict::Reproduced(_)), "{v:?}");
    assert_eq!(p_status(&cands), precision);

    let (v, cands) = a01(".200", Some(true), SpearmanPVariant::NotStated);
    assert!(matches!(v, L1Verdict::Reproduced(_)), "{v:?}");
    assert_eq!(p_status(&cands), precision);

    let (v, cands) = a01(".000012", Some(false), SpearmanPVariant::TApproximation);
    assert!(matches!(v, L1Verdict::NotReproduced { .. }), "{v:?}");
    assert_eq!(wire(p_status(&cands)), ruled_out());
}

/// A01 itself, end to end: "They were related, r(10) = -.97, p = .200." on
/// its 12 whole-number rows, Spearman with no variant stated, the
/// whole-number question left at its default ("Rounded, or not sure"). Part
/// C read "Reproduced, except p, which was not decided". R: rho -0.965035,
/// exact p < 2.2e-16 printed by `cor.test`, t approximation 3.880985e-07.
#[test]
fn a01_is_not_reproduced_under_the_default_answer() {
    let rows: Vec<Vec<String>> = [
        (1, 30),
        (2, 28),
        (3, 29),
        (4, 25),
        (5, 26),
        (6, 22),
        (7, 24),
        (8, 20),
        (9, 21),
        (10, 18),
        (11, 19),
        (12, 15),
    ]
    .iter()
    .map(|(x, y)| vec![x.to_string(), y.to_string()])
    .collect();
    let raw = RawTable::from_rows(vec!["xa".into(), "yb".into()], rows);
    let data = || -> Dataset {
        build_dataset(
            raw.clone(),
            DataFingerprint::of_bytes(b"a01", "part C follow-up", TableRef::FirstTable),
        )
        .expect("dataset")
    };
    let mut s = L1RunState::new();
    s.load_dataset(data());
    let id = s
        .add_claim(&UserClaim {
            family: Family::R,
            statistic: "-.97",
            df: vec!["10"],
            n: None,
            p: UserP::Reported {
                raw: ".200",
                relation: Relation::Eq,
            },
            location: loc(),
            sentence: None,
        })
        .expect("claim");
    let draft = BindingDraft::new(
        L1MethodId::Spearman,
        VariantChoice::not_stated(VariantKind::SpearmanP),
    )
    .role(RoleName::X, &[ColumnAlias(1)])
    .role(RoleName::Y, &[ColumnAlias(2)]);
    let pid = s
        .add_proposal(id, draft, ProposalSource::Manual { edited_from: None })
        .expect("proposal");
    let token = s.confirm(pid).expect("confirmed").1;
    let pair = (s.claim(id).unwrap().revision.clone(), token);
    let out = reproduce(&s, &[pair], || Ok(data()));
    let rep = &out.claims[0];
    assert!(
        matches!(rep.verdict, L1Verdict::NotReproduced { .. }),
        "{:?}",
        rep.verdict
    );
    assert_eq!(wire(p_status(&rep.candidates)), ruled_out());
}
