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

/// The shared missing-value tokens. `pub` since Phase 2, Step 2: Level 1
/// (`stats_l1::dataset`) reads cells by the same list rather than a copy.
pub fn is_missing(cell: &str) -> bool {
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
// Range: a computation that overflowed or underflowed is refused
// ---------------------------------------------------------------------------

/// The existing verifier's message when a computation overflowed. Plain, and
/// exact: [`RangeFault::of`] recognises it by equality.
pub const OVERFLOW_MESSAGE: &str =
    "the values are too large for this computation: an intermediate result overflowed, so nothing was computed";

/// The existing verifier's message when a computation underflowed.
pub const UNDERFLOW_MESSAGE: &str =
    "the values are too small for this computation: a variance or sum of squares underflowed to zero although the values differ, so nothing was computed";

/// Why a computation was refused because a double could not hold one of its
/// numbers. Added in Phase 2, Step 3 (fix): every function here refuses with
/// one of these, as a [`GaplyError::Validation`] carrying the matching
/// message, instead of computing through it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RangeFault {
    /// A sum, product, square, quotient or square root was infinite or NaN.
    Overflow,
    /// A sum of squares (a variance, a model or residual sum of squares, a
    /// Householder vector's squared norm, a cross-product's diagonal) or an
    /// expected count came out zero or subnormal although a term in it was
    /// not zero.
    Underflow,
}

impl RangeFault {
    /// The fault an error from this module carries, if it is one.
    pub fn of(e: &GaplyError) -> Option<RangeFault> {
        match e {
            GaplyError::Validation(m) if m == OVERFLOW_MESSAGE => Some(RangeFault::Overflow),
            GaplyError::Validation(m) if m == UNDERFLOW_MESSAGE => Some(RangeFault::Underflow),
            _ => None,
        }
    }

    fn error(self) -> GaplyError {
        GaplyError::Validation(
            match self {
                RangeFault::Overflow => OVERFLOW_MESSAGE,
                RangeFault::Underflow => UNDERFLOW_MESSAGE,
            }
            .into(),
        )
    }
}

// ---------------------------------------------------------------------------
// Small statistics helpers (deterministic arithmetic)
// ---------------------------------------------------------------------------

/// The mean as R's `mean` computes it: the sum over n, then refined by the
/// mean of the residuals from it (a second pass). A plain `sum / n` carries
/// about one unit in the last place of the SUM: on values near 1e9 with an
/// SD of 10 that is ~1e-7 absolute, which the centred variance and t then
/// inherit, and Level 1 missed R by up to 1.5e-6 (evaluation fixes, part C,
/// m-10). R on this machine (aarch64) accumulates in double, as here.
/// The descriptive summary (`stats_desc`) calls it too and has no mean of
/// its own (Research OS, Phase 2A).
pub(crate) fn mean(xs: &[f64]) -> f64 {
    let n = xs.len() as f64;
    let m = xs.iter().sum::<f64>() / n;
    if !m.is_finite() {
        return m;
    }
    let residual: f64 = xs.iter().map(|x| x - m).sum();
    m + residual / n
}

/// `v`, or an overflow refusal when it is infinite or NaN.
fn finite(v: f64) -> Result<f64, GaplyError> {
    if v.is_finite() {
        Ok(v)
    } else {
        Err(RangeFault::Overflow.error())
    }
}

/// The mean, refused when the sum overflowed.
fn checked_mean(xs: &[f64]) -> Result<f64, GaplyError> {
    finite(mean(xs))
}

/// `sum` of squares, refused as an underflow when it is zero or subnormal
/// although a term in it was not zero: the squares fell below the smallest
/// normal double and the sum lost them. A sum that is zero because every
/// term is zero passes; that is the caller's own zero-variance case.
fn underflow_unless_normal(sum: f64, any_term_nonzero: bool) -> Result<f64, GaplyError> {
    if any_term_nonzero && sum < f64::MIN_POSITIVE {
        Err(RangeFault::Underflow.error())
    } else {
        Ok(sum)
    }
}

/// Σ t², in order, with every term and the sum checked: overflow when one
/// is not finite, underflow as [`underflow_unless_normal`].
fn sum_of_squares(terms: impl IntoIterator<Item = f64>) -> Result<f64, GaplyError> {
    let mut sum = 0.0;
    let mut any = false;
    for t in terms {
        finite(t)?;
        any |= t != 0.0;
        sum += t.powi(2);
    }
    underflow_unless_normal(finite(sum)?, any)
}

