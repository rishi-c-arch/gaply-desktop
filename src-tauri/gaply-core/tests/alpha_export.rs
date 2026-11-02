//! **Cronbach's alpha's exports (Research OS, Phase 8I-2, decision 379).**
//!
//! * The CSV reads back field by field, in the screen's order (the
//!   confidence level, k, the rows read and used, alpha with its interval,
//!   standardized alpha and the inter-item correlations, then each item's
//!   block), through an RFC 4180 reader that lives HERE, not in shipped code.
//!   Every number reads back to the same double.
//! * Anything not computable is an empty value with its closed code; the
//!   reversed flag is 1 or 0; the negative-alpha note is its own record.
//! * Columns are named by alias (`C1`, `C2`, …) and their index among the
//!   items, never by header, so a stored result and a live one export the
//!   same bytes; the scale's ends are not in the CSV; the manifest's JSON
//!   holds no header, cell or path (sentinels, with positive controls).
//! * The bytes are pinned on macOS aarch64 (captured 2 Nov 2026): a golden
//!   CSV and the manifest's JSON for four items over twelve rows, one
//!   reversed on a scale of 1 to 5, a ticked missing code, one row left out
//!   by an empty cell and one by the code, and one item constant over the
//!   rows used. Linux x86_64 checks the same bytes exactly: measured outside
//!   this Mac at d5fd559, its bytes are macOS aarch64's in both files
//!   (decisions 389 and 400). Every other platform checks that pin's layout
//!   (decision 346's check, `pin_support`, unchanged): Feldt's F comes from
//!   `stats_dist::f_upper` through the platform's maths library, so another
//!   platform's last digits may differ. It never passes by skipping, the
//!   choice is `pin_check` with a test of its own, and the layout check has
//!   a test of its own on this pin, both run on every platform.
//! * Ten runs give byte-identical CSV and manifest bytes.

use gaply_core::contracts::codes::Closed;
use gaply_core::contracts::{Arch, OperationId, Os, ParamKey, ParamValue, Platform, RunManifest};
use gaply_core::stats_alpha::export::{manifest_json, result_csv};
use gaply_core::stats_alpha::{
    compute, manifest, spec, AlphaCode, AlphaOutcome, AlphaParams, Interval, ScaleEnds, Stat,
};
use gaply_core::stats_desc::Alias;
use gaply_core::stats_l1::dataset::{
    build_dataset, ColumnAlias, DataFingerprint, Dataset, RawTable, TableRef,
};

mod pin_support;
use pin_support::{csv_layout_matches, json_layout_matches};

fn dataset(headers: &[&str], rows: &[&[&str]]) -> Dataset {
    build_dataset(
        RawTable::from_rows(
            headers.iter().map(|h| h.to_string()).collect(),
            rows.iter()
                .map(|r| r.iter().map(|c| c.to_string()).collect())
                .collect(),
        ),
        DataFingerprint::of_bytes(
            format!("{headers:?}{rows:?}").as_bytes(),
            "alpha export",
            TableRef::FirstTable,
        ),
    )
    .expect("dataset")
}

fn aliases(items: &[u32]) -> Vec<Alias> {
    items.iter().map(|v| Alias(ColumnAlias(*v))).collect()
}

/// The golden's settings: every item, the second reversed on 1 to 5, -99
/// ticked.
fn golden_params() -> AlphaParams {
    AlphaParams {
        items: aliases(&[1, 2, 3, 4]),
        reversed: aliases(&[2]),
        scale: Some(ScaleEnds { min: 1.0, max: 5.0 }),
        missing_codes: vec![-99.0],
    }
}

fn platform() -> Platform {
    Platform {
        os: Os::Macos,
        arch: Arch::Aarch64,
    }
}

/// Four items, twelve rows. With -99 ticked, rows 1 to 10 are used (n =
/// 10): the second item, reversed on 1 to 5, runs with the first and the
/// third, and the fourth is 3 in every row, so standardized alpha, r̄ and its
/// range, and the fourth's corrected item-total correlation are not
/// computable. Row 11 is left out under the second (an empty cell) and row
/// 12 under the first (the code). Alpha is about 0.87, so both of Feldt's
/// ends lie above 0.5.
fn golden_data(headers: [&str; 4]) -> Dataset {
    dataset(
        &headers,
        &[
            &["1", "5", "1", "3"],
            &["2", "4", "1", "3"],
            &["2", "5", "2", "3"],
            &["3", "3", "3", "3"],
            &["3", "2", "4", "3"],
            &["4", "2", "4", "3"],
            &["4", "1", "5", "3"],
            &["5", "1", "5", "3"],
            &["5", "2", "4", "3"],
            &["1", "4", "2", "3"],
            &["3", "", "3", "3"],
            &["-99", "3", "3", "3"],
        ],
    )
}

