//! Level 1 reproduction through the run state (Phase 2, Step 4): group
//! selection, filters and missing values; stated and AI-suggested variants;
//! stale tokens, an edited claim, an unbound claim, and data that was never
//! opened or could not be read. No file, no network: the data is read
//! "again" from the same in-memory rows by the closure `reproduce` calls.
//!
//! The printed numbers each claim carries are computed here from the rows the
//! binding selects, by hand-picked indices and `stats_verify` directly, not
//! through Level 1, so the selection is what is under test.

use std::cell::Cell;

use gaply_core::extract::SectionKind;
use gaply_core::stats_l0::number::Relation;
use gaply_core::stats_l0::pvalue;
use gaply_core::stats_l0::result::Family;
use gaply_core::stats_l1::binding::{BindingDraft, ProposalSource};
use gaply_core::stats_l1::catalogue::{L1MethodId, RoleName, TVariant, Variant, VariantChoice, VariantKind, VariantSetBy};
use gaply_core::stats_l1::claim::{ClaimId, ClaimLocation, UserClaim, UserP};
use gaply_core::stats_l1::compute::{ComputedVariant, Exclusion, ExclusionReason, L1Refusal};
use gaply_core::stats_l1::dataset::{build_dataset, ColumnAlias, DataFingerprint, Dataset, L1IngestRefusal, RawTable, TableRef};
use gaply_core::stats_l1::reproduce::{
    reproduce, BindingSourceKind, DataRead, L1CouldNotCheck, L1Note, L1Verdict, MatchNote, NumberKind, NumberStatus, ReproductionOutcome,
};
use gaply_core::stats_l1::run_state::{GateRejection, L1RunState};
use gaply_core::stats_l1::token::{ConfirmationToken, RevokedBecause};
use gaply_core::stats_verify;

const HEADERS: [&str; 2] = ["score", "group"];
/// Four rows per group; one score in group b is missing.
const ROWS: [[&str; 2]; 12] = [
    ["12.1", "a"], ["14.3", "a"], ["11.8", "a"], ["13.6", "a"],
    ["15.2", "b"], ["", "b"], ["16.9", "b"], ["17.4", "b"],
    ["9.9", "c"], ["10.4", "c"], ["8.7", "c"], ["11.2", "c"],
];

fn dataset(bytes: &[u8]) -> Dataset {
    let raw = RawTable::from_rows(
        HEADERS.iter().map(|h| h.to_string()).collect(),
        ROWS.iter().map(|r| r.iter().map(|c| c.to_string()).collect()).collect(),
    );
    build_dataset(raw, DataFingerprint::of_bytes(bytes, "in-memory", TableRef::FirstTable)).expect("dataset")
}

/// a vs b, b's missing score dropped: the numbers a reader would print.
fn printed_a_vs_b() -> (String, String, String) {
    let a = [12.1, 14.3, 11.8, 13.6];
    let b = [15.2, 16.9, 17.4];
    let t = stats_verify::t_test_student(&a, &b).unwrap();
    let p = pvalue::t_two_tailed(t.t, t.df).unwrap();
    assert!(p > 0.001 && p < 0.05, "a moderate p keeps the fixture readable: {p}");
    (format!("{:.2}", t.t), format!("{}", t.df), format!("{:.3}", p).trim_start_matches('0').to_string())
}

fn loc(paragraph: usize) -> ClaimLocation {
    ClaimLocation { section_index: 0, section: SectionKind::Results, paragraph, sentence: 0 }
}

/// A run with one claim, `t(df) = t, p = p`, and the data loaded.
fn run_with_claim(t: &str, df: &str, p: &str) -> L1RunState {
    let mut s = L1RunState::new();
    s.load_dataset(dataset(b"bytes v1"));
    s.add_claim(&UserClaim {
        family: Family::T,
        statistic: t,
        df: vec![df],
        n: None,
        p: UserP::Reported { raw: p, relation: Relation::Eq },
        location: loc(0),
        sentence: None,
    })
    .expect("claim");
    s
}

fn a_vs_b(variant: VariantChoice) -> BindingDraft {
    BindingDraft::new(L1MethodId::IndependentT, variant)
        .role(RoleName::Outcome, &[ColumnAlias(1)])
        .role(RoleName::Group, &[ColumnAlias(2)])
        .filter(ColumnAlias(2), &["a", "b"])
        .levels(RoleName::Group, &["a", "b"])
}