/// Sample variance (n-1 denominator), with the mean and every squared
/// deviation range-checked. Also the descriptive summary's variance
/// (`stats_desc`, Research OS Phase 2A).
pub(crate) fn sample_variance(xs: &[f64]) -> Result<f64, GaplyError> {
    let n = xs.len();
    if n < 2 {
        return Ok(0.0);
    }
    let m = checked_mean(xs)?;
    Ok(sum_of_squares(xs.iter().map(|x| x - m))? / (n - 1) as f64)
}

/// Two-sided p from a |t| and df, guarding the perfect-fit (|t| = ∞) case.
fn t_two_sided(t_abs: f64, df: f64) -> f64 {
    if !t_abs.is_finite() {
        return 0.0; // perfect separation / correlation
    }
    // Not `expect`: Welch's df is `inf / inf = NaN` when a group's variance
    // overflows (cells near 1e200), and that used to panic. A df that defines
    // no distribution gives a NaN p. Found by Level 1's non-finite refusal
    // test (Phase 2, Step 3).
    let Ok(dist) = StudentsT::new(0.0, 1.0, df) else {
        return f64::NAN;
    };
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
    /// "welch" (unequal variances, default) or "student" (pooled); "paired"
    /// and "one_sample" for those tests.
    pub method: &'static str,
    /// The standard error t divides by: sqrt(va/na + vb/nb) for Welch,
    /// sqrt(sp²(1/na + 1/nb)) for Student, sd(a − b)/sqrt(n) paired. Exposed
    /// for the t-test as an analysis (`stats_ttest`, Research OS Phase 4B-1),
    /// so its standard error and interval come from here and nowhere else.
    pub se: f64,
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
    let (va, vb) = (sample_variance(a)?, sample_variance(b)?);
    let sa = va / na;
    let sb = vb / nb;
    let se = finite((sa + sb).sqrt())?;
    if se == 0.0 {
        return Err(GaplyError::Validation(
            "both groups have zero variance — the t-statistic is undefined".into(),
        ));
    }
    let (ma, mb) = (checked_mean(a)?, checked_mean(b)?);
    let t = finite((ma - mb) / se)?;
    // Welch–Satterthwaite degrees of freedom. Its squares of the variances
    // underflow at cells near 1e-80, long before the variances do.
    let numerator = underflow_unless_normal(finite((sa + sb).powi(2))?, true)?;
    let denominator = underflow_unless_normal(finite(sa.powi(2) / (na - 1.0) + sb.powi(2) / (nb - 1.0))?, true)?;
    let df = finite(numerator / denominator)?;
    Ok(TTestResult { t, df, p_two_sided: t_two_sided(t.abs(), df), mean_a: ma, mean_b: mb, method: "welch", se })
}

/// Student's pooled-variance t-test (assumes equal variances).
pub fn t_test_student(a: &[f64], b: &[f64]) -> Result<TTestResult, GaplyError> {
    require_group("group A", a)?;
    require_group("group B", b)?;
    let (na, nb) = (a.len() as f64, b.len() as f64);
    let (va, vb) = (sample_variance(a)?, sample_variance(b)?);
    let df = na + nb - 2.0;
    let sp2 = finite(((na - 1.0) * va + (nb - 1.0) * vb) / df)?;
    let se = finite((sp2 * (1.0 / na + 1.0 / nb)).sqrt())?;
    if se == 0.0 {
        return Err(GaplyError::Validation(
            "pooled variance is zero — the t-statistic is undefined".into(),
        ));
    }
    let (ma, mb) = (checked_mean(a)?, checked_mean(b)?);
    let t = finite((ma - mb) / se)?;
    Ok(TTestResult { t, df, p_two_sided: t_two_sided(t.abs(), df), mean_a: ma, mean_b: mb, method: "student", se })
}

/// Paired-samples t-test: `a[i]` and `b[i]` are one subject measured twice.
/// t is the mean of the differences `a - b` over its standard error, on
/// `n - 1` df, as R's `t.test(a, b, paired = TRUE)`. Added in Phase 2, Step 3
/// for Level 1; the existing verifier does not call it.
pub fn t_test_paired(a: &[f64], b: &[f64]) -> Result<TTestResult, GaplyError> {
    if a.len() != b.len() {
        return Err(GaplyError::Validation("a paired t-test needs equal-length columns".into()));
    }
    require_group("pairs", a)?;
    let d: Vec<f64> = a.iter().zip(b).map(|(x, y)| x - y).collect();
    // A difference of two finite cells can itself overflow; the mean of the
    // differences is then not finite. The one-sample t of the differences
    // against 0 (Research OS, Phase 8D-1, decision 254): `md − 0.0` is `md`,
    // bit for bit, so t is `md / se` as before.
    let (t, df, se) = one_sample_t(
        &d,
        0.0,
        "the differences have zero variance — the t-statistic is undefined",
    )?;
    Ok(TTestResult {
        t,
        df,
        p_two_sided: t_two_sided(t.abs(), df),
        mean_a: checked_mean(a)?,
        mean_b: checked_mean(b)?,
        method: "paired",
        se,
    })
}

