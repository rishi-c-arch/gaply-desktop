//! **R's "data are essentially constant" rule in the three analysis t-tests
//! (Research OS, Phase 8D-1b, decision 259).** A standard error below
//! 10 · ε · scale is rounding, not spread, and t built on it means nothing:
//! before this rule Gaply reported t near 1e16 and p = 0 for such data. Each
//! refusal case here was run first against `04d8705`'s code, where it
//! computed (the t is in the phase report), and is refused with the new code
//! after.
//!
//! The scales: one-sample |mean|; independent max(|mean 1|, |mean 2|), both
//! as R's `t.test.default`; paired max(|mean of the differences|, mean of
//! |first|, mean of |second|), the last two Gaply's own, because the
//! rounding in first minus second grows with the size of the values.
//!
//! Each method also has a case at least ten times above its threshold that
//! computes before and after with the same bytes, pinned on macOS aarch64,
//! where they were captured (the bytes hold p, which goes through the
//! platform's maths library); elsewhere only the refusal and the ratio are
//! asserted.
//!
//! The codes are compared by their wire words so that this file compiled,
//! and ran red, before the codes existed.

use gaply_core::contracts::codes::Closed;
use gaply_core::contracts::ContentHash;
use gaply_core::stats_desc::Alias;
use gaply_core::stats_l1::dataset::{
    build_dataset, ColumnAlias, DataFingerprint, Dataset, RawTable, TableRef,
};
use gaply_core::stats_onesample::{
    one_sample_compute, paired_compute, OneSampleParams, PairedParams,
};
use gaply_core::stats_ttest::{compute, TTestParams, Variant};

const GROUPS: &str = "stats.ttest.groups_essentially_constant";
const VALUES: &str = "stats.onesample.values_essentially_constant";
const DIFFERENCES: &str = "stats.onesample.differences_essentially_constant";

/// R's factor: 10 · `.Machine$double.eps`.
const FACTOR: f64 = 10.0 * f64::EPSILON;

fn table(headers: &[&str], rows: Vec<Vec<String>>) -> Dataset {
    build_dataset(
        RawTable::from_rows(headers.iter().map(|h| h.to_string()).collect(), rows),
        DataFingerprint::of_bytes(b"8d1b essentially constant", "test", TableRef::FirstTable),
    )
    .expect("dataset")
}

fn two_groups(a: &[&str], b: &[&str]) -> Dataset {
    let rows = a
        .iter()
        .map(|v| vec![v.to_string(), "a".into()])
        .chain(b.iter().map(|v| vec![v.to_string(), "b".into()]))
        .collect();
    table(&["y", "g"], rows)
}

fn one_column(x: &[&str]) -> Dataset {
    table(&["x"], x.iter().map(|v| vec![v.to_string()]).collect())
}

fn two_columns(first: &[&str], second: &[&str]) -> Dataset {
    let rows = first
        .iter()
        .zip(second)
        .map(|(a, b)| vec![a.to_string(), b.to_string()])
        .collect();
    table(&["a", "b"], rows)
}

fn ttest_params(variant: Variant) -> TTestParams {
    TTestParams {
        outcome: Alias(ColumnAlias(1)),
        group: Alias(ColumnAlias(2)),
        levels: None,
        variant: Some(variant),
        missing_codes: vec![],
    }
}

fn one_params(test_value: f64) -> OneSampleParams {
    OneSampleParams {
        outcome: Alias(ColumnAlias(1)),
        test_value: Some(test_value),
        missing_codes: vec![],
    }
}

fn paired_params() -> PairedParams {
    PairedParams {
        first: Alias(ColumnAlias(1)),
        second: Alias(ColumnAlias(2)),
        missing_codes: vec![],
    }
}

fn mean(xs: &[f64]) -> f64 {
    xs.iter().sum::<f64>() / xs.len() as f64
}

fn parse(xs: &[&str]) -> Vec<f64> {
    xs.iter().map(|s| s.parse().expect("number")).collect()
}

