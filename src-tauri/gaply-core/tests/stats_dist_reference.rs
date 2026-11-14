//! **`gaply_core::stats_dist` against R 4.6.1 (Research OS, Phase 8E,
//! decision 275), over `tests/fixtures/stats_dist/` (written with
//! `/usr/local/bin/Rscript` by `make_dist_fixtures.R`; every statistic placed
//! at a target p through R's quantile function, and R's p computed from that
//! statistic, as 8A's references were).**
//!
//! THE GATES, each stated with its measurement:
//!
//! | function | rows | gate | and |
//! |---|---|---|---|
//! | `cornish_fisher_975` | 20 df from 1e3 to 1e7 | 1e-14 · R's `qt` (R's own error is about 5e-16; df 1e3, 2,500 and 4,998 are the coefficient check, where the last terms are largest) | |
//! | `t_quantile_975` | the same 20 | 1e-12 · R at every row | at df 1e4 the bits of `ttest_quantile.rs`'s pin |
//! | `normal_quantile_975` | 1 | 1e-15 · R's `qnorm(0.975)` | the distance in ulps printed |
//! | `t_two_tailed` | 338 (13 df × 26 t) | 1e-9 · R's `pt` | at df 1e4 bit for bit `stats_l0`'s; `stats_l0`'s own error printed on every row above 1e4, ungated |
//! | `f_upper`, df1 = 1 | 144 (16 df2 × 9 F) | 1e-9 · R's `pf` | at df2 ≤ 1e4 bit for bit `stats_l0`'s; `stats_l0`'s own error printed on every row, ungated above 1e4 (8A's unmeasured stretch from df2 5e3 to 5e4) |
//! | `f_upper`, df1 = 2 and 3 | 54 | bit for bit `stats_l0`'s | its error against R printed, ungated (decision 275) |
//! | `chi_square_upper` (8X-1) | 316 (12 df to 2,401), `chi_p.csv` | 2e-11 · R's `pchisq`, ten times 1.812e-12 measured | bit for bit `stats_l0`'s at every row |
//! | `fisher_2x2` (8X-1) | 197 tables × 3 p, `fisher_p.csv` | 4e-10 · R's `fisher.test`, ten times 3.351e-11 measured | a zero margin gives 1 three times |
//! | `rank_sum_exact_tails` (8Y-1) | 1,063 (n1 and n2 among 1 to 49, u), `wilcox_p.csv` | 2e-13 · R's `pwilcox`, both tails, ten times 1.310e-14 measured | each tail its own sum; `None` above 49 |
//! | `signed_rank_exact_tails` (8Z-1) | 2,010 (n 1 to 49, v), `signrank_p.csv` | 3.7e-14 · R's `psignrank`, both tails, ten times 3.650e-15 measured | `None` above 49, and for v above the top |
//! | `f_upper`, the regression (9A-1) | 1,200 (df1 1 to 50 × df2 1 to 1e5 × 10 tails to 1e-100), `f_upper_regression.tsv` | 1.1e-9 · R's `pf`, ten times 1.096e-10 measured | decision 316's stop, 1e-8, asserted too |
//!
//! The two 8X-1 files are written by a second script,
//! `make_xtab_dist_fixtures.R` (decision 576), and 8Y-1's by a third,
//! `make_ranks_dist_fixtures.R` (decision 603), so the files before them stay
//! byte for byte; a row whose R p is below the smallest normal double is
//! not compared, and gaply's must be below it too.
//!
//! Then the range (`None` above `MEASURED_MAX_DF`, at a df that defines no
//! distribution, and for a statistic that is not a number; 0 for an
//! overflowing t; 1 for F = 0) and the properties (p even in t and 1 at 0;
//! p in [0, 1] and not increasing in |t|; p at the quantile 0.05 within
//! 1e-9; the quantile not increasing in df across the switch at 1e4).
//!
//! A t row at df 1e4, or a df1 = 1 row at df2 ≤ 1e4, over its gate is
//! `stats_l0`'s engine over its gate where 8A measured it inside: that stops
//! the step, and the switch does not move without the user. Every row is
//! printed, and the largest error per function:
//!
//! ```text
//! cargo test -p gaply_core --test stats_dist_reference -- --nocapture
//! ```

use gaply_core::stats_dist::{
    chi_square_upper, cornish_fisher_975, f_upper, fisher_2x2, fisher_two_sided_sum,
    normal_quantile_975, t_quantile_975, t_two_tailed, CHI_SQUARE_MAX_DF, FISHER_TIE_TOLERANCE,
    LARGE_DF, MEASURED_MAX_DF,
};
use gaply_core::stats_dist::{rank_sum_exact_tails, RANK_SUM_EXACT_MAX};
use gaply_core::stats_dist::{signed_rank_exact_tails, SIGNED_RANK_EXACT_MAX};
use gaply_core::stats_l0::pvalue;

const DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/stats_dist");
const PROVENANCE: &str = "/usr/local/bin/Rscript make_dist_fixtures.R";
/// The second script, for crosstabs' two distributions (Phase 8X-1, decision
/// 576): `chi_p.csv` and `fisher_p.csv`.
const PROVENANCE_XTAB: &str = "/usr/local/bin/Rscript make_xtab_dist_fixtures.R";
const R_VERSION: &str = "R version 4.6.1";

/// The bits `ttest_quantile.rs` pins for the bisection at df 1e4.
const BISECTION_BITS_AT_1E4: u64 = 4611506780746194082;

const QUANTILE_GATE: f64 = 1e-12;
const EXPANSION_GATE: f64 = 1e-14;
const NORMAL_GATE: f64 = 1e-15;
const P_GATE: f64 = 1e-9;

/// The rows of a fixture, after its provenance header (asserted) and its
/// column header (asserted), each field parsed as a double.
fn rows(name: &str, header: &str) -> Vec<Vec<f64>> {
    rows_by(name, header, PROVENANCE)
}

/// [`rows`] for a file written by another script, named by `provenance`.
fn rows_by(name: &str, header: &str, provenance_of: &str) -> Vec<Vec<f64>> {
    let text =
        std::fs::read_to_string(format!("{DIR}/{name}")).unwrap_or_else(|e| panic!("{name}: {e}"));
    let provenance: Vec<&str> = text.lines().take_while(|l| l.starts_with('#')).collect();
    assert!(
        provenance.iter().any(|h| h.contains(provenance_of)),
        "{name}: not written by {provenance_of}"
    );
    assert!(
        provenance.iter().any(|h| h.contains(R_VERSION)),
        "{name}: not written with {R_VERSION}"
    );
    let mut lines = text.lines().filter(|l| !l.starts_with('#'));
    assert_eq!(lines.next(), Some(header), "{name}: the column header");
    lines
        .map(|l| {
            l.split(',')
                .map(|f| f.parse().unwrap_or_else(|e| panic!("{name}: `{f}`: {e}")))
                .collect()
        })
        .collect()
}

fn relative(got: f64, want: f64) -> f64 {
    if want == 0.0 {
        got.abs()
    } else {
        (got - want).abs() / want.abs()
    }
}

/// The largest error seen, with the row it came from.
#[derive(Default)]
struct Worst(f64, String);

impl Worst {
    fn see(&mut self, ratio: f64, row: String) {
        if ratio >= self.0 {
            *self = Worst(ratio, row);
        }
    }
}

fn just_above(x: f64) -> f64 {
    f64::from_bits(x.to_bits() + 1)
}

fn just_below(x: f64) -> f64 {
    f64::from_bits(x.to_bits() - 1)
}

#[test]
fn cornish_fisher_975_is_within_1e_14_of_r_qt_from_df_1e3() {
    let rows = rows("t_quantile.csv", "df,q");
    let mut worst = Worst::default();
    let mut over = Vec::new();
    for r in &rows {
        let (df, want) = (r[0], r[1]);
        let got = cornish_fisher_975(df).unwrap_or_else(|| panic!("df {df}: no expansion"));
        let ratio = relative(got, want);
        println!(
            "cornish_fisher df {df:>10}  gaply {got:.17e}  R {want:.17e}  relative {ratio:.3e}"
        );
        if ratio > EXPANSION_GATE {
            over.push(format!("df {df}: relative {ratio:e}"));
        }
        worst.see(ratio, format!("df {df}"));
    }
    println!(
        "cornish_fisher_975: {} rows; the largest |gaply - R| / R is {:.3e} ({})",
        rows.len(),
        worst.0,
        worst.1
    );
    assert_eq!(rows.len(), 20, "t_quantile.csv rows");
    assert!(
        over.is_empty(),
        "over the gate {EXPANSION_GATE:e}:\n{}",
        over.join("\n")
    );
}

