//! **The ingest applies the limits it is given (Research OS, Phase 8B,
//! decision 243).** The ingest takes its limits as a value,
//! `supplementary::IngestLimits`, and three named values exist, each with
//! today's numbers: `LEVEL_1` (Level 1's open and compute, and
//! `stats_reproduction`), `SUPPLEMENTARY` (the reviewer agent's supplementary
//! files and the Stats Verifier page's legacy lane) and `METHODS` (the
//! descriptive summary, the t-test and their reruns).
//!
//! * each value's fields are today's constants, and the registry's resource
//!   profiles agree with the value their capabilities read through;
//! * for `LEVEL_1` and `METHODS`, through the forms of the ingest their
//!   callers use, a file exactly at each limit opens and one over it gets the
//!   limit's own refusal, with the limit from the value;
//! * for `SUPPLEMENTARY`, through `parse_supplementary_with`, the same files
//!   parse, and one over each gives today's message exactly;
//! * once per field, a value built here, smaller than the constant, is the
//!   one the ingest and the parser apply, and the classifier that tells a
//!   failed parse apart gives the same code.
//!
//! * a `.sav` or `.mat` holding more variables than the column limit is
//!   refused as too wide under `LEVEL_1` and `METHODS`, as a CSV is, and
//!   `SUPPLEMENTARY` keeps its first `max_cols` with the note (Phase 8B-2,
//!   decision 246).
//!
//! The source guard that each production caller passes its named value is
//! `tests/ingest_limits_guard.rs`. Every file here is synthetic and written
//! into a directory this test creates and removes.

mod desc_support;

use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde_json::json;

use app_lib::stats_l1_ingest::{
    declared_uncompressed_total, load_dataset_from_bytes, load_dataset_from_bytes_with,
    load_dataset_keeping_bytes, load_dataset_with, L1IngestRefusal,
    MAX_WORKBOOK_UNCOMPRESSED_BYTES,
};
use app_lib::supplementary::{
    exceeds_col_cap_bytes, exceeds_col_cap_with, exceeds_content_cap_bytes, exceeds_row_cap_bytes,
    exceeds_row_cap_with, parse_supplementary_bytes_with, parse_supplementary_with, IngestLimits,
    MAX_COLS, MAX_FILE_SIZE_BYTES, MAX_ROWS, MAX_TOTAL_CHARS,
};
use gaply_core::research_project::DataFormat;
use gaply_core::stats_l1::dataset::{DatasetRefusal, MAX_COLUMNS};
use gaply_core::stats_registry::{Kind, Registry};
use gaply_core::{Database, GaplyError};

/// A directory of this test's own, removed when the value is dropped.
struct Scratch(PathBuf);

