//! **The structural equation models' exports (Research OS, Phase 9C-4,
//! decision 800).**
//!
//! * The CSV reads back field by field: the rows used, the iterations and the
//!   attempt, each loading and path, covariance, variance and error variance,
//!   squared multiple correlation and effect, every measure of fit in its three
//!   rows, the rows left out (a count line and a column line each), the notes,
//!   the limitations and the manifest's hashes. Every number reads back to the
//!   same double.
//! * A line's first ends are its `to` (or `a`, or the variance's variable), its
//!   second its `from` (or `b`): a column its index in the model's columns and
//!   its alias, a factor its number with an empty name. The headers never reach
//!   either file (a sentinel, with its positive control).
//! * A value that is not computable is an empty field with its code.
//! * The bytes are pinned on macOS aarch64 (captured 15 Nov 2026): a golden CSV
//!   and the manifest's JSON for a factor on three columns with a column
//!   pointing into it and a path from it to a fourth column, a row left out by
//!   each reason. Every other platform checks that pin's layout
//!   (`pin_support`) until a Linux run gives its bytes (decision 800). It never
//!   passes by skipping.
//! * Ten runs give byte-identical CSV and manifest bytes.

use gaply_core::contracts::{Arch, OperationId, Os, Platform, RunManifest};
use gaply_core::stats_l1::dataset::{build_dataset, DataFingerprint, Dataset, RawTable, TableRef};
use gaply_core::stats_sem::export::{manifest_json, result_csv};
use gaply_core::stats_sem::{compute, manifest, spec, SemCode, SemOutcome, SemParams, Stat};

mod pin_support;
use pin_support::{csv_layout_matches, json_layout_matches};

const HEADER_SENTINEL: &str = "HDR_SENTINEL_s9c";

fn dataset(headers: &[&str], rows: &[Vec<String>]) -> Dataset {
    build_dataset(
        RawTable::from_rows(headers.iter().map(|h| h.to_string()).collect(), rows.to_vec()),
        DataFingerprint::of_bytes(format!("{headers:?}{rows:?}").as_bytes(), "sem export", TableRef::FirstTable),
    )
    .expect("dataset")
}

fn platform() -> Platform {
    Platform { os: Os::Macos, arch: Arch::Aarch64 }
}

/// A factor on columns 1, 2 and 3; column 4 points into it; it points into column 5;
/// the code −99.
fn params() -> SemParams {
    serde_json::from_str(
        r#"{"indicators":[["C1","C2","C3"]],
            "paths":[{"to":{"factor":1},"from":{"column":"C4"}},{"to":{"column":"C5"},"from":{"factor":1}}],
            "covariances":[],"equal":[],"missingCodes":[-99]}"#,
    )
    .unwrap()
}

/// Sixty rows of five columns by an integer formula, the factor the first three's
/// shared part, column 4 its cause and column 5 its effect; then a row left out by
/// each reason: an empty cell in column 2, a code in column 5.
fn golden_rows() -> Vec<Vec<String>> {
    let mut rows: Vec<Vec<String>> = (1..=60i64)
        .map(|i| {
            let cause = (i * 7907) % 1013 - 506;
            let factor = cause + (i * 104729) % 1009 - 504;
            let noise = |j: i64| (i * 2654435761 + j * 7919 + i * j * 31) % 997 - 498;
            [factor + noise(1), 2 * factor + noise(2), 3 * factor + noise(3), cause, factor + noise(5)].iter().map(i64::to_string).collect()
        })
        .collect();
    rows.push(vec!["1".into(), String::new(), "3".into(), "4".into(), "5".into()]);
    rows.push(vec!["1".into(), "2".into(), "3".into(), "4".into(), "-99".into()]);
    rows
}

fn headers() -> Vec<&'static str> {
    vec![HEADER_SENTINEL, "x2", "x3", "x4", "x5"]
}

fn run_of(d: &Dataset, p: SemParams) -> (SemOutcome, RunManifest<SemCode>) {
    let s = spec(OperationId(21), d, p);
    let r = compute(d, &s.parameters).expect("computes");
    let m = manifest(&s, d, &r, 0, platform());
    (r, m)
}

fn golden_case() -> (Vec<u8>, Vec<u8>) {
    let d = dataset(&headers(), &golden_rows());
    let (r, m) = run_of(&d, params());
    (result_csv(&r, &m), manifest_json(&m))
}

fn read_csv(bytes: &[u8]) -> Vec<Vec<String>> {
    let text = String::from_utf8(bytes.to_vec()).expect("utf-8");
    assert!(text.ends_with("\r\n"));
    text.trim_end_matches("\r\n").split("\r\n").map(|l| l.split(',').map(|f| f.trim_matches('"').to_string()).collect()).collect()
}

fn line<'a>(csv: &'a [Vec<String>], table: &str, statistic: &str) -> &'a Vec<String> {
    csv.iter().find(|r| r[0] == table && r[1] == statistic).unwrap_or_else(|| panic!("{table} {statistic}"))
}

fn num(s: Stat) -> f64 {
    match s {
        Stat::Value(v) => v,
        Stat::NotComputable(_) => panic!("not computable"),
    }
}

