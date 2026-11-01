//! **`gaply_core::stats_corr` against R 4.6.1 (Research OS, Phase 8H,
//! decisions 338 to 341), over `tests/fixtures/stats_corr/` (written by
//! `/usr/local/bin/Rscript make_corr_fixtures.R`).**
//!
//! Every gate is ten times the largest error measured over every row
//! (decision 341), printed with the measurement:
//!
//! ```text
//! cargo test -p gaply_core --test corr_r_golden -- --nocapture
//! ```
//!
//! A p is gated relatively where both values are at least
//! `f64::MIN_POSITIVE`; below that, both must be below it. The gates are
//! asserted on every platform (decision 277).

use std::collections::BTreeMap;

use gaply_core::contracts::codes::Closed;
use gaply_core::stats_corr::{
    compute, Bounded, Coefficient, CorrCode, CorrOutcome, CorrParams, Interval, MissingHandling,
    PairOutcome, Stat, TestStatistic,
};
use gaply_core::stats_desc::Alias;
use gaply_core::stats_dist::normal_two_tailed;
use gaply_core::stats_l1::dataset::{build_dataset, DataFingerprint, Dataset, RawTable, TableRef};
use gaply_core::stats_verify::{pearson, pearson_clamped, RangeFault};

const DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/stats_corr");
const PROVENANCE: &str = "/usr/local/bin/Rscript make_corr_fixtures.R";
const R_VERSION: &str = "R version 4.6.1";

/// `normal_two_tailed` against R's `2 * pnorm(-abs(z))`, relative, where
/// both are at least `f64::MIN_POSITIVE`. Measured 1 Nov 2026, macOS
/// aarch64: 9.958e-11 at z 0.75 (statrs's `erfc`, as decision 325 found for
/// the studentized range), over 130 rows down to p 9.2e-308; 10 rows are
/// both below `MIN_POSITIVE`.
const NORMAL_TAIL_GATE: f64 = 1e-9;

/// The gates per family, each ten times the largest error measured over
/// every case (decision 341), rounded up at one significant figure;
/// relative unless named absolute. Measured 1 Nov 2026, macOS aarch64, over
/// 2,621 values of 23 cases:
///
/// * `estimate.near1` is ABSOLUTE, for |R's coefficient| above 1 − 1e-9.
///   Every such row is a perfect ±1 on both sides (Gaply's three
///   predicates; R's value, or the exact ±1 the generator writes where R's
///   ranks are identical and its arithmetic gave 1 − 2 ulp,
///   measurements.txt), so it measured 0 and its gate is 0.
/// * `z`'s worst is the 100,000-row pair, against the generator's z from an
///   exact S and decision 339's variance in R doubles. That value is itself
///   2.2e-10 from the variance in exact fractions (297.53320152559661,
///   computed outside R as a diagnostic, 1 Nov 2026), and Gaply's z, whose
///   sums are exact integers, is that value: the 2.2e-10 is R's rounding.
///   cor.test's own z there is 1.8e-7 off (measurements.txt).
/// * `t` and `p` worst at |r| = 0.999999, where 1 − r² amplifies a last-
///   place difference in r by about 1e6.
const GATES: &[(&str, f64)] = &[
    ("desc.mean", 2e-13),    // 1.66e-14, big desc.0.mean
    ("desc.sd", 3e-13),      // 2.12e-14, big desc.0.sd
    ("estimate", 9e-15),     // 8.34e-16, k10_n30 pearson.3-7
    ("estimate.near1", 0.0), // ABSOLUTE 0, every perfect coefficient
    ("t", 2e-9),             // 1.12e-10, near_one pearson.0-1
    ("z", 3e-9),             // 2.20e-10, big kendall.0-1 (see above)
    ("p", 4e-8),             // 3.13e-9, near_one pearson.0-1 (p 2.2e-81)
    ("ci", 7e-13),           // 6.52e-14, likert spearman.1-2.ci_upper
];

