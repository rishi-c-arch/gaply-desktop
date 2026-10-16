//! **Does the data reproduce each printed result?** Comparison, variants and
//! the Level 1 verdict (Phase 2, Step 4). Deterministic and offline.
//!
//! Every number a claim printed, read through Level 0's [`ReportedNumber`],
//! is compared with Step 3's exact values and gets a status: matched, not
//! matched, or not checked with a closed reason.
//!
//! - **Statistic**: the computed value must fall inside the printed interval,
//!   widened by the method's gate ([`L1Registry`]). t compares in magnitude,
//!   because its sign depends on which group comes first (noted); r and rho
//!   compare with their sign.
//! - **df**, by the method's df type, not by how it is printed. Integer-df
//!   variants need the printed df to equal the computed one; a printed df that
//!   is not a whole number, where no candidate variant has a fractional df,
//!   signals a correction Gaply does not compute (Welch's ANOVA,
//!   Brown-Forsythe, Greenhouse-Geisser): `CouldNotCheck(VariantNotAvailable)`.
//!   Welch's df matches a printed whole number when that is the computed df
//!   rounded or truncated (noted which), and a printed decimal when the
//!   computed df falls inside its interval.
//! - **p**: Phase 1's own comparison, [`compare_p`], unchanged, with the exact
//!   computed p as a zero-width range. Where Level 0 returned a Concern or a
//!   CouldNotCheck for the claim's p, the p is not checked here and never
//!   makes the claim NotReproduced.
//! - **n**, where the claim prints it, or for r where its df implies it:
//!   equal to the n used.
//!
//! **Variants.** Not stated: every variant Step 3 computes is a candidate, and
//! the claim is reproduced when every checked number matches under ONE of
//! them, which the outcome names. Stated: that variant only. A variant Gaply
//! does not compute is never assumed to match.
//!
//! **The file's own precision and truncated prints (Step 8b).** A number
//! that misses its printed interval is first checked against TRUNCATION: a
//! printed equality whose digits are the exact value truncated toward zero
//! is matched, with a note (rule 2; Level 0 has no such rule). Otherwise,
//! when the result carries a precision range ([`Precision`]) that the
//! printed interval meets (for p, by Phase 1's comparison over the p range),
//! the number is not checked, `DataPrecision`: the stored cells' rounding
//! alone could explain the miss (rule 1). A statistic in that state makes
//! the claim CouldNotCheck; a p in it leaves a matched claim Reproduced with
//! the note.
//!
//! The outcome never says which number is wrong: it shows each printed number
//! beside the computed one, with what it had to fall in and a status. No
//! score, no percentage, no Concern.

use super::binding::{ConfirmedBinding, OlsTarget, ProposalReason, ProposalSource, WholeNumberAnswer};
use super::catalogue::{ContinuityVariant, L1MethodId, SpearmanPVariant, TVariant, Variant, VariantChoice, VariantSetBy};
use super::claim::{ClaimId, ClaimLocation, ClaimOrigin, L1Claim, RevisionHash};
use super::compute::{self, ComputedResult, ComputedVariant, InputCounts, L1Computation, L1Refusal, L1Registry, OlsTerm, Precision};
use super::dataset::{Dataset, L1IngestRefusal};
use super::run_state::{GateRejection, L1RunState};
use super::token::ConfirmationToken;
use super::ENGINE_VERSION;
use crate::stats_l0::input::{DocumentContext, L0Input};
use crate::stats_l0::method::{
    digest_hex, CheckNote, Computed, ConcernReason, CouldNotCheckReason, L0Verdict, MethodCheck, Provenance, ToleranceRule,
    VerificationMethod,
};
use crate::stats_l0::number::{Relation, ReportedNumber};
use crate::stats_l0::p_consistency::{
    alpha_of, compare_p, conclusion_changes_of, double_rounded_reach, is_variance_ratio_candidate, widen, Alpha, PComparison, PConsistency,
};
use crate::stats_l0::pvalue;
use crate::equation::rational::Rational;
use crate::stats_l0::result::{Family, PValue, ReportedResult};

/// The record's provenance: the methods are R's, measured against it in
/// `tests/l1_r_golden.rs`.
pub const PROVENANCE: Provenance =
    Provenance::Published { reference: "R Core Team, R: A Language and Environment for Statistical Computing (stats package)" };

/* ------------------------------------------------------------------------ *
 *  Numbers
 * ------------------------------------------------------------------------ */

/// Which printed number a comparison is about.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NumberKind {
    Statistic,
    /// The `index`-th printed df (0 for t, r and chi-square; 0 and 1 for F).
    Df { index: usize },
    P,
    N { source: NSource },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NSource {
    /// The claim prints n.
    Stated,
    /// r with no n printed: its df implies n = df + 2.
    ImpliedByDf,
}

/// The printed side of a comparison.
#[derive(Debug, Clone, PartialEq)]
pub enum Printed {
    Number(ReportedNumber),
    /// `ns`, `n.s.`: a p printed only as not significant.
    NotSignificant,
}

/// Why a number matched in a way the reader should be told.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MatchNote {
    /// t compared without its sign.
    InMagnitude,
    /// Welch's df printed as the computed df truncated.
    DfTruncated,
    /// Welch's df printed as the computed df rounded.
    DfRounded,
    /// Phase 1's note on the p comparison.
    P(CheckNote),
    /// A printed equality whose digits are the exact value truncated toward
    /// zero, not rounded (Step 8b, rule 2).
    Truncated,
    /// The same for t, compared without its sign.
    TruncatedInMagnitude,
    /// A printed equality that is the exact value rounded twice: first to 3
    /// decimals, or to one more decimal than printed, then to the printed
    /// decimals (evaluation fixes, part C, B-2).
    DoubleRounded,
    /// The same for a t compared without its sign.
    DoubleRoundedInMagnitude,
}

/// Why a printed number was not checked. Closed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotCheckedReason {
    /// Level 0, on the printed numbers alone, returned a Concern for this p.
    PConcernAtLevel0(ConcernReason),
    /// Level 0, on the printed numbers alone, could not check this p.
    PCouldNotCheckAtLevel0(CouldNotCheckReason),
    /// The exact p matches the printed one only as a one-tailed test, and the
    /// paper does not say one-tailed: Level 0's unstated one-tailed ambiguity,
    /// seen on the exact p. Never "not matched" (a false NotReproduced is the
    /// worst error); Step 7 words it exactly as Level 0's
    /// `one_tailed_unstated` note, beside the p.
    OneTailedUnstatedOnExactP,
    /// Phase 1's comparison of the exact p returned another Concern.
    PConcern(ConcernReason),
    /// Phase 1's comparison of the exact p could not be made.
    PCouldNotCheck(CouldNotCheckReason),
    /// Spearman with no variant stated: the printed p does not match the t
    /// approximation (R's `cor.test(exact = FALSE)`, what SPSS reports). The
    /// paper may have used the exact p, which Level 1 does not compute yet,
    /// so the p alone does not make the claim NotReproduced. A stated t
    /// approximation compares the p normally.
    SpearmanPMayBeExact,
    /// Spearman with no variant stated and tied values: R's `cor.test` then
    /// gives the t approximation itself, but other software gives an exact
    /// or permutation p with ties, which Level 1 does not bound, so a printed
    /// p that misses the t approximation is not checked (part C).
    SpearmanPWithTies,
    /// The printed number misses the exact value only by what the rounding
    /// of the stored cells (to `decimals`) could explain (Step 8b, rule 1).
    DataPrecision { decimals: u32 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NumberStatus {
    Matched(Option<MatchNote>),
    NotMatched,
    NotChecked(NotCheckedReason),
}

/// One printed number beside the computed one.
#[derive(Debug, Clone, PartialEq)]
pub struct NumberComparison {
    pub kind: NumberKind,
    pub printed: Printed,
    /// The exact computed value: the statistic, a df, the p, or the n used.
    pub computed: f64,
    /// What the computed value had to fall in: the printed interval widened by
    /// the gate (statistic and df; in magnitude for t), the printed p's
    /// interval by its relation (`<` from 0, `>` to 1), or the printed count.
    pub accepted: (f64, f64),
    pub status: NumberStatus,
    /// What the stored cells' rounding could make the computed value, when
    /// that range is not zero (Step 8b): signed, as computed.
    pub precision: Option<(f64, f64)>,
}

/// Every number compared under one candidate variant.
#[derive(Debug, Clone, PartialEq)]
pub struct CandidateComparison {
    pub variant: ComputedVariant,
    pub numbers: Vec<NumberComparison>,
    /// Phase 1's p comparison under this variant; Level 0's own verdict on
    /// the printed numbers when that left the p unchecked.
    pub p_outcome: Option<L0Verdict>,
    /// Phase 1's `conclusion_changes` for this variant's exact p.
    pub conclusion_changes: Option<bool>,
    /// The decimals the bound cells are stored to, when that is not exact
    /// (Step 8b): what the view says beside a precision range.
    pub stored_decimals: Option<u32>,
}

impl CandidateComparison {
    /// Every checked number matched and the statistic was checked.
    pub fn matches(&self) -> bool {
        statuses_match(self.numbers.iter().map(|n| (n.kind, n.status)))
    }
    /// At least one checked number did not match.
    pub fn fails(&self) -> bool {
        self.numbers.iter().any(|n| n.status == NumberStatus::NotMatched)
    }
}

/// The candidate rule on statuses alone: the statistic matched, and no number
/// is `NotMatched`. `NotChecked` numbers neither help nor hurt.
pub fn statuses_match(numbers: impl Iterator<Item = (NumberKind, NumberStatus)> + Clone) -> bool {
    numbers.clone().any(|(k, s)| k == NumberKind::Statistic && matches!(s, NumberStatus::Matched(_)))
        && !numbers.into_iter().any(|(_, s)| s == NumberStatus::NotMatched)
}

/* ------------------------------------------------------------------------ *
 *  The verdict
 * ------------------------------------------------------------------------ */

/// The Level 1 verdict for one claim. Closed, distinct from Level 0's
/// `L0Verdict`. `CouldNotCheck` is never a pass. No Concern at Level 1.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum L1Verdict {
    /// Every checked number matches under one variant, and the statistic was
    /// checked.
    Reproduced(Vec<L1Note>),
    /// Under every candidate variant, at least one checked number does not
    /// match. `conclusion_changes`: `Some(true)` only when every candidate's
    /// exact p falls on the other side of the claim's alpha from the printed
    /// p; `Some(false)` when none does; `None` otherwise, with a note.
    NotReproduced { conclusion_changes: Option<bool> },
    CouldNotCheck(L1CouldNotCheck),
}

/// What a reader is told beside the verdict. Closed codes with numeric
/// parameters only.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum L1Note {
    /// A variant under which every checked number matched; one note per such
    /// variant, in computation order.
    VariantMatched(ComputedVariant),
    /// t was compared in magnitude.
    SignComparedInMagnitude,
    DfTruncated { index: usize },
    DfRounded { index: usize },
    /// Phase 1's note on the p comparison.
    P(CheckNote),
    NotChecked { number: NumberKind, why: NotCheckedReason },
    /// A continuity correction was stated for a table that is not 2×2, where
    /// R applies none: the uncorrected statistic is compared.
    ContinuityDoesNotApply,
    /// Why `conclusion_changes` is `None`.
    ConclusionNotAssessed(ConclusionGap),
    /// A printed equality matched as the exact value truncated, not rounded.
    PrintedTruncated { number: NumberKind },
    /// Spearman's p matched the t approximation, the method named here
    /// (R's `cor.test(exact = FALSE)`, SPSS).
    SpearmanPByTApproximation,
    /// A printed equality matched as the exact value rounded twice (part C).
    PrintedDoubleRounded { number: NumberKind },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConclusionGap {
    /// The candidates' p values fall on different sides.
    CandidatesDisagree,
    /// The document states more than one alpha.
    SeveralAlphas,
    /// The p was not checked, so its side is not compared.
    PNotChecked,
    /// Phase 1's comparison could not place the printed p.
    Unplaced,
}

