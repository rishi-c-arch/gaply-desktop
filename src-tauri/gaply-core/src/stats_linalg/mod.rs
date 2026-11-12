//! **`gaply_core::stats_linalg`: the dense linear algebra the exploratory factor
//! analysis needs (Research OS, Phase 9B-1, decision 699).** A symmetric
//! eigensolver, a Cholesky factor with its inverse and log-determinant, and
//! least squares with several right-hand sides and no intercept. (9B-1's polar
//! factor went in 9B-2, decision 719: SPSS's varimax rotates pairs of factors
//! and needs no singular value decomposition.)
//!
//! Matrices are `Vec<Vec<f64>>`, row by row; an eigenvector or a column is a
//! `Vec<f64>`. Every function is `pub(crate)`: this is arithmetic for the
//! statistics modules, not an interface. It names nothing else in the crate and
//! holds no type of its own.
//!
//! **The same bits on every platform.** Every operation is +, −, ×, ÷ or a
//! square root, each correctly rounded by IEEE 754; a power is written as a
//! product (`x * x`), never `powi`, whose precision Rust documents as
//! unspecified. The one platform function is `ln`, in [`log_det`], as
//! `stats_xtab` already uses it. No fused multiply-add is written, and Rust
//! never contracts `a * b + c` into one by itself.
//!
//! R's `eigen`, `chol`, `chol2inv`, `determinant` and `qr.solve` are the
//! oracle (`tests/fixtures/stats_linalg/make_linalg_fixtures.R`, compared in
//! `stats_linalg_r_golden.rs`); each algorithm here is written from its
//! published description, not from any tool's source (ADR-008).

/// The sweeps [`eigen_symmetric`] makes before it refuses (decision 699).
pub(crate) const JACOBI_MAX_SWEEPS: usize = 100;

/// [`cholesky`] refuses at row j when the value left there before its square
/// root is at most this times a_jj (decision 699): that item's squared
/// multiple correlation with the items before it is then at least 1 − 1e-9.
pub(crate) const CHOLESKY_TOLERANCE: f64 = 1e-9;

/// The sign rule's tolerance (decision 699): a sum within this times the
/// 1-norm counts as 0, and an entry must be above it in size to set the sign.
pub(crate) const SIGN_TOLERANCE: f64 = 1e-10;


/// [`least_squares`] refuses a column whose norm left after the columns
/// before it is at most this times its own norm: LINPACK's dqrdc2 test at R's
/// `qr` default, under which R's `qr.solve` stops with "singular matrix".
pub(crate) const LEAST_SQUARES_RANK_TOLERANCE: f64 = 1e-7;

/// The identity of order `n`.
fn identity(n: usize) -> Vec<Vec<f64>> {
    (0..n).map(|i| (0..n).map(|j| if i == j { 1.0 } else { 0.0 }).collect()).collect()
}

/// Whether the vector `v` is to be reversed under decision 699's sign rule:
/// when its sum s has |s| above [`SIGN_TOLERANCE`] times its 1-norm, when s
/// is below 0; otherwise (the sum is 0 but for rounding, which R and Rust need
/// not round alike) when its first entry above [`SIGN_TOLERANCE`] in size is
/// below 0. A vector with no such entry is kept.
pub(crate) fn sign_reversed(v: &[f64]) -> bool {
    let sum: f64 = v.iter().sum();
    let norm: f64 = v.iter().map(|x| x.abs()).sum();
    if sum.abs() > SIGN_TOLERANCE * norm {
        sum < 0.0
    } else {
        v.iter().find(|x| x.abs() > SIGN_TOLERANCE).is_some_and(|x| *x < 0.0)
    }
}

/// Every eigenvalue of the symmetric matrix `a`, in descending order (ties
/// kept in index order), each with its unit eigenvector, by the cyclic Jacobi
/// method with Rutishauser's update (decision 699). Each eigenvector's sign is
/// set by [`sign_reversed`]. `None` when an entry is not finite, or when the
/// off-diagonal sum of squares is still above (ε ‖A‖_F)² after
/// [`JACOBI_MAX_SWEEPS`] sweeps.
pub(crate) fn eigen_symmetric(a: &[Vec<f64>]) -> Option<(Vec<f64>, Vec<Vec<f64>>)> {
    eigen_within(a, JACOBI_MAX_SWEEPS)
}

