//! **The ingest's cell cap (Research OS, Phase 8F-1, decision 282).** Cells
//! are the data rows kept times the columns kept (for a `.sav` or `.mat`,
//! the variables kept). `IngestLimits::max_cells` is checked while parsing,
//! after the row and the column checks of the same row, and a file over it
//! is refused as `too_many_cells`, with the limit from the value.
//!
//! * for a CSV or TSV, a workbook, a `.sav` and a `.mat`, with a small
//!   injected cap: at the cap the file opens, one cell over is refused, and
//!   the parse stopped where the cap was crossed: a CSV's rows read stop
//!   there; a workbook, `.sav` or `.mat` converts no cell;
//! * a refusal for rows, columns or cell text in the same file keeps its own
//!   code, in decision 282's order;
//! * decision 282's corner tests: in the named values' shape (`max_cells` =
//!   `max_rows × max_cols`), at 50 rows × 4 columns, every format opens,
//!   parses and refuses exactly as it does with no cell cap at all, through
//!   the methods' open and through `parse_supplementary_with`;
//! * a file with no columns is refused as before, never with a panic.
//!
//! Every file is synthetic and small, written into a folder this test
//! creates and removes. `sav_bytes` and `mat_bytes` are copied from
//! `tests/ingest_limits.rs`, so each test binary builds its own files.

mod desc_support;

use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

use app_lib::stats_l1_ingest::{
    load_dataset_keeping_bytes, load_dataset_keeping_bytes_with_progress, L1IngestRefusal,
};
use app_lib::supplementary::{
    parse_supplementary_bytes_with, parse_supplementary_with, IngestLimits, SupplementaryEvidence,
};
use desc_support::{xlsx, TempDir, C};
use gaply_core::stats_l1::dataset::DatasetRefusal;
use gaply_core::GaplyError;

/// `METHODS` with its rows, columns and cells replaced.
fn cap(rows: usize, cols: usize, cells: usize) -> IngestLimits {
    IngestLimits {
        max_rows: rows,
        max_cols: cols,
        max_cells: cells,
        ..IngestLimits::METHODS
    }
}

fn cells_refusal(limit_cells: usize) -> Result<usize, L1IngestRefusal> {
    Err(L1IngestRefusal::TooManyCells { limit_cells })
}

/// The methods' open: `Ok(rows)` or the refusal.
fn open(p: &Path, l: IngestLimits) -> Result<usize, L1IngestRefusal> {
    load_dataset_keeping_bytes(p, l).map(|(d, _, _)| d.rows())
}

/// The methods' open with a counter: the result, and the rows read it holds.
fn open_counting(p: &Path, l: IngestLimits) -> (Result<usize, L1IngestRefusal>, u64) {
    let rows = AtomicU64::new(u64::MAX);
    let r = load_dataset_keeping_bytes_with_progress(p, Some(&rows), l).map(|(d, _, _)| d.rows());
    (r, rows.load(Ordering::Relaxed))
}

/// A header of `cols` names and `rows` rows of numbers, joined by `sep`.
fn table(cols: usize, rows: usize, sep: &str) -> Vec<u8> {
    let mut s = (1..=cols)
        .map(|c| format!("c{c}"))
        .collect::<Vec<_>>()
        .join(sep);
    s.push('\n');
    for r in 0..rows {
        s.push_str(
            &(1..=cols)
                .map(|c| (r * c % 97).to_string())
                .collect::<Vec<_>>()
                .join(sep),
        );
        s.push('\n');
    }
    s.into_bytes()
}

/// The same table as a workbook.
fn sheet(cols: usize, rows: usize) -> Vec<u8> {
    let names: Vec<String> = (1..=cols).map(|c| format!("c{c}")).collect();
    let headers: Vec<&str> = names.iter().map(String::as_str).collect();
    let values: Vec<Vec<String>> = (0..rows)
        .map(|r| (1..=cols).map(|c| (r * c % 97).to_string()).collect())
        .collect();
    let rows: Vec<Vec<C>> = values
        .iter()
        .map(|r| r.iter().map(|v| C::N(v)).collect())
        .collect();
    xlsx(&headers, &rows)
}

/// A `.sav` of `vars` numeric variables and `rows` cases, written by the
/// reader's own crate (copied from `tests/ingest_limits.rs`).
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

