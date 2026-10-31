//! **`gaply_core::stats_dist`'s studentized range, `t_quantile`, and
//! `f_upper` at the ANOVA's df, against their oracles (Research OS, Phase
//! 8G, decisions 314 to 316).**
//!
//! * The studentized range against mpmath at 30 digits
//!   (`fixtures/stats_anova/srange_mpmath.tsv`, written by
//!   `make_srange_fixtures.py`, its q placed by `make_srange_placement.R`):
//!   the upper tail at every row whose p is at least `SRANGE_P_FLOOR`, and
//!   the quantile at a subset (the module doc of the second test says which,
//!   and why). df 1 to 1.5 is gated against mpmath alone: R's `ptukey` and
//!   `qtukey` return NaN below df 2 (measured, ptukey(3, 3, 1.5)).
//! * R's own `ptukey` against mpmath, printed with its measured error and
//!   gated at ten times it, so the precision this file attributes to R is a
//!   measurement: R's tail is 1 − cdf, and above df 25,000 it is the
//!   infinite-df value.
//! * `t_quantile` against R's `qt` (`fixtures/stats_anova/t_quantile.tsv`),
//!   and its gap to `t_quantile_975` at 0.05 (decision 314).
//! * `f_upper` against R's `pf` at df1 2 to 49, df2 2.5 to 1e5, fractional
//!   df2 included (`fixtures/stats_anova/f_upper.tsv`): the range
//!   `stats_dist_reference.rs` runs ungated (its line 16).
//!
//! Every gate is ten times the largest error measured, the measurement
//! beside it (31 Oct 2026, macOS aarch64), asserted on every platform as
//! decision 277 made 8E's pins. Print every row:
//!
//! ```text
//! cargo test -p gaply_core --test studentized_range_reference -- --nocapture
//! ```

use gaply_core::stats_dist::{
    f_upper, studentized_range_quantile, studentized_range_upper, t_quantile, t_quantile_975,
    t_two_tailed, MEASURED_MAX_DF, SRANGE_MAX_K, SRANGE_MIN_DF, SRANGE_MIN_K, SRANGE_P_FLOOR,
};

const DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/stats_anova");

/// 6.34e-11 measured, q 8.973 at k 10, df infinite (p 1e-8): statrs's
/// `erfc`'s own error, flat in k once the inner grid has 40 panels.
const UPPER_GATE: f64 = 7e-10;
/// 4.13e-11 measured, p 0.549 at k 10, df 1.
const QUANTILE_GATE: f64 = 5e-10;
/// R's `ptukey`, absolute: 3.37e-3 measured, q 55.72 at k 50, df 2
/// (mpmath 6.63e-3, R 1.0e-2). A measurement of the oracle, not a gate on
/// Gaply: it says how far R's tail is from the definition.
const R_PTUKEY_GATE: f64 = 3.4e-2;
/// 9.80e-12 measured, p 0.05 at df 15,000 (Hill's p under the bisection).
const T_QUANTILE_GATE: f64 = 1e-10;
/// 9.80e-12 measured, at df 15,000: below 1e4 the gap is the targets'
/// (0.05 against 0.050000000000000044), above it Hill's bisection against
/// Cornish–Fisher.
const T_QUANTILE_975_GAP_GATE: f64 = 1e-10;
/// 1.09e-10 measured, F 5.335 at df1 49, df2 1e5 (p 1e-30).
const F_UPPER_GATE: f64 = 2e-9;

/// The stop of decision 316: if the tail cannot reach 1e-8 relative for p
/// in [floor, 1] over its domain, the method is not written.
const DECISION_316_STOP: f64 = 1e-8;

fn rows(name: &str, provenance: &str) -> Vec<Vec<String>> {
    let text =
        std::fs::read_to_string(format!("{DIR}/{name}")).unwrap_or_else(|e| panic!("{name}: {e}"));
    assert!(
        text.lines()
            .any(|l| l.starts_with('#') && l.contains(provenance)),
        "{name}: not written by {provenance}"
    );
    text.lines()
        .filter(|l| !l.starts_with('#'))
        .skip(1)
        .map(|l| l.split('\t').map(str::to_string).collect())
        .collect()
}

fn df_of(s: &str) -> f64 {
    if s == "inf" {
        f64::INFINITY
    } else {
        s.parse().unwrap()
    }
}

struct Gold {
    q: f64,
    k: u32,
    df: f64,
    df_text: String,
    p: f64,
    /// mpmath's degree 4 against degree 5, on the rows checked.
    self_error: Option<f64>,
    r_ptukey: Option<f64>,
    target: Option<f64>,
}

fn gold() -> Vec<Gold> {
    rows("srange_mpmath.tsv", "make_srange_fixtures.py")
        .into_iter()
        .map(|f| Gold {
            q: f[0].parse().unwrap(),
            k: f[1].parse().unwrap(),
            df: df_of(&f[2]),
            df_text: f[2].clone(),
            p: f[3].parse().unwrap(),
            self_error: f[4].parse().ok(),
            r_ptukey: f[5].parse().ok().filter(|v: &f64| v.is_finite()),
            target: f[6].parse().ok(),
        })
        .collect()
}

