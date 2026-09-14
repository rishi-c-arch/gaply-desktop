//! **Tier-0 arithmetic findings — §6b.2's output.**
//!
//! > *"The output is a finding with `EpistemicStatus`, evidence pointers to
//! > both sides of the disagreement, and a reviewer verification trail. An LLM
//! > may EXPLAIN the finding in the report. It does not decide it."*
//!
//! # Why a finding carries no tier field
//!
//! §4.4 says Tier 0 overrides everything above it. Everything this module emits
//! is Tier 0 **by construction** — the purity guard
//! (`tests/equation_is_llm_free.rs`) is what makes that true, not a field — so
//! a per-finding `tier` would be a constant with a settable API, which is how a
//! Tier-0 badge ends up on something that did not earn it. The report layer
//! attaches [`CertaintyTier::MathematicallyCertain`](crate::report::CertaintyTier)
//! when it composes; the engine states what it found.
//!
//! # The order the checks run in, which is the whole design
//!
//! 1. **Is this a claim at all?** `DO (mg/L) = (Vtitrant × N × 8000) / Vsample`
//!    defines `DO`; it asserts nothing to verify. A [`ClaimKind::Definition`]
//!    is not a finding, and it is what the equation graph binds from.
//! 2. **Do both sides compute?** If so, compare exactly.
//! 3. **Is the difference display rounding?** When the computed value ROUNDS to
//!    the reported one at the precision the author displayed, there is no
//!    discrepancy to explain and nothing is reported. Slovin's
//!    `237,000/380.2 = 623.36` (exactly 623.35613…) is this case, and it is the
//!    negative control for the whole engine.
//! 4. **Otherwise, both readings.** The decimals are ambiguous by construction
//!    (§11 D156): judge under `AsWritten` and `AsRounded` and report which
//!    agree. Never pick one silently.
//! 5. **Free variables instead of numbers?** Then it is an algebraic identity,
//!    and [`super::equiv`] decides it — with a witness when it is false.

use crate::epistemic::{Agreement, DecimalReading, EpistemicStatus};

use super::equiv::{self, Equivalence};
use super::expr::{Bindings, Expr};
use super::interval::{eval_interval, Interval, IntervalBindings};
use super::linear::{Equation, Side};
use super::rational::Rational;

/// A value the research record supplies, in BOTH readings at once.
///
/// **A binding carries the precision of the literal it came from.** `e = 0.04`
/// read out of a declaration is exactly as ambiguous as `0.04` written inline,
/// and a binding stored as a bare `Rational` silently becomes a zero-width
/// interval — which narrows the rounded reading, makes the sides disjoint, and
/// turns a correct manuscript into a `DETECTED` finding. That is the failure
/// this type exists to make impossible.
///
/// The two maps are only ever written through [`BoundValues::insert`], so they
/// cannot drift apart — §11 D156's rule about one concept with two spellings,
/// applied to a data structure.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BoundValues {
    exact: Bindings,
    intervals: IntervalBindings,
}

impl BoundValues {
    pub fn new() -> BoundValues {
        BoundValues::default()
    }

    /// Bind `name`, stating how many decimal places the SOURCE displayed.
    /// `decimals == 0` means an exact integer — see [`Interval::of_literal`].
    pub fn insert(&mut self, name: impl Into<String>, value: Rational, decimals: u32) {
        let name = name.into();
        if let Some(i) = Interval::of_literal(value, decimals) {
            self.intervals.insert(name.clone(), i);
        }
        self.exact.insert(name, value);
    }

    pub fn is_empty(&self) -> bool {
        self.exact.is_empty()
    }
    pub fn exact(&self) -> &Bindings {
        &self.exact
    }
    pub fn intervals(&self) -> &IntervalBindings {
        &self.intervals
    }
    pub fn contains(&self, name: &str) -> bool {
        self.exact.contains_key(name)
    }
}

