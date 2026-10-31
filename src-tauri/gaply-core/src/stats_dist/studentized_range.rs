//! **The studentized range: its upper tail and its quantile (Research OS,
//! Phase 8G, decision 314).** Tukey's HSD and Games-Howell take their p and
//! their simultaneous interval from here.
//!
//! **The definition, and nothing else.** With W the range of `k` independent
//! standard normals and S an independent χ_ν / √ν, Q = W / S. So
//!
//! ```text
//! P(W > w)       = k ∫ φ(z) [Φ(z)^(k−1) − (Φ(z) − Φ(z − w))^(k−1)] dz      (z over ℝ)
//! P(Q > q; k, ν) = ∫ P(W > q·s) f_ν(s) ds                                  (s over (0, ∞))
//! ```
//!
//! f_ν being the density of χ_ν / √ν. This is the integral Copenhaver and
//! Holland describe, M. D. Copenhaver and B. S. Holland, "Computation of the
//! distribution of the maximum studentized range statistic with application
//! to multiple significance testing of simple effects", Journal of
//! Statistical Computation and Simulation 30(1), 1–15, 1988,
//! <https://doi.org/10.1080/00949658808811082>, and that R. E. Lund and
//! J. R. Lund evaluate in "Algorithm AS 190: Probabilities and upper
//! quantiles for the studentized range", Applied Statistics 32(2), 204–210,
//! 1983, <https://doi.org/10.2307/2347300>. Both are cited for the method;
//! no code of either, and no published listing, is copied here (decision
//! 314). The quadrature below is this module's own.
//!
//! **The stable form, required by decision 314.** The bracket is never
//! formed as a difference of two powers, and the tail never as 1 − cdf:
//! with r = Φ(z − w) / Φ(z),
//! Φ(z)^(k−1) − (Φ(z) − Φ(z − w))^(k−1) = −Φ(z)^(k−1) · expm1((k − 1) ·
//! ln1p(−r)), and Φ(x) = erfc(−x / √2) / 2 with statrs's `erfc`.
//!
//! **The outer integral in u = ln s.** The density of u is
//! h(u) = f_ν(eᵘ) eᵘ, and with a = ν / 2
//!
//! ```text
//! ln h(u) = ½ ln a + ½ ln(2/π) − δ(a) − a · (expm1(2u) − 2u)
//! ```
//!
//! δ(a) being Stirling's remainder, ln Γ(a) − ((a − ½) ln a − a + ½ ln 2π):
//! the constants that cancel at large ν (each about ν ln ν) are cancelled on
//! paper, so h keeps its digits at ν = 1e7, where its width in u is about
//! 1/√(2ν), 2.2e-4. Below a = 10, δ is that difference itself, through
//! statrs's `ln_gamma`; from 10 on, the first five terms of Stirling's
//! series, 1/(12a) − 1/(360a³) + 1/(1260a⁵) − 1/(1680a⁷) + 1/(1188a⁹), whose
//! next term is below 2e-14 at a = 10. In u the small-ν density's long left
//! tail (h ∝ e^{νu}) and its large-ν spike both have a finite, known extent,
//! found by bisection on ln h before any node is placed, so no node grid can
//! step over the spike.
//!
//! **The quadrature, chosen and stated (decision 314).** Both integrals are
//! Gauss–Legendre, the nodes computed at run time by Newton's method on the
//! Legendre polynomial's three-term recurrence. The inner is a fixed grid:
//! [`INNER_PANELS`] equal panels over z in [−[`Z_END`], [`Z_END`]], [`NODES`]
//! points each, its weights, Φ and Φ^(k−1) computed once per call, since the
//! integrand is at most k·φ(z) whatever w is. Measured against mpmath, 10
//! panels left 2e-7 at k = 50 (the maximum's density is narrow there), 40
//! and 80 panels the same 6.3e-11. That floor is statrs's `erfc`, measured
//! against mpmath at up to 8e-11 relative for |x| from 0.7 on, the same
//! function 8E's t p rests on. The outer is adaptive with [`OUTER_NODES`]
//! points: it starts on fixed panels, a panel is accepted when its rule and
//! the sum of the rule on its two halves agree within its share of
//! [`OUTER_REL`] times the first estimate, and is halved otherwise, to
//! [`MAX_DEPTH`] levels. P(W > w) is 0 from w = [`W_NEGLIGIBLE`] on, and the
//! outer range ends where q·s reaches it.
//!
//! **Speed, decision 322.** Three changes, each measured against mpmath and
//! each leaving the gated maxima where they were (6.335e-11 for the tail,
//! 4.130e-11 for the quantile, 5.99e-11 for the tail below df 200): inner
//! nodes whose term is bounded below [`INNER_TERM_FLOOR`] are not summed;
//! the outer starts on 4 panels, not 8; and the quantile's search at finite
//! df takes its first step from the infinite-df tail's slope and then the
//! secant through its last two points. The one-way ANOVA's worst case
//! (50 levels, 100,000 rows, all three post hoc tests, release) went from
//! 29.3 s to 8.4 s. No df is rounded, and no quantile is shared between
//! pairs: each is searched at its own df to the same stopping rule.
//!
//! **The domain, measured (`tests/studentized_range_reference.rs`).** k from
//! [`SRANGE_MIN_K`] to [`SRANGE_MAX_K`], ν from [`SRANGE_MIN_DF`] to
//! `MEASURED_MAX_DF` or infinite; `None` outside, and the methods refuse with
//! their own closed code. A p is computed to the gate down to
//! [`SRANGE_P_FLOOR`]; below it the methods show "below" the floor, never
//! the number (decision 314).
//!
//! Pure, as the module above: `std` and `statrs` only, no state between
//! calls.

