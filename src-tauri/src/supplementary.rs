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
//! .xlsx/.xls/.ods (calamine), .csv/.tsv (csv crate), .txt/.md (plain text),
//! SPSS .sav (`ambers`, pure Rust), MATLAB .mat v5 / `-v7` (`matfile`, pure
//! Rust). **MATLAB 7.3 is NOT supported**: it is an HDF5 container, and every
//! reader for it in the registry either binds libhdf5 or is an unproven 0.x
//! reimplementation. It is DETECTED and refused with an actionable message
//! rather than mis-parsed.

use std::io::Read;
use std::path::Path;

use serde::Serialize;

use gaply_core::GaplyError;

/// Reject any file larger than this BEFORE reading it (metadata check). 10 MB
/// comfortably covers real supplementary tables while bounding the eager Excel
/// load and any single read.
pub const MAX_FILE_SIZE_BYTES: u64 = 10 * 1024 * 1024;
/// Max data rows extracted (excess -> rejected). DATA rows: the header row
/// is not counted, so a table of exactly 5,000 rows is read (evaluation
/// fixes, part C, m-7; it was refused before, the header counted as a row).
pub const MAX_ROWS: usize = 5_000;
/// Max columns per row (excess -> rejected).
pub const MAX_COLS: usize = 128;
/// Max chars kept per cell (excess -> CLAMPED, so one long cell can't blow
/// memory without rejecting an otherwise-fine file).
pub const MAX_CELL_LEN: usize = 512;
/// Hard cap on total extracted characters across all cells/text — the real
/// aggregate memory bound. Exceeding it REJECTS the file.
pub const MAX_TOTAL_CHARS: usize = 2 * 1024 * 1024;

/// The version of this parser, recorded in every Level 1 data fingerprint
/// beside the file's hash (Phase 2, Step 2). Bump it when a parse of the same
/// bytes could change: the reader crates' exact versions are pinned in
/// `Cargo.toml` (`ambers = "=0.4.4"`, `matfile = "=0.5.0"`) and `Cargo.lock`.
pub const PARSER_VERSION: &str = "supplementary ingest 1; calamine 0.26, csv 1, ambers 0.4.4, matfile 0.5.0";

/// The kind of supplementary source.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SupplementaryKind {
    Spreadsheet,
    Csv,
    Text,
    /// SPSS `.sav`.
    Spss,
    /// MATLAB `.mat`, v5 container (what `save('-v7')` writes).
    Matlab,
}

/// **One variable's metadata, preserved because losing it loses the analysis.**
///
/// A CSV column is a name and some cells. An SPSS variable additionally carries
/// its label, its declared type, the labels attached to its codes, and the
/// codes the author declared as MISSING — and a value of 99 means nothing
/// without the last two. Carried beside the table rather than flattened into
/// it, so nothing has to be re-derived from a string.
#[derive(Clone, Serialize)]
pub struct VariableMeta {
    pub name: String,
    /// The source format's own type string (`F8.2`, `A5`), not a re-coding.
    pub declared_type: String,
    pub is_numeric: bool,
    pub label: Option<String>,
    /// `value -> label`, in file order.
    pub value_labels: Vec<(String, String)>,
    /// Codes/ranges the author declared user-missing, verbatim.
    pub user_missing: Vec<String>,
    /// The same declarations, typed, for Level 1 (Phase 2, Step 2). Not
    /// serialised: `SupplementaryEvidence` is serialised toward the
    /// PublishReady reviewer, and that payload must not change.
    #[serde(skip)]
    pub user_missing_rules: Vec<gaply_core::stats_l1::dataset::MissingRule>,
    /// `code -> label` with the code written as the cells print it (`1`, not
    /// `Numeric(1.0)`), for Level 1's level labels. Not serialised, as above.
    #[serde(skip)]
    pub value_label_codes: Vec<(String, String)>,
}

/// Counts only, never the text (part C follow-up, item 7).
impl std::fmt::Debug for VariableMeta {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VariableMeta").field("is_numeric", &self.is_numeric).field("label", &self.label.is_some()).field("value_labels", &self.value_labels.len()).field("user_missing", &self.user_missing.len()).finish_non_exhaustive()
    }
}

/// A single extracted table (first row treated as headers).
#[derive(Clone, Serialize)]
pub struct Table {
    pub headers: Vec<String>,
    pub rows: Vec<Vec<String>>,
}

/// Counts only, never the text (part C follow-up, item 7).
impl std::fmt::Debug for Table {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Table").field("columns", &self.headers.len()).field("rows", &self.rows.len()).finish_non_exhaustive()
    }
}

/// Neutral evidence extracted from one supplementary file.
#[derive(Clone, Serialize)]
pub struct SupplementaryEvidence {
    pub file_name: String,
    pub kind: SupplementaryKind,
    pub tables: Vec<Table>,
    pub text_summary: String,
    /// Honest notes about caps applied (clamped cells, truncated text).
    pub notes: Vec<String>,
    /// Per-variable metadata, when the format carries any. Empty for CSV and
    /// plain text, which have none to lose.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub variables: Vec<VariableMeta>,
    /// The SPSS weight variable the file declares, if any (Phase 2, Step 2).
    /// Not serialised: see `VariableMeta::user_missing_rules`.
    #[serde(skip)]
    pub weight_variable: Option<String>,
    /// `true` when the source holds more than [`MAX_ROWS`] rows and only the
    /// first [`MAX_ROWS`] are in `tables` (`.sav` and `.mat`; CSV and
    /// spreadsheets refuse instead). Level 1 never computes on a truncated
    /// table. Not serialised.
    #[serde(skip)]
    pub rows_truncated: bool,
    /// The sheet a spreadsheet's table was read from. Not serialised.
    #[serde(skip)]
    pub sheet_name: Option<String>,
}