fn confirm(s: &mut L1RunState, draft: BindingDraft, source: ProposalSource) -> (ClaimId, ConfirmationToken) {
    let id = s.add_proposal(ClaimId(0), draft, source).expect("proposal");
    let token = s.confirm(id).expect("confirmed").1;
    (ClaimId(0), token)
}

fn manual() -> ProposalSource {
    ProposalSource::Manual { edited_from: None }
}

/// `reproduce`, counting how often the data is read.
fn run(s: &L1RunState, pairs: &[(gaply_core::stats_l1::claim::RevisionHash, ConfirmationToken)], reads: &Cell<usize>) -> ReproductionOutcome {
    reproduce(s, pairs, || {
        reads.set(reads.get() + 1);
        Ok(dataset(b"bytes v1"))
    })
}

fn pair(s: &L1RunState, token: &ConfirmationToken) -> (gaply_core::stats_l1::claim::RevisionHash, ConfirmationToken) {
    (s.claim(ClaimId(0)).unwrap().revision.clone(), token.clone())
}

#[test]
fn group_selection_filters_and_missing_values_reach_the_verdict_and_the_counts() {
    let (t, df, p) = printed_a_vs_b();
    let mut s = run_with_claim(&t, &df, &p);
    let (_, token) = confirm(&mut s, a_vs_b(VariantChoice::not_stated(VariantKind::T)), manual());
    let reads = Cell::new(0);
    let out = run(&s, &[pair(&s, &token)], &reads);
    assert_eq!(reads.get(), 1);
    assert_eq!(out.data, DataRead::Read);
    let c = &out.claims[0];
    let L1Verdict::Reproduced(notes) = &c.verdict else { panic!("{:?}", c.verdict) };
    assert!(notes.contains(&L1Note::VariantMatched(ComputedVariant::StudentPooled)), "{notes:?}");
    assert!(notes.contains(&L1Note::SignComparedInMagnitude), "{notes:?}");
    let counts = c.counts.as_ref().unwrap();
    assert_eq!(counts.rows_read, 12);
    assert_eq!(counts.n_used, 7);
    assert_eq!(
        counts.excluded,
        vec![
            Exclusion { reason: ExclusionReason::FilteredOut, rows: 4 },
            Exclusion { reason: ExclusionReason::MissingInBoundColumn, rows: 1 },
        ]
    );
    assert_eq!(out.counts.reproduced, 1);
    let record = c.record.as_ref().unwrap();
    assert_eq!(record.check.method_id, "independent_t");
    assert_eq!(record.method_version, "1");
    assert_eq!(record.binding_source, BindingSourceKind::Manual { edited_from_ai: false });
    assert_eq!(record.variant_set_by, VariantSetBy::NotStated);
    assert_eq!(record.check.inputs_digest.len(), 64);
}

#[test]
fn a_stated_welch_variant_computes_welch_only() {
    let (t, df, p) = printed_a_vs_b();
    let mut s = run_with_claim(&t, &df, &p);
    let (_, token) = confirm(&mut s, a_vs_b(VariantChoice::user_ticked(Variant::T(TVariant::Welch))), manual());
    let out = run(&s, &[pair(&s, &token)], &Cell::new(0));
    let c = &out.claims[0];
    assert_eq!(c.candidates.iter().map(|c| c.variant).collect::<Vec<_>>(), vec![ComputedVariant::Welch]);
    assert_eq!(c.record.as_ref().unwrap().variant_set_by, VariantSetBy::UserTicked);
    // The printed numbers are Student's (df 5). Under Welch alone, the df 5
    // is Welch's df rounded (matched, noted), and Welch's t misses the
    // printed Student t by less than the cells' rounding to 1 decimal could
    // move it: since Step 8b (rule 1) that is CouldNotCheck, not
    // NotReproduced. Still Welch only, as the candidates above show.
    assert_eq!(c.verdict, L1Verdict::CouldNotCheck(L1CouldNotCheck::DataPrecision { decimals: 1 }));
    let df = c.candidates[0].numbers.iter().find(|n| n.kind == NumberKind::Df { index: 0 }).unwrap();
    assert_eq!(df.status, NumberStatus::Matched(Some(MatchNote::DfRounded)), "the df, as the comment says");
}

