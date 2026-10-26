//! **Property and metamorphic tests of the one-sample and paired t-tests as
//! analyses (Research OS, Phase 8D-1).**
//!
//! proptest as `ttest_properties.rs` uses it: a fixed seed, failure
//! persistence off (no proptest-regressions file is ever written), bounded
//! generators. Values are on the grid j/128 with |j| ≤ 1.28e8, so in ±1e6; a
//! shift c is on the same grid, so x + c is exact; a scale is k = m · 2^-15
//! with m in [33, 32,768,000], so k is in [1e-3, 1e3] and k·x is exact
//! (|j·m| < 2^53). A miss is therefore Gaply's, never the input's rounding.
//! Each sample has 2 to 40 values or pairs.
//!
//! THE TOLERANCES, stated per relation, with ε = 1e-12 (the golden gate):
//!
//! * A shift by c, a scale by k, and swapping first and second: each side
//!   meets the golden gate against its own exact value, so the two may differ
//!   by the SUM of their two gates (each gate's scale computed on its own
//!   side), plus one unit in the last place of the reference side's value.
//!   Swapping sums the differences in another order (the pairs are sorted by
//!   their difference, which reverses), so it is held to the same rule, not
//!   to bits. For p, t's two gates are carried into p through Student's t
//!   density, 2 · f(|t|) · (sum of t's gates), plus 1e-15. df: EXACT.
//! * Adding c to both paired columns: EXACT for every statistic of the
//!   differences (on the grid, (x + c) − (y + c) is x − y exactly, and the
//!   pairs sort the same); each column's mean and sd within the shift rule.
//! * The paired test on two columns and the one-sample test, test value 0, on
//!   a column of their differences: EXACT (the same values, in the same
//!   order, through the same arithmetic).
//! * A test value equal to the computed mean: t = 0 and p = 1, EXACT.
//! * Permuting the rows: EXACT, equal canonical bytes. p in [0, 1] and the
//!   interval contains the mean difference: EXACT.
//!
//! Values, or differences, that are all equal are refused, never computed;
//! the generators keep that case and check its code.

use proptest::prelude::*;
use proptest::test_runner::{Config as ProptestConfig, RngAlgorithm, RngSeed};
use statrs::distribution::{Continuous, StudentsT};

use gaply_core::stats_desc::Alias;
use gaply_core::stats_l1::dataset::{
    build_dataset, ColumnAlias, DataFingerprint, Dataset, RawTable, TableRef,
};
use gaply_core::stats_onesample::{
    one_sample_compute, paired_compute, OneSampleCode, OneSampleParams, OneSampleResult,
    PairedParams, PairedResult,
};

const EPS: f64 = 1e-12;
const GRID: f64 = 128.0;
const SEED: u64 = 20_261_026;

fn config() -> ProptestConfig {
    ProptestConfig {
        cases: 512,
        failure_persistence: None,
        rng_algorithm: RngAlgorithm::ChaCha,
        rng_seed: RngSeed::Fixed(SEED),
        ..ProptestConfig::default()
    }
}

/// A dataset of numeric columns, each cell as text a file would hold.
fn dataset(columns: &[&[f64]]) -> Dataset {
    let headers: Vec<String> = (1..=columns.len()).map(|i| format!("c{i}")).collect();
    let rows: Vec<Vec<String>> = (0..columns[0].len())
        .map(|r| columns.iter().map(|c| format!("{}", c[r])).collect())
        .collect();
    build_dataset(
        RawTable::from_rows(headers, rows),
        DataFingerprint::of_bytes(b"synthetic", "proptest", TableRef::FirstTable),
    )
    .expect("dataset")
}

fn one(values: &[f64], test_value: f64) -> Result<OneSampleResult, OneSampleCode> {
    one_sample_compute(
        &dataset(&[values]),
        &OneSampleParams {
            outcome: Alias(ColumnAlias(1)),
            test_value: Some(test_value),
            missing_codes: vec![],
        },
    )
    .map_err(|d| d.code)
}

fn paired(first: &[f64], second: &[f64]) -> Result<PairedResult, OneSampleCode> {
    paired_compute(
        &dataset(&[first, second]),
        &PairedParams {
            first: Alias(ColumnAlias(1)),
            second: Alias(ColumnAlias(2)),
            missing_codes: vec![],
        },
    )
    .map_err(|d| d.code)
}

fn grid(j: &[i64]) -> Vec<f64> {
    j.iter().map(|v| *v as f64 / GRID).collect()
}

