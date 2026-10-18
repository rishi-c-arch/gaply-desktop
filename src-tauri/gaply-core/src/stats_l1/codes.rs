//! **Stable snake_case codes for every closed reason, note and status a
//! finished Level 1 result can carry.**
//!
//! One function per enum, each an exhaustive match with no wildcard arm, so a
//! new variant is a compile error here rather than a code nobody named. Never
//! derived from `Debug`: a Rust rename must not change what leaves the device.
//! Parameters are numbers or nested codes, never text.
//!
//! The wire form: `{"code": "df_truncated", "index": 0}`. A nested code with
//! no parameters is written as its bare word:
//! `{"code": "variant_matched", "variant": "student_pooled"}`.
//!
//! One code set for every reader: the helper layer's `result_explanation`
//! sends these, the app crate's views carry them, and the screen maps each to
//! one plain sentence. They live here, beside the enums, so a screen with AI
//! help off still has them and this module still names no helper layer
//! (moved from the helper layer in Step 7, unchanged). The test at the bottom
//! lists every code.

use std::collections::BTreeMap;

use serde::Serialize;

use crate::stats_l0::method::{CheckNote, ConcernReason, CouldNotCheckReason, NotApplicableReason};
use crate::stats_l1::binding::ProposalReason;
use crate::stats_l1::catalogue::{ContinuityVariant, SpearmanPVariant, TVariant, Variant, VariantSetBy, WordingSource};
use crate::stats_l1::compute::{ComputedVariant, ExclusionReason, L1Refusal, NonFiniteAt, OlsTerm};
use crate::stats_l1::dataset::{DatasetRefusal, L1IngestRefusal};
use crate::stats_l1::reproduce::{
    BindingSourceKind,
    BindingGap, ConclusionGap, Contradiction, L1CouldNotCheck, L1Note, MatchNote, NSource, NotCheckedReason, NumberKind,
    NumberStatus, StatisticGap, VariantGap,
};
use crate::stats_l1::run_state::{GateRejection, StaleField};
use crate::stats_l1::token::RevokedBecause;

/// A code and its parameters.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct WireCode {
    pub code: &'static str,
    #[serde(flatten)]
    pub params: BTreeMap<&'static str, Param>,
}

