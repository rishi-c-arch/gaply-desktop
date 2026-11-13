//! **`gaply_core::stats_cfa` against R (Research OS, Phase 9C-1, decision 746).**
//! `tests/fixtures/stats_cfa/make_cfa_fixtures.R`, run as /usr/local/bin/Rscript
//! with lavaan 0.7-3, fits each case with lavaan, refines it by its own Fisher
//! scoring to a Newton decrement of 1e-20 and computes every statistic and fit
//! measure from decision 745's formulas (cross-checked against lavaan there);
//! this test computes each case with Gaply and compares every number within its
//! family's gate, ten times the family's measured error, and every code, note,
//! exclusion and refusal exactly. Two cases are built by a formula identical in
//! R and here, and have no file.

use std::collections::BTreeMap;

use gaply_core::contracts::codes::Closed;
use gaply_core::contracts::ParamValue;
use gaply_core::stats_cfa::{compute, CfaOutcome, CfaParams, Stat};
use gaply_core::stats_desc::Alias;
use gaply_core::stats_l1::dataset::{build_dataset, ColumnAlias, DataFingerprint, Dataset, RawTable, TableRef};

const DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/stats_cfa");
const PROVENANCE: &str = "/usr/local/bin/Rscript make_cfa_fixtures.R";
const R_VERSION: &str = "R version 4.6.1";
const LAVAAN: &str = "lavaan 0.7.3";

/// Each family's gate, relative (absolute where R's value is 0): ten times
/// the largest error measured over every case, rounded up (macOS aarch64, 13
/// Nov 2026), printed by the test below.
fn gate(family: &str) -> f64 {
    match family {
        // Worst 2.017e-9, a loading of 0.056 in worse_than_independence, whose
        // likelihood is nearly flat: both fits stop at a decrement of 1e-20 and
        // still differ there. Every other case is within 1e-10.
        "estimate" => 2.1e-8,
        // Worst 1.235e-9, the same case.
        "se" => 1.3e-8,
        // Worst 2.831e-9, the same case.
        "cr" => 2.9e-8,
        // Worst 2.148e-9 (two_by_two, a P of 0.23), relative over max(1, C.R.²)
        // (see `error_of`).
        "p" => 2.2e-8,
        // Worst 3.279e-9, an SMC of 3.5e-4 in worse_than_independence.
        "standardized" => 3.3e-8,
        // Worst 2.124e-12, two_by_two's CMIN of 0.251.
        "fit_chi" => 2.2e-11,
        // Worst 1.574e-12, missing_and_codes's AGFI.
        "fit_index" => 1.6e-11,
        // Worst 6.643e-13, two_by_two's F0 interval.
        "fit_interval" => 6.7e-12,
        // Worst 9.298e-12 of the largest variance, two_by_two's RMR.
        "fit_rmr" => 9.3e-11,
        other => panic!("no gate for {other}"),
    }
}

/// Below this, R's value is taken as rounding and the error as absolute. P
/// values are relative at any size: a p of 5e-20 must be right to its own size.
/// RMR has its own family, in the data's variance units (see `rmr_scale`).
fn floor(family: &str) -> f64 {
    if family == "p" {
        0.0
    } else {
        1e-12
    }
}

/// A value of either side: a number or a closed word.
#[derive(Debug, Clone, PartialEq)]
enum V {
    Num(f64),
    Word(String),
}

fn header_lines(name: &str) -> (Vec<String>, Vec<Vec<String>>) {
    let text = std::fs::read_to_string(format!("{DIR}/{name}")).unwrap_or_else(|e| panic!("{name}: {e}"));
    let provenance: Vec<&str> = text.lines().take_while(|l| l.starts_with('#')).collect();
    assert!(provenance.iter().any(|h| h.contains(PROVENANCE)), "{name}: not written by {PROVENANCE}");
    let mut lines = text.lines().filter(|l| !l.starts_with('#'));
    let header: Vec<String> = lines.next().unwrap().split('\t').map(str::to_string).collect();
    (header, lines.map(|l| l.split('\t').map(str::to_string).collect()).collect())
}

