//! **`percent_matches_count`: is a printed percentage the count over its
//! denominator?** Gaply arithmetic; no publication behind it.
//!
//! Applies only when count, denominator and percentage are printed together,
//! as in `12 of 40 (30%)` or `12/40, 30.0%`. `Consistent` when
//! 100 · count / denominator lies in the percentage's printed interval,
//! otherwise `Inconsistent`, with `conclusion_changes` not assessed. Exact
//! rationals throughout.
//!
//! The count and the denominator must be counts (exact integers); a zero
//! denominator, or anything that is not a count, is
//! `CouldNotCheck(InvalidInput)`.
//!
//! Phase 1 has no reader for these forms: the method is built and pinned on
//! typed inputs and on `percent_count_fixtures.csv`.

use std::cmp::Ordering;

use super::method::{
    digest_hex, Applicability, Computed, CouldNotCheckReason, L0Verdict, Level, MethodCheck, NotApplicableReason,
    Provenance, ToleranceRule, VerificationMethod,
};
use super::number::{exact_cmp, ReportedNumber, Relation};
use super::summary::{CountPercentInput, SummaryInput};
use crate::equation::rational::Rational;

pub const ID: &str = "percent_matches_count";
pub const ENGINE_VERSION: &str = "percent_matches_count/1";
pub const PROVENANCE: Provenance = Provenance::GaplyArithmetic;
pub const TOLERANCE: ToleranceRule = ToleranceRule::RoundingOnly;

#[derive(Debug, Clone, Copy)]
pub struct PercentMatchesCount;

impl VerificationMethod<SummaryInput> for PercentMatchesCount {
    fn id(&self) -> &'static str {
        ID
    }
    fn level(&self) -> Level {
        Level::L0
    }
    fn provenance(&self) -> Provenance {
        PROVENANCE
    }
    fn tolerance(&self) -> ToleranceRule {
        TOLERANCE
    }
    fn applicability(&self, input: &SummaryInput) -> Applicability {
        match input {
            SummaryInput::CountWithPercent(_) => Applicability::Applicable,
            _ => Applicability::NotApplicable(NotApplicableReason::OtherInputKind),
        }
    }
    fn run(&self, input: &SummaryInput) -> MethodCheck {
        let mut computed = Vec::new();
        let (outcome, inputs_digest) = match input {
            SummaryInput::CountWithPercent(c) => (evaluate(c, &mut computed), digest(c)),
            _ => (INVALID, digest_hex(ID)),
        };
        MethodCheck {
            method_id: ID,
            provenance: PROVENANCE,
            engine_version: ENGINE_VERSION,
            inputs_digest,
            computed,
            tolerance: self.tolerance(),
            outcome,
        }
    }
}

const INVALID: L0Verdict = L0Verdict::CouldNotCheck(CouldNotCheckReason::InvalidInput);

/// The exact integer a count holds, if it is one.
pub(crate) fn whole(n: &ReportedNumber) -> Option<i128> {
    let v = n.value();
    let iv = n.interval();
    (n.relation() == Relation::Eq && v.is_integer() && iv.lo == iv.hi && !v.is_negative()).then(|| v.numer())
}

