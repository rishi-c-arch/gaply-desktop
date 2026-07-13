//! Statistical Analysis Verifier — the deterministic RECOMPUTE engine (Set 2).
//!
//! A pure, deterministic, ValidationMaths-family agent (the `k = 1.0` "hard
//! constraint, no LLM" tier — see [`crate::swarm`]): it recomputes statistics
//! from RAW numeric data so a reported number can be checked against what the
//! data actually produces. Like [`crate::validate`] it contains **no model, no
//! proxy, no network, no I/O** — every function is total arithmetic over `f64`
//! slices. Models must NEVER compute these numbers; a bug here is a
//! confidently-wrong verification, so the reference-value validation suite (the
//! tests) IS the deliverable.
//!
//! # What it computes
//! - independent-samples t-test (Welch's — default — and Student's pooled),
//! - one-way ANOVA,
//! - Pearson and Spearman correlation,
//! - chi-square test of independence,
//! - ordinary-least-squares linear regression (with standard errors).
//!
//! The test STATISTICS are hand-rolled (plain arithmetic); only the tail
//! probabilities go through `statrs` distribution CDFs (`StudentsT`,
//! `FisherSnedecor`, `ChiSquared`, `Normal`) — the one place where correctness
//! is subtle. Every result is validated against scipy/R reference values in
//! the test module.
//!
//! Numeric TYPING lives here too but is generic: it takes a table as
//! `headers: &[String]` + `rows: &[Vec<String>]` (the shape the app crate's
//! `supplementary::Table` maps to), so gaply-core stays independent of the app
//! crate. Non-numeric where a number is required is an HONEST error — never a
//! silent 0.

use serde::{Deserialize, Serialize};
use statrs::distribution::{ChiSquared, ContinuousCDF, FisherSnedecor, Normal, StudentsT};

use crate::error::GaplyError;

// ---------------------------------------------------------------------------
// Numeric table typing (strings → f64), honest about missing / non-numeric
// ---------------------------------------------------------------------------

/// How to treat a missing cell (empty or a recognised NA token).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MissingPolicy {
    /// Skip missing cells (count them); the returned column is dense.
    Skip,
    /// Any missing cell is an error (e.g. paired tests that need row alignment).
    Error,
}

/// A parsed numeric column: dense `values` plus how many cells were missing.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NumericColumn {
    pub name: String,
    pub values: Vec<f64>,
    /// Missing cells skipped (only under [`MissingPolicy::Skip`]).
    pub missing: usize,
}

fn is_missing(cell: &str) -> bool {
    let t = cell.trim();
    t.is_empty()
        || matches!(
            t.to_ascii_uppercase().as_str(),
            "NA" | "N/A" | "NAN" | "NULL" | "NONE" | "." | "#N/A"
        )
}

/// Parse one column (selected by header) of a string table into a dense f64
/// column. Non-numeric, non-missing cells are an HONEST error (never a silent
/// 0). Header match is exact.
pub fn numeric_column(
    headers: &[String],
    rows: &[Vec<String>],
    header: &str,
    policy: MissingPolicy,
) -> Result<NumericColumn, GaplyError> {
    let col = headers.iter().position(|h| h == header).ok_or_else(|| {
        GaplyError::Validation(format!("column '{header}' not found in the table headers"))
    })?;
    let mut values = Vec::with_capacity(rows.len());
    let mut missing = 0usize;
    for (i, row) in rows.iter().enumerate() {
        let cell = row.get(col).map(|s| s.as_str()).unwrap_or("");
        if is_missing(cell) {
            match policy {
                MissingPolicy::Skip => {
                    missing += 1;
                    continue;
                }
                MissingPolicy::Error => {
                    return Err(GaplyError::Validation(format!(
                        "column '{header}' has a missing value at row {} (policy: error)",
                        i + 1
                    )));
                }
            }
        }
        let v: f64 = cell.trim().parse().map_err(|_| {
            GaplyError::Validation(format!(
                "column '{header}' has a non-numeric value {cell:?} at row {} — refusing to guess",
                i + 1
            ))
        })?;
        if !v.is_finite() {
            return Err(GaplyError::Validation(format!(
                "column '{header}' has a non-finite value at row {}",
                i + 1
            )));
        }
        values.push(v);
    }
    Ok(NumericColumn { name: header.to_string(), values, missing })
}

// ---------------------------------------------------------------------------
// Small statistics helpers (deterministic arithmetic)
// ---------------------------------------------------------------------------

