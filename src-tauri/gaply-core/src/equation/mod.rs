//! **The mathematical verification engine — Tier 0 (§6b).**
//!
//! > LLMs reason about mathematics. Symbolic computation verifies it.
//! > — §6b, stated there as a hard constraint rather than a preference.
//!
//! Tier 0 (§4.4) is the tier that OVERRIDES everything above it, including
//! model consensus: *"Eight agents agreeing an equation is correct is not
//! evidence about the equation."* A tier with that authority has to earn it
//! structurally, not by assertion, and that shapes every decision here:
//!
//! * **Exact arithmetic.** [`rational::Rational`], not `f64`. A verifier that
//!   reported `0.1 + 0.2 ≠ 0.3` would be confidently wrong about the one thing
//!   it exists to be right about.
//! * **Refusal over inference.** Every ambiguity is `UNVERIFIED`, per §6b.3's
//!   *"stated as such, never guessed"*. `β1X1` is not split into `β1 × X1`;
//!   `f(x)` is not resolved into a call or a product; an irrational root is an
//!   error, not an approximation.
//! * **No model on the path.** See below — this is enforced by a test, not by
//!   this paragraph.
//!
//! # What is already built, and is NOT rebuilt here
//!
//! §6b.2 names five checks. **Two of them already ship.**
//! [`crate::stats_verify`] is a deterministic recompute engine — Welch's and
//! Student's *t*, one-way ANOVA, Pearson, Spearman, χ², OLS — every result
//! validated against scipy/R reference values, with no model, no proxy, no
//! network and no I/O. That is §6b.2's **numerical substitution** and
//! **formula validation**, and duplicating them would create a second numeric
//! authority for the same quantities. This module computes nothing
//! `stats_verify` computes.
//!
//! What is genuinely new is **equation extraction**, **symbolic equivalence**
//! and **dimensional consistency**.
//!
//! # The gating item is [`linear`], not OMML — measured, not assumed
//!
//! §6b.1 named an OMML reader as the gate for the whole subsystem. Across the
//! six manuscripts (`examples/equation_survey.rs`, `examples/textmath_scan.rs`,
//! `examples/equation_parse_probe.rs`): **21 equation-shaped lines survive
//! `docparse` intact and zero lines of OMML exist in any of them.** Researchers
//! in this corpus type their mathematics, units included. So [`linear`] is the
//! reader that unlocks §6b; the OMML reader is the smaller, later piece
//! (3 of 17 documents), and its priority rests on a correctness argument —
//! `docparse` used to flatten OMML into fabricated digits (§11 D155) — rather
//! than on coverage.
//!
//! # NO MODEL IS ON THIS PATH, and that is checked
//!
//! `tests/equation_is_llm_free.rs` scans these files for any route to a model,
//! a proxy, the network, the database or the filesystem, and fails on the edit
//! that introduces one. A comment saying "deterministic" is not a guarantee;
//! `stats_verify` and `validate` carry the same claim in prose and it has held
//! because nobody has tried, which is not the same as being enforced.
//!
//! It is a source scan for the reason [`crate::extract`]'s regex guard is one:
//! the structural cause is checkable, deterministic, and fails on the exact
//! edit that would break it.

pub mod check;
pub mod equiv;
pub mod expr;
pub mod graph;
pub mod interval;
pub mod linear;
pub mod rational;
