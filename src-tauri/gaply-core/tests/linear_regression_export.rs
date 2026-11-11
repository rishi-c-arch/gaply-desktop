//! **The linear regression's exports (Research OS, Phase 9A-1, decision 671).**
//!
//! * The CSV reads back field by field: each model column's mean and standard
//!   deviation, the rows used, the model summary, the analysis of variance, the
//!   intercept, each predictor, the residuals' summary, the rows left out (a count
//!   line and a column line each), the notes, the limitations and the manifest's
//!   hashes. Every number reads back to the same double.
//! * A column is its index in the model's list (the outcome 0, the predictors 1 to
//!   k) and its alias; the intercept's lines leave both column fields empty. The
//!   headers never reach either file (a sentinel, with its positive control).
//! * A value that is not computable is an empty field with its code.
//! * The bytes are pinned on macOS aarch64 (captured 11 Nov 2026): a golden CSV and
//!   the manifest's JSON for three predictors with rows left out by each reason.
//!   Every other platform, Linux x86_64 included, checks that pin's layout
//!   (decision 346's check, `pin_support`, unchanged) until a Linux run measures
//!   it: the p values come from the platform's maths library through statrs, so
//!   another platform's last digits may differ. It never passes by skipping.
//! * Ten runs give byte-identical CSV and manifest bytes.

use gaply_core::contracts::{Arch, OperationId, Os, Platform, RunManifest};
use gaply_core::stats_desc::Alias;
use gaply_core::stats_l1::dataset::{
    build_dataset, ColumnAlias, DataFingerprint, Dataset, RawTable, TableRef,
};
use gaply_core::stats_linear_regression::export::{manifest_json, result_csv};
use gaply_core::stats_linear_regression::{
    compute, manifest, spec, LinearRegressionCode, LinearRegressionOutcome, LinearRegressionParams,
    Stat,
};

mod pin_support;
use pin_support::{csv_layout_matches, json_layout_matches};

const HEADER_SENTINEL: &str = "HDR_SENTINEL_r91";

fn dataset(headers: &[&str], rows: &[Vec<&str>]) -> Dataset {
    build_dataset(
        RawTable::from_rows(
            headers.iter().map(|h| h.to_string()).collect(),
            rows.iter().map(|r| r.iter().map(|c| c.to_string()).collect()).collect(),
        ),
        DataFingerprint::of_bytes(format!("{headers:?}{rows:?}").as_bytes(), "linear regression export", TableRef::FirstTable),
    )
    .expect("dataset")
}

fn platform() -> Platform {
    Platform {
        os: Os::Macos,
        arch: Arch::Aarch64,
    }
}

fn params(outcome: u32, predictors: &[u32], codes: Vec<f64>) -> LinearRegressionParams {
    LinearRegressionParams {
        outcome: Alias(ColumnAlias(outcome)),
        predictors: predictors.iter().map(|n| Alias(ColumnAlias(*n))).collect(),
        missing_codes: codes,
    }
}

/// Fourteen rows, an outcome and three predictors: eleven complete, one row left
/// out by each reason.
fn golden_rows() -> Vec<Vec<&'static str>> {
    vec![
        vec!["7.1", "1.2", "0.5", "10"],
        vec!["8.4", "2.5", "-1.25", "12"],
        vec!["9.9", "3.1", "2", "9.5"],
        vec!["12.2", "4.8", "0.75", "14"],
        vec!["11.6", "5", "-0.5", "11"],
        vec!["13.9", "6.3", "1.5", "13.5"],
        vec!["16.3", "7.7", "-2", "15"],
        vec!["14.1", "8.1", "0.25", "10.5"],
        vec!["17.8", "9.4", "1", "16"],
        vec!["19.5", "10.2", "-0.75", "12.5"],
        vec!["18.4", "11.9", "2.5", "17.5"],
        vec!["", "3", "1", "2"],
        vec!["5", "-9", "1", "2"],
        vec!["6", "4", "1", ""],
    ]
}

fn run_of(d: &Dataset, p: LinearRegressionParams) -> (LinearRegressionOutcome, RunManifest<LinearRegressionCode>) {
    let s = spec(OperationId(17), d, p);
    let r = compute(d, &s.parameters).expect("computes");
    let m = manifest(&s, d, &r, 0, platform());
    (r, m)
}

fn golden_case() -> (Vec<u8>, Vec<u8>) {
    let d = dataset(&[HEADER_SENTINEL, "x1", "x2", "x3"], &golden_rows());
    let (r, m) = run_of(&d, params(1, &[2, 3, 4], vec![-9.0]));
    (result_csv(&r, &m), manifest_json(&m))
}

