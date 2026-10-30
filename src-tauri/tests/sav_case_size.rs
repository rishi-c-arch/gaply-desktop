//! **A `.sav` that crashed ambers is refused with no panic (Research OS,
//! Phase 8F-2, decision 291).** ambers 0.4.4 divides by the header's nominal
//! case size × 8 on each of its three read paths, so a file with no
//! variables, or with variables and a case size of 0 or less (which PSPP's
//! "System File Format" chapter says some software writes), panicked under
//! every caller. `supplementary`'s check reads the header's layout code and
//! case size and the first record's type, in the file's own byte order:
//!
//! * the dictionary's end right after the header → an empty SPSS table, as a
//!   file with no columns gives;
//! * otherwise a case size of 0 or less → malformed;
//! * otherwise the file goes to ambers as before.
//!
//! Here: the reviewer agent's supplementary files
//! (`parse_supplementary_with` under `SUPPLEMENTARY`, as `commands.rs`'s
//! `run_publishready_measured` calls it), the Stats Verifier's legacy lane
//! (`commands::run_stats_preview`, since `load_first_table` is private), the
//! methods' open, and a proper file reading as before little-endian and
//! refused big-endian (since 8F-3, decision 295; `sav_big_endian.rs`).
//! Level 1's open and compute are in `stats_l1_commands.rs`, the
//! reproduction in `stats_l1_reproduction.rs`, the rerun in
//! `project_rerun.rs`. Each catches a panic so that every file is reported.

mod desc_support;
mod sav_support;

use std::panic::{catch_unwind, AssertUnwindSafe};

use app_lib::commands::run_stats_preview;
use app_lib::runs::DatasetSlots;
use app_lib::stats_desc_commands::open_with;
use app_lib::stats_l1_ingest::load_dataset_from_bytes_with;
use app_lib::supplementary::{parse_supplementary_with, IngestLimits};
use desc_support::TempDir;
use gaply_core::research_project::DataFormat;
use gaply_core::GaplyError;
use sav_support::{crashing, expected_refusal, proper, Order};

/// Runs `f` on each crashing file, written under `dir`, and fails naming
/// every file that panicked or gave something other than `want(name)`.
fn each<T: std::fmt::Debug>(
    dir: &TempDir,
    f: impl Fn(&std::path::Path) -> T,
    ok: impl Fn(&str, &T) -> bool,
) {
    let mut wrong = Vec::new();
    for (name, bytes) in crashing() {
        let p = dir.write(&format!("{name}.sav"), &bytes);
        match catch_unwind(AssertUnwindSafe(|| f(&p))) {
            Err(_) => wrong.push(format!("{name}: panicked")),
            Ok(got) if !ok(name, &got) => wrong.push(format!("{name}: {got:?}")),
            Ok(_) => {}
        }
    }
    assert!(wrong.is_empty(), "{wrong:#?}");
}

fn malformed(e: &GaplyError) -> bool {
    matches!(e, GaplyError::Validation(m) if m.starts_with("malformed or unsupported .sav file"))
}

#[test]
fn the_reviewer_agents_files_give_an_empty_table_or_malformed_with_no_panic() {
    let dir = TempDir::new("sav_reviewer");
    each(
        &dir,
        |p| parse_supplementary_with(p, IngestLimits::SUPPLEMENTARY),
        |name, got| match got {
            Ok(ev) => {
                name == "no_variables"
                    && ev.tables.len() == 1
                    && ev.tables[0].headers.is_empty()
                    && ev.tables[0].rows.is_empty()
            }
            Err(e) => name != "no_variables" && malformed(e),
        },
    );
}

#[test]
fn the_legacy_lane_gives_an_empty_preview_or_malformed_with_no_panic() {
    let dir = TempDir::new("sav_legacy");
    each(
        &dir,
        |p| run_stats_preview(p.to_string_lossy().into_owned(), None),
        |name, got| match got {
            Ok(v) => name == "no_variables" && v.headers.is_empty() && v.row_count == 0,
            Err(e) => name != "no_variables" && malformed(e),
        },
    );
}

#[test]
fn the_methods_open_gives_no_columns_or_malformed_with_no_panic() {
    let dir = TempDir::new("sav_methods");
    each(
        &dir,
        |p| {
            open_with(
                &DatasetSlots::default(),
                p,
                None,
                IngestLimits::this_computer(),
            )
            .map(|v| v.profile.rows)
            .map_err(|e| match e {
                GaplyError::Validation(m) => serde_json::from_str::<serde_json::Value>(&m).unwrap(),
                other => panic!("not a validation error: {other:?}"),
            })
        },
        |name, got| got.as_ref().err() == Some(&expected_refusal(name)),
    );
}

/// A proper file reads as before little-endian, and is refused big-endian
/// since 8F-3 (decision 295). Until 8F-3 the big-endian half pinned ambers
/// 0.4.4's wrong reading (each value its big-endian bytes read the other way
/// round, 1.5 as 3.13984e-319; D303's finding), under the name
/// `a_proper_sav_reads_as_before_in_either_byte_order`.
#[test]
fn a_proper_sav_reads_as_before_little_endian_and_is_refused_big_endian() {
    let dir = TempDir::new("sav_proper");
    let bytes = proper(Order::Little);
    let p = dir.write("le.sav", &bytes);
    let ev = parse_supplementary_with(&p, IngestLimits::SUPPLEMENTARY)
        .unwrap_or_else(|e| panic!("{e:?}"));
    let t = &ev.tables[0];
    assert_eq!(t.headers, ["X", "Y"]);
    let numbers: Vec<Vec<f64>> = t
        .rows
        .iter()
        .map(|r| r.iter().map(|c| c.parse::<f64>().unwrap()).collect())
        .collect();
    assert_eq!(numbers, [[1.5, 2.25], [3.0, 4.75]]);
    let ds = load_dataset_from_bytes_with(&bytes, DataFormat::Sav, IngestLimits::LEVEL_1)
        .unwrap_or_else(|r| panic!("{r:?}"));
    assert_eq!((ds.rows(), ds.schema.columns.len()), (2, 2));
    let view = open_with(
        &DatasetSlots::default(),
        &p,
        None,
        IngestLimits::this_computer(),
    )
    .unwrap_or_else(|e| panic!("{e:?}"));
    assert_eq!(view.profile.rows, 2);

    let bytes = proper(Order::Big);
    let p = dir.write("be.sav", &bytes);
    let e = parse_supplementary_with(&p, IngestLimits::SUPPLEMENTARY)
        .err()
        .expect("refused");
    assert!(malformed(&e), "{e:?}");
    assert_eq!(
        load_dataset_from_bytes_with(&bytes, DataFormat::Sav, IngestLimits::LEVEL_1).err(),
        Some(gaply_core::stats_l1::dataset::L1IngestRefusal::SavBigEndian)
    );
    let e = open_with(
        &DatasetSlots::default(),
        &p,
        None,
        IngestLimits::this_computer(),
    )
    .err()
    .expect("refused");
    assert!(
        matches!(&e, GaplyError::Validation(m) if m == r#"{"code":"sav_big_endian"}"#),
        "{e:?}"
    );
}
