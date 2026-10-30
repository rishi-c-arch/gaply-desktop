//! **The independent t-test's canonical bytes on three computed cases,
//! pinned (Research OS, Phase 8D-1, decision 254).** 8D-1 moves the 95%
//! interval out of `stats_ttest`'s arithmetic into one function the one-sample
//! and paired t-tests share. The R goldens compare within a gate and the wire
//! fixtures are written by hand, so neither would show a changed bit; this
//! does. Written and captured on `81aed16`'s code before the move.
//!
//! The SHA-256 of `TTestResult::canonical_bytes()` (what the manifest's
//! `output_hash` hashes) for Student's test, Welch's test, and a case with
//! rows left out under two reasons. The bytes hold p, which goes through the
//! platform's maths library; captured on macOS aarch64, and asserted on
//! every platform since Research OS Phase 8E (decision 277): the reviewer
//! ran this pin on Linux x86_64 at `3d1253e` with the assertion forced on and
//! got macOS's bytes, CI's Linux job checks it on every push, and the Windows
//! job on a pull request is Windows's measurement. A red there names the
//! platform's digest, and "Bitwise" then needs its word in a step of its own.

use gaply_core::contracts::ContentHash;
use gaply_core::stats_desc::Alias;
use gaply_core::stats_l1::dataset::{build_dataset, DataFingerprint, Dataset, RawTable, TableRef};
use gaply_core::stats_ttest::{compute, TTestCode, TTestParams, Variant};

/// Captured on `81aed16`'s code, macOS aarch64, 26 Oct 2026.
const STUDENT: &str = "d52df4bdef300592e062c6c47155750b5a3381c2a71c82b69ac3050e0ffbc409";
const WELCH: &str = "57f4b05fe44a62c4ec58e47eeeeee7cb393c18c01289492f7f16bd6629705974";
const EXCLUSIONS: &str = "f1f78bf41df1d7e89bf3cc56790258418d869f5523d41cd26f6ef8b062d02eb1";

fn dataset(rows: &[(&str, &str)]) -> Dataset {
    build_dataset(
        RawTable::from_rows(
            vec!["y".into(), "g".into()],
            rows.iter()
                .map(|(y, g)| vec![y.to_string(), g.to_string()])
                .collect(),
        ),
        DataFingerprint::of_bytes(b"8d1 ttest bytes pin", "test", TableRef::FirstTable),
    )
    .expect("dataset")
}

fn params(variant: Variant, missing_codes: Vec<f64>) -> TTestParams {
    TTestParams {
        outcome: Alias(gaply_core::stats_l1::dataset::ColumnAlias(1)),
        group: Alias(gaply_core::stats_l1::dataset::ColumnAlias(2)),
        levels: None,
        variant: Some(variant),
        missing_codes,
    }
}

const ROWS: &[(&str, &str)] = &[
    ("12.25", "a"),
    ("9.5", "a"),
    ("14.125", "a"),
    ("11", "a"),
    ("10.75", "a"),
    ("15.5", "b"),
    ("13.25", "b"),
    ("19", "b"),
    ("16.75", "b"),
    ("21.5", "b"),
    ("12", "b"),
];

/// Rows left out as missing_outcome (empty y) and missing_code (-99).
const WITH_EXCLUSIONS: &[(&str, &str)] = &[
    ("12.25", "a"),
    ("", "a"),
    ("9.5", "a"),
    ("-99", "a"),
    ("14.125", "a"),
    ("15.5", "b"),
    ("-99", "b"),
    ("13.25", "b"),
    ("19", "b"),
    ("", "b"),
];

#[test]
fn the_independent_t_tests_canonical_bytes_are_as_before_8d1() {
    let cases = [
        ("student", ROWS, params(Variant::Student, vec![]), STUDENT),
        ("welch", ROWS, params(Variant::Welch, vec![]), WELCH),
        (
            "exclusions",
            WITH_EXCLUSIONS,
            params(Variant::Welch, vec![-99.0]),
            EXCLUSIONS,
        ),
    ];
    let mut changed = Vec::new();
    for (name, rows, p, pinned) in cases {
        let r = compute(&dataset(rows), &p).unwrap_or_else(|e| panic!("{name}: {e:?}"));
        if name == "exclusions" {
            let codes: Vec<TTestCode> = r.exclusions.iter().map(|e| e.code).collect();
            assert_eq!(
                codes,
                [TTestCode::MissingOutcome, TTestCode::MissingCode],
                "two reasons"
            );
        }
        let got = ContentHash::of_bytes(&r.canonical_bytes());
        println!("{name}: {}", got.as_hex());
        if got.as_hex() != pinned {
            changed.push(name);
        }
    }
    assert!(
        changed.is_empty(),
        "the canonical bytes changed (captured on macOS aarch64): {changed:?}"
    );
}