/// [`eigen_symmetric`] with its sweep limit given, so a unit test can reach
/// the refusal.
fn eigen_within(a: &[Vec<f64>], max_sweeps: usize) -> Option<(Vec<f64>, Vec<Vec<f64>>)> {
    let n = a.len();
    if a.iter().any(|r| r.len() != n || r.iter().any(|x| !x.is_finite())) {
        return None;
    }
    let mut m: Vec<Vec<f64>> = a.to_vec();
    let mut v = identity(n);
    let norm: f64 = a.iter().flatten().map(|x| x * x).sum::<f64>().sqrt();
    let limit = (f64::EPSILON * norm) * (f64::EPSILON * norm);
    let off = |m: &[Vec<f64>]| {
        let mut s = 0.0;
        for (p, row) in m.iter().enumerate() {
            for (q, x) in row.iter().enumerate() {
                if p != q {
                    s += x * x;
                }
            }
        }
        s
    };
    let mut sweeps = 0;
    while !(off(&m) <= limit) {
        if sweeps == max_sweeps {
            return None;
        }
        for p in 0..n {
            for q in p + 1..n {
                rotate(&mut m, &mut v, p, q);
            }
        }
        sweeps += 1;
    }
    let values: Vec<f64> = (0..n).map(|i| m[i][i]).collect();
    let mut order: Vec<usize> = (0..n).collect();
    // A stable sort: equal eigenvalues keep their index order.
    order.sort_by(|&i, &j| values[j].partial_cmp(&values[i]).expect("finite"));
    let vectors = order
        .iter()
        .map(|&j| {
            let mut col: Vec<f64> = (0..n).map(|r| v[r][j]).collect();
            if sign_reversed(&col) {
                col.iter_mut().for_each(|x| *x = -*x);
            }
            col
        })
        .collect();
    Some((order.iter().map(|&j| values[j]).collect(), vectors))
}

/// One Jacobi rotation zeroing `m[p][q]`, applied to `m` and to the columns
/// p and q of `v`. A pair whose entry is exactly 0 is skipped: with a_pp =
/// a_qq it would give θ = 0/0.
fn rotate(m: &mut [Vec<f64>], v: &mut [Vec<f64>], p: usize, q: usize) {
    let apq = m[p][q];
    if apq == 0.0 {
        return;
    }
    let theta = (m[q][q] - m[p][p]) / (2.0 * apq);
    let sign = if theta >= 0.0 { 1.0 } else { -1.0 };
    let t = sign / (theta.abs() + (theta * theta + 1.0).sqrt());
    let c = 1.0 / (t * t + 1.0).sqrt();
    let s = t * c;
    let tau = s / (1.0 + c);
    m[p][p] -= t * apq;
    m[q][q] += t * apq;
    m[p][q] = 0.0;
    m[q][p] = 0.0;
    for r in 0..m.len() {
        if r != p && r != q {
            let (g, h) = (m[r][p], m[r][q]);
            let rp = g - s * (h + g * tau);
            let rq = h + s * (g - h * tau);
            m[r][p] = rp;
            m[p][r] = rp;
            m[r][q] = rq;
            m[q][r] = rq;
        }
    }
    for row in v.iter_mut() {
        let (g, h) = (row[p], row[q]);
        row[p] = g - s * (h + g * tau);
        row[q] = h + s * (g - h * tau);
    }
}

/// The lower Cholesky factor L of the symmetric positive-definite `a`, A =
/// L Lᵀ. `Err(j)`, j 1-based, at the first row whose value left before its
/// square root is at most [`CHOLESKY_TOLERANCE`] times a_jj, or is not a
/// number (decision 699).
pub(crate) fn cholesky(a: &[Vec<f64>]) -> Result<Vec<Vec<f64>>, usize> {
    let n = a.len();
    let mut l = vec![vec![0.0; n]; n];
    for j in 0..n {
        let mut left = a[j][j];
        for k in 0..j {
            left -= l[j][k] * l[j][k];
        }
        if !(left > CHOLESKY_TOLERANCE * a[j][j]) {
            return Err(j + 1);
        }
        let d = left.sqrt();
        l[j][j] = d;
        for i in j + 1..n {
            let mut s = a[i][j];
            for k in 0..j {
                s -= l[i][k] * l[j][k];
            }
            l[i][j] = s / d;
        }
    }
    Ok(l)
}