/// The same case as above on EXACT data: whole-number scores, so the cells'
/// rounding gives no precision range (Step 8b, rule 1), and Student-printed
/// numbers under a stated Welch variant must still be NotReproduced. Keeps
/// the coverage the 1-decimal case above lost to rule 1.
#[test]
fn a_stated_welch_variant_refuses_students_numbers_on_exact_data() {
    // Unequal sizes and spreads, so Welch and Student differ well beyond the
    // second decimal.
    let a = [10.0, 12.0, 11.0, 13.0, 12.0, 14.0];
    let b = [15.0, 25.0, 9.0, 30.0, 12.0, 28.0, 20.0, 18.0];
    let student = stats_verify::t_test_student(&a, &b).unwrap();
    let welch = stats_verify::t_test_welch(&a, &b).unwrap();
    let p = pvalue::t_two_tailed(student.t, student.df).unwrap();
    let (t, df, p) = (format!("{:.2}", student.t), format!("{}", student.df), format!("{:.3}", p).trim_start_matches('0').to_string());
    assert!((welch.t.abs() - student.t.abs()).abs() > 0.05, "the fixture separates the variants: {} vs {}", welch.t, student.t);

    let rows: Vec<Vec<String>> = a
        .iter()
        .map(|v| vec![v.to_string(), "a".to_string()])
        .chain(b.iter().map(|v| vec![v.to_string(), "b".to_string()]))
        .collect();
    let exact = |bytes: &[u8]| {
        build_dataset(RawTable::from_rows(vec!["score".into(), "group".into()], rows.clone()), DataFingerprint::of_bytes(bytes, "in-memory", TableRef::FirstTable))
            .expect("dataset")
    };
    let mut s = L1RunState::new();
    s.load_dataset(exact(b"exact v1"));
    s.add_claim(&UserClaim {
        family: Family::T,
        statistic: &t,
        df: vec![df.as_str()],
        n: None,
        p: UserP::Reported { raw: &p, relation: Relation::Eq },
        location: loc(0),
        sentence: None,
    })
    .expect("claim");
    let draft = BindingDraft::new(L1MethodId::IndependentT, VariantChoice::user_ticked(Variant::T(TVariant::Welch)))
        .role(RoleName::Outcome, &[ColumnAlias(1)])
        .role(RoleName::Group, &[ColumnAlias(2)])
        .levels(RoleName::Group, &["a", "b"]);
    let id = s.add_proposal(ClaimId(0), draft, ProposalSource::Manual { edited_from: None }).expect("proposal");
    let token = s.confirm(id).expect("confirmed").1;
    let out = reproduce(&s, &[(s.claim(ClaimId(0)).unwrap().revision.clone(), token)], || Ok(exact(b"exact v1")));
    let c = &out.claims[0];
    // The verdict first: if a stated Welch ever let Student's numbers match,
    // this is the line that says so.
    assert!(matches!(c.verdict, L1Verdict::NotReproduced { .. }), "{:?}", c.verdict);
    assert_eq!(c.candidates.iter().map(|c| c.variant).collect::<Vec<_>>(), vec![ComputedVariant::Welch]);
    // Exact cells: no range on any number, so nothing is excused by rule 1.
    assert!(c.candidates[0].numbers.iter().all(|n| n.precision.is_none()), "{:?}", c.candidates[0].numbers);
}

#[test]
fn an_ai_suggested_variant_left_unconfirmed_is_treated_as_not_stated() {
    let (t, df, p) = printed_a_vs_b();
    let mut s = run_with_claim(&t, &df, &p);
    let ai = ProposalSource::AiSuggestion { task_id: "binding_suggestion".into(), request_id: "req_1".into() };
    let (_, token) = confirm(&mut s, a_vs_b(VariantChoice::user_ticked(Variant::T(TVariant::Welch))), ai);
    let out = run(&s, &[pair(&s, &token)], &Cell::new(0));
    let c = &out.claims[0];
    assert_eq!(
        c.candidates.iter().map(|c| c.variant).collect::<Vec<_>>(),
        vec![ComputedVariant::StudentPooled, ComputedVariant::Welch]
    );
    assert_eq!(c.record.as_ref().unwrap().binding_source, BindingSourceKind::AiSuggestion);
    assert_eq!(c.record.as_ref().unwrap().variant_set_by, VariantSetBy::NotStated);
    assert!(matches!(c.verdict, L1Verdict::Reproduced(_)), "{:?}", c.verdict);
}

