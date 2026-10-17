//! **The descriptive summary's exports (Research OS, Phase 2A).**
//!
//! * The CSV reads back field by field: an RFC 4180 reader that lives HERE,
//!   not in shipped code, strips exactly one leading apostrophe from a
//!   quoted field that has one, and every header, label, statistic name,
//!   number and code comes back exactly, including labels that start with
//!   `=`, `+`, `-`, `@`, an apostrophe, a tab or a carriage return, and ones
//!   holding quotes, commas and line breaks.
//! * Every field a spreadsheet could read as a formula starts with the
//!   guard apostrophe in the bytes.
//! * The manifest's JSON holds no header, label, cell or path (sentinels).
//! * `the_export_r_reads_back` prints one export and its exact values for
//!   the by-hand check with `read.csv` (recorded in the Phase 2A report).

use gaply_core::contracts::codes::Closed;
use gaply_core::contracts::{Arch, OperationId, Os, Platform};
use gaply_core::stats_desc::export::{manifest_json, result_csv};
use gaply_core::stats_desc::{
    describe, manifest, spec, Alias, ColumnRequest, ColumnResult, DescribeParams, DescribeResult,
    Stat, TreatAs,
};
use gaply_core::stats_l1::dataset::{
    build_dataset, ColumnAlias, DataFingerprint, Dataset, RawTable, TableRef,
};

fn table(headers: &[&str], rows: &[&[&str]]) -> Dataset {
    let raw = RawTable::from_rows(
        headers.iter().map(|h| h.to_string()).collect(),
        rows.iter()
            .map(|r| r.iter().map(|c| c.to_string()).collect())
            .collect(),
    );
    build_dataset(
        raw,
        DataFingerprint::of_bytes(
            format!("{headers:?}{rows:?}").as_bytes(),
            "test",
            TableRef::FirstTable,
        ),
    )
    .expect("dataset")
}

/// One field as read: its text, and whether it was quoted.
#[derive(Debug, Clone, PartialEq)]
struct Field {
    quoted: bool,
    text: String,
}

/// RFC 4180: CRLF between records, `"` around a field, `""` inside one.
/// Then, for a quoted field, exactly one leading apostrophe is removed.
fn read_csv(bytes: &[u8]) -> Vec<Vec<Field>> {
    let s = std::str::from_utf8(bytes).expect("utf-8");
    let mut records = Vec::new();
    let mut record = Vec::new();
    let mut chars = s.chars().peekable();
    loop {
        let mut field = Field {
            quoted: false,
            text: String::new(),
        };
        if chars.peek() == Some(&'"') {
            field.quoted = true;
            chars.next();
            loop {
                match chars.next().expect("a closing quote") {
                    '"' if chars.peek() == Some(&'"') => {
                        chars.next();
                        field.text.push('"');
                    }
                    '"' => break,
                    c => field.text.push(c),
                }
            }
        } else {
            while let Some(&c) = chars.peek() {
                if c == ',' || c == '\r' {
                    break;
                }
                field.text.push(c);
                chars.next();
            }
        }
        if field.quoted && field.text.starts_with('\'') {
            field.text.remove(0);
        }
        record.push(field);
        match chars.next() {
            Some(',') => continue,
            Some('\r') => {
                assert_eq!(chars.next(), Some('\n'), "a record ends with CRLF");
                records.push(std::mem::take(&mut record));
                if chars.peek().is_none() {
                    return records;
                }
            }
            None => panic!("the last record has no CRLF"),
            Some(c) => panic!("unexpected {c:?} after a field"),
        }
    }
}

/// What each record of the export must hold, built from the result.
fn expected(result: &DescribeResult) -> Vec<[String; 7]> {
    let mut out = vec![[
        "alias",
        "column",
        "kind",
        "statistic",
        "level",
        "value",
        "note",
    ]
    .map(String::from)];
    let mut push = |a: &str, c: &str, k: &str, s: &str, l: &str, v: String, n: &str| {
        out.push([
            a.into(),
            c.into(),
            k.into(),
            s.into(),
            l.into(),
            v,
            n.into(),
        ]);
    };
    for column in &result.columns {
        match column {
            ColumnResult::Numeric(s) => {
                let a = s.alias.0.name();
                push(&a, &s.header, "numeric", "n", "", s.n.to_string(), "");
                push(
                    &a,
                    &s.header,
                    "numeric",
                    "missing",
                    "",
                    s.missing.to_string(),
                    "",
                );
                for (name, stat) in [
                    ("mean", s.mean),
                    ("sd", s.sd),
                    ("min", s.min),
                    ("q1", s.q1),
                    ("median", s.median),
                    ("q3", s.q3),
                    ("max", s.max),
                ] {
                    match stat {
                        Stat::Value(v) => {
                            push(&a, &s.header, "numeric", name, "", format!("{v}"), "")
                        }
                        Stat::NotComputable(code) => push(
                            &a,
                            &s.header,
                            "numeric",
                            name,
                            "",
                            String::new(),
                            code.wire(),
                        ),
                    }
                }
            }
            ColumnResult::Categorical(s) => {
                let a = s.alias.0.name();
                push(&a, &s.header, "categorical", "n", "", s.n.to_string(), "");
                push(
                    &a,
                    &s.header,
                    "categorical",
                    "missing",
                    "",
                    s.missing.to_string(),
                    "",
                );
                for l in &s.levels {
                    push(
                        &a,
                        &s.header,
                        "categorical",
                        "count",
                        &l.label,
                        l.count.to_string(),
                        "",
                    );
                }
            }
        }
    }
    out
}

