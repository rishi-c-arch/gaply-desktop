//! **How far a deposited file's own rounding can move a computed statistic**
//! (Phase 2, Step 8b, rule 1).
//!
//! A data file is often stored rounded: a proportion written `.55`, a score
//! written `12.1`. Level 1 computes exactly on what is stored, so its exact
//! value carries that rounding. This module measures it.
//!
//! - **Stored precision.** Per bound numeric column, `d` is the most decimals
//!   any of its cells shows in its shortest round-trip decimal form (Rust's
//!   `Display` for `f64`), capped at [`MAX_DECIMALS`]. Each of its cells is
//!   uncertain by `δ = 0.5 × 10^-d`, so a column of whole numbers by
//!   `δ = 0.5`: data deposited as whole numbers (ms, %, ages) were often
//!   analysed unrounded (evaluation fixes, part C, B-1). Until part C a
//!   whole-number column was exact, and the deep evaluation measured 527
//!   false "Not reproduced" of 600 at 0 decimals, 74 of 200 even for
//!   reaction times in ms. A column is exact only when the user says it
//!   holds exact whole numbers (counts, rating-scale points): the binding
//!   carries that answer, and the caller passes `δ = 0`. The same L and F
//!   apply: t, F and r are scale-free, so whole numbers at `δ = 0.5` are the
//!   sweep's decimal data at another scale.
//! - **The range** (evaluation fixes, part A). The ratio r is δ over the
//!   smallest standard deviation among the bound groups or columns (see
//!   [`RangeParts::ratio`]). Above [`VALIDITY_LIMIT`] the range is
//!   unbounded and any miss is not checked. Below it, the range is the hull
//!   of the first-order worst case `[S − b, S + b]`, `b = Σ |∂S/∂x_i| δ_i`,
//!   and the exact statistic at the two corners where every cell moves by δ
//!   in the direction of its derivative's sign, then widened by
//!   [`WIDENING`]. Both constants are measured, not chosen. The
//!   first-order range alone was not a bound: on data whose spread is close
//!   to the rounding step it missed by a factor of 4 (the joint evaluation's
//!   witness). The derivatives are analytic,
//!   in centred forms (deviations from the means, never raw sums of squares,
//!   which cancel on data with a large mean and a small spread), so the cost
//!   is one pass over the rows: O(n) for t, F and r, O(n·p²) for OLS.
//!   Welch's df gets the same treatment.
//! - **Spearman.** Ranks are exact unless rounding could reorder or tie two
//!   values: no ties and every gap between sorted values above `2δ` means rho
//!   is exact; otherwise the range is unbounded and any miss is not checked.
//!
//! The p range is not made here: the caller takes it from the ends of the
//! statistic range with Level 0's p engine.
//!
//! Pure arithmetic: no I/O, no model, no clock.

/// Decimals beyond this are representation noise, not a stored precision.
pub const MAX_DECIMALS: u32 = 15;

/// The decimals `v` shows in its shortest round-trip form, capped.
pub fn decimals(v: f64) -> u32 {
    if !v.is_finite() || v == v.trunc() {
        return 0;
    }
    let s = format!("{v}");
    let d = s.split_once('.').map_or(0, |(_, frac)| frac.len() as u32);
    d.min(MAX_DECIMALS)
}

/// A column's stored precision: the most decimals any of its cells shows.
pub fn column_decimals(values: impl IntoIterator<Item = f64>) -> u32 {
    values.into_iter().map(decimals).max().unwrap_or(0)
}

/// The uncertainty of one cell stored to `d` decimals: half a unit of its
/// last place, so 0.5 for whole numbers. A column the user says holds exact
/// whole numbers is not passed through here; its `δ` is 0 (part C, B-1).
pub fn half_unit(d: u32) -> f64 {
    0.5 * 10f64.powi(-(d as i32))
}

/// One bound column's numbers, as they enter a computation, with the
/// uncertainty of each.
#[derive(Debug, Clone, Copy)]
pub struct Stored<'a> {
    pub values: &'a [f64],
    pub delta: f64,
}

fn mean(xs: &[f64]) -> f64 {
    xs.iter().sum::<f64>() / xs.len() as f64
}

fn ss(xs: &[f64], m: f64) -> f64 {
    xs.iter().map(|x| (x - m).powi(2)).sum()
}

