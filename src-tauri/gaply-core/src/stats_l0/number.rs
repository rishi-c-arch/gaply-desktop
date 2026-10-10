//! **`ReportedNumber`: a number exactly as a paper printed it.**
//!
//! A printed number is an INTERVAL, never a point. `2.31` is every value that
//! rounds to `2.31`, so it is `[2.305, 2.315]`, closed at both ends so that
//! round-half-up, round-half-even and round-half-down all land inside it. A
//! check that treated `2.31` as the point 2.31 would call an author wrong for
//! rounding, which is the false `Inconsistent` this verifier exists not to
//! produce.
//!
//! # Read from the string, never through a float
//!
//! The value is built by [`Rational::parse_decimal`] and the interval by
//! [`Interval::of_printed`]: exact rationals throughout. A strict lexer stands
//! in front of them because `parse_decimal` is permissive on purpose (it drops
//! thousands separators for the equation engine), and here a comma is
//! ambiguous between a thousands separator and a decimal comma, so it is not
//! read at all.
//!
//! Bounds become `f64` in exactly two places, [`ReportedNumber::contains`] and
//! [`ReportedNumber::overlaps`], at the moment a computed value is compared.
//! This module never calls `Rational::cmp_val`, `Interval::new`,
//! `Interval::contains_zero` or `Interval::overlaps`, because each of those can
//! fall back to `f64` on overflow.
//!
//! # Not read is not an error
//!
//! A string this lexer does not accept is [`NumberNotRead`] with a typed reason.
//! Detection accepts a number only on positive evidence of its form; anything
//! else is counted as not read, and low coverage is the acceptable cost.
//!
//! # Measured or counted
//!
//! [`ReportedNumber::measured`] widens every value by half a unit in its last
//! printed place, integers included: a statistic printed `3` is `[2.5, 3.5]`.
//! [`ReportedNumber::count`] is for integers that were counted, not rounded
//! (n, the `12` in "12 of 40", an integer df) and is an exact point.
//! [`ReportedNumber::p_value`] is the only constructor that turns a printed
//! zero into a bound.

use crate::equation::interval::Interval;
use crate::equation::rational::Rational;

/// How the reported number relates to the quantity it reports.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Relation {
    Eq,
    Lt,
    Le,
    Gt,
    Ge,
}

/// Why a printed string was not read as a number. Never an error: a count of
/// these is coverage, not failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NumberNotRead {
    /// Nothing left after trimming.
    Empty,
    /// A comma anywhere. `1,204` (thousands) and `0,05` (decimal comma) cannot
    /// be told apart from the string, so neither is read.
    Comma,
    /// `12%`. A percentage is a different quantity from its digits.
    Percent,
    /// `1.2e-5`, `1.2×10`.
    ScientificNotation,
    /// `3-5`: a dash after a digit is a range, not a sign.
    Range,
    /// Any space inside the number, U+202F and other Unicode spaces included.
    InternalSpace,
    /// Anything else that is not an optional minus (hyphen, U+2212, or a
    /// leading U+2013), digits and at most one point with digits after it:
    /// `+-1`, `5.`, `007`, `+1`, letters.
    Malformed,
    /// The exact arithmetic cannot hold this many digits.
    TooManyDigits,
    /// [`ReportedNumber::count`] was given a decimal point.
    DecimalInCount,
    /// [`ReportedNumber::count`] was given a minus sign.
    NegativeCount,
    /// [`ReportedNumber::p_value`] was given a sign or a value above 1.
    PValueOutOfRange,
}

/// A number exactly as printed. Fields are private so the interval can only be
/// one its printed digits allow. Deliberately NOT `Serialize`: `raw` is
/// manuscript text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReportedNumber {
    raw: String,
    relation: Relation,
    decimals: u32,
    value: Rational,
    interval: Interval,
}

