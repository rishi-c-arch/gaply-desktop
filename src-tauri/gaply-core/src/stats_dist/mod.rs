//! **The methods' distribution functions: the two-tailed p of Student's t,
//! its upper 97.5% point, the upper-tail p of F, and the normal 0.975
//! quantile, each measured against R to 10,000,000 degrees of freedom
//! (Research OS, Phase 8E, decision 275).**
//!
//! Every p and quantile of the methods on the user's data comes from here:
//! the three t-tests now (`stats_ttest`, `stats_onesample`), and the one-way
//! ANOVA, the correlations and the regression when they come. Level 0 and
//! Level 1 keep `stats_l0::pvalue` as it is, and so does their own range
//! (Level 0 gives Could not check above df 1e5, decision 90): this module
//! names that engine's two functions under aliases, `l0_t_two_tailed` and
//! `l0_f_upper`, and returns their values, bit for bit, wherever they are
//! measured within their gate.
//!
//! **Two constants, one predicate.** [`LARGE_DF`] (1e4) is where the methods
//! switch to the large-sample formulas; the one private predicate `large`,
//! df > `LARGE_DF`, decides the switch for the p, the quantile and F alike.
//! [`MEASURED_MAX_DF`] (1e7) is the largest df measured against R; above it
//! every function that takes a df returns `None`. 8A measured the engine at
//! df 1e4 and below within every gate (the quantile 5.2e-13 at 1e4, the p
//! within 1e-9 to 1e5, F with df1 = 1 at df2 5e3 within 1e-9), and over
//! the gate above: the quantile 1.01e-12 at 1.5e4 and 3.58e-9 at 1e7, the
//! t p 1.4e-9 at 2e5 and 1.1e-7 at 1e7, F with df1 = 1 1.77e-9 at df2 5e4
//! (`~/gaply-corpus/os/phase8/phase8a_proposal.md` §4). So the switch is at
//! 1e4, and no number at or below it changes by a bit.
//!
//! **The sources, each implemented from its published text, never from R's
//! source (ADR-008).**
//! * [`t_two_tailed`] above `LARGE_DF`: G. W. Hill, "Algorithm 395: Student's
//!   t-distribution", Communications of the ACM 13(10), 617–619, 1970,
//!   <https://doi.org/10.1145/355598.362775> (the ACM page answered 403 on
//!   30 Oct 2026; the algorithm's large-df branch was read in Alan Miller's
//!   Fortran 90 transcription of Algorithm 395,
//!   <https://jblevins.org/mirror/amiller/cacm395.f90>, 30 Oct 2026, whose
//!   coefficients are decision 275's): with a = df − 0.5, b = 48a² and
//!   y = a · ln(1 + t²/df), z = (((((−0.4y − 3.3)y − 24)y − 85.5) /
//!   (0.8y² + 100 + b) + y + 3) / b + 1) · √y, and p = 2 · (1 − Φ(z)) =
//!   erfc(z / √2), with statrs's `erfc`. `ln_1p` takes the place of Hill's
//!   small-y branch, which took log(1 + y) only above 1e-6 because log(1 + y)
//!   loses digits there. A y of 1,500 or more gives p = 0 (the normal tail
//!   at z ≈ 38.7 is below the smallest double), so an overflowing t² gives 0,
//!   never NaN.
//! * [`t_quantile_975`] above `LARGE_DF`: [`cornish_fisher_975`], Abramowitz
//!   and Stegun, Handbook of Mathematical Functions (1964), formula 26.7.5,
//!   p. 949, read as the page's scan at
//!   <https://personal.math.ubc.ca/~cbm/aands/page_949.htm> on 30 Oct 2026:
//!   t_p ~ x_p + g1(x_p)/ν + g2(x_p)/ν² + g3(x_p)/ν³ + g4(x_p)/ν⁴, with
//!   g1 = (x³ + x)/4, g2 = (5x⁵ + 16x³ + 3x)/96, g3 = (3x⁷ + 19x⁵ + 17x³ −
//!   15x)/384 and g4 = (79x⁹ + 776x⁷ + 1482x⁵ − 1920x³ − 945x)/92160, every
//!   coefficient as the page prints it. At or below `LARGE_DF` the quantile is
//!   the bisection over `stats_l0::pvalue::t_two_tailed` that `stats_ttest`
//!   shipped in 4B-1 (decision 62's fallback), moved here verbatim: its
//!   target is still `2.0 * (1.0 - UPPER_TAIL_POINT)`, which is
//!   0.050000000000000044 and not the double 0.05 (writing 0.05 would change
//!   bits; `tests/ttest_quantile.rs` pins all 30 of them).
//! * [`f_upper`] with df1 = 1 above `LARGE_DF`: F(1, d) is the square of t
//!   with d df, so Hill's formula on f/df2 is its upper tail. Every other
//!   case is `stats_l0::pvalue::f_upper`'s value, bit for bit; with df1 of 2
//!   or more it is not gated at large df2 (8A measured df1 2, 9, 10 and 50
//!   within 1e-9 to df2 1e6), and 8G measures the range its ANOVA reaches:
//!   df1 2 to 49, df2 to 1e5, within 2e-9 · p of R's `pf`
//!   (`tests/studentized_range_reference.rs`, 1.09e-10 measured).
//! * [`normal_quantile_975`]: statrs 0.18.0's `Normal(0, 1)::inverse_cdf` at
//!   0.975, measured against R's `qnorm(0.975)`.
//! * [`t_quantile`] (Phase 8G, the Bonferroni and Games-Howell intervals): any
//!   upper point, by bisection on [`t_two_tailed`]; within 1e-10 of R's `qt`
//!   where gated, and of [`t_quantile_975`] at 0.05.
//! * [`studentized_range_upper`] and [`studentized_range_quantile`] (Phase 8G,
//!   decision 314, Tukey and Games-Howell): the module `studentized_range`,
//!   measured against an mpmath evaluation of the definition, 570 rows for k
//!   2 to 50 and df 1 to infinite, within 6.3e-11 relative down to p 1e-10.
//!
//! Measured against R 4.6.1 in `tests/stats_dist_reference.rs` over
//! `tests/fixtures/stats_dist/` (`make_dist_fixtures.R`): the quantile within
//! 1e-12 of `qt` from df 1e4 to 1e7 (the expansion itself within 1e-14 from
//! 1e3), the t p within 1e-9 · p of `pt` from df 1e4 to 1e7 for p down to
//! 1e-300, F with df1 = 1 within 1e-9 · p of `pf` from df2 5e3 to 1e7, and
//! `qnorm` within 1e-15. The reviewer's measurements against exact values
//! (mpmath, Linux x86_64) are on `docs/methods/large_samples.md`.
//!
//! Pure: no file, network, process, clock, model or log
//! (`tests/contracts_are_pure.rs`); only `std` and `statrs` from outside the
//! crate, and of the crate only `stats_l0::pvalue`'s two functions.

