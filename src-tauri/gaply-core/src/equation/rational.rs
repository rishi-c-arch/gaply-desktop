//! Exact rational arithmetic — the reason this engine can say "certain".
//!
//! **Why not `f64`.** Tier 0 (§4.4) is the tier that overrides model consensus,
//! so a Tier-0 disagreement has to be a fact about the arithmetic and not an
//! artefact of binary floating point. `0.1 + 0.2 != 0.3` in `f64`, and a
//! verification engine that reported that as a manuscript defect would be
//! confidently wrong about the one thing it exists to be right about.
//!
//! Every decimal literal a manuscript can write is exactly a rational, so the
//! whole of the parsed arithmetic — `+ - × ÷` and integer powers — stays exact.
//!
//! **Overflow is an honest `None`, never a wrap and never a panic.** A 128-bit
//! numerator is far beyond anything a manuscript writes, but "far beyond" is not
//! a proof, and a silently wrapped product would be a fabricated number in a
//! module written to stop fabricated numbers.

/// An exact rational. Always reduced, `den > 0`; `0` is `0/1`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Rational {
    num: i128,
    den: i128,
}

fn gcd(a: i128, b: i128) -> i128 {
    let (mut a, mut b) = (a.abs(), b.abs());
    while b != 0 {
        let t = a % b;
        a = b;
        b = t;
    }
    a
}

impl Rational {
    pub const ZERO: Rational = Rational { num: 0, den: 1 };
    pub const ONE: Rational = Rational { num: 1, den: 1 };

    /// Reduce and normalise the sign. `den == 0` is `None`, not a panic —
    /// division by zero is something a manuscript can actually write.
    pub fn new(num: i128, den: i128) -> Option<Rational> {
        if den == 0 {
            return None;
        }
        let g = gcd(num, den).max(1);
        let (mut n, mut d) = (num / g, den / g);
        if d < 0 {
            n = n.checked_neg()?;
            d = d.checked_neg()?;
        }
        Some(Rational { num: n, den: d })
    }

    pub fn from_int(n: i128) -> Rational {
        Rational { num: n, den: 1 }
    }

    pub fn numer(&self) -> i128 {
        self.num
    }
    pub fn denom(&self) -> i128 {
        self.den
    }
    pub fn is_zero(&self) -> bool {
        self.num == 0
    }
    pub fn is_negative(&self) -> bool {
        self.num < 0
    }
    pub fn is_integer(&self) -> bool {
        self.den == 1
    }

    pub fn add(self, o: Rational) -> Option<Rational> {
        let n = self
            .num
            .checked_mul(o.den)?
            .checked_add(o.num.checked_mul(self.den)?)?;
        Rational::new(n, self.den.checked_mul(o.den)?)
    }

    pub fn sub(self, o: Rational) -> Option<Rational> {
        self.add(o.neg()?)
    }

    pub fn neg(self) -> Option<Rational> {
        Some(Rational { num: self.num.checked_neg()?, den: self.den })
    }

    pub fn mul(self, o: Rational) -> Option<Rational> {
        Rational::new(self.num.checked_mul(o.num)?, self.den.checked_mul(o.den)?)
    }

    pub fn div(self, o: Rational) -> Option<Rational> {
        if o.is_zero() {
            return None;
        }
        Rational::new(self.num.checked_mul(o.den)?, self.den.checked_mul(o.num)?)
    }

    /// Integer powers only. A fractional exponent is not generally rational
    /// (`2^(1/2)`), and this module refuses to approximate — the caller turns
    /// the `None` into `UNVERIFIED` rather than a guess (§6b.3).
    pub fn powi(self, e: i32) -> Option<Rational> {
        if e == 0 {
            return Some(Rational::ONE);
        }
        let mut acc = Rational::ONE;
        let base = if e < 0 { Rational::ONE.div(self)? } else { self };
        for _ in 0..e.unsigned_abs() {
            acc = acc.mul(base)?;
        }
        Some(acc)
    }