fn run_of(d: &Dataset, p: AlphaParams) -> (AlphaOutcome, RunManifest<AlphaCode>) {
    let s = spec(OperationId(12), d, p);
    let r = compute(d, &s.parameters).expect("computes");
    let m = manifest(&s, d, &r, 0, platform());
    (r, m)
}

/// The golden: its bytes as captured.
fn golden_case() -> (Vec<u8>, Vec<u8>) {
    let d = golden_data(["x", "y", "z", "w"]);
    let (r, m) = run_of(&d, golden_params());
    (result_csv(&r, &m), manifest_json(&m))
}

/// RFC 4180: CRLF between records, `"` around a field, `""` inside one. A
/// quoted field's text with its quotes removed; the aliases and codes here
/// never start with a formula character, so no apostrophe is expected.
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
                        chars.next();
                        field.push('"');
                    }
                    '"' => quoted = !quoted,
                    ',' if !quoted => fields.push(std::mem::take(&mut field)),
                    c => field.push(c),
                }
            }
            fields.push(field);
            fields.try_into().expect("eight fields")
        })
        .collect()
}

/// What each record must hold, built from the result's and the manifest's
/// own fields, in the order decision 379 gives.
fn expected(r: &AlphaOutcome, m: &RunManifest<AlphaCode>) -> Vec<[String; 8]> {
    let f = |v: f64| format!("{v}");
    let rec = |t: &str, s: &str, at: Option<(usize, Alias)>, v: String, w: &str| {
        let (i, a) = at
            .map(|(i, a)| (i.to_string(), format!("C{}", a.0 .0)))
            .unwrap_or_default();
        [
            t.into(),
            s.into(),
            i,
            a,
            String::new(),
            String::new(),
            v,
            w.into(),
        ]
    };
    let stat = |s: Stat| match s {
        Stat::Value(v) => (f(v), ""),
        Stat::NotComputable(c) => (String::new(), c.wire()),
    };
    let mut out = vec![
        [
            "table",
            "statistic",
            "first_index",
            "first",
            "second_index",
            "second",
            "value",
            "word",
        ]
        .map(String::from),
        rec("result", "confidence_level", None, f(0.95), ""),
        rec("result", "k", None, r.k.to_string(), ""),
        rec("result", "rows_read", None, m.rows_read.to_string(), ""),
        rec("result", "rows_used", None, r.rows_used.to_string(), ""),
    ];
    let (v, w) = stat(r.alpha);
    out.push(rec("alpha", "alpha", None, v, w));
    for (end, k) in [("ci_lower", 0), ("ci_upper", 1)] {
        out.push(match r.interval {
            Interval::Computed { lower, upper } => {
                rec("alpha", end, None, f([lower, upper][k]), "")
            }
            Interval::NotComputable(c) => rec("alpha", end, None, String::new(), c.wire()),
        });
    }
    for (name, s) in [
        ("standardized", r.standardized),
        ("inter_item_mean", r.inter_item.mean),
        ("inter_item_min", r.inter_item.min),
        ("inter_item_max", r.inter_item.max),
    ] {
        let (v, w) = stat(s);
        out.push(rec("alpha", name, None, v, w));
    }
    for (i, item) in r.items.iter().enumerate() {
        let at = Some((i, item.item));
        let flag = if item.reversed { "1" } else { "0" };
        out.push(rec("items", "reversed", at, flag.into(), ""));
        for (name, s) in [
            ("mean", item.mean),
            ("sd", item.sd),
            ("scale_mean_if_deleted", item.scale_mean_if_deleted),
            ("scale_variance_if_deleted", item.scale_variance_if_deleted),
            ("corrected_item_total", item.corrected_item_total),
            ("alpha_if_deleted", item.alpha_if_deleted),
        ] {
            let (v, w) = stat(s);
            out.push(rec("items", name, at, v, w));
        }
    }
    for e in &r.exclusions {
        let at = match e.params.get(&ParamKey::Column) {
            Some(ParamValue::Int(n)) => r
                .items
                .iter()
                .position(|it| i64::from(it.item.0 .0) == *n)
                .map(|i| (i, r.items[i].item)),
            _ => None,
        };
        out.push(rec(
            "excluded",
            "rows",
            at,
            e.count.to_string(),
            e.code.wire(),
        ));
    }
    for c in &r.notes {
        out.push(rec("note", "", None, String::new(), c.wire()));
    }
    for c in &r.limitations {
        out.push(rec("limitation", "", None, String::new(), c.wire()));
    }
    for h in &m.inputs {
        out.push(rec(
            "manifest",
            "input_sha256",
            None,
            String::new(),
            h.as_hex(),
        ));
    }
    out.push(rec(
        "manifest",
        "parameters_sha256",
        None,
        String::new(),
        m.parameters_hash.as_hex(),
    ));
    out.push(rec(
        "manifest",
        "output_sha256",
        None,
        String::new(),
        m.output_hash.as_hex(),
    ));
    out
}

