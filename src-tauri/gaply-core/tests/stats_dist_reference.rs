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
//!
//! The two 8X-1 files are written by a second script,
//! `make_xtab_dist_fixtures.R` (decision 576), so the four files above stay
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