fn evaluate(c: &CountPercentInput, computed: &mut Vec<(&'static str, Computed)>) -> L0Verdict {
    let (Some(count), Some(den)) = (whole(&c.count), whole(&c.denominator)) else { return INVALID };
    if den == 0 || c.percent.relation() != Relation::Eq {
        return INVALID;
    }
    let Some(share) = count.checked_mul(100).and_then(|x| Rational::new(x, den)) else { return INVALID };
    let p = c.percent.interval();
    computed.push(("percent_of_count", Computed::Exact(share)));
    computed.push(("reported_percent", Computed::ExactRange { lo: p.lo, hi: p.hi }));
    match (exact_cmp(p.lo, share), exact_cmp(share, p.hi)) {
        (Some(a), Some(b)) if a != Ordering::Greater && b != Ordering::Greater => L0Verdict::Consistent(None),
        (Some(_), Some(_)) => L0Verdict::Inconsistent { conclusion_changes: None },
        _ => INVALID,
    }
}

fn digest(c: &CountPercentInput) -> String {
    digest_hex(&format!(
        "{ID}\ncount={}\ndenominator={}\npercent={}\n",
        c.count.canonical(),
        c.denominator.canonical(),
        c.percent.canonical()
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stats_l0::summary::GrimInput;

    fn input(count: &str, den: &str, percent: &str) -> SummaryInput {
        SummaryInput::CountWithPercent(CountPercentInput {
            count: ReportedNumber::count(count).unwrap(),
            denominator: ReportedNumber::count(den).unwrap(),
            percent: ReportedNumber::measured(percent).unwrap(),
        })
    }
    fn verdict(count: &str, den: &str, percent: &str) -> L0Verdict {
        let i = input(count, den, percent);
        assert_eq!(PercentMatchesCount.applicability(&i), Applicability::Applicable);
        PercentMatchesCount.run(&i).outcome
    }

    #[test]
    fn twelve_of_forty_is_thirty_percent() {
        // 100 · 12 / 40 = 30, inside "30" = [29.5, 30.5] and "30.0" = [29.95, 30.05].
        assert_eq!(verdict("12", "40", "30"), L0Verdict::Consistent(None));
        assert_eq!(verdict("12", "40", "30.0"), L0Verdict::Consistent(None));
    }

    #[test]
    fn a_percentage_the_count_cannot_give_is_inconsistent() {
        // 100 · 12 / 40 = 30; "33" is [32.5, 33.5].
        assert_eq!(verdict("12", "40", "33"), L0Verdict::Inconsistent { conclusion_changes: None });
        // Printed precision is part of the claim: 1/3 = 33.333…; "33.3" is
        // [33.25, 33.35] and holds it, "33.4" is [33.35, 33.45] and does not.
        assert_eq!(verdict("1", "3", "33.3"), L0Verdict::Consistent(None));
        assert_eq!(verdict("1", "3", "33.4"), L0Verdict::Inconsistent { conclusion_changes: None });
    }

    #[test]
    fn a_share_exactly_at_a_rounding_boundary_is_consistent_either_way() {
        // 1/8 = 12.5%: printed "12" ([11.5, 12.5]) or "13" ([12.5, 13.5]).
        assert_eq!(verdict("1", "8", "12"), L0Verdict::Consistent(None));
        assert_eq!(verdict("1", "8", "13"), L0Verdict::Consistent(None));
        assert_eq!(verdict("1", "8", "14"), L0Verdict::Inconsistent { conclusion_changes: None });
    }

    #[test]
    fn another_input_kind_is_not_applicable() {
        let mean = SummaryInput::Mean(GrimInput::new(
            ReportedNumber::measured("5.19").unwrap(),
            ReportedNumber::count("28").unwrap(),
        ));
        assert_eq!(
            PercentMatchesCount.applicability(&mean),
            Applicability::NotApplicable(NotApplicableReason::OtherInputKind)
        );
    }

    #[test]
    fn a_zero_denominator_or_a_non_count_could_not_be_checked() {
        assert_eq!(verdict("0", "0", "0"), INVALID);
        let i = SummaryInput::CountWithPercent(CountPercentInput {
            count: ReportedNumber::measured("12.5").unwrap(),
            denominator: ReportedNumber::count("40").unwrap(),
            percent: ReportedNumber::measured("30").unwrap(),
        });
        assert_eq!(PercentMatchesCount.run(&i).outcome, INVALID);
    }

    #[test]
    fn the_check_is_labelled() {
        let c = PercentMatchesCount.run(&input("12", "40", "30"));
        assert_eq!(c.provenance, Provenance::GaplyArithmetic);
        assert_eq!(c.computed[0], ("percent_of_count", Computed::Exact(Rational::from_int(30))));
    }
}
