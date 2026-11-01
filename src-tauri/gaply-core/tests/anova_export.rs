//! **The one-way ANOVA's exports (Research OS, Phase 8G-2, decision 327).**
//!
//! * The CSV reads back field by field, in the screen's order (the group
//!   table, the ANOVA table, Welch's ANOVA, Levene's two lines, the effect
//!   sizes, then each chosen post hoc table), through an RFC 4180 reader that
//!   lives HERE, not in shipped code. Every number reads back to the same
//!   double, and level labels that start with `=`, `-`, `@` or an apostrophe
//!   come back exactly, with the guard apostrophe in the bytes.
//! * Anything not computable is an empty value with its closed code; a p
//!   below the studentized range's floor is written as the floor with the
//!   word `below`, never as a number of its own.
//! * The manifest's hashes close the CSV; the manifest's JSON holds no
//!   header, label, cell or path (sentinels, with positive controls).
//! * The bytes are pinned: a golden CSV and the manifest's JSON for one
//!   computed case with all three post hoc tests. Captured on macOS aarch64,
//!   1 Nov 2026, and asserted on every platform, as `ttest_bytes_pin.rs` is
//!   (decision 277): the bytes hold p through the platform's maths library,
//!   so a red on Linux or Windows names that platform's bytes.
//! * Ten runs give byte-identical CSV and manifest bytes.

use gaply_core::contracts::codes::Closed;
use gaply_core::contracts::{Arch, OperationId, Os, Platform};
use gaply_core::stats_anova::export::{manifest_json, result_csv};
use gaply_core::stats_anova::{
    compute, manifest, spec, AnovaCode, AnovaOutcome, AnovaParams, FOutcome, PairOutcome, PostHoc,
    PostHocP, PostHocPairs, Stat, Table,
};
use gaply_core::stats_desc::Alias;
use gaply_core::stats_l1::dataset::{
    build_dataset, ColumnAlias, DataFingerprint, Dataset, RawTable, TableRef,
};

