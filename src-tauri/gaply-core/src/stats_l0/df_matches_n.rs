//! **`df_matches_n`: does a correlation's df equal n − 2?** Gaply arithmetic;
//! no publication behind it.
//!
//! Applies only to r, and only when n is printed inside the same unit. For t
//! and F the df depends on the design (groups, cells, repeated measures), and a
//! chi-square's df does not depend on n at all, so all three are not
//! applicable. A test in Results is never connected to a sample size in
//! Methods: only the unit's own n is used.
//!
//! Not applicable either when the unit's sentence says partial, semi-partial or
//! controlling: a partial correlation's df is n − 2 − k, and k is not printed.
//!
//! `Consistent` when n − 2 lies in the df's printed interval (an integer df is
//! a point, so that is equality), otherwise `Inconsistent`, with
//! `conclusion_changes` not assessed. Exact rationals throughout.
//!
//! # Not reachable from a manuscript today
//!
//! The Step 3 reader accepts `N =` inside a chi-square's brackets only; an r
//! printed `r(48, N = 50)` is `MalformedDegreesOfFreedom`, so no result it
//! returns is an r with an n. The method is built and pinned on typed inputs.

use std::cmp::Ordering;

use super::input::L0Input;
use super::method::{
    digest_hex, Applicability, Computed, CouldNotCheckReason, L0Verdict, Level, MethodCheck, NotApplicableReason,
    Provenance, ToleranceRule, VerificationMethod,
};
use super::number::exact_cmp;
use super::result::{Family, ReportedResult};
use crate::equation::rational::Rational;

pub const ID: &str = "df_matches_n";
pub const ENGINE_VERSION: &str = "df_matches_n/1";
pub const PROVENANCE: Provenance = Provenance::GaplyArithmetic;
pub const TOLERANCE: ToleranceRule = ToleranceRule::RoundingOnly;

#[derive(Debug, Clone, Copy)]
pub struct DfMatchesN;

impl<'a> VerificationMethod<L0Input<'a>> for DfMatchesN {
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
    fn applicability(&self, input: &L0Input<'a>) -> Applicability {
        let r = input.result;
        if r.family != Family::R {
            return Applicability::NotApplicable(NotApplicableReason::DfNotFixedByN);
        }
        if r.n.is_none() {
            return Applicability::NotApplicable(NotApplicableReason::NoSampleSizeInUnit);
        }
        if r.sentence_says_partial {
            return Applicability::NotApplicable(NotApplicableReason::PartialCorrelation);
        }
        Applicability::Applicable
    }
    fn run(&self, input: &L0Input<'a>) -> MethodCheck {
        let mut computed = Vec::new();
        let outcome = evaluate(input.result, &mut computed);
        MethodCheck {
            method_id: ID,
            provenance: PROVENANCE,
            engine_version: ENGINE_VERSION,
            inputs_digest: digest(input.result),
            computed,
            tolerance: self.tolerance(),
            outcome,
        }
    }
}

const INVALID: L0Verdict = L0Verdict::CouldNotCheck(CouldNotCheckReason::InvalidInput);

