//! **The correlations' exports (Research OS, Phase 8H-2, decision 354).**
//!
//! * The CSV reads back field by field, in the screen's order (the
//!   confidence level, the rows read and used, the descriptives, then each
//!   chosen coefficient's pairs), through an RFC 4180 reader that lives HERE,
//!   not in shipped code. Every number reads back to the same double.
//! * Anything not computable is an empty value with its closed code; an
//!   infinite t is the word `stats.corr.perfect_coefficient`, never a number.
//! * Columns are named by alias (`C1`, `C2`, …) and their index among the
//!   chosen variables, never by header, so a stored result and a live one
//!   export the same bytes; the manifest's JSON holds no header, cell or path
//!   (sentinels, with positive controls).
//! * The bytes are pinned on macOS aarch64 (captured 1 Nov 2026): a golden
//!   CSV and the manifest's JSON for one listwise case with all three
//!   coefficients and a ticked missing code, a perfect pair, pairs not
//!   computable and Kendall's intervals of too few rows. Every other platform,
//!   Linux x86_64 included, checks that pin's layout (decision 346's check,
//!   `pin_support`) until a later step pins Linux's bytes from a Linux run:
//!   the numbers pass through `atanh`, `tanh` and statrs, the platform's maths
//!   library. It never passes by skipping, and the layout check has a test of
//!   its own on this pin, run on every platform.
//! * Ten runs give byte-identical CSV and manifest bytes.

use gaply_core::contracts::codes::Closed;
use gaply_core::contracts::{Arch, OperationId, Os, ParamKey, ParamValue, Platform, RunManifest};
use gaply_core::stats_corr::export::{manifest_json, result_csv};
use gaply_core::stats_corr::{
    compute, manifest, spec, Bounded, Coefficient, CorrCode, CorrOutcome, CorrParams, Interval,
    MissingHandling, PairOutcome, Stat, TestStatistic,
};
use gaply_core::stats_desc::Alias;
use gaply_core::stats_l1::dataset::{
    build_dataset, ColumnAlias, DataFingerprint, Dataset, RawTable, TableRef,
};

mod pin_support;
use pin_support::{csv_layout_matches, json_layout_matches};

const ALL: [Coefficient; 3] = [
    Coefficient::Pearson,
    Coefficient::Spearman,
    Coefficient::Kendall,
];

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
            "corr export",
            TableRef::FirstTable,
        ),
    )
    .expect("dataset")
}

fn params(vars: &[u32], coefficients: &[Coefficient], missing: MissingHandling) -> CorrParams {
    CorrParams {
        variables: vars.iter().map(|v| Alias(ColumnAlias(*v))).collect(),
        coefficients: coefficients.to_vec(),
        missing,
        missing_codes: vec![-99.0],
    }
}

fn platform() -> Platform {
    Platform {
        os: Os::Macos,
        arch: Arch::Aarch64,
    }
}

/// Four columns, seven rows. Listwise with -99 ticked, rows 1 to 4 are used
/// (n = 4): y = 2x + 1 is a perfect pair, w is constant there (its three
/// pairs not computable), and every Kendall interval needs n of 5. Rows 5 to
/// 7 are left out under x (a code), y (an empty cell) and z (a code).
fn golden_data(headers: [&str; 4]) -> Dataset {
    dataset(
        &headers,
        &[
            &["1", "3", "2.5", "7"],
            &["2", "5", "1", "7"],
            &["3", "7", "4", "7"],
            &["4", "9", "3.5", "7"],
            &["-99", "11", "5", "7"],
            &["5", "", "6", "7"],
            &["6", "13", "-99", "7"],
        ],
    )
}

fn run_of(d: &Dataset, p: CorrParams) -> (CorrOutcome, RunManifest<CorrCode>) {
    let s = spec(OperationId(11), d, p);
    let r = compute(d, &s.parameters).expect("computes");
    let m = manifest(&s, d, &r, 0, platform());
    (r, m)
}