use std::f64::consts::{FRAC_2_PI, PI, SQRT_2};

use statrs::function::erf::erfc;
use statrs::function::gamma::ln_gamma;

use super::MEASURED_MAX_DF;

/// The fewest means the distribution is defined for.
pub const SRANGE_MIN_K: u32 = 2;
/// The most: `stats_ttest::MAX_GROUP_LEVELS`, the most levels a group may
/// have (decision 312).
pub const SRANGE_MAX_K: u32 = 50;
/// The fewest degrees of freedom: a Games-Howell pair of two groups of two
/// values has a Welch df of 1 or more.
pub const SRANGE_MIN_DF: f64 = 1.0;
/// The smallest upper-tail p computed to the gate (decision 314). Below it
/// a method shows the p as below this number.
pub const SRANGE_P_FLOOR: f64 = 1e-10;

/// Gauss–Legendre points per panel of the inner integral.
pub const NODES: usize = 10;
/// Gauss–Legendre points per panel of the outer integral.
pub const OUTER_NODES: usize = 20;
/// The inner integral's fixed panels over z in [−[`Z_END`], [`Z_END`]]:
/// width 0.5. Ten (width 2) left 2e-7 at k = 50; 80 measured as 40.
pub const INNER_PANELS: usize = 40;
/// The inner integral's range in z: its integrand is at most k·φ(z), below
/// 1e-21 outside it.
pub const Z_END: f64 = 10.0;
/// An inner node whose term can never reach this (its weight times
/// Φ(z)^(k−1), the term's bound for every w) is not summed: the
/// [`INNER_PANELS`] × [`NODES`] left out change P(W > w), and so P(Q > q), by
/// at most 400 × 1e-30, against a p of 1e-10 or more (decision 322). At
/// k = 50 that is every node below z ≈ −0.7, nearly half the grid.
pub const INNER_TERM_FLOOR: f64 = 1e-30;
/// The most times a panel is halved.
pub const MAX_DEPTH: u32 = 40;
/// The outer integral's tolerance, relative to its first estimate.
pub const OUTER_REL: f64 = 1e-11;
/// From this range on, P(W > w) is taken as 0: it is below k² e^(−w²/4),
/// 1e-40 at k = 50.
pub const W_NEGLIGIBLE: f64 = 20.0;
/// The outer integral's starting panels: 8 and 4 measured the same against
/// mpmath (decision 322), and 4 costs half.
const OUTER_PANELS: usize = 4;
/// ln h at the ends of the outer range: h below e^-78 (about 1e-34) there.
const LN_H_END: f64 = -78.0;
/// The quantile's stopping width, relative to q.
pub const QUANTILE_REL_WIDTH: f64 = 1e-11;
/// The quantile's search also stops when |ln P(Q > q) − ln p| is at most
/// this: the tail's own error is about 6e-11, so a closer bracket only
/// chases rounding.
pub const QUANTILE_LN_GAP: f64 = 1e-12;
/// The most steps of the quantile's search.
const QUANTILE_MAX_STEPS: u32 = 200;