const TRICKY: [&str; 10] = [
    "=SUM(A1:A9)",
    "+cmd",
    "-2",
    "@cmd",
    "'quoted",
    "\tTab",
    "\rCR",
    "has \"quotes\", and a comma",
    "two\r\nlines",
    "plain",
];

fn tricky_dataset() -> Dataset {
    // Headers and level labels that a spreadsheet would read as formulas.
    let rows: Vec<Vec<&str>> = TRICKY
        .iter()
        .enumerate()
        .map(|(i, label)| vec![["1.5", "2.25", "1e9", "", "-3"][i % 5], *label])
        .collect();
    let rows: Vec<&[&str]> = rows.iter().map(Vec::as_slice).collect();
    table(&["=HYPERLINK(\"x\")", "-label"], &rows)
}

fn tricky_result(d: &Dataset) -> DescribeResult {
    let params = DescribeParams {
        columns: vec![
            ColumnRequest {
                alias: Alias(ColumnAlias(1)),
                treat_as: TreatAs::Numeric,
            },
            ColumnRequest {
                alias: Alias(ColumnAlias(2)),
                treat_as: TreatAs::Categorical,
            },
        ],
        missing_codes: vec![],
    };
    describe(d, &params).expect("describes")
}

#[test]
fn the_csv_reads_back_field_by_field() {
    let d = tricky_dataset();
    let result = tricky_result(&d);
    let records = read_csv(&result_csv(&result));
    let want = expected(&result);
    assert_eq!(records.len(), want.len());
    for (got, want) in records.iter().zip(&want) {
        assert_eq!(got.len(), 7);
        for (i, (g, w)) in got.iter().zip(want).enumerate() {
            assert_eq!(g.text, *w, "field {i}");
            assert_eq!(
                g.quoted,
                i != 5 || want[0] == "alias",
                "only the value field is unquoted (header row aside)"
            );
            if i == 5 && !g.text.is_empty() && want[0] != "alias" {
                let v: f64 = g.text.parse().expect("a number");
                assert_eq!(
                    format!("{v}"),
                    *w,
                    "the number reads back to the same double"
                );
            }
        }
    }
    // Every tricky label came through, as a level, verbatim (the dataset
    // trims cells, so the tab and CR labels read back trimmed: that is the
    // dataset's rule, and the export is exact about what it was given).
    let ColumnResult::Categorical(c) = &result.columns[1] else {
        panic!("categorical")
    };
    for label in TRICKY {
        assert!(
            c.levels.iter().any(|l| l.label == label.trim()),
            "{label:?}"
        );
    }
    let header = &records[1][1].text;
    assert_eq!(header, "=HYPERLINK(\"x\")");
}