fn table(headers: &[&str], rows: &[Vec<String>]) -> Dataset {
    let raw = RawTable::from_rows(
        headers.iter().map(|h| h.to_string()).collect(),
        rows.to_vec(),
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

fn rows(pairs: &[(&str, &str)]) -> Vec<Vec<String>> {
    pairs
        .iter()
        .map(|(y, g)| vec![y.to_string(), g.to_string()])
        .collect()
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

const ALL_THREE: [PostHoc; 3] = [PostHoc::Tukey, PostHoc::GamesHowell, PostHoc::Bonferroni];

fn params(post_hoc: &[PostHoc], missing_codes: Vec<f64>) -> AnovaParams {
    AnovaParams {
        outcome: Alias(ColumnAlias(1)),
        group: Alias(ColumnAlias(2)),
        levels: None,
        post_hoc: post_hoc.to_vec(),
        missing_codes,
    }
}

fn platform() -> Platform {
    Platform {
        os: Os::Macos,
        arch: Arch::Aarch64,
    }
}

type Record = [String; 8];

/// What each record must hold, built from the result's and the manifest's
/// own fields, in the order decision 327 gives.
fn expected(r: &AnovaOutcome, m: &gaply_core::contracts::RunManifest<AnovaCode>) -> Vec<Record> {
    let f = |v: f64| format!("{v}");
    let row = |t: &str, s: &str, i: String, l: &str, i2: String, l2: &str, v: String, w: &str| {
        [
            t.to_string(),
            s.to_string(),
            i,
            l.to_string(),
            i2,
            l2.to_string(),
            v,
            w.to_string(),
        ]
    };
    let plain = |t: &str, s: &str, v: String, w: &str| {
        row(t, s, String::new(), "", String::new(), "", v, w)
    };
    let stat = |t: &str, s: &str, v: Stat| match v {
        Stat::Value(x) => plain(t, s, f(x), ""),
        Stat::NotComputable(c) => plain(t, s, String::new(), c.wire()),
    };
    let label = |level: u32| -> String {
        r.groups
            .iter()
            .find(|g| g.level == level)
            .map(|g| g.label.clone())
            .expect("a pair's level is a group's")
    };
    let mut out = vec![
        [
            "table",
            "statistic",
            "level_index",
            "level",
            "level_index_2",
            "level_2",
            "value",
            "word",
        ]
        .map(String::from),
        plain("result", "confidence_level", f(0.95), ""),
    ];
    for g in &r.groups {
        for (name, v) in [
            ("n", g.n.to_string()),
            ("mean", f(g.mean)),
            ("sd", f(g.sd)),
            ("se", f(g.se)),
            ("ci_lower", f(g.ci_lower)),
            ("ci_upper", f(g.ci_upper)),
            ("min", f(g.min)),
            ("max", f(g.max)),
        ] {
            out.push(row(
                "groups",
                name,
                g.level.to_string(),
                &g.label,
                String::new(),
                "",
                v,
                "",
            ));
        }
    }
    match r.table {
        Table::Computed(t) => {
            for (name, v) in [
                ("ss_between", t.ss_between),
                ("ss_within", t.ss_within),
                ("ss_total", t.ss_total),
                ("df_between", t.df_between),
                ("df_within", t.df_within),
                ("ms_between", t.ms_between),
                ("ms_within", t.ms_within),
            ] {
                out.push(plain("anova", name, f(v), ""));
            }
            out.push(stat("anova", "f", t.f));
            out.push(stat("anova", "p", t.p));
        }
        Table::NotComputable(c) => {
            out.push(plain("anova", "not_computable", String::new(), c.wire()))
        }
    }
    for (name, test) in [
        ("welch", r.welch),
        ("levene_mean", r.levene_mean),
        ("levene_median", r.levene_median),
    ] {
        match test {
            FOutcome::Computed(t) => {
                for (s, v) in [("f", t.f), ("df1", t.df1), ("df2", t.df2), ("p", t.p)] {
                    out.push(plain(name, s, f(v), ""));
                }
            }
            FOutcome::NotComputable(c) => {
                out.push(plain(name, "not_computable", String::new(), c.wire()))
            }
        }
    }
    out.push(stat("effect_sizes", "eta_squared", r.eta_squared));
    out.push(stat("effect_sizes", "omega_squared", r.omega_squared));
    for t in &r.post_hoc {
        let name = match t.test {
            PostHoc::Tukey => "tukey",
            PostHoc::GamesHowell => "games_howell",
            PostHoc::Bonferroni => "bonferroni",
        };
        let statistic = match t.test {
            PostHoc::Bonferroni => "t",
            _ => "q",
        };
        match &t.pairs {
            PostHocPairs::NotComputable(c) => {
                out.push(plain(name, "not_computable", String::new(), c.wire()))
            }
            PostHocPairs::Computed(pairs) => {
                for p in pairs {
                    let (a, b) = (label(p.first), label(p.second));
                    let pr = |s: &str, v: String, w: &str| {
                        row(
                            name,
                            s,
                            p.first.to_string(),
                            &a,
                            p.second.to_string(),
                            &b,
                            v,
                            w,
                        )
                    };
                    match p.outcome {
                        PairOutcome::Computed(s) => {
                            out.push(pr("difference", f(s.difference), ""));
                            out.push(pr("se", f(s.se), ""));
                            out.push(pr("statistic", f(s.statistic), statistic));
                            out.push(pr("df", f(s.df), ""));
                            out.push(match s.p {
                                PostHocP::Value(v) => pr("p", f(v), ""),
                                PostHocP::Below(floor) => pr("p", f(floor), "below"),
                            });
                            out.push(pr("ci_lower", f(s.ci_lower), ""));
                            out.push(pr("ci_upper", f(s.ci_upper), ""));
                        }
                        PairOutcome::NotComputable(c) => {
                            out.push(pr("not_computable", String::new(), c.wire()))
                        }
                    }
                }
            }
        }
    }
    for e in &r.exclusions {
        out.push(plain(
            "excluded",
            "rows",
            e.count.to_string(),
            e.code.wire(),
        ));
    }
    for c in &r.notes {
        out.push(plain("note", "", String::new(), c.wire()));
    }
    for c in &r.limitations {
        out.push(plain("limitation", "", String::new(), c.wire()));
    }
    for h in &m.inputs {
        out.push(plain("manifest", "input_sha256", String::new(), h.as_hex()));
    }
    out.push(plain(
        "manifest",
        "parameters_sha256",
        String::new(),
        m.parameters_hash.as_hex(),
    ));
    out.push(plain(
        "manifest",
        "output_sha256",
        String::new(),
        m.output_hash.as_hex(),
    ));
    out
}

/// Three groups whose labels a spreadsheet would run, unequal variances, a
/// missing code, a missing outcome and a missing group.
fn tricky(labels: [&str; 3]) -> Dataset {
    let [a, b, c] = labels;
    table(
        &["score", "arm"],
        &rows(&[
            ("1.5", a),
            ("2.25", a),
            ("3.75", a),
            ("2.5", a),
            ("-99", a),
            ("4.5", b),
            ("6.125", b),
            ("", b),
            ("7", b),
            ("5.25", b),
            ("9.5", c),
            ("12.75", c),
            ("8", c),
            ("15.5", c),
            ("5.5", ""),
        ]),
    )
}

fn check_reads_back(d: &Dataset, p: &AnovaParams) -> (AnovaOutcome, String) {
    let s = spec(OperationId(2), d, p.clone());
    let r = compute(d, &s.parameters).expect("computes");
    let m = manifest(&s, d, &r, 3, platform());
    let bytes = result_csv(&r, &m);
    let records = read_csv(&bytes);
    let want = expected(&r, &m);
    assert_eq!(records.len(), want.len(), "one record per expected row");
    for (got, want) in records.iter().zip(&want) {
        assert_eq!(got.len(), 8, "{want:?}");
        for (i, (g, w)) in got.iter().zip(want).enumerate() {
            assert_eq!(g.text, *w, "field {i} of {want:?}");
            assert_eq!(
                g.quoted,
                i != 6 || want[0] == "table",
                "only the value field is unquoted (header row aside): {want:?}"
            );
            if i == 6 && !g.text.is_empty() && want[0] != "table" {
                let v: f64 = g.text.parse().expect("a number");
                assert_eq!(
                    format!("{v}"),
                    *w,
                    "the number reads back to the same double"
                );
            }
        }
    }
    (r, String::from_utf8(bytes).expect("utf-8"))
}

#[test]
fn the_csv_reads_back_field_by_field_in_the_screens_order() {
    let mut seen = 0;
    for labels in [["=a", "-b", "@c"], ["'d", "+e", "f"]] {
        for post_hoc in [&[][..], &ALL_THREE[..], &[PostHoc::GamesHowell][..]] {
            let d = tricky(labels);
            let (r, csv) = check_reads_back(&d, &params(post_hoc, vec![-99.0]));
            assert_eq!(r.post_hoc.len(), post_hoc.len());
            for label in labels {
                assert!(r.groups.iter().any(|g| g.label == label), "{label}");
                if label.starts_with(['=', '-', '@', '\'', '+']) {
                    assert!(
                        csv.contains(&format!("\"'{label}\"")),
                        "{label} unguarded:\n{csv}"
                    );
                }
                seen += 1;
            }
            assert!(
                !csv.contains("\"=") && !csv.contains("\"-") && !csv.contains("\"@"),
                "no quoted field starts with a formula character"
            );
            // The screen's order: groups, anova, welch, levene, effect sizes,
            // post hoc, then the rest.
            let order: Vec<&str> = csv
                .split("\r\n")
                .skip(1)
                .filter(|l| !l.is_empty())
                .filter_map(|l| l.split(',').next())
                .collect();
            let mut tables: Vec<&str> = order.clone();
            tables.dedup();
            let mut want = vec![
                "\"result\"",
                "\"groups\"",
                "\"anova\"",
                "\"welch\"",
                "\"levene_mean\"",
                "\"levene_median\"",
                "\"effect_sizes\"",
            ];
            for t in post_hoc {
                want.push(match t {
                    PostHoc::Tukey => "\"tukey\"",
                    PostHoc::GamesHowell => "\"games_howell\"",
                    PostHoc::Bonferroni => "\"bonferroni\"",
                });
            }
            want.extend(["\"excluded\"", "\"limitation\"", "\"manifest\""]);
            assert_eq!(tables, want, "the tables' order");
        }
    }
    assert_eq!(seen, 18);
}

#[test]
fn a_p_below_the_floor_is_written_as_below_it_never_as_a_number() {
    // Three groups far apart: every Tukey and Games-Howell p is below 1e-10.
    let mut data = Vec::new();
    for (k, centre) in [("a", 0.0), ("b", 50.0), ("c", 100.0)] {
        for i in 0..30 {
            data.push((format!("{}", centre + (i % 7) as f64 * 0.25), k.to_string()));
        }
    }
    let pairs: Vec<(&str, &str)> = data.iter().map(|(y, g)| (y.as_str(), g.as_str())).collect();
    let d = table(&["y", "g"], &rows(&pairs));
    let (r, csv) = check_reads_back(&d, &params(&ALL_THREE, vec![]));
    assert!(
        r.notes.contains(&AnovaCode::PBelowFloor),
        "the note is there"
    );
    let mut below = 0;
    for line in csv.split("\r\n") {
        let f: Vec<&str> = line.split(',').collect();
        if f.len() == 8 && (f[0] == "\"tukey\"" || f[0] == "\"games_howell\"") && f[1] == "\"p\"" {
            assert_eq!(f[7], "\"below\"", "{line}");
            assert_eq!(
                f[6].parse::<f64>().unwrap(),
                1e-10,
                "the floor, from the result"
            );
            below += 1;
        }
    }
    assert_eq!(below, 6, "three pairs, two tests, each below the floor");
    assert!(csv.contains("\"note\",\"\",\"\",\"\",\"\",\"\",,\"stats.anova.p_below_floor\""));
}

#[test]
fn what_is_not_computable_is_written_as_its_code() {
    // Every group constant, different means: no F, no Welch, no Levene, no
    // post hoc; the sums of squares and eta squared are there.
    let d = table(
        &["y", "g"],
        &rows(&[
            ("1", "a"),
            ("1", "a"),
            ("2", "b"),
            ("2", "b"),
            ("4", "c"),
            ("4", "c"),
        ]),
    );
    let (r, csv) = check_reads_back(&d, &params(&ALL_THREE, vec![]));
    for want in [
        "\"anova\",\"f\",\"\",\"\",\"\",\"\",,\"stats.anova.within_variance_zero\"",
        "\"welch\",\"not_computable\",\"\",\"\",\"\",\"\",,\"stats.anova.within_variance_zero\"",
        "\"levene_median\",\"not_computable\",\"\",\"\",\"\",\"\",,\"stats.anova.within_variance_zero\"",
        "\"tukey\",\"not_computable\",\"\",\"\",\"\",\"\",,\"stats.anova.within_variance_zero\"",
    ] {
        assert!(csv.contains(want), "{want} missing:\n{csv}");
    }
    assert!(matches!(r.eta_squared, Stat::Value(_)));
    assert!(csv.contains("\"effect_sizes\",\"eta_squared\",\"\",\"\",\"\",\"\",1,\"\""));
    // One group constant: Welch is not computable, and a Games-Howell pair
    // of two constant groups is, while the other pairs are computed.
    let d = table(
        &["y", "g"],
        &rows(&[
            ("3", "a"),
            ("3", "a"),
            ("5", "b"),
            ("5", "b"),
            ("1", "c"),
            ("2.5", "c"),
            ("4", "c"),
        ]),
    );
    let (_, csv) = check_reads_back(&d, &params(&[PostHoc::GamesHowell], vec![]));
    assert!(csv.contains(
        "\"welch\",\"not_computable\",\"\",\"\",\"\",\"\",,\"stats.anova.welch_variance_zero\""
    ));
    assert!(csv.contains(
        "\"games_howell\",\"not_computable\",\"0\",\"a\",\"1\",\"b\",,\"stats.anova.pair_both_constant\""
    ));
    assert!(csv.contains("\"games_howell\",\"difference\",\"0\",\"a\",\"2\",\"c\","));
}

/// The golden: one computed case, all three tests, a missing code, a
/// missing outcome and a missing group, its bytes as captured.
fn golden_case() -> (Vec<u8>, Vec<u8>) {
    let d = tricky(["ctrl", "dose_lo", "dose_hi"]);
    let s = spec(OperationId(9), &d, params(&ALL_THREE, vec![-99.0]));
    let r = compute(&d, &s.parameters).expect("computes");
    let m = manifest(&s, &d, &r, 0, platform());
    (result_csv(&r, &m), manifest_json(&m))
}

fn pinned(name: &str, got: &[u8]) {
    let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/stats_anova")
        .join(name);
    if std::env::var_os("GAPLY_WRITE_ANOVA_EXPORT_PIN").is_some() {
        std::fs::write(&p, got).expect("write the pin");
    }
    let want = std::fs::read(&p).unwrap_or_else(|e| panic!("{name}: {e}"));
    assert!(
        got == want.as_slice(),
        "{name}: the bytes differ from the pin.\n--- got ---\n{}\n--- pinned ---\n{}",
        String::from_utf8_lossy(got),
        String::from_utf8_lossy(&want)
    );
}

#[test]
fn the_export_bytes_are_pinned() {
    let (csv, json) = golden_case();
    pinned("export_golden.csv", &csv);
    pinned("export_golden_manifest.json", &json);
}

#[test]
fn the_manifest_json_holds_no_header_label_cell_or_path() {
    let d = table(
        &["SENTINEL_HDR_8g2a", "SENTINEL_GRP_8g2b"],
        &rows(&[
            ("98765.4321", "SENTINEL_LBL_77aa"),
            ("1.5", "SENTINEL_LBL_77aa"),
            ("2.75", "SENTINEL_LBL_77aa"),
            ("3.5", "zz_sentinel_b"),
            ("4.5", "zz_sentinel_b"),
            ("5.25", "zz_sentinel_c"),
            ("7.25", "zz_sentinel_c"),
            ("6", ""),
        ]),
    );
    let s = spec(OperationId(1), &d, params(&ALL_THREE, vec![]));
    let r = compute(&d, &s.parameters).expect("computes");
    let m = manifest(&s, &d, &r, 4, platform());
    let json = String::from_utf8(manifest_json(&m)).expect("utf-8");
    let csv = String::from_utf8(result_csv(&r, &m)).expect("utf-8");
    // Positive controls: the labels are in the user's own CSV, and the
    // manifest is the manifest, its hashes in the CSV too.
    assert!(csv.contains("SENTINEL_LBL_77aa") && csv.contains("zz_sentinel_b"));
    assert!(csv.contains(m.output_hash.as_hex()) && csv.contains(m.parameters_hash.as_hex()));
    assert!(
        json.contains("\"capabilityId\": \"stats.analyze.one_way_anova\"")
            && json.contains("\"outputHash\""),
        "{json}"
    );
    for leak in [
        "SENTINEL", "sentinel", "zz_", "98765", "/", "\\", ".csv", "Users",
    ] {
        assert!(!json.contains(leak), "the manifest holds {leak:?}:\n{json}");
    }
    // The CSV is the user's result: a group's max is their value (98765.4321
    // is the first group's), but no header is in it.
    assert!(csv.contains("98765.4321"), "the positive control: the max");
    assert!(
        !csv.contains("SENTINEL_HDR") && !csv.contains("SENTINEL_GRP"),
        "no header in the CSV"
    );
    assert!(
        !format!("{m:?}").contains("SENTINEL") && !format!("{r:?}").contains("SENTINEL"),
        "Debug prints counts only"
    );
    let left_out: u64 = m.exclusions.iter().map(|e| e.count).sum();
    assert_eq!((m.rows_read, m.rows_used, left_out), (8, 7, 1));
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