/// What kind of statement one `=` makes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClaimKind {
    /// A name is being given a formula. Nothing to verify; everything to bind.
    Definition { name: String },
    /// Both sides reduce to numbers.
    Numeric,
    /// Both sides carry free variables — an identity that holds always or not.
    Identity,
}

/// One reading's verdict, with the numbers behind it, so a reader never has to
/// take the conclusion on trust.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReadingOutcome {
    pub reading: DecimalReading,
    pub holds: bool,
    /// What each side came to under this reading, rendered as the manuscript
    /// would write it.
    pub left: String,
    pub right: String,
}

/// A pointer to one side of the disagreement — §6b's "evidence pointers to
/// both sides". Verbatim source text, never a re-rendering of the parse.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvidencePointer {
    /// The side's text exactly as it appears in the manuscript.
    pub text: String,
    /// Its value, where one could be computed.
    pub value: Option<String>,
}

/// A Tier-0 finding about one `=`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArithmeticFinding {
    pub status: EpistemicStatus,
    pub kind: ClaimKind,
    /// The whole line as the manuscript wrote it.
    pub source_line: String,
    /// Which `=` in a chain, zero-based — §6b.2 wants the disagreement located.
    pub claim_index: usize,
    pub left: EvidencePointer,
    pub right: EvidencePointer,
    /// Both readings, always both, even when they agree.
    pub readings: Vec<ReadingOutcome>,
    /// One sentence naming the evidence. Never "possible inconsistency".
    pub message: String,
    /// What a reviewer does to check this by hand, step by step. §6b's
    /// "reviewer verification trail".
    pub trail: Vec<String>,
}

impl ArithmeticFinding {
    /// Does this reach the author? Confirmed and Unverified do not: a report
    /// listing every check that passed buries the ones that did not.
    pub fn is_reportable(&self) -> bool {
        self.status.is_finding()
    }
}

/// Check every `=` in an equation, in order.
///
/// `bindings` supplies values the research record knows (a `where` clause, an
/// analysis record). Absent bindings are never invented — §6b.3.
pub fn check_equation(eq: &Equation, bindings: &BoundValues) -> Vec<ArithmeticFinding> {
    eq.claims()
        .enumerate()
        .map(|(i, (l, r))| check_claim(eq, i, l, r, bindings))
        .collect()
}

/// Everything reportable, in source order.
pub fn findings_for(eq: &Equation, bindings: &BoundValues) -> Vec<ArithmeticFinding> {
    check_equation(eq, bindings).into_iter().filter(ArithmeticFinding::is_reportable).collect()
}

/// **What is this equation DEFINING, if anything?**
///
/// A manuscript writing `R² = 1 − (SSres/SStot)` is giving `R²` a meaning, not
/// asserting an identity that must hold for every value of `R`. Testing it as
/// one produces a witness — `at R = 2, SSres = 3, SStot = 2.5 the left is 4 and
/// the right is −0.2` — and a `DETECTED` Tier-0 finding against the textbook
/// definition of the coefficient of determination. Measured in
/// `Disha Correction .docx`.
///
/// The predicate, generalising "one side is a bare unbound name":
///
/// > **An equality whose two sides share NO variable is a definition.** The left
/// > is being given meaning by the right; there is nothing the two jointly
/// > constrain, so there is nothing to test.
///
/// `(a+b)² = a² + b²` shares `a` and `b` across the `=` and IS a claim — it is
/// testable, it is false, and it still reports. `P_N = P_G − R`,
/// `Magnesium Hardness = Total Hardness − Calcium Hardness` and
/// `n = N/(1+Ne²)` share nothing and are definitions.
fn defined_quantity(left: &Side, right: &Side, b: &BoundValues) -> Option<String> {
    let lv = left.expr.variables();
    let rv = right.expr.variables();
    // Anything already valued by the record is not being defined here.
    let unbound = |vs: &[String]| vs.iter().any(|v| !b.contains(v));
    if !unbound(&lv) && !unbound(&rv) {
        return None;
    }
    if lv.is_empty() && rv.is_empty() {
        return None;
    }
    if lv.iter().any(|v| rv.contains(v)) {
        return None;
    }
    // Name it the way the manuscript did.
    let side = if unbound(&lv) { left } else { right };
    Some(match &side.expr {
        Expr::Var(n) => n.clone(),
        _ => side.text.clone(),
    })
}