use std::f64::consts::SQRT_2;

use statrs::distribution::{ContinuousCDF, Normal};
use statrs::function::erf::erfc;

use crate::stats_l0::pvalue::f_upper as l0_f_upper;
use crate::stats_l0::pvalue::t_two_tailed as l0_t_two_tailed;

mod studentized_range;
pub use self::studentized_range::{
    studentized_range_quantile, studentized_range_upper, INNER_PANELS, INNER_TERM_FLOOR, MAX_DEPTH,
    NODES, OUTER_NODES, OUTER_REL, QUANTILE_LN_GAP, QUANTILE_REL_WIDTH, SRANGE_MAX_K,
    SRANGE_MIN_DF, SRANGE_MIN_K, SRANGE_P_FLOOR, W_NEGLIGIBLE, Z_END,
};

/// Above this many degrees of freedom the methods take the large-sample
/// formulas (Hill's p, the Cornish–Fisher quantile); at or below it they
/// take `stats_l0::pvalue`'s values and the bisection, bit for bit as before
/// 8E.
pub const LARGE_DF: f64 = 1e4;

/// The largest df measured against R (`tests/stats_dist_reference.rs`).
/// Above it every function that takes a df returns `None`, and the methods
/// refuse with their out-of-range code. Unreachable through a dataset in
/// the app: a df above it needs more than 10,000,002 rows.
pub const MEASURED_MAX_DF: f64 = 1e7;

/// The quantile the interval uses: 1 − (1 − 0.95) / 2.
const UPPER_TAIL_POINT: f64 = 0.975;

/// The bracket the quantile is searched in. At df = 1 the two-sided p of
/// 1e3 is about 6.4e-4, below 0.05, so every df of 1 or more has its root
/// inside.
const QUANTILE_BRACKET: [f64; 2] = [0.0, 1e3];
/// More halvings than a width of 1e3 needs to reach one unit in the last
/// place (about 62); the loop stops earlier, when the midpoint is an end.
const QUANTILE_MAX_HALVINGS: u32 = 200;