/// The nodes and weights of the `N`-point Gauss–Legendre rule on [−1, 1].
struct Rule<const N: usize> {
    x: [f64; N],
    w: [f64; N],
}

impl<const N: usize> Rule<N> {
    /// The rule, by Newton's method from Tricomi's starting values, the
    /// polynomial and its derivative from the three-term recurrence
    /// (j + 1) P_{j+1} = (2j + 1) x P_j − j P_{j−1}.
    fn new() -> Rule<N> {
        let n = N;
        let mut rule = Rule {
            x: [0.0; N],
            w: [0.0; N],
        };
        for i in 0..n.div_ceil(2) {
            let mut x = (PI * (i as f64 + 0.75) / (n as f64 + 0.5)).cos();
            let mut dp = 0.0;
            for _ in 0..100 {
                let (p, d) = legendre(n, x);
                dp = d;
                let step = p / d;
                x -= step;
                if step.abs() <= 1e-16 {
                    break;
                }
            }
            let w = 2.0 / ((1.0 - x * x) * dp * dp);
            rule.x[i] = x;
            rule.w[i] = w;
            rule.x[n - 1 - i] = -x;
            rule.w[n - 1 - i] = w;
        }
        rule
    }

    /// The rule on [a, b].
    fn on(&self, f: &mut impl FnMut(f64) -> f64, a: f64, b: f64) -> f64 {
        let (mid, half) = ((a + b) / 2.0, (b - a) / 2.0);
        let mut sum = 0.0;
        for (x, w) in self.x.iter().zip(&self.w) {
            sum += w * f(mid + half * x);
        }
        sum * half
    }

    /// `whole`, the rule on [a, b], refined until it and its two halves
    /// agree within `tol`; the halves' sum is returned.
    fn refine(
        &self,
        f: &mut impl FnMut(f64) -> f64,
        a: f64,
        b: f64,
        whole: f64,
        tol: f64,
        depth: u32,
    ) -> f64 {
        let m = (a + b) / 2.0;
        let left = self.on(f, a, m);
        let right = self.on(f, m, b);
        let halves = left + right;
        if depth == 0 || (halves - whole).abs() <= tol || !halves.is_finite() {
            return halves;
        }
        self.refine(f, a, m, left, tol / 2.0, depth - 1)
            + self.refine(f, m, b, right, tol / 2.0, depth - 1)
    }

    /// ∫ f over the panels `edges` (ascending), each refined within its
    /// share of `rel` · the first estimate + `abs`.
    fn integrate(&self, f: &mut impl FnMut(f64) -> f64, edges: &[f64], rel: f64, abs: f64) -> f64 {
        let wholes: Vec<f64> = edges.windows(2).map(|e| self.on(f, e[0], e[1])).collect();
        let estimate: f64 = wholes.iter().sum();
        let tol = (rel * estimate.abs() + abs) / wholes.len() as f64;
        edges
            .windows(2)
            .zip(&wholes)
            .map(|(e, whole)| self.refine(f, e[0], e[1], *whole, tol, MAX_DEPTH))
            .sum()
    }
}

/// P_n(x) and P_n'(x), for |x| < 1.
fn legendre(n: usize, x: f64) -> (f64, f64) {
    let (mut p0, mut p1) = (1.0, x);
    for j in 1..n {
        let j = j as f64;
        let p2 = ((2.0 * j + 1.0) * x * p1 - j * p0) / (j + 1.0);
        p0 = p1;
        p1 = p2;
    }
    (p1, n as f64 * (x * p1 - p0) / (x * x - 1.0))
}