/// Counts only, never the text (part C follow-up, item 7).
impl std::fmt::Debug for SupplementaryEvidence {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SupplementaryEvidence").field("kind", &self.kind).field("tables", &self.tables).field("text_chars", &self.text_summary.chars().count()).field("notes", &self.notes.len()).field("variables", &self.variables.len()).field("weight_variable", &self.weight_variable.is_some()).field("rows_truncated", &self.rows_truncated).finish_non_exhaustive()
    }
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
        "sav" => parse_sav(path, file_name),
        "mat" => parse_mat(path, file_name),
        "xlsx" | "xls" | "xlsb" | "ods" | "xlsm" => parse_spreadsheet(path, file_name),
        "csv" | "tsv" => parse_csv(path, file_name, if ext == "tsv" { b'\t' } else { b',' }),
        "txt" | "text" | "md" | "log" => parse_text(path, file_name),
        other => Err(GaplyError::Validation(format!(
            "unsupported supplementary format {other:?}; supported: xlsx, xls, ods, \
             csv, tsv, txt, md, sav, mat"
        ))),
    }
}

/// **Level 1 only (part C follow-up, item 3): [`parse_supplementary`] on
/// bytes already read**, so the file that was checked and hashed is the file
/// parsed, and the path is never opened again. `ext` is the lower-case
/// extension, which picks the parser exactly as the path does. Each format is
/// parsed by the same code as through the path, from a reader over `bytes`;
/// the evidence's `file_name` is empty (Level 1 never reads it).
pub fn parse_supplementary_bytes(bytes: &[u8], ext: &str) -> Result<SupplementaryEvidence, GaplyError> {
    let size = bytes.len() as u64;
    if size > MAX_FILE_SIZE_BYTES {
        return Err(GaplyError::Validation(format!(
            "file too large: {size} bytes exceeds the {MAX_FILE_SIZE_BYTES}-byte cap; \
             not read into memory"
        )));
    }
    let file_name = String::new();
    match ext {
        "sav" => sav_evidence(ambers::scan_sav_from_reader(std::io::Cursor::new(bytes), SAV_BATCH_ROWS).and_then(read_sav_rows), file_name),
        "mat" => mat_evidence(bytes, file_name),
        "xlsx" | "xls" | "xlsb" | "ods" | "xlsm" => spreadsheet_evidence(workbook_from_bytes(bytes, ext), file_name),
        "csv" | "tsv" => csv_evidence(bytes, file_name, if ext == "tsv" { b'\t' } else { b',' }),
        "txt" | "text" | "md" | "log" => text_evidence(bytes, file_name),
        other => Err(GaplyError::Validation(format!(
            "unsupported supplementary format {other:?}; supported: xlsx, xls, ods, \
             csv, tsv, txt, md, sav, mat"
        ))),
    }
}

/// The rows `ambers::scan_sav` reads per batch; the bytes path uses the same.
const SAV_BATCH_ROWS: usize = 100_000;

/// A workbook from bytes, by extension exactly as `open_workbook_auto` picks
/// one from a path.
fn workbook_from_bytes<'a>(bytes: &'a [u8], ext: &str) -> Result<calamine::Sheets<std::io::Cursor<&'a [u8]>>, calamine::Error> {
    use calamine::{open_workbook_from_rs, Error, Ods, Sheets, Xls, Xlsb, Xlsx};
    let c = std::io::Cursor::new(bytes);
    Ok(match ext {
        "xls" => Sheets::Xls(open_workbook_from_rs::<Xls<_>, _>(c).map_err(Error::Xls)?),
        "xlsx" | "xlsm" => Sheets::Xlsx(open_workbook_from_rs::<Xlsx<_>, _>(c).map_err(Error::Xlsx)?),
        "xlsb" => Sheets::Xlsb(open_workbook_from_rs::<Xlsb<_>, _>(c).map_err(Error::Xlsb)?),
        "ods" => Sheets::Ods(open_workbook_from_rs::<Ods<_>, _>(c).map_err(Error::Ods)?),
        _ => calamine::open_workbook_auto_from_rs(c)?,
    })
}

/// **Level 1 only (Phase 2, Step 2): does the file's table hold more rows
/// than [`MAX_ROWS`]?** A new function that [`parse_supplementary`] and its
/// existing callers never call, so their refusals keep their exact text.
///
/// Level 1 calls it after a parse has FAILED, to tell the row-cap refusal
/// apart from every other one without reading the message. It mirrors the
/// parsers' own row checks: CSV/TSV count records with the same reader
/// settings and stop where the parser would have stopped first (a malformed
/// record, a row over the column cap); a spreadsheet reads its first sheet's
/// height, as `parse_spreadsheet` does. `.sav` and `.mat` never refuse on
/// rows (they truncate, and say so in `rows_truncated`), and every other
/// format holds no table, so both answer `false`.
pub fn exceeds_row_cap(path: &Path) -> Result<bool, GaplyError> {
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
    match ext.as_str() {
        "csv" | "tsv" => {
            let file = std::fs::File::open(path)
                .map_err(|e| GaplyError::Validation(format!("cannot open csv: {e}")))?;
            Ok(csv_exceeds_row_cap(std::io::BufReader::new(file), &ext))
        }
        "xlsx" | "xls" | "xlsb" | "ods" | "xlsm" => Ok(sheet_exceeds_row_cap(calamine::open_workbook_auto(path))),
        _ => Ok(false),
    }
}

/// [`exceeds_row_cap`] on bytes already read (part C follow-up, item 3).
pub fn exceeds_row_cap_bytes(bytes: &[u8], ext: &str) -> bool {
    match ext {
        "csv" | "tsv" => csv_exceeds_row_cap(bytes, ext),
        "xlsx" | "xls" | "xlsb" | "ods" | "xlsm" => sheet_exceeds_row_cap(workbook_from_bytes(bytes, ext)),
        _ => false,
    }
}

fn csv_exceeds_row_cap(reader: impl Read, ext: &str) -> bool {
    let mut rdr = csv::ReaderBuilder::new()
        .has_headers(false)
        .flexible(true)
        .delimiter(if ext == "tsv" { b'\t' } else { b',' })
        .from_reader(reader);
    for (i, rec) in rdr.records().enumerate() {
        if i > MAX_ROWS {
            return true;
        }
        match rec {
            Ok(r) if r.len() <= MAX_COLS => {}
            _ => return false,
        }
    }
    false
}

fn sheet_exceeds_row_cap<RS: Read + std::io::Seek>(wb: Result<calamine::Sheets<RS>, calamine::Error>) -> bool {
    use calamine::Reader;
    let Ok(mut wb) = wb else { return false };
    let Some(sheet) = wb.sheet_names().first().cloned() else { return false };
    let Ok(range) = wb.worksheet_range(&sheet) else { return false };
    range.get_size().0 > MAX_ROWS + 1
}

