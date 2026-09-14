//! **Dimensional consistency — §6b.2's second genuinely new check.**
//!
//! > *"units on both sides of every equation, and on every variable that
//! > carries them."*
//!
//! # What the corpus carries, measured before this was designed
//!
//! `examples/unit_scan.rs` over the six manuscripts found **14 unit
//! annotations in three forms**:
//!
//! | form | count | what it is |
//! |---|---:|---|
//! | `mg/L` | 9 | a compound DIMENSION — mass per volume |
//! | `as CaCO₃` | 3 | a BASIS, and no dimension at all |
//! | `mg/L as CaCO₃` | 2 | both |
//!
//! **Five of fourteen — 36% — carry `as CaCO₃`, which is not a unit.** It says
//! the quantity is expressed as the equivalent mass of calcium carbonate. Two
//! quantities both in `mg/L`, one on a CaCO₃ basis and one not, are the same
//! dimension and are NOT interchangeable; a hardness in mg/L as CaCO₃ and a
//! chloride in mg/L cannot be added, and no exponent vector can say so.
//!
//! So a unit system built only out of dimensions has two options on this
//! corpus, and both are wrong: refuse 36% of what it meets, or drop the
//! qualifier and silently treat unlike quantities as like. [`Unit`] therefore
//! carries the basis ALONGSIDE the dimension, and the two are compared
//! separately.
//!
//! # What is NOT checked, stated rather than implied
//!
//! **Magnitude.** `mg/L` and `kg/m³` have the same dimension and differ by a
//! factor of a thousand. This module answers §6b.2's question — whether the
//! units balance — and not "is the scale right", which needs a conversion
//! table and is a different check with a different failure mode. A consistent
//! verdict here is not a claim that the numbers are.
//!
//! **`N` is deliberately not in the unit table.** Every occurrence in this
//! corpus is NORMALITY (equivalents per litre), and `N` is also the SI symbol
//! for the newton. A table that guessed would be right about this corpus and
//! wrong about the next one; an unknown unit is `UNVERIFIED`, which is the
//! §6b.3 answer.

use std::collections::BTreeMap;

use super::expr::Expr;

/// Exponents over the SI base quantities, in this order:
/// mass, length, time, amount, temperature, current, luminous intensity.
///
/// Volume is `length³`, so `mg/L` is `M·L⁻³`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Dimension([i8; 7]);

const M: usize = 0;
const L: usize = 1;
const T: usize = 2;
const N_AMOUNT: usize = 3;

impl Dimension {
    pub const DIMENSIONLESS: Dimension = Dimension([0; 7]);

    fn base(i: usize, e: i8) -> Dimension {
        let mut d = [0i8; 7];
        d[i] = e;
        Dimension(d)
    }

    pub fn is_dimensionless(&self) -> bool {
        self.0.iter().all(|e| *e == 0)
    }

    fn mul(self, o: Dimension) -> Option<Dimension> {
        let mut out = [0i8; 7];
        for i in 0..7 {
            out[i] = self.0[i].checked_add(o.0[i])?;
        }
        Some(Dimension(out))
    }

    fn div(self, o: Dimension) -> Option<Dimension> {
        let mut out = [0i8; 7];
        for i in 0..7 {
            out[i] = self.0[i].checked_sub(o.0[i])?;
        }
        Some(Dimension(out))
    }

    fn powi(self, e: i32) -> Option<Dimension> {
        let e = i8::try_from(e).ok()?;
        let mut out = [0i8; 7];
        for i in 0..7 {
            out[i] = self.0[i].checked_mul(e)?;
        }
        Some(Dimension(out))
    }

    /// A readable rendering — `M·L⁻³`, not `[1,-3,0,0,0,0,0]`. A finding has to
    /// be legible to the person who wrote the formula.
    pub fn render(&self) -> String {
        const NAMES: [&str; 7] = ["M", "L", "T", "N", "Θ", "I", "J"];
        let mut parts = Vec::new();
        for (i, e) in self.0.iter().enumerate() {
            match e {
                0 => {}
                1 => parts.push(NAMES[i].to_string()),
                _ => parts.push(format!("{}{}", NAMES[i], superscript(*e))),
            }
        }
        if parts.is_empty() {
            "dimensionless".into()
        } else {
            parts.join("·")
        }
    }
}

