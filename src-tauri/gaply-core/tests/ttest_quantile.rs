//! The t-test's 95% quantile against R's `qt(0.975, df)`
//! (`tests/fixtures/stats_ttest/quantiles.csv`, written with
//! `/usr/local/bin/Rscript make_ttest_fixtures.R`), over 33 df from 1 to 1e7,
//! integer and fractional, as Welch's df is.
//!
//! `t_quantile_975` lives in `stats_dist` since Research OS Phase 8E
//! (decision 275), re-exported by `stats_ttest`. At df ≤ 1e4 it inverts
//! `stats_l0::pvalue::t_two_tailed` by bisection on [0, 1e3] (decision 62's
//! fallback; statrs's `StudentsT::inverse_cdf` was measured here first, on
//! these 33 rows, and missed the gate: 6.1e-13 at worst up to df 4,998, then
//! 3.5e-6 at df 1e5, 6.1e-5 at 1e6 and 6.8e-2 at 1e7, Phase 4B-1 report).
//! Above 1e4 it is Abramowitz and Stegun 26.7.5 about the normal quantile:
//! the bisection inherited `pvalue::t_two_tailed`'s own error at large df
//! (measured 1.8e-11 at 1e5, 8.8e-11 at 1e6, 3.6e-9 at 1e7) and was refused
//! there until 8E. THE GATE: |gaply − R| ≤ 1e-12 · R, for every df up to
//! `QUANTILE_MAX_DF` (1e7, `stats_dist::MEASURED_MAX_DF`); above it the
//! quantile is `None`, which the range test below requires. The ingest's
//! 5,000 rows give at most df 4,998. The bisection's bits at the 30 df up to
//! 1e4 are pinned below, on every platform. It prints every row and the
//! largest measured ratio:
//!
//! ```text
//! cargo test -p gaply_core --test ttest_quantile -- --nocapture
//! ```

use gaply_core::stats_ttest::{t_quantile_975, QUANTILE_MAX_DF};

const PROVENANCE: &str = "/usr/local/bin/Rscript make_ttest_fixtures.R";
const GATE: f64 = 1e-12;

#[test]
fn the_quantile_matches_r_qt_within_the_gate() {
    let text = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/stats_ttest/quantiles.csv"
    ))
    .expect("quantiles.csv is present");
    assert!(text
        .lines()
        .take_while(|l| l.starts_with('#'))
        .any(|h| h.contains(PROVENANCE)));
    let mut lines = text.lines().filter(|l| !l.starts_with('#'));
    assert_eq!(lines.next(), Some("df,q"));
    let mut worst = (0.0f64, 0.0f64);
    let mut over = Vec::new();
    let mut refused = 0;
    let mut checked = 0;
    for line in lines {
        let (df, q) = line.split_once(',').expect("two fields");
        let (df, want): (f64, f64) = (df.parse().unwrap(), q.parse().unwrap());
        if df > QUANTILE_MAX_DF {
            assert_eq!(
                t_quantile_975(df),
                None,
                "df {df}: above the measured range"
            );
            println!("df {df:>10}  refused (above {QUANTILE_MAX_DF})  R {want:.17e}");
            refused += 1;
            continue;
        }
        let got = t_quantile_975(df).unwrap_or_else(|| panic!("df {df}: no quantile"));
        let ratio = (got - want).abs() / want;
        println!("df {df:>10}  gaply {got:.17e}  R {want:.17e}  relative {ratio:.3e}");
        if ratio > GATE {
            over.push(format!("df {df}: relative {ratio:e}"));
        }
        if ratio >= worst.0 {
            worst = (ratio, df);
        }
        checked += 1;
    }
    println!(
        "{checked} quantiles; the largest |gaply - R| / R is {:.3e} (df {})",
        worst.0, worst.1
    );
    assert_eq!(
        (checked, refused),
        (33, 0),
        "33 df gated, none above the range (1e7)"
    );
    assert!(
        over.is_empty(),
        "over the gate {GATE:e}:\n{}",
        over.join("\n")
    );
}

#[test]
fn a_df_that_defines_no_distribution_has_no_quantile() {
    // The double just above `QUANTILE_MAX_DF` (1e7) is the first refused.
    for df in [
        0.0,
        -1.0,
        f64::NAN,
        f64::INFINITY,
        f64::from_bits(1e7f64.to_bits() + 1),
    ] {
        assert!(t_quantile_975(df).is_none(), "df {df}");
    }
}

/// The bits of the bisection at every df ≤ 1e4 of `quantiles.csv`, captured
/// on `3d1253e`'s code (macOS aarch64, 30 Oct 2026, Research OS Phase 8E,
/// decision 277) before `t_quantile_975` moved into `stats_dist`, and equal
/// to the reviewer's capture of the same code on Linux x86_64. The bisection
/// is sums, halvings and comparisons over `stats_l0::pvalue::t_two_tailed`,
/// so it is asserted on every platform. `f64` keys, so a fractional df
/// (1.5, 3.7, 12.25, 17.3456, 150.5) is matched as the file writes it.
const QUANTILE_BITS: [(f64, u64); 30] = [
    (1.0, 4623342575419149596),
    (1.5, 4618460178856166761),
    (2.0, 4616530374734988991),
    (2.5, 4615231825907230376),
    (3.0, 4614348650797318560),
    (3.7, 4613639501394209115),
    (4.0, 4613434417370609123),
    (5.0, 4612970854498565947),
    (6.0, 4612692374450529869),
    (7.0, 4612507079249189659),
    (8.0, 4612375078482027572),
    (9.0, 4612276343877733153),
    (10.0, 4612199741451784961),
    (12.25, 4612077587023370573),
    (15.0, 4611982016489588377),
    (17.3456, 4611926100636587174),
    (20.0, 4611879590901924976),
    (25.0, 4611820087329384907),
    (30.0, 4611781207536611213),
    (40.0, 4611733475987352907),
    (50.0, 4611705291834421867),
    (75.0, 4611650449691095198),
    (100.0, 4611613832564182859),
    (150.5, 4611577265274807533),
    (200.0, 4611559450270617660),
    (500.0, 4611527130700552793),
    (1000.0, 4611516408725831409),
    (2500.0, 4611509987781572529),
    (4998.0, 4611507850359042474),
    (10000.0, 4611506780746194082),
];

#[test]
fn the_quantile_keeps_its_bits_to_df_1e4() {
    let text = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/stats_ttest/quantiles.csv"
    ))
    .expect("quantiles.csv is present");
    let mut checked = 0;
    let mut wrong = Vec::new();
    for line in text.lines().filter(|l| !l.starts_with('#')).skip(1) {
        let (df, _) = line.split_once(',').expect("two fields");
        let df: f64 = df.parse().unwrap();
        if df > 1e4 {
            continue;
        }
        let got = t_quantile_975(df)
            .unwrap_or_else(|| panic!("df {df}: no quantile"))
            .to_bits();
        let (_, want) = QUANTILE_BITS
            .iter()
            .find(|(d, _)| *d == df)
            .unwrap_or_else(|| panic!("df {df}: not in the pinned table"));
        println!(
            "df {df:>8}  bits {got}  pinned {want}  {}",
            if got == *want { "equal" } else { "DIFFERENT" }
        );
        if got != *want {
            wrong.push(format!("df {df}: this platform {got}, pinned {want}"));
        }
        checked += 1;
    }
    assert_eq!(checked, 30, "every df at or below 1e4 in quantiles.csv");
    assert!(
        wrong.is_empty(),
        "the bisection's bits changed:\n{}",
        wrong.join("\n")
    );
}