/// Why a claim could not be checked. Closed codes with numeric parameters.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum L1CouldNotCheck {
    /// The token did not open the gate: unknown, revoked (the claim was
    /// edited, the dataset replaced, the binding withdrawn) or stale.
    TokenStale(GateRejection),
    /// Step 3 refused to compute.
    Refused(L1Refusal),
    /// The data file could not be read again.
    DataUnreadable(L1IngestRefusal),
    MethodNotAvailable,
    VariantNotAvailable(VariantGap),
    /// The confirmed binding cannot produce the printed statistic.
    BindingInvalid(BindingGap),
    StatisticNotCheckable(StatisticGap),
    /// The statistic misses only by what the rounding of the stored cells
    /// could explain, under every candidate that does not fail outright.
    DataPrecision { decimals: u32 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VariantGap {
    /// The binding states Spearman's exact p, which Gaply does not compute.
    SpearmanExactP,
    /// A printed df that is not a whole number on a method whose candidate
    /// variants all have integer df.
    FractionalDf { index: usize },
    /// A printed df that is not a whole number where the binding states a
    /// variant with an integer df and the method has a variant with a
    /// fractional one (Welch's numbers under a stated Student): the printed
    /// df contradicts the stated variant (part C, m-4).
    DfContradictsStatedVariant { index: usize },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BindingGap {
    /// The method does not produce the claim's statistic (an F bound to a
    /// t-test, a z to anything).
    FamilyDoesNotFitMethod,
    /// OLS: the target (overall F or a coefficient) does not produce the
    /// claim's statistic.
    OlsTargetDoesNotFitFamily,
    /// The claim prints a different number of df than the statistic has.
    DfCountDoesNotFit { printed: usize, expected: usize },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatisticGap {
    /// The printed statistic defines no value of its kind: |r| at or beyond
    /// 1, a negative F or chi-square.
    PrintedOutOfRange,
    /// No computed result for a candidate variant.
    NoComputedResult,
}

/* ------------------------------------------------------------------------ *
 *  Provenance
 * ------------------------------------------------------------------------ */

/// Where the binding came from, as a closed code: the local proposer's
/// matched header tokens stay behind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BindingSourceKind {
    Local(ProposalReason),
    AiSuggestion,
    /// Set or edited by hand; `edited_from_ai` when an AI suggestion is
    /// anywhere in what it was edited from.
    Manual { edited_from_ai: bool },
}

impl BindingSourceKind {
    pub fn of(source: &ProposalSource) -> BindingSourceKind {
        fn has_ai(s: &ProposalSource) -> bool {
            match s {
                ProposalSource::AiSuggestion { .. } => true,
                ProposalSource::Local { .. } => false,
                ProposalSource::Manual { edited_from } => edited_from.as_deref().is_some_and(has_ai),
            }
        }
        match source {
            ProposalSource::Local { reason, .. } => BindingSourceKind::Local(*reason),
            ProposalSource::AiSuggestion { .. } => BindingSourceKind::AiSuggestion,
            ProposalSource::Manual { edited_from } => {
                BindingSourceKind::Manual { edited_from_ai: edited_from.as_deref().is_some_and(has_ai) }
            }
        }
    }
}

/// Provenance for one computed claim: Phase 1's [`MethodCheck`], unchanged,
/// plus what Level 1 adds.
///
/// The check's `inputs_digest` is SHA-256 over the method id and version, the
/// data fingerprint, the claim revision and the normalised binding. Its
/// `outcome` is Phase 1's p comparison under the variant the outcome shows,
/// or Level 0's verdict on the printed numbers when that left the p
/// unchecked.
#[derive(Debug, Clone, PartialEq)]
pub struct L1Record {
    pub check: MethodCheck,
    pub method_version: &'static str,
    pub binding_source: BindingSourceKind,
    pub variant_set_by: VariantSetBy,
}

/* ------------------------------------------------------------------------ *
 *  One claim
 * ------------------------------------------------------------------------ */

/// Everything Level 1 says about one claim. Local only: never `Serialize`
/// (the printed numbers are manuscript text).
#[derive(Debug, Clone, PartialEq)]
pub struct ClaimReproduction {
    pub claim: ClaimId,
    pub revision: RevisionHash,
    pub location: ClaimLocation,
    pub method: Option<L1MethodId>,
    pub variant: Option<VariantChoice>,
    pub verdict: L1Verdict,
    /// Notes beside any verdict (a `None` conclusion's reason, numbers not
    /// checked). A `Reproduced` verdict carries its own.
    pub notes: Vec<L1Note>,
    /// Every candidate variant's comparisons; empty when nothing was computed.
    pub candidates: Vec<CandidateComparison>,
    /// Rows read, excluded by reason, and the n used.
    pub counts: Option<InputCounts>,
    pub record: Option<L1Record>,
    /// The bound whole-number columns and the answer each was confirmed with
    /// (part C, B-1): what the result card says it assumed.
    pub whole_numbers: Vec<WholeNumberAnswer>,
}

impl ClaimReproduction {
    fn could_not_check(claim: &L1Claim, why: L1CouldNotCheck, binding: Option<&ConfirmedBinding>) -> ClaimReproduction {
        ClaimReproduction {
            claim: claim.id,
            revision: claim.revision.clone(),
            location: claim.location,
            method: binding.map(|b| b.method()),
            variant: binding.map(|b| b.variant()),
            verdict: L1Verdict::CouldNotCheck(why),
            notes: Vec::new(),
            candidates: Vec::new(),
            counts: None,
            record: None,
            whole_numbers: binding.map(|b| b.whole_numbers().to_vec()).unwrap_or_default(),
        }
    }

    /// The candidate the outcome shows first: the first that matched, else the
    /// first computed.
    pub fn shown(&self) -> Option<&CandidateComparison> {
        self.candidates.iter().find(|c| c.matches()).or(self.candidates.first())
    }
}

/// What the claim's binding asks for, as the comparison needs it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BindingFacts {
    pub method: L1MethodId,
    pub variant: Variant,
    pub target: Option<OlsTarget>,
}

impl BindingFacts {
    pub fn of(b: &ConfirmedBinding) -> BindingFacts {
        BindingFacts { method: b.method(), variant: b.variant().variant(), target: b.target() }
    }
}

/// The candidate results for the binding's variant, and any note the choice
/// needs.
fn candidates<'c>(facts: &BindingFacts, c: &'c L1Computation) -> Result<(Vec<&'c ComputedResult>, Vec<L1Note>), L1CouldNotCheck> {
    let pick = |v: ComputedVariant| c.results.iter().filter(move |r| r.variant == v);
    let mut notes = Vec::new();
    let chosen: Vec<&ComputedResult> = match (facts.method, facts.variant) {
        (L1MethodId::IndependentT, Variant::T(TVariant::Student)) => pick(ComputedVariant::StudentPooled).collect(),
        (L1MethodId::IndependentT, Variant::T(TVariant::Welch)) => pick(ComputedVariant::Welch).collect(),
        (L1MethodId::Spearman, Variant::SpearmanP(SpearmanPVariant::Exact)) => {
            return Err(L1CouldNotCheck::VariantNotAvailable(VariantGap::SpearmanExactP))
        }
        (L1MethodId::ChiSquareIndependence, Variant::Continuity(ContinuityVariant::No)) => {
            pick(ComputedVariant::ChiSquareUncorrected).collect()
        }
        (L1MethodId::ChiSquareIndependence, Variant::Continuity(ContinuityVariant::Yes)) => {
            let yates: Vec<_> = pick(ComputedVariant::ChiSquareYatesR).collect();
            if yates.is_empty() {
                notes.push(L1Note::ContinuityDoesNotApply);
                pick(ComputedVariant::ChiSquareUncorrected).collect()
            } else {
                yates
            }
        }
        (L1MethodId::Ols, _) => match facts.target {
            Some(OlsTarget::OverallF) => pick(ComputedVariant::OlsOverallF).collect(),
            Some(OlsTarget::Coefficient { predictor }) => {
                pick(ComputedVariant::OlsCoefficient(OlsTerm::Predictor(predictor))).collect()
            }
            None => return Err(L1CouldNotCheck::BindingInvalid(BindingGap::OlsTargetDoesNotFitFamily)),
        },
        _ => c.results.iter().collect(),
    };
    if chosen.is_empty() {
        return Err(L1CouldNotCheck::StatisticNotCheckable(StatisticGap::NoComputedResult));
    }
    Ok((chosen, notes))
}

/// The family a computed variant's statistic is printed as, and its df count.
fn family_of(v: ComputedVariant) -> (Family, usize) {
    match v {
        ComputedVariant::StudentPooled | ComputedVariant::Welch | ComputedVariant::Paired | ComputedVariant::OlsCoefficient(_) => {
            (Family::T, 1)
        }
        ComputedVariant::OneWay | ComputedVariant::OlsOverallF => (Family::F, 2),
        ComputedVariant::Pearson | ComputedVariant::SpearmanTApproximation => (Family::R, 1),
        ComputedVariant::ChiSquareUncorrected | ComputedVariant::ChiSquareYatesR => (Family::ChiSquare, 1),
    }
}

/// Whether a variant's df is a whole number by construction.
fn integer_df(v: ComputedVariant) -> bool {
    v != ComputedVariant::Welch
}

/// `[lo, hi]` widened outward by `gate` times each end's size, plus the
/// margin `ReportedNumber::overlaps` allows for the rational-to-double step.
fn widened(lo: f64, hi: f64, gate: f64) -> (f64, f64) {
    let m = |v: f64| v.abs() * (gate + 4.0 * f64::EPSILON) + f64::MIN_POSITIVE;
    (lo - m(lo), hi + m(hi))
}

/// The printed number's interval as doubles, by its relation, and in
/// magnitude when asked (an interval straddling zero starts at zero).
fn printed_range(n: &ReportedNumber, magnitude: bool, floor: f64, ceiling: f64) -> (f64, f64) {
    let iv = n.interval();
    let (mut lo, mut hi) = (iv.lo.to_f64(), iv.hi.to_f64());
    if magnitude {
        let (a, b) = (lo.abs(), hi.abs());
        (lo, hi) = if lo <= 0.0 && hi >= 0.0 { (0.0, a.max(b)) } else { (a.min(b), a.max(b)) };
    }
    match n.relation() {
        Relation::Eq => (lo, hi),
        Relation::Lt | Relation::Le => (floor, hi),
        Relation::Gt | Relation::Ge => (lo, ceiling),
    }
}