/// A fixture's rows, split on tabs, after its `#` header (which must name
/// the generator and R's version) and its column header.
fn table(name: &str, header: &str) -> Vec<Vec<String>> {
    let text = std::fs::read_to_string(format!("{DIR}/{name}")).expect("fixture");
    let mut lines = text.lines();
    let provenance: Vec<&str> = lines.by_ref().take(2).collect();
    assert!(provenance[0].contains(PROVENANCE), "{name}: {provenance:?}");
    assert!(provenance[1].contains(R_VERSION), "{name}: {provenance:?}");
    let rest: Vec<&str> = lines.filter(|l| !l.starts_with('#')).collect();
    assert_eq!(rest[0], header, "{name}");
    rest[1..]
        .iter()
        .map(|l| l.split('\t').map(str::to_string).collect())
        .collect()
}

/// The relative error of `got`, or `None` where both are below
/// `f64::MIN_POSITIVE` in magnitude (then both must be).
fn p_error(got: f64, want: f64) -> Result<Option<f64>, String> {
    let tiny = |v: f64| v.abs() < f64::MIN_POSITIVE;
    match (tiny(got), tiny(want)) {
        (true, true) => Ok(None),
        (false, false) => Ok(Some((got - want).abs() / want.abs())),
        _ => Err(format!(
            "got {got:e}, want {want:e}: one is below f64::MIN_POSITIVE and one is not"
        )),
    }
}

#[test]
fn normal_two_tailed_is_within_its_gate_of_r_down_to_p_1e_300() {
    let rows = table("normal_tail.tsv", "z\tp");
    assert!(rows.len() >= 100, "{} rows", rows.len());
    let mut worst = (0.0f64, 0.0f64);
    let mut below = 0;
    let mut smallest = 1.0f64;
    for r in &rows {
        let z: f64 = r[0].parse().unwrap();
        let want: f64 = r[1].parse().unwrap();
        for sign in [1.0, -1.0] {
            let got = normal_two_tailed(sign * z);
            match p_error(got, want).unwrap_or_else(|e| panic!("z {z}: {e}")) {
                Some(e) => {
                    if want > 0.0 {
                        smallest = smallest.min(want);
                    }
                    if e > worst.0 {
                        worst = (e, z);
                    }
                }
                None => below += 1,
            }
        }
    }
    println!(
        "normal_two_tailed: {} rows, worst relative error {:.3e} at z {}, {} rows both below MIN_POSITIVE, smallest gated p {:e}",
        rows.len(),
        worst.0,
        worst.1,
        below,
        smallest
    );
    assert!(
        smallest < 1e-300,
        "the grid must reach p 1e-300: {smallest:e}"
    );
    assert!(worst.0 <= NORMAL_TAIL_GATE, "{:e} over the gate", worst.0);
    assert_eq!(normal_two_tailed(0.0), 1.0);
}

/// The columns of a fixture data file, by header, every cell a number.
fn columns(file: &str) -> Vec<(String, Vec<f64>)> {
    let text = std::fs::read_to_string(format!("{DIR}/{file}")).expect("data file");
    let mut lines = text.lines().filter(|l| !l.starts_with('#'));
    let headers: Vec<String> = lines
        .next()
        .unwrap()
        .split(',')
        .map(str::to_string)
        .collect();
    let mut cols: Vec<(String, Vec<f64>)> = headers.into_iter().map(|h| (h, Vec::new())).collect();
    for l in lines {
        for (c, cell) in cols.iter_mut().zip(l.split(',')) {
            c.1.push(cell.parse().expect("a number"));
        }
    }
    cols
}

/// Decision 339: on collinear data for which `stats_verify`'s Pearson r is
/// 1 + 1 ulp, `pearson` refuses (its t is the root of a negative number),
/// and `pearson_clamped` gives r = 1 exactly with an infinite t, as R's
/// `cor` gives 1 there (measurements.txt).
#[test]
fn pearson_clamped_gives_exactly_one_where_pearson_r_is_one_ulp_above() {
    let cols = columns("data_ulp.csv");
    let (x, y) = (&cols[1].1, &cols[2].1);
    let refused = pearson(x, y).expect_err("1 + 1 ulp has no real t");
    assert_eq!(RangeFault::of(&refused), Some(RangeFault::Overflow));
    let c = pearson_clamped(x, y).expect("clamped");
    assert_eq!(c.coefficient, 1.0);
    assert!(
        c.statistic.is_infinite() && c.statistic > 0.0,
        "{}",
        c.statistic
    );
    assert_eq!(c.df, 3.0);
    let mirrored: Vec<f64> = y.iter().map(|v| -v).collect();
    let c = pearson_clamped(x, &mirrored).expect("clamped");
    assert_eq!(c.coefficient, -1.0);
    assert!(c.statistic.is_infinite() && c.statistic < 0.0);
}

