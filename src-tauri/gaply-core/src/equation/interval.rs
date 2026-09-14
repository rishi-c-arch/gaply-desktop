//! **The `AsRounded` reading: what a manuscript's decimals could have been.**
//!
//! [`DecimalReading::AsRounded`](crate::epistemic::DecimalReading::AsRounded)
//! says `0.108` stands for any value in `[0.1075, 0.1085)`. This evaluates an
//! expression over those intervals, so an arithmetic claim can be judged
//! against every value the author might have rounded from.
//!
//! # The one distinction that decides how permissive this is
//!
//! **A number written WITHOUT a decimal point is exact; a number written WITH
//! one may be a rounding.** `1`, `2`, `8000`, `237,000` are structural
//! constants and counts — an author who writes `1 + N·e²` means one, and
//! widening it to `[0.5, 1.5]` would make almost any chain "explainable by
//! rounding" and the check would find nothing, ever. `0.04` and `0.769` are
//! displayed values and carry `±5 × 10⁻⁽ᵈ⁺¹⁾`.
//!
//! This is the convention manuscripts already follow, and it is the difference
//! between a check that can fire and one that cannot.
//!
//! # Soundness, and which direction the looseness runs
//!
//! Interval arithmetic here is an ENCLOSURE, not the exact range: `[−1,2]²`
//! evaluates to `[−2,4]` by repeated multiplication where the true range is
//! `[0,4]`. The enclosure always CONTAINS the truth, so an overlap can be
//! reported where none exists, but a disjointness can never be. Disjoint
//! intervals are therefore a proof, and that is the only direction a finding is
//! raised from — the same asymmetry as [`super::equiv`].

use super::expr::{EvalError, Expr};
use super::rational::Rational;
use std::collections::BTreeMap;

/// A closed interval of exact rationals, `lo ≤ hi`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Interval {
    pub lo: Rational,
    pub hi: Rational,
}

impl Interval {
    pub fn point(v: Rational) -> Interval {
        Interval { lo: v, hi: v }
    }

    pub fn new(a: Rational, b: Rational) -> Interval {
        if a.cmp_val(&b) == std::cmp::Ordering::Greater {
            Interval { lo: b, hi: a }
        } else {
            Interval { lo: a, hi: b }
        }
    }

    /// The interval a literal with `decimals` displayed places stands for.
    /// `decimals == 0` is EXACT — see the module header.
    pub fn of_literal(value: Rational, decimals: u32) -> Option<Interval> {
        if decimals == 0 {
            return Some(Interval::point(value));
        }
        // ±5 × 10⁻⁽ᵈ⁺¹⁾ — half a unit in the last displayed place.
        let half_ulp = Rational::new(5, 10i128.checked_pow(decimals.checked_add(1)?)?)?;
        Some(Interval { lo: value.sub(half_ulp)?, hi: value.add(half_ulp)? })
    }

    pub fn contains_zero(&self) -> bool {
        self.lo.cmp_val(&Rational::ZERO) != std::cmp::Ordering::Greater
            && self.hi.cmp_val(&Rational::ZERO) != std::cmp::Ordering::Less
    }

    /// Do these intervals share any point? **Disjointness is the proof**; an
    /// overlap is only a failure to refute.
    pub fn overlaps(&self, o: &Interval) -> bool {
        self.lo.cmp_val(&o.hi) != std::cmp::Ordering::Greater
            && o.lo.cmp_val(&self.hi) != std::cmp::Ordering::Greater
    }