/// Student's pooled t on two groups of one column, uncertain by `delta`.
pub fn student_t_bound(a: &[f64], c: &[f64], delta: f64) -> f64 {
    if delta == 0.0 {
        return 0.0;
    }
    student_t_gradient(a, c).iter().map(|g| g.abs()).sum::<f64>() * delta
}

/// Welch's t and its df on two groups of one column: `(b_t, b_df)`.
pub fn welch_bounds(a: &[f64], c: &[f64], delta: f64) -> (f64, f64) {
    if delta == 0.0 {
        return (0.0, 0.0);
    }
    let (gt, gdf) = welch_gradients(a, c);
    (gt.iter().map(|g| g.abs()).sum::<f64>() * delta, gdf.iter().map(|g| g.abs()).sum::<f64>() * delta)
}

/// The paired t on two columns, each with its own uncertainty.
pub fn paired_t_bound(first: Stored<'_>, second: Stored<'_>) -> f64 {
    if first.delta == 0.0 && second.delta == 0.0 {
        return 0.0;
    }
    let d: Vec<f64> = first.values.iter().zip(second.values).map(|(x, y)| x - y).collect();
    paired_t_gradient(&d).iter().map(|g| g.abs()).sum::<f64>() * (first.delta + second.delta)
}

/// The one-way ANOVA's F on groups of one column.
pub fn one_way_f_bound(groups: &[&[f64]], delta: f64) -> f64 {
    if delta == 0.0 {
        return 0.0;
    }
    one_way_f_gradient(groups).iter().map(|g| g.abs()).sum::<f64>() * delta
}

/// Pearson's r on two columns.
pub fn pearson_bound(x: Stored<'_>, y: Stored<'_>) -> f64 {
    if x.delta == 0.0 && y.delta == 0.0 {
        return 0.0;
    }
    let n = x.values.len();
    pearson_gradient(x.values, y.values).iter().enumerate().map(|(i, g)| g.abs() * if i < n { x.delta } else { y.delta }).sum()
}

/// Whether rounding could reorder or tie two of a column's values.
pub fn could_reorder(col: Stored<'_>) -> bool {
    if col.delta == 0.0 {
        return false;
    }
    let mut v = col.values.to_vec();
    v.sort_by(|a, b| a.total_cmp(b));
    v.windows(2).any(|w| w[1] - w[0] <= 2.0 * col.delta)
}

/// `(X'X)^{-1}` for a small symmetric positive-definite matrix, by
/// Gauss-Jordan with partial pivoting. `None` when singular.
fn invert(a: &[Vec<f64>]) -> Option<Vec<Vec<f64>>> {
    let p = a.len();
    let mut m: Vec<Vec<f64>> = a.iter().cloned().collect();
    let mut inv: Vec<Vec<f64>> = (0..p).map(|i| (0..p).map(|j| if i == j { 1.0 } else { 0.0 }).collect()).collect();
    for col in 0..p {
        let pivot = (col..p).max_by(|&i, &j| m[i][col].abs().total_cmp(&m[j][col].abs()))?;
        if m[pivot][col] == 0.0 || !m[pivot][col].is_finite() {
            return None;
        }
        m.swap(col, pivot);
        inv.swap(col, pivot);
        let d = m[col][col];
        for j in 0..p {
            m[col][j] /= d;
            inv[col][j] /= d;
        }
        for i in 0..p {
            if i != col {
                let f = m[i][col];
                if f != 0.0 {
                    for j in 0..p {
                        m[i][j] -= f * m[col][j];
                        inv[i][j] -= f * inv[col][j];
                    }
                }
            }
        }
    }
    Some(inv)
}

/// OLS with an intercept: the bound on the overall F and on every
/// coefficient's t (intercept first), from the fitted coefficients `beta`.
/// `None` when `X'X` cannot be inverted.
pub fn ols_bounds(y: Stored<'_>, xs: &[Stored<'_>], beta: &[f64]) -> Option<(f64, Vec<f64>)> {
    let n = y.values.len();
    let p = xs.len() + 1;
    if xs.iter().all(|x| x.delta == 0.0) && y.delta == 0.0 {
        return Some((0.0, vec![0.0; p]));
    }
    let (gf, gt) = ols_gradients(y.values, &xs.iter().map(|x| x.values).collect::<Vec<_>>(), beta)?;
    let delta = |i: usize| if i < n { y.delta } else { xs[i / n - 1].delta };
    let b = |g: &[f64]| g.iter().enumerate().map(|(i, v)| v.abs() * delta(i)).sum::<f64>();
    Some((b(&gf), gt.iter().map(|g| b(g)).collect()))
}