impl Scratch {
    fn new(tag: &str) -> Self {
        let d = std::env::temp_dir().join(format!(
            "gaply_8b_ingest_limits_{tag}_{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        Scratch(d)
    }

    fn write(&self, name: &str, bytes: &[u8]) -> PathBuf {
        let p = self.0.join(name);
        std::fs::write(&p, bytes).unwrap();
        p
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

/// A header and `rows` data rows of two numeric columns.
fn rows_csv(rows: usize) -> String {
    let mut s = String::from("a,b\n");
    for i in 0..rows {
        s.push_str(&format!("{i},{}\n", i % 7));
    }
    s
}

/// A header of `cols` names and two data rows of numbers.
fn wide_csv(cols: usize) -> String {
    let line = |f: &dyn Fn(usize) -> String| (1..=cols).map(f).collect::<Vec<_>>().join(",");
    format!(
        "{}\n{}\n{}\n",
        line(&|c| format!("c{c}")),
        line(&|c| c.to_string()),
        line(&|c| (c * 2).to_string())
    )
}

/// Two text columns whose cells hold exactly `total` UTF-8 bytes, header
/// included, each cell at most 512 characters, so none is clamped.
fn content_csv(total: usize) -> String {
    let mut s = String::from("a,b\n");
    let mut left = total - 2;
    while left > 0 {
        let one = left.min(512);
        left -= one;
        let two = left.min(512);
        left -= two;
        s.push_str(&format!("{},{}\n", "x".repeat(one), "y".repeat(two)));
    }
    s
}

fn message(e: GaplyError) -> String {
    match e {
        GaplyError::Validation(m) => m,
        other => panic!("not a validation error: {other:?}"),
    }
}

#[test]
fn each_named_value_is_todays_constants_and_its_capabilities_profiles() {
    for (name, l) in [
        ("LEVEL_1", IngestLimits::LEVEL_1),
        ("SUPPLEMENTARY", IngestLimits::SUPPLEMENTARY),
        ("METHODS", IngestLimits::METHODS),
    ] {
        assert_eq!(l.max_rows, MAX_ROWS, "{name}");
        assert_eq!(l.max_cols, MAX_COLS, "{name}");
        assert_eq!(l.max_file_size_bytes, MAX_FILE_SIZE_BYTES, "{name}");
        assert_eq!(l.max_total_chars, MAX_TOTAL_CHARS, "{name}");
        assert_eq!(
            l.max_workbook_uncompressed_bytes, MAX_WORKBOOK_UNCOMPRESSED_BYTES,
            "{name}"
        );
        // Phase 8F-1 (decision 282): the cell cap is the rows times the
        // columns, 640,000, so the rows and the columns bind first.
        assert_eq!(l.max_cells, l.max_rows * l.max_cols, "{name}");
        assert_eq!(l.max_cells, 640_000, "{name}");
        assert!(
            l.max_cols <= MAX_COLUMNS,
            "{name}: the dataset's own width is the ceiling"
        );
    }
    let (mut verify, mut methods) = (0, 0);
    for e in Registry::statistics().entries() {
        let p = e.descriptor.resource_profile;
        let l = match e.kind {
            Kind::Verify(_) => {
                verify += 1;
                IngestLimits::LEVEL_1
            }
            // Phase 8D-1: the one-sample and paired t-tests are methods on the
            // user's data, Planned until 8D-2, with the methods' profile.
            Kind::Describe
            | Kind::AnalyzeIndependentT
            | Kind::AnalyzeOneSampleT
            | Kind::AnalyzePairedT => {
                methods += 1;
                IngestLimits::METHODS
            }
            Kind::Check => continue,
        };
        let id = e.descriptor.id.as_str();
        assert_eq!(p.max_rows, Some(l.max_rows as u64), "{id}");
        assert_eq!(p.max_columns, Some(l.max_cols as u64), "{id}");
        assert_eq!(p.max_input_bytes, Some(l.max_total_chars as u64), "{id}");
    }
    assert_eq!(
        (verify, methods),
        (7, 4),
        "seven Level 1 capabilities; the summary and the three t-tests"
    );
}

/// What a file at, or one over, a limit gives through one form of the
/// ingest: `Ok(rows)` or the refusal.
type Open<'a> = &'a dyn Fn(&Path) -> Result<usize, L1IngestRefusal>;

fn the_three_limits_hold(l: IngestLimits, open: Open<'_>, who: &str) {
    let s = Scratch::new(who);
    let at = s.write("rows_at.csv", rows_csv(l.max_rows).as_bytes());
    assert_eq!(open(&at), Ok(l.max_rows), "{who}: rows at the limit");
    let over = s.write("rows_over.csv", rows_csv(l.max_rows + 1).as_bytes());
    assert_eq!(
        open(&over),
        Err(L1IngestRefusal::FileTooLarge {
            limit_rows: l.max_rows
        }),
        "{who}: one row over"
    );

    let at = s.write("wide_at.csv", wide_csv(l.max_cols).as_bytes());
    assert_eq!(open(&at), Ok(2), "{who}: columns at the limit");
    let over = s.write("wide_over.csv", wide_csv(l.max_cols + 1).as_bytes());
    assert_eq!(
        open(&over),
        Err(L1IngestRefusal::Dataset(DatasetRefusal::TooWide {
            columns: l.max_cols + 1,
            limit: l.max_cols
        })),
        "{who}: one column over"
    );

    let at = s.write("text_at.csv", content_csv(l.max_total_chars).as_bytes());
    assert!(
        open(&at).is_ok(),
        "{who}: cell text at the limit: {:?}",
        open(&at)
    );
    let over = s.write(
        "text_over.csv",
        content_csv(l.max_total_chars + 1).as_bytes(),
    );
    assert_eq!(
        open(&over),
        Err(L1IngestRefusal::ContentTooLarge {
            limit_bytes: l.max_total_chars
        }),
        "{who}: one byte of cell text over"
    );
}

#[test]
fn level_1_holds_its_three_limits_through_load_dataset_with() {
    let l = IngestLimits::LEVEL_1;
    the_three_limits_hold(l, &|p| load_dataset_with(p, l).map(|d| d.rows()), "level_1");
}

#[test]
fn methods_hold_their_three_limits_through_the_open_and_the_rerun() {
    let l = IngestLimits::METHODS;
    the_three_limits_hold(
        l,
        &|p| load_dataset_keeping_bytes(p, l).map(|(d, _, _)| d.rows()),
        "methods_open",
    );
    the_three_limits_hold(
        l,
        &|p| {
            load_dataset_from_bytes_with(&std::fs::read(p).unwrap(), DataFormat::Csv, l)
                .map(|d| d.rows())
        },
        "methods_rerun",
    );
}

#[test]
fn supplementary_parses_at_its_three_limits_and_says_todays_message_over_them() {
    let l = IngestLimits::SUPPLEMENTARY;
    let s = Scratch::new("supplementary");
    let parse =
        |name: &str, body: String| parse_supplementary_with(&s.write(name, body.as_bytes()), l);

    assert_eq!(
        parse("rows_at.csv", rows_csv(l.max_rows)).unwrap().tables[0]
            .rows
            .len(),
        l.max_rows
    );
    assert_eq!(
        message(parse("rows_over.csv", rows_csv(l.max_rows + 1)).unwrap_err()),
        "csv has more than 5000 rows; cap is 5000"
    );

    assert_eq!(
        parse("wide_at.csv", wide_csv(l.max_cols)).unwrap().tables[0]
            .headers
            .len(),
        l.max_cols
    );
    assert_eq!(
        message(parse("wide_over.csv", wide_csv(l.max_cols + 1)).unwrap_err()),
        "csv row 0 has 129 columns; cap is 128"
    );

    assert!(parse("text_at.csv", content_csv(l.max_total_chars)).is_ok());
    assert_eq!(
        message(parse("text_over.csv", content_csv(l.max_total_chars + 1)).unwrap_err()),
        "extracted content exceeds the 2097152-char cap"
    );
}

/// `LEVEL_1` with one field changed: a value the constants never hold.
fn smaller(change: impl FnOnce(&mut IngestLimits)) -> IngestLimits {
    let mut l = IngestLimits::LEVEL_1;
    change(&mut l);
    l
}

#[test]
fn the_row_limit_applied_is_the_one_given() {
    let l = smaller(|l| l.max_rows = 3);
    let s = Scratch::new("rows");
    let bytes = rows_csv(4).into_bytes();
    let p = s.write("four.csv", &bytes);
    let refusal = Err(L1IngestRefusal::FileTooLarge { limit_rows: 3 });
    assert_eq!(
        load_dataset_with(&p, IngestLimits::LEVEL_1).map(|d| d.rows()),
        Ok(4),
        "the constant would open it"
    );
    assert_eq!(load_dataset_with(&p, l).map(|d| d.rows()), refusal);
    assert_eq!(
        load_dataset_keeping_bytes(&p, l).map(|(d, _, _)| d.rows()),
        refusal
    );
    assert_eq!(
        load_dataset_from_bytes_with(&bytes, DataFormat::Csv, l).map(|d| d.rows()),
        refusal
    );
    assert_eq!(
        message(parse_supplementary_with(&p, l).unwrap_err()),
        "csv has more than 3 rows; cap is 3"
    );
    assert_eq!(
        message(parse_supplementary_bytes_with(&bytes, "csv", l).unwrap_err()),
        "csv has more than 3 rows; cap is 3"
    );
    // The classifier that tells the failed parse apart says the same.
    assert!(exceeds_row_cap_bytes(&bytes, "csv", l));
    assert!(exceeds_row_cap_with(&p, l).unwrap());
    assert!(!exceeds_row_cap_bytes(&bytes, "csv", IngestLimits::LEVEL_1));

    // A .sav, which the reader cuts at max_rows + 1 and the ingest refuses
    // on rows_truncated.
    let sav = sav_bytes(1, 4);
    let q = s.write("four.sav", &sav);
    assert_eq!(
        load_dataset_with(&q, IngestLimits::LEVEL_1).map(|d| d.rows()),
        Ok(4)
    );
    assert_eq!(load_dataset_with(&q, l).map(|d| d.rows()), refusal);
    let ev = parse_supplementary_with(&q, l).unwrap();
    assert!(ev.rows_truncated);
    assert_eq!(ev.tables[0].rows.len(), 3);

    // A .mat, whose arrays are cut at max_rows.
    let mat = fixture("analysis_v7.mat");
    let n = load_dataset_with(&mat, IngestLimits::LEVEL_1)
        .expect("the fixture opens")
        .rows();
    assert!(n >= 2, "{n}");
    let m = smaller(|l| l.max_rows = n - 1);
    assert_eq!(
        load_dataset_with(&mat, m).map(|d| d.rows()),
        Err(L1IngestRefusal::FileTooLarge { limit_rows: n - 1 })
    );
    assert_eq!(
        parse_supplementary_with(&mat, m).unwrap().tables[0]
            .rows
            .len(),
        n - 1
    );
}

#[test]
fn the_column_limit_applied_is_the_one_given() {
    let l = smaller(|l| l.max_cols = 3);
    let s = Scratch::new("cols");
    let bytes = wide_csv(4).into_bytes();
    let p = s.write("four.csv", &bytes);
    let refusal = Err(L1IngestRefusal::Dataset(DatasetRefusal::TooWide {
        columns: 4,
        limit: 3,
    }));
    assert_eq!(
        load_dataset_with(&p, IngestLimits::LEVEL_1).map(|d| d.rows()),
        Ok(2),
        "the constant would open it"
    );
    assert_eq!(load_dataset_with(&p, l).map(|d| d.rows()), refusal);
    assert_eq!(
        load_dataset_keeping_bytes(&p, l).map(|(d, _, _)| d.rows()),
        refusal
    );
    assert_eq!(
        load_dataset_from_bytes_with(&bytes, DataFormat::Csv, l).map(|d| d.rows()),
        refusal
    );
    assert_eq!(
        message(parse_supplementary_with(&p, l).unwrap_err()),
        "csv row 0 has 4 columns; cap is 3"
    );
    assert_eq!(exceeds_col_cap_bytes(&bytes, "csv", l), Some(4));
    assert_eq!(exceeds_col_cap_with(&p, l).unwrap(), Some(4));
    assert_eq!(
        exceeds_col_cap_bytes(&bytes, "csv", IngestLimits::LEVEL_1),
        None
    );
}

#[test]
fn the_cell_text_limit_applied_is_the_one_given() {
    let l = smaller(|l| l.max_total_chars = 10);
    let s = Scratch::new("text");
    // "a" + "b" + "12345" + "6789": 11 bytes of cell text.
    let bytes = b"a,b\n12345,6789\n".to_vec();
    let p = s.write("eleven.csv", &bytes);
    let refusal = Err(L1IngestRefusal::ContentTooLarge { limit_bytes: 10 });
    assert_eq!(
        load_dataset_with(&p, IngestLimits::LEVEL_1).map(|d| d.rows()),
        Ok(1),
        "the constant would open it"
    );
    assert_eq!(load_dataset_with(&p, l).map(|d| d.rows()), refusal);
    assert_eq!(
        load_dataset_keeping_bytes(&p, l).map(|(d, _, _)| d.rows()),
        refusal
    );
    assert_eq!(
        load_dataset_from_bytes_with(&bytes, DataFormat::Csv, l).map(|d| d.rows()),
        refusal
    );
    assert_eq!(
        message(parse_supplementary_with(&p, l).unwrap_err()),
        "extracted content exceeds the 10-char cap"
    );
    assert!(exceeds_content_cap_bytes(&bytes, "csv", l));
    assert!(!exceeds_content_cap_bytes(
        &bytes,
        "csv",
        IngestLimits::LEVEL_1
    ));
    // Plain text is cut at the same limit, and says so.
    let t = s.write("notes.txt", b"0123456789abcdef");
    let ev = parse_supplementary_with(&t, l).unwrap();
    assert_eq!(ev.text_summary, "0123456789");
    assert_eq!(ev.notes, vec!["text truncated to 10 chars".to_string()]);
}

#[test]
fn the_file_size_limit_applied_is_the_one_given() {
    let l = smaller(|l| l.max_file_size_bytes = 20);
    let s = Scratch::new("bytes");
    let bytes = rows_csv(5).into_bytes();
    assert_eq!(bytes.len(), 24, "the file is over 20 bytes");
    let n = bytes.len() as u64;
    let p = s.write("small.csv", &bytes);
    let refusal = Err(L1IngestRefusal::TooLarge {
        bytes: n,
        limit: 20,
    });
    assert_eq!(
        load_dataset_with(&p, IngestLimits::LEVEL_1).map(|d| d.rows()),
        Ok(5),
        "the constant would open it"
    );
    assert_eq!(load_dataset_with(&p, l).map(|d| d.rows()), refusal);
    assert_eq!(
        load_dataset_keeping_bytes(&p, l).map(|(d, _, _)| d.rows()),
        refusal
    );
    assert_eq!(
        load_dataset_from_bytes_with(&bytes, DataFormat::Csv, l).map(|d| d.rows()),
        refusal
    );
    let said = format!("file too large: {n} bytes exceeds the 20-byte cap; not read into memory");
    assert_eq!(message(parse_supplementary_with(&p, l).unwrap_err()), said);
    assert_eq!(
        message(parse_supplementary_bytes_with(&bytes, "csv", l).unwrap_err()),
        said
    );
}

#[test]
fn the_workbook_limit_applied_is_the_one_given_and_the_parser_applies_none() {
    let p = fixture("sample.xlsx");
    let bytes = std::fs::read(&p).unwrap();
    let declared = declared_uncompressed_total(&bytes).expect("a zip");
    assert!(
        load_dataset_with(&p, IngestLimits::LEVEL_1).is_ok(),
        "the constant would open it"
    );
    let l = smaller(|l| l.max_workbook_uncompressed_bytes = declared - 1);
    let refusal = Err(L1IngestRefusal::ArchiveTooLarge {
        declared,
        limit: declared - 1,
    });
    assert_eq!(load_dataset_with(&p, l).map(|d| d.rows()), refusal);
    assert_eq!(
        load_dataset_keeping_bytes(&p, l).map(|(d, _, _)| d.rows()),
        refusal
    );
    assert_eq!(
        load_dataset_from_bytes_with(&bytes, DataFormat::Xlsx, l).map(|d| d.rows()),
        refusal
    );
    // The parser has no inflation check, today or with the field set: only
    // the ingest's byte checks read it (decision 243).
    assert!(parse_supplementary_with(&p, l).is_ok());
}

/// A `.sav` of `vars` numeric variables and `rows` cases, written by the
/// reader's own crate.
fn sav_bytes(vars: usize, rows: usize) -> Vec<u8> {
    use arrow::array::Float64Array;
    use arrow::datatypes::{DataType, Field, Schema};
    let fields: Vec<Field> = (1..=vars)
        .map(|v| Field::new(format!("v{v}"), DataType::Float64, false))
        .collect();
    let columns: Vec<std::sync::Arc<dyn arrow::array::Array>> = (1..=vars)
        .map(|v| {
            std::sync::Arc::new(Float64Array::from(
                (0..rows).map(|r| (r * v) as f64).collect::<Vec<_>>(),
            )) as _
        })
        .collect();
    let batch = arrow::record_batch::RecordBatch::try_new(
        std::sync::Arc::new(Schema::new(fields)),
        columns,
    )
    .unwrap();
    let mut buf = std::io::Cursor::new(Vec::new());
    ambers::write_sav_to_writer(
        &mut buf,
        &batch,
        &ambers::SpssMetadata::default(),
        ambers::Compression::Zlib,
        None,
    )
    .unwrap();
    buf.into_inner()
}

/// An uncompressed MATLAB v5 file of `vars` double column vectors named
/// `v1`, `v2`, …, each of `rows` values, as `save('-v6')` writes them: one
/// miMATRIX element a variable, with its flags, dimensions, name and real
/// part.
fn mat_bytes(vars: usize, rows: usize) -> Vec<u8> {
    fn u32s(out: &mut Vec<u8>, vs: &[u32]) {
        for v in vs {
            out.extend_from_slice(&v.to_le_bytes());
        }
    }
    let mut out = vec![b' '; 116];
    out[..19].copy_from_slice(b"MATLAB 5.0 MAT-file");
    out.extend_from_slice(&[0u8; 8]);
    out.extend_from_slice(&0x0100u16.to_le_bytes());
    out.extend_from_slice(b"IM");
    for v in 1..=vars {
        let name = format!("v{v}");
        let mut m = Vec::new();
        // Array flags (miUINT32): mxDOUBLE_CLASS. Dimensions (miINT32): rows x 1.
        u32s(&mut m, &[6, 8, 6, 0, 5, 8, rows as u32, 1]);
        // The name (miINT8), padded to 8 bytes.
        u32s(&mut m, &[1, name.len() as u32]);
        m.extend_from_slice(name.as_bytes());
        m.resize(m.len() + name.len().div_ceil(8) * 8 - name.len(), 0);
        // The real part (miDOUBLE).
        u32s(&mut m, &[9, (rows * 8) as u32]);
        for r in 0..rows {
            m.extend_from_slice(&((r * v) as f64).to_le_bytes());
        }
        u32s(&mut out, &[14, m.len() as u32]);
        out.extend_from_slice(&m);
    }
    out
}

/// The 129-variable `.sav` and `.mat`, with the format a rerun reads them as.
fn over_the_column_limit() -> [(&'static str, Vec<u8>, DataFormat); 2] {
    [
        ("wide.sav", sav_bytes(MAX_COLS + 1, 2), DataFormat::Sav),
        ("wide.mat", mat_bytes(MAX_COLS + 1, 2), DataFormat::Mat),
    ]
}

/// **Decision 246: a `.sav` or `.mat` holding more variables than the
/// column limit is refused as too wide**, under `LEVEL_1` and `METHODS`,
/// through each form of the ingest their callers use: Level 1's
/// `load_dataset_with`, the open's `load_dataset_keeping_bytes` and the
/// rerun's `load_dataset_from_bytes`. The refusal carries the number the file
/// holds and the value's limit, as a CSV's does. Until 8B-2 (this test was
/// 8B's `…_opens_with_the_first_128_today`) the file opened with its first
/// 128.
#[test]
fn a_sav_or_mat_with_more_variables_than_the_column_limit_is_refused_as_too_wide() {
    assert_eq!(MAX_COLS, 128);
    let s = Scratch::new("sav_wide");
    let refusal = Err(L1IngestRefusal::Dataset(DatasetRefusal::TooWide {
        columns: 129,
        limit: 128,
    }));
    for (name, bytes, format) in over_the_column_limit() {
        let p = s.write(name, &bytes);
        assert_eq!(
            load_dataset_with(&p, IngestLimits::LEVEL_1).map(|d| d.rows()),
            refusal,
            "{name}: Level 1"
        );
        assert_eq!(
            load_dataset_keeping_bytes(&p, IngestLimits::METHODS).map(|(d, _, _)| d.rows()),
            refusal,
            "{name}: the methods' open"
        );
        assert_eq!(
            load_dataset_from_bytes(&bytes, format).map(|d| d.rows()),
            refusal,
            "{name}: the rerun"
        );
    }
}

/// The variable limit refused on is the value's, not the constant's.
#[test]
fn the_variable_limit_applied_is_the_one_given() {
    let l = smaller(|l| l.max_cols = 3);
    let s = Scratch::new("vars");
    let refusal = Err(L1IngestRefusal::Dataset(DatasetRefusal::TooWide {
        columns: 4,
        limit: 3,
    }));
    for (name, bytes, format) in [
        ("four.sav", sav_bytes(4, 2), DataFormat::Sav),
        ("four.mat", mat_bytes(4, 2), DataFormat::Mat),
    ] {
        let p = s.write(name, &bytes);
        assert_eq!(
            load_dataset_with(&p, IngestLimits::LEVEL_1).map(|d| d.schema.columns.len()),
            Ok(4),
            "{name}: the constant would open it"
        );
        assert_eq!(
            load_dataset_with(&p, l).map(|d| d.rows()),
            refusal,
            "{name}"
        );
        assert_eq!(
            load_dataset_keeping_bytes(&p, l).map(|(d, _, _)| d.rows()),
            refusal,
            "{name}"
        );
        assert_eq!(
            load_dataset_from_bytes_with(&bytes, format, l).map(|d| d.rows()),
            refusal,
            "{name}"
        );
    }
}

/// Unchanged by 8B-2: a `.sav` or `.mat` at the column limit opens through
/// every form, and `SUPPLEMENTARY` (the reviewer agent's files and the
/// legacy lane) keeps the first 128 of 129 variables with today's note.
#[test]
fn at_the_column_limit_a_sav_or_mat_opens_and_supplementary_keeps_the_first_128_of_129() {
    let s = Scratch::new("sav_at");
    for (name, bytes, format) in [
        ("at.sav", sav_bytes(MAX_COLS, 2), DataFormat::Sav),
        ("at.mat", mat_bytes(MAX_COLS, 2), DataFormat::Mat),
    ] {
        let p = s.write(name, &bytes);
        let shape = |d: gaply_core::stats_l1::dataset::Dataset| (d.rows(), d.schema.columns.len());
        assert_eq!(
            load_dataset_with(&p, IngestLimits::LEVEL_1).map(shape),
            Ok((2, 128)),
            "{name}"
        );
        assert_eq!(
            load_dataset_keeping_bytes(&p, IngestLimits::METHODS).map(|(d, _, _)| shape(d)),
            Ok((2, 128)),
            "{name}"
        );
        assert_eq!(
            load_dataset_from_bytes(&bytes, format).map(shape),
            Ok((2, 128)),
            "{name}"
        );
    }
    for (name, bytes, _) in over_the_column_limit() {
        let p = s.write(name, &bytes);
        let ev = parse_supplementary_with(&p, IngestLimits::SUPPLEMENTARY).unwrap();
        assert_eq!(ev.tables[0].headers.len(), 128, "{name}");
        assert_eq!(ev.tables[0].rows.len(), 2, "{name}");
        assert_eq!(ev.variables.len(), 128, "{name}");
        assert_eq!(
            ev.notes,
            vec!["128 of 129 variables kept; cap is 128".to_string()],
            "{name}"
        );
    }
}

/// Every key of a JSON value, at any depth.
fn keys(v: &serde_json::Value, out: &mut Vec<String>) {
    match v {
        serde_json::Value::Object(m) => {
            for (k, x) in m {
                out.push(k.clone());
                keys(x, out);
            }
        }
        serde_json::Value::Array(a) => a.iter().for_each(|x| keys(x, out)),
        _ => {}
    }
}

/// `variables_held` says how many variables a truncated `.sav` or `.mat`
/// holds, and never reaches the reviewer's payload: `commands.rs` sends
/// `serde_json::to_value(&ev)` of exactly this evidence.
#[test]
fn the_variables_held_are_recorded_and_never_reach_the_reviewers_payload() {
    let s = Scratch::new("held");
    for (name, bytes, _) in over_the_column_limit() {
        let ev =
            parse_supplementary_with(&s.write(name, &bytes), IngestLimits::SUPPLEMENTARY).unwrap();
        assert_eq!(
            ev.variables_held,
            Some(129),
            "{name}: populated, so the check means something"
        );
        assert!(
            format!("{ev:?}").contains("variables_held: Some(129)"),
            "{name}: Debug shows the count"
        );
        let v = serde_json::to_value(&ev).unwrap();
        let mut found = Vec::new();
        keys(&v, &mut found);
        assert!(
            !found.iter().any(|k| k == "variables_held"),
            "{name}: the key reached the payload"
        );
        assert!(
            !v.to_string().contains("variables_held"),
            "{name}: the name reached the payload"
        );
    }
    let at = parse_supplementary_with(
        &s.write("at.sav", &sav_bytes(MAX_COLS, 2)),
        IngestLimits::SUPPLEMENTARY,
    )
    .unwrap();
    assert_eq!(at.variables_held, None, "every variable kept");
}

/// The refusal reaches the summary's and the t-test's screens (both open a
/// file through `stats_desc_open`) as a CSV's does, through IPC: the JSON
/// `fixtures/ingestRefusals.ts`'s `dataset too_wide` holds, which their
/// vitest tests word as decision 243's sentence.
#[test]
fn the_refusal_reaches_the_screens_in_the_shape_of_a_csvs() {
    let dir = desc_support::TempDir::new("8b2_wide_wire");
    let app_data = dir.0.join("appdata");
    std::fs::create_dir_all(&app_data).unwrap();
    let (_app, w) = desc_support::app(&app_data, Arc::new(Database::in_memory().unwrap()), None);
    let want =
        json!({"code": "dataset", "refusal": {"code": "too_wide", "columns": 129, "limit": 128}});
    let mut files: Vec<(&str, Vec<u8>)> = over_the_column_limit()
        .into_iter()
        .map(|(n, b, _)| (n, b))
        .collect();
    files.push(("wide.csv", wide_csv(MAX_COLS + 1).into_bytes()));
    for (name, bytes) in files {
        let p = dir.write(name, &bytes);
        let e = desc_support::call(
            &w,
            "stats_desc_open",
            json!({ "path": p.to_string_lossy() }),
        )
        .expect_err(name);
        assert_eq!(desc_support::refusal(&e), want, "{name}");
    }
}
