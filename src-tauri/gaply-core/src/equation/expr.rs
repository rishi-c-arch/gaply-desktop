//! The expression tree, and the ONE canonical form both readers target.
//!
//! `linear.rs` reads equations a manuscript typed as text; `omml.rs` reads
//! equations Word stored as OMML. **They produce the same `Expr`**, which is
//! what makes symbolic equivalence between the two possible at all — the
//! Slovin's-formula case in `Corrected_Chapters_3_4_Jitesh_Agarwal.docx` writes
//! the symbolic form and its numeric substitution as two separate OMML
//! elements, and a manuscript that defines a formula in text and applies it in
//! an equation editor is the same problem across two readers.
//!
//! Evaluation is exact ([`super::rational::Rational`]) or it is
//! [`EvalError`] — never approximate. §6b.3: where the record cannot bind an
//! equation's inputs the check is `UNVERIFIED`, "stated as such, never
//! guessed", and an `f64` fallback would be exactly the guess it forbids.

use std::collections::BTreeMap;

use super::rational::Rational;

/// A parsed expression.
///
/// `Sub` and `Div` are kept rather than desugared at parse time because a
/// finding has to quote the manuscript back to its author: `a − b` reported as
/// `a + (−1 × b)` is correct and unrecognisable. Desugaring happens in
/// [`Expr::canonical`], which is a separate, reversible step.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Expr {
    /// A numeric literal and the decimal places it DISPLAYS — the author's own
    /// statement of the precision they claim. See [`Rational::parse_decimal`].
    Num { value: Rational, decimals: u32 },
    Var(String),
    Neg(Box<Expr>),
    Add(Box<Expr>, Box<Expr>),
    Sub(Box<Expr>, Box<Expr>),
    Mul(Box<Expr>, Box<Expr>),
    Div(Box<Expr>, Box<Expr>),
    Pow(Box<Expr>, Box<Expr>),
    Sqrt(Box<Expr>),
    /// A named function the engine does not evaluate (`log`, `sin`, …). Present
    /// so an equation containing one still PARSES and still participates in
    /// symbolic equivalence; evaluation of it is `Unsupported`, not a guess.
    Func(String, Vec<Expr>),
}

impl Expr {
    pub fn num(value: Rational, decimals: u32) -> Expr {
        Expr::Num { value, decimals }
    }
    pub fn int(n: i128) -> Expr {
        Expr::Num { value: Rational::from_int(n), decimals: 0 }
    }

    /// Every variable named in the expression, deduplicated and ordered.
    pub fn variables(&self) -> Vec<String> {
        let mut out = Vec::new();
        self.walk(&mut |e| {
            if let Expr::Var(v) = e {
                if !out.contains(v) {
                    out.push(v.clone());
                }
            }
        });
        out.sort();
        out
    }

    /// The COARSEST precision any literal in this expression claims, or `None`
    /// when it contains no literals. This is what an equality between two
    /// computed sides can honestly be judged at.
    pub fn coarsest_decimals(&self) -> Option<u32> {
        let mut out: Option<u32> = None;
        self.walk(&mut |e| {
            if let Expr::Num { decimals, .. } = e {
                out = Some(out.map_or(*decimals, |d: u32| d.min(*decimals)));
            }
        });
        out
    }

    /// True when the expression is a bare numeric literal — a REPORTED VALUE
    /// rather than a computation. The distinction decides how an equality is
    /// judged: a reported value is compared at the precision it displays.
    pub fn as_reported_value(&self) -> Option<(Rational, u32)> {
        match self {
            Expr::Num { value, decimals } => Some((*value, *decimals)),
            Expr::Neg(inner) => inner
                .as_reported_value()
                .and_then(|(v, d)| v.neg().map(|v| (v, d))),
            _ => None,
        }
    }

    fn walk(&self, f: &mut impl FnMut(&Expr)) {
        f(self);
        match self {
            Expr::Num { .. } | Expr::Var(_) => {}
            Expr::Neg(a) | Expr::Sqrt(a) => a.walk(f),
            Expr::Add(a, b)
            | Expr::Sub(a, b)
            | Expr::Mul(a, b)
            | Expr::Div(a, b)
            | Expr::Pow(a, b) => {
                a.walk(f);
                b.walk(f);
            }
            Expr::Func(_, args) => {
                for a in args {
                    a.walk(f);
                }
            }
        }
    }
}