/// The independent t-test on `a` and `b` under Student and Welch: each
/// refused with the new code. A result is the defect this rule closes; both
/// variants run before the test fails, and each computed t is printed.
fn independent_refused(a: &[&str], b: &[&str]) {
    let mut wrong = Vec::new();
    for variant in [Variant::Student, Variant::Welch] {
        match compute(&two_groups(a, b), &ttest_params(variant)) {
            Ok(r) => wrong.push(format!(
                "{variant:?}: computed t {:e}, se {:e}, p {:e}",
                r.t, r.standard_error, r.p
            )),
            Err(e) if e.code.wire() == GROUPS => {}
            Err(e) => wrong.push(format!("{variant:?}: refused {}", e.code.wire())),
        }
    }
    assert!(wrong.is_empty(), "expected {GROUPS}: {wrong:#?}");
}

/* ---- refused: below the threshold ---------------------------------------- */

/// Group A 1.0, 1.0000000000000002, 1.0 and group B 2.0, 2.0000000000000004,
/// 2.0: each group's spread is one ulp. Before the rule t ≈ −4.93e15.
#[test]
fn groups_one_ulp_apart_are_essentially_constant_under_student_and_welch() {
    let a = ["1.0", "1.0000000000000002", "1.0"];
    let b = ["2.0", "2.0000000000000004", "2.0"];
    independent_refused(&a, &b);
}

/// The scale is the LARGER mean, as R's: group A near 1 and group B near
/// 1000, each one ulp apart. se ≈ 4.64e-14; the threshold at max(|m1|, |m2|)
/// is ≈ 2.22e-12 and refuses; at |m1| alone it would be ≈ 2.22e-15 and
/// compute. Rust-only: R's reader turns 1000.0000000000001 into 1000.
#[test]
fn the_independent_scale_is_the_larger_of_the_two_means() {
    let a = ["1.0", "1.0000000000000002", "1.0"];
    let b = ["1000.0", "1000.0000000000001", "1000.0"];
    independent_refused(&a, &b);
}

/// 0.3, 0.30000000000000004, 0.3, 0.3 against 0: before the rule t ≈ 1.87e16.
#[test]
fn values_one_ulp_apart_are_essentially_constant() {
    let d = one_column(&["0.3", "0.30000000000000004", "0.3", "0.3"]);
    match one_sample_compute(&d, &one_params(0.0)) {
        Ok(r) => panic!(
            "computed t {:e}, se {:e}, p {:e}; expected {VALUES}",
            r.t, r.standard_error, r.p
        ),
        Err(e) => assert_eq!(e.code.wire(), VALUES),
    }
}

fn paired_refused(first: &[&str], second: &[&str]) {
    match paired_compute(&two_columns(first, second), &paired_params()) {
        Ok(r) => panic!(
            "computed t {:e}, se {:e}, p {:e}; expected {DIFFERENCES}",
            r.t, r.standard_error, r.p
        ),
        Err(e) => assert_eq!(e.code.wire(), DIFFERENCES),
    }
}

/// b = a − 2 on a 3-decimal grid (8D-1's open question 1): the differences
/// miss 2 by an ulp on some rows. R stops here too, on |mean of the
/// differences| alone. Before the rule t ≈ 1.42e16.
#[test]
fn a_second_column_that_is_the_first_minus_two_is_essentially_constant() {
    paired_refused(
        &["5.123", "4.871", "6.042", "3.987", "5.555"],
        &["3.123", "2.871", "4.042", "1.987", "3.555"],
    );
}

/// Every pair differs by 0.1, with values near 100: the rounding in first
/// minus second (~1e-14) is above R's threshold at |mean of the differences|
/// = 0.1 (2.2e-16), so R computes t ≈ 2.9e13. Gaply's scale, the mean of
/// |first| and of |second| (~100), refuses it (decision 259). Before the rule
/// se ≈ 3.48e-15 and t ≈ 2.9e13.
#[test]
fn pairs_that_all_differ_by_a_tenth_are_essentially_constant_where_r_computes() {
    paired_refused(
        &["100.2", "101.7", "99.3", "102.4", "98.6"],
        &["100.1", "101.6", "99.2", "102.3", "98.5"],
    );
}