/* ------------------------------------------------------------------------ *
 *  The range (evaluation fixes, part A, section 2)
 * ------------------------------------------------------------------------ */

/// The validity limit L: when the rounding step is large against the spread
/// of the data, `ratio` (see [`RangeParts::ratio`]) above L, the range is
/// unbounded and any miss is not checked. Measured 15 Oct 2026 by the sweep
/// in `tests/l1_precision_sweep.rs` (an `#[ignore]` test; 1,677 statistics
/// over 8 kinds): the needed widening first exceeds 2.0 at r = 0.2033 (an
/// OLS F), so L = 0.2031 halved, 0.1016, rounded down. D270 records it.
pub const VALIDITY_LIMIT: f64 = 0.10;

/// The widening factor F applied to the hull of the first-order range and
/// the two sign corners. Measured by the same sweep: the most any statistic
/// at or below L needed was 1.0439 (an OLS F, r = 0.101), rounded up.
pub const WIDENING: f64 = 1.1;

/// What a statistic's range is made from, before the limit and the widening.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RangeParts {
    /// The exact statistic on the stored cells.
    pub value: f64,
    /// The first-order half-width, `Σ |∂S/∂x_i| δ_i`.
    pub first_order: f64,
    /// The exact statistic with every cell moved by its δ against, and along,
    /// the sign of its derivative: the two sign corners. NaN when a corner
    /// cannot be computed.
    pub corner_lo: f64,
    pub corner_hi: f64,
    /// δ over the standard deviation of each bound group or column (for a
    /// paired t, also the differences, with δ the two columns' δ added; for
    /// OLS, the part of each column the rest of the model does not explain),
    /// the largest of them. Infinite when a standard deviation is zero.
    pub ratio: f64,
}

/// A statistic's range under the stored cells' rounding.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum StatRange {
    /// Nothing stored is rounded.
    Exact,
    /// `[lo, hi]`, signed.
    Range(f64, f64),
    /// The rounding step is too large against the data's spread for a range
    /// to be trusted: any miss is not checked.
    Unbounded,
}

impl RangeParts {
    /// The hull of `[value - first_order, value + first_order]` and the two
    /// corners.
    pub fn hull(&self) -> (f64, f64) {
        let lo = (self.value - self.first_order).min(self.corner_lo).min(self.corner_hi);
        let hi = (self.value + self.first_order).max(self.corner_lo).max(self.corner_hi);
        (lo, hi)
    }

    /// The range under `limit` and `widening`: unbounded above the limit or
    /// when anything is not finite; otherwise the hull, each side widened
    /// away from the value by the factor.
    pub fn range_with(&self, limit: f64, widening: f64) -> StatRange {
        let finite = self.value.is_finite() && self.first_order.is_finite() && self.corner_lo.is_finite() && self.corner_hi.is_finite();
        if !finite || !(self.ratio <= limit) {
            return StatRange::Unbounded;
        }
        let (lo, hi) = self.hull();
        StatRange::Range(self.value - widening * (self.value - lo), self.value + widening * (hi - self.value))
    }

    /// The range under the measured [`VALIDITY_LIMIT`] and [`WIDENING`].
    pub fn range(&self) -> StatRange {
        self.range_with(VALIDITY_LIMIT, WIDENING)
    }
}

/// The sample standard deviation; zero for fewer than two values.
fn sd(xs: &[f64]) -> f64 {
    if xs.len() < 2 {
        return 0.0;
    }
    let m = mean(xs);
    (ss(xs, m) / (xs.len() as f64 - 1.0)).sqrt()
}