/// Why an expression could not be evaluated exactly. Every variant becomes an
/// `UNVERIFIED` finding, never a number.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EvalError {
    /// A variable with no binding in the research record (§6b.3).
    UnboundVariable(String),
    DivisionByZero,
    /// A power whose exponent is not an integer — `2^(1/2)` is irrational and
    /// this engine does not approximate.
    NonIntegerExponent,
    /// A square root that is not exact.
    IrrationalRoot,
    /// A named function the engine does not compute.
    UnsupportedFunction(String),
    /// 128-bit overflow. Honest, and unreachable for anything a manuscript
    /// writes, but "unreachable" is not a proof.
    Overflow,
}

impl std::fmt::Display for EvalError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EvalError::UnboundVariable(v) => {
                write!(f, "no value is bound to `{v}` in the research record")
            }
            EvalError::DivisionByZero => write!(f, "division by zero"),
            EvalError::NonIntegerExponent => {
                write!(f, "a non-integer exponent is not exactly computable")
            }
            EvalError::IrrationalRoot => write!(f, "the square root is not exact"),
            EvalError::UnsupportedFunction(n) => {
                write!(f, "`{n}(…)` is not computed by the deterministic engine")
            }
            EvalError::Overflow => write!(f, "the computation exceeded exact 128-bit range"),
        }
    }
}

/// Bindings from variable name to exact value.
pub type Bindings = BTreeMap<String, Rational>;

impl Expr {
    /// Evaluate exactly. Any inexactness is an [`EvalError`], never a float.
    pub fn eval(&self, b: &Bindings) -> Result<Rational, EvalError> {
        match self {
            Expr::Num { value, .. } => Ok(*value),
            Expr::Var(v) => b.get(v).copied().ok_or_else(|| EvalError::UnboundVariable(v.clone())),
            Expr::Neg(a) => a.eval(b)?.neg().ok_or(EvalError::Overflow),
            Expr::Add(x, y) => x.eval(b)?.add(y.eval(b)?).ok_or(EvalError::Overflow),
            Expr::Sub(x, y) => x.eval(b)?.sub(y.eval(b)?).ok_or(EvalError::Overflow),
            Expr::Mul(x, y) => x.eval(b)?.mul(y.eval(b)?).ok_or(EvalError::Overflow),
            Expr::Div(x, y) => {
                let d = y.eval(b)?;
                if d.is_zero() {
                    return Err(EvalError::DivisionByZero);
                }
                x.eval(b)?.div(d).ok_or(EvalError::Overflow)
            }
            Expr::Pow(x, y) => {
                let e = y.eval(b)?;
                if !e.is_integer() {
                    return Err(EvalError::NonIntegerExponent);
                }
                let e: i32 = i32::try_from(e.numer()).map_err(|_| EvalError::Overflow)?;
                let base = x.eval(b)?;
                if base.is_zero() && e < 0 {
                    return Err(EvalError::DivisionByZero);
                }
                base.powi(e).ok_or(EvalError::Overflow)
            }
            Expr::Sqrt(x) => {
                let v = x.eval(b)?;
                exact_sqrt(v).ok_or(EvalError::IrrationalRoot)
            }
            Expr::Func(name, _) => Err(EvalError::UnsupportedFunction(name.clone())),
        }
    }
}

/// An exact rational square root, or `None`. `sqrt(9/4) = 3/2`; `sqrt(2)` is
/// not a rational and this returns `None` rather than an approximation.
fn exact_sqrt(v: Rational) -> Option<Rational> {
    if v.is_negative() {
        return None;
    }
    let isqrt = |n: i128| -> Option<i128> {
        if n < 0 {
            return None;
        }
        if n < 2 {
            return Some(n);
        }
        let mut x = (n as f64).sqrt() as i128;
        // Correct the float estimate exactly.
        while x > 0 && x.checked_mul(x)? > n {
            x -= 1;
        }
        while (x + 1).checked_mul(x + 1).is_some_and(|s| s <= n) {
            x += 1;
        }
        (x.checked_mul(x)? == n).then_some(x)
    };
    Rational::new(isqrt(v.numer())?, isqrt(v.denom())?)
}

// ---------------------------------------------------------------------------
// Canonical form
// ---------------------------------------------------------------------------

/// A canonical expression: sums of products of powers, with commutative
/// operands in a deterministic order and constants folded exactly.
///
/// Two expressions with equal canonical forms are equivalent. **The converse
/// does not hold** — `(a+b)²` and `a²+2ab+b²` canonicalise differently — which
/// is why [`super::equiv`] does not stop here.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Canon {
    /// A rational constant.
    Const(CanonRat),
    Var(String),
    /// Sorted, ≥ 2 operands, no nested `Sum`.
    Sum(Vec<Canon>),
    /// Sorted, ≥ 2 operands, no nested `Prod`.
    Prod(Vec<Canon>),
    /// Base and an integer exponent (negative encodes division).
    Pow(Box<Canon>, i32),
    /// A root or an opaque function — carried through equivalence structurally.
    Func(String, Vec<Canon>),
}