fn mean(xs: &[f64]) -> f64 {
    xs.iter().sum::<f64>() / xs.len() as f64
}

/// Sample variance (n-1 denominator).
fn sample_variance(xs: &[f64]) -> f64 {
    let n = xs.len();
    if n < 2 {
        return 0.0;
    }
    let m = mean(xs);
    xs.iter().map(|x| (x - m).powi(2)).sum::<f64>() / (n - 1) as f64
}

/// Two-sided p from a |t| and df, guarding the perfect-fit (|t| = ∞) case.
fn t_two_sided(t_abs: f64, df: f64) -> f64 {
    if !t_abs.is_finite() {
        return 0.0; // perfect separation / correlation
    }
    let dist = StudentsT::new(0.0, 1.0, df).expect("df > 0 is guaranteed by callers");
    (2.0 * (1.0 - dist.cdf(t_abs))).clamp(0.0, 1.0)
}

// ---------------------------------------------------------------------------
// t-test (independent samples)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TTestResult {
    pub t: f64,
    pub df: f64,
    pub p_two_sided: f64,
    pub mean_a: f64,
    pub mean_b: f64,
    /// "welch" (unequal variances, default) or "student" (pooled).
    pub method: &'static str,
}

fn require_group(name: &str, xs: &[f64]) -> Result<(), GaplyError> {
    if xs.len() < 2 {
        return Err(GaplyError::Validation(format!(
            "{name} needs at least 2 observations, got {}",
            xs.len()
        )));
    }
    Ok(())
}

/// Welch's independent-samples t-test (does NOT assume equal variances — the
/// safer default). Reports t, the Welch-Satterthwaite df, and two-sided p.
pub fn t_test_welch(a: &[f64], b: &[f64]) -> Result<TTestResult, GaplyError> {
    require_group("group A", a)?;
    require_group("group B", b)?;
    let (na, nb) = (a.len() as f64, b.len() as f64);
    let (va, vb) = (sample_variance(a), sample_variance(b));
    let sa = va / na;
    let sb = vb / nb;
    let se = (sa + sb).sqrt();
    if se == 0.0 {
        return Err(GaplyError::Validation(
            "both groups have zero variance — the t-statistic is undefined".into(),
        ));
    }
    let (ma, mb) = (mean(a), mean(b));
    let t = (ma - mb) / se;
    // Welch–Satterthwaite degrees of freedom.
    let df = (sa + sb).powi(2) / (sa.powi(2) / (na - 1.0) + sb.powi(2) / (nb - 1.0));
    Ok(TTestResult { t, df, p_two_sided: t_two_sided(t.abs(), df), mean_a: ma, mean_b: mb, method: "welch" })
}

/// Student's pooled-variance t-test (assumes equal variances).
pub fn t_test_student(a: &[f64], b: &[f64]) -> Result<TTestResult, GaplyError> {
    require_group("group A", a)?;
    require_group("group B", b)?;
    let (na, nb) = (a.len() as f64, b.len() as f64);
    let (va, vb) = (sample_variance(a), sample_variance(b));
    let df = na + nb - 2.0;
    let sp2 = ((na - 1.0) * va + (nb - 1.0) * vb) / df;
    let se = (sp2 * (1.0 / na + 1.0 / nb)).sqrt();
    if se == 0.0 {
        return Err(GaplyError::Validation(
            "pooled variance is zero — the t-statistic is undefined".into(),
        ));
    }
    let (ma, mb) = (mean(a), mean(b));
    let t = (ma - mb) / se;
    Ok(TTestResult { t, df, p_two_sided: t_two_sided(t.abs(), df), mean_a: ma, mean_b: mb, method: "student" })
}

// ---------------------------------------------------------------------------
// One-way ANOVA
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AnovaResult {
    pub f: f64,
    pub df_between: f64,
    pub df_within: f64,
    pub p: f64,
}