    pub fn cmp_val(&self, o: &Rational) -> std::cmp::Ordering {
        // a/b ? c/d  with b,d > 0  <=>  a*d ? c*b
        match (self.num.checked_mul(o.den), o.num.checked_mul(self.den)) {
            (Some(l), Some(r)) => l.cmp(&r),
            // Fall back to f64 only for ORDERING of values too large to compare
            // exactly. Never used for equality — see `eq`.
            _ => self
                .to_f64()
                .partial_cmp(&o.to_f64())
                .unwrap_or(std::cmp::Ordering::Equal),
        }
    }

    pub fn abs(self) -> Rational {
        Rational { num: self.num.abs(), den: self.den }
    }

    /// Lossy — for DISPLAY and for ordering only. Never for a verdict.
    pub fn to_f64(self) -> f64 {
        self.num as f64 / self.den as f64
    }

    /// Parse a decimal literal, returning the value and the number of decimal
    /// places it DISPLAYS.
    ///
    /// The decimal count is not decoration: it is the author's own statement of
    /// the precision they are claiming, and it is what a reported value is
    /// compared at. `623.36` claims two places; `623` claims none.
    ///
    /// Thousands separators are accepted because manuscripts write them
    /// (`237,000`), and the Indian grouping (`2,37,000`) as well — the
    /// separator positions are not validated, only removed.
    pub fn parse_decimal(s: &str) -> Option<(Rational, u32)> {
        let s = s.trim();
        let cleaned: String = s.chars().filter(|c| *c != ',' && *c != '\u{202f}' && *c != ' ').collect();
        if cleaned.is_empty() {
            return None;
        }
        let (int_part, frac_part) = match cleaned.split_once('.') {
            Some((a, b)) => (a, b),
            None => (cleaned.as_str(), ""),
        };
        if int_part.is_empty() && frac_part.is_empty() {
            return None;
        }
        if !int_part.chars().all(|c| c.is_ascii_digit() || c == '-' || c == '+')
            || !frac_part.chars().all(|c| c.is_ascii_digit())
        {
            return None;
        }
        let decimals = frac_part.len() as u32;
        let digits = format!("{int_part}{frac_part}");
        let digits = if digits.starts_with('+') { &digits[1..] } else { &digits[..] };
        let num: i128 = digits.parse().ok()?;
        let den = 10i128.checked_pow(decimals)?;
        Some((Rational::new(num, den)?, decimals))
    }

    /// Round to `decimals` places, half away from zero — the convention a
    /// manuscript's own rounding follows.
    pub fn round_to(self, decimals: u32) -> Option<Rational> {
        let scale = Rational::from_int(10i128.checked_pow(decimals)?);
        let scaled = self.mul(scale)?;
        // floor(|x| + 1/2) with the sign restored
        let half = Rational::new(1, 2)?;
        let shifted = scaled.abs().add(half)?;
        let floored = shifted.num.div_euclid(shifted.den);
        let signed = if self.is_negative() { floored.checked_neg()? } else { floored };
        Rational::from_int(signed).div(scale)
    }

    /// Decimal rendering at `decimals` places. Used in findings, where the
    /// reader must see both numbers in the same form the manuscript used.
    pub fn to_decimal_string(self, decimals: u32) -> String {
        let r = match self.round_to(decimals) {
            Some(r) => r,
            None => return format!("{}/{}", self.num, self.den),
        };
        let scale = match 10i128.checked_pow(decimals) {
            Some(s) => s,
            None => return format!("{}/{}", self.num, self.den),
        };
        let scaled = match r.num.checked_mul(scale / gcd(scale, r.den).max(1)) {
            Some(_) => (r.num * (scale / r.den.max(1))).abs(),
            None => return format!("{}/{}", self.num, self.den),
        };
        let sign = if r.is_negative() { "-" } else { "" };
        if decimals == 0 {
            return format!("{sign}{scaled}");
        }
        let s = format!("{scaled:0width$}", width = decimals as usize + 1);
        let (a, b) = s.split_at(s.len() - decimals as usize);
        format!("{sign}{a}.{b}")
    }
}