#[test]
fn t_quantile_975_is_within_1e_12_of_r_qt_and_keeps_the_bisections_bits_at_1e4() {
    let rows = rows("t_quantile.csv", "df,q");
    let mut worst = Worst::default();
    let mut over = Vec::new();
    let mut at_1e4 = 0;
    for r in &rows {
        let (df, want) = (r[0], r[1]);
        let got = t_quantile_975(df).unwrap_or_else(|| panic!("df {df}: no quantile"));
        let ratio = relative(got, want);
        let route = if df > LARGE_DF {
            "expansion"
        } else {
            "bisection"
        };
        println!("t_quantile df {df:>10}  {route:<9}  gaply {got:.17e}  R {want:.17e}  relative {ratio:.3e}");
        if ratio > QUANTILE_GATE {
            over.push(format!("df {df}: relative {ratio:e}"));
        }
        worst.see(ratio, format!("df {df}"));
        if df == 1e4 {
            assert_eq!(
                got.to_bits(),
                BISECTION_BITS_AT_1E4,
                "df 1e4: the bisection's pinned bits"
            );
            at_1e4 += 1;
        }
    }
    println!(
        "t_quantile_975: {} rows; the largest |gaply - R| / R is {:.3e} ({})",
        rows.len(),
        worst.0,
        worst.1
    );
    assert_eq!(
        (rows.len(), at_1e4),
        (20, 1),
        "t_quantile.csv rows, and one at df 1e4"
    );
    assert!(
        over.is_empty(),
        "over the gate {QUANTILE_GATE:e}:\n{}",
        over.join("\n")
    );
}

#[test]
fn normal_quantile_975_is_within_1e_15_of_r_qnorm() {
    let rows = rows("normal_quantile.csv", "p,q");
    assert_eq!(rows.len(), 1, "normal_quantile.csv rows");
    let (p, want) = (rows[0][0], rows[0][1]);
    assert_eq!(p, 0.975);
    let got = normal_quantile_975();
    let ratio = relative(got, want);
    let ulps = (got.to_bits() as i64 - want.to_bits() as i64).abs();
    println!(
        "normal_quantile_975  gaply {got:.17e}  R {want:.17e}  relative {ratio:.3e}  {ulps} ulps"
    );
    assert!(
        ratio <= NORMAL_GATE,
        "over the gate {NORMAL_GATE:e}: relative {ratio:e}"
    );
}

#[test]
fn t_two_tailed_is_within_1e_9_of_r_pt_and_is_stats_l0s_at_df_1e4() {
    let rows = rows("t_p.csv", "df,t,p");
    let mut worst = Worst::default();
    let mut worst_l0 = Worst::default();
    let mut over = Vec::new();
    let mut at_1e4 = 0;
    let mut above = 0;
    for r in &rows {
        let (df, t, want) = (r[0], r[1], r[2]);
        let got = t_two_tailed(t, df).unwrap_or_else(|| panic!("df {df} t {t}: no p"));
        let ratio = relative(got, want);
        let l0 =
            pvalue::t_two_tailed(t, df).unwrap_or_else(|| panic!("df {df} t {t}: no stats_l0 p"));
        let l0_ratio = relative(l0, want);
        println!(
            "t_p df {df:>10}  t {t:>22.17e}  gaply {got:.17e}  R {want:.17e}  relative {ratio:.3e}  stats_l0 relative {l0_ratio:.3e}"
        );
        if ratio > P_GATE {
            over.push(format!("df {df} t {t}: relative {ratio:e}"));
        }
        worst.see(ratio, format!("df {df} t {t}"));
        if df == 1e4 {
            assert_eq!(
                got.to_bits(),
                l0.to_bits(),
                "df 1e4 t {t}: not stats_l0's bits"
            );
            at_1e4 += 1;
        } else {
            assert!(
                df > LARGE_DF,
                "df {df}: every other row is above the switch"
            );
            worst_l0.see(l0_ratio, format!("df {df} t {t}"));
            above += 1;
        }
    }
    println!(
        "t_two_tailed: {} rows ({at_1e4} at df 1e4, {above} above); the largest |gaply - R| / R is {:.3e} ({}); stats_l0's own, above 1e4 and ungated, is {:.3e} ({})",
        rows.len(),
        worst.0,
        worst.1,
        worst_l0.0,
        worst_l0.1
    );
    assert_eq!(
        (rows.len(), at_1e4),
        (338, 26),
        "t_p.csv rows, and 26 at df 1e4"
    );
    assert!(
        over.is_empty(),
        "over the gate {P_GATE:e}:\n{}",
        over.join("\n")
    );
}

#[test]
fn f_upper_with_df1_1_is_within_1e_9_of_r_pf_and_is_stats_l0s_at_df2_1e4_and_below() {
    let rows = rows("f_p.csv", "df1,df2,f,p");
    let mut worst = Worst::default();
    let mut worst_l0_above = Worst::default();
    let mut over = Vec::new();
    let (mut at_or_below, mut above) = (0, 0);
    for r in rows.iter().filter(|r| r[0] == 1.0) {
        let (df2, f, want) = (r[1], r[2], r[3]);
        let got = f_upper(f, 1.0, df2).unwrap_or_else(|| panic!("df2 {df2} F {f}: no p"));
        let ratio = relative(got, want);
        let l0 = pvalue::f_upper(f, 1.0, df2)
            .unwrap_or_else(|| panic!("df2 {df2} F {f}: no stats_l0 p"));
        let l0_ratio = relative(l0, want);
        println!(
            "f_p df1 1 df2 {df2:>10}  F {f:>22.17e}  gaply {got:.17e}  R {want:.17e}  relative {ratio:.3e}  stats_l0 relative {l0_ratio:.3e}"
        );
        if ratio > P_GATE {
            over.push(format!("df2 {df2} F {f}: relative {ratio:e}"));
        }
        worst.see(ratio, format!("df2 {df2} F {f}"));
        if df2 > LARGE_DF {
            worst_l0_above.see(l0_ratio, format!("df2 {df2} F {f}"));
            above += 1;
        } else {
            assert_eq!(
                got.to_bits(),
                l0.to_bits(),
                "df2 {df2} F {f}: not stats_l0's bits"
            );
            at_or_below += 1;
        }
    }
    println!(
        "f_upper df1 = 1: {} rows ({at_or_below} at df2 <= 1e4, {above} above); the largest |gaply - R| / R is {:.3e} ({}); stats_l0's own above 1e4, ungated, is {:.3e} ({})",
        at_or_below + above,
        worst.0,
        worst.1,
        worst_l0_above.0,
        worst_l0_above.1
    );
    assert_eq!(rows.len(), 198, "f_p.csv rows");
    assert_eq!(
        (at_or_below, above),
        (36, 108),
        "df1 = 1 rows at or below 1e4, and above"
    );
    assert!(
        over.is_empty(),
        "over the gate {P_GATE:e}:\n{}",
        over.join("\n")
    );
}

#[test]
fn f_upper_with_df1_2_and_3_is_stats_l0s_bit_for_bit() {
    let rows = rows("f_p.csv", "df1,df2,f,p");
    let mut worst = Worst::default();
    let mut counted = 0;
    for r in rows.iter().filter(|r| r[0] == 2.0 || r[0] == 3.0) {
        let (df1, df2, f, want) = (r[0], r[1], r[2], r[3]);
        let got = f_upper(f, df1, df2).unwrap_or_else(|| panic!("df1 {df1} df2 {df2} F {f}: no p"));
        let l0 = pvalue::f_upper(f, df1, df2)
            .unwrap_or_else(|| panic!("df1 {df1} df2 {df2} F {f}: no stats_l0 p"));
        let ratio = relative(got, want);
        println!(
            "f_p df1 {df1} df2 {df2:>10}  F {f:>22.17e}  gaply {got:.17e}  R {want:.17e}  relative {ratio:.3e} (ungated)"
        );
        assert_eq!(
            got.to_bits(),
            l0.to_bits(),
            "df1 {df1} df2 {df2} F {f}: not stats_l0's bits"
        );
        worst.see(ratio, format!("df1 {df1} df2 {df2} F {f}"));
        counted += 1;
    }
    println!(
        "f_upper df1 = 2 and 3: {counted} rows, every one stats_l0's bits; its largest |gaply - R| / R is {:.3e} ({}), ungated",
        worst.0, worst.1
    );
    assert_eq!(counted, 54, "df1 = 2 and 3 rows");
}