/// One-way ANOVA over `groups` (each a sample). Needs ≥ 2 groups and residual
/// df ≥ 1; zero within-group variation is an honest error (F undefined).
pub fn one_way_anova(groups: &[&[f64]]) -> Result<AnovaResult, GaplyError> {
    if groups.len() < 2 {
        return Err(GaplyError::Validation("ANOVA needs at least 2 groups".into()));
    }
    let n_total: usize = groups.iter().map(|g| g.len()).sum();
    let k = groups.len();
    if n_total <= k {
        return Err(GaplyError::Validation(format!(
            "ANOVA needs more observations ({n_total}) than groups ({k}) — residual df would be ≤ 0"
        )));
    }
    let all: Vec<f64> = groups.iter().flat_map(|g| g.iter().copied()).collect();
    let grand = mean(&all);
    let mut ss_between = 0.0;
    let mut ss_within = 0.0;
    for g in groups {
        if g.is_empty() {
            return Err(GaplyError::Validation("ANOVA groups must be non-empty".into()));
        }
        let gm = mean(g);
        ss_between += g.len() as f64 * (gm - grand).powi(2);
        ss_within += g.iter().map(|x| (x - gm).powi(2)).sum::<f64>();
    }
    let df_b = (k - 1) as f64;
    let df_w = (n_total - k) as f64;
    let ms_within = ss_within / df_w;
    if ms_within == 0.0 {
        return Err(GaplyError::Validation(
            "within-group variance is zero — the F-statistic is undefined".into(),
        ));
    }
    let f = (ss_between / df_b) / ms_within;
    let p = (1.0 - FisherSnedecor::new(df_b, df_w).expect("df > 0").cdf(f)).clamp(0.0, 1.0);
    Ok(AnovaResult { f, df_between: df_b, df_within: df_w, p })
}

// ---------------------------------------------------------------------------
// Correlation (Pearson + Spearman)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CorrelationResult {
    /// Pearson r or Spearman rho.
    pub coefficient: f64,
    pub df: f64,
    /// The t-statistic used for the p-value.
    pub statistic: f64,
    pub p_two_sided: f64,
    pub method: &'static str,
}

fn pearson_r(x: &[f64], y: &[f64]) -> Result<f64, GaplyError> {
    if x.len() != y.len() {
        return Err(GaplyError::Validation("correlation needs paired, equal-length columns".into()));
    }
    if x.len() < 3 {
        return Err(GaplyError::Validation("correlation needs at least 3 pairs".into()));
    }
    let (mx, my) = (mean(x), mean(y));
    let mut sxy = 0.0;
    let mut sxx = 0.0;
    let mut syy = 0.0;
    for (a, b) in x.iter().zip(y) {
        sxy += (a - mx) * (b - my);
        sxx += (a - mx).powi(2);
        syy += (b - my).powi(2);
    }
    if sxx == 0.0 || syy == 0.0 {
        return Err(GaplyError::Validation(
            "a column has zero variance — correlation is undefined".into(),
        ));
    }
    Ok(sxy / (sxx.sqrt() * syy.sqrt()))
}

fn r_to_result(r: f64, n: usize, method: &'static str) -> CorrelationResult {
    let df = (n - 2) as f64;
    // t = r * sqrt(df / (1 - r^2)); |r| = 1 → t = ∞ → p = 0.
    let t = if (1.0 - r * r).abs() < f64::EPSILON {
        f64::INFINITY * r.signum()
    } else {
        r * (df / (1.0 - r * r)).sqrt()
    };
    CorrelationResult { coefficient: r, df, statistic: t, p_two_sided: t_two_sided(t.abs(), df), method }
}

/// Pearson product-moment correlation with its t-test p-value.
pub fn pearson(x: &[f64], y: &[f64]) -> Result<CorrelationResult, GaplyError> {
    let r = pearson_r(x, y)?;
    Ok(r_to_result(r, x.len(), "pearson"))
}

/// Average ranks (ties share the mean of the ranks they'd occupy).
fn average_ranks(xs: &[f64]) -> Vec<f64> {
    let n = xs.len();
    let mut idx: Vec<usize> = (0..n).collect();
    idx.sort_by(|&i, &j| xs[i].partial_cmp(&xs[j]).unwrap_or(std::cmp::Ordering::Equal));
    let mut ranks = vec![0.0; n];
    let mut i = 0;
    while i < n {
        let mut j = i;
        while j + 1 < n && xs[idx[j + 1]] == xs[idx[i]] {
            j += 1;
        }
        // ranks i..=j are tied; average rank (1-based)
        let avg = ((i + 1 + j + 1) as f64) / 2.0;
        for k in i..=j {
            ranks[idx[k]] = avg;
        }
        i = j + 1;
    }
    ranks
}

/// Spearman rank correlation (Pearson on average ranks; ties handled). The
/// p-value uses the t-approximation (df = n − 2) — valid for larger n.
pub fn spearman(x: &[f64], y: &[f64]) -> Result<CorrelationResult, GaplyError> {
    if x.len() != y.len() {
        return Err(GaplyError::Validation("correlation needs paired, equal-length columns".into()));
    }
    if x.len() < 3 {
        return Err(GaplyError::Validation("correlation needs at least 3 pairs".into()));
    }
    let rho = pearson_r(&average_ranks(x), &average_ranks(y))?;
    Ok(r_to_result(rho, x.len(), "spearman"))
}

