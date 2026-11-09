//! **Crosstabs' exports (Research OS, Phase 8X-1, decision 577).**
//!
//! * The CSV reads back field by field, in the screen's order: each cell's
//!   eight numbers row by row, the totals, n and the expected counts' facts,
//!   the tests, the measures, then the rows left out (a count line and a
//!   column line each), the note, the limitations and the manifest's hashes.
//!   Every number reads back to the same double.
//! * A level is its index and its label; a label that starts with a formula
//!   character gets one leading apostrophe (`stats_desc::export::text`).
//!   The headers never reach the CSV, and the manifest's JSON holds no
//!   header, label or path (sentinels, with positive controls).
//! * A table larger than 2 x 2 has no continuity-corrected or Fisher lines.
//! * The bytes are pinned on macOS aarch64 (captured 9 Nov 2026): a golden
//!   CSV and the manifest's JSON for a 2 x 2 table over 23 rows, with a
//!   ticked missing code in each column, an empty cell in each, and a cell
//!   with an expected count below 5. Every other platform, Linux x86_64
//!   included, checks that pin's layout (decision 346's check,
//!   `pin_support`, unchanged), as the correlations' and alpha's pins first
//!   did (decisions 354 and 379): the chi-square's p and Fisher's come from
//!   the platform's maths library through statrs, so another platform's last
//!   digits may differ. Linux x86_64 is measured outside this Mac before it
//!   is made exact (decision 577). It never passes by skipping.
//! * Ten runs give byte-identical CSV and manifest bytes.

use gaply_core::contracts::{Arch, OperationId, Os, Platform, RunManifest};
use gaply_core::stats_desc::Alias;
use gaply_core::stats_l1::dataset::{
    build_dataset, ColumnAlias, DataFingerprint, Dataset, RawTable, TableRef,
};
use gaply_core::stats_xtab::export::{manifest_json, result_csv};
use gaply_core::stats_xtab::{
    compute, manifest, spec, CrosstabsOutcome, CrosstabsParams, XtabCode,
};

mod pin_support;
use pin_support::{csv_layout_matches, json_layout_matches};

fn dataset(headers: [&str; 2], rows: &[(&str, &str)]) -> Dataset {
    build_dataset(
        RawTable::from_rows(
            headers.iter().map(|h| h.to_string()).collect(),
            rows.iter()
                .map(|(r, c)| vec![r.to_string(), c.to_string()])
                .collect(),
        ),
        DataFingerprint::of_bytes(
            format!("{headers:?}{rows:?}").as_bytes(),
            "xtab export",
            TableRef::FirstTable,
        ),
    )
    .expect("dataset")
}

fn platform() -> Platform {
    Platform {
        os: Os::Macos,
        arch: Arch::Aarch64,
    }
}

fn params(codes: Vec<f64>) -> CrosstabsParams {
    CrosstabsParams {
        rows: Alias(ColumnAlias(1)),
        columns: Alias(ColumnAlias(2)),
        missing_codes: codes,
    }
}

/// The golden's 23 rows: 2 x 2 over "=lo" and "hi" by "no" and "yes", one
/// row left out by each reason.
fn golden_rows() -> Vec<(&'static str, &'static str)> {
    let mut rows = Vec::new();
    for (r, c, n) in [("=lo", "no", 8), ("=lo", "yes", 2), ("hi", "no", 3), ("hi", "yes", 6)] {
        for _ in 0..n {
            rows.push((r, c));
        }
    }
    rows.extend([("", "no"), ("-9", "yes"), ("hi", ""), ("=lo", "-9")]);
    rows
}

fn run_of(d: &Dataset, p: CrosstabsParams) -> (CrosstabsOutcome, RunManifest<XtabCode>) {
    let s = spec(OperationId(13), d, p);
    let r = compute(d, &s.parameters).expect("computes");
    let m = manifest(&s, d, &r, 0, platform());
    (r, m)
}

fn golden_case() -> (Vec<u8>, Vec<u8>) {
    let d = dataset(["rows", "columns"], &golden_rows());
    let (r, m) = run_of(&d, params(vec![-9.0]));
    (result_csv(&r, &m), manifest_json(&m))
}

/// RFC 4180: CRLF between records, `"` around a field, `""` inside one.
fn read_csv(bytes: &[u8]) -> Vec<[String; 8]> {
    let s = std::str::from_utf8(bytes).expect("utf-8");
    let body = s
        .strip_suffix("\r\n")
        .expect("the last record ends with CRLF");
    body.split("\r\n")
        .map(|line| {
            let mut fields = Vec::new();
            let mut field = String::new();
            let mut quoted = false;
            let mut chars = line.chars().peekable();
            while let Some(c) = chars.next() {
                match c {
                    '"' if quoted && chars.peek() == Some(&'"') => {
                        field.push('"');
                        chars.next();
                    }
                    '"' => quoted = !quoted,
                    ',' if !quoted => fields.push(std::mem::take(&mut field)),
                    _ => field.push(c),
                }
            }
            fields.push(field);
            fields.try_into().expect("eight fields")
        })
        .collect()
}