#[test]
fn the_upper_tail_matches_mpmath_down_to_the_floor_over_the_whole_domain() {
    let rows = gold();
    assert!(
        rows.len() > 400,
        "{} rows: the grid did not load",
        rows.len()
    );
    let mut worst = (0.0f64, String::new());
    let mut worst_self = 0.0f64;
    let mut compared = 0;
    let mut small_df = 0;
    let mut checked = 0;
    for g in &rows {
        if g.p < SRANGE_P_FLOOR {
            continue;
        }
        if let Some(e) = g.self_error {
            worst_self = worst_self.max(e);
            checked += 1;
        }
        let got = studentized_range_upper(g.q, g.k, g.df).expect("in the domain");
        let rel = (got - g.p).abs() / g.p;
        println!(
            "q {:<14} k {:>2} df {:>6} mpmath {:.15e} gaply {:.15e} rel {rel:.2e}",
            g.q, g.k, g.df_text, g.p, got
        );
        compared += 1;
        if g.df < 2.0 {
            small_df += 1;
        }
        if rel > worst.0 {
            worst = (rel, format!("q {} k {} df {}", g.q, g.k, g.df_text));
        }
    }
    println!(
        "upper tail: {compared} rows ({small_df} below df 2), largest {:.3e} at {}, mpmath's own error at most {worst_self:.1e}",
        worst.0, worst.1
    );
    assert!(
        compared > 300 && small_df > 20,
        "{compared} rows, {small_df} below df 2"
    );
    // The reference must be well inside what it gates: its own error at
    // most 1e-12, a tenth of the tail's measured error.
    assert!(checked > 100, "{checked} rows checked at the next degree");
    assert!(
        worst_self <= 1e-12,
        "the reference's own error {worst_self:e}"
    );
    assert!(
        worst.0 < DECISION_316_STOP,
        "decision 316's stop: {:e} at {}",
        worst.0,
        worst.1
    );
    assert!(
        worst.0 <= UPPER_GATE,
        "{:e} over the gate {UPPER_GATE:e} at {}",
        worst.0,
        worst.1
    );
}

/// The quantile at mpmath's p, back to its q. A subset, because an
/// unoptimised test build takes about a second per quantile: every row
/// placed at p 0.05 and at 1e-10, and below df 2 the q of 4, 100 and 1e5.
#[test]
fn the_quantile_inverts_the_tail_to_mpmaths_q() {
    let mut worst = (0.0f64, String::new());
    let mut compared = 0;
    for g in gold() {
        let chosen = matches!(g.target, Some(t) if t == 0.05 || t == 1e-10)
            || (g.df < 2.0 && [4.0, 100.0, 1e5].contains(&g.q));
        if !chosen || g.p < SRANGE_P_FLOOR || g.p >= 1.0 {
            continue;
        }
        let q = studentized_range_quantile(g.p, g.k, g.df)
            .unwrap_or_else(|| panic!("p {:e} k {} df {}: in the domain", g.p, g.k, g.df_text));
        let rel = (q - g.q).abs() / g.q;
        println!(
            "p {:.6e} k {:>2} df {:>6} q {} gaply {q:.15e} rel {rel:.2e}",
            g.p, g.k, g.df_text, g.q
        );
        compared += 1;
        if rel > worst.0 {
            worst = (rel, format!("p {:e} k {} df {}", g.p, g.k, g.df_text));
        }
    }
    println!(
        "quantile: {compared} rows, largest {:.3e} at {}",
        worst.0, worst.1
    );
    assert!(compared > 80, "{compared} rows");
    assert!(
        worst.0 <= QUANTILE_GATE,
        "{:e} over the gate {QUANTILE_GATE:e} at {}",
        worst.0,
        worst.1
    );
}

/// R's `ptukey` against mpmath where R answers (df 2 and above), so the
/// precision this step attributes to R is measured, not quoted. Its error is
/// absolute (1 − cdf); above df 25,000 R returns the infinite-df value.
#[test]
fn rs_ptukey_is_measured_against_mpmath() {
    let mut worst = (0.0f64, String::new());
    for g in gold() {
        let Some(r) = g.r_ptukey else { continue };
        if g.df > 25_000.0 {
            continue;
        }
        let abs = (r - g.p).abs();
        if abs > worst.0 {
            worst = (
                abs,
                format!(
                    "q {} k {} df {} (mpmath {:e}, R {r:e})",
                    g.q, g.k, g.df_text, g.p
                ),
            );
        }
    }
    println!(
        "R's ptukey: largest absolute error {:.3e} at {}",
        worst.0, worst.1
    );
    assert!(worst.0 <= R_PTUKEY_GATE, "{:e} at {}", worst.0, worst.1);
}