/// The largest `δ / sd` over `(values, δ)` pairs with `δ > 0`; infinite when
/// such a column has no spread.
pub fn ratio_of(cols: &[(&[f64], f64)]) -> f64 {
    cols.iter()
        .filter(|(_, d)| *d > 0.0)
        .map(|(v, d)| {
            let s = sd(v);
            if s > 0.0 && s.is_finite() {
                d / s
            } else {
                f64::INFINITY
            }
        })
        .fold(0.0, f64::max)
}

/// The parts from flat `cells`, their `deltas` and the statistic's
/// `gradient` over them, with `stat` evaluating the exact statistic on a
/// moved copy of the cells.
fn parts(cells: &[f64], deltas: &[f64], gradient: &[f64], value: f64, ratio: f64, stat: impl Fn(&[f64]) -> Option<f64>) -> RangeParts {
    let first_order = gradient.iter().zip(deltas).map(|(g, d)| g.abs() * d).sum();
    let moved = |sign: f64| -> Vec<f64> {
        cells.iter().zip(deltas).zip(gradient).map(|((x, d), g)| x + sign * d * if *g > 0.0 { 1.0 } else if *g < 0.0 { -1.0 } else { 0.0 }).collect()
    };
    let up = stat(&moved(1.0)).unwrap_or(f64::NAN);
    let down = stat(&moved(-1.0)).unwrap_or(f64::NAN);
    RangeParts { value, first_order, corner_lo: up.min(down), corner_hi: up.max(down), ratio }
}

fn concat(a: &[f64], b: &[f64]) -> Vec<f64> {
    a.iter().chain(b).copied().collect()
}

/// Student's pooled t's gradient over `a` then `c`.
pub fn student_t_gradient(a: &[f64], c: &[f64]) -> Vec<f64> {
    let (n1, n2) = (a.len() as f64, c.len() as f64);
    let (m1, m2) = (mean(a), mean(c));
    let df = n1 + n2 - 2.0;
    let sp2 = (ss(a, m1) + ss(c, m2)) / df;
    let sp = sp2.sqrt();
    let k = (1.0 / n1 + 1.0 / n2).sqrt();
    let t = (m1 - m2) / (sp * k);
    a.iter()
        .map(|x| 1.0 / (n1 * sp * k) - t * (x - m1) / (df * sp2))
        .chain(c.iter().map(|x| -1.0 / (n2 * sp * k) - t * (x - m2) / (df * sp2)))
        .collect()
}

/// Welch's t's and df's gradients over `a` then `c`.
pub fn welch_gradients(a: &[f64], c: &[f64]) -> (Vec<f64>, Vec<f64>) {
    let (n1, n2) = (a.len() as f64, c.len() as f64);
    let (m1, m2) = (mean(a), mean(c));
    let v1 = ss(a, m1) / (n1 - 1.0) / n1;
    let v2 = ss(c, m2) / (n2 - 1.0) / n2;
    let v = v1 + v2;
    let t = (m1 - m2) / v.sqrt();
    let w = v1 * v1 / (n1 - 1.0) + v2 * v2 / (n2 - 1.0);
    let (mut gt, mut gdf) = (Vec::new(), Vec::new());
    for (xs, n, m, vg, sign) in [(a, n1, m1, v1, 1.0), (c, n2, m2, v2, -1.0)] {
        for x in xs {
            let dv = 2.0 * (x - m) / ((n - 1.0) * n);
            gt.push(sign / n / v.sqrt() - t * dv / (2.0 * v));
            let dw = 2.0 * vg * dv / (n - 1.0);
            gdf.push((2.0 * v * dv * w - v * v * dw) / (w * w));
        }
    }
    (gt, gdf)
}

/// The paired t's gradient over the differences `first - second`.
fn paired_t_gradient(d: &[f64]) -> Vec<f64> {
    let n = d.len() as f64;
    let md = mean(d);
    let sd2 = ss(d, md) / (n - 1.0);
    let s = sd2.sqrt();
    let t = md * n.sqrt() / s;
    d.iter().map(|di| n.sqrt() / (n * s) - t * (di - md) / ((n - 1.0) * sd2)).collect()
}