/// A printed equality whose digits are `value` truncated toward zero at the
/// printed decimals (Step 8b, rule 2): `|printed| <= |value| < |printed| +
/// one unit in its last place`, the lower end widened by the gate. Signs
/// must agree unless the comparison is in magnitude.
pub fn truncated_match(printed: &ReportedNumber, value: f64, magnitude: bool, gate: f64) -> bool {
    if printed.relation() != Relation::Eq {
        return false;
    }
    let pv = printed.value().to_f64();
    if !magnitude && pv != 0.0 && value != 0.0 && pv.signum() != value.signum() {
        return false;
    }
    // In units of the last printed place: the print is the integer `k`, and
    // the value truncates to it when `k <= scaled < k + 1` (the lower end
    // widened by the gate).
    let scale = 10f64.powi(printed.decimals() as i32);
    let k = (pv.abs() * scale).round();
    let scaled = value.abs() * scale;
    scaled >= k - (k * (gate + 4.0 * f64::EPSILON) + f64::MIN_POSITIVE) && scaled < k + 1.0
}

/// Two closed intervals meet.
fn meets(a: (f64, f64), b: (f64, f64)) -> bool {
    a.0 <= b.1 && b.0 <= a.1
}

/// A signed range as magnitudes.
fn magnitude_range((lo, hi): (f64, f64)) -> (f64, f64) {
    if lo <= 0.0 && hi >= 0.0 {
        (0.0, lo.abs().max(hi.abs()))
    } else {
        (lo.abs().min(hi.abs()), lo.abs().max(hi.abs()))
    }
}

/// A printed equality that is `value` rounded twice: Level 0's
/// [`double_rounded_reach`] (the exact value rounded half up first to 3
/// decimals, or to one more decimal than printed, then to the printed
/// decimals), the lower end widened by the gate. Signs must agree unless the
/// comparison is in magnitude (part C, B-2).
pub fn double_rounded_match(printed: &ReportedNumber, value: f64, magnitude: bool, gate: f64) -> bool {
    let Some((lo, hi)) = double_rounded_reach(printed) else { return false };
    let pv = printed.value().to_f64();
    if !magnitude && value != 0.0 && pv.signum() != value.signum() {
        return false;
    }
    let v = value.abs();
    v >= lo - (lo * (gate + 4.0 * f64::EPSILON) + f64::MIN_POSITIVE) && v < hi
}

/// Whether a computed variant's statistic is compared without its sign. Only
/// a t whose sign depends on the order of two groups or two measurements:
/// an independent t (group order), a paired t (which column is First). An
/// OLS coefficient's t keeps its sign, because the sign is the finding
/// (evaluation fixes, part C, M-1); r and rho keep theirs; F and chi-square
/// have none.
pub fn in_magnitude(variant: ComputedVariant) -> bool {
    matches!(variant, ComputedVariant::StudentPooled | ComputedVariant::Welch | ComputedVariant::Paired)
}

/// The statistic: inside the printed interval widened by the gate. In
/// magnitude when `magnitude` ([`in_magnitude`]).
pub fn compare_statistic(printed: &ReportedNumber, computed: f64, magnitude: bool, gate: f64) -> NumberComparison {
    let (lo, hi) = printed_range(printed, magnitude, f64::NEG_INFINITY, f64::INFINITY);
    let accepted = widened(lo, hi, gate);
    let value = if magnitude { computed.abs() } else { computed };
    let inside = value >= accepted.0 && value <= accepted.1;
    let status = match (inside, magnitude) {
        (true, true) => NumberStatus::Matched(Some(MatchNote::InMagnitude)),
        (true, false) => NumberStatus::Matched(None),
        (false, true) if truncated_match(printed, computed, true, gate) => NumberStatus::Matched(Some(MatchNote::TruncatedInMagnitude)),
        (false, false) if truncated_match(printed, computed, false, gate) => NumberStatus::Matched(Some(MatchNote::Truncated)),
        (false, true) if double_rounded_match(printed, computed, true, gate) => NumberStatus::Matched(Some(MatchNote::DoubleRoundedInMagnitude)),
        (false, false) if double_rounded_match(printed, computed, false, gate) => NumberStatus::Matched(Some(MatchNote::DoubleRounded)),
        (false, _) => NumberStatus::NotMatched,
    };
    NumberComparison { kind: NumberKind::Statistic, printed: Printed::Number(printed.clone()), computed, accepted, status, precision: None }
}

/// Whether a printed df is a whole number: its value, not its digits (`45.0`
/// is 45).
fn is_whole(n: &ReportedNumber) -> bool {
    n.value().is_integer()
}

/// One df. `integer` is the candidate variant's df type.
pub fn compare_df(printed: &ReportedNumber, index: usize, computed: f64, integer: bool, gate: f64) -> NumberComparison {
    let kind = NumberKind::Df { index };
    let row = |accepted: (f64, f64), status| NumberComparison {
        kind,
        printed: Printed::Number(printed.clone()),
        computed,
        accepted,
        status,
        precision: None,
    };
    let value = printed.value().to_f64();
    if integer {
        // Equality; a printed fraction can never equal an integer df.
        let status = if is_whole(printed) && computed == value { NumberStatus::Matched(None) } else { NumberStatus::NotMatched };
        return row((value, value), status);
    }
    if is_whole(printed) && printed.decimals() == 0 {
        // Welch printed as a whole number: rounded or truncated.
        let (rounded, truncated) = (computed.round(), computed.trunc());
        let status = if value == rounded {
            NumberStatus::Matched(Some(MatchNote::DfRounded))
        } else if value == truncated {
            NumberStatus::Matched(Some(MatchNote::DfTruncated))
        } else if double_rounded_match(printed, computed, false, gate) {
            NumberStatus::Matched(Some(MatchNote::DoubleRounded))
        } else {
            NumberStatus::NotMatched
        };
        return row((truncated, rounded.max(truncated)), status);
    }
    let (lo, hi) = printed_range(printed, false, f64::NEG_INFINITY, f64::INFINITY);
    let accepted = widened(lo, hi, gate);
    let status = if computed >= accepted.0 && computed <= accepted.1 {
        NumberStatus::Matched(None)
    } else if truncated_match(printed, computed, false, gate) {
        NumberStatus::Matched(Some(MatchNote::Truncated))
    } else if double_rounded_match(printed, computed, false, gate) {
        NumberStatus::Matched(Some(MatchNote::DoubleRounded))
    } else {
        NumberStatus::NotMatched
    };
    row(accepted, status)
}

/// Level 0's p outcome on the claim's printed numbers, with a not-checked
/// reason when it is a Concern or a CouldNotCheck.
fn level0_p_gap(result: &ReportedResult, ctx: &DocumentContext) -> Option<(NotCheckedReason, L0Verdict)> {
    let verdict = PConsistency.run(&L0Input { result, context: ctx }).outcome;
    match verdict {
        L0Verdict::Concern(r) => Some((NotCheckedReason::PConcernAtLevel0(r), verdict)),
        L0Verdict::CouldNotCheck(r) => Some((NotCheckedReason::PCouldNotCheckAtLevel0(r), verdict)),
        L0Verdict::Consistent(_) | L0Verdict::Inconsistent { .. } => None,
    }
}

/// The p, by Phase 1's [`compare_p`] with the exact p as a zero-width range.
/// Returns the row and Phase 1's verdict.
pub fn compare_p_exact(
    result: &ReportedResult,
    ctx: &DocumentContext,
    computed: &ComputedResult,
) -> (NumberComparison, L0Verdict) {
    let mut trail = Vec::new();
    let alpha = alpha_of(ctx, &mut trail);
    let input = PComparison { p: &result.p, family: result.family, variance_ratio_candidate: is_variance_ratio_candidate(result) };
    let two_tailed = || match (computed.statistic, computed.df2) {
        (compute::StatisticKind::F, Some(df2)) => pvalue::f_two_tailed(computed.value, computed.df1, df2).map(|p| (p, p)),
        _ => None,
    };
    let verdict = compare_p(&input, computed.p, computed.p, two_tailed, ctx, &alpha, &mut trail);
    let status = match verdict {
        L0Verdict::Consistent(note) => NumberStatus::Matched(note.map(MatchNote::P)),
        L0Verdict::Inconsistent { .. } => NumberStatus::NotMatched,
        L0Verdict::Concern(ConcernReason::OneTailedUnstated) => NumberStatus::NotChecked(NotCheckedReason::OneTailedUnstatedOnExactP),
        L0Verdict::Concern(r) => NumberStatus::NotChecked(NotCheckedReason::PConcern(r)),
        L0Verdict::CouldNotCheck(r) => NumberStatus::NotChecked(NotCheckedReason::PCouldNotCheck(r)),
    };
    (p_row(result, computed.p, status), verdict)
}

/// Phase 1's comparison of the printed p with a RANGE of p, `[lo, hi]`: the
/// p range the stored cells' rounding allows (Step 8b).
fn compare_p_range(result: &ReportedResult, ctx: &DocumentContext, computed: &ComputedResult, (lo, hi): (f64, f64)) -> L0Verdict {
    let mut trail = Vec::new();
    let alpha = alpha_of(ctx, &mut trail);
    let input = PComparison { p: &result.p, family: result.family, variance_ratio_candidate: is_variance_ratio_candidate(result) };
    let two_tailed = || match (computed.statistic, computed.df2, computed.precision) {
        (compute::StatisticKind::F, Some(df2), Precision::Range { statistic: (a, b), .. }) => {
            let ends = [a.max(0.0), b.max(0.0)].map(|f| pvalue::f_two_tailed(f, computed.df1, df2));
            let (Some(x), Some(y)) = (ends[0], ends[1]) else { return None };
            // The two-tailed F p peaks at 1 where F crosses 1.
            let top = if a <= 1.0 && b >= 1.0 { 1.0 } else { x.max(y) };
            Some((x.min(y), top))
        }
        _ => None,
    };
    compare_p(&input, lo, hi, two_tailed, ctx, &alpha, &mut trail)
}

/// Rule 1 on one row still `NotMatched`: `DataPrecision` when `accepted`
/// (the printed side, as compared) meets `range` (the computed side, as
/// compared). The range is attached for the view either way.
fn with_precision(mut row: NumberComparison, signed: Option<(f64, f64)>, compared: impl Fn((f64, f64)) -> bool, decimals: u32) -> NumberComparison {
    if let Some(r) = signed {
        row.precision = Some(r);
        if row.status == NumberStatus::NotMatched && compared(r) {
            row.status = NumberStatus::NotChecked(NotCheckedReason::DataPrecision { decimals });
        }
    }
    row
}

fn p_row(result: &ReportedResult, computed: f64, status: NumberStatus) -> NumberComparison {
    let (printed, accepted) = match &result.p {
        PValue::Reported(p) => (Printed::Number(p.clone()), printed_range(p, false, 0.0, 1.0)),
        PValue::NotSignificant => (Printed::NotSignificant, (0.0, 1.0)),
    };
    NumberComparison { kind: NumberKind::P, printed, computed, accepted, status, precision: None }
}

/// n, printed or implied by r's df, against the n used.
fn compare_n(result: &ReportedResult, n_used: usize) -> Option<NumberComparison> {
    let (printed, source, implied) = match (&result.n, result.family, result.df.first()) {
        (Some(n), _, _) => (n, NSource::Stated, n.value()),
        (None, Family::R, Some(df)) => (df, NSource::ImpliedByDf, df.value().add(Rational::from_int(2))?),
        _ => return None,
    };
    let value = implied.to_f64();
    let status = if implied.denom() == 1 && value == n_used as f64 { NumberStatus::Matched(None) } else { NumberStatus::NotMatched };
    Some(NumberComparison {
        kind: NumberKind::N { source },
        printed: Printed::Number(printed.clone()),
        computed: n_used as f64,
        accepted: (value, value),
        status,
        precision: None,
    })
}

