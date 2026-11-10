//! **`gaply_core::stats_related_ranks` against R 4.6.1 (Research OS, Phase 8Z-1,
//! decisions 636 and 638), over `tests/fixtures/stats_related_ranks/` (written by
//! `/usr/local/bin/Rscript make_related_ranks_fixtures.R`).**
//!
//! Every number of every case is compared with R's, and each family has its gate,
//! set at ten times the largest error measured (decision 316), printed with the
//! measurement:
//!
//! ```text
//! cargo test -p gaply_core --test related_ranks_r_golden -- --nocapture
//! ```
//!
//! R's oracles: `wilcox.test(paired = TRUE, exact = FALSE, correct = FALSE)` (V and
//! the asymptotic p), `wilcox.test(paired = TRUE, exact = TRUE)` (the exact p, asked
//! for only with no tied difference and no zero), `friedman.test` (Q, df, p),
//! `median`, `rank(ties.method = "average")`. z, T minus, the rank-biserial
//! correlation, each side's counts and sums, the Friedman deviation form and
//! Kendall's W have no base function: the same formulas are written twice, in the
//! generator and in `stats_related_ranks`. Counts, df, exclusions and notes are
//! compared exactly. Every p value is compared by a pure relative error; every
//! other number relative below a floor of 1e-12.

use std::collections::BTreeMap;

use gaply_core::contracts::codes::Closed;
use gaply_core::contracts::{OperationId, ParamKey, ParamValue};
use gaply_core::stats_desc::Alias;
use gaply_core::stats_l1::dataset::{
    build_dataset, ColumnAlias, DataFingerprint, Dataset, RawTable, TableRef,
};
use gaply_core::stats_related_ranks::{
    compute, spec, validate, RelatedRanksCode, RelatedRanksOutcome, RelatedRanksParams, RelatedTest,
    SignRanks, Stat,
};

const DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/stats_related_ranks");
const PROVENANCE: &str = "/usr/local/bin/Rscript make_related_ranks_fixtures.R";
const R_VERSION: &str = "R version 4.6.1";

/// The gates, each ten times the largest error measured over every case (the
/// measurement beside it, 10 Nov 2026, macOS aarch64). A family measured at 0, bit
/// for bit in every case, has a floor of 1e-15.
const GATES: &[(&str, f64)] = &[
    ("median", 1e-15), // 0 measured: every median bit for bit
    ("ranks", 1e-15),  // 0 measured: rank sums, means, V and T minus bit for bit
    ("z", 1e-15),      // 0 measured: the formula written twice, bit for bit
    ("p", 1.1e-9),     // 1.030e-10, w_tied_abs wilcoxon.p, pure relative (statrs's erfc against R's pnorm)
    ("q", 1e-15),      // 0 measured: Q's form written twice, bit for bit
    ("q_friedman", 1.3e-15), // 1.903e-16, f_ties (1.219e-16 before C came from an exact integer; the gate was kept): Gaply's Q against friedman.test's own form
    ("effect", 1e-15), // 0 measured: the rank-biserial correlation and Kendall's W bit for bit
];

/// The keys compared exactly: whole numbers.
fn exact(key: &str) -> bool {
    key.starts_with("excluded.")
        || key.starts_with("note.")
        || key.ends_with(".n")
        || key.ends_with(".count")
        || key.ends_with(".df")
        || key.ends_with(".zero_differences")
        || key.ends_with(".tie_runs")
        || matches!(key, "rows_used" | "columns")
}

fn family(key: &str) -> &'static str {
    let tail = key.rsplit('.').next().unwrap_or("");
    match tail {
        "p" | "exact_p" => "p",
        "median" | "median_difference" => "median",
        "rank_sum" | "mean_rank" | "v" | "t_minus" => "ranks",
        "z" => "z",
        "q" => "q",
        "q_friedman_test" => "q_friedman",
        _ => "effect",
    }
}

