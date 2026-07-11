//! Supplementary-file parsing for PublishReady — Excel/CSV/text into a neutral
//! [`SupplementaryEvidence`] struct so the reviewer can later reason over the
//! statistical/analysis context. APP-CRATE ONLY (gaply-core stays pure).
//!
//! # Memory is the hard constraint (8GB tier)
//!
//! Every bound below is an explicit named constant, enforced so a huge file
//! CANNOT blow memory:
//! - [`MAX_FILE_SIZE_BYTES`] is checked via file METADATA *before* any content
//!   is read — a 500 MB spreadsheet is rejected instantly, never read into RAM.
//! - CSV/text are STREAMED with per-row/col/total caps, so even a file under
//!   the size cap can't accumulate unbounded extracted content.
//! - Excel (calamine) loads eagerly, so the file-size cap is its memory bound;
//!   the row/col caps then validate the loaded sheet. (A tiny xlsx that
//!   decompresses to gigabytes — a zip bomb — is a residual risk calamine's
//!   eager load doesn't fully bound; noted as a follow-up.)
//!
//! # Untrusted content
//!
//! Extracted cell/text content is UNTRUSTED for prompt purposes. This set only
//! STRUCTURES it; when it is later serialized toward any model it must go
//! through `llm_safe()` (the same discipline as web/manuscript text) — that
//! guard belongs to the downstream wiring set, not here.
//!
//! # Scope
//!
//! .xlsx/.xls/.ods (calamine), .csv/.tsv (csv crate), .txt/.md (plain text).
//! SPSS (.sav) and MATLAB (.mat) are DEFERRED follow-ups.

use std::io::Read;
use std::path::Path;

use serde::Serialize;

use gaply_core::GaplyError;

/// Reject any file larger than this BEFORE reading it (metadata check). 10 MB
/// comfortably covers real supplementary tables while bounding the eager Excel
/// load and any single read.
pub const MAX_FILE_SIZE_BYTES: u64 = 10 * 1024 * 1024;
/// Max data rows extracted (excess -> rejected).
pub const MAX_ROWS: usize = 5_000;
/// Max columns per row (excess -> rejected).
pub const MAX_COLS: usize = 128;
/// Max chars kept per cell (excess -> CLAMPED, so one long cell can't blow
/// memory without rejecting an otherwise-fine file).
pub const MAX_CELL_LEN: usize = 512;
/// Hard cap on total extracted characters across all cells/text — the real
/// aggregate memory bound. Exceeding it REJECTS the file.
pub const MAX_TOTAL_CHARS: usize = 2 * 1024 * 1024;

/// The kind of supplementary source.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SupplementaryKind {
    Spreadsheet,
    Csv,
    Text,
}

/// A single extracted table (first row treated as headers).
#[derive(Debug, Clone, Serialize)]
pub struct Table {
    pub headers: Vec<String>,
    pub rows: Vec<Vec<String>>,
}

/// Neutral evidence extracted from one supplementary file.
#[derive(Debug, Clone, Serialize)]
pub struct SupplementaryEvidence {
    pub file_name: String,
    pub kind: SupplementaryKind,
    pub tables: Vec<Table>,
    pub text_summary: String,
    /// Honest notes about caps applied (clamped cells, truncated text).
    pub notes: Vec<String>,
}

/// Parse a supplementary file into neutral evidence, memory-capped. Never
/// panics: malformed/oversize/unsupported inputs return a clear `GaplyError`.
pub fn parse_supplementary(path: &Path) -> Result<SupplementaryEvidence, GaplyError> {
    // Size cap FIRST — metadata only, no content read. A 500 MB file dies here.
    let size = std::fs::metadata(path)
        .map_err(|e| GaplyError::Validation(format!("cannot stat file: {e}")))?
        .len();
    if size > MAX_FILE_SIZE_BYTES {
        return Err(GaplyError::Validation(format!(
            "file too large: {size} bytes exceeds the {MAX_FILE_SIZE_BYTES}-byte cap; \
             not read into memory"
        )));
    }

    let file_name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unnamed")
        .to_string();
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();

    match ext.as_str() {
        "xlsx" | "xls" | "xlsb" | "ods" | "xlsm" => parse_spreadsheet(path, file_name),
        "csv" | "tsv" => parse_csv(path, file_name, if ext == "tsv" { b'\t' } else { b',' }),
        "txt" | "text" | "md" | "log" => parse_text(path, file_name),
        other => Err(GaplyError::Validation(format!(
            "unsupported supplementary format {other:?}; supported: xlsx, xls, ods, csv, tsv, txt, md"
        ))),
    }
}