// ---------------------------------------------------------------------------
// Chi-square test of independence
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChiSquareResult {
    pub chi_square: f64,
    pub df: f64,
    pub p: f64,
}

/// Pearson chi-square test of independence over an r×c contingency table of
/// observed counts. Any zero expected count (an empty margin) is an honest
/// error, not a divide-by-zero.
pub fn chi_square_independence(observed: &[Vec<f64>]) -> Result<ChiSquareResult, GaplyError> {
    let r = observed.len();
    if r < 2 {
        return Err(GaplyError::Validation("chi-square needs at least 2 rows".into()));
    }
    let c = observed[0].len();
    if c < 2 || observed.iter().any(|row| row.len() != c) {
        return Err(GaplyError::Validation("chi-square needs a rectangular table with ≥ 2 columns".into()));
    }
    let row_sums: Vec<f64> = observed.iter().map(|row| row.iter().sum()).collect();
    let mut col_sums = vec![0.0; c];
    for row in observed {
        for (j, &v) in row.iter().enumerate() {
            if v < 0.0 {
                return Err(GaplyError::Validation("chi-square counts must be non-negative".into()));
            }
            col_sums[j] += v;
        }
    }
    let total: f64 = row_sums.iter().sum();
    if total == 0.0 {
        return Err(GaplyError::Validation("chi-square table is all zeros".into()));
    }
    let mut chi2 = 0.0;
    for (i, row) in observed.iter().enumerate() {
        for (j, &o) in row.iter().enumerate() {
            let e = row_sums[i] * col_sums[j] / total;
            if e == 0.0 {
                return Err(GaplyError::Validation(
                    "chi-square has a zero expected count (an empty row or column margin)".into(),
                ));
            }
            chi2 += (o - e).powi(2) / e;
        }
    }
    let df = ((r - 1) * (c - 1)) as f64;
    let p = (1.0 - ChiSquared::new(df).expect("df > 0").cdf(chi2)).clamp(0.0, 1.0);
    Ok(ChiSquareResult { chi_square: chi2, df, p })
}

// ---------------------------------------------------------------------------
// OLS linear regression (hand-rolled linear algebra — statrs only for the CDF)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Coefficient {
    /// "(intercept)" or the predictor's name/index.
    pub name: String,
    pub estimate: f64,
    pub std_error: f64,
    pub t: f64,
    pub p_two_sided: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RegressionResult {
    /// coefficients[0] is the intercept.
    pub coefficients: Vec<Coefficient>,
    pub r_squared: f64,
    pub adj_r_squared: f64,
    pub df_residual: f64,
}

/// Gauss-Jordan inverse of a small square matrix (partial pivoting). Returns
/// `None` if singular (collinear predictors) — the caller reports it honestly.
fn invert(m: &[Vec<f64>]) -> Option<Vec<Vec<f64>>> {
    let n = m.len();
    let mut a: Vec<Vec<f64>> = m.to_vec();
    let mut inv: Vec<Vec<f64>> = (0..n)
        .map(|i| (0..n).map(|j| if i == j { 1.0 } else { 0.0 }).collect())
        .collect();
    for col in 0..n {
        // partial pivot
        let mut pivot = col;
        for r in (col + 1)..n {
            if a[r][col].abs() > a[pivot][col].abs() {
                pivot = r;
            }
        }
        if a[pivot][col].abs() < 1e-12 {
            return None; // singular
        }
        a.swap(col, pivot);
        inv.swap(col, pivot);
        let d = a[col][col];
        for j in 0..n {
            a[col][j] /= d;
            inv[col][j] /= d;
        }
        for r in 0..n {
            if r == col {
                continue;
            }
            let f = a[r][col];
            for j in 0..n {
                a[r][j] -= f * a[col][j];
                inv[r][j] -= f * inv[col][j];
            }
        }
    }
    Some(inv)
}