/// The one-way F's gradient over the groups' cells, in group order.
pub fn one_way_f_gradient(groups: &[&[f64]]) -> Vec<f64> {
    let n: f64 = groups.iter().map(|g| g.len() as f64).sum();
    let k = groups.len() as f64;
    let grand = groups.iter().flat_map(|g| g.iter()).sum::<f64>() / n;
    let means: Vec<f64> = groups.iter().map(|g| mean(g)).collect();
    let ssb: f64 = groups.iter().zip(&means).map(|(g, m)| g.len() as f64 * (m - grand).powi(2)).sum();
    let ssw: f64 = groups.iter().zip(&means).map(|(g, m)| ss(g, *m)).sum();
    let ratio = (n - k) / (k - 1.0);
    let mut out = Vec::new();
    for (g, m) in groups.iter().zip(&means) {
        for x in g.iter() {
            out.push(ratio * (2.0 * (m - grand) * ssw - ssb * 2.0 * (x - m)) / (ssw * ssw));
        }
    }
    out
}

/// Pearson's r's gradient over `x` then `y`.
pub fn pearson_gradient(x: &[f64], y: &[f64]) -> Vec<f64> {
    let (mx, my) = (mean(x), mean(y));
    let sxx = ss(x, mx);
    let syy = ss(y, my);
    let sxy: f64 = x.iter().zip(y).map(|(a, b)| (a - mx) * (b - my)).sum();
    let root = (sxx * syy).sqrt();
    let r = sxy / root;
    let gx = x.iter().zip(y).map(|(a, c)| (c - my) / root - r * (a - mx) / sxx);
    let gy = x.iter().zip(y).map(|(a, c)| (a - mx) / root - r * (c - my) / syy);
    gx.chain(gy).collect()
}

/// Student's and Welch's t on two groups of one column, and Welch's df.
pub struct IndependentParts {
    pub student: RangeParts,
    pub welch: RangeParts,
    pub welch_df: RangeParts,
}

pub fn independent_t_parts(a: &[f64], c: &[f64], delta: f64) -> IndependentParts {
    let cells = concat(a, c);
    let deltas = vec![delta; cells.len()];
    let n1 = a.len();
    let ratio = ratio_of(&[(a, delta), (c, delta)]);
    let split = |v: &[f64]| (v[..n1].to_vec(), v[n1..].to_vec());
    let student_value = crate::stats_verify::t_test_student(a, c).map(|r| r.t).unwrap_or(f64::NAN);
    let welch = crate::stats_verify::t_test_welch(a, c).ok();
    let student = parts(&cells, &deltas, &student_t_gradient(a, c), student_value, ratio, |v| {
        let (x, y) = split(v);
        crate::stats_verify::t_test_student(&x, &y).ok().map(|r| r.t)
    });
    let (gt, gdf) = welch_gradients(a, c);
    let welch_t = parts(&cells, &deltas, &gt, welch.as_ref().map_or(f64::NAN, |r| r.t), ratio, |v| {
        let (x, y) = split(v);
        crate::stats_verify::t_test_welch(&x, &y).ok().map(|r| r.t)
    });
    let welch_df = parts(&cells, &deltas, &gdf, welch.as_ref().map_or(f64::NAN, |r| r.df), ratio, |v| {
        let (x, y) = split(v);
        crate::stats_verify::t_test_welch(&x, &y).ok().map(|r| r.df)
    });
    IndependentParts { student, welch: welch_t, welch_df }
}

/// The paired t on two columns, each with its own δ.
pub fn paired_t_parts(first: Stored<'_>, second: Stored<'_>) -> RangeParts {
    let n = first.values.len();
    let d: Vec<f64> = first.values.iter().zip(second.values).map(|(x, y)| x - y).collect();
    let gd = paired_t_gradient(&d);
    let gradient: Vec<f64> = gd.iter().copied().chain(gd.iter().map(|g| -g)).collect();
    let cells = concat(first.values, second.values);
    let deltas: Vec<f64> = std::iter::repeat(first.delta).take(n).chain(std::iter::repeat(second.delta).take(n)).collect();
    let ratio = ratio_of(&[(first.values, first.delta), (second.values, second.delta), (&d, first.delta + second.delta)]);
    let value = crate::stats_verify::t_test_paired(first.values, second.values).map(|r| r.t).unwrap_or(f64::NAN);
    parts(&cells, &deltas, &gradient, value, ratio, |v| crate::stats_verify::t_test_paired(&v[..n], &v[n..]).ok().map(|r| r.t))
}

