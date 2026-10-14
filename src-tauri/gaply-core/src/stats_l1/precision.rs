//! **How far a deposited file's own rounding can move a computed statistic**
//! (Phase 2, Step 8b, rule 1).
//!
//! A data file is often stored rounded: a proportion written `.55`, a score
//! written `12.1`. Level 1 computes exactly on what is stored, so its exact
//! value carries that rounding. This module measures it.
//!
//! - **Stored precision.** Per bound numeric column, `d` is the most decimals
//!   any of its cells shows in its shortest round-trip decimal form (Rust's
//!   `Display` for `f64`), capped at [`MAX_DECIMALS`]. A column whose every
//!   cell is a whole number is exact; otherwise each of its cells is
//!   uncertain by `δ = 0.5 × 10^-d`.
//! - **The range.** The first-order worst case `b = Σ |∂S/∂x_i| δ_i` over
//!   the bound cells, giving `[S − b, S + b]`. The derivatives are analytic,
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

/// The uncertainty of one cell stored to `d` decimals: zero for a column of
/// whole numbers.
pub fn half_unit(d: u32) -> f64 {
    if d == 0 {
        0.0
    } else {
        0.5 * 10f64.powi(-(d as i32))
    }
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
    let (n1, n2) = (a.len() as f64, c.len() as f64);
    let (m1, m2) = (mean(a), mean(c));
    let df = n1 + n2 - 2.0;
    let sp2 = (ss(a, m1) + ss(c, m2)) / df;
    let sp = sp2.sqrt();
    let k = (1.0 / n1 + 1.0 / n2).sqrt();
    let t = (m1 - m2) / (sp * k);
    let g1: f64 = a.iter().map(|x| (1.0 / (n1 * sp * k) - t * (x - m1) / (df * sp2)).abs()).sum();
    let g2: f64 = c.iter().map(|x| (-1.0 / (n2 * sp * k) - t * (x - m2) / (df * sp2)).abs()).sum();
    (g1 + g2) * delta
}

/// Welch's t and its df on two groups of one column: `(b_t, b_df)`.
pub fn welch_bounds(a: &[f64], c: &[f64], delta: f64) -> (f64, f64) {
    if delta == 0.0 {
        return (0.0, 0.0);
    }
    let (n1, n2) = (a.len() as f64, c.len() as f64);
    let (m1, m2) = (mean(a), mean(c));
    let v1 = ss(a, m1) / (n1 - 1.0) / n1;
    let v2 = ss(c, m2) / (n2 - 1.0) / n2;
    let v = v1 + v2;
    let t = (m1 - m2) / v.sqrt();
    let w = v1 * v1 / (n1 - 1.0) + v2 * v2 / (n2 - 1.0);
    let (mut bt, mut bdf) = (0.0, 0.0);
    for (xs, n, m, vg, sign) in [(a, n1, m1, v1, 1.0), (c, n2, m2, v2, -1.0)] {
        for x in xs {
            let dv = 2.0 * (x - m) / ((n - 1.0) * n);
            let dt = sign / n / v.sqrt() - t * dv / (2.0 * v);
            let dw = 2.0 * vg * dv / (n - 1.0);
            let ddf = (2.0 * v * dv * w - v * v * dw) / (w * w);
            bt += dt.abs();
            bdf += ddf.abs();
        }
    }
    (bt * delta, bdf * delta)
}

/// The paired t on two columns, each with its own uncertainty.
pub fn paired_t_bound(first: Stored<'_>, second: Stored<'_>) -> f64 {
    if first.delta == 0.0 && second.delta == 0.0 {
        return 0.0;
    }
    let d: Vec<f64> = first.values.iter().zip(second.values).map(|(x, y)| x - y).collect();
    let n = d.len() as f64;
    let md = mean(&d);
    let sd2 = ss(&d, md) / (n - 1.0);
    let sd = sd2.sqrt();
    let t = md * n.sqrt() / sd;
    let g: f64 = d.iter().map(|di| (n.sqrt() / (n * sd) - t * (di - md) / ((n - 1.0) * sd2)).abs()).sum();
    g * (first.delta + second.delta)
}

/// The one-way ANOVA's F on groups of one column.
pub fn one_way_f_bound(groups: &[&[f64]], delta: f64) -> f64 {
    if delta == 0.0 {
        return 0.0;
    }
    let n: f64 = groups.iter().map(|g| g.len() as f64).sum();
    let k = groups.len() as f64;
    let grand = groups.iter().flat_map(|g| g.iter()).sum::<f64>() / n;
    let means: Vec<f64> = groups.iter().map(|g| mean(g)).collect();
    let ssb: f64 = groups.iter().zip(&means).map(|(g, m)| g.len() as f64 * (m - grand).powi(2)).sum();
    let ssw: f64 = groups.iter().zip(&means).map(|(g, m)| ss(g, *m)).sum();
    let ratio = (n - k) / (k - 1.0);
    let mut b = 0.0;
    for (g, m) in groups.iter().zip(&means) {
        for x in g.iter() {
            let dssb = 2.0 * (m - grand);
            let dssw = 2.0 * (x - m);
            b += (ratio * (dssb * ssw - ssb * dssw) / (ssw * ssw)).abs();
        }
    }
    b * delta
}