/// Combine every candidate's `conclusion_changes`.
fn combined_conclusion(cands: &[CandidateComparison], alpha: &Alpha) -> (Option<bool>, Option<ConclusionGap>) {
    if cands.iter().any(|c| c.numbers.iter().any(|n| n.kind == NumberKind::P && matches!(n.status, NumberStatus::NotChecked(_)))) {
        return (None, Some(ConclusionGap::PNotChecked));
    }
    if matches!(alpha, Alpha::Several(_)) {
        return (None, Some(ConclusionGap::SeveralAlphas));
    }
    let values: Vec<Option<bool>> = cands.iter().map(|c| c.conclusion_changes).collect();
    if values.iter().any(Option::is_none) {
        return (None, Some(ConclusionGap::Unplaced));
    }
    if values.iter().all(|v| *v == Some(true)) {
        (Some(true), None)
    } else if values.iter().all(|v| *v == Some(false)) {
        (Some(false), None)
    } else {
        (None, Some(ConclusionGap::CandidatesDisagree))
    }
}

/// The notes a matched candidate carries.
fn notes_of(c: &CandidateComparison) -> Vec<L1Note> {
    let mut notes = Vec::new();
    for n in &c.numbers {
        match (n.kind, n.status) {
            (NumberKind::Statistic, NumberStatus::Matched(Some(MatchNote::InMagnitude))) => notes.push(L1Note::SignComparedInMagnitude),
            (NumberKind::Df { index }, NumberStatus::Matched(Some(MatchNote::DfTruncated))) => notes.push(L1Note::DfTruncated { index }),
            (NumberKind::Df { index }, NumberStatus::Matched(Some(MatchNote::DfRounded))) => notes.push(L1Note::DfRounded { index }),
            (_, NumberStatus::Matched(Some(MatchNote::P(note)))) => notes.push(L1Note::P(note)),
            (number, NumberStatus::Matched(Some(MatchNote::Truncated))) => notes.push(L1Note::PrintedTruncated { number }),
            (number, NumberStatus::Matched(Some(MatchNote::TruncatedInMagnitude))) => {
                notes.push(L1Note::SignComparedInMagnitude);
                notes.push(L1Note::PrintedTruncated { number });
            }
            (number, NumberStatus::Matched(Some(MatchNote::DoubleRounded))) => notes.push(L1Note::PrintedDoubleRounded { number }),
            (number, NumberStatus::Matched(Some(MatchNote::DoubleRoundedInMagnitude))) => {
                notes.push(L1Note::SignComparedInMagnitude);
                notes.push(L1Note::PrintedDoubleRounded { number });
            }
            (number, NumberStatus::NotChecked(why)) => notes.push(L1Note::NotChecked { number, why }),
            _ => {}
        }
        if c.variant == ComputedVariant::SpearmanTApproximation && n.kind == NumberKind::P && matches!(n.status, NumberStatus::Matched(_)) {
            notes.push(L1Note::SpearmanPByTApproximation);
        }
    }
    notes
}

/// The verdict from the candidates' comparisons: reproduced when one
/// candidate matches, not reproduced when every candidate fails, could not
/// check otherwise (no statistic checked).
pub fn decide(cands: &[CandidateComparison], alpha: &Alpha) -> (L1Verdict, Vec<L1Note>) {
    let matched: Vec<&CandidateComparison> = cands.iter().filter(|c| c.matches()).collect();
    if let Some(first) = matched.first() {
        let mut notes: Vec<L1Note> = matched.iter().map(|c| L1Note::VariantMatched(c.variant)).collect();
        notes.extend(notes_of(first));
        return (L1Verdict::Reproduced(notes), Vec::new());
    }
    if !cands.is_empty() && cands.iter().all(CandidateComparison::fails) {
        let (conclusion_changes, gap) = combined_conclusion(cands, alpha);
        let notes = gap.map(L1Note::ConclusionNotAssessed).into_iter().collect();
        return (L1Verdict::NotReproduced { conclusion_changes }, notes);
    }
    // Rule 1: a candidate that does not fail, whose statistic only the
    // stored cells' rounding stands between.
    let precision = cands.iter().filter(|c| !c.fails()).find_map(|c| {
        c.numbers.iter().find_map(|n| match (n.kind, n.status) {
            (NumberKind::Statistic, NumberStatus::NotChecked(NotCheckedReason::DataPrecision { decimals })) => Some(decimals),
            _ => None,
        })
    });
    if let Some(decimals) = precision {
        return (L1Verdict::CouldNotCheck(L1CouldNotCheck::DataPrecision { decimals }), Vec::new());
    }
    (L1Verdict::CouldNotCheck(L1CouldNotCheck::StatisticNotCheckable(StatisticGap::NoComputedResult)), Vec::new())
}

/// Compare one claim with one computation. Pure: no state, no I/O.
pub fn compare_claim(
    result: &ReportedResult,
    ctx: &DocumentContext,
    facts: &BindingFacts,
    computation: &L1Computation,
) -> Result<(Vec<CandidateComparison>, Vec<L1Note>), L1CouldNotCheck> {
    let (chosen, notes) = candidates(facts, computation)?;
    let gate = L1Registry::get(facts.method).ok_or(L1CouldNotCheck::MethodNotAvailable)?.relative_gate;

    // The binding must be able to produce the printed statistic.
    for c in &chosen {
        let (family, df_count) = family_of(c.variant);
        if family != result.family {
            let gap = if facts.method == L1MethodId::Ols {
                BindingGap::OlsTargetDoesNotFitFamily
            } else {
                BindingGap::FamilyDoesNotFitMethod
            };
            return Err(L1CouldNotCheck::BindingInvalid(gap));
        }
        if !result.df.is_empty() && result.df.len() != df_count {
            return Err(L1CouldNotCheck::BindingInvalid(BindingGap::DfCountDoesNotFit { printed: result.df.len(), expected: df_count }));
        }
    }
    let v = result.statistic.value();
    let out_of_range = match result.family {
        Family::R => v.numer().abs() >= v.denom(),
        Family::F | Family::ChiSquare => v.is_negative(),
        Family::T | Family::Z => false,
    };
    if out_of_range {
        return Err(L1CouldNotCheck::StatisticNotCheckable(StatisticGap::PrintedOutOfRange));
    }
    // A fractional printed df where no candidate has a fractional df: the
    // stated variant contradicts the printed df when the method HAS a
    // fractional-df variant (Welch's numbers under a stated Student); else a
    // correction Gaply does not compute.
    if chosen.iter().all(|c| integer_df(c.variant)) {
        if let Some(index) = result.df.iter().position(|d| !is_whole(d)) {
            let gap = if facts.method == L1MethodId::IndependentT && facts.variant == Variant::T(TVariant::Student) {
                VariantGap::DfContradictsStatedVariant { index }
            } else {
                VariantGap::FractionalDf { index }
            };
            return Err(L1CouldNotCheck::VariantNotAvailable(gap));
        }
    }

    let level0_gap = level0_p_gap(result, ctx);
    let mut trail = Vec::new();
    let alpha = alpha_of(ctx, &mut trail);

    let mut cands = Vec::with_capacity(chosen.len());
    for c in chosen {
        let magnitude = in_magnitude(c.variant);
        let mut stat = compare_statistic(&result.statistic, c.value, magnitude, gate);
        match c.precision {
            Precision::Range { decimals, statistic, .. } => {
                let accepted = stat.accepted;
                stat = with_precision(stat, Some(statistic), |r| meets(accepted, if magnitude { magnitude_range(r) } else { r }), decimals);
            }
            Precision::Unbounded { decimals } if stat.status == NumberStatus::NotMatched => {
                stat.status = NumberStatus::NotChecked(NotCheckedReason::DataPrecision { decimals });
            }
            _ => {}
        }
        let mut numbers = vec![stat];
        let computed_df = [Some(c.df1), c.df2];
        for (index, printed) in result.df.iter().enumerate() {
            let df = computed_df[index].expect("df count checked above");
            let mut row = compare_df(printed, index, df, integer_df(c.variant), gate);
            match c.precision {
                Precision::Range { decimals, df: Some(range), .. } => {
                    // What the printed df stands for: its interval, or for a
                    // whole number printed against Welch's df, the values that
                    // round or truncate to it.
                    let v = printed.value().to_f64();
                    let printed_side = if is_whole(printed) && printed.decimals() == 0 { (v - 0.5, v + 1.0) } else { row.accepted };
                    row = with_precision(row, Some(range), |r| meets(printed_side, r), decimals);
                }
                // A fractional df (Welch's) moves with the data's rounding;
                // when that range is unbounded, a miss is not decided, as the
                // statistic and p are (part C: 14 of the deep evaluation's
                // scenario-B Welch cases were false "Not reproduced" until
                // this). An integer df does not move and is still compared.
                Precision::Unbounded { decimals } if !integer_df(c.variant) && row.status == NumberStatus::NotMatched => {
                    row.status = NumberStatus::NotChecked(NotCheckedReason::DataPrecision { decimals });
                }
                _ => {}
            }
            numbers.push(row);
        }
        let (mut p, p_outcome) = match level0_gap {
            // Level 0's own verdict is what the record carries for this p.
            Some((why, level0)) => (p_row(result, c.p, NumberStatus::NotChecked(why)), Some(level0)),
            None => {
                let (row, verdict) = compare_p_exact(result, ctx, c);
                (row, Some(verdict))
            }
        };
        // Rule 2: a printed equality that is the exact p truncated.
        if p.status == NumberStatus::NotMatched {
            if let PValue::Reported(printed) = &result.p {
                if truncated_match(printed, c.p, false, 0.0) {
                    p.status = NumberStatus::Matched(Some(MatchNote::Truncated));
                }
            }
        }
        // Rule 1: the p range the stored cells allow, by Phase 1's rule.
        match c.precision {
            Precision::Range { decimals, p: range, .. } if level0_gap.is_none() => {
                p = with_precision(p, Some(range), |r| matches!(compare_p_range(result, ctx, c, r), L0Verdict::Consistent(_)), decimals);
            }
            Precision::Unbounded { decimals } if p.status == NumberStatus::NotMatched => {
                p.status = NumberStatus::NotChecked(NotCheckedReason::DataPrecision { decimals });
            }
            _ => {}
        }
        // Spearman with no variant stated: the p is the t approximation's.
        // A printed p that misses it may be R's exact p, which Level 1 does
        // not compute but can BOUND (evaluation fixes, part C, M-2): it is
        // not checked only when it meets the band that surely holds the exact
        // p; outside the band it stays not matched. With ties, R's cor.test
        // gives the t approximation itself, but other software gives an
        // exact or permutation p with ties, which no band here covers, so a
        // miss stays not checked. Above R's exact range (n > 1290) R gives
        // the t approximation, and the p is compared normally. When the
        // paper's words or the user's tick state the t approximation, the p
        // is compared normally (review follow-up, item 3); a stated exact p
        // is refused earlier, as a variant not available.
        if facts.method == L1MethodId::Spearman && !facts.variant.is_stated() && p.status == NumberStatus::NotMatched {
            if computation.ties != Some(false) {
                p.status = NumberStatus::NotChecked(NotCheckedReason::SpearmanPWithTies);
            } else if c.n <= SPEARMAN_EXACT_MAX_N {
                let band = spearman_exact_band(c.p);
                if matches!(compare_p_range(result, ctx, c, band), L0Verdict::Consistent(_) | L0Verdict::Concern(_)) {
                    p.status = NumberStatus::NotChecked(NotCheckedReason::SpearmanPMayBeExact);
                }
            }
        }
        let conclusion_changes = match p.status {
            NumberStatus::NotChecked(_) => None,
            _ => {
                let (lo, hi) = widen(c.p, c.p);
                conclusion_changes_of(&result.p, lo, hi, &alpha)
            }
        };
        numbers.push(p);
        numbers.extend(compare_n(result, c.n));
        let stored_decimals = match c.precision {
            Precision::Range { decimals, .. } | Precision::Unbounded { decimals } => Some(decimals),
            Precision::Exact => None,
        };
        cands.push(CandidateComparison { variant: c.variant, numbers, p_outcome, conclusion_changes, stored_decimals });
    }
    Ok((cands, notes))
}