/// `Rational` with a total order, so canonical operands can be sorted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CanonRat(pub Rational);

impl PartialOrd for CanonRat {
    fn partial_cmp(&self, o: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(o))
    }
}
impl Ord for CanonRat {
    fn cmp(&self, o: &Self) -> std::cmp::Ordering {
        self.0.cmp_val(&o.0)
    }
}

impl Expr {
    /// Desugar and normalise. `Sub`/`Neg` become multiplication by −1, `Div`
    /// becomes a negative power, `Sqrt` stays a `Func` (it is not generally a
    /// rational power of a rational), nested sums and products flatten,
    /// constants fold exactly, and commutative operands sort.
    pub fn canonical(&self) -> Canon {
        let c = self.to_canon_raw();
        normalise(c)
    }

    fn to_canon_raw(&self) -> Canon {
        match self {
            Expr::Num { value, .. } => Canon::Const(CanonRat(*value)),
            Expr::Var(v) => Canon::Var(v.clone()),
            Expr::Neg(a) => {
                Canon::Prod(vec![Canon::Const(CanonRat(Rational::from_int(-1))), a.to_canon_raw()])
            }
            Expr::Add(x, y) => Canon::Sum(vec![x.to_canon_raw(), y.to_canon_raw()]),
            Expr::Sub(x, y) => Canon::Sum(vec![
                x.to_canon_raw(),
                Canon::Prod(vec![
                    Canon::Const(CanonRat(Rational::from_int(-1))),
                    y.to_canon_raw(),
                ]),
            ]),
            Expr::Mul(x, y) => Canon::Prod(vec![x.to_canon_raw(), y.to_canon_raw()]),
            Expr::Div(x, y) => {
                Canon::Prod(vec![x.to_canon_raw(), Canon::Pow(Box::new(y.to_canon_raw()), -1)])
            }
            Expr::Pow(x, y) => match y.as_ref() {
                Expr::Num { value, .. } if value.is_integer() => {
                    match i32::try_from(value.numer()) {
                        Ok(e) => Canon::Pow(Box::new(x.to_canon_raw()), e),
                        // Out of i32 range: keep it opaque rather than wrong.
                        Err(_) => Canon::Func(
                            "pow".into(),
                            vec![x.to_canon_raw(), y.to_canon_raw()],
                        ),
                    }
                }
                _ => Canon::Func("pow".into(), vec![x.to_canon_raw(), y.to_canon_raw()]),
            },
            Expr::Sqrt(a) => Canon::Func("sqrt".into(), vec![a.to_canon_raw()]),
            Expr::Func(n, args) => {
                Canon::Func(n.clone(), args.iter().map(Expr::to_canon_raw).collect())
            }
        }
    }
}

fn normalise(c: Canon) -> Canon {
    match c {
        Canon::Const(_) | Canon::Var(_) => c,
        Canon::Func(n, args) => Canon::Func(n, args.into_iter().map(normalise).collect()),
        Canon::Pow(b, e) => {
            let b = normalise(*b);
            if e == 1 {
                return b;
            }
            if e == 0 {
                return Canon::Const(CanonRat(Rational::ONE));
            }
            if let Canon::Const(CanonRat(v)) = &b {
                if let Some(p) = v.powi(e) {
                    return Canon::Const(CanonRat(p));
                }
            }
            // (x^a)^b = x^(a*b)
            if let Canon::Pow(inner, ie) = &b {
                if let Some(prod) = ie.checked_mul(e) {
                    return normalise(Canon::Pow(inner.clone(), prod));
                }
            }
            Canon::Pow(Box::new(b), e)
        }
        Canon::Sum(terms) => {
            let mut flat = Vec::new();
            let mut konst = Rational::ZERO;
            let mut overflowed = false;
            for t in terms {
                match normalise(t) {
                    Canon::Sum(inner) => flat.extend(inner),
                    Canon::Const(CanonRat(v)) => match konst.add(v) {
                        Some(k) => konst = k,
                        None => {
                            overflowed = true;
                            flat.push(Canon::Const(CanonRat(v)));
                        }
                    },
                    other => flat.push(other),
                }
            }
            // Collect like terms: c1*X + c2*X = (c1+c2)*X.
            flat = combine_like_terms(flat);
            if !konst.is_zero() || (flat.is_empty() && !overflowed) {
                flat.push(Canon::Const(CanonRat(konst)));
            }
            flat.sort();
            match flat.len() {
                0 => Canon::Const(CanonRat(Rational::ZERO)),
                1 => flat.pop().expect("len checked"),
                _ => Canon::Sum(flat),
            }
        }
        Canon::Prod(factors) => {
            let mut flat = Vec::new();
            let mut konst = Rational::ONE;
            for f in factors {
                match normalise(f) {
                    Canon::Prod(inner) => flat.extend(inner),
                    Canon::Const(CanonRat(v)) => match konst.mul(v) {
                        Some(k) => konst = k,
                        None => flat.push(Canon::Const(CanonRat(v))),
                    },
                    other => flat.push(other),
                }
            }
            if konst.is_zero() {
                return Canon::Const(CanonRat(Rational::ZERO));
            }
            flat = combine_like_factors(flat);
            if konst != Rational::ONE || flat.is_empty() {
                flat.push(Canon::Const(CanonRat(konst)));
            }
            flat.sort();
            match flat.len() {
                0 => Canon::Const(CanonRat(Rational::ONE)),
                1 => flat.pop().expect("len checked"),
                _ => Canon::Prod(flat),
            }
        }
    }
}