/// The CSV of `p` on `d`, read back and checked record by record; every
/// number reads back to the same double.
fn check_reads_back(d: &Dataset, p: AlphaParams) -> (AlphaOutcome, String) {
    let (r, m) = run_of(d, p);
    let bytes = result_csv(&r, &m);
    let got = read_csv(&bytes);
    let want = expected(&r, &m);
    assert_eq!(got.len(), want.len(), "records");
    for (i, (g, w)) in got.iter().zip(&want).enumerate() {
        assert_eq!(g, w);
        let counted = w[0] == "result" && w[1] != "confidence_level";
        let flag = w[1] == "reversed";
        if i > 0 && !w[6].is_empty() && !counted && !flag && w[0] != "excluded" {
            let back: f64 = g[6].parse().expect("a number");
            assert_eq!(back.to_bits(), w[6].parse::<f64>().unwrap().to_bits());
        }
    }
    (r, String::from_utf8(bytes).expect("utf-8"))
}

#[test]
fn the_csv_reads_back_in_the_screens_order_with_every_digit() {
    let d = golden_data(["x", "y", "z", "w"]);
    let (r, csv) = check_reads_back(&d, golden_params());
    assert_eq!((r.k, r.rows_used), (4, 10));
    let tables: Vec<&str> = csv
        .split("\r\n")
        .skip(1)
        .filter(|l| !l.is_empty())
        .map(|l| l.split(',').next().unwrap())
        .fold(Vec::new(), |mut v, t| {
            if v.last() != Some(&t) {
                v.push(t);
            }
            v
        });
    assert_eq!(
        tables,
        [
            "\"result\"",
            "\"alpha\"",
            "\"items\"",
            "\"excluded\"",
            "\"limitation\"",
            "\"manifest\""
        ]
    );
    // Each item's block in the items' order, seven statistics each.
    let items: Vec<String> = csv
        .split("\r\n")
        .filter(|l| l.starts_with("\"items\""))
        .map(|l| l.split(',').take(4).collect::<Vec<_>>().join(","))
        .collect();
    assert_eq!(items.len(), 4 * 7);
    for (i, block) in items.chunks(7).enumerate() {
        assert!(
            block
                .iter()
                .all(|l| l.ends_with(&format!("\"{i}\",\"C{}\"", i + 1))),
            "{block:?}"
        );
    }
    // Nothing reversed, no scale: every flag 0, and the same order.
    let plain = AlphaParams {
        items: aliases(&[1, 3]),
        reversed: Vec::new(),
        scale: None,
        missing_codes: vec![-99.0],
    };
    let (r, csv) = check_reads_back(&d, plain);
    assert_eq!(r.k, 2);
    assert!(csv.contains("\"items\",\"reversed\",\"0\",\"C1\",\"\",\"\",0,\"\""));
    assert!(csv.contains("\"items\",\"reversed\",\"1\",\"C3\",\"\",\"\",0,\"\""));
}