/// The largest `n` for which R's `cor.test(method = "spearman")` gives its
/// exact p (`pspearman`, `n <= 1290`; R 4.6.1's `cor.test.default`).
pub const SPEARMAN_EXACT_MAX_N: usize = 1290;

/// The most R's exact Spearman p was found to EXCEED the t-approximation p,
/// as a ratio, over every n from 4 to 1290 without ties and a grid of rho,
/// for exact p from .001 to 1 (part C, M-2; `band.R`, D270): 6.116444 at
/// n = 7, rho = .964 (exact .002778, t .000454). Rounded up.
pub const SPEARMAN_EXACT_OVER_T: f64 = 6.1165;

/// The most the t-approximation p was found to exceed R's exact p: 1.004814
/// at n = 23, rho = .221. Rounded up.
pub const SPEARMAN_T_OVER_EXACT: f64 = 1.0049;

/// The margin on both measured ratios.
pub const SPEARMAN_BAND_MARGIN: f64 = 2.0;

/// The range R's exact Spearman p surely lies in, from the t-approximation
/// p: the measured ratios, each widened by [`SPEARMAN_BAND_MARGIN`].
pub fn spearman_exact_band(t_approximation_p: f64) -> (f64, f64) {
    let lo = t_approximation_p / (SPEARMAN_T_OVER_EXACT * SPEARMAN_BAND_MARGIN);
    let hi = (t_approximation_p * SPEARMAN_EXACT_OVER_T * SPEARMAN_BAND_MARGIN).min(1.0);
    (lo, hi)
}

/// The provenance record for a computed claim.
fn record(binding: &ConfirmedBinding, gate: f64, shown: Option<&CandidateComparison>, n_used: usize) -> L1Record {
    let method = binding.method();
    let canonical = format!(
        "{}\nversion={}\nfingerprint={}\nclaim_revision={}\nbinding={}\n",
        method.id(),
        method.version(),
        binding.fingerprint_digest(),
        binding.claim_revision().as_str(),
        binding.digest()
    );
    let mut computed: Vec<(&'static str, Computed)> = Vec::new();
    if let Some(c) = shown {
        for n in &c.numbers {
            let name = match n.kind {
                NumberKind::Statistic => "statistic",
                NumberKind::Df { index: 0 } => "df1",
                NumberKind::Df { .. } => "df2",
                NumberKind::P => "p",
                NumberKind::N { .. } => continue,
            };
            computed.push((name, Computed::Range { lo: n.computed, hi: n.computed }));
        }
    }
    computed.push(("n_used", Computed::Range { lo: n_used as f64, hi: n_used as f64 }));
    // Every computed candidate has a p outcome; a record with none shown says
    // so rather than borrowing a verdict.
    let outcome = shown.and_then(|c| c.p_outcome).unwrap_or(L0Verdict::CouldNotCheck(CouldNotCheckReason::NoMethodRan));
    L1Record {
        check: MethodCheck {
            method_id: method.id(),
            provenance: PROVENANCE,
            engine_version: ENGINE_VERSION,
            inputs_digest: digest_hex(&canonical),
            computed,
            tolerance: ToleranceRule::RelativeOutward { margin: gate, cap: f64::INFINITY },
            outcome,
        },
        method_version: method.version(),
        binding_source: BindingSourceKind::of(binding.source()),
        variant_set_by: binding.variant().set_by(),
    }
}

/// One claim from its binding and computation.
pub fn reproduce_claim(claim: &L1Claim, ctx: &DocumentContext, binding: &ConfirmedBinding, computation: &L1Computation) -> ClaimReproduction {
    let facts = BindingFacts::of(binding);
    let (candidates, choice_notes) = match compare_claim(&claim.result, ctx, &facts, computation) {
        Ok(x) => x,
        Err(why) => return ClaimReproduction::could_not_check(claim, why, Some(binding)),
    };
    let mut trail = Vec::new();
    let alpha = alpha_of(ctx, &mut trail);
    let (verdict, mut notes) = decide(&candidates, &alpha);
    let verdict = match verdict {
        L1Verdict::Reproduced(mut n) => {
            n.extend(choice_notes);
            L1Verdict::Reproduced(n)
        }
        other => {
            notes.extend(choice_notes);
            other
        }
    };
    let gate = L1Registry::get(facts.method).map(|m| m.relative_gate).unwrap_or(0.0);
    let mut out = ClaimReproduction {
        claim: claim.id,
        revision: claim.revision.clone(),
        location: claim.location,
        method: Some(facts.method),
        variant: Some(binding.variant()),
        verdict,
        notes,
        candidates,
        counts: Some(computation.counts.clone()),
        record: None,
        whole_numbers: binding.whole_numbers().to_vec(),
    };
    out.record = Some(record(binding, gate, out.shown(), computation.counts.n_used));
    out
}

/* ------------------------------------------------------------------------ *
 *  Every claim in a run
 * ------------------------------------------------------------------------ */

/// Whether, and how, the data file was read for this run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DataRead {
    /// No claim opened the gate, so the file was not opened.
    NotNeeded,
    Read,
    Unreadable(L1IngestRefusal),
}

/// Counts of verdicts. Counts only: no score, no percentage.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct L1VerdictCounts {
    pub reproduced: usize,
    pub not_reproduced: usize,
    pub could_not_check: usize,
}

/// The significance threshold a run compares p with: as the paper printed
/// it, and whether the paper stated it (otherwise `.05`, the default). The
/// screen says which, every time it mentions the threshold.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AlphaUsed {
    pub printed: Vec<String>,
    pub stated: bool,
}

/// Level 1 for every claim in a run.
#[derive(Debug, Clone, PartialEq)]
pub struct ReproductionOutcome {
    pub claims: Vec<ClaimReproduction>,
    pub counts: L1VerdictCounts,
    pub data: DataRead,
    /// Pairs whose revision names no live claim and no claim's previous one.
    pub unmatched_pairs: usize,
    /// Claims no pair names (part C, M-4): the user skipped them. They are
    /// left out of `claims` and of `counts`, never shown as could not check.
    pub skipped: usize,
    pub alpha: AlphaUsed,
}

/// The document context a run compares p with: the manuscript's, or the
/// defaults (alpha .05, nothing one-tailed, no correction) when the claims
/// were entered by hand.
fn context_of(state: &L1RunState) -> DocumentContext {
    state.context().cloned().unwrap_or(DocumentContext { thresholds: Vec::new(), one_tailed_at: None, correction_at: None })
}

