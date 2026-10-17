//! An overflowed or underflowed computation is refused, never a verdict.
//!
//! Cells near ±1e200 overflow a square (1e400); cells near ±1e-200 underflow
//! one (1e-400 is 0). Checking only the final numbers misses both: an
//! infinite variance gives t = 0, a finite df and p = 1, all finite and all
//! wrong. `stats_verify` checks every intermediate, and both paths get it:
//!
//! * the existing Stats Verifier (`stats_verdict::verify`) returns its
//!   existing validation error with `OVERFLOW_MESSAGE` or `UNDERFLOW_MESSAGE`;
//! * Level 1 (`stats_l1::compute`) returns `NonFinite { at: Result }` for an
//!   overflow and `Underflow` for an underflow.
//!
//! One overflow case and one underflow case per test kind and per method,
//! wherever the method can reach them. Two cannot, and are pinned instead:
//!
//! * Spearman computes on ranks, which are 1..n whatever the cells' scale, so
//!   its result on 1e200- or 1e-200-scaled data is the unit-scale result.
//! * Level 1's chi-square counts rows: every cell of its table is a whole
//!   number no larger than the rows read. The existing verifier's chi-square
//!   takes counts from the user's table, so it is covered below.

use gaply_core::error::GaplyError;
use gaply_core::extract::SectionKind;
use gaply_core::stats_l0::number::Relation;
use gaply_core::stats_l0::result::Family;
use gaply_core::stats_l1::binding::{BindingDraft, OlsTarget, ProposalSource};
use gaply_core::stats_l1::catalogue::{L1MethodId, MethodCatalogue, RoleName, VariantChoice};
use gaply_core::stats_l1::claim::{ClaimLocation, UserClaim, UserP};
use gaply_core::stats_l1::compute::{compute, L1Computation, L1Refusal, NonFiniteAt};
use gaply_core::stats_l1::dataset::{build_dataset, ColumnAlias, DataFingerprint, RawTable, TableRef};
use gaply_core::stats_l1::run_state::L1RunState;
use gaply_core::stats_verdict::{verify, AnalysisSpec, ColumnRoles, ReportedStatistic, TestKind, Tolerance};
use gaply_core::stats_verify::{OVERFLOW_MESSAGE, UNDERFLOW_MESSAGE};

const OVER: f64 = 1e200;
const UNDER: f64 = 1e-200;

// Unit-scale samples with unequal values in every group and pair.
const A: [f64; 3] = [1.0, -1.0, 2.0];
const B: [f64; 3] = [-3.0, 1.5, -2.0];
const C3: [f64; 3] = [0.5, 2.5, -1.5];
const X: [f64; 5] = [1.0, -1.0, 2.0, -2.0, 0.5];
const Y: [f64; 5] = [2.0, -1.0, 1.0, -3.0, 0.0];

fn cell(v: f64) -> String {
    format!("{v:e}")
}

fn scaled(xs: &[f64], k: f64) -> Vec<f64> {
    xs.iter().map(|v| v * k).collect()
}

/* ------------------------------------------------------------------------ *
 *  The existing Stats Verifier
 * ------------------------------------------------------------------------ */

fn spec(kind: TestKind, roles: ColumnRoles) -> AnalysisSpec {
    AnalysisSpec {
        test_kind: kind,
        roles,
        reported: ReportedStatistic { statistic: Some(1.0), p_value: Some(0.5), coefficient_index: None },
        tolerance: Tolerance::default(),
    }
}

/// Columns side by side, as a table of strings.
fn table(headers: &[&str], columns: &[Vec<f64>]) -> (Vec<String>, Vec<Vec<String>>) {
    let n = columns[0].len();
    let rows = (0..n).map(|i| columns.iter().map(|c| cell(c[i])).collect()).collect();
    (headers.iter().map(|h| h.to_string()).collect(), rows)
}

/// What the verifier returned, in words a failure message can show.
fn verifier(kind: TestKind, roles: ColumnRoles, headers: &[&str], columns: &[Vec<f64>]) -> String {
    let (h, r) = table(headers, columns);
    match verify(&spec(kind, roles), &h, &r) {
        Ok(v) => format!("a verdict: {:?} (statistic {}, p {})", v.verdict, v.recomputed.statistic, v.recomputed.p_value),
        Err(GaplyError::Validation(m)) if m == OVERFLOW_MESSAGE => "overflow".into(),
        Err(GaplyError::Validation(m)) if m == UNDERFLOW_MESSAGE => "underflow".into(),
        Err(e) => format!("another error: {e}"),
    }
}

fn samples(cols: &[&str]) -> ColumnRoles {
    ColumnRoles::Samples { columns: cols.iter().map(|c| c.to_string()).collect() }
}