/// One-sample t-test of `x` against `mu`, as R's `t.test(x, mu = mu)`:
/// t = (mean(x) − mu) / sqrt(var(x) / n) on n − 1 df (Research OS, Phase
/// 8D-1, decision 254). `mean_a` is the mean of `x` and `mean_b` is `mu`.
/// `mu` must be finite.
pub fn t_test_one_sample(x: &[f64], mu: f64) -> Result<TTestResult, GaplyError> {
    require_group("values", x)?;
    finite(mu)?;
    let (t, df, se) = one_sample_t(
        x,
        mu,
        "the values have zero variance — the t-statistic is undefined",
    )?;
    Ok(TTestResult {
        t,
        df,
        p_two_sided: t_two_sided(t.abs(), df),
        mean_a: checked_mean(x)?,
        mean_b: mu,
        method: "one_sample",
        se,
    })
}

/// THE one-sample t arithmetic, shared by [`t_test_one_sample`] and
/// [`t_test_paired`] (decision 254): the mean, the standard error
/// sqrt(var / n) with its range checks, a standard error of exactly 0 refused
/// with `zero_variance`, then t = (mean − mu) / se and df = n − 1. In that
/// order, so `t_test_paired`'s refusals are what they were. `(t, df, se)`.
fn one_sample_t(x: &[f64], mu: f64, zero_variance: &str) -> Result<(f64, f64, f64), GaplyError> {
    let n = x.len() as f64;
    let m = checked_mean(x)?;
    let se = finite((sample_variance(x)? / n).sqrt())?;
    if se == 0.0 {
        return Err(GaplyError::Validation(zero_variance.into()));
    }
    let t = finite((m - mu) / se)?;
    Ok((t, n - 1.0, se))
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
    /// Σ nᵢ (meanᵢ − grand mean)², the sum F's numerator divides by its df
    /// (Research OS, Phase 8G, decision 313: the one-way ANOVA on the user's
    /// data reads it for its table and its effect sizes).
    pub ss_between: f64,
    /// Σ over groups of Σ (x − meanᵢ)², the sum F's denominator divides by
    /// its df (decision 313).
    pub ss_within: f64,
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
    let grand = checked_mean(&all)?;
    let mut ss_between = 0.0;
    let mut ss_within = 0.0;
    let mut means_differ = false;
    for g in groups {
        if g.is_empty() {
            return Err(GaplyError::Validation("ANOVA groups must be non-empty".into()));
        }
        let gm = checked_mean(g)?;
        let d = finite(gm - grand)?;
        means_differ |= d != 0.0;
        ss_between += g.len() as f64 * d.powi(2);
        // Each group's own sum of squares is range-checked: a group whose
        // values differ only below the smallest normal square is refused.
        ss_within += sum_of_squares(g.iter().map(|x| x - gm))?;
    }
    let ss_between = underflow_unless_normal(finite(ss_between)?, means_differ)?;
    let ss_within = finite(ss_within)?;
    let df_b = (k - 1) as f64;
    let df_w = (n_total - k) as f64;
    let ms_within = ss_within / df_w;
    if ms_within == 0.0 {
        return Err(GaplyError::Validation(
            "within-group variance is zero — the F-statistic is undefined".into(),
        ));
    }
    // A NaN F used to reach statrs' beta function and panic there.
    let f = finite((ss_between / df_b) / ms_within)?;
    let p = (1.0 - FisherSnedecor::new(df_b, df_w).expect("df > 0").cdf(f)).clamp(0.0, 1.0);
    Ok(AnovaResult {
        f,
        df_between: df_b,
        df_within: df_w,
        p,
        ss_between,
        ss_within,
    })
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
    let (mx, my) = (checked_mean(x)?, checked_mean(y)?);
    let mut sxy = 0.0;
    let mut sxx = 0.0;
    let mut syy = 0.0;
    let (mut x_varies, mut y_varies) = (false, false);
    for (a, b) in x.iter().zip(y) {
        let (dx, dy) = (finite(a - mx)?, finite(b - my)?);
        x_varies |= dx != 0.0;
        y_varies |= dy != 0.0;
        sxy += dx * dy;
        sxx += dx.powi(2);
        syy += dy.powi(2);
    }
    let sxx = underflow_unless_normal(finite(sxx)?, x_varies)?;
    let syy = underflow_unless_normal(finite(syy)?, y_varies)?;
    let sxy = finite(sxy)?;
    if sxx == 0.0 || syy == 0.0 {
        return Err(GaplyError::Validation(
            "a column has zero variance — correlation is undefined".into(),
        ));
    }
    finite(sxy / (sxx.sqrt() * syy.sqrt()))
}