/// A p value's error is purely relative; every other number's is relative below a
/// floor of 1e-12, where R's value is rounding noise about 0.
fn error(family: &str, got: f64, want: f64) -> f64 {
    if got == want {
        0.0
    } else if family == "p" {
        if want == 0.0 {
            got.abs()
        } else {
            (got - want).abs() / want.abs()
        }
    } else {
        (got - want).abs() / want.abs().max(1e-12)
    }
}

struct Case {
    name: String,
    file: String,
    columns: Vec<String>,
    missing_codes: Vec<f64>,
}

fn table(name: &str) -> Vec<Vec<String>> {
    let text =
        std::fs::read_to_string(format!("{DIR}/{name}")).unwrap_or_else(|e| panic!("{name}: {e}"));
    let stamp: Vec<&str> = text.lines().take_while(|l| l.starts_with('#')).collect();
    assert!(
        stamp.iter().any(|l| l.contains(PROVENANCE)),
        "{name}: not written by {PROVENANCE}"
    );
    assert!(
        stamp.iter().any(|l| l.contains(R_VERSION)),
        "{name}: not written with {R_VERSION}"
    );
    text.lines()
        .filter(|l| !l.starts_with('#'))
        .skip(1)
        .map(|l| l.split('\t').map(str::to_string).collect())
        .collect()
}

fn cases() -> Vec<Case> {
    table("cases.tsv")
        .into_iter()
        .map(|f| Case {
            name: f[0].clone(),
            file: f[1].clone(),
            columns: f[2].split(' ').map(str::to_string).collect(),
            missing_codes: if f[3] == "-" {
                Vec::new()
            } else {
                f[3].split(' ').map(|v| v.parse().unwrap()).collect()
            },
        })
        .collect()
}

fn expected() -> BTreeMap<String, BTreeMap<String, String>> {
    let mut out: BTreeMap<String, BTreeMap<String, String>> = BTreeMap::new();
    for f in table("expected.tsv") {
        out.entry(f[0].clone())
            .or_default()
            .insert(f[1].clone(), f[2].clone());
    }
    out
}

/// A case's dataset: its data file, or its counts expanded row by row, each line
/// in the file's order repeated n times, as the generator expands it.
fn data(case: &Case) -> Dataset {
    let bytes = std::fs::read(format!("{DIR}/{}", case.file)).expect("data file");
    let text = String::from_utf8(bytes.clone()).expect("utf-8");
    let mut lines = text.lines().filter(|l| !l.starts_with('#'));
    let header: Vec<String> = lines.next().expect("header").split(',').map(str::to_string).collect();
    let (headers, rows): (Vec<String>, Vec<Vec<String>>) = if case.file.starts_with("counts_") {
        let width = header.len() - 1;
        let mut rows = Vec::new();
        for l in lines {
            let f: Vec<&str> = l.split(',').collect();
            for _ in 0..f[width].parse::<u64>().expect("a count") {
                rows.push(f[..width].iter().map(|s| s.to_string()).collect());
            }
        }
        (header[..width].to_vec(), rows)
    } else {
        // Empty trailing cells are kept: split gives them.
        (header, lines.map(|l| l.split(',').map(str::to_string).collect()).collect())
    };
    build_dataset(
        RawTable::from_rows(headers, rows),
        DataFingerprint::of_bytes(&bytes, "related ranks golden", TableRef::FirstTable),
    )
    .expect("dataset")
}

fn alias(d: &Dataset, header: &str) -> Alias {
    match d.schema.columns.iter().find(|c| c.header == header) {
        Some(c) => Alias(c.alias),
        // A column the dataset does not have: the refusal case's.
        None => Alias(ColumnAlias(99)),
    }
}

enum Got {
    Number(f64),
    Nc(String),
    Absent,
}

fn stat(s: Stat) -> Got {
    match s {
        Stat::Value(v) => Got::Number(v),
        Stat::NotComputable(c) => Got::Nc(c.wire().to_string()),
    }
}

fn sign(m: &mut BTreeMap<String, Got>, name: &str, s: &SignRanks) {
    m.insert(format!("wilcoxon.{name}.count"), Got::Number(s.count as f64));
    m.insert(format!("wilcoxon.{name}.rank_sum"), Got::Number(s.rank_sum));
    m.insert(
        format!("wilcoxon.{name}.mean_rank"),
        s.mean_rank.map_or(Got::Absent, Got::Number),
    );
}