/// Above `y` = 1,500 Hill's z is about 38.7 or more, where the normal tail
/// is below the smallest double: p is 0 there, and an overflowing t² (y
/// infinite) must not reach the rational term, which would be NaN.
const HILL_UNDERFLOW_Y: f64 = 1500.0;

/// THE switch: the one predicate that decides, for the p, the quantile and
/// F alike, whether a df takes the large-sample formula.
fn large(df: f64) -> bool {
    df > LARGE_DF
}

/// A df this module answers for: finite, above 0 and at most
/// [`MEASURED_MAX_DF`]. Written `!(df <= MEASURED_MAX_DF)` nowhere: a NaN
/// fails `is_finite` first.
fn measured(df: f64) -> Option<f64> {
    (df.is_finite() && df > 0.0 && df <= MEASURED_MAX_DF).then_some(df)
}

/// Hill's (1970) large-df two-tailed p, from `t²/df` (so the t and the F
/// routes pass the same quantity) and `df`, which the caller has checked is
/// above [`LARGE_DF`]. Never NaN: a y of [`HILL_UNDERFLOW_Y`] or more is 0.
fn hill_two_tailed(t2_over_df: f64, df: f64) -> f64 {
    let a = df - 0.5;
    let b = 48.0 * a * a;
    let y = a * t2_over_df.ln_1p();
    if y >= HILL_UNDERFLOW_Y {
        return 0.0;
    }
    let z = (((((-0.4 * y - 3.3) * y - 24.0) * y - 85.5) / (0.8 * y * y + 100.0 + b) + y + 3.0)
        / b
        + 1.0)
        * y.sqrt();
    erfc(z / SQRT_2)
}

/// Two-tailed p of a t statistic with `df` degrees of freedom (fractional
/// allowed, as for Welch). At df ≤ [`LARGE_DF`] it is
/// `stats_l0::pvalue::t_two_tailed`'s value, bit for bit; above it, Hill's
/// formula (the module doc). `None` for a t that is NaN or infinite, or a
/// df that is not finite and above 0, or above [`MEASURED_MAX_DF`].
pub fn t_two_tailed(t: f64, df: f64) -> Option<f64> {
    let df = measured(df)?;
    if !t.is_finite() {
        return None;
    }
    if large(df) {
        Some(hill_two_tailed(t * t / df, df))
    } else {
        l0_t_two_tailed(t, df)
    }
}

/// The upper 97.5% point of the standard normal: statrs's `Normal(0, 1)`
/// `inverse_cdf` at 0.975, measured against R's `qnorm(0.975)` in
/// `tests/stats_dist_reference.rs`. 8H's correlations use it.
pub fn normal_quantile_975() -> f64 {
    Normal::new(0.0, 1.0)
        .expect("the standard normal: a mean of 0 and a standard deviation of 1 are finite and positive")
        .inverse_cdf(UPPER_TAIL_POINT)
}

/// The upper 97.5% point of Student's t by Abramowitz and Stegun 26.7.5,
/// four terms, about x = [`normal_quantile_975`] (the module doc). Public so
/// the reference test can check the coefficients at df 1e3, 2,500 and
/// 4,998, where the last terms are largest: `Some` for a finite df from 1e3
/// to [`MEASURED_MAX_DF`], `None` outside. The methods reach it only above
/// [`LARGE_DF`], through [`t_quantile_975`].
pub fn cornish_fisher_975(df: f64) -> Option<f64> {
    let df = measured(df).filter(|df| *df >= 1e3)?;
    let x = normal_quantile_975();
    let (x3, x5, x7, x9) = (x * x * x, x * x * x * x * x, x.powi(7), x.powi(9));
    let g1 = (x3 + x) / 4.0;
    let g2 = (5.0 * x5 + 16.0 * x3 + 3.0 * x) / 96.0;
    let g3 = (3.0 * x7 + 19.0 * x5 + 17.0 * x3 - 15.0 * x) / 384.0;
    let g4 = (79.0 * x9 + 776.0 * x7 + 1482.0 * x5 - 1920.0 * x3 - 945.0 * x) / 92160.0;
    Some(x + g1 / df + g2 / (df * df) + g3 / (df * df * df) + g4 / (df * df * df * df))
}

