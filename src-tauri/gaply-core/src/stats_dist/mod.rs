//! **The methods' distribution functions: the two-tailed p of Student's t,
//! its upper 97.5% point, the upper-tail p of F, and the normal 0.975
//! quantile, each measured against R to 10,000,000 degrees of freedom
//! (Research OS, Phase 8E, decision 275).**
//!
//! Every p and quantile of the methods on the user's data comes from here:
//! the three t-tests now (`stats_ttest`, `stats_onesample`), and the one-way
//! ANOVA, the correlations and the linear regression, which uses
//! [`t_two_tailed`], [`t_quantile_975`] and [`f_upper`] (Phase 9A-1, decision
//! 670; `f_upper` measured at its df1 1 to 50 and df2 to 1e5 in
//! `tests/stats_dist_reference.rs`, 1.096e-10 · R's `pf`). Level 0 and
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
//! * [`chi_square_upper`] (Phase 8X-1, decision 576, crosstabs):
//!   `stats_l0::pvalue::chi_square_upper`, bit for bit, to
//!   [`CHI_SQUARE_MAX_DF`] (2,401), measured against R's `pchisq`.
//! * [`fisher_2x2`] (Phase 8X-1, decision 576, crosstabs): Fisher's exact
//!   test for a 2 x 2 table, R. A. Fisher, "On the interpretation of χ² from
//!   contingency tables, and the calculation of P", Journal of the Royal
//!   Statistical Society 85(1), 87–94, 1922,
//!   <https://doi.org/10.2307/2340521>: the hypergeometric probabilities of
//!   the first cell given the margins, measured against R's `fisher.test`
//!   (`make_xtab_dist_fixtures.R`'s `fisher_p.csv`).
//! * [`rank_sum_exact_tails`] (Phase 8Y-1, decision 603, the Mann-Whitney
//!   test): the exact distribution of U for two groups of 1 to
//!   [`RANK_SUM_EXACT_MAX`] (49) untied values, by a recurrence of additions
//!   from H. B. Mann and D. R. Whitney (1947), measured against R's `pwilcox`
//!   (`make_ranks_dist_fixtures.R`'s `wilcox_p.csv`).
//! * [`signed_rank_exact_tails`] (Phase 8Z-1, decision 637, the Wilcoxon
//!   signed-rank test): the exact distribution of V for 1 to
//!   [`SIGNED_RANK_EXACT_MAX`] (49) untied non-zero differences, by a recurrence
//!   of additions of integers, measured against R's `psignrank`
//!   (`make_signed_rank_dist_fixtures.R`'s `signrank_p.csv`).
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
//! crate, and of the crate only `stats_l0::pvalue`'s three functions.

use std::f64::consts::SQRT_2;

use statrs::distribution::{ContinuousCDF, Normal};
use statrs::function::erf::erfc;
use statrs::function::factorial::ln_binomial;

use crate::stats_l0::pvalue::chi_square_upper as l0_chi_square_upper;
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