/// Clamp a cell to [`MAX_CELL_LEN`] chars; returns (clamped, was_clamped).
fn clamp_cell(s: &str) -> (String, bool) {
    if s.chars().count() <= MAX_CELL_LEN {
        (s.to_string(), false)
    } else {
        (s.chars().take(MAX_CELL_LEN).collect(), true)
    }
}

fn cell_to_string(d: &calamine::Data) -> String {
    use calamine::Data;
    match d {
        Data::Empty => String::new(),
        Data::String(s) => s.clone(),
        Data::Float(f) => f.to_string(),
        Data::Int(i) => i.to_string(),
        Data::Bool(b) => b.to_string(),
        // DateTime / Iso / Error variants: a stable debug form is fine for stats context.
        other => format!("{other:?}"),
    }
}

fn parse_spreadsheet(path: &Path, file_name: String) -> Result<SupplementaryEvidence, GaplyError> {
    use calamine::{open_workbook_auto, Reader};

    let mut wb = open_workbook_auto(path)
        .map_err(|e| GaplyError::Validation(format!("cannot open spreadsheet: {e}")))?;
    let sheet = wb
        .sheet_names()
        .first()
        .cloned()
        .ok_or_else(|| GaplyError::Validation("spreadsheet has no sheets".into()))?;
    let range = wb
        .worksheet_range(&sheet)
        .map_err(|e| GaplyError::Validation(format!("cannot read sheet {sheet:?}: {e}")))?;

    let (height, width) = range.get_size();
    if height > MAX_ROWS {
        return Err(GaplyError::Validation(format!(
            "spreadsheet has {height} rows; cap is {MAX_ROWS}"
        )));
    }
    if width > MAX_COLS {
        return Err(GaplyError::Validation(format!(
            "spreadsheet has {width} columns; cap is {MAX_COLS}"
        )));
    }

    let mut notes = Vec::new();
    let mut total = 0usize;
    let mut headers: Vec<String> = Vec::new();
    let mut rows: Vec<Vec<String>> = Vec::new();
    let mut clamped = 0usize;

    for (i, row) in range.rows().enumerate() {
        let mut out_row = Vec::with_capacity(row.len());
        for cell in row {
            let (val, was_clamped) = clamp_cell(&cell_to_string(cell));
            if was_clamped {
                clamped += 1;
            }
            total += val.len();
            if total > MAX_TOTAL_CHARS {
                return Err(GaplyError::Validation(format!(
                    "extracted content exceeds the {MAX_TOTAL_CHARS}-char cap"
                )));
            }
            out_row.push(val);
        }
        if i == 0 {
            headers = out_row;
        } else {
            rows.push(out_row);
        }
    }
    if clamped > 0 {
        notes.push(format!("{clamped} cell(s) clamped to {MAX_CELL_LEN} chars"));
    }

    Ok(SupplementaryEvidence {
        file_name,
        kind: SupplementaryKind::Spreadsheet,
        tables: vec![Table { headers, rows }],
        text_summary: String::new(),
        notes,
    })
}

