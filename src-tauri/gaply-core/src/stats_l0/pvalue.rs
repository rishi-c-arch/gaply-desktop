//! **The p-value a test statistic implies, one thin function per family.**
//!
//! Each is a direct call into `statrs` 0.18, the distribution library
//! `stats_verify.rs` already uses. Upper tails come from `sf`, the survival
//! function, never `1 - cdf`: `1 - cdf` cancels in the far tail. F is the one
//! exception, and [`f_upper`] says why. `stats_verify.rs` computes its tails as
//! `1 - cdf`; it is left as it is.
//!
//! Precision against R is a gate, not an assumption:
//! `tests/l0_p_reference.rs` over `tests/fixtures/stats_l0/p_fixtures.csv`
//! holds every row to a relative 1e-9 of R's p.
//!
//! Inputs are points, not [`super::number::ReportedNumber`] intervals: the
//! caller evaluates an interval's endpoints. `None` means the arguments do not
//! define a p-value: a df that is not finite and above 0, a statistic that is
//! NaN or infinite, an |r| of 1 or more, a negative F or chi-square.

use statrs::distribution::{ChiSquared, ContinuousCDF, Normal, StudentsT};
use statrs::function::beta::checked_beta_reg;

/// How far a recomputed p may sit from the true p through engine error alone:
/// ten times the 1e-9 relative gate in `tests/l0_p_reference.rs`. It covers
/// the ENGINE only, not rounding in the manuscript. Step 5's `p_consistency`
/// widens each recomputed p endpoint outward by this relative margin. Nothing
/// uses it yet.
pub const ENGINE_P_RELATIVE_MARGIN: f64 = 1e-8;

/// A df a printed result can carry: finite and above 0.
fn df_ok(df: f64) -> Option<f64> {
    (df.is_finite() && df > 0.0).then_some(df)
}

/// A statistic that is a number: not NaN, not infinite.
fn stat_ok(x: f64) -> Option<f64> {
    x.is_finite().then_some(x)
}

/// Two-tailed p of a t statistic with `df` degrees of freedom (fractional
/// allowed, as for Welch).
pub fn t_two_tailed(t: f64, df: f64) -> Option<f64> {
    let t = stat_ok(t)?;
    let dist = StudentsT::new(0.0, 1.0, df_ok(df)?).ok()?;
    Some(2.0 * dist.sf(t.abs()))
}

/// Upper-tail p of an F statistic with `df1`, `df2` degrees of freedom
/// (fractional allowed, as for Welch's ANOVA).
///
/// Not `FisherSnedecor::sf`. statrs 0.18 computes that as
/// `beta_reg(df2/2, df1/2, 1 - df1·F/(df1·F + df2))` (`fisher_snedecor.rs:175`),
/// and the `1 - …` cancels when `df1·F` dwarfs `df2`. The same quantity is
/// `beta_reg(df2/2, df1/2, df2/(df1·F + df2))`, with no subtraction. Measured
/// over `p_fixtures.csv` (2527 rows, 9 Oct 2026), worst relative error against
/// R for F rows with p below 1e-6:
///
/// - `FisherSnedecor::sf`: 1.0. Ten rows (F(1, 1) and F(3, 1), F at least
///   5e15) returned exactly 0 where R gives 1e-8 to 1e-12, and 16 rows were
///   off by more than 1e-6.
/// - this form: 1.2e-12, and no row off by more than 1e-9.
pub fn f_upper(f: f64, df1: f64, df2: f64) -> Option<f64> {
    let f = stat_ok(f)?;
    let (df1, df2) = (df_ok(df1)?, df_ok(df2)?);
    // Stated for the reader; not the only guard. A negative F also puts the
    // beta argument outside [0, 1], which `checked_beta_reg` refuses, so
    // deleting this line leaves `a_negative_f_or_chi_square_is_none` green.
    if f < 0.0 {
        return None;
    }
    if f == 0.0 {
        return Some(1.0);
    }
    checked_beta_reg(df2 / 2.0, df1 / 2.0, df2 / (df1 * f + df2)).ok()
}

/// Upper-tail p of a chi-square statistic with `df` degrees of freedom.
pub fn chi_square_upper(x: f64, df: f64) -> Option<f64> {
    let x = stat_ok(x)?;
    let dist = ChiSquared::new(df_ok(df)?).ok()?;
    if x < 0.0 {
        return None;
    }
    Some(dist.sf(x))
}

/// Two-tailed p of a correlation `r` with `df = n - 2`, through
/// `t = r * sqrt(df / (1 - r^2))`. An |r| of 1 or more is `None`: its t is
/// infinite or undefined.
pub fn r_two_tailed(r: f64, df: f64) -> Option<f64> {
    let r = stat_ok(r)?;
    let df = df_ok(df)?;
    if r.abs() >= 1.0 {
        return None;
    }
    t_two_tailed(r * (df / (1.0 - r * r)).sqrt(), df)
}