fn dataset(headers: Vec<String>, rows: Vec<Vec<String>>, label: &str) -> Dataset {
    build_dataset(
        RawTable::from_rows(headers, rows),
        DataFingerprint::of_bytes(label.as_bytes(), "cfa golden", TableRef::FirstTable),
    )
    .expect("dataset")
}

/// make_cfa_fixtures.R's formula_data: K factors over N rows and p items, item j
/// on factor (j − 1) mod K, every value an integer.
pub fn formula_data(p: u64, k: u64, n: u64) -> Dataset {
    let headers: Vec<String> = (1..=p).map(|j| format!("v{j}")).collect();
    let rows: Vec<Vec<String>> = (1..=n)
        .map(|i| {
            (1..=p)
                .map(|j| {
                    let f = (j - 1) % k;
                    let score = ((i * (7907 + 104729 * f)) % 1013) as i64 - 506;
                    let shared = ((i * 2654435761) % 1009) as i64 - 504;
                    let noise = ((i * 104729 + j * 7919 + i * j * 31) % 997) as i64 - 498;
                    ((2 + (j % 3) as i64) * (score + shared) + noise).to_string()
                })
                .collect()
        })
        .collect();
    dataset(headers, rows, &format!("formula {p} {k} {n}"))
}

fn data_file(name: &str) -> Dataset {
    let text = std::fs::read_to_string(format!("{DIR}/data/{name}")).unwrap_or_else(|e| panic!("{name}: {e}"));
    let provenance: Vec<&str> = text.lines().take_while(|l| l.starts_with('#')).collect();
    assert!(provenance.iter().any(|h| h.contains(PROVENANCE)), "{name}: not written by {PROVENANCE}");
    let mut lines = text.lines().filter(|l| !l.starts_with('#'));
    let headers: Vec<String> = lines.next().unwrap().split(',').map(str::to_string).collect();
    let rows: Vec<Vec<String>> = lines.map(|l| l.split(',').map(str::to_string).collect()).collect();
    dataset(headers, rows, name)
}

/// A case's dataset and spec, from cases.tsv's row.
pub fn case_input(row: &[String]) -> (Dataset, CfaParams) {
    let d = match row[1].strip_prefix("formula:") {
        Some(spec) => {
            let v: Vec<u64> = spec.split(':').map(|x| x.parse().unwrap()).collect();
            formula_data(v[0], v[1], v[2])
        }
        None => data_file(&row[1]),
    };
    let indicators = row[2]
        .split(';')
        .map(|f| f.split(',').map(|c| Alias(ColumnAlias(c.parse().unwrap()))).collect())
        .collect();
    let missing_codes = if row[4].is_empty() { vec![] } else { row[4].split(',').map(|c| c.parse().unwrap()).collect() };
    (d, CfaParams { indicators, correlated_factors: row[3] == "true", missing_codes })
}

fn word(c: gaply_core::stats_cfa::CfaCode) -> String {
    c.wire().strip_prefix("stats.cfa.").unwrap().to_string()
}

fn v(s: Stat) -> V {
    match s {
        Stat::Value(x) => V::Num(x),
        Stat::NotComputable(c) => V::Word(word(c)),
    }
}

