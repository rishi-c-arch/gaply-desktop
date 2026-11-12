//! **The factor analysis's exports (Research OS, Phase 9B-1, decision 701).**
//!
//! * The CSV reads back field by field: each item's mean and SD, the rows used,
//!   the determinant, KMO and each item's MSA, Bartlett's test, every eigenvalue,
//!   the extraction and the rotation, the rows left out (a count line and a column
//!   line each), the notes, the limitations and the manifest's hashes. Every
//!   number reads back to the same double.
//! * An item is its index in the spec's list and its alias; a factor or an
//!   eigenvalue is its number from 1 in the second index field. The headers never
//!   reach either file (a sentinel, with its positive control).
//! * A value that is not computable is an empty field with its code.
//! * The bytes are pinned on macOS aarch64 (captured 11 Nov 2026): a golden CSV
//!   and the manifest's JSON for six items, promax, rows left out by each reason.
//!   Every other platform checks that pin's layout (`pin_support`) until a Linux
//!   run gives its bytes (decision 701): Bartlett's p comes from the platform's
//!   maths library through statrs, and ln|R| from the platform's `ln`. It never
//!   passes by skipping.
//! * Ten runs give byte-identical CSV and manifest bytes.

use gaply_core::contracts::{Arch, OperationId, Os, Platform, RunManifest};
use gaply_core::stats_desc::Alias;
use gaply_core::stats_factor::export::{manifest_json, result_csv};
use gaply_core::stats_factor::{compute, manifest, spec, Extraction, FactorCode, FactorOutcome, FactorParams, Rotation, Stat};
use gaply_core::stats_l1::dataset::{build_dataset, ColumnAlias, DataFingerprint, Dataset, RawTable, TableRef};

mod pin_support;
use pin_support::{csv_layout_matches, json_layout_matches};

const HEADER_SENTINEL: &str = "HDR_SENTINEL_f62";

fn dataset(headers: &[&str], rows: &[Vec<String>]) -> Dataset {
    build_dataset(
        RawTable::from_rows(headers.iter().map(|h| h.to_string()).collect(), rows.to_vec()),
        DataFingerprint::of_bytes(format!("{headers:?}{rows:?}").as_bytes(), "factor export", TableRef::FirstTable),
    )
    .expect("dataset")
}

fn platform() -> Platform {
    Platform {
        os: Os::Macos,
        arch: Arch::Aarch64,
    }
}

fn params(rotation: Rotation, codes: Vec<f64>) -> FactorParams {
    FactorParams {
        items: (1..=6).map(|j| Alias(ColumnAlias(j))).collect(),
        extraction: Extraction::PrincipalAxis,
        factors: Some(2),
        rotation,
        iterations: 50,
        convergence: 0.001,
        missing_codes: codes,
    }
}

