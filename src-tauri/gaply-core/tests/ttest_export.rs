//! **The t-test's exports and determinism (Research OS, Phase 4B-1).**
//!
//! * The CSV reads back field by field: an RFC 4180 reader that lives HERE,
//!   not in shipped code, strips exactly one leading apostrophe from a quoted
//!   field that has one, and every statistic name, label, number and word
//!   comes back exactly, including level labels that start with `=`, `-`,
//!   `@` and an apostrophe. The export uses the descriptive summary's own
//!   field writer, so the guard is one rule.
//! * The manifest's JSON holds no header, label, cell or path (sentinels,
//!   with positive controls), and rows used plus the rows left out are the
//!   rows read.
//! * Ten runs give byte-identical result and manifest bytes (the elapsed
//!   time, the one field the caller measures, is passed as the same value).

use gaply_core::contracts::codes::Closed;
use gaply_core::contracts::{Arch, OperationId, Os, Platform};
use gaply_core::stats_desc::Alias;
use gaply_core::stats_l1::dataset::{
    build_dataset, ColumnAlias, DataFingerprint, Dataset, RawTable, TableRef,
};
use gaply_core::stats_ttest::export::{manifest_json, result_csv};
use gaply_core::stats_ttest::{compute, manifest, spec, Stat, TTestParams, TTestResult, Variant};

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

fn params(variant: Variant) -> TTestParams {
    TTestParams {
        outcome: Alias(ColumnAlias(1)),
        group: Alias(ColumnAlias(2)),
        levels: None,
        variant: Some(variant),
        missing_codes: vec![-99.0],
    }
}

fn platform() -> Platform {
    Platform {
        os: Os::Macos,
        arch: Arch::Aarch64,
    }
}

/// What each record must hold, built from the result's own fields.
fn expected(r: &TTestResult) -> Vec<[String; 5]> {
    let f = |v: f64| format!("{v}");
    let row = |s: &str, g: &str, l: &str, v: String, w: &str| {
        [
            s.to_string(),
            g.to_string(),
            l.to_string(),
            v,
            w.to_string(),
        ]
    };
    let variant = match r.variant {
        Variant::Student => "student",
        Variant::Welch => "welch",
    };
    let sw = serde_json::to_value(r.d_standardizer).unwrap();
    let sw = sw.as_str().unwrap();
    let mut out = vec![
        row("statistic", "group", "level", "value".into(), "word"),
        row("variant", "", "", String::new(), variant),
        row("t", "", "", f(r.t), ""),
        row("df", "", "", f(r.df), ""),
        row("p", "", "", f(r.p), ""),
        row("mean_difference", "", "", f(r.mean_difference), ""),
        row("standard_error", "", "", f(r.standard_error), ""),
        row("confidence_level", "", "", f(r.confidence_level), ""),
        row("ci_lower", "", "", f(r.ci_lower), ""),
        row("ci_upper", "", "", f(r.ci_upper), ""),
        row("cohens_d", "", "", f(r.cohens_d), sw),
        row("d_standardizer", "", "", f(r.d_standardizer_value), sw),
    ];
    out.push(match r.sd_ratio {
        Stat::Value(v) => row("sd_ratio", "", "", f(v), ""),
        Stat::NotComputable(c) => row("sd_ratio", "", "", String::new(), c.wire()),
    });
    for (k, g) in r.groups.iter().enumerate() {
        let n = (k + 1).to_string();
        out.push(row("level_index", &n, &g.label, g.level.to_string(), ""));
        out.push(row("n", &n, &g.label, g.n.to_string(), ""));
        out.push(row("mean", &n, &g.label, f(g.mean), ""));
        out.push(row("sd", &n, &g.label, f(g.sd), ""));
    }
    for e in &r.exclusions {
        out.push(row("excluded", "", "", e.count.to_string(), e.code.wire()));
    }
    for c in &r.limitations {
        out.push(row("limitation", "", "", String::new(), c.wire()));
    }
    out
}

fn tricky(labels: [&str; 2]) -> Dataset {
    let [a, b] = labels;
    table(
        &["score", "arm"],
        &[
            &["1.5", a],
            &["2.25", a],
            &["3.75", a],
            &["-99", a],
            &["4.5", b],
            &["6.125", b],
            &["", b],
            &["5.5", ""],
            &["7", b],
        ],
    )
}