/// The lexer's accepted form: sign, and the digits on each side of the point.
struct Lexed {
    negative: bool,
    int: String,
    frac: Option<String>,
}

fn is_dash(c: char) -> bool {
    matches!(c, '-' | '\u{2212}' | '\u{2013}' | '\u{2014}')
}

/// The strict lexer. `s` is already trimmed.
fn lex(s: &str) -> Result<Lexed, NumberNotRead> {
    if s.is_empty() {
        return Err(NumberNotRead::Empty);
    }
    if s.contains(',') {
        return Err(NumberNotRead::Comma);
    }
    if s.contains('%') {
        return Err(NumberNotRead::Percent);
    }
    let chars: Vec<char> = s.chars().collect();
    let after_digit = |w: &[char]| w[0].is_ascii_digit() || w[0] == '.';
    if s.contains('×') || chars.windows(2).any(|w| after_digit(w) && matches!(w[1], 'e' | 'E')) {
        return Err(NumberNotRead::ScientificNotation);
    }
    if s.chars().any(char::is_whitespace) {
        return Err(NumberNotRead::InternalSpace);
    }
    if chars
        .windows(2)
        .any(|w| (w[0].is_ascii_digit() || w[0] == '.') && is_dash(w[1]))
    {
        return Err(NumberNotRead::Range);
    }

    // A leading en dash is a minus sign: PDF text extraction often yields one.
    // Between digits it is a range, already refused above.
    let (negative, body) = match s
        .strip_prefix('-')
        .or_else(|| s.strip_prefix('\u{2212}'))
        .or_else(|| s.strip_prefix('\u{2013}'))
    {
        Some(rest) => (true, rest),
        None => (false, s),
    };
    let (int, frac) = match body.split_once('.') {
        Some((i, f)) => (i, Some(f)),
        None => (body, None),
    };
    let all_digits = |t: &str| t.chars().all(|c| c.is_ascii_digit());
    let int_ok = all_digits(int) && (int.len() <= 1 || !int.starts_with('0'));
    let frac_ok = frac.map_or(true, |f| !f.is_empty() && all_digits(f));
    let has_digit = !int.is_empty() || frac.is_some();
    if !(int_ok && frac_ok && has_digit) {
        return Err(NumberNotRead::Malformed);
    }
    Ok(Lexed { negative, int: int.to_string(), frac: frac.map(str::to_string) })
}

impl Lexed {
    /// The exact value and the decimals printed. Only ASCII digits, at most one
    /// leading `-` and one `.` with digits after it reach `parse_decimal`.
    fn exact(&self) -> Result<(Rational, u32), NumberNotRead> {
        let int = if self.int.is_empty() { "0" } else { self.int.as_str() };
        let sign = if self.negative { "-" } else { "" };
        let normalised = match &self.frac {
            Some(f) => format!("{sign}{int}.{f}"),
            None => format!("{sign}{int}"),
        };
        Rational::parse_decimal(&normalised).ok_or(NumberNotRead::TooManyDigits)
    }
}

impl ReportedNumber {
    /// A statistic, estimate, mean or percentage value printed as `raw`, with
    /// relation `Eq`. Always widened by half a unit in the last printed place,
    /// integers included.
    pub fn measured(raw: &str) -> Result<ReportedNumber, NumberNotRead> {
        let raw = raw.trim();
        let (value, decimals) = lex(raw)?.exact()?;
        let interval = Interval::of_printed(value, decimals).ok_or(NumberNotRead::TooManyDigits)?;
        Ok(ReportedNumber { raw: raw.to_string(), relation: Relation::Eq, decimals, value, interval })
    }

