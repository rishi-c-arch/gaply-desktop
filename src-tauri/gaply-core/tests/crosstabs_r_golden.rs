//! **`gaply_core::stats_xtab` against R 4.6.1 (Research OS, Phase 8X-1,
//! decisions 575 to 577), over `tests/fixtures/stats_xtab/` (written by
//! `/usr/local/bin/Rscript make_xtab_fixtures.R`).**
//!
//! Every number of every case is compared with R's, and each family has its
//! gate, set at ten times the largest error measured (decision 316), printed
//! with the measurement:
//!
//! ```text
//! cargo test -p gaply_core --test crosstabs_r_golden -- --nocapture
//! ```
//!
//! R's oracles: `table`, `chisq.test` (its expected counts, residuals and
//! adjusted standardized residuals, and the continuity-corrected statistic),
//! `fisher.test`, `pchisq`; Pearson's chi-square, the likelihood ratio and
//! the three measures from their formulas on whole-number O − E (the same
//! formulas written twice for G², phi, V and C; the generator's header says
//! so). Counts, df, levels, labels and exclusions are compared exactly. Every
//! p value is compared by a pure relative error, as
//! `tests/stats_dist_reference.rs` does; every other number relative below a
//! floor of 1e-12, as `tests/anova_r_golden.rs` does.
//!
//! `near_independent` (n11 n22 − n12 n21 = 1 at n = 100,000) is its own
//! family: its X² and G² are about 1.6e-14, where Gaply's sums over cells
//! (`stats_verify::chi_square_independence`'s, and G²'s) are rounding noise
//! and G² is clamped at 0, while R's are taken with O − E in whole numbers.
//! Its residuals are in that family too: O − E is 1 / n, about 1e-5, which a
//! double holds to about 1e-8 of itself after subtracting E near 25,000. Its
//! counts, df, expected counts and proportions are gated as every case's.
//! It is the case where G² is clamped: summed as `stats_xtab` sums it, G² is
//! −5.5e-12 before the clamp, so Gaply's G² is 0 and its p 1: those two are
//! checked exactly, against R's whole-number G² of 1.6e-14, not gated.
//!
//! The refusal cases name a code and its parameters, and the first error of
//! `compute` must be that code with those parameters.

use std::collections::BTreeMap;

use gaply_core::contracts::codes::Closed;
use gaply_core::contracts::{OperationId, ParamKey, ParamValue};
use gaply_core::stats_desc::Alias;
use gaply_core::stats_l1::dataset::{
    build_dataset, ColumnAlias, DataFingerprint, Dataset, RawTable, TableRef,
};
use gaply_core::stats_xtab::{
    compute, spec, validate, ChiOutcome, CrosstabsOutcome, CrosstabsParams, Stat,
};

const DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/stats_xtab");
const PROVENANCE: &str = "/usr/local/bin/Rscript make_xtab_fixtures.R";
const R_VERSION: &str = "R version 4.6.1";

/// The gates, each ten times the largest error measured over every case
/// (the measurement beside it, 9 Nov 2026, macOS aarch64).
const GATES: &[(&str, f64)] = &[
    ("cell", 4e-14),     // 3.68e-15, codepoint_text cell.2.2.residual
    ("pearson", 9e-15),  // 8.78e-16, t50x50 pearson.statistic
    ("lr", 6e-12),       // 5.68e-13, t2x2_large likelihood_ratio.statistic
    ("yates", 1e-15),    // 0 measured: every case bit for bit; a floor of 1e-15
    ("p", 8e-11),        // 7.91e-12, t2x2_large fisher.two_sided, pure relative
    ("measures", 5e-15), // 4.05e-16, t50x50 phi
    ("near", 1.4e-7),    // 1.36e-8, near_independent cell.0.0.residual (O − E of 1e-5 after subtracting E near 25,000)
];

/// The keys compared exactly: whole numbers and labels.
fn exact(key: &str) -> bool {
    key.ends_with(".observed")
        || key.ends_with(".df")
        || key.starts_with("row_total.")
        || key.starts_with("column_total.")
        || key.starts_with("excluded.")
        || key.starts_with("row_level.")
        || key.starts_with("column_level.")
        || key.starts_with("note.")
        || matches!(
            key,
            "n" | "rows" | "columns" | "expected_below_five" | "cell_count"
        )
}