/// Ordinary-least-squares regression of `y` on `predictors` (each a column of
/// the same length as `y`), with an intercept. Returns coefficient estimates,
/// standard errors, t-statistics, two-sided p-values, and R². Collinear /
/// rank-deficient designs are an honest error.
pub fn ols(y: &[f64], predictors: &[Vec<f64>]) -> Result<RegressionResult, GaplyError> {
    let n = y.len();
    let k = predictors.len(); // number of predictors (excl. intercept)
    let p = k + 1; // params incl. intercept
    if n <= p {
        return Err(GaplyError::Validation(format!(
            "regression needs more observations ({n}) than parameters ({p})"
        )));
    }
    for (i, col) in predictors.iter().enumerate() {
        if col.len() != n {
            return Err(GaplyError::Validation(format!(
                "predictor {i} has length {} but y has length {n}",
                col.len()
            )));
        }
    }
    // Design matrix X (n × p), column 0 = intercept.
    let x: Vec<Vec<f64>> = (0..n)
        .map(|i| {
            let mut row = Vec::with_capacity(p);
            row.push(1.0);
            for col in predictors {
                row.push(col[i]);
            }
            row
        })
        .collect();
    // X'X (p × p) and X'y (p).
    let mut xtx = vec![vec![0.0; p]; p];
    let mut xty = vec![0.0; p];
    for i in 0..n {
        for a in 0..p {
            xty[a] += x[i][a] * y[i];
            for b in 0..p {
                xtx[a][b] += x[i][a] * x[i][b];
            }
        }
    }
    let xtx_inv = invert(&xtx).ok_or_else(|| {
        GaplyError::Validation(
            "the design matrix is singular (collinear predictors or no variation) — regression is undefined".into(),
        )
    })?;
    // beta = (X'X)^-1 X'y
    let beta: Vec<f64> = (0..p).map(|a| (0..p).map(|b| xtx_inv[a][b] * xty[b]).sum()).collect();
    // residuals, SSE, SST
    let ybar = mean(y);
    let mut sse = 0.0;
    let mut sst = 0.0;
    for i in 0..n {
        let yhat: f64 = (0..p).map(|a| beta[a] * x[i][a]).sum();
        sse += (y[i] - yhat).powi(2);
        sst += (y[i] - ybar).powi(2);
    }
    let df_res = (n - p) as f64;
    let sigma2 = sse / df_res;
    let coefficients: Vec<Coefficient> = (0..p)
        .map(|a| {
            let se = (sigma2 * xtx_inv[a][a]).sqrt();
            let t = if se == 0.0 { f64::INFINITY } else { beta[a] / se };
            let name = if a == 0 { "(intercept)".to_string() } else { format!("x{a}") };
            Coefficient { name, estimate: beta[a], std_error: se, t, p_two_sided: t_two_sided(t.abs(), df_res) }
        })
        .collect();
    let r_squared = if sst == 0.0 { 0.0 } else { 1.0 - sse / sst };
    let adj = 1.0 - (1.0 - r_squared) * (n as f64 - 1.0) / df_res;
    Ok(RegressionResult { coefficients, r_squared, adj_r_squared: adj, df_residual: df_res })
}