/* ---- computed: at least ten times above the threshold ---------------------- */

/// Captured on `04d8705`'s code, macOS aarch64, 26 Oct 2026.
const STUDENT_ABOVE: &str = "1161e828cf8e654213916319f4b5b7257290609f99d3d737171e1a32057e0372";
const WELCH_ABOVE: &str = "bc87103c6effa6c363a01023facf0ccbe1ac2f7883959634d0bb9f722ef4eccb";
const ONE_SAMPLE_ABOVE: &str = "45ee7e82caf25564b9ad136f83a5e11e6dac4752d2e89757d715a67d4b7f169c";
const PAIRED_ABOVE: &str = "e60fc218e3758eda4b2c2574feff1a85fc1bd0ff9953dd71f1d180e2c3f7603b";

fn pinned(name: &str, bytes: &[u8], want: &str) {
    let got = ContentHash::of_bytes(bytes);
    println!("{name}: {}", got.as_hex());
    if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
        assert_eq!(got.as_hex(), want, "{name}: the bytes changed");
    }
}

fn above(name: &str, se: f64, scale: f64) {
    let ratio = se / (FACTOR * scale);
    println!(
        "{name}: se {se:e}, threshold {:e}, ratio {ratio:.1}",
        FACTOR * scale
    );
    assert!(ratio >= 10.0, "{name}: {ratio} times the threshold");
}

/// A 1.0, 1.0000000000001, 1.0000000000002 and B 2.0, 2.0000000000001,
/// 2.0000000000002: se ≈ 8.2e-14, about 18 times the threshold at 2.
#[test]
fn groups_ten_times_above_the_threshold_compute_with_the_same_bytes() {
    let a = ["1.0", "1.0000000000001", "1.0000000000002"];
    let b = ["2.0", "2.0000000000001", "2.0000000000002"];
    let scale = mean(&parse(&a)).abs().max(mean(&parse(&b)).abs());
    for (variant, want) in [
        (Variant::Student, STUDENT_ABOVE),
        (Variant::Welch, WELCH_ABOVE),
    ] {
        let r = compute(&two_groups(&a, &b), &ttest_params(variant))
            .unwrap_or_else(|e| panic!("{variant:?}: {e:?}"));
        above(&format!("{variant:?}"), r.standard_error, scale);
        pinned(&format!("{variant:?}"), &r.canonical_bytes(), want);
    }
}

/// 0.3, 0.3000000000001, 0.3000000000002, 0.3 against 0: se ≈ 4.8e-14, about
/// 72 times the threshold at 0.3.
#[test]
fn values_ten_times_above_the_threshold_compute_with_the_same_bytes() {
    let x = ["0.3", "0.3000000000001", "0.3000000000002", "0.3"];
    let r = one_sample_compute(&one_column(&x), &one_params(0.0)).expect("computes");
    above("one-sample", r.standard_error, mean(&parse(&x)).abs());
    pinned("one-sample", &r.canonical_bytes(), ONE_SAMPLE_ABOVE);
}

/// Values near 100 whose differences are 0.1 + 0, 1, 2, 3, 4 × 1e-11: se ≈
/// 7e-12, about 30 times the threshold at Gaply's own, larger scale (~100).
#[test]
fn pairs_ten_times_above_their_own_threshold_compute_with_the_same_bytes() {
    let first = ["100.2", "101.7", "99.3", "102.4", "98.6"];
    let second = [
        "100.1",
        "101.59999999999",
        "99.19999999998",
        "102.29999999997",
        "98.49999999996",
    ];
    let r = paired_compute(&two_columns(&first, &second), &paired_params()).expect("computes");
    let (f, s) = (parse(&first), parse(&second));
    let d: Vec<f64> = f.iter().zip(&s).map(|(x, y)| x - y).collect();
    let abs = |v: &[f64]| mean(&v.iter().map(|x| x.abs()).collect::<Vec<_>>());
    let scale = mean(&d).abs().max(abs(&f)).max(abs(&s));
    above("paired", r.standard_error, scale);
    pinned("paired", &r.canonical_bytes(), PAIRED_ABOVE);
}
