//! **Phase 2 evaluation fixes, part C: the deep evaluation's findings outside
//! the AI layer, each pinned on the report's own inputs**
//! (`~/gaply-corpus/eval3/deep_evaluation.md`).
//!
//! - B-1: whole-number columns. The three smallest reaction-time cases its
//!   scale generator made false "Not reproduced" (`fixtures/stats_l1/
//!   whole_numbers/rt_*.csv`), and a Likert item with a planted typo that
//!   "Exact whole numbers" must still catch.
//! - B-2: double rounding. The two real-paper cases the report traces to it
//!   (pone.0106616 claim 7, pone.0260659 claim 3) and its three examples
//!   (.07465 printed .08, .50472 printed .51, .03488 printed .04).
//! - M-1: an OLS coefficient's sign. M-2: Spearman's p with no variant
//!   stated. M-3: an edit that changes no number. M-4: skipped claims.
//! - m-4: Welch's numbers under a stated Student. A |rho| a few units in the
//!   last place from 1.
//!
//! Every number from R names `/usr/local/bin/Rscript` in its fixture header,
//! or is quoted from the report where the report computed it with R.

use gaply_core::extract::{extract_from_text, SectionKind};
use gaply_core::stats_l0::input::DocumentContext;
use gaply_core::stats_l0::method::CheckNote;
use gaply_core::stats_l0::number::Relation;
use gaply_core::stats_l0::p_consistency::alpha_of;
use gaply_core::stats_l0::pvalue;
use gaply_core::stats_l0::result::{read_results, Family, TableStructure};
use gaply_core::stats_l1::binding::{
    BindingDraft, OlsTarget, ProposalSource, Rejection, WholeNumberAnswer, WholeNumbers,
};
use gaply_core::stats_l1::catalogue::{
    L1MethodId, RoleName, SpearmanPVariant, TVariant, Variant, VariantChoice, VariantKind,
};
use gaply_core::stats_l1::claim::{
    claims_from_reading, ClaimId, ClaimLocation, ClaimOrigin, L1Claim, RevisionHash, UserClaim,
    UserP,
};
use gaply_core::stats_l1::compute::{
    ComputedResult, ComputedVariant, InputCounts, L1Computation, L1Refusal, NonFiniteAt, OlsTerm,
    Precision, StatisticKind, Tail,
};
use gaply_core::stats_l1::dataset::{
    build_dataset, ColumnAlias, DataFingerprint, Dataset, RawTable, TableRef,
};
use gaply_core::stats_l1::reproduce::{
    compare_claim, decide, reproduce, BindingFacts, L1CouldNotCheck, L1Note, L1Verdict,
    NotCheckedReason, NumberKind, ReproductionOutcome, VariantGap,
};
use gaply_core::stats_l1::run_state::{ConfirmError, L1RunState};
use gaply_core::stats_l1::token::ConfirmationToken;
use gaply_core::stats_l1::ENGINE_VERSION;

const C: fn(u32) -> ColumnAlias = ColumnAlias;

/* ---- fixtures ------------------------------------------------------------ */

/// A fixture CSV: its `#` header lines and its table.
fn fixture(dir: &str, name: &str) -> (Vec<String>, RawTable) {
    let path = format!(
        "{}/tests/fixtures/stats_l1/{dir}/{name}.csv",
        env!("CARGO_MANIFEST_DIR")
    );
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"));
    let header: Vec<String> = text
        .lines()
        .filter(|l| l.starts_with('#'))
        .map(str::to_string)
        .collect();
    assert!(
        header.iter().any(|l| l.contains("/usr/local/bin/Rscript")),
        "{name} names its generator"
    );
    let mut lines = text
        .lines()
        .filter(|l| !l.starts_with('#') && !l.trim().is_empty());
    let headers = lines
        .next()
        .unwrap()
        .split(',')
        .map(str::to_string)
        .collect();
    let rows = lines
        .map(|l| l.split(',').map(str::to_string).collect())
        .collect();
    (header, RawTable::from_rows(headers, rows))
}

fn dataset(raw: &RawTable, bytes: &[u8]) -> Dataset {
    build_dataset(
        raw.clone(),
        DataFingerprint::of_bytes(bytes, "part C", TableRef::FirstTable),
    )
    .expect("dataset")
}

fn loc() -> ClaimLocation {
    ClaimLocation {
        section_index: 0,
        section: SectionKind::Results,
        paragraph: 0,
        sentence: 0,
    }
}

/// One printed claim, one draft, confirmed and reproduced on `raw`.
fn run_one(
    raw: &RawTable,
    family: Family,
    stat: &str,
    df: &[&str],
    p: (&str, Relation),
    draft: BindingDraft,
) -> ReproductionOutcome {
    let mut s = L1RunState::new();
    s.load_dataset(dataset(raw, b"bytes"));
    let id = s
        .add_claim(&UserClaim {
            family,
            statistic: stat,
            df: df.to_vec(),
            n: None,
            p: UserP::Reported {
                raw: p.0,
                relation: p.1,
            },
            location: loc(),
            sentence: None,
        })
        .expect("claim");
    let pid = s
        .add_proposal(id, draft, ProposalSource::Manual { edited_from: None })
        .expect("proposal");
    let token = s.confirm(pid).expect("confirmed").1;
    let pair = (s.claim(id).unwrap().revision.clone(), token);
    reproduce(&s, &[pair], || Ok(dataset(raw, b"bytes")))
}