/* ---- the cases ------------------------------------------------------------ */

struct Case {
    name: String,
    file: String,
    variables: Vec<String>,
    coefficients: Vec<Coefficient>,
    missing: MissingHandling,
    codes: Vec<f64>,
}

fn cases() -> Vec<Case> {
    table(
        "cases.tsv",
        "case\tfile\tvariables\tcoefficients\tmissing\tmissing_codes",
    )
    .into_iter()
    .map(|f| Case {
        name: f[0].clone(),
        file: f[1].clone(),
        variables: f[2].split(' ').map(str::to_string).collect(),
        coefficients: f[3]
            .split(' ')
            .map(|c| match c {
                "pearson" => Coefficient::Pearson,
                "spearman" => Coefficient::Spearman,
                "kendall" => Coefficient::Kendall,
                other => panic!("coefficient {other}"),
            })
            .collect(),
        missing: match f[4].as_str() {
            "pairwise" => MissingHandling::Pairwise,
            "listwise" => MissingHandling::Listwise,
            other => panic!("missing {other}"),
        },
        codes: if f[5] == "-" {
            Vec::new()
        } else {
            f[5].split(' ').map(|v| v.parse().unwrap()).collect()
        },
    })
    .collect()
}

fn expected() -> BTreeMap<String, BTreeMap<String, String>> {
    let mut out: BTreeMap<String, BTreeMap<String, String>> = BTreeMap::new();
    for f in table("expected.tsv", "case\tkey\tvalue") {
        out.entry(f[0].clone())
            .or_default()
            .insert(f[1].clone(), f[2].clone());
    }
    out
}

fn dataset_from(headers: Vec<String>, rows: Vec<Vec<String>>, bytes: &[u8]) -> Dataset {
    build_dataset(
        RawTable::from_rows(headers, rows),
        DataFingerprint::of_bytes(bytes, "corr golden", TableRef::FirstTable),
    )
    .expect("dataset")
}

/// The 100,000-row pair, from the generator's formula: the same IEEE
/// operations, so the same doubles, written shortest-round-trip.
fn big() -> Dataset {
    let rows = (0u64..100_000)
        .map(|i| {
            let x = ((i * 7919) % 10007) as f64 / 100.0;
            let y = x * 0.3 + ((i * 104729) % 1009) as f64 / 50.0;
            vec![(i + 1).to_string(), format!("{x:?}"), format!("{y:?}")]
        })
        .collect();
    dataset_from(
        vec!["id".into(), "x".into(), "y".into()],
        rows,
        b"big by formula",
    )
}

fn data(case: &Case) -> Dataset {
    if case.file == "-" {
        assert_eq!(case.name, "big");
        return big();
    }
    let bytes = std::fs::read(format!("{DIR}/{}", case.file)).expect("data file");
    let text = String::from_utf8(bytes.clone()).expect("utf-8");
    let mut lines = text.lines().filter(|l| !l.starts_with('#'));
    let headers: Vec<String> = lines
        .next()
        .expect("header")
        .split(',')
        .map(str::to_string)
        .collect();
    let rows = lines
        .map(|l| l.split(',').map(str::to_string).collect())
        .collect();
    dataset_from(headers, rows, &bytes)
}

fn alias(d: &Dataset, header: &str) -> Alias {
    Alias(
        d.schema
            .columns
            .iter()
            .find(|c| c.header == header)
            .unwrap_or_else(|| panic!("no column {header}"))
            .alias,
    )
}

fn params(case: &Case, d: &Dataset) -> CorrParams {
    CorrParams {
        variables: case.variables.iter().map(|h| alias(d, h)).collect(),
        coefficients: case.coefficients.clone(),
        missing: case.missing,
        missing_codes: case.codes.clone(),
    }
}