#[test]
fn the_csv_reads_back_in_the_screens_order_and_every_number_to_its_double() {
    let d = dataset(["rows", "columns"], &golden_rows());
    let (r, m) = run_of(&d, params(vec![-9.0]));
    let rows = read_csv(&result_csv(&r, &m));
    assert_eq!(
        rows[0],
        [
            "table",
            "statistic",
            "row_index",
            "row_level",
            "column_index",
            "column_level",
            "value",
            "word"
        ]
        .map(String::from)
    );
    // The first cell: level 0 of each, "=lo" (with its apostrophe) and "no".
    assert_eq!(
        rows[1],
        ["cells", "observed", "0", "'=lo", "0", "no", "8", ""].map(String::from)
    );
    let tables: Vec<&str> = rows[1..].iter().map(|f| f[0].as_str()).collect();
    let mut order: Vec<&str> = Vec::new();
    for t in &tables {
        if order.last() != Some(t) {
            order.push(t);
        }
    }
    assert_eq!(
        order,
        [
            "cells",
            "row_totals",
            "column_totals",
            "table",
            "pearson",
            "likelihood_ratio",
            "continuity_corrected",
            "fisher",
            "measures",
            "excluded",
            "note",
            "limitation",
            "manifest"
        ]
    );
    // Every cell's eight numbers, each reading back to the result's double.
    for (k, f) in r.frequencies.iter().enumerate() {
        let lines = &rows[1 + 8 * k..1 + 8 * (k + 1)];
        let names: Vec<&str> = lines.iter().map(|l| l[1].as_str()).collect();
        assert_eq!(
            names,
            [
                "observed",
                "expected",
                "row_proportion",
                "column_proportion",
                "total_proportion",
                "residual",
                "pearson_residual",
                "adjusted_residual"
            ]
        );
        let values: Vec<f64> = lines.iter().map(|l| l[6].parse().unwrap()).collect();
        assert_eq!(
            values,
            [
                f.observed as f64,
                f.expected,
                f.row_proportion,
                f.column_proportion,
                f.total_proportion,
                f.residual,
                f.pearson_residual,
                f.adjusted_residual
            ]
        );
    }
    // The rows left out: four reasons, each a count line and a column line,
    // rows' column first.
    let excluded: Vec<(String, String, String)> = rows
        .iter()
        .filter(|f| f[0] == "excluded")
        .map(|f| (f[1].clone(), f[6].clone(), f[7].clone()))
        .collect();
    let line = |s: &str, v: &str, w: &str| (s.to_string(), v.to_string(), w.to_string());
    assert_eq!(
        excluded,
        [
            line("rows", "1", "stats.xtab.missing_row"),
            line("column", "1", "stats.xtab.missing_row"),
            line("rows", "1", "stats.xtab.missing_code"),
            line("column", "1", "stats.xtab.missing_code"),
            line("rows", "1", "stats.xtab.missing_column"),
            line("column", "2", "stats.xtab.missing_column"),
            line("rows", "1", "stats.xtab.missing_code"),
            line("column", "2", "stats.xtab.missing_code"),
        ]
    );
    let note: Vec<&str> = rows
        .iter()
        .filter(|f| f[0] == "note")
        .map(|f| f[7].as_str())
        .collect();
    assert_eq!(note, ["stats.xtab.expected_below_five"]);
    let fisher: Vec<f64> = rows
        .iter()
        .filter(|f| f[0] == "fisher")
        .map(|f| f[6].parse().unwrap())
        .collect();
    let f = r.fisher.expect("a 2 x 2 table has Fisher's test");
    assert_eq!(fisher, [f.two_sided, f.less, f.greater]);
}

#[test]
fn a_larger_table_has_no_continuity_or_fisher_lines() {
    let mut rows = Vec::new();
    for (r, c, n) in [
        ("a", "x", 4),
        ("a", "y", 2),
        ("b", "x", 1),
        ("b", "y", 5),
        ("c", "x", 3),
        ("c", "y", 3),
    ] {
        for _ in 0..n {
            rows.push((r, c));
        }
    }
    let d = dataset(["r", "c"], &rows);
    let (r, m) = run_of(&d, params(Vec::new()));
    assert!(r.continuity_corrected.is_none() && r.fisher.is_none());
    let lines = read_csv(&result_csv(&r, &m));
    assert!(lines
        .iter()
        .all(|f| f[0] != "continuity_corrected" && f[0] != "fisher"));
    assert!(lines.iter().any(|f| f[0] == "pearson" && f[1] == "p"));
}