/// The upper 97.5% point of Student's t with `df` degrees of freedom: at
/// df ≤ [`LARGE_DF`] the q whose two-sided p,
/// `stats_l0::pvalue::t_two_tailed(q, df)`, is 0.05, found by bisection on
/// [0, 1e3] (decision 62's fallback, moved from `stats_ttest` verbatim in
/// 8E: statrs's own `StudentsT::inverse_cdf` was measured first and did not
/// meet the gate, within 6.1e-13 of R's `qt` up to df 4,998 but 3.5e-6 off
/// at df 1e5 and 6.8e-2 at 1e7, Phase 4B-1 report); above it
/// [`cornish_fisher_975`]. `None` for a df that is not finite and above 0,
/// above [`MEASURED_MAX_DF`], or whose root is not in the bracket. Measured
/// against R's `qt` in `tests/ttest_quantile.rs` and
/// `tests/stats_dist_reference.rs`.
pub fn t_quantile_975(df: f64) -> Option<f64> {
    let df = measured(df)?;
    if large(df) {
        cornish_fisher_975(df)
    } else {
        bisection_975(df)
    }
}

/// 4B-1's bisection, as it was in `stats_ttest` (its guard is
/// [`t_quantile_975`]'s now).
fn bisection_975(df: f64) -> Option<f64> {
    let target = 2.0 * (1.0 - UPPER_TAIL_POINT);
    let p = |q: f64| l0_t_two_tailed(q, df);
    let [mut lo, mut hi] = QUANTILE_BRACKET;
    // p falls as q rises: above the target at lo, below it at hi.
    if !(p(lo)? > target && p(hi)? < target) {
        return None;
    }
    for _ in 0..QUANTILE_MAX_HALVINGS {
        let mid = lo + (hi - lo) / 2.0;
        if mid == lo || mid == hi {
            break;
        }
        if p(mid)? > target {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    // The end whose p is nearer the target; the lower one on a tie.
    let q = if (p(hi)? - target).abs() < (p(lo)? - target).abs() {
        hi
    } else {
        lo
    };
    (q.is_finite() && q > 0.0).then_some(q)
}

/// The t whose two-sided p, [`t_two_tailed`]`(t, df)`, is `p`, for any p in
/// (0, 1) and any df [`t_two_tailed`] answers for: by bisection at every df
/// (Research OS, Phase 8G, decision 314), the upper end doubled from 1 until
/// its p is below `p`, then halved to adjacent doubles as
/// [`t_quantile_975`]'s bisection is, and the end whose p is nearer `p`
/// taken. Bonferroni's intervals take it at 0.05 / m. Every interval at 0.05
/// keeps [`t_quantile_975`], whose target is 0.050000000000000044 and whose
/// formula above 1e4 is Cornish–Fisher, so the two are not bit-equal at
/// 0.05: `tests/studentized_range_reference.rs` measures and gates the gap.
/// `None` for a p outside (0, 1), a df [`t_two_tailed`] refuses, or a root
/// beyond 1e300.
pub fn t_quantile(p: f64, df: f64) -> Option<f64> {
    let df = measured(df)?;
    if !(p > 0.0 && p < 1.0) {
        return None;
    }
    let tp = |t: f64| t_two_tailed(t, df);
    let (mut lo, mut hi) = (0.0, 1.0);
    while tp(hi)? > p {
        lo = hi;
        hi *= 2.0;
        if hi > 1e300 {
            return None;
        }
    }
    for _ in 0..QUANTILE_MAX_HALVINGS * 6 {
        let mid = lo + (hi - lo) / 2.0;
        if mid == lo || mid == hi {
            break;
        }
        if tp(mid)? > p {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    let t = if (tp(hi)? - p).abs() < (tp(lo)? - p).abs() {
        hi
    } else {
        lo
    };
    (t.is_finite() && t > 0.0).then_some(t)
}

/// Upper-tail p of an F statistic with `df1`, `df2` degrees of freedom. With
/// df1 = 1 and df2 above [`LARGE_DF`], Hill's formula on f/df2 (F(1, d) is
/// the square of t with d df); every other case with df2 at most
/// [`MEASURED_MAX_DF`] is `stats_l0::pvalue::f_upper`'s value, bit for bit.
/// `None` for a df2 above [`MEASURED_MAX_DF`] or not finite and above 0, and
/// for an F that is negative, NaN or infinite, on both routes; F = 0 is 1.
pub fn f_upper(f: f64, df1: f64, df2: f64) -> Option<f64> {
    let df2 = measured(df2)?;
    if df1 == 1.0 && large(df2) {
        if !f.is_finite() || f < 0.0 {
            return None;
        }
        if f == 0.0 {
            return Some(1.0);
        }
        Some(hill_two_tailed(f / df2, df2))
    } else {
        l0_f_upper(f, df1, df2)
    }
}