#[test]
fn what_is_not_computable_is_its_code_and_the_reversed_flag_is_one_or_zero() {
    let (csv, _) = golden_case();
    let csv = String::from_utf8(csv).unwrap();
    for want in [
        "\"result\",\"k\",\"\",\"\",\"\",\"\",4,\"\"",
        "\"result\",\"rows_read\",\"\",\"\",\"\",\"\",12,\"\"",
        "\"result\",\"rows_used\",\"\",\"\",\"\",\"\",10,\"\"",
        "\"alpha\",\"standardized\",\"\",\"\",\"\",\"\",,\"stats.alpha.item_constant\"",
        "\"alpha\",\"inter_item_mean\",\"\",\"\",\"\",\"\",,\"stats.alpha.item_constant\"",
        "\"alpha\",\"inter_item_min\",\"\",\"\",\"\",\"\",,\"stats.alpha.item_constant\"",
        "\"alpha\",\"inter_item_max\",\"\",\"\",\"\",\"\",,\"stats.alpha.item_constant\"",
        "\"items\",\"reversed\",\"0\",\"C1\",\"\",\"\",0,\"\"",
        "\"items\",\"reversed\",\"1\",\"C2\",\"\",\"\",1,\"\"",
        "\"items\",\"corrected_item_total\",\"3\",\"C4\",\"\",\"\",,\"stats.alpha.item_constant\"",
        "\"items\",\"sd\",\"3\",\"C4\",\"\",\"\",0,\"\"",
        "\"excluded\",\"rows\",\"0\",\"C1\",\"\",\"\",1,\"stats.alpha.missing_code\"",
        "\"excluded\",\"rows\",\"1\",\"C2\",\"\",\"\",1,\"stats.alpha.missing_value\"",
        "\"limitation\",\"\",\"\",\"\",\"\",\"\",,\"stats.alpha.assumes_one_dimension\"",
        "\"limitation\",\"\",\"\",\"\",\"\",\"\",,\"stats.alpha.no_omega\"",
    ] {
        assert!(csv.contains(want), "{want} missing:\n{csv}");
    }
    assert!(!csv.contains("\"note\""), "no note on a positive alpha");
    // Both of Feldt's ends lie above 0.5 (decision 379's golden case).
    let end = |name: &str| -> f64 {
        let line = csv
            .split("\r\n")
            .find(|l| l.starts_with(&format!("\"alpha\",\"{name}\",")))
            .expect(name);
        line.split(',').nth(6).unwrap().parse().expect("a number")
    };
    assert!(
        end("ci_lower") > 0.5 && end("ci_upper") > end("ci_lower"),
        "{csv}"
    );
    let lower = csv.to_lowercase();
    for bad in ["inf", "nan", "null"] {
        assert!(!lower.contains(bad), "{bad} in the CSV:\n{csv}");
    }
}

/// Decision 379: the negative-alpha note's record, on its own, not pinned.
#[test]
fn a_negative_alpha_has_its_note_record() {
    let d = dataset(
        &["a", "b"],
        &[
            &["1", "4"],
            &["2", "5"],
            &["3", "1"],
            &["4", "3"],
            &["5", "2"],
        ],
    );
    let p = AlphaParams {
        items: aliases(&[1, 2]),
        reversed: Vec::new(),
        scale: None,
        missing_codes: Vec::new(),
    };
    let (r, csv) = check_reads_back(&d, p);
    assert!(
        matches!(r.alpha, Stat::Value(v) if v < 0.0),
        "{:?}",
        r.alpha
    );
    assert_eq!(r.notes, [AlphaCode::NegativeAlpha]);
    let notes: Vec<&str> = csv
        .split("\r\n")
        .filter(|l| l.starts_with("\"note\""))
        .collect();
    assert_eq!(
        notes,
        ["\"note\",\"\",\"\",\"\",\"\",\"\",,\"stats.alpha.negative_alpha\""]
    );
    // Before the limitations.
    let at = |s: &str| csv.find(s).expect(s);
    assert!(at("\"note\"") < at("\"limitation\""));
    assert!(csv.contains(
        "\"items\",\"alpha_if_deleted\",\"0\",\"C1\",\"\",\"\",,\"stats.alpha.two_items\""
    ));
}