#[test]
fn the_csv_reads_back_field_by_field_with_labels_a_spreadsheet_could_run() {
    let mut seen = Vec::new();
    for labels in [["=a", "-b"], ["@c", "'d"]] {
        for variant in [Variant::Student, Variant::Welch] {
            let d = tricky(labels);
            let r = compute(&d, &params(variant)).expect("computes");
            let bytes = result_csv(&r);
            let records = read_csv(&bytes);
            let want = expected(&r);
            assert_eq!(records.len(), want.len());
            for (got, want) in records.iter().zip(&want) {
                assert_eq!(got.len(), 5);
                for (i, (g, w)) in got.iter().zip(want).enumerate() {
                    assert_eq!(g.text, *w, "field {i} of {want:?}");
                    assert_eq!(
                        g.quoted,
                        i != 3 || want[0] == "statistic",
                        "only the value field is unquoted (header row aside)"
                    );
                    if i == 3 && !g.text.is_empty() && want[0] != "statistic" {
                        let v: f64 = g.text.parse().expect("a number");
                        assert_eq!(
                            format!("{v}"),
                            *w,
                            "the number reads back to the same double"
                        );
                    }
                }
            }
            // The labels came back exactly, and in the bytes each one carries
            // the guard apostrophe.
            let csv = String::from_utf8(bytes).expect("utf-8");
            for label in labels {
                assert!(r.groups.iter().any(|g| g.label == label), "{label}");
                assert!(
                    csv.contains(&format!("\"'{}\"", label.replace('"', "\"\""))),
                    "{label} unguarded:\n{csv}"
                );
                seen.push(label);
            }
            assert!(
                !csv.contains("\"=") && !csv.contains("\"-") && !csv.contains("\"@"),
                "no quoted field starts with a formula character"
            );
        }
    }
    // Sorted by code point: "'d" (0x27) before "@c" (0x40), "-b" (0x2d) before "=a" (0x3d).
    assert_eq!(seen.len(), 8);
}

#[test]
fn the_manifest_json_holds_no_header_label_cell_or_path_and_its_rows_add_up() {
    let d = table(
        &["SENTINEL_HDR_4b1e", "SENTINEL_GRP_90cd"],
        &[
            &["98765.4321", "SENTINEL_LBL_77aa"],
            &["1.5", "SENTINEL_LBL_77aa"],
            &["2.75", "SENTINEL_LBL_77aa"],
            &["", "SENTINEL_LBL_77aa"],
            &["3.5", "zz_other"],
            &["4.5", "zz_sentinel_b"],
            &["5.25", "zz_sentinel_b"],
            &["6", ""],
        ],
    );
    let p = TTestParams {
        outcome: Alias(ColumnAlias(1)),
        group: Alias(ColumnAlias(2)),
        levels: Some([0, 2]),
        variant: Some(Variant::Welch),
        missing_codes: vec![],
    };
    let s = spec(OperationId(1), &d, p);
    let r = compute(&d, &s.parameters).expect("computes");
    let m = manifest(&s, &d, &r, 4, platform());
    let json = String::from_utf8(manifest_json(&m)).expect("utf-8");
    // Positive controls: the sentinels are in the result, and the manifest
    // is the manifest.
    let csv = String::from_utf8(result_csv(&r)).expect("utf-8");
    assert!(csv.contains("SENTINEL_LBL_77aa") && csv.contains("zz_sentinel_b"));
    assert!(
        json.contains("\"capabilityId\": \"stats.analyze.independent_t\"")
            && json.contains("\"outputHash\""),
        "{json}"
    );
    for leak in [
        "SENTINEL", "sentinel", "zz_", "98765", "/", "\\", ".csv", "Users",
    ] {
        assert!(!json.contains(leak), "the manifest holds {leak:?}:\n{json}");
    }
    let params_json = serde_json::to_string(&s.parameters).expect("json");
    assert!(!params_json.contains("SENTINEL"), "{params_json}");
    assert!(
        !format!("{m:?}").contains("SENTINEL") && !format!("{r:?}").contains("SENTINEL"),
        "Debug prints counts only"
    );
    assert_eq!(m.rows_read, 8);
    let left_out: u64 = m.exclusions.iter().map(|e| e.count).sum();
    assert_eq!(m.rows_used, 5);
    assert_eq!(
        m.rows_used + left_out,
        m.rows_read,
        "used plus left out is read"
    );
}

#[test]
fn ten_runs_give_the_same_result_and_manifest_bytes() {
    let d = tricky(["ctrl", "trt"]);
    for variant in [Variant::Student, Variant::Welch] {
        let s = spec(OperationId(7), &d, params(variant));
        let mut results = Vec::new();
        let mut manifests = Vec::new();
        for _ in 0..10 {
            let r = compute(&d, &s.parameters).expect("computes");
            manifests.push(manifest_json(&manifest(&s, &d, &r, 0, platform())));
            results.push(r.canonical_bytes());
        }
        assert!(
            results.windows(2).all(|w| w[0] == w[1]),
            "result bytes differ"
        );
        assert!(
            manifests.windows(2).all(|w| w[0] == w[1]),
            "manifest bytes differ"
        );
        assert_eq!(results.len(), 10);
    }
}