/// How a value appears in a finding: exact when it terminates, marked when it
/// does not. See [`Rational::to_display_string`].
fn render(v: Rational) -> String {
    v.to_display_string()
}

fn check_claim(
    eq: &Equation,
    index: usize,
    left: &Side,
    right: &Side,
    b: &BoundValues,
) -> ArithmeticFinding {
    let mut f = ArithmeticFinding {
        status: EpistemicStatus::Unverified,
        kind: ClaimKind::Numeric,
        source_line: eq.text.clone(),
        claim_index: index,
        left: EvidencePointer { text: left.text.clone(), value: None },
        right: EvidencePointer { text: right.text.clone(), value: None },
        readings: Vec::new(),
        message: String::new(),
        trail: Vec::new(),
    };

    // 1. A definition asserts nothing to check.
    if let Some(name) = defined_quantity(left, right, b) {
        f.kind = ClaimKind::Definition { name: name.clone() };
        f.status = EpistemicStatus::Unverified;
        f.message = format!(
            "`{name}` is defined here, not asserted — there is no value in the research \
             record to check it against."
        );
        f.trail.push(format!("Supply a value for `{name}` to have this recomputed."));
        return f;
    }

    let lv = left.expr.eval(b.exact());
    let rv = right.expr.eval(b.exact());

    // 2/3/4. Both sides compute.
    if let (Ok(l), Ok(r)) = (&lv, &rv) {
        f.left.value = Some(render(*l));
        f.right.value = Some(render(*r));

        if l == r {
            f.status = EpistemicStatus::Confirmed;
            f.message = format!("Both sides are exactly {}.", render(*l));
            f.readings.push(ReadingOutcome {
                reading: DecimalReading::AsWritten,
                holds: true,
                left: render(*l),
                right: render(*r),
            });
            return f;
        }

        // 3. Display rounding of a REPORTED value — checked before the readings,
        //    because there is no discrepancy to explain.
        if let Some((rounded_to, reported, computed)) = display_rounding(left, right, *l, *r) {
            f.status = EpistemicStatus::Confirmed;
            f.message = format!(
                "{} is what {} rounds to at {} decimal place{}, the precision the \
                 manuscript displays.",
                render(reported),
                render(computed),
                rounded_to,
                if rounded_to == 1 { "" } else { "s" }
            );
            f.trail.push(format!(
                "Compute {} and round to {rounded_to} decimal places: {}.",
                render(computed),
                render(reported)
            ));
            return f;
        }

        // 4. Genuinely different as written. Ask what the decimals meant.
        f.readings.push(ReadingOutcome {
            reading: DecimalReading::AsWritten,
            holds: false,
            left: render(*l),
            right: render(*r),
        });

        let (rounded_holds, rounded_detail) =
            match (eval_interval(&left.expr, b.intervals()), eval_interval(&right.expr, b.intervals())) {
                (Ok(li), Ok(ri)) => (
                    li.overlaps(&ri),
                    Some((
                        format!("[{}, {}]", render(li.lo), render(li.hi)),
                        format!("[{}, {}]", render(ri.lo), render(ri.hi)),
                    )),
                ),
                _ => (false, None),
            };
        match &rounded_detail {
            Some((li, ri)) => f.readings.push(ReadingOutcome {
                reading: DecimalReading::AsRounded,
                holds: rounded_holds,
                left: li.clone(),
                right: ri.clone(),
            }),
            None => {
                // The rounded reading could not be computed. Say `UNVERIFIED`
                // rather than letting a failure to evaluate read as a refutation.
                f.status = EpistemicStatus::Unverified;
                f.message = format!(
                    "As written the two sides are {} and {}, but the rounded reading of \
                     the decimals could not be computed, so whether rounding explains \
                     the difference is unknown.",
                    render(*l),
                    render(*r)
                );
                return f;
            }
        }

        let agreement = Agreement::of(false, rounded_holds);
        f.status = agreement.status();
        let diff = l.sub(*r).map(|d| render(d.abs())).unwrap_or_else(|| "—".into());
        f.message = match f.status {
            EpistemicStatus::Detected => format!(
                "The two sides differ by {diff}, and no rounding of the values as \
                 written can make them equal."
            ),
            _ => format!(
                "As written the two sides are {} and {}, a difference of {diff}. Read as \
                 rounded values they overlap, so the difference may be rounding — which \
                 of the two the manuscript means is not recoverable from the text.",
                render(*l),
                render(*r)
            ),
        };
        f.trail.push(format!("Left, as written: {} = {}.", left.text, render(*l)));
        f.trail.push(format!("Right, as written: {} = {}.", right.text, render(*r)));
        if let Some((li, ri)) = rounded_detail {
            f.trail.push(format!(
                "Reading every displayed decimal as a rounding, the left lies in {li} and \
                 the right in {ri}; they {}.",
                if rounded_holds { "overlap" } else { "cannot meet" }
            ));
        }
        if f.status == EpistemicStatus::RequiresAuthorConfirmation {
            f.trail.push(
                "Confirm whether the values printed are the values used, or displayed \
                 roundings of more precise ones."
                    .into(),
            );
        }
        return f;
    }

    // 5. Free variables: an algebraic identity.
    f.kind = ClaimKind::Identity;
    match equiv::compare(&left.expr, &right.expr) {
        Equivalence::Identical => {
            f.status = EpistemicStatus::Confirmed;
            f.message = "The two sides are the same expression.".into();
        }
        Equivalence::AgreesAtEveryPointTested { points } => {
            f.status = EpistemicStatus::Supported;
            f.message = format!(
                "The two sides agreed at all {points} values tested. That is evidence, \
                 not a proof — the engine does not expand algebraically."
            );
        }
        Equivalence::Differs { at, left: lval, right: rval } => {
            f.status = EpistemicStatus::Detected;
            let witness = at
                .iter()
                .map(|(k, v)| format!("{k} = {}", render(*v)))
                .collect::<Vec<_>>()
                .join(", ");
            f.message = format!(
                "The two sides are not equal: at {witness} the left is {} and the right \
                 is {}.",
                render(lval),
                render(rval)
            );
            f.trail.push(format!("Substitute {witness} into `{}` to get {}.", left.text, render(lval)));
            f.trail.push(format!(
                "Substitute the same into `{}` to get {}.",
                right.text,
                render(rval)
            ));
            f.left.value = Some(render(lval));
            f.right.value = Some(render(rval));
        }
        Equivalence::Undetermined { reason } => {
            f.status = EpistemicStatus::Unverified;
            f.message = format!("The two sides could not be compared: {reason}.");
        }
    }
    f
}