fn superscript(e: i8) -> String {
    let digits = ['\u{2070}', '\u{00B9}', '\u{00B2}', '\u{00B3}', '\u{2074}', '\u{2075}', '\u{2076}', '\u{2077}', '\u{2078}', '\u{2079}'];
    let mut s = String::new();
    if e < 0 {
        s.push('\u{207B}');
    }
    for c in e.unsigned_abs().to_string().chars() {
        s.push(digits[c.to_digit(10).unwrap_or(0) as usize]);
    }
    s
}

/// A unit as a manuscript writes it: a dimension, a basis, or both.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unit {
    /// `None` when the annotation states no dimension — `as CaCO₃` alone.
    pub dimension: Option<Dimension>,
    /// The basis a quantity is expressed ON. **Not a dimension.** `CaCO₃`.
    pub basis: Option<String>,
    /// The annotation verbatim, for a finding to quote.
    pub text: String,
}

impl Unit {
    pub fn dimensionless() -> Unit {
        Unit { dimension: Some(Dimension::DIMENSIONLESS), basis: None, text: "1".into() }
    }

    pub fn render(&self) -> String {
        match (&self.dimension, &self.basis) {
            (Some(d), Some(b)) => format!("{} as {b}", d.render()),
            (Some(d), None) => d.render(),
            (None, Some(b)) => format!("(dimension not stated) as {b}"),
            (None, None) => "(no unit)".into(),
        }
    }
}

/// Why a unit could not be established.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UnitError {
    /// A symbol the table does not know — refused by name, never assumed.
    UnknownUnit(String),
    /// A variable with no declared unit.
    Undeclared(String),
    /// Adding quantities of different dimension.
    Mismatch { left: String, right: String, context: &'static str },
    /// Same dimension, different basis. Dimensionally fine and scientifically
    /// wrong — the case the basis field exists for.
    BasisConflict { left: String, right: String },
    /// A power whose exponent is not a known integer.
    NonIntegerExponent,
    Unsupported(String),
}

impl std::fmt::Display for UnitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            UnitError::UnknownUnit(u) => write!(f, "`{u}` is not a unit this engine knows"),
            UnitError::Undeclared(v) => write!(f, "`{v}` carries no declared unit"),
            UnitError::Mismatch { left, right, context } => {
                write!(f, "{context} requires matching units, but the two sides are {left} and {right}")
            }
            UnitError::BasisConflict { left, right } => write!(
                f,
                "the dimensions agree but the bases differ — {left} against {right}; \
                 quantities on different bases are not interchangeable"
            ),
            UnitError::NonIntegerExponent => write!(f, "a non-integer exponent has no unit"),
            UnitError::Unsupported(x) => write!(f, "{x} has no unit this engine can derive"),
        }
    }
}

/// The unit table. Scoped to what this corpus and adjacent analytical chemistry
/// use; anything outside it is [`UnitError::UnknownUnit`], never a guess.
fn lookup(sym: &str) -> Option<Dimension> {
    Some(match sym {
        // mass
        "g" | "mg" | "µg" | "ug" | "kg" | "ng" | "pg" => Dimension::base(M, 1),
        // volume — length cubed
        "L" | "l" | "mL" | "ml" | "µL" | "uL" | "dL" | "cm3" | "m3" => Dimension::base(L, 3),
        // length
        "m" | "cm" | "mm" | "nm" | "km" | "µm" | "um" => Dimension::base(L, 1),
        // time
        "s" | "sec" | "min" | "h" | "hr" | "d" | "day" => Dimension::base(T, 1),
        // amount
        "mol" | "mmol" | "µmol" | "umol" | "eq" | "meq" => Dimension::base(N_AMOUNT, 1),
        // dimensionless
        "%" | "percent" | "ratio" | "index" | "1" => Dimension::DIMENSIONLESS,
        _ => return None,
    })
}

