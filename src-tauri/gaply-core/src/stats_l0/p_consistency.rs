//! **`p_consistency`: can the printed statistic, its df and its p-value all be
//! right at once?**
//!
//! Every printed number is the interval its digits allow. The statistic's
//! interval, at each end and at each end of any fractional df, gives a range of
//! p-values through [`super::pvalue`]; that range is widened outward by
//! [`ENGINE_P_RELATIVE_MARGIN`] (engine error only: rounding is already in the
//! intervals) and compared with the reported p through
//! [`ReportedNumber::overlaps`]. The first matching rule wins:
//!
//! - **a.** The ranges overlap: `Consistent`.
//! - **b.** t, r or z only: half the recomputed range overlaps. `Consistent`
//!   noted `OneTailedStated` when the document says one-tailed, otherwise
//!   `Concern(OneTailedUnstated)`.
//! - **floor.** The reported p is `=`, its printed value is a power of ten at
//!   .001 or below, and the whole widened range lies below the printed
//!   interval: `Consistent` noted `FloorPrintedAsEquality` (SPSS and similar
//!   print their display floor as an equality; the true p is smaller still).
//!   Before the two-tailed F rule: it is the more specific reading, and the
//!   conclusion cannot change.
//! - **two-tailed F.** F with df1 of 2 or more only: the widened two-tailed
//!   (variance-ratio) range overlaps: `Concern(FTwoTailed)`. F(1, ·) is a
//!   squared t and never a variance ratio. No "stated" upgrade: papers say
//!   "two-tailed" about their t-tests. Known cost: an ANOVA F with df1 of 2
//!   or more printed with twice its p is a Concern, not an Inconsistent.
//! - **c.** The reported p is larger than the recomputed range and the document
//!   names a correction for multiple testing: `CouldNotCheck(AdjustedP)`.
//! - **d.** Otherwise `Inconsistent { conclusion_changes }`.
//!
//! A result printed `ns` is `Consistent` when the recomputed p can reach alpha.
//! An F with df1 of 2 or more whose upper-tail p cannot but whose two-tailed p
//! can is `Concern(FTwoTailed)`; anything else is `Inconsistent`. Inputs that do not
//! define a p-value, and any comparison `overlaps` declines, are
//! `CouldNotCheck(InvalidInput)`.
//!
//! # Corners, not only the statistic's ends
//!
//! A fractional df (Welch's 28.4) is an interval too, so the p-value is taken
//! at every combination of the statistic's ends and each df's ends, and the
//! range is their minimum and maximum. An integer df is a point and adds no
//! corners. Each p function is monotone in its statistic; in df it is monotone
//! for t and chi-square, and the F and t intervals a manuscript prints are a
//! few hundredths wide.
//!
//! # Alpha
//!
//! From Step 3's distinct stated thresholds: one value is used; none means .05,
//! recorded as `alpha_defaulted`; more than one leaves `conclusion_changes`
//! unassessed (`None`) and records every value.

use super::input::{DocumentContext, L0Input};
use super::method::{
    digest_hex, Applicability, CheckNote, Computed, ConcernReason, CouldNotCheckReason, L0Verdict, Level,
    MethodCheck, Provenance, ToleranceRule, VerificationMethod,
};
use super::number::{exact_cmp, ReportedNumber, Relation};
use super::pvalue::{self, ENGINE_P_RELATIVE_MARGIN};
use super::result::{Family, PValue, ReportedResult};
use crate::equation::rational::Rational;

pub const ID: &str = "p_consistency";

/// The engine under the method: [`pvalue::ENGINE_VERSION`].
pub const ENGINE_VERSION: &str = pvalue::ENGINE_VERSION;

pub const TOLERANCE: ToleranceRule = ToleranceRule::RelativeOutward { margin: ENGINE_P_RELATIVE_MARGIN, cap: 1.0 };

/// Recomputing p from the printed statistic and df is statcheck's method. The
/// departures from statcheck are Gaply's and are pinned in
/// `tests/l0_statcheck_departures.rs`.
pub const PROVENANCE: Provenance = Provenance::Published {
    reference: "Nuijten, Hartgerink, van Assen, Epskamp and Wicherts 2016, Behavior Research Methods 48(4)",
};

/// The p-consistency method. Applicable to every [`Family`] the reader reads.
#[derive(Debug, Clone, Copy)]
pub struct PConsistency;

impl<'a> VerificationMethod<L0Input<'a>> for PConsistency {
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
        match input.result.family {
            Family::T | Family::F | Family::ChiSquare | Family::R | Family::Z => Applicability::Applicable,
        }
    }
    fn run(&self, input: &L0Input<'a>) -> MethodCheck {
        let mut computed = Vec::new();
        let outcome = evaluate(input.result, input.context, &mut computed);
        MethodCheck {
            method_id: ID,
            provenance: PROVENANCE,
            engine_version: ENGINE_VERSION,
            inputs_digest: digest(input.result, input.context),
            computed,
            tolerance: TOLERANCE,
            outcome,
        }
    }
}