/// A⁻¹ from A's Cholesky factor `l`: L⁻¹ by forward substitution, then
/// A⁻¹ = L⁻ᵀ L⁻¹, as R's `chol2inv` gives it; exactly symmetric.
pub(crate) fn spd_inverse(l: &[Vec<f64>]) -> Vec<Vec<f64>> {
    let n = l.len();
    let mut w = vec![vec![0.0; n]; n];
    for j in 0..n {
        w[j][j] = 1.0 / l[j][j];
        for i in j + 1..n {
            let mut s = 0.0;
            for k in j..i {
                s += l[i][k] * w[k][j];
            }
            w[i][j] = -s / l[i][i];
        }
    }
    let mut inv = vec![vec![0.0; n]; n];
    for i in 0..n {
        for j in 0..=i {
            let mut s = 0.0;
            for row in w.iter().skip(i) {
                s += row[i] * row[j];
            }
            inv[i][j] = s;
            inv[j][i] = s;
        }
    }
    inv
}

/// ln |A| from A's Cholesky factor `l`: 2 Σ ln l_jj.
pub(crate) fn log_det(l: &[Vec<f64>]) -> f64 {
    2.0 * l.iter().enumerate().map(|(j, row)| row[j].ln()).sum::<f64>()
}

/// The least-squares solution B of X B = Y, X being n × m with n ≥ m and Y
/// n × r, no intercept, by Householder QR on X with the same reflections
/// applied to Y, then back substitution. `None` when n < m, an entry is not
/// finite, or a column's norm left after the columns before it is at most
/// [`LEAST_SQUARES_RANK_TOLERANCE`] times its own (R's `qr.solve` refuses
/// such an X as singular).
pub(crate) fn least_squares(x: &[Vec<f64>], y: &[Vec<f64>]) -> Option<Vec<Vec<f64>>> {
    let n = x.len();
    let m = x.first().map_or(0, |r| r.len());
    let r = y.first().map_or(0, |r| r.len());
    if n < m || m == 0 || y.len() != n {
        return None;
    }
    if x.iter().chain(y).any(|row| row.iter().any(|v| !v.is_finite())) || x.iter().any(|row| row.len() != m) || y.iter().any(|row| row.len() != r) {
        return None;
    }
    let mut a = x.to_vec();
    let mut b = y.to_vec();
    let original: Vec<f64> = (0..m).map(|j| x.iter().map(|row| row[j] * row[j]).sum::<f64>().sqrt()).collect();
    for j in 0..m {
        let norm = (j..n).map(|i| a[i][j] * a[i][j]).sum::<f64>().sqrt();
        if !(norm > LEAST_SQUARES_RANK_TOLERANCE * original[j]) || !norm.is_finite() {
            return None;
        }
        let alpha = if a[j][j] >= 0.0 { -norm } else { norm };
        let mut h: Vec<f64> = (j..n).map(|i| a[i][j]).collect();
        h[0] -= alpha;
        let hh: f64 = h.iter().map(|v| v * v).sum();
        for k in j..m {
            let d: f64 = (j..n).map(|i| h[i - j] * a[i][k]).sum();
            let f = 2.0 * d / hh;
            for i in j..n {
                a[i][k] -= f * h[i - j];
            }
        }
        for k in 0..r {
            let d: f64 = (j..n).map(|i| h[i - j] * b[i][k]).sum();
            let f = 2.0 * d / hh;
            for i in j..n {
                b[i][k] -= f * h[i - j];
            }
        }
    }
    let mut out = vec![vec![0.0; r]; m];
    for k in 0..r {
        for j in (0..m).rev() {
            let mut s = b[j][k];
            for (l, row) in out.iter().enumerate().skip(j + 1) {
                s -= a[j][l] * row[k];
            }
            out[j][k] = s / a[j][j];
        }
    }
    Some(out)
}

#[cfg(test)]
mod stats_linalg_r_golden;

#[cfg(test)]
mod tests {
    use super::*;

    /// The largest |entry| of M.
    fn largest(m: &[Vec<f64>]) -> f64 {
        m.iter().flatten().fold(0.0, |a, x| a.max(x.abs()))
    }

