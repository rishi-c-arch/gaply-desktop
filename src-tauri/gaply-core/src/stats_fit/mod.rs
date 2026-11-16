//! **`gaply_core::stats_fit`: AMOS's measures of fit, in three rows, for any
//! model fitted by maximum likelihood to one covariance matrix (Research OS,
//! Phase 9C-4, decision 796).** Moved from `stats_cfa` (Phase 9C-1, decision
//! 745; the rows, Phase 9C-2, decision 763; the zero rules, decision 773), its
//! arithmetic and its order unchanged, so that the confirmatory factor analysis
//! and the structural equation models compute every measure with one function.
//!
//! With C = CMIN, d = DF, q = NPAR, n = N − 1, p* = p (p + 1) / 2 and b the
//! independence model, [`model_fit`] computes one row of AMOS's Appendix C
//! against the independence model's C_b and d_b; [`fit_measures`] applies it to
//! the default model (the one the user specified), the saturated model (Σ = S,
//! every moment a parameter, d = 0) and the independence model (Σ = diag(S),
//! only the variances free).
//!
//! The result names no method: each of a row's 36 statistics is a
//! [`FitValue`], a number or one of seven closed reasons, and each method maps
//! the reasons to its own codes. Each row's two interval searches call that
//! row's check point before every evaluation of the noncentral lower tail
//! (decision 796, from decision 767's second answer), so a long bisection can
//! be stopped; the counts of check points are unchanged.
//!
//! Pure: no file, network, process, clock, model or log
//! (`tests/contracts_are_pure.rs`); every operation +, −, ×, ÷, √ and ln, and
//! the distributions of `stats_dist`.

use crate::stats_dist::{
    chi_square_quantile_upper, chi_square_upper_wide, noncentral_chi_square_ncp_with,
    noncentral_chi_square_tails,
};
use crate::stats_linalg::multiply;

/// At 0 degrees of freedom a minimum F of at most this is written as 0
/// (decision 761). The review of 9C-1 measured the largest F at d = 0 over 1,011
/// solutions at 7.1e-15.
pub const DF_ZERO_ROUNDING: f64 = 1e-12;
/// PCLOSE's RMSEA under the null, 0.05: its ncp is 0.05² n d (AMOS, Appendix C).
pub const PCLOSE_RMSEA: f64 = 0.05;

/// Why a measure of fit has no value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FitCode {
    /// The measure divides by, or needs, degrees of freedom the row has none of.
    DfZero,
    /// HOELTER divides by C, which is 0.
    CminZero,
    /// A comparison divides by C_b, which is 0 (decision 773).
    IndependenceCminZero,
    /// BCC needs N above p + 2.
    TooFewRowsForBcc,
    /// A 0/0 of quantities that vanished.
    TooSmall,
    /// An infinity.
    Overflow,
    /// The distribution gives no value there.
    DistributionOutOfRange,
}

/// A measure of fit: a number, or the closed reason it is not defined.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum FitValue {
    Value(f64),
    NotComputable(FitCode),
}

/// One model's row of AMOS's measures of fit (its Appendix C): NPAR and DF, and
/// the 36 statistics. The 90% intervals are `…_lo90` and `…_hi90`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct FitRow {
    pub npar: u64,
    pub df: u64,
    pub cmin: FitValue,
    pub p: FitValue,
    pub cmin_df: FitValue,
    pub rmr: FitValue,
    pub gfi: FitValue,
    pub agfi: FitValue,
    pub pgfi: FitValue,
    pub nfi: FitValue,
    pub rfi: FitValue,
    pub ifi: FitValue,
    pub tli: FitValue,
    pub cfi: FitValue,
    pub pratio: FitValue,
    pub pnfi: FitValue,
    pub pcfi: FitValue,
    pub ncp: FitValue,
    pub ncp_lo90: FitValue,
    pub ncp_hi90: FitValue,
    pub fmin: FitValue,
    pub f0: FitValue,
    pub f0_lo90: FitValue,
    pub f0_hi90: FitValue,
    pub rmsea: FitValue,
    pub rmsea_lo90: FitValue,
    pub rmsea_hi90: FitValue,
    pub pclose: FitValue,
    pub aic: FitValue,
    pub bcc: FitValue,
    pub bic: FitValue,
    pub caic: FitValue,
    pub ecvi: FitValue,
    pub ecvi_lo90: FitValue,
    pub ecvi_hi90: FitValue,
    pub mecvi: FitValue,
    pub hoelter05: FitValue,
    pub hoelter01: FitValue,
}