fn r_to_result(r: f64, n: usize, method: &'static str) -> Result<CorrelationResult, GaplyError> {
    let df = (n - 2) as f64;
    // t = r * sqrt(df / (1 - r^2)); |r| = 1 → t = ∞ → p = 0. That infinity
    // is the exact boundary, not an overflow, and is kept; any other
    // non-finite t is refused.
    let t = if (1.0 - r * r).abs() < f64::EPSILON {
        f64::INFINITY * r.signum()
    } else {
        finite(r * (df / (1.0 - r * r)).sqrt())?
    };
    Ok(CorrelationResult { coefficient: r, df, statistic: t, p_two_sided: t_two_sided(t.abs(), df), method })
}

/// Pearson product-moment correlation with its t-test p-value.
pub fn pearson(x: &[f64], y: &[f64]) -> Result<CorrelationResult, GaplyError> {
    let r = pearson_r(x, y)?;
    r_to_result(r, x.len(), "pearson")
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
    r_to_result(rho, x.len(), "spearman")
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
    chi_square_with(observed, |o, e| (o - e).powi(2) / e)
}

/// R's `chisq.test(correct = TRUE)` on a 2×2 table, df 1: each cell's
/// `|O - E|` is reduced by `min(0.5, |O - E|)`, so a cell never goes below
/// zero. Added in Phase 2, Step 3 for Level 1; the existing verifier does not
/// call it. Any other shape is an honest error: the correction is defined for
/// a 2×2 table only.
pub fn chi_square_2x2_yates(observed: &[Vec<f64>]) -> Result<ChiSquareResult, GaplyError> {
    if observed.len() != 2 || observed.iter().any(|row| row.len() != 2) {
        return Err(GaplyError::Validation("a continuity correction is defined for a 2×2 table only".into()));
    }
    chi_square_with(observed, |o, e| {
        let d = (o - e).abs();
        (d - d.min(0.5)).powi(2) / e
    })
}