#[test]
fn every_function_returns_none_outside_its_measured_range() {
    let above = just_above(MEASURED_MAX_DF);
    assert!(above > 1e7 && above < 1e7 + 1.0);
    for df in [above, f64::INFINITY, f64::NAN, 0.0, -1.0] {
        assert_eq!(t_two_tailed(1.0, df), None, "t_two_tailed at df {df}");
        assert_eq!(t_quantile_975(df), None, "t_quantile_975 at df {df}");
        assert_eq!(
            cornish_fisher_975(df),
            None,
            "cornish_fisher_975 at df {df}"
        );
        for df1 in [1.0, 2.0, 3.0] {
            assert_eq!(f_upper(1.0, df1, df), None, "f_upper df1 {df1} at df2 {df}");
        }
    }
    // At the measured maximum itself, every one answers.
    assert!(t_two_tailed(1.0, MEASURED_MAX_DF).is_some());
    assert!(t_quantile_975(MEASURED_MAX_DF).is_some());
    assert!(cornish_fisher_975(MEASURED_MAX_DF).is_some());
    assert!(f_upper(1.0, 1.0, MEASURED_MAX_DF).is_some());
    assert!(f_upper(1.0, 3.0, MEASURED_MAX_DF).is_some());
    // The expansion is public from df 1e3; just below it is None.
    assert!(cornish_fisher_975(1e3).is_some());
    assert_eq!(cornish_fisher_975(just_below(1e3)), None);

    // A t that is not a number, on both routes.
    for df in [1e5, 10.0] {
        for t in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            assert_eq!(t_two_tailed(t, df), None, "t {t} at df {df}");
        }
    }
    // An overflowing t² is 0, never NaN.
    assert_eq!(t_two_tailed(1e200, 1e5), Some(0.0));
    assert_eq!(t_two_tailed(-1e200, 1e5), Some(0.0));

    // An F that is negative or not a number, on both routes; F = 0 is 1.
    for df2 in [1e5, 100.0] {
        for f in [-1.0, f64::NAN, f64::INFINITY] {
            assert_eq!(f_upper(f, 1.0, df2), None, "F {f} df1 1 at df2 {df2}");
            assert_eq!(f_upper(f, 2.0, df2), None, "F {f} df1 2 at df2 {df2}");
        }
        assert_eq!(f_upper(0.0, 1.0, df2), Some(1.0), "F 0 at df2 {df2}");
    }
}

#[test]
fn p_is_even_in_t_one_at_zero_in_unit_range_and_not_increasing_in_t() {
    for df in [10_001.0, 1e5, 1e7] {
        assert_eq!(t_two_tailed(0.0, df), Some(1.0), "df {df}: p at 0");
        let mut last = 1.0f64;
        for i in 0..=200 {
            let t = 40.0 * i as f64 / 200.0;
            let p = t_two_tailed(t, df).unwrap();
            let m = t_two_tailed(-t, df).unwrap();
            assert_eq!(p.to_bits(), m.to_bits(), "df {df} t {t}: p(-t) is p(t)");
            assert!((0.0..=1.0).contains(&p), "df {df} t {t}: p {p}");
            assert!(p <= last, "df {df} t {t}: p rose from {last} to {p}");
            last = p;
        }
        println!("df {df}: p even, 1 at 0, in [0, 1] and not increasing over 201 t from 0 to 40");
    }
}

#[test]
fn p_at_the_quantile_is_0_05_within_1e_9_and_the_quantile_does_not_increase_in_df() {
    let rows = rows("t_quantile.csv", "df,q");
    let mut worst = Worst::default();
    let mut checked = 0;
    let mut last: Option<(f64, f64)> = None;
    for r in &rows {
        let (df, r_q) = (r[0], r[1]);
        let q = t_quantile_975(df).unwrap();
        if let Some((last_df, last_q)) = last {
            assert!(last_df < df, "t_quantile.csv is in ascending df");
            assert!(
                q <= last_q,
                "df {df}: the quantile rose from {last_q} (df {last_df}) to {q}"
            );
        }
        last = Some((df, q));
        if df < 1e4 {
            continue;
        }
        for (which, at) in [("gaply's q", q), ("R's q", r_q)] {
            let p = t_two_tailed(at, df).unwrap();
            let off = (p / 0.05 - 1.0).abs();
            println!("df {df:>10}  p at {which} {at:.17e} is {p:.17e}  |p / 0.05 - 1| {off:.3e}");
            assert!(
                off <= P_GATE,
                "df {df}: p at {which} is {p}, {off:e} from 0.05"
            );
            worst.see(off, format!("df {df}, {which}"));
        }
        checked += 1;
    }
    println!(
        "{checked} df at or above 1e4; the largest |p / 0.05 - 1| is {:.3e} ({}); the quantile does not increase over all {} df",
        worst.0,
        worst.1,
        rows.len()
    );
    assert_eq!((checked, rows.len()), (17, 20));
}

/* ------------------------------------------------------------------------ *
 *  Crosstabs' distributions (Research OS, Phase 8X-1, decision 576), over
 *  make_xtab_dist_fixtures.R's chi_p.csv and fisher_p.csv
 * ------------------------------------------------------------------------ */

/// R's smallest normal double: below it R's own p carries fewer digits, so
/// a row whose R p is below it is counted and not compared (decision 576).
const R_SMALLEST_NORMAL: f64 = f64::MIN_POSITIVE;

/// Ten times the largest relative error measured over chi_p.csv (decision
/// 316): 1.812e-12, at df 2,401 (macOS aarch64, 9 Nov 2026), printed by the
/// test below.
const CHI_GATE: f64 = 2e-11;
/// Ten times the largest relative error measured over fisher_p.csv, over
/// all three p values (decision 316): 3.351e-11, greater at [50 49950; 40
/// 49960], a total of 100,000 (macOS aarch64, 9 Nov 2026).
const FISHER_GATE: f64 = 4e-10;

#[test]
fn chi_square_upper_is_within_its_gate_of_r_pchisq_to_df_2401() {
    let rows = rows_by("chi_p.csv", "df,x,p", PROVENANCE_XTAB);
    let mut worst = Worst::default();
    let mut over = Vec::new();
    let (mut compared, mut below_normal) = (0, 0);
    let mut dfs = std::collections::BTreeSet::new();
    for r in &rows {
        let (df, x, want) = (r[0], r[1], r[2]);
        dfs.insert(df as u64);
        let got = chi_square_upper(x, df).unwrap_or_else(|| panic!("df {df} x {x}: refused"));
        assert_eq!(
            Some(got.to_bits()),
            pvalue::chi_square_upper(x, df).map(f64::to_bits),
            "df {df} x {x}: not stats_l0's value, bit for bit"
        );
        if want < R_SMALLEST_NORMAL {
            assert!(got < R_SMALLEST_NORMAL, "df {df} x {x}: R's p {want:e} is below the smallest normal, gaply's {got:e}");
            below_normal += 1;
            continue;
        }
        compared += 1;
        let ratio = relative(got, want);
        println!("chi df {df:>6} x {x:.17e}  gaply {got:.17e}  R {want:.17e}  relative {ratio:.3e}");
        if ratio > CHI_GATE {
            over.push(format!("df {df} x {x}: relative {ratio:e}"));
        }
        worst.see(ratio, format!("df {df} x {x}"));
    }
    println!(
        "chi_square_upper: {compared} rows compared, {below_normal} with R's p below the smallest normal; the largest |gaply - R| / R is {:.3e} ({}); gate {CHI_GATE:e}",
        worst.0, worst.1
    );
    assert_eq!(dfs.len(), 12, "chi_p.csv's df: {dfs:?}");
    assert!(dfs.contains(&2401), "the largest df a 50 x 50 table reaches");
    assert!(compared >= 250, "{compared} rows compared");
    assert!(over.is_empty(), "over the gate {CHI_GATE:e}:\n{}", over.join("\n"));
}

