//! **The one code set the Level 1 screen's wording table is keyed by.**
//!
//! Every word `gaply_core::stats_l1::codes` can put on the wire, as a code or
//! as a bare nested code, is listed in
//! `src/screens/statsverifier/l1/l1Codes.json`; the screen's vitest
//! (`l1Wording.vitest.ts`) requires exactly one plain sentence for each entry.
//! This test is the Rust half: it drives every code function over every
//! variant of its enum and requires the file to name exactly the words that
//! come out, no more and no fewer.
//!
//! What it cannot see: a new enum variant whose code function is updated
//! (`codes.rs` matches exhaustively, so that is forced) but which is not added
//! to the inputs below. `every_code_is_listed` in `codes.rs` fails on that
//! case, and the list below is copied from its inputs, so update both.

use std::collections::BTreeSet;

use gaply_core::stats_l0::method::{CheckNote, ConcernReason, CouldNotCheckReason, NotApplicableReason};
use gaply_core::stats_l1::binding::ProposalReason;
use gaply_core::stats_l1::catalogue::{ContinuityVariant, SpearmanPVariant, TVariant, Variant, VariantSetBy, WordingSource};
use gaply_core::stats_l1::codes::{self, WireCode};
use gaply_core::stats_l1::compute::{ComputedVariant, ExclusionReason, L1Refusal, NonFiniteAt, OlsTerm};
use gaply_core::stats_l1::dataset::{ColumnAlias, DatasetRefusal, L1IngestRefusal};
use gaply_core::stats_l1::reproduce::{
    BindingGap, BindingSourceKind, ConclusionGap, L1CouldNotCheck, L1Note, MatchNote, NSource, NotCheckedReason, NumberKind,
    NumberStatus, StatisticGap, VariantGap,
};
use gaply_core::stats_l1::run_state::{GateRejection, StaleField};
use gaply_core::stats_l1::token::RevokedBecause;

/// Every code and every bare nested code, from the wire form.
fn words(v: &serde_json::Value, out: &mut BTreeSet<String>) {
    match v {
        serde_json::Value::Object(m) => {
            // `code`'s value is a code; any other string value is a bare
            // nested code. Numbers are parameters.
            for x in m.values() {
                match x {
                    serde_json::Value::String(s) => {
                        out.insert(s.clone());
                    }
                    other => words(other, out),
                }
            }
        }
        serde_json::Value::Array(a) => a.iter().for_each(|x| words(x, out)),
        _ => {}
    }
}