    fn add(self, o: Interval) -> Option<Interval> {
        Some(Interval { lo: self.lo.add(o.lo)?, hi: self.hi.add(o.hi)? })
    }
    fn sub(self, o: Interval) -> Option<Interval> {
        Some(Interval { lo: self.lo.sub(o.hi)?, hi: self.hi.sub(o.lo)? })
    }
    fn neg(self) -> Option<Interval> {
        Some(Interval { lo: self.hi.neg()?, hi: self.lo.neg()? })
    }
    fn mul(self, o: Interval) -> Option<Interval> {
        let c = [
            self.lo.mul(o.lo)?,
            self.lo.mul(o.hi)?,
            self.hi.mul(o.lo)?,
            self.hi.mul(o.hi)?,
        ];
        let mut lo = c[0];
        let mut hi = c[0];
        for v in &c[1..] {
            if v.cmp_val(&lo) == std::cmp::Ordering::Less {
                lo = *v;
            }
            if v.cmp_val(&hi) == std::cmp::Ordering::Greater {
                hi = *v;
            }
        }
        Some(Interval { lo, hi })
    }
    /// Division by an interval STRADDLING zero is undefined, not infinite —
    /// the result is unbounded and no honest enclosure exists.
    fn div(self, o: Interval) -> Option<Interval> {
        if o.contains_zero() {
            return None;
        }
        let recip = Interval::new(Rational::ONE.div(o.hi)?, Rational::ONE.div(o.lo)?);
        self.mul(recip)
    }
    fn powi(self, e: i32) -> Option<Interval> {
        if e == 0 {
            return Some(Interval::point(Rational::ONE));
        }
        let base = if e < 0 { Interval::point(Rational::ONE).div(self)? } else { self };
        let mut acc = Interval::point(Rational::ONE);
        for _ in 0..e.unsigned_abs() {
            acc = acc.mul(base)?;
        }
        Some(acc)
    }
}

/// Variable bindings for the interval reading.
pub type IntervalBindings = BTreeMap<String, Interval>;