fn both_scales(name: &str, run: impl Fn(f64) -> String) {
    assert_eq!(run(OVER), "overflow", "{name}, cells near ±1e200");
    assert_eq!(run(UNDER), "underflow", "{name}, cells near ±1e-200");
}

#[test]
fn the_verifier_refuses_an_overflowed_or_underflowed_welch_t() {
    both_scales("Welch", |k| verifier(TestKind::TTestWelch, samples(&["a", "b"]), &["a", "b"], &[scaled(&A, k), scaled(&B, k)]));
}

#[test]
fn the_verifier_refuses_an_overflowed_or_underflowed_student_t() {
    both_scales("Student", |k| verifier(TestKind::TTestStudent, samples(&["a", "b"]), &["a", "b"], &[scaled(&A, k), scaled(&B, k)]));
}

#[test]
fn the_verifier_refuses_an_overflowed_or_underflowed_anova() {
    both_scales("ANOVA", |k| {
        verifier(TestKind::Anova, samples(&["a", "b", "c"]), &["a", "b", "c"], &[scaled(&A, k), scaled(&B, k), scaled(&C3, k)])
    });
}

#[test]
fn the_verifier_refuses_an_overflowed_or_underflowed_pearson() {
    let roles = || ColumnRoles::Paired { x: "x".into(), y: "y".into() };
    both_scales("Pearson", |k| verifier(TestKind::Pearson, roles(), &["x", "y"], &[scaled(&X, k), scaled(&Y, k)]));
}

#[test]
fn the_verifier_refuses_an_overflowed_or_underflowed_chi_square() {
    let roles = || ColumnRoles::Contingency { count_columns: vec!["c1".into(), "c2".into()] };
    both_scales("chi-square", |k| {
        verifier(TestKind::ChiSquare, roles(), &["c1", "c2"], &[scaled(&[12.0, 7.0, 4.0], k), scaled(&[5.0, 15.0, 9.0], k)])
    });
}

#[test]
fn the_verifier_refuses_an_overflowed_or_underflowed_regression() {
    let roles = || ColumnRoles::Regression { outcome: "y".into(), predictors: vec!["x".into()] };
    both_scales("OLS", |k| verifier(TestKind::Ols, roles(), &["y", "x"], &[scaled(&Y, k), scaled(&X, k)]));
}

/// Unreachable: Spearman works on ranks. The same verdict at every scale.
#[test]
fn the_verifiers_spearman_cannot_overflow_or_underflow_it_ranks() {
    let roles = || ColumnRoles::Paired { x: "x".into(), y: "y".into() };
    let at = |k: f64| verifier(TestKind::Spearman, roles(), &["x", "y"], &[scaled(&X, k), scaled(&Y, k)]);
    let unit = at(1.0);
    assert!(unit.starts_with("a verdict"), "{unit}");
    assert_eq!(at(OVER), unit);
    assert_eq!(at(UNDER), unit);
}

/* ------------------------------------------------------------------------ *
 *  Level 1
 * ------------------------------------------------------------------------ */

/// One claim, `roles` bound and confirmed, computed on the same bytes read
/// again: the path a user's claim takes.
fn level1(method: L1MethodId, headers: &[&str], rows: Vec<Vec<String>>, roles: &[(RoleName, &[u32])]) -> Result<L1Computation, L1Refusal> {
    let raw = RawTable::from_rows(headers.iter().map(|h| h.to_string()).collect(), rows);
    let bytes = format!("{raw:?}").into_bytes();
    let dataset = || build_dataset(raw.clone(), DataFingerprint::of_bytes(&bytes, "range", TableRef::FirstTable)).expect("dataset");
    let mut draft = BindingDraft::new(method, VariantChoice::not_stated(MethodCatalogue::entry(method).variant_kind));
    for (role, cols) in roles {
        let cols: Vec<ColumnAlias> = cols.iter().map(|&c| ColumnAlias(c)).collect();
        draft = draft.role(*role, &cols);
    }
    if method == L1MethodId::Ols {
        draft = draft.target(OlsTarget::OverallF);
    }
    let mut state = L1RunState::new();
    state.load_dataset(dataset());
    let claim = state
        .add_claim(&UserClaim {
            family: Family::T,
            statistic: "1.00",
            df: vec!["10"],
            n: None,
            p: UserP::Reported { raw: ".05", relation: Relation::Eq },
            location: ClaimLocation { section_index: 0, section: SectionKind::Results, paragraph: 0, sentence: 0 },
            sentence: None,
        })
        .expect("claim");
    let proposal = state.add_proposal(claim, draft, ProposalSource::Manual { edited_from: None }).expect("proposal");
    let token = state.confirm(proposal).unwrap_or_else(|e| panic!("confirm {e:?}")).1;
    compute(&state, &token, claim, &dataset())
}