fn not_stated(kind: VariantKind) -> VariantChoice {
    VariantChoice::not_stated(kind)
}

/* ---- B-1: whole-number columns ----------------------------------------- */

/// The three reaction-time cases (ms, SD 120), each with the draft its
/// method needs and the numbers R printed from the unrounded values.
fn reaction_time_cases() -> Vec<(
    &'static str,
    Family,
    &'static str,
    &'static str,
    &'static str,
    BindingDraft,
    Vec<ColumnAlias>,
)> {
    vec![
        (
            "rt_indt_s00186",
            Family::T,
            "0.73",
            "18",
            ".477",
            BindingDraft::new(
                L1MethodId::IndependentT,
                VariantChoice::user_ticked(Variant::T(TVariant::Student)),
            )
            .role(RoleName::Outcome, &[C(1)])
            .role(RoleName::Group, &[C(2)]),
            vec![C(1)],
        ),
        (
            "rt_paired_s00418",
            Family::T,
            "-0.35",
            "9",
            ".732",
            BindingDraft::new(L1MethodId::PairedT, not_stated(VariantKind::None))
                .role(RoleName::First, &[C(1)])
                .role(RoleName::Second, &[C(2)]),
            vec![C(1), C(2)],
        ),
        (
            "rt_pearson_s00234",
            Family::R,
            "-.06",
            "9",
            ".852",
            BindingDraft::new(L1MethodId::Pearson, not_stated(VariantKind::None))
                .role(RoleName::X, &[C(1)])
                .role(RoleName::Y, &[C(2)]),
            vec![C(1), C(2)],
        ),
    ]
}

/// B-1, the report's own inputs: reaction times stored as whole numbers
/// while R computed on the unrounded values. At 5163aec each was a false
/// "Not reproduced". With the default answer, "Rounded, or not sure", the
/// file's rounding (half a millisecond) is in the range, and none may be.
#[test]
fn reaction_times_stored_as_whole_numbers_are_never_not_reproduced_by_default() {
    for (name, family, stat, df, p, draft, bound) in reaction_time_cases() {
        let (header, raw) = fixture("whole_numbers", name);
        let printed = format!("({df}) = {stat}, p = {p}");
        assert!(
            header.iter().any(|l| l.contains(&printed)),
            "{name}: the header prints {printed}"
        );
        let out = run_one(&raw, family, stat, &[df], (p, Relation::Eq), draft);
        let c = &out.claims[0];
        assert!(
            !matches!(c.verdict, L1Verdict::NotReproduced { .. }),
            "{name}: {:?}",
            c.verdict
        );
        // The p is what misses: it is not decided, on the file's rounding.
        let rounding = L1Note::NotChecked {
            number: NumberKind::P,
            why: NotCheckedReason::DataPrecision { decimals: 0 },
        };
        match &c.verdict {
            L1Verdict::Reproduced(notes) => assert!(notes.contains(&rounding), "{name}: {notes:?}"),
            L1Verdict::CouldNotCheck(L1CouldNotCheck::DataPrecision { decimals: 0 }) => {}
            other => panic!("{name}: {other:?}"),
        }
        let want: Vec<WholeNumberAnswer> = bound
            .iter()
            .map(|&column| WholeNumberAnswer {
                column,
                answer: WholeNumbers::RoundedOrNotSure,
            })
            .collect();
        assert_eq!(
            c.whole_numbers, want,
            "{name}: the card says which answer was used"
        );
    }
}