/// An uncompressed MATLAB v5 file of `vars` double column vectors of `rows`
/// values (copied from `tests/ingest_limits.rs`).
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
        u32s(&mut m, &[6, 8, 6, 0, 5, 8, rows as u32, 1]);
        u32s(&mut m, &[1, name.len() as u32]);
        m.extend_from_slice(name.as_bytes());
        m.resize(m.len() + name.len().div_ceil(8) * 8 - name.len(), 0);
        u32s(&mut m, &[9, (rows * 8) as u32]);
        for r in 0..rows {
            m.extend_from_slice(&((r * v) as f64).to_le_bytes());
        }
        u32s(&mut out, &[14, m.len() as u32]);
        out.extend_from_slice(&m);
    }
    out
}

/// A table of `cols` columns and `rows` rows in one of the four formats.
fn file(format: &str, cols: usize, rows: usize) -> Vec<u8> {
    match format {
        "csv" => table(cols, rows, ","),
        "tsv" => table(cols, rows, "\t"),
        "xlsx" => sheet(cols, rows),
        "sav" => sav_bytes(cols, rows),
        "mat" => mat_bytes(cols, rows),
        other => panic!("{other}"),
    }
}

fn evidence(bytes: &[u8], ext: &str, l: IngestLimits) -> SupplementaryEvidence {
    parse_supplementary_bytes_with(bytes, ext, l).unwrap_or_else(|e| panic!("{ext}: {e:?}"))
}

fn message(e: GaplyError) -> String {
    match e {
        GaplyError::Validation(m) => m,
        other => panic!("not a validation error: {other:?}"),
    }
}

#[test]
fn every_format_opens_at_the_cell_cap_and_is_refused_one_cell_over() {
    let d = TempDir::new("cells_at_and_over");
    // 12 cells: 3 columns by 4 rows (or 2 variables by 6 cases).
    for (format, cols, at_rows) in [
        ("csv", 3, 4),
        ("tsv", 3, 4),
        ("xlsx", 3, 4),
        ("sav", 2, 6),
        ("mat", 2, 6),
    ] {
        let l = cap(50, 4, 12);
        let at = d.write(&format!("at.{format}"), &file(format, cols, at_rows));
        assert_eq!(open(&at, l), Ok(at_rows), "{format}: at the cap");
        let over = d.write(&format!("over.{format}"), &file(format, cols, at_rows + 1));
        assert_eq!(
            open(&over, l),
            cells_refusal(12),
            "{format}: one row of cells over"
        );
        // The parse says so with a flag, not an error, and holds no rows.
        let ev = evidence(&file(format, cols, at_rows + 1), format, l);
        assert!(ev.cells_over, "{format}");
        assert!(
            ev.tables.iter().all(|t| t.rows.is_empty()),
            "{format}: no row is kept"
        );
        assert!(
            !evidence(&file(format, cols, at_rows), format, l).cells_over,
            "{format}"
        );
    }
}

#[test]
fn a_csv_stops_reading_at_the_first_record_over_the_cell_cap() {
    let d = TempDir::new("cells_csv_stop");
    let l = cap(5_000, 4, 12);
    // 3 columns: the fifth data row takes the cells to 15.
    let p = d.write("long.csv", &table(3, 2_500, ","));
    let (r, rows_read) = open_counting(&p, l);
    assert_eq!(r, cells_refusal(12));
    assert_eq!(
        rows_read, 4,
        "the rows read when it stopped, not the file's 2,500"
    );
    // At the cap, the counter ends at the rows read.
    let p = d.write("at.csv", &table(3, 4, ","));
    assert_eq!(open_counting(&p, l), (Ok(4), 4));
}

#[test]
fn a_workbook_sav_or_mat_over_the_cap_converts_no_cell() {
    let d = TempDir::new("cells_no_conversion");
    let l = cap(5_000, 4, 12);
    for (format, cols, rows) in [("xlsx", 3, 2_500), ("sav", 2, 2_500), ("mat", 2, 2_500)] {
        let bytes = file(format, cols, rows);
        let p = d.write(&format!("long.{format}"), &bytes);
        let (r, rows_read) = open_counting(&p, l);
        assert_eq!(r, cells_refusal(12), "{format}");
        assert_eq!(rows_read, 0, "{format}: no row was converted");
        let ev = evidence(&bytes, format, l);
        assert!(ev.cells_over, "{format}");
        assert!(ev.tables.iter().all(|t| t.rows.is_empty()), "{format}");
    }
    // A workbook converts not even its header row.
    let ev = evidence(&sheet(3, 2_500), "xlsx", l);
    assert!(ev.tables.iter().all(|t| t.headers.is_empty()));
}