#[test]
fn a_withdrawn_token_is_stale_and_the_file_is_never_opened() {
    let (t, df, p) = printed_a_vs_b();
    let mut s = run_with_claim(&t, &df, &p);
    let (_, token) = confirm(&mut s, a_vs_b(VariantChoice::not_stated(VariantKind::T)), manual());
    let pairs = [pair(&s, &token)];
    assert!(s.withdraw(&token));
    let reads = Cell::new(0);
    let out = run(&s, &pairs, &reads);
    assert_eq!(out.claims[0].verdict, L1Verdict::CouldNotCheck(L1CouldNotCheck::TokenStale(GateRejection::Revoked(RevokedBecause::Withdrawn))));
    assert_eq!(reads.get(), 0, "no claim opened the gate, so the data was not read");
    assert_eq!(out.data, DataRead::NotNeeded);
}

#[test]
fn a_claim_edited_after_confirmation_is_stale_not_unbound() {
    let (t, df, p) = printed_a_vs_b();
    let mut s = run_with_claim(&t, &df, &p);
    let (_, token) = confirm(&mut s, a_vs_b(VariantChoice::not_stated(VariantKind::T)), manual());
    let pairs = [pair(&s, &token)];
    let edited = UserClaim {
        family: Family::T,
        statistic: "9.99",
        df: vec![df.as_str()],
        n: None,
        p: UserP::Reported { raw: &p, relation: Relation::Eq },
        location: loc(0),
        sentence: None,
    };
    s.edit_claim(ClaimId(0), &edited).expect("edited");
    let reads = Cell::new(0);
    let out = run(&s, &pairs, &reads);
    assert_eq!(
        out.claims[0].verdict,
        L1Verdict::CouldNotCheck(L1CouldNotCheck::TokenStale(GateRejection::Revoked(RevokedBecause::ClaimRevised)))
    );
    assert_eq!(out.unmatched_pairs, 0, "the pair named the claim's previous revision");
    assert_eq!(reads.get(), 0);
}

#[test]
fn a_claim_without_a_pair_is_not_bound_and_a_pair_naming_nothing_is_counted() {
    let (t, df, p) = printed_a_vs_b();
    let mut s = run_with_claim(&t, &df, &p);
    let (_, token) = confirm(&mut s, a_vs_b(VariantChoice::not_stated(VariantKind::T)), manual());
    let other = run_with_claim("1.00", "5", ".363");
    let foreign = (other.claim(ClaimId(0)).unwrap().revision.clone(), token);
    let out = run(&s, &[foreign], &Cell::new(0));
    assert_eq!(out.claims[0].verdict, L1Verdict::CouldNotCheck(L1CouldNotCheck::NotBound));
    assert_eq!(out.unmatched_pairs, 1);
}

#[test]
fn data_that_cannot_be_read_again_is_could_not_check_for_every_claim_that_needed_it() {
    let (t, df, p) = printed_a_vs_b();
    let mut s = run_with_claim(&t, &df, &p);
    let (_, token) = confirm(&mut s, a_vs_b(VariantChoice::not_stated(VariantKind::T)), manual());
    let out = reproduce(&s, &[pair(&s, &token)], || Err(L1IngestRefusal::Malformed));
    assert_eq!(out.claims[0].verdict, L1Verdict::CouldNotCheck(L1CouldNotCheck::DataUnreadable(L1IngestRefusal::Malformed)));
    assert_eq!(out.data, DataRead::Unreadable(L1IngestRefusal::Malformed));
    // A file read again with other bytes: Step 3 refuses it.
    let out = reproduce(&s, &[pair(&s, &token)], || Ok(dataset(b"bytes v2")));
    assert_eq!(out.claims[0].verdict, L1Verdict::CouldNotCheck(L1CouldNotCheck::Refused(L1Refusal::FingerprintChanged)));
}