/// Gaply's result under make_cfa_fixtures.R's keys.
pub fn keyed(r: &CfaOutcome) -> BTreeMap<String, V> {
    let mut out = BTreeMap::new();
    let mut put = |k: String, x: V| {
        out.insert(k, x);
    };
    put("rows_used".into(), V::Num(r.rows_used as f64));
    for (i, l) in r.loadings.iter().enumerate() {
        for (f, s) in [("estimate", l.estimate), ("se", l.se), ("cr", l.cr), ("p", l.p), ("standardized", l.standardized)] {
            put(format!("loading.{i}.{f}"), v(s));
        }
    }
    for x in &r.factor_variances {
        for (f, s) in [("estimate", x.estimate), ("se", x.se), ("cr", x.cr), ("p", x.p)] {
            put(format!("fvar.{}.{f}", x.factor), v(s));
        }
    }
    for x in &r.factor_covariances {
        for (f, s) in [("estimate", x.estimate), ("se", x.se), ("cr", x.cr), ("p", x.p), ("correlation", x.correlation)] {
            put(format!("fcov.{}.{}.{f}", x.factors[0], x.factors[1]), v(s));
        }
    }
    for (i, x) in r.error_variances.iter().enumerate() {
        for (f, s) in [("estimate", x.estimate), ("se", x.se), ("cr", x.cr), ("p", x.p)] {
            put(format!("evar.{i}.{f}"), v(s));
        }
    }
    for (i, x) in r.smc.iter().enumerate() {
        put(format!("smc.{i}"), v(x.value));
    }
    let f = &r.fit;
    for (name, s) in [
        ("cmin", f.cmin), ("p", f.p), ("cmin_df", f.cmin_df), ("rmr", f.rmr), ("gfi", f.gfi), ("agfi", f.agfi),
        ("pgfi", f.pgfi), ("nfi", f.nfi), ("rfi", f.rfi), ("ifi", f.ifi), ("tli", f.tli), ("cfi", f.cfi),
        ("pratio", f.pratio), ("pnfi", f.pnfi), ("pcfi", f.pcfi), ("ncp", f.ncp), ("ncp_lo90", f.ncp_lo90),
        ("ncp_hi90", f.ncp_hi90), ("fmin", f.fmin), ("f0", f.f0), ("f0_lo90", f.f0_lo90), ("f0_hi90", f.f0_hi90),
        ("rmsea", f.rmsea), ("rmsea_lo90", f.rmsea_lo90), ("rmsea_hi90", f.rmsea_hi90), ("pclose", f.pclose),
        ("aic", f.aic), ("bcc", f.bcc), ("bic", f.bic), ("caic", f.caic), ("ecvi", f.ecvi), ("ecvi_lo90", f.ecvi_lo90),
        ("ecvi_hi90", f.ecvi_hi90), ("mecvi", f.mecvi), ("hoelter_05", f.hoelter05), ("hoelter_01", f.hoelter01),
        ("independence_cmin", f.independence_cmin),
    ] {
        put(format!("fit.{name}"), v(s));
    }
    for (name, n) in [("npar", f.npar), ("df", f.df), ("independence_npar", f.independence_npar), ("independence_df", f.independence_df)] {
        put(format!("fit.{name}"), V::Num(n as f64));
    }
    for (k, c) in r.notes.iter().enumerate() {
        put(format!("note.{k}"), V::Word(word(*c)));
    }
    for (k, e) in r.exclusions.iter().enumerate() {
        let column = e.params.values().next().map(|p| match p {
            ParamValue::Int(n) => n.to_string(),
            _ => String::new(),
        });
        put(format!("exclusion.{k}"), V::Word(format!("{}:{}:{}", word(e.code), column.unwrap_or_default(), e.count)));
    }
    out
}