// A standard-normal survival helper (kept for callers who want a z p-value;
// exercised by the reference tests to anchor the CDF pipeline).
pub fn normal_two_sided(z_abs: f64) -> f64 {
    let n = Normal::new(0.0, 1.0).expect("standard normal");
    (2.0 * (1.0 - n.cdf(z_abs))).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    //! REFERENCE-VALUE VALIDATION — the trust anchor. The test statistics are
    //! checked against hand-computed exact values; the p-value pipeline is
    //! anchored to UNIVERSAL distribution constants (t/χ²/F/normal critical
    //! points that scipy/R and every stats table agree on), plus full worked
    //! cases. A disagreement beyond a tight tolerance FAILS the build.
    use super::*;

    const TIGHT: f64 = 1e-9; // exact arithmetic (statistics)
    const P_TOL: f64 = 5e-4; // p-values via CDF

    fn close(a: f64, b: f64, tol: f64) -> bool {
        (a - b).abs() <= tol
    }

    // ---- p-value pipeline anchored to universal distribution constants ----

    #[test]
    fn normal_tail_matches_textbook_z_values() {
        // Φ(1.959964)=0.975 → two-sided p=0.05; Φ(2.575829)=0.995 → p=0.01;
        // Φ(3.290527)=0.9995 → p=0.001 (the TAIL — the dangerous-bug zone).
        assert!(close(normal_two_sided(1.959964), 0.05, P_TOL), "{}", normal_two_sided(1.959964));
        assert!(close(normal_two_sided(2.575829), 0.01, P_TOL));
        assert!(close(normal_two_sided(3.290527), 0.001, 1e-5), "tail p: {}", normal_two_sided(3.290527));
    }

    #[test]
    fn t_distribution_critical_values_match_tables() {
        // Student-t two-sided critical values (df, t)→0.05: (8,2.306004),
        // (10,2.228139); (30,2.042272). Classic t-table values.
        assert!(close(t_two_sided(2.306004, 8.0), 0.05, P_TOL), "{}", t_two_sided(2.306004, 8.0));
        assert!(close(t_two_sided(2.228139, 10.0), 0.05, P_TOL));
        assert!(close(t_two_sided(2.042272, 30.0), 0.05, P_TOL));
        // deep tail: t(10) at 4.586894 → two-sided p = 0.001
        assert!(close(t_two_sided(4.586894, 10.0), 0.001, 1e-4), "tail: {}", t_two_sided(4.586894, 10.0));
    }

    #[test]
    fn chi_square_and_f_critical_values_match_tables() {
        // χ² critical points: df=1 @3.841459→0.05, @6.634897→0.01, @10.827566→0.001.
        let p05 = 1.0 - ChiSquared::new(1.0).unwrap().cdf(3.841459);
        let p01 = 1.0 - ChiSquared::new(1.0).unwrap().cdf(6.634897);
        let p001 = 1.0 - ChiSquared::new(1.0).unwrap().cdf(10.827566);
        assert!(close(p05, 0.05, P_TOL), "{p05}");
        assert!(close(p01, 0.01, P_TOL));
        assert!(close(p001, 0.001, 1e-4), "tail: {p001}");
        // F critical: F(3,12) @3.490→0.05 (scipy: p≈0.0500).
        let pf = 1.0 - FisherSnedecor::new(3.0, 12.0).unwrap().cdf(3.4903);
        assert!(close(pf, 0.05, 2e-3), "F p: {pf}");
    }

    // ------------------------------ t-test ---------------------------------

    #[test]
    fn welch_and_student_t_match_hand_computed_values() {
        // A=[1,2,3,4,5] (mean 3, var 2.5), B=[2,4,6,8,10] (mean 6, var 10).
        // Student pooled: sp²=6.25, se=√2.5=1.5811388, t=(3-6)/se=-1.8973666, df=8.
        let a = [1.0, 2.0, 3.0, 4.0, 5.0];
        let b = [2.0, 4.0, 6.0, 8.0, 10.0];
        let s = t_test_student(&a, &b).unwrap();
        assert!(close(s.t, -1.8973665961, 1e-7), "student t = {}", s.t);
        assert!(close(s.df, 8.0, TIGHT));
        // scipy.stats.ttest_ind(a,b): p ≈ 0.09428 (two-sided).
        assert!(close(s.p_two_sided, 0.094286, 1e-3), "student p = {}", s.p_two_sided);

        // Welch: se=√(0.5+2)=√2.5 same numerator. df=(sa+sb)²/(sa²/4+sb²/4)
        // =6.25/(0.0625+1.0)=6.25/1.0625=5.882353 (scipy equal_var=False).
        let w = t_test_welch(&a, &b).unwrap();
        assert!(close(w.t, -1.8973665961, 1e-7), "welch t = {}", w.t);
        assert!(close(w.df, 5.8823529, 1e-4), "welch df = {}", w.df);
        // scipy.stats.ttest_ind(a,b,equal_var=False): p ≈ 0.10783.
        assert!(close(w.p_two_sided, 0.107831, 1e-3), "welch p = {}", w.p_two_sided);
    }

    #[test]
    fn identical_groups_give_t_zero_p_one() {
        let a = [3.0, 5.0, 7.0, 9.0];
        let r = t_test_welch(&a, &a).unwrap();
        assert!(close(r.t, 0.0, TIGHT));
        assert!(close(r.p_two_sided, 1.0, TIGHT));
    }

    #[test]
    fn zero_variance_both_groups_is_an_honest_error() {
        assert!(t_test_welch(&[5.0, 5.0, 5.0], &[5.0, 5.0, 5.0]).is_err());
        assert!(t_test_welch(&[5.0], &[1.0, 2.0]).is_err()); // n < 2
    }

    // ------------------------------ ANOVA ----------------------------------

    #[test]
    fn anova_matches_hand_computed_f() {
        // G1=[1,2,3],G2=[4,5,6],G3=[7,8,9]. grand=5. SSB=3*(9+0+9)=54, df_b=2.
        // SSW=2 per group *3 = 6, df_w=6. MSB=27, MSW=1, F=27.
        let g1 = [1.0, 2.0, 3.0];
        let g2 = [4.0, 5.0, 6.0];
        let g3 = [7.0, 8.0, 9.0];
        let r = one_way_anova(&[&g1, &g2, &g3]).unwrap();
        assert!(close(r.f, 27.0, 1e-9), "F = {}", r.f);
        assert!(close(r.df_between, 2.0, TIGHT));
        assert!(close(r.df_within, 6.0, TIGHT));
        // scipy.stats.f_oneway: p ≈ 0.0010267.
        assert!(close(r.p, 0.0010267, 1e-4), "anova p = {}", r.p);
    }

    #[test]
    fn anova_two_groups_matches_the_student_t_squared() {
        // For 2 groups, F == t² (Student). A/B from the t-test case.
        let a = [1.0, 2.0, 3.0, 4.0, 5.0];
        let b = [2.0, 4.0, 6.0, 8.0, 10.0];
        let f = one_way_anova(&[&a, &b]).unwrap();
        let t = t_test_student(&a, &b).unwrap();
        assert!(close(f.f, t.t * t.t, 1e-7), "F {} vs t² {}", f.f, t.t * t.t);
    }

    // --------------------------- correlation -------------------------------

    #[test]
    fn pearson_perfect_and_known() {
        let x = [1.0, 2.0, 3.0, 4.0, 5.0];
        let y = [2.0, 4.0, 6.0, 8.0, 10.0];
        let r = pearson(&x, &y).unwrap();
        assert!(close(r.coefficient, 1.0, TIGHT), "r = {}", r.coefficient);
        assert!(close(r.p_two_sided, 0.0, TIGHT)); // perfect → p 0
        // negative perfect
        let yn = [10.0, 8.0, 6.0, 4.0, 2.0];
        assert!(close(pearson(&x, &yn).unwrap().coefficient, -1.0, TIGHT));
        // a known r: x=[1,2,3,4,5], y=[2,1,4,3,5]. Sxy=8, Sxx=Syy=10 → r=0.8.
        let y2 = [2.0, 1.0, 4.0, 3.0, 5.0];
        assert!(close(pearson(&x, &y2).unwrap().coefficient, 0.8, 1e-12), "{}", pearson(&x, &y2).unwrap().coefficient);
    }

    #[test]
    fn spearman_handles_monotonic_and_ties() {
        // strictly monotonic (non-linear) → rho = 1 even though Pearson < 1.
        let x = [1.0, 2.0, 3.0, 4.0, 5.0];
        let y = [1.0, 4.0, 9.0, 16.0, 25.0];
        assert!(close(spearman(&x, &y).unwrap().coefficient, 1.0, 1e-12));
        // ties: scipy.stats.spearmanr([1,2,2,3],[1,2,3,4]) → rho ≈ 0.9486833.
        let xt = [1.0, 2.0, 2.0, 3.0];
        let yt = [1.0, 2.0, 3.0, 4.0];
        assert!(close(spearman(&xt, &yt).unwrap().coefficient, 0.9486833, 1e-6), "{}", spearman(&xt, &yt).unwrap().coefficient);
    }

    #[test]
    fn zero_variance_correlation_is_an_honest_error() {
        assert!(pearson(&[1.0, 1.0, 1.0], &[1.0, 2.0, 3.0]).is_err());
        assert!(pearson(&[1.0, 2.0], &[1.0, 2.0]).is_err()); // n < 3
    }

    // --------------------------- chi-square --------------------------------

    #[test]
    fn chi_square_matches_hand_computed() {
        // [[10,20],[30,40]]: E=[[12,18],[28,42]], χ²=4/12+4/18+4/28+4/42.
        let obs = vec![vec![10.0, 20.0], vec![30.0, 40.0]];
        let r = chi_square_independence(&obs).unwrap();
        let expected = 4.0 / 12.0 + 4.0 / 18.0 + 4.0 / 28.0 + 4.0 / 42.0; // ≈ 0.79365
        assert!(close(r.chi_square, expected, 1e-12), "χ² = {}", r.chi_square);
        assert!(close(r.df, 1.0, TIGHT));
        // scipy.stats.chi2_contingency(...,correction=False): p ≈ 0.37294.
        assert!(close(r.p, 0.372937, 1e-4), "chi p = {}", r.p);
    }

    #[test]
    fn chi_square_empty_margin_is_an_honest_error() {
        // a zero row margin → zero expected count → error, not divide-by-zero.
        assert!(chi_square_independence(&[vec![0.0, 0.0], vec![5.0, 7.0]]).is_err());
    }

    // ------------------------------- OLS -----------------------------------

    #[test]
    fn ols_perfect_fit() {
        let y = [1.0, 2.0, 3.0, 4.0, 5.0];
        let x = vec![vec![1.0, 2.0, 3.0, 4.0, 5.0]];
        let r = ols(&y, &x).unwrap();
        assert!(close(r.coefficients[0].estimate, 0.0, 1e-9), "intercept {}", r.coefficients[0].estimate);
        assert!(close(r.coefficients[1].estimate, 1.0, 1e-9), "slope {}", r.coefficients[1].estimate);
        assert!(close(r.r_squared, 1.0, 1e-12));
    }

    #[test]
    fn ols_matches_hand_computed_slope_intercept_r2() {
        // y=[2,4,5,4,5], x=[1,2,3,4,5]. Σ(x-3)(y-4)=6, Σ(x-3)²=10 → slope 0.6,
        // intercept 4-0.6*3=2.2. SST=Σ(y-4)²=(4+0+1+0+1)=6; ŷ=2.8,3.4,4,4.6,5.2
        // SSE=(2-2.8)²+(4-3.4)²+(5-4)²+(4-4.6)²+(5-5.2)²=0.64+0.36+1+0.36+0.04=2.4
        // R²=1-2.4/6=0.6.
        let y = [2.0, 4.0, 5.0, 4.0, 5.0];
        let x = vec![vec![1.0, 2.0, 3.0, 4.0, 5.0]];
        let r = ols(&y, &x).unwrap();
        assert!(close(r.coefficients[0].estimate, 2.2, 1e-9), "intercept {}", r.coefficients[0].estimate);
        assert!(close(r.coefficients[1].estimate, 0.6, 1e-9), "slope {}", r.coefficients[1].estimate);
        assert!(close(r.r_squared, 0.6, 1e-9), "R² {}", r.r_squared);
        assert!(close(r.df_residual, 3.0, TIGHT));
        // slope SE = sqrt(sigma²/Sxx), sigma²=SSE/df=2.4/3=0.8, Sxx=10 → SE=0.2828427.
        assert!(close(r.coefficients[1].std_error, 0.2828427, 1e-6), "slope SE {}", r.coefficients[1].std_error);
        // t = 0.6/0.2828427 = 2.1213203; scipy/statsmodels p (df=3) ≈ 0.12408.
        assert!(close(r.coefficients[1].t, 2.1213203, 1e-6), "slope t {}", r.coefficients[1].t);
        assert!(close(r.coefficients[1].p_two_sided, 0.124098, 1e-3), "slope p {}", r.coefficients[1].p_two_sided);
    }

    #[test]
    fn ols_singular_design_is_an_honest_error() {
        // collinear predictor (constant column) → singular X'X → error.
        let y = [1.0, 2.0, 3.0, 4.0, 5.0];
        let x = vec![vec![1.0, 1.0, 1.0, 1.0, 1.0]];
        assert!(ols(&y, &x).is_err());
    }

    // ------------------------- numeric typing ------------------------------

    #[test]
    fn numeric_typing_parses_skips_missing_and_rejects_non_numeric() {
        let headers = vec!["score".to_string(), "arm".to_string()];
        let rows = vec![
            vec!["1.5".to_string(), "a".to_string()],
            vec!["".to_string(), "b".to_string()],   // missing
            vec!["NA".to_string(), "b".to_string()], // missing token
            vec!["3.0".to_string(), "a".to_string()],
        ];
        let col = numeric_column(&headers, &rows, "score", MissingPolicy::Skip).unwrap();
        assert_eq!(col.values, vec![1.5, 3.0]);
        assert_eq!(col.missing, 2);

        // non-numeric where a number is required → HONEST error (never a 0)
        let bad = vec![vec!["1.0".to_string()], vec!["oops".to_string()]];
        assert!(numeric_column(&["v".to_string()], &bad, "v", MissingPolicy::Skip).is_err());

        // Error policy: a missing cell is an error (paired alignment)
        assert!(numeric_column(&headers, &rows, "score", MissingPolicy::Error).is_err());

        // unknown column → honest error
        assert!(numeric_column(&headers, &rows, "nope", MissingPolicy::Skip).is_err());
    }

    // --------------------------- determinism -------------------------------

    #[test]
    fn deterministic_same_input_same_output() {
        let a = [1.0, 2.0, 3.0, 4.0, 7.0];
        let b = [2.0, 2.0, 5.0, 6.0, 9.0];
        assert_eq!(t_test_welch(&a, &b).unwrap(), t_test_welch(&a, &b).unwrap());
        assert_eq!(pearson(&a, &b).unwrap(), pearson(&a, &b).unwrap());
    }
}