const INVALID: L0Verdict = L0Verdict::CouldNotCheck(CouldNotCheckReason::InvalidInput);

enum Alpha {
    One(Rational),
    Several(Vec<Rational>),
}

fn alpha_of(ctx: &DocumentContext, computed: &mut Vec<(&'static str, Computed)>) -> Alpha {
    let values: Vec<Rational> = ctx.thresholds.iter().map(ReportedNumber::value).collect();
    match values.len() {
        0 => {
            let a = Rational::new(5, 100).expect("1/20");
            computed.push(("alpha", Computed::Exact(a)));
            computed.push(("alpha_defaulted", Computed::Flag(true)));
            Alpha::One(a)
        }
        1 => {
            computed.push(("alpha", Computed::Exact(values[0])));
            computed.push(("alpha_defaulted", Computed::Flag(false)));
            Alpha::One(values[0])
        }
        _ => {
            computed.push(("alpha_values", Computed::ExactList(values.clone())));
            computed.push(("alpha_defaulted", Computed::Flag(false)));
            Alpha::Several(values)
        }
    }
}

fn evaluate(r: &ReportedResult, ctx: &DocumentContext, computed: &mut Vec<(&'static str, Computed)>) -> L0Verdict {
    let alpha = alpha_of(ctx, computed);
    let Some((p_lo, p_hi)) = recomputed_range(r) else { return INVALID };
    computed.push(("recomputed_p", Computed::Range { lo: p_lo, hi: p_hi }));
    let (lo, hi) = widen(p_lo, p_hi);
    computed.push(("compared_p", Computed::Range { lo, hi }));

    let reported = match &r.p {
        PValue::Reported(p) => p,
        PValue::NotSignificant => {
            let v = not_significant(hi, &alpha);
            if is_variance_ratio_candidate(r) && matches!(v, L0Verdict::Inconsistent { .. }) {
                let Some((t_lo, t_hi)) = f_two_tailed_range(r) else { return INVALID };
                let (w_lo, w_hi) = widen(t_lo, t_hi);
                computed.push(("two_tailed_p", Computed::Range { lo: w_lo, hi: w_hi }));
                if not_significant(w_hi, &alpha) == L0Verdict::Consistent(None) {
                    return L0Verdict::Concern(ConcernReason::FTwoTailed);
                }
            }
            return v;
        }
    };

    // a. The ranges overlap.
    match reported.overlaps(lo, hi, 0.0) {
        None => return INVALID,
        Some(true) => return L0Verdict::Consistent(None),
        Some(false) => {}
    }

    // b. Two-tailed families only: consistent as a one-tailed test.
    if matches!(r.family, Family::T | Family::R | Family::Z) {
        let (h_lo, h_hi) = widen(p_lo / 2.0, p_hi / 2.0);
        computed.push(("one_tailed_p", Computed::Range { lo: h_lo, hi: h_hi }));
        match reported.overlaps(h_lo, h_hi, 0.0) {
            None => return INVALID,
            Some(true) => {
                if let Some(at) = ctx.one_tailed_at {
                    // The outcome rests on this mention: record where it is.
                    computed.push(("one_tailed_stated_at", Computed::Position(at)));
                    return L0Verdict::Consistent(Some(CheckNote::OneTailedStated));
                }
                return L0Verdict::Concern(ConcernReason::OneTailedUnstated);
            }
            Some(false) => {}
        }
    }

    // The floor rule: `p = .0001` where the true p is smaller still.
    if reported.relation() == Relation::Eq
        && is_power_of_ten_floor(reported.value())
        && hi < reported.interval().lo.to_f64()
    {
        return L0Verdict::Consistent(Some(CheckNote::FloorPrintedAsEquality));
    }

    // The two-tailed F rule: consistent as a variance-ratio test. df1 >= 2 only.
    if is_variance_ratio_candidate(r) {
        let Some((t_lo, t_hi)) = f_two_tailed_range(r) else { return INVALID };
        let (w_lo, w_hi) = widen(t_lo, t_hi);
        computed.push(("two_tailed_p", Computed::Range { lo: w_lo, hi: w_hi }));
        match reported.overlaps(w_lo, w_hi, 0.0) {
            None => return INVALID,
            Some(true) => return L0Verdict::Concern(ConcernReason::FTwoTailed),
            Some(false) => {}
        }
    }

    // c. Larger than recomputed, with a correction named. Having failed (a),
    // the reported p lies wholly above or wholly below [lo, hi]; it is above
    // exactly when it reaches [hi, 1].
    match reported.overlaps(hi, 1.0, 0.0) {
        None => return INVALID,
        Some(true) => {
            if let Some(at) = ctx.correction_at {
                computed.push(("correction_named_at", Computed::Position(at)));
                return L0Verdict::CouldNotCheck(CouldNotCheckReason::AdjustedP);
            }
        }
        Some(false) => {}
    }

    // d.
    L0Verdict::Inconsistent { conclusion_changes: conclusion_changes(reported, lo, hi, &alpha) }
}

/// Rule 6: `ns` claims p above alpha. With several thresholds the smallest is
/// used, so `ns` against any stated alpha is accepted.
fn not_significant(hi: f64, alpha: &Alpha) -> L0Verdict {
    let (a, assessed) = match alpha {
        Alpha::One(a) => (a.to_f64(), true),
        Alpha::Several(v) => (v.iter().map(|a| a.to_f64()).fold(f64::INFINITY, f64::min), false),
    };
    if hi >= a {
        L0Verdict::Consistent(None)
    } else {
        // The reported side is above alpha by definition; the recomputed range
        // is wholly below it.
        L0Verdict::Inconsistent { conclusion_changes: assessed.then_some(true) }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Side {
    Below,
    Above,
}

/// Which side of `a` the reported p lies on, if it lies wholly on one. A
/// value that can equal `a` is on neither: `p = .05` against .05 is never a
/// side. `Err` when the exact comparison overflows.
fn reported_side(p: &ReportedNumber, a: Rational) -> Result<Option<Side>, ()> {
    use std::cmp::Ordering::{Greater, Less};
    let iv = p.interval();
    let c = |x: Rational| exact_cmp(x, a).ok_or(());
    Ok(match p.relation() {
        Relation::Eq if c(iv.hi)? == Less => Some(Side::Below),
        Relation::Eq if c(iv.lo)? == Greater => Some(Side::Above),
        Relation::Eq => None,
        Relation::Lt => (c(iv.hi)? != Greater).then_some(Side::Below),
        Relation::Le => (c(iv.hi)? == Less).then_some(Side::Below),
        Relation::Gt => (c(iv.lo)? != Less).then_some(Side::Above),
        Relation::Ge => (c(iv.lo)? == Greater).then_some(Side::Above),
    })
}

/// Rule 5. `Some(true)` only when the reported p and the whole recomputed
/// range fall on opposite sides of alpha.
fn conclusion_changes(reported: &ReportedNumber, lo: f64, hi: f64, alpha: &Alpha) -> Option<bool> {
    let Alpha::One(a) = alpha else { return None };
    let reported = reported_side(reported, *a).ok()?;
    let af = a.to_f64();
    let recomputed = if hi < af {
        Some(Side::Below)
    } else if lo > af {
        Some(Side::Above)
    } else {
        None
    };
    Some(matches!((reported, recomputed), (Some(x), Some(y)) if x != y))
}

/// Widen outward by the engine margin; the upper end is capped at 1.
fn widen(lo: f64, hi: f64) -> (f64, f64) {
    (lo * (1.0 - ENGINE_P_RELATIVE_MARGIN), (hi * (1.0 + ENGINE_P_RELATIVE_MARGIN)).min(1.0))
}

/// An F the two-tailed rule may read as a variance ratio: df1 of 2 or more,
/// exactly. F(1, ·) is a squared t, the commonest ANOVA form, and a variance
/// ratio with one numerator df would need a sample of two.
fn is_variance_ratio_candidate(r: &ReportedResult) -> bool {
    r.family == Family::F && r.df.first().is_some_and(|d| d.value().numer() >= 2 * d.value().denom())
}

/// `1/10^k` with `k >= 3`: .001, .0001, … Exact, on the reduced fraction.
fn is_power_of_ten_floor(v: Rational) -> bool {
    if v.numer() != 1 {
        return false;
    }
    let mut d = v.denom();
    let mut k = 0;
    while d > 1 && d % 10 == 0 {
        d /= 10;
        k += 1;
    }
    d == 1 && k >= 3
}

/// The statistic's two ends (sizes for t, r and z; clamped at 0 for F and
/// chi-square) and every combination of the df's ends. `None` when the printed
/// statistic itself defines no p-value.
///
/// `Rational::to_f64` may move an endpoint by a few ulps; the resulting change
/// in p is orders of magnitude below the engine margin.
fn grid(r: &ReportedResult) -> Option<([f64; 2], Vec<Vec<f64>>)> {
    let s = r.statistic.interval();
    let (mut s_lo, mut s_hi) = (s.lo.to_f64(), s.hi.to_f64());
    let value = r.statistic.value();
    match r.family {
        Family::T | Family::R | Family::Z => {
            if r.family == Family::R && value.numer().abs() >= value.denom() {
                return None; // |r| printed at or beyond 1
            }
            // |statistic|: an interval straddling zero starts at zero.
            let (a, b) = (s_lo.abs(), s_hi.abs());
            (s_lo, s_hi) = if s_lo <= 0.0 && s_hi >= 0.0 { (0.0, a.max(b)) } else { (a.min(b), a.max(b)) };
        }
        Family::F | Family::ChiSquare => {
            if value.is_negative() {
                return None;
            }
            // A non-negative printed value whose rounding interval dips below 0.
            s_lo = s_lo.max(0.0);
        }
    }

    let df_ends: Vec<[f64; 2]> = r.df.iter().map(|d| [d.interval().lo.to_f64(), d.interval().hi.to_f64()]).collect();
    let mut corners: Vec<Vec<f64>> = vec![Vec::new()];
    for ends in &df_ends {
        corners = corners.iter().flat_map(|c| ends.iter().map(move |e| [c.as_slice(), &[*e]].concat())).collect();
    }
    Some(([s_lo, s_hi], corners))
}

/// The p-values the printed numbers allow: `(min, max)` over every corner.
/// `None` when the inputs do not define a p-value.
fn recomputed_range(r: &ReportedResult) -> Option<(f64, f64)> {
    let (stats, corners) = grid(r)?;
    let p_at = |stat: f64, df: &[f64]| -> Option<f64> {
        match r.family {
            Family::T => pvalue::t_two_tailed(stat, *df.first()?),
            Family::F => pvalue::f_upper(stat, *df.first()?, *df.get(1)?),
            Family::ChiSquare => pvalue::chi_square_upper(stat, *df.first()?),
            // An r interval reaching 1 from a printed |r| below 1: p tends to 0.
            Family::R if stat >= 1.0 => {
                pvalue::r_two_tailed(0.0, *df.first()?)?;
                Some(0.0)
            }
            Family::R => pvalue::r_two_tailed(stat, *df.first()?),
            Family::Z => pvalue::z_two_tailed(stat),
        }
    };

    let (mut lo, mut hi) = (f64::INFINITY, f64::NEG_INFINITY);
    for stat in stats {
        for df in &corners {
            let p = p_at(stat, df)?;
            lo = lo.min(p);
            hi = hi.max(p);
        }
    }
    (lo.is_finite() && hi.is_finite()).then_some((lo, hi))
}

/// The two-tailed F p-values the printed numbers allow, over the same corners.
/// The two-tailed p is not monotone in F: it rises to 1 at the median and
/// falls again. When the corners lie on both sides of the median (the upper
/// tail is the smaller at one and the lower at another), the range reaches 1.
fn f_two_tailed_range(r: &ReportedResult) -> Option<(f64, f64)> {
    let (stats, corners) = grid(r)?;
    let (mut lo, mut hi) = (f64::INFINITY, f64::NEG_INFINITY);
    let (mut upper_smaller, mut lower_smaller) = (false, false);
    for stat in stats {
        for df in &corners {
            let (d1, d2) = (*df.first()?, *df.get(1)?);
            let (u, l) = (pvalue::f_upper(stat, d1, d2)?, pvalue::f_lower(stat, d1, d2)?);
            upper_smaller |= u <= l;
            lower_smaller |= l <= u;
            let p = pvalue::f_two_tailed(stat, d1, d2)?;
            lo = lo.min(p);
            hi = hi.max(p);
        }
    }
    if upper_smaller && lower_smaller {
        hi = 1.0;
    }
    (lo.is_finite() && hi.is_finite()).then_some((lo, hi))
}

/// SHA-256 over a canonical encoding of the typed inputs: exact rationals,
/// relations and flags. No printed text enters it.
fn digest(r: &ReportedResult, ctx: &DocumentContext) -> String {
    let q = |x: Rational| format!("{}/{}", x.numer(), x.denom());
    let iv = ReportedNumber::canonical;
    let mut s = format!("{ID}\nfamily={:?}\nstatistic={}\n", r.family, iv(&r.statistic));
    for d in &r.df {
        s.push_str(&format!("df={}\n", iv(d)));
    }
    match &r.p {
        PValue::Reported(p) => s.push_str(&format!("p={}\n", iv(p))),
        PValue::NotSignificant => s.push_str("p=ns\n"),
    }
    for t in &ctx.thresholds {
        s.push_str(&format!("threshold={}\n", q(t.value())));
    }
    s.push_str(&format!("one_tailed_at={:?}\ncorrection_at={:?}\n", ctx.one_tailed_at, ctx.correction_at));
    digest_hex(&s)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::extract::extract_from_text;
    use crate::stats_l0::method::TextPosition;
    use crate::stats_l0::result::{read_results, TableStructure};

    /// Read `text` as a whole document and run the method on its only result.
    fn check(text: &str) -> MethodCheck {
        let ex = extract_from_text(&format!("A Title\n\n{text}\n"));
        let reading = read_results(&ex, TableStructure::NotAvailable);
        assert_eq!(reading.results.len(), 1, "{text:?}: {:?}", reading.not_read);
        let ctx = DocumentContext::read(&ex, &reading);
        let input = L0Input { result: &reading.results[0], context: &ctx };
        assert_eq!(PConsistency.applicability(&input), Applicability::Applicable);
        PConsistency.run(&input)
    }
    fn verdict(text: &str) -> L0Verdict {
        check(text).outcome
    }
    fn results(body: &str) -> String {
        format!("Results\n{body}")
    }
    fn computed<'c>(c: &'c MethodCheck, name: &str) -> Option<&'c Computed> {
        c.computed.iter().find(|(n, _)| *n == name).map(|(_, v)| v)
    }
    /// The index of the Results section in a document built by `check`.
    fn results_section() -> usize {
        let ex = extract_from_text(&format!("A Title\n\n{}\n", results("x")));
        ex.sections.iter().position(|s| s.kind == crate::extract::SectionKind::Results).expect("Results")
    }
    fn q(s: &str) -> Rational {
        Rational::parse_decimal(s).unwrap().0
    }

    // ---- the named cases ----

    /// The statcheck paper's own example. t(28) = 2.0 is any t in [1.95, 2.05],
    /// whose p runs from .0498 to .0613, and .0498 is below .05. A point
    /// comparison at t = 2.0 (p = .0553) would wrongly flag it.
    #[test]
    fn the_statcheck_example_t28_2_0_p_below_05_is_consistent() {
        let c = check(&results("The groups differed, t(28) = 2.0, p < .05."));
        assert_eq!(c.outcome, L0Verdict::Consistent(None));
        let Some(Computed::Range { lo, .. }) = computed(&c, "recomputed_p") else { panic!("{c:?}") };
        assert!(*lo < 0.05 && *lo > 0.0498, "{lo}");
    }

    #[test]
    fn t28_2_10_with_p_01_is_inconsistent_without_a_conclusion_change() {
        // p for [2.095, 2.105] is [.0444, .0453]; .01 is [.005, .015]. Both
        // below .05, so the conclusion stands.
        assert_eq!(
            verdict(&results("The groups differed, t(28) = 2.10, p = .01.")),
            L0Verdict::Inconsistent { conclusion_changes: Some(false) }
        );
    }

    #[test]
    fn a_reported_p_on_the_other_side_of_alpha_is_a_conclusion_change() {
        // t(28) = 1.20 gives p near .24: not significant. Reported .01: significant.
        assert_eq!(
            verdict(&results("The groups differed, t(28) = 1.20, p = .01.")),
            L0Verdict::Inconsistent { conclusion_changes: Some(true) }
        );
    }

    #[test]
    fn two_stated_thresholds_leave_the_conclusion_unassessed_and_record_both() {
        let c = check(&results(
            "The significance level was p < .05.\n\nThe alpha level was p < .01 for follow-ups.\n\n\
             The groups differed, t(28) = 1.20, p = .01.",
        ));
        assert_eq!(c.outcome, L0Verdict::Inconsistent { conclusion_changes: None });
        assert_eq!(computed(&c, "alpha_values"), Some(&Computed::ExactList(vec![q("0.05"), q("0.01")])));
    }

    // ---- rule a ----

    #[test]
    fn rule_a_overlapping_ranges_are_consistent() {
        assert_eq!(verdict(&results("The groups differed, t(28) = 2.10, p = .045.")), L0Verdict::Consistent(None));
        assert_eq!(verdict(&results("Dose mattered, F(2, 70) = 4.48, p = .015.")), L0Verdict::Consistent(None));
        assert_eq!(verdict(&results("Uptake differed, χ2(1, N = 226) = 6.90, p = .009.")), L0Verdict::Consistent(None));
        assert_eq!(verdict(&results("Scores correlated, r(48) = .31, p = .03.")), L0Verdict::Consistent(None));
        assert_eq!(verdict(&results("The test gave z = 2.05, p = .04.")), L0Verdict::Consistent(None));
        // A negative t is read by its size.
        assert_eq!(verdict(&results("The groups differed, t(28) = -2.10, p = .045.")), L0Verdict::Consistent(None));
    }

    #[test]
    fn a_statistic_whose_interval_straddles_zero_reaches_p_one() {
        // t = 0.0 is [-0.05, 0.05], so |t| runs from 0, where p = 1. Taking only
        // the sizes of the two ends would stop at p(0.05) = .96 and miss 1.000.
        let c = check(&results("No difference, t(28) = 0.0, p = 1.000."));
        assert_eq!(c.outcome, L0Verdict::Consistent(None));
        let Some(Computed::Range { hi, .. }) = computed(&c, "recomputed_p") else { panic!() };
        assert_eq!(*hi, 1.0);
    }

    // ---- rule b ----

    #[test]
    fn rule_b_half_the_range_with_one_tailed_stated_is_consistent_with_a_note() {
        // Two-tailed [.0444, .0453]; one-tailed [.0222, .0227]; .023 is [.0225, .0235].
        let c = check(&results("Tests were one-tailed.\n\nThe groups differed, t(28) = 2.10, p = .023."));
        assert_eq!(c.outcome, L0Verdict::Consistent(Some(CheckNote::OneTailedStated)));
        // Where the mention is, by index only: the Results section, paragraph 0.
        let at = TextPosition { section: results_section(), paragraph: Some(0) };
        assert_eq!(computed(&c, "one_tailed_stated_at"), Some(&Computed::Position(at)));
    }

    #[test]
    fn rule_b_half_the_range_without_one_tailed_stated_is_a_concern() {
        let v = verdict(&results("The groups differed, t(28) = 2.10, p = .023."));
        assert_eq!(v, L0Verdict::Concern(ConcernReason::OneTailedUnstated));
    }

    #[test]
    fn rule_b_does_not_apply_to_an_upper_tail_family() {
        // F(1, 28) = 4.41 is t(28) = 2.10 squared: p near .045. Halved it
        // would match .023, but F has no one-tailed reading.
        let v = verdict(&results("A one-tailed test was used. The groups differed, F(1, 28) = 4.41, p = .023."));
        assert_eq!(v, L0Verdict::Inconsistent { conclusion_changes: Some(false) });
    }

    // ---- the floor rule (regressions from PMC13101904) ----

    #[test]
    fn a_software_floor_printed_as_an_equality_is_consistent_with_a_note() {
        // F(1, 62) = 78.22 has p near 1.4e-12; "p = 0.0001" is SPSS's floor.
        assert_eq!(
            verdict(&results("Old participants were slower, F(1, 62) = 78.22, p = 0.0001.")),
            L0Verdict::Consistent(Some(CheckNote::FloorPrintedAsEquality))
        );
    }

    #[test]
    fn a_true_p_at_the_floor_is_consistent_through_rule_a_without_a_note() {
        // F(1, 62) = 17.39 has p in [9.61e-5, 9.65e-5], inside [.00005, .00015].
        assert_eq!(verdict(&results("Errors rose, F(1, 62) = 17.39, p = 0.0001.")), L0Verdict::Consistent(None));
    }

    #[test]
    fn a_p_that_is_not_a_power_of_ten_floor_stays_inconsistent() {
        // t(28) = 6.00 has p near 1.9e-6. Neither .01 nor .002 is a floor.
        for body in ["The groups differed, t(28) = 6.00, p = .01.", "The groups differed, t(28) = 6.00, p = .002."] {
            assert_eq!(verdict(&results(body)), L0Verdict::Inconsistent { conclusion_changes: Some(false) }, "{body}");
        }
    }

    #[test]
    fn only_a_power_of_ten_at_or_below_001_is_a_floor() {
        for v in ["0.001", "0.0001", "0.00001"] {
            assert!(is_power_of_ten_floor(q(v)), "{v}");
        }
        for v in ["0.01", "0.1", "0.002", "0.0005", "0.0002", "1"] {
            assert!(!is_power_of_ten_floor(q(v)), "{v}");
        }
    }

    #[test]
    fn a_floor_on_an_f_is_read_as_a_floor_before_any_two_tailed_reading() {
        // F(1, 62) = 19.76: upper p 3.7e-5, below "0.0001" = [.00005, .00015];
        // doubled (7.4e-5) it would sit inside. The floor rule comes first.
        assert_eq!(
            verdict(&results("Old participants were slower, F(1,62) = 19.76, p = 0.0001.")),
            L0Verdict::Consistent(Some(CheckNote::FloorPrintedAsEquality))
        );
    }

    // ---- the two-tailed F rule (regressions from PMC13101904) ----

    /// The df1 limit, pinned. F(1, 57) = 4.20 has upper p [.04491, .04516];
    /// doubled [.08983, .09032], which ".090" = [.0895, .0905] overlaps. With
    /// one numerator df it is a squared t, never a variance ratio.
    #[test]
    fn an_f_with_one_numerator_df_is_never_read_two_tailed() {
        assert_eq!(
            verdict(&results("Dose mattered, F(1, 57) = 4.20, p = .090.")),
            L0Verdict::Inconsistent { conclusion_changes: Some(true) }
        );
        // Nor through rule 6: F(1, 57) = 4.20 printed ns (upper p .045 < .05).
        assert_eq!(
            verdict(&results("Dose mattered, F(1, 57) = 4.20, ns.")),
            L0Verdict::Inconsistent { conclusion_changes: Some(true) }
        );
    }

    #[test]
    fn a_variance_ratio_f_printed_ns_is_a_two_tailed_concern() {
        // F(10, 10) = 3.20: upper p .040 (significant), two-tailed .080 (ns).
        let c = check(&results("The ratios differed, F(10, 10) = 3.20, N.S."));
        assert_eq!(c.outcome, L0Verdict::Concern(ConcernReason::FTwoTailed));
        assert!(computed(&c, "two_tailed_p").is_some());
    }

    #[test]
    fn a_variance_ratio_f_with_its_two_tailed_p_is_a_concern() {
        // F(10, 10) = 6.40: upper .0035, two-tailed .0071, inside [.005, .015].
        assert_eq!(
            verdict(&results("The ratios differed, F(10, 10) = 6.40, p = 0.01.")),
            L0Verdict::Concern(ConcernReason::FTwoTailed)
        );
    }

    #[test]
    fn an_f_wrong_under_both_tails_stays_inconsistent() {
        // F(10, 10) = 8.25: upper .0013, two-tailed .0026; .01 is [.005, .015].
        assert_eq!(
            verdict(&results("The ratios differed, F(10, 10) = 8.25, p = 0.01.")),
            L0Verdict::Inconsistent { conclusion_changes: Some(false) }
        );
        // And "ns" whose two-tailed p is still below alpha.
        assert_eq!(
            verdict(&results("The ratios differed, F(10, 10) = 8.25, ns.")),
            L0Verdict::Inconsistent { conclusion_changes: Some(true) }
        );
    }

    /// The rule's known cost, pinned: a one-way ANOVA F printed with twice its
    /// true p reads as a two-tailed F, so it is a Concern, not an Inconsistent.
    /// F(2, 57) = 3.20: upper p [.04797, .04840]; two-tailed [.09593, .09680].
    #[test]
    fn an_anova_f_printed_with_twice_its_p_is_a_concern_not_inconsistent() {
        assert_eq!(
            verdict(&results("Dose mattered, F(2, 57) = 3.20, p = .096.")),
            L0Verdict::Concern(ConcernReason::FTwoTailed)
        );
    }

    #[test]
    fn a_two_tailed_f_range_that_crosses_the_median_reaches_one() {
        // F(10, 10) = 1.0 is [0.95, 1.05], around the median 1: upper p near .5,
        // two-tailed reaching 1, which ".99" is near.
        let c = check(&results("The ratios matched, F(10, 10) = 1.0, p = .99."));
        assert_eq!(c.outcome, L0Verdict::Concern(ConcernReason::FTwoTailed));
        let Some(Computed::Range { hi, .. }) = computed(&c, "two_tailed_p") else { panic!("{c:?}") };
        assert_eq!(*hi, 1.0);
    }

    // ---- rule c ----

    #[test]
    fn rule_c_a_larger_p_with_a_correction_named_could_not_be_checked() {
        let c = check(&results("P-values were Bonferroni corrected.\n\nThe groups differed, t(28) = 2.10, p = .27."));
        assert_eq!(c.outcome, L0Verdict::CouldNotCheck(CouldNotCheckReason::AdjustedP));
        let at = TextPosition { section: results_section(), paragraph: Some(0) };
        assert_eq!(computed(&c, "correction_named_at"), Some(&Computed::Position(at)));
    }

    #[test]
    fn a_flag_that_did_not_decide_the_outcome_is_not_recorded() {
        // A correction is named, but the reported p is smaller: rule d, and the
        // correction played no part.
        let c = check(&results("P-values were Bonferroni corrected.\n\nThe groups differed, t(28) = 2.10, p = .01."));
        assert!(matches!(c.outcome, L0Verdict::Inconsistent { .. }));
        assert_eq!(computed(&c, "correction_named_at"), None);
        assert_eq!(computed(&c, "one_tailed_stated_at"), None);
    }

    #[test]
    fn rule_c_needs_both_a_larger_p_and_a_named_correction() {
        // Larger, no correction named: rule d.
        assert_eq!(
            verdict(&results("The groups differed, t(28) = 2.10, p = .27.")),
            L0Verdict::Inconsistent { conclusion_changes: Some(true) }
        );
        // Correction named, but the reported p is SMALLER: rule d.
        assert_eq!(
            verdict(&results("After Bonferroni correction the groups differed, t(28) = 2.10, p = .01.")),
            L0Verdict::Inconsistent { conclusion_changes: Some(false) }
        );
    }

    // ---- rule d is every Inconsistent above; rule 5 ----

    #[test]
    fn rule_5_no_stated_threshold_defaults_alpha_and_says_so() {
        let c = check(&results("The groups differed, t(28) = 1.20, p = .01."));
        assert_eq!(computed(&c, "alpha"), Some(&Computed::Exact(q("0.05"))));
        assert_eq!(computed(&c, "alpha_defaulted"), Some(&Computed::Flag(true)));
    }

    #[test]
    fn rule_5_one_stated_threshold_is_the_alpha() {
        // Alpha .01: reported .02 and recomputed p near .068 are both above it,
        // so no conclusion changes. At the default .05 it would.
        let c = check(&results("Significance was set at p < .01.\n\nThe groups differed, t(28) = 1.90, p = .02."));
        assert_eq!(c.outcome, L0Verdict::Inconsistent { conclusion_changes: Some(false) });
        assert_eq!(computed(&c, "alpha"), Some(&Computed::Exact(q("0.01"))));
        assert_eq!(computed(&c, "alpha_defaulted"), Some(&Computed::Flag(false)));
    }

    #[test]
    fn rule_5_p_equal_to_alpha_is_never_a_conclusion_change() {
        // ".05" is [.045, .055], which holds .05: on neither side, whichever
        // side the recomputed p is on. t(28) = 4.00 gives p near .0004 (below);
        // t(28) = 1.20 gives p near .24 (above).
        for body in ["The groups differed, t(28) = 4.00, p = .05.", "The groups differed, t(28) = 1.20, p = .05."] {
            assert_eq!(verdict(&results(body)), L0Verdict::Inconsistent { conclusion_changes: Some(false) }, "{body}");
        }
    }

    // ---- rule 6 ----

    #[test]
    fn rule_6_ns_is_consistent_when_the_recomputed_p_reaches_alpha() {
        assert_eq!(verdict(&results("No difference, t(28) = 1.20, ns.")), L0Verdict::Consistent(None));
    }

    #[test]
    fn rule_6_ns_is_inconsistent_when_the_recomputed_p_is_wholly_below_alpha() {
        assert_eq!(
            verdict(&results("No difference, t(28) = 4.00, ns.")),
            L0Verdict::Inconsistent { conclusion_changes: Some(true) }
        );
    }

    // ---- rule 7 ----

    #[test]
    fn rule_7_inputs_that_define_no_p_could_not_be_checked() {
        for body in [
            "The groups differed, t(0) = 2.10, p = .04.",
            "Scores correlated, r(48) = 1.00, p = .001.",
            "Scores correlated, r(48) = -1.20, p = .001.",
            "Dose mattered, F(1, 20) = -2.00, p = .04.",
            "Uptake differed, χ2(1) = -1.00, p = .04.",
        ] {
            assert_eq!(verdict(&results(body)), INVALID, "{body}");
        }
    }

    #[test]
    fn rule_7_a_comparison_overlaps_declines_could_not_be_checked() {
        // `overlaps` declines a range that is not finite. No printed input
        // reaches this, so the check is made on the comparison directly.
        let p = ReportedNumber::p_value(".04", Relation::Eq).unwrap();
        assert_eq!(p.overlaps(f64::NAN, 0.5, 0.0), None);
        let ctx = DocumentContext { thresholds: Vec::new(), one_tailed_at: None, correction_at: None };
        let mut c = Vec::new();
        let ex = extract_from_text("A Title\n\nResults\nThe groups differed, t(28) = 2.10, p = .04.\n");
        let mut r = read_results(&ex, TableStructure::NotAvailable).results.remove(0);
        r.statistic = ReportedNumber::measured("2.10").unwrap();
        assert_ne!(evaluate(&r, &ctx, &mut c), INVALID);
        // A df that is a point at zero reaches the engine and is declined there.
        r.df = vec![ReportedNumber::count("0").unwrap()];
        assert_eq!(evaluate(&r, &ctx, &mut Vec::new()), INVALID);
    }

    // ---- determinism and the record ----

    #[test]
    fn the_same_input_gives_a_byte_identical_check() {
        for body in [
            "The groups differed, t(28) = 2.10, p = .01.",
            "No difference, t(28) = 1.20, ns.",
            "Variances differed, t(28.4) = 2.10, p = .045.",
        ] {
            let a = format!("{:?}", check(&results(body)));
            let b = format!("{:?}", check(&results(body)));
            assert_eq!(a, b, "{body}");
        }
    }

    #[test]
    fn the_check_records_its_rule_and_never_names_a_wrong_number() {
        let c = check(&results("The groups differed, t(28) = 2.10, p = .01."));
        assert_eq!(c.method_id, "p_consistency");
        assert_eq!(c.tolerance, ToleranceRule::RelativeOutward { margin: 1e-8, cap: 1.0 });
        assert_eq!(c.inputs_digest.len(), 64);
        assert_eq!(c.engine_version, pvalue::ENGINE_VERSION);
        // The record holds computed values and the verdict, nothing that
        // assigns the fault to the statistic, the df or the p.
        let names: Vec<&str> = c.computed.iter().map(|(n, _)| *n).collect();
        assert_eq!(names, vec!["alpha", "alpha_defaulted", "recomputed_p", "compared_p", "one_tailed_p"]);
    }

    #[test]
    fn the_compared_range_is_the_recomputed_one_widened_by_the_engine_margin() {
        assert_eq!(widen(0.04, 0.05), (0.04 * (1.0 - 1e-8), 0.05 * (1.0 + 1e-8)));
        assert_eq!(widen(0.5, 1.0).1, 1.0, "capped at 1");
        let c = check(&results("The groups differed, t(28) = 2.10, p = .045."));
        let (Some(Computed::Range { lo, hi }), Some(Computed::Range { lo: wl, hi: wh })) =
            (computed(&c, "recomputed_p"), computed(&c, "compared_p"))
        else {
            panic!("{c:?}")
        };
        assert_eq!((*wl, *wh), widen(*lo, *hi));
        assert!(wl < lo && wh > hi);
    }

    #[test]
    fn a_fractional_df_widens_the_recomputed_range() {
        // t(28.4) is df in [28.35, 28.45]: four corners, a range at least as
        // wide as either integer neighbour's would give at its own df.
        let c = check(&results("Variances differed, t(28.4) = 2.10, p = .045."));
        assert_eq!(c.outcome, L0Verdict::Consistent(None));
        let Some(Computed::Range { lo, hi }) = computed(&c, "recomputed_p") else { panic!() };
        let at = |t: f64, df: f64| pvalue::t_two_tailed(t, df).unwrap();
        assert_eq!(*lo, at(2.105, 28.45));
        assert_eq!(*hi, at(2.095, 28.35));
    }
}
