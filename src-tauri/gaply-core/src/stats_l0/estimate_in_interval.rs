//! **`estimate_in_interval`: does a printed effect size lie inside the
//! confidence interval printed straight after it?** Gaply arithmetic; no
//! publication behind it.
//!
//! Applies only when one unit prints an effect size and a confidence interval
//! and the interval follows the effect size directly ([`CiFollows::EffectSize`]).
//! An interval anywhere else in the unit, straight after the test statistic for
//! example, may belong to an estimate that is not printed (a mean difference),
//! so it is not applicable. The interval is never compared with the test
//! statistic.
//!
//! Every number is its printed interval. The interval's widest reading runs
//! from the low end of the printed lower bound to the high end of the printed
//! upper bound:
//!
//! - The printed lower bound lies wholly above the printed upper bound:
//!   `Inconsistent`.
//! - The estimate's interval shares a point with that widest reading:
//!   `Consistent`.
//! - The estimate taken the other way shares a point with it:
//!   `Concern(EstimateDirection)`. For a ratio measure (odds, hazard or risk
//!   ratio, by the name the reader kept) the other way is the reciprocal; for
//!   any other measure it is the negation, as when a magnitude is printed
//!   beside a signed interval (`d = 0.45, 95% CI [−0.80, −0.10]`). Either may
//!   be a reporting convention rather than an error, so it is never
//!   `Inconsistent`. A ratio whose estimate interval reaches zero has no
//!   reciprocal and skips this rule.
//! - Otherwise `Inconsistent`.
//!
//! `conclusion_changes` is never assessed (`None`). Every comparison is exact,
//! on rationals; one that would overflow is `CouldNotCheck(InvalidInput)`.

use std::cmp::Ordering;

use super::input::L0Input;
use super::method::{
    digest_hex, Applicability, Computed, ConcernReason, CouldNotCheckReason, L0Verdict, Level, MethodCheck,
    NotApplicableReason, Provenance, ToleranceRule, VerificationMethod,
};
use super::number::{exact_cmp, ReportedNumber};
use super::result::CiFollows;
use crate::equation::rational::Rational;

pub const ID: &str = "estimate_in_interval";
pub const ENGINE_VERSION: &str = "estimate_in_interval/1";
pub const PROVENANCE: Provenance = Provenance::GaplyArithmetic;
pub const TOLERANCE: ToleranceRule = ToleranceRule::RoundingOnly;

#[derive(Debug, Clone, Copy)]
pub struct EstimateInInterval;

impl<'a> VerificationMethod<L0Input<'a>> for EstimateInInterval {
    fn id(&self) -> &'static str {
        ID
    }
    fn level(&self) -> Level {
        Level::L0
    }
    fn provenance(&self) -> Provenance {
        PROVENANCE
    }
    fn applicability(&self, input: &L0Input<'a>) -> Applicability {
        let r = input.result;
        match (&r.effect_size, &r.ci) {
            (Some(_), Some(ci)) if ci.follows == CiFollows::EffectSize => Applicability::Applicable,
            (Some(_), Some(_)) => Applicability::NotApplicable(NotApplicableReason::IntervalNotAfterEstimate),
            _ => Applicability::NotApplicable(NotApplicableReason::NoEstimateAndInterval),
        }
    }
    fn run(&self, input: &L0Input<'a>) -> MethodCheck {
        let r = input.result;
        let mut computed = Vec::new();
        let (outcome, inputs_digest) = match (&r.effect_size, &r.ci) {
            (Some(e), Some(ci)) => {
                let measure = Measure::of(&e.name);
                (
                    evaluate(&e.value, &ci.low, &ci.high, measure, &mut computed),
                    digest(&e.value, &ci.low, &ci.high, measure),
                )
            }
            // `run` is only called when applicable; this keeps it total.
            _ => (INVALID, digest_hex(ID)),
        };
        MethodCheck {
            method_id: ID,
            provenance: PROVENANCE,
            engine_version: ENGINE_VERSION,
            inputs_digest,
            computed,
            tolerance: TOLERANCE,
            outcome,
        }
    }
}

const INVALID: L0Verdict = L0Verdict::CouldNotCheck(CouldNotCheckReason::InvalidInput);
const INCONSISTENT: L0Verdict = L0Verdict::Inconsistent { conclusion_changes: None };

/// Which way an estimate can be taken the other way round.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Measure {
    /// Signed: the other direction is the negation.
    Signed,
    /// Odds, hazard or risk ratio: the other direction is the reciprocal.
    Ratio,
}