fn every_code() -> Vec<WireCode> {
    let mut all = Vec::new();
    for n in [CheckNote::OneTailedStated, CheckNote::FloorPrintedAsEquality] {
        all.push(codes::check_note(n));
    }
    for r in [ConcernReason::OneTailedUnstated, ConcernReason::FTwoTailed, ConcernReason::EstimateDirection] {
        all.push(codes::concern_reason(r));
    }
    let not_applicable = [
        NotApplicableReason::OtherInputKind,
        NotApplicableReason::NoEstimateAndInterval,
        NotApplicableReason::IntervalNotAfterEstimate,
        NotApplicableReason::DfNotFixedByN,
        NotApplicableReason::NoSampleSizeInUnit,
        NotApplicableReason::PartialCorrelation,
        NotApplicableReason::GranularityUnknown,
    ];
    for r in not_applicable {
        all.push(codes::not_applicable_reason(r));
        all.push(codes::l0_could_not_check_reason(CouldNotCheckReason::NotApplicable(r)));
    }
    for r in [CouldNotCheckReason::AdjustedP, CouldNotCheckReason::InvalidInput, CouldNotCheckReason::Uninformative, CouldNotCheckReason::NoMethodRan] {
        all.push(codes::l0_could_not_check_reason(r));
    }
    for k in [NumberKind::Statistic, NumberKind::Df { index: 0 }, NumberKind::P, NumberKind::N { source: NSource::Stated }, NumberKind::N { source: NSource::ImpliedByDf }] {
        all.push(codes::number_kind(k));
    }
    for n in [MatchNote::InMagnitude, MatchNote::DfTruncated, MatchNote::DfRounded, MatchNote::P(CheckNote::OneTailedStated)] {
        all.push(codes::match_note(n));
    }
    let not_checked = [
        NotCheckedReason::PConcernAtLevel0(ConcernReason::FTwoTailed),
        NotCheckedReason::PCouldNotCheckAtLevel0(CouldNotCheckReason::AdjustedP),
        NotCheckedReason::OneTailedUnstatedOnExactP,
        NotCheckedReason::PConcern(ConcernReason::OneTailedUnstated),
        NotCheckedReason::PCouldNotCheck(CouldNotCheckReason::InvalidInput),
        NotCheckedReason::SpearmanExactPNotComputed,
    ];
    for r in not_checked {
        all.push(codes::not_checked_reason(r));
    }
    for s in [NumberStatus::Matched(None), NumberStatus::Matched(Some(MatchNote::InMagnitude)), NumberStatus::NotMatched, NumberStatus::NotChecked(NotCheckedReason::SpearmanExactPNotComputed)] {
        all.push(codes::number_status(s));
    }
    for v in [
        ComputedVariant::StudentPooled,
        ComputedVariant::Welch,
        ComputedVariant::Paired,
        ComputedVariant::OneWay,
        ComputedVariant::Pearson,
        ComputedVariant::SpearmanTApproximation,
        ComputedVariant::ChiSquareUncorrected,
        ComputedVariant::ChiSquareYatesR,
        ComputedVariant::OlsOverallF,
        ComputedVariant::OlsCoefficient(OlsTerm::Intercept),
        ComputedVariant::OlsCoefficient(OlsTerm::Predictor(ColumnAlias(3))),
    ] {
        all.push(codes::computed_variant(v));
    }
    for g in [ConclusionGap::CandidatesDisagree, ConclusionGap::SeveralAlphas, ConclusionGap::PNotChecked, ConclusionGap::Unplaced] {
        all.push(codes::conclusion_gap(g));
    }
    for n in [
        L1Note::VariantMatched(ComputedVariant::StudentPooled),
        L1Note::SignComparedInMagnitude,
        L1Note::DfTruncated { index: 0 },
        L1Note::DfRounded { index: 0 },
        L1Note::P(CheckNote::FloorPrintedAsEquality),
        L1Note::NotChecked { number: NumberKind::P, why: NotCheckedReason::OneTailedUnstatedOnExactP },
        L1Note::ContinuityDoesNotApply,
        L1Note::ConclusionNotAssessed(ConclusionGap::SeveralAlphas),
    ] {
        all.push(codes::l1_note(n));
    }
    for r in [ExclusionReason::FilteredOut, ExclusionReason::MissingInBoundColumn, ExclusionReason::LevelNotSelected] {
        all.push(codes::exclusion_reason(r));
    }
    let gates = [
        GateRejection::Unknown,
        GateRejection::Revoked(RevokedBecause::ClaimRevised),
        GateRejection::Revoked(RevokedBecause::DatasetReplaced),
        GateRejection::Revoked(RevokedBecause::Withdrawn),
        GateRejection::ClaimMismatch,
        GateRejection::Stale(StaleField::Fingerprint),
        GateRejection::Stale(StaleField::ClaimRevision),
        GateRejection::Stale(StaleField::MethodVersion),
        GateRejection::Stale(StaleField::EngineVersion),
        GateRejection::Stale(StaleField::Binding),
    ];
    for g in gates {
        all.push(codes::gate_rejection(g));
    }
    let refusals = [
        L1Refusal::Gate(GateRejection::Unknown),
        L1Refusal::FingerprintChanged,
        L1Refusal::WeightedFile,
        L1Refusal::MethodNotAvailable,
        L1Refusal::TooFewCompleteCases { n: 2, minimum: 3 },
        L1Refusal::ZeroVariance,
        L1Refusal::SingleGroup { groups: 1 },
        L1Refusal::EmptyRowOrColumn { empty_rows: 1, empty_columns: 0 },
        L1Refusal::SingularDesign,
        L1Refusal::NotNumeric { column: ColumnAlias(2) },
        L1Refusal::NonFinite { at: NonFiniteAt::Input },
        L1Refusal::NonFinite { at: NonFiniteAt::Result },
        L1Refusal::Underflow,
    ];
    for r in refusals {
        all.push(codes::l1_refusal(r));
    }
    let ingest = [
        L1IngestRefusal::NotReadable,
        L1IngestRefusal::TooLarge { bytes: 10, limit: 5 },
        L1IngestRefusal::UnsupportedFormat,
        L1IngestRefusal::Malformed,
        L1IngestRefusal::NoTable,
        L1IngestRefusal::FileTooLarge { limit_rows: 5000 },
        L1IngestRefusal::FileChangedWhileReading,
        L1IngestRefusal::Dataset(DatasetRefusal::TooWide { columns: 200, limit: 128 }),
        L1IngestRefusal::Dataset(DatasetRefusal::NoColumns),
    ];
    for r in ingest {
        all.push(codes::l1_ingest_refusal(r));
    }
    for r in [
        L1CouldNotCheck::NotBound,
        L1CouldNotCheck::MethodNotAvailable,
        L1CouldNotCheck::VariantNotAvailable(VariantGap::SpearmanExactP),
        L1CouldNotCheck::VariantNotAvailable(VariantGap::FractionalDf { index: 1 }),
        L1CouldNotCheck::BindingInvalid(BindingGap::FamilyDoesNotFitMethod),
        L1CouldNotCheck::BindingInvalid(BindingGap::OlsTargetDoesNotFitFamily),
        L1CouldNotCheck::BindingInvalid(BindingGap::DfCountDoesNotFit { printed: 1, expected: 2 }),
        L1CouldNotCheck::StatisticNotCheckable(StatisticGap::PrintedOutOfRange),
        L1CouldNotCheck::StatisticNotCheckable(StatisticGap::NoComputedResult),
    ] {
        all.push(codes::l1_could_not_check(r));
    }
    for g in gates {
        all.push(codes::l1_could_not_check(L1CouldNotCheck::TokenStale(g)));
    }
    for r in refusals {
        all.push(codes::l1_could_not_check(L1CouldNotCheck::Refused(r)));
    }
    for r in ingest {
        all.push(codes::l1_could_not_check(L1CouldNotCheck::DataUnreadable(r)));
    }
    for v in [
        Variant::None,
        Variant::T(TVariant::Student),
        Variant::T(TVariant::Welch),
        Variant::T(TVariant::NotStated),
        Variant::Continuity(ContinuityVariant::Yes),
        Variant::Continuity(ContinuityVariant::No),
        Variant::SpearmanP(SpearmanPVariant::TApproximation),
        Variant::SpearmanP(SpearmanPVariant::Exact),
    ] {
        all.push(codes::variant(v));
    }
    for s in [
        VariantSetBy::NotStated,
        VariantSetBy::WordList { source: WordingSource::ClaimSentence, word: "Welch" },
        VariantSetBy::WordList { source: WordingSource::MethodsPassage, word: "Yates" },
        VariantSetBy::UserTicked,
    ] {
        all.push(codes::variant_set_by(s));
    }
    for b in [
        BindingSourceKind::Local(ProposalReason::HeaderMatchesSentence),
        BindingSourceKind::Local(ProposalReason::FirstEligibleColumns),
        BindingSourceKind::AiSuggestion,
        BindingSourceKind::Manual { edited_from_ai: false },
        BindingSourceKind::Manual { edited_from_ai: true },
    ] {
        all.push(codes::binding_source(b));
    }
    all
}

