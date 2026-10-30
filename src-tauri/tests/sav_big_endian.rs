//! **A big-endian `.sav` is refused (Research OS, Phase 8F-3, decision
//! 295).** ambers 0.4.4 reads a big-endian file's header and dictionary in
//! the file's byte order, but an uncompressed file's values and a compressed
//! file's raw values (after bytecode 253) always little-endian, so 1.5 read
//! as 3.13984e-319, with no warning. A compressed file's whole numbers from
//! -99 to 151 come from a table built from the bias and read rightly, but
//! nothing in the header says which values took which path, so every
//! big-endian file is refused before ambers, compressed or not, those that
//! read rightly today included.
//!
//! Here, for each of `sav_support::same_values`' three files: the methods'
//! open gives `sav_big_endian`; the reviewer agent's files
//! (`parse_supplementary_with` under `SUPPLEMENTARY`) and the legacy lane
//! (`commands::run_stats_preview`) give supplementary's malformed error; and
//! the same files little-endian read their values exactly, under all three.
//! Level 1's open and compute are in `stats_l1_commands.rs`, the
//! reproduction in `stats_l1_reproduction.rs`, the rerun in
//! `project_rerun.rs`.

mod desc_support;
mod sav_support;

use app_lib::commands::run_stats_preview;
use app_lib::runs::DatasetSlots;
use app_lib::stats_desc_commands::open_with;
use app_lib::stats_l1_ingest::load_dataset_from_bytes_with;
use app_lib::supplementary::{parse_supplementary_with, IngestLimits};
use desc_support::TempDir;
use gaply_core::research_project::DataFormat;
use gaply_core::stats_l1::dataset::L1IngestRefusal;
use gaply_core::GaplyError;
use sav_support::{same_values, Order};

fn malformed(e: &GaplyError) -> bool {
    matches!(e, GaplyError::Validation(m) if m.starts_with("malformed or unsupported .sav file"))
}

fn numbers(rows: &[Vec<String>]) -> Vec<f64> {
    rows.iter()
        .flatten()
        .map(|c| c.parse::<f64>().unwrap_or(f64::NAN))
        .collect()
}

#[test]
fn the_methods_open_refuses_a_big_endian_sav_as_sav_big_endian() {
    let dir = TempDir::new("be_methods");
    let mut wrong = Vec::new();
    for (name, bytes, _) in same_values(Order::Big) {
        let p = dir.write(&format!("{name}.sav"), &bytes);
        match open_with(
            &DatasetSlots::default(),
            &p,
            None,
            IngestLimits::this_computer(),
        ) {
            Ok(v) => wrong.push(format!("{name}: opened, {} rows", v.profile.rows)),
            Err(GaplyError::Validation(m)) => {
                let v: serde_json::Value = serde_json::from_str(&m).expect("a closed code");
                if v != serde_json::json!({ "code": "sav_big_endian" }) {
                    wrong.push(format!("{name}: {v}"));
                }
            }
            Err(e) => wrong.push(format!("{name}: {e:?}")),
        }
        match load_dataset_from_bytes_with(&bytes, DataFormat::Sav, IngestLimits::this_computer()) {
            Err(L1IngestRefusal::SavBigEndian) => {}
            other => wrong.push(format!("{name} from bytes: {:?}", other.map(|d| d.rows()))),
        }
    }
    assert!(wrong.is_empty(), "{wrong:#?}");
}

#[test]
fn the_reviewer_agents_files_and_the_legacy_lane_refuse_a_big_endian_sav_as_malformed() {
    let dir = TempDir::new("be_supp");
    let mut wrong = Vec::new();
    for (name, bytes, _) in same_values(Order::Big) {
        let p = dir.write(&format!("{name}.sav"), &bytes);
        match parse_supplementary_with(&p, IngestLimits::SUPPLEMENTARY) {
            Err(e) if malformed(&e) => {}
            Err(e) => wrong.push(format!("reviewer {name}: {e:?}")),
            Ok(ev) => wrong.push(format!(
                "reviewer {name}: read {:?}",
                numbers(&ev.tables[0].rows)
            )),
        }
        match run_stats_preview(p.to_string_lossy().into_owned(), None) {
            Err(e) if malformed(&e) => {}
            Err(e) => wrong.push(format!("legacy {name}: {e:?}")),
            Ok(v) => wrong.push(format!("legacy {name}: read {} rows", v.row_count)),
        }
    }
    assert!(wrong.is_empty(), "{wrong:#?}");
}

#[test]
fn the_same_values_little_endian_read_exactly_under_every_caller() {
    let dir = TempDir::new("le_same");
    for (name, bytes, want) in same_values(Order::Little) {
        let p = dir.write(&format!("{name}.sav"), &bytes);
        let ev = parse_supplementary_with(&p, IngestLimits::SUPPLEMENTARY)
            .unwrap_or_else(|e| panic!("{name}: {e:?}"));
        assert_eq!(ev.tables[0].headers, ["X"], "{name}");
        assert_eq!(numbers(&ev.tables[0].rows), want, "{name}");
        let preview = run_stats_preview(p.to_string_lossy().into_owned(), None)
            .unwrap_or_else(|e| panic!("{name}: {e:?}"));
        assert_eq!(preview.row_count, want.len(), "{name}");
        let ds = load_dataset_from_bytes_with(&bytes, DataFormat::Sav, IngestLimits::LEVEL_1)
            .unwrap_or_else(|r| panic!("{name}: {r:?}"));
        assert_eq!(
            (ds.rows(), ds.schema.columns.len()),
            (want.len(), 1),
            "{name}"
        );
        let view = open_with(
            &DatasetSlots::default(),
            &p,
            None,
            IngestLimits::this_computer(),
        )
        .unwrap_or_else(|e| panic!("{name}: {e:?}"));
        assert_eq!(view.profile.rows, want.len() as u64, "{name}");
    }
}

/// What ambers 0.4.4 reads from the big-endian files, before the check:
/// pinned as the reason for it. Read straight through ambers, as
/// supplementary did before 8F-3: the two files with raw values read each
/// value's bytes the other way round; the whole numbers read rightly.
#[test]
fn ambers_reads_a_big_endian_files_raw_values_byte_swapped() {
    for (name, bytes, want) in same_values(Order::Big) {
        let (batch, _) = ambers::read_sav_from_reader(std::io::Cursor::new(bytes))
            .unwrap_or_else(|e| panic!("{name}: {e:?}"));
        let col = batch
            .column(0)
            .as_any()
            .downcast_ref::<arrow::array::Float64Array>()
            .expect("a numeric column");
        let got: Vec<f64> = col.values().to_vec();
        if name == "bytecode_1_to_5" {
            assert_eq!(got, want, "{name}: the bias' table reads rightly");
        } else {
            let swapped: Vec<f64> = want
                .iter()
                .map(|w| f64::from_le_bytes(w.to_be_bytes()))
                .collect();
            assert_eq!(got, swapped, "{name}: each value its bytes swapped");
        }
    }
}