/// Which row's check point is called: the default model's, before each evaluation of its
/// interval searches, or the independence model's, before its row and each evaluation of its
/// own (decision 796).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FitPoint {
    Default,
    Independence,
}

/// The three rows.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct FitRows {
    pub model: FitRow,
    pub saturated: FitRow,
    pub independence: FitRow,
}

/// The model's degrees of freedom, p (p + 1) / 2 − q, by checked subtraction:
/// `Ok(d)`, or `Err` with the count by which q exceeds the moments, which each
/// caller refuses as `model_not_identified` (Research OS, Phase 9C-5, decision
/// 812). Each caller's identification check refuses such a spec before any
/// fit, so the `Err` is a guard that no run reaches.
pub(crate) fn model_df(p: usize, q: u64) -> Result<u64, u64> {
    let moments = (p * (p + 1) / 2) as u64;
    moments.checked_sub(q).ok_or_else(|| q - moments)
}

/// What the fit is computed from: S with divisor N and ln|S|; the fitted Σ̂ and
/// Σ̂⁻¹; the minimum F as the estimation reached it; NPAR q; the degrees of
/// freedom d, from [`model_df`]; N and n = N − 1.
pub(crate) struct FitInput<'a> {
    pub s: &'a [Vec<f64>],
    pub log_det_s: f64,
    pub sigma: &'a [Vec<f64>],
    pub sigma_inverse: &'a [Vec<f64>],
    pub f: f64,
    pub q: u64,
    pub d: u64,
    pub big_n: f64,
    pub n: f64,
}

/// `v` as a value; an infinity is `overflow`; a NaN, a 0/0 of quantities that
/// vanished, is `too_small`.
fn stat(v: f64) -> FitValue {
    if v.is_finite() {
        FitValue::Value(v)
    } else if v.is_nan() {
        FitValue::NotComputable(FitCode::TooSmall)
    } else {
        FitValue::NotComputable(FitCode::Overflow)
    }
}

/// A distribution's value, or `distribution_out_of_range` where it gives none.
fn dist(v: Option<f64>) -> FitValue {
    v.map_or(FitValue::NotComputable(FitCode::DistributionOutOfRange), stat)
}

/// `v` unless the degrees of freedom are 0.
fn unless_df_zero(d: u64, v: impl FnOnce() -> FitValue) -> FitValue {
    if d == 0 {
        FitValue::NotComputable(FitCode::DfZero)
    } else {
        v()
    }
}

