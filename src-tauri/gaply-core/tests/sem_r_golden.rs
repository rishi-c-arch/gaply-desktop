//! **`gaply_core::stats_sem` against R (Research OS, Phase 9C-4, decision 800).**
//! `tests/fixtures/stats_sem/make_sem_fixtures.R`, run as /usr/local/bin/Rscript
//! with lavaan 0.7-3, fits each case with lavaan, refines it by its own Fisher
//! scoring on the RAM matrices to a Newton decrement of 1e-20 and computes every
//! statistic, effect and fit measure from decisions 745 and 799's definitions
//! (cross-checked against lavaan there); this test computes each case with
//! Gaply and compares every number within its family's gate, ten times the
//! family's measured error, and every code, note, exclusion and refusal
//! exactly. Two cases are built by a formula identical in R and here (the
//! confirmatory factor analysis's), and have no file; three cases read the CFA
//! golden's data files where they are.

use std::collections::BTreeMap;

use gaply_core::contracts::codes::Closed;
use gaply_core::contracts::ParamValue;
use gaply_core::stats_l1::dataset::{build_dataset, DataFingerprint, Dataset, RawTable, TableRef};
use gaply_core::stats_sem::{compute, SemOutcome, SemParams, SemVariable, Stat};

const DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/stats_sem");
const PROVENANCE: &str = "/usr/local/bin/Rscript make_sem_fixtures.R";
const CFA_PROVENANCE: &str = "/usr/local/bin/Rscript make_cfa_fixtures.R";
const R_VERSION: &str = "R version 4.6.1";
const LAVAAN: &str = "lavaan 0.7.3";

/// Each family's gate, relative (absolute where R's value is 0): ten times the
/// largest error measured over every case, rounded up (macOS aarch64, 15 Nov
/// 2026), printed by the test below.
fn gate(family: &str) -> f64 {
    match family {
        "estimate" => GATES[0],
        "se" => GATES[1],
        "cr" => GATES[2],
        "p" => GATES[3],
        "standardized" => GATES[4],
        "effect" => GATES[5],
        "fit_chi" => GATES[6],
        "fit_index" => GATES[7],
        "fit_interval" => GATES[8],
        "fit_rmr" => GATES[9],
        other => panic!("no gate for {other}"),
    }
}

/// Measured with every gate at 0 (logs/p2_measure2_1794724243.log):
///
/// | family | worst | where |
/// |---|---|---|
/// | estimate | 2.120e-8 | example7, academic ← attract (−0.002, its C.R. −0.04: a flat likelihood both fits stop on at a decrement of 1e-20) |
/// | se | 3.601e-10 | missing_and_codes, a variance |
/// | cr | 2.108e-8 | example7, the same weight |
/// | p | 9.284e-10 | missing_and_codes, a variance |
/// | standardized | 2.120e-8 | example7, the same weight |
/// | effect | 2.120e-8 | example7, the same weight's direct effect |
/// | fit_chi | 6.687e-12 | formula_30's P |
/// | fit_index | 2.345e-12 | missing_and_codes's AGFI |
/// | fit_interval | 9.038e-13 | wheaton_a's PCLOSE |
/// | fit_rmr | 6.864e-12 | missing_and_codes's RMR |
const GATES: [f64; 10] = [2.2e-7, 3.7e-9, 2.2e-7, 9.3e-9, 2.2e-7, 2.2e-7, 6.7e-11, 2.4e-11, 9.1e-12, 6.9e-11];

/// Below this, R's value is taken as rounding and the error as absolute. P
/// values are relative at any size.
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
    build_dataset(RawTable::from_rows(headers, rows), DataFingerprint::of_bytes(label.as_bytes(), "sem golden", TableRef::FirstTable)).expect("dataset")
}

/// make_sem_fixtures.R's formula_data, the confirmatory factor analysis's:
/// K factors over N rows and p columns, column j on factor (j − 1) mod K.
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

/// A data file: this golden's own under data/, or the CFA golden's by its path.
pub fn data_file(name: &str) -> Dataset {
    let (path, provenance_line) = match name.strip_prefix("../") {
        Some(rest) => (format!("{DIR}/../{rest}"), CFA_PROVENANCE),
        None => (format!("{DIR}/data/{name}"), PROVENANCE),
    };
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{name}: {e}"));
    let provenance: Vec<&str> = text.lines().take_while(|l| l.starts_with('#')).collect();
    assert!(provenance.iter().any(|h| h.contains(provenance_line)), "{name}: not written by {provenance_line}");
    let mut lines = text.lines().filter(|l| !l.starts_with('#'));
    let headers: Vec<String> = lines.next().unwrap().split(',').map(str::to_string).collect();
    let rows: Vec<Vec<String>> = lines.map(|l| l.split(',').map(str::to_string).collect()).collect();
    dataset(headers, rows, name)
}