/// Split a product into its numeric coefficient and the rest, so `2x` and `3x`
/// can be recognised as like terms.
fn split_coefficient(c: &Canon) -> (Rational, Canon) {
    match c {
        Canon::Prod(fs) => {
            let mut k = Rational::ONE;
            let mut rest = Vec::new();
            for f in fs {
                match f {
                    Canon::Const(CanonRat(v)) => match k.mul(*v) {
                        Some(n) => k = n,
                        None => rest.push(f.clone()),
                    },
                    other => rest.push(other.clone()),
                }
            }
            let rest = match rest.len() {
                0 => Canon::Const(CanonRat(Rational::ONE)),
                1 => rest.into_iter().next().expect("len checked"),
                _ => {
                    let mut r = rest;
                    r.sort();
                    Canon::Prod(r)
                }
            };
            (k, rest)
        }
        other => (Rational::ONE, other.clone()),
    }
}

fn combine_like_terms(terms: Vec<Canon>) -> Vec<Canon> {
    let mut acc: Vec<(Canon, Rational)> = Vec::new();
    for t in terms {
        let (k, base) = split_coefficient(&t);
        match acc.iter_mut().find(|(b, _)| *b == base) {
            Some((_, total)) => match total.add(k) {
                Some(s) => *total = s,
                None => acc.push((base, k)),
            },
            None => acc.push((base, k)),
        }
    }
    acc.into_iter()
        .filter(|(_, k)| !k.is_zero())
        .map(|(base, k)| {
            if k == Rational::ONE {
                base
            } else {
                normalise(Canon::Prod(vec![Canon::Const(CanonRat(k)), base]))
            }
        })
        .collect()
}

