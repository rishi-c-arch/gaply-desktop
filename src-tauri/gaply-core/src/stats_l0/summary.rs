//! **What the summary-statistic methods are given: a mean, a percentage, or a
//! count with its denominator and percentage.**
//!
//! Phase 1 reads none of these from a manuscript. `percent_matches_count` waits
//! for a reader of `12 of 40 (30%)`, planned after Phase 1; GRIM waits for a
//! person to say the data are integer-valued, wired in Phase 2. These methods
//! sit in [`super::registry::MethodRegistry::SUMMARY`], beside but not in
//! `LEVEL_0`, which is the list the manuscript path runs.
//!
//! Deliberately NOT `Serialize`: every field keeps its printed text.

use super::number::ReportedNumber;

/// Whether the data behind a mean or percentage take integer values (a Likert
/// item, a count). GRIM means nothing without it, and it is never inferred
/// from the manuscript's wording: someone has to say so.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Granularity {
    IntegerValued,
    Unknown,
}

/// A mean or a percentage, its n, and the number of integer items averaged per
/// participant.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GrimInput {
    /// The mean, or the percentage as printed (`71` for 71%), measured.
    pub value: ReportedNumber,
    /// A count.
    pub n: ReportedNumber,
    /// Items per participant; 1 unless stated.
    pub items: u32,
    pub granularity: Granularity,
}

impl GrimInput {
    /// One item, granularity unknown: the defaults, before anyone says more.
    pub fn new(value: ReportedNumber, n: ReportedNumber) -> GrimInput {
        GrimInput { value, n, items: 1, granularity: Granularity::Unknown }
    }
}

/// `12 of 40 (30%)`: a count, its denominator and the percentage printed with them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CountPercentInput {
    pub count: ReportedNumber,
    pub denominator: ReportedNumber,
    /// The percentage as printed (`30` for 30%), measured.
    pub percent: ReportedNumber,
}

/// One input to a summary-statistic method.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SummaryInput {
    Mean(GrimInput),
    Percent(GrimInput),
    CountWithPercent(CountPercentInput),
}