/// Pearson's r on two columns.
pub fn pearson_bound(x: Stored<'_>, y: Stored<'_>) -> f64 {
    if x.delta == 0.0 && y.delta == 0.0 {
        return 0.0;
    }
    let (mx, my) = (mean(x.values), mean(y.values));
    let sxx = ss(x.values, mx);
    let syy = ss(y.values, my);
    let sxy: f64 = x.values.iter().zip(y.values).map(|(a, b)| (a - mx) * (b - my)).sum();
    let root = (sxx * syy).sqrt();
    let r = sxy / root;
    let mut b = 0.0;
    for (a, c) in x.values.iter().zip(y.values) {
        let dx = (c - my) / root - r * (a - mx) / sxx;
        let dy = (a - mx) / root - r * (c - my) / syy;
        b += dx.abs() * x.delta + dy.abs() * y.delta;
    }
    b
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
    let k = xs.len();
    let p = k + 1;
    if xs.iter().all(|x| x.delta == 0.0) && y.delta == 0.0 {
        return Some((0.0, vec![0.0; p]));
    }
    let row = |i: usize| -> Vec<f64> {
        let mut r = Vec::with_capacity(p);
        r.push(1.0);
        r.extend(xs.iter().map(|x| x.values[i]));
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
    let resid: Vec<f64> = (0..n).map(|i| y.values[i] - row(i).iter().zip(beta).map(|(a, b)| a * b).sum::<f64>()).collect();
    let rss: f64 = resid.iter().map(|e| e * e).sum();
    let my = mean(y.values);
    let tss = ss(y.values, my);
    let dfr = (n - p) as f64;
    let sigma2 = rss / dfr;
    let f_ratio = dfr / k as f64;
    let se: Vec<f64> = (0..p).map(|j| (sigma2 * c[j][j]).sqrt()).collect();
    let t: Vec<f64> = (0..p).map(|j| beta[j] / se[j]).collect();

    let (mut bf, mut bt) = (0.0, vec![0.0; p]);
    for i in 0..n {
        let r = row(i);
        let cx: Vec<f64> = (0..p).map(|a| (0..p).map(|b| c[a][b] * r[b]).sum()).collect();
        let e = resid[i];
        // y_i
        if y.delta > 0.0 {
            let df = f_ratio * (2.0 * (y.values[i] - my) * rss - tss * 2.0 * e) / (rss * rss);
            bf += df.abs() * y.delta;
            let dsig = 2.0 * e / dfr;
            for j in 0..p {
                let dt = cx[j] / se[j] - 0.5 * t[j] * (dsig / sigma2);
                bt[j] += dt.abs() * y.delta;
            }
        }
        // x_{i,l}, l = 1..=k (column 0 is the intercept, not a cell)
        for l in 1..p {
            let delta = xs[l - 1].delta;
            if delta == 0.0 {
                continue;
            }
            let drss = -2.0 * e * beta[l];
            let df = f_ratio * (-tss * drss) / (rss * rss);
            bf += df.abs() * delta;
            let dsig = drss / dfr;
            for j in 0..p {
                let dbeta = c[j][l] * e - cx[j] * beta[l];
                let dcjj = -2.0 * c[j][l] * cx[j];
                let dt = dbeta / se[j] - 0.5 * t[j] * (dsig / sigma2 + dcjj / c[j][j]);
                bt[j] += dt.abs() * delta;
            }
        }
    }
    Some((bf, bt))
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
        assert_eq!(half_unit(0), 0.0);
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

    #[test]
    fn whole_number_columns_are_exact() {
        let a = [3.0, 4.0, 5.0, 6.0];
        let c = [5.0, 7.0, 6.0, 8.0];
        let d = column_decimals(a.iter().chain(&c).copied());
        assert_eq!(d, 0);
        assert_eq!(student_t_bound(&a, &c, half_unit(d)), 0.0);
        assert_eq!(welch_bounds(&a, &c, half_unit(d)), (0.0, 0.0));
    }

    #[test]
    fn spearman_reordering_is_judged_by_gaps_against_two_deltas() {
        assert!(!could_reorder(Stored { values: &[1.0, 1.2, 1.5], delta: 0.05 }));
        assert!(could_reorder(Stored { values: &[1.0, 1.08, 1.5], delta: 0.05 }));
        assert!(could_reorder(Stored { values: &[1.0, 1.0, 1.5], delta: 0.05 }));
        assert!(!could_reorder(Stored { values: &[1.0, 1.0, 1.5], delta: 0.0 }));
    }
}