fn read_csv(bytes: &[u8]) -> Vec<Vec<String>> {
    let text = String::from_utf8(bytes.to_vec()).expect("utf-8");
    assert!(text.ends_with("\r\n"));
    text.trim_end_matches("\r\n")
        .split("\r\n")
        .map(|l| l.split(',').map(|f| f.trim_matches('"').to_string()).collect())
        .collect()
}

fn line<'a>(csv: &'a [Vec<String>], table: &str, statistic: &str) -> &'a Vec<String> {
    csv.iter()
        .find(|r| r[0] == table && r[1] == statistic)
        .unwrap_or_else(|| panic!("{table} {statistic}"))
}

fn num(s: Stat) -> f64 {
    match s {
        Stat::Value(v) => v,
        Stat::NotComputable(_) => panic!("not computable"),
    }
}

#[test]
fn the_csv_reads_back_and_every_number_to_its_double() {
    let d = dataset(&["y", "x1", "x2", "x3"], &golden_rows());
    let (r, m) = run_of(&d, params(1, &[2, 3, 4], vec![-9.0]));
    let csv = read_csv(&result_csv(&r, &m));
    assert_eq!(csv[0], ["table", "statistic", "column_index", "column", "column_index_2", "column_2", "value", "word"]);
    for row in &csv {
        assert_eq!(row.len(), 8, "{row:?}");
    }
    let means: Vec<(&str, &str)> = csv.iter().filter(|r| r[0] == "columns" && r[1] == "mean").map(|r| (r[2].as_str(), r[3].as_str())).collect();
    assert_eq!(means, [("0", "C1"), ("1", "C2"), ("2", "C3"), ("3", "C4")]);
    assert_eq!(line(&csv, "columns", "mean")[6].parse::<f64>().unwrap(), num(r.columns[0].mean));
    assert_eq!(line(&csv, "rows", "used")[6], "11");
    assert_eq!(line(&csv, "model", "r_squared")[6].parse::<f64>().unwrap(), num(r.model.r_squared));
    assert_eq!(line(&csv, "anova", "f")[6].parse::<f64>().unwrap(), num(r.anova.f));
    assert_eq!(line(&csv, "anova", "residual_df")[6], "7");
    assert_eq!(line(&csv, "anova", "total_df")[6], "10");
    let b = line(&csv, "intercept", "b");
    assert_eq!(b[2..6], ["", "", "", ""], "the intercept is no column");
    assert_eq!(b[6].parse::<f64>().unwrap(), num(r.intercept.b));
    let predictors: Vec<&Vec<String>> = csv.iter().filter(|r| r[0] == "predictor" && r[1] == "b").collect();
    assert_eq!(predictors.iter().map(|r| (r[2].as_str(), r[3].as_str())).collect::<Vec<_>>(), [("1", "C2"), ("2", "C3"), ("3", "C4")]);
    for (row, p) in predictors.iter().zip(&r.predictors) {
        assert_eq!(row[6].parse::<f64>().unwrap(), num(p.b));
    }
    assert_eq!(csv.iter().filter(|r| r[0] == "predictor").count(), 3 * 12);
    assert_eq!(line(&csv, "residuals", "standardized_residual_maximum")[6].parse::<f64>().unwrap(), num(r.residuals.standardized_residual.maximum));
    let excluded: Vec<&Vec<String>> = csv.iter().filter(|r| r[0] == "excluded").collect();
    assert_eq!(excluded.len(), 6, "three rows, two lines each");
    assert_eq!(csv.iter().filter(|r| r[0] == "limitation").count(), 10);
    assert_eq!(line(&csv, "manifest", "output_sha256")[7], m.output_hash.as_hex());
}

#[test]
fn a_not_computable_value_is_an_empty_field_with_its_code() {
    let d = dataset(&["x", "y"], &[vec!["0", "2"], vec!["3", "11"], vec!["4", "14"]]);
    let (r, m) = run_of(&d, params(2, &[1], vec![]));
    let csv = read_csv(&result_csv(&r, &m));
    let f = line(&csv, "anova", "f");
    assert_eq!((f[6].as_str(), f[7].as_str()), ("", "stats.linear_regression.perfect_fit"));
    assert_eq!(line(&csv, "note", "")[7], "stats.linear_regression.essentially_perfect_fit");
}