/// The standard normal cdf, erfc(−x / √2) / 2.
fn phi_cdf(x: f64) -> f64 {
    0.5 * erfc(-x / SQRT_2)
}

/// The standard normal density.
fn phi_pdf(x: f64) -> f64 {
    (-0.5 * x * x).exp() / (2.0 * PI).sqrt()
}

/// Whether (q, k, ν) is in the measured domain.
fn in_domain(k: u32, df: f64) -> bool {
    (SRANGE_MIN_K..=SRANGE_MAX_K).contains(&k)
        && (df == f64::INFINITY || (df >= SRANGE_MIN_DF && df <= MEASURED_MAX_DF))
}

/// The inner integral's fixed nodes for one k: at each z, the weight times
/// k·φ(z), Φ(z) and Φ(z)^(k−1), computed once per call, so that P(W > w)
/// costs one `erfc` per node.
struct Inner {
    km1: f64,
    z: Vec<f64>,
    weight: Vec<f64>,
    cdf: Vec<f64>,
    power: Vec<f64>,
}

impl Inner {
    fn new(rule: &Rule<NODES>, k: u32) -> Inner {
        let km1 = f64::from(k - 1);
        let width = 2.0 * Z_END / INNER_PANELS as f64;
        let mut inner = Inner {
            km1,
            z: Vec::new(),
            weight: Vec::new(),
            cdf: Vec::new(),
            power: Vec::new(),
        };
        for panel in 0..INNER_PANELS {
            let a = -Z_END + width * panel as f64;
            let (mid, half) = (a + width / 2.0, width / 2.0);
            for (x, w) in rule.x.iter().zip(&rule.w) {
                let z = mid + half * x;
                let cdf = phi_cdf(z);
                let weight = w * half * f64::from(k) * phi_pdf(z);
                let power = cdf.powf(km1);
                // The node's term is weight · power · (1 − bracket ratio),
                // at most weight · power whatever w is: below the floor it
                // is left out of every sum (decision 322).
                if weight * power < INNER_TERM_FLOOR {
                    continue;
                }
                inner.z.push(z);
                inner.weight.push(weight);
                inner.cdf.push(cdf);
                inner.power.push(power);
            }
        }
        inner
    }

    /// P(W > w): the range of k standard normals above `w` ≥ 0, in the
    /// stable form (the module doc).
    fn range_upper(&self, w: f64) -> f64 {
        if w <= 0.0 {
            return 1.0;
        }
        if w >= W_NEGLIGIBLE {
            return 0.0;
        }
        let mut sum = 0.0;
        for j in 0..self.z.len() {
            let (cdf, power) = (self.cdf[j], self.power[j]);
            if power == 0.0 {
                continue;
            }
            let r = phi_cdf(self.z[j] - w) / cdf;
            sum += self.weight[j] * -power * (self.km1 * (-r).ln_1p()).exp_m1();
        }
        sum.clamp(0.0, 1.0)
    }
}

/// Stirling's remainder δ(a) = ln Γ(a) − ((a − ½) ln a − a + ½ ln 2π).
fn stirling_remainder(a: f64) -> f64 {
    if a < 10.0 {
        ln_gamma(a) - ((a - 0.5) * a.ln() - a + 0.5 * (2.0 * PI).ln())
    } else {
        let a2 = a * a;
        (1.0 / 12.0
            - (1.0 / 360.0 - (1.0 / 1260.0 - (1.0 / 1680.0 - 1.0 / (1188.0 * a2)) / a2) / a2) / a2)
            / a
    }
}

/// The x in [lo, hi] where the monotone `f` crosses 0, by 200 halvings.
fn crossing(f: impl Fn(f64) -> f64, mut lo: f64, mut hi: f64) -> f64 {
    let rising = f(hi) > f(lo);
    for _ in 0..200 {
        let mid = lo + (hi - lo) / 2.0;
        if mid == lo || mid == hi {
            break;
        }
        if (f(mid) > 0.0) == rising {
            hi = mid;
        } else {
            lo = mid;
        }
    }
    lo + (hi - lo) / 2.0
}