/// **Level 1 only (Phase 2, Step 8b): the column count of a table wider
/// than [`MAX_COLS`].** Like [`exceeds_row_cap`], called only after a parse
/// has FAILED, to tell the column-cap refusal apart without reading the
/// message. It mirrors the parsers' order: CSV/TSV records are read with the
/// same settings until the first that is malformed, past the row cap, or
/// over the column cap; a spreadsheet's first sheet is too tall before it is
/// too wide. `None` when the table is not too wide, or the parse failed for
/// another reason first.
pub fn exceeds_col_cap(path: &Path) -> Result<Option<usize>, GaplyError> {
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
    match ext.as_str() {
        "csv" | "tsv" => {
            let file = std::fs::File::open(path)
                .map_err(|e| GaplyError::Validation(format!("cannot open csv: {e}")))?;
            Ok(csv_exceeds_col_cap(std::io::BufReader::new(file), &ext))
        }
        "xlsx" | "xls" | "xlsb" | "ods" | "xlsm" => Ok(sheet_exceeds_col_cap(calamine::open_workbook_auto(path))),
        _ => Ok(None),
    }
}

/// [`exceeds_col_cap`] on bytes already read (part C follow-up, item 3).
pub fn exceeds_col_cap_bytes(bytes: &[u8], ext: &str) -> Option<usize> {
    match ext {
        "csv" | "tsv" => csv_exceeds_col_cap(bytes, ext),
        "xlsx" | "xls" | "xlsb" | "ods" | "xlsm" => sheet_exceeds_col_cap(workbook_from_bytes(bytes, ext)),
        _ => None,
    }
}

fn csv_exceeds_col_cap(reader: impl Read, ext: &str) -> Option<usize> {
    let mut rdr = csv::ReaderBuilder::new()
        .has_headers(false)
        .flexible(true)
        .delimiter(if ext == "tsv" { b'\t' } else { b',' })
        .from_reader(reader);
    for (i, rec) in rdr.records().enumerate() {
        if i > MAX_ROWS {
            return None;
        }
        match rec {
            Ok(r) if r.len() > MAX_COLS => return Some(r.len()),
            Ok(_) => {}
            Err(_) => return None,
        }
    }
    None
}

fn sheet_exceeds_col_cap<RS: Read + std::io::Seek>(wb: Result<calamine::Sheets<RS>, calamine::Error>) -> Option<usize> {
    use calamine::Reader;
    let mut wb = wb.ok()?;
    let sheet = wb.sheet_names().first().cloned()?;
    let range = wb.worksheet_range(&sheet).ok()?;
    let (height, width) = range.get_size();
    (height <= MAX_ROWS + 1 && width > MAX_COLS).then_some(width)
}

/// **Level 1 only (Phase 2, Step 8b): a `.xls` that is not a readable
/// compound document**, told apart by the parser's TYPED error
/// (`XlsError::Cfb`), never its text: an Excel format older than BIFF8
/// (BIFF2 to BIFF5 workbooks are not compound files) or a damaged file. The
/// parser's error does not say which, so neither does the refusal.
pub fn xls_not_compound_document(path: &Path) -> bool {
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
    ext == "xls" && matches!(calamine::open_workbook::<calamine::Xls<_>, _>(path), Err(calamine::XlsError::Cfb(_)))
}

/// [`xls_not_compound_document`] on bytes already read (part C follow-up,
/// item 3).
pub fn xls_not_compound_document_bytes(bytes: &[u8], ext: &str) -> bool {
    ext == "xls"
        && matches!(
            calamine::open_workbook_from_rs::<calamine::Xls<_>, _>(std::io::Cursor::new(bytes)),
            Err(calamine::XlsError::Cfb(_))
        )
}

/// **Level 1 and the descriptive summary (evaluation of Research OS Phases
/// 2 and 3, F4): does the table hold more than [`MAX_TOTAL_CHARS`] bytes of
/// cell text?** Like [`exceeds_row_cap_bytes`], called only after a parse has
/// FAILED, to tell the content cap apart without reading the message. It
/// mirrors the parsers' order and count: CSV/TSV records are read with the
/// same settings and stop at the first that is past the row cap, malformed
/// or over the column cap (each its own refusal); a spreadsheet's first
/// sheet is checked for its row and column caps first. Each cell is clamped
/// as the parser clamps it and its UTF-8 length added, header included.
pub fn exceeds_content_cap_bytes(bytes: &[u8], ext: &str) -> bool {
    match ext {
        "csv" | "tsv" => csv_exceeds_content_cap(bytes, ext),
        "xlsx" | "xls" | "xlsb" | "ods" | "xlsm" => sheet_exceeds_content_cap(workbook_from_bytes(bytes, ext)),
        _ => false,
    }
}

fn csv_exceeds_content_cap(reader: impl Read, ext: &str) -> bool {
    let mut rdr = csv::ReaderBuilder::new()
        .has_headers(false)
        .flexible(true)
        .delimiter(if ext == "tsv" { b'\t' } else { b',' })
        .from_reader(reader);
    let mut total = 0usize;
    for (i, rec) in rdr.records().enumerate() {
        if i > MAX_ROWS {
            return false;
        }
        let Ok(rec) = rec else { return false };
        if rec.len() > MAX_COLS {
            return false;
        }
        for field in rec.iter() {
            total += clamp_cell(field).0.len();
            if total > MAX_TOTAL_CHARS {
                return true;
            }
        }
    }
    false
}

fn sheet_exceeds_content_cap<RS: Read + std::io::Seek>(wb: Result<calamine::Sheets<RS>, calamine::Error>) -> bool {
    use calamine::Reader;
    let Ok(mut wb) = wb else { return false };
    let Some(sheet) = wb.sheet_names().first().cloned() else { return false };
    let Ok(range) = wb.worksheet_range(&sheet) else { return false };
    let (height, width) = range.get_size();
    if height > MAX_ROWS + 1 || width > MAX_COLS {
        return false;
    }
    let mut total = 0usize;
    for row in range.rows() {
        for cell in row {
            total += clamp_cell(&cell_to_string(cell)).0.len();
            if total > MAX_TOTAL_CHARS {
                return true;
            }
        }
    }
    false
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
    spreadsheet_evidence(calamine::open_workbook_auto(path), file_name)
}

