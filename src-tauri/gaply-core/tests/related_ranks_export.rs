//! **The related rank tests' exports (Research OS, Phase 8Z-1, decision 638).**
//!
//! * The CSV reads back field by field: each column's median, the rows used, the
//!   test's lines, the rows left out (a count line and a column line each), the
//!   notes, the limitations and the manifest's hashes. Every number reads back to
//!   the same double.
//! * A column is its index and its alias. The headers never reach either file
//!   (a sentinel, with its positive control).
//! * A Friedman result has no Wilcoxon lines, a value that is not computable is
//!   an empty field with its code, and a side with no difference an empty mean.
//! * The bytes are pinned on macOS aarch64 (captured 10 Nov 2026): a golden CSV and
//!   the manifest's JSON for a Wilcoxon test with its exact p and rows left out by
//!   each reason. Every other platform, Linux x86_64 included, checks that pin's
//!   layout (decision 346's check, `pin_support`, unchanged) until a Linux run
//!   measures it: the normal p comes from the platform's maths library through
//!   statrs, so another platform's last digits may differ. It never passes by
//!   skipping.
//! * Ten runs give byte-identical CSV and manifest bytes.

use gaply_core::contracts::{Arch, OperationId, Os, Platform, RunManifest};
use gaply_core::stats_desc::Alias;
use gaply_core::stats_l1::dataset::{
    build_dataset, ColumnAlias, DataFingerprint, Dataset, RawTable, TableRef,
};
use gaply_core::stats_related_ranks::export::{manifest_json, result_csv};
use gaply_core::stats_related_ranks::{
    compute, manifest, spec, RelatedRanksCode, RelatedRanksOutcome, RelatedRanksParams,
};

mod pin_support;
use pin_support::{csv_layout_matches, json_layout_matches};

const HEADER_SENTINEL: &str = "HDR_SENTINEL_z81";

fn dataset(headers: &[&str], rows: &[Vec<&str>]) -> Dataset {
    build_dataset(
        RawTable::from_rows(
            headers.iter().map(|h| h.to_string()).collect(),
            rows.iter().map(|r| r.iter().map(|c| c.to_string()).collect()).collect(),
        ),
        DataFingerprint::of_bytes(format!("{headers:?}{rows:?}").as_bytes(), "related export", TableRef::FirstTable),
    )
    .expect("dataset")
}

fn platform() -> Platform {
    Platform {
        os: Os::Macos,
        arch: Arch::Aarch64,
    }
}

fn params(columns: &[u32], codes: Vec<f64>) -> RelatedRanksParams {
    RelatedRanksParams {
        columns: columns.iter().map(|n| Alias(ColumnAlias(*n))).collect(),
        missing_codes: codes,
    }
}

/// Eleven rows: eight complete pairs, untied, one row left out by each reason in
/// each column.
fn golden_rows() -> Vec<Vec<&'static str>> {
    vec![
        vec!["1.25", "2.5"],
        vec!["3.5", "3.1"],
        vec!["2.75", "4.25"],
        vec!["5.9", "1.6"],
        vec!["4.15", "5.05"],
        vec!["7.3", "4.2"],
        vec!["6.05", "3.95"],
        vec!["8.4", "2.3"],
        vec!["", "1"],
        vec!["-9", "2"],
        vec!["3", ""],
    ]
}

fn run_of(d: &Dataset, p: RelatedRanksParams) -> (RelatedRanksOutcome, RunManifest<RelatedRanksCode>) {
    let s = spec(OperationId(13), d, p);
    let r = compute(d, &s.parameters).expect("computes");
    let m = manifest(&s, d, &r, 0, platform());
    (r, m)
}

