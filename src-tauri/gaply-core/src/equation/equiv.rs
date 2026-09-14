//! **Symbolic equivalence — §6b.2's first check.**
//!
//! > *"the same quantity written two ways in two places is recognised as the
//! > same; a sign or denominator that differs is flagged."*
//!
//! # Why canonical form alone is not the answer
//!
//! [`Expr::canonical`](super::expr::Expr::canonical) sorts, flattens, folds
//! constants exactly and collects like terms and like factors. Equal canonical
//! forms therefore PROVE equivalence. **Unequal ones prove nothing** — `(a+b)/c`
//! and `a/c + b/c` are equal at every point either is defined and canonicalise
//! differently, because normalisation does not distribute. That limit is pinned
//! in `linear.rs` by `unequal_canonical_forms_do_not_mean_unequal_expressions`,
//! so it cannot later be mistaken for a bug.
//!
//! Making canonicalisation complete means a full rational-function normal form.
//! That is a large amount of machinery to get exactly right, and getting it
//! subtly wrong would be a confidently-wrong Tier-0 verdict — the failure mode
//! this whole engine is written against.
//!
//! # What this does instead: a WITNESS, or an honest maybe
//!
//! Evaluate both expressions at a fixed set of rational points.
//!
//! * Agree at every point and canonical forms match → [`Equivalence::Identical`].
//! * **Disagree at any point → [`Equivalence::Differs`], and the point is
//!   carried in the verdict.** This is a PROOF, and it is the direction that
//!   produces findings: a single counterexample settles it, and a reader can
//!   check the arithmetic by hand.
//! * Agree at every point but canonicalise differently →
//!   [`Equivalence::AgreesAtEveryPointTested`]. **Not a proof**, and it is not
//!   named as one.
//! * Anything not exactly computable → [`Equivalence::Undetermined`].
//!
//! The asymmetry is deliberate and matches §6b.3: the engine is allowed to be
//! certain when it holds a counterexample, and must say "as far as I tested"
//! otherwise. A Tier-0 finding is only ever raised from the certain direction.
//!
//! # Why the probe points are constants and not random
//!
//! A Tier-0 result must be reproducible: the same manuscript must produce the
//! same finding on every machine and every run. Random probing would make a
//! finding depend on a seed. The points are small distinct rationals, mixing
//! signs and fractions so a difference in sign or in a denominator shows up.

use std::collections::BTreeSet;

use super::expr::{Bindings, EvalError, Expr};
use super::rational::Rational;

/// What the engine can say about two expressions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Equivalence {
    /// Canonical forms match AND every probe agrees. Equivalent, proven.
    Identical,
    /// Every probe agreed, but the canonical forms differ, so this is evidence
    /// rather than proof. `points` is how many were tested.
    ///
    /// Deliberately NOT called `Equivalent`: §6b.3's discipline is that a name
    /// must not claim more than the method delivers.
    AgreesAtEveryPointTested { points: usize },
    /// A witness: at these bindings the two sides take different values.
    /// Certain, and checkable by hand.
    Differs { at: Vec<(String, Rational)>, left: Rational, right: Rational },
    /// Neither proven nor refuted — an unsupported function, an irrational
    /// root, an overflow, or no point at which both sides were defined.
    Undetermined { reason: String },
}

impl Equivalence {
    /// True only where the engine holds a proof. `AgreesAtEveryPointTested` is
    /// deliberately excluded — it is the case a caller must handle explicitly.
    pub fn is_proven_equivalent(&self) -> bool {
        matches!(self, Equivalence::Identical)
    }
    /// True only where the engine holds a counterexample.
    pub fn is_proven_different(&self) -> bool {
        matches!(self, Equivalence::Differs { .. })
    }
}

/// Fixed probe values. Small, distinct, mixed in sign, and including
/// non-integers so that a wrong denominator separates from a right one.
///
/// Distinctness matters: at `a = b` the expressions `a − b` and `0` agree, and
/// a probe set with a repeated value would "confirm" a difference away.
const PROBE_VALUES: &[(i128, i128)] = &[
    (2, 1),
    (3, 1),
    (5, 2),
    (-7, 3),
    (11, 1),
    (-13, 5),
    (17, 4),
    (23, 7),
];

/// Assign a distinct probe value to each variable, rotating through
/// [`PROBE_VALUES`] with an offset so that different rounds give different
/// assignments.
fn bindings_for(vars: &[String], round: usize) -> Option<Bindings> {
    let mut b = Bindings::new();
    for (i, v) in vars.iter().enumerate() {
        let (n, d) = PROBE_VALUES[(i + round * 3) % PROBE_VALUES.len()];
        b.insert(v.clone(), Rational::new(n, d)?);
    }
    Some(b)
}

/// How many distinct assignments to try. Each round shifts every variable, so
/// `ROUNDS × |PROBE_VALUES|` distinct value combinations are reachable.
const ROUNDS: usize = 5;