/// The golden: its bytes as captured.
fn golden_case() -> (Vec<u8>, Vec<u8>) {
    let d = golden_data(["x", "y", "z", "w"]);
    let (r, m) = run_of(&d, params(&[1, 2, 3, 4], &ALL, MissingHandling::Listwise));
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
/// own fields, in the order decision 354 gives.
fn expected(r: &CorrOutcome, m: &RunManifest<CorrCode>) -> Vec<[String; 8]> {
    let f = |v: f64| format!("{v}");
    let idx = |a: Alias| {
        let i = r
            .descriptives
            .iter()
            .position(|d| d.variable == a)
            .expect("a variable of the result");
        (i.to_string(), format!("C{}", a.0 .0))
    };
    let rec = |t: &str, s: &str, a: Option<Alias>, b: Option<Alias>, v: String, w: &str| {
        let (i1, a1) = a.map(idx).unwrap_or_default();
        let (i2, a2) = b.map(idx).unwrap_or_default();
        [t.into(), s.into(), i1, a1, i2, a2, v, w.into()]
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
        rec("result", "confidence_level", None, None, f(0.95), ""),
        rec(
            "result",
            "rows_read",
            None,
            None,
            m.rows_read.to_string(),
            "",
        ),
        rec(
            "result",
            "rows_used",
            None,
            None,
            r.rows_used.to_string(),
            "",
        ),
    ];
    let stat = |s: Stat| match s {
        Stat::Value(v) => (f(v), ""),
        Stat::NotComputable(c) => (String::new(), c.wire()),
    };
    for d in &r.descriptives {
        let a = Some(d.variable);
        out.push(rec("descriptives", "n", a, None, d.n.to_string(), ""));
        let (v, w) = stat(d.mean);
        out.push(rec("descriptives", "mean", a, None, v, w));
        let (v, w) = stat(d.sd);
        out.push(rec("descriptives", "sd", a, None, v, w));
    }
    for t in &r.coefficients {
        let (name, est) = match t.coefficient {
            Coefficient::Pearson => ("pearson", "r"),
            Coefficient::Spearman => ("spearman", "rho"),
            Coefficient::Kendall => ("kendall", "tau_b"),
        };
        for p in &t.pairs {
            let (a, b) = (Some(p.first), Some(p.second));
            out.push(rec(name, "n", a, b, p.n.to_string(), ""));
            let s = match p.outcome {
                PairOutcome::Computed(s) => s,
                PairOutcome::NotComputable(c) => {
                    out.push(rec(name, "not_computable", a, b, String::new(), c.wire()));
                    continue;
                }
            };
            out.push(rec(name, est, a, b, f(s.estimate), ""));
            match s.test {
                TestStatistic::T(t) => {
                    out.push(match t.t {
                        Bounded::Value(v) => rec(name, "t", a, b, f(v), ""),
                        Bounded::Infinite(c) => rec(name, "t", a, b, String::new(), c.wire()),
                    });
                    out.push(rec(name, "df", a, b, f(t.df), ""));
                }
                TestStatistic::Z(z) => out.push(rec(name, "z", a, b, f(z), "")),
            }
            out.push(rec(name, "p", a, b, f(s.p), ""));
            for (end, v) in [("ci_lower", 0), ("ci_upper", 1)] {
                out.push(match s.interval {
                    Interval::Computed { lower, upper } => {
                        rec(name, end, a, b, f([lower, upper][v]), "")
                    }
                    Interval::NotComputable(c) => rec(name, end, a, b, String::new(), c.wire()),
                });
            }
        }
    }
    for e in &r.exclusions {
        let column = match e.params.get(&ParamKey::Column) {
            Some(ParamValue::Int(n)) => Some(Alias(ColumnAlias(*n as u32))),
            _ => None,
        };
        out.push(rec(
            "excluded",
            "rows",
            column,
            None,
            e.count.to_string(),
            e.code.wire(),
        ));
    }
    for c in &r.limitations {
        out.push(rec("limitation", "", None, None, String::new(), c.wire()));
    }
    for h in &m.inputs {
        out.push(rec(
            "manifest",
            "input_sha256",
            None,
            None,
            String::new(),
            h.as_hex(),
        ));
    }
    out.push(rec(
        "manifest",
        "parameters_sha256",
        None,
        None,
        String::new(),
        m.parameters_hash.as_hex(),
    ));
    out.push(rec(
        "manifest",
        "output_sha256",
        None,
        None,
        String::new(),
        m.output_hash.as_hex(),
    ));
    out
}

/// The CSV of `p` on `d`, read back and checked record by record; every
/// number reads back to the same double.
fn check_reads_back(d: &Dataset, p: CorrParams) -> (CorrOutcome, String) {
    let (r, m) = run_of(d, p);
    let bytes = result_csv(&r, &m);
    let got = read_csv(&bytes);
    let want = expected(&r, &m);
    assert_eq!(got.len(), want.len(), "records");
    for (i, (g, w)) in got.iter().zip(&want).enumerate() {
        assert_eq!(g, w);
        if i > 0 && !w[6].is_empty() && w[0] != "result" && w[1] != "n" && w[0] != "excluded" {
            let back: f64 = g[6].parse().expect("a number");
            assert_eq!(back.to_bits(), w[6].parse::<f64>().unwrap().to_bits());
        }
    }
    (r, String::from_utf8(bytes).expect("utf-8"))
}

#[test]
fn the_csv_reads_back_in_the_screens_order_with_every_digit() {
    let d = golden_data(["x", "y", "z", "w"]);
    // Listwise, the golden's case.
    let (r, csv) = check_reads_back(&d, params(&[1, 2, 3, 4], &ALL, MissingHandling::Listwise));
    assert_eq!(r.rows_used, 4);
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
            "\"descriptives\"",
            "\"pearson\"",
            "\"spearman\"",
            "\"kendall\"",
            "\"excluded\"",
            "\"limitation\"",
            "\"manifest\""
        ]
    );
    // Pairwise and one coefficient: one row left out names no column.
    let (r, csv) = check_reads_back(&d, params(&[1, 3], &ALL[1..2], MissingHandling::Pairwise));
    assert_eq!(r.coefficients.len(), 1);
    assert!(
        csv.contains(
            "\"excluded\",\"rows\",\"\",\"\",\"\",\"\",2,\"stats.corr.fewer_than_two_values\""
        ),
        "{csv}"
    );
    assert!(!csv.contains("\"pearson\"") && !csv.contains("\"kendall\""));
}

