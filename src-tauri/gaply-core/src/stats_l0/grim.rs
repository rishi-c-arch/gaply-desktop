//! **GRIM: can a mean (or a percentage) of integer data be reached with this
//! n?** Published: Brown and Heathers 2017, Social Psychological and
//! Personality Science 8(4). Two methods, `grim_mean` and `grim_percent`.
//!
//! A mean of N = n · items integer values is k / N for some integer total k.
//! The printed mean is its interval [lo, hi], closed at both ends, so rounding
//! up and rounding down at the half are both accepted (scrutiny's default,
//! `rounding = "up_or_down"`). A percentage is the same with [lo, hi] / 100.
//!
//! - **Not applicable** (`GranularityUnknown`) unless the caller says the data
//!   are integer-valued. Nothing in Phase 1 says so; scale type is never
//!   inferred from text.
//! - **`CouldNotCheck(Uninformative)`**, never `Consistent`, when EVERY value
//!   at the printed precision is reachable with this N, so a match says
//!   nothing. See below for the exact condition.
//! - **`Consistent`** when some integer k has k / N in the interval, i.e.
//!   ceil(lo · N) ≤ floor(hi · N). Otherwise **`Inconsistent`**, with
//!   `conclusion_changes` not assessed.
//!
//! n must be a count, items at least 1; otherwise `CouldNotCheck(InvalidInput)`,
//! as is any exact product that would overflow.
//!
//! # When every value is reachable
//!
//! With d decimals in the proportion (a mean's own decimals, a percentage's
//! plus two), write M = 10^d. The printed value j / M is reachable when some
//! k / N lies within 1 / (2M) of it: |kM − jN| ≤ N / 2. As j runs over the
//! integers, jN mod M runs over exactly the multiples of g = gcd(N, M) in
//! [0, M). So j is UNreachable when x = jN mod M has N / 2 < x < M − N / 2,
//! and every value is reachable exactly when no multiple of g lies strictly
//! between N / 2 and M − N / 2.
//!
//! N ≥ M makes that interval empty: the familiar threshold. But it is not the
//! only case. With M = 100: N = 80 (g = 20; the multiples 40 and 60 sit ON
//! the ends 40 and 60) and N = 96 (g = 4; 48 and 52 on the ends 48 and 52)
//! reach every two-decimal value, and a test that called them informative
//! would return `Consistent` for every mean it was given. Measured with
//! scrutiny 0.6.1: `grim(x, 80)` and `grim(x, 96)` are TRUE for all 601
//! two-decimal x from 1.00 to 7.00, while `grim(x, 81)` is FALSE for 114.

use super::method::{
    digest_hex, Applicability, Computed, CouldNotCheckReason, L0Verdict, Level, MethodCheck, NotApplicableReason,
    Provenance, ToleranceRule, VerificationMethod,
};
use super::number::Relation;
use super::percent_matches_count::whole;
use super::summary::{Granularity, GrimInput, SummaryInput};
use crate::equation::rational::Rational;

pub const MEAN_ID: &str = "grim_mean";
pub const PERCENT_ID: &str = "grim_percent";
pub const ENGINE_VERSION: &str = "grim/1";
pub const PROVENANCE: Provenance = Provenance::Published {
    reference: "Brown and Heathers 2017, Social Psychological and Personality Science 8(4)",
};
pub const TOLERANCE: ToleranceRule = ToleranceRule::RoundingOnly;

/// GRIM on a mean.
#[derive(Debug, Clone, Copy)]
pub struct GrimMean;

/// GRIM on a percentage.
#[derive(Debug, Clone, Copy)]
pub struct GrimPercent;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Mean,
    Percent,
}

impl Kind {
    fn id(self) -> &'static str {
        match self {
            Kind::Mean => MEAN_ID,
            Kind::Percent => PERCENT_ID,
        }
    }
    fn input(self, input: &SummaryInput) -> Option<&GrimInput> {
        match (self, input) {
            (Kind::Mean, SummaryInput::Mean(g)) | (Kind::Percent, SummaryInput::Percent(g)) => Some(g),
            _ => None,
        }
    }
    fn applicability(self, input: &SummaryInput) -> Applicability {
        match self.input(input) {
            None => Applicability::NotApplicable(NotApplicableReason::OtherInputKind),
            Some(g) if g.granularity != Granularity::IntegerValued => {
                Applicability::NotApplicable(NotApplicableReason::GranularityUnknown)
            }
            Some(_) => Applicability::Applicable,
        }
    }
    fn run(self, input: &SummaryInput) -> MethodCheck {
        let mut computed = Vec::new();
        let (outcome, inputs_digest) = match self.input(input) {
            Some(g) => (evaluate(self, g, &mut computed), digest(self, g)),
            None => (INVALID, digest_hex(self.id())),
        };
        MethodCheck {
            method_id: self.id(),
            provenance: PROVENANCE,
            engine_version: ENGINE_VERSION,
            inputs_digest,
            computed,
            tolerance: TOLERANCE,
            outcome,
        }
    }
}