/// The spreadsheet parser after the workbook is opened, from a path or from
/// bytes.
fn spreadsheet_evidence<RS: Read + std::io::Seek>(
    wb: Result<calamine::Sheets<RS>, calamine::Error>,
    file_name: String,
) -> Result<SupplementaryEvidence, GaplyError> {
    use calamine::Reader;

    let mut wb = wb.map_err(|e| GaplyError::Validation(format!("cannot open spreadsheet: {e}")))?;
    let sheet = wb
        .sheet_names()
        .first()
        .cloned()
        .ok_or_else(|| GaplyError::Validation("spreadsheet has no sheets".into()))?;
    let range = wb
        .worksheet_range(&sheet)
        .map_err(|e| GaplyError::Validation(format!("cannot read sheet {sheet:?}: {e}")))?;

    let (height, width) = range.get_size();
    // The header row is not a data row (evaluation fixes, part C, m-7).
    if height > MAX_ROWS + 1 {
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
        variables: Vec::new(),
        weight_variable: None,
        rows_truncated: false,
        sheet_name: Some(sheet),
    })
}

/// **MATLAB 7.3 is an HDF5 container and is refused, not mis-parsed.**
///
/// Detected from the file's own bytes: a `.mat` v5 header is 128 bytes of
/// descriptive text followed by a version/endian field, and a 7.3 file follows
/// that header with the HDF5 signature `\x89HDF\r\n\x1a\n`. Matching the
/// SIGNATURE rather than the header text is deliberate — the text says
/// "MATLAB 7.3 MAT-file" on files written by MATLAB and says other things on
/// files written by everything else, while the signature is the format.
fn is_matlab_73(bytes: &[u8]) -> bool {
    const HDF5_SIGNATURE: &[u8] = b"\x89HDF\r\n\x1a\n";
    bytes.windows(HDF5_SIGNATURE.len()).take(1024).any(|w| w == HDF5_SIGNATURE)
}

/// MATLAB `.mat`, v5 container — what `save('file.mat','-v7')` writes.
fn parse_mat(path: &Path, file_name: String) -> Result<SupplementaryEvidence, GaplyError> {
    let bytes = std::fs::read(path)
        .map_err(|e| GaplyError::Validation(format!("cannot read .mat: {e}")))?;
    mat_evidence(&bytes, file_name)
}

/// The `.mat` parser on the file's bytes, read from a path or already read.
fn mat_evidence(bytes: &[u8], file_name: String) -> Result<SupplementaryEvidence, GaplyError> {
    if is_matlab_73(bytes) {
        return Err(GaplyError::Validation(
            "MATLAB 7.3 files are not supported yet; re-save with \
             save('file.mat','-v7')"
                .to_string(),
        ));
    }
    // MEASURED LIMITATIONS OF `matfile` 0.5.0, 22 Sep 2026, by single-type probe:
    //
    //   f64  OK      i64  OK      u8   OK      f32  OK      2-D  OK
    //   i32  FAIL — "An error occurred while parsing the file", WHOLE FILE
    //
    // char and cell arrays do NOT fail; the reader returns the numeric
    // variables and drops them SILENTLY. So the two failure modes are
    // different and are reported differently: a dropped variable is a note, an
    // int32 variable is a refusal, because nothing can be recovered from the
    // file once the parse aborts.
    let mat = matfile::MatFile::parse(std::io::Cursor::new(bytes)).map_err(|e| {
        GaplyError::Validation(format!(
            "malformed or unsupported .mat file: {e}. Note: this reader cannot \
             parse int32 variables and fails on the whole file when one is \
             present; re-saving the affected variables as double or int64 is a \
             workaround"
        ))
    })?;

    let mut headers = Vec::new();
    let mut columns: Vec<Vec<String>> = Vec::new();
    let mut variables = Vec::new();
    let mut notes = Vec::new();
    let mut rows_truncated = false;
    for arr in mat.arrays().iter().take(MAX_COLS) {
        let (declared_type, cells, truncated) = matlab_cells(arr);
        rows_truncated |= truncated;
        headers.push(arr.name().to_string());
        variables.push(VariableMeta {
            name: arr.name().to_string(),
            declared_type,
            is_numeric: true,
            // A v5 .mat carries variable NAMES and dimensions; it has no
            // variable-label, value-label or user-missing concept at all.
            // Empty here is the format's own absence, not a parse failure.
            label: None,
            value_labels: Vec::new(),
            user_missing: Vec::new(),
            user_missing_rules: Vec::new(),
            value_label_codes: Vec::new(),
        });
        columns.push(cells);
    }
    if mat.arrays().len() > MAX_COLS {
        notes.push(format!(
            "{} of {} variables kept; cap is {MAX_COLS}",
            MAX_COLS,
            mat.arrays().len()
        ));
    }
    // NOTHING DROPPED IS HIDDEN. The reader returns only the variables it
    // understood, so a char or cell variable vanishes with no signal. Count
    // the container's own variable entries and say how many did not survive.
    let declared = count_mat_variables(bytes);
    if declared > mat.arrays().len() {
        notes.push(format!(
            "{} of {declared} variables in the file were not read: char/cell \
             arrays are not supported yet by this reader and are skipped",
            declared - mat.arrays().len()
        ));
    }
    let rows = columns_to_rows(&columns, &mut notes);
    Ok(SupplementaryEvidence {
        file_name,
        kind: SupplementaryKind::Matlab,
        text_summary: format!("MATLAB v5 file with {} variable(s)", headers.len()),
        tables: vec![Table { headers, rows }],
        notes,
        variables,
        weight_variable: None,
        rows_truncated,
        sheet_name: None,
    })
}

