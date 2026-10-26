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
//! The source guard that each production caller passes its named value is
//! `tests/ingest_limits_guard.rs`. Every file here is synthetic and written
//! into a directory this test creates and removes.

use std::path::{Path, PathBuf};

use app_lib::stats_l1_ingest::{
    declared_uncompressed_total, load_dataset_from_bytes_with, load_dataset_keeping_bytes,
    load_dataset_with, L1IngestRefusal, MAX_WORKBOOK_UNCOMPRESSED_BYTES,
};
use app_lib::supplementary::{
    exceeds_col_cap_bytes, exceeds_col_cap_with, exceeds_content_cap_bytes, exceeds_row_cap_bytes,
    exceeds_row_cap_with, parse_supplementary_bytes_with, parse_supplementary_with, IngestLimits,
    MAX_COLS, MAX_FILE_SIZE_BYTES, MAX_ROWS, MAX_TOTAL_CHARS,
};
use gaply_core::research_project::DataFormat;
use gaply_core::stats_l1::dataset::{DatasetRefusal, MAX_COLUMNS};
use gaply_core::stats_registry::{Kind, Registry};
use gaply_core::GaplyError;

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
            Kind::Describe | Kind::AnalyzeIndependentT => {
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
        (7, 2),
        "seven Level 1 capabilities; the summary and the t-test"
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

/// **The open question of 8B, measured, with no change.** A `.sav` with more
/// variables than the column limit is not refused as too wide: the reader
/// keeps the first `max_cols` (`sav_evidence`), the ingest checks only
/// `rows_truncated`, and the file opens with 128 of its 129 columns. This
/// pins today's behaviour so the decision that changes it is a visible diff.
#[test]
fn a_sav_with_more_variables_than_the_column_limit_opens_with_the_first_128_today() {
    let s = Scratch::new("sav_wide");
    let p = s.write("wide.sav", &sav_bytes(MAX_COLS + 1, 2));
    for l in [IngestLimits::LEVEL_1, IngestLimits::METHODS] {
        let d = load_dataset_with(&p, l).expect("opens today");
        assert_eq!(d.schema.columns.len(), MAX_COLS);
        assert_eq!(d.rows(), 2);
    }
    let ev = parse_supplementary_with(&p, IngestLimits::SUPPLEMENTARY).unwrap();
    assert_eq!(
        ev.notes,
        vec!["128 of 129 variables kept; cap is 128".to_string()]
    );
}