/// The table checks and the sum over cells, with the per-cell term supplied.
fn chi_square_with(observed: &[Vec<f64>], term: impl Fn(f64, f64) -> f64) -> Result<ChiSquareResult, GaplyError> {
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
    for &m in row_sums.iter().chain(&col_sums) {
        finite(m)?;
    }
    let total: f64 = finite(row_sums.iter().sum())?;
    if total == 0.0 {
        return Err(GaplyError::Validation("chi-square table is all zeros".into()));
    }
    let mut chi2 = 0.0;
    for (i, row) in observed.iter().enumerate() {
        for (j, &o) in row.iter().enumerate() {
            let product = finite(row_sums[i] * col_sums[j])?;
            let e = product / total;
            // Both margins non-zero and the expected count not a normal
            // double: the product or the quotient underflowed.
            if row_sums[i] > 0.0 && col_sums[j] > 0.0 && (product < f64::MIN_POSITIVE || e < f64::MIN_POSITIVE) {
                return Err(RangeFault::Underflow.error());
            }
            if e == 0.0 {
                return Err(GaplyError::Validation(
                    "chi-square has a zero expected count (an empty row or column margin)".into(),
                ));
            }
            chi2 += finite(term(o, e))?;
        }
    }
    let chi2 = finite(chi2)?;
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
    /// The overall F: `(SS_model / k) / (SS_residual / df_residual)`, with
    /// `SS_model` summed from the fitted values as R's `summary.lm` does
    /// (not `SS_total - SS_residual`). Added in Phase 2, Step 3.
    pub f_statistic: f64,
    /// `k`, the number of predictors: the overall F's first df.
    pub df_model: f64,
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
    // X'X and X'y overflow at cells near 1e200. A diagonal entry of X'X is a
    // column's sum of squares and underflows at cells near 1e-200.
    for a in 0..p {
        finite(xty[a])?;
        for b in 0..p {
            finite(xtx[a][b])?;
        }
        underflow_unless_normal(xtx[a][a], x.iter().any(|row| row[a] != 0.0))?;
    }
    let xtx_inv = invert(&xtx).ok_or_else(|| {
        GaplyError::Validation(
            "the design matrix is singular (collinear predictors or no variation) — regression is undefined".into(),
        )
    })?;
    for row in &xtx_inv {
        for &v in row {
            finite(v)?;
        }
    }
    // beta = (X'X)^-1 X'y
    let beta: Vec<f64> = (0..p).map(|a| (0..p).map(|b| xtx_inv[a][b] * xty[b]).sum()).collect();
    for &b in &beta {
        finite(b)?;
    }
    // residuals, SSE, SST
    let ybar = checked_mean(y)?;
    let yhat: Vec<f64> = (0..n).map(|i| (0..p).map(|a| beta[a] * x[i][a]).sum()).collect();
    let sse = sum_of_squares((0..n).map(|i| y[i] - yhat[i]))?;
    let sst = sum_of_squares(y.iter().map(|v| v - ybar))?;
    let ssm = sum_of_squares(yhat.iter().map(|v| v - ybar))?;
    let df_res = (n - p) as f64;
    let sigma2 = sse / df_res;
    let coefficients = (0..p)
        .map(|a| {
            let se = finite((sigma2 * xtx_inv[a][a]).sqrt())?;
            // A zero SE is a perfect fit (every residual exactly zero): the
            // infinite t is that boundary, not an overflow, and is kept.
            let t = if se == 0.0 { f64::INFINITY } else { finite(beta[a] / se)? };
            let name = if a == 0 { "(intercept)".to_string() } else { format!("x{a}") };
            Ok(Coefficient { name, estimate: beta[a], std_error: se, t, p_two_sided: t_two_sided(t.abs(), df_res) })
        })
        .collect::<Result<Vec<Coefficient>, GaplyError>>()?;
    let r_squared = if sst == 0.0 { 0.0 } else { 1.0 - sse / sst };
    let adj = 1.0 - (1.0 - r_squared) * (n as f64 - 1.0) / df_res;
    let df_model = k as f64;
    // As above: only a perfect fit's F may be infinite.
    let f_statistic = (ssm / df_model) / sigma2;
    if sigma2 != 0.0 {
        finite(f_statistic)?;
    }
    Ok(RegressionResult { coefficients, r_squared, adj_r_squared: adj, df_residual: df_res, f_statistic, df_model })
}

/// R's rank tolerance for `lm`: `qr(tol = 1e-07)`.
pub const QR_RANK_TOLERANCE: f64 = 1e-7;

