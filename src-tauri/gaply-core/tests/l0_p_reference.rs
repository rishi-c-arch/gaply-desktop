//! How closely `stats_l0::pvalue` matches R, measured over
//! `tests/fixtures/stats_l0/p_fixtures.csv` (written by `make_p_fixtures.R`
//! with `/usr/local/bin/Rscript`; see that script for the grid).
//!
//! A GATE: every row's engine p must be within a relative 1e-9 of R's p
//! (tolerance set at Step 4 from the measured figures; `ENGINE_P_RELATIVE_MARGIN`
//! in `stats_l0::pvalue` is ten times it). It also asserts that the fixture is
//! R's and that every family has at least 400 rows. It prints, per family and
//! separately for rows with R's p below 1e-6, the worst absolute and relative
//! error and the row where each occurs:
//!
//! ```text
//! cargo test -p gaply_core --test l0_p_reference -- --nocapture
//! ```
//!
//! Beside the engine it prints the same figures for the `1 - cdf` form that
//! `stats_verify.rs` uses, so the difference `sf` makes is on record. That
//! comparison is output only; nothing asserts on it.

use std::collections::BTreeMap;

use gaply_core::stats_l0::pvalue;
use statrs::distribution::{ChiSquared, ContinuousCDF, FisherSnedecor, Normal, StudentsT};

/// Every row's engine p must lie within this relative distance of R's p.
const RELATIVE_GATE: f64 = 1e-9;

struct Row {
    family: String,
    statistic: f64,
    df1: Option<f64>,
    df2: Option<f64>,
    p: f64,
}

fn num(s: &str) -> Option<f64> {
    (s != "NA").then(|| s.parse::<f64>().unwrap_or_else(|e| panic!("bad number {s:?}: {e}")))
}

fn load() -> (Vec<String>, Vec<Row>) {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/stats_l0/p_fixtures.csv");
    let text = std::fs::read_to_string(path).expect("p_fixtures.csv is present");
    let mut header = Vec::new();
    let mut rows = Vec::new();
    let mut columns_seen = false;
    for line in text.lines() {
        if let Some(h) = line.strip_prefix('#') {
            header.push(h.trim().to_string());
            continue;
        }
        if !columns_seen {
            assert_eq!(line, "family,statistic,df1,df2,p", "column header");
            columns_seen = true;
            continue;
        }
        let f: Vec<&str> = line.split(',').collect();
        assert_eq!(f.len(), 5, "row {line:?}");
        rows.push(Row {
            family: f[0].to_string(),
            statistic: num(f[1]).expect("statistic"),
            df1: num(f[2]),
            df2: num(f[3]),
            p: num(f[4]).expect("p"),
        });
    }
    (header, rows)
}

fn engine(r: &Row) -> Option<f64> {
    match r.family.as_str() {
        "t" => pvalue::t_two_tailed(r.statistic, r.df1?),
        "F" => pvalue::f_upper(r.statistic, r.df1?, r.df2?),
        "chi" => pvalue::chi_square_upper(r.statistic, r.df1?),
        "r" => pvalue::r_two_tailed(r.statistic, r.df1?),
        "z" => pvalue::z_two_tailed(r.statistic),
        other => panic!("unknown family {other:?}"),
    }
}

/// The `1 - cdf` form, as `stats_verify.rs` computes its tails. Comparison only.
fn one_minus_cdf(r: &Row) -> f64 {
    let t2 = |t: f64, df: f64| 2.0 * (1.0 - StudentsT::new(0.0, 1.0, df).unwrap().cdf(t.abs()));
    match r.family.as_str() {
        "t" => t2(r.statistic, r.df1.unwrap()),
        "F" => 1.0 - FisherSnedecor::new(r.df1.unwrap(), r.df2.unwrap()).unwrap().cdf(r.statistic),
        "chi" => 1.0 - ChiSquared::new(r.df1.unwrap()).unwrap().cdf(r.statistic),
        "r" => {
            let df = r.df1.unwrap();
            t2(r.statistic * (df / (1.0 - r.statistic * r.statistic)).sqrt(), df)
        }
        "z" => 2.0 * (1.0 - Normal::new(0.0, 1.0).unwrap().cdf(r.statistic.abs())),
        _ => unreachable!(),
    }
}

#[derive(Default)]
struct Worst {
    n: usize,
    abs: (f64, Option<usize>),
    rel: (f64, Option<usize>),
}

impl Worst {
    fn add(&mut self, i: usize, got: f64, want: f64) {
        self.n += 1;
        let a = (got - want).abs();
        if a > self.abs.0 || self.abs.1.is_none() {
            self.abs = (a, Some(i));
        }
        if want > 0.0 {
            let r = a / want;
            if r > self.rel.0 || self.rel.1.is_none() {
                self.rel = (r, Some(i));
            }
        }
    }
}