#[test]
fn a_perfect_t_is_its_word_and_what_is_not_computable_is_its_code() {
    let (csv, _) = golden_case();
    let csv = String::from_utf8(csv).unwrap();
    for want in [
        "\"pearson\",\"r\",\"0\",\"C1\",\"1\",\"C2\",1,\"\"",
        "\"pearson\",\"t\",\"0\",\"C1\",\"1\",\"C2\",,\"stats.corr.perfect_coefficient\"",
        "\"pearson\",\"p\",\"0\",\"C1\",\"1\",\"C2\",0,\"\"",
        "\"pearson\",\"ci_lower\",\"0\",\"C1\",\"1\",\"C2\",1,\"\"",
        "\"spearman\",\"t\",\"0\",\"C1\",\"1\",\"C2\",,\"stats.corr.perfect_coefficient\"",
        "\"pearson\",\"not_computable\",\"0\",\"C1\",\"3\",\"C4\",,\"stats.corr.variable_constant\"",
        "\"kendall\",\"ci_lower\",\"0\",\"C1\",\"2\",\"C3\",,\"stats.corr.interval_too_few_rows\"",
        "\"excluded\",\"rows\",\"0\",\"C1\",\"\",\"\",1,\"stats.corr.missing_code\"",
        "\"excluded\",\"rows\",\"1\",\"C2\",\"\",\"\",1,\"stats.corr.missing_value\"",
        "\"excluded\",\"rows\",\"2\",\"C3\",\"\",\"\",1,\"stats.corr.missing_code\"",
    ] {
        assert!(csv.contains(want), "{want} missing:\n{csv}");
    }
    // A perfect Kendall pair keeps a finite z and its p.
    assert!(csv.contains("\"kendall\",\"tau_b\",\"0\",\"C1\",\"1\",\"C2\",1,\"\""));
    assert!(csv.contains("\"kendall\",\"z\",\"0\",\"C1\",\"1\",\"C2\","));
    let lower = csv.to_lowercase();
    for bad in ["inf", "nan", "null"] {
        assert!(!lower.contains(bad), "{bad} in the CSV:\n{csv}");
    }
}