#[test]
fn the_screen_code_set_names_exactly_the_codes_level_1_can_emit() {
    let mut emitted = BTreeSet::new();
    for c in every_code() {
        words(&serde_json::to_value(&c).unwrap(), &mut emitted);
    }
    // A positive count: the drive reached the codes.
    assert!(emitted.len() > 100, "only {} codes: the drive would pass vacuously", emitted.len());

    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../src/screens/statsverifier/l1/l1Codes.json");
    let file: Vec<String> = serde_json::from_str(&std::fs::read_to_string(path).expect("l1Codes.json")).expect("a JSON list");
    let listed: BTreeSet<String> = file.iter().cloned().collect();
    assert_eq!(listed.len(), file.len(), "l1Codes.json lists a code twice");
    let mut sorted = file.clone();
    sorted.sort();
    assert_eq!(file, sorted, "l1Codes.json is kept sorted");

    let missing: Vec<&String> = emitted.difference(&listed).collect();
    let extra: Vec<&String> = listed.difference(&emitted).collect();
    assert!(
        missing.is_empty() && extra.is_empty(),
        "l1Codes.json must list exactly the codes Level 1 emits.\n  missing: {missing:?}\n  not emitted: {extra:?}\n  expected file:\n{}",
        serde_json::to_string_pretty(&emitted.iter().collect::<Vec<_>>()).unwrap()
    );
}