#[test]
fn every_field_a_spreadsheet_could_run_carries_the_guard_apostrophe() {
    let d = tricky_dataset();
    let csv = String::from_utf8(result_csv(&tricky_result(&d))).expect("utf-8");
    assert!(csv.contains("\"'=HYPERLINK(\"\"x\"\")\""), "the header");
    assert!(csv.contains("\"'-label\""), "a header starting with -");
    for guarded in [
        "\"'=SUM(A1:A9)\"",
        "\"'+cmd\"",
        "\"'-2\"",
        "\"'@cmd\"",
        "\"''quoted\"",
    ] {
        assert!(csv.contains(guarded), "{guarded} missing from:\n{csv}");
    }
    assert!(
        csv.contains("\"plain\""),
        "an ordinary label is not changed"
    );
    assert!(
        !csv.contains("\"=") && !csv.contains("\"+") && !csv.contains("\"@"),
        "no quoted field starts with a formula character"
    );
    assert!(csv
        .lines()
        .all(|l| !l.starts_with('=') && !l.starts_with('+') && !l.starts_with('@')));
    // The guard covers a tab and a carriage return at a field's start too.
    let raw = table(&["x"], &[&["a"]]);
    let mut r = describe(
        &raw,
        &DescribeParams {
            columns: vec![ColumnRequest {
                alias: Alias(ColumnAlias(1)),
                treat_as: TreatAs::Categorical,
            }],
            missing_codes: vec![],
        },
    )
    .expect("describes");
    let ColumnResult::Categorical(c) = &mut r.columns[0] else {
        panic!("categorical")
    };
    c.levels[0].label = "\tA".into();
    c.header = "\rB".into();
    let csv = String::from_utf8(result_csv(&r)).expect("utf-8");
    assert!(
        csv.contains("\"'\tA\"") && csv.contains("\"'\rB\""),
        "{csv:?}"
    );
    let back = read_csv(csv.as_bytes());
    assert_eq!(
        (back[3][4].text.as_str(), back[3][1].text.as_str()),
        ("\tA", "\rB")
    );
}

#[test]
fn the_manifest_json_holds_no_header_label_cell_or_path() {
    let d = table(
        &["SENTINEL_HDR_4b1e", "group"],
        &[
            &["98765.4321", "SENTINEL_LBL_77aa"],
            &["1.5", "b"],
            &["", "SENTINEL_LBL_77aa"],
        ],
    );
    let params = DescribeParams {
        columns: vec![
            ColumnRequest {
                alias: Alias(ColumnAlias(1)),
                treat_as: TreatAs::Numeric,
            },
            ColumnRequest {
                alias: Alias(ColumnAlias(2)),
                treat_as: TreatAs::Categorical,
            },
        ],
        missing_codes: vec![],
    };
    let s = spec(OperationId(1), &d, params);
    let r = describe(&d, &s.parameters).expect("describes");
    let m = manifest(
        &s,
        &d,
        &r,
        4,
        Platform {
            os: Os::Macos,
            arch: Arch::Aarch64,
        },
    );
    let json = String::from_utf8(manifest_json(&m)).expect("utf-8");
    // Positive controls: the sentinels are in the result, and the manifest
    // is the manifest.
    let csv = String::from_utf8(result_csv(&r)).expect("utf-8");
    assert!(
        csv.contains("SENTINEL_HDR_4b1e")
            && csv.contains("SENTINEL_LBL_77aa")
            && csv.contains("98765.4321")
    );
    assert!(
        json.contains("\"capabilityId\": \"stats.describe\"") && json.contains("\"outputHash\""),
        "{json}"
    );
    for leak in ["SENTINEL", "98765", "group", "/", "\\", ".csv", "Users"] {
        assert!(!json.contains(leak), "the manifest holds {leak:?}:\n{json}");
    }
    let params_json = serde_json::to_string(&s.parameters).expect("json");
    assert!(
        !params_json.contains("SENTINEL") && !params_json.contains("group"),
        "{params_json}"
    );
    assert!(
        !format!("{m:?}").contains("SENTINEL") && !format!("{r:?}").contains("SENTINEL"),
        "Debug prints counts only"
    );
}

/// Prints one export, and each of its numbers to 17 significant digits and
/// as its IEEE 754 bit pattern, between markers, for the by-hand read-back
/// with R (compare the bits: R's own decimal parser is not correctly
/// rounded, and misread `9.9999999690625000e8` by one unit in the last
/// place when this was measured):
///
/// ```text
/// cargo test -p gaply_core --test desc_export the_export_r_reads_back -- --nocapture
/// ```
#[test]
fn the_export_r_reads_back() {
    let d = table(
        &["=score", "group"],
        &[
            &["1000000012.3456", "=a"],
            &["999999987.25", "-b"],
            &["1000000003.5", "@c"],
            &["", "'d"],
            &["1000000000.125", "=a"],
        ],
    );
    let result = tricky_result(&d);
    let csv = String::from_utf8(result_csv(&result)).expect("utf-8");
    println!("----- BEGIN EXPORT -----\n{csv}----- END EXPORT -----");
    println!("----- BEGIN EXPECTED -----");
    for (i, row) in expected(&result).iter().enumerate().skip(1) {
        if let Ok(v) = row[5].parse::<f64>() {
            println!("{i},{v:.16e},{:016x}", v.to_bits());
        }
    }
    println!("----- END EXPECTED -----");
    assert!(csv.starts_with("\"alias\""));
}