/// Decimal places shown for a value whose decimal expansion does not
/// terminate. Five is enough for a reader to re-do the division by hand and
/// recognise the number, and the result is ALWAYS marked with `…` so a
/// truncation can never be mistaken for the exact value.
const NON_TERMINATING_DISPLAY_DECIMALS: u32 = 5;

impl Rational {
    /// The rendering shown to an author in a finding.
    ///
    /// An exact terminating decimal where one exists; otherwise a truncation
    /// MARKED as one. `237,000/380.2` is `1185000/1901` exactly, and a finding
    /// that printed that would be correct and useless — the author wrote
    /// `623.36` and needs to see `623.35613…`.
    ///
    /// The mark is not decoration. This is a Tier-0 engine, and an unmarked
    /// `623.35613` would assert an exactness the value does not have.
    pub fn to_display_string(self) -> String {
        if self.terminates() {
            self.to_string()
        } else {
            format!("{}…", self.to_decimal_string(NON_TERMINATING_DISPLAY_DECIMALS))
        }
    }

    /// Does the decimal expansion terminate? It does exactly when the reduced
    /// denominator has no prime factor but 2 and 5.
    pub fn terminates(&self) -> bool {
        let mut d = self.den;
        while d % 2 == 0 {
            d /= 2;
        }
        while d % 5 == 0 {
            d /= 5;
        }
        d == 1
    }
}