fn family(case: &str, key: &str) -> &'static str {
    let tail = key.rsplit('.').next().unwrap_or("");
    let is_p = tail == "p" || key.starts_with("fisher.");
    let near_statistic = key.starts_with("pearson.")
        || key.starts_with("likelihood_ratio.")
        || key.starts_with("continuity_corrected.")
        || key.starts_with("fisher.")
        || matches!(key, "phi" | "cramers_v" | "contingency_coefficient");
    if case == "near_independent" && (near_statistic || key.contains("residual")) {
        "near"
    } else if is_p {
        "p"
    } else if key.starts_with("cell.") || key == "smallest_expected" {
        "cell"
    } else if key.starts_with("pearson.") {
        "pearson"
    } else if key.starts_with("likelihood_ratio.") {
        "lr"
    } else if key.starts_with("continuity_corrected.") {
        "yates"
    } else {
        "measures"
    }
}

/// A p value's error is purely relative; every other number's is relative
/// below a floor of 1e-12, where R's value is rounding noise about 0.
fn error(family: &str, key: &str, got: f64, want: f64) -> f64 {
    let is_p = key.ends_with(".p") || key.starts_with("fisher.");
    if got == want {
        0.0
    } else if is_p || family == "p" {
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
    rows: String,
    columns: String,
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
            rows: f[2].clone(),
            columns: f[3].clone(),
            missing_codes: if f[4] == "-" {
                Vec::new()
            } else {
                f[4].split(' ').map(|v| v.parse().unwrap()).collect()
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

/// A case's dataset: its data file, or its counts expanded row by row, each
/// pair in the file's order repeated n times, as the generator expands it.
fn data(case: &Case) -> Dataset {
    let bytes = std::fs::read(format!("{DIR}/{}", case.file)).expect("data file");
    let text = String::from_utf8(bytes.clone()).expect("utf-8");
    let mut lines = text.lines().filter(|l| !l.starts_with('#'));
    let header = lines.next().expect("header");
    let rows: Vec<Vec<String>> = if case.file.starts_with("counts_") {
        assert_eq!(header, "r,c,n", "{}", case.file);
        let mut rows = Vec::new();
        for l in lines {
            let f: Vec<&str> = l.split(',').collect();
            for _ in 0..f[2].parse::<u64>().expect("a count") {
                rows.push(vec![
                    (rows.len() + 1).to_string(),
                    f[0].to_string(),
                    f[1].to_string(),
                ]);
            }
        }
        rows
    } else {
        assert_eq!(header, "id,r,c", "{}", case.file);
        lines
            .map(|l| l.split(',').map(str::to_string).collect())
            .collect()
    };
    build_dataset(
        RawTable::from_rows(vec!["id".into(), "r".into(), "c".into()], rows),
        DataFingerprint::of_bytes(&bytes, "xtab golden", TableRef::FirstTable),
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

fn chi(m: &mut BTreeMap<String, Got>, name: &str, t: ChiOutcome) {
    match t {
        ChiOutcome::Computed(t) => {
            m.insert(format!("{name}.statistic"), Got::Number(t.statistic));
            m.insert(format!("{name}.df"), Got::Number(t.df));
            m.insert(format!("{name}.p"), Got::Number(t.p));
        }
        ChiOutcome::NotComputable(c) => {
            m.insert(format!("{name}.statistic"), Got::Nc(c.wire().to_string()));
        }
    }
}

enum Got {
    Number(f64),
    Text(String),
    Nc(String),
}

fn stat(s: Stat) -> Got {
    match s {
        Stat::Value(v) => Got::Number(v),
        Stat::NotComputable(c) => Got::Nc(format!("{c:?}")),
    }
}

fn keys(r: &CrosstabsOutcome) -> BTreeMap<String, Got> {
    let mut m = BTreeMap::new();
    m.insert("n".into(), Got::Number(r.n as f64));
    m.insert("rows".into(), Got::Number(r.row_levels.len() as f64));
    m.insert("columns".into(), Got::Number(r.column_levels.len() as f64));
    for l in &r.row_levels {
        m.insert(format!("row_level.{}", l.level), Got::Text(l.label.clone()));
    }
    for l in &r.column_levels {
        m.insert(format!("column_level.{}", l.level), Got::Text(l.label.clone()));
    }
    for (i, t) in r.row_totals.iter().enumerate() {
        m.insert(format!("row_total.{i}"), Got::Number(*t as f64));
    }
    for (j, t) in r.column_totals.iter().enumerate() {
        m.insert(format!("column_total.{j}"), Got::Number(*t as f64));
    }
    for f in &r.frequencies {
        for (k, v) in [
            ("observed", f.observed as f64),
            ("expected", f.expected),
            ("row_proportion", f.row_proportion),
            ("column_proportion", f.column_proportion),
            ("total_proportion", f.total_proportion),
            ("residual", f.residual),
            ("pearson_residual", f.pearson_residual),
            ("adjusted_residual", f.adjusted_residual),
        ] {
            m.insert(format!("cell.{}.{}.{k}", f.row, f.column), Got::Number(v));
        }
    }
    chi(&mut m, "pearson", r.pearson);
    chi(&mut m, "likelihood_ratio", r.likelihood_ratio);
    if let Some(t) = r.continuity_corrected {
        chi(&mut m, "continuity_corrected", t);
    }
    if let Some(f) = r.fisher {
        m.insert("fisher.two_sided".into(), Got::Number(f.two_sided));
        m.insert("fisher.less".into(), Got::Number(f.less));
        m.insert("fisher.greater".into(), Got::Number(f.greater));
    }
    m.insert("phi".into(), stat(r.phi));
    m.insert("cramers_v".into(), stat(r.cramers_v));
    m.insert(
        "contingency_coefficient".into(),
        stat(r.contingency_coefficient),
    );
    m.insert(
        "expected_below_five".into(),
        Got::Number(r.expected_below_five as f64),
    );
    m.insert("cell_count".into(), Got::Number(r.cell_count as f64));
    m.insert(
        "smallest_expected".into(),
        Got::Number(r.smallest_expected),
    );
    m.insert(
        "note.expected_below_five".into(),
        Got::Number(if r.notes.is_empty() { 0.0 } else { 1.0 }),
    );
    let mut excluded: BTreeMap<String, f64> = [
        "excluded.missing_row",
        "excluded.missing_code.rows",
        "excluded.missing_column",
        "excluded.missing_code.columns",
    ]
    .into_iter()
    .map(|k| (k.to_string(), 0.0))
    .collect();
    for e in &r.exclusions {
        match e.code.wire().to_string().as_str() {
            "stats.xtab.missing_row" => {
                excluded.insert("excluded.missing_row".into(), e.count as f64);
            }
            "stats.xtab.missing_column" => {
                excluded.insert("excluded.missing_column".into(), e.count as f64);
            }
            // Told apart by the column each names: `code_exclusions`.
            "stats.xtab.missing_code" => {}
            other => panic!("unknown exclusion {other}"),
        }
    }
    for (k, v) in excluded {
        m.insert(k, Got::Number(v));
    }
    m
}

/// The two missing_code exclusions, told apart by the column each names.
fn code_exclusions(r: &CrosstabsOutcome, rows: Alias, columns: Alias) -> [(String, f64); 2] {
    let mut out = [
        ("excluded.missing_code.rows".to_string(), 0.0),
        ("excluded.missing_code.columns".to_string(), 0.0),
    ];
    for e in &r.exclusions {
        if e.code.wire().to_string() != "stats.xtab.missing_code" {
            continue;
        }
        let at = e.params.get(&ParamKey::Column).cloned();
        if at == Some(ParamValue::Int(i64::from(rows.0 .0))) {
            out[0].1 = e.count as f64;
        } else if at == Some(ParamValue::Int(i64::from(columns.0 .0))) {
            out[1].1 = e.count as f64;
        } else {
            panic!("a missing_code exclusion names neither column");
        }
    }
    out
}

#[derive(Default)]
struct Worst(f64, String);

fn param(e: &gaply_core::contracts::Diagnostic<gaply_core::stats_xtab::XtabCode>, key: ParamKey) -> Option<i64> {
    match e.params.get(&key) {
        Some(ParamValue::Int(v)) => Some(*v),
        _ => None,
    }
}

#[test]
fn every_number_of_every_case_is_within_its_gate_of_r() {
    let want_all = expected();
    let cases = cases();
    assert_eq!(cases.len(), 23, "the generator's cases");
    let mut worst: BTreeMap<&str, Worst> = BTreeMap::new();
    let mut over = Vec::new();
    let mut compared = 0usize;
    let mut clamped: Vec<(String, f64, f64)> = Vec::new();
    let (mut refusals, mut results) = (0, 0);
    for case in &cases {
        let want = want_all
            .get(&case.name)
            .unwrap_or_else(|| panic!("{}: no expected values", case.name));
        let d = data(case);
        let (rows, columns) = (alias(&d, &case.rows), alias(&d, &case.columns));
        let params = CrosstabsParams {
            rows,
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
            if let Some(which) = want.get("refused.column") {
                let a = if which == "rows" { rows } else { columns };
                assert_eq!(param(&e, ParamKey::Column), Some(i64::from(a.0 .0)), "{} column", case.name);
            }
            continue;
        }
        results += 1;
        let r = compute(&d, &params).unwrap_or_else(|e| panic!("{}: refused {e:?}", case.name));
        let mut got = keys(&r);
        for (k, v) in code_exclusions(&r, rows, columns) {
            got.insert(k, Got::Number(v));
        }
        for (key, w) in want {
            let g = got
                .get(key)
                .unwrap_or_else(|| panic!("{}: the result has no {key}", case.name));
            match g {
                Got::Text(t) => assert_eq!(t, w, "{} {key}", case.name),
                Got::Nc(c) => panic!("{} {key}: not computable ({c}) where R gives {w}", case.name),
                Got::Number(v) => {
                    let wv: f64 = w.parse().unwrap_or_else(|_| panic!("{} {key}: {w}", case.name));
                    compared += 1;
                    if exact(key) {
                        assert_eq!(*v, wv, "{} {key}", case.name);
                        continue;
                    }
                    if case.name == "near_independent"
                        && matches!(key.as_str(), "likelihood_ratio.statistic" | "likelihood_ratio.p")
                    {
                        // The clamp: Gaply's G² is 0 and its p 1, exactly, where R's
                        // whole-number G² is a positive 1.6e-14 (`clamped` below).
                        clamped.push((key.clone(), *v, wv));
                        continue;
                    }
                    let fam = family(&case.name, key);
                    let err = error(fam, key, *v, wv);
                    let gate = GATES.iter().find(|(f, _)| *f == fam).unwrap().1;
                    if err > gate {
                        over.push(format!("{} {key}: gaply {v:e} R {wv:e} error {err:e} gate {gate:e}", case.name));
                    }
                    if fam == "near" {
                        println!("near {key}: gaply {v:e} R {wv:e} error {err:.3e}");
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
        println!("{fam:<9} largest error {:.3e} ({})  gate {gate:e}", w.0, w.1);
    }
    println!("{results} results, {refusals} refusals, {compared} numbers compared");
    println!("near_independent's clamped likelihood ratio: {clamped:?}");
    let mut clamped_keys: Vec<&str> = clamped.iter().map(|c| c.0.as_str()).collect();
    clamped_keys.sort_unstable();
    assert_eq!(clamped_keys, ["likelihood_ratio.p", "likelihood_ratio.statistic"]);
    for (key, got, want) in &clamped {
        if key == "likelihood_ratio.statistic" {
            assert_eq!(*got, 0.0, "G² clamped at 0");
            assert!(*want > 0.0 && *want < 1e-13, "R's G² {want:e}");
        } else {
            assert_eq!(*got, 1.0, "the p of a G² of 0");
            assert!(*want > 1.0 - 1e-6 && *want < 1.0, "R's p {want:e}");
        }
    }
    assert_eq!((results, refusals), (14, 9));
    assert!(over.is_empty(), "over the gate:\n{}", over.join("\n"));
}