#[test]
fn chi_square_upper_answers_to_df_2401_and_refuses_above_it() {
    assert_eq!(CHI_SQUARE_MAX_DF, 2401.0);
    assert!(chi_square_upper(2401.0, CHI_SQUARE_MAX_DF).is_some());
    for df in [just_above(CHI_SQUARE_MAX_DF), 2402.0, 1e7, f64::INFINITY, f64::NAN, 0.0, -1.0] {
        assert_eq!(chi_square_upper(1.0, df), None, "df {df}");
    }
    for x in [f64::NAN, f64::INFINITY, -1.0] {
        assert_eq!(chi_square_upper(x, 4.0), None, "x {x}");
    }
    assert_eq!(chi_square_upper(0.0, 4.0), Some(1.0));
}

#[test]
fn fisher_2x2_is_within_its_gate_of_r_fisher_test() {
    let rows = rows_by(
        "fisher_p.csv",
        "a,b,c,d,two_sided,less,greater",
        PROVENANCE_XTAB,
    );
    let mut worst = [Worst::default(), Worst::default(), Worst::default()];
    let mut over = Vec::new();
    let (mut compared, mut below_normal, mut large) = (0, 0, 0);
    for r in &rows {
        let (a, b, c, d) = (r[0] as u64, r[1] as u64, r[2] as u64, r[3] as u64);
        if a + b + c + d >= 100_000 {
            large += 1;
        }
        let got = fisher_2x2(a, b, c, d);
        for (i, (name, g, want)) in [
            ("two_sided", got.two_sided, r[4]),
            ("less", got.less, r[5]),
            ("greater", got.greater, r[6]),
        ]
        .into_iter()
        .enumerate()
        {
            assert!((0.0..=1.0).contains(&g), "[{a} {b}; {c} {d}] {name}: {g}");
            if want < R_SMALLEST_NORMAL {
                assert!(g < R_SMALLEST_NORMAL, "[{a} {b}; {c} {d}] {name}: R's p {want:e}, gaply's {g:e}");
                below_normal += 1;
                continue;
            }
            compared += 1;
            let ratio = relative(g, want);
            println!("fisher [{a} {b}; {c} {d}] {name:<9}  gaply {g:.17e}  R {want:.17e}  relative {ratio:.3e}");
            if ratio > FISHER_GATE {
                over.push(format!("[{a} {b}; {c} {d}] {name}: relative {ratio:e}"));
            }
            worst[i].see(ratio, format!("[{a} {b}; {c} {d}]"));
        }
    }
    for (name, w) in ["two_sided", "less", "greater"].iter().zip(&worst) {
        println!("fisher_2x2 {name}: the largest |gaply - R| / R is {:.3e} ({}); gate {FISHER_GATE:e}", w.0, w.1);
    }
    println!("fisher_2x2: {} tables, {compared} p values compared, {below_normal} with R's p below the smallest normal, {large} of a total of 100,000", rows.len());
    assert!(rows.len() >= 190, "{} tables", rows.len());
    assert!(large >= 7, "{large} tables of a total of 100,000");
    assert!(over.is_empty(), "over the gate {FISHER_GATE:e}:\n{}", over.join("\n"));
}

#[test]
fn fisher_2x2_with_a_zero_margin_is_one_three_times() {
    for t in [(0, 0, 3, 4), (3, 0, 4, 0), (0, 6, 0, 2), (5, 2, 0, 0), (0, 0, 0, 0)] {
        let f = fisher_2x2(t.0, t.1, t.2, t.3);
        assert_eq!((f.two_sided, f.less, f.greater), (1.0, 1.0, 1.0), "{t:?}");
    }
}

#[test]
fn fisher_two_sided_sum_counts_a_density_within_1e_7_of_the_observed_as_a_tie() {
    assert_eq!(FISHER_TIE_TOLERANCE, 1e-7);
    // Index 2 is above index 0 by 5e-8 relative: a tie, so counted.
    let d = [1.0, 2.0, 1.0 + 5e-8];
    let want = (1.0 + 1.0 + 5e-8) / (4.0 + 5e-8);
    let near = |got: f64, want: f64| relative(got, want) <= 1e-15;
    assert!(near(fisher_two_sided_sum(&d, 0), want), "observed 0, its floating-point tie counted");
    assert!(near(fisher_two_sided_sum(&d, 2), want), "observed 2, index 0 below it");
    // Above by 2e-7 relative: not a tie.
    let e = [1.0, 2.0, 1.0 + 2e-7];
    assert!(near(fisher_two_sided_sum(&e, 0), 1.0 / (4.0 + 2e-7)), "2e-7 above is not a tie");
    // The observed at the mode takes every table, clamped at 1.
    assert_eq!(fisher_two_sided_sum(&d, 1), 1.0);
}

/* ------------------------------------------------------------------------ *
 *  The Mann-Whitney statistic's exact distribution (Research OS, Phase
 *  8Y-1, decision 603), written by a third script,
 *  `make_ranks_dist_fixtures.R`, so the six files above stay byte for byte.
 * ------------------------------------------------------------------------ */

/// The third script: `wilcox_p.csv`.
const PROVENANCE_RANKS: &str = "/usr/local/bin/Rscript make_ranks_dist_fixtures.R";
/// Ten times the largest relative error measured over wilcox_p.csv, over
/// both tails (decision 316): 1.310e-14, the lower tail at n1 49, n2 49,
/// u 1,200 (macOS aarch64, 10 Nov 2026), printed by the test below.
const WILCOX_GATE: f64 = 2e-13;

#[test]
fn rank_sum_exact_tails_are_within_their_gate_of_r_pwilcox() {
    let rows = rows_by("wilcox_p.csv", "n1,n2,u,lower,upper", PROVENANCE_RANKS);
    let mut worst = [Worst::default(), Worst::default()];
    let mut over = Vec::new();
    let (mut compared, mut below_normal) = (0, 0);
    let mut sizes = std::collections::BTreeSet::new();
    for r in &rows {
        let (n1, n2, u) = (r[0] as u64, r[1] as u64, r[2] as u64);
        sizes.insert((n1, n2));
        let got = rank_sum_exact_tails(u, n1, n2)
            .unwrap_or_else(|| panic!("n1 {n1} n2 {n2} u {u}: refused"));
        for (i, (name, g, want)) in [("lower", got.lower, r[3]), ("upper", got.upper, r[4])]
            .into_iter()
            .enumerate()
        {
            assert!((0.0..=1.0).contains(&g), "n1 {n1} n2 {n2} u {u} {name}: {g}");
            if want < R_SMALLEST_NORMAL {
                assert!(g < R_SMALLEST_NORMAL, "n1 {n1} n2 {n2} u {u} {name}: R's {want:e}, gaply's {g:e}");
                below_normal += 1;
                continue;
            }
            compared += 1;
            let ratio = relative(g, want);
            println!("wilcox n1 {n1:>2} n2 {n2:>2} u {u:>4} {name:<5}  gaply {g:.17e}  R {want:.17e}  relative {ratio:.3e}");
            if ratio > WILCOX_GATE {
                over.push(format!("n1 {n1} n2 {n2} u {u} {name}: relative {ratio:e}"));
            }
            worst[i].see(ratio, format!("n1 {n1} n2 {n2} u {u}"));
        }
    }
    for (name, w) in ["lower", "upper"].iter().zip(&worst) {
        println!("rank_sum_exact_tails {name}: the largest |gaply - R| / R is {:.3e} ({}); gate {WILCOX_GATE:e}", w.0, w.1);
    }
    println!("rank_sum_exact_tails: {} rows, {compared} tails compared, {below_normal} with R's below the smallest normal", rows.len());
    assert_eq!(sizes.len(), 64, "every pair of the eight sizes");
    assert!(sizes.contains(&(49, 49)), "the largest groups the exact p is given for");
    assert!(rows.len() >= 1000, "{} rows", rows.len());
    assert!(over.is_empty(), "over the gate {WILCOX_GATE:e}:\n{}", over.join("\n"));
}

#[test]
fn rank_sum_exact_tails_answer_to_49_and_refuse_above_it() {
    assert_eq!(RANK_SUM_EXACT_MAX, 49);
    let top = rank_sum_exact_tails(0, RANK_SUM_EXACT_MAX, RANK_SUM_EXACT_MAX)
        .expect("49 and 49 are answered");
    assert!(top.lower > 0.0 && top.upper == 1.0, "{top:?}");
    assert!(rank_sum_exact_tails(49 * 49, 49, 49).is_some(), "u at n1 n2");
    for (u, n1, n2) in [
        (0, 50, 49),
        (0, 49, 50),
        (0, 50, 50),
        (0, 0, 5),
        (0, 5, 0),
        (26, 5, 5),
        (u64::MAX, 3, 3),
    ] {
        assert_eq!(rank_sum_exact_tails(u, n1, n2), None, "u {u} n1 {n1} n2 {n2}");
    }
}