/// The one-way ANOVA's F on groups of one column.
pub fn one_way_f_parts(groups: &[&[f64]], delta: f64) -> RangeParts {
    let cells: Vec<f64> = groups.iter().flat_map(|g| g.iter().copied()).collect();
    let deltas = vec![delta; cells.len()];
    let sizes: Vec<usize> = groups.iter().map(|g| g.len()).collect();
    let ratio = ratio_of(&groups.iter().map(|g| (*g, delta)).collect::<Vec<_>>());
    let value = crate::stats_verify::one_way_anova(groups).map(|r| r.f).unwrap_or(f64::NAN);
    parts(&cells, &deltas, &one_way_f_gradient(groups), value, ratio, |v| {
        let mut at = 0;
        let gs: Vec<&[f64]> = sizes
            .iter()
            .map(|&k| {
                let g = &v[at..at + k];
                at += k;
                g
            })
            .collect();
        crate::stats_verify::one_way_anova(&gs).ok().map(|r| r.f)
    })
}

/// Pearson's r on two columns.
pub fn pearson_parts(x: Stored<'_>, y: Stored<'_>) -> RangeParts {
    let n = x.values.len();
    let cells = concat(x.values, y.values);
    let deltas: Vec<f64> = std::iter::repeat(x.delta).take(n).chain(std::iter::repeat(y.delta).take(n)).collect();
    let ratio = ratio_of(&[(x.values, x.delta), (y.values, y.delta)]);
    let value = crate::stats_verify::pearson(x.values, y.values).map(|r| r.coefficient).unwrap_or(f64::NAN);
    parts(&cells, &deltas, &pearson_gradient(x.values, y.values), value, ratio, |v| {
        crate::stats_verify::pearson(&v[..n], &v[n..]).ok().map(|r| r.coefficient)
    })
}

/// OLS with an intercept: the overall F's and every coefficient's t's
/// (intercept first) parts, from the fitted coefficients `beta`. `None` when
/// `X'X` cannot be inverted.
pub fn ols_parts(y: Stored<'_>, xs: &[Stored<'_>], beta: &[f64]) -> Option<(RangeParts, Vec<RangeParts>)> {
    let n = y.values.len();
    let (gf, gt) = ols_gradients(y.values, &xs.iter().map(|x| x.values).collect::<Vec<_>>(), beta)?;
    let cells: Vec<f64> = y.values.iter().copied().chain(xs.iter().flat_map(|x| x.values.iter().copied())).collect();
    let deltas: Vec<f64> = std::iter::repeat(y.delta).take(n).chain(xs.iter().flat_map(|x| std::iter::repeat(x.delta).take(n))).collect();
    let ratio = ols_ratio(y, xs, beta)?;
    let fit = |v: &[f64]| {
        let preds: Vec<Vec<f64>> = (0..xs.len()).map(|l| v[n * (l + 1)..n * (l + 2)].to_vec()).collect();
        crate::stats_verify::ols_qr(&v[..n], &preds).ok()
    };
    let here = fit(&cells)?;
    let f = parts(&cells, &deltas, &gf, here.f_statistic, ratio, |v| fit(v).map(|r| r.f_statistic));
    let ts = gt
        .iter()
        .enumerate()
        .map(|(j, g)| parts(&cells, &deltas, g, here.coefficients[j].t, ratio, |v| fit(v).map(|r| r.coefficients[j].t)))
        .collect();
    Some((f, ts))
}

