//! **What ambers 0.4.6 reads that 0.4.4 did not (Research OS, Phase 8F-4,
//! decisions 305 to 307).** Each file is hand-made by `sav_support` and read
//! through the reviewer agent's files (`parse_supplementary_with` under
//! `SUPPLEMENTARY`), the legacy lane (`commands::run_stats_preview`), the
//! methods' open (`open_with`, by path) and Level 1's ingest from bytes
//! (`load_dataset_from_bytes_with`, which the methods' rerun and the bytes
//! entry share). Level 1's open and compute are in `stats_l1_commands.rs`,
//! the reproduction in `stats_l1_reproduction.rs`, the rerun in
//! `project_rerun.rs`. Each caller runs inside `catch_unwind`, so a panic is
//! reported as one.
//!
//! Decision 306: a file with variables and a case size of 0 or -1 reads
//! exactly what the same file with its proper case size reads, on each of
//! ambers' three read paths, and a big-endian one is still refused as
//! big-endian. Decision 307: the cases 0.4.5 and 0.4.6 fix. Two files are
//! never read under ambers 0.4.4, where one reads past a buffer and the
//! other can ask for gigabytes; their tests stop first if `Cargo.lock`
//! names a version before 0.4.5. ambers' findings for these files are in
//! `sav_ambers_warnings.rs`, which needs 0.4.6's `SpssMetadata::warnings`.

mod desc_support;
mod sav_support;

use std::panic::{catch_unwind, AssertUnwindSafe};
use std::time::Instant;

use app_lib::commands::run_stats_preview;
use app_lib::runs::DatasetSlots;
use app_lib::stats_desc_commands::open_with;
use app_lib::stats_l1_ingest::load_dataset_from_bytes_with;
use app_lib::supplementary::{parse_supplementary_with, IngestLimits, PARSER_VERSION};
use desc_support::TempDir;
use gaply_core::research_project::DataFormat;
use gaply_core::stats_l1::dataset::{Cell, Dataset, L1IngestRefusal, MissingRule};
use gaply_core::GaplyError;
use sav_support::{
    big_endian_non_positive, fixes, locked_ambers_version, non_positive_case_size,
    require_ambers_0_4_5_or_later, Want,
};

/// What one caller gave.
#[derive(Debug, Clone, PartialEq)]
enum Got {
    /// Headers and cells, row by row.
    Table(Vec<String>, Vec<Vec<String>>),
    /// Headers and a row count (the legacy lane), or a row count alone (the
    /// methods' open, with no headers).
    Count(Vec<String>, u64),
    Refused(String),
    Panicked,
}

fn caught<T>(f: impl FnOnce() -> T) -> Result<T, ()> {
    catch_unwind(AssertUnwindSafe(f)).map_err(|_| ())
}

fn refusal(e: &GaplyError) -> String {
    match e {
        GaplyError::Validation(m) if m.starts_with("malformed or unsupported .sav file") => {
            "malformed".into()
        }
        GaplyError::Validation(m) => m.clone(),
        other => format!("{other:?}"),
    }
}

fn level_1_rows(d: &Dataset) -> Got {
    let cols: Vec<&[Cell]> = d
        .schema
        .columns
        .iter()
        .map(|c| d.column_cells(c.alias).unwrap())
        .collect();
    let rows = (0..d.rows())
        .map(|r| {
            cols.iter()
                .map(|c| match &c[r] {
                    Cell::Missing => String::new(),
                    Cell::Number(v) => v.to_string(),
                    Cell::Text(t) => t.clone(),
                })
                .collect()
        })
        .collect();
    Got::Table(
        d.schema.columns.iter().map(|c| c.header.clone()).collect(),
        rows,
    )
}