/// A key's family.
fn family(key: &str) -> &'static str {
    if let Some(name) = key.strip_prefix("fit.") {
        return match name {
            "npar" | "df" | "independence_npar" | "independence_df" | "hoelter_05" | "hoelter_01" => "exact",
            "ncp_lo90" | "ncp_hi90" | "f0_lo90" | "f0_hi90" | "rmsea_lo90" | "rmsea_hi90" | "ecvi_lo90" | "ecvi_hi90" | "pclose" => "fit_interval",
            // RMR is a residual covariance, in the data's variance units: at an exact
            // fit it is the arithmetic's rounding of 0 (5.6e-15 in R, 3.5e-14 in Gaply
            // on phi_not_pd, whose variances are near 100), where an error relative
            // to it means nothing; it is compared against the case's largest variance.
            "rmr" => "fit_rmr",
            "gfi" | "agfi" | "pgfi" | "nfi" | "rfi" | "ifi" | "tli" | "cfi" | "pratio" | "pnfi" | "pcfi" | "rmsea" => "fit_index",
            _ => "fit_chi",
        };
    }
    if key == "rows_used" || key.starts_with("note.") || key.starts_with("exclusion.") {
        return "exact";
    }
    if key.starts_with("smc.") || key.ends_with(".standardized") || key.ends_with(".correlation") {
        return "standardized";
    }
    for (end, f) in [(".estimate", "estimate"), (".se", "se"), (".cr", "cr"), (".p", "p")] {
        if key.ends_with(end) {
            return f;
        }
    }
    panic!("no family for {key}")
}

/// The case's largest fitted variance, λ_i² φ_f(i) + θ_i: RMR's scale.
fn rmr_scale(r: &CfaOutcome) -> f64 {
    let num = |s: Stat| if let Stat::Value(x) = s { x } else { 0.0 };
    r.loadings
        .iter()
        .zip(&r.error_variances)
        .map(|(l, e)| num(l.estimate).powi(2) * num(r.factor_variances[l.factor as usize - 1].estimate) + num(e.estimate))
        .fold(0.0, f64::max)
}

/// The error of Gaply's `a` against R's `b` for `key` of `family` (decision 746), a
/// relative error but where the quantity's own conditioning or rounding says
/// otherwise (measured by the hunt of 13 Nov 2026):
/// * a P below the smallest normal double on both sides agrees at rounding (a
///   subnormal carries fewer digits than a gate asks; R gave 4e-323 and Gaply
///   4.4e-323), and above it a P's relative error is divided by max(1, C.R.²):
///   d ln p / d ln z is about −z², so a P is as accurate as its C.R. allows;
/// * CMIN, NCP, FMIN and F0 within (N − 1)(p + 1) · 1e-13 of 0 on both sides
///   agree at rounding: at an exact fit F is the arithmetic's rounding of 0
///   (3.6e-15 in R, 7.1e-15 in Gaply);
/// * RMR against the case's largest variance (`rmr_scale`);
/// * otherwise relative, absolute below the family's floor.
pub fn error_of(family: &str, key: &str, a: f64, b: f64, r: &CfaOutcome, keys: &BTreeMap<String, V>) -> f64 {
    if family == "p" {
        if a.abs() < f64::MIN_POSITIVE && b.abs() < f64::MIN_POSITIVE {
            return 0.0;
        }
        let cr_key = format!("{}cr", key.strip_suffix('p').expect("a P key"));
        let z = match keys.get(&cr_key) {
            Some(V::Num(z)) => *z,
            _ => 1.0,
        };
        let rel = if b == 0.0 { a.abs() } else { (a - b).abs() / b.abs() };
        return rel / (z * z).max(1.0);
    }
    if family == "fit_chi" && ["fit.cmin", "fit.ncp", "fit.fmin", "fit.f0"].contains(&key) {
        let rounding = (r.rows_used as f64 - 1.0) * (r.items.len() as f64 + 1.0) * 1e-13;
        if a.abs() <= rounding && b.abs() <= rounding {
            return 0.0;
        }
    }
    let scale = if family == "fit_rmr" { rmr_scale(r) } else { b.abs().max(floor(family)) };
    if scale == 0.0 {
        a.abs()
    } else {
        (a - b).abs() / scale
    }
}

fn parse(value: &str) -> V {
    value.parse::<f64>().map(V::Num).unwrap_or_else(|_| V::Word(value.to_string()))
}

