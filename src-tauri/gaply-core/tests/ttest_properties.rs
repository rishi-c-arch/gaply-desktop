//! **Property and metamorphic tests of the t-test as an analysis (Research
//! OS, Phase 4B-1).**
//!
//! proptest as Phase 2A uses it (decision 3, decision 67): a fixed seed,
//! failure persistence off (no proptest-regressions file is ever written),
//! bounded generators. Values are on the grid j/128 with |j| ≤ 1.28e8, so in
//! ±1e6; a shift c is on the same grid, so x + c is exact; a scale is
//! k = m · 2^-15 with m in [33, 32,768,000], so k is in [1e-3, 1e3] and k·x
//! is exact (|j·m| < 2^53). A miss is therefore Gaply's, never the input's
//! rounding. Each group has 2 to 40 values.
//!
//! THE TOLERANCES, stated per relation, with ε = 1e-12 (the golden gate):
//!
//! * Swapping the two levels: EXACT. t, the mean difference, d and the
//!   interval change sign bit for bit, and p, df, the standard error and the
//!   sd ratio keep every bit: every sum and product in them is commutative
//!   in IEEE arithmetic, and a − b is exactly −(b − a).
//! * Permuting the rows: EXACT, the result's canonical bytes are equal (each
//!   group's values are sorted before any arithmetic).
//! * A shift by c, and a scale by k: each side meets the golden gate against
//!   its own exact value, so the two may differ by the SUM of their two
//!   gates (the gate's scale computed on each side), plus one unit in the
//!   last place of the reference side's own product or sum: the same rule as
//!   Phase 2A's shift and scale relations. For p, t's two gates are carried
//!   into p through Student's t density, 2 · f(|t|) · (sum of t's gates),
//!   plus 1e-15.
//! * p lies in [0, 1] and the interval contains the mean difference: EXACT.
//! * Welch's df lies in [min(n1, n2) − 1, n1 + n2 − 2], within ε · (n1 + n2).
//!
//! Both groups constant is refused, never computed; the generators keep
//! that case and check its code.

use proptest::prelude::*;
use proptest::test_runner::{Config as ProptestConfig, RngAlgorithm, RngSeed};
use statrs::distribution::{Continuous, StudentsT};

use gaply_core::stats_desc::Alias;
use gaply_core::stats_l1::dataset::{
    build_dataset, ColumnAlias, DataFingerprint, Dataset, RawTable, TableRef,
};
use gaply_core::stats_ttest::{compute, Stat, TTestCode, TTestParams, TTestResult, Variant};

const EPS: f64 = 1e-12;
const GRID: f64 = 128.0;
const SEED: u64 = 20_261_018;

fn config() -> ProptestConfig {
    ProptestConfig {
        // 6 properties × 512 cases, both variants each: 0.58 s in a debug
        // build (measured 18 Oct 2026, Phase 4B-1 report).
        cases: 512,
        failure_persistence: None,
        rng_algorithm: RngAlgorithm::ChaCha,
        rng_seed: RngSeed::Fixed(SEED),
        ..ProptestConfig::default()
    }
}

/// Rows `(y, g)` with g "a" or "b", as text a file would hold.
fn dataset(rows: &[(f64, &str)]) -> Dataset {
    let headers = vec!["y".to_string(), "g".to_string()];
    let table: Vec<Vec<String>> = rows
        .iter()
        .map(|(y, g)| vec![format!("{y}"), g.to_string()])
        .collect();
    build_dataset(
        RawTable::from_rows(headers, table),
        DataFingerprint::of_bytes(b"synthetic", "proptest", TableRef::FirstTable),
    )
    .expect("dataset")
}

fn params(levels: [u32; 2], variant: Variant) -> TTestParams {
    TTestParams {
        outcome: Alias(ColumnAlias(1)),
        group: Alias(ColumnAlias(2)),
        levels: Some(levels),
        variant: Some(variant),
        missing_codes: vec![],
    }
}

