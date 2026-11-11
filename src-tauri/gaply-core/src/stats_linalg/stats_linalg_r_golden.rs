//! stats_linalg against R (Research OS, Phase 9B-1, decision 699): R 4.6.1's
//! `eigen`, `chol`, `chol2inv`, `determinant(log = TRUE)`, `svd` and
//! `qr.solve` over `tests/fixtures/stats_linalg/make_linalg_fixtures.R`'s 30
//! matrices, 3 to 128 rows, condition numbers from 1 to 1.5e10, near-ties and
//! exact ties.
//!
//! A unit test inside the module rather than a file under `tests/`: the
//! functions are `pub(crate)` (decision 699), which an integration test cannot
//! reach. It reads its fixture with `include_str!`, so the module's purity scan
//! (`tests/contracts_are_pure.rs`) still finds no filesystem here; and since
//! that scan forbids printing, each family's measured error is reported in the
//! assertion's message, which a run with the gates at 0 shows in full.
//!
//! - Eigenvalues: every one, by the largest |λ| of the case.
//! - Eigenvectors: only where the gap to each neighbouring eigenvalue is at
//!   least 1e-6 of the largest; within a smaller gap a vector is one choice of
//!   many, and only the eigenvalues and the reconstruction are compared. Signs
//!   are set by the same rule on both sides.
//! - Cholesky: where R's factor passes decision 699's rule, L, A⁻¹ and ln|A|;
//!   where it does not, Rust must refuse at the same row; a case within a
//!   factor of 2 of the rule is counted and not compared.
//! - The polar factor U Vᵀ and the sum of the singular values; least squares.
//!
//! Gates are tiered by the condition number, since the errors of an inverse,
//! an eigenvector and a solution grow with it.

use super::*;

const LINALG: &str = include_str!("../../tests/fixtures/stats_linalg/linalg.tsv");
const CASES: &str = include_str!("../../tests/fixtures/stats_linalg/cases.tsv");

/// The tiers of the condition number: at most 1e3, at most 1e8, above.
fn tier(condition: f64) -> usize {
    if condition <= 1e3 {
        0
    } else if condition <= 1e8 {
        1
    } else {
        2
    }
}

/// Each family's gate in each tier: ten times the largest error measured in
/// it, rounded up (macOS aarch64, 11 Nov 2026; the measurement is
/// ~/gaply-corpus/os/phase9/9b1/logs/p1_linalg_measure_1794410002.log, a run with every gate
/// at 0). A tier with no case is `None`, and a case there fails the test.
///
/// | family | ≤ 1e3 | ≤ 1e8 | > 1e8 |
/// |---|---|---|---|
/// | eigenvalue (by the largest abs λ) | 1.727e-15 | 1.887e-15 | 3.109e-15 |
/// | eigenvector | 1.367e-13 | 5.706e-12 | 2.935e-11 |
/// | reconstruction (by the largest abs a) | 2.442e-15 | 1.577e-15 | 9.795e-16 |
/// | orthogonality | 1.110e-15 | 4.441e-16 | 6.661e-16 |
/// | cholesky (by the largest abs l) | 8.882e-16 | 8.123e-14 | 2.947e-12 |
/// | inverse (by the largest abs entry) | 7.275e-15 | 2.377e-10 | 8.260e-8 |
/// | log_det (by max(abs ln det, 1)) | 2.572e-16 | 3.384e-11 | 6.557e-10 |
/// | polar | 1.332e-15 | none | none |
/// | sigma_sum (relative) | 1.101e-15 | none | none |
/// | least_squares (by the largest abs b) | 6.777e-16 | 8.614e-12 | none |
const GATES: &[(&str, [Option<f64>; 3])] = &[
    ("eigenvalue", [Some(1.8e-14), Some(1.9e-14), Some(3.2e-14)]),
    ("eigenvector", [Some(1.4e-12), Some(5.8e-11), Some(3e-10)]),
    ("reconstruction", [Some(2.5e-14), Some(1.6e-14), Some(9.8e-15)]),
    ("orthogonality", [Some(1.2e-14), Some(4.5e-15), Some(6.7e-15)]),
    ("cholesky", [Some(8.9e-15), Some(8.2e-13), Some(3e-11)]),
    ("inverse", [Some(7.3e-14), Some(2.4e-9), Some(8.3e-7)]),
    ("log_det", [Some(2.6e-15), Some(3.4e-10), Some(6.6e-9)]),
    ("polar", [Some(1.4e-14), None, None]),
    ("sigma_sum", [Some(1.2e-14), None, None]),
    ("least_squares", [Some(6.8e-15), Some(8.7e-11), None]),
];