#[test]
fn a_refusal_for_rows_columns_or_text_keeps_its_code_in_decision_282s_order() {
    let d = TempDir::new("cells_order");
    for format in ["csv", "xlsx"] {
        // Rows before cells: the fifth row is over 4 rows and over 12 cells.
        let p = d.write(&format!("rows.{format}"), &file(format, 3, 5));
        assert_eq!(
            open(&p, cap(4, 3, 12)),
            Err(L1IngestRefusal::FileTooLarge { limit_rows: 4 }),
            "{format}"
        );
        // Columns before cells.
        let p = d.write(&format!("wide.{format}"), &file(format, 4, 5));
        assert_eq!(
            open(&p, cap(50, 3, 12)),
            Err(L1IngestRefusal::Dataset(DatasetRefusal::TooWide {
                columns: 4,
                limit: 3
            })),
            "{format}"
        );
    }
    // Cells before text in the same record; text in an earlier record first.
    // The header's cells hold 6 bytes, the rows 3, 3, 3, 3 and 4: in all 9,
    // 12, 15, 18 and 22 after each row.
    let p = d.write("text.csv", &table(3, 5, ","));
    let text = |chars| IngestLimits {
        max_total_chars: chars,
        ..cap(50, 3, 12)
    };
    assert_eq!(
        open(&p, text(20)),
        cells_refusal(12),
        "the fifth row crosses both"
    );
    assert_eq!(
        open(&p, text(10)),
        Err(L1IngestRefusal::ContentTooLarge { limit_bytes: 10 }),
        "the second row crosses the text first"
    );
    // A workbook's cells are checked from its size, before its text.
    let p = d.write("text.xlsx", &sheet(3, 5));
    assert_eq!(open(&p, text(10)), cells_refusal(12));

    // A .sav or .mat over the variables and the cells: too wide first.
    for format in ["sav", "mat"] {
        let p = d.write(&format!("wide.{format}"), &file(format, 5, 10));
        assert_eq!(
            open(&p, cap(50, 4, 12)),
            Err(L1IngestRefusal::Dataset(DatasetRefusal::TooWide {
                columns: 5,
                limit: 4
            })),
            "{format}"
        );
    }
    // Over the rows and the cells, where the rows allow fewer than the cells:
    // the rows first.
    for format in ["sav", "mat"] {
        let p = d.write(&format!("rows.{format}"), &file(format, 2, 10));
        assert_eq!(
            open(&p, cap(5, 4, 12)),
            Err(L1IngestRefusal::FileTooLarge { limit_rows: 5 }),
            "{format}"
        );
    }
    // A .mat knows every array's length before it renders a cell, so a file
    // over the rows is refused on its rows even where the cells allow fewer.
    let p = d.write("rows_first.mat", &mat_bytes(2, 10));
    assert_eq!(
        open(&p, cap(5, 4, 8)),
        Err(L1IngestRefusal::FileTooLarge { limit_rows: 5 })
    );
    // A .sav's read is bounded by the rows the cells allow, plus one
    // (decision 282), so where the cells allow fewer rows than the row limit
    // it cannot see past them: 10 cases, the cells allowing 4, the rows 5.
    // It reads 5, which is not over the rows, and is over the cells.
    let p = d.write("rows_unseen.sav", &sav_bytes(2, 10));
    assert_eq!(open(&p, cap(5, 4, 8)), cells_refusal(8));
}

/// What a file gives through the methods' open and through
/// `parse_supplementary_with`, in a form two runs can be compared in.
fn outcome(p: &Path, l: IngestLimits) -> (Result<usize, L1IngestRefusal>, String) {
    let parsed = match parse_supplementary_with(p, l) {
        Ok(ev) => format!(
            "ok: {} headers, {} rows, rows_truncated {}, variables_held {:?}, cells_over {}, notes {:?}",
            ev.tables.first().map_or(0, |t| t.headers.len()),
            ev.tables.first().map_or(0, |t| t.rows.len()),
            ev.rows_truncated,
            ev.variables_held,
            ev.cells_over,
            ev.notes
        ),
        Err(e) => format!("err: {}", message(e)),
    };
    (open(p, l), parsed)
}