#[test]
fn rank_sum_exact_tails_are_each_their_own_sum() {
    // At the far ends a tail is one arrangement in C(N, n1): 1 minus the
    // other tail would cancel to 0 there. And each tail at u, with the other
    // tail at u + 1, adds to 1, the centre's tails are equal by symmetry, and
    // both tails count u itself.
    let (n1, n2) = (49, 49);
    let total = (1..=49u32).fold(1.0f64, |c, i| c * f64::from(49 + i) / f64::from(i));
    let end = rank_sum_exact_tails(n1 * n2, n1, n2).expect("u at n1 n2");
    assert!(relative(end.upper, 1.0 / total) <= 1e-12, "{} against {}", end.upper, 1.0 / total);
    let start = rank_sum_exact_tails(0, n1, n2).expect("u at 0");
    assert!(relative(start.lower, 1.0 / total) <= 1e-12, "{}", start.lower);
    for (n1, n2) in [(3, 5), (10, 10), (20, 35)] {
        for u in 0..n1 * n2 {
            let a = rank_sum_exact_tails(u, n1, n2).expect("in range");
            let b = rank_sum_exact_tails(u + 1, n1, n2).expect("in range");
            assert!((a.lower + b.upper - 1.0).abs() <= 1e-14, "n1 {n1} n2 {n2} u {u}");
            let mirror = rank_sum_exact_tails(n1 * n2 - u, n1, n2).expect("in range");
            assert!(relative(a.lower, mirror.upper) <= 1e-14, "n1 {n1} n2 {n2} u {u}");
        }
    }
}

/* ------------------------------------------------------------------------ *
 *  The Wilcoxon signed-rank statistic's exact distribution (Research OS,
 *  Phase 8Z-1, decision 637), written by a fourth script,
 *  `make_signed_rank_dist_fixtures.R`, so the files above stay byte for byte.
 * ------------------------------------------------------------------------ */

/// The fourth script: `signrank_p.csv`.
const PROVENANCE_SIGNED_RANK: &str = "/usr/local/bin/Rscript make_signed_rank_dist_fixtures.R";
/// Ten times the largest relative error measured over signrank_p.csv, over
/// both tails (decision 316): 3.650e-15, at n 48, v 353 and its mirror, v 823
/// (macOS aarch64, 10 Nov 2026), printed by the test below; rounded up.
const SIGNRANK_GATE: f64 = 3.7e-14;

#[test]
fn signed_rank_exact_tails_are_within_their_gate_of_r_psignrank() {
    let rows = rows_by("signrank_p.csv", "n,v,lower,upper", PROVENANCE_SIGNED_RANK);
    let mut worst = [Worst::default(), Worst::default()];
    let mut over = Vec::new();
    let mut compared = 0;
    let mut sizes = std::collections::BTreeSet::new();
    for r in &rows {
        let (n, v) = (r[0] as u64, r[1] as u64);
        sizes.insert(n);
        let got = signed_rank_exact_tails(v, n).unwrap_or_else(|| panic!("n {n} v {v}: refused"));
        for (i, (name, g, want)) in [("lower", got.lower, r[2]), ("upper", got.upper, r[3])]
            .into_iter()
            .enumerate()
        {
            assert!((0.0..=1.0).contains(&g), "n {n} v {v} {name}: {g}");
            compared += 1;
            let ratio = relative(g, want);
            if ratio > SIGNRANK_GATE {
                over.push(format!("n {n} v {v} {name}: relative {ratio:e}"));
            }
            worst[i].see(ratio, format!("n {n} v {v}"));
        }
    }
    for (name, w) in ["lower", "upper"].iter().zip(&worst) {
        println!("signed_rank_exact_tails {name}: the largest |gaply - R| / R is {:.3e} ({}); gate {SIGNRANK_GATE:e}", w.0, w.1);
    }
    println!("signed_rank_exact_tails: {} rows, {compared} tails compared", rows.len());
    assert_eq!(sizes.len(), 49, "every n from 1 to 49");
    assert_eq!(compared, 2 * rows.len(), "every row compared");
    assert!(over.is_empty(), "over the gate {SIGNRANK_GATE:e}:\n{}", over.join("\n"));
}

#[test]
fn signed_rank_exact_tails_answer_to_49_and_refuse_above_it() {
    assert_eq!(SIGNED_RANK_EXACT_MAX, 49);
    let top = 49 * 50 / 2;
    let low = signed_rank_exact_tails(0, 49).expect("49 is answered");
    // One arrangement in 2^49: every difference negative.
    assert_eq!(low.lower, 1.0 / (1u64 << 49) as f64);
    assert_eq!(low.upper, 1.0);
    let high = signed_rank_exact_tails(top, 49).expect("v at the top");
    assert_eq!(high.upper, 1.0 / (1u64 << 49) as f64);
    assert_eq!(high.lower, 1.0);
    for (v, n) in [(0, 50), (0, 0), (top + 1, 49), (7, 3), (u64::MAX, 3)] {
        assert_eq!(signed_rank_exact_tails(v, n), None, "v {v} n {n}");
    }
}

/// Every one of the 2^n sign arrangements counted one by one, an independent
/// check of the recurrence: the tails exactly, and each pair adding as they
/// must.
#[test]
fn signed_rank_exact_tails_equal_a_count_of_every_arrangement() {
    for n in 1u64..=14 {
        let top = (n * (n + 1) / 2) as usize;
        let mut counts = vec![0u64; top + 1];
        for mask in 0u64..(1 << n) {
            let v: u64 = (0..n).filter(|i| mask >> i & 1 == 1).map(|i| i + 1).sum();
            counts[v as usize] += 1;
        }
        let total = (1u64 << n) as f64;
        for v in 0..=top {
            let got = signed_rank_exact_tails(v as u64, n).expect("in range");
            let lower: u64 = counts[..=v].iter().sum();
            let upper: u64 = counts[v..].iter().sum();
            assert_eq!(got.lower, lower as f64 / total, "n {n} v {v}");
            assert_eq!(got.upper, upper as f64 / total, "n {n} v {v}");
        }
    }
}

/* ------------------------------------------------------------------------ *
 *  F at the linear regression's degrees of freedom (Research OS, Phase 9A-1,
 *  decision 670), written by a fifth script, `make_linreg_dist_fixtures.R`, so
 *  the files above stay byte for byte.
 * ------------------------------------------------------------------------ */

/// The fifth script: `f_upper_regression.tsv`.
const PROVENANCE_LINREG: &str = "/usr/local/bin/Rscript make_linreg_dist_fixtures.R";
/// Decision 316's stop: a row above this stops the step, and the gate is not
/// widened to admit it (decision 670).
const LINREG_STOP: f64 = 1e-8;
/// Ten times the largest relative error measured over f_upper_regression.tsv
/// (decision 316): 1.096e-10, at df1 49, df2 1e5 and an upper tail of 1e-100
/// (macOS aarch64, 11 Nov 2026), printed by the test below; rounded up.
const F_REGRESSION_GATE: f64 = 1.1e-9;

/// The rows of `f_upper_regression.tsv`: f, df1, df2, tail, pf.
fn linreg_rows() -> Vec<[f64; 5]> {
    let name = "f_upper_regression.tsv";
    let text = std::fs::read_to_string(format!("{DIR}/{name}")).unwrap_or_else(|e| panic!("{name}: {e}"));
    let provenance: Vec<&str> = text.lines().take_while(|l| l.starts_with('#')).collect();
    assert!(provenance.iter().any(|h| h.contains(PROVENANCE_LINREG)), "{name}: not written by {PROVENANCE_LINREG}");
    assert!(provenance.iter().any(|h| h.contains(R_VERSION)), "{name}: not written with {R_VERSION}");
    let mut lines = text.lines().filter(|l| !l.starts_with('#'));
    assert_eq!(lines.next(), Some("f\tdf1\tdf2\ttail\tpf"), "{name}: the column header");
    lines
        .map(|l| {
            let v: Vec<f64> = l.split('\t').map(|f| f.parse().unwrap_or_else(|e| panic!("{name}: `{f}`: {e}"))).collect();
            assert_eq!(v.len(), 5, "{name}: `{l}`");
            [v[0], v[1], v[2], v[3], v[4]]
        })
        .collect()
}