impl Measure {
    /// From the effect-size name as the reader kept it (`OR`, `odds ratio`,
    /// `HR`, …), in any case. Everything else is `Signed`.
    pub fn of(name: &str) -> Measure {
        let n = name.trim().to_lowercase();
        let n = n.split_whitespace().collect::<Vec<_>>().join(" ");
        match n.as_str() {
            "or" | "hr" | "rr" | "odds ratio" | "hazard ratio" | "risk ratio" => Measure::Ratio,
            _ => Measure::Signed,
        }
    }
}

/// `[lo, hi]` taken the other way: negated, or for a positive ratio interval
/// reciprocated. `None` when that is undefined or would overflow.
fn other_way(lo: Rational, hi: Rational, measure: Measure) -> Option<(Rational, Rational)> {
    match measure {
        Measure::Signed => Some((hi.neg()?, lo.neg()?)),
        Measure::Ratio if lo.numer() > 0 => {
            Some((Rational::new(hi.denom(), hi.numer())?, Rational::new(lo.denom(), lo.numer())?))
        }
        Measure::Ratio => None,
    }
}

/// Does `[a_lo, a_hi]` share a point with `[b_lo, b_hi]`? `None` on overflow.
fn overlaps(a_lo: Rational, a_hi: Rational, b_lo: Rational, b_hi: Rational) -> Option<bool> {
    Some(exact_cmp(a_hi, b_lo)? != Ordering::Less && exact_cmp(a_lo, b_hi)? != Ordering::Greater)
}