/// **How many variables the FILE declares, counted from the container.**
///
/// `matfile` returns only what it parsed, so a char or cell variable leaves no
/// trace in its output and a reader is told nothing. A v5 `.mat` is a flat
/// sequence of data elements after the 128-byte header, each with an 8-byte
/// tag `(type, byte_length)`; a top-level variable is `miMATRIX` = 14. Walking
/// those tags counts the variables the file CLAIMS, which is all this needs —
/// it reads no array contents and decodes no types.
fn count_mat_variables(bytes: &[u8]) -> usize {
    const HEADER: usize = 128;
    const MI_MATRIX: u32 = 14;
    if bytes.len() <= HEADER + 8 {
        return 0;
    }
    // Endianness is declared by the two bytes at 126: "IM" little, "MI" big.
    let little = &bytes[126..128] == b"IM";
    let u32_at = |o: usize| -> u32 {
        let b = [bytes[o], bytes[o + 1], bytes[o + 2], bytes[o + 3]];
        if little { u32::from_le_bytes(b) } else { u32::from_be_bytes(b) }
    };
    let (mut off, mut n) = (HEADER, 0usize);
    while off + 8 <= bytes.len() {
        let (ty, len) = (u32_at(off), u32_at(off + 4) as usize);
        if len == 0 || len > bytes.len() {
            break;
        }
        if ty == MI_MATRIX {
            n += 1;
        }
        // Elements are padded to an 8-byte boundary.
        off += 8 + len.div_ceil(8) * 8;
    }
    n
}

/// **What a `.mat` file's compressed elements inflate to, counted** (part C
/// follow-up 2). `matfile` inflates each `miCOMPRESSED` element with
/// `read_to_end` and nothing bounds it: measured, a 7.7 MB file reached a
/// 1,048 MB peak. Each top-level compressed element is inflated here into a
/// sink that only counts (flate2, approved for this alone), stopping as soon
/// as the total passes `limit`. `Err(counted)` past the limit; `Ok(total)`
/// otherwise, including for a file this walk cannot read, which is left to
/// the parser to refuse. Nothing inflated is kept.
///
/// The walk: after the 128-byte header, each element is an 8-byte tag
/// `(type, length)`; a compressed element (15) is not padded, every other is
/// padded to 8 bytes (`count_mat_variables` pads every element, which is
/// wrong only after a compressed one; it is left as it is).
pub fn count_mat_inflation(bytes: &[u8], limit: u64) -> Result<u64, u64> {
    const HEADER: usize = 128;
    const MI_COMPRESSED: u32 = 15;
    if bytes.len() <= HEADER + 8 {
        return Ok(0);
    }
    let little = &bytes[126..128] == b"IM";
    let u32_at = |o: usize| -> u32 {
        let b = [bytes[o], bytes[o + 1], bytes[o + 2], bytes[o + 3]];
        if little { u32::from_le_bytes(b) } else { u32::from_be_bytes(b) }
    };
    let (mut off, mut total) = (HEADER, 0u64);
    while off + 8 <= bytes.len() {
        let (ty, len) = (u32_at(off), u32_at(off + 4) as usize);
        let Some(data) = bytes.get(off + 8..off + 8 + len) else { break };
        if ty == MI_COMPRESSED {
            let room = limit.saturating_sub(total);
            let mut counted = flate2::read::ZlibDecoder::new(data).take(room.saturating_add(1));
            let Ok(n) = std::io::copy(&mut counted, &mut std::io::sink()) else { break };
            total = total.saturating_add(n);
            if n > room {
                return Err(total);
            }
            off += 8 + len;
        } else {
            off += 8 + len.div_ceil(8) * 8;
        }
    }
    Ok(total)
}

/// What a zlib-compressed SPSS file's trailer declares (part C follow-up 2,
/// item 3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ZsavTrailer {
    /// Not a `$FL3` file: nothing to read.
    NotZsav,
    /// The most any one allocation or all blocks together would take, from
    /// the declared block size and each block's declared inflated size.
    Declared { inflated: u64 },
    /// No zlib header points at a trailer the file holds, or the trailer
    /// cannot be what it declares (a negative size, a block count its length
    /// does not hold, a block outside the file).
    Malformed,
}

/// **The trailer of a `.zsav`, read before ambers reads it.** Ambers
/// allocates each block at the size the trailer declares and the trailer's
/// entries at the count it declares; measured, a trailer declaring a 2 GiB
/// block made a 2,048 MB allocation, one declaring 2^31 - 1 blocks a
/// 49,152 MB one, and a negative size panicked. The zlib header is found by
/// its own shape (its first field is its own offset, and its trailer offset
/// and length end the file), in either byte order.
pub fn zsav_trailer(bytes: &[u8]) -> ZsavTrailer {
    if bytes.get(..4) != Some(&b"$FL3"[..]) {
        return ZsavTrailer::NotZsav;
    }
    for little in [true, false] {
        let i64_at = |o: usize| -> Option<i64> {
            let b: [u8; 8] = bytes.get(o..o + 8)?.try_into().ok()?;
            Some(if little { i64::from_le_bytes(b) } else { i64::from_be_bytes(b) })
        };
        let i32_at = |o: usize| -> Option<i32> {
            let b: [u8; 4] = bytes.get(o..o + 4)?.try_into().ok()?;
            Some(if little { i32::from_le_bytes(b) } else { i32::from_be_bytes(b) })
        };
        let len = bytes.len() as i64;
        let header = (0..bytes.len().saturating_sub(24)).find(|&p| {
            matches!((i64_at(p), i64_at(p + 8), i64_at(p + 16)), (Some(a), Some(off), Some(n)) if a == p as i64 && off > 0 && n > 0 && off.checked_add(n) == Some(len))
        });
        let Some(p) = header else { continue };
        let (Some(zt), Some(zlen)) = (i64_at(p + 8), i64_at(p + 16)) else { return ZsavTrailer::Malformed };
        let zt = zt as usize;
        let (Some(block), Some(blocks)) = (i32_at(zt + 16), i32_at(zt + 20)) else { return ZsavTrailer::Malformed };
        if block <= 0 || blocks < 0 || i64::from(blocks).checked_mul(24).and_then(|x| x.checked_add(24)) != Some(zlen) {
            return ZsavTrailer::Malformed;
        }
        let mut inflated: u64 = block as u64;
        let mut total: u64 = 0;
        for k in 0..blocks as usize {
            let e = zt + 24 + 24 * k;
            let (Some(at), Some(size), Some(packed)) = (i64_at(e + 8), i32_at(e + 16), i32_at(e + 20)) else { return ZsavTrailer::Malformed };
            if at < 0 || size < 0 || packed < 0 || at.checked_add(i64::from(packed)).map_or(true, |end| end > len) {
                return ZsavTrailer::Malformed;
            }
            inflated = inflated.max(size as u64);
            total = total.saturating_add(size as u64);
        }
        return ZsavTrailer::Declared { inflated: inflated.max(total) };
    }
    ZsavTrailer::Malformed
}