fn describe(r: &Row, got: f64) -> String {
    let df = match (r.df1, r.df2) {
        (Some(a), Some(b)) => format!("({a}, {b})"),
        (Some(a), None) => format!("({a})"),
        _ => String::new(),
    };
    format!("{}{df} = {:.17e}  R p = {:.17e}  got {:.17e}", r.family, r.statistic, r.p, got)
}

#[test]
fn engine_p_values_against_r() {
    let (header, rows) = load();
    assert!(header.iter().any(|h| h.contains("/usr/local/bin/Rscript")), "header names the Rscript: {header:?}");
    assert!(header.iter().any(|h| h.starts_with("R version")), "header carries R.version.string: {header:?}");

    let mut count: BTreeMap<&str, usize> = BTreeMap::new();
    for r in &rows {
        *count.entry(r.family.as_str()).or_default() += 1;
    }
    for fam in ["t", "F", "chi", "r", "z"] {
        assert!(count.get(fam).copied().unwrap_or(0) >= 400, "{fam}: {count:?}");
    }

    // (family, band, form) -> worst. band: "p>=1e-6" or "p<1e-6"; form: engine or 1-cdf.
    let mut worst: BTreeMap<(String, &str, &str), Worst> = BTreeMap::new();
    let mut got_all = Vec::with_capacity(rows.len());
    for (i, r) in rows.iter().enumerate() {
        let got = engine(r).unwrap_or_else(|| panic!("engine refused row {i}: {}", describe(r, f64::NAN)));
        assert!((0.0..=1.0).contains(&got), "row {i}: {}", describe(r, got));
        got_all.push(got);
        let band = if r.p < 1e-6 { "p<1e-6" } else { "p>=1e-6" };
        worst.entry((r.family.clone(), band, "engine (sf)")).or_default().add(i, got, r.p);
        worst.entry((r.family.clone(), band, "1 - cdf")).or_default().add(i, one_minus_cdf(r), r.p);
    }

    println!("rows per family: {count:?}  ({} rows)", rows.len());
    for ((fam, band, form), w) in &worst {
        let (ai, ri) = (w.abs.1.unwrap(), w.rel.1);
        println!("{fam:>3} {band:<8} {form:<12} n={:<4} worst_abs={:.3e}  worst_rel={:.3e}", w.n, w.abs.0, w.rel.0);
        if *form == "engine (sf)" {
            println!("      abs at row {ai}: {}", describe(&rows[ai], got_all[ai]));
            if let Some(ri) = ri {
                println!("      rel at row {ri}: {}", describe(&rows[ri], got_all[ri]));
            }
        }
    }

    // How many rows reach each relative error, per family: the spread behind
    // the worst case. Then every row above 1e-6, which is where a tolerance
    // decision bites.
    let bands = [1e-12, 1e-9, 1e-6, 1e-3];
    let mut over: BTreeMap<&str, [usize; 4]> = BTreeMap::new();
    let mut large = Vec::new();
    for (i, r) in rows.iter().enumerate() {
        let rel = if r.p > 0.0 { (got_all[i] - r.p).abs() / r.p } else { 0.0 };
        let e = over.entry(r.family.as_str()).or_default();
        for (k, b) in bands.iter().enumerate() {
            if rel > *b {
                e[k] += 1;
            }
        }
        if rel > 1e-6 {
            large.push(format!("   row {i} rel={rel:.3e}: {}", describe(r, got_all[i])));
        }
    }
    println!("engine rows with relative error above 1e-12 / 1e-9 / 1e-6 / 1e-3:");
    for (fam, c) in &over {
        println!("   {fam:>3} {c:?} of {}", count[fam]);
    }
    println!("engine rows with relative error above 1e-6: {}", large.len());
    for l in &large {
        println!("{l}");
    }

    println!("statcheck rows, t(28):");
    for (i, r) in rows.iter().enumerate() {
        if r.family == "t" && r.df1 == Some(28.0) && [1.95, 2.0, 2.05, 2.095, 2.1, 2.105].contains(&r.statistic) {
            println!("   t = {:<6} R p = {:.17}  engine = {:.17}  diff = {:.3e}", r.statistic, r.p, got_all[i], got_all[i] - r.p);
        }
    }

    // The gate, after everything above is printed.
    let off: Vec<String> = rows
        .iter()
        .enumerate()
        .filter(|(i, r)| (got_all[*i] - r.p).abs() > RELATIVE_GATE * r.p)
        .map(|(i, r)| format!("row {i}: {}", describe(r, got_all[i])))
        .collect();
    assert!(off.is_empty(), "{} of {} rows off R by more than a relative {RELATIVE_GATE:e}:\n{}", off.len(), rows.len(), off.join("\n"));
}

/// The statcheck paper's example: a reported t(28) = 2.0 is any t in
/// [1.95, 2.05], whose p runs from .0498 to .0613.
#[test]
fn statcheck_t28_span_is_reproduced() {
    let at = |t: f64| format!("{:.4}", pvalue::t_two_tailed(t, 28.0).expect("t(28)"));
    assert_eq!(at(2.05), "0.0498");
    assert_eq!(at(1.95), "0.0613");
}