#[test]
fn neither_file_holds_a_header_and_the_manifest_holds_no_label() {
    let d = dataset(["SENTINEL_HDR_ROWS_8x1", "SENTINEL_HDR_COLS_8x1"], &golden_rows());
    let (r, m) = run_of(&d, params(vec![-9.0]));
    let csv = String::from_utf8(result_csv(&r, &m)).unwrap();
    let json = String::from_utf8(manifest_json(&m)).unwrap();
    // Positive controls: the headers are the dataset's, the CSV holds the
    // labels, and the manifest is the manifest, its hashes in the CSV too.
    assert_eq!(
        d.schema.column(ColumnAlias(1)).map(|c| c.header.as_str()),
        Some("SENTINEL_HDR_ROWS_8x1")
    );
    assert!(csv.contains("\"hi\"") && csv.contains("\"yes\""));
    assert!(csv.contains(m.output_hash.as_hex()) && csv.contains(m.parameters_hash.as_hex()));
    assert!(
        json.contains("\"capabilityId\": \"stats.analyze.crosstabs\"")
            && json.contains("\"outputHash\""),
        "{json}"
    );
    for leak in ["SENTINEL", "HDR", "/", "\\", ".csv", "Users"] {
        assert!(!csv.contains(leak), "the CSV holds {leak:?}:\n{csv}");
        assert!(!json.contains(leak), "the manifest holds {leak:?}:\n{json}");
    }
    for label in ["=lo", "\"hi\"", "\"yes\"", "\"no\""] {
        assert!(!json.contains(label), "the manifest holds {label:?}:\n{json}");
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
        .join("tests/fixtures/stats_xtab")
        .join(name)
}

fn pin_bytes(name: &str) -> Vec<u8> {
    std::fs::read(pin_path(name)).unwrap_or_else(|e| panic!("{name}: {e}"))
}

/// The pin's difference from `got`, if any, so both files are reported.
fn pinned(name: &str, got: &[u8]) -> Option<String> {
    if std::env::var_os("GAPLY_WRITE_XTAB_EXPORT_PIN").is_some() {
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

/// The macOS aarch64 pin: its own platform's, and the layout every other
/// platform is checked against (decision 577).
const MAC_CSV: &str = "export_golden.csv";
const MAC_JSON: &str = "export_golden_manifest.json";

/// How a platform checks the pin: its exact bytes, or the layout check of
/// `pin_support`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PinCheck {
    Exact,
    Layout,
}

/// The check for a platform, by `std::env::consts::OS` and `ARCH`, which
/// name the `target_os` and `target_arch` this test was built for.
fn pin_check(os: &str, arch: &str) -> PinCheck {
    match (os, arch) {
        ("macos", "aarch64") => PinCheck::Exact,
        _ => PinCheck::Layout,
    }
}

/// Run on every platform: exact bytes on macOS aarch64, the layout check
/// everywhere else, Linux x86_64 included until it is measured, never a
/// skip.
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
            panic!(
                "{MAC_CSV}: the layout differs: {e}\n--- got ---\n{}",
                String::from_utf8_lossy(&csv)
            );
        }
        if let Err(e) = json_layout_matches(&json, &pin_bytes(MAC_JSON)) {
            panic!(
                "{MAC_JSON}: the layout differs: {e}\n--- got ---\n{}",
                String::from_utf8_lossy(&json)
            );
        }
    }
}

/// Linux relies on this: the layout check on altered copies of the pin, on
/// every platform.
#[test]
fn the_layout_check_allows_a_few_ulps_and_nothing_else() {
    let pin = pin_bytes(MAC_CSV);
    let text = String::from_utf8(pin.clone()).expect("utf-8");
    let altered = |old: &str, new: &str| -> Vec<u8> {
        assert_eq!(text.matches(old).count(), 1, "{old} once in the pin");
        text.replacen(old, new, 1).into_bytes()
    };
    assert_eq!(csv_layout_matches(&pin, &pin), Ok(()));
    let json_pin = pin_bytes(MAC_JSON);
    assert_eq!(json_layout_matches(&json_pin, &json_pin), Ok(()));
    // Pearson's p, through statrs's chi-square, moved by n units in the
    // last place.
    let line = text
        .split("\r\n")
        .find(|l| l.starts_with("\"pearson\",\"p\","))
        .expect("Pearson's p")
        .to_string();
    let value = line.split(',').nth(6).unwrap().to_string();
    let moved = |n: u64| {
        let v: f64 = value.parse().expect("a number");
        line.replacen(&value, &format!("{}", f64::from_bits(v.to_bits() + n)), 1)
    };
    assert_eq!(csv_layout_matches(&altered(&line, &moved(1)), &pin), Ok(()));
    assert!(
        csv_layout_matches(&altered(&line, &moved(5)), &pin).is_err(),
        "5 ulps off"
    );
    let word = "\"note\",\"\",\"\",\"\",\"\",\"\",,\"stats.xtab.expected_below_five\"";
    assert!(
        csv_layout_matches(
            &altered(word, &word.replace("expected_below_five", "overflow")),
            &pin
        )
        .is_err(),
        "a changed word"
    );
    assert!(
        csv_layout_matches(&altered(&format!("{word}\r\n"), ""), &pin).is_err(),
        "a missing line"
    );
}