/// "Exact whole numbers" keeps the zero range: the same reaction-time cases
/// are then strict again (the user said the stored values are the values),
/// and a Likert item's planted typo is caught, as at 5163aec.
#[test]
fn exact_whole_numbers_keeps_the_zero_range_and_its_catches() {
    for (name, family, stat, df, p, draft, bound) in reaction_time_cases() {
        let (_, raw) = fixture("whole_numbers", name);
        let out = run_one(
            &raw,
            family,
            stat,
            &[df],
            (p, Relation::Eq),
            draft.exact_whole_numbers(&bound),
        );
        assert!(
            matches!(out.claims[0].verdict, L1Verdict::NotReproduced { .. }),
            "{name}: {:?}",
            out.claims[0].verdict
        );
        assert!(
            out.claims[0]
                .whole_numbers
                .iter()
                .all(|w| w.answer == WholeNumbers::Exact),
            "{name}"
        );
    }
    let (header, raw) = fixture("whole_numbers", "likert_indt_typo");
    assert!(
        header
            .iter()
            .any(|l| l.contains("planted typo print: t(22) = 2.50, p = .021")),
        "{header:?}"
    );
    let draft = || {
        BindingDraft::new(
            L1MethodId::IndependentT,
            VariantChoice::user_ticked(Variant::T(TVariant::Student)),
        )
        .role(RoleName::Outcome, &[C(1)])
        .role(RoleName::Group, &[C(2)])
    };
    let exact = run_one(
        &raw,
        Family::T,
        "2.50",
        &["22"],
        (".021", Relation::Eq),
        draft().exact_whole_numbers(&[C(1)]),
    );
    assert!(
        matches!(exact.claims[0].verdict, L1Verdict::NotReproduced { .. }),
        "{:?}",
        exact.claims[0].verdict
    );
    let honest = run_one(
        &raw,
        Family::T,
        "2.49",
        &["22"],
        (".021", Relation::Eq),
        draft().exact_whole_numbers(&[C(1)]),
    );
    assert!(
        matches!(honest.claims[0].verdict, L1Verdict::Reproduced(_)),
        "{:?}",
        honest.claims[0].verdict
    );
}

/// The answer is part of the confirmed binding: another answer is another
/// digest (so a token for one never opens the other), an answer for a column
/// that is not a bound whole-number column is refused, and an AI suggestion
/// never carries one.
#[test]
fn the_whole_number_answer_is_part_of_the_binding_and_only_the_user_gives_it() {
    let (_, raw) = fixture("whole_numbers", "rt_indt_s00186");
    let mut s = L1RunState::new();
    s.load_dataset(dataset(&raw, b"bytes"));
    let id = s
        .add_claim(&UserClaim {
            family: Family::T,
            statistic: "0.73",
            df: vec!["18"],
            n: None,
            p: UserP::Reported {
                raw: ".477",
                relation: Relation::Eq,
            },
            location: loc(),
            sentence: None,
        })
        .unwrap();
    let base = BindingDraft::new(L1MethodId::IndependentT, not_stated(VariantKind::T))
        .role(RoleName::Outcome, &[C(1)])
        .role(RoleName::Group, &[C(2)]);
    let rounded = s
        .add_proposal(
            id,
            base.clone(),
            ProposalSource::Manual { edited_from: None },
        )
        .unwrap();
    let exact = s
        .add_proposal(
            id,
            base.clone().exact_whole_numbers(&[C(1)]),
            ProposalSource::Manual { edited_from: None },
        )
        .unwrap();
    let d1 = s.confirm(rounded).unwrap().0.digest().to_string();
    let d2 = s.confirm(exact).unwrap().0.digest().to_string();
    assert_ne!(d1, d2);
    // The group column is not a numeric slot.
    let wrong = s
        .add_proposal(
            id,
            base.clone().exact_whole_numbers(&[C(2)]),
            ProposalSource::Manual { edited_from: None },
        )
        .unwrap();
    assert_eq!(
        s.confirm(wrong).unwrap_err(),
        ConfirmError::Rejected(Rejection::WholeNumberAnswerNotApplicable(C(2)))
    );
    // An AI suggestion's answer is cleared: the column is rounded, or not sure.
    let ai = s
        .add_proposal(
            id,
            base.exact_whole_numbers(&[C(1)]),
            ProposalSource::AiSuggestion {
                task_id: "binding_suggestion".into(),
                request_id: "r".into(),
            },
        )
        .unwrap();
    assert!(s
        .proposal(ai)
        .unwrap()
        .binding
        .exact_whole_numbers
        .is_empty());
    let (b, _) = s.confirm(ai).unwrap();
    assert_eq!(
        b.whole_number_answer(C(1)),
        Some(WholeNumbers::RoundedOrNotSure)
    );
}

/* ---- comparisons on computed values ------------------------------------- */

