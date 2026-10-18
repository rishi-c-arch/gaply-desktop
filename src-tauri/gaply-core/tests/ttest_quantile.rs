//! The t-test's 95% quantile against R's `qt(0.975, df)`
//! (`tests/fixtures/stats_ttest/quantiles.csv`, written with
//! `/usr/local/bin/Rscript make_ttest_fixtures.R`), over 33 df from 1 to 1e7,
//! integer and fractional, as Welch's df is.
//!
//! The quantile inverts `stats_l0::pvalue::t_two_tailed` by bisection on
//! [0, 1e3] (decision 62's fallback). statrs's `StudentsT::inverse_cdf` was
//! measured here first, on these 33 rows, and missed the gate: 6.1e-13 at
//! worst up to df 4,998, then 3.5e-6 at df 1e5, 6.1e-5 at 1e6 and 6.8e-2 at
//! 1e7 (Phase 4B-1 report). THE GATE: |gaply − R| ≤ 1e-12 · R, for every df
//! up to `QUANTILE_MAX_DF` (1e4). Above it the bisection inherits
//! `pvalue::t_two_tailed`'s own error at large df (measured 1.8e-11 at 1e5,
//! 8.8e-11 at 1e6, 3.6e-9 at 1e7), so the quantile is refused there, and this
//! test requires the refusal. The ingest's 5,000 rows give at most df 4,998.
//! It prints every row and the largest measured ratio:
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
        (30, 3),
        "30 df gated, 3 above the range refused"
    );
    assert!(
        over.is_empty(),
        "over the gate {GATE:e}:\n{}",
        over.join("\n")
    );
}

#[test]
fn a_df_that_defines_no_distribution_has_no_quantile() {
    for df in [0.0, -1.0, f64::NAN, f64::INFINITY, 1e5] {
        assert!(t_quantile_975(df).is_none(), "df {df}");
    }
}