/// P(Q > q; k, ν) for finite ν, by the outer integral in u = ln s.
fn studentized_upper_finite(rule: &Rule<OUTER_NODES>, inner: &Inner, q: f64, df: f64) -> f64 {
    let a = df / 2.0;
    let ln_c = 0.5 * a.ln() + 0.5 * FRAC_2_PI.ln() - stirling_remainder(a);
    let ln_h = move |u: f64| ln_c - a * ((2.0 * u).exp_m1() - 2.0 * u);
    // ln h rises to its peak at u = 0 and falls after it.
    let u_lo = crossing(|u| ln_h(u) - LN_H_END, -1e3, 0.0);
    let mut u_hi = crossing(|u| ln_h(u) - LN_H_END, 0.0, 50.0);
    u_hi = u_hi.min((W_NEGLIGIBLE / q).ln());
    if u_hi <= u_lo {
        return 0.0;
    }
    let mut integrand = |u: f64| {
        let h = ln_h(u).exp();
        if h == 0.0 {
            return 0.0;
        }
        inner.range_upper(q * u.exp()) * h
    };
    let width = (u_hi - u_lo) / OUTER_PANELS as f64;
    let edges: Vec<f64> = (0..=OUTER_PANELS)
        .map(|i| {
            if i == OUTER_PANELS {
                u_hi
            } else {
                u_lo + width * i as f64
            }
        })
        .collect();
    rule.integrate(&mut integrand, &edges, OUTER_REL, 0.0)
        .clamp(0.0, 1.0)
}

/// The upper tail of the studentized range, P(Q > q), for `k` means and
/// `df` degrees of freedom (infinite allowed): the module doc's integral.
/// `None` outside the measured domain, or for a q that is negative, NaN or
/// infinite; 1 at q = 0. Below [`SRANGE_P_FLOOR`] the value is returned but
/// is not to the gate: callers show it as below the floor.
pub fn studentized_range_upper(q: f64, k: u32, df: f64) -> Option<f64> {
    if !in_domain(k, df) || !q.is_finite() || q < 0.0 {
        return None;
    }
    if q == 0.0 {
        return Some(1.0);
    }
    let rule = Rule::<OUTER_NODES>::new();
    let inner = Inner::new(&Rule::<NODES>::new(), k);
    Some(upper_with(&rule, &inner, q, df))
}

/// P(Q > q) on tables already built for this k.
fn upper_with(rule: &Rule<OUTER_NODES>, inner: &Inner, q: f64, df: f64) -> f64 {
    if q == 0.0 {
        1.0
    } else if df == f64::INFINITY {
        inner.range_upper(q)
    } else {
        studentized_upper_finite(rule, inner, q, df)
    }
}

/// The q whose upper tail is `p`: the inverse of
/// [`studentized_range_upper`], by a bracketed search on ln P(Q > q) − ln p
/// (the secant through the last two points while it stays inside the
/// bracket, else regula falsi with the Illinois step, else halving), stopped
/// when the bracket is narrower than [`QUANTILE_REL_WIDTH`] · q or the gap is
/// within [`QUANTILE_LN_GAP`]. At finite df
/// the search starts from the infinite-df quantile, found the same way on
/// the inner integral alone. In the tail Q is heavier at finite df, so that
/// quantile is below the answer, and the first upper end is a Newton step
/// from it on the infinite-df tail's slope, lengthened by half and doubled
/// in distance until it is past the answer; at a large p and a small df it
/// can be above the answer (p 0.73, k 50, df 1), and the search then
/// brackets between 0 and it.
/// `None` outside the domain, or for a p that is not in [`SRANGE_P_FLOOR`],
/// 1]; 0 at p = 1.
pub fn studentized_range_quantile(p: f64, k: u32, df: f64) -> Option<f64> {
    if !in_domain(k, df) || !(SRANGE_P_FLOOR..=1.0).contains(&p) {
        return None;
    }
    if p == 1.0 {
        return Some(0.0);
    }
    let rule = Rule::<OUTER_NODES>::new();
    let inner = Inner::new(&Rule::<NODES>::new(), k);
    let infinite = solve(p, 0.0, None, |q| {
        upper_with(&rule, &inner, q, f64::INFINITY)
    })?;
    if df == f64::INFINITY {
        return Some(infinite);
    }
    // The infinite-df tail's slope in ln P at its quantile, from the inner
    // integral alone: the finite search's first step (decision 322).
    let h = START_SLOPE_STEP * infinite;
    let slope =
        (inner.range_upper(infinite + h).ln() - inner.range_upper(infinite - h).ln()) / (2.0 * h);
    solve(p, infinite, Some(slope), |q| {
        upper_with(&rule, &inner, q, df)
    })
}