/// What Level 1 returned, in words.
fn outcome(r: Result<L1Computation, L1Refusal>) -> String {
    match r {
        Ok(c) => format!("computed: {:?}", c.results.iter().map(|r| (r.variant, r.value, r.p)).collect::<Vec<_>>()),
        Err(L1Refusal::NonFinite { at: NonFiniteAt::Result }) => "overflow".into(),
        Err(L1Refusal::Underflow) => "underflow".into(),
        Err(other) => format!("another refusal: {other:?}"),
    }
}

/// A value column and a group column, groups coded 1, 2, 3 in order.
fn grouped(groups: &[Vec<f64>]) -> Vec<Vec<String>> {
    groups.iter().enumerate().flat_map(|(g, vs)| vs.iter().map(move |v| vec![cell(*v), (g + 1).to_string()])).collect()
}

fn pairs(a: &[f64], b: &[f64]) -> Vec<Vec<String>> {
    a.iter().zip(b).map(|(x, y)| vec![cell(*x), cell(*y)]).collect()
}

/// Student's and Welch's t are computed together; either one overflowing or
/// underflowing refuses both.
#[test]
fn level_1_refuses_an_overflowed_or_underflowed_independent_t() {
    both_scales("Level 1 independent t", |k| {
        outcome(level1(
            L1MethodId::IndependentT,
            &["y", "g"],
            grouped(&[scaled(&A, k), scaled(&B, k)]),
            &[(RoleName::Outcome, &[1]), (RoleName::Group, &[2])],
        ))
    });
}

/// The case in the request: pre and post near ±1e200, whose differences'
/// variance is infinite, so t was 0 on 2 df with p = 1.
#[test]
fn level_1_refuses_an_overflowed_or_underflowed_paired_t() {
    let pre = [1.0, -1.0, 2.0];
    let post = [-1.0, 2.0, -2.0];
    both_scales("Level 1 paired t", |k| {
        outcome(level1(
            L1MethodId::PairedT,
            &["pre", "post"],
            pairs(&scaled(&pre, k), &scaled(&post, k)),
            &[(RoleName::First, &[1]), (RoleName::Second, &[2])],
        ))
    });
}

/// Differences that are themselves infinite (cells near ±1.5e308) all
/// compare equal, and must not read as "the differences do not vary".
#[test]
fn level_1_refuses_paired_differences_that_overflow() {
    let pre = [1.5e308, 1.6e308, 1.7e308];
    let post = [-1.5e308, -1.6e308, -1.7e308];
    let got = outcome(level1(L1MethodId::PairedT, &["pre", "post"], pairs(&pre, &post), &[(RoleName::First, &[1]), (RoleName::Second, &[2])]));
    assert_eq!(got, "overflow");
}

#[test]
fn level_1_refuses_an_overflowed_or_underflowed_anova() {
    both_scales("Level 1 ANOVA", |k| {
        outcome(level1(
            L1MethodId::OneWayAnova,
            &["y", "g"],
            grouped(&[scaled(&A, k), scaled(&B, k), scaled(&C3, k)]),
            &[(RoleName::Outcome, &[1]), (RoleName::Group, &[2])],
        ))
    });
}

#[test]
fn level_1_refuses_an_overflowed_or_underflowed_pearson() {
    both_scales("Level 1 Pearson", |k| {
        outcome(level1(L1MethodId::Pearson, &["x", "y"], pairs(&scaled(&X, k), &scaled(&Y, k)), &[(RoleName::X, &[1]), (RoleName::Y, &[2])]))
    });
}

/// The overall F and every coefficient come from one fit; the fit is refused.
#[test]
fn level_1_refuses_an_overflowed_or_underflowed_ols() {
    both_scales("Level 1 OLS", |k| {
        outcome(level1(
            L1MethodId::Ols,
            &["y", "x"],
            pairs(&scaled(&Y, k), &scaled(&X, k)),
            &[(RoleName::Outcome, &[1]), (RoleName::Predictors, &[2])],
        ))
    });
}

/// Unreachable: ranks. The same results at every scale.
#[test]
fn level_1_spearman_cannot_overflow_or_underflow_it_ranks() {
    let at = |k: f64| {
        outcome(level1(L1MethodId::Spearman, &["x", "y"], pairs(&scaled(&X, k), &scaled(&Y, k)), &[(RoleName::X, &[1]), (RoleName::Y, &[2])]))
    };
    let unit = at(1.0);
    assert!(unit.starts_with("computed"), "{unit}");
    assert_eq!(at(OVER), unit);
    assert_eq!(at(UNDER), unit);
}