#[test]
fn f_upper_is_within_its_gate_of_r_pf_at_the_regressions_degrees_of_freedom() {
    let rows = linreg_rows();
    let mut worst = Worst::default();
    let (mut over, mut stopped) = (Vec::new(), Vec::new());
    let mut df1s = std::collections::BTreeSet::new();
    let mut df2s = std::collections::BTreeSet::new();
    let mut smallest = 1.0f64;
    for [f, df1, df2, tail, want] in &rows {
        df1s.insert(*df1 as u64);
        df2s.insert(*df2 as u64);
        smallest = smallest.min(*want);
        let got = f_upper(*f, *df1, *df2).unwrap_or_else(|| panic!("df1 {df1} df2 {df2} tail {tail}: None"));
        assert!((0.0..=1.0).contains(&got), "df1 {df1} df2 {df2} tail {tail}: {got}");
        let ratio = relative(got, *want);
        let row = format!("df1 {df1} df2 {df2} tail {tail:e} (F {f:e}, R {want:e}, gaply {got:e})");
        if ratio > LINREG_STOP {
            stopped.push(format!("{row}: relative {ratio:e}"));
        }
        if ratio > F_REGRESSION_GATE {
            over.push(format!("{row}: relative {ratio:e}"));
        }
        worst.see(ratio, row);
    }
    println!("f_upper at the regression's df: the largest |gaply - R| / R is {:.3e} ({}); gate {F_REGRESSION_GATE:e}", worst.0, worst.1);
    println!("f_upper at the regression's df: {} rows, R's smallest p {smallest:e}", rows.len());
    assert_eq!(rows.len(), 1200, "12 df1 x 10 df2 x 10 tails, none skipped");
    assert_eq!(df1s.into_iter().collect::<Vec<_>>(), vec![1, 2, 3, 5, 7, 10, 15, 20, 30, 40, 49, 50]);
    assert_eq!(df2s.len(), 10);
    assert!(stopped.is_empty(), "above decision 316's stop {LINREG_STOP:e}; the method waits for a decision:\n{}", stopped.join("\n"));
    assert!(over.is_empty(), "over the gate {F_REGRESSION_GATE:e}:\n{}", over.join("\n"));
}

/* ------------------------------------------------------------------------ *
 *  The chi-square beyond df 2,401 (Research OS, Phase 9B-1, decision 699):
 *  make_factor_dist_fixtures.R's chi_p_factor.tsv. The earlier scripts are
 *  not run; their files stay byte for byte.
 * ------------------------------------------------------------------------ */

/// The sixth script: `chi_p_factor.tsv`.
const PROVENANCE_FACTOR: &str = "/usr/local/bin/Rscript make_factor_dist_fixtures.R";
/// Decision 699's stop: a row above this stops the step at the end of Part 1,
/// and Bartlett's p waits for a decision; the gate is not widened to admit it.
const FACTOR_STOP: f64 = 1e-8;
/// Ten times the largest relative error measured over chi_p_factor.tsv
/// (decision 699): 8.965e-12, at df 8,000 and an upper tail of 0.05 (macOS
/// aarch64, 11 Nov 2026), printed by the test below; rounded up.
const CHI_WIDE_GATE: f64 = 9e-11;

/// The rows of `chi_p_factor.tsv`: x, df, tail, p.
fn factor_rows() -> Vec<[f64; 4]> {
    let name = "chi_p_factor.tsv";
    let text = std::fs::read_to_string(format!("{DIR}/{name}")).unwrap_or_else(|e| panic!("{name}: {e}"));
    let provenance: Vec<&str> = text.lines().take_while(|l| l.starts_with('#')).collect();
    assert!(provenance.iter().any(|h| h.contains(PROVENANCE_FACTOR)), "{name}: not written by {PROVENANCE_FACTOR}");
    assert!(provenance.iter().any(|h| h.contains(R_VERSION)), "{name}: not written with {R_VERSION}");
    let mut lines = text.lines().filter(|l| !l.starts_with('#'));
    assert_eq!(lines.next(), Some("x\tdf\ttail\tp"), "{name}: the column header");
    lines
        .map(|l| {
            let v: Vec<f64> = l.split('\t').map(|f| f.parse().unwrap_or_else(|e| panic!("{name}: `{f}`: {e}"))).collect();
            assert_eq!(v.len(), 4, "{name}: `{l}`");
            [v[0], v[1], v[2], v[3]]
        })
        .collect()
}

#[test]
fn chi_square_upper_wide_is_within_its_gate_of_r_pchisq_to_df_8128() {
    use gaply_core::stats_dist::chi_square_upper_wide;
    let rows = factor_rows();
    let mut worst = Worst::default();
    let (mut over, mut stopped) = (Vec::new(), Vec::new());
    let mut dfs = std::collections::BTreeSet::new();
    for [x, df, tail, want] in &rows {
        dfs.insert(*df as u64);
        let got = chi_square_upper_wide(*x, *df).unwrap_or_else(|| panic!("df {df} tail {tail}: refused"));
        assert_eq!(
            Some(got.to_bits()),
            pvalue::chi_square_upper(*x, *df).map(f64::to_bits),
            "df {df} tail {tail}: not stats_l0's value, bit for bit"
        );
        let ratio = relative(got, *want);
        let row = format!("df {df} tail {tail:e} (x {x:e}, R {want:e}, gaply {got:e})");
        println!("chi wide {row}: relative {ratio:.3e}");
        if ratio > FACTOR_STOP {
            stopped.push(format!("{row}: relative {ratio:e}"));
        }
        if ratio > CHI_WIDE_GATE {
            over.push(format!("{row}: relative {ratio:e}"));
        }
        worst.see(ratio, row);
    }
    println!("chi_square_upper_wide: {} rows; the largest |gaply - R| / R is {:.3e} ({}); gate {CHI_WIDE_GATE:e}", rows.len(), worst.0, worst.1);
    assert_eq!(rows.len(), 88, "8 df x 11 tails, none skipped");
    assert_eq!(dfs.into_iter().collect::<Vec<_>>(), vec![1225, 2346, 2415, 3000, 4000, 6000, 8000, 8128]);
    assert!(stopped.is_empty(), "above decision 699's stop {FACTOR_STOP:e}; Bartlett's p waits for a decision:\n{}", stopped.join("\n"));
    assert!(over.is_empty(), "over the gate {CHI_WIDE_GATE:e}:\n{}", over.join("\n"));
}

/// Decision 699: the wide chi-square answers to df 8,128 and refuses above;
/// at or below df 2,401 it is chi_square_upper, bit for bit, and
/// CHI_SQUARE_MAX_DF is unchanged.
#[test]
fn chi_square_upper_wide_answers_to_df_8128_is_the_narrow_one_below_2401_and_refuses_above() {
    use gaply_core::stats_dist::{chi_square_upper_wide, CHI_SQUARE_WIDE_MAX_DF};
    assert_eq!((CHI_SQUARE_WIDE_MAX_DF, CHI_SQUARE_MAX_DF), (8128.0, 2401.0));
    assert!(chi_square_upper_wide(8128.0, CHI_SQUARE_WIDE_MAX_DF).is_some());
    assert!(chi_square_upper_wide(3000.0, 2402.0).is_some(), "the narrow one refuses 2,402; the wide one answers");
    for df in [just_above(CHI_SQUARE_WIDE_MAX_DF), 8129.0, 1e7, f64::INFINITY, f64::NAN, 0.0, -1.0] {
        assert_eq!(chi_square_upper_wide(1.0, df), None, "df {df}");
    }
    for x in [f64::NAN, f64::INFINITY, -1.0] {
        assert_eq!(chi_square_upper_wide(x, 4000.0), None, "x {x}");
    }
    for (x, df) in [(0.5, 1.0), (3.0, 4.0), (2401.0, 2401.0), (0.0, 25.0), (1e4, 2000.0)] {
        assert_eq!(chi_square_upper_wide(x, df).map(f64::to_bits), chi_square_upper(x, df).map(f64::to_bits), "x {x} df {df}");
    }
}

/* ------------------------------------------------------------------------ *
 *  The noncentral chi-square, its ncp and the chi-square's upper quantile
 *  (Research OS, Phase 9C-1, decision 744): make_cfa_dist_fixtures.R's four
 *  files. The earlier scripts are not run; their files stay byte for byte.
 * ------------------------------------------------------------------------ */