/* ---- what a result says, by the fixture's keys ----------------------------- */

enum Got {
    Number(f64),
    Count(u64),
    Infinite(f64),
    Nc(CorrCode),
}

fn coefficient_name(c: Coefficient) -> &'static str {
    match c {
        Coefficient::Pearson => "pearson",
        Coefficient::Spearman => "spearman",
        Coefficient::Kendall => "kendall",
    }
}

fn stat(s: Stat) -> Got {
    match s {
        Stat::Value(v) => Got::Number(v),
        Stat::NotComputable(c) => Got::Nc(c),
    }
}

fn keys(r: &CorrOutcome, variables: &[Alias]) -> BTreeMap<String, Got> {
    let index = |a: Alias| {
        variables
            .iter()
            .position(|v| *v == a)
            .expect("a case variable")
    };
    let mut m = BTreeMap::new();
    m.insert("rows_used".to_string(), Got::Count(r.rows_used));
    for e in &r.exclusions {
        let name = e
            .code
            .wire()
            .strip_prefix("stats.corr.")
            .unwrap()
            .to_string();
        let key = match e.params.get(&gaply_core::contracts::ParamKey::Column) {
            Some(&gaply_core::contracts::ParamValue::Int(c)) => {
                let a = variables
                    .iter()
                    .position(|v| i64::from(v.0 .0) == c)
                    .expect("a case variable");
                format!("excl.{name}.{a}")
            }
            _ => format!("excl.{name}"),
        };
        m.insert(key, Got::Count(e.count));
    }
    for (i, d) in r.descriptives.iter().enumerate() {
        assert_eq!(index(d.variable), i, "descriptives in the variables' order");
        m.insert(format!("desc.{i}.n"), Got::Count(d.n));
        m.insert(format!("desc.{i}.mean"), stat(d.mean));
        m.insert(format!("desc.{i}.sd"), stat(d.sd));
    }
    for table in &r.coefficients {
        let name = coefficient_name(table.coefficient);
        for pair in &table.pairs {
            let key = format!("{name}.{}-{}", index(pair.first), index(pair.second));
            m.insert(format!("{key}.n"), Got::Count(pair.n));
            match &pair.outcome {
                PairOutcome::NotComputable(c) => {
                    m.insert(format!("{key}.nc"), Got::Nc(*c));
                }
                PairOutcome::Computed(s) => {
                    m.insert(format!("{key}.estimate"), Got::Number(s.estimate));
                    match s.test {
                        TestStatistic::T(t) => {
                            m.insert(
                                format!("{key}.statistic"),
                                match t.t {
                                    Bounded::Value(v) => Got::Number(v),
                                    Bounded::Infinite(_) => Got::Infinite(s.estimate.signum()),
                                },
                            );
                            m.insert(format!("{key}.df"), Got::Number(t.df));
                        }
                        TestStatistic::Z(z) => {
                            m.insert(format!("{key}.statistic"), Got::Number(z));
                        }
                    }
                    m.insert(format!("{key}.p"), Got::Number(s.p));
                    match s.interval {
                        Interval::Computed { lower, upper } => {
                            m.insert(format!("{key}.ci_lower"), Got::Number(lower));
                            m.insert(format!("{key}.ci_upper"), Got::Number(upper));
                        }
                        Interval::NotComputable(c) => {
                            m.insert(format!("{key}.ci"), Got::Nc(c));
                        }
                    }
                }
            }
        }
    }
    m
}

/// A key's gate family.
fn family(key: &str, want: f64) -> &'static str {
    let tail = key.rsplit('.').next().unwrap_or("");
    if key.starts_with("desc.") {
        return if tail == "mean" {
            "desc.mean"
        } else {
            "desc.sd"
        };
    }
    match tail {
        "estimate" if want.abs() > 1.0 - 1e-9 => "estimate.near1",
        "estimate" => "estimate",
        "statistic" if key.starts_with("kendall.") => "z",
        "statistic" => "t",
        "p" => "p",
        _ => "ci",
    }
}

fn gate(fam: &str) -> f64 {
    GATES
        .iter()
        .find(|g| g.0 == fam)
        .unwrap_or_else(|| panic!("no gate {fam}"))
        .1
}