/// The OLS reading of the ratio: each column's spread is the part of it the
/// rest of the model does not explain, because that is what its rounding
/// has to beat. For a predictor, the standard deviation of its residual on
/// the intercept and the other predictors, `sqrt(1 / C_jj / (n - 1))` with
/// `C = (X'X)^-1`; for the outcome, the residual standard deviation
/// `sqrt(RSS / (n - p))`. Measured: with each column's own standard
/// deviation, near-collinear predictors needed widenings in the thousands at
/// r = 0.03 (`tests/l1_precision_sweep.rs`).
fn ols_ratio(y: Stored<'_>, xs: &[Stored<'_>], beta: &[f64]) -> Option<f64> {
    let n = y.values.len();
    let p = xs.len() + 1;
    let row = |i: usize| -> Vec<f64> { std::iter::once(1.0).chain(xs.iter().map(|x| x.values[i])).collect() };
    let mut xtx = vec![vec![0.0; p]; p];
    for i in 0..n {
        let r = row(i);
        for a in 0..p {
            for b in 0..p {
                xtx[a][b] += r[a] * r[b];
            }
        }
    }
    let c = invert(&xtx)?;
    let rss: f64 = (0..n).map(|i| (y.values[i] - row(i).iter().zip(beta).map(|(a, b)| a * b).sum::<f64>()).powi(2)).sum();
    let spread = |delta: f64, sd: f64| if delta == 0.0 { 0.0 } else if sd > 0.0 && sd.is_finite() { delta / sd } else { f64::INFINITY };
    let mut ratio = spread(y.delta, (rss / (n - p) as f64).sqrt());
    for (j, x) in xs.iter().enumerate() {
        ratio = ratio.max(spread(x.delta, (1.0 / c[j + 1][j + 1] / (n as f64 - 1.0)).sqrt()));
    }
    Some(ratio)
}