/// OLS with an intercept by Householder QR, the way R's `lm` fits it. Added in
/// Phase 2, Step 3 for Level 1; [`ols`] and the existing verifier are
/// unchanged and do not call it.
///
/// [`ols`] solves the normal equations, whose rounding error grows with the
/// SQUARE of the design's condition number. QR works on the design itself,
/// so its error grows with the condition number once.
///
/// The singular-design test is LINPACK `dqrdc2`'s, which `lm` calls with
/// `tol = 1e-7`: a column is negligible when the norm left in it, after the
/// reflections of the columns before it, is below `tol` times its OWN original
/// norm. That makes the test scale-free: a predictor measured in millions and
/// one measured in hundredths are each judged against themselves. R moves a
/// negligible column to the end and reports its coefficient as `NA`; this
/// refuses instead, so a design R would call rank-deficient is an error here.
///
/// The output follows `summary.lm`: residuals are `Q` applied to the effects
/// beyond the rank, fitted values are `y` minus residuals, `SS_model` is summed
/// from the fitted values about their mean, standard errors come from the
/// diagonal of `R^-1 R^-T` times the residual variance.
pub fn ols_qr(y: &[f64], predictors: &[Vec<f64>]) -> Result<RegressionResult, GaplyError> {
    let n = y.len();
    let k = predictors.len();
    let p = k + 1;
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
    // The design by columns, column 0 the intercept. Reduced in place to R
    // (upper triangle) with the Householder vectors kept separately.
    let mut a: Vec<Vec<f64>> = Vec::with_capacity(p);
    a.push(vec![1.0; n]);
    a.extend(predictors.iter().cloned());
    let original_norm: Vec<f64> = a.iter().map(|c| norm2(c)).collect();
    let mut effects = y.to_vec();
    let mut reflectors: Vec<Vec<f64>> = Vec::with_capacity(p);
    for j in 0..p {
        let remaining = norm2(&a[j][j..]);
        // dqrdc2 treats an all-zero column's original norm as 1.
        let reference = if original_norm[j] == 0.0 { 1.0 } else { original_norm[j] };
        if !(remaining >= QR_RANK_TOLERANCE * reference) {
            return Err(GaplyError::Validation(
                "the design matrix is rank-deficient at R's tolerance (collinear predictors or no variation) — regression is undefined".into(),
            ));
        }
        // v = x - alpha e1, alpha = -sign(x0) |x|: no cancellation in v0.
        let alpha = if a[j][j] >= 0.0 { -remaining } else { remaining };
        let mut v = a[j][j..].to_vec();
        v[0] -= alpha;
        // The reflector's squared norm is a sum of squares: it overflows at
        // cells near 1e200 and underflows at cells near 1e-200, although
        // `norm2` (scaled) does neither.
        let vv = sum_of_squares(v.iter().copied())?;
        let reflect = |col: &mut [f64]| -> Result<(), GaplyError> {
            let s = finite(v.iter().zip(col.iter()).map(|(a, b)| a * b).sum::<f64>() * 2.0 / vv)?;
            for (c, vi) in col.iter_mut().zip(&v) {
                *c = finite(*c - s * vi)?;
            }
            Ok(())
        };
        for col in a.iter_mut().skip(j + 1) {
            reflect(&mut col[j..])?;
        }
        reflect(&mut effects[j..])?;
        a[j][j] = alpha;
        for x in a[j][j + 1..].iter_mut() {
            *x = 0.0;
        }
        reflectors.push(v);
    }
    // R^-1 by back substitution (upper triangular, column j of `a` holds
    // R[0..=j][j]).
    let r = |i: usize, j: usize| a[j][i];
    let mut r_inv = vec![vec![0.0; p]; p];
    for col in 0..p {
        for i in (0..=col).rev() {
            let mut s = if i == col { 1.0 } else { 0.0 };
            for m in (i + 1)..=col {
                s -= r(i, m) * r_inv[m][col];
            }
            r_inv[i][col] = s / r(i, i);
        }
    }
    for row in &r_inv {
        for &v in row {
            finite(v)?;
        }
    }
    let beta: Vec<f64> = (0..p).map(|i| (i..p).map(|m| r_inv[i][m] * effects[m]).sum()).collect();
    for &b in &beta {
        finite(b)?;
    }
    // Residuals: Q (0, ..., 0, effects[p..]), applying the reflectors in reverse.
    let mut resid = vec![0.0; n];
    resid[p..].copy_from_slice(&effects[p..]);
    for (j, v) in reflectors.iter().enumerate().rev() {
        let vv = sum_of_squares(v.iter().copied())?;
        let s = finite(v.iter().zip(&resid[j..]).map(|(a, b)| a * b).sum::<f64>() * 2.0 / vv)?;
        for (c, vi) in resid[j..].iter_mut().zip(v) {
            *c -= s * vi;
        }
    }
    let fitted: Vec<f64> = y.iter().zip(&resid).map(|(y, e)| y - e).collect();
    let rss = sum_of_squares(resid.iter().copied())?;
    let fbar = checked_mean(&fitted)?;
    let mss = sum_of_squares(fitted.iter().map(|f| f - fbar))?;
    let df_res = (n - p) as f64;
    let sigma2 = rss / df_res;
    let coefficients = (0..p)
        .map(|i| {
            let unscaled = finite(r_inv[i][i..].iter().map(|x| x * x).sum::<f64>())?;
            let se = finite((sigma2 * unscaled).sqrt())?;
            // A zero SE is a perfect fit (every residual exactly zero): the
            // infinite t is that boundary, not an overflow, and is kept.
            let t = if se == 0.0 { f64::INFINITY } else { finite(beta[i] / se)? };
            let name = if i == 0 { "(intercept)".to_string() } else { format!("x{i}") };
            Ok(Coefficient { name, estimate: beta[i], std_error: se, t, p_two_sided: t_two_sided(t.abs(), df_res) })
        })
        .collect::<Result<Vec<Coefficient>, GaplyError>>()?;
    let r_squared = if mss + rss == 0.0 { 0.0 } else { mss / (mss + rss) };
    let adj = 1.0 - (1.0 - r_squared) * (n as f64 - 1.0) / df_res;
    let df_model = k as f64;
    // As above: only a perfect fit's F may be infinite.
    let f_statistic = (mss / df_model) / sigma2;
    if sigma2 != 0.0 {
        finite(f_statistic)?;
    }
    Ok(RegressionResult { coefficients, r_squared, adj_r_squared: adj, df_residual: df_res, f_statistic, df_model })
}