fn claim(family: Family, statistic: &str, df: &[&str], p: &str, relation: Relation) -> L1Claim {
    let u = UserClaim {
        family,
        statistic,
        df: df.to_vec(),
        n: None,
        p: UserP::Reported { raw: p, relation },
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

fn result(
    variant: ComputedVariant,
    statistic: StatisticKind,
    value: f64,
    df: f64,
    p: f64,
    n: usize,
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
        precision: Precision::Exact,
    }
}

fn computation(
    method: L1MethodId,
    results: Vec<ComputedResult>,
    n: usize,
    ties: Option<bool>,
) -> L1Computation {
    L1Computation {
        claim: ClaimId(0),
        method,
        counts: InputCounts {
            rows_read: n,
            excluded: Vec::new(),
            n_used: n,
        },
        results,
        ties,
        engine_version: ENGINE_VERSION,
    }
}

fn verdict(c: &L1Claim, facts: BindingFacts, comp: &L1Computation) -> L1Verdict {
    match compare_claim(&c.result, &ctx(), &facts, comp) {
        Err(why) => L1Verdict::CouldNotCheck(why),
        Ok((cands, _)) => {
            let alpha = alpha_of(&ctx(), &mut Vec::new());
            decide(&cands, &alpha).0
        }
    }
}

fn notes(v: &L1Verdict) -> &[L1Note] {
    match v {
        L1Verdict::Reproduced(n) => n,
        other => panic!("not reproduced: {other:?}"),
    }
}

/* ---- B-2: double rounding ----------------------------------------------- */

/// The report's two real papers. pone.0106616 claim 7: printed
/// `t(23) = 1.59, p = .13`, data t = 1.593567, p = .124684 (R:
/// `2*pt(-1.593567, 23)` = .1246840); .1247 -> .125 -> .13.
/// pone.0260659 claim 3: printed `t = 3.09, df = 1133.87, p < .01`, Welch
/// from the data t = 3.084800, df = 1133.870390; 3.0848 -> 3.085 -> 3.09.
#[test]
fn the_reports_two_real_paper_cases_reproduce_as_double_rounded() {
    let c7 = claim(Family::T, "1.59", &["23"], ".13", Relation::Eq);
    let paired = result(
        ComputedVariant::Paired,
        StatisticKind::T,
        1.593567,
        23.0,
        pvalue::t_two_tailed(1.593567, 23.0).unwrap(),
        24,
    );
    assert!((paired.p - 0.124684).abs() < 5e-7, "{}", paired.p);
    let v = verdict(
        &c7,
        BindingFacts {
            method: L1MethodId::PairedT,
            variant: Variant::None,
            target: None,
        },
        &computation(L1MethodId::PairedT, vec![paired], 24, None),
    );
    assert!(
        notes(&v).contains(&L1Note::P(CheckNote::DoubleRounded)),
        "{v:?}"
    );

    let c3 = claim(Family::T, "3.09", &["1133.87"], ".01", Relation::Lt);
    let welch = result(
        ComputedVariant::Welch,
        StatisticKind::T,
        3.084800,
        1133.870390,
        pvalue::t_two_tailed(3.084800, 1133.870390).unwrap(),
        1303,
    );
    let student = result(
        ComputedVariant::StudentPooled,
        StatisticKind::T,
        3.022682,
        1301.0,
        pvalue::t_two_tailed(3.022682, 1301.0).unwrap(),
        1303,
    );
    let facts = BindingFacts {
        method: L1MethodId::IndependentT,
        variant: Variant::T(TVariant::NotStated),
        target: None,
    };
    let v = verdict(
        &c3,
        facts,
        &computation(L1MethodId::IndependentT, vec![student, welch], 1303, None),
    );
    let n = notes(&v);
    assert!(
        n.contains(&L1Note::VariantMatched(ComputedVariant::Welch)),
        "{n:?}"
    );
    assert!(
        n.contains(&L1Note::PrintedDoubleRounded {
            number: NumberKind::Statistic
        }),
        "{n:?}"
    );
}

/// The report's three examples from its 144 honest double-rounded cases
/// (eval3 cases c000313, c000321, c000331: Student's t, stated; R's exact
/// values from `s3/gen_l1.R`). Plain rounding still wins, with no note; one
/// unit past the double-rounding range is still not matched.
#[test]
fn the_reports_double_rounded_p_examples_match_with_their_note_and_plain_rounding_wins() {
    let student = BindingFacts {
        method: L1MethodId::IndependentT,
        variant: Variant::T(TVariant::Student),
        target: None,
    };
    for (t, df, exact_t, exact_p, printed) in [
        ("-1.82", "54", -1.81775016102242, 0.074651635676024, ".08"),
        ("-0.67", "269", -0.667977148527654, 0.504720882577387, ".51"),
        ("2.14", "93", 2.14110293432388, 0.0348804819616076, ".04"),
    ] {
        let dfv: f64 = df.parse().unwrap();
        let r = result(
            ComputedVariant::StudentPooled,
            StatisticKind::T,
            exact_t,
            dfv,
            exact_p,
            dfv as usize + 2,
        );
        let comp = computation(L1MethodId::IndependentT, vec![r], dfv as usize + 2, None);
        let v = verdict(
            &claim(Family::T, t, &[df], printed, Relation::Eq),
            student,
            &comp,
        );
        assert!(
            notes(&v).contains(&L1Note::P(CheckNote::DoubleRounded)),
            "{printed}: {v:?}"
        );
        // Printed straight to 2 decimals: plain rounding, no note.
        let plain = format!(".{:02}", (exact_p * 100.0).round() as i64);
        let v = verdict(
            &claim(Family::T, t, &[df], &plain, Relation::Eq),
            student,
            &comp,
        );
        assert!(
            !notes(&v).contains(&L1Note::P(CheckNote::DoubleRounded)),
            "{plain}: {v:?}"
        );
        // A last-digit typo past both readings.
        let typo = format!(".{:02}", (exact_p * 100.0).round() as i64 + 2);
        let v = verdict(
            &claim(Family::T, t, &[df], &typo, Relation::Eq),
            student,
            &comp,
        );
        assert!(
            matches!(v, L1Verdict::NotReproduced { .. }),
            "{typo}: {v:?}"
        );
    }
}

/* ---- M-1: signs ---------------------------------------------------------- */

/// The report's planted sign flip (eval3 c019532): an OLS coefficient whose
/// t is +0.3555 (R: `summary(lm(...))`, df 5, p .7367) printed `-0.36`.
/// Its sign IS the finding: not matched. The same print with the sign the
/// data gives matches. An independent t keeps its magnitude rule.
#[test]
fn an_ols_coefficients_sign_is_compared_and_an_independent_ts_is_not() {
    let coef = |printed: &str| {
        let r = result(
            ComputedVariant::OlsCoefficient(OlsTerm::Predictor(C(2))),
            StatisticKind::T,
            0.355518258606636,
            5.0,
            0.736707924070642,
            8,
        );
        let facts = BindingFacts {
            method: L1MethodId::Ols,
            variant: Variant::None,
            target: Some(OlsTarget::Coefficient { predictor: C(2) }),
        };
        verdict(
            &claim(Family::T, printed, &["5"], ".737", Relation::Eq),
            facts,
            &computation(L1MethodId::Ols, vec![r], 8, None),
        )
    };
    assert!(
        matches!(coef("-0.36"), L1Verdict::NotReproduced { .. }),
        "{:?}",
        coef("-0.36")
    );
    assert!(
        matches!(coef("0.36"), L1Verdict::Reproduced(_)),
        "{:?}",
        coef("0.36")
    );
    // An independent t: the sign follows the order of the groups.
    let r = result(
        ComputedVariant::StudentPooled,
        StatisticKind::T,
        -2.1,
        28.0,
        pvalue::t_two_tailed(2.1, 28.0).unwrap(),
        30,
    );
    let facts = BindingFacts {
        method: L1MethodId::IndependentT,
        variant: Variant::T(TVariant::Student),
        target: None,
    };
    let v = verdict(
        &claim(Family::T, "2.10", &["28"], ".045", Relation::Eq),
        facts,
        &computation(L1MethodId::IndependentT, vec![r], 30, None),
    );
    assert!(
        notes(&v).contains(&L1Note::SignComparedInMagnitude),
        "{v:?}"
    );
}

/* ---- M-2: Spearman's p with no variant stated --------------------------- */

fn spearman(printed_p: &str, rho: f64, n: usize, ties: bool) -> L1Verdict {
    let df = (n - 2) as f64;
    let r = result(
        ComputedVariant::SpearmanTApproximation,
        StatisticKind::Rho,
        rho,
        df,
        pvalue::r_two_tailed(rho, df).unwrap(),
        n,
    );
    let facts = BindingFacts {
        method: L1MethodId::Spearman,
        variant: Variant::SpearmanP(SpearmanPVariant::NotStated),
        target: None,
    };
    let printed_rho = format!("{rho:.2}").replace("0.", ".");
    verdict(
        &claim(
            Family::R,
            &printed_rho,
            &[&(n - 2).to_string()],
            printed_p,
            Relation::Eq,
        ),
        facts,
        &computation(L1MethodId::Spearman, vec![r], n, Some(ties)),
    )
}

/// The report's case: `rho = -.77, n = 18, p = 1.000`. R's exact p is
/// .000285 and the t approximation .000186 (`band.R`); 1.000 lies outside
/// any band the exact p can occupy, so it is not matched. A p inside the
/// band stays not checked; with ties R gives the t approximation itself,
/// and the p stays not checked (other software's exact p with ties is not
/// bounded); above n = 1290 R uses the t approximation, so it is compared.
#[test]
fn spearmans_p_outside_the_exact_band_is_not_matched() {
    assert!(
        matches!(
            spearman("1.000", -0.77, 18, false),
            L1Verdict::NotReproduced { .. }
        ),
        "{:?}",
        spearman("1.000", -0.77, 18, false)
    );
    // R's exact p, printed: inside the band.
    let exact = spearman(".000285", -0.77, 18, false);
    let L1Verdict::Reproduced(n) = &exact else {
        panic!("{exact:?}")
    };
    assert!(
        n.contains(&L1Note::NotChecked {
            number: NumberKind::P,
            why: NotCheckedReason::SpearmanPMayBeExact
        }),
        "{n:?}"
    );
    // Ties: not checked, with its own reason.
    let tied = spearman("1.000", -0.77, 18, true);
    let L1Verdict::Reproduced(n) = &tied else {
        panic!("{tied:?}")
    };
    assert!(
        n.contains(&L1Note::NotChecked {
            number: NumberKind::P,
            why: NotCheckedReason::SpearmanPWithTies
        }),
        "{n:?}"
    );
    // n above R's exact range: a missed p is not matched.
    assert!(matches!(
        spearman(".900", 0.06, 1300, false),
        L1Verdict::NotReproduced { .. }
    ));
}

/* ---- m-4: Welch's numbers under a stated Student ------------------------- */

#[test]
fn welchs_fractional_df_under_a_stated_student_says_it_contradicts_the_variant() {
    let c = claim(Family::T, "1.35", &["14.60"], ".198", Relation::Eq);
    let student = result(
        ComputedVariant::StudentPooled,
        StatisticKind::T,
        -0.981543,
        21.0,
        0.337499,
        23,
    );
    let facts = BindingFacts {
        method: L1MethodId::IndependentT,
        variant: Variant::T(TVariant::Student),
        target: None,
    };
    let v = verdict(
        &c,
        facts,
        &computation(L1MethodId::IndependentT, vec![student], 23, None),
    );
    assert_eq!(
        v,
        L1Verdict::CouldNotCheck(L1CouldNotCheck::VariantNotAvailable(
            VariantGap::DfContradictsStatedVariant { index: 0 }
        ))
    );
    // A method with no fractional-df variant keeps its reason.
    let f = claim(Family::F, "3.20", &["2", "18.4"], ".06", Relation::Eq);
    let anova = ComputedResult {
        variant: ComputedVariant::OneWay,
        statistic: StatisticKind::F,
        value: 3.2,
        df1: 2.0,
        df2: Some(27.0),
        p: 0.06,
        tail: Tail::Upper,
        n: 30,
        precision: Precision::Exact,
    };
    let facts = BindingFacts {
        method: L1MethodId::OneWayAnova,
        variant: Variant::None,
        target: None,
    };
    assert_eq!(
        verdict(
            &f,
            facts,
            &computation(L1MethodId::OneWayAnova, vec![anova], 30, None)
        ),
        L1Verdict::CouldNotCheck(L1CouldNotCheck::VariantNotAvailable(
            VariantGap::FractionalDf { index: 1 }
        ))
    );
}

/* ---- M-3: an edit that changes no number ---------------------------------- */

const ARM_A: [f64; 5] = [6.1, 9.3, 3.8, 12.6, 5.2];
const ARM_B: [f64; 9] = [15.2, 16.9, 17.4, 16.0, 15.8, 16.4, 17.1, 15.5, 16.6];

/// The report's probe (`s9/probes.txt`): the sentence states Welch, the
/// numbers are Student's. Before the edit: Not reproduced. An edit that
/// retypes the same numbers, sending no sentence as the edit form does, used
/// to drop the sentence, lose the stated variant and flip the verdict.
#[test]
fn an_edit_that_changes_no_number_keeps_the_sentence_and_the_verdict() {
    let t = gaply_core::stats_verify::t_test_student(&ARM_A, &ARM_B).unwrap();
    let p = pvalue::t_two_tailed(t.t, t.df).unwrap();
    let (ts, dfs, ps) = (
        format!("{:.2}", t.t.abs()),
        format!("{}", t.df),
        format!("{p:.3}").trim_start_matches('0').to_string(),
    );
    let text = format!("A Study\n\nMethods\n\nTwo arms.\n\nResults\n\nThe treated arm differed from control in Welch's t-test, t({dfs}) = {ts}, p = {ps}.\n");
    let ex = extract_from_text(&text);
    let reading = read_results(&ex, TableStructure::NotAvailable);
    let context = DocumentContext::read(&ex, &reading);
    let (claims, _) = claims_from_reading(&ex, &reading);
    assert_eq!(claims.len(), 1);
    let rows: Vec<Vec<String>> = ARM_A
        .iter()
        .map(|v| vec![v.to_string(), "trt".into()])
        .chain(ARM_B.iter().map(|v| vec![v.to_string(), "ctl".into()]))
        .collect();
    let raw = RawTable::from_rows(vec!["score".into(), "arm".into()], rows);
    let mut s = L1RunState::new();
    s.load_dataset(dataset(&raw, b"arms"));
    s.set_claims(claims);
    s.set_context(context);
    let run = |s: &mut L1RunState| -> (L1Verdict, String) {
        let c = s.claim(ClaimId(0)).unwrap().clone();
        // The variant as the command layer reads it: from the claim's sentence.
        let draft = BindingDraft::new(
            L1MethodId::IndependentT,
            VariantChoice::from_wording(VariantKind::T, &c.sentence, None),
        )
        .role(RoleName::Outcome, &[C(1)])
        .role(RoleName::Group, &[C(2)]);
        let pid = s
            .add_proposal(c.id, draft, ProposalSource::Manual { edited_from: None })
            .unwrap();
        let token: ConfirmationToken = s.confirm(pid).unwrap().1;
        let pair: (RevisionHash, ConfirmationToken) = (c.revision.clone(), token);
        let out = reproduce(s, &[pair], || Ok(dataset(&raw, b"arms")));
        (out.claims[0].verdict.clone(), c.sentence)
    };
    let (before, sentence) = run(&mut s);
    assert!(
        matches!(before, L1Verdict::NotReproduced { .. }),
        "{before:?}"
    );
    let location = s.claim(ClaimId(0)).unwrap().location;
    s.edit_claim(
        ClaimId(0),
        &UserClaim {
            family: Family::T,
            statistic: &ts,
            df: vec![&dfs],
            n: None,
            p: UserP::Reported {
                raw: &ps,
                relation: Relation::Eq,
            },
            location,
            sentence: None,
        },
    )
    .unwrap();
    let (after, kept) = run(&mut s);
    assert_eq!(kept, sentence, "the edit kept the sentence");
    assert_eq!(
        after, before,
        "an edit that changes no number does not change the verdict"
    );
}

/* ---- M-4: skipped claims -------------------------------------------------- */

/// The report's probe: one bound claim and one the user skipped. The skipped
/// claim is counted as skipped, and is neither shown nor counted as "Could
/// not check".
#[test]
fn a_skipped_claim_is_counted_as_skipped_never_as_could_not_check() {
    let rows: Vec<Vec<String>> = ARM_A
        .iter()
        .map(|v| vec![v.to_string(), "trt".into()])
        .chain(ARM_B.iter().map(|v| vec![v.to_string(), "ctl".into()]))
        .collect();
    let raw = RawTable::from_rows(vec!["score".into(), "arm".into()], rows);
    let mut s = L1RunState::new();
    s.load_dataset(dataset(&raw, b"arms"));
    let t = gaply_core::stats_verify::t_test_student(&ARM_A, &ARM_B).unwrap();
    let ps = format!("{:.3}", pvalue::t_two_tailed(t.t, t.df).unwrap())
        .trim_start_matches('0')
        .to_string();
    let (ts, dfs) = (format!("{:.2}", t.t.abs()), format!("{}", t.df));
    let add = |s: &mut L1RunState, stat: &str, df: &str, p: &str, paragraph: usize| {
        s.add_claim(&UserClaim {
            family: Family::T,
            statistic: stat,
            df: vec![df],
            n: None,
            p: UserP::Reported {
                raw: p,
                relation: Relation::Eq,
            },
            location: ClaimLocation { paragraph, ..loc() },
            sentence: None,
        })
        .unwrap()
    };
    let bound = add(&mut s, &ts, &dfs, &ps, 0);
    let _skipped = add(&mut s, "2.10", "40", ".042", 1);
    let draft = BindingDraft::new(L1MethodId::IndependentT, not_stated(VariantKind::T))
        .role(RoleName::Outcome, &[C(1)])
        .role(RoleName::Group, &[C(2)]);
    let pid = s
        .add_proposal(bound, draft, ProposalSource::Manual { edited_from: None })
        .unwrap();
    let token = s.confirm(pid).unwrap().1;
    let out = reproduce(
        &s,
        &[(s.claim(bound).unwrap().revision.clone(), token)],
        || Ok(dataset(&raw, b"arms")),
    );
    assert_eq!(out.claims.len(), 1, "only the bound claim has a result");
    assert_eq!(out.claims[0].claim, bound);
    assert_eq!(out.skipped, 1);
    assert_eq!(out.counts.could_not_check, 0);
    assert_eq!(out.counts.reproduced, 1);
}

/* ---- Spearman: |rho| within a few units in the last place of 1 ---------- */

/// The report's note: three perfectly monotone pairs give rho =
/// 0.9999999999999998, not 1, so the |rho| = 1 refusal did not fire and the
/// t approximation reported p = 1.3e-8 on df 1 (R: rho exactly 1). Within a
/// few units in the last place of 1 now counts as 1.
#[test]
fn a_rho_a_few_units_in_the_last_place_from_one_is_refused_as_one() {
    let raw = RawTable::from_rows(
        vec!["x".into(), "y".into()],
        vec![
            vec!["1".into(), "10".into()],
            vec!["2".into(), "20".into()],
            vec!["3".into(), "35".into()],
        ],
    );
    let mut s = L1RunState::new();
    s.load_dataset(dataset(&raw, b"mono"));
    let id = s
        .add_claim(&UserClaim {
            family: Family::R,
            statistic: ".99",
            df: vec!["1"],
            n: None,
            p: UserP::Reported {
                raw: ".001",
                relation: Relation::Lt,
            },
            location: loc(),
            sentence: None,
        })
        .unwrap();
    let draft = BindingDraft::new(L1MethodId::Spearman, not_stated(VariantKind::SpearmanP))
        .role(RoleName::X, &[C(1)])
        .role(RoleName::Y, &[C(2)]);
    let pid = s
        .add_proposal(id, draft, ProposalSource::Manual { edited_from: None })
        .unwrap();
    let token = s.confirm(pid).unwrap().1;
    let out = reproduce(
        &s,
        &[(s.claim(id).unwrap().revision.clone(), token)],
        || Ok(dataset(&raw, b"mono")),
    );
    assert_eq!(
        out.claims[0].verdict,
        L1Verdict::CouldNotCheck(L1CouldNotCheck::Refused(L1Refusal::NonFinite {
            at: NonFiniteAt::Result
        }))
    );
}

/* ---- m-10: data with a 1e9 offset ------------------------------------------ */

/// R's numbers from a fixture header: `stat`, then each df, then `p`.
fn r_numbers(header: &[String]) -> (f64, Vec<f64>, f64) {
    let line = header
        .iter()
        .find(|l| l.contains("stat = "))
        .expect("R line");
    let field = |key: &str| -> String {
        let at = line.find(&format!("{key} = ")).unwrap() + key.len() + 3;
        line[at..]
            .split(", p = ")
            .next()
            .unwrap()
            .split(", df = ")
            .next()
            .unwrap()
            .trim()
            .to_string()
    };
    let df = field("df")
        .split(',')
        .map(|d| d.trim().parse().unwrap())
        .collect();
    (
        field("stat").parse().unwrap(),
        df,
        field("p").parse().unwrap(),
    )
}

fn rel(a: f64, b: f64) -> f64 {
    (a - b).abs() / b.abs()
}

/// The groups of a two-column fixture, in label order.
fn groups(raw: &RawTable) -> Vec<Vec<f64>> {
    let mut labels: Vec<String> = raw.rows.iter().map(|r| r[1].clone()).collect();
    labels.sort();
    labels.dedup();
    labels
        .iter()
        .map(|l| {
            raw.rows
                .iter()
                .filter(|r| &r[1] == l)
                .map(|r| r[0].trim().parse().unwrap())
                .collect()
        })
        .collect()
}

/// The report's smallest offset cases (values about 1e9, SD 10): t and F
/// were off R by up to 1.5e-6 at 5163aec, past every gate, because the mean
/// was a plain `sum / n`. R refines the mean with a second pass; so does
/// `stats_verify` now. Gates as registered: 1e-12 for both methods.
#[test]
fn offset_data_meet_the_gates_against_r() {
    let gate = 1e-12;
    let (h, raw) = fixture("offset", "independent_t_student_d00195");
    let g = groups(&raw);
    let st = gaply_core::stats_verify::t_test_student(&g[0], &g[1]).unwrap();
    let (r_t, _, r_p) = r_numbers(&h);
    assert!(
        rel(st.t, r_t) <= gate,
        "Student t {} vs R {r_t}: {:.1e}",
        st.t,
        rel(st.t, r_t)
    );
    let p = pvalue::t_two_tailed(st.t, st.df).unwrap();
    assert!(rel(p, r_p) <= 1e-9, "Student p {p} vs R {r_p}");

    let (h, raw) = fixture("offset", "independent_t_welch_d00196");
    let g = groups(&raw);
    let we = gaply_core::stats_verify::t_test_welch(&g[0], &g[1]).unwrap();
    let (r_t, r_df, _) = r_numbers(&h);
    assert!(
        rel(we.t, r_t) <= gate,
        "Welch t {} vs R {r_t}: {:.1e}",
        we.t,
        rel(we.t, r_t)
    );
    assert!(
        rel(we.df, r_df[0]) <= gate,
        "Welch df {} vs R {}",
        we.df,
        r_df[0]
    );

    let (h, raw) = fixture("offset", "one_way_anova_oneway_d01793");
    let g = groups(&raw);
    let refs: Vec<&[f64]> = g.iter().map(Vec::as_slice).collect();
    let f = gaply_core::stats_verify::one_way_anova(&refs).unwrap();
    let (r_f, _, _) = r_numbers(&h);
    assert!(
        rel(f.f, r_f) <= gate,
        "F {} vs R {r_f}: {:.1e}",
        f.f,
        rel(f.f, r_f)
    );
}

/// Found by re-running the deep evaluation's scenario B after the whole-number
/// rule: 14 Welch cases (no variant stated, whole numbers stored) stayed false
/// "Not reproduced". Welch's df range was unbounded, so the statistic and p
/// were not decided, but the printed df was still compared strictly. A
/// fractional df whose range is unbounded is not decided either; an integer
/// df does not move with the data's rounding and is still compared.
#[test]
fn an_unbounded_welch_df_is_not_decided_rather_than_not_matched() {
    for (name, stat, df, p) in [
        ("welch_df_c005134", "-1.39", "2.75", ".265"),
        ("welch_df_c005115", "-1.38", "85.41", ".172"),
    ] {
        let (header, raw) = fixture("whole_numbers", name);
        assert!(
            header
                .iter()
                .any(|l| l.contains(&format!("t({df}) = {stat}, p = {p}"))),
            "{name}"
        );
        let draft = BindingDraft::new(L1MethodId::IndependentT, not_stated(VariantKind::T))
            .role(RoleName::Outcome, &[C(1)])
            .role(RoleName::Group, &[C(2)]);
        let out = run_one(&raw, Family::T, stat, &[df], (p, Relation::Eq), draft);
        let v = &out.claims[0].verdict;
        assert!(
            !matches!(v, L1Verdict::NotReproduced { .. }),
            "{name}: {v:?}"
        );
    }
}