/// OLS gradients over `y` then each predictor's column: the overall F's, and
/// each coefficient's t's (intercept first).
fn ols_gradients(y: &[f64], xs: &[&[f64]], beta: &[f64]) -> Option<(Vec<f64>, Vec<Vec<f64>>)> {
    let n = y.len();
    let k = xs.len();
    let p = k + 1;
    let row = |i: usize| -> Vec<f64> {
        let mut r = Vec::with_capacity(p);
        r.push(1.0);
        r.extend(xs.iter().map(|x| x[i]));
        r
    };
    let mut xtx = vec![vec![0.0; p]; p];
    for i in 0..n {
        let r = row(i);
        for a in 0..p {
            for b in 0..p {
                xtx[a][b] += r[a] * r[b];
            }
        }
    }
    let c = invert(&xtx)?;
    let resid: Vec<f64> = (0..n).map(|i| y[i] - row(i).iter().zip(beta).map(|(a, b)| a * b).sum::<f64>()).collect();
    let rss: f64 = resid.iter().map(|e| e * e).sum();
    let my = mean(y);
    let tss = ss(y, my);
    let dfr = (n - p) as f64;
    let sigma2 = rss / dfr;
    let f_ratio = dfr / k as f64;
    let se: Vec<f64> = (0..p).map(|j| (sigma2 * c[j][j]).sqrt()).collect();
    let t: Vec<f64> = (0..p).map(|j| beta[j] / se[j]).collect();
    let mut gf = vec![0.0; n * p];
    let mut gt = vec![vec![0.0; n * p]; p];
    for i in 0..n {
        let r = row(i);
        let cx: Vec<f64> = (0..p).map(|a| (0..p).map(|b| c[a][b] * r[b]).sum()).collect();
        let e = resid[i];
        // y_i
        gf[i] = f_ratio * (2.0 * (y[i] - my) * rss - tss * 2.0 * e) / (rss * rss);
        let dsig = 2.0 * e / dfr;
        for j in 0..p {
            gt[j][i] = cx[j] / se[j] - 0.5 * t[j] * (dsig / sigma2);
        }
        // x_{i,l}, l = 1..=k (column 0 is the intercept, not a cell)
        for l in 1..p {
            let at = n * l + i;
            let drss = -2.0 * e * beta[l];
            gf[at] = f_ratio * (-tss * drss) / (rss * rss);
            let dsig = drss / dfr;
            for j in 0..p {
                let dbeta = c[j][l] * e - cx[j] * beta[l];
                let dcjj = -2.0 * c[j][l] * cx[j];
                gt[j][at] = dbeta / se[j] - 0.5 * t[j] * (dsig / sigma2 + dcjj / c[j][j]);
            }
        }
    }
    Some((gf, gt))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decimals_are_read_from_the_shortest_round_trip_form() {
        assert_eq!(decimals(12.0), 0);
        assert_eq!(decimals(12.1), 1);
        assert_eq!(decimals(0.55), 2);
        assert_eq!(decimals(1.0 / 3.0), MAX_DECIMALS);
        assert_eq!(column_decimals([1.0, 2.5, 3.25]), 2);
        assert_eq!(column_decimals([1.0, 2.0, 30.0]), 0);
        assert_eq!(half_unit(0), 0.5);
        assert!((half_unit(2) - 0.005).abs() < 1e-18);
    }

    /// Each analytic bound against a brute-force central difference.
    #[test]
    fn analytic_derivatives_agree_with_central_differences() {
        let a = [12.1, 14.3, 11.8, 13.6, 12.9];
        let c = [15.2, 16.9, 17.4, 16.0, 14.8, 15.5];
        let h = 1e-6;
        let student = |a: &[f64], c: &[f64]| {
            let (n1, n2) = (a.len() as f64, c.len() as f64);
            let (m1, m2) = (mean(a), mean(c));
            let sp = ((ss(a, m1) + ss(c, m2)) / (n1 + n2 - 2.0)).sqrt();
            (m1 - m2) / (sp * (1.0 / n1 + 1.0 / n2).sqrt())
        };
        let mut brute = 0.0;
        for which in 0..2 {
            let len = if which == 0 { a.len() } else { c.len() };
            for i in 0..len {
                let (mut ap, mut am, mut cp, mut cm) = (a.to_vec(), a.to_vec(), c.to_vec(), c.to_vec());
                if which == 0 {
                    ap[i] += h;
                    am[i] -= h;
                } else {
                    cp[i] += h;
                    cm[i] -= h;
                }
                brute += ((student(&ap, &cp) - student(&am, &cm)) / (2.0 * h)).abs();
            }
        }
        let analytic = student_t_bound(&a, &c, 1.0);
        assert!((analytic - brute).abs() < 1e-5 * brute, "{analytic} vs {brute}");

        let anova = |gs: &[Vec<f64>]| {
            let n: f64 = gs.iter().map(|g| g.len() as f64).sum();
            let grand = gs.iter().flatten().sum::<f64>() / n;
            let ssb: f64 = gs.iter().map(|g| g.len() as f64 * (mean(g) - grand).powi(2)).sum();
            let ssw: f64 = gs.iter().map(|g| ss(g, mean(g))).sum();
            (ssb / (gs.len() as f64 - 1.0)) / (ssw / (n - gs.len() as f64))
        };
        let gs = vec![a.to_vec(), c.to_vec(), vec![9.9, 10.4, 8.7, 11.2]];
        let mut brute = 0.0;
        for gi in 0..gs.len() {
            for i in 0..gs[gi].len() {
                let (mut p, mut m) = (gs.clone(), gs.clone());
                p[gi][i] += h;
                m[gi][i] -= h;
                brute += ((anova(&p) - anova(&m)) / (2.0 * h)).abs();
            }
        }
        let refs: Vec<&[f64]> = gs.iter().map(Vec::as_slice).collect();
        let analytic = one_way_f_bound(&refs, 1.0);
        assert!((analytic - brute).abs() < 1e-5 * brute, "{analytic} vs {brute}");
    }

    /// Part C, B-1: a whole-number column is rounded to whole numbers (δ =
    /// 0.5) unless the user says the numbers are exact (δ = 0, passed by the
    /// caller), where until part C it was always exact.
    #[test]
    fn whole_number_columns_are_rounded_unless_said_to_be_exact() {
        let a = [3.0, 4.0, 5.0, 6.0];
        let c = [5.0, 7.0, 6.0, 8.0];
        let d = column_decimals(a.iter().chain(&c).copied());
        assert_eq!(d, 0);
        assert_eq!(half_unit(d), 0.5);
        assert!(student_t_bound(&a, &c, half_unit(d)) > 0.0);
        assert_eq!(student_t_bound(&a, &c, 0.0), 0.0);
        assert_eq!(welch_bounds(&a, &c, 0.0), (0.0, 0.0));
    }

    #[test]
    fn spearman_reordering_is_judged_by_gaps_against_two_deltas() {
        assert!(!could_reorder(Stored { values: &[1.0, 1.2, 1.5], delta: 0.05 }));
        assert!(could_reorder(Stored { values: &[1.0, 1.08, 1.5], delta: 0.05 }));
        assert!(could_reorder(Stored { values: &[1.0, 1.0, 1.5], delta: 0.05 }));
        assert!(!could_reorder(Stored { values: &[1.0, 1.0, 1.5], delta: 0.0 }));
    }
}