fn keys(r: &RelatedRanksOutcome) -> BTreeMap<String, Got> {
    let mut m = BTreeMap::new();
    m.insert("rows_used".into(), Got::Number(r.rows_used as f64));
    m.insert("columns".into(), Got::Number(r.columns.len() as f64));
    for (i, c) in r.columns.iter().enumerate() {
        m.insert(format!("column.{i}.median"), Got::Number(c.median));
    }
    match &r.test {
        RelatedTest::Wilcoxon(t) => {
            m.insert("wilcoxon.n".into(), Got::Number(t.n as f64));
            m.insert("wilcoxon.zero_differences".into(), Got::Number(t.zero_differences as f64));
            m.insert("wilcoxon.tie_runs".into(), Got::Number(t.tie_runs as f64));
            sign(&mut m, "negative", &t.negative);
            sign(&mut m, "positive", &t.positive);
            m.insert("wilcoxon.v".into(), Got::Number(t.v));
            m.insert("wilcoxon.t_minus".into(), Got::Number(t.t_minus));
            m.insert("wilcoxon.z".into(), stat(t.z));
            m.insert("wilcoxon.p".into(), stat(t.p));
            m.insert("wilcoxon.exact_p".into(), t.exact_p.map_or(Got::Absent, Got::Number));
            m.insert("wilcoxon.rank_biserial".into(), stat(t.rank_biserial));
            m.insert("wilcoxon.median_difference".into(), stat(t.median_difference));
        }
        RelatedTest::Friedman(t) => {
            for (i, c) in t.columns.iter().enumerate() {
                m.insert(format!("friedman.column.{i}.rank_sum"), Got::Number(c.rank_sum));
                m.insert(format!("friedman.column.{i}.mean_rank"), Got::Number(c.mean_rank));
            }
            m.insert("friedman.q".into(), stat(t.q));
            if let Stat::Value(q) = t.q {
                m.insert("friedman.q_friedman_test".into(), Got::Number(q));
            }
            m.insert("friedman.df".into(), Got::Number(t.df));
            m.insert("friedman.p".into(), stat(t.p));
            m.insert("friedman.kendalls_w".into(), stat(t.kendalls_w));
        }
    }
    for (code, key) in [
        (RelatedRanksCode::ExactNotGivenTies, "note.exact_not_given_ties"),
        (RelatedRanksCode::ExactNotGivenZeros, "note.exact_not_given_zeros"),
        (RelatedRanksCode::ExactNotGivenSize, "note.exact_not_given_size"),
        (RelatedRanksCode::ZeroDifferencesDropped, "note.zero_differences_dropped"),
        (RelatedRanksCode::RowsBelowFive, "note.rows_below_five"),
    ] {
        m.insert(key.into(), Got::Number(if r.notes.contains(&code) { 1.0 } else { 0.0 }));
    }
    for (j, c) in r.columns.iter().enumerate() {
        for reason in ["missing_value", "missing_code"] {
            let n = r
                .exclusions
                .iter()
                .filter(|e| e.code.wire() == format!("stats.related_ranks.{reason}"))
                .filter(|e| e.params.get(&ParamKey::Column) == Some(&ParamValue::Int(i64::from(c.column.0 .0))))
                .map(|e| e.count)
                .sum::<u64>();
            m.insert(format!("excluded.{j}.{reason}"), Got::Number(n as f64));
        }
    }
    m
}

#[derive(Default)]
struct Worst(f64, String);

fn param(e: &gaply_core::contracts::Diagnostic<RelatedRanksCode>, key: ParamKey) -> Option<i64> {
    match e.params.get(&key) {
        Some(ParamValue::Int(v)) => Some(*v),
        _ => None,
    }
}

