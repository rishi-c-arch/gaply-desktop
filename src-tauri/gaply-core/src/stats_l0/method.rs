//! **The method contract: what every verification method is, and what it
//! returns.**
//!
//! Domain-neutral on purpose. Nothing here knows what a method checks; it knows
//! that a method has an identity and a level, says whether an input is one it
//! can judge, and returns a [`MethodCheck`] that records how it judged.
//!
//! `MethodCheck` because `Check` already names a module in this crate, and
//! [`ToleranceRule`] and [`CheckNote`] because `Tolerance` (`stats_verdict`) and
//! `Note` (`notes`) are taken.
//!
//! # The verdict set is closed
//!
//! [`L0Verdict`] has four variants and no score. `CouldNotCheck` is never a
//! pass. `Inconsistent` carries whether the conclusion changes and nothing
//! else: a check never says WHICH of the numbers it compared is wrong, because
//! from the manuscript alone that cannot be known.
//!
//! Deliberately NOT `Serialize`: nothing here carries manuscript text today,
//! and the wire form, when there is one, is a decision of its own.

use crate::equation::rational::Rational;

/// Which level of the verifier a method belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Level {
    /// The manuscript alone: no data file, no binding.
    L0,
}

/// Where a method's rule comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Provenance {
    /// A published method; `reference` names the publication.
    Published { reference: &'static str },
    /// Arithmetic Gaply states for itself, with no publication behind it.
    GaplyArithmetic,
}

/// Whether a method can judge an input at all. `NotApplicable` is not a
/// verdict: the method is simply not run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Applicability {
    Applicable,
    NotApplicable(NotApplicableReason),
}

/// Why a method does not judge an input.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotApplicableReason {
    /// The input is of a kind this method does not judge (a mean given to a
    /// percentage method).
    OtherInputKind,
    /// The unit prints no effect size, or no confidence interval.
    NoEstimateAndInterval,
    /// The interval does not follow the effect size directly, so it may belong
    /// to an estimate that is not printed.
    IntervalNotAfterEstimate,
    /// The family's df is not fixed by n alone: t and F need the design, and a
    /// chi-square's df does not depend on n.
    DfNotFixedByN,
    /// No n printed inside the unit.
    NoSampleSizeInUnit,
    /// The sentence says partial, semi-partial or controlling: the df of a
    /// partial correlation is n - 2 - k.
    PartialCorrelation,
    /// Nobody has said the data are integer-valued. Never inferred from text.
    GranularityUnknown,
}

/// A verification method. `run` is called only on input for which
/// `applicability` returned `Applicable`, and never panics.
pub trait VerificationMethod<I: ?Sized> {
    fn id(&self) -> &'static str;
    fn level(&self) -> Level;
    fn provenance(&self) -> Provenance;
    /// The tolerance every [`MethodCheck`] this method returns records. Part of
    /// the contract so that the registry can see it: a method is registered
    /// only with one (`tests/every_method_has_fixtures.rs`).
    fn tolerance(&self) -> ToleranceRule;
    fn applicability(&self, input: &I) -> Applicability;
    fn run(&self, input: &I) -> MethodCheck;
}

/// A value a method computed, typed.
#[derive(Debug, Clone, PartialEq)]
pub enum Computed {
    /// A closed range of reals.
    Range { lo: f64, hi: f64 },
    /// A closed range of exact values: a printed number's interval, or an
    /// interval built from the ends of two.
    ExactRange { lo: Rational, hi: Rational },
    /// An exact value.
    Exact(Rational),
    /// Several exact values, in the order they were found.
    ExactList(Vec<Rational>),
    Flag(bool),
    /// Where in the document something was found. Never the text itself.
    Position(TextPosition),
}

/// A place in a document by index only: the section, and the paragraph within
/// it, or `None` for the section's heading.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextPosition {
    pub section: usize,
    pub paragraph: Option<usize>,
}

/// The tolerance a method applied, stated as the rule itself.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ToleranceRule {
    /// Each computed endpoint moved outward by `margin` times its own size;
    /// the upper endpoint is then capped at `cap`.
    RelativeOutward { margin: f64, cap: f64 },
    /// No margin beyond the printed intervals themselves: every comparison is
    /// made on exact rationals.
    RoundingOnly,
}

/// SHA-256, hex, of a method's canonical encoding of its typed inputs: the
/// form [`MethodCheck::inputs_digest`] holds.
pub fn digest_hex(canonical: &str) -> String {
    use sha2::{Digest, Sha256};
    Sha256::digest(canonical.as_bytes()).iter().map(|b| format!("{b:02x}")).collect()
}

/// The record of one method run on one input.
#[derive(Debug, Clone, PartialEq)]
pub struct MethodCheck {
    pub method_id: &'static str,
    pub provenance: Provenance,
    pub engine_version: &'static str,
    /// SHA-256, hex, of the method's canonical encoding of its typed inputs.
    /// Never of manuscript text.
    pub inputs_digest: String,
    /// Named values, in the order computed.
    pub computed: Vec<(&'static str, Computed)>,
    pub tolerance: ToleranceRule,
    pub outcome: L0Verdict,
}

/// The closed verdict set: what one method says about one result, and what
/// [`super::outcome::claim_verdict`] says about the whole claim.
///
/// Not [`crate::stats_verdict::Verdict`] (`Match` / `Mismatch`), because the two
/// answer different questions. That one is Level 1: does the DATA reproduce the
/// reported value? This one is Level 0: can the printed numbers all be right at
/// once, judged from the manuscript alone? A Level 0 verdict has no data to
/// match against, and a Level 1 verdict has no `Concern` or `CouldNotCheck`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum L0Verdict {
    Consistent(Option<CheckNote>),
    /// `conclusion_changes`: `Some(true)` only when the reported value and the
    /// whole recomputed range fall on opposite sides of the decision threshold;
    /// `None` when that was not assessed.
    Inconsistent { conclusion_changes: Option<bool> },
    Concern(ConcernReason),
    CouldNotCheck(CouldNotCheckReason),
}

/// Why a `Consistent` needed a reading the manuscript states.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckNote {
    /// Consistent only as a one-tailed test, and the document says one-tailed.
    OneTailedStated,
    /// `p = .0001` and the like: a power of ten at or below .001 printed as an
    /// equality while the true p is smaller still. A software display floor;
    /// the conclusion cannot change.
    FloorPrintedAsEquality,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConcernReason {
    /// Consistent only as a one-tailed test, and the document never says so.
    OneTailedUnstated,
    /// An F consistent only as a two-tailed (variance-ratio) test.
    FTwoTailed,
    /// An estimate outside its interval that would lie inside it with the
    /// other direction: its negation (a magnitude printed beside a signed
    /// interval), or for a ratio measure its reciprocal.
    EstimateDirection,
}

/// Never a pass.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CouldNotCheckReason {
    /// The reported value is larger than recomputed and the document names a
    /// correction for multiple testing: it may be an adjusted value, which
    /// cannot be recomputed from one result.
    AdjustedP,
    /// The inputs do not define the computation, or a comparison could not be
    /// made honestly.
    InvalidInput,
    /// GRIM: every value at the printed precision is reachable with this many
    /// units, so a match says nothing.
    Uninformative,
    /// A registered method did not judge this claim. Produced only by
    /// [`super::outcome::claim_verdict`]; a method never returns it.
    NotApplicable(NotApplicableReason),
    /// No method ran on the claim at all: the check list was empty. Produced
    /// only by [`super::outcome::claim_verdict`], so the rule is total.
    NoMethodRan,
}
