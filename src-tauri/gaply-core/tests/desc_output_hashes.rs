//! **Three output hashes, pinned, so another platform's bytes show as red
//! (Research OS decision 15, Phase 2B).**
//!
//! "Bitwise" in a run manifest means the same inputs, engine version and
//! platform give the same bytes. Whether OTHER platforms give the same bytes
//! is not claimed: this test asks them. It runs in every job that runs
//! `cargo test -p gaply_core` or `--workspace` (the clean-checkout job on
//! Linux, the Windows build check, and this machine, macOS aarch64), and each
//! one that computes different bytes for these three results goes red.
//!
//! The three are cases of Phase 2A's R golden set (`desc_r_golden.rs`), the
//! numeric ones most sensitive to summation order and rounding: a mean near
//! 1e9 with sd 10 (`offset_1e9`), 5,000 rows (`large_5000`, the worst sd
//! ratio against R) and a heavy tail (`heavy_tail`). R checks the values;
//! this test checks only that the bytes repeat. The hash is the manifest's
//! `outputHash`, SHA-256 of `DescribeResult::canonical_bytes`.
//!
//! Captured on macOS aarch64 (Rust 1.97.0), 17 Oct 2026. If a platform
//! disagrees, record which and how before changing a pin: the claim to fix
//! may be `Bitwise`, not the hash.

use gaply_core::contracts::{Arch, ContentHash, OperationId, Os, Platform};
use gaply_core::stats_desc::{
    describe, manifest, spec, Alias, ColumnRequest, DescribeParams, TreatAs,
};
use gaply_core::stats_l1::dataset::{build_dataset, DataFingerprint, Dataset, RawTable, TableRef};

const DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/stats_desc");

/// (case, data file, column, the pinned output hash).
const PINS: &[(&str, &str, &str, &str)] = &[
    (
        "offset_1e9",
        "data_offset.csv",
        "x",
        "5fd0af5daee4d8dfcb466411096e7f26e513bf2cadfb0e1605ff085026fe5b4a",
    ),
    (
        "large_5000",
        "data_large.csv",
        "x",
        "15d71b773fbf156d921981c637d3443c688ffbd8cf547a496bfdc939e68e7acb",
    ),
    (
        "heavy_tail",
        "data_heavy_tail.csv",
        "x",
        "b3b2edbf2730fc85a681bdc7fae1f78efe9167e5afc024e9f5318bb5ae123ed4",
    ),
];

/// The data file as `desc_r_golden.rs` reads it: '#' lines dropped, then a
/// CSV with no quoted fields.
fn data(name: &str) -> Dataset {
    let bytes = std::fs::read(format!("{DIR}/{name}")).expect("data file");
    let text = String::from_utf8(bytes.clone()).expect("utf-8");
    let mut lines = text.lines().filter(|l| !l.starts_with('#'));
    let headers: Vec<String> = lines
        .next()
        .expect("header")
        .split(',')
        .map(str::to_string)
        .collect();
    let rows: Vec<Vec<String>> = lines
        .map(|l| l.split(',').map(str::to_string).collect())
        .collect();
    build_dataset(
        RawTable::from_rows(headers, rows),
        DataFingerprint::of_bytes(&bytes, "golden csv", TableRef::FirstTable),
    )
    .expect("dataset")
}

fn output_hash(file: &str, column: &str) -> String {
    let dataset = data(file);
    let alias = dataset
        .schema
        .columns
        .iter()
        .find(|c| c.header == column)
        .map(|c| Alias(c.alias))
        .expect("column");
    let params = DescribeParams {
        columns: vec![ColumnRequest {
            alias,
            treat_as: TreatAs::Numeric,
        }],
        missing_codes: Vec::new(),
    };
    let result = describe(&dataset, &params).expect("describe");
    let spec = spec(OperationId(1), &dataset, params);
    // The platform is a field of the manifest, not of the output, so a fixed
    // one keeps the output hash about the numbers only.
    let platform = Platform {
        os: Os::Macos,
        arch: Arch::Aarch64,
    };
    let m = manifest(&spec, &dataset, &result, 0, platform);
    assert_eq!(
        m.output_hash,
        ContentHash::of_bytes(&result.canonical_bytes())
    );
    m.output_hash.as_hex().to_string()
}

#[test]
fn three_golden_results_hash_to_the_pinned_bytes_on_this_platform() {
    let platform = Platform::current();
    let mut wrong = Vec::new();
    for (case, file, column, pinned) in PINS {
        let got = output_hash(file, column);
        println!("{case}: {got} ({platform:?})");
        if got != *pinned {
            wrong.push(format!(
                "{case}: pinned {pinned}, this platform computed {got}"
            ));
        }
    }
    assert!(
        wrong.is_empty(),
        "this platform ({platform:?}) gives other bytes than the pins (macOS aarch64):\n{}",
        wrong.join("\n")
    );
}