fn sample() -> impl Strategy<Value = Vec<i64>> {
    prop::collection::vec(-128_000_000i64..=128_000_000, 2..=40)
}

fn pairs() -> impl Strategy<Value = Vec<(i64, i64)>> {
    prop::collection::vec(
        (-128_000_000i64..=128_000_000, -128_000_000i64..=128_000_000),
        2..=40,
    )
}

fn shift() -> impl Strategy<Value = i64> {
    -128_000_000i64..=128_000_000
}

fn scale() -> impl Strategy<Value = f64> {
    (33i64..=32_768_000).prop_map(|m| m as f64 / 32_768.0)
}

/// |a − b| within the sum of the two sides' gates, plus one ulp of b.
fn close(a: f64, b: f64, gate_a: f64, gate_b: f64) -> bool {
    (a - b).abs() <= gate_a + gate_b + f64::EPSILON * b.abs()
}

/// t's gate on one side: ε · max(|t|, M / se).
fn t_gate(t: f64, m: f64, se: f64) -> f64 {
    EPS * t.abs().max(m / se)
}

/// p's allowance between two sides: t's two gates carried through f.
fn p_allowed(t: f64, df: f64, t_gates: f64) -> f64 {
    let f = StudentsT::new(0.0, 1.0, df).expect("df").pdf(t.abs());
    2.0 * f * t_gates + 1e-15
}

/// The one-sample statistics, each with its gate's scale on its side.
fn one_side(r: &OneSampleResult) -> (f64, [(f64, f64); 6]) {
    let m = r.mean.abs() + r.test_value.abs();
    let width = r.ci_upper - r.ci_lower;
    (
        m,
        [
            (r.t, t_gate(r.t, m, r.standard_error)),
            (r.standard_error, EPS * r.standard_error),
            (r.cohens_d, EPS * r.cohens_d.abs().max(m / r.sd)),
            (r.mean_difference, EPS * m.max(r.standard_error)),
            (r.sd, EPS * r.sd.max(r.mean.abs())),
            (
                width,
                EPS * (r.ci_lower.abs() + r.ci_upper.abs() + 2.0 * m + 2.0 * r.standard_error),
            ),
        ],
    )
}

fn invariants(mean_difference: f64, ci: [f64; 2], p: f64) {
    assert!((0.0..=1.0).contains(&p), "p {p}");
    assert!(
        ci[0] <= mean_difference && mean_difference <= ci[1],
        "{ci:?} {mean_difference}"
    );
}