/// The relative step of the infinite-df slope's central difference.
const START_SLOPE_STEP: f64 = 1e-3;
/// How far past the slope's estimate of the root the first trial point is
/// placed: the finite-df tail is the flatter one in ln P, so a step of the
/// infinite-df slope alone falls short.
const START_OVERSHOOT: f64 = 1.5;

/// The q in (0, ∞) where the falling `upper` crosses `p`, searched upward
/// from `start` when `start` is at or below it, and in (0, `start`) when
/// above. With `slope`, d ln P / dq near `start`, the first upper end is
/// [`START_OVERSHOOT`] Newton steps from `start` and grows by doubling its
/// distance; without, it is 1.25 · `start` (4 from 0) and doubles.
fn solve(p: f64, start: f64, slope: Option<f64>, upper: impl Fn(f64) -> f64) -> Option<f64> {
    let target = p.ln();
    let gap = |q: f64| upper(q).ln() - target;
    let (mut lo, mut g_lo) = (start, gap(start));
    let (mut hi, mut g_hi);
    if g_lo < 0.0 {
        // `start` is above the answer: at a large p and a small df the
        // finite-df tail is the lighter one. q = 0 has P = 1, above any p.
        (hi, g_hi) = (lo, g_lo);
        (lo, g_lo) = (0.0, gap(0.0));
    } else if g_lo >= 0.0 {
        let newton = match slope {
            Some(s) if s < 0.0 && g_lo > 0.0 && s.is_finite() => Some(START_OVERSHOOT * g_lo / -s),
            _ => None,
        };
        let mut step = newton.unwrap_or(0.0);
        hi = match newton {
            Some(d) => start + d,
            None if start > 0.0 => start * 1.25,
            None => 4.0,
        };
        g_hi = gap(hi);
        while g_hi > 0.0 {
            (lo, g_lo) = (hi, g_hi);
            if newton.is_some() {
                step *= 2.0;
                hi = lo + step;
            } else {
                hi *= 2.0;
            }
            if !(hi <= 1e15) {
                return None;
            }
            g_hi = gap(hi);
        }
    } else {
        return None;
    }
    let mut side = 0i8;
    // The last two points evaluated: the secant through them is the next
    // point when it falls inside the bracket (decision 322), the bracket's
    // regula falsi with the Illinois step otherwise.
    let ((mut a, mut g_a), (mut b, mut g_b)) = ((lo, g_lo), (hi, g_hi));
    for _ in 0..QUANTILE_MAX_STEPS {
        if hi - lo <= QUANTILE_REL_WIDTH * hi {
            break;
        }
        let through_last = b - g_b * (b - a) / (g_b - g_a);
        let secant = hi - g_hi * (hi - lo) / (g_hi - g_lo);
        let mid = if through_last.is_finite() && through_last > lo && through_last < hi {
            through_last
        } else if secant.is_finite() && secant > lo && secant < hi {
            secant
        } else {
            lo + (hi - lo) / 2.0
        };
        let g = gap(mid);
        ((a, g_a), (b, g_b)) = ((b, g_b), (mid, g));
        if g.abs() <= QUANTILE_LN_GAP {
            return (mid.is_finite() && mid > 0.0).then_some(mid);
        }
        if g > 0.0 {
            (lo, g_lo) = (mid, g);
            if side == -1 {
                g_hi /= 2.0;
            }
            side = -1;
        } else {
            (hi, g_hi) = (mid, g);
            if side == 1 {
                g_lo /= 2.0;
            }
            side = 1;
        }
    }
    let q = lo + (hi - lo) / 2.0;
    (q.is_finite() && q > 0.0).then_some(q)
}