fn rows_of(a: &[f64], b: &[f64]) -> Vec<(f64, &'static str)> {
    let mut rows: Vec<(f64, &'static str)> = a.iter().map(|v| (*v, "a")).collect();
    rows.extend(b.iter().map(|v| (*v, "b")));
    rows
}

fn grid_group() -> impl Strategy<Value = Vec<i64>> {
    prop::collection::vec(-128_000_000i64..=128_000_000, 2..=40)
}

/// Two groups, sometimes with ties, sometimes one or both constant.
fn two_groups() -> impl Strategy<Value = (Vec<i64>, Vec<i64>)> {
    let group = prop_oneof![
        6 => grid_group(),
        1 => (-128_000_000i64..=128_000_000, 2usize..=40).prop_map(|(j, n)| vec![j; n]),
        1 => prop::collection::vec(-3i64..=3, 2..=40),
    ];
    (group.clone(), group)
}

fn values(js: &[i64], f: impl Fn(f64) -> f64) -> Vec<f64> {
    js.iter().map(|&j| f(j as f64 / GRID)).collect()
}

fn constant(js: &[i64]) -> bool {
    js.iter().all(|j| *j == js[0])
}

/// The t-test on `a` (level "a", index 0) and `b`, or the refusal it must be.
fn run(a: &[f64], b: &[f64], levels: [u32; 2], variant: Variant) -> Option<TTestResult> {
    let ds = dataset(&rows_of(a, b));
    let both_constant = a.iter().all(|v| *v == a[0]) && b.iter().all(|v| *v == b[0]);
    match compute(&ds, &params(levels, variant)) {
        Ok(r) => {
            assert!(!both_constant, "both groups constant must be refused");
            Some(r)
        }
        Err(e) => {
            assert!(both_constant, "refused: {e:?}");
            assert_eq!(e.code, TTestCode::BothGroupsConstant);
            None
        }
    }
}

fn ulp(x: f64) -> f64 {
    let a = x.abs();
    if a == 0.0 {
        f64::MIN_POSITIVE
    } else {
        f64::from_bits(a.to_bits() + 1) - a
    }
}

fn big_m(r: &TTestResult) -> f64 {
    r.groups[0].mean.abs() + r.groups[1].mean.abs()
}

/// The golden gates' scales, on one side.
fn md_scale(r: &TTestResult) -> f64 {
    big_m(r).max(r.standard_error)
}
fn t_scale(r: &TTestResult) -> f64 {
    r.t.abs().max(big_m(r) / r.standard_error)
}
fn d_scale(r: &TTestResult) -> f64 {
    r.cohens_d.abs().max(big_m(r) / r.d_standardizer_value)
}
fn ci_scale(r: &TTestResult, bound: f64) -> f64 {
    bound.abs().max(big_m(r)).max(r.standard_error)
}
fn sd_scale(sd: f64) -> f64 {
    if sd == 0.0 {
        1.0
    } else {
        sd
    }
}
fn density(t: f64, df: f64) -> f64 {
    StudentsT::new(0.0, 1.0, df).expect("df").pdf(t.abs())
}

fn ratio(r: &TTestResult) -> Option<f64> {
    match r.sd_ratio {
        Stat::Value(v) => Some(v),
        Stat::NotComputable(_) => None,
    }
}

proptest! {
    #![proptest_config(config())]

    #[test]
    fn swapping_the_levels_negates_the_signed_statistics_bit_for_bit((a, b) in two_groups()) {
        let (a, b) = (values(&a, |x| x), values(&b, |x| x));
        for variant in [Variant::Student, Variant::Welch] {
            let (Some(x), Some(y)) = (run(&a, &b, [0, 1], variant), run(&a, &b, [1, 0], variant)) else {
                continue;
            };
            prop_assert_eq!(y.t.to_bits(), (-x.t).to_bits());
            prop_assert_eq!(y.mean_difference.to_bits(), (-x.mean_difference).to_bits());
            prop_assert_eq!(y.cohens_d.to_bits(), (-x.cohens_d).to_bits());
            prop_assert_eq!(y.ci_lower.to_bits(), (-x.ci_upper).to_bits());
            prop_assert_eq!(y.ci_upper.to_bits(), (-x.ci_lower).to_bits());
            prop_assert_eq!(y.p.to_bits(), x.p.to_bits());
            prop_assert_eq!(y.df.to_bits(), x.df.to_bits());
            prop_assert_eq!(y.standard_error.to_bits(), x.standard_error.to_bits());
            prop_assert_eq!(y.d_standardizer_value.to_bits(), x.d_standardizer_value.to_bits());
            prop_assert_eq!(ratio(&y).map(f64::to_bits), ratio(&x).map(f64::to_bits));
            prop_assert_eq!(&y.groups[0], &x.groups[1]);
            prop_assert_eq!(&y.groups[1], &x.groups[0]);
        }
    }

    #[test]
    fn permuting_the_rows_changes_no_byte(
        rows in two_groups()
            .prop_map(|(a, b)| rows_of(&values(&a, |x| x), &values(&b, |x| x)))
            .prop_flat_map(|rows| (Just(rows.clone()), Just(rows).prop_shuffle()))
    ) {
        let (original, shuffled) = rows;
        for variant in [Variant::Student, Variant::Welch] {
            let one = compute(&dataset(&original), &params([0, 1], variant));
            let two = compute(&dataset(&shuffled), &params([0, 1], variant));
            match (one, two) {
                (Ok(x), Ok(y)) => prop_assert_eq!(x.canonical_bytes(), y.canonical_bytes()),
                (Err(x), Err(y)) => prop_assert_eq!(x, y),
                (x, y) => prop_assert!(false, "{:?} vs {:?}", x, y),
            }
        }
    }

    #[test]
    fn a_shift_changes_only_the_means(
        (a, b) in two_groups(),
        c in (-128_000_000i64..=128_000_000).prop_map(|j| j as f64 / GRID),
    ) {
        let (xa, xb) = (values(&a, |x| x), values(&b, |x| x));
        let (ya, yb) = (values(&a, |x| x + c), values(&b, |x| x + c));
        for variant in [Variant::Student, Variant::Welch] {
            let (Some(x), Some(y)) = (run(&xa, &xb, [0, 1], variant), run(&ya, &yb, [0, 1], variant)) else {
                continue;
            };
            for k in 0..2 {
                let (mx, my) = (x.groups[k].mean, y.groups[k].mean);
                let bound = EPS * (mx.abs().max(sd_scale(x.groups[k].sd)) + my.abs().max(sd_scale(y.groups[k].sd))) + ulp(mx + c);
                prop_assert!((my - (mx + c)).abs() <= bound, "mean {}: {:e} vs {:e} + {:e}", k, my, mx, c);
                let bound = EPS * (sd_scale(x.groups[k].sd) + sd_scale(y.groups[k].sd)) + ulp(x.groups[k].sd);
                prop_assert!((y.groups[k].sd - x.groups[k].sd).abs() <= bound, "sd {}", k);
            }
            let close = |name: &str, got: f64, want: f64, bound: f64| {
                prop_assert!((got - want).abs() <= bound, "{}: {:e} vs {:e}, bound {:e}", name, got, want, bound);
                Ok(())
            };
            close("mean difference", y.mean_difference, x.mean_difference, EPS * (md_scale(&x) + md_scale(&y)) + ulp(x.mean_difference))?;
            close("se", y.standard_error, x.standard_error, EPS * (x.standard_error + y.standard_error) + ulp(x.standard_error))?;
            let t_bound = EPS * (t_scale(&x) + t_scale(&y)) + ulp(x.t);
            close("t", y.t, x.t, t_bound)?;
            close("df", y.df, x.df, EPS * (x.df + y.df))?;
            close("p", y.p, x.p, 2.0 * density(x.t, x.df) * t_bound + 1e-15)?;
            close("d", y.cohens_d, x.cohens_d, EPS * (d_scale(&x) + d_scale(&y)) + ulp(x.cohens_d))?;
            close("ci_lower", y.ci_lower, x.ci_lower, EPS * (ci_scale(&x, x.ci_lower) + ci_scale(&y, y.ci_lower)) + ulp(x.ci_lower))?;
            close("ci_upper", y.ci_upper, x.ci_upper, EPS * (ci_scale(&x, x.ci_upper) + ci_scale(&y, y.ci_upper)) + ulp(x.ci_upper))?;
        }
    }

    #[test]
    fn a_scale_scales_the_difference_its_error_interval_and_sds_and_not_t_p_or_d(
        (a, b) in two_groups(),
        m in 33i64..=32_768_000,
    ) {
        let k = m as f64 / 32_768.0;
        let (xa, xb) = (values(&a, |x| x), values(&b, |x| x));
        let (ya, yb) = (values(&a, |x| x * k), values(&b, |x| x * k));
        for variant in [Variant::Student, Variant::Welch] {
            let (Some(x), Some(y)) = (run(&xa, &xb, [0, 1], variant), run(&ya, &yb, [0, 1], variant)) else {
                continue;
            };
            let close = |name: &str, got: f64, want: f64, bound: f64| {
                prop_assert!((got - want).abs() <= bound, "{}: {:e} vs {:e}, bound {:e}", name, got, want, bound);
                Ok(())
            };
            close("mean difference", y.mean_difference, k * x.mean_difference, EPS * (k * md_scale(&x) + md_scale(&y)) + ulp(k * x.mean_difference))?;
            close("se", y.standard_error, k * x.standard_error, EPS * (k * x.standard_error + y.standard_error) + ulp(k * x.standard_error))?;
            close("ci_lower", y.ci_lower, k * x.ci_lower, EPS * (k * ci_scale(&x, x.ci_lower) + ci_scale(&y, y.ci_lower)) + ulp(k * x.ci_lower))?;
            close("ci_upper", y.ci_upper, k * x.ci_upper, EPS * (k * ci_scale(&x, x.ci_upper) + ci_scale(&y, y.ci_upper)) + ulp(k * x.ci_upper))?;
            for g in 0..2 {
                let (sx, sy) = (x.groups[g].sd, y.groups[g].sd);
                close("sd", sy, k * sx, EPS * (k * sd_scale(sx) + sd_scale(sy)) + ulp(k * sx))?;
            }
            let t_bound = EPS * (t_scale(&x) + t_scale(&y)) + ulp(x.t);
            close("t", y.t, x.t, t_bound)?;
            close("df", y.df, x.df, EPS * (x.df + y.df))?;
            close("p", y.p, x.p, 2.0 * density(x.t, x.df) * t_bound + 1e-15)?;
            close("d", y.cohens_d, x.cohens_d, EPS * (d_scale(&x) + d_scale(&y)) + ulp(x.cohens_d))?;
        }
    }

    #[test]
    fn p_is_a_probability_and_the_interval_contains_the_difference((a, b) in two_groups()) {
        let (a, b) = (values(&a, |x| x), values(&b, |x| x));
        for variant in [Variant::Student, Variant::Welch] {
            let Some(r) = run(&a, &b, [0, 1], variant) else { continue };
            prop_assert!((0.0..=1.0).contains(&r.p), "p {}", r.p);
            prop_assert!(r.ci_lower <= r.mean_difference && r.mean_difference <= r.ci_upper);
            prop_assert!(r.standard_error > 0.0 && r.d_standardizer_value > 0.0);
        }
    }

    #[test]
    fn welchs_df_lies_between_its_bounds((a, b) in two_groups()) {
        prop_assume!(!(constant(&a) && constant(&b)));
        let (va, vb) = (values(&a, |x| x), values(&b, |x| x));
        let r = run(&va, &vb, [0, 1], Variant::Welch).expect("not both constant");
        let (n1, n2) = (a.len() as f64, b.len() as f64);
        let tol = EPS * (n1 + n2);
        prop_assert!(r.df >= n1.min(n2) - 1.0 - tol, "df {} below min(n) - 1 = {}", r.df, n1.min(n2) - 1.0);
        prop_assert!(r.df <= n1 + n2 - 2.0 + tol, "df {} above n1 + n2 - 2 = {}", r.df, n1 + n2 - 2.0);
        let s = run(&va, &vb, [0, 1], Variant::Student).expect("not both constant");
        prop_assert_eq!(s.df, n1 + n2 - 2.0, "Student's df");
    }
}
