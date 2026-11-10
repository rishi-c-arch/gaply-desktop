//! **The rank tests' exports (Research OS, Phase 8Y-1, decision 604).**
//!
//! * The CSV reads back field by field, in the screen's order: each
//!   included group's count, median, rank sum and mean rank, the included
//!   values and their tie runs, the test's lines, then the rows left out (a
//!   count line and a column line each), the notes, the limitations and the
//!   manifest's hashes. Every number reads back to the same double.
//! * A group is its index and its label; a label that starts with a formula
//!   character gets one leading apostrophe (`stats_desc::export::text`).
//!   The headers never reach the CSV, and the manifest's JSON holds no
//!   header, label or path (sentinels, with positive controls).
//! * A Kruskal-Wallis result has no Mann-Whitney lines, and a value that is
//!   not computable is an empty field with its code.
//! * The bytes are pinned on macOS aarch64 (captured 10 Nov 2026) and, since a
//!   Linux run measured the same bytes (decisions 612, 615 and 618), on Linux
//!   x86_64: a golden CSV and the manifest's JSON for a Mann-Whitney test over
//!   16 rows, with its exact p, one row left out by each of the four reasons
//!   and a group below five values. Every other platform checks that pin's
//!   layout (decision 346's check, `pin_support`, unchanged): the normal p
//!   comes from the platform's maths library through statrs, so another
//!   platform's last digits may differ. It never passes by skipping.
//! * Ten runs give byte-identical CSV and manifest bytes.

use gaply_core::contracts::{Arch, OperationId, Os, Platform, RunManifest};
use gaply_core::stats_desc::Alias;
use gaply_core::stats_l1::dataset::{
    build_dataset, ColumnAlias, DataFingerprint, Dataset, RawTable, TableRef,
};
use gaply_core::stats_ranks::export::{manifest_json, result_csv};
use gaply_core::stats_ranks::{
    compute, manifest, spec, IndependentRanksOutcome, IndependentRanksParams, RankTest, RanksCode,
};

mod pin_support;
use pin_support::{csv_layout_matches, json_layout_matches};