/// Is the difference just the author displaying a rounded result?
///
/// Returns `(decimals, reported, computed)` when one side is a bare literal and
/// the other's exact value rounds to it at that literal's displayed precision.
fn display_rounding(
    left: &Side,
    right: &Side,
    lv: Rational,
    rv: Rational,
) -> Option<(u32, Rational, Rational)> {
    for (reported_side, reported_val, computed_val) in
        [(right, rv, lv), (left, lv, rv)]
    {
        if let Some((value, decimals)) = reported_side.expr.as_reported_value() {
            if decimals == 0 && computed_val.is_integer() {
                // Both are integers and they differ — nothing to round away.
                continue;
            }
            if computed_val.round_to(decimals) == Some(value) {
                return Some((decimals, reported_val, computed_val));
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::equation::linear::parse_equation;

    fn check(line: &str) -> Vec<ArithmeticFinding> {
        let eq = parse_equation(line).unwrap_or_else(|e| panic!("{line:?}: {e}"));
        check_equation(&eq, &BoundValues::new())
    }
    fn reportable(line: &str) -> Vec<ArithmeticFinding> {
        let eq = parse_equation(line).unwrap_or_else(|e| panic!("{line:?}: {e}"));
        findings_for(&eq, &BoundValues::new())
    }

    /// The line, verbatim, from `Corrected_Chapters_3_4_Jitesh_Agarwal.docx`
    /// §3.8.1. Word stores it as OMML; the OMML reader hands the parser this.
    const SLOVIN: &str =
        "n = 237,000/(1+237,000(0.04)²) = 237,000/(1+379.2) = 237,000/380.2 = 623.36";

    /// The line, verbatim, from `Revised Health Economics Paper FINAL (1).docx`
    /// §5.8, as `docparse` delivers it.
    const WEIGHTED: &str =
        "Weighted provision = (0.108 × 0.78) + (0.500 × 0.13) + (0.769 × 0.06) \
         + (0.810 × 0.03) = 0.084 + 0.065 + 0.046 + 0.024 = 21.9%";

    // ------------------------------------------------------------------
    // THE NEGATIVE CONTROL: a consistent statistic produces NO finding
    // ------------------------------------------------------------------

    /// Every link of Slovin's chain holds, and the last one holds only because
    /// 623.35613… is what `623.36` displays. **If this ever produces a finding,
    /// the engine is reporting correct arithmetic as an error**, which is worse
    /// than missing a real one — it is the failure that makes a Tier-0 verdict
    /// untrustworthy.
    #[test]
    fn the_slovin_chain_is_consistent_and_produces_no_finding() {
        let all = check(SLOVIN);
        assert_eq!(all.len(), 4, "four `=` signs, four claims");

        // The first link DEFINES n; there is no value in the record for it, so
        // the honest answer is UNVERIFIED rather than a verdict.
        assert_eq!(all[0].kind, ClaimKind::Definition { name: "n".into() });
        assert_eq!(all[0].status, EpistemicStatus::Unverified);

        // Every arithmetic link holds.
        for c in &all[1..] {
            assert_eq!(
                c.status,
                EpistemicStatus::Confirmed,
                "claim {} ({} = {}) should hold: {}",
                c.claim_index,
                c.left.text,
                c.right.text,
                c.message
            );
        }
        assert!(reportable(SLOVIN).is_empty(), "the negative control must be silent");
    }

    /// The last link specifically: the discrepancy vanishes under the author's
    /// own displayed precision, and the message says so in their terms.
    #[test]
    fn display_rounding_is_recognised_before_the_readings_are_consulted() {
        let all = check(SLOVIN);
        let last = all.last().expect("four claims");
        assert_eq!(last.status, EpistemicStatus::Confirmed);
        assert!(last.message.contains("623.36"), "{}", last.message);
        assert!(last.message.contains("623.35613"), "{}", last.message);
        assert!(last.message.contains("2 decimal places"), "{}", last.message);
        // And it did NOT reach the two-readings branch.
        assert!(last.readings.is_empty(), "{:?}", last.readings);
    }

    /// The engine must not mistake *any* nearby number for a rounding. Change
    /// the reported value and the same chain must fire.
    #[test]
    fn the_negative_control_fires_when_the_reported_value_is_actually_wrong() {
        let broken = SLOVIN.replace("623.36", "632.36");
        let found = reportable(&broken);
        assert_eq!(found.len(), 1, "{found:#?}");
        assert_eq!(found[0].status, EpistemicStatus::Detected);
        assert!(found[0].message.contains("no rounding"), "{}", found[0].message);
    }

    // ------------------------------------------------------------------
    // THE FIRST TIER-0 FINDING, on a real manuscript, with no model
    // ------------------------------------------------------------------

    /// `(0.108 × 0.78) + (0.500 × 0.13) + (0.769 × 0.06) + (0.810 × 0.03)` is
    /// 0.21968, which displays as 22.0%. The manuscript's next step writes
    /// 0.219 — the products rounded to three places and then summed. Read as
    /// exact values the chain is false; read as roundings the sides overlap.
    ///
    /// **Neither reading is recoverable from the text**, so the finding states
    /// both and asks (§11 D156).
    #[test]
    fn the_weighted_provision_chain_is_a_finding_that_reports_both_readings() {
        let found = reportable(WEIGHTED);
        assert_eq!(found.len(), 1, "exactly one link disagrees: {found:#?}");
        let f = &found[0];

        assert_eq!(f.status, EpistemicStatus::RequiresAuthorConfirmation);
        assert_eq!(f.claim_index, 1, "the second `=`, located");
        assert_eq!(f.kind, ClaimKind::Numeric);

        // Both readings present, always both.
        assert_eq!(f.readings.len(), 2);
        let written = &f.readings[0];
        assert_eq!(written.reading, DecimalReading::AsWritten);
        assert!(!written.holds);
        assert_eq!(written.left, "0.21968");
        assert_eq!(written.right, "0.219");
        let rounded = &f.readings[1];
        assert_eq!(rounded.reading, DecimalReading::AsRounded);
        assert!(rounded.holds, "the intervals overlap");

        // Evidence pointers to BOTH sides, verbatim from the manuscript.
        assert!(f.left.text.contains("0.108 × 0.78"), "{}", f.left.text);
        assert!(f.right.text.contains("0.084 + 0.065"), "{}", f.right.text);
        assert_eq!(f.left.value.as_deref(), Some("0.21968"));
        assert_eq!(f.right.value.as_deref(), Some("0.219"));
        assert!(f.source_line.contains("21.9%"), "the whole line is kept");

        // The difference, named rather than described.
        assert!(f.message.contains("0.00068"), "{}", f.message);

        // A reviewer verification trail that can be followed by hand.
        assert!(f.trail.len() >= 4, "{:?}", f.trail);
        assert!(f.trail.iter().any(|t| t.contains("0.21968")));
        assert!(f.trail.iter().any(|t| t.contains("overlap")));
        assert!(
            f.trail.iter().any(|t| t.contains("displayed roundings")),
            "the author must be told what question they are answering: {:?}",
            f.trail
        );
    }

    /// The third link of the same chain: `0.084 + … + 0.024 = 21.9%` is exactly
    /// true, and must stay silent. A finding engine that fires on every link of
    /// a chain containing one error is not locating anything.
    #[test]
    fn the_links_that_hold_in_the_same_chain_stay_silent() {
        let all = check(WEIGHTED);
        assert_eq!(all.len(), 3);
        assert_eq!(all[0].status, EpistemicStatus::Unverified, "a definition");
        assert!(matches!(all[0].kind, ClaimKind::Definition { .. }));
        assert_eq!(all[2].status, EpistemicStatus::Confirmed, "{}", all[2].message);
    }

    // ------------------------------------------------------------------
    // The other statuses
    // ------------------------------------------------------------------

    /// Both readings agreeing it fails is the unambiguous case: DETECTED.
    /// Integers do not widen, so no rounding can rescue this.
    #[test]
    fn an_error_no_rounding_can_explain_is_detected_outright() {
        let found = reportable("total = 40 + 30 + 20 = 100");
        assert_eq!(found.len(), 1, "{found:#?}");
        assert_eq!(found[0].status, EpistemicStatus::Detected);
        assert_eq!(found[0].readings.len(), 2);
        assert!(!found[0].readings[1].holds, "integers carry no rounding slack");
        assert!(found[0].message.contains("10"), "{}", found[0].message);
    }

    /// A false algebraic identity, decided by a witness rather than an opinion.
    #[test]
    fn a_false_identity_is_detected_and_names_the_counterexample() {
        let found = reportable("(a + b)² = a² + b²");
        assert_eq!(found.len(), 1, "{found:#?}");
        let f = &found[0];
        assert_eq!(f.status, EpistemicStatus::Detected);
        assert_eq!(f.kind, ClaimKind::Identity);
        assert!(f.message.contains("a = "), "{}", f.message);
        assert!(f.trail.iter().any(|t| t.starts_with("Substitute")), "{:?}", f.trail);
    }

    #[test]
    fn a_true_identity_is_confirmed_and_silent() {
        let all = check("N × e × e / (2 × b) = e² × N / (b × 2)");
        assert_eq!(all[0].status, EpistemicStatus::Confirmed);
        assert!(reportable("N × e × e / (2 × b) = e² × N / (b × 2)").is_empty());
    }

    /// An identity the prober agrees with but cannot prove is `SUPPORTED`, not
    /// `CONFIRMED` — the name must not claim more than the method delivers.
    #[test]
    fn an_unproven_agreement_is_supported_not_confirmed() {
        let all = check("(a + b)/c = a/c + b/c");
        assert_eq!(all[0].status, EpistemicStatus::Supported);
        assert!(all[0].message.contains("not a proof"), "{}", all[0].message);
        assert!(!all[0].is_reportable());
    }

    /// A formula from `chapter3 .docx`, verbatim. It DEFINES `DO`; with no
    /// value in the record there is nothing to verify, and the engine says so
    /// instead of inventing one.
    #[test]
    fn a_definition_from_the_corpus_is_unverified_not_a_finding() {
        let all = check("DO (mg/L) = (Vtitrant × N × 8000) / Vsample");
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].status, EpistemicStatus::Unverified);
        assert_eq!(all[0].kind, ClaimKind::Definition { name: "DO".into() });
        assert!(!all[0].is_reportable());
        assert!(all[0].message.contains("defined here"), "{}", all[0].message);
    }

    /// With the record supplying values, the same formula becomes checkable —
    /// the point of the equation graph.
    #[test]
    fn the_same_definition_is_recomputed_once_the_record_supplies_values() {
        let eq = parse_equation("DO = (Vtitrant × N × 8000) / Vsample").unwrap();
        let mut b = BoundValues::new();
        b.insert("Vtitrant", Rational::parse_decimal("5.0").unwrap().0, 1);
        b.insert("N", Rational::parse_decimal("0.025").unwrap().0, 3);
        b.insert("Vsample", Rational::from_int(200), 0);
        b.insert("DO", Rational::from_int(5), 0);
        let out = check_equation(&eq, &b);
        assert_eq!(out[0].status, EpistemicStatus::Confirmed, "{}", out[0].message);

        // A value that is wrong by more than any rounding of the stated inputs.
        b.insert("DO", Rational::from_int(60), 0);
        let out = check_equation(&eq, &b);
        assert_eq!(out[0].status, EpistemicStatus::Detected, "{}", out[0].message);

        // And one that is only wrong under the exact reading is a QUESTION,
        // not a verdict — the precision of `5.0` and `0.025` is carried
        // through the binding, which is the whole point of `BoundValues`.
        b.insert("DO", Rational::parse_decimal("5.1").unwrap().0, 1);
        let out = check_equation(&eq, &b);
        assert_eq!(
            out[0].status,
            EpistemicStatus::RequiresAuthorConfirmation,
            "{}",
            out[0].message
        );
    }

    /// Every finding names its evidence — the `consistency.rs` rule, which this
    /// engine inherits. "Possible inconsistency detected" is not a finding.
    #[test]
    fn every_reportable_finding_quotes_both_sides_and_carries_a_trail() {
        for line in [WEIGHTED, "total = 40 + 30 + 20 = 100", "(a + b)² = a² + b²"] {
            for f in reportable(line) {
                assert!(!f.left.text.is_empty() && !f.right.text.is_empty(), "{f:#?}");
                assert!(!f.message.is_empty() && !f.trail.is_empty(), "{f:#?}");
                assert!(
                    f.message.chars().any(|c| c.is_ascii_digit()),
                    "a finding must name numbers, not categories: {}",
                    f.message
                );
                assert!(f.source_line.contains('='), "{f:#?}");
            }
        }
    }
}