/// Thirty-two rows of six items on two factors, by an integer formula; then a row
/// left out by each reason: an empty cell in item 2, a code in item 5.
fn golden_rows() -> Vec<Vec<String>> {
    let mut rows: Vec<Vec<String>> = (1..=32u64)
        .map(|i| {
            (1..=6u64)
                .map(|j| {
                    let f = (j - 1) % 2;
                    let score = ((i * 7907 + f * 15485863) % 1013) as i64 - 506;
                    let noise = ((i * 104729 + j * 7919 + i * j * 31) % 997) as i64 - 498;
                    (2 * score + noise).to_string()
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

fn run_of(d: &Dataset, p: FactorParams) -> (FactorOutcome, RunManifest<FactorCode>) {
    let s = spec(OperationId(19), d, p);
    let r = compute(d, &s.parameters).expect("computes");
    let m = manifest(&s, d, &r, 0, platform());
    (r, m)
}

fn golden_case() -> (Vec<u8>, Vec<u8>) {
    let d = dataset(&headers(), &golden_rows());
    let (r, m) = run_of(&d, params(Rotation::Promax, vec![-99.0]));
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
    let (r, m) = run_of(&d, params(Rotation::Promax, vec![-99.0]));
    let csv = read_csv(&result_csv(&r, &m));
    assert_eq!(csv[0], ["table", "statistic", "column_index", "column", "column_index_2", "column_2", "value", "word"]);
    for row in &csv {
        assert_eq!(row.len(), 8, "{row:?}");
    }
    let means: Vec<(&str, &str)> = csv.iter().filter(|r| r[0] == "items" && r[1] == "mean").map(|r| (r[2].as_str(), r[3].as_str())).collect();
    assert_eq!(means, [("0", "C1"), ("1", "C2"), ("2", "C3"), ("3", "C4"), ("4", "C5"), ("5", "C6")]);
    assert_eq!(line(&csv, "items", "mean")[6].parse::<f64>().unwrap(), num(r.descriptives[0].mean));
    assert_eq!(line(&csv, "rows", "used")[6], "32");
    assert_eq!(line(&csv, "kmo", "overall")[6].parse::<f64>().unwrap(), num(r.kmo.overall));
    assert_eq!(line(&csv, "bartlett", "df")[6], "15");
    assert_eq!(line(&csv, "bartlett", "p")[6].parse::<f64>().unwrap(), num(r.bartlett.p));
    let eig: Vec<(&str, &str)> = csv.iter().filter(|r| r[0] == "eigenvalues" && r[1] == "total").map(|r| (r[2].as_str(), r[4].as_str())).collect();
    assert_eq!(eig, [("", "1"), ("", "2"), ("", "3"), ("", "4"), ("", "5"), ("", "6")]);
    assert_eq!((line(&csv, "extraction", "method")[7].as_str(), line(&csv, "extraction", "factors")[6].as_str()), ("principal_axis", "2"));
    let loadings: Vec<&Vec<String>> = csv.iter().filter(|r| r[0] == "extraction_loadings").collect();
    assert_eq!(loadings.len(), 12);
    assert_eq!((loadings[1][2].as_str(), loadings[1][3].as_str(), loadings[1][4].as_str()), ("0", "C1", "2"));
    assert_eq!(loadings[1][6].parse::<f64>().unwrap(), num(r.extraction.loadings[0][1]));
    let rot = r.rotation.as_ref().unwrap();
    assert_eq!(line(&csv, "rotation", "method")[7], "promax");
    assert_eq!(csv.iter().filter(|r| r[0] == "rotation_loadings" && r[1] == "pattern").count(), 12);
    assert_eq!(csv.iter().filter(|r| r[0] == "rotation_structure").count(), 12);
    let phi: Vec<&Vec<String>> = csv.iter().filter(|r| r[0] == "factor_correlations").collect();
    assert_eq!(phi.iter().map(|r| (r[2].as_str(), r[4].as_str())).collect::<Vec<_>>(), [("1", "1"), ("1", "2"), ("2", "1"), ("2", "2")]);
    assert_eq!(phi[1][6].parse::<f64>().unwrap(), num(rot.factor_correlations.as_ref().unwrap()[0][1]));
    assert_eq!(csv.iter().filter(|r| r[0] == "rotation_transformation" || r[0] == "rotation_sums").count(), 0, "promax gives neither");
    let excluded: Vec<&Vec<String>> = csv.iter().filter(|r| r[0] == "excluded").collect();
    assert_eq!(excluded.len(), 4, "two rows, two lines each");
    assert_eq!(csv.iter().filter(|r| r[0] == "limitation").count(), 9);
    assert_eq!(line(&csv, "manifest", "output_sha256")[7], m.output_hash.as_hex());
}

#[test]
fn varimax_writes_its_transformation_and_sums() {
    let d = dataset(&["a", "b", "c", "d", "e", "f"], &golden_rows());
    let (r, m) = run_of(&d, params(Rotation::Varimax, vec![-99.0]));
    let csv = read_csv(&result_csv(&r, &m));
    assert_eq!(csv.iter().filter(|r| r[0] == "rotation_transformation").count(), 4);
    assert_eq!(csv.iter().filter(|r| r[0] == "rotation_sums").count(), 6);
    assert_eq!(csv.iter().filter(|r| r[0] == "rotation_structure" || r[0] == "factor_correlations").count(), 0);
}

#[test]
fn a_not_computable_value_is_an_empty_field_with_its_code() {
    let rows: Vec<Vec<String>> = [[1, 1, 1], [-1, 1, 1], [1, -1, 1], [-1, -1, 1], [1, 1, -1], [-1, 1, -1], [1, -1, -1], [-1, -1, -1]]
        .iter()
        .map(|r| r.iter().map(|x| x.to_string()).collect())
        .collect();
    let d = dataset(&["a", "b", "c"], &rows);
    let mut p = params(Rotation::None, vec![]);
    p.items.truncate(3);
    p.extraction = Extraction::PrincipalComponents;
    p.factors = Some(1);
    let (r, m) = run_of(&d, p);
    let csv = read_csv(&result_csv(&r, &m));
    let k = line(&csv, "kmo", "overall");
    assert_eq!((k[6].as_str(), k[7].as_str()), ("", "stats.factor.no_correlation"), "a KMO of 0/0 (decision 714)");
    assert_eq!(line(&csv, "note", "")[7], "stats.factor.eigenvalues_not_distinct");
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
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/stats_factor").join(name)
}

fn pin_bytes(name: &str) -> Vec<u8> {
    std::fs::read(pin_path(name)).unwrap_or_else(|e| panic!("{name}: {e}"))
}

/// The pin's difference from `got`, if any, so both files are reported.
fn pinned(name: &str, got: &[u8]) -> Option<String> {
    if std::env::var_os("GAPLY_WRITE_FACTOR_EXPORT_PIN").is_some() {
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
/// everywhere else until a Linux run gives its bytes (decision 701).
fn pin_check(os: &str, arch: &str) -> PinCheck {
    match (os, arch) {
        ("macos", "aarch64") => PinCheck::Exact,
        _ => PinCheck::Layout,
    }
}

#[test]
fn the_pin_is_checked_exactly_on_macos_aarch64_only() {
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