#[test]
fn the_csv_and_the_manifest_name_columns_by_alias_and_hold_no_header_cell_or_path() {
    let d = golden_data([
        "SENTINEL_HDR_8h2a",
        "SENTINEL_HDR_8h2b",
        "zz_sentinel_c",
        "=SUM(A1)",
    ]);
    let (r, m) = run_of(&d, params(&[1, 2, 3, 4], &ALL, MissingHandling::Listwise));
    let csv = String::from_utf8(result_csv(&r, &m)).unwrap();
    let json = String::from_utf8(manifest_json(&m)).unwrap();
    // Positive controls: the headers are the dataset's, the CSV names the
    // aliases, and the manifest is the manifest, its hashes in the CSV too.
    assert_eq!(
        d.schema.column(ColumnAlias(1)).map(|c| c.header.as_str()),
        Some("SENTINEL_HDR_8h2a")
    );
    assert!(csv.contains("\"C1\"") && csv.contains("\"C4\""));
    assert!(csv.contains(m.output_hash.as_hex()) && csv.contains(m.parameters_hash.as_hex()));
    assert!(
        json.contains("\"capabilityId\": \"stats.analyze.correlations\"")
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
    let (r2, m2) = run_of(
        &plain,
        params(&[1, 2, 3, 4], &ALL, MissingHandling::Listwise),
    );
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
        .join("tests/fixtures/stats_corr")
        .join(name)
}

fn pin_bytes(name: &str) -> Vec<u8> {
    std::fs::read(pin_path(name)).unwrap_or_else(|e| panic!("{name}: {e}"))
}

/// The pin's difference from `got`, if any, so both files are reported.
fn pinned(name: &str, got: &[u8]) -> Option<String> {
    if std::env::var_os("GAPLY_WRITE_CORR_EXPORT_PIN").is_some() {
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
/// platform is checked against (decision 354).
const MAC_CSV: &str = "export_golden.csv";
const MAC_JSON: &str = "export_golden_manifest.json";

#[test]
fn the_export_bytes_are_pinned() {
    let (csv, json) = golden_case();
    if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
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

/// Linux relies on this (decision 354): the layout check on altered copies
/// of the correlations' pin, on every platform.
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
    // Pearson's lower end for x and z, a number through atanh and tanh,
    // moved by n units in the last place.
    let line = text
        .split("\r\n")
        .find(|l| l.starts_with("\"pearson\",\"ci_lower\",\"0\",\"C1\",\"2\",\"C3\","))
        .expect("Pearson's lower end for C1 and C3")
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
    // A changed word: the perfect pair's t called a number's.
    let word = "\"pearson\",\"t\",\"0\",\"C1\",\"1\",\"C2\",,\"stats.corr.perfect_coefficient\"";
    assert!(
        csv_layout_matches(
            &altered(word, &word.replace("perfect_coefficient", "overflow")),
            &pin
        )
        .is_err(),
        "a changed word"
    );
    // A missing line: that same t line.
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