#[test]
fn every_number_of_every_case_is_within_its_gate_of_r() {
    let (cases_header, cases) = header_lines("cases.tsv");
    assert_eq!(cases_header, ["case", "data", "factors", "correlated", "codes"]);
    let text = std::fs::read_to_string(format!("{DIR}/expected.tsv")).unwrap();
    let provenance: Vec<&str> = text.lines().take_while(|l| l.starts_with('#')).collect();
    assert!(provenance.iter().any(|h| h.contains(R_VERSION)), "expected.tsv: not written with {R_VERSION}");
    assert!(provenance.iter().any(|h| h.contains(LAVAAN)), "expected.tsv: not written with {LAVAAN}");
    let (eh, rows) = header_lines("expected.tsv");
    assert_eq!(eh, ["case", "key", "value"]);
    let mut by_case: BTreeMap<String, Vec<(String, String)>> = BTreeMap::new();
    for r in rows {
        by_case.entry(r[0].clone()).or_default().push((r[1].clone(), r[2].clone()));
    }
    let mut worst: BTreeMap<&str, (f64, String)> = BTreeMap::new();
    let mut failures = Vec::new();
    let mut compared = 0usize;
    for row in &cases {
        let name = &row[0];
        let want = by_case.get(name).unwrap_or_else(|| panic!("{name}: no expected rows"));
        let (d, params) = case_input(row);
        match compute(&d, &params) {
            Err(e) => {
                let mut got = word(e.code);
                for (k, p) in &e.params {
                    if let ParamValue::Int(n) = p {
                        got.push_str(&format!(":{}={n}", serde_json::to_value(k).unwrap().as_str().unwrap()));
                    }
                }
                assert_eq!(want.len(), 1, "{name}: a refusal has one expected row");
                assert_eq!(want[0].0, "refusal", "{name}: Gaply refused ({got}) where R gave a result");
                assert_eq!(got, want[0].1, "{name}: the refusal");
                compared += 1;
            }
            Ok(r) => {
                let got = keyed(&r);
                let notes = want.iter().filter(|(k, _)| k.starts_with("note.")).count();
                assert_eq!(r.notes.len(), notes, "{name}: notes {:?}", r.notes);
                let exclusions = want.iter().filter(|(k, _)| k.starts_with("exclusion.")).count();
                assert_eq!(r.exclusions.len(), exclusions, "{name}: exclusions");
                for (key, value) in want {
                    let g = got.get(key).unwrap_or_else(|| panic!("{name}: Gaply has no {key}"));
                    let w = parse(value);
                    let fam = family(key);
                    compared += 1;
                    match (g, &w) {
                        (V::Num(a), V::Num(b)) => {
                            let err = error_of(fam, key, *a, *b, &r, &got);
                            if fam == "exact" {
                                if a != b {
                                    failures.push(format!("{name} {key}: R {b}, Gaply {a}"));
                                }
                                continue;
                            }
                            let e = worst.entry(fam).or_insert((0.0, String::new()));
                            if err > e.0 {
                                *e = (err, format!("{name} {key} (R {b:e}, Gaply {a:e})"));
                            }
                            if err > gate(fam) {
                                failures.push(format!("{name} {key}: R {b:e}, Gaply {a:e}, {err:e} over {fam}'s gate {:e}", gate(fam)));
                            }
                        }
                        _ => {
                            if *g != w {
                                failures.push(format!("{name} {key}: R {w:?}, Gaply {g:?}"));
                            }
                        }
                    }
                }
            }
        }
    }
    for (fam, (e, at)) in &worst {
        println!("cfa golden {fam}: worst {e:.3e} at {at}; gate {:e}", gate(fam));
    }
    println!("cfa golden: {} cases, {compared} values compared", cases.len());
    assert_eq!(cases.len(), 14, "the script's cases, none skipped");
    assert_eq!(compared, 3541, "every expected row compared");
    assert!(failures.is_empty(), "{} failures:\n{}", failures.len(), failures.join("\n"));
}
