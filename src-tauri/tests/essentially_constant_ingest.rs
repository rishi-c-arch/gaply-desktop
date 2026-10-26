//! **The real-app check's file, through the app's own ingest (Research OS,
//! Phase 8D-1b, decision 259).** `~/gaply-corpus/os/phase8/realapp_8d1b.md`
//! opens a CSV of two groups one ulp apart (`HTNEAR.csv`) on the t-test's
//! screen. That screen reads a file through `stats_desc_open`, which is
//! `load_dataset_keeping_bytes(path, IngestLimits::METHODS)`. `gaply_core`'s
//! tests build their datasets from strings, so they cannot say whether the
//! CSV reader in front of `str::parse::<f64>` keeps a 17-digit cell. This
//! writes the same bytes to a temporary file, reads them through that ingest,
//! checks every value's bits, and runs the t-test, which must refuse with
//! `groups_essentially_constant` under Student and Welch.

mod desc_support;

use app_lib::stats_l1_ingest::load_dataset_keeping_bytes;
use app_lib::supplementary::IngestLimits;
use gaply_core::stats_desc::Alias;
use gaply_core::stats_l1::dataset::ColumnAlias;
use gaply_core::stats_ttest::{compute, TTestCode, TTestParams, Variant};

/// `HTNEAR.csv`, byte for byte as the checklist's one-liner writes it.
const HTNEAR: &[u8] =
    b"x,group\n1.0,A\n1.0000000000000002,A\n1.0,A\n2.0,B\n2.0000000000000004,B\n2.0,B\n";

#[test]
fn the_checklists_file_is_read_to_the_bit_and_refused_as_essentially_constant() {
    let dir = desc_support::TempDir::new("8d1b_htnear");
    let path = dir.write("HTNEAR.csv", HTNEAR);
    let (dataset, bytes, ext) =
        load_dataset_keeping_bytes(&path, IngestLimits::METHODS).expect("the ingest reads it");
    assert_eq!(bytes, HTNEAR);
    assert_eq!(ext, "csv");

    let x: Vec<u64> = dataset
        .column_cells(ColumnAlias(1))
        .expect("x")
        .iter()
        .map(|c| c.number().expect("a number").to_bits())
        .collect();
    let want: Vec<u64> = [
        1.0,
        1.0 + f64::EPSILON,
        1.0,
        2.0,
        2.0 + 2.0 * f64::EPSILON,
        2.0,
    ]
    .iter()
    .map(|v: &f64| v.to_bits())
    .collect();
    assert_eq!(x, want, "every cell read as the double its text names");

    for variant in [Variant::Student, Variant::Welch] {
        let params = TTestParams {
            outcome: Alias(ColumnAlias(1)),
            group: Alias(ColumnAlias(2)),
            levels: None,
            variant: Some(variant),
            missing_codes: vec![],
        };
        match compute(&dataset, &params) {
            Ok(r) => panic!("{variant:?}: computed t {:e}", r.t),
            Err(e) => assert_eq!(e.code, TTestCode::GroupsEssentiallyConstant, "{variant:?}"),
        }
    }
}