/// The seventh script.
const PROVENANCE_CFA: &str = "/usr/local/bin/Rscript make_cfa_dist_fixtures.R";

/// The rows of one of the seventh script's files, under its expected header.
fn cfa_rows(name: &str, header: &str) -> Vec<Vec<f64>> {
    let text = std::fs::read_to_string(format!("{DIR}/{name}")).unwrap_or_else(|e| panic!("{name}: {e}"));
    let provenance: Vec<&str> = text.lines().take_while(|l| l.starts_with('#')).collect();
    assert!(provenance.iter().any(|h| h.contains(PROVENANCE_CFA)), "{name}: not written by {PROVENANCE_CFA}");
    assert!(provenance.iter().any(|h| h.contains(R_VERSION)), "{name}: not written with {R_VERSION}");
    let mut lines = text.lines().filter(|l| !l.starts_with('#'));
    assert_eq!(lines.next(), Some(header), "{name}: the column header");
    let width = header.split('\t').count();
    lines
        .map(|l| {
            let v: Vec<f64> = l.split('\t').map(|f| f.parse().unwrap_or_else(|e| panic!("{name}: `{f}`: {e}"))).collect();
            assert_eq!(v.len(), width, "{name}: `{l}`");
            v
        })
        .collect()
}

/// Decision 744's plan for R's pchisq with an ncp, absolute: ten times R's own
/// error, about 1.25e-12 to df 2,000 and 7.5e-12 at df 8,128. A row above it
/// stops the step at the end of Part 1.
fn nc_absolute_plan(df: f64) -> f64 {
    if df > 2000.0 {
        7.5e-11
    } else {
        1.25e-11
    }
}
/// The plan in relative terms where R's tail is at least 1e-4: the absolute
/// plan over 1e-4.
const NC_RELATIVE_PLAN: f64 = 1.25e-7;
/// Ten times the largest absolute error measured over nc_pchisq.tsv, rounded up
/// (decision 744): 3.317e-12, the lower tail at x 3,975.35, df 2, ncp 3,500
/// (macOS aarch64, 13 Nov 2026), printed by the test below.
const NC_ABSOLUTE_GATE: f64 = 3.4e-11;
/// Ten times the largest relative error measured where R's tail is at least
/// 1e-4, rounded up: 1.084e-10, the upper tail of 0.0235 at df 8,128, ncp 3,500.
const NC_RELATIVE_GATE: f64 = 1.1e-9;
/// Decision 744's gate over the Poisson mixture's rows, absolute, and relative
/// for a far tail (the smaller of the two).
const NC_MIXTURE_GATE: f64 = 1e-8;
/// The quantile's plan, relative: a row above it stops the step.
const CHI_QUANTILE_PLAN: f64 = 1e-10;
/// Ten times the largest relative error measured over chi_quantile.tsv, rounded
/// up: 1.607e-14, alpha 0.01 at df 2,000 (macOS aarch64, 13 Nov 2026).
const CHI_QUANTILE_GATE: f64 = 1.7e-13;
/// Decision 744's gate for the RMSEA interval's ncp, relative.
const NCP_GATE: f64 = 1e-10;

#[test]
fn noncentral_chi_square_tails_are_within_their_gates_of_r_pchisq() {
    use gaply_core::stats_dist::noncentral_chi_square_tails;
    let rows = cfa_rows("nc_pchisq.tsv", "x\tdf\tncp\tlower\tupper");
    let (mut abs_worst, mut rel_worst) = (Worst::default(), Worst::default());
    let (mut stopped, mut over) = (Vec::new(), Vec::new());
    let mut relative_rows = 0;
    for v in &rows {
        let (x, df, ncp, lower, upper) = (v[0], v[1], v[2], v[3], v[4]);
        let (gl, gu) = noncentral_chi_square_tails(x, df, ncp).unwrap_or_else(|| panic!("x {x} df {df} ncp {ncp}: refused"));
        for (tail, got, want) in [("lower", gl, lower), ("upper", gu, upper)] {
            let row = format!("{tail} x {x:e} df {df} ncp {ncp} (R {want:e}, gaply {got:e})");
            let abs = (got - want).abs();
            abs_worst.see(abs, row.clone());
            if abs > nc_absolute_plan(df) {
                stopped.push(format!("{row}: absolute {abs:e}"));
            }
            if abs > NC_ABSOLUTE_GATE {
                over.push(format!("{row}: absolute {abs:e}"));
            }
            if want >= 1e-4 {
                relative_rows += 1;
                let rel = relative(got, want);
                rel_worst.see(rel, row.clone());
                if rel > NC_RELATIVE_PLAN {
                    stopped.push(format!("{row}: relative {rel:e}"));
                }
                if rel > NC_RELATIVE_GATE {
                    over.push(format!("{row}: relative {rel:e}"));
                }
            }
        }
    }
    println!("noncentral vs pchisq: {} rows; absolute worst {:.3e} ({}); relative worst {:.3e} over {relative_rows} tails at least 1e-4 ({})", rows.len(), abs_worst.0, abs_worst.1, rel_worst.0, rel_worst.1);
    assert_eq!(rows.len(), 598, "the script's rows, none skipped");
    assert!(stopped.is_empty(), "above decision 744's plan; the step stops at the end of Part 1:\n{}", stopped.join("\n"));
    assert!(over.is_empty(), "over the gates:\n{}", over.join("\n"));
}

#[test]
fn noncentral_chi_square_tails_are_within_1e_8_of_the_poisson_mixture_where_pchisq_cannot_reach() {
    use gaply_core::stats_dist::noncentral_chi_square_tails;
    let rows = cfa_rows("nc_mixture.tsv", "x\tdf\tncp\tz\tlower\tupper");
    let mut worst = Worst::default();
    let mut over = Vec::new();
    for v in &rows {
        let (x, df, ncp, z, lower, upper) = (v[0], v[1], v[2], v[3], v[4], v[5]);
        let (gl, gu) = noncentral_chi_square_tails(x, df, ncp).unwrap_or_else(|| panic!("x {x} df {df} ncp {ncp}: refused"));
        for (tail, got, want) in [("lower", gl, lower), ("upper", gu, upper)] {
            let row = format!("{tail} z {z} df {df} ncp {ncp:e} (R {want:e}, gaply {got:e})");
            // Absolute for both; relative for the far tail, the smaller of the two.
            let far = want < 0.5;
            let err = if far { relative(got, want).max((got - want).abs()) } else { (got - want).abs() };
            worst.see(err, row.clone());
            if err > NC_MIXTURE_GATE {
                over.push(format!("{row}: {err:e}"));
            }
        }
    }
    println!("noncentral vs the mixture: {} rows; worst {:.3e} ({}); gate {NC_MIXTURE_GATE:e}", rows.len(), worst.0, worst.1);
    assert_eq!(rows.len(), 135, "the script's rows, none skipped");
    assert!(over.is_empty(), "over decision 744's gate; the step stops at the end of Part 1:\n{}", over.join("\n"));
}

#[test]
fn chi_square_quantile_upper_is_within_its_gate_of_r_qchisq() {
    use gaply_core::stats_dist::chi_square_quantile_upper;
    let rows = cfa_rows("chi_quantile.tsv", "alpha\tdf\tx");
    let mut worst = Worst::default();
    let (mut stopped, mut over) = (Vec::new(), Vec::new());
    for v in &rows {
        let (alpha, df, want) = (v[0], v[1], v[2]);
        let got = chi_square_quantile_upper(alpha, df).unwrap_or_else(|| panic!("alpha {alpha} df {df}: refused"));
        let rel = relative(got, want);
        let row = format!("alpha {alpha} df {df} (R {want:e}, gaply {got:e})");
        worst.see(rel, row.clone());
        if rel > CHI_QUANTILE_PLAN {
            stopped.push(format!("{row}: {rel:e}"));
        }
        if rel > CHI_QUANTILE_GATE {
            over.push(format!("{row}: {rel:e}"));
        }
    }
    println!("quantile vs qchisq: {} rows; relative worst {:.3e} ({})", rows.len(), worst.0, worst.1);
    assert_eq!(rows.len(), 18);
    assert!(stopped.is_empty(), "above the plan; the step stops at the end of Part 1:\n{}", stopped.join("\n"));
    assert!(over.is_empty(), "over the gate:\n{}", over.join("\n"));
}