proptest! {
    #![proptest_config(config())]

    /// Shifting the values and the test value by c leaves t, p, d, the
    /// standard error and the interval's width unchanged, within the rule.
    #[test]
    fn one_sample_shift_leaves_t_p_d_se_and_the_width(j in sample(), mu in shift(), c in shift()) {
        let x = grid(&j);
        let (mu, c) = (mu as f64 / GRID, c as f64 / GRID);
        let shifted: Vec<f64> = x.iter().map(|v| v + c).collect();
        match (one(&x, mu), one(&shifted, mu + c)) {
            (Ok(a), Ok(b)) => {
                invariants(a.mean_difference, [a.ci_lower, a.ci_upper], a.p);
                prop_assert_eq!(a.df, b.df);
                let (_, sa) = one_side(&a);
                let (_, sb) = one_side(&b);
                // t, se, d and the width; the mean difference too, which is
                // the same quantity.
                for (k, name) in [(0, "t"), (1, "se"), (2, "d"), (3, "mean_difference"), (5, "width")] {
                    prop_assert!(close(sb[k].0, sa[k].0, sb[k].1, sa[k].1), "{} {} {}", name, sb[k].0, sa[k].0);
                }
                prop_assert!((b.p - a.p).abs() <= p_allowed(a.t, a.df, sa[0].1 + sb[0].1), "p {} {}", b.p, a.p);
            }
            (Err(e), Err(f)) => {
                prop_assert_eq!(e, OneSampleCode::ValuesConstant);
                prop_assert_eq!(f, OneSampleCode::ValuesConstant);
            }
            (a, b) => prop_assert!(false, "{:?} {:?}", a.map(|r| r.t), b.map(|r| r.t)),
        }
    }

    /// Scaling the values and the test value by k > 0 scales the mean
    /// difference, its standard error, its interval and the sd by k, and
    /// leaves t, p and d unchanged, within the rule.
    #[test]
    fn one_sample_scale_scales_the_differences_and_leaves_t_p_d(j in sample(), mu in shift(), k in scale()) {
        let x = grid(&j);
        let mu = mu as f64 / GRID;
        let scaled: Vec<f64> = x.iter().map(|v| v * k).collect();
        let (Ok(a), Ok(b)) = (one(&x, mu), one(&scaled, mu * k)) else {
            prop_assert_eq!(one(&x, mu).map(|_| ()), Err(OneSampleCode::ValuesConstant));
            return Ok(());
        };
        prop_assert_eq!(a.df, b.df);
        let (_, sa) = one_side(&a);
        let (_, sb) = one_side(&b);
        for (i, name) in [(0, "t"), (2, "d")] {
            prop_assert!(close(sb[i].0, sa[i].0, sb[i].1, sa[i].1), "{} {} {}", name, sb[i].0, sa[i].0);
        }
        for (i, name) in [(1, "se"), (3, "mean_difference"), (4, "sd"), (5, "width")] {
            prop_assert!(close(sb[i].0, sa[i].0 * k, sb[i].1, sa[i].1 * k), "{} {} {}", name, sb[i].0, sa[i].0 * k);
        }
        let mb = b.mean.abs() + b.test_value.abs();
        for (got, want) in [(b.ci_lower, a.ci_lower * k), (b.ci_upper, a.ci_upper * k)] {
            let gate = EPS * want.abs().max(mb).max(b.standard_error);
            prop_assert!(close(got, want, gate, gate), "ci {} {}", got, want);
        }
        prop_assert!((b.p - a.p).abs() <= p_allowed(a.t, a.df, sa[0].1 + sb[0].1));
    }

    /// A test value equal to the computed mean gives t = 0 and p = 1.
    #[test]
    fn one_sample_at_its_own_mean_gives_t_zero_and_p_one(j in sample()) {
        let x = grid(&j);
        if let Ok(a) = one(&x, 0.0) {
            let r = one(&x, a.mean).expect("computes");
            prop_assert_eq!(r.t.to_bits(), 0f64.to_bits());
            prop_assert_eq!(r.p, 1.0);
            prop_assert_eq!(r.mean_difference, 0.0);
        }
    }

    /// Permuting the rows changes no bit, of either test.
    #[test]
    fn permuting_the_rows_changes_no_bit(
        rows in pairs().prop_flat_map(|p| (Just(p.clone()), Just(p).prop_shuffle())),
        mu in shift(),
    ) {
        let (original, shuffled) = rows;
        let (a, b): (Vec<i64>, Vec<i64>) = original.iter().copied().unzip();
        let (sa, sb): (Vec<i64>, Vec<i64>) = shuffled.iter().copied().unzip();
        let mu = mu as f64 / GRID;
        let o1 = one(&grid(&a), mu).map(|r| r.canonical_bytes());
        let o2 = one(&grid(&sa), mu).map(|r| r.canonical_bytes());
        prop_assert_eq!(o1, o2);
        let p1 = paired(&grid(&a), &grid(&b)).map(|r| r.canonical_bytes());
        let p2 = paired(&grid(&sa), &grid(&sb)).map(|r| r.canonical_bytes());
        prop_assert_eq!(p1, p2);
    }

    /// Swapping first and second negates t, the mean difference and d_z,
    /// swaps and negates the interval's ends, and leaves p, df and the
    /// standard error unchanged, within the rule.
    #[test]
    fn swapping_first_and_second_negates_and_keeps_p(rows in pairs()) {
        let (a, b): (Vec<i64>, Vec<i64>) = rows.iter().copied().unzip();
        let (a, b) = (grid(&a), grid(&b));
        match (paired(&a, &b), paired(&b, &a)) {
            (Ok(r), Ok(s)) => {
                invariants(r.mean_difference, [r.ci_lower, r.ci_upper], r.p);
                prop_assert_eq!(r.df, s.df);
                let m = r.first.mean.abs() + r.second.mean.abs();
                let tg = t_gate(r.t, m, r.standard_error);
                prop_assert!(close(-s.t, r.t, tg, tg), "t {} {}", s.t, r.t);
                let mg = EPS * m.max(r.standard_error);
                prop_assert!(close(-s.mean_difference, r.mean_difference, mg, mg));
                let dg = EPS * r.cohens_dz.abs().max(m / r.sd_differences);
                prop_assert!(close(-s.cohens_dz, r.cohens_dz, dg, dg));
                let sg = EPS * r.standard_error;
                prop_assert!(close(s.standard_error, r.standard_error, sg, sg));
                for (got, want) in [(-s.ci_upper, r.ci_lower), (-s.ci_lower, r.ci_upper)] {
                    let g = EPS * want.abs().max(m).max(r.standard_error);
                    prop_assert!(close(got, want, g, g), "ci {} {}", got, want);
                }
                prop_assert!((s.p - r.p).abs() <= p_allowed(r.t, r.df, 2.0 * tg));
                // Each column changes place; its mean and sd are summed in the
                // other order, so they too are held to the rule, not to bits.
                for (got, want) in [(s.first, r.second), (s.second, r.first)] {
                    let g = EPS * want.mean.abs().max(want.sd);
                    prop_assert!(close(got.mean, want.mean, g, g), "mean {} {}", got.mean, want.mean);
                    prop_assert!(close(got.sd, want.sd, g, g), "sd {} {}", got.sd, want.sd);
                }
            }
            (Err(e), Err(f)) => {
                prop_assert_eq!(e, OneSampleCode::DifferencesConstant);
                prop_assert_eq!(f, OneSampleCode::DifferencesConstant);
            }
            (r, s) => prop_assert!(false, "{:?} {:?}", r.map(|r| r.t), s.map(|s| s.t)),
        }
    }

    /// Adding c to both columns leaves every statistic of the differences
    /// unchanged, bit for bit, and moves each column's mean by c.
    #[test]
    fn adding_c_to_both_columns_changes_no_statistic_of_the_differences(rows in pairs(), c in shift()) {
        let (a, b): (Vec<i64>, Vec<i64>) = rows.iter().copied().unzip();
        let (a, b) = (grid(&a), grid(&b));
        let c = c as f64 / GRID;
        let (ac, bc): (Vec<f64>, Vec<f64>) = (a.iter().map(|v| v + c).collect(), b.iter().map(|v| v + c).collect());
        match (paired(&a, &b), paired(&ac, &bc)) {
            (Ok(r), Ok(s)) => {
                for (got, want) in [
                    (s.t, r.t), (s.df, r.df), (s.p, r.p), (s.mean_difference, r.mean_difference),
                    (s.sd_differences, r.sd_differences), (s.standard_error, r.standard_error),
                    (s.ci_lower, r.ci_lower), (s.ci_upper, r.ci_upper), (s.cohens_dz, r.cohens_dz),
                ] {
                    prop_assert_eq!(got.to_bits(), want.to_bits());
                }
                for (got, want) in [(s.first, r.first), (s.second, r.second)] {
                    let g = |m: f64, sd: f64| EPS * m.abs().max(sd);
                    prop_assert!(close(got.mean, want.mean + c, g(got.mean, got.sd), g(want.mean + c, want.sd)));
                    prop_assert!(close(got.sd, want.sd, g(got.mean, got.sd), g(want.mean, want.sd)));
                }
            }
            (Err(e), Err(f)) => prop_assert_eq!(e, f),
            (r, s) => prop_assert!(false, "{:?} {:?}", r.map(|r| r.t), s.map(|s| s.t)),
        }
    }

    /// The paired test on two columns gives the same bits as the one-sample
    /// test, test value 0, on a column of their differences.
    #[test]
    fn the_paired_test_is_the_one_sample_test_of_the_differences(rows in pairs()) {
        let (a, b): (Vec<i64>, Vec<i64>) = rows.iter().copied().unzip();
        let (a, b) = (grid(&a), grid(&b));
        let d: Vec<f64> = a.iter().zip(&b).map(|(x, y)| x - y).collect();
        match (paired(&a, &b), one(&d, 0.0)) {
            (Ok(p), Ok(o)) => {
                for (got, want) in [
                    (p.t, o.t), (p.df, o.df), (p.p, o.p), (p.mean_difference, o.mean_difference),
                    (p.standard_error, o.standard_error), (p.ci_lower, o.ci_lower),
                    (p.ci_upper, o.ci_upper), (p.cohens_dz, o.cohens_d), (p.sd_differences, o.sd),
                ] {
                    prop_assert_eq!(got.to_bits(), want.to_bits());
                }
                prop_assert_eq!(p.n_pairs, o.n);
            }
            (Err(e), Err(f)) => {
                prop_assert_eq!(e, OneSampleCode::DifferencesConstant);
                prop_assert_eq!(f, OneSampleCode::ValuesConstant);
            }
            (p, o) => prop_assert!(false, "{:?} {:?}", p.map(|r| r.t), o.map(|r| r.t)),
        }
    }
}