    /// ‖VᵀV − I‖ and ‖A − VΛVᵀ‖, each as its largest |entry|, the second over
    /// A's largest.
    fn orthogonality_and_reconstruction(a: &[Vec<f64>], values: &[f64], vectors: &[Vec<f64>]) -> (f64, f64) {
        let n = a.len();
        let mut orth = 0.0f64;
        let mut recon = 0.0f64;
        for i in 0..n {
            for j in 0..n {
                let vv: f64 = (0..n).map(|r| vectors[i][r] * vectors[j][r]).sum();
                orth = orth.max((vv - if i == j { 1.0 } else { 0.0 }).abs());
                let back: f64 = (0..n).map(|k| vectors[k][i] * values[k] * vectors[k][j]).sum();
                recon = recon.max((a[i][j] - back).abs());
            }
        }
        (orth, recon / largest(a))
    }

    fn close(got: f64, want: f64, ulps: f64) -> bool {
        (got - want).abs() <= ulps * f64::EPSILON * want.abs().max(1.0)
    }

    #[test]
    fn a_diagonal_matrix_gives_its_diagonal_in_descending_order_with_unit_vectors() {
        let a = vec![vec![3.0, 0.0, 0.0], vec![0.0, 1.0, 0.0], vec![0.0, 0.0, 2.0]];
        let (values, vectors) = eigen_symmetric(&a).unwrap();
        assert_eq!(values, vec![3.0, 2.0, 1.0]);
        assert_eq!(vectors, vec![vec![1.0, 0.0, 0.0], vec![0.0, 0.0, 1.0], vec![0.0, 1.0, 0.0]]);
    }

    /// [[2, 1], [1, 3]]: λ = 5/2 ± √(1/4 + 1), by hand.
    #[test]
    fn a_two_by_two_matrix_gives_the_formulas_eigenvalues() {
        let a = vec![vec![2.0, 1.0], vec![1.0, 3.0]];
        let (values, vectors) = eigen_symmetric(&a).unwrap();
        let r = (0.25f64 + 1.0).sqrt();
        assert!(close(values[0], 2.5 + r, 4.0) && close(values[1], 2.5 - r, 4.0), "{values:?}");
        let (orth, recon) = orthogonality_and_reconstruction(&a, &values, &vectors);
        assert!(orth <= 4.0 * f64::EPSILON && recon <= 4.0 * f64::EPSILON, "{orth:e} {recon:e}");
    }

    /// The equicorrelation matrix of order 5 at ρ = 0.4: 1 + 4ρ = 2.6 once,
    /// with the vector of ones over √5, and 1 − ρ = 0.6 four times.
    #[test]
    fn an_equicorrelation_matrix_gives_its_known_eigenvalues() {
        let a: Vec<Vec<f64>> = (0..5).map(|i| (0..5).map(|j| if i == j { 1.0 } else { 0.4 }).collect()).collect();
        let (values, vectors) = eigen_symmetric(&a).unwrap();
        assert!(close(values[0], 2.6, 8.0), "{values:?}");
        assert!(values[1..].iter().all(|v| close(*v, 0.6, 8.0)), "{values:?}");
        let unit = 1.0 / 5f64.sqrt();
        assert!(vectors[0].iter().all(|x| close(*x, unit, 8.0)), "{:?}", vectors[0]);
        let (orth, recon) = orthogonality_and_reconstruction(&a, &values, &vectors);
        assert!(orth <= 8.0 * f64::EPSILON && recon <= 8.0 * f64::EPSILON, "{orth:e} {recon:e}");
    }