/// The Euclidean norm, scaled so a column of large values cannot overflow
/// its sum of squares.
fn norm2(x: &[f64]) -> f64 {
    let scale = x.iter().fold(0.0f64, |m, v| m.max(v.abs()));
    if scale == 0.0 || !scale.is_finite() {
        return scale;
    }
    scale * x.iter().map(|v| (v / scale).powi(2)).sum::<f64>().sqrt()
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
    fn an_overflowing_welch_df_is_refused_not_a_panic() {
        // Welch's df was inf / inf here, and `StudentsT::new` panicked on it;
        // a NaN p replaced the panic, and a refusal replaced the NaN.
        let e = t_test_welch(&[1e200, -1e200], &[3e200, -2e200]).unwrap_err();
        assert_eq!(RangeFault::of(&e), Some(RangeFault::Overflow), "{e}");
    }

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

    /// Research OS, Phase 8G, decision 313: the two sums of squares the
    /// one-way ANOVA on the user's data reads, from the values this function
    /// already computes, and nothing else changed.
    #[test]
    fn anova_reports_the_sums_of_squares_it_divides() {
        let g1 = [1.0, 2.0, 3.0];
        let g2 = [4.0, 5.0, 6.0];
        let g3 = [7.0, 8.0, 9.0];
        let r = one_way_anova(&[&g1, &g2, &g3]).unwrap();
        assert_eq!(r.ss_between, 54.0);
        assert_eq!(r.ss_within, 6.0);
        assert_eq!(r.f, (r.ss_between / r.df_between) / (r.ss_within / r.df_within));
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

    #[test]
    fn ols_qr_matches_the_hand_computed_fit() {
        // The same data as the hand-computed test above.
        let y = [2.0, 4.0, 5.0, 4.0, 5.0];
        let x = vec![vec![1.0, 2.0, 3.0, 4.0, 5.0]];
        let r = ols_qr(&y, &x).unwrap();
        assert!(close(r.coefficients[0].estimate, 2.2, 1e-12), "intercept {}", r.coefficients[0].estimate);
        assert!(close(r.coefficients[1].estimate, 0.6, 1e-12), "slope {}", r.coefficients[1].estimate);
        assert!(close(r.r_squared, 0.6, 1e-12), "R² {}", r.r_squared);
        assert!(close(r.coefficients[1].std_error, 0.08f64.sqrt(), 1e-12), "slope SE {}", r.coefficients[1].std_error);
        // F = t² for one predictor: 4.5.
        assert!(close(r.f_statistic, 4.5, 1e-12), "F {}", r.f_statistic);
        assert_eq!((r.df_model, r.df_residual), (1.0, 3.0));
    }

    #[test]
    fn ols_qr_refuses_a_rank_deficient_design() {
        let y = [1.0, 2.0, 3.0, 4.0, 6.0];
        // Constant: a multiple of the intercept.
        assert!(ols_qr(&y, &[vec![3.0; 5]]).is_err());
        // x2 = 2 x1.
        let x1 = vec![1.0, 2.0, 4.0, 3.0, 5.0];
        let x2: Vec<f64> = x1.iter().map(|v| 2.0 * v).collect();
        assert!(ols_qr(&y, &[x1, x2]).is_err());
        // All zeros.
        assert!(ols_qr(&y, &[vec![0.0; 5]]).is_err());
    }

    /// The tolerance is per column, as `dqrdc2`'s is. A predictor in the
    /// hundredths beside one in the millions is judged against its own norm;
    /// judged against the largest diagonal of R it would read as singular.
    #[test]
    fn ols_qr_tolerance_is_relative_to_each_columns_own_norm() {
        let big = vec![1.02e6, 0.97e6, 1.05e6, 0.99e6, 1.01e6, 0.96e6, 1.04e6];
        let small = vec![0.012, 0.009, 0.011, 0.008, 0.013, 0.010, 0.007];
        let y = [3.1, 2.4, 3.6, 2.2, 3.3, 2.0, 3.4];
        let r = ols_qr(&y, &[big, small]).expect("a full-rank design is fitted");
        assert!(r.f_statistic.is_finite() && r.coefficients.iter().all(|c| c.t.is_finite()));
    }

    // ------------------- range: overflow and underflow ---------------------
    //
    // Cells near ±1e200 overflow a square; cells near ±1e-200 underflow one.
    // Every function refuses both, with the fault typed. Written before the
    // checks (Phase 2, Step 3 fix) and run red first.

    const OVER: f64 = 1e200;
    const UNDER: f64 = 1e-200;
    const A: [f64; 3] = [1.0, -1.0, 2.0];
    const B: [f64; 3] = [-3.0, 1.5, -2.0];
    const C3: [f64; 3] = [0.5, 2.5, -1.5];
    const X: [f64; 5] = [1.0, -1.0, 2.0, -2.0, 0.5];
    const Y: [f64; 5] = [2.0, -1.0, 1.0, -3.0, 0.0];

    fn k(xs: &[f64], k: f64) -> Vec<f64> {
        xs.iter().map(|v| v * k).collect()
    }

    /// What a function returned, in words a failure message can show.
    fn fault<T: std::fmt::Debug>(r: Result<T, GaplyError>) -> String {
        match r {
            Ok(v) => format!("computed {v:?}"),
            Err(e) => match RangeFault::of(&e) {
                Some(RangeFault::Overflow) => "overflow".into(),
                Some(RangeFault::Underflow) => "underflow".into(),
                None => format!("another error: {e}"),
            },
        }
    }

    fn refuses_both(name: &str, run: impl Fn(f64) -> String) {
        assert_eq!(run(OVER), "overflow", "{name}, cells near ±1e200");
        assert_eq!(run(UNDER), "underflow", "{name}, cells near ±1e-200");
    }

    #[test]
    fn welch_refuses_overflow_and_underflow() {
        refuses_both("welch", |s| fault(t_test_welch(&k(&A, s), &k(&B, s))));
    }

    /// Welch's df squares the variances: at cells near 1e-80 the variances
    /// are normal (1e-160) and their squares are not (1e-320). Added while
    /// writing the checks, and run red against them before this test passed.
    #[test]
    fn welch_df_refuses_the_underflow_of_its_squared_variances() {
        assert_eq!(fault(t_test_welch(&k(&A, 1e-80), &k(&B, 1e-80))), "underflow");
        // Student's t has no such square and computes at that scale.
        assert!(t_test_student(&k(&A, 1e-80), &k(&B, 1e-80)).is_ok());
    }

    #[test]
    fn student_refuses_overflow_and_underflow() {
        // The overflow used to compute: an infinite pooled variance, t = 0,
        // df = 4, p = 1.
        refuses_both("student", |s| fault(t_test_student(&k(&A, s), &k(&B, s))));
    }

    #[test]
    fn paired_refuses_overflow_and_underflow() {
        refuses_both("paired", |s| fault(t_test_paired(&k(&A, s), &k(&B, s))));
        // Differences that overflow themselves.
        assert_eq!(fault(t_test_paired(&[1.5e308, 1.6e308, 1.7e308], &[-1.5e308, -1.6e308, -1.7e308])), "overflow");
    }

    #[test]
    fn anova_refuses_overflow_and_underflow() {
        refuses_both("anova", |s| fault(one_way_anova(&[&k(&A, s), &k(&B, s), &k(&C3, s)])));
    }

    #[test]
    fn pearson_refuses_overflow_and_underflow() {
        refuses_both("pearson", |s| fault(pearson(&k(&X, s), &k(&Y, s))));
    }

    /// Unreachable: Spearman correlates ranks, which are 1..n at any scale.
    #[test]
    fn spearman_cannot_overflow_or_underflow_it_ranks() {
        let unit = spearman(&X, &Y).unwrap();
        assert_eq!(spearman(&k(&X, OVER), &k(&Y, OVER)).unwrap(), unit);
        assert_eq!(spearman(&k(&X, UNDER), &k(&Y, UNDER)).unwrap(), unit);
    }

    #[test]
    fn chi_square_refuses_overflow_and_underflow() {
        let t = |s: f64| vec![k(&[12.0, 5.0], s), k(&[7.0, 15.0], s), k(&[4.0, 9.0], s)];
        refuses_both("chi-square", |s| fault(chi_square_independence(&t(s))));
    }

    #[test]
    fn yates_refuses_overflow_and_underflow() {
        let t = |s: f64| vec![k(&[12.0, 5.0], s), k(&[7.0, 15.0], s)];
        refuses_both("yates", |s| fault(chi_square_2x2_yates(&t(s))));
    }

    #[test]
    fn ols_refuses_overflow_and_underflow() {
        refuses_both("ols", |s| fault(ols(&k(&Y, s), &[k(&X, s)])));
    }

    #[test]
    fn ols_qr_refuses_overflow_and_underflow() {
        refuses_both("ols_qr", |s| fault(ols_qr(&k(&Y, s), &[k(&X, s)])));
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