#[test]
fn in_the_named_values_shape_every_format_behaves_as_with_no_cell_cap() {
    // 50 rows by 4 columns, 200 cells: the shape of MAX_ROWS × MAX_COLS.
    let shaped = cap(50, 4, 200);
    let uncapped = cap(50, 4, usize::MAX);
    let d = TempDir::new("cells_corners");
    for format in ["csv", "xlsx", "sav", "mat"] {
        let at = d.write(&format!("at.{format}"), &file(format, 4, 50));
        let rows_over = d.write(&format!("rows_over.{format}"), &file(format, 4, 51));
        let wide = d.write(&format!("wide.{format}"), &file(format, 5, 50));
        for (p, what) in [(&at, "50 x 4"), (&rows_over, "51 x 4"), (&wide, "50 x 5")] {
            assert_eq!(
                outcome(p, shaped),
                outcome(p, uncapped),
                "{format}, {what}: the cell cap changed something"
            );
        }
        assert_eq!(open(&at, shaped), Ok(50), "{format}: opens at the corner");
        assert_eq!(
            open(&rows_over, shaped),
            Err(L1IngestRefusal::FileTooLarge { limit_rows: 50 }),
            "{format}: one row over"
        );
        assert_eq!(
            open(&wide, shaped),
            Err(L1IngestRefusal::Dataset(DatasetRefusal::TooWide {
                columns: 5,
                limit: 4
            })),
            "{format}: one column over"
        );
    }
    // The supplementary lane's words, today's.
    assert_eq!(
        outcome(&d.0.join("rows_over.csv"), shaped).1,
        "err: csv has more than 50 rows; cap is 50"
    );
    assert_eq!(
        outcome(&d.0.join("wide.csv"), shaped).1,
        "err: csv row 0 has 5 columns; cap is 4"
    );
    assert_eq!(
        outcome(&d.0.join("rows_over.xlsx"), shaped).1,
        "err: spreadsheet has 52 rows; cap is 50"
    );
    assert_eq!(
        outcome(&d.0.join("wide.xlsx"), shaped).1,
        "err: spreadsheet has 5 columns; cap is 4"
    );
    for format in ["sav", "mat"] {
        let (_, rows) = outcome(&d.0.join(format!("rows_over.{format}")), shaped);
        assert!(rows.starts_with("ok: 4 headers, 50 rows, rows_truncated true, variables_held None, cells_over false"), "{format}: {rows}");
        let (_, wide) = outcome(&d.0.join(format!("wide.{format}")), shaped);
        assert!(wide.starts_with("ok: 4 headers, 50 rows, rows_truncated false, variables_held Some(5), cells_over false"), "{format}: {wide}");
    }
}

/// A `.sav` with no variables is not here: ambers 0.4.4 divided by the row's
/// width while reading one (`scanner.rs`, each of its three read paths), so
/// before 8F-2 it panicked under any limits. Since 8F-2 (decision 291)
/// `supplementary`'s header check gives it an empty table first, tested in
/// `tests/sav_case_size.rs` under every caller. Gaply's own arithmetic for
/// it, `max_cells` over zero variables, is pinned in `supplementary.rs`'s
/// unit tests (`zero_columns_allow_the_row_limit_and_never_divide_by_zero`).
#[test]
fn a_file_with_no_columns_is_refused_as_before_without_a_panic() {
    let d = TempDir::new("cells_no_columns");
    let small = cap(50, 4, 12);
    let uncapped = cap(50, 4, usize::MAX);
    let files: [(&str, Vec<u8>); 3] = [
        ("empty.csv", Vec::new()),
        ("empty.xlsx", xlsx(&[], &[])),
        ("none.mat", mat_bytes(0, 0)),
    ];
    for (name, bytes) in files {
        let p = d.write(name, &bytes);
        let (got, before) = (open(&p, small), open(&p, uncapped));
        println!("{name}: {got:?}");
        assert_eq!(got, before, "{name}: the cell cap changed the refusal");
        assert!(got.is_err(), "{name}: a file with no columns does not open");
    }
}
