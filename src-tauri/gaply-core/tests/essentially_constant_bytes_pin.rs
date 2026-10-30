//! **Every computed case of the t-test's and the one-sample R goldens, its
//! canonical bytes pinned (Research OS, Phase 8D-1b, decision 259).** 8D-1b
//! adds R's "data are essentially constant" rule to the three analysis
//! t-tests. The rule only refuses: a result that computed before must keep
//! every bit. The R goldens compare within a gate, so they would show a case
//! that became a refusal but not a changed bit; this does. Written and
//! captured on `04d8705`'s code, before the rule.
//!
//! For each golden file, one line per computed case, `case variant sha256`,
//! in the file's case order, where the digest is of the result's
//! `canonical_bytes()` (what the manifest's `output_hash` hashes); then one
//! SHA-256 over those lines, pinned with the number of cases. Every line is
//! printed, so a changed digest names its case. The bytes hold p, which goes
//! through the platform's maths library. Since Research OS Phase 8E
//! (decision 277) the cases at df ≤ 1e4 are asserted on every platform: the
//! reviewer got macOS's digests on Linux x86_64 at `3d1253e`, CI's Linux job
//! checks them on every push, and the Windows job on a pull request is
//! Windows's measurement. The cases above df 1e4 (`large_df` under Student
//! and Welch; `one_df_10001` and `paired_df_10001`), which compute since 8E
//! through Hill's p (`ln_1p` and statrs's `erfc`, which calls the platform's
//! `exp`), are a second digest per golden file, captured on macOS aarch64
//! and asserted there only; elsewhere the test prints it and asserts the
//! count.

use std::collections::BTreeMap;

use gaply_core::contracts::ContentHash;
use gaply_core::stats_desc::Alias;
use gaply_core::stats_l1::dataset::{build_dataset, DataFingerprint, Dataset, RawTable, TableRef};
use gaply_core::stats_onesample::{
    one_sample_compute, paired_compute, OneSampleParams, PairedParams,
};
use gaply_core::stats_ttest::{compute, TTestParams, Variant};

const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures");

/// The cases at df ≤ 1e4: captured on `04d8705`'s code, macOS aarch64,
/// 26 Oct 2026, and unchanged by 8E (every line is printed, so 8E's report
/// diffs them against the run before it). Asserted on every platform.
const TTEST_CASES: usize = 32;
const TTEST_DIGEST: &str = "048fc4d482c0e61ee5b0af196d561859ddf99530476ba816f421040061755540";
const ONESAMPLE_CASES: usize = 25;
const ONESAMPLE_DIGEST: &str = "54c4645e15801ac74508bd85801d979d8c60d1e98958c6314a5a00082bde3939";
/// The cases above df 1e4, computed since 8E: captured on macOS aarch64,
/// 30 Oct 2026, and asserted there only (decision 277).
const TTEST_LARGE_CASES: usize = 2;
const TTEST_LARGE_DIGEST: &str = "82c5161839bf61c306d3e81b7ef51bdc26e19dcd03a9f54b4d4e8bb74e70ee6a";
const ONESAMPLE_LARGE_CASES: usize = 2;
const ONESAMPLE_LARGE_DIGEST: &str =
    "df6a98f4c0a78cb4b7118f8aa975253e39ca7f19e4475682c4cf695eb1db6fe3";

/// The switch of `stats_dist` (its `LARGE_DF`): a case above it goes through
/// the platform's `exp`.
const LARGE_DF: f64 = 1e4;

/// A golden file's rows grouped by their key columns, in file order of first
/// appearance; `key` is the number of leading columns that name a case.
fn cases(dir: &str, key: usize) -> Vec<(Vec<String>, Vec<(String, String)>)> {
    let text = std::fs::read_to_string(format!("{FIXTURES}/{dir}/golden.csv")).expect("golden.csv");
    let mut out: Vec<(Vec<String>, Vec<(String, String)>)> = Vec::new();
    for line in text.lines().filter(|l| !l.starts_with('#')).skip(1) {
        let f: Vec<&str> = line.split(',').collect();
        let k: Vec<String> = f[..key].iter().map(|s| s.to_string()).collect();
        let stat = (f[f.len() - 2].to_string(), f[f.len() - 1].to_string());
        match out.iter_mut().find(|(have, _)| *have == k) {
            Some((_, stats)) => stats.push(stat),
            None => out.push((k, vec![stat])),
        }
    }
    out
}

/// A data file of the golden's folder, its '#' lines skipped.
fn data(dir: &str, name: &str) -> Dataset {
    let bytes = std::fs::read(format!("{FIXTURES}/{dir}/{name}")).expect("data file");
    let text = String::from_utf8(bytes.clone()).expect("utf-8");
    let mut lines = text.lines().filter(|l| !l.starts_with('#'));
    let headers: Vec<String> = lines
        .next()
        .expect("header")
        .split(',')
        .map(str::to_string)
        .collect();
    let width = headers.len();
    let rows: Vec<Vec<String>> = lines
        .map(|l| {
            let mut r: Vec<String> = l.split(',').map(str::to_string).collect();
            r.resize(width, String::new());
            r
        })
        .collect();
    build_dataset(
        RawTable::from_rows(headers, rows),
        DataFingerprint::of_bytes(&bytes, "golden csv", TableRef::FirstTable),
    )
    .expect("dataset")
}