/// Parse an annotation such as `mg/L`, `as CaCO₃`, `mg/L as CaCO₃`, `%`.
///
/// The basis clause is split off FIRST, because it is not a unit and must not
/// reach the unit table.
pub fn parse_unit(text: &str) -> Result<Unit, UnitError> {
    let raw = text.trim();
    let (dim_part, basis) = match split_basis(raw) {
        Some((d, b)) => (d, Some(b)),
        None => (raw, None),
    };
    let dim_part = dim_part.trim();
    if dim_part.is_empty() {
        // `as CaCO₃` with no dimension. A real form — 3 of 14 in the corpus.
        return Ok(Unit { dimension: None, basis, text: raw.to_string() });
    }
    let mut dim = Dimension::DIMENSIONLESS;
    let mut denominator = false;
    for token in dim_part.split_inclusive(['/', '\u{00B7}', '*', ' ']) {
        let sep = token.chars().last().filter(|c| "/\u{00B7}* ".contains(*c));
        let sym = token.trim_end_matches(['/', '\u{00B7}', '*', ' ']).trim();
        if !sym.is_empty() {
            let d = lookup(sym).ok_or_else(|| UnitError::UnknownUnit(sym.to_string()))?;
            dim = if denominator {
                dim.div(d).ok_or_else(|| UnitError::UnknownUnit(sym.to_string()))?
            } else {
                dim.mul(d).ok_or_else(|| UnitError::UnknownUnit(sym.to_string()))?
            };
        }
        // A `/` makes every FOLLOWING factor a divisor, which is how `mg/L`
        // and `mg/L/day` are both read the way a chemist means them.
        if sep == Some('/') {
            denominator = true;
        }
    }
    Ok(Unit { dimension: Some(dim), basis, text: raw.to_string() })
}

/// Split `mg/L as CaCO₃` into `mg/L` and `CaCO₃`.
fn split_basis(s: &str) -> Option<(&str, String)> {
    let lower = s.to_lowercase();
    let at = lower.find(" as ").map(|i| (i, i + 4)).or_else(|| {
        lower.strip_prefix("as ").map(|_| (0, 3))
    })?;
    let basis = s[at.1..].trim();
    if basis.is_empty() {
        return None;
    }
    Some((&s[..at.0], basis.to_string()))
}

/// What each named quantity is measured in — built from the `(mg/L)` on the
/// left of its own defining equation.
pub type UnitEnv = BTreeMap<String, Unit>;

/// Derive the unit of an expression, or say why it cannot be derived.
pub fn unit_of(e: &Expr, env: &UnitEnv) -> Result<Unit, UnitError> {
    match e {
        // A bare number carries no unit. It is dimensionless, not unknown —
        // that is what lets `DO1 − 2` be refused and `DO1 − DO5` checked.
        Expr::Num { .. } => Ok(Unit::dimensionless()),
        Expr::Var(v) => env.get(v).cloned().ok_or_else(|| UnitError::Undeclared(v.clone())),
        Expr::Neg(a) => unit_of(a, env),
        Expr::Add(a, b) => combine_additive(a, b, env, "addition"),
        Expr::Sub(a, b) => combine_additive(a, b, env, "subtraction"),
        Expr::Mul(a, b) => {
            let (x, y) = (unit_of(a, env)?, unit_of(b, env)?);
            let dim = both_dimensions(&x, &y)?;
            Ok(Unit {
                dimension: Some(dim.0.mul(dim.1).ok_or(UnitError::NonIntegerExponent)?),
                basis: merge_basis(&x, &y)?,
                text: format!("{} × {}", x.render(), y.render()),
            })
        }
        Expr::Div(a, b) => {
            let (x, y) = (unit_of(a, env)?, unit_of(b, env)?);
            let dim = both_dimensions(&x, &y)?;
            Ok(Unit {
                dimension: Some(dim.0.div(dim.1).ok_or(UnitError::NonIntegerExponent)?),
                basis: merge_basis(&x, &y)?,
                text: format!("{} / {}", x.render(), y.render()),
            })
        }
        Expr::Pow(a, b) => {
            let exp = match b.as_ref() {
                Expr::Num { value, .. } if value.is_integer() => {
                    i32::try_from(value.numer()).map_err(|_| UnitError::NonIntegerExponent)?
                }
                _ => return Err(UnitError::NonIntegerExponent),
            };
            let x = unit_of(a, env)?;
            let d = x.dimension.ok_or_else(|| {
                UnitError::Unsupported(format!("`{}` states no dimension", x.text))
            })?;
            let text = format!("{}^{exp}", x.render());
            Ok(Unit {
                dimension: Some(d.powi(exp).ok_or(UnitError::NonIntegerExponent)?),
                basis: x.basis,
                text,
            })
        }
        // A root halves the exponents, which is only a unit when every exponent
        // is even. Refused rather than rounded.
        Expr::Sqrt(_) => Err(UnitError::Unsupported("a square root".into())),
        Expr::Func(n, _) => Err(UnitError::Unsupported(format!("`{n}(…)`"))),
    }
}