/// AMOS's Appendix C (decisions 745 and 763): one function, [`model_fit`],
/// applied to the default model, the saturated model and, after
/// `at(FitPoint::Independence)`, the independence model, each against the
/// independence model's C_b and d_b. `at(FitPoint::Default)` is called before
/// each evaluation of the default row's two interval searches, and
/// `at(FitPoint::Independence)` before each of the independence row's (decision
/// 796); the saturated row, at d = 0, searches none.
pub(crate) fn fit_measures<E>(input: &FitInput<'_>, at: &mut impl FnMut(FitPoint) -> Result<(), E>) -> Result<FitRows, E> {
    let FitInput { s, log_det_s, sigma, sigma_inverse, f, q, d, big_n, n } = *input;
    let p = s.len();
    // Decision 823 (the review of 365b638, finding 8): q and d arrive apart, so
    // a caller that took d from anything but model_df is caught in debug builds.
    debug_assert_eq!(model_df(p, q), Ok(d), "FitInput's d is not model_df(p, q)");
    let moments = (p * (p + 1) / 2) as u64;
    // Decision 745 clamps a minimum below 0 to 0 at every d. At d = 0 a model
    // with as many parameters as moments that has converged reproduces S, so
    // its F is 0 and rounding of either sign is not a fit: an F of at most
    // DF_ZERO_ROUNDING there is written as 0 before every measure is computed
    // from C (decision 761). Above it, F is kept as computed. Rounding above 0
    // made CFI 1 − C / C = 0 wherever C_b < d_b (decision 759's B2).
    let f_min = if d == 0 && f <= DF_ZERO_ROUNDING { 0.0 } else { f.max(0.0) };
    let c = n * f_min;
    // The independence model: only the variances free, so Σ = diag(S) and its
    // discrepancy is Σ ln s_ii − ln |S|.
    let db = (p * (p - 1) / 2) as u64;
    let ln_diag: f64 = (0..p).map(|i| s[i][i].ln()).sum();
    // Clamped at 0 as F is: on columns uncorrelated to rounding the difference
    // can round below 0 (−3.4e-15 on Walsh columns; the review of 9C-2,
    // finding 6), and C_b is the independence row's own CMIN.
    let cb = n * (ln_diag - log_det_s).max(0.0);
    let (rmr, gfi) = rmr_and_gfi(s, sigma, sigma_inverse);
    let model = model_fit(Row { q, d, c, rmr, gfi }, cb, db, p, big_n, n, &mut || at(FitPoint::Default))?;
    // The saturated model (decision 763): Σ = S, every moment a parameter, d = 0
    // and C = 0; its RMR 0 and its GFI 1 written, not computed from S⁻¹ S, and
    // its NFI and IFI 1 (decision 773).
    let saturated = saturated_fit(moments, cb, db, p, big_n, n);
    at(FitPoint::Independence)?;
    let diagonal: Vec<Vec<f64>> = (0..p).map(|i| (0..p).map(|j| if i == j { s[i][i] } else { 0.0 }).collect()).collect();
    let diagonal_inverse: Vec<Vec<f64>> = (0..p).map(|i| (0..p).map(|j| if i == j { 1.0 / s[i][i] } else { 0.0 }).collect()).collect();
    let (rmr_b, gfi_b) = rmr_and_gfi(s, &diagonal, &diagonal_inverse);
    let mut baseline = model_fit(Row { q: p as u64, d: db, c: cb, rmr: rmr_b, gfi: gfi_b }, cb, db, p, big_n, n, &mut || at(FitPoint::Independence))?;
    // AMOS prints the baseline against itself as .000 and its PRATIO as 1.000
    // (decision 763); the formulas give other values where C_b ≤ d_b (CFI 1 by
    // the zero denominator, IFI and TLI −0), so these are written, not computed.
    let zero = FitValue::Value(0.0);
    baseline.nfi = zero;
    baseline.rfi = zero;
    baseline.ifi = zero;
    baseline.tli = zero;
    baseline.cfi = zero;
    baseline.pratio = FitValue::Value(1.0);
    baseline.pnfi = zero;
    baseline.pcfi = zero;
    Ok(FitRows { model, saturated, independence: baseline })
}

/// RMR over the lower triangle with the diagonal, on S with divisor N, and GFI
/// from Σ̂⁻¹ S (decision 745), for a model's Σ̂ and Σ̂⁻¹.
pub(crate) fn rmr_and_gfi(s: &[Vec<f64>], sigma: &[Vec<f64>], inverse: &[Vec<f64>]) -> (f64, f64) {
    let p = s.len();
    let pstar = ((p * (p + 1) / 2) as u64) as f64;
    let mut squares = 0.0;
    for i in 0..p {
        for j in 0..=i {
            let r = s[i][j] - sigma[i][j];
            squares += r * r;
        }
    }
    let rmr = (squares / pstar).sqrt();
    let m = multiply(inverse, s);
    let (mut residual, mut whole) = (0.0, 0.0);
    for i in 0..p {
        for j in 0..p {
            let a = m[i][j] - if i == j { 1.0 } else { 0.0 };
            let b = m[j][i] - if i == j { 1.0 } else { 0.0 };
            residual += a * b;
            whole += m[i][j] * m[j][i];
        }
    }
    (rmr, 1.0 - residual / whole)
}