impl VerificationMethod<SummaryInput> for GrimMean {
    fn id(&self) -> &'static str {
        MEAN_ID
    }
    fn level(&self) -> Level {
        Level::L0
    }
    fn provenance(&self) -> Provenance {
        PROVENANCE
    }
    fn applicability(&self, input: &SummaryInput) -> Applicability {
        Kind::Mean.applicability(input)
    }
    fn run(&self, input: &SummaryInput) -> MethodCheck {
        Kind::Mean.run(input)
    }
}

impl VerificationMethod<SummaryInput> for GrimPercent {
    fn id(&self) -> &'static str {
        PERCENT_ID
    }
    fn level(&self) -> Level {
        Level::L0
    }
    fn provenance(&self) -> Provenance {
        PROVENANCE
    }
    fn applicability(&self, input: &SummaryInput) -> Applicability {
        Kind::Percent.applicability(input)
    }
    fn run(&self, input: &SummaryInput) -> MethodCheck {
        Kind::Percent.run(input)
    }
}

const INVALID: L0Verdict = L0Verdict::CouldNotCheck(CouldNotCheckReason::InvalidInput);

fn gcd(mut a: i128, mut b: i128) -> i128 {
    while b != 0 {
        (a, b) = (b, a % b);
    }
    a.abs()
}

/// Every value with `decimals` decimals is reachable as k / `units`: no
/// multiple of g = gcd(units, 10^decimals) lies strictly between units / 2 and
/// 10^decimals − units / 2 (module docs). Worked in doubled integers to keep
/// the halves exact. `None` on overflow.
fn every_value_reachable(units: i128, decimals: u32) -> Option<bool> {
    let m = 10i128.checked_pow(decimals)?;
    let g = gcd(units, m);
    // The smallest multiple of g strictly above units / 2.
    let first = g.checked_mul(units / (2 * g) + 1)?;
    Some(first.checked_mul(2)? >= m.checked_mul(2)?.checked_sub(units)?)
}

/// ceil(q · units / scale) and floor(q · units / scale), q = a / b with b > 0.
fn scaled(q: Rational, units: i128, scale: i128) -> Option<(i128, i128)> {
    let num = q.numer().checked_mul(units)?;
    let den = q.denom().checked_mul(scale)?;
    let floor = num.div_euclid(den);
    let ceil = if num.rem_euclid(den) == 0 { floor } else { floor.checked_add(1)? };
    Some((ceil, floor))
}