/// Decide what can be said about `left` and `right`.
pub fn compare(left: &Expr, right: &Expr) -> Equivalence {
    let mut vars: BTreeSet<String> = BTreeSet::new();
    vars.extend(left.variables());
    vars.extend(right.variables());
    let vars: Vec<String> = vars.into_iter().collect();

    let mut evaluated = 0usize;
    let mut last_reason: Option<String> = None;

    for round in 0..ROUNDS {
        let Some(b) = bindings_for(&vars, round) else {
            last_reason = Some("a probe value could not be constructed".into());
            continue;
        };
        match (left.eval(&b), right.eval(&b)) {
            (Ok(l), Ok(r)) => {
                evaluated += 1;
                if l != r {
                    let at = vars.iter().map(|v| (v.clone(), b[v])).collect();
                    return Equivalence::Differs { at, left: l, right: r };
                }
            }
            // A point where either side is undefined (a zero denominator, say)
            // is not evidence either way — skip it and keep the reason in case
            // NO point works.
            (Err(e), _) | (_, Err(e)) => {
                if matches!(
                    e,
                    EvalError::UnsupportedFunction(_)
                        | EvalError::NonIntegerExponent
                        | EvalError::IrrationalRoot
                ) {
                    // Structural: no probe will ever succeed. Stop early.
                    return Equivalence::Undetermined { reason: e.to_string() };
                }
                last_reason = Some(e.to_string());
            }
        }
    }

    if evaluated == 0 {
        return Equivalence::Undetermined {
            reason: last_reason
                .unwrap_or_else(|| "no probe point at which both sides are defined".into()),
        };
    }
    if left.canonical() == right.canonical() {
        Equivalence::Identical
    } else {
        Equivalence::AgreesAtEveryPointTested { points: evaluated }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::equation::linear::parse_equation;

    fn sides(s: &str) -> (Expr, Expr) {
        let eq = parse_equation(s).unwrap_or_else(|e| panic!("{s:?}: {e}"));
        (eq.sides[0].expr.clone(), eq.sides[1].expr.clone())
    }

    #[test]
    fn the_same_expression_written_two_ways_is_identical() {
        let (l, r) = sides("N × e × e / (2 × b) = e² × N / (b × 2)");
        assert_eq!(compare(&l, &r), Equivalence::Identical);
        assert!(compare(&l, &r).is_proven_equivalent());
    }

    /// §6b.2's named case. The witness is the finding's evidence.
    #[test]
    fn a_sign_in_a_denominator_produces_a_witness_not_an_opinion() {
        let (l, r) = sides("N/(1 + N × e²) = N/(1 − N × e²)");
        match compare(&l, &r) {
            Equivalence::Differs { at, left, right } => {
                assert_ne!(left, right);
                assert_eq!(at.len(), 2, "both variables are named in the witness");
                // The witness must actually reproduce the difference by hand.
                let b: Bindings = at.into_iter().collect();
                assert_eq!(l.eval(&b).unwrap(), left);
                assert_eq!(r.eval(&b).unwrap(), right);
            }
            other => panic!("expected a witness, got {other:?}"),
        }
    }

    /// The other half of §6b.2's sentence: a denominator that differs.
    /// `N/(1+N)×e²` is what flattening the Slovin OMML used to produce.
    #[test]
    fn a_misplaced_denominator_produces_a_witness() {
        let (l, r) = sides("N/(1 + N × e²) = N/(1 + N) × e²");
        assert!(compare(&l, &r).is_proven_different());
    }

    /// The case canonical form cannot settle, named honestly.
    #[test]
    fn distribution_agrees_at_every_point_and_is_not_called_proven() {
        let (l, r) = sides("(a + b)/c = a/c + b/c");
        match compare(&l, &r) {
            Equivalence::AgreesAtEveryPointTested { points } => {
                assert!(points >= 4, "too few usable probe points: {points}");
            }
            other => panic!("expected an unproven agreement, got {other:?}"),
        }
        // And it must NOT be reported as proof.
        assert!(!compare(&l, &r).is_proven_equivalent());
        assert!(!compare(&l, &r).is_proven_different());
    }

    #[test]
    fn a_binomial_expansion_agrees_and_a_wrong_one_gives_a_witness() {
        let (l, r) = sides("(a + b)² = a² + 2 × a × b + b²");
        assert!(matches!(
            compare(&l, &r),
            Equivalence::AgreesAtEveryPointTested { .. } | Equivalence::Identical
        ));
        // The classic error.
        let (l2, r2) = sides("(a + b)² = a² + b²");
        assert!(compare(&l2, &r2).is_proven_different());
    }

    #[test]
    fn an_unsupported_function_is_undetermined_not_a_difference() {
        let (l, r) = sides("log(x) × 2 = 2 × log(x)");
        assert!(matches!(compare(&l, &r), Equivalence::Undetermined { .. }));
    }

    /// A probe point is not evidence when a side is undefined there. With
    /// `1/(x − 2)` one round binds `x = 2`; that round must be skipped, not
    /// counted as a difference.
    #[test]
    fn a_point_where_a_side_is_undefined_is_skipped_not_counted() {
        let (l, r) = sides("1/(x − 2) = 1/(x − 2)");
        match compare(&l, &r) {
            Equivalence::Identical => {}
            other => panic!("expected identical, got {other:?}"),
        }
    }

    #[test]
    fn probing_is_deterministic_across_runs() {
        let (l, r) = sides("N/(1 + N × e²) = N/(1 − N × e²)");
        let first = compare(&l, &r);
        for _ in 0..20 {
            assert_eq!(compare(&l, &r), first, "a Tier-0 result must not vary");
        }
    }

    /// Probe values must be DISTINCT, or `a − b` and `0` agree everywhere and
    /// a real difference is confirmed away.
    #[test]
    fn distinct_probe_values_separate_expressions_that_agree_only_on_a_diagonal() {
        let (l, r) = sides("a − b = 0");
        assert!(compare(&l, &r).is_proven_different());
        let (l2, r2) = sides("a × b = a + b");
        assert!(compare(&l2, &r2).is_proven_different());
    }
}