/// What a model's row is computed from: its NPAR, DF, CMIN, RMR and GFI.
pub(crate) struct Row {
    pub q: u64,
    pub d: u64,
    pub c: f64,
    pub rmr: f64,
    pub gfi: f64,
}

/// The saturated model's row (decision 763): Σ = S, every moment a parameter, d
/// = 0 and C = 0. Its RMR 0, GFI 1, NFI 1 and IFI 1 are written, not computed
/// (decision 773): at C_b = 0 NFI's 1 − 0/0 and IFI's 0/0 have no value,
/// though the model reproduces S. Its CFI is 1 by its rule at every C_b. At d =
/// 0 no interval is searched, so no check point is called.
pub(crate) fn saturated_fit(moments: u64, cb: f64, db: u64, p: usize, big_n: f64, n: f64) -> FitRow {
    let never = &mut || Ok::<(), std::convert::Infallible>(());
    let mut row = match model_fit(Row { q: moments, d: 0, c: 0.0, rmr: 0.0, gfi: 1.0 }, cb, db, p, big_n, n, never) {
        Ok(row) => row,
        Err(never) => match never {},
    };
    row.nfi = FitValue::Value(1.0);
    row.ifi = FitValue::Value(1.0);
    row
}

/// A comparison with the independence model (NFI, RFI, IFI, TLI, CFI, PNFI,
/// PCFI): `v` as [`stat`] gives it, but `independence_cmin_zero` where `v` is
/// not finite and C_b is 0, since then the cause is the comparison's division
/// by C_b, not a value too small or too large (decision 773). A finite `v` is
/// unchanged at every C_b.
pub(crate) fn against_baseline(v: f64, cb: f64) -> FitValue {
    if cb == 0.0 && !v.is_finite() {
        FitValue::NotComputable(FitCode::IndependenceCminZero)
    } else {
        stat(v)
    }
}

