//! **The findings ambers 0.4.6 gives for 8F-4's files (Research OS, Phase
//! 8F-4, decision 307).** `SpssMetadata::warnings` is new in 0.4.5: a file
//! that contradicts itself is read from its more reliable part and the
//! contradiction is listed (ambers' `warnings_and_errors.md`). Gaply shows
//! none of them and logs none of them in this step; this pins, for each
//! hand-made file and each tracked fixture, what `read_sav_from_reader`
//! returns (metadata cloned after the read, so data-time findings are in
//! it) or the error it refuses with, so a reader of the report and the next
//! upgrade see the same list.
//!
//! Gaply's own calls read the metadata before the rows (`read_sav_rows_with`
//! clones `scanner.metadata()` first), so its copy carries the header's
//! findings and not the two found while reading (rows read, dates nulled).
//! This file needs ambers 0.4.5 or later to compile.

mod sav_support;

use sav_support::{fixes, non_positive_case_size, type_field_minus_2};

/// What ambers gives for `bytes`: its findings, or its error.
fn findings(bytes: &[u8]) -> Result<Vec<String>, String> {
    ambers::read_sav_from_reader(std::io::Cursor::new(bytes.to_vec()))
        .map(|(_, meta)| meta.warnings)
        .map_err(|e| e.to_string())
}

fn files() -> Vec<(String, Vec<u8>)> {
    let mut v: Vec<(String, Vec<u8>)> = fixes()
        .into_iter()
        .map(|(n, b, _, _)| (n.to_string(), b))
        .collect();
    v.push(("type_field_minus_2".into(), type_field_minus_2()));
    for (n, b, _) in non_positive_case_size() {
        v.push((n, b));
    }
    for f in ["analysis.sav", "l1_weighted.sav"] {
        v.push((
            format!("fixture {f}"),
            std::fs::read(format!("{}/tests/fixtures/{f}", env!("CARGO_MANIFEST_DIR"))).unwrap(),
        ));
    }
    v
}

/// Each file's findings or error, measured under ambers 0.4.6.
const WANT: &[(&str, &str)] = &[
    ("zero_cases_uncompressed", "header declares 0 rows but 5 rows were read"),
    ("zero_cases_bytecode", "header declares 0 rows but 5 rows were read"),
    ("case_size_larger_uncompressed", "header declares 3 values per row but the variable list defines 2; used 2"),
    ("case_size_larger_bytecode", "header declares 3 values per row but the variable list defines 2; used 2"),
    ("case_size_smaller_uncompressed", "header declares 1 values per row but the variable list defines 2; used 2"),
    ("case_size_smaller_bytecode", "header declares 1 values per row but the variable list defines 2; used 2"),
    ("date_out_of_range", "2 date/time values were outside the representable range (years 1-9999) and were set to null"),
    ("code_page", "no findings"),
    ("duplicate_names", "error: invalid dictionary: duplicate variable name \"X\"; SPSS also rejects this file"),
    ("type_field_256", "error: invalid variable record: variable \"Y\" declares type 256; valid values are 0 (numeric), 1-255 (string width) or -1 (continuation)"),
    ("label_count_oversized", "error: I/O error: failed to fill whole buffer"),
    ("info_record_oversized", "error: truncated file: expected 1300000000 bytes, got 303"),
    ("type_field_minus_2", "variable record 1 (\"C\") declares type -2; treated as a continuation slot"),
    ("uncompressed_case_size_0", "header does not declare the number of values per row (case size 0); used the 2 defined by the variable list"),
    ("bytecode_case_size_0", "header does not declare the number of values per row (case size 0); used the 2 defined by the variable list"),
    ("zlib_case_size_0", "header does not declare the number of values per row (case size 0); used the 2 defined by the variable list"),
    ("uncompressed_case_size_-1", "header does not declare the number of values per row (case size -1); used the 2 defined by the variable list"),
    ("bytecode_case_size_-1", "header does not declare the number of values per row (case size -1); used the 2 defined by the variable list"),
    ("zlib_case_size_-1", "header does not declare the number of values per row (case size -1); used the 2 defined by the variable list"),
    ("fixture analysis.sav", "no findings"),
    ("fixture l1_weighted.sav", "no findings"),
];

#[test]
fn ambers_findings_for_each_file_are_as_measured() {
    sav_support::require_ambers_0_4_5_or_later("these files");
    let print = std::env::var("GAPLY_SAV_RECORD").is_ok();
    let mut wrong = Vec::new();
    let all = files();
    for (name, bytes) in &all {
        let got = match findings(bytes) {
            Ok(w) if w.is_empty() => "no findings".to_string(),
            Ok(w) => w.join(" | "),
            Err(e) => format!("error: {e}"),
        };
        if print {
            println!("FINDING\t(\"{name}\", {got:?}),");
            println!(
                "SHA256\t{name}\t{}",
                gaply_core::stats_l1::dataset::sha256_hex(bytes)
            );
        }
        match WANT.iter().find(|w| w.0 == name) {
            Some((_, w)) if *w == got => {}
            _ => wrong.push(format!("{name}: {got}")),
        }
    }
    assert_eq!(all.len(), WANT.len(), "one entry per file");
    assert!(wrong.is_empty(), "{wrong:#?}");
}