fn golden_case() -> (Vec<u8>, Vec<u8>) {
    let d = dataset(&[HEADER_SENTINEL, "after"], &golden_rows());
    let (r, m) = run_of(&d, params(&[1, 2], vec![-9.0]));
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

#[test]
fn the_csv_reads_back_and_every_number_to_its_double() {
    let d = dataset(&["a", "b"], &golden_rows());
    let (r, m) = run_of(&d, params(&[1, 2], vec![-9.0]));
    let csv = read_csv(&result_csv(&r, &m));
    assert_eq!(csv[0], ["table", "statistic", "column_index", "column", "column_index_2", "column_2", "value", "word"]);
    for row in &csv {
        assert_eq!(row.len(), 8, "{row:?}");
    }
    assert_eq!(line(&csv, "columns", "median")[2..4], ["0", "C1"]);
    assert_eq!(line(&csv, "columns", "median")[6].parse::<f64>().unwrap(), r.columns[0].median);
    assert_eq!(line(&csv, "rows", "used")[6], "8");
    let gaply_core::stats_related_ranks::RelatedTest::Wilcoxon(t) = r.test else { panic!("Wilcoxon") };
    let v = line(&csv, "wilcoxon", "v");
    assert_eq!(v[2..6], ["0", "C1", "1", "C2"]);
    assert_eq!(v[6].parse::<f64>().unwrap(), t.v);
    assert_eq!(line(&csv, "wilcoxon", "exact_p")[6].parse::<f64>().unwrap(), t.exact_p.unwrap());
    let excluded: Vec<&Vec<String>> = csv.iter().filter(|r| r[0] == "excluded").collect();
    assert_eq!(excluded.len(), 6, "three reasons, two lines each");
    assert_eq!(csv.iter().filter(|r| r[0] == "limitation").count(), 8);
    assert_eq!(line(&csv, "manifest", "output_sha256")[7], m.output_hash.as_hex());
}

#[test]
fn a_friedman_result_has_no_wilcoxon_lines_and_a_not_computable_value_is_its_code() {
    let d = dataset(&["a", "b", "c"], &[vec!["1", "1", "1"], vec!["2", "2", "2"], vec!["3", "3", "3"]]);
    let (r, m) = run_of(&d, params(&[3, 1, 2], vec![]));
    let csv = read_csv(&result_csv(&r, &m));
    assert!(csv.iter().all(|r| r[0] != "wilcoxon"));
    let q = line(&csv, "friedman", "q");
    assert_eq!((q[6].as_str(), q[7].as_str()), ("", "stats.related_ranks.all_values_tied"));
    let columns: Vec<&str> = csv.iter().filter(|r| r[0] == "friedman" && r[1] == "rank_sum").map(|r| r[3].as_str()).collect();
    assert_eq!(columns, ["C3", "C1", "C2"], "the list's order");
    assert_eq!(line(&csv, "friedman", "df")[6], "2");
}

#[test]
fn a_side_with_no_difference_has_an_empty_mean_and_an_absent_exact_p_carries_its_note() {
    let d = dataset(&["a", "b"], &[vec!["5", "1"], vec!["9", "1"], vec!["8", "4"], vec!["7", "7"]]);
    let (r, m) = run_of(&d, params(&[1, 2], vec![]));
    let csv = read_csv(&result_csv(&r, &m));
    let mean = line(&csv, "wilcoxon", "negative_mean_rank");
    assert_eq!((mean[6].as_str(), mean[7].as_str()), ("", ""));
    assert_eq!(line(&csv, "wilcoxon", "exact_p")[7], "stats.related_ranks.exact_not_given_ties");
}

#[test]
fn neither_file_holds_a_header() {
    let (csv, json) = golden_case();
    let d = dataset(&[HEADER_SENTINEL, "after"], &golden_rows());
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
        .join("tests/fixtures/stats_related_ranks")
        .join(name)
}

fn pin_bytes(name: &str) -> Vec<u8> {
    std::fs::read(pin_path(name)).unwrap_or_else(|e| panic!("{name}: {e}"))
}

/// The pin's difference from `got`, if any, so both files are reported.
fn pinned(name: &str, got: &[u8]) -> Option<String> {
    if std::env::var_os("GAPLY_WRITE_RELATED_RANKS_EXPORT_PIN").is_some() {
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

/// The check for a platform, by `std::env::consts::OS` and `ARCH`.
fn pin_check(os: &str, arch: &str) -> PinCheck {
    match (os, arch) {
        ("macos", "aarch64") => PinCheck::Exact,
        _ => PinCheck::Layout,
    }
}

/// Decision 638, run on every platform: exact bytes on macOS aarch64, the layout
/// check everywhere else, Linux x86_64 included until it is measured, never a skip.
#[test]
fn the_pin_is_checked_exactly_on_macos_aarch64_and_by_its_layout_elsewhere() {
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
        .find(|l| l.starts_with("\"wilcoxon\",\"p\","))
        .expect("the Wilcoxon p")
        .to_string();
    let value = line.split(',').nth(6).unwrap().to_string();
    let moved = |n: u64| {
        let v: f64 = value.parse().expect("a number");
        line.replacen(&value, &format!("{}", f64::from_bits(v.to_bits() + n)), 1)
    };
    assert_eq!(csv_layout_matches(&altered(&line, &moved(1)), &pin), Ok(()));
    assert!(csv_layout_matches(&altered(&line, &moved(5)), &pin).is_err(), "5 ulps off");
}