/// A case's dataset and spec, from cases.tsv's row: the spec is Gaply's own
/// JSON, as the screen would send it.
pub fn case_input(row: &[String]) -> (Dataset, SemParams) {
    let d = match row[1].strip_prefix("formula:") {
        Some(spec) => {
            let v: Vec<u64> = spec.split(':').map(|x| x.parse().unwrap()).collect();
            formula_data(v[0], v[1], v[2])
        }
        None => data_file(&row[1]),
    };
    (d, serde_json::from_str(&row[2]).unwrap_or_else(|e| panic!("{}: the spec: {e}", row[0])))
}

fn word(c: gaply_core::stats_sem::SemCode) -> String {
    c.wire().strip_prefix("stats.sem.").unwrap().to_string()
}

fn v(s: Stat) -> V {
    match s {
        Stat::Value(x) => V::Num(x),
        Stat::NotComputable(c) => V::Word(word(c)),
    }
}

fn variable(x: SemVariable) -> String {
    match x {
        SemVariable::Factor(n) => format!("f{n}"),
        SemVariable::Column(a) => a.0.name(),
    }
}

/// Gaply's result under make_sem_fixtures.R's keys.
pub fn keyed(r: &SemOutcome) -> BTreeMap<String, V> {
    let mut out = BTreeMap::new();
    let mut put = |k: String, x: V| {
        out.insert(k, x);
    };
    put("rows_used".into(), V::Num(r.rows_used as f64));
    for (i, w) in r.weights.iter().enumerate() {
        for (f, s) in [("estimate", w.estimate), ("se", w.se), ("cr", w.cr), ("p", w.p), ("standardized", w.standardized)] {
            put(format!("weight.{i}.{f}"), v(s));
        }
    }
    for (i, c) in r.covariances.iter().enumerate() {
        for (f, s) in [("estimate", c.estimate), ("se", c.se), ("cr", c.cr), ("p", c.p), ("correlation", c.correlation)] {
            put(format!("cov.{i}.{f}"), v(s));
        }
    }
    for (i, x) in r.variances.iter().enumerate() {
        for (f, s) in [("estimate", x.estimate), ("se", x.se), ("cr", x.cr), ("p", x.p)] {
            put(format!("var.{i}.{f}"), v(s));
        }
    }
    for (i, x) in r.smc.iter().enumerate() {
        put(format!("smc.{i}"), v(x.value));
    }
    for (i, e) in r.effects.iter().enumerate() {
        put(format!("effect.{i}.ends"), V::Word(format!("{}<{}", variable(e.to), variable(e.from))));
        for (f, s) in [
            ("total", e.total),
            ("direct", e.direct),
            ("indirect", e.indirect),
            ("total_standardized", e.total_standardized),
            ("direct_standardized", e.direct_standardized),
            ("indirect_standardized", e.indirect_standardized),
        ] {
            put(format!("effect.{i}.{f}"), v(s));
        }
    }
    for (prefix, f) in [("fit", &r.fit.model), ("fit_saturated", &r.fit.saturated), ("fit_independence", &r.fit.independence)] {
        for (name, s) in [
            ("cmin", f.cmin), ("p", f.p), ("cmin_df", f.cmin_df), ("rmr", f.rmr), ("gfi", f.gfi), ("agfi", f.agfi),
            ("pgfi", f.pgfi), ("nfi", f.nfi), ("rfi", f.rfi), ("ifi", f.ifi), ("tli", f.tli), ("cfi", f.cfi),
            ("pratio", f.pratio), ("pnfi", f.pnfi), ("pcfi", f.pcfi), ("ncp", f.ncp), ("ncp_lo90", f.ncp_lo90),
            ("ncp_hi90", f.ncp_hi90), ("fmin", f.fmin), ("f0", f.f0), ("f0_lo90", f.f0_lo90), ("f0_hi90", f.f0_hi90),
            ("rmsea", f.rmsea), ("rmsea_lo90", f.rmsea_lo90), ("rmsea_hi90", f.rmsea_hi90), ("pclose", f.pclose),
            ("aic", f.aic), ("bcc", f.bcc), ("bic", f.bic), ("caic", f.caic), ("ecvi", f.ecvi), ("ecvi_lo90", f.ecvi_lo90),
            ("ecvi_hi90", f.ecvi_hi90), ("mecvi", f.mecvi), ("hoelter_05", f.hoelter05), ("hoelter_01", f.hoelter01),
        ] {
            put(format!("{prefix}.{name}"), v(s));
        }
        for (name, n) in [("npar", f.npar), ("df", f.df)] {
            put(format!("{prefix}.{name}"), V::Num(n as f64));
        }
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
    if let Some(name) = key.strip_prefix("fit.").or_else(|| key.strip_prefix("fit_saturated.")).or_else(|| key.strip_prefix("fit_independence.")) {
        return match name {
            "npar" | "df" | "hoelter_05" | "hoelter_01" => "exact",
            "ncp_lo90" | "ncp_hi90" | "f0_lo90" | "f0_hi90" | "rmsea_lo90" | "rmsea_hi90" | "ecvi_lo90" | "ecvi_hi90" | "pclose" => "fit_interval",
            "rmr" => "fit_rmr",
            "gfi" | "agfi" | "pgfi" | "nfi" | "rfi" | "ifi" | "tli" | "cfi" | "pratio" | "pnfi" | "pcfi" | "rmsea" => "fit_index",
            _ => "fit_chi",
        };
    }
    if key == "rows_used" || key.starts_with("note.") || key.starts_with("exclusion.") || key.ends_with(".ends") {
        return "exact";
    }
    if key.starts_with("smc.") || key.ends_with("standardized") || key.ends_with(".correlation") {
        return "standardized";
    }
    if key.starts_with("effect.") {
        return "effect";
    }
    for (end, f) in [(".estimate", "estimate"), (".se", "se"), (".cr", "cr"), (".p", "p")] {
        if key.ends_with(end) {
            return f;
        }
    }
    panic!("no family for {key}")
}

/// RMR's scale: the largest variance estimate of the result.
fn rmr_scale(r: &SemOutcome) -> f64 {
    r.variances.iter().map(|x| if let Stat::Value(v) = x.estimate { v.abs() } else { 0.0 }).fold(0.0, f64::max)
}

/// The error of Gaply's `a` against R's `b` for `key` of `family`, as the CFA
/// golden's: a P relative over max(1, C.R.²) and equal below the smallest normal
/// double; an effect relative to the larger of its own total and direct
/// effects; CMIN, NCP, FMIN and F0 within (N − 1)(p + 1) · 1e-13 of 0 on both
/// sides equal; RMR against the largest variance; otherwise relative, absolute
/// below the family's floor.
pub fn error_of(family: &str, key: &str, a: f64, b: f64, r: &SemOutcome, keys: &BTreeMap<String, V>) -> f64 {
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
        let rounding = (r.rows_used as f64 - 1.0) * (r.columns.len() as f64 + 1.0) * 1e-13;
        if a.abs() <= rounding && b.abs() <= rounding {
            return 0.0;
        }
    }
    // An effect is compared against the larger of its total and direct effects: an
    // indirect effect that is 0 is the rounding of total − direct (R −2.2e-16 and Gaply
    // +2.2e-16 on poldem), where an error relative to it means nothing.
    if let Some(rest) = key.strip_prefix("effect.") {
        let (index, field) = rest.split_once('.').expect("an effect key");
        let suffix = if field.ends_with("_standardized") { "_standardized" } else { "" };
        let size = |name: &str| match keys.get(&format!("effect.{index}.{name}{suffix}")) {
            Some(V::Num(x)) => x.abs(),
            _ => 0.0,
        };
        let scale = b.abs().max(size("total")).max(size("direct")).max(floor(family));
        return (a - b).abs() / scale;
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
    assert_eq!(cases_header, ["case", "data", "spec"]);
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
                // Every family counted, so an extra row of Gaply's fails too (the review of 9C-4).
                for (prefix, what) in [("note.", "notes"), ("exclusion.", "exclusions"), ("effect.", "effects"), ("weight.", "weights"), ("cov.", "covariances"), ("var.", "variances"), ("smc.", "squared multiple correlations")] {
                    let w = want.iter().filter(|(k, _)| k.starts_with(prefix)).count();
                    let g = got.keys().filter(|k| k.starts_with(prefix)).count();
                    assert_eq!(g, w, "{name}: {what}");
                }
                for (key, value) in want {
                    let g = got.get(key).unwrap_or_else(|| panic!("{name}: Gaply has no {key}"));
                    let w = parse(value);
                    let fam = family(key);
                    compared += 1;
                    match (g, &w) {
                        (V::Num(a), V::Num(b)) => {
                            if fam == "exact" {
                                if a != b {
                                    failures.push(format!("{name} {key}: R {b}, Gaply {a}"));
                                }
                                continue;
                            }
                            let err = error_of(fam, key, *a, *b, &r, &got);
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
        println!("sem golden {fam}: worst {e:.3e} at {at}; gate {:e}", gate(fam));
    }
    println!("sem golden: {} cases, {compared} values compared", cases.len());
    assert_eq!(cases.len(), 21, "the script's cases, none skipped");
    assert_eq!(compared, COMPARED, "every expected row compared");
    assert!(failures.is_empty(), "{} failures:\n{}", failures.len(), failures.join("\n"));
}

/// Every expected row: make_sem_fixtures.R's count, 6,458.
const COMPARED: usize = 6458;