fn both_dimensions(x: &Unit, y: &Unit) -> Result<(Dimension, Dimension), UnitError> {
    match (x.dimension, y.dimension) {
        (Some(a), Some(b)) => Ok((a, b)),
        _ => Err(UnitError::Unsupported(format!(
            "`{}` or `{}` states no dimension",
            x.text, y.text
        ))),
    }
}

/// Two bases cannot be combined — `mg as CaCO₃ × mg as N` is not a quantity
/// this engine will name. One basis propagates.
fn merge_basis(x: &Unit, y: &Unit) -> Result<Option<String>, UnitError> {
    match (&x.basis, &y.basis) {
        (Some(a), Some(b)) if a != b => {
            Err(UnitError::BasisConflict { left: x.render(), right: y.render() })
        }
        (Some(a), _) => Ok(Some(a.clone())),
        (_, Some(b)) => Ok(Some(b.clone())),
        _ => Ok(None),
    }
}

fn combine_additive(
    a: &Expr,
    b: &Expr,
    env: &UnitEnv,
    context: &'static str,
) -> Result<Unit, UnitError> {
    let (x, y) = (unit_of(a, env)?, unit_of(b, env)?);
    let (dx, dy) = both_dimensions(&x, &y)?;
    if dx != dy {
        return Err(UnitError::Mismatch { left: x.render(), right: y.render(), context });
    }
    // **Same dimension is not the same quantity.** This is the check the basis
    // field exists for, and 5 of the corpus's 14 annotations need it.
    if x.basis != y.basis {
        return Err(UnitError::BasisConflict { left: x.render(), right: y.render() });
    }
    Ok(x)
}

/// The verdict for one equality.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DimensionVerdict {
    /// Both sides derive the same unit.
    Consistent { unit: String },
    /// Both sides derive a unit and they differ. Certain.
    Inconsistent { left: String, right: String, detail: String },
    /// At least one side could not be derived — §6b.3's honest answer.
    Unverified { reason: String },
}