/// The rule, on the three printed numbers and the kind of measure.
pub fn evaluate(
    estimate: &ReportedNumber,
    low: &ReportedNumber,
    high: &ReportedNumber,
    measure: Measure,
    computed: &mut Vec<(&'static str, Computed)>,
) -> L0Verdict {
    let e = estimate.interval();
    let (lo, hi) = (low.interval().lo, high.interval().hi);
    computed.push(("estimate", Computed::ExactRange { lo: e.lo, hi: e.hi }));
    computed.push(("interval", Computed::ExactRange { lo, hi }));

    // The lower bound's interval wholly above the upper bound's.
    match exact_cmp(low.interval().lo, high.interval().hi) {
        None => return INVALID,
        Some(Ordering::Greater) => return INCONSISTENT,
        Some(_) => {}
    }
    match overlaps(e.lo, e.hi, lo, hi) {
        None => return INVALID,
        Some(true) => return L0Verdict::Consistent(None),
        Some(false) => {}
    }
    // The other direction: a magnitude beside a signed interval, or a ratio
    // printed the other way up.
    if let Some((o_lo, o_hi)) = other_way(e.lo, e.hi, measure) {
        let name = match measure {
            Measure::Signed => "negated_estimate",
            Measure::Ratio => "reciprocal_estimate",
        };
        computed.push((name, Computed::ExactRange { lo: o_lo, hi: o_hi }));
        match overlaps(o_lo, o_hi, lo, hi) {
            None => return INVALID,
            Some(true) => return L0Verdict::Concern(ConcernReason::EstimateDirection),
            Some(false) => {}
        }
    }
    INCONSISTENT
}

fn digest(estimate: &ReportedNumber, low: &ReportedNumber, high: &ReportedNumber, measure: Measure) -> String {
    digest_hex(&format!(
        "{ID}\nestimate={}\nlow={}\nhigh={}\nmeasure={measure:?}\n",
        estimate.canonical(),
        low.canonical(),
        high.canonical()
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::extract::extract_from_text;
    use crate::stats_l0::input::DocumentContext;
    use crate::stats_l0::result::{read_results, TableStructure};

    /// Read `body` as the Results of a document; apply the method to its only result.
    fn on(body: &str) -> (Applicability, Option<MethodCheck>) {
        let ex = extract_from_text(&format!("A Title\n\nResults\n{body}\n"));
        let reading = read_results(&ex, TableStructure::NotAvailable);
        assert_eq!(reading.results.len(), 1, "{body:?}: {:?}", reading.not_read);
        let ctx = DocumentContext::read(&ex, &reading);
        let input = L0Input { result: &reading.results[0], context: &ctx };
        let a = EstimateInInterval.applicability(&input);
        let c = (a == Applicability::Applicable).then(|| EstimateInInterval.run(&input));
        (a, c)
    }
    fn verdict(body: &str) -> L0Verdict {
        on(body).1.expect("applicable").outcome
    }
    fn m(s: &str) -> ReportedNumber {
        ReportedNumber::measured(s).unwrap()
    }

    #[test]
    fn an_estimate_inside_its_interval_is_consistent() {
        // d = 0.45 is [0.445, 0.455]; the interval at its widest is
        // [0.095, 0.805]. They overlap.
        assert_eq!(
            verdict("It worked, t(30) = 2.50, d = 0.45, 95% CI [0.10, 0.80], p = .018."),
            L0Verdict::Consistent(None)
        );
    }

    #[test]
    fn an_estimate_outside_its_interval_is_inconsistent() {
        // d = 0.95 is [0.945, 0.955]; the widest interval [0.095, 0.805] ends
        // below 0.945.
        assert_eq!(verdict("It worked, t(30) = 2.50, d = 0.95, 95% CI [0.10, 0.80], p = .018."), INCONSISTENT);
    }

    #[test]
    fn an_interval_after_the_statistic_is_not_applicable() {
        // The interval follows t: it may be the interval of a mean difference.
        let (a, c) = on("It worked, t(30) = 2.50, 95% CI [0.10, 0.80], d = 0.95, p = .018.");
        assert_eq!(a, Applicability::NotApplicable(NotApplicableReason::IntervalNotAfterEstimate));
        assert!(c.is_none());
        // With the p between them, too.
        let (a, _) = on("It worked, t(30) = 2.50, d = 0.95, p = .018, 95% CI [0.10, 0.80].");
        assert_eq!(a, Applicability::NotApplicable(NotApplicableReason::IntervalNotAfterEstimate));
    }

    #[test]
    fn a_unit_without_both_items_is_not_applicable() {
        for body in [
            "It worked, t(30) = 2.50, d = 0.45, p = .018.",
            "It worked, t(30) = 2.50, 95% CI [0.10, 0.80], p = .018.",
            "It worked, t(30) = 2.50, p = .018.",
        ] {
            assert_eq!(on(body).0, Applicability::NotApplicable(NotApplicableReason::NoEstimateAndInterval), "{body}");
        }
    }

    #[test]
    fn an_estimate_after_the_p_with_its_interval_is_checked() {
        // η2 = .245 is [.2445, .2455]; [0.10, 0.38] widest [0.095, 0.385].
        assert_eq!(
            verdict("Groups differed, F(3, 157) = 7.92, p < .001, η2 = .245, 95% CI [0.10, 0.38]."),
            L0Verdict::Consistent(None)
        );
    }

    #[test]
    fn rounding_at_either_end_is_never_inconsistent() {
        let mut c = Vec::new();
        // Estimate 0.81 is [0.805, 0.815]; upper bound 0.80 is [0.795, 0.805].
        // They share 0.805 exactly: an estimate of 0.8049 rounded up, an upper
        // bound of 0.8050 rounded down to even.
        assert_eq!(evaluate(&m("0.81"), &m("0.10"), &m("0.80"), Measure::Signed, &mut c), L0Verdict::Consistent(None));
        // 0.82 is [0.815, 0.825]: wholly above 0.805.
        assert_eq!(evaluate(&m("0.82"), &m("0.10"), &m("0.80"), Measure::Signed, &mut c), INCONSISTENT);
        // Lower end: estimate 0.09 [0.085, 0.095] meets 0.10 [0.095, 0.105].
        assert_eq!(evaluate(&m("0.09"), &m("0.10"), &m("0.80"), Measure::Signed, &mut c), L0Verdict::Consistent(None));
        assert_eq!(evaluate(&m("0.08"), &m("0.10"), &m("0.80"), Measure::Signed, &mut c), INCONSISTENT);
    }

    #[test]
    fn a_lower_bound_above_the_upper_bound_is_inconsistent() {
        let mut c = Vec::new();
        // Lower 0.80 is [0.795, 0.805], upper 0.10 is [0.095, 0.105]: wholly
        // above. The estimate lies between the printed numbers and still the
        // interval cannot be right.
        assert_eq!(evaluate(&m("0.45"), &m("0.80"), &m("0.10"), Measure::Signed, &mut c), INCONSISTENT);
        // The case only the reversal rule decides. Estimate 0.1 is [0.05, 0.15];
        // lower 0.12 is [0.115, 0.125], upper 0.10 is [0.095, 0.105]. The
        // estimate reaches both 0.115 and 0.105, so the overlap test alone
        // would pass it; but the lower bound lies wholly above the upper.
        assert_eq!(evaluate(&m("0.1"), &m("0.12"), &m("0.10"), Measure::Signed, &mut c), INCONSISTENT);
        // Lower 0.5 is [0.45, 0.55] and upper 0.49 is [0.485, 0.495]: the
        // printed values are reversed, but a true lower of 0.46 and a true
        // upper of 0.49 print exactly so. Not Inconsistent.
        assert_eq!(evaluate(&m("0.47"), &m("0.5"), &m("0.49"), Measure::Signed, &mut c), L0Verdict::Consistent(None));
    }

    #[test]
    fn negative_estimates_and_bounds_are_compared_as_printed() {
        let mut c = Vec::new();
        // −0.45 [−0.455, −0.445] inside [−0.805, −0.095].
        assert_eq!(evaluate(&m("\u{2212}0.45"), &m("\u{2212}0.80"), &m("\u{2212}0.10"), Measure::Signed, &mut c), L0Verdict::Consistent(None));
    }

    const DIRECTION: L0Verdict = L0Verdict::Concern(ConcernReason::EstimateDirection);

    #[test]
    fn a_magnitude_beside_a_signed_interval_is_a_direction_concern() {
        let mut c = Vec::new();
        // d = 0.45 is [0.445, 0.455]; the interval at its widest is
        // [−0.805, −0.095]. Missed. Negated, [−0.455, −0.445] lies inside.
        let v = evaluate(&m("0.45"), &m("\u{2212}0.80"), &m("\u{2212}0.10"), Measure::Signed, &mut c);
        assert_eq!(v, DIRECTION);
        assert!(c.iter().any(|(n, _)| *n == "negated_estimate"), "{c:?}");
    }

    #[test]
    fn an_estimate_wrong_both_ways_stays_inconsistent() {
        let mut c = Vec::new();
        // d = 0.95 is [0.945, 0.955]; negated [−0.955, −0.945], below −0.805.
        assert_eq!(evaluate(&m("0.95"), &m("\u{2212}0.80"), &m("\u{2212}0.10"), Measure::Signed, &mut c), INCONSISTENT);
    }

    #[test]
    fn a_ratio_printed_the_other_way_up_is_a_direction_concern() {
        let mut c = Vec::new();
        // OR = 2.50 is [2.495, 2.505]; the interval at its widest is
        // [0.245, 0.605]. Missed. Reciprocal [1/2.505, 1/2.495] =
        // [0.39920, 0.40080], inside.
        assert_eq!(evaluate(&m("2.50"), &m("0.25"), &m("0.60"), Measure::Ratio, &mut c), DIRECTION);
        assert!(c.iter().any(|(n, _)| *n == "reciprocal_estimate"), "{c:?}");
        // OR = 2.50 against [3.00, 4.00]: missed, and the reciprocal 0.4 too.
        assert_eq!(evaluate(&m("2.50"), &m("3.00"), &m("4.00"), Measure::Ratio, &mut c), INCONSISTENT);
    }

    #[test]
    fn each_measure_is_turned_only_its_own_way() {
        let mut c = Vec::new();
        // A signed d = 2.50 against [0.25, 0.60]: its reciprocal would fit,
        // but a d is not a ratio. Negated [−2.505, −2.495] does not fit.
        assert_eq!(evaluate(&m("2.50"), &m("0.25"), &m("0.60"), Measure::Signed, &mut c), INCONSISTENT);
        // A ratio's estimate interval reaching zero has no reciprocal: "0.0"
        // is [−0.05, 0.05]. Against [0.25, 0.60] it stays Inconsistent.
        assert_eq!(evaluate(&m("0.0"), &m("0.25"), &m("0.60"), Measure::Ratio, &mut c), INCONSISTENT);
    }

    #[test]
    fn ratio_names_are_recognised_in_any_case() {
        for n in ["OR", "or", "HR", "RR", "odds ratio", "Odds  Ratio", "hazard ratio", "risk ratio"] {
            assert_eq!(Measure::of(n), Measure::Ratio, "{n:?}");
        }
        for n in ["d", "Cohen's d", "η2", "g", "r", "R2", "Cramer's V"] {
            assert_eq!(Measure::of(n), Measure::Signed, "{n:?}");
        }
    }

    #[test]
    fn a_ratio_read_from_text_is_checked_as_a_ratio() {
        // The reader keeps the name "OR"; the interval follows it directly.
        assert_eq!(
            verdict("Uptake differed, χ2(1, N = 226) = 6.90, p < .01, OR = 2.50, 95% CI [0.25, 0.60]."),
            DIRECTION
        );
    }

    #[test]
    fn the_check_is_labelled_and_records_both_ranges() {
        let c = on("It worked, t(30) = 2.50, d = 0.45, 95% CI [0.10, 0.80], p = .018.").1.unwrap();
        assert_eq!(c.method_id, ID);
        assert_eq!(c.provenance, Provenance::GaplyArithmetic);
        assert_eq!(c.tolerance, ToleranceRule::RoundingOnly);
        let names: Vec<&str> = c.computed.iter().map(|(n, _)| *n).collect();
        assert_eq!(names, vec!["estimate", "interval"]);
        assert_eq!(c.inputs_digest.len(), 64);
    }
}