#[test]
fn the_domain_is_refused_outside_its_measured_range_and_the_ends_are_exact() {
    let up = studentized_range_upper;
    assert_eq!(up(0.0, 3, 10.0), Some(1.0));
    for (q, k, df) in [
        (3.0, SRANGE_MIN_K - 1, 10.0),
        (3.0, SRANGE_MAX_K + 1, 10.0),
        (3.0, 3, SRANGE_MIN_DF * 0.99),
        (3.0, 3, f64::NAN),
        (3.0, 3, f64::from_bits(MEASURED_MAX_DF.to_bits() + 1)),
        (-1.0, 3, 10.0),
        (f64::NAN, 3, 10.0),
        (f64::INFINITY, 3, 10.0),
    ] {
        assert_eq!(up(q, k, df), None, "q {q} k {k} df {df}");
    }
    assert!(up(3.0, 3, MEASURED_MAX_DF).is_some() && up(3.0, 3, f64::INFINITY).is_some());
    assert_eq!(studentized_range_quantile(1.0, 3, 10.0), Some(0.0));
    for p in [SRANGE_P_FLOOR * 0.5, 0.0, 1.5, f64::NAN] {
        assert_eq!(studentized_range_quantile(p, 3, 10.0), None, "p {p}");
    }
}

#[test]
fn the_tail_falls_in_q_and_rises_in_k() {
    for df in [1.0, 3.0, 30.0, f64::INFINITY] {
        let mut last = 1.0;
        for q in [0.5, 1.0, 2.0, 3.0, 5.0, 8.0, 13.0, 21.0] {
            let p = studentized_range_upper(q, 5, df).unwrap();
            assert!(p <= last, "df {df}: P(Q > {q}) = {p} above {last}");
            last = p;
        }
        let mut last = 0.0;
        for k in [2, 3, 5, 10, 20, 50] {
            let p = studentized_range_upper(4.0, k, df).unwrap();
            assert!(p >= last, "df {df}: k {k} gives {p} below {last}");
            last = p;
        }
    }
}

#[test]
fn t_quantile_matches_rs_qt_and_its_gap_to_t_quantile_975_at_005_is_measured() {
    let mut worst = (0.0f64, String::new());
    let mut gap = (0.0f64, 0.0f64);
    let mut n = 0;
    for f in rows("t_quantile.tsv", "make_anova_fixtures.R") {
        let (p, df, r): (f64, f64, f64) = (
            f[0].parse().unwrap(),
            f[1].parse().unwrap(),
            f[2].parse().unwrap(),
        );
        let t = t_quantile(p, df).expect("a quantile");
        let rel = (t - r).abs() / r;
        n += 1;
        // Gated where Bonferroni reaches: df N − k is at least 2, and p is
        // 0.05 / m with m at most 1,225. Outside it the row is printed: at df
        // 1 and p 1e-10 (t near 6.4e9) the quantile carries stats_l0's tail
        // error one for one, 8.3e-8 measured, and no method reaches it.
        if df < 2.0 || p < 0.05 / 1225.0 * (1.0 - 1e-12) {
            println!("ungated: p {p:e} df {df} rel {rel:.2e}");
        } else if rel > worst.0 {
            worst = (rel, format!("p {p:e} df {df}"));
        }
        // The quantile returns the t whose p is nearest the target.
        let back = t_two_tailed(t, df).unwrap();
        assert!((back - p).abs() <= 1e-9 * p, "p {p} df {df}: back {back}");
        if p == 0.05 {
            let q975 = t_quantile_975(df).unwrap();
            let g = (t - q975).abs() / q975;
            if g > gap.0 {
                gap = (g, df);
            }
        }
    }
    println!(
        "t_quantile: {n} rows, largest {:.3e} at {}",
        worst.0, worst.1
    );
    println!(
        "t_quantile(0.05) against t_quantile_975: largest gap {:.3e} at df {}",
        gap.0, gap.1
    );
    assert!(n == 128, "{n} rows");
    assert!(worst.0 <= T_QUANTILE_GATE, "{:e} at {}", worst.0, worst.1);
    assert!(
        gap.0 <= T_QUANTILE_975_GAP_GATE,
        "{:e} at df {}",
        gap.0,
        gap.1
    );
    assert_eq!(t_quantile(0.0, 10.0), None);
    assert_eq!(t_quantile(1.0, 10.0), None);
}

#[test]
fn f_upper_matches_rs_pf_at_the_df_the_anova_reaches() {
    let mut worst = (0.0f64, String::new());
    let mut n = 0;
    for f in rows("f_upper.tsv", "make_anova_fixtures.R") {
        let v: Vec<f64> = f.iter().map(|s| s.parse().unwrap()).collect();
        let (fv, df1, df2, r) = (v[0], v[1], v[2], v[3]);
        let p = f_upper(fv, df1, df2).expect("a p");
        let rel = (p - r).abs() / r;
        n += 1;
        if rel > worst.0 {
            worst = (rel, format!("F {fv:e} df1 {df1} df2 {df2} (R {r:e})"));
        }
    }
    println!("f_upper: {n} rows, largest {:.3e} at {}", worst.0, worst.1);
    assert!(n > 200, "{n} rows");
    assert!(worst.0 <= F_UPPER_GATE, "{:e} at {}", worst.0, worst.1);
}