#[test]
fn neither_file_holds_a_header() {
    let (csv, json) = golden_case();
    let d = dataset(&[HEADER_SENTINEL, "x1", "x2", "x3"], &golden_rows());
    assert!(d.schema.columns.iter().any(|c| c.header == HEADER_SENTINEL), "the positive control");
    for bytes in [&csv, &json] {
        assert!(!String::from_utf8_lossy(bytes).contains(HEADER_SENTINEL));
    }
}

#[test]
fn ten_runs_give_the_same_bytes() {
    let first = golden_case();
    for _ in 0..9 {
        assert_eq!(golden_case(), first);
    }
}

fn pin_path(name: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/stats_linear_regression")
        .join(name)
}

fn pin_bytes(name: &str) -> Vec<u8> {
    std::fs::read(pin_path(name)).unwrap_or_else(|e| panic!("{name}: {e}"))
}

/// The pin's difference from `got`, if any, so both files are reported.
fn pinned(name: &str, got: &[u8]) -> Option<String> {
    if std::env::var_os("GAPLY_WRITE_LINEAR_REGRESSION_EXPORT_PIN").is_some() {
        std::fs::write(pin_path(name), got).expect("write the pin");
    }
    let want = pin_bytes(name);
    (got != want.as_slice()).then(|| {
        format!(
            "{name}: the bytes differ from the pin.\n--- got ---\n{}\n--- pinned ---\n{}",
            String::from_utf8_lossy(got),
            String::from_utf8_lossy(&want)
        )
    })
}

const MAC_CSV: &str = "export_golden.csv";
const MAC_JSON: &str = "export_golden_manifest.json";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PinCheck {
    Exact,
    Layout,
}

/// The check for a platform, by `std::env::consts::OS` and `ARCH`: exact on macOS
/// aarch64, where the pin was captured, until the user's Linux run measures Linux
/// x86_64 (decision 671).
fn pin_check(os: &str, arch: &str) -> PinCheck {
    match (os, arch) {
        ("macos", "aarch64") => PinCheck::Exact,
        _ => PinCheck::Layout,
    }
}

#[test]
fn the_pin_is_checked_exactly_on_macos_aarch64_and_by_layout_elsewhere() {
    for (os, arch, want) in [
        ("macos", "aarch64", PinCheck::Exact),
        ("linux", "x86_64", PinCheck::Layout),
        ("windows", "x86_64", PinCheck::Layout),
        ("macos", "x86_64", PinCheck::Layout),
        ("linux", "aarch64", PinCheck::Layout),
    ] {
        assert_eq!(pin_check(os, arch), want, "{os}, {arch}");
    }
}

#[test]
fn the_export_bytes_are_pinned() {
    let (csv, json) = golden_case();
    if pin_check(std::env::consts::OS, std::env::consts::ARCH) == PinCheck::Exact {
        let differ: Vec<String> = [pinned(MAC_CSV, &csv), pinned(MAC_JSON, &json)]
            .into_iter()
            .flatten()
            .collect();
        assert!(differ.is_empty(), "{}", differ.join("\n"));
    } else {
        if let Err(e) = csv_layout_matches(&csv, &pin_bytes(MAC_CSV)) {
            panic!("{MAC_CSV}: the layout differs: {e}");
        }
        if let Err(e) = json_layout_matches(&json, &pin_bytes(MAC_JSON)) {
            panic!("{MAC_JSON}: the layout differs: {e}");
        }
    }
}

/// Every platform but macOS aarch64 relies on this: the layout check on altered
/// copies of the pin, run on every platform.
#[test]
fn the_layout_check_allows_a_few_ulps_and_nothing_else() {
    let pin = pin_bytes(MAC_CSV);
    let text = String::from_utf8(pin.clone()).expect("utf-8");
    let altered = |old: &str, new: &str| -> Vec<u8> {
        assert_eq!(text.matches(old).count(), 1, "{old} once in the pin");
        text.replacen(old, new, 1).into_bytes()
    };
    assert_eq!(csv_layout_matches(&pin, &pin), Ok(()));
    let line = text
        .split("\r\n")
        .find(|l| l.starts_with("\"anova\",\"p\","))
        .expect("the F's p")
        .to_string();
    let value = line.split(',').nth(6).unwrap().to_string();
    let moved = |n: u64| {
        let v: f64 = value.parse().expect("a number");
        line.replacen(&value, &format!("{}", f64::from_bits(v.to_bits() + n)), 1)
    };
    assert_eq!(csv_layout_matches(&altered(&line, &moved(1)), &pin), Ok(()));
    assert!(csv_layout_matches(&altered(&line, &moved(5)), &pin).is_err(), "5 ulps off");
}