/// Check the units on both sides of one equality.
pub fn check_sides(left: &Expr, right: &Expr, env: &UnitEnv) -> DimensionVerdict {
    let l = match unit_of(left, env) {
        Ok(u) => u,
        Err(e) => return DimensionVerdict::Unverified { reason: e.to_string() },
    };
    let r = match unit_of(right, env) {
        Ok(u) => u,
        Err(UnitError::Mismatch { left, right, context }) => {
            return DimensionVerdict::Inconsistent {
                left: left.clone(),
                right: right.clone(),
                detail: UnitError::Mismatch { left, right, context }.to_string(),
            }
        }
        Err(UnitError::BasisConflict { left, right }) => {
            return DimensionVerdict::Inconsistent {
                left: left.clone(),
                right: right.clone(),
                detail: UnitError::BasisConflict { left, right }.to_string(),
            }
        }
        Err(e) => return DimensionVerdict::Unverified { reason: e.to_string() },
    };
    match (l.dimension, r.dimension) {
        (Some(a), Some(b)) if a == b && l.basis == r.basis => {
            DimensionVerdict::Consistent { unit: l.render() }
        }
        (Some(a), Some(b)) if a == b => DimensionVerdict::Inconsistent {
            left: l.render(),
            right: r.render(),
            detail: UnitError::BasisConflict { left: l.render(), right: r.render() }.to_string(),
        },
        (Some(_), Some(_)) => DimensionVerdict::Inconsistent {
            left: l.render(),
            right: r.render(),
            detail: format!(
                "the left is {} and the right is {}",
                l.render(),
                r.render()
            ),
        },
        _ => DimensionVerdict::Unverified {
            reason: format!(
                "a side states no dimension — left {}, right {}",
                l.render(),
                r.render()
            ),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn u(s: &str) -> Unit {
        parse_unit(s).unwrap_or_else(|e| panic!("{s:?}: {e}"))
    }
    /// The multi-word names `chapter3 .docx` declares by defining them.
    fn declared_names() -> Vec<String> {
        ["Total Hardness", "Calcium Hardness", "Magnesium Hardness", "Total Alkalinity",
         "Carbonate Alkalinity", "Corrected Absorbance"]
            .iter()
            .map(|s| s.to_string())
            .collect()
    }

    fn sides(line: &str) -> (Expr, Expr) {
        let eq = crate::equation::linear::parse_equation_with(line, &declared_names())
            .unwrap_or_else(|e| panic!("{line:?}: {e}"));
        (eq.sides[0].expr.clone(), eq.sides[1].expr.clone())
    }

    /// The unit environment `chapter3 .docx` declares, built from the `(mg/L)`
    /// on the left of each defining equation — which is where a manuscript
    /// actually states a quantity's units.
    fn water_chemistry_env() -> UnitEnv {
        let mut env = UnitEnv::new();
        for (name, unit) in [
            ("DO1", "mg/L"),
            ("DO5", "mg/L"),
            ("Total Hardness", "mg/L as CaCO₃"),
            ("Calcium Hardness", "mg/L as CaCO₃"),
            ("Chloride", "mg/L"),
            ("pH", "%"),
        ] {
            env.insert(name.into(), u(unit));
        }
        env
    }

    // ---- the three forms the corpus carries ------------------------------

    #[test]
    fn a_compound_unit_parses_to_mass_per_volume() {
        let mgl = u("mg/L");
        assert_eq!(mgl.dimension.unwrap().render(), "M·L⁻³");
        assert!(mgl.basis.is_none());
    }

    /// 3 of the corpus's 14 annotations. A basis with NO dimension is a real
    /// form and must survive parsing rather than being an error.
    #[test]
    fn a_bare_basis_states_no_dimension_and_that_is_not_a_failure() {
        let a = u("as CaCO₃");
        assert_eq!(a.dimension, None);
        assert_eq!(a.basis.as_deref(), Some("CaCO₃"));
        assert!(a.render().contains("not stated"), "{}", a.render());
    }

    #[test]
    fn a_unit_with_a_basis_keeps_both_apart() {
        let h = u("mg/L as CaCO₃");
        assert_eq!(h.dimension.unwrap().render(), "M·L⁻³");
        assert_eq!(h.basis.as_deref(), Some("CaCO₃"));
        // The basis must not have reached the unit table.
        assert!(parse_unit("CaCO₃").is_err(), "a basis is not a unit");
    }

    // ---- THE CHECK THAT ONLY THE BASIS FIELD CAN MAKE --------------------

    /// **The case the whole design turns on.** Both sides are `mg/L`; one is on
    /// a CaCO₃ basis and one is not. Every dimension agrees and the subtraction
    /// is still wrong, and no exponent vector can say so.
    #[test]
    fn same_dimension_different_basis_is_caught() {
        let env = water_chemistry_env();
        let (l, r) = sides("x = Total Hardness − Chloride");
        let _ = l;
        match unit_of(&r, &env) {
            Err(UnitError::BasisConflict { left, right }) => {
                assert!(left.contains("CaCO₃"), "{left}");
                assert!(!right.contains("CaCO₃"), "{right}");
            }
            other => panic!("expected a basis conflict, got {other:?}"),
        }
    }

    /// And the same subtraction on a matching basis is fine — the check must
    /// not simply refuse everything with a basis on it.
    #[test]
    fn a_subtraction_on_a_matching_basis_is_consistent() {
        let env = water_chemistry_env();
        let (_, r) = sides("Magnesium Hardness = Total Hardness − Calcium Hardness");
        let unit = unit_of(&r, &env).expect("both operands are mg/L as CaCO₃");
        assert_eq!(unit.dimension.unwrap().render(), "M·L⁻³");
        assert_eq!(unit.basis.as_deref(), Some("CaCO₃"));
    }

    // ---- the corpus's own formulas ---------------------------------------

    /// `chapter3 .docx`, verbatim. Both operands are mg/L, so the difference is
    /// mg/L and the declared left-hand unit agrees.
    #[test]
    fn the_bod_formula_from_the_corpus_is_dimensionally_consistent() {
        let env = water_chemistry_env();
        let (_, r) = sides("BOD (mg/L) = DO1 − DO5");
        let declared = u("mg/L");
        let derived = unit_of(&r, &env).expect("both sides declared");
        assert_eq!(derived.dimension, declared.dimension);
        assert_eq!(derived.basis, declared.basis);
    }

    /// **The negative control, broken on purpose.** Subtracting a dimensionless
    /// quantity from a concentration must fail, or the check is not checking.
    #[test]
    fn subtracting_a_dimensionless_quantity_from_a_concentration_fails() {
        let env = water_chemistry_env();
        let (_, r) = sides("BOD = DO1 − pH");
        match unit_of(&r, &env) {
            Err(UnitError::Mismatch { left, right, context }) => {
                assert_eq!(context, "subtraction");
                assert_eq!(left, "M·L⁻³");
                assert_eq!(right, "dimensionless");
            }
            other => panic!("expected a mismatch, got {other:?}"),
        }
        // A bare number is dimensionless too, not unknown — which is what makes
        // `DO1 − 2` refusable rather than UNVERIFIED.
        let (_, r2) = sides("BOD = DO1 − 2");
        assert!(matches!(unit_of(&r2, &env), Err(UnitError::Mismatch { .. })));
    }

    #[test]
    fn multiplication_and_division_compose_dimensions() {
        let mut env = UnitEnv::new();
        env.insert("m".into(), u("mg"));
        env.insert("v".into(), u("L"));
        let (_, r) = sides("c = m / v");
        assert_eq!(unit_of(&r, &env).unwrap().dimension.unwrap().render(), "M·L⁻³");
        let (_, r2) = sides("a = v × v");
        assert_eq!(unit_of(&r2, &env).unwrap().dimension.unwrap().render(), "L⁶");
        let (_, r3) = sides("a = v²");
        assert_eq!(unit_of(&r3, &env).unwrap().dimension.unwrap().render(), "L⁶");
    }

    // ---- refusals --------------------------------------------------------

    /// `N` in this corpus is NORMALITY and in SI it is the newton. A table that
    /// guessed would be right here and wrong elsewhere.
    #[test]
    fn an_ambiguous_symbol_is_refused_rather_than_assumed() {
        assert_eq!(parse_unit("N"), Err(UnitError::UnknownUnit("N".into())));
    }

    #[test]
    fn a_variable_with_no_declared_unit_is_undeclared_not_dimensionless() {
        let (_, r) = sides("y = Vtitrant × 8000");
        assert_eq!(
            unit_of(&r, &UnitEnv::new()),
            Err(UnitError::Undeclared("Vtitrant".into()))
        );
    }

    #[test]
    fn a_root_and_an_unknown_function_have_no_derivable_unit() {
        let env = water_chemistry_env();
        let (_, r) = sides("y = sqrt(DO1)");
        assert!(matches!(unit_of(&r, &env), Err(UnitError::Unsupported(_))));
        let (_, r2) = sides("y = 2 × log(DO1)");
        assert!(matches!(unit_of(&r2, &env), Err(UnitError::Unsupported(_))));
    }

    // ---- the equality verdict --------------------------------------------

    #[test]
    fn the_verdict_names_both_units_when_they_disagree() {
        let env = water_chemistry_env();
        // Two bare names either side of `=` is prose by the §11 D157 guard, so
        // the test uses the arithmetic shape a manuscript actually writes.
        let (l, r) = sides("Chloride = Total Hardness − Calcium Hardness");
        match check_sides(&l, &r, &env) {
            DimensionVerdict::Inconsistent { left, right, detail } => {
                assert!(left.contains("M·L⁻³") && right.contains("CaCO₃"), "{left} / {right}");
                assert!(detail.contains("not interchangeable"), "{detail}");
            }
            other => panic!("expected inconsistent, got {other:?}"),
        }
    }

    #[test]
    fn an_undeclared_side_is_unverified_not_inconsistent() {
        let env = water_chemistry_env();
        let (l, r) = sides("Chloride = Vtitrant × 2");
        assert!(matches!(check_sides(&l, &r, &env), DimensionVerdict::Unverified { .. }));
    }

    /// A bare basis on one side means the dimension was never stated, so the
    /// comparison is UNVERIFIED — not a mismatch and not a pass.
    #[test]
    fn a_side_stating_only_a_basis_is_unverified() {
        let mut env = UnitEnv::new();
        env.insert("Total Alkalinity".into(), u("as CaCO₃"));
        env.insert("Carbonate Alkalinity".into(), u("as CaCO₃"));
        let (l, r) = sides("Total Alkalinity = 2 × Carbonate Alkalinity");
        match check_sides(&l, &r, &env) {
            DimensionVerdict::Unverified { reason } => {
                assert!(reason.contains("no dimension"), "{reason}");
            }
            other => panic!("expected unverified, got {other:?}"),
        }
    }

    /// Magnitude is NOT checked, and the test says so rather than leaving a
    /// reader to discover it from a wrong answer.
    #[test]
    fn magnitude_is_deliberately_not_checked() {
        let a = u("mg/L");
        let b = u("kg/m3");
        assert_eq!(a.dimension, b.dimension, "same dimension, a factor of 10^6 apart");
    }
}