fn dataset(headers: [&str; 2], rows: &[(&str, &str)]) -> Dataset {
    build_dataset(
        RawTable::from_rows(
            headers.iter().map(|h| h.to_string()).collect(),
            rows.iter()
                .map(|(y, g)| vec![y.to_string(), g.to_string()])
                .collect(),
        ),
        DataFingerprint::of_bytes(
            format!("{headers:?}{rows:?}").as_bytes(),
            "ranks export",
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

fn params(levels: Option<Vec<u32>>, codes: Vec<f64>) -> IndependentRanksParams {
    IndependentRanksParams {
        outcome: Alias(ColumnAlias(1)),
        group: Alias(ColumnAlias(2)),
        levels,
        missing_codes: codes,
    }
}

/// The golden's 16 rows: "=lo" (5 values) against "hi" (7), untied, with
/// "zz" left out by the levels ticked and one row left out by each reason.
fn golden_rows() -> Vec<(&'static str, &'static str)> {
    vec![
        ("1.25", "=lo"),
        ("4.5", "=lo"),
        ("2.75", "=lo"),
        ("6", "=lo"),
        ("3.5", "=lo"),
        ("5.25", "hi"),
        ("7.5", "hi"),
        ("8", "hi"),
        ("6.75", "hi"),
        ("9.5", "hi"),
        ("4", "hi"),
        ("10.25", "hi"),
        ("3", ""),
        ("2", "zz"),
        ("", "=lo"),
        ("-9", "hi"),
    ]
}

fn golden_params() -> IndependentRanksParams {
    params(Some(vec![0, 1]), vec![-9.0])
}

fn run_of(
    d: &Dataset,
    p: IndependentRanksParams,
) -> (IndependentRanksOutcome, RunManifest<RanksCode>) {
    let s = spec(OperationId(13), d, p);
    let r = compute(d, &s.parameters).expect("computes");
    let m = manifest(&s, d, &r, 0, platform());
    (r, m)
}

fn golden_case() -> (Vec<u8>, Vec<u8>) {
    let d = dataset(["outcome", "group"], &golden_rows());
    let (r, m) = run_of(&d, golden_params());
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
    let d = dataset(["outcome", "group"], &golden_rows());
    let (r, m) = run_of(&d, golden_params());
    let rows = read_csv(&result_csv(&r, &m));
    assert_eq!(
        rows[0],
        ["table", "statistic", "group_index", "group", "group_index_2", "group_2", "value", "word"]
            .map(String::from)
    );
    assert_eq!(
        rows[1],
        ["groups", "n", "0", "'=lo", "", "", "5", ""].map(String::from)
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
        ["groups", "ranks", "mann_whitney", "excluded", "limitation", "manifest"]
    );
    for (k, g) in r.groups.iter().enumerate() {
        let lines = &rows[1 + 4 * k..1 + 4 * (k + 1)];
        let names: Vec<&str> = lines.iter().map(|l| l[1].as_str()).collect();
        assert_eq!(names, ["n", "median", "rank_sum", "mean_rank"]);
        let values: Vec<f64> = lines.iter().map(|l| l[6].parse().unwrap()).collect();
        assert_eq!(values, [g.n as f64, g.median, g.rank_sum, g.mean_rank]);
        assert!(lines.iter().all(|l| l[2] == g.level.to_string()));
    }
    let RankTest::MannWhitney(t) = r.test else {
        panic!("two groups: the Mann-Whitney test")
    };
    let test: Vec<&[String; 8]> = rows.iter().filter(|f| f[0] == "mann_whitney").collect();
    let names: Vec<&str> = test.iter().map(|l| l[1].as_str()).collect();
    assert_eq!(names, ["u1", "u2", "z", "p", "exact_p", "rank_biserial"]);
    // Each line names the pair: the first group, then the second.
    for l in &test {
        assert_eq!(l[2..6], ["0", "'=lo", "1", "hi"].map(String::from), "{l:?}");
    }
    let number = |i: usize| -> f64 { test[i][6].parse().unwrap() };
    assert_eq!((number(0), number(1)), (t.u1, t.u2));
    assert_eq!(Some(number(4)), t.exact_p);
    assert_eq!(number(5), t.rank_biserial);
    let excluded: Vec<(String, String, String)> = rows
        .iter()
        .filter(|f| f[0] == "excluded")
        .map(|f| (f[1].clone(), f[6].clone(), f[7].clone()))
        .collect();
    let line = |s: &str, v: &str, w: &str| (s.to_string(), v.to_string(), w.to_string());
    assert_eq!(
        excluded,
        [
            line("rows", "1", "stats.ranks.missing_group"),
            line("column", "2", "stats.ranks.missing_group"),
            line("rows", "1", "stats.ranks.other_level"),
            line("column", "2", "stats.ranks.other_level"),
            line("rows", "1", "stats.ranks.missing_outcome"),
            line("column", "1", "stats.ranks.missing_outcome"),
            line("rows", "1", "stats.ranks.missing_code"),
            line("column", "1", "stats.ranks.missing_code"),
        ]
    );
    let notes: Vec<&str> = rows
        .iter()
        .filter(|f| f[0] == "note")
        .map(|f| f[7].as_str())
        .collect();
    assert!(notes.is_empty(), "five and seven values, untied: {notes:?}");
    assert_eq!(rows.iter().filter(|f| f[0] == "limitation").count(), 7);
}

#[test]
fn a_kruskal_wallis_result_has_no_mann_whitney_lines_and_a_not_computable_value_is_its_code() {
    let rows: Vec<(&str, &str)> = ["a", "b", "c"]
        .iter()
        .flat_map(|g| std::iter::repeat(("7", *g)).take(3))
        .collect();
    let d = dataset(["y", "g"], &rows);
    let (r, m) = run_of(&d, params(None, Vec::new()));
    let lines = read_csv(&result_csv(&r, &m));
    assert!(lines.iter().all(|f| f[0] != "mann_whitney"));
    let kw: Vec<(String, String, String)> = lines
        .iter()
        .filter(|f| f[0] == "kruskal_wallis")
        .map(|f| (f[1].clone(), f[6].clone(), f[7].clone()))
        .collect();
    let nc = "stats.ranks.all_values_tied".to_string();
    assert_eq!(
        kw,
        [
            ("h".to_string(), String::new(), nc.clone()),
            ("df".to_string(), "2".to_string(), String::new()),
            ("p".to_string(), String::new(), nc.clone()),
            ("epsilon_squared".to_string(), String::new(), nc),
        ]
    );
    // An absent exact p carries the note that says why.
    let d = dataset(["y", "g"], &[("1", "a"), ("1", "b"), ("2", "a"), ("3", "b")]);
    let (r, m) = run_of(&d, params(None, Vec::new()));
    let lines = read_csv(&result_csv(&r, &m));
    let exact = lines
        .iter()
        .find(|f| f[0] == "mann_whitney" && f[1] == "exact_p")
        .expect("an exact p line");
    assert_eq!((exact[6].as_str(), exact[7].as_str()), ("", "stats.ranks.exact_not_given_ties"));
}

#[test]
fn neither_file_holds_a_header_and_the_manifest_holds_no_label() {
    let d = dataset(["SENTINEL_HDR_OUTCOME_8y1", "SENTINEL_HDR_GROUP_8y1"], &golden_rows());
    let (r, m) = run_of(&d, golden_params());
    let csv = String::from_utf8(result_csv(&r, &m)).unwrap();
    let json = String::from_utf8(manifest_json(&m)).unwrap();
    assert_eq!(
        d.schema.column(ColumnAlias(1)).map(|c| c.header.as_str()),
        Some("SENTINEL_HDR_OUTCOME_8y1")
    );
    assert!(csv.contains("\"hi\"") && csv.contains("\"'=lo\""));
    assert!(csv.contains(m.output_hash.as_hex()) && csv.contains(m.parameters_hash.as_hex()));
    assert!(
        json.contains("\"capabilityId\": \"stats.analyze.independent_ranks\"")
            && json.contains("\"outputHash\""),
        "{json}"
    );
    for leak in ["SENTINEL", "HDR", "/", "\\", ".csv", "Users"] {
        assert!(!csv.contains(leak), "the CSV holds {leak:?}:\n{csv}");
        assert!(!json.contains(leak), "the manifest holds {leak:?}:\n{json}");
    }
    for label in ["=lo", "\"hi\"", "zz"] {
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
        .join("tests/fixtures/stats_ranks")
        .join(name)
}

fn pin_bytes(name: &str) -> Vec<u8> {
    std::fs::read(pin_path(name)).unwrap_or_else(|e| panic!("{name}: {e}"))
}

/// The pin's difference from `got`, if any, so both files are reported.
fn pinned(name: &str, got: &[u8]) -> Option<String> {
    if std::env::var_os("GAPLY_WRITE_RANKS_EXPORT_PIN").is_some() {
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
/// platform is checked against (decision 604).
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
        ("macos", "aarch64") | ("linux", "x86_64") => PinCheck::Exact,
        _ => PinCheck::Layout,
    }
}

/// Decisions 604 and 618, run on every platform: exact bytes on macOS aarch64
/// and Linux x86_64, the layout check everywhere else, never a skip, as
/// crosstabs' pin is.
#[test]
fn the_pin_is_checked_exactly_on_macos_aarch64_and_linux_x86_64() {
    for (os, arch, want) in [
        ("macos", "aarch64", PinCheck::Exact),
        ("linux", "x86_64", PinCheck::Exact),
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

/// Every platform but macOS aarch64 relies on this: the layout check on
/// altered copies of the pin, run on every platform.
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
    // The normal approximation's p, through statrs's erfc, moved by n units
    // in the last place.
    let line = text
        .split("\r\n")
        .find(|l| l.starts_with("\"mann_whitney\",\"p\","))
        .expect("the Mann-Whitney p")
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
    let word = "\"limitation\",\"\",\"\",\"\",\"\",\"\",,\"stats.ranks.no_weights\"";
    assert!(
        csv_layout_matches(
            &altered(word, &word.replace("no_weights", "overflow")),
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

/// Decision 618: values near the largest double, through the whole method and
/// its export: no infinity, no NaN and no null in either file.
#[test]
fn values_near_the_largest_double_export_no_inf_no_null_and_no_nan() {
    let d = dataset(
        ["outcome", "group"],
        &[("1e308", "a"), ("1.5e308", "a"), ("1", "b"), ("2", "b")],
    );
    let params = IndependentRanksParams {
        outcome: Alias(ColumnAlias(1)),
        group: Alias(ColumnAlias(2)),
        levels: None,
        missing_codes: vec![],
    };
    let result = compute(&d, &params).unwrap();
    let s = spec(OperationId(1), &d, params);
    let m = manifest(&s, &d, &result, 1, platform());
    let csv = String::from_utf8(result_csv(&result, &m)).unwrap();
    let json = String::from_utf8(manifest_json(&m)).unwrap();
    // The manifest's one null is its `seed`, as the pin's is.
    let json = json.replace("\"seed\": null", "\"seed\": 0");
    for (name, text) in [("csv", &csv), ("manifest", &json)] {
        let lower = text.to_lowercase();
        for bad in ["inf", "nan", "null"] {
            // `limitation` words and hex hashes aside, a bad token stands alone
            // between delimiters.
            let hit = lower
                .split(|c: char| !c.is_ascii_alphanumeric())
                .any(|t| t == bad);
            assert!(!hit, "{name} holds {bad}:\n{text}");
        }
    }
    assert!(csv.contains("\"groups\",\"median\""), "{csv}");
    assert!(csv.contains(",125"), "the median written out: {csv}");
}
