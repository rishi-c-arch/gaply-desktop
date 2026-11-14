//! **The confirmatory factor analysis's exports (Research OS, Phase 9C-1,
//! decision 746).**
//!
//! * The CSV reads back field by field: the rows used and the iterations, each
//!   loading, factor variance, factor covariance, error variance and squared
//!   multiple correlation, every measure of fit and the independence model's, the
//!   rows left out (a count line and a column line each), the notes, the
//!   limitations and the manifest's hashes. Every number reads back to the same
//!   double.
//! * A column is its index in the spec's list and its alias; a factor is its
//!   number from 1 in the second index field, a covariance its first factor's in
//!   the first. The headers never reach either file (a sentinel, with its
//!   positive control).
//! * A value that is not computable is an empty field with its code.
//! * The bytes are pinned on macOS aarch64 (captured 13 Nov 2026): a golden CSV
//!   and the manifest's JSON for six columns on two correlated factors, a row
//!   left out by each reason. Every other platform checks that pin's layout
//!   (`pin_support`) until a Linux run gives its bytes: the logarithms come from
//!   the platform's `ln` and the distributions through statrs. It never passes by
//!   skipping.
//! * Ten runs give byte-identical CSV and manifest bytes.

use gaply_core::contracts::{Arch, OperationId, Os, Platform, RunManifest};
use gaply_core::stats_cfa::export::{manifest_json, result_csv};
use gaply_core::stats_cfa::{compute, manifest, spec, CfaCode, CfaOutcome, CfaParams, Stat};
use gaply_core::stats_desc::Alias;
use gaply_core::stats_l1::dataset::{build_dataset, ColumnAlias, DataFingerprint, Dataset, RawTable, TableRef};

mod pin_support;
use pin_support::{csv_layout_matches, json_layout_matches};

const HEADER_SENTINEL: &str = "HDR_SENTINEL_c9a";

fn dataset(headers: &[&str], rows: &[Vec<String>]) -> Dataset {
    build_dataset(
        RawTable::from_rows(headers.iter().map(|h| h.to_string()).collect(), rows.to_vec()),
        DataFingerprint::of_bytes(format!("{headers:?}{rows:?}").as_bytes(), "cfa export", TableRef::FirstTable),
    )
    .expect("dataset")
}

fn platform() -> Platform {
    Platform {
        os: Os::Macos,
        arch: Arch::Aarch64,
    }
}

fn a(n: u32) -> Alias {
    Alias(ColumnAlias(n))
}

/// Factor 1 is columns 1, 3, 5; factor 2 is 2, 4, 6; correlated; the code −99.
fn params() -> CfaParams {
    CfaParams {
        indicators: vec![vec![a(1), a(3), a(5)], vec![a(2), a(4), a(6)]],
        correlated_factors: true,
        missing_codes: vec![-99.0],
    }
}

/// Forty rows of six columns on two factors, by cfa_r_golden.rs's integer
/// formula; then a row left out by each reason: an empty cell in column 2, a
/// code in column 5.
fn golden_rows() -> Vec<Vec<String>> {
    let mut rows: Vec<Vec<String>> = (1..=40u64)
        .map(|i| {
            (1..=6u64)
                .map(|j| {
                    let f = (j - 1) % 2;
                    let score = ((i * (7907 + 104729 * f)) % 1013) as i64 - 506;
                    let shared = ((i * 2654435761) % 1009) as i64 - 504;
                    let noise = ((i * 104729 + j * 7919 + i * j * 31) % 997) as i64 - 498;
                    ((2 + (j % 3) as i64) * (score + shared) + noise).to_string()
                })
                .collect()
        })
        .collect();
    rows.push(vec!["1".into(), String::new(), "3".into(), "4".into(), "5".into(), "6".into()]);
    rows.push(vec!["1".into(), "2".into(), "3".into(), "4".into(), "-99".into(), "6".into()]);
    rows
}

fn headers() -> Vec<&'static str> {
    vec![HEADER_SENTINEL, "x2", "x3", "x4", "x5", "x6"]
}