/// The four callers' readings of `bytes`, written to `dir` as `name.sav`.
fn readings(dir: &TempDir, name: &str, bytes: &[u8]) -> [(&'static str, Got); 4] {
    let p = dir.write(&format!("{name}.sav"), bytes);
    let reviewer = match caught(|| parse_supplementary_with(&p, IngestLimits::SUPPLEMENTARY)) {
        Err(()) => Got::Panicked,
        Ok(Err(e)) => Got::Refused(refusal(&e)),
        Ok(Ok(ev)) => Got::Table(ev.tables[0].headers.clone(), ev.tables[0].rows.clone()),
    };
    let legacy = match caught(|| run_stats_preview(p.to_string_lossy().into_owned(), None)) {
        Err(()) => Got::Panicked,
        Ok(Err(e)) => Got::Refused(refusal(&e)),
        Ok(Ok(v)) => Got::Count(v.headers, v.row_count as u64),
    };
    let methods = match caught(|| {
        open_with(
            &DatasetSlots::default(),
            &p,
            None,
            IngestLimits::this_computer(),
        )
    }) {
        Err(()) => Got::Panicked,
        Ok(Err(GaplyError::Validation(m))) => Got::Refused(m),
        Ok(Err(e)) => Got::Refused(format!("{e:?}")),
        Ok(Ok(v)) => Got::Count(Vec::new(), v.profile.rows),
    };
    let level_1 = match caught(|| {
        load_dataset_from_bytes_with(bytes, DataFormat::Sav, IngestLimits::LEVEL_1)
    }) {
        Err(()) => Got::Panicked,
        Ok(Err(r)) => Got::Refused(format!("{r:?}")),
        Ok(Ok(d)) => level_1_rows(&d),
    };
    [
        ("reviewer", reviewer),
        ("legacy", legacy),
        ("methods", methods),
        ("level 1", level_1),
    ]
}

/// Two cells are the same when both are numbers of one value, or the same
/// text.
fn same_cell(a: &str, b: &str) -> bool {
    match (a.parse::<f64>(), b.parse::<f64>()) {
        (Ok(x), Ok(y)) => x == y,
        _ => a == b,
    }
}

fn same_table(got: &[Vec<String>], want: &[Vec<String>]) -> bool {
    got.len() == want.len()
        && got
            .iter()
            .zip(want)
            .all(|(g, w)| g.len() == w.len() && g.iter().zip(w).all(|(a, b)| same_cell(a, b)))
}

/// Each caller's reading against `want`, as a list of what was wrong.
fn check(dir: &TempDir, name: &str, bytes: &[u8], want: &Want) -> Vec<String> {
    let mut wrong = Vec::new();
    for (caller, got) in readings(dir, name, bytes) {
        let ok = match (want, &got) {
            (
                Want::Reads {
                    headers,
                    rows,
                    missing,
                },
                Got::Table(h, r),
            ) => {
                let mut rows = rows.clone();
                if caller == "level 1" {
                    for &(i, j) in missing {
                        rows[i][j] = String::new();
                    }
                }
                h == headers && same_table(r, &rows)
            }
            (Want::Reads { headers, rows, .. }, Got::Count(h, n)) => {
                (h.is_empty() || h == headers) && *n == rows.len() as u64
            }
            (Want::Malformed, Got::Refused(m)) => match caller {
                "methods" => m == r#"{"code":"malformed"}"#,
                "level 1" => m == "Malformed",
                _ => m == "malformed",
            },
            _ => false,
        };
        if !ok {
            wrong.push(format!("{name}, {caller}: {got:?}"));
        }
    }
    wrong
}

/// Decision 307's files named, through every caller here.
fn through_every_caller(names: &[&str]) {
    let dir = TempDir::new("sav_fixes");
    let all = fixes();
    let mut wrong = Vec::new();
    for name in names {
        let (_, bytes, want, safe) = all
            .iter()
            .find(|f| f.0 == *name)
            .unwrap_or_else(|| panic!("no file {name}"));
        if !safe {
            require_ambers_0_4_5_or_later(name);
        }
        wrong.extend(check(&dir, name, bytes, want));
    }
    assert!(wrong.is_empty(), "{wrong:#?}");
}

fn level_1(name: &str) -> Dataset {
    let (_, bytes, _, _) = fixes().into_iter().find(|f| f.0 == name).unwrap();
    load_dataset_from_bytes_with(&bytes, DataFormat::Sav, IngestLimits::LEVEL_1)
        .unwrap_or_else(|r| panic!("{name}: {r:?}"))
}

/* ---- decision 305 --------------------------------------------------------- */

/// `PARSER_VERSION` enters every Level 1 fingerprint, and its doc says to
/// bump it when a parse of the same bytes could change: it must name the
/// ambers that `Cargo.lock` holds, so an upgrade cannot leave it stale.
#[test]
fn parser_version_names_the_ambers_version_in_cargo_lock() {
    let v = locked_ambers_version();
    assert!(
        PARSER_VERSION.contains(&format!("ambers {v},")),
        "PARSER_VERSION {PARSER_VERSION:?} does not name ambers {v}"
    );
    assert_eq!(PARSER_VERSION.matches("ambers ").count(), 1);
}

/* ---- decision 306 --------------------------------------------------------- */

/// Straight through ambers, as both of supplementary's calls read: the batch
/// of a file with a case size of 0 or -1 equals the batch of the same file
/// with its proper case size, uncompressed, bytecode and zlib.
#[test]
fn a_case_size_of_0_or_minus_1_reads_exactly_on_each_of_ambers_read_paths() {
    let mut wrong = Vec::new();
    for (name, bytes, proper) in non_positive_case_size() {
        let want = ambers::read_sav_from_reader(std::io::Cursor::new(proper))
            .unwrap()
            .0;
        match caught(|| ambers::read_sav_from_reader(std::io::Cursor::new(bytes))) {
            Err(()) => wrong.push(format!("{name}: panicked")),
            Ok(Err(e)) => wrong.push(format!("{name}: {e}")),
            Ok(Ok((batch, _))) if batch != want => wrong.push(format!("{name}: {batch:?}")),
            Ok(Ok(_)) => {}
        }
    }
    assert!(wrong.is_empty(), "{wrong:#?}");
}

/// And through every caller here: each reads what it reads from the file
/// with its proper case size, with no panic.
#[test]
fn a_case_size_of_0_or_minus_1_reads_exactly_through_every_caller() {
    let dir = TempDir::new("sav_sized");
    let mut wrong = Vec::new();
    for (name, bytes, proper) in non_positive_case_size() {
        let want = readings(&dir, &format!("{name}_proper"), &proper);
        assert!(
            want.iter()
                .all(|(_, g)| !matches!(g, Got::Refused(_) | Got::Panicked)),
            "{name}: {want:?}"
        );
        for ((caller, got), (_, w)) in readings(&dir, &name, &bytes).into_iter().zip(want) {
            if got != w {
                wrong.push(format!("{name}, {caller}: {got:?}, not {w:?}"));
            }
        }
    }
    assert!(wrong.is_empty(), "{wrong:#?}");
}

/// A big-endian file with variables and a case size of 0 or -1 is refused
/// as big-endian, uncompressed or compressed: the methods' open and Level 1
/// give `sav_big_endian`, the reviewer agent's files and the legacy lane
/// malformed.
#[test]
fn a_big_endian_file_of_case_size_0_or_minus_1_is_refused_as_big_endian() {
    let dir = TempDir::new("sav_be_sized");
    let mut wrong = Vec::new();
    for (name, bytes) in big_endian_non_positive() {
        let got = readings(&dir, &name, &bytes);
        let want = [
            Got::Refused("malformed".into()),
            Got::Refused("malformed".into()),
            Got::Refused(r#"{"code":"sav_big_endian"}"#.into()),
            Got::Refused(format!("{:?}", L1IngestRefusal::SavBigEndian)),
        ];
        for ((caller, g), w) in got.into_iter().zip(want) {
            if g != w {
                wrong.push(format!("{name}, {caller}: {g:?}"));
            }
        }
    }
    assert!(wrong.is_empty(), "{wrong:#?}");
}

/* ---- decision 307 --------------------------------------------------------- */

/// A header declaring 0 cases with five rows behind it. Under 0.4.4 the
/// uncompressed file read no rows and the bytecode one panicked at a debug
/// assertion before an unsafe write (in a release build, a write past a
/// buffer); 0.4.6 reads the five rows (issue #2).
#[test]
fn a_header_of_0_cases_with_rows_behind_it_reads_the_rows_present() {
    through_every_caller(&["zero_cases_uncompressed", "zero_cases_bytecode"]);
}

/// A positive header case size larger than the variable list: 0.4.4 read
/// rows of the header's width, so its columns were other cells; 0.4.6 reads
/// the variable list's width (issue #1).
#[test]
fn a_case_size_larger_than_the_variable_list_reads_from_the_variable_list() {
    through_every_caller(&["case_size_larger_uncompressed", "case_size_larger_bytecode"]);
}

/// Smaller than the variable list: under 0.4.4 an unchecked pointer read
/// past the buffer, undefined behaviour, so this is never run there.
#[test]
fn a_case_size_smaller_than_the_variable_list_reads_from_the_variable_list() {
    through_every_caller(&[
        "case_size_smaller_uncompressed",
        "case_size_smaller_bytecode",
    ]);
}

/// A date variable holding 2026-01-01, 12000-01-01 and 1e300 seconds: 0.4.6
/// sets the two outside years 1 to 9999 to null, which Gaply reads as
/// missing. Under 0.4.4 they were cell text, "+12000-01-01" and arrow's
/// cast error, a level on screen.
#[test]
fn a_date_outside_years_1_to_9999_is_missing() {
    through_every_caller(&["date_out_of_range"]);
    let d = level_1("date_out_of_range");
    let dates = d.column_cells(d.schema.columns[0].alias).unwrap();
    assert_eq!(
        dates.iter().map(Cell::is_missing).collect::<Vec<_>>(),
        [false, true, true]
    );
}

/// A windows-1252 file: the variable name CAFÉ and NOM's string missing
/// value "NÉANT" decode in the file's encoding under 0.4.6 (issue #5), so
/// the missing value applies to the cell that holds it.
#[test]
fn a_code_page_files_names_and_string_missing_value_read_in_its_encoding() {
    through_every_caller(&["code_page"]);
    let ev = code_page_evidence();
    let nom = &ev.variables[1];
    assert!(
        matches!(&nom.user_missing_rules[..], [MissingRule::Text(t)] if t.trim_end() == "NÉANT"),
        "{:?}",
        nom.user_missing
    );
    let d = level_1("code_page");
    let noms = d.column_cells(d.schema.columns[1].alias).unwrap();
    assert_eq!(
        noms.iter().map(Cell::is_missing).collect::<Vec<_>>(),
        [false, true],
        "NÉANT is declared missing"
    );
}

/// The same file's variable label and value label, which decoded in the
/// file's encoding under 0.4.4 already: green under both.
#[test]
fn a_code_page_files_variable_and_value_labels_read_in_its_encoding() {
    let ev = code_page_evidence();
    let cafe = &ev.variables[0];
    assert_eq!(cafe.label.as_deref(), Some("Café crème"));
    assert_eq!(
        cafe.value_label_codes,
        [("1".to_string(), "été".to_string())]
    );
}

fn code_page_evidence() -> app_lib::supplementary::SupplementaryEvidence {
    let dir = TempDir::new("sav_code_page");
    let p = dir.write("code_page.sav", &sav_support::code_page().bytes());
    parse_supplementary_with(&p, IngestLimits::SUPPLEMENTARY).unwrap()
}

/// Two variables named X: 0.4.4 read both; 0.4.6 refuses the file, as SPSS
/// does.
#[test]
fn duplicate_variable_names_are_refused_as_malformed() {
    through_every_caller(&["duplicate_names"]);
}

/// A variable record whose type field is 256, no type SPSS defines: 0.4.4
/// read it; 0.4.6 refuses it (issue #4).
#[test]
fn a_type_field_of_256_is_refused_as_malformed() {
    through_every_caller(&["type_field_256"]);
}

/// A value-label record declaring a billion labels, and an extension record
/// declaring 1.3 GB, in a file of a few hundred bytes: refused by every
/// caller within a second (issue #3). Never run under 0.4.4, which could
/// ask this computer for gigabytes.
#[test]
fn sizes_declared_far_beyond_the_file_are_refused_within_a_second() {
    for name in ["label_count_oversized", "info_record_oversized"] {
        let t = Instant::now();
        through_every_caller(&[name]);
        assert!(
            t.elapsed().as_secs_f64() < 1.0,
            "{name} took {:?}",
            t.elapsed()
        );
    }
}

/// Measured, not a fix: a type field of -2 between two numeric variables.
/// 0.4.6 treats it as a string continuation slot with a finding and reads
/// the two numbers either side (its CHANGELOG, issue #4: SPSS opens such
/// files), so it is not refused.
#[test]
fn a_type_field_of_minus_2_is_read_as_a_continuation_slot() {
    let dir = TempDir::new("sav_minus_2");
    let want = Want::Reads {
        headers: vec!["X".into(), "Z".into()],
        rows: vec![vec!["1".into(), "2".into()], vec!["3".into(), "4".into()]],
        missing: Vec::new(),
    };
    let wrong = check(
        &dir,
        "type_field_minus_2",
        &sav_support::type_field_minus_2(),
        &want,
    );
    assert!(wrong.is_empty(), "{wrong:#?}");
}