fn parse_csv(path: &Path, file_name: String, delim: u8) -> Result<SupplementaryEvidence, GaplyError> {
    let file = std::fs::File::open(path)
        .map_err(|e| GaplyError::Validation(format!("cannot open csv: {e}")))?;
    let mut rdr = csv::ReaderBuilder::new()
        .has_headers(false)
        .flexible(true)
        .delimiter(delim)
        .from_reader(std::io::BufReader::new(file));

    let mut notes = Vec::new();
    let mut total = 0usize;
    let mut clamped = 0usize;
    let mut headers: Vec<String> = Vec::new();
    let mut rows: Vec<Vec<String>> = Vec::new();

    // Streaming: only ever one record + our capped output is resident.
    for (i, rec) in rdr.records().enumerate() {
        if i >= MAX_ROWS {
            return Err(GaplyError::Validation(format!(
                "csv has more than {MAX_ROWS} rows; cap is {MAX_ROWS}"
            )));
        }
        let rec = rec.map_err(|e| GaplyError::Validation(format!("malformed csv at row {i}: {e}")))?;
        if rec.len() > MAX_COLS {
            return Err(GaplyError::Validation(format!(
                "csv row {i} has {} columns; cap is {MAX_COLS}",
                rec.len()
            )));
        }
        let mut out_row = Vec::with_capacity(rec.len());
        for field in rec.iter() {
            let (val, was_clamped) = clamp_cell(field);
            if was_clamped {
                clamped += 1;
            }
            total += val.len();
            if total > MAX_TOTAL_CHARS {
                return Err(GaplyError::Validation(format!(
                    "extracted content exceeds the {MAX_TOTAL_CHARS}-char cap"
                )));
            }
            out_row.push(val);
        }
        if i == 0 {
            headers = out_row;
        } else {
            rows.push(out_row);
        }
    }
    if clamped > 0 {
        notes.push(format!("{clamped} cell(s) clamped to {MAX_CELL_LEN} chars"));
    }

    Ok(SupplementaryEvidence {
        file_name,
        kind: SupplementaryKind::Csv,
        tables: vec![Table { headers, rows }],
        text_summary: String::new(),
        notes,
    })
}