impl std::fmt::Display for Rational {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.den == 1 {
            write!(f, "{}", self.num)
        } else {
            // Exact terminating decimal when the denominator allows it, so a
            // finding shows `0.219` rather than `219/1000`.
            let mut d = self.den;
            let (mut twos, mut fives) = (0u32, 0u32);
            while d % 2 == 0 {
                d /= 2;
                twos += 1;
            }
            while d % 5 == 0 {
                d /= 5;
                fives += 1;
            }
            if d == 1 {
                write!(f, "{}", self.to_decimal_string(twos.max(fives)))
            } else {
                write!(f, "{}/{}", self.num, self.den)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_float_trap_this_module_exists_to_avoid() {
        // 0.1 + 0.2 != 0.3 in f64. A Tier-0 engine on f64 would report a
        // manuscript defect here.
        let a = Rational::parse_decimal("0.1").unwrap().0;
        let b = Rational::parse_decimal("0.2").unwrap().0;
        let c = Rational::parse_decimal("0.3").unwrap().0;
        assert_eq!(a.add(b).unwrap(), c);
        assert!(0.1f64 + 0.2f64 != 0.3f64, "the f64 claim this test rests on");
    }

    #[test]
    fn thousands_separators_are_removed_in_both_groupings() {
        assert_eq!(Rational::parse_decimal("237,000").unwrap().0, Rational::from_int(237_000));
        // The Indian grouping, as written in the manuscript this was measured on.
        assert_eq!(Rational::parse_decimal("2,37,000").unwrap().0, Rational::from_int(237_000));
    }

    #[test]
    fn a_literal_carries_the_precision_its_author_claimed() {
        assert_eq!(Rational::parse_decimal("623.36").unwrap().1, 2);
        assert_eq!(Rational::parse_decimal("623").unwrap().1, 0);
        assert_eq!(Rational::parse_decimal("0.500").unwrap().1, 3);
        // 0.500 and 0.5 are the same VALUE and different CLAIMS.
        assert_eq!(
            Rational::parse_decimal("0.500").unwrap().0,
            Rational::parse_decimal("0.5").unwrap().0
        );
        assert_ne!(
            Rational::parse_decimal("0.500").unwrap().1,
            Rational::parse_decimal("0.5").unwrap().1
        );
    }

    #[test]
    fn slovins_formula_is_exact_end_to_end() {
        // 237,000 / (1 + 237,000 × 0.04²)
        let n = Rational::from_int(237_000);
        let e = Rational::parse_decimal("0.04").unwrap().0;
        let den = Rational::ONE.add(n.mul(e.powi(2).unwrap()).unwrap()).unwrap();
        assert_eq!(den, Rational::parse_decimal("380.2").unwrap().0);
        let v = n.div(den).unwrap();
        // Exact value rounds to the reported 623.36 at the reported precision.
        assert_eq!(v.round_to(2).unwrap(), Rational::parse_decimal("623.36").unwrap().0);
        assert_eq!(v.to_decimal_string(2), "623.36");
        assert_eq!(v.to_decimal_string(0), "623");
    }

    #[test]
    fn division_by_zero_is_none_not_a_panic() {
        assert!(Rational::ONE.div(Rational::ZERO).is_none());
        assert!(Rational::new(1, 0).is_none());
    }

    #[test]
    fn overflow_is_none_not_a_wrap() {
        let big = Rational::from_int(i128::MAX / 2);
        assert!(big.mul(big).is_none());
        assert!(big.add(big).is_some());
        assert!(big.add(big).unwrap().add(big).is_none());
    }

    #[test]
    fn rounding_is_half_away_from_zero_and_symmetric() {
        let p = Rational::parse_decimal("0.125").unwrap().0;
        assert_eq!(p.round_to(2).unwrap().to_decimal_string(2), "0.13");
        let n = Rational::parse_decimal("-0.125").unwrap().0;
        assert_eq!(n.round_to(2).unwrap().to_decimal_string(2), "-0.13");
    }

    #[test]
    fn the_health_economics_chain_evaluates_exactly() {
        // (0.108 × 0.78) + (0.500 × 0.13) + (0.769 × 0.06) + (0.810 × 0.03)
        let terms = [("0.108", "0.78"), ("0.500", "0.13"), ("0.769", "0.06"), ("0.810", "0.03")];
        let mut sum = Rational::ZERO;
        for (a, b) in terms {
            let a = Rational::parse_decimal(a).unwrap().0;
            let b = Rational::parse_decimal(b).unwrap().0;
            sum = sum.add(a.mul(b).unwrap()).unwrap();
        }
        assert_eq!(sum, Rational::parse_decimal("0.21968").unwrap().0);
        // The manuscript's own next step, 0.084 + 0.065 + 0.046 + 0.024:
        let stated = Rational::parse_decimal("0.219").unwrap().0;
        assert_ne!(sum, stated);
        // At the stated three decimals the products give 0.220, not 0.219.
        assert_eq!(sum.round_to(3).unwrap().to_decimal_string(3), "0.220");
    }

    #[test]
    fn a_non_terminating_value_is_shown_truncated_and_MARKED() {
        // 237,000 / 380.2 — the Slovin computation.
        let v = Rational::from_int(237_000)
            .div(Rational::parse_decimal("380.2").unwrap().0)
            .unwrap();
        assert!(!v.terminates());
        assert_eq!(v.to_string(), "1185000/1901", "exact, and unreadable");
        assert_eq!(v.to_display_string(), "623.35613…");
        // The mark is what stops a truncation reading as an exact value.
        assert!(v.to_display_string().ends_with('…'));
        // A terminating value carries no mark.
        assert_eq!(Rational::parse_decimal("0.219").unwrap().0.to_display_string(), "0.219");
    }

    #[test]
    fn display_prefers_an_exact_terminating_decimal() {
        assert_eq!(Rational::parse_decimal("0.219").unwrap().0.to_string(), "0.219");
        assert_eq!(Rational::from_int(624).to_string(), "624");
        // A repeating decimal stays a fraction rather than becoming a lie.
        assert_eq!(Rational::new(1, 3).unwrap().to_string(), "1/3");
    }
}