struct Case {
    family: String,
    condition: f64,
    items: Vec<(String, usize, usize, f64)>,
}

fn cases() -> Vec<(String, Case)> {
    let mut out: Vec<(String, Case)> = Vec::new();
    for line in CASES.lines().filter(|l| !l.starts_with('#')) {
        let f: Vec<&str> = line.split('\t').collect();
        out.push((
            f[0].to_string(),
            Case {
                family: f[1].to_string(),
                condition: f[3].parse().unwrap_or(f64::INFINITY),
                items: Vec::new(),
            },
        ));
    }
    for line in LINALG.lines().filter(|l| !l.starts_with('#')) {
        let f: Vec<&str> = line.split('\t').collect();
        let case = out.iter_mut().find(|(n, _)| n == f[0]).expect("a case of cases.tsv");
        case.1.items.push((f[1].to_string(), f[2].parse().unwrap(), f[3].parse().unwrap(), f[4].parse().unwrap()));
    }
    out
}

/// The full matrix `item` of a case; `symmetric` fills the upper triangle
/// from the lower one written.
fn matrix(case: &Case, item: &str, symmetric: bool) -> Option<Vec<Vec<f64>>> {
    let cells: Vec<&(String, usize, usize, f64)> = case.items.iter().filter(|c| c.0 == item).collect();
    if cells.is_empty() {
        return None;
    }
    let rows = cells.iter().map(|c| c.1).max().unwrap() + 1;
    let cols = cells.iter().map(|c| c.2).max().unwrap() + 1;
    let mut m = vec![vec![0.0; cols.max(if symmetric { rows } else { 0 })]; rows];
    for c in cells {
        m[c.1][c.2] = c.3;
        if symmetric {
            m[c.2][c.1] = c.3;
        }
    }
    Some(m)
}

fn largest(m: &[Vec<f64>]) -> f64 {
    m.iter().flatten().fold(0.0, |a, x| a.max(x.abs()))
}

fn largest_difference(a: &[Vec<f64>], b: &[Vec<f64>]) -> f64 {
    assert_eq!((a.len(), a[0].len()), (b.len(), b[0].len()), "shapes");
    a.iter().flatten().zip(b.iter().flatten()).fold(0.0, |m, (x, y)| m.max((x - y).abs()))
}