fn evaluate(kind: Kind, g: &GrimInput, computed: &mut Vec<(&'static str, Computed)>) -> L0Verdict {
    let Some(n) = whole(&g.n) else { return INVALID };
    if n == 0 || g.items == 0 || g.value.relation() != Relation::Eq {
        return INVALID;
    }
    let Some(units) = n.checked_mul(i128::from(g.items)) else { return INVALID };
    let (scale, decimals) = match kind {
        Kind::Mean => (1, g.value.decimals()),
        Kind::Percent => (100, g.value.decimals() + 2),
    };
    computed.push(("units", Computed::Exact(Rational::from_int(units))));
    computed.push(("decimals", Computed::Exact(Rational::from_int(i128::from(decimals)))));

    match every_value_reachable(units, decimals) {
        None => return INVALID,
        Some(true) => return L0Verdict::CouldNotCheck(CouldNotCheckReason::Uninformative),
        Some(false) => {}
    }

    let iv = g.value.interval();
    let (Some((first, _)), Some((_, last))) = (scaled(iv.lo, units, scale), scaled(iv.hi, units, scale)) else {
        return INVALID;
    };
    // The integer totals k with k / units in the interval: [first, last].
    computed.push((
        "totals",
        Computed::ExactRange { lo: Rational::from_int(first), hi: Rational::from_int(last) },
    ));
    if first <= last {
        L0Verdict::Consistent(None)
    } else {
        L0Verdict::Inconsistent { conclusion_changes: None }
    }
}

fn digest(kind: Kind, g: &GrimInput) -> String {
    digest_hex(&format!(
        "{}\nvalue={}\ndecimals={}\nn={}\nitems={}\ngranularity={:?}\n",
        kind.id(),
        g.value.canonical(),
        g.value.decimals(),
        g.n.canonical(),
        g.items,
        g.granularity
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stats_l0::number::ReportedNumber;

    fn grim(value: &str, n: &str, items: u32, integer: bool) -> GrimInput {
        GrimInput {
            value: ReportedNumber::measured(value).unwrap(),
            n: ReportedNumber::count(n).unwrap(),
            items,
            granularity: if integer { Granularity::IntegerValued } else { Granularity::Unknown },
        }
    }
    fn mean(value: &str, n: &str, items: u32) -> L0Verdict {
        let i = SummaryInput::Mean(grim(value, n, items, true));
        assert_eq!(GrimMean.applicability(&i), Applicability::Applicable);
        GrimMean.run(&i).outcome
    }
    fn percent(value: &str, n: &str) -> L0Verdict {
        let i = SummaryInput::Percent(grim(value, n, 1, true));
        assert_eq!(GrimPercent.applicability(&i), Applicability::Applicable);
        GrimPercent.run(&i).outcome
    }
    const INCONSISTENT: L0Verdict = L0Verdict::Inconsistent { conclusion_changes: None };
    const UNINFORMATIVE: L0Verdict = L0Verdict::CouldNotCheck(CouldNotCheckReason::Uninformative);

    #[test]
    fn mean_5_19_with_n_28_is_inconsistent() {
        // "5.19" is [5.185, 5.195]. N = 28: 5.185 · 28 = 145.18, 5.195 · 28 =
        // 145.46. No integer in [145.18, 145.46]: 145/28 = 5.1786 and
        // 146/28 = 5.2143 both round away from 5.19.
        assert_eq!(mean("5.19", "28", 1), INCONSISTENT);
    }

    #[test]
    fn mean_5_19_with_n_32_is_consistent() {
        // N = 32: [5.185 · 32, 5.195 · 32] = [165.92, 166.24] holds 166, and
        // 166/32 = 5.1875, which rounds to 5.19.
        assert_eq!(mean("5.19", "32", 1), L0Verdict::Consistent(None));
    }

    #[test]
    fn mean_2_84_with_n_16_and_2_items_is_consistent() {
        // N = 16 · 2 = 32: [2.835 · 32, 2.845 · 32] = [90.72, 91.04] holds 91;
        // 91/32 = 2.84375, which rounds to 2.84.
        assert_eq!(mean("2.84", "16", 2), L0Verdict::Consistent(None));
        // With one item, N = 16: [45.36, 45.52] holds no integer.
        assert_eq!(mean("2.84", "16", 1), INCONSISTENT);
    }

    #[test]
    fn percent_71_with_n_43_is_inconsistent() {
        // "71" is [70.5, 71.5]%, the proportion [0.705, 0.715]. N = 43:
        // [30.315, 30.745] holds no integer; 30/43 = 69.77% and 31/43 =
        // 72.09%.
        assert_eq!(percent("71", "43"), INCONSISTENT);
        // 72 is [0.715, 0.725] · 43 = [30.745, 31.175]: 31 is in.
        assert_eq!(percent("72", "43"), L0Verdict::Consistent(None));
    }

    #[test]
    fn mean_5_19_with_n_500_is_uninformative() {
        // N = 500 ≥ 10^2: every two-decimal value has some k/500 within
        // 0.005 (they are 0.002 apart). Never Consistent.
        assert_eq!(mean("5.19", "500", 1), UNINFORMATIVE);
        // The same N with three decimals is informative again: 1000 > 500.
        // "5.191" is [5.1905, 5.1915] · 500 = [2595.25, 2595.75], no integer.
        assert_eq!(mean("5.191", "500", 1), INCONSISTENT);
    }

    #[test]
    fn below_the_threshold_n_80_and_96_still_reach_every_two_decimal_value() {
        // The module docs derive these; checked here over every two-decimal
        // mean from 1.00 to 7.00 by the reachability arithmetic itself.
        for units in [80i128, 96] {
            assert_eq!(every_value_reachable(units, 2), Some(true), "{units}");
            for j in 100..=700 {
                let (first, _) = scaled(Rational::new(2 * j - 1, 200).unwrap(), units, 1).unwrap();
                let (_, last) = scaled(Rational::new(2 * j + 1, 200).unwrap(), units, 1).unwrap();
                assert!(first <= last, "{j}/100 unreachable with N = {units}");
            }
        }
        // And the method says so rather than Consistent.
        assert_eq!(mean("5.19", "80", 1), UNINFORMATIVE);
        assert_eq!(mean("5.19", "32", 3), UNINFORMATIVE); // N = 96
        assert_eq!(percent("71", "80"), UNINFORMATIVE);
    }

    #[test]
    fn the_reachability_rule_matches_a_direct_count_for_every_small_n() {
        // For every N from 1 to 250 and d of 1 and 2: the closed form agrees
        // with counting the reachable values over one period j = 0..10^d.
        let mut checked = 0;
        for d in 1..=2u32 {
            let m = 10i128.pow(d);
            for units in 1..=250i128 {
                let direct = (0..m).all(|j| {
                    let lo = Rational::new(2 * j - 1, 2 * m).unwrap();
                    let hi = Rational::new(2 * j + 1, 2 * m).unwrap();
                    scaled(lo, units, 1).unwrap().0 <= scaled(hi, units, 1).unwrap().1
                });
                assert_eq!(every_value_reachable(units, d), Some(direct), "N = {units}, d = {d}");
                checked += 1;
            }
        }
        assert_eq!(checked, 500);
    }

    #[test]
    fn without_the_integer_valued_flag_grim_is_not_applicable() {
        let na = Applicability::NotApplicable(NotApplicableReason::GranularityUnknown);
        assert_eq!(GrimMean.applicability(&SummaryInput::Mean(grim("5.19", "28", 1, false))), na);
        assert_eq!(GrimPercent.applicability(&SummaryInput::Percent(grim("71", "43", 1, false))), na);
        // The default constructor leaves it unknown.
        let d = GrimInput::new(ReportedNumber::measured("5.19").unwrap(), ReportedNumber::count("28").unwrap());
        assert_eq!(GrimMean.applicability(&SummaryInput::Mean(d)), na);
    }

    #[test]
    fn each_grim_judges_only_its_own_kind() {
        let na = Applicability::NotApplicable(NotApplicableReason::OtherInputKind);
        assert_eq!(GrimMean.applicability(&SummaryInput::Percent(grim("71", "43", 1, true))), na);
        assert_eq!(GrimPercent.applicability(&SummaryInput::Mean(grim("5.19", "28", 1, true))), na);
    }

    #[test]
    fn a_rounding_boundary_is_reachable_both_ways() {
        // N = 8: 1/8 = 0.125 sits on the boundary between "0.12" and "0.13";
        // both are Consistent. 0.14 is [0.135, 0.145] · 8 = [1.08, 1.16]: no.
        assert_eq!(mean("0.12", "8", 1), L0Verdict::Consistent(None));
        assert_eq!(mean("0.13", "8", 1), L0Verdict::Consistent(None));
        assert_eq!(mean("0.14", "8", 1), INCONSISTENT);
    }

    #[test]
    fn invalid_counts_could_not_be_checked() {
        let mut g = grim("5.19", "28", 0, true);
        assert_eq!(GrimMean.run(&SummaryInput::Mean(g.clone())).outcome, INVALID);
        g.items = 1;
        g.n = ReportedNumber::count("0").unwrap();
        assert_eq!(GrimMean.run(&SummaryInput::Mean(g.clone())).outcome, INVALID);
        g.n = ReportedNumber::measured("28.5").unwrap();
        assert_eq!(GrimMean.run(&SummaryInput::Mean(g)).outcome, INVALID);
    }

    #[test]
    fn the_check_is_labelled_published_with_its_reference() {
        let c = GrimMean.run(&SummaryInput::Mean(grim("5.19", "32", 1, true)));
        assert_eq!(c.method_id, MEAN_ID);
        assert_eq!(c.provenance, PROVENANCE);
        let Provenance::Published { reference } = c.provenance else { panic!() };
        assert!(reference.contains("Brown and Heathers 2017"));
        let names: Vec<&str> = c.computed.iter().map(|(n, _)| *n).collect();
        assert_eq!(names, vec!["units", "decimals", "totals"]);
    }
}