/// Two-tailed p of a standard-normal z.
pub fn z_two_tailed(z: f64) -> Option<f64> {
    let z = stat_ok(z)?;
    let n = Normal::new(0.0, 1.0).expect("the standard normal is valid");
    Some(2.0 * n.sf(z.abs()))
}

#[cfg(test)]
mod tests {
    use super::*;

    const NON_POSITIVE_DF: [f64; 4] = [0.0, -1.0, -0.5, f64::NEG_INFINITY];
    const NOT_A_NUMBER: [f64; 3] = [f64::NAN, f64::INFINITY, f64::NEG_INFINITY];

    #[test]
    fn a_df_at_or_below_zero_is_none_for_every_family() {
        for df in NON_POSITIVE_DF {
            assert_eq!(t_two_tailed(2.0, df), None, "t df {df}");
            assert_eq!(f_upper(4.0, df, 20.0), None, "F df1 {df}");
            assert_eq!(f_upper(4.0, 2.0, df), None, "F df2 {df}");
            assert_eq!(chi_square_upper(4.0, df), None, "chi df {df}");
            assert_eq!(r_two_tailed(0.3, df), None, "r df {df}");
        }
        // A NaN or infinite df is not a printed df either.
        for df in [f64::NAN, f64::INFINITY] {
            assert_eq!(t_two_tailed(2.0, df), None, "t df {df}");
            assert_eq!(f_upper(4.0, df, 20.0), None, "F df1 {df}");
            assert_eq!(chi_square_upper(4.0, df), None, "chi df {df}");
            assert_eq!(r_two_tailed(0.3, df), None, "r df {df}");
        }
    }

    #[test]
    fn a_nan_or_infinite_statistic_is_none() {
        for x in NOT_A_NUMBER {
            assert_eq!(t_two_tailed(x, 10.0), None, "t {x}");
            assert_eq!(f_upper(x, 2.0, 20.0), None, "F {x}");
            assert_eq!(chi_square_upper(x, 3.0), None, "chi {x}");
            assert_eq!(r_two_tailed(x, 10.0), None, "r {x}");
            assert_eq!(z_two_tailed(x), None, "z {x}");
        }
    }

    #[test]
    fn an_r_of_one_or_more_in_size_is_none() {
        for r in [1.0, -1.0, 1.5, -1.0000001] {
            assert_eq!(r_two_tailed(r, 10.0), None, "r {r}");
        }
        assert!(r_two_tailed(0.999, 10.0).is_some());
    }

    #[test]
    fn a_negative_f_or_chi_square_is_none() {
        assert_eq!(f_upper(-0.1, 2.0, 20.0), None);
        assert_eq!(chi_square_upper(-0.1, 3.0), None);
    }

    #[test]
    fn a_statistic_of_zero_gives_p_one_for_every_family() {
        assert_eq!(t_two_tailed(0.0, 10.0), Some(1.0));
        assert_eq!(f_upper(0.0, 2.0, 20.0), Some(1.0));
        assert_eq!(chi_square_upper(0.0, 3.0), Some(1.0));
        assert_eq!(r_two_tailed(0.0, 10.0), Some(1.0));
        assert_eq!(z_two_tailed(0.0), Some(1.0));
    }

    #[test]
    fn a_negative_t_r_or_z_gives_the_same_p_as_its_positive() {
        assert_eq!(t_two_tailed(-2.31, 58.0), t_two_tailed(2.31, 58.0));
        assert_eq!(r_two_tailed(-0.31, 48.0), r_two_tailed(0.31, 48.0));
        assert_eq!(z_two_tailed(-2.05), z_two_tailed(2.05));
        assert!(t_two_tailed(-2.31, 58.0).unwrap() < 0.05);
    }

    #[test]
    fn a_fractional_df_is_accepted_for_t_f_and_chi_square() {
        let t = t_two_tailed(2.10, 28.4).expect("Welch t");
        let f = f_upper(4.2, 2.0, 27.35).expect("Welch F");
        let c = chi_square_upper(6.9, 2.5).expect("fractional chi-square df");
        for p in [t, f, c] {
            assert!(p > 0.0 && p < 1.0, "{p}");
        }
        // Fractional df changes the answer: it is used, not truncated.
        assert_ne!(t, t_two_tailed(2.10, 28.0).unwrap());
        assert_ne!(f, f_upper(4.2, 2.0, 27.0).unwrap());
        assert_ne!(c, chi_square_upper(6.9, 2.0).unwrap());
    }
}