    /// The Hilbert matrix of order 4: R 4.6.1's eigen in this session
    /// (~/gaply-corpus/os/phase9/9b1/logs/hilbert4_eigen_*.log); its exact
    /// determinant is 1 / 6,048,000.
    #[test]
    fn the_hilbert_matrix_of_order_4_gives_rs_eigenvalues_and_its_exact_determinant() {
        let a: Vec<Vec<f64>> = (0..4).map(|i| (0..4).map(|j| 1.0 / (i + j + 1) as f64).collect()).collect();
        let (values, vectors) = eigen_symmetric(&a).unwrap();
        let r = [1.5002142800592424, 0.1691412202214497, 0.0067382736057606122, 9.670230402261135e-05];
        for (got, want) in values.iter().zip(r) {
            assert!(((got - want) / want).abs() <= 1e-12, "{got} {want}");
        }
        let (orth, recon) = orthogonality_and_reconstruction(&a, &values, &vectors);
        assert!(orth <= 1e-14 && recon <= 1e-14, "{orth:e} {recon:e}");
        let l = cholesky(&a).unwrap();
        let exact = (1.0f64 / 6_048_000.0).ln();
        assert!((log_det(&l) - exact).abs() <= 1e-12, "{} {exact}", log_det(&l));
        let inv = spd_inverse(&l);
        for i in 0..4 {
            for j in 0..4 {
                let p: f64 = (0..4).map(|k| a[i][k] * inv[k][j]).sum();
                assert!((p - if i == j { 1.0 } else { 0.0 }).abs() <= 1e-10, "{i} {j} {p}");
            }
        }
    }

    /// Decision 699: a_12 = 0 with a_11 = a_22 gives θ = 0/0 unless the pair is
    /// skipped. Its eigenvalues are 1 + √2/2, 1 and 1 − √2/2, by hand.
    #[test]
    fn a_zero_pair_is_skipped_and_gives_no_nan() {
        let a = vec![vec![1.0, 0.0, 0.5], vec![0.0, 1.0, 0.5], vec![0.5, 0.5, 1.0]];
        let (values, vectors) = eigen_symmetric(&a).unwrap();
        assert!(values.iter().chain(vectors.iter().flatten()).all(|x| x.is_finite()), "{values:?} {vectors:?}");
        let h = 2f64.sqrt() / 2.0;
        assert!(close(values[0], 1.0 + h, 8.0) && close(values[1], 1.0, 8.0) && close(values[2], 1.0 - h, 8.0), "{values:?}");
    }

    /// Decision 699's sign rule, both branches.
    #[test]
    fn the_sign_rule_makes_the_sum_positive_or_else_the_first_entry_above_its_tolerance() {
        assert!(sign_reversed(&[-0.5, 0.2, -0.1]));
        assert!(!sign_reversed(&[0.5, -0.2, 0.1]));
        // A sum that is 0 but for rounding: the first entry above 1e-10 decides.
        assert!(sign_reversed(&[1e-12, -0.7071067811865476, 0.7071067811865475]));
        assert!(!sign_reversed(&[0.7071067811865476, -0.7071067811865475, 0.0]));
        assert!(!sign_reversed(&[0.0, 0.0]));
    }

    #[test]
    fn every_eigenvector_follows_the_sign_rule() {
        let a: Vec<Vec<f64>> = (0..6).map(|i| (0..6).map(|j| 1.0 / (1.0 + (i as f64 - j as f64).abs())).collect()).collect();
        let (_, vectors) = eigen_symmetric(&a).unwrap();
        for v in &vectors {
            assert!(!sign_reversed(v), "{v:?}");
        }
    }

    #[test]
    fn a_matrix_holding_a_nan_or_an_infinity_is_refused() {
        assert!(eigen_symmetric(&[vec![1.0, f64::NAN], vec![f64::NAN, 1.0]]).is_none());
        assert!(eigen_symmetric(&[vec![f64::INFINITY]]).is_none());
        assert!(least_squares(&[vec![f64::NAN]], &[vec![1.0]]).is_none());
    }

    #[test]
    fn jacobi_refuses_when_its_sweeps_run_out() {
        let a: Vec<Vec<f64>> = (0..5).map(|i| (0..5).map(|j| 1.0 / (i + j + 1) as f64).collect()).collect();
        assert!(eigen_within(&a, 1).is_none());
        assert!(eigen_within(&a, JACOBI_MAX_SWEEPS).is_some());
    }