fn evaluate(r: &ReportedResult, computed: &mut Vec<(&'static str, Computed)>) -> L0Verdict {
    let (Some(n), [df]) = (&r.n, r.df.as_slice()) else { return INVALID };
    let Some(expected) = n.value().sub(Rational::from_int(2)) else { return INVALID };
    let d = df.interval();
    computed.push(("n_minus_2", Computed::Exact(expected)));
    computed.push(("df", Computed::ExactRange { lo: d.lo, hi: d.hi }));
    match (exact_cmp(d.lo, expected), exact_cmp(expected, d.hi)) {
        (Some(a), Some(b)) if a != Ordering::Greater && b != Ordering::Greater => L0Verdict::Consistent(None),
        (Some(_), Some(_)) => L0Verdict::Inconsistent { conclusion_changes: None },
        _ => INVALID,
    }
}

fn digest(r: &ReportedResult) -> String {
    let mut s = format!("{ID}\nfamily={:?}\n", r.family);
    for d in &r.df {
        s.push_str(&format!("df={}\n", d.canonical()));
    }
    if let Some(n) = &r.n {
        s.push_str(&format!("n={}\n", n.canonical()));
    }
    s.push_str(&format!("partial={}\n", r.sentence_says_partial));
    digest_hex(&s)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::extract::{Location, SectionKind};
    use crate::stats_l0::input::DocumentContext;
    use crate::stats_l0::number::{ReportedNumber, Relation};
    use crate::stats_l0::result::{PValue, Source};

    /// A result built by hand: the reader never returns an r with an n.
    fn result(family: Family, df: &[&str], n: Option<&str>, partial: bool) -> ReportedResult {
        let df_number = |s: &str| ReportedNumber::count(s).or_else(|_| ReportedNumber::measured(s)).unwrap();
        ReportedResult {
            id: 0,
            location: Location::in_section(SectionKind::Results, 1, 0),
            raw: String::new(),
            family,
            statistic: ReportedNumber::measured(".31").unwrap(),
            df: df.iter().map(|d| df_number(d)).collect(),
            n: n.map(|n| ReportedNumber::count(n).unwrap()),
            p: PValue::Reported(ReportedNumber::p_value(".03", Relation::Eq).unwrap()),
            effect_size: None,
            ci: None,
            sentence_says_partial: partial,
            source: Source::Unknown,
        }
    }
    fn ctx() -> DocumentContext {
        DocumentContext { thresholds: Vec::new(), one_tailed_at: None, correction_at: None }
    }
    fn applicability(r: &ReportedResult) -> Applicability {
        DfMatchesN.applicability(&L0Input { result: r, context: &ctx() })
    }
    fn verdict(r: &ReportedResult) -> L0Verdict {
        let c = ctx();
        let input = L0Input { result: r, context: &c };
        assert_eq!(DfMatchesN.applicability(&input), Applicability::Applicable);
        DfMatchesN.run(&input).outcome
    }

    #[test]
    fn df_equal_to_n_minus_2_is_consistent() {
        // n = 50, so n − 2 = 48 = df.
        assert_eq!(verdict(&result(Family::R, &["48"], Some("50"), false)), L0Verdict::Consistent(None));
    }

    #[test]
    fn df_not_equal_to_n_minus_2_is_inconsistent() {
        // n = 50, n − 2 = 48; df printed 49 is the point 49.
        assert_eq!(
            verdict(&result(Family::R, &["49"], Some("50"), false)),
            L0Verdict::Inconsistent { conclusion_changes: None }
        );
        // df = n, the commonest slip, too.
        assert_eq!(
            verdict(&result(Family::R, &["50"], Some("50"), false)),
            L0Verdict::Inconsistent { conclusion_changes: None }
        );
    }

    #[test]
    fn a_fractional_df_is_its_interval() {
        // 47.6 is [47.55, 47.65]: 48 is outside. 48.0 is [47.95, 48.05]: inside.
        assert_eq!(
            verdict(&result(Family::R, &["47.6"], Some("50"), false)),
            L0Verdict::Inconsistent { conclusion_changes: None }
        );
        assert_eq!(verdict(&result(Family::R, &["48.0"], Some("50"), false)), L0Verdict::Consistent(None));
    }

    #[test]
    fn t_f_and_chi_square_are_not_applicable() {
        let na = Applicability::NotApplicable(NotApplicableReason::DfNotFixedByN);
        assert_eq!(applicability(&result(Family::T, &["48"], Some("50"), false)), na);
        assert_eq!(applicability(&result(Family::F, &["1", "48"], Some("50"), false)), na);
        // χ2(1, N = 226): the reader does give a chi-square its n, and still no.
        assert_eq!(applicability(&result(Family::ChiSquare, &["1"], Some("226"), false)), na);
        assert_eq!(applicability(&result(Family::Z, &[], Some("50"), false)), na);
    }

    #[test]
    fn an_r_without_n_in_its_unit_is_not_applicable() {
        assert_eq!(
            applicability(&result(Family::R, &["48"], None, false)),
            Applicability::NotApplicable(NotApplicableReason::NoSampleSizeInUnit)
        );
    }

    #[test]
    fn a_partial_correlation_is_not_applicable() {
        // r(47) with n = 50 controlling for one variable: 50 − 2 − 1 = 47. As
        // a plain r it would read as Inconsistent.
        assert_eq!(
            applicability(&result(Family::R, &["47"], Some("50"), true)),
            Applicability::NotApplicable(NotApplicableReason::PartialCorrelation)
        );
    }

    /// The premise the module header states: the reader returns no r with an
    /// n, so this method is inert on manuscripts until the reader changes.
    #[test]
    fn the_reader_gives_no_r_an_n_today() {
        use crate::extract::extract_from_text;
        use crate::stats_l0::result::{read_results, Reason, TableStructure};
        let ex = extract_from_text("A Title\n\nResults\nScores correlated, r(48, N = 50) = .31, p = .03.\n");
        let reading = read_results(&ex, TableStructure::NotAvailable);
        assert!(reading.results.is_empty());
        assert_eq!(reading.not_read[0].reason, Reason::MalformedDegreesOfFreedom);
    }

    #[test]
    fn the_check_is_labelled() {
        let r = result(Family::R, &["48"], Some("50"), false);
        let c = ctx();
        let check = DfMatchesN.run(&L0Input { result: &r, context: &c });
        assert_eq!(check.provenance, Provenance::GaplyArithmetic);
        assert_eq!(check.computed[0], ("n_minus_2", Computed::Exact(Rational::from_int(48))));
    }
}