#[test]
fn stats_linalg_agrees_with_r_within_each_familys_gate() {
    let mut worst: Vec<(&str, [f64; 3])> = GATES.iter().map(|(f, _)| (*f, [0.0; 3])).collect();
    let mut see = |family: &str, t: usize, e: f64| {
        let w = worst.iter_mut().find(|(f, _)| *f == family).expect("a family");
        assert!(e.is_finite(), "{family}: a non-finite error");
        w.1[t] = w.1[t].max(e);
    };
    let (mut vectors_compared, mut vectors_skipped, mut refusals, mut borderline, mut symmetric) = (0, 0, 0, 0, 0);
    let all = cases();
    assert_eq!(all.len(), 30, "the fixture's cases");
    for (name, case) in &all {
        let t = tier(case.condition);
        match case.family.as_str() {
            "symmetric" => {
                symmetric += 1;
                let a = matrix(case, "A", true).unwrap();
                let n = a.len();
                let (values, vectors) = eigen_symmetric(&a).unwrap_or_else(|| panic!("{name}: eigen refused"));
                let r_values: Vec<f64> = (0..n).map(|k| matrix(case, "eval", false).unwrap()[k][0]).collect();
                let scale = r_values.iter().fold(0.0f64, |m, v| m.max(v.abs()));
                see("eigenvalue", t, values.iter().zip(&r_values).fold(0.0f64, |m, (x, y)| m.max((x - y).abs())) / scale);
                let r_vectors = matrix(case, "evec", false).unwrap();
                for k in 0..n {
                    let gap = [k.checked_sub(1), Some(k + 1).filter(|j| *j < n)]
                        .iter()
                        .flatten()
                        .map(|j| (r_values[k] - r_values[*j]).abs())
                        .fold(f64::INFINITY, f64::min);
                    if gap >= 1e-6 * scale {
                        vectors_compared += 1;
                        see("eigenvector", t, (0..n).map(|r| (vectors[k][r] - r_vectors[r][k]).abs()).fold(0.0, f64::max));
                    } else {
                        vectors_skipped += 1;
                    }
                }
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
                see("orthogonality", t, orth);
                see("reconstruction", t, recon / largest(&a));
                if case.items.iter().any(|c| c.0 == "chol_borderline") {
                    borderline += 1;
                } else if let Some(r) = case.items.iter().find(|c| c.0 == "chol_refused") {
                    refusals += 1;
                    assert_eq!(cholesky(&a), Err(r.1), "{name}: refused at R's row");
                } else {
                    let l = cholesky(&a).unwrap_or_else(|j| panic!("{name}: refused at {j}, R did not"));
                    let r_l = matrix(case, "chol", true).unwrap();
                    let lower: Vec<Vec<f64>> = (0..n).map(|i| (0..n).map(|j| if j <= i { r_l[i][j] } else { 0.0 }).collect()).collect();
                    see("cholesky", t, largest_difference(&l, &lower) / largest(&lower));
                    let r_inv = matrix(case, "inv", true).unwrap();
                    see("inverse", t, largest_difference(&spd_inverse(&l), &r_inv) / largest(&r_inv));
                    let r_logdet = matrix(case, "logdet", false).unwrap()[0][0];
                    see("log_det", t, (log_det(&l) - r_logdet).abs() / r_logdet.abs().max(1.0));
                }
            }
            "polar" => {
                let b = matrix(case, "B", false).unwrap();
                let (p, sum) = polar_factor(&b).unwrap_or_else(|| panic!("{name}: polar refused"));
                see("polar", t, largest_difference(&p, &matrix(case, "polar", false).unwrap()));
                let r_sum = matrix(case, "sigma_sum", false).unwrap()[0][0];
                see("sigma_sum", t, (sum - r_sum).abs() / r_sum);
            }
            "least_squares" => {
                let x = matrix(case, "X", false).unwrap();
                let y = matrix(case, "Y", false).unwrap();
                let got = least_squares(&x, &y).unwrap_or_else(|| panic!("{name}: least squares refused"));
                let want = matrix(case, "ls", false).unwrap();
                see("least_squares", t, largest_difference(&got, &want) / largest(&want));
            }
            other => panic!("{name}: family {other}"),
        }
    }
    assert_eq!((symmetric, refusals, borderline), (20, 1, 1), "the fixture's symmetric cases, refusals and borderline cases");
    assert!(vectors_compared > 100 && vectors_skipped >= 5, "{vectors_compared} vectors compared, {vectors_skipped} skipped");
    let table: String = worst.iter().map(|(f, w)| format!("\n  {f:15} {:.3e} {:.3e} {:.3e}", w[0], w[1], w[2])).collect();
    for ((family, w), (_, gates)) in worst.iter().zip(GATES) {
        for t in 0..3 {
            match gates[t] {
                Some(g) => assert!(w[t] <= g, "{family}, tier {t}: {:e} over {g:e}; measured (tier ≤1e3, ≤1e8, >1e8):{table}\n  vectors compared {vectors_compared}, skipped {vectors_skipped}", w[t]),
                None => assert_eq!(w[t], 0.0, "{family}, tier {t}: a case where none was expected"),
            }
        }
    }
}