    #[test]
    fn repeated_runs_give_the_same_bits() {
        let a: Vec<Vec<f64>> = (0..7).map(|i| (0..7).map(|j| ((i * 7 + j * 3) % 5) as f64 / 10.0 + if i == j { 2.0 } else { 0.0 }).collect()).collect();
        let a: Vec<Vec<f64>> = (0..7).map(|i| (0..7).map(|j| (a[i][j] + a[j][i]) / 2.0).collect()).collect();
        let bits = |r: &(Vec<f64>, Vec<Vec<f64>>)| r.0.iter().chain(r.1.iter().flatten()).map(|x| x.to_bits()).collect::<Vec<u64>>();
        assert_eq!(bits(&eigen_symmetric(&a).unwrap()), bits(&eigen_symmetric(&a).unwrap()));
        let l1 = cholesky(&a).unwrap();
        let l2 = cholesky(&a).unwrap();
        assert_eq!(l1, l2);
        assert_eq!(log_det(&l1).to_bits(), log_det(&l2).to_bits());
    }

    /// [[1, r], [r, 1]]: the value left at row 2 is 1 − r², refused at or
    /// below 1e-9 (decision 699).
    #[test]
    fn cholesky_refuses_at_or_below_its_tolerance_naming_the_row() {
        assert_eq!(cholesky(&[vec![1.0, 1.0], vec![1.0, 1.0]]), Err(2));
        let r = |left: f64| (1.0 - left).sqrt();
        assert_eq!(cholesky(&[vec![1.0, r(0.5e-9)], vec![r(0.5e-9), 1.0]]), Err(2));
        assert!(cholesky(&[vec![1.0, r(2e-9)], vec![r(2e-9), 1.0]]).is_ok());
        assert_eq!(cholesky(&[vec![0.0]]), Err(1));
        assert_eq!(cholesky(&[vec![f64::NAN]]), Err(1));
    }

    /// diag(2, 3): ln 6; a 2 x 2 factor by hand.
    #[test]
    fn log_det_is_twice_the_sum_of_the_logs_of_the_factors_diagonal() {
        let l = cholesky(&[vec![2.0, 0.0], vec![0.0, 3.0]]).unwrap();
        assert!((log_det(&l) - 6f64.ln()).abs() <= 4.0 * f64::EPSILON);
        let l = cholesky(&[vec![4.0, 2.0], vec![2.0, 5.0]]).unwrap();
        assert_eq!(l, vec![vec![2.0, 0.0], vec![1.0, 2.0]]);
        assert!((log_det(&l) - 16f64.ln()).abs() <= 4.0 * f64::EPSILON);
        let inv = spd_inverse(&l);
        // [[4, 2], [2, 5]]⁻¹ = [[5, −2], [−2, 4]] / 16.
        let want = [[5.0 / 16.0, -2.0 / 16.0], [-2.0 / 16.0, 4.0 / 16.0]];
        for i in 0..2 {
            for j in 0..2 {
                assert!(close(inv[i][j], want[i][j], 4.0), "{inv:?}");
            }
        }
    }

    /// No intercept: x = (1, 2, 3), y = (1, 1, 2) gives Σxy / Σx² = 9/14 (a
    /// model with an intercept gives 1/2).
    #[test]
    fn least_squares_has_no_intercept_and_solves_several_right_hand_sides() {
        let b = least_squares(&[vec![1.0], vec![2.0], vec![3.0]], &[vec![1.0, 2.0], vec![1.0, 4.0], vec![2.0, 6.0]]).unwrap();
        assert!(close(b[0][0], 9.0 / 14.0, 8.0), "{b:?}");
        assert!(close(b[0][1], 2.0, 8.0), "{b:?}");
        let x = vec![vec![1.0, 2.0], vec![3.0, -1.0], vec![0.5, 4.0], vec![-2.0, 1.0]];
        let truth = [[1.5, -0.25], [0.75, 2.0]];
        let y: Vec<Vec<f64>> = x.iter().map(|r| (0..2).map(|k| r[0] * truth[0][k] + r[1] * truth[1][k]).collect()).collect();
        let b = least_squares(&x, &y).unwrap();
        for j in 0..2 {
            for k in 0..2 {
                assert!(close(b[j][k], truth[j][k], 16.0), "{b:?}");
            }
        }
        assert!(least_squares(&[vec![1.0, 2.0]], &[vec![1.0]]).is_none(), "fewer rows than columns");
        assert!(least_squares(&[vec![1.0, 2.0], vec![2.0, 4.0], vec![3.0, 6.0]], &[vec![1.0], vec![2.0], vec![3.0]]).is_none(), "collinear");
    }
}