    /// An integer that was counted, not rounded: n, an item count, an integer
    /// df. An exact point.
    pub fn count(raw: &str) -> Result<ReportedNumber, NumberNotRead> {
        let raw = raw.trim();
        let lexed = lex(raw)?;
        if lexed.frac.is_some() {
            return Err(NumberNotRead::DecimalInCount);
        }
        if lexed.negative {
            return Err(NumberNotRead::NegativeCount);
        }
        let (value, decimals) = lexed.exact()?;
        Ok(ReportedNumber {
            raw: raw.to_string(),
            relation: Relation::Eq,
            decimals,
            value,
            interval: Interval::point(value),
        })
    }

    /// A p-value printed as `raw` with `relation`.
    ///
    /// - `Eq` with a value printed as zero to its last place (`.000`, `0.00`)
    ///   becomes `Lt` half a unit in that place: `p = .000` is `p < .0005`.
    ///   This rule belongs to p-values only, not to the type.
    /// - Any other `Eq` is widened like [`ReportedNumber::measured`].
    /// - An inequality's printed bound is a threshold the author chose, not a
    ///   rounding, so it is an exact point.
    pub fn p_value(raw: &str, relation: Relation) -> Result<ReportedNumber, NumberNotRead> {
        let raw = raw.trim();
        let lexed = lex(raw)?;
        if lexed.negative {
            return Err(NumberNotRead::PValueOutOfRange);
        }
        let (value, decimals) = lexed.exact()?;
        // value > 1, compared on the reduced fraction (den > 0), exactly.
        if value.numer() > value.denom() {
            return Err(NumberNotRead::PValueOutOfRange);
        }
        let (relation, value, interval) = match relation {
            Relation::Eq if value.is_zero() => {
                let bound = Rational::new(5, 10i128.checked_pow(decimals + 1).ok_or(NumberNotRead::TooManyDigits)?)
                    .ok_or(NumberNotRead::TooManyDigits)?;
                (Relation::Lt, bound, Interval::point(bound))
            }
            Relation::Eq => {
                (Relation::Eq, value, Interval::of_printed(value, decimals).ok_or(NumberNotRead::TooManyDigits)?)
            }
            r => (r, value, Interval::point(value)),
        };
        Ok(ReportedNumber { raw: raw.to_string(), relation, decimals, value, interval })
    }

    pub fn raw(&self) -> &str {
        &self.raw
    }
    pub fn relation(&self) -> Relation {
        self.relation
    }
    pub fn decimals(&self) -> u32 {
        self.decimals
    }
    pub fn value(&self) -> Rational {
        self.value
    }
    pub fn interval(&self) -> Interval {
        self.interval
    }

    /// The typed value for an inputs digest: relation and exact interval, as
    /// `Eq[num/den,num/den]`. No printed text.
    pub fn canonical(&self) -> String {
        let q = |x: Rational| format!("{}/{}", x.numer(), x.denom());
        format!("{:?}[{},{}]", self.relation, q(self.interval.lo), q(self.interval.hi))
    }

    /// Could the computed value `x` be the reported number? `slack` is the
    /// caller's engine precision; it only ever widens. For `Lt`/`Le` only the
    /// upper side applies, for `Gt`/`Ge` only the lower, and a value exactly at
    /// a bound is accepted: being at a threshold is never grounds for
    /// `Inconsistent`.
    ///
    /// `None` when the comparison cannot be made honestly: `x` or `slack` not
    /// finite, or `slack` negative. The caller turns that into
    /// `CouldNotCheck`, never into a mismatch.
    pub fn contains(&self, x: f64, slack: f64) -> Option<bool> {
        self.overlaps(x, x, slack)
    }

    /// Does the computed range `[lo, hi]` share any point with the reported
    /// number? Same rules and same `None` as [`ReportedNumber::contains`], plus
    /// `None` when `lo > hi`.
    pub fn overlaps(&self, lo: f64, hi: f64, slack: f64) -> Option<bool> {
        if !(lo.is_finite() && hi.is_finite() && slack.is_finite()) || slack < 0.0 || lo > hi {
            return None;
        }
        // The only place the bounds become f64.
        let r_lo = outward_down(self.interval.lo.to_f64()) - slack;
        let r_hi = outward_up(self.interval.hi.to_f64()) + slack;
        let below_ok = hi >= r_lo;
        let above_ok = lo <= r_hi;
        Some(match self.relation {
            Relation::Eq => below_ok && above_ok,
            Relation::Lt | Relation::Le => above_ok,
            Relation::Gt | Relation::Ge => below_ok,
        })
    }
}