/// Evaluate under the rounded reading. Every inexactness is an [`EvalError`],
/// exactly as in the exact evaluator — this never approximates either.
pub fn eval_interval(e: &Expr, b: &IntervalBindings) -> Result<Interval, EvalError> {
    match e {
        Expr::Num { value, decimals } => {
            Interval::of_literal(*value, *decimals).ok_or(EvalError::Overflow)
        }
        Expr::Var(v) => b.get(v).copied().ok_or_else(|| EvalError::UnboundVariable(v.clone())),
        Expr::Neg(a) => eval_interval(a, b)?.neg().ok_or(EvalError::Overflow),
        Expr::Add(x, y) => {
            eval_interval(x, b)?.add(eval_interval(y, b)?).ok_or(EvalError::Overflow)
        }
        Expr::Sub(x, y) => {
            eval_interval(x, b)?.sub(eval_interval(y, b)?).ok_or(EvalError::Overflow)
        }
        Expr::Mul(x, y) => {
            eval_interval(x, b)?.mul(eval_interval(y, b)?).ok_or(EvalError::Overflow)
        }
        Expr::Div(x, y) => {
            let d = eval_interval(y, b)?;
            if d.contains_zero() {
                return Err(EvalError::DivisionByZero);
            }
            eval_interval(x, b)?.div(d).ok_or(EvalError::Overflow)
        }
        Expr::Pow(x, y) => {
            // The exponent is a count, never a rounded measurement.
            let ev = y.eval(&Default::default())?;
            if !ev.is_integer() {
                return Err(EvalError::NonIntegerExponent);
            }
            let ex: i32 = i32::try_from(ev.numer()).map_err(|_| EvalError::Overflow)?;
            let base = eval_interval(x, b)?;
            if base.contains_zero() && ex < 0 {
                return Err(EvalError::DivisionByZero);
            }
            base.powi(ex).ok_or(EvalError::Overflow)
        }
        // A root of an interval is not generally an interval of rationals.
        Expr::Sqrt(_) => Err(EvalError::IrrationalRoot),
        Expr::Func(n, _) => Err(EvalError::UnsupportedFunction(n.clone())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::equation::linear::parse_equation;

    fn side(s: &str, i: usize) -> Expr {
        parse_equation(s).unwrap().sides[i].expr.clone()
    }
    fn ev(s: &str, i: usize) -> Interval {
        eval_interval(&side(s, i), &IntervalBindings::new()).unwrap()
    }
    fn r(s: &str) -> Rational {
        Rational::parse_decimal(s).unwrap().0
    }

    #[test]
    fn an_integer_is_exact_and_a_decimal_is_not() {
        assert_eq!(
            Interval::of_literal(Rational::from_int(8000), 0).unwrap(),
            Interval::point(Rational::from_int(8000))
        );
        let i = Interval::of_literal(r("0.108"), 3).unwrap();
        assert_eq!(i.lo, r("0.1075"));
        assert_eq!(i.hi, r("0.1085"));
    }

    /// If integers widened, `1 + N·e²` would swallow any discrepancy and the
    /// check could never fire. This is the pin on that.
    #[test]
    fn a_structural_one_does_not_widen() {
        let i = ev("x = 1", 1);
        assert_eq!(i.lo, Rational::ONE);
        assert_eq!(i.hi, Rational::ONE);
    }

    #[test]
    fn the_enclosure_contains_the_exact_value() {
        let exact = side("x = (0.108 × 0.78) + (0.500 × 0.13)", 1)
            .eval(&Default::default())
            .unwrap();
        let i = ev("x = (0.108 × 0.78) + (0.500 × 0.13)", 1);
        assert_ne!(i.lo, i.hi, "a decimal computation must have width");
        assert!(i.overlaps(&Interval::point(exact)), "{i:?} must contain {exact}");
    }

    /// The 21.9% case, under the rounded reading: the two sides overlap, so
    /// rounding COULD explain the disagreement — which is why it becomes a
    /// question for the author rather than a flat `DETECTED`.
    #[test]
    fn the_weighted_provision_sides_overlap_under_the_rounded_reading() {
        let line = "Weighted provision = (0.108 × 0.78) + (0.500 × 0.13) + (0.769 × 0.06) \
                    + (0.810 × 0.03) = 0.084 + 0.065 + 0.046 + 0.024 = 21.9%";
        let products = ev(line, 1);
        let terms = ev(line, 2);
        assert!(products.overlaps(&terms), "{products:?} vs {terms:?}");
        // And as written they are NOT equal — that is the whole tension.
        assert_ne!(
            side(line, 1).eval(&Default::default()).unwrap(),
            side(line, 2).eval(&Default::default()).unwrap()
        );
    }

    #[test]
    fn multiplication_across_zero_takes_the_widest_corner() {
        let a = Interval::new(Rational::from_int(-1), Rational::from_int(2));
        let b = Interval::new(Rational::from_int(-3), Rational::from_int(4));
        let m = a.mul(b).unwrap();
        assert_eq!(m.lo, Rational::from_int(-6));
        assert_eq!(m.hi, Rational::from_int(8));
    }

    #[test]
    fn dividing_by_an_interval_that_straddles_zero_is_refused() {
        let a = Interval::point(Rational::ONE);
        let b = Interval::new(Rational::from_int(-1), Rational::from_int(1));
        assert!(a.div(b).is_none());
        assert_eq!(
            eval_interval(&side("x = 1/(y − 2)", 1), &{
                let mut m = IntervalBindings::new();
                m.insert("y".into(), Interval::new(Rational::from_int(1), Rational::from_int(3)));
                m
            })
            .unwrap_err(),
            EvalError::DivisionByZero
        );
    }

    #[test]
    fn overlap_is_symmetric_and_touching_counts() {
        let a = Interval::new(r("0.1"), r("0.2"));
        let b = Interval::new(r("0.2"), r("0.3"));
        assert!(a.overlaps(&b) && b.overlaps(&a), "a shared endpoint is an overlap");
        let c = Interval::new(r("0.3"), r("0.4"));
        assert!(!a.overlaps(&c) && !c.overlaps(&a));
    }

    /// An enclosure may be wider than the true range; it must never be
    /// narrower, or a disjointness — the only thing that raises a finding —
    /// could be wrong.
    #[test]
    fn the_power_enclosure_is_wide_but_sound() {
        let mut b = IntervalBindings::new();
        b.insert("x".into(), Interval::new(Rational::from_int(-1), Rational::from_int(2)));
        let sq = eval_interval(&side("y = x²", 1), &b).unwrap();
        assert_eq!(sq.lo, Rational::from_int(-2), "wider than the true 0");
        assert_eq!(sq.hi, Rational::from_int(4));
        // Soundness: every true value is inside.
        for v in [-1i128, 0, 1, 2] {
            let t = Rational::from_int(v * v);
            assert!(sq.overlaps(&Interval::point(t)), "{t} escaped the enclosure");
        }
    }
}