fn alias(d: &Dataset, header: &str) -> Alias {
    Alias(
        d.schema
            .columns
            .iter()
            .find(|c| c.header == header)
            .expect("column")
            .alias,
    )
}

fn codes(s: &str) -> Vec<f64> {
    s.split(';')
        .filter(|s| !s.is_empty())
        .map(|s| s.parse().expect("code"))
        .collect()
}

fn refused(stats: &[(String, String)]) -> bool {
    stats.iter().any(|(name, _)| name == "expected_refusal")
}

/// `case variant sha256` per computed case, then their digest.
fn digest(lines: &[String]) -> String {
    ContentHash::of_bytes(lines.join("\n").as_bytes())
        .as_hex()
        .to_string()
}

/// `everywhere`: asserted on every platform (the cases at df ≤ 1e4), or on
/// macOS aarch64 only (the cases above).
fn check(name: &str, lines: &[String], cases: usize, pinned: &str, everywhere: bool) {
    for l in lines {
        println!("{name}: {l}");
    }
    let got = digest(lines);
    println!("{name}: {} cases, digest {got}", lines.len());
    assert_eq!(lines.len(), cases, "{name}: the computed cases");
    if everywhere || cfg!(all(target_os = "macos", target_arch = "aarch64")) {
        assert_eq!(got, pinned, "{name}: a computed result's bytes changed");
    }
}

#[test]
fn every_computed_t_test_golden_case_keeps_its_bytes() {
    // case, data, level_index1, level_index2, missing_codes, variant.
    let mut lines = Vec::new();
    let mut large = Vec::new();
    let mut seen: BTreeMap<String, ()> = BTreeMap::new();
    for (k, stats) in cases("stats_ttest", 6) {
        if refused(&stats) {
            continue;
        }
        let d = data("stats_ttest", &k[1]);
        let p = TTestParams {
            outcome: alias(&d, "y"),
            group: alias(&d, "g"),
            levels: Some([k[2].parse().expect("index"), k[3].parse().expect("index")]),
            variant: Some(match k[5].as_str() {
                "student" => Variant::Student,
                "welch" => Variant::Welch,
                other => panic!("variant {other}"),
            }),
            missing_codes: codes(&k[4]),
        };
        let r = compute(&d, &p).unwrap_or_else(|e| panic!("{} {}: {e:?}", k[0], k[5]));
        let line = format!(
            "{} {} {}",
            k[0],
            k[5],
            ContentHash::of_bytes(&r.canonical_bytes()).as_hex()
        );
        assert!(
            seen.insert(format!("{} {}", k[0], k[5]), ()).is_none(),
            "{line}: twice"
        );
        if r.df > LARGE_DF {
            large.push(line);
        } else {
            lines.push(line);
        }
    }
    check("ttest", &lines, TTEST_CASES, TTEST_DIGEST, true);
    check(
        "ttest above 1e4",
        &large,
        TTEST_LARGE_CASES,
        TTEST_LARGE_DIGEST,
        false,
    );
}

#[test]
fn every_computed_one_sample_and_paired_golden_case_keeps_its_bytes() {
    // case, data, method, test_value, missing_codes.
    let mut lines = Vec::new();
    let mut large = Vec::new();
    for (k, stats) in cases("stats_onesample", 5) {
        if refused(&stats) {
            continue;
        }
        let d = data("stats_onesample", &k[1]);
        let (df, bytes) = match k[2].as_str() {
            "one_sample" => {
                let p = OneSampleParams {
                    outcome: alias(&d, "x"),
                    test_value: Some(k[3].parse().expect("test value")),
                    missing_codes: codes(&k[4]),
                };
                let r = one_sample_compute(&d, &p).unwrap_or_else(|e| panic!("{}: {e:?}", k[0]));
                (r.df, r.canonical_bytes())
            }
            "paired" => {
                let p = PairedParams {
                    first: alias(&d, "a"),
                    second: alias(&d, "b"),
                    missing_codes: codes(&k[4]),
                };
                let r = paired_compute(&d, &p).unwrap_or_else(|e| panic!("{}: {e:?}", k[0]));
                (r.df, r.canonical_bytes())
            }
            other => panic!("method {other}"),
        };
        let line = format!(
            "{} {} {}",
            k[0],
            k[2],
            ContentHash::of_bytes(&bytes).as_hex()
        );
        if df > LARGE_DF {
            large.push(line);
        } else {
            lines.push(line);
        }
    }
    check("onesample", &lines, ONESAMPLE_CASES, ONESAMPLE_DIGEST, true);
    check(
        "onesample above 1e4",
        &large,
        ONESAMPLE_LARGE_CASES,
        ONESAMPLE_LARGE_DIGEST,
        false,
    );
}