/// `x * x^2 = x^3` — needed or `N·e·e` and `N·e²` canonicalise differently.
fn combine_like_factors(factors: Vec<Canon>) -> Vec<Canon> {
    let mut acc: Vec<(Canon, i32)> = Vec::new();
    for f in factors {
        let (base, e) = match f {
            Canon::Pow(b, e) => (*b, e),
            other => (other, 1),
        };
        match acc.iter_mut().find(|(b, _)| *b == base) {
            Some((_, total)) => match total.checked_add(e) {
                Some(s) => *total = s,
                None => acc.push((base, e)),
            },
            None => acc.push((base, e)),
        }
    }
    acc.into_iter()
        .filter(|(_, e)| *e != 0)
        .map(|(b, e)| if e == 1 { b } else { Canon::Pow(Box::new(b), e) })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn n(s: &str) -> Expr {
        let (v, d) = Rational::parse_decimal(s).unwrap();
        Expr::num(v, d)
    }
    fn v(s: &str) -> Expr {
        Expr::Var(s.into())
    }
    fn add(a: Expr, b: Expr) -> Expr {
        Expr::Add(Box::new(a), Box::new(b))
    }
    fn mul(a: Expr, b: Expr) -> Expr {
        Expr::Mul(Box::new(a), Box::new(b))
    }
    fn div(a: Expr, b: Expr) -> Expr {
        Expr::Div(Box::new(a), Box::new(b))
    }
    fn pow(a: Expr, e: i128) -> Expr {
        Expr::Pow(Box::new(a), Box::new(Expr::int(e)))
    }

    #[test]
    fn slovins_formula_evaluates_exactly_from_its_declared_variables() {
        // n = N / (1 + N·e²)
        let f = div(v("N"), add(Expr::int(1), mul(v("N"), pow(v("e"), 2))));
        let mut b = Bindings::new();
        b.insert("N".into(), Rational::from_int(237_000));
        b.insert("e".into(), Rational::parse_decimal("0.04").unwrap().0);
        let got = f.eval(&b).unwrap();
        assert_eq!(got.to_decimal_string(2), "623.36");
        assert_eq!(f.variables(), vec!["N".to_string(), "e".to_string()]);
    }

    #[test]
    fn an_unbound_variable_is_an_error_not_a_zero() {
        let f = mul(v("N"), v("e"));
        let err = f.eval(&Bindings::new()).unwrap_err();
        assert_eq!(err, EvalError::UnboundVariable("N".into()));
        assert!(err.to_string().contains('N'));
    }

    #[test]
    fn commutation_and_association_are_the_same_canonical_form() {
        assert_eq!(add(v("a"), v("b")).canonical(), add(v("b"), v("a")).canonical());
        assert_eq!(
            add(add(v("a"), v("b")), v("c")).canonical(),
            add(v("a"), add(v("b"), v("c"))).canonical()
        );
        assert_eq!(mul(v("x"), v("y")).canonical(), mul(v("y"), v("x")).canonical());
    }

    #[test]
    fn repeated_multiplication_and_a_power_agree() {
        // N·e·e vs N·e² — the exact shape Slovin's formula is written in.
        assert_eq!(
            mul(v("N"), mul(v("e"), v("e"))).canonical(),
            mul(v("N"), pow(v("e"), 2)).canonical()
        );
    }

    #[test]
    fn division_is_a_negative_power_so_a_over_b_matches_a_times_b_inverse() {
        assert_eq!(
            div(v("a"), v("b")).canonical(),
            mul(v("a"), pow(v("b"), -1)).canonical()
        );
    }

    #[test]
    fn a_sign_difference_survives_canonicalisation() {
        // The check §6b.2 names: "a sign or denominator that differs is flagged".
        assert_ne!(
            add(v("a"), v("b")).canonical(),
            Expr::Sub(Box::new(v("a")), Box::new(v("b"))).canonical()
        );
    }

    #[test]
    fn a_denominator_difference_survives_canonicalisation() {
        // n = N/(1+Ne²)  vs  n = N/(1+N)e²  — the second is what flattening
        // the OMML used to produce.
        let right = div(v("N"), add(Expr::int(1), mul(v("N"), pow(v("e"), 2))));
        let wrong = mul(div(v("N"), add(Expr::int(1), v("N"))), pow(v("e"), 2));
        assert_ne!(right.canonical(), wrong.canonical());
    }

    #[test]
    fn like_terms_collect_so_2x_plus_3x_is_5x() {
        let lhs = add(mul(Expr::int(2), v("x")), mul(Expr::int(3), v("x")));
        let rhs = mul(Expr::int(5), v("x"));
        assert_eq!(lhs.canonical(), rhs.canonical());
    }

    #[test]
    fn constants_fold_exactly_not_in_binary_floating_point() {
        let lhs = add(add(n("0.1"), n("0.2")), v("x"));
        let rhs = add(n("0.3"), v("x"));
        assert_eq!(lhs.canonical(), rhs.canonical());
    }

    #[test]
    fn a_reported_value_is_a_bare_literal_and_a_computation_is_not() {
        assert!(n("623.36").as_reported_value().is_some());
        assert_eq!(n("623.36").as_reported_value().unwrap().1, 2);
        assert!(div(v("N"), v("d")).as_reported_value().is_none());
    }

    #[test]
    fn an_irrational_root_is_an_error_and_an_exact_one_is_a_value() {
        let exact = Expr::Sqrt(Box::new(n("2.25")));
        assert_eq!(exact.eval(&Bindings::new()).unwrap().to_decimal_string(1), "1.5");
        let irrational = Expr::Sqrt(Box::new(Expr::int(2)));
        assert_eq!(irrational.eval(&Bindings::new()).unwrap_err(), EvalError::IrrationalRoot);
    }

    #[test]
    fn an_unknown_function_is_unsupported_not_ignored() {
        let f = Expr::Func("log".into(), vec![v("x")]);
        assert_eq!(
            f.eval(&Bindings::new()).unwrap_err(),
            EvalError::UnsupportedFunction("log".into())
        );
    }

    #[test]
    fn coarsest_precision_is_the_least_precise_literal_present() {
        let e = add(n("0.084"), n("0.06"));
        assert_eq!(e.coarsest_decimals(), Some(2));
        assert_eq!(v("x").coarsest_decimals(), None);
    }
}