/// One MATLAB array as `(declared_type, cells, truncated)`: `truncated` says
/// the array held more than [`MAX_ROWS`] values and only the first are kept.
/// Numeric data is rendered with `serde_json`-style shortest round-trip so a
/// value is never re-formatted into something the author did not write
/// (ONTOLOGY §4.20, TEXT).
fn matlab_cells(arr: &matfile::Array) -> (String, Vec<String>, bool) {
    use matfile::NumericData;
    let dims = arr
        .size()
        .iter()
        .map(|d| d.to_string())
        .collect::<Vec<_>>()
        .join("x");
    macro_rules! render {
        ($v:expr) => {
            ($v.iter().take(MAX_ROWS).map(|x| x.to_string()).collect::<Vec<_>>(), $v.len() > MAX_ROWS)
        };
    }
    let (cells, truncated) = match arr.data() {
        NumericData::Double { real, .. } => render!(real),
        NumericData::Single { real, .. } => render!(real),
        NumericData::Int8 { real, .. } => render!(real),
        NumericData::UInt8 { real, .. } => render!(real),
        NumericData::Int16 { real, .. } => render!(real),
        NumericData::UInt16 { real, .. } => render!(real),
        NumericData::Int32 { real, .. } => render!(real),
        NumericData::UInt32 { real, .. } => render!(real),
        NumericData::Int64 { real, .. } => render!(real),
        NumericData::UInt64 { real, .. } => render!(real),
    };
    (format!("matlab:{dims}"), cells, truncated)
}

/// SPSS `.sav`, via `ambers` (pure Rust).
fn parse_sav(path: &Path, file_name: String) -> Result<SupplementaryEvidence, GaplyError> {
    sav_evidence(ambers::scan_sav(path).and_then(read_sav_rows), file_name)
}

/// The rows of an opened `.sav` and its metadata: at most [`MAX_ROWS`] + 1
/// rows, enough to keep [`MAX_ROWS`] and to know there were more (part C
/// follow-up, item 4). A byte-coded or zlib-compressed (`.zsav`) file holds
/// rows far beyond its size on disk, and `ambers::read_sav` reads every one:
/// measured, 40 million rows in a 40 KB file peaked at 330 MB. The output is
/// unchanged: it was already cut to [`MAX_ROWS`] rows.
fn read_sav_rows<R: Read + std::io::Seek>(
    mut scanner: ambers::Scanner<R>,
) -> Result<(arrow::record_batch::RecordBatch, ambers::SpssMetadata), ambers::error::SpssError> {
    let meta = scanner.metadata().clone();
    scanner.limit(MAX_ROWS + 1);
    let batch = scanner.collect_single()?;
    Ok((batch, meta))
}

/// The `.sav` parser after `ambers` has read the file, from a path or from
/// bytes.
fn sav_evidence(
    read: Result<(arrow::record_batch::RecordBatch, ambers::SpssMetadata), ambers::error::SpssError>,
    file_name: String,
) -> Result<SupplementaryEvidence, GaplyError> {
    let (batch, meta) =
        read.map_err(|e| GaplyError::Validation(format!("malformed or unsupported .sav file: {e}")))?;

    let mut notes = Vec::new();
    let mut headers = Vec::new();
    let mut variables = Vec::new();
    for name in meta.variable_names.iter().take(MAX_COLS) {
        let declared_type =
            meta.variable_formats.get(name).cloned().unwrap_or_else(|| "unknown".into());
        // SPSS's own type letter: `A*` is a string variable, everything else
        // numeric. Taken from the file's declared format, not inferred.
        let is_numeric = !declared_type.trim_start().starts_with('A');
        variables.push(VariableMeta {
            name: name.clone(),
            declared_type,
            is_numeric,
            label: meta.variable_labels.get(name).cloned(),
            value_labels: meta
                .variable_value_labels
                .get(name)
                .map(|m| m.iter().map(|(v, l)| (format!("{v:?}"), l.clone())).collect())
                .unwrap_or_default(),
            user_missing: meta
                .variable_missing_values
                .get(name)
                .map(|specs| specs.iter().map(|s| format!("{s:?}")).collect())
                .unwrap_or_default(),
            user_missing_rules: meta
                .variable_missing_values
                .get(name)
                .map(|specs| specs.iter().map(missing_rule).collect())
                .unwrap_or_default(),
            value_label_codes: meta
                .variable_value_labels
                .get(name)
                .map(|m| m.iter().map(|(v, l)| (value_code(v), l.clone())).collect())
                .unwrap_or_default(),
        });
        headers.push(name.clone());
    }
    if meta.variable_names.len() > MAX_COLS {
        notes.push(format!(
            "{} of {} variables kept; cap is {MAX_COLS}",
            MAX_COLS,
            meta.variable_names.len()
        ));
    }
    let columns: Vec<Vec<String>> = (0..headers.len())
        .map(|i| {
            let col = batch.column(i);
            (0..col.len().min(MAX_ROWS))
                .map(|r| arrow_cell(col, r))
                .collect()
        })
        .collect();
    let rows = columns_to_rows(&columns, &mut notes);
    Ok(SupplementaryEvidence {
        file_name,
        kind: SupplementaryKind::Spss,
        text_summary: format!(
            "SPSS file with {} variable(s) and {} case(s)",
            headers.len(),
            meta.number_rows.unwrap_or_default()
        ),
        tables: vec![Table { headers, rows }],
        notes,
        variables,
        weight_variable: meta.weight_variable.clone(),
        rows_truncated: batch.num_rows() > MAX_ROWS,
        sheet_name: None,
    })
}