/// The error of `got` against `want` in its family: absolute near +-1,
/// a p as `p_error`, otherwise relative (absolute where want is 0).
fn error(fam: &str, got: f64, want: f64) -> Result<Option<f64>, String> {
    match fam {
        "estimate.near1" => Ok(Some((got - want).abs())),
        "p" => p_error(got, want),
        _ if want == 0.0 => Ok(Some(got.abs())),
        _ => Ok(Some((got - want).abs() / want.abs())),
    }
}

#[test]
fn every_case_matches_r_within_its_family_gate() {
    let all = expected();
    let cases = cases();
    assert_eq!(cases.len(), all.len(), "one expected block per case");
    let mut worst: BTreeMap<&'static str, (f64, String)> = BTreeMap::new();
    let mut failures = Vec::new();
    let mut compared = 0usize;
    for case in &cases {
        let want = &all[&case.name];
        let d = data(case);
        let p = params(case, &d);
        let got = compute(&d, &p);
        if let Some(code) = want.get("refused") {
            match got {
                Err(e) => assert_eq!(e.code.wire(), format!("stats.corr.{code}"), "{}", case.name),
                Ok(_) => {
                    failures.push(format!("{}: R refuses ({code}), Gaply computed", case.name))
                }
            }
            compared += 1;
            continue;
        }
        let r = got.unwrap_or_else(|e| panic!("{}: refused {:?}", case.name, e.code));
        let g = keys(&r, &p.variables);
        for k in g.keys() {
            if !want.contains_key(k) {
                failures.push(format!("{} {k}: Gaply has it, R does not", case.name));
            }
        }
        for (key, value) in want {
            compared += 1;
            let Some(got) = g.get(key) else {
                failures.push(format!("{} {key}: R has {value}, Gaply nothing", case.name));
                continue;
            };
            let at = format!("{} {key}", case.name);
            match (value.strip_prefix("NC:"), got) {
                (Some(code), Got::Nc(c)) if c.wire() == format!("stats.corr.{code}") => {}
                (Some(code), _) => failures.push(format!("{at}: want NC {code}")),
                (None, Got::Nc(c)) => {
                    failures.push(format!("{at}: want {value}, got NC {}", c.wire()))
                }
                (None, Got::Count(n)) => {
                    if value.parse::<u64>().ok() != Some(*n) {
                        failures.push(format!("{at}: want {value}, got {n}"));
                    }
                }
                (None, Got::Infinite(sign)) => {
                    let ok = (value == "Inf" && *sign > 0.0) || (value == "-Inf" && *sign < 0.0);
                    if !ok {
                        failures.push(format!(
                            "{at}: want {value}, got an infinite t of sign {sign}"
                        ));
                    }
                }
                (None, Got::Number(v)) => {
                    if key.ends_with(".df") || key.ends_with(".n") {
                        if value.parse::<f64>().ok() != Some(*v) {
                            failures.push(format!("{at}: want {value}, got {v}"));
                        }
                        continue;
                    }
                    let w: f64 = match value.parse() {
                        Ok(w) if value != "Inf" && value != "-Inf" => w,
                        _ => {
                            failures.push(format!("{at}: want {value}, got {v}"));
                            continue;
                        }
                    };
                    let fam = family(key, w);
                    match error(fam, *v, w) {
                        Err(e) => failures.push(format!("{at}: {e}")),
                        Ok(None) => {}
                        Ok(Some(e)) => {
                            let entry = worst.entry(fam).or_insert((0.0, String::new()));
                            if e > entry.0 {
                                *entry = (e, at.clone());
                            }
                            if e > gate(fam) {
                                failures.push(format!("{at}: {fam} error {e:.3e} over the gate {:e} (got {v:e}, want {w:e})", gate(fam)));
                            }
                        }
                    }
                }
            }
        }
    }
    for (fam, (e, at)) in &worst {
        println!("worst {fam}: {e:.3e} at {at}");
    }
    println!(
        "{compared} expected values compared over {} cases",
        cases.len()
    );
    assert!(compared > 2000, "{compared}");
    assert!(
        failures.is_empty(),
        "{} failures:\n{}",
        failures.len(),
        failures.join("\n")
    );
}