#[test]
fn every_number_of_every_case_is_within_its_gate_of_r() {
    let want_all = expected();
    let cases = cases();
    let mut worst: BTreeMap<&str, Worst> = BTreeMap::new();
    let mut over = Vec::new();
    let mut compared = 0usize;
    let (mut refusals, mut results, mut not_computable, mut absent) = (0, 0, 0, 0);
    for case in &cases {
        let want = want_all
            .get(&case.name)
            .unwrap_or_else(|| panic!("{}: no expected values", case.name));
        let d = data(case);
        let columns: Vec<Alias> = case.columns.iter().map(|h| alias(&d, h)).collect();
        let params = RelatedRanksParams {
            columns,
            missing_codes: case.missing_codes.clone(),
        };
        if let Some(code) = want.get("refused") {
            refusals += 1;
            let e = compute(&d, &params)
                .err()
                .unwrap_or_else(|| panic!("{}: R refuses with {code}, gaply computed", case.name));
            assert_eq!(&e.code.wire().to_string(), code, "{}: the first refusal", case.name);
            let report = validate(&d, &spec(OperationId(1), &d, params.clone()));
            assert_eq!(
                report.errors.first().map(|x| x.code.wire().to_string()),
                Some(code.clone()),
                "{}: validate's first error is run's",
                case.name
            );
            for (key, pk) in [
                ("refused.index", ParamKey::Index),
                ("refused.count", ParamKey::Count),
                ("refused.limit", ParamKey::Limit),
            ] {
                if let Some(v) = want.get(key) {
                    assert_eq!(param(&e, pk), Some(v.parse::<i64>().unwrap()), "{} {key}", case.name);
                }
            }
            if let Some(header) = want.get("refused.column") {
                assert_eq!(
                    param(&e, ParamKey::Column),
                    Some(i64::from(alias(&d, header).0 .0)),
                    "{} column",
                    case.name
                );
            }
            continue;
        }
        results += 1;
        let r = compute(&d, &params).unwrap_or_else(|e| panic!("{}: refused {e:?}", case.name));
        let got = keys(&r);
        for (key, w) in want {
            let g = got
                .get(key)
                .unwrap_or_else(|| panic!("{}: the result has no {key}", case.name));
            match g {
                Got::Nc(c) => {
                    not_computable += 1;
                    assert_eq!(&format!("nc {c}"), w, "{} {key}: not computable", case.name);
                }
                Got::Absent => {
                    absent += 1;
                    assert_eq!(w, "absent", "{} {key}: absent where R gives {w}", case.name);
                }
                Got::Number(v) => {
                    let wv: f64 = w
                        .parse()
                        .unwrap_or_else(|_| panic!("{} {key}: gaply {v} where R gives {w}", case.name));
                    compared += 1;
                    if exact(key) {
                        assert_eq!(*v, wv, "{} {key}", case.name);
                        continue;
                    }
                    let fam = family(key);
                    let err = error(fam, *v, wv);
                    let gate = GATES.iter().find(|(f, _)| *f == fam).unwrap().1;
                    if err > gate {
                        over.push(format!("{} {key}: gaply {v:e} R {wv:e} error {err:e} gate {gate:e}", case.name));
                    }
                    let w = worst.entry(fam).or_default();
                    if err >= w.0 {
                        *w = Worst(err, format!("{} {key}", case.name));
                    }
                }
            }
        }
        // And the result carries no number R does not have.
        for key in got.keys() {
            assert!(want.contains_key(key), "{}: {key} has no R value", case.name);
        }
    }
    for (fam, gate) in GATES {
        let w = worst.get(fam).map(|w| (w.0, w.1.as_str())).unwrap_or((0.0, "none"));
        println!("{fam:<11} largest error {:.3e} ({})  gate {gate:e}", w.0, w.1);
    }
    println!("{results} results, {refusals} refusals, {compared} numbers compared, {not_computable} not computable, {absent} absent");
    assert_eq!((results, refusals), (20, 8));
    assert_eq!(compared, 782, "every number the generator wrote");
    assert_eq!((not_computable, absent), (6, 12));
    assert!(over.is_empty(), "over the gate:\n{}", over.join("\n"));
}