/// An SPSS user-missing declaration, typed.
fn missing_rule(spec: &ambers::MissingSpec) -> gaply_core::stats_l1::dataset::MissingRule {
    use gaply_core::stats_l1::dataset::MissingRule;
    match spec {
        ambers::MissingSpec::Value(v) => MissingRule::Value(*v),
        ambers::MissingSpec::Range { lo, hi } => MissingRule::Range { lo: *lo, hi: *hi },
        ambers::MissingSpec::StringValue(s) => MissingRule::Text(s.clone()),
    }
}

/// A value-label key as the cells print it: `1` for `Numeric(1.0)`, the
/// string itself for a string code.
fn value_code(v: &ambers::Value) -> String {
    match v {
        ambers::Value::Numeric(x) => gaply_core::stats_l1::dataset::number_code(*x),
        ambers::Value::String(s) => s.clone(),
    }
}

/// One Arrow cell as the string the file holds. A null is the empty string,
/// which is what a missing cell is in every other parser here.
fn arrow_cell(col: &std::sync::Arc<dyn arrow::array::Array>, row: usize) -> String {
    use arrow::util::display::{ArrayFormatter, FormatOptions};
    if col.is_null(row) {
        return String::new();
    }
    ArrayFormatter::try_new(col.as_ref(), &FormatOptions::default())
        .map(|f| f.value(row).to_string())
        .unwrap_or_default()
}

/// Column-major -> row-major, padding short columns so every row has the same
/// width. Ragged input is a property of the file, not an error.
fn columns_to_rows(columns: &[Vec<String>], notes: &mut Vec<String>) -> Vec<Vec<String>> {
    let depth = columns.iter().map(|c| c.len()).max().unwrap_or(0).min(MAX_ROWS);
    if columns.iter().any(|c| c.len() != depth) {
        notes.push("variables have differing lengths; short ones padded".into());
    }
    (0..depth)
        .map(|r| columns.iter().map(|c| c.get(r).cloned().unwrap_or_default()).collect())
        .collect()
}

fn parse_csv(path: &Path, file_name: String, delim: u8) -> Result<SupplementaryEvidence, GaplyError> {
    let file = std::fs::File::open(path)
        .map_err(|e| GaplyError::Validation(format!("cannot open csv: {e}")))?;
    csv_evidence(std::io::BufReader::new(file), file_name, delim)
}

/// The CSV parser on a reader: an opened file, or bytes already read.
fn csv_evidence(reader: impl Read, file_name: String, delim: u8) -> Result<SupplementaryEvidence, GaplyError> {
    let mut rdr = csv::ReaderBuilder::new()
        .has_headers(false)
        .flexible(true)
        .delimiter(delim)
        .from_reader(reader);

    let mut notes = Vec::new();
    let mut total = 0usize;
    let mut clamped = 0usize;
    let mut headers: Vec<String> = Vec::new();
    let mut rows: Vec<Vec<String>> = Vec::new();

    // Streaming: only ever one record + our capped output is resident.
    for (i, rec) in rdr.records().enumerate() {
        if i > MAX_ROWS {
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
        variables: Vec::new(),
        weight_variable: None,
        rows_truncated: false,
        sheet_name: None,
    })
}

fn parse_text(path: &Path, file_name: String) -> Result<SupplementaryEvidence, GaplyError> {
    let file = std::fs::File::open(path)
        .map_err(|e| GaplyError::Validation(format!("cannot open text file: {e}")))?;
    text_evidence(std::io::BufReader::new(file), file_name)
}

/// The text parser on a reader: an opened file, or bytes already read.
fn text_evidence(reader: impl Read, file_name: String) -> Result<SupplementaryEvidence, GaplyError> {
    // Read at most MAX_TOTAL_CHARS+1 bytes — bounded regardless of file size.
    let mut buf = Vec::new();
    reader
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
        variables: Vec::new(),
        weight_variable: None,
        rows_truncated: false,
        sheet_name: None,
    })
}

#[cfg(test)]
mod tests {
    // ---- C6: SPSS and MATLAB -------------------------------------------
    // Fixture provenance, stated where the fixtures are used:
    //   analysis.sav      — written by pyreadstat 1.2.7 (wraps ReadStat), and
    //                       read back by the same library to confirm it really
    //                       carries labels, value labels and user-missing.
    //   analysis_v7.mat   — written by scipy.io.savemat(format='5'), which is
    //                       the container `save('-v7')` produces.
    //   analysis_v73.mat  — written by h5py: a GENUINE HDF5 file, not a
    //                       synthesised header, so the detection test cannot
    //                       pass by matching something I made up.