fn run_of(d: &Dataset, p: CfaParams) -> (CfaOutcome, RunManifest<CfaCode>) {
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
    text.trim_end_matches("\r\n")
        .split("\r\n")
        .map(|l| l.split(',').map(|f| f.trim_matches('"').to_string()).collect())
        .collect()
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
    let d = dataset(&["a", "b", "c", "d", "e", "f"], &golden_rows());
    let (r, m) = run_of(&d, params());
    let csv = read_csv(&result_csv(&r, &m));
    assert_eq!(csv[0], ["table", "statistic", "column_index", "column", "column_index_2", "column_2", "value", "word"]);
    for row in &csv {
        assert_eq!(row.len(), 8, "{row:?}");
    }
    assert_eq!(line(&csv, "rows", "used")[6], "40");
    assert_eq!(line(&csv, "estimation", "iterations")[6], r.iterations.to_string());
    let estimates: Vec<(&str, &str, &str)> = csv.iter().filter(|r| r[0] == "loadings" && r[1] == "estimate").map(|r| (r[2].as_str(), r[3].as_str(), r[4].as_str())).collect();
    assert_eq!(estimates, [("0", "C1", "1"), ("1", "C3", "1"), ("2", "C5", "1"), ("3", "C2", "2"), ("4", "C4", "2"), ("5", "C6", "2")]);
    let second = csv.iter().filter(|r| r[0] == "loadings" && r[1] == "estimate").nth(1).unwrap();
    assert_eq!(second[6].parse::<f64>().unwrap(), num(r.loadings[1].estimate));
    let se = csv.iter().find(|r| r[0] == "loadings" && r[1] == "se").unwrap();
    assert_eq!((se[6].as_str(), se[7].as_str()), ("", "stats.cfa.fixed_parameter"), "a marker's standard error");
    let cov = line(&csv, "factor_covariances", "correlation");
    assert_eq!((cov[2].as_str(), cov[4].as_str()), ("1", "2"));
    assert_eq!(cov[6].parse::<f64>().unwrap(), num(r.factor_covariances[0].correlation));
    assert_eq!(line(&csv, "factor_variances", "estimate")[4], "1");
    assert_eq!(csv.iter().filter(|r| r[0] == "error_variances").count(), 6 * 4);
    assert_eq!(csv.iter().filter(|r| r[0] == "smc").count(), 6);
    assert_eq!(line(&csv, "fit", "cmin")[6].parse::<f64>().unwrap(), num(r.fit.model.cmin));
    assert_eq!(line(&csv, "fit", "df")[6], r.fit.model.df.to_string());
    assert_eq!(line(&csv, "fit", "rmsea_hi90")[6].parse::<f64>().unwrap(), num(r.fit.model.rmsea_hi90));
    assert_eq!(csv.iter().filter(|r| r[0] == "fit").count(), 38);
    // Decision 763: the saturated and independence models' rows, as tables of
    // their own with the same statistic names.
    assert_eq!(csv.iter().filter(|r| r[0] == "fit_saturated").count(), 38);
    assert_eq!(csv.iter().filter(|r| r[0] == "fit_independence").count(), 38);
    assert_eq!(line(&csv, "fit_independence", "df")[6], "15");
    assert_eq!(line(&csv, "fit_saturated", "df")[6], "0");
    let excluded: Vec<&Vec<String>> = csv.iter().filter(|r| r[0] == "excluded").collect();
    assert_eq!(excluded.len(), 4, "two rows, two lines each");
    assert_eq!(csv.iter().filter(|r| r[0] == "limitation").count(), 10);
    assert_eq!(line(&csv, "manifest", "output_sha256")[7], m.output_hash.as_hex());
}

#[test]
fn a_not_computable_value_is_an_empty_field_with_its_code() {
    // One factor of three columns: 0 degrees of freedom.
    let rows: Vec<Vec<String>> = golden_rows().into_iter().take(40).map(|r| r[..3].to_vec()).collect();
    let d = dataset(&["a", "b", "c"], &rows);
    let p = CfaParams { indicators: vec![vec![a(1), a(2), a(3)]], correlated_factors: true, missing_codes: vec![] };
    let (r, m) = run_of(&d, p);
    assert_eq!(r.fit.model.df, 0);
    let csv = read_csv(&result_csv(&r, &m));
    let rmsea = line(&csv, "fit", "rmsea");
    assert_eq!((rmsea[6].as_str(), rmsea[7].as_str()), ("", "stats.cfa.df_zero"));
    let notes: Vec<&str> = csv.iter().filter(|r| r[0] == "note").map(|r| r[7].as_str()).collect();
    assert_eq!(notes.last(), Some(&"stats.cfa.df_zero"), "{notes:?}");
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
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/stats_cfa").join(name)
}

fn pin_bytes(name: &str) -> Vec<u8> {
    std::fs::read(pin_path(name)).unwrap_or_else(|e| panic!("{name}: {e}"))
}

/// The pin's difference from `got`, if any, so both files are reported.
fn pinned(name: &str, got: &[u8]) -> Option<String> {
    if std::env::var_os("GAPLY_WRITE_CFA_EXPORT_PIN").is_some() {
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

/// Exact on macOS aarch64, where the pin was captured; the layout check
/// everywhere else until a Linux run gives its bytes (decision 746).
fn pin_check(os: &str, arch: &str) -> PinCheck {
    match (os, arch) {
        ("macos", "aarch64") => PinCheck::Exact,
        _ => PinCheck::Layout,
    }
}

#[test]
fn the_pin_is_checked_exactly_on_macos_aarch64_alone() {
    for (os, arch, want) in [
        ("macos", "aarch64", PinCheck::Exact),
        ("linux", "x86_64", PinCheck::Layout),
        ("windows", "x86_64", PinCheck::Layout),
        ("macos", "x86_64", PinCheck::Layout),
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