/// A parameter: a number, a bare nested code, or a nested code with its own
/// parameters.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(untagged)]
pub enum Param {
    Number(u64),
    Word(&'static str),
    Code(Box<WireCode>),
}

fn c(code: &'static str) -> WireCode {
    WireCode { code, params: BTreeMap::new() }
}

impl WireCode {
    fn num(mut self, k: &'static str, v: usize) -> WireCode {
        self.params.insert(k, Param::Number(v as u64));
        self
    }
    fn num64(mut self, k: &'static str, v: u64) -> WireCode {
        self.params.insert(k, Param::Number(v));
        self
    }
    fn with(mut self, k: &'static str, inner: WireCode) -> WireCode {
        let p = if inner.params.is_empty() { Param::Word(inner.code) } else { Param::Code(Box::new(inner)) };
        self.params.insert(k, p);
        self
    }
}

/* ---- Level 0's enums ---------------------------------------------------- */

pub fn check_note(n: CheckNote) -> WireCode {
    match n {
        CheckNote::OneTailedStated => c("one_tailed_stated"),
        CheckNote::FloorPrintedAsEquality => c("floor_printed_as_equality"),
        CheckNote::DoubleRounded => c("double_rounded"),
    }
}

pub fn concern_reason(r: ConcernReason) -> WireCode {
    match r {
        ConcernReason::OneTailedUnstated => c("one_tailed_unstated"),
        ConcernReason::FTwoTailed => c("f_two_tailed"),
        ConcernReason::EstimateDirection => c("estimate_direction"),
    }
}

pub fn not_applicable_reason(r: NotApplicableReason) -> WireCode {
    match r {
        NotApplicableReason::OtherInputKind => c("other_input_kind"),
        NotApplicableReason::NoEstimateAndInterval => c("no_estimate_and_interval"),
        NotApplicableReason::IntervalNotAfterEstimate => c("interval_not_after_estimate"),
        NotApplicableReason::DfNotFixedByN => c("df_not_fixed_by_n"),
        NotApplicableReason::NoSampleSizeInUnit => c("no_sample_size_in_unit"),
        NotApplicableReason::PartialCorrelation => c("partial_correlation"),
        NotApplicableReason::GranularityUnknown => c("granularity_unknown"),
    }
}

pub fn l0_could_not_check_reason(r: CouldNotCheckReason) -> WireCode {
    match r {
        CouldNotCheckReason::AdjustedP => c("adjusted_p"),
        CouldNotCheckReason::InvalidInput => c("invalid_input"),
        CouldNotCheckReason::Uninformative => c("uninformative"),
        CouldNotCheckReason::NotApplicable(why) => c("not_applicable").with("reason", not_applicable_reason(why)),
        CouldNotCheckReason::NoMethodRan => c("no_method_ran"),
    }
}

/* ---- numbers ------------------------------------------------------------ */

pub fn n_source(s: NSource) -> WireCode {
    match s {
        NSource::Stated => c("stated"),
        NSource::ImpliedByDf => c("implied_by_df"),
    }
}

pub fn number_kind(k: NumberKind) -> WireCode {
    match k {
        NumberKind::Statistic => c("statistic"),
        NumberKind::Df { index } => c("df").num("index", index),
        NumberKind::P => c("p"),
        NumberKind::N { source } => c("n").with("source", n_source(source)),
    }
}

pub fn match_note(n: MatchNote) -> WireCode {
    match n {
        MatchNote::InMagnitude => c("in_magnitude"),
        MatchNote::DfTruncated => c("df_truncated"),
        MatchNote::DfRounded => c("df_rounded"),
        MatchNote::P(note) => c("p_note").with("note", check_note(note)),
        MatchNote::Truncated => c("truncated"),
        MatchNote::TruncatedInMagnitude => c("truncated_in_magnitude"),
        MatchNote::DoubleRounded => c("double_rounded"),
        MatchNote::DoubleRoundedInMagnitude => c("double_rounded_in_magnitude"),
    }
}

pub fn not_checked_reason(r: NotCheckedReason) -> WireCode {
    match r {
        NotCheckedReason::PConcernAtLevel0(why) => c("p_concern_at_level_0").with("reason", concern_reason(why)),
        NotCheckedReason::PCouldNotCheckAtLevel0(why) => {
            c("p_could_not_check_at_level_0").with("reason", l0_could_not_check_reason(why))
        }
        NotCheckedReason::OneTailedUnstatedOnExactP => c("one_tailed_unstated_on_exact_p"),
        NotCheckedReason::PConcern(why) => c("p_concern").with("reason", concern_reason(why)),
        NotCheckedReason::PCouldNotCheck(why) => c("p_could_not_check").with("reason", l0_could_not_check_reason(why)),
        NotCheckedReason::SpearmanPMayBeExact => c("spearman_p_may_be_exact"),
        NotCheckedReason::SpearmanPWithTies => c("spearman_p_with_ties"),
        NotCheckedReason::DataPrecision { decimals } => c("data_precision").num("decimals", decimals as usize),
    }
}

pub fn contradiction(r: Contradiction) -> WireCode {
    match r {
        Contradiction::PRuledOutByPrintedStatistic => c("p_ruled_out_by_printed_statistic"),
    }
}

pub fn number_status(s: NumberStatus) -> WireCode {
    match s {
        NumberStatus::Matched(None) => c("matched"),
        NumberStatus::Matched(Some(note)) => c("matched").with("note", match_note(note)),
        NumberStatus::NotMatched => c("not_matched"),
        NumberStatus::NotChecked(why) => c("not_checked").with("why", not_checked_reason(why)),
        // Not matched, with the reason (part C follow-up).
        NumberStatus::Contradicted(why) => c("not_matched").with("why", contradiction(why)),
    }
}

/* ---- variants, notes and gaps ------------------------------------------- */

pub fn ols_term(t: OlsTerm) -> WireCode {
    match t {
        OlsTerm::Intercept => c("intercept"),
        OlsTerm::Predictor(alias) => c("predictor").num64("column", u64::from(alias.0)),
    }
}

pub fn computed_variant(v: ComputedVariant) -> WireCode {
    match v {
        ComputedVariant::StudentPooled => c("student_pooled"),
        ComputedVariant::Welch => c("welch"),
        ComputedVariant::Paired => c("paired"),
        ComputedVariant::OneWay => c("one_way"),
        ComputedVariant::Pearson => c("pearson"),
        ComputedVariant::SpearmanTApproximation => c("spearman_t_approximation"),
        ComputedVariant::ChiSquareUncorrected => c("chi_square_uncorrected"),
        ComputedVariant::ChiSquareYatesR => c("chi_square_yates"),
        ComputedVariant::OlsOverallF => c("ols_overall_f"),
        ComputedVariant::OlsCoefficient(term) => c("ols_coefficient").with("term", ols_term(term)),
    }
}

/// A binding's variant, the word a screen and the AI input use for it. Every
/// not-stated member is the one word `not_stated`.
pub fn variant(v: Variant) -> WireCode {
    match v {
        Variant::None => c("none"),
        Variant::T(TVariant::Student) => c("student"),
        Variant::T(TVariant::Welch) => c("welch"),
        Variant::Continuity(ContinuityVariant::Yes) => c("continuity_correction"),
        Variant::Continuity(ContinuityVariant::No) => c("no_continuity_correction"),
        Variant::SpearmanP(SpearmanPVariant::TApproximation) => c("t_approximation"),
        Variant::SpearmanP(SpearmanPVariant::Exact) => c("exact"),
        Variant::T(TVariant::NotStated) | Variant::Continuity(ContinuityVariant::NotStated) | Variant::SpearmanP(SpearmanPVariant::NotStated) => {
            c("not_stated")
        }
    }
}

pub fn wording_source(s: WordingSource) -> WireCode {
    match s {
        WordingSource::ClaimSentence => c("claim_sentence"),
        WordingSource::MethodsPassage => c("methods_passage"),
    }
}

/// Who set a binding's variant. `stated_in_paper` only when Step 2's word list
/// found it in the paper's own text; the matched word itself is not carried.
pub fn variant_set_by(s: VariantSetBy) -> WireCode {
    match s {
        VariantSetBy::NotStated => c("not_stated"),
        VariantSetBy::WordList { source, word: _ } => c("stated_in_paper").with("source", wording_source(source)),
        VariantSetBy::UserTicked => c("user_ticked"),
    }
}

pub fn proposal_reason(r: ProposalReason) -> WireCode {
    match r {
        ProposalReason::HeaderMatchesSentence => c("header_matches_sentence"),
        ProposalReason::FirstEligibleColumns => c("first_eligible_columns"),
    }
}

/// Where a confirmed binding came from.
pub fn binding_source(b: BindingSourceKind) -> WireCode {
    match b {
        BindingSourceKind::Local(r) => c("local").with("reason", proposal_reason(r)),
        BindingSourceKind::AiSuggestion => c("ai_suggestion"),
        BindingSourceKind::Manual { edited_from_ai: false } => c("manual"),
        BindingSourceKind::Manual { edited_from_ai: true } => c("manual_edited_from_ai"),
    }
}

pub fn conclusion_gap(g: ConclusionGap) -> WireCode {
    match g {
        ConclusionGap::CandidatesDisagree => c("candidates_disagree"),
        ConclusionGap::SeveralAlphas => c("several_alphas"),
        ConclusionGap::PNotChecked => c("p_not_checked"),
        ConclusionGap::Unplaced => c("unplaced"),
        ConclusionGap::PJudgedOnPrintedNumbers => c("p_judged_on_printed_numbers"),
    }
}

pub fn l1_note(n: L1Note) -> WireCode {
    match n {
        L1Note::VariantMatched(v) => c("variant_matched").with("variant", computed_variant(v)),
        L1Note::SignComparedInMagnitude => c("sign_compared_in_magnitude"),
        L1Note::DfTruncated { index } => c("df_truncated").num("index", index),
        L1Note::DfRounded { index } => c("df_rounded").num("index", index),
        L1Note::P(note) => c("p_note").with("note", check_note(note)),
        L1Note::NotChecked { number, why } => c("not_checked").with("number", number_kind(number)).with("why", not_checked_reason(why)),
        L1Note::ContinuityDoesNotApply => c("continuity_does_not_apply"),
        L1Note::ConclusionNotAssessed(gap) => c("conclusion_not_assessed").with("gap", conclusion_gap(gap)),
        L1Note::PrintedTruncated { number } => c("printed_truncated").with("number", number_kind(number)),
        L1Note::SpearmanPByTApproximation => c("spearman_p_t_approximation"),
        L1Note::PrintedDoubleRounded { number } => c("printed_double_rounded").with("number", number_kind(number)),
    }
}

pub fn exclusion_reason(r: ExclusionReason) -> WireCode {
    match r {
        ExclusionReason::FilteredOut => c("filtered_out"),
        ExclusionReason::MissingInBoundColumn => c("missing_in_bound_column"),
        ExclusionReason::LevelNotSelected => c("level_not_selected"),
    }
}

/* ---- why a claim could not be checked ----------------------------------- */

pub fn revoked_because(r: RevokedBecause) -> WireCode {
    match r {
        RevokedBecause::ClaimRevised => c("claim_revised"),
        RevokedBecause::DatasetReplaced => c("dataset_replaced"),
        RevokedBecause::Withdrawn => c("withdrawn"),
    }
}

pub fn stale_field(f: StaleField) -> WireCode {
    match f {
        StaleField::Fingerprint => c("fingerprint"),
        StaleField::ClaimRevision => c("claim_revision"),
        StaleField::MethodVersion => c("method_version"),
        StaleField::EngineVersion => c("engine_version"),
        StaleField::Binding => c("binding"),
    }
}

pub fn gate_rejection(g: GateRejection) -> WireCode {
    match g {
        GateRejection::Unknown => c("unknown_token"),
        GateRejection::Revoked(why) => c("revoked").with("because", revoked_because(why)),
        GateRejection::ClaimMismatch => c("claim_mismatch"),
        GateRejection::Stale(field) => c("stale").with("field", stale_field(field)),
    }
}

pub fn non_finite_at(a: NonFiniteAt) -> WireCode {
    match a {
        NonFiniteAt::Input => c("input"),
        NonFiniteAt::Result => c("result"),
    }
}

pub fn l1_refusal(r: L1Refusal) -> WireCode {
    match r {
        L1Refusal::Gate(g) => c("gate").with("rejection", gate_rejection(g)),
        L1Refusal::FingerprintChanged => c("fingerprint_changed"),
        L1Refusal::WeightedFile => c("weighted_file"),
        L1Refusal::MethodNotAvailable => c("method_not_available"),
        L1Refusal::TooFewCompleteCases { n, minimum } => c("too_few_complete_cases").num("n", n).num("minimum", minimum),
        L1Refusal::ZeroVariance => c("zero_variance"),
        L1Refusal::SingleGroup { groups } => c("single_group").num("groups", groups),
        L1Refusal::EmptyRowOrColumn { empty_rows, empty_columns } => {
            c("empty_row_or_column").num("empty_rows", empty_rows).num("empty_columns", empty_columns)
        }
        L1Refusal::SingularDesign => c("singular_design"),
        L1Refusal::NotNumeric { column } => c("not_numeric").num64("column", u64::from(column.0)),
        L1Refusal::NonFinite { at } => c("non_finite").with("at", non_finite_at(at)),
        L1Refusal::Underflow => c("underflow"),
    }
}

pub fn dataset_refusal(r: DatasetRefusal) -> WireCode {
    match r {
        DatasetRefusal::TooWide { columns, limit } => c("too_wide").num("columns", columns).num("limit", limit),
        DatasetRefusal::NoColumns => c("no_columns"),
    }
}

pub fn l1_ingest_refusal(r: L1IngestRefusal) -> WireCode {
    match r {
        L1IngestRefusal::NotReadable => c("not_readable"),
        L1IngestRefusal::TooLarge { bytes, limit } => c("too_large").num64("bytes", bytes).num64("limit", limit),
        L1IngestRefusal::UnsupportedFormat => c("unsupported_format"),
        L1IngestRefusal::Malformed => c("malformed"),
        L1IngestRefusal::NoTable => c("no_table"),
        L1IngestRefusal::FileTooLarge { limit_rows } => c("file_too_large").num("limit_rows", limit_rows),
        L1IngestRefusal::FileChangedWhileReading => c("file_changed_while_reading"),
        L1IngestRefusal::XlsNotReadable => c("xls_not_readable"),
        L1IngestRefusal::Dataset(d) => c("dataset").with("refusal", dataset_refusal(d)),
        L1IngestRefusal::NotARegularFile => c("not_a_regular_file"),
        L1IngestRefusal::ArchiveTooLarge { declared, limit } => c("archive_too_large").num64("declared", declared).num64("limit", limit),
        L1IngestRefusal::ContentTooLarge { limit_bytes } => c("content_too_large").num("limit_bytes", limit_bytes),
        L1IngestRefusal::EncodingNotUtf8 => c("encoding_not_utf8"),
    }
}

pub fn variant_gap(g: VariantGap) -> WireCode {
    match g {
        VariantGap::SpearmanExactP => c("spearman_exact_p"),
        VariantGap::FractionalDf { index } => c("fractional_df").num("index", index),
        VariantGap::DfContradictsStatedVariant { index } => c("df_contradicts_stated_variant").num("index", index),
    }
}

pub fn binding_gap(g: BindingGap) -> WireCode {
    match g {
        BindingGap::FamilyDoesNotFitMethod => c("family_does_not_fit_method"),
        BindingGap::OlsTargetDoesNotFitFamily => c("ols_target_does_not_fit_family"),
        BindingGap::DfCountDoesNotFit { printed, expected } => c("df_count_does_not_fit").num("printed", printed).num("expected", expected),
    }
}

pub fn statistic_gap(g: StatisticGap) -> WireCode {
    match g {
        StatisticGap::PrintedOutOfRange => c("printed_out_of_range"),
        StatisticGap::NoComputedResult => c("no_computed_result"),
    }
}

pub fn l1_could_not_check(r: L1CouldNotCheck) -> WireCode {
    match r {
        L1CouldNotCheck::TokenStale(g) => c("token_stale").with("rejection", gate_rejection(g)),
        L1CouldNotCheck::Refused(r) => c("refused").with("refusal", l1_refusal(r)),
        L1CouldNotCheck::DataUnreadable(r) => c("data_unreadable").with("refusal", l1_ingest_refusal(r)),
        L1CouldNotCheck::MethodNotAvailable => c("method_not_available"),
        L1CouldNotCheck::VariantNotAvailable(g) => c("variant_not_available").with("gap", variant_gap(g)),
        L1CouldNotCheck::BindingInvalid(g) => c("binding_invalid").with("gap", binding_gap(g)),
        L1CouldNotCheck::StatisticNotCheckable(g) => c("statistic_not_checkable").with("gap", statistic_gap(g)),
        L1CouldNotCheck::DataPrecision { decimals } => c("data_precision").num("decimals", decimals as usize),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stats_l1::dataset::ColumnAlias;

    fn j(w: WireCode) -> String {
        serde_json::to_string(&w).unwrap()
    }

    /// Every code of every enum, in the wire form. A rename, a removal or a
    /// new variant shows up here.
    #[test]
    fn every_code_is_listed() {
        let mut got: Vec<String> = Vec::new();
        for n in [CheckNote::OneTailedStated, CheckNote::FloorPrintedAsEquality, CheckNote::DoubleRounded] {
            got.push(j(check_note(n)));
        }
        for r in [ConcernReason::OneTailedUnstated, ConcernReason::FTwoTailed, ConcernReason::EstimateDirection] {
            got.push(j(concern_reason(r)));
        }
        for r in [
            NotApplicableReason::OtherInputKind,
            NotApplicableReason::NoEstimateAndInterval,
            NotApplicableReason::IntervalNotAfterEstimate,
            NotApplicableReason::DfNotFixedByN,
            NotApplicableReason::NoSampleSizeInUnit,
            NotApplicableReason::PartialCorrelation,
            NotApplicableReason::GranularityUnknown,
        ] {
            got.push(j(not_applicable_reason(r)));
        }
        for r in [
            CouldNotCheckReason::AdjustedP,
            CouldNotCheckReason::InvalidInput,
            CouldNotCheckReason::Uninformative,
            CouldNotCheckReason::NotApplicable(NotApplicableReason::DfNotFixedByN),
            CouldNotCheckReason::NoMethodRan,
        ] {
            got.push(j(l0_could_not_check_reason(r)));
        }
        for k in [NumberKind::Statistic, NumberKind::Df { index: 1 }, NumberKind::P, NumberKind::N { source: NSource::Stated }, NumberKind::N { source: NSource::ImpliedByDf }] {
            got.push(j(number_kind(k)));
        }
        for n in [MatchNote::InMagnitude, MatchNote::DfTruncated, MatchNote::DfRounded, MatchNote::P(CheckNote::OneTailedStated), MatchNote::Truncated, MatchNote::TruncatedInMagnitude, MatchNote::DoubleRounded, MatchNote::DoubleRoundedInMagnitude] {
            got.push(j(match_note(n)));
        }
        for r in [
            NotCheckedReason::PConcernAtLevel0(ConcernReason::FTwoTailed),
            NotCheckedReason::PCouldNotCheckAtLevel0(CouldNotCheckReason::AdjustedP),
            NotCheckedReason::OneTailedUnstatedOnExactP,
            NotCheckedReason::PConcern(ConcernReason::OneTailedUnstated),
            NotCheckedReason::PCouldNotCheck(CouldNotCheckReason::InvalidInput),
            NotCheckedReason::SpearmanPMayBeExact,
            NotCheckedReason::SpearmanPWithTies,
            NotCheckedReason::DataPrecision { decimals: 2 },
        ] {
            got.push(j(not_checked_reason(r)));
        }
        for s in [
            NumberStatus::Matched(None),
            NumberStatus::Matched(Some(MatchNote::InMagnitude)),
            NumberStatus::NotMatched,
            NumberStatus::NotChecked(NotCheckedReason::SpearmanPMayBeExact),
            NumberStatus::Contradicted(Contradiction::PRuledOutByPrintedStatistic),
        ] {
            got.push(j(number_status(s)));
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
            got.push(j(computed_variant(v)));
        }
        for g in [
            ConclusionGap::CandidatesDisagree,
            ConclusionGap::SeveralAlphas,
            ConclusionGap::PNotChecked,
            ConclusionGap::Unplaced,
            ConclusionGap::PJudgedOnPrintedNumbers,
        ] {
            got.push(j(conclusion_gap(g)));
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
            L1Note::PrintedTruncated { number: NumberKind::P },
            L1Note::SpearmanPByTApproximation,
            L1Note::PrintedDoubleRounded { number: NumberKind::Statistic },
        ] {
            got.push(j(l1_note(n)));
        }
        for r in [ExclusionReason::FilteredOut, ExclusionReason::MissingInBoundColumn, ExclusionReason::LevelNotSelected] {
            got.push(j(exclusion_reason(r)));
        }
        for g in [
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
        ] {
            got.push(j(gate_rejection(g)));
        }
        for r in [
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
        ] {
            got.push(j(l1_refusal(r)));
        }
        for r in [
            L1IngestRefusal::NotReadable,
            L1IngestRefusal::TooLarge { bytes: 10, limit: 5 },
            L1IngestRefusal::UnsupportedFormat,
            L1IngestRefusal::Malformed,
            L1IngestRefusal::NoTable,
            L1IngestRefusal::FileTooLarge { limit_rows: 5000 },
            L1IngestRefusal::FileChangedWhileReading,
            L1IngestRefusal::XlsNotReadable,
            L1IngestRefusal::Dataset(DatasetRefusal::TooWide { columns: 200, limit: 128 }),
            L1IngestRefusal::Dataset(DatasetRefusal::NoColumns),
            L1IngestRefusal::NotARegularFile,
            L1IngestRefusal::ArchiveTooLarge { declared: 1_572_865_485, limit: 67_108_864 },
            L1IngestRefusal::ContentTooLarge { limit_bytes: 2_097_152 },
            L1IngestRefusal::EncodingNotUtf8,
        ] {
            got.push(j(l1_ingest_refusal(r)));
        }
        for r in [
            L1CouldNotCheck::TokenStale(GateRejection::ClaimMismatch),
            L1CouldNotCheck::Refused(L1Refusal::ZeroVariance),
            L1CouldNotCheck::DataUnreadable(L1IngestRefusal::NoTable),
            L1CouldNotCheck::MethodNotAvailable,
            L1CouldNotCheck::VariantNotAvailable(VariantGap::SpearmanExactP),
            L1CouldNotCheck::VariantNotAvailable(VariantGap::FractionalDf { index: 1 }),
            L1CouldNotCheck::VariantNotAvailable(VariantGap::DfContradictsStatedVariant { index: 0 }),
            L1CouldNotCheck::BindingInvalid(BindingGap::FamilyDoesNotFitMethod),
            L1CouldNotCheck::BindingInvalid(BindingGap::OlsTargetDoesNotFitFamily),
            L1CouldNotCheck::BindingInvalid(BindingGap::DfCountDoesNotFit { printed: 1, expected: 2 }),
            L1CouldNotCheck::StatisticNotCheckable(StatisticGap::PrintedOutOfRange),
            L1CouldNotCheck::StatisticNotCheckable(StatisticGap::NoComputedResult),
            L1CouldNotCheck::DataPrecision { decimals: 2 },
        ] {
            got.push(j(l1_could_not_check(r)));
        }

        let expect = [
            r#"{"code":"one_tailed_stated"}"#,
            r#"{"code":"floor_printed_as_equality"}"#,
            r#"{"code":"double_rounded"}"#,
            r#"{"code":"one_tailed_unstated"}"#,
            r#"{"code":"f_two_tailed"}"#,
            r#"{"code":"estimate_direction"}"#,
            r#"{"code":"other_input_kind"}"#,
            r#"{"code":"no_estimate_and_interval"}"#,
            r#"{"code":"interval_not_after_estimate"}"#,
            r#"{"code":"df_not_fixed_by_n"}"#,
            r#"{"code":"no_sample_size_in_unit"}"#,
            r#"{"code":"partial_correlation"}"#,
            r#"{"code":"granularity_unknown"}"#,
            r#"{"code":"adjusted_p"}"#,
            r#"{"code":"invalid_input"}"#,
            r#"{"code":"uninformative"}"#,
            r#"{"code":"not_applicable","reason":"df_not_fixed_by_n"}"#,
            r#"{"code":"no_method_ran"}"#,
            r#"{"code":"statistic"}"#,
            r#"{"code":"df","index":1}"#,
            r#"{"code":"p"}"#,
            r#"{"code":"n","source":"stated"}"#,
            r#"{"code":"n","source":"implied_by_df"}"#,
            r#"{"code":"in_magnitude"}"#,
            r#"{"code":"df_truncated"}"#,
            r#"{"code":"df_rounded"}"#,
            r#"{"code":"p_note","note":"one_tailed_stated"}"#,
            r#"{"code":"truncated"}"#,
            r#"{"code":"truncated_in_magnitude"}"#,
            r#"{"code":"double_rounded"}"#,
            r#"{"code":"double_rounded_in_magnitude"}"#,
            r#"{"code":"p_concern_at_level_0","reason":"f_two_tailed"}"#,
            r#"{"code":"p_could_not_check_at_level_0","reason":"adjusted_p"}"#,
            r#"{"code":"one_tailed_unstated_on_exact_p"}"#,
            r#"{"code":"p_concern","reason":"one_tailed_unstated"}"#,
            r#"{"code":"p_could_not_check","reason":"invalid_input"}"#,
            r#"{"code":"spearman_p_may_be_exact"}"#,
            r#"{"code":"spearman_p_with_ties"}"#,
            r#"{"code":"data_precision","decimals":2}"#,
            r#"{"code":"matched"}"#,
            r#"{"code":"matched","note":"in_magnitude"}"#,
            r#"{"code":"not_matched"}"#,
            r#"{"code":"not_checked","why":"spearman_p_may_be_exact"}"#,
            r#"{"code":"not_matched","why":"p_ruled_out_by_printed_statistic"}"#,
            r#"{"code":"student_pooled"}"#,
            r#"{"code":"welch"}"#,
            r#"{"code":"paired"}"#,
            r#"{"code":"one_way"}"#,
            r#"{"code":"pearson"}"#,
            r#"{"code":"spearman_t_approximation"}"#,
            r#"{"code":"chi_square_uncorrected"}"#,
            r#"{"code":"chi_square_yates"}"#,
            r#"{"code":"ols_overall_f"}"#,
            r#"{"code":"ols_coefficient","term":"intercept"}"#,
            r#"{"code":"ols_coefficient","term":{"code":"predictor","column":3}}"#,
            r#"{"code":"candidates_disagree"}"#,
            r#"{"code":"several_alphas"}"#,
            r#"{"code":"p_not_checked"}"#,
            r#"{"code":"unplaced"}"#,
            r#"{"code":"p_judged_on_printed_numbers"}"#,
            r#"{"code":"variant_matched","variant":"student_pooled"}"#,
            r#"{"code":"sign_compared_in_magnitude"}"#,
            r#"{"code":"df_truncated","index":0}"#,
            r#"{"code":"df_rounded","index":0}"#,
            r#"{"code":"p_note","note":"floor_printed_as_equality"}"#,
            r#"{"code":"not_checked","number":"p","why":"one_tailed_unstated_on_exact_p"}"#,
            r#"{"code":"continuity_does_not_apply"}"#,
            r#"{"code":"conclusion_not_assessed","gap":"several_alphas"}"#,
            r#"{"code":"printed_truncated","number":"p"}"#,
            r#"{"code":"spearman_p_t_approximation"}"#,
            r#"{"code":"printed_double_rounded","number":"statistic"}"#,
            r#"{"code":"filtered_out"}"#,
            r#"{"code":"missing_in_bound_column"}"#,
            r#"{"code":"level_not_selected"}"#,
            r#"{"code":"unknown_token"}"#,
            r#"{"code":"revoked","because":"claim_revised"}"#,
            r#"{"code":"revoked","because":"dataset_replaced"}"#,
            r#"{"code":"revoked","because":"withdrawn"}"#,
            r#"{"code":"claim_mismatch"}"#,
            r#"{"code":"stale","field":"fingerprint"}"#,
            r#"{"code":"stale","field":"claim_revision"}"#,
            r#"{"code":"stale","field":"method_version"}"#,
            r#"{"code":"stale","field":"engine_version"}"#,
            r#"{"code":"stale","field":"binding"}"#,
            r#"{"code":"gate","rejection":"unknown_token"}"#,
            r#"{"code":"fingerprint_changed"}"#,
            r#"{"code":"weighted_file"}"#,
            r#"{"code":"method_not_available"}"#,
            r#"{"code":"too_few_complete_cases","minimum":3,"n":2}"#,
            r#"{"code":"zero_variance"}"#,
            r#"{"code":"single_group","groups":1}"#,
            r#"{"code":"empty_row_or_column","empty_columns":0,"empty_rows":1}"#,
            r#"{"code":"singular_design"}"#,
            r#"{"code":"not_numeric","column":2}"#,
            r#"{"code":"non_finite","at":"input"}"#,
            r#"{"code":"non_finite","at":"result"}"#,
            r#"{"code":"underflow"}"#,
            r#"{"code":"not_readable"}"#,
            r#"{"code":"too_large","bytes":10,"limit":5}"#,
            r#"{"code":"unsupported_format"}"#,
            r#"{"code":"malformed"}"#,
            r#"{"code":"no_table"}"#,
            r#"{"code":"file_too_large","limit_rows":5000}"#,
            r#"{"code":"file_changed_while_reading"}"#,
            r#"{"code":"xls_not_readable"}"#,
            r#"{"code":"dataset","refusal":{"code":"too_wide","columns":200,"limit":128}}"#,
            r#"{"code":"dataset","refusal":"no_columns"}"#,
            r#"{"code":"not_a_regular_file"}"#,
            r#"{"code":"archive_too_large","declared":1572865485,"limit":67108864}"#,
            r#"{"code":"content_too_large","limit_bytes":2097152}"#,
            r#"{"code":"encoding_not_utf8"}"#,
            r#"{"code":"token_stale","rejection":"claim_mismatch"}"#,
            r#"{"code":"refused","refusal":"zero_variance"}"#,
            r#"{"code":"data_unreadable","refusal":"no_table"}"#,
            r#"{"code":"method_not_available"}"#,
            r#"{"code":"variant_not_available","gap":"spearman_exact_p"}"#,
            r#"{"code":"variant_not_available","gap":{"code":"fractional_df","index":1}}"#,
            r#"{"code":"variant_not_available","gap":{"code":"df_contradicts_stated_variant","index":0}}"#,
            r#"{"code":"binding_invalid","gap":"family_does_not_fit_method"}"#,
            r#"{"code":"binding_invalid","gap":"ols_target_does_not_fit_family"}"#,
            r#"{"code":"binding_invalid","gap":{"code":"df_count_does_not_fit","expected":2,"printed":1}}"#,
            r#"{"code":"statistic_not_checkable","gap":"printed_out_of_range"}"#,
            r#"{"code":"statistic_not_checkable","gap":"no_computed_result"}"#,
            r#"{"code":"data_precision","decimals":2}"#,
        ];
        assert_eq!(got, expect);
    }

    /// Step 7's additions, listed the same way.
    #[test]
    fn every_variant_and_set_by_code_is_listed() {
        let got: Vec<String> = [
            Variant::None,
            Variant::T(TVariant::Student),
            Variant::T(TVariant::Welch),
            Variant::T(TVariant::NotStated),
            Variant::Continuity(ContinuityVariant::Yes),
            Variant::Continuity(ContinuityVariant::No),
            Variant::Continuity(ContinuityVariant::NotStated),
            Variant::SpearmanP(SpearmanPVariant::TApproximation),
            Variant::SpearmanP(SpearmanPVariant::Exact),
            Variant::SpearmanP(SpearmanPVariant::NotStated),
        ]
        .into_iter()
        .map(|v| j(variant(v)))
        .chain(
            [
                VariantSetBy::NotStated,
                VariantSetBy::WordList { source: WordingSource::ClaimSentence, word: "Welch" },
                VariantSetBy::WordList { source: WordingSource::MethodsPassage, word: "Yates" },
                VariantSetBy::UserTicked,
            ]
            .into_iter()
            .map(|s| j(variant_set_by(s))),
        )
        .collect();
        let expect = [
            r#"{"code":"none"}"#,
            r#"{"code":"student"}"#,
            r#"{"code":"welch"}"#,
            r#"{"code":"not_stated"}"#,
            r#"{"code":"continuity_correction"}"#,
            r#"{"code":"no_continuity_correction"}"#,
            r#"{"code":"not_stated"}"#,
            r#"{"code":"t_approximation"}"#,
            r#"{"code":"exact"}"#,
            r#"{"code":"not_stated"}"#,
            r#"{"code":"not_stated"}"#,
            r#"{"code":"stated_in_paper","source":"claim_sentence"}"#,
            r#"{"code":"stated_in_paper","source":"methods_passage"}"#,
            r#"{"code":"user_ticked"}"#,
        ];
        assert_eq!(got, expect);
    }

    #[test]
    fn every_binding_source_code_is_listed() {
        let got: Vec<String> = [
            BindingSourceKind::Local(ProposalReason::HeaderMatchesSentence),
            BindingSourceKind::Local(ProposalReason::FirstEligibleColumns),
            BindingSourceKind::AiSuggestion,
            BindingSourceKind::Manual { edited_from_ai: false },
            BindingSourceKind::Manual { edited_from_ai: true },
        ]
        .into_iter()
        .map(|b| j(binding_source(b)))
        .collect();
        assert_eq!(
            got,
            [
                r#"{"code":"local","reason":"header_matches_sentence"}"#,
                r#"{"code":"local","reason":"first_eligible_columns"}"#,
                r#"{"code":"ai_suggestion"}"#,
                r#"{"code":"manual"}"#,
                r#"{"code":"manual_edited_from_ai"}"#,
            ]
        );
    }

    #[test]
    fn every_code_word_is_snake_case() {
        fn words(v: &serde_json::Value, out: &mut Vec<String>) {
            match v {
                serde_json::Value::String(s) => out.push(s.clone()),
                serde_json::Value::Object(o) => {
                    for (k, x) in o {
                        out.push(k.clone());
                        words(x, out);
                    }
                }
                _ => {}
            }
        }
        let samples = [
            l1_note(L1Note::NotChecked { number: NumberKind::N { source: NSource::ImpliedByDf }, why: NotCheckedReason::PCouldNotCheckAtLevel0(CouldNotCheckReason::NotApplicable(NotApplicableReason::GranularityUnknown)) }),
            l1_could_not_check(L1CouldNotCheck::Refused(L1Refusal::Gate(GateRejection::Stale(StaleField::EngineVersion)))),
        ];
        for s in samples {
            let mut w = Vec::new();
            words(&serde_json::to_value(&s).unwrap(), &mut w);
            assert!(w.len() >= 6);
            for x in w {
                assert!(x.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_'), "{x}");
            }
        }
    }
}