/// `a` against `b` exactly, on reduced fractions with positive denominators.
/// `None` when the cross products overflow: never an f64 fallback.
pub fn exact_cmp(a: Rational, b: Rational) -> Option<std::cmp::Ordering> {
    let l = a.numer().checked_mul(b.denom())?;
    let r = b.numer().checked_mul(a.denom())?;
    Some(l.cmp(&r))
}

/// `Rational::to_f64` is `num as f64 / den as f64`: up to three roundings,
/// each at most half an ulp. Moving the bound outward by four ulps' worth
/// guarantees the conversion can never NARROW the interval, which is the one
/// direction that could manufacture an `Inconsistent`.
fn conversion_margin(v: f64) -> f64 {
    v.abs() * 4.0 * f64::EPSILON + f64::MIN_POSITIVE
}
fn outward_down(v: f64) -> f64 {
    v - conversion_margin(v)
}
fn outward_up(v: f64) -> f64 {
    v + conversion_margin(v)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r(s: &str) -> Rational {
        Rational::parse_decimal(s).unwrap().0
    }

    const SLACK: f64 = 1e-12;

    #[test]
    fn a_printed_value_contains_its_whole_rounding_interval_and_no_more() {
        // "2.31": two decimals, half a unit in the last place = 0.005,
        // so [2.31 - 0.005, 2.31 + 0.005] = [2.305, 2.315].
        let n = ReportedNumber::measured("2.31").unwrap();
        assert_eq!(n.interval().lo, r("2.305"));
        assert_eq!(n.interval().hi, r("2.315"));
        // 2.3149 < 2.315; both endpoints are inside a closed interval.
        assert_eq!(n.contains(2.3149, SLACK), Some(true));
        assert_eq!(n.contains(2.305, SLACK), Some(true));
        assert_eq!(n.contains(2.315, SLACK), Some(true));
        // 2.3151 is 0.0001 above 2.315, far beyond 1e-12 of slack.
        assert_eq!(n.contains(2.3151, SLACK), Some(false));
        // 2.3049 is 0.0001 below 2.305.
        assert_eq!(n.contains(2.3049, SLACK), Some(false));
    }

    #[test]
    fn trailing_zeros_are_part_of_the_claim() {
        // "0.60": decimals 2, half-unit 0.005 -> [0.595, 0.605].
        // "0.6":  decimals 1, half-unit 0.05  -> [0.55, 0.65].
        let a = ReportedNumber::measured("0.60").unwrap();
        let b = ReportedNumber::measured("0.6").unwrap();
        assert_eq!(a.decimals(), 2);
        assert_eq!(b.decimals(), 1);
        assert_eq!(a.interval().lo, r("0.595"));
        assert_eq!(a.interval().hi, r("0.605"));
        assert_eq!(b.interval().lo, r("0.55"));
        assert_eq!(b.interval().hi, r("0.65"));
        assert_ne!(a.interval(), b.interval());
        // Same exact value, different reported numbers.
        assert_eq!(a.value(), b.value());
        assert_ne!(a, b);
    }

    #[test]
    fn a_leading_dot_and_both_minus_signs_are_read() {
        // ".024" and "0.024": 24/1000, three decimals each.
        let a = ReportedNumber::measured(".024").unwrap();
        let b = ReportedNumber::measured("0.024").unwrap();
        assert_eq!(a.value(), b.value());
        assert_eq!(a.decimals(), 3);
        assert_eq!(b.decimals(), 3);
        // U+2212 MINUS SIGN: -24/1000, three decimals; raw keeps the sign as printed.
        let c = ReportedNumber::measured("\u{2212}0.024").unwrap();
        assert_eq!(c.value(), r("-0.024"));
        assert_eq!(c.decimals(), 3);
        assert_eq!(c.raw(), "\u{2212}0.024");
        // ASCII hyphen gives the same value.
        assert_eq!(ReportedNumber::measured("-0.024").unwrap().value(), c.value());
    }

    #[test]
    fn a_leading_en_dash_is_a_minus_and_an_inner_one_is_a_range() {
        // "–0.5" (U+2013 first): -5/10, one decimal; raw keeps the dash.
        let n = ReportedNumber::measured("\u{2013}0.5").unwrap();
        assert_eq!(n.value(), r("-0.5"));
        assert_eq!(n.decimals(), 1);
        assert_eq!(n.raw(), "\u{2013}0.5");
        // "0.2–0.5": the dash follows a digit, so it is a range.
        assert_eq!(ReportedNumber::measured("0.2\u{2013}0.5"), Err(NumberNotRead::Range));
    }

    /// The conversion margin, pinned on a case where it is live. Found by
    /// search, not constructed: "0.6471313452454534" (16 decimals) has the
    /// upper bound N/10^16 + 1/(2·10^16) = 12942626904909069 / 2·10^16
    /// = 0.64713134524545345. Its correctly rounded f64 is
    /// 0.6471313452454535, but `to_f64` divides two rounded integers and gets
    /// 0.6471313452454533, which is INSIDE the true bound. Without the
    /// margin, a computed value sitting exactly on the bound would test as
    /// outside at zero slack: a false Inconsistent at a boundary.
    #[test]
    fn a_value_exactly_at_a_bound_stays_inside_when_conversion_rounds_inward() {
        let n = ReportedNumber::measured("0.6471313452454534").unwrap();
        let at_bound = 0.6471313452454535_f64;
        // The premise: the naive conversion really does round inward here.
        assert!(n.interval().hi.to_f64() < at_bound, "case no longer exercises the margin");
        assert_eq!(n.contains(at_bound, 0.0), Some(true));
    }

    #[test]
    fn a_p_printed_as_zero_becomes_a_bound_half_a_unit_up() {
        // ".000": three decimals, half a unit in the third place = 5 * 10^-4.
        let p = ReportedNumber::p_value(".000", Relation::Eq).unwrap();
        assert_eq!(p.relation(), Relation::Lt);
        assert_eq!(p.value(), r("0.0005"));
        assert_eq!(p.raw(), ".000");
        // ".00": two decimals, half a unit in the second place = 5 * 10^-3.
        let q = ReportedNumber::p_value(".00", Relation::Eq).unwrap();
        assert_eq!(q.relation(), Relation::Lt);
        assert_eq!(q.value(), r("0.005"));
        // ".001" is not zero: it stays Eq, widened to [0.0005, 0.0015].
        let s = ReportedNumber::p_value(".001", Relation::Eq).unwrap();
        assert_eq!(s.relation(), Relation::Eq);
        assert_eq!(s.interval().lo, r("0.0005"));
        assert_eq!(s.interval().hi, r("0.0015"));
    }

    #[test]
    fn the_zero_rule_is_not_general_behaviour_of_the_type() {
        // measured(".000") is the interval [-0.0005, 0.0005], still Eq.
        let m = ReportedNumber::measured(".000").unwrap();
        assert_eq!(m.relation(), Relation::Eq);
        assert_eq!(m.interval().lo, r("-0.0005"));
        assert_eq!(m.interval().hi, r("0.0005"));
    }

    #[test]
    fn unsupported_forms_are_not_read_each_with_its_reason() {
        assert_eq!(ReportedNumber::measured("1.2e-5"), Err(NumberNotRead::ScientificNotation));
        assert_eq!(ReportedNumber::measured("1,204"), Err(NumberNotRead::Comma));
        assert_eq!(ReportedNumber::measured("0,05"), Err(NumberNotRead::Comma));
        assert_eq!(ReportedNumber::measured("12%"), Err(NumberNotRead::Percent));
        assert_eq!(ReportedNumber::measured("3-5"), Err(NumberNotRead::Range));
        assert_eq!(ReportedNumber::measured("3\u{2013}5"), Err(NumberNotRead::Range));
    }

    #[test]
    fn malformed_signs_points_and_spaces_are_not_read() {
        for s in ["+-1", "5.", ".", "-", "--1", "+1", "007", "1.2.3", "abc", "one", "\u{2013}\u{2013}0.5"] {
            assert_eq!(ReportedNumber::measured(s), Err(NumberNotRead::Malformed), "{s:?}");
        }
        for s in ["1 204", "1\u{202f}204", "1\u{a0}204"] {
            assert_eq!(ReportedNumber::measured(s), Err(NumberNotRead::InternalSpace), "{s:?}");
        }
        assert_eq!(ReportedNumber::measured("   "), Err(NumberNotRead::Empty));
        // Surrounding whitespace is trimmed, not rejected, and raw is the trimmed form.
        assert_eq!(ReportedNumber::measured("  2.31 ").unwrap().raw(), "2.31");
    }

    #[test]
    fn a_measured_integer_widens_and_a_count_does_not() {
        // measured "12": zero decimals, half a unit in the units place = 0.5.
        let m = ReportedNumber::measured("12").unwrap();
        assert_eq!(m.interval().lo, r("11.5"));
        assert_eq!(m.interval().hi, r("12.5"));
        // count "12": counted, not rounded, so the point [12, 12].
        let c = ReportedNumber::count("12").unwrap();
        assert_eq!(c.interval().lo, Rational::from_int(12));
        assert_eq!(c.interval().hi, Rational::from_int(12));
        // measured "3" is [2.5, 3.5]: 2.6 inside, 3.6 outside.
        let t = ReportedNumber::measured("3").unwrap();
        assert_eq!(t.contains(2.6, SLACK), Some(true));
        assert_eq!(t.contains(3.6, SLACK), Some(false));
    }

    #[test]
    fn a_count_refuses_a_point_and_a_sign() {
        assert_eq!(ReportedNumber::count("28.4"), Err(NumberNotRead::DecimalInCount));
        assert_eq!(ReportedNumber::count("-3"), Err(NumberNotRead::NegativeCount));
        assert_eq!(ReportedNumber::count("1,204"), Err(NumberNotRead::Comma));
    }

    #[test]
    fn a_p_outside_zero_to_one_is_not_read() {
        assert_eq!(ReportedNumber::p_value("1.20", Relation::Eq), Err(NumberNotRead::PValueOutOfRange));
        assert_eq!(ReportedNumber::p_value("-0.01", Relation::Eq), Err(NumberNotRead::PValueOutOfRange));
        assert!(ReportedNumber::p_value("1.00", Relation::Eq).is_ok());
    }

    #[test]
    fn an_inequality_bound_is_exact_and_one_sided() {
        // "p < .05": the true p is anything up to 0.05.
        let p = ReportedNumber::p_value(".05", Relation::Lt).unwrap();
        assert_eq!(p.interval(), Interval::point(r("0.05")));
        assert_eq!(p.contains(0.0001, SLACK), Some(true));
        assert_eq!(p.contains(0.05, SLACK), Some(true));
        assert_eq!(p.contains(0.0501, SLACK), Some(false));
        // "p > .05": anything from 0.05 up.
        let g = ReportedNumber::p_value(".05", Relation::Gt).unwrap();
        assert_eq!(g.contains(0.9, SLACK), Some(true));
        assert_eq!(g.contains(0.0499, SLACK), Some(false));
    }

    #[test]
    fn overlaps_compares_a_computed_range() {
        // "2.31" = [2.305, 2.315]
        let n = ReportedNumber::measured("2.31").unwrap();
        assert_eq!(n.overlaps(2.30, 2.306, SLACK), Some(true));
        assert_eq!(n.overlaps(2.314, 2.40, SLACK), Some(true));
        assert_eq!(n.overlaps(2.20, 2.30, SLACK), Some(false));
        assert_eq!(n.overlaps(2.3151, 2.40, SLACK), Some(false));
    }

    #[test]
    fn slack_only_widens() {
        // 2.3151 is 1e-4 outside [2.305, 2.315]; slack 2e-4 brings it in.
        let n = ReportedNumber::measured("2.31").unwrap();
        assert_eq!(n.contains(2.3151, 2e-4), Some(true));
        assert_eq!(n.contains(2.3151, 0.0), Some(false));
    }

    #[test]
    fn a_comparison_that_cannot_be_made_honestly_is_none() {
        let n = ReportedNumber::measured("2.31").unwrap();
        assert_eq!(n.contains(f64::NAN, SLACK), None);
        assert_eq!(n.contains(f64::INFINITY, SLACK), None);
        assert_eq!(n.contains(2.31, -1e-12), None);
        assert_eq!(n.contains(2.31, f64::NAN), None);
        assert_eq!(n.overlaps(2.4, 2.3, SLACK), None);
    }

    #[test]
    fn negative_zero_keeps_its_sign_only_in_raw() {
        let n = ReportedNumber::measured("-0.000").unwrap();
        assert!(n.value().is_zero());
        assert_eq!(n.raw(), "-0.000");
    }

    /// Every two-decimal string from "0.00" to "9.99": each interval is
    /// exactly 0.01 wide (2 * 0.005), and each one's upper end is exactly the
    /// next one's lower end (k/100 + 0.005 = (k+1)/100 - 0.005). With positive
    /// width and closed ends, that shared end is the ONLY point they share.
    /// Compared as exact rationals.
    #[test]
    fn neighbouring_two_decimal_intervals_tile_exactly() {
        let width = Rational::new(1, 100).unwrap();
        let mut prev: Option<Interval> = None;
        let mut checked = 0;
        for k in 0..1000u32 {
            let s = format!("{}.{:02}", k / 100, k % 100);
            let i = ReportedNumber::measured(&s).unwrap().interval();
            assert_eq!(i.hi.sub(i.lo).unwrap(), width, "{s}");
            if let Some(p) = prev {
                assert_eq!(p.hi, i.lo, "{s} must start where its neighbour ends");
            }
            prev = Some(i);
            checked += 1;
        }
        assert_eq!(checked, 1000);
    }

    /// The overflow limit, derived and then pinned. For "0.0…01" with d
    /// decimals the value is 1/10^d and the half-unit 1/(2·10^d); `Rational`
    /// adds them over the denominator 10^d · 2·10^d = 2·10^(2d), which fits in
    /// i128 (max ≈ 1.7·10^38) only while 2d ≤ 37. So d = 18 is read and d = 19
    /// is not, far below parse_decimal's own limit near 38 digits. The limit
    /// depends on the value: a reducible one like "0.5000…" goes further.
    #[test]
    fn too_many_digits_is_not_read_and_the_limit_is_pinned() {
        let at = |d: usize| ReportedNumber::measured(&format!("0.{}1", "0".repeat(d - 1)));
        let last_ok = (1..=40).take_while(|&d| at(d).is_ok()).last().unwrap();
        assert_eq!(last_ok, 18);
        assert_eq!(at(19), Err(NumberNotRead::TooManyDigits));
        assert_eq!(at(40), Err(NumberNotRead::TooManyDigits));
        assert_eq!(ReportedNumber::count(&"9".repeat(40)), Err(NumberNotRead::TooManyDigits));
    }
}