/// One model's row of Appendix C's measures (decisions 745 and 763), against
/// the independence model's C_b and d_b, for p columns and N rows. `step` is
/// called before each evaluation of the noncentral lower tail in the two
/// interval searches (decision 796); its `Err` stops the row.
pub(crate) fn model_fit<E>(
    row: Row,
    cb: f64,
    db: u64,
    p: usize,
    big_n: f64,
    n: f64,
    step: &mut impl FnMut() -> Result<(), E>,
) -> Result<FitRow, E> {
    let Row { q, d, c, rmr, gfi } = row;
    let pf = p as f64;
    let qf = q as f64;
    let df = d as f64;
    let pstar = ((p * (p + 1) / 2) as u64) as f64;
    let dbf = db as f64;
    let ncp = (c - df).max(0.0);
    let lo = if d == 0 { None } else { noncentral_chi_square_ncp_with(c, df, 0.95, &mut *step)? };
    let hi = if d == 0 { None } else { noncentral_chi_square_ncp_with(c, df, 0.05, &mut *step)? };
    let interval = |v: Option<f64>, f: &dyn Fn(f64) -> f64| unless_df_zero(d, || dist(v.map(f)));
    let nfi = 1.0 - c / cb;
    let cfi_denominator = (cb - dbf).max(c - df).max(0.0);
    let cfi = if cfi_denominator == 0.0 { 1.0 } else { 1.0 - ncp / cfi_denominator };
    let pratio = df / dbf;
    let bcc = if big_n <= pf + 2.0 {
        FitValue::NotComputable(FitCode::TooFewRowsForBcc)
    } else {
        stat(c + 2.0 * qf * (big_n - 1.0) / (big_n - pf - 2.0))
    };
    let hoelter = |alpha: f64| {
        unless_df_zero(d, || {
            if c == 0.0 {
                FitValue::NotComputable(FitCode::CminZero)
            } else {
                match chi_square_quantile_upper(alpha, df) {
                    Some(x) => stat((n * x / c + 1.0).floor()),
                    None => FitValue::NotComputable(FitCode::DistributionOutOfRange),
                }
            }
        })
    };
    Ok(FitRow {
        npar: q,
        cmin: stat(c),
        df: d,
        p: unless_df_zero(d, || dist(chi_square_upper_wide(c, df))),
        cmin_df: unless_df_zero(d, || stat(c / df)),
        rmr: stat(rmr),
        gfi: stat(gfi),
        agfi: unless_df_zero(d, || stat(1.0 - (pstar / df) * (1.0 - gfi))),
        pgfi: unless_df_zero(d, || stat((df / pstar) * gfi)),
        nfi: against_baseline(nfi, cb),
        rfi: unless_df_zero(d, || against_baseline(1.0 - (c / df) / (cb / dbf), cb)),
        ifi: against_baseline((cb - c) / (cb - df), cb),
        tli: unless_df_zero(d, || against_baseline((cb / dbf - c / df) / (cb / dbf - 1.0), cb)),
        cfi: against_baseline(cfi, cb),
        pratio: unless_df_zero(d, || stat(pratio)),
        pnfi: unless_df_zero(d, || against_baseline(nfi * pratio, cb)),
        pcfi: unless_df_zero(d, || against_baseline(cfi * pratio, cb)),
        ncp: stat(ncp),
        ncp_lo90: interval(lo, &|x| x),
        ncp_hi90: interval(hi, &|x| x),
        fmin: stat(c / n),
        f0: stat(ncp / n),
        f0_lo90: interval(lo, &|x| x / n),
        f0_hi90: interval(hi, &|x| x / n),
        rmsea: unless_df_zero(d, || stat((ncp / n / df).sqrt())),
        rmsea_lo90: interval(lo, &|x| (x / (n * df)).sqrt()),
        rmsea_hi90: interval(hi, &|x| (x / (n * df)).sqrt()),
        pclose: unless_df_zero(d, || {
            dist(noncentral_chi_square_tails(c, df, PCLOSE_RMSEA * PCLOSE_RMSEA * n * df).map(|t| t.1))
        }),
        aic: stat(c + 2.0 * qf),
        bcc,
        bic: stat(c + qf * big_n.ln()),
        caic: stat(c + qf * (big_n.ln() + 1.0)),
        ecvi: stat((c + 2.0 * qf) / n),
        ecvi_lo90: interval(lo, &|x| (x + df + 2.0 * qf) / n),
        ecvi_hi90: interval(hi, &|x| (x + df + 2.0 * qf) / n),
        mecvi: match bcc {
            FitValue::Value(v) => stat(v / n),
            code => code,
        },
        hoelter05: hoelter(0.05),
        hoelter01: hoelter(0.01),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Decision 812 (decision 808's third answer): the model's degrees of
    /// freedom by checked subtraction, below, at and above the p (p + 1) / 2
    /// moments; never a wrapped or panicking subtraction.
    #[test]
    fn model_df_subtracts_by_checking_below_at_and_above_the_moments() {
        // p = 6: 21 moments.
        assert_eq!(model_df(6, 15), Ok(6));
        assert_eq!(model_df(6, 21), Ok(0));
        assert_eq!(model_df(6, 22), Err(1));
        assert_eq!(model_df(6, 40), Err(19));
        // p = 1: one moment; q far above it.
        assert_eq!(model_df(1, u64::MAX), Err(u64::MAX - 1));
        assert_eq!(model_df(128, 0), Ok(8256));
    }

    /// Decision 823 (N2; the review of 365b638, finding 8): FitInput carries
    /// q and d apart, so fit_measures asserts, in debug builds, that d is
    /// model_df(p, q) with p = s.len(). Two columns are 3 moments; q = 2 makes
    /// d 1, and this input says 0.
    #[cfg(debug_assertions)]
    #[test]
    #[should_panic(expected = "FitInput's d is not model_df(p, q)")]
    fn fit_measures_refuses_an_input_whose_d_disagrees_with_its_q() {
        let identity = vec![vec![1.0, 0.0], vec![0.0, 1.0]];
        assert_eq!(model_df(2, 2), Ok(1), "the premise");
        let input = FitInput { s: &identity, log_det_s: 0.0, sigma: &identity, sigma_inverse: &identity, f: 0.0, q: 2, d: 0, big_n: 100.0, n: 99.0 };
        let _ = fit_measures(&input, &mut |_| Ok::<(), std::convert::Infallible>(()));
    }
}