    fn fixture(name: &str) -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures").join(name)
    }

    /// **An SPSS variable is more than a column name**, and every part of it a
    /// reader needs to interpret a value is preserved.
    /// Part C follow-up, item 4: a `.zsav` holds rows far beyond its size on
    /// disk (measured: 40 million rows in 40 KB, a 330 MB peak), and ambers
    /// read every one. Only the first MAX_ROWS + 1 are read now: enough to
    /// keep MAX_ROWS and to know there were more.
    #[test]
    fn a_sav_is_read_no_further_than_one_row_past_the_cap() {
        use arrow::array::Float64Array;
        use arrow::datatypes::{DataType, Field, Schema};
        let rows = 200_000;
        let schema = std::sync::Arc::new(Schema::new(vec![Field::new("x", DataType::Float64, false)]));
        let batch = arrow::record_batch::RecordBatch::try_new(schema, vec![std::sync::Arc::new(Float64Array::from(vec![1.0; rows]))]).unwrap();
        let mut buf = std::io::Cursor::new(Vec::new());
        ambers::write_sav_to_writer(&mut buf, &batch, &ambers::SpssMetadata::default(), ambers::Compression::Zlib, None).unwrap();
        let bytes = buf.into_inner();
        assert!(bytes.len() < 10_000, "{} bytes hold {rows} rows", bytes.len());
        let scanner = ambers::scan_sav_from_reader(std::io::Cursor::new(&bytes[..]), SAV_BATCH_ROWS).unwrap();
        let (read, _) = read_sav_rows(scanner).unwrap();
        assert_eq!(read.num_rows(), MAX_ROWS + 1);
        let ev = parse_supplementary_bytes(&bytes, "sav").unwrap();
        assert!(ev.rows_truncated);
        assert_eq!(ev.tables[0].rows.len(), MAX_ROWS);
    }

    #[test]
    fn an_spss_file_keeps_names_types_labels_value_labels_and_user_missing() {
        let ev = parse_supplementary(&fixture("analysis.sav")).expect("parses");
        assert_eq!(ev.kind, SupplementaryKind::Spss);
        let t = &ev.tables[0];
        assert_eq!(t.headers, vec!["subject", "group", "site", "score"], "variable names");
        assert_eq!(t.rows.len(), 4, "four cases: {:?}", t.rows);

        let by = |n: &str| ev.variables.iter().find(|v| v.name == n).expect(n).clone();

        // NUMERIC vs STRING, from the file's declared format, not guessed.
        assert!(by("score").is_numeric, "{:?}", by("score").declared_type);
        assert!(!by("site").is_numeric, "A5 is a string variable: {:?}", by("site").declared_type);

        // VARIABLE LABELS.
        assert_eq!(by("group").label.as_deref(), Some("Treatment group"));
        assert_eq!(by("score").label.as_deref(), Some("Outcome score"));

        // VALUE LABELS — without these, `99` is just a number.
        let vl = by("group").value_labels;
        assert!(
            vl.iter().any(|(_, l)| l == "control") && vl.iter().any(|(_, l)| l == "not recorded"),
            "value labels lost: {vl:?}"
        );

        // USER-MISSING — the author declared 99 as missing, and a mean computed
        // without knowing that is wrong by construction.
        assert!(!by("group").user_missing.is_empty(), "user-missing lost");
        assert!(by("score").user_missing.is_empty(), "score declares no missing");
    }

    /// A MATLAB v5 file parses into the same record, with its variable names.
    #[test]
    fn a_matlab_v7_file_parses_into_the_same_record_as_csv() {
        let ev = parse_supplementary(&fixture("analysis_v7.mat")).expect("parses");
        assert_eq!(ev.kind, SupplementaryKind::Matlab);
        let t = &ev.tables[0];
        for want in ["reaction_ms", "trials", "accuracy"] {
            assert!(t.headers.iter().any(|h| h == want), "missing {want}: {:?}", t.headers);
        }
        assert!(
            t.rows.iter().flatten().any(|c| c.contains("412.5")),
            "a known value is missing: {:?}",
            t.rows
        );
        // Nothing was skipped, so nothing is claimed to have been.
        assert!(
            !ev.notes.iter().any(|n| n.contains("not read")),
            "a fully-read file must not report skipped variables: {:?}",
            ev.notes
        );
        // A v5 .mat has no label/value-label/missing concept; absence is the
        // FORMAT's and is recorded as empty rather than invented.
        assert!(ev.variables.iter().all(|v| v.label.is_none()));
    }

    /// **A mixed file keeps its numbers and NAMES what it could not read.**
    ///
    /// Measured 22 Sep 2026: `matfile` 0.5.0 returns the numeric variables and
    /// drops char and cell arrays SILENTLY — the file parses, and two of three
    /// variables simply vanish with no signal. Counting the container's own
    /// `miMATRIX` entries is what makes the loss visible.
    #[test]
    fn a_mixed_matlab_file_keeps_numerics_and_reports_what_it_skipped() {
        let ev = parse_supplementary(&fixture("analysis_mixed.mat")).expect("must not reject");
        let t = &ev.tables[0];
        // The numeric variable SURVIVES — the file is not rejected wholesale.
        assert!(t.headers.iter().any(|h| h == "reaction_ms"), "{:?}", t.headers);
        assert!(t.rows.iter().flatten().any(|c| c.contains("412.5")), "{:?}", t.rows);
        // The char and cell variables are absent from the table…
        assert!(!t.headers.iter().any(|h| h == "notes" || h == "condition"), "{:?}", t.headers);
        // …and their absence is REPORTED, with the reason.
        let note = ev
            .notes
            .iter()
            .find(|n| n.contains("not read"))
            .unwrap_or_else(|| panic!("skipped variables were not reported: {:?}", ev.notes));
        assert!(note.contains("char/cell"), "the reason must be named: {note}");
        assert!(note.contains('2'), "both skipped variables must be counted: {note}");
    }

    /// **A file this reader cannot parse at all fails with the MEASURED
    /// reason and a workaround**, not a generic error.
    ///
    /// `matfile` 0.5.0 aborts the whole file on an int32 variable (isolated by
    /// single-type probe: f64/i64/u8/f32/2-D all parse, i32 does not).
    #[test]
    fn an_int32_matlab_file_fails_with_the_measured_reason() {
        let err = parse_supplementary(&fixture("analysis_int32.mat")).expect_err("must fail");
        let msg = err.to_string();
        assert!(msg.contains("int32"), "the known cause must be named: {msg}");
        assert!(msg.contains("int64") || msg.contains("double"), "offer a workaround: {msg}");
    }

    /// **MATLAB 7.3 is refused with an actionable message, never mis-parsed.**
    /// The fixture is a real HDF5 file, so this cannot pass by matching a
    /// header string I wrote myself.
    #[test]
    fn a_matlab_73_file_is_refused_with_an_actionable_message() {
        let err = parse_supplementary(&fixture("analysis_v73.mat")).expect_err("must refuse");
        let msg = err.to_string();
        assert!(msg.contains("MATLAB 7.3"), "{msg}");
        assert!(msg.contains("save('file.mat','-v7')"), "must say what to do instead: {msg}");
    }

    /// **NEGATIVE CONTROL: malformed files fail with a reason, never silently.**
    #[test]
    fn a_malformed_analysis_file_fails_with_a_clear_reason() {
        let dir = std::env::temp_dir();
        for (name, bytes) in [
            ("broken.sav", &b"$FL2 not really an spss file at all"[..]),
            ("broken.mat", &b"MATLAB 5.0 MAT-file but truncated"[..]),
        ] {
            let p = dir.join(format!("gaply_c6_{}_{name}", std::process::id()));
            std::fs::write(&p, bytes).expect("write");
            let err = parse_supplementary(&p).expect_err("must fail");
            let msg = err.to_string();
            assert!(
                msg.contains("malformed or unsupported"),
                "failure must say what went wrong, got: {msg}"
            );
            assert!(!msg.is_empty());
            let _ = std::fs::remove_file(&p);
        }
    }

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