#[test]
fn the_csv_reads_back_and_every_number_to_its_double() {
    let d = dataset(&["a", "b", "c", "d", "e"], &golden_rows());
    let (r, m) = run_of(&d, params());
    let csv = read_csv(&result_csv(&r, &m));
    assert_eq!(csv[0], ["table", "statistic", "column_index", "column", "column_index_2", "column_2", "value", "word"]);
    for row in &csv {
        assert_eq!(row.len(), 8, "{row:?}");
    }
    assert_eq!(line(&csv, "rows", "used")[6], "60");
    assert_eq!(line(&csv, "estimation", "iterations")[6], r.iterations.to_string());
    assert_eq!(line(&csv, "estimation", "attempt")[6], r.attempt.to_string());
    let loadings: Vec<(&str, &str, &str, &str)> =
        csv.iter().filter(|r| r[0] == "loadings" && r[1] == "estimate").map(|r| (r[2].as_str(), r[3].as_str(), r[4].as_str(), r[5].as_str())).collect();
    assert_eq!(loadings, [("0", "C1", "1", ""), ("1", "C2", "1", ""), ("2", "C3", "1", "")]);
    let paths: Vec<(&str, &str, &str, &str)> =
        csv.iter().filter(|r| r[0] == "paths" && r[1] == "estimate").map(|r| (r[2].as_str(), r[3].as_str(), r[4].as_str(), r[5].as_str())).collect();
    assert_eq!(paths, [("1", "", "3", "C4"), ("4", "C5", "1", "")], "the factor by its number, a column by its index and alias");
    assert_eq!(line(&csv, "paths", "estimate")[6].parse::<f64>().unwrap(), num(r.weights[3].estimate));
    let se = line(&csv, "loadings", "se");
    assert_eq!((se[6].as_str(), se[7].as_str()), ("", "stats.sem.fixed_parameter"), "a first loading's standard error");
    assert_eq!(csv.iter().filter(|r| r[0] == "variances").count(), 4, "column 4's variance");
    assert_eq!(csv.iter().filter(|r| r[0] == "error_variances").count(), 5 * 4, "the factor's and four columns' errors");
    assert_eq!(csv.iter().filter(|r| r[0] == "smc").count(), 5);
    let effects: Vec<(&str, &str)> = csv.iter().filter(|r| r[0] == "effects" && r[1] == "total").map(|r| (r[2].as_str(), r[4].as_str())).collect();
    assert_eq!(effects, [("1", "3"), ("4", "1"), ("4", "3")], "the factor from column 4, column 5 from the factor and from column 4");
    assert_eq!(line(&csv, "effects", "indirect")[6].parse::<f64>().unwrap(), num(r.effects[0].indirect));
    assert_eq!(line(&csv, "fit", "cmin")[6].parse::<f64>().unwrap(), num(r.fit.model.cmin));
    assert_eq!(csv.iter().filter(|r| r[0] == "fit").count(), 38);
    assert_eq!(csv.iter().filter(|r| r[0] == "fit_saturated").count(), 38);
    assert_eq!(csv.iter().filter(|r| r[0] == "fit_independence").count(), 38);
    let excluded: Vec<&Vec<String>> = csv.iter().filter(|r| r[0] == "excluded").collect();
    assert_eq!(excluded.len(), 4, "two rows, two lines each");
    assert_eq!(csv.iter().filter(|r| r[0] == "limitation").count(), 15);
    assert_eq!(line(&csv, "manifest", "output_sha256")[7], m.output_hash.as_hex());
}

#[test]
fn a_not_computable_value_is_an_empty_field_with_its_code() {
    let rows: Vec<Vec<String>> = golden_rows().into_iter().take(60).map(|r| r[..3].to_vec()).collect();
    let d = dataset(&["a", "b", "c"], &rows);
    let p: SemParams = serde_json::from_str(r#"{"indicators":[["C1","C2","C3"]],"paths":[],"covariances":[],"equal":[],"missingCodes":[]}"#).unwrap();
    let (r, m) = run_of(&d, p);
    assert_eq!(r.fit.model.df, 0);
    let csv = read_csv(&result_csv(&r, &m));
    let rmsea = line(&csv, "fit", "rmsea");
    assert_eq!((rmsea[6].as_str(), rmsea[7].as_str()), ("", "stats.sem.df_zero"));
    let notes: Vec<&str> = csv.iter().filter(|r| r[0] == "note").map(|r| r[7].as_str()).collect();
    assert_eq!(notes.last(), Some(&"stats.sem.df_zero"), "{notes:?}");
}

#[test]
fn neither_file_holds_a_header() {
    let (csv, json) = golden_case();
    let d = dataset(&headers(), &golden_rows());
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
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/stats_sem").join(name)
}

fn pin_bytes(name: &str) -> Vec<u8> {
    std::fs::read(pin_path(name)).unwrap_or_else(|e| panic!("{name}: {e}"))
}

/// The pin's difference from `got`, if any, so both files are reported.
fn pinned(name: &str, got: &[u8]) -> Option<String> {
    if std::env::var_os("GAPLY_WRITE_SEM_EXPORT_PIN").is_some() {
        std::fs::write(pin_path(name), got).expect("write the pin");
    }
    let want = pin_bytes(name);
    (got != want.as_slice()).then(|| {
        format!("{name}: the bytes differ from the pin.\n--- got ---\n{}\n--- pinned ---\n{}", String::from_utf8_lossy(got), String::from_utf8_lossy(&want))
    })
}

const MAC_CSV: &str = "export_golden.csv";
const MAC_JSON: &str = "export_golden_manifest.json";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PinCheck {
    Exact,
    Layout,
}

/// Exact on macOS aarch64, where the pin was captured (decision 800); the layout
/// check everywhere else until a Linux run gives its bytes.
fn pin_check(os: &str, arch: &str) -> PinCheck {
    match (os, arch) {
        ("macos", "aarch64") => PinCheck::Exact,
        _ => PinCheck::Layout,
    }
}

#[test]
fn the_pin_is_checked_exactly_on_macos_aarch64_only_until_a_linux_run() {
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
        let differ: Vec<String> = [pinned(MAC_CSV, &csv), pinned(MAC_JSON, &json)].into_iter().flatten().collect();
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