/// The two-tailed p of a standard normal z, P(|Z| ≥ |z|) = erfc(|z| / √2),
/// with statrs's `erfc` (the function Hill's large-df p already uses), never
/// as 2 · (1 − Φ(|z|)), which cancels to 0 below p ≈ 1e-16 (Phase 8H,
/// decision 339; Kendall's tau-b's p). Measured against R's
/// `2 * pnorm(-abs(z))` in `tests/corr_r_golden.rs` from z 0 to 40, p from 1
/// down past 1e-300 to 0 (the gate and its measurement are there). A z that
/// is NaN gives NaN; an infinite one gives 0.
pub fn normal_two_tailed(z: f64) -> f64 {
    erfc(z.abs() / SQRT_2)
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

/// The largest df1 [`f_quantile_upper`] answers for: the largest its grid
/// measures (`tests/alpha_r_golden.rs`, Research OS, Phase 8I, decision
/// 368), and the largest a dataset reaches under the methods' 100,000 rows
/// (df1 = n − 1).
pub const F_QUANTILE_MAX_DF1: f64 = 99_999.0;

/// The F whose upper-tail p, [`f_upper`]`(f, df1, df2)`, is `p_upper`
/// (Research OS, Phase 8I, decision 368; Feldt's interval for Cronbach's
/// alpha): by bisection on [`f_upper`], as [`t_quantile`] is on
/// [`t_two_tailed`], the upper end doubled from 1 until its p is below
/// `p_upper`, then halved to adjacent doubles, and the end whose p is nearer
/// `p_upper` taken. Defined by the upper-tail p, so the 0.975 quantile is
/// `p_upper` 0.025 exactly. It checks df1 itself, which [`f_upper`] does
/// not: finite, above 0 and at most [`F_QUANTILE_MAX_DF1`]; df2 as
/// [`f_upper`] does, to [`MEASURED_MAX_DF`]. Gated on its relative error in
/// F against an R `uniroot` oracle, never on [`f_upper`]'s p: at large df1
/// statrs's incomplete beta carries up to 1.2e-9 of p, which moves F by
/// about 1e-12. `None` for a p outside (0, 1), a df out of range, or a root
/// beyond 1e300.
pub fn f_quantile_upper(p_upper: f64, df1: f64, df2: f64) -> Option<f64> {
    let df1 = measured(df1).filter(|d| *d <= F_QUANTILE_MAX_DF1)?;
    let df2 = measured(df2)?;
    if !(p_upper > 0.0 && p_upper < 1.0) {
        return None;
    }
    let fp = |f: f64| f_upper(f, df1, df2);
    let (mut lo, mut hi) = (0.0, 1.0);
    while fp(hi)? > p_upper {
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
        if fp(mid)? > p_upper {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    let f = if (fp(hi)? - p_upper).abs() < (fp(lo)? - p_upper).abs() {
        hi
    } else {
        lo
    };
    (f.is_finite() && f > 0.0).then_some(f)
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

/* ------------------------------------------------------------------------ *
 *  Crosstabs' two distributions (Research OS, Phase 8X-1, decision 576)
 * ------------------------------------------------------------------------ */

/// The largest df [`chi_square_upper`] answers for: (50 − 1)², a table of 50
/// levels by 50, the largest crosstabs reaches (decision 575), and the
/// largest df measured against R's `pchisq` (`tests/stats_dist_reference.rs`
/// over `make_xtab_dist_fixtures.R`'s `chi_p.csv`). A range of its own, as
/// [`F_QUANTILE_MAX_DF1`] is.
pub const CHI_SQUARE_MAX_DF: f64 = 2401.0;

/// Upper-tail p of a chi-square statistic `x` with `df` degrees of freedom:
/// `stats_l0::pvalue::chi_square_upper`'s value, bit for bit, the one
/// implementation, for a df from above 0 to [`CHI_SQUARE_MAX_DF`]. `None`
/// above that df, for a df that is not finite and above 0, and for an `x`
/// that is negative, NaN or infinite; `x` = 0 is 1.
pub fn chi_square_upper(x: f64, df: f64) -> Option<f64> {
    if !(df.is_finite() && df > 0.0 && df <= CHI_SQUARE_MAX_DF) {
        return None;
    }
    l0_chi_square_upper(x, df)
}

/// The largest df [`chi_square_upper_wide`] answers for: 128 · 127 / 2,
/// Bartlett's test of sphericity over the factor analysis's 128 items, and
/// the largest df measured against R's `pchisq`
/// (`tests/stats_dist_reference.rs` over `make_factor_dist_fixtures.R`'s
/// `chi_p_factor.tsv`; Research OS, Phase 9B-1, decision 699). A range of its
/// own beside [`CHI_SQUARE_MAX_DF`], which keeps crosstabs' 2,401.
pub const CHI_SQUARE_WIDE_MAX_DF: f64 = 8128.0;

/// [`chi_square_upper`] over the wider range: the upper-tail p of a
/// chi-square statistic `x` with `df` degrees of freedom,
/// `stats_l0::pvalue::chi_square_upper`'s value, bit for bit, for a df from
/// above 0 to [`CHI_SQUARE_WIDE_MAX_DF`] (decision 699). `None` above that
/// df, for a df that is not finite and above 0, and for an `x` that is
/// negative, NaN or infinite; `x` = 0 is 1.
pub fn chi_square_upper_wide(x: f64, df: f64) -> Option<f64> {
    if !(df.is_finite() && df > 0.0 && df <= CHI_SQUARE_WIDE_MAX_DF) {
        return None;
    }
    l0_chi_square_upper(x, df)
}

/// Fisher's exact test's three p values for a 2 x 2 table.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Fisher2x2 {
    /// The tables no more probable than the observed one, ties within
    /// [`FISHER_TIE_TOLERANCE`] counted.
    pub two_sided: f64,
    /// P(X ≤ a): the observed table and those with a smaller first cell.
    pub less: f64,
    /// P(X ≥ a): the observed table and those with a larger first cell.
    pub greater: f64,
}

/// The relative tolerance by which a table's probability counts as equal to
/// the observed one's in the two-sided sum: at most the observed
/// probability times 1 + 1e-7, so two tables equally probable in exact
/// arithmetic and a few units apart as doubles are both counted. Taken from
/// R's `fisher.test`'s results, which count such ties; R's help page states
/// no value, and nothing is taken from R's source (ADR-008). Recorded as a
/// departure for approval (Phase 8X-1 report, decision 576); its effect is
/// fixed by `fisher_p.csv`.
pub const FISHER_TIE_TOLERANCE: f64 = 1e-7;

/// Fisher's exact test for the table [a b; c d], by the hypergeometric
/// distribution of the first cell given the margins: in R's terms x = a,
/// m = a + c (the first column's total), n = b + d and k = a + b (the first
/// row's total). Each table's probability is taken in log space,
/// ln C(m, x) + ln C(n, k − x) with statrs's `ln_binomial`, shifted by the
/// largest and exponentiated, then divided by their sum, in one pass over
/// the possible first cells. `less` and `greater` are each summed as its
/// own tail, from its far end, never one as 1 minus the other; the
/// two-sided p is [`fisher_two_sided_sum`]. A table with a zero margin has
/// one possible table, so its three p values are 1. Each p is at most 1.
pub fn fisher_2x2(a: u64, b: u64, c: u64, d: u64) -> Fisher2x2 {
    let (m, n, k) = (a + c, b + d, a + b);
    if m == 0 || n == 0 || k == 0 || k == m + n {
        return Fisher2x2 {
            two_sided: 1.0,
            less: 1.0,
            greater: 1.0,
        };
    }
    let lo = k.saturating_sub(n);
    let hi = k.min(m);
    let ln: Vec<f64> = (lo..=hi)
        .map(|x| ln_binomial(m, x) + ln_binomial(n, k - x))
        .collect();
    let top = ln.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let densities: Vec<f64> = ln.iter().map(|l| (l - top).exp()).collect();
    let observed = (a - lo) as usize;
    let total: f64 = densities.iter().sum();
    let less: f64 = densities[..=observed].iter().sum();
    let greater: f64 = densities[observed..].iter().rev().sum();
    Fisher2x2 {
        two_sided: fisher_two_sided_sum(&densities, observed),
        less: (less / total).min(1.0),
        greater: (greater / total).min(1.0),
    }
}

/// THE tie rule of Fisher's two-sided p, on supplied densities (each
/// table's probability, or any common multiple of them): the sum of the
/// densities at most `densities[observed]` times 1 + [`FISHER_TIE_TOLERANCE`],
/// over the sum of them all, clamped at 1. A function of its own so the rule
/// is tested on chosen numbers (`tests/stats_dist_reference.rs`), since a
/// natural table rarely tells it apart.
pub fn fisher_two_sided_sum(densities: &[f64], observed: usize) -> f64 {
    let bound = densities[observed] * (1.0 + FISHER_TIE_TOLERANCE);
    let total: f64 = densities.iter().sum();
    let tail: f64 = densities.iter().filter(|d| **d <= bound).sum();
    (tail / total).min(1.0)
}

/* ------------------------------------------------------------------------ *
 *  The Mann-Whitney statistic's exact distribution (Research OS, Phase
 *  8Y-1, decision 603)
 * ------------------------------------------------------------------------ */

/// The largest group size [`rank_sum_exact_tails`] answers for: 49, below
/// the 50 values at which the exact p is no longer given (decision 602),
/// and the largest measured against R's `pwilcox`
/// (`tests/stats_dist_reference.rs` over `make_ranks_dist_fixtures.R`'s
/// `wilcox_p.csv`). A range of its own, as [`CHI_SQUARE_MAX_DF`] is.
pub const RANK_SUM_EXACT_MAX: u64 = 49;

/// The two tails of the Mann-Whitney statistic at `u`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RankSumTails {
    /// P(U ≤ u).
    pub lower: f64,
    /// P(U ≥ u).
    pub upper: f64,
}

/// The tails of the Mann-Whitney statistic U, the number of pairs (x, y)
/// with x from the first group of `n1` values, y from the second of `n2`,
/// and x above y, at `u`, when no two values are tied: R's W. Every one of
/// the C(n1 + n2, n1) orders of the two groups is equally likely, and the
/// counts of orders giving each U come from the recurrence
/// f(m, n, u) = f(m − 1, n, u − n) + f(m, n − 1, u) (the largest value is
/// the first group's, above all n of the second, or the second's), with
/// f(m, 0, u) = f(0, n, u) = 1 at u = 0 and 0 elsewhere: H. B. Mann and
/// D. R. Whitney, "On a test of whether one of two random variables is
/// stochastically larger than the other", Annals of Mathematical Statistics
/// 18(1), 50–60, 1947, <https://doi.org/10.1214/aoms/1177730491>, written
/// from its text, never from R's source (ADR-008). The counts are additions
/// of positive doubles only, one table row per first-group size, updated in
/// place for each second-group size, so nothing cancels. `lower` is the sum
/// of the counts from 0 to u and `upper` the sum from n1 n2 down to u, each
/// summed as its own tail, never one as 1 minus the other, each divided by
/// the total count once and clamped at 1. `None` for a group size of 0 or
/// above [`RANK_SUM_EXACT_MAX`], or a `u` above n1 n2.
pub fn rank_sum_exact_tails(u: u64, n1: u64, n2: u64) -> Option<RankSumTails> {
    let sizes = 1..=RANK_SUM_EXACT_MAX;
    if !(sizes.contains(&n1) && sizes.contains(&n2)) || u > n1 * n2 {
        return None;
    }
    let counts = rank_sum_counts(n1 as usize, n2 as usize);
    let u = u as usize;
    let total: f64 = counts.iter().sum();
    let lower: f64 = counts[..=u].iter().sum();
    let upper: f64 = counts[u..].iter().rev().sum();
    Some(RankSumTails {
        lower: (lower / total).min(1.0),
        upper: (upper / total).min(1.0),
    })
}

/// The largest number of non-zero differences [`signed_rank_exact_tails`]
/// answers for: 49, below the 50 pairs at which the exact p is no longer given
/// (decision 636), and the largest measured against R's `psignrank`
/// (`tests/stats_dist_reference.rs` over
/// `make_signed_rank_dist_fixtures.R`'s `signrank_p.csv`). A range of its own.
pub const SIGNED_RANK_EXACT_MAX: u64 = 49;

/// The two tails of the Wilcoxon signed-rank statistic at `v`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SignedRankTails {
    /// P(V ≤ v).
    pub lower: f64,
    /// P(V ≥ v).
    pub upper: f64,
}

/// An exact test's two-sided p from its two tails: twice the smaller, at most 1, and
/// `None` unless both are finite, each checked first, since `f64::min` turns a NaN into
/// the other operand (Phase 8Z-2, decision 652: the one home of the formula the rank
/// tests each wrote). A caller maps `None` to its own code.
pub fn exact_two_sided(lower: f64, upper: f64) -> Option<f64> {
    if !(lower.is_finite() && upper.is_finite()) {
        return None;
    }
    Some((2.0 * lower.min(upper)).min(1.0))
}

/// The tails of the Wilcoxon signed-rank statistic V, the sum of the ranks of
/// the positive differences among `n` non-zero differences with no ties, at
/// `v`. Each of the 2^n sign arrangements is equally likely, and the counts of
/// arrangements giving each V come from the recurrence
/// f(n, v) = f(n − 1, v) + f(n − 1, v − n) (the difference of rank n is negative
/// or positive), with f(0, 0) = 1: F. Wilcoxon, "Individual comparisons by
/// ranking methods", Biometrics Bulletin 1(6), 80–83, 1945,
/// <https://doi.org/10.2307/3001968>, written from its text, never from R's
/// source (ADR-008). The counts are integers below 2^49, so every double here
/// is exact. `lower` is the sum of the counts from 0 to v and `upper` the sum
/// from the top down to v, each divided by 2^n once. `None` for an `n` of 0 or
/// above [`SIGNED_RANK_EXACT_MAX`], or a `v` above n (n + 1) / 2.
pub fn signed_rank_exact_tails(v: u64, n: u64) -> Option<SignedRankTails> {
    if !(1..=SIGNED_RANK_EXACT_MAX).contains(&n) || v > n * (n + 1) / 2 {
        return None;
    }
    let counts = signed_rank_counts(n as usize);
    let v = v as usize;
    let total = (1u64 << n) as f64;
    let lower: f64 = counts[..=v].iter().sum();
    let upper: f64 = counts[v..].iter().rev().sum();
    Some(SignedRankTails {
        lower: lower / total,
        upper: upper / total,
    })
}

/// The count of the 2^n sign arrangements giving each V from 0 to n (n + 1) / 2
/// ([`signed_rank_exact_tails`]'s recurrence), in place, each rank's pass
/// taking the values from the top down.
fn signed_rank_counts(n: usize) -> Vec<f64> {
    let mut f = vec![0.0f64; n * (n + 1) / 2 + 1];
    f[0] = 1.0;
    for k in 1..=n {
        for v in (k..=k * (k + 1) / 2).rev() {
            f[v] += f[v - k];
        }
    }
    f
}

/// The count of orders of `n1` and `n2` untied values giving each U from 0
/// to n1 n2 ([`rank_sum_exact_tails`]'s recurrence).
fn rank_sum_counts(n1: usize, n2: usize) -> Vec<f64> {
    let width = n1 * n2 + 1;
    // f[m][u] for m from 0 to n1, at the second group's size n: n = 0 first.
    let mut f = vec![vec![0.0f64; width]; n1 + 1];
    for row in &mut f {
        row[0] = 1.0;
    }
    for n in 1..=n2 {
        // Ascending m: f[m - 1] already holds size n, f[m] still holds
        // n − 1, as the recurrence takes them.
        for m in 1..=n1 {
            let (done, rest) = f.split_at_mut(m);
            let (previous, row) = (&done[m - 1], &mut rest[0]);
            for v in (n..=m * n).rev() {
                row[v] += previous[v - n];
            }
        }
    }
    f.swap_remove(n1)
}

#[cfg(test)]
mod tests {
    use super::exact_two_sided;

    /// Decision 652: the one home of the exact two-sided p. Twice the smaller tail, at most
    /// 1; none unless both tails are finite, checked first, since `f64::min` turns a NaN
    /// into the other operand.
    #[test]
    fn the_exact_two_sided_p_is_twice_the_smaller_tail_and_at_most_one() {
        assert_eq!(exact_two_sided(0.25, 0.75), Some(0.5));
        assert_eq!(exact_two_sided(0.75, 0.25), Some(0.5));
        assert_eq!(exact_two_sided(0.75, 0.75), Some(1.0));
        assert_eq!(exact_two_sided(0.5000000000000001, 0.75), Some(1.0));
        assert_eq!(exact_two_sided(0.0, 1.0), Some(0.0));
    }

    #[test]
    fn a_tail_that_is_not_finite_gives_no_exact_two_sided_p() {
        for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            assert_eq!(exact_two_sided(bad, 0.25), None, "{bad} first");
            assert_eq!(exact_two_sided(0.25, bad), None, "{bad} second");
            assert_eq!(exact_two_sided(bad, bad), None, "{bad} both");
        }
    }
}