#[test]
fn the_rmsea_intervals_ncp_is_within_1e_10_of_uniroot() {
    use gaply_core::stats_dist::noncentral_chi_square_ncp;
    let rows = cfa_rows("rmsea_ncp.tsv", "C\td\tncp_lower_95\tncp_lower_05");
    let mut worst = Worst::default();
    let mut over = Vec::new();
    for v in &rows {
        let (c, d) = (v[0], v[1]);
        for (target, want) in [(0.95, v[2]), (0.05, v[3])] {
            let got = noncentral_chi_square_ncp(c, d, target).unwrap_or_else(|| panic!("C {c} d {d}: refused"));
            let rel = relative(got, want);
            let row = format!("C {c} d {d} target {target} (R {want:e}, gaply {got:e})");
            worst.see(rel, row.clone());
            if rel > NCP_GATE {
                over.push(format!("{row}: {rel:e}"));
            }
        }
    }
    println!("ncp vs uniroot: {} rows; relative worst {:.3e} ({})", rows.len(), worst.0, worst.1);
    assert_eq!(rows.len(), 30);
    assert!(over.is_empty(), "over decision 744's gate; the step stops at the end of Part 1:\n{}", over.join("\n"));
}

/// Decision 744: the tails sum to 1 within 4 ε where neither is tiny; each is
/// monotone in x and in the ncp over a grid; and each refusal.
#[test]
fn the_noncentral_tails_sum_to_1_are_monotone_and_refuse_outside_their_range() {
    use gaply_core::stats_dist::{chi_square_quantile_upper, noncentral_chi_square_ncp, noncentral_chi_square_tails, CHI_SQUARE_WIDE_MAX_DF};
    let mut checked = 0;
    for df in [1.0f64, 2.0, 5.0, 24.0, 500.0, 8128.0] {
        for ncp in [0.0, 0.5, 3.0, 40.0, 900.0] {
            let sd = (2.0 * (df + 2.0 * ncp)).sqrt();
            let mut last = (0.0f64, 1.0f64);
            for k in -40..=40 {
                let x = df + ncp + 0.1 * f64::from(k) * sd;
                if x <= 0.0 {
                    continue;
                }
                let (lo, up) = noncentral_chi_square_tails(x, df, ncp).unwrap();
                if lo > 1e-3 && up > 1e-3 {
                    assert!((lo + up - 1.0).abs() <= 4.0 * f64::EPSILON, "df {df} ncp {ncp} x {x}: {lo} + {up}");
                    checked += 1;
                }
                assert!(lo >= last.0 && up <= last.1, "df {df} ncp {ncp} x {x}: not monotone in x");
                last = (lo, up);
            }
        }
        // Monotone in the ncp at a fixed x: the lower tail falls, the upper rises.
        let x = df + 10.0;
        let mut last = (1.0f64, 0.0f64);
        for ncp in [0.0, 0.01, 0.1, 1.0, 5.0, 20.0, 100.0, 1000.0] {
            let (lo, up) = noncentral_chi_square_tails(x, df, ncp).unwrap();
            assert!(lo <= last.0 && up >= last.1, "df {df} x {x} ncp {ncp}: not monotone in the ncp");
            last = (lo, up);
        }
    }
    assert!(checked > 500, "the sum was checked at {checked} points only");
    // x = 0 before any statrs call; ncp 0 is the central chi-square's.
    assert_eq!(noncentral_chi_square_tails(0.0, 3.0, 2.0), Some((0.0, 1.0)));
    assert_eq!(noncentral_chi_square_tails(7.0, 3.0, 0.0).map(|t| t.1), gaply_core::stats_dist::chi_square_upper_wide(7.0, 3.0));
    // Refusals.
    for (x, df, ncp) in [(1.0, 0.0, 1.0), (1.0, -1.0, 1.0), (1.0, CHI_SQUARE_WIDE_MAX_DF + 1.0, 1.0), (1.0, f64::NAN, 1.0), (-1.0, 3.0, 1.0), (f64::INFINITY, 3.0, 1.0), (f64::NAN, 3.0, 1.0), (1.0, 3.0, -1.0), (1.0, 3.0, f64::INFINITY), (1.0, 3.0, f64::NAN)] {
        assert_eq!(noncentral_chi_square_tails(x, df, ncp), None, "x {x} df {df} ncp {ncp}");
    }
    for t in [0.0, 1.0, -0.1, 1.5, f64::NAN] {
        assert_eq!(noncentral_chi_square_ncp(10.0, 3.0, t), None, "target {t}");
        assert_eq!(chi_square_quantile_upper(t, 3.0), None, "alpha {t}");
    }
    assert_eq!(chi_square_quantile_upper(0.05, 0.0), None);
    assert_eq!(chi_square_quantile_upper(0.05, CHI_SQUARE_WIDE_MAX_DF + 1.0), None);
    // 0 when the lower tail at ncp 0 is already at or below the target.
    assert_eq!(noncentral_chi_square_ncp(1.0, 10.0, 0.95), Some(0.0));
}

/* ---- the review of 9C-1 (Research OS, Phase 9C-2, decision 761) ------------- */

/// Runs `f` on its own thread and waits at most 5 s for it, so a call that
/// never returns fails the test instead of hanging the suite. A thread that
/// does not return is left running until the test binary exits.
fn within_5_s<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> Option<T> {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(f());
    });
    rx.recv_timeout(std::time::Duration::from_secs(5)).ok()
}

/// Decision 759's B1: the ncp's inversion bisected two functions, statrs's
/// `gamma_lr` at ncp 0 and the Poisson mixture above it, which differ in the
/// last bits; where the first is above the target and the second just above
/// 0 is not, the bracket's top halved to 5e-324 and the stop never held. The
/// review's three triples, and five doubles near 34.764251683501314 at df 50
/// and .05 that hung on the Mac before the fix (a scan of ±400 ulps found 58,
/// at offsets 0 to 60 but 1, 5 and 6; these are offsets 0, 2, 10, 30 and 60).
/// Each now returns 0 within 5 s. Each case returns at the ncp-0 test, so either
/// half of the fix alone ends it (both recorded equivalent in the mutation
/// record); the midpoint stop is a guard no known case reaches (the review of
/// 9C-2, finding 7).
#[test]
fn the_ncps_inversion_returns_where_decision_759_found_it_never_did() {
    use gaply_core::stats_dist::noncentral_chi_square_ncp;
    let cases = [
        (7.8147279032511845, 3.0, 0.95),
        (0.10258658877510106, 2.0, 0.05),
        (11.07049769351635, 5.0, 0.95),
        (34.764251683501314, 50.0, 0.05),
        (34.76425168350133, 50.0, 0.05),
        (34.764251683501385, 50.0, 0.05),
        (34.76425168350153, 50.0, 0.05),
        (34.76425168350174, 50.0, 0.05),
    ];
    for (x, df, target) in cases {
        let got = within_5_s(move || noncentral_chi_square_ncp(x, df, target));
        assert_eq!(got, Some(Some(0.0)), "x {x:?} df {df} target {target}: no answer within 5 s, or not 0");
    }
}

/// Decision 759's m1: the upper tail was 1.0000000000000002 at (1e-4, 8, 20);
/// both tails are clamped to [0, 1].
#[test]
fn the_noncentral_tails_stay_within_0_and_1() {
    use gaply_core::stats_dist::noncentral_chi_square_tails;
    let (lo, up) = noncentral_chi_square_tails(1e-4, 8.0, 20.0).unwrap();
    assert!((0.0..=1.0).contains(&up), "upper {up:?}");
    assert!((0.0..=1.0).contains(&lo), "lower {lo:?}");
}

/// Decision 759's m2: at an ncp of 2^53 or more the mixture's steps stop
/// moving and (100, 3, 1e300) never returned; above NONCENTRAL_MAX_NCP the
/// tails are `None`, and so is the ncp's inversion when its bracket passes it.
#[test]
fn the_noncentral_tails_refuse_an_ncp_above_their_limit_at_once() {
    use gaply_core::stats_dist::{noncentral_chi_square_ncp, noncentral_chi_square_tails};
    assert_eq!(within_5_s(|| noncentral_chi_square_tails(100.0, 3.0, 1e300)), Some(None), "(100, 3, 1e300)");
    assert_eq!(within_5_s(|| noncentral_chi_square_ncp(1e300, 3.0, 0.05)), Some(None), "(1e300, 3, .05)");
}