fn parse_text(path: &Path, file_name: String) -> Result<SupplementaryEvidence, GaplyError> {
    let file = std::fs::File::open(path)
        .map_err(|e| GaplyError::Validation(format!("cannot open text file: {e}")))?;
    // Read at most MAX_TOTAL_CHARS+1 bytes — bounded regardless of file size.
    let mut buf = Vec::new();
    std::io::BufReader::new(file)
        .take(MAX_TOTAL_CHARS as u64 + 1)
        .read_to_end(&mut buf)
        .map_err(|e| GaplyError::Validation(format!("cannot read text: {e}")))?;

    let mut notes = Vec::new();
    let truncated = buf.len() > MAX_TOTAL_CHARS;
    buf.truncate(MAX_TOTAL_CHARS);
    // Lossy decode so a non-UTF-8 byte can't error; still bounded.
    let mut text = String::from_utf8_lossy(&buf).into_owned();
    if truncated {
        text.truncate(MAX_TOTAL_CHARS);
        notes.push(format!("text truncated to {MAX_TOTAL_CHARS} chars"));
    }

    Ok(SupplementaryEvidence {
        file_name,
        kind: SupplementaryKind::Text,
        tables: Vec::new(),
        text_summary: text,
        notes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn tmp(name: &str, bytes: &[u8]) -> std::path::PathBuf {
        let p = std::env::temp_dir().join(format!("gaply_supp_{}_{}", std::process::id(), name));
        std::fs::File::create(&p).unwrap().write_all(bytes).unwrap();
        p
    }

    #[test]
    fn csv_parses_into_evidence() {
        let p = tmp("ok.csv", b"id,mean,sd\n1,3.2,0.4\n2,4.1,0.6\n");
        let ev = parse_supplementary(&p).unwrap();
        std::fs::remove_file(&p).ok();
        assert_eq!(ev.kind, SupplementaryKind::Csv);
        assert_eq!(ev.tables[0].headers, vec!["id", "mean", "sd"]);
        assert_eq!(ev.tables[0].rows.len(), 2);
        assert_eq!(ev.tables[0].rows[0], vec!["1", "3.2", "0.4"]);
    }

    #[test]
    fn tsv_uses_tab_delimiter() {
        let p = tmp("ok.tsv", b"a\tb\n1\t2\n");
        let ev = parse_supplementary(&p).unwrap();
        std::fs::remove_file(&p).ok();
        assert_eq!(ev.tables[0].headers, vec!["a", "b"]);
        assert_eq!(ev.tables[0].rows[0], vec!["1", "2"]);
    }

    #[test]
    fn xlsx_parses_into_evidence() {
        let p = std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/sample.xlsx"));
        let ev = parse_supplementary(p).unwrap();
        assert_eq!(ev.kind, SupplementaryKind::Spreadsheet);
        assert_eq!(ev.tables[0].headers, vec!["id", "mean", "sd"]);
        assert_eq!(ev.tables[0].rows.len(), 2);
        assert_eq!(ev.tables[0].rows[0][0], "1");
        assert!(ev.tables[0].rows[0][1].starts_with("3.2"));
    }

    #[test]
    fn text_parses_into_summary() {
        let p = tmp("lab.txt", b"Sample prep: n=48 adults. Recall scored next morning.");
        let ev = parse_supplementary(&p).unwrap();
        std::fs::remove_file(&p).ok();
        assert_eq!(ev.kind, SupplementaryKind::Text);
        assert!(ev.text_summary.contains("n=48 adults"));
        assert!(ev.tables.is_empty());
    }

    #[test]
    fn empty_file_is_honest_not_a_panic() {
        let p = tmp("empty.csv", b"");
        let ev = parse_supplementary(&p).unwrap();
        std::fs::remove_file(&p).ok();
        assert!(ev.tables[0].headers.is_empty());
        assert!(ev.tables[0].rows.is_empty());
    }

    #[test]
    fn unsupported_format_is_a_clear_error() {
        let p = tmp("data.sav", b"\x00\x01spss");
        let err = parse_supplementary(&p).unwrap_err();
        std::fs::remove_file(&p).ok();
        assert!(matches!(err, GaplyError::Validation(m) if m.contains("unsupported") && m.contains("sav")));
    }

    #[test]
    fn malformed_spreadsheet_errors_gracefully() {
        // Not a real xlsx (zip) — calamine must error, not panic.
        let p = tmp("fake.xlsx", b"this is not a zip/xlsx file at all");
        let r = parse_supplementary(&p);
        std::fs::remove_file(&p).ok();
        assert!(r.is_err());
    }

    // ---- the CRITICAL memory tests ----

    #[test]
    fn oversize_file_rejected_before_read() {
        // 11 MB of zeros — over MAX_FILE_SIZE_BYTES. Must be rejected by the
        // METADATA check, never read into memory.
        let big = vec![0u8; (MAX_FILE_SIZE_BYTES + 1024 * 1024) as usize];
        let p = tmp("huge.csv", &big);
        drop(big);
        let err = parse_supplementary(&p).unwrap_err();
        std::fs::remove_file(&p).ok();
        assert!(matches!(err, GaplyError::Validation(m) if m.contains("too large") && m.contains("not read")));
    }

    #[test]
    fn too_many_rows_rejected_while_streaming() {
        // A small CSV (well under the size cap) but with more than MAX_ROWS
        // rows — rejected during streaming, having read only ~MAX_ROWS rows.
        let mut data = String::from("id\n");
        for i in 0..(MAX_ROWS + 50) {
            data.push_str(&format!("{i}\n"));
        }
        assert!(data.len() < MAX_FILE_SIZE_BYTES as usize, "fixture must be under the size cap");
        let p = tmp("manyrows.csv", data.as_bytes());
        let err = parse_supplementary(&p).unwrap_err();
        std::fs::remove_file(&p).ok();
        assert!(matches!(err, GaplyError::Validation(m) if m.contains("rows") && m.contains("cap")));
    }

    #[test]
    fn long_cell_is_clamped_not_rejected() {
        let mut data = String::from("note\n");
        data.push_str(&"x".repeat(MAX_CELL_LEN + 500));
        data.push('\n');
        let p = tmp("longcell.csv", data.as_bytes());
        let ev = parse_supplementary(&p).unwrap();
        std::fs::remove_file(&p).ok();
        assert_eq!(ev.tables[0].rows[0][0].chars().count(), MAX_CELL_LEN);
        assert!(ev.notes.iter().any(|n| n.contains("clamped")));
    }
}