#[test]
fn the_csv_and_the_manifest_name_columns_by_alias_and_hold_no_header_cell_or_path() {
    let d = golden_data([
        "SENTINEL_HDR_8i2a",
        "SENTINEL_HDR_8i2b",
        "zz_sentinel_c",
        "=SUM(A1)",
    ]);
    let (r, m) = run_of(&d, golden_params());
    let csv = String::from_utf8(result_csv(&r, &m)).unwrap();
    let json = String::from_utf8(manifest_json(&m)).unwrap();
    // Positive controls: the headers are the dataset's, the CSV names the
    // aliases, and the manifest is the manifest, its hashes in the CSV too.
    assert_eq!(
        d.schema.column(ColumnAlias(1)).map(|c| c.header.as_str()),
        Some("SENTINEL_HDR_8i2a")
    );
    assert!(csv.contains("\"C1\"") && csv.contains("\"C4\""));
    assert!(csv.contains(m.output_hash.as_hex()) && csv.contains(m.parameters_hash.as_hex()));
    assert!(
        json.contains("\"capabilityId\": \"stats.analyze.cronbach_alpha\"")
            && json.contains("\"outputHash\""),
        "{json}"
    );
    for leak in [
        "SENTINEL", "sentinel", "zz_", "SUM", "/", "\\", ".csv", "Users",
    ] {
        assert!(!csv.contains(leak), "the CSV holds {leak:?}:\n{csv}");
        assert!(!json.contains(leak), "the manifest holds {leak:?}:\n{json}");
    }
    // The same run on plain headers exports the same bytes: the headers
    // never reach them (the fingerprint differs, so the input hash line is
    // set aside).
    let plain = golden_data(["x", "y", "z", "w"]);
    let (r2, m2) = run_of(&plain, golden_params());
    let strip = |s: String| -> String {
        s.split("\r\n")
            .filter(|l| !l.contains("input_sha256"))
            .collect::<Vec<_>>()
            .join("\r\n")
    };
    assert_eq!(r, r2);
    assert_eq!(
        strip(csv),
        strip(String::from_utf8(result_csv(&r2, &m2)).unwrap())
    );
}

fn pin_path(name: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/stats_alpha")
        .join(name)
}

fn pin_bytes(name: &str) -> Vec<u8> {
    std::fs::read(pin_path(name)).unwrap_or_else(|e| panic!("{name}: {e}"))
}

/// The pin's difference from `got`, if any, so both files are reported.
fn pinned(name: &str, got: &[u8]) -> Option<String> {
    if std::env::var_os("GAPLY_WRITE_ALPHA_EXPORT_PIN").is_some() {
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
/// platform is checked against (decision 379).
const MAC_CSV: &str = "export_golden.csv";
const MAC_JSON: &str = "export_golden_manifest.json";

/// How a platform checks the pin: its exact bytes, or the layout check of
/// `pin_support` (decision 379).
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

/// Decisions 379 and 400, run on every platform: exact bytes on macOS aarch64
/// and on Linux x86_64 (measured outside this Mac at d5fd559: the same bytes,
/// decision 389), the layout check everywhere else, never a skip.
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

/// Every platform but macOS aarch64 relies on this (decision 379): the
/// layout check on altered copies of alpha's pin, on every platform.
#[test]
fn the_layout_check_allows_a_few_ulps_and_nothing_else() {
    let pin = pin_bytes(MAC_CSV);
    let text = String::from_utf8(pin.clone()).expect("utf-8");
    let altered = |old: &str, new: &str| -> Vec<u8> {
        assert_eq!(text.matches(old).count(), 1, "{old} once in the pin");
        text.replacen(old, new, 1).into_bytes()
    };
    // Positive controls: each pin against itself.
    assert_eq!(csv_layout_matches(&pin, &pin), Ok(()));
    let json_pin = pin_bytes(MAC_JSON);
    assert_eq!(json_layout_matches(&json_pin, &json_pin), Ok(()));
    // Alpha's lower end, a number through Feldt's F, moved by n units in
    // the last place.
    let line = text
        .split("\r\n")
        .find(|l| l.starts_with("\"alpha\",\"ci_lower\","))
        .expect("alpha's lower end")
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
    // A changed word: standardized alpha's code.
    let word = "\"alpha\",\"standardized\",\"\",\"\",\"\",\"\",,\"stats.alpha.item_constant\"";
    assert!(
        csv_layout_matches(
            &altered(word, &word.replace("item_constant", "overflow")),
            &pin
        )
        .is_err(),
        "a changed word"
    );
    // A missing line: that same line.
    assert!(
        csv_layout_matches(&altered(&format!("{word}\r\n"), ""), &pin).is_err(),
        "a missing line"
    );
}

#[test]
fn ten_runs_give_the_same_csv_and_manifest_bytes() {
    let mut out = Vec::new();
    for _ in 0..10 {
        out.push(golden_case());
    }
    assert!(out.windows(2).all(|w| w[0] == w[1]), "bytes differ");
    assert_eq!(out.len(), 10);
}