/// Level 1 over every claim in `state`. `pairs` are the (claim revision,
/// token) pairs the user confirmed; `read` reads the data file again, and is
/// called at most once, only after at least one claim opened the gate.
pub fn reproduce(
    state: &L1RunState,
    pairs: &[(RevisionHash, ConfirmationToken)],
    read: impl FnOnce() -> Result<Dataset, L1IngestRefusal>,
) -> ReproductionOutcome {
    let ctx = context_of(state);
    let mut used = vec![false; pairs.len()];
    enum Plan<'a> {
        Done(L1CouldNotCheck),
        Compute(&'a ConfirmationToken, &'a ConfirmedBinding),
    }
    let mut plans: Vec<(&L1Claim, Plan)> = Vec::new();
    let mut skipped = 0;
    for claim in state.claims() {
        let at = |rev: &RevisionHash| pairs.iter().position(|(r, _)| r == rev);
        let previous = match &claim.origin {
            ClaimOrigin::UserEdited { from } => Some(from),
            _ => None,
        };
        let plan = match at(&claim.revision) {
            Some(i) => {
                used[i] = true;
                let token = &pairs[i].1;
                match state.gate(token, claim.id) {
                    Ok(binding) => Plan::Compute(token, binding),
                    Err(g) => Plan::Done(L1CouldNotCheck::TokenStale(g)),
                }
            }
            None => match previous.and_then(at) {
                // Confirmed on the claim before it was edited.
                Some(i) => {
                    used[i] = true;
                    let rejection = state.gate(&pairs[i].1, claim.id).err().unwrap_or(GateRejection::Stale(super::run_state::StaleField::ClaimRevision));
                    Plan::Done(L1CouldNotCheck::TokenStale(rejection))
                }
                // No pair names the claim or its previous revision: the user
                // skipped it (part C, M-4). Counted, never a result.
                None => {
                    skipped += 1;
                    continue;
                }
            },
        };
        plans.push((claim, plan));
    }

    // Read at the first claim that opened the gate, and only then: a run in
    // which no token is live never opens the file.
    let mut read = Some(read);
    let mut dataset: Option<Result<Dataset, L1IngestRefusal>> = None;
    let mut counts = L1VerdictCounts::default();
    let claims: Vec<ClaimReproduction> = plans
        .into_iter()
        .map(|(claim, plan)| {
            let out = match plan {
                Plan::Done(why) => ClaimReproduction::could_not_check(claim, why, None),
                Plan::Compute(token, binding) => {
                    let d = dataset.get_or_insert_with(|| read.take().map_or(Err(L1IngestRefusal::NotReadable), |f| f()));
                    match d {
                        Err(e) => ClaimReproduction::could_not_check(claim, L1CouldNotCheck::DataUnreadable(*e), Some(binding)),
                        Ok(d) => match compute::compute(state, token, claim.id, d) {
                            Ok(c) => reproduce_claim(claim, &ctx, binding, &c),
                            Err(L1Refusal::Gate(g)) => {
                                ClaimReproduction::could_not_check(claim, L1CouldNotCheck::TokenStale(g), Some(binding))
                            }
                            Err(L1Refusal::MethodNotAvailable) => {
                                ClaimReproduction::could_not_check(claim, L1CouldNotCheck::MethodNotAvailable, Some(binding))
                            }
                            Err(r) => ClaimReproduction::could_not_check(claim, L1CouldNotCheck::Refused(r), Some(binding)),
                        },
                    }
                }
            };
            match out.verdict {
                L1Verdict::Reproduced(_) => counts.reproduced += 1,
                L1Verdict::NotReproduced { .. } => counts.not_reproduced += 1,
                L1Verdict::CouldNotCheck(_) => counts.could_not_check += 1,
            }
            out
        })
        .collect();
    tracing::debug!(
        claims = claims.len(),
        reproduced = counts.reproduced,
        not_reproduced = counts.not_reproduced,
        could_not_check = counts.could_not_check,
        "stats_l1: reproduction"
    );
    let data = match &dataset {
        None => DataRead::NotNeeded,
        Some(Ok(_)) => DataRead::Read,
        Some(Err(e)) => DataRead::Unreadable(*e),
    };
    let alpha = AlphaUsed { printed: ctx.alpha_printed(), stated: !ctx.alpha().1 };
    ReproductionOutcome { claims, counts, data, unmatched_pairs: used.iter().filter(|u| !**u).count(), skipped, alpha }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::extract::SectionKind;
    use crate::stats_l0::method::TextPosition;
    use crate::stats_l1::claim::{UserClaim, UserP};
    use crate::stats_l1::compute::{StatisticKind, Tail};

    fn ctx() -> DocumentContext {
        DocumentContext { thresholds: Vec::new(), one_tailed_at: None, correction_at: None }
    }

    fn loc() -> ClaimLocation {
        ClaimLocation { section_index: 0, section: SectionKind::Results, paragraph: 0, sentence: 0 }
    }

    /// A claim as a user would type it.
    fn claim(family: Family, statistic: &str, df: &[&str], n: Option<&str>, p: UserP<'_>) -> L1Claim {
        let u = UserClaim { family, statistic, df: df.to_vec(), n, p, location: loc(), sentence: None };
        L1Claim::user_entered(ClaimId(0), &u, ClaimOrigin::UserAdded).expect("numbers read")
    }

    fn p_eq(raw: &str) -> UserP<'_> {
        UserP::Reported { raw, relation: Relation::Eq }
    }

    fn t_res(variant: ComputedVariant, t: f64, df: f64, p: f64, n: usize) -> ComputedResult {
        ComputedResult { variant, statistic: StatisticKind::T, value: t, df1: df, df2: None, p, tail: Tail::TwoSided, n, precision: Precision::Exact }
    }

    fn t_exact(variant: ComputedVariant, t: f64, df: f64, n: usize) -> ComputedResult {
        t_res(variant, t, df, pvalue::t_two_tailed(t, df).unwrap(), n)
    }

    fn comp(method: L1MethodId, results: Vec<ComputedResult>, n: usize, ties: Option<bool>) -> L1Computation {
        L1Computation {
            claim: ClaimId(0),
            method,
            counts: InputCounts { rows_read: n, excluded: Vec::new(), n_used: n },
            results,
            ties,
            engine_version: ENGINE_VERSION,
        }
    }

    fn facts(method: L1MethodId, variant: Variant) -> BindingFacts {
        BindingFacts { method, variant, target: None }
    }

    fn not_stated_t() -> BindingFacts {
        facts(L1MethodId::IndependentT, Variant::T(TVariant::NotStated))
    }

    /// The verdict and notes, as `reproduce_claim` decides them.
    fn verdict(c: &L1Claim, ctx: &DocumentContext, f: &BindingFacts, comp: &L1Computation) -> (L1Verdict, Vec<L1Note>, Vec<CandidateComparison>) {
        match compare_claim(&c.result, ctx, f, comp) {
            Err(why) => (L1Verdict::CouldNotCheck(why), Vec::new(), Vec::new()),
            Ok((cands, _)) => {
                let mut trail = Vec::new();
                let alpha = alpha_of(ctx, &mut trail);
                let (v, n) = decide(&cands, &alpha);
                (v, n, cands)
            }
        }
    }

    fn status_of(cands: &[CandidateComparison], variant: ComputedVariant, kind: NumberKind) -> NumberStatus {
        cands.iter().find(|c| c.variant == variant).unwrap().numbers.iter().find(|n| n.kind == kind).unwrap().status
    }

    const GATE: f64 = 1e-12;

    /* ---- the statistic --------------------------------------------------- */

    #[test]
    fn a_statistic_on_each_side_of_each_rounding_boundary() {
        // 2.10 prints [2.095, 2.105] by rounding, and [2.10, 2.11) by
        // truncation (Step 8b, rule 2): just above the rounding interval is a
        // truncated print, matched with its note; at 2.11 it is neither. Just
        // below, down to 2.0945, it is a double-rounded print (2.0949 shown
        // as 2.095, printed 2.10; part C, B-2), matched with its note.
        let p = ReportedNumber::measured("2.10").unwrap();
        let truncated = NumberStatus::Matched(Some(MatchNote::Truncated));
        let double = NumberStatus::Matched(Some(MatchNote::DoubleRounded));
        for (x, want) in [
            (2.105, NumberStatus::Matched(None)),
            (2.105_000_01, truncated),
            (2.109_999, truncated),
            (2.11, NumberStatus::NotMatched),
            (2.095, NumberStatus::Matched(None)),
            (2.094_999_99, double),
            (2.094_5, double),
            (2.094_499_99, NumberStatus::NotMatched),
            (2.1, NumberStatus::Matched(None)),
        ] {
            assert_eq!(compare_statistic(&p, x, false, GATE).status, want, "{x}");
        }
        // A negative printed r: [-0.455, -0.445]; truncated toward zero it
        // covers (-0.46, -0.45]; double-rounded, (-0.445, -0.4445].
        let r = ReportedNumber::measured("-.45").unwrap();
        for (x, want) in [
            (-0.455, NumberStatus::Matched(None)),
            (-0.455_000_01, truncated),
            (-0.46, NumberStatus::NotMatched),
            (-0.445, NumberStatus::Matched(None)),
            (-0.444_999_99, double),
            (-0.444_499_99, NumberStatus::NotMatched),
            (0.444_999_99, NumberStatus::NotMatched),
        ] {
            assert_eq!(compare_statistic(&r, x, false, GATE).status, want, "{x}");
        }
    }

    /// Step 8b, rule 2, on the case section 4 found (PLOS ONE 0256959,
    /// claim 7): F = 22.0455 printed 22.04 is the exact value truncated.
    fn f_verdict(statistic: &str, p: UserP<'_>, exact_p: f64) -> (L1Verdict, Vec<CandidateComparison>) {
        let c = claim(Family::F, statistic, &["2", "183"], None, p);
        let f = ComputedResult {
            variant: ComputedVariant::OneWay,
            statistic: StatisticKind::F,
            value: 22.0455,
            df1: 2.0,
            df2: Some(183.0),
            p: exact_p,
            tail: Tail::Upper,
            n: 186,
            precision: Precision::Exact,
        };
        let (v, _, cands) = verdict(&c, &ctx(), &facts(L1MethodId::OneWayAnova, Variant::None), &comp(L1MethodId::OneWayAnova, vec![f], 186, None));
        (v, cands)
    }

    #[test]
    fn a_statistic_printed_truncated_matches_with_its_note_rounded_without_and_otherwise_not() {
        let lt = UserP::Reported { raw: ".001", relation: Relation::Lt };
        let exact = pvalue::f_upper(22.0455, 2.0, 183.0).unwrap();
        let (v, cands) = f_verdict("22.04", lt, exact);
        assert_eq!(status_of(&cands, ComputedVariant::OneWay, NumberKind::Statistic), NumberStatus::Matched(Some(MatchNote::Truncated)));
        let L1Verdict::Reproduced(notes) = v else { panic!("{v:?}") };
        assert!(notes.contains(&L1Note::PrintedTruncated { number: NumberKind::Statistic }), "{notes:?}");

        let (v, _) = f_verdict("22.05", lt, exact);
        let L1Verdict::Reproduced(notes) = v else { panic!("{v:?}") };
        assert!(!notes.iter().any(|n| matches!(n, L1Note::PrintedTruncated { .. })), "rounded, no note: {notes:?}");

        let (v, _) = f_verdict("22.03", lt, exact);
        assert!(matches!(v, L1Verdict::NotReproduced { .. }), "{v:?}");
    }

    #[test]
    fn a_p_printed_truncated_matches_with_its_note_and_an_inequality_is_unchanged() {
        // 0256959, claim 15: the exact p .0253 printed "p = .02".
        let (v, cands) = f_verdict("22.05", p_eq(".02"), 0.0253);
        assert_eq!(status_of(&cands, ComputedVariant::OneWay, NumberKind::P), NumberStatus::Matched(Some(MatchNote::Truncated)));
        let L1Verdict::Reproduced(notes) = v else { panic!("{v:?}") };
        assert!(notes.contains(&L1Note::PrintedTruncated { number: NumberKind::P }), "{notes:?}");
        // "p < .03" matched as before, with no truncation note.
        let (_, cands) = f_verdict("22.05", UserP::Reported { raw: ".03", relation: Relation::Lt }, 0.0253);
        assert_eq!(status_of(&cands, ComputedVariant::OneWay, NumberKind::P), NumberStatus::Matched(None));
        // "p < .02" is not an equality: the rule does not touch it, and it is
        // not matched (Phase 1 leaves it to its own F rule).
        let (_, cands) = f_verdict("22.05", UserP::Reported { raw: ".02", relation: Relation::Lt }, 0.0253);
        let got = status_of(&cands, ComputedVariant::OneWay, NumberKind::P);
        assert!(!matches!(got, NumberStatus::Matched(_)), "{got:?}");
    }

    /// What each computed variant compares in magnitude (part C, M-1): only
    /// a t whose sign follows the order of two groups or two measurements.
    #[test]
    fn t_compares_in_magnitude_with_a_note_and_r_with_its_sign() {
        for v in [ComputedVariant::StudentPooled, ComputedVariant::Welch, ComputedVariant::Paired] {
            assert!(in_magnitude(v), "{v:?}");
        }
        for v in [
            ComputedVariant::OlsCoefficient(OlsTerm::Intercept),
            ComputedVariant::OlsCoefficient(OlsTerm::Predictor(crate::stats_l1::dataset::ColumnAlias(2))),
            ComputedVariant::Pearson,
            ComputedVariant::SpearmanTApproximation,
            ComputedVariant::OneWay,
            ComputedVariant::OlsOverallF,
            ComputedVariant::ChiSquareUncorrected,
            ComputedVariant::ChiSquareYatesR,
        ] {
            assert!(!in_magnitude(v), "{v:?}");
        }
        let t = ReportedNumber::measured("2.10").unwrap();
        assert_eq!(compare_statistic(&t, -2.1, true, GATE).status, NumberStatus::Matched(Some(MatchNote::InMagnitude)));
        let neg = ReportedNumber::measured("-2.10").unwrap();
        assert_eq!(compare_statistic(&neg, 2.1, true, GATE).status, NumberStatus::Matched(Some(MatchNote::InMagnitude)));
        let r = ReportedNumber::measured(".45").unwrap();
        assert_eq!(compare_statistic(&r, -0.45, false, GATE).status, NumberStatus::NotMatched);
        assert_eq!(compare_statistic(&r, 0.45, false, GATE).status, NumberStatus::Matched(None));
    }

    /* ---- df ---------------------------------------------------------------- */

    #[test]
    fn welchs_df_of_27_63_printed_four_ways() {
        let df = |raw: &str| {
            let n = if raw.contains('.') { ReportedNumber::measured(raw) } else { ReportedNumber::count(raw) }.unwrap();
            compare_df(&n, 0, 27.63, false, GATE).status
        };
        assert_eq!(df("27"), NumberStatus::Matched(Some(MatchNote::DfTruncated)));
        assert_eq!(df("28"), NumberStatus::Matched(Some(MatchNote::DfRounded)));
        assert_eq!(df("27.6"), NumberStatus::Matched(None));
        assert_eq!(df("26"), NumberStatus::NotMatched);
        assert_eq!(df("27.7"), NumberStatus::NotMatched);
    }

    #[test]
    fn a_decimal_df_and_a_printed_p_on_each_side_of_each_rounding_boundary() {
        // Welch's df printed 27.6: [27.55, 27.65], widened by the gate.
        // Truncated, 27.6 covers [27.6, 27.7) (Step 8b, rule 2); double-
        // rounded, [27.545, 27.55) (part C, B-2).
        let df = ReportedNumber::measured("27.6").unwrap();
        for (x, want) in [
            (27.65, NumberStatus::Matched(None)),
            (27.650_000_1, NumberStatus::Matched(Some(MatchNote::Truncated))),
            (27.7, NumberStatus::NotMatched),
            (27.55, NumberStatus::Matched(None)),
            (27.549_999_9, NumberStatus::Matched(Some(MatchNote::DoubleRounded))),
            (27.544_999_9, NumberStatus::NotMatched),
        ] {
            assert_eq!(compare_df(&df, 0, x, false, GATE).status, want, "df {x}");
        }
        // p printed .045: [.0445, .0455], by Phase 1's comparison (engine
        // margin 1e-8 relative). Neither outside value is within one-tailed
        // reach, so each is not matched rather than not checked.
        // Above the interval, up to .046, the print is the exact p truncated.
        for (x, want) in [
            (0.0455, NumberStatus::Matched(None)),
            (0.045_500_2, NumberStatus::Matched(Some(MatchNote::Truncated))),
            (0.046, NumberStatus::NotMatched),
            (0.0445, NumberStatus::Matched(None)),
            (0.044_499_8, NumberStatus::Matched(Some(MatchNote::P(CheckNote::DoubleRounded)))),
            (0.044_449_9, NumberStatus::NotMatched),
        ] {
            let (_, _, cands) = t_claim_with_p(p_eq(".045"), x);
            assert_eq!(status_of(&cands, ComputedVariant::Paired, NumberKind::P), want, "p {x}");
        }
    }

    #[test]
    fn an_integer_df_must_equal_and_a_whole_decimal_reads_as_whole() {
        let count = ReportedNumber::count("45").unwrap();
        assert_eq!(compare_df(&count, 1, 45.0, true, GATE).status, NumberStatus::Matched(None));
        assert_eq!(compare_df(&count, 1, 44.0, true, GATE).status, NumberStatus::NotMatched);
        let whole = ReportedNumber::measured("45.0").unwrap();
        assert_eq!(compare_df(&whole, 1, 45.0, true, GATE).status, NumberStatus::Matched(None));
    }

    #[test]
    fn a_fractional_df_on_anova_is_could_not_check_variant_not_available() {
        let c = claim(Family::F, "3.21", &["2", "45.31"], None, p_eq(".049"));
        let f = ComputedResult {
            variant: ComputedVariant::OneWay,
            statistic: StatisticKind::F,
            value: 3.21,
            df1: 2.0,
            df2: Some(45.0),
            p: pvalue::f_upper(3.21, 2.0, 45.0).unwrap(),
            tail: Tail::Upper,
            n: 48,
            precision: Precision::Exact,
        };
        let (v, _, _) = verdict(&c, &ctx(), &facts(L1MethodId::OneWayAnova, Variant::None), &comp(L1MethodId::OneWayAnova, vec![f], 48, None));
        assert_eq!(v, L1Verdict::CouldNotCheck(L1CouldNotCheck::VariantNotAvailable(VariantGap::FractionalDf { index: 1 })));
    }

    /* ---- variants ------------------------------------------------------------ */

    #[test]
    fn with_no_variant_stated_student_matching_where_welch_does_not_is_reproduced_and_named() {
        let c = claim(Family::T, "2.10", &["28"], None, p_eq(".045"));
        let computation = comp(
            L1MethodId::IndependentT,
            vec![t_exact(ComputedVariant::StudentPooled, 2.1003, 28.0, 30), t_exact(ComputedVariant::Welch, 2.31, 25.4, 30)],
            30,
            None,
        );
        let (v, _, _) = verdict(&c, &ctx(), &not_stated_t(), &computation);
        let L1Verdict::Reproduced(notes) = v else { panic!("{v:?}") };
        assert_eq!(notes[0], L1Note::VariantMatched(ComputedVariant::StudentPooled));
        assert!(!notes.contains(&L1Note::VariantMatched(ComputedVariant::Welch)), "{notes:?}");
    }

    #[test]
    fn the_statistic_matching_student_and_the_p_matching_only_welch_is_not_reproduced() {
        let c = claim(Family::T, "2.10", &["28"], None, p_eq(".030"));
        let computation = comp(
            L1MethodId::IndependentT,
            vec![t_exact(ComputedVariant::StudentPooled, 2.1003, 28.0, 30), t_res(ComputedVariant::Welch, 2.40, 27.9, 0.030, 30)],
            30,
            None,
        );
        let (v, _, cands) = verdict(&c, &ctx(), &not_stated_t(), &computation);
        assert!(matches!(v, L1Verdict::NotReproduced { .. }), "{v:?}");
        assert!(matches!(status_of(&cands, ComputedVariant::StudentPooled, NumberKind::Statistic), NumberStatus::Matched(_)));
        assert_eq!(status_of(&cands, ComputedVariant::StudentPooled, NumberKind::P), NumberStatus::NotMatched);
        assert_eq!(status_of(&cands, ComputedVariant::Welch, NumberKind::Statistic), NumberStatus::NotMatched);
        assert!(matches!(status_of(&cands, ComputedVariant::Welch, NumberKind::P), NumberStatus::Matched(_)));
    }

    #[test]
    fn a_stated_variant_is_the_only_candidate() {
        let c = claim(Family::T, "2.10", &["28"], None, p_eq(".045"));
        let computation = comp(
            L1MethodId::IndependentT,
            vec![t_exact(ComputedVariant::StudentPooled, 2.1003, 28.0, 30), t_exact(ComputedVariant::Welch, 2.31, 25.4, 30)],
            30,
            None,
        );
        let (v, _, cands) = verdict(&c, &ctx(), &facts(L1MethodId::IndependentT, Variant::T(TVariant::Welch)), &computation);
        assert_eq!(cands.iter().map(|c| c.variant).collect::<Vec<_>>(), vec![ComputedVariant::Welch]);
        assert!(matches!(v, L1Verdict::NotReproduced { .. }), "Student matched, but Welch was stated: {v:?}");
    }

    #[test]
    fn spearmans_p_is_not_checked_when_it_misses_unless_the_t_approximation_is_stated() {
        let c = claim(Family::R, ".45", &["18"], None, p_eq(".041"));
        let rho = |p: f64| ComputedResult {
            variant: ComputedVariant::SpearmanTApproximation,
            statistic: StatisticKind::Rho,
            value: 0.4502,
            df1: 18.0,
            df2: None,
            p,
            tail: Tail::TwoSided,
            n: 20,
            precision: Precision::Exact,
        };
        let f = facts(L1MethodId::Spearman, Variant::SpearmanP(SpearmanPVariant::NotStated));
        // No variant stated: the t-approximation p does not match, and the
        // paper may have used the exact p. Without ties .041 lies inside the
        // band that holds R's exact p (part C, M-2), so it is not checked;
        // with ties it is not checked for its own reason.
        for (ties, why) in [(Some(false), NotCheckedReason::SpearmanPMayBeExact), (Some(true), NotCheckedReason::SpearmanPWithTies)] {
            let (v, _, cands) = verdict(&c, &ctx(), &f, &comp(L1MethodId::Spearman, vec![rho(0.0464)], 20, ties));
            assert_eq!(status_of(&cands, ComputedVariant::SpearmanTApproximation, NumberKind::P), NumberStatus::NotChecked(why));
            assert!(matches!(v, L1Verdict::Reproduced(_)), "ties {ties:?}: {v:?}");
        }
        // The t approximation stated: the exact p is not a candidate, so the p
        // is compared and the mismatch makes the claim NotReproduced, with or
        // without ties (review follow-up, item 3).
        let stated = facts(L1MethodId::Spearman, Variant::SpearmanP(SpearmanPVariant::TApproximation));
        for ties in [Some(false), Some(true)] {
            let (v, _, cands) = verdict(&c, &ctx(), &stated, &comp(L1MethodId::Spearman, vec![rho(0.0464)], 20, ties));
            assert_eq!(status_of(&cands, ComputedVariant::SpearmanTApproximation, NumberKind::P), NumberStatus::NotMatched);
            assert!(matches!(v, L1Verdict::NotReproduced { .. }), "ties {ties:?}: {v:?}");
        }
        // Stated exact: not computed, never assumed to match.
        let exact = facts(L1MethodId::Spearman, Variant::SpearmanP(SpearmanPVariant::Exact));
        let (v, _, _) = verdict(&c, &ctx(), &exact, &comp(L1MethodId::Spearman, vec![rho(0.041)], 20, Some(false)));
        assert_eq!(v, L1Verdict::CouldNotCheck(L1CouldNotCheck::VariantNotAvailable(VariantGap::SpearmanExactP)));
    }

    #[test]
    fn a_statistic_the_method_cannot_produce_is_a_binding_that_does_not_fit() {
        let c = claim(Family::F, "4.41", &["1", "28"], None, p_eq(".045"));
        let computation = comp(L1MethodId::IndependentT, vec![t_exact(ComputedVariant::StudentPooled, 2.1, 28.0, 30)], 30, None);
        let (v, _, _) = verdict(&c, &ctx(), &facts(L1MethodId::IndependentT, Variant::T(TVariant::Student)), &computation);
        assert_eq!(v, L1Verdict::CouldNotCheck(L1CouldNotCheck::BindingInvalid(BindingGap::FamilyDoesNotFitMethod)));
    }

    /* ---- p: Phase 1's rules, carried through ------------------------------- */

    fn t_claim_with_p(p: UserP<'_>, exact_p: f64) -> (L1Verdict, Vec<L1Note>, Vec<CandidateComparison>) {
        t_claim_with_p_in(p, exact_p, &ctx())
    }

    fn t_claim_with_p_in(p: UserP<'_>, exact_p: f64, ctx: &DocumentContext) -> (L1Verdict, Vec<L1Note>, Vec<CandidateComparison>) {
        let c = claim(Family::T, "2.05", &["28"], None, p);
        let computation = comp(L1MethodId::PairedT, vec![t_res(ComputedVariant::Paired, 2.05, 28.0, exact_p, 29)], 29, None);
        verdict(&c, ctx, &facts(L1MethodId::PairedT, Variant::None), &computation)
    }

    #[test]
    fn alpha_edges_p_equal_to_alpha_printed_as_equal_and_as_less_than() {
        // The exact p is .05: "= .05" holds it, and so does "< .05" within
        // Phase 1's engine margin (its own rule, unchanged).
        assert!(matches!(t_claim_with_p(p_eq(".05"), 0.05).0, L1Verdict::Reproduced(_)));
        let lt = UserP::Reported { raw: ".05", relation: Relation::Lt };
        assert!(matches!(t_claim_with_p(lt, 0.05).0, L1Verdict::Reproduced(_)));
        // "< .05" against an exact .12: not matched, even one-tailed (.06);
        // .12 and "< .05" are on opposite sides of .05, so the conclusion
        // changes. (An exact .06 would be Phase 1's one-tailed Concern, .03,
        // and so not checked.)
        assert_eq!(t_claim_with_p(lt, 0.12).0, L1Verdict::NotReproduced { conclusion_changes: Some(true) });
        let (v, _, cands) = t_claim_with_p(lt, 0.06);
        assert!(matches!(v, L1Verdict::Reproduced(_)), "{v:?}");
        assert_eq!(status_of(&cands, ComputedVariant::Paired, NumberKind::P), NumberStatus::NotChecked(NotCheckedReason::OneTailedUnstatedOnExactP));
        let L1Verdict::Reproduced(notes) = v else { unreachable!() };
        assert!(notes.contains(&L1Note::NotChecked { number: NumberKind::P, why: NotCheckedReason::OneTailedUnstatedOnExactP }), "{notes:?}");
        // "= .05" against .0444: not matched, and "= .05" is on neither side
        // of alpha, so the conclusion does not change. (.0449 is .045 rounded
        // to 3 decimals, then .05: a double-rounded print, part C.)
        assert_eq!(t_claim_with_p(p_eq(".05"), 0.0444).0, L1Verdict::NotReproduced { conclusion_changes: Some(false) });
        let (v, _, _) = t_claim_with_p(p_eq(".05"), 0.0449);
        assert!(matches!(&v, L1Verdict::Reproduced(n) if n.contains(&L1Note::P(CheckNote::DoubleRounded))), "{v:?}");
    }

    #[test]
    fn a_p_that_matches_only_one_tailed_is_not_checked_unless_the_paper_says_one_tailed() {
        // t(28) = 2.05 gives a two-tailed p of about .0499; .025 is half of it.
        let exact = pvalue::t_two_tailed(2.05, 28.0).unwrap();
        let (v, notes, cands) = t_claim_with_p(p_eq(".025"), exact);
        assert_eq!(
            status_of(&cands, ComputedVariant::Paired, NumberKind::P),
            NumberStatus::NotChecked(NotCheckedReason::PConcernAtLevel0(ConcernReason::OneTailedUnstated))
        );
        assert!(matches!(v, L1Verdict::Reproduced(_)), "{v:?} {notes:?}");
        assert_eq!(cands[0].p_outcome, Some(L0Verdict::Concern(ConcernReason::OneTailedUnstated)), "the record carries Level 0's verdict");
        let stated = DocumentContext { one_tailed_at: Some(TextPosition { section: 0, paragraph: Some(0) }), ..ctx() };
        let (v, _, cands) = t_claim_with_p_in(p_eq(".025"), exact, &stated);
        assert_eq!(status_of(&cands, ComputedVariant::Paired, NumberKind::P), NumberStatus::Matched(Some(MatchNote::P(CheckNote::OneTailedStated))));
        let L1Verdict::Reproduced(notes) = v else { panic!() };
        assert!(notes.contains(&L1Note::P(CheckNote::OneTailedStated)), "{notes:?}");
    }

    #[test]
    fn a_display_floor_printed_as_an_equality_matches_with_its_note() {
        let c = claim(Family::T, "6.50", &["28"], None, p_eq(".001"));
        let computation = comp(L1MethodId::PairedT, vec![t_exact(ComputedVariant::Paired, 6.5004, 28.0, 29)], 29, None);
        let (v, _, cands) = verdict(&c, &ctx(), &facts(L1MethodId::PairedT, Variant::None), &computation);
        assert_eq!(status_of(&cands, ComputedVariant::Paired, NumberKind::P), NumberStatus::Matched(Some(MatchNote::P(CheckNote::FloorPrintedAsEquality))));
        assert!(matches!(v, L1Verdict::Reproduced(_)));
    }

    #[test]
    fn an_adjusted_p_with_a_correction_named_is_not_checked_and_never_not_reproduced() {
        let named = DocumentContext { correction_at: Some(TextPosition { section: 0, paragraph: None }), ..ctx() };
        let exact = pvalue::t_two_tailed(2.05, 28.0).unwrap();
        let (v, _, cands) = t_claim_with_p_in(p_eq(".150"), exact, &named);
        assert_eq!(
            status_of(&cands, ComputedVariant::Paired, NumberKind::P),
            NumberStatus::NotChecked(NotCheckedReason::PCouldNotCheckAtLevel0(CouldNotCheckReason::AdjustedP))
        );
        assert!(matches!(v, L1Verdict::Reproduced(_)), "{v:?}");
    }

    #[test]
    fn ns_is_compared_with_alpha() {
        assert!(matches!(t_claim_with_p(UserP::NotSignificant, 0.21).0, L1Verdict::Reproduced(_)));
        assert_eq!(t_claim_with_p(UserP::NotSignificant, 0.01).0, L1Verdict::NotReproduced { conclusion_changes: Some(true) });
    }

    #[test]
    fn conclusion_changes_is_none_with_a_note_when_candidates_disagree() {
        // Neither candidate matches the statistic; Student's p is below .05
        // like the printed one, Welch's above it.
        let c = claim(Family::T, "3.00", &["28"], None, p_eq(".010"));
        let computation = comp(
            L1MethodId::IndependentT,
            vec![t_res(ComputedVariant::StudentPooled, 2.5, 28.0, 0.012, 30), t_res(ComputedVariant::Welch, 2.0, 27.9, 0.07, 30)],
            30,
            None,
        );
        let (v, notes, _) = verdict(&c, &ctx(), &not_stated_t(), &computation);
        assert_eq!(v, L1Verdict::NotReproduced { conclusion_changes: None });
        assert_eq!(notes, vec![L1Note::ConclusionNotAssessed(ConclusionGap::CandidatesDisagree)]);
    }

    /* ---- n ----------------------------------------------------------------- */

    #[test]
    fn n_stated_or_implied_by_rs_df_is_the_n_used() {
        let rho = ComputedResult {
            variant: ComputedVariant::Pearson,
            statistic: StatisticKind::R,
            value: 0.45,
            df1: 28.0,
            df2: None,
            p: pvalue::r_two_tailed(0.45, 28.0).unwrap(),
            tail: Tail::TwoSided,
            n: 30,
            precision: Precision::Exact,
        };
        let f = facts(L1MethodId::Pearson, Variant::None);
        let implied = claim(Family::R, ".45", &["28"], None, p_eq(".013"));
        let (_, _, cands) = verdict(&implied, &ctx(), &f, &comp(L1MethodId::Pearson, vec![rho], 30, None));
        assert_eq!(status_of(&cands, ComputedVariant::Pearson, NumberKind::N { source: NSource::ImpliedByDf }), NumberStatus::Matched(None));
        let stated = claim(Family::R, ".45", &["28"], Some("31"), p_eq(".013"));
        let (v, _, cands) = verdict(&stated, &ctx(), &f, &comp(L1MethodId::Pearson, vec![rho], 30, None));
        assert_eq!(status_of(&cands, ComputedVariant::Pearson, NumberKind::N { source: NSource::Stated }), NumberStatus::NotMatched);
        assert!(matches!(v, L1Verdict::NotReproduced { .. }));
    }

    /* ---- the verdict table ----------------------------------------------------- */

    fn row(kind: NumberKind, status: NumberStatus) -> NumberComparison {
        NumberComparison { kind, printed: Printed::NotSignificant, computed: 0.0, accepted: (0.0, 0.0), status, precision: None }
    }

    /// Every combination of number statuses, one or two candidate variants,
    /// and the p outcomes Level 0 and Level 1 can give, walked to its verdict.
    /// The expectation is stated per candidate in words: a candidate
    /// reproduces when its statistic matched and no number is not matched; a
    /// claim is reproduced when some candidate reproduces, and not reproduced
    /// when every candidate has a number not matched.
    #[test]
    fn the_verdict_table() {
        use super::NumberStatus::*;
        let stat = [Matched(None), Matched(Some(MatchNote::InMagnitude)), NotMatched];
        let df = [None, Some(Matched(None)), Some(Matched(Some(MatchNote::DfTruncated))), Some(NotMatched)];
        let p = [
            Matched(None),
            Matched(Some(MatchNote::P(CheckNote::FloorPrintedAsEquality))),
            NotMatched,
            NotChecked(NotCheckedReason::PConcernAtLevel0(ConcernReason::OneTailedUnstated)),
            NotChecked(NotCheckedReason::PConcernAtLevel0(ConcernReason::FTwoTailed)),
            NotChecked(NotCheckedReason::PCouldNotCheckAtLevel0(CouldNotCheckReason::AdjustedP)),
            NotChecked(NotCheckedReason::OneTailedUnstatedOnExactP),
            NotChecked(NotCheckedReason::SpearmanPMayBeExact),
        ];
        let n = [None, Some(Matched(None)), Some(NotMatched)];
        let mut singles = Vec::new();
        for s in stat {
            for d in df {
                for q in p {
                    for m in n {
                        let mut numbers = vec![row(NumberKind::Statistic, s)];
                        numbers.extend(d.map(|d| row(NumberKind::Df { index: 0 }, d)));
                        numbers.push(row(NumberKind::P, q));
                        numbers.extend(m.map(|m| row(NumberKind::N { source: NSource::Stated }, m)));
                        let reproduces = s != NotMatched && d != Some(NotMatched) && q != NotMatched && m != Some(NotMatched);
                        singles.push((numbers, reproduces));
                    }
                }
            }
        }
        let alpha = Alpha::One(crate::equation::rational::Rational::new(5, 100).unwrap());
        let cand = |v: ComputedVariant, numbers: &[NumberComparison]| CandidateComparison {
            variant: v,
            numbers: numbers.to_vec(),
            p_outcome: None,
            conclusion_changes: Some(false),
            stored_decimals: None,
        };
        let mut table = std::collections::BTreeMap::<(usize, &str), usize>::new();
        for (a, ra) in &singles {
            let (v, _) = decide(&[cand(ComputedVariant::Paired, a)], &alpha);
            let want = if *ra { "Reproduced" } else { "NotReproduced" };
            assert_eq!(matches!(v, L1Verdict::Reproduced(_)), *ra, "{a:?}");
            assert_eq!(matches!(v, L1Verdict::NotReproduced { .. }), !*ra, "{a:?}");
            *table.entry((1, want)).or_default() += 1;
            for (b, rb) in &singles {
                let (v, _) = decide(&[cand(ComputedVariant::StudentPooled, a), cand(ComputedVariant::Welch, b)], &alpha);
                let want = if *ra || *rb { "Reproduced" } else { "NotReproduced" };
                match (&v, *ra || *rb) {
                    (L1Verdict::Reproduced(notes), true) => {
                        // Names exactly the candidates that reproduce.
                        assert_eq!(notes.contains(&L1Note::VariantMatched(ComputedVariant::StudentPooled)), *ra);
                        assert_eq!(notes.contains(&L1Note::VariantMatched(ComputedVariant::Welch)), *rb);
                    }
                    (L1Verdict::NotReproduced { .. }, false) => {}
                    _ => panic!("{a:?} / {b:?} -> {v:?}"),
                }
                *table.entry((2, want)).or_default() += 1;
            }
        }
        // A p that Level 0 or Level 1 did not check never makes a claim not
        // reproduced: every single candidate whose only fault would be its p
        // reproduces when that p is not checked.
        for (numbers, reproduces) in &singles {
            let only_p_unchecked = numbers.iter().all(|r| r.status != NotMatched) && numbers.iter().any(|r| matches!(r.status, NotChecked(_)));
            if only_p_unchecked {
                assert!(*reproduces);
            }
        }
        // No candidate at all: could not check, never a pass.
        assert!(matches!(decide(&[], &alpha).0, L1Verdict::CouldNotCheck(_)));
        println!("candidates  verdict         combinations");
        for ((k, v), count) in &table {
            println!("{k:>10}  {v:<14}  {count}");
        }
        assert_eq!(singles.len(), 3 * 4 * 8 * 3);
        assert_eq!(table.values().sum::<usize>(), singles.len() + singles.len() * singles.len());
    }
}
