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

use std::cell::Cell;
use std::io::Read;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

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

/// **The ingest's limits, as a value it is given (Research OS, Phase 8B,
/// decision 243).** The caps above were one shared set, read by Level 1, the
/// reviewer agent's supplementary files, the Stats Verifier page's legacy
/// lane and the methods on the user's data alike, so none could change for
/// one user alone. Each function here and in `stats_l1_ingest` that applies a
/// cap now takes it from the `IngestLimits` it is given, and each refusal
/// carries the limit from that value.
///
/// Three named values exist, one per user, each built from today's
/// constants; no number differs between them in 8B. A production caller
/// passes its own (`tests/ingest_limits_guard.rs`), and nothing outside this
/// impl builds one. [`MAX_CELL_LEN`]'s clamp, `import_guard`'s 100 MiB cap,
/// `stats_l1_ingest::MAX_SHEET_XML_DEPTH` and the dataset's own
/// `MAX_COLUMNS` are not part of it.
///
/// `max_workbook_uncompressed_bytes` is read only by the ingest's byte checks
/// (`stats_l1_ingest`'s `check_bytes`), which Level 1 and the methods go
/// through and [`parse_supplementary_with`] does not: `SUPPLEMENTARY`
/// carries today's number, which no caller of it applies.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IngestLimits {
    /// Data rows; the header is not counted.
    pub max_rows: usize,
    pub max_cols: usize,
    pub max_file_size_bytes: u64,
    /// Cell text in all, UTF-8 bytes after clamping, header included; the
    /// bytes a plain text file keeps.
    pub max_total_chars: usize,
    /// What a zip workbook's parts, a `.zsav`'s blocks or a `.mat`'s
    /// compressed elements may inflate to.
    pub max_workbook_uncompressed_bytes: u64,
    /// Cells: the data rows kept (the header excluded, at most `max_rows`)
    /// times the columns kept (the header's width, at most `max_cols`); for
    /// a `.sav` or `.mat`, the rows kept times the variables kept (Research
    /// OS, Phase 8F-1, decision 282). Checked while parsing, after the row
    /// and the column checks of the same row; a file over it sets
    /// [`SupplementaryEvidence::cells_over`], and the ingest refuses it as
    /// `too_many_cells`. Every named value takes `max_rows × max_cols`, so
    /// their rows and columns bind first and none of them can be over it.
    pub max_cells: usize,
}

impl IngestLimits {
    /// Level 1: `stats_l1_commands`'s open and compute, and
    /// `stats_reproduction`. Unchanged until 8L measures it (decision 242).
    pub const LEVEL_1: IngestLimits = IngestLimits {
        max_rows: MAX_ROWS,
        max_cols: MAX_COLS,
        max_file_size_bytes: MAX_FILE_SIZE_BYTES,
        max_total_chars: MAX_TOTAL_CHARS,
        max_workbook_uncompressed_bytes: crate::stats_l1_ingest::MAX_WORKBOOK_UNCOMPRESSED_BYTES,
        max_cells: MAX_ROWS * MAX_COLS,
    };
    /// The reviewer agent's supplementary files and the Stats Verifier
    /// page's legacy lane (`load_first_table`, under `run_stats_preview` and
    /// `run_stats_verify`).
    pub const SUPPLEMENTARY: IngestLimits = IngestLimits {
        max_rows: MAX_ROWS,
        max_cols: MAX_COLS,
        max_file_size_bytes: MAX_FILE_SIZE_BYTES,
        max_total_chars: MAX_TOTAL_CHARS,
        max_workbook_uncompressed_bytes: crate::stats_l1_ingest::MAX_WORKBOOK_UNCOMPRESSED_BYTES,
        max_cells: MAX_ROWS * MAX_COLS,
    };
    /// The methods' limits before 8F-2, today's constants: kept by
    /// `stats_desc_commands::open` (the tests' wrapper) and by
    /// `load_dataset_from_bytes`. Since 8F-2 the methods' open and rerun
    /// read through [`IngestLimits::this_computer`] (decision 288).
    pub const METHODS: IngestLimits = IngestLimits {
        max_rows: MAX_ROWS,
        max_cols: MAX_COLS,
        max_file_size_bytes: MAX_FILE_SIZE_BYTES,
        max_total_chars: MAX_TOTAL_CHARS,
        max_workbook_uncompressed_bytes: crate::stats_l1_ingest::MAX_WORKBOOK_UNCOMPRESSED_BYTES,
        max_cells: MAX_ROWS * MAX_COLS,
    };

    /// The methods' limits on a computer of `tier`, decision 281's table
    /// (Research OS, Phase 8F-1). Built here because nothing outside this
    /// impl builds an `IngestLimits` (`tests/ingest_limits_guard.rs`).
    /// `memory_tier::MethodLimits` pairs it with the tier's slot budget.
    /// Production reaches it through [`IngestLimits::this_computer`], since
    /// 8F-2 (decision 288).
    ///
    /// No tier goes above 100,000 rows (decision 242.3), and every file cap
    /// stays under decision 107's 100 MiB project cap. On 16 and 32 GB the
    /// cell cap is `100,000 × 128`, so the rows and the columns bind first.
    pub const fn for_tier(tier: crate::memory_tier::MemoryTier) -> IngestLimits {
        use crate::memory_tier::MemoryTier;
        const MIB: u64 = 1024 * 1024;
        match tier {
            MemoryTier::Gb8 => IngestLimits {
                max_rows: 100_000,
                max_cols: MAX_COLS,
                max_file_size_bytes: 64 * MIB,
                max_total_chars: 64 * MIB as usize,
                max_workbook_uncompressed_bytes: 96 * MIB,
                max_cells: 5_000_000,
            },
            MemoryTier::Gb16 | MemoryTier::Gb32 => IngestLimits {
                max_rows: 100_000,
                max_cols: MAX_COLS,
                max_file_size_bytes: 96 * MIB,
                max_total_chars: 96 * MIB as usize,
                max_workbook_uncompressed_bytes: 256 * MIB,
                max_cells: 100_000 * MAX_COLS,
            },
        }
    }

    /// The methods' limits on this computer (Research OS, Phase 8F-2,
    /// decision 288): [`IngestLimits::for_tier`] of
    /// `memory_tier::this_computer()`'s tier, the memory read once per
    /// process. `stats_desc_open` and the rerun type it inline as their
    /// call's last argument (`tests/ingest_limits_guard.rs`); [`open`] keeps
    /// [`IngestLimits::METHODS`].
    ///
    /// [`open`]: crate::stats_desc_commands::open
    pub fn this_computer() -> IngestLimits {
        IngestLimits::for_tier(crate::memory_tier::this_computer().tier)
    }
}

/// How often the open's rows read are stored: every this many data rows,
/// and once more on every exit, success or refusal (Research OS, Phase 8F-1,
/// decision 280).
pub const PROGRESS_EVERY_ROWS: usize = 1_000;

/// One parse's rows read: data rows taken into the table, the header not
/// counted (decision 280). A CSV or TSV counts them as its records are read;
/// a workbook, `.sav` or `.mat`, whose reader delivers its rows at once,
/// counts them as they are converted. A record that is refused is not
/// counted. Stored into the open's counter, when it has one, every
/// [`PROGRESS_EVERY_ROWS`] rows and by [`RowsRead::finish`]. A count only.
struct RowsRead<'a> {
    progress: Option<&'a AtomicU64>,
    rows: Cell<usize>,
}

impl<'a> RowsRead<'a> {
    fn to(progress: Option<&'a AtomicU64>) -> RowsRead<'a> {
        RowsRead {
            progress,
            rows: Cell::new(0),
        }
    }

    /// `rows` data rows are now in the table.
    fn row(&self, rows: usize) {
        self.rows.set(rows);
        if rows % PROGRESS_EVERY_ROWS == 0 {
            self.store();
        }
    }

    /// The last count, on the parse's exit.
    fn finish(&self) {
        self.store();
    }

    fn store(&self) {
        if let Some(p) = self.progress {
            p.store(self.rows.get() as u64, Ordering::Relaxed);
        }
    }
}

/// The rows a table of `columns` kept columns may keep: `max_rows`, or fewer
/// when the cell cap allows fewer (decision 282). Zero columns hold no
/// cells, so only the row limit applies, and such a file is refused for
/// having no columns, as before; never a division by zero.
fn rows_the_cells_allow(columns: usize, limits: IngestLimits) -> usize {
    limits
        .max_cells
        .checked_div(columns)
        .map_or(limits.max_rows, |rows| rows.min(limits.max_rows))
}

/// The evidence of a CSV, TSV or workbook over the cell cap: no cell kept,
/// the flag set (decision 282).
fn over_the_cells(file_name: String, kind: SupplementaryKind) -> SupplementaryEvidence {
    SupplementaryEvidence {
        file_name,
        kind,
        tables: vec![Table {
            headers: Vec::new(),
            rows: Vec::new(),
        }],
        text_summary: String::new(),
        notes: Vec::new(),
        variables: Vec::new(),
        weight_variable: None,
        rows_truncated: false,
        variables_held: None,
        sheet_name: None,
        cells_over: true,
    }
}

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
    /// `true` when the source holds more than the parse's `max_rows` rows and
    /// only the first `max_rows` are in `tables` (`.sav` and `.mat`; CSV and
    /// spreadsheets refuse instead). Level 1 never computes on a truncated
    /// table. Not serialised.
    #[serde(skip)]
    pub rows_truncated: bool,
    /// The number of variables the source holds, when it holds more than the
    /// parse's `max_cols` and only the first `max_cols` are in `tables`
    /// (`.sav` and `.mat`; for a `.mat`, the variables the reader returned).
    /// `None` when every variable was kept. Level 1 and the methods refuse
    /// such a file as too wide (Research OS, Phase 8B-2, decision 246). A
    /// count only. Not serialised, so it never reaches the PublishReady
    /// reviewer's payload.
    #[serde(skip)]
    pub variables_held: Option<usize>,
    /// The sheet a spreadsheet's table was read from. Not serialised.
    #[serde(skip)]
    pub sheet_name: Option<String>,
    /// `true` when the table holds more cells than the parse's `max_cells`
    /// (Research OS, Phase 8F-1, decision 282). The parse stopped where the
    /// cap was crossed and `tables` holds no rows: a CSV or TSV at the first
    /// record over it, a workbook, `.sav` or `.mat` before any cell was
    /// converted. Not a parse error, so no classifier re-reads the file; the
    /// ingest refuses it as `too_many_cells`. No named value's cap can be
    /// reached (its rows and columns bind first), so the reviewer agent's
    /// files never set it. Not serialised, like `rows_truncated`.
    #[serde(skip)]
    pub cells_over: bool,
}

/// Counts only, never the text (part C follow-up, item 7).
impl std::fmt::Debug for SupplementaryEvidence {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SupplementaryEvidence").field("kind", &self.kind).field("tables", &self.tables).field("text_chars", &self.text_summary.chars().count()).field("notes", &self.notes.len()).field("variables", &self.variables.len()).field("weight_variable", &self.weight_variable.is_some()).field("rows_truncated", &self.rows_truncated).field("variables_held", &self.variables_held).field("cells_over", &self.cells_over).finish_non_exhaustive()
    }
}

/// [`parse_supplementary_with`] under [`IngestLimits::SUPPLEMENTARY`], for
/// tests and examples; production code passes its own named limits.
#[doc(hidden)]
pub fn parse_supplementary(path: &Path) -> Result<SupplementaryEvidence, GaplyError> {
    parse_supplementary_with(path, IngestLimits::SUPPLEMENTARY)
}

/// Parse a supplementary file into neutral evidence, memory-capped by
/// `limits`. Never panics: malformed/oversize/unsupported inputs return a
/// clear `GaplyError`.
pub fn parse_supplementary_with(
    path: &Path,
    limits: IngestLimits,
) -> Result<SupplementaryEvidence, GaplyError> {
    // Size cap FIRST — metadata only, no content read. A 500 MB file dies here.
    let size = std::fs::metadata(path)
        .map_err(|e| GaplyError::Validation(format!("cannot stat file: {e}")))?
        .len();
    if size > limits.max_file_size_bytes {
        return Err(GaplyError::Validation(format!(
            "file too large: {size} bytes exceeds the {}-byte cap; \
             not read into memory",
            limits.max_file_size_bytes
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
        "sav" => parse_sav(path, file_name, limits),
        "mat" => parse_mat(path, file_name, limits),
        "xlsx" | "xls" | "xlsb" | "ods" | "xlsm" => parse_spreadsheet(path, file_name, limits),
        "csv" | "tsv" => parse_csv(
            path,
            file_name,
            if ext == "tsv" { b'\t' } else { b',' },
            limits,
        ),
        "txt" | "text" | "md" | "log" => parse_text(path, file_name, limits),
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
#[doc(hidden)]
pub fn parse_supplementary_bytes(bytes: &[u8], ext: &str) -> Result<SupplementaryEvidence, GaplyError> {
    parse_supplementary_bytes_with(bytes, ext, IngestLimits::SUPPLEMENTARY)
}

/// [`parse_supplementary_bytes`] under the `limits` it is given.
pub fn parse_supplementary_bytes_with(
    bytes: &[u8],
    ext: &str,
    limits: IngestLimits,
) -> Result<SupplementaryEvidence, GaplyError> {
    parse_supplementary_bytes_with_progress(bytes, ext, None, limits)
}

/// [`parse_supplementary_bytes_with`], storing its rows read into
/// `progress` as it parses (Research OS, Phase 8F-1, decision 280): every
/// [`PROGRESS_EVERY_ROWS`] data rows, and once on its exit, success or
/// refusal. Only the methods' open passes a counter
/// (`stats_l1_ingest::load_dataset_keeping_bytes_with_progress`).
pub fn parse_supplementary_bytes_with_progress(
    bytes: &[u8],
    ext: &str,
    progress: Option<&AtomicU64>,
    limits: IngestLimits,
) -> Result<SupplementaryEvidence, GaplyError> {
    let rows = RowsRead::to(progress);
    let out = evidence_from_bytes(bytes, ext, &rows, limits);
    rows.finish();
    out
}

/// The parse of [`parse_supplementary_bytes_with_progress`], counting its
/// rows into `rows`.
fn evidence_from_bytes(
    bytes: &[u8],
    ext: &str,
    rows: &RowsRead<'_>,
    limits: IngestLimits,
) -> Result<SupplementaryEvidence, GaplyError> {
    let size = bytes.len() as u64;
    if size > limits.max_file_size_bytes {
        return Err(GaplyError::Validation(format!(
            "file too large: {size} bytes exceeds the {}-byte cap; \
             not read into memory",
            limits.max_file_size_bytes
        )));
    }
    let file_name = String::new();
    match ext {
        "sav" => match sav_header(bytes) {
            SavHeader::Read => sav_evidence(
                ambers::scan_sav_from_reader(std::io::Cursor::new(bytes), SAV_BATCH_ROWS)
                    .and_then(|s| read_sav_rows_with(s, limits)),
                file_name,
                rows,
                limits,
            ),
            SavHeader::NoVariables => sav_evidence(Ok(no_variables()), file_name, rows, limits),
            SavHeader::CaseSizeNotPositive => Err(case_size_not_positive()),
            SavHeader::BigEndian => Err(big_endian()),
        },
        "mat" => mat_evidence(bytes, file_name, rows, limits),
        "xlsx" | "xls" | "xlsb" | "ods" | "xlsm" => {
            spreadsheet_evidence(workbook_from_bytes(bytes, ext), file_name, rows, limits)
        }
        "csv" | "tsv" => csv_evidence(
            bytes,
            file_name,
            if ext == "tsv" { b'\t' } else { b',' },
            rows,
            limits,
        ),
        "txt" | "text" | "md" | "log" => text_evidence(bytes, file_name, limits),
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
/// than `limits.max_rows`?** A new function that [`parse_supplementary`] and its
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
///
/// Under [`IngestLimits::LEVEL_1`], for tests; see [`exceeds_row_cap_with`].
#[doc(hidden)]
pub fn exceeds_row_cap(path: &Path) -> Result<bool, GaplyError> {
    exceeds_row_cap_with(path, IngestLimits::LEVEL_1)
}

/// [`exceeds_row_cap`] under the `limits` it is given.
pub fn exceeds_row_cap_with(path: &Path, limits: IngestLimits) -> Result<bool, GaplyError> {
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
    match ext.as_str() {
        "csv" | "tsv" => {
            let file = std::fs::File::open(path)
                .map_err(|e| GaplyError::Validation(format!("cannot open csv: {e}")))?;
            Ok(csv_exceeds_row_cap(
                std::io::BufReader::new(file),
                &ext,
                limits,
            ))
        }
        "xlsx" | "xls" | "xlsb" | "ods" | "xlsm" => Ok(sheet_exceeds_row_cap(
            calamine::open_workbook_auto(path),
            limits,
        )),
        _ => Ok(false),
    }
}

/// [`exceeds_row_cap`] on bytes already read (part C follow-up, item 3).
pub fn exceeds_row_cap_bytes(bytes: &[u8], ext: &str, limits: IngestLimits) -> bool {
    match ext {
        "csv" | "tsv" => csv_exceeds_row_cap(bytes, ext, limits),
        "xlsx" | "xls" | "xlsb" | "ods" | "xlsm" => {
            sheet_exceeds_row_cap(workbook_from_bytes(bytes, ext), limits)
        }
        _ => false,
    }
}

fn csv_exceeds_row_cap(reader: impl Read, ext: &str, limits: IngestLimits) -> bool {
    let mut rdr = csv::ReaderBuilder::new()
        .has_headers(false)
        .flexible(true)
        .delimiter(if ext == "tsv" { b'\t' } else { b',' })
        .from_reader(reader);
    for (i, rec) in rdr.records().enumerate() {
        if i > limits.max_rows {
            return true;
        }
        match rec {
            Ok(r) if r.len() <= limits.max_cols => {}
            _ => return false,
        }
    }
    false
}

fn sheet_exceeds_row_cap<RS: Read + std::io::Seek>(
    wb: Result<calamine::Sheets<RS>, calamine::Error>,
    limits: IngestLimits,
) -> bool {
    use calamine::Reader;
    let Ok(mut wb) = wb else { return false };
    let Some(sheet) = wb.sheet_names().first().cloned() else { return false };
    let Ok(range) = wb.worksheet_range(&sheet) else { return false };
    range.get_size().0 > limits.max_rows + 1
}

/// **Level 1 only (Phase 2, Step 8b): the column count of a table wider
/// than `limits.max_cols`.** Like [`exceeds_row_cap`], called only after a parse
/// has FAILED, to tell the column-cap refusal apart without reading the
/// message. It mirrors the parsers' order: CSV/TSV records are read with the
/// same settings until the first that is malformed, past the row cap, or
/// over the column cap; a spreadsheet's first sheet is too tall before it is
/// too wide. `None` when the table is not too wide, or the parse failed for
/// another reason first.
///
/// Under [`IngestLimits::LEVEL_1`], for tests; see [`exceeds_col_cap_with`].
#[doc(hidden)]
pub fn exceeds_col_cap(path: &Path) -> Result<Option<usize>, GaplyError> {
    exceeds_col_cap_with(path, IngestLimits::LEVEL_1)
}

/// [`exceeds_col_cap`] under the `limits` it is given.
pub fn exceeds_col_cap_with(path: &Path, limits: IngestLimits) -> Result<Option<usize>, GaplyError> {
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
    match ext.as_str() {
        "csv" | "tsv" => {
            let file = std::fs::File::open(path)
                .map_err(|e| GaplyError::Validation(format!("cannot open csv: {e}")))?;
            Ok(csv_exceeds_col_cap(
                std::io::BufReader::new(file),
                &ext,
                limits,
            ))
        }
        "xlsx" | "xls" | "xlsb" | "ods" | "xlsm" => Ok(sheet_exceeds_col_cap(
            calamine::open_workbook_auto(path),
            limits,
        )),
        _ => Ok(None),
    }
}

/// [`exceeds_col_cap`] on bytes already read (part C follow-up, item 3).
pub fn exceeds_col_cap_bytes(bytes: &[u8], ext: &str, limits: IngestLimits) -> Option<usize> {
    match ext {
        "csv" | "tsv" => csv_exceeds_col_cap(bytes, ext, limits),
        "xlsx" | "xls" | "xlsb" | "ods" | "xlsm" => {
            sheet_exceeds_col_cap(workbook_from_bytes(bytes, ext), limits)
        }
        _ => None,
    }
}

fn csv_exceeds_col_cap(reader: impl Read, ext: &str, limits: IngestLimits) -> Option<usize> {
    let mut rdr = csv::ReaderBuilder::new()
        .has_headers(false)
        .flexible(true)
        .delimiter(if ext == "tsv" { b'\t' } else { b',' })
        .from_reader(reader);
    for (i, rec) in rdr.records().enumerate() {
        if i > limits.max_rows {
            return None;
        }
        match rec {
            Ok(r) if r.len() > limits.max_cols => return Some(r.len()),
            Ok(_) => {}
            Err(_) => return None,
        }
    }
    None
}

fn sheet_exceeds_col_cap<RS: Read + std::io::Seek>(
    wb: Result<calamine::Sheets<RS>, calamine::Error>,
    limits: IngestLimits,
) -> Option<usize> {
    use calamine::Reader;
    let mut wb = wb.ok()?;
    let sheet = wb.sheet_names().first().cloned()?;
    let range = wb.worksheet_range(&sheet).ok()?;
    let (height, width) = range.get_size();
    (height <= limits.max_rows + 1 && width > limits.max_cols).then_some(width)
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
/// 2 and 3, F10): is a CSV or TSV text in an encoding other than UTF-8?** Like
/// [`exceeds_row_cap_bytes`], called only after a parse has FAILED, so a file
/// that opens is never affected. True for a UTF-16 byte-order mark, or for
/// bytes that are not valid UTF-8 and hold no control character other than
/// tab, line feed and carriage return (Latin-1, Windows-1252). Bytes with
/// other control characters are not text in any encoding, and stay malformed.
pub fn not_utf8_text_bytes(bytes: &[u8], ext: &str) -> bool {
    if ext != "csv" && ext != "tsv" {
        return false;
    }
    if bytes.starts_with(&[0xFF, 0xFE]) || bytes.starts_with(&[0xFE, 0xFF]) {
        return true;
    }
    std::str::from_utf8(bytes).is_err()
        && !bytes
            .iter()
            .any(|&b| (b < 0x20 && b != b'\t' && b != b'\n' && b != b'\r') || b == 0x7F)
}

/// **Level 1 and the descriptive summary (evaluation of Research OS Phases
/// 2 and 3, F4): does the table hold more than `limits.max_total_chars`
/// bytes of cell text?** Like [`exceeds_row_cap_bytes`], called only after a parse has
/// FAILED, to tell the content cap apart without reading the message. It
/// mirrors the parsers' order and count: CSV/TSV records are read with the
/// same settings and stop at the first that is past the row cap, malformed
/// or over the column cap (each its own refusal); a spreadsheet's first
/// sheet is checked for its row and column caps first. Each cell is clamped
/// as the parser clamps it and its UTF-8 length added, header included.
pub fn exceeds_content_cap_bytes(bytes: &[u8], ext: &str, limits: IngestLimits) -> bool {
    match ext {
        "csv" | "tsv" => csv_exceeds_content_cap(bytes, ext, limits),
        "xlsx" | "xls" | "xlsb" | "ods" | "xlsm" => {
            sheet_exceeds_content_cap(workbook_from_bytes(bytes, ext), limits)
        }
        _ => false,
    }
}

fn csv_exceeds_content_cap(reader: impl Read, ext: &str, limits: IngestLimits) -> bool {
    let mut rdr = csv::ReaderBuilder::new()
        .has_headers(false)
        .flexible(true)
        .delimiter(if ext == "tsv" { b'\t' } else { b',' })
        .from_reader(reader);
    let mut total = 0usize;
    for (i, rec) in rdr.records().enumerate() {
        if i > limits.max_rows {
            return false;
        }
        let Ok(rec) = rec else { return false };
        if rec.len() > limits.max_cols {
            return false;
        }
        for field in rec.iter() {
            total += clamp_cell(field).0.len();
            if total > limits.max_total_chars {
                return true;
            }
        }
    }
    false
}

fn sheet_exceeds_content_cap<RS: Read + std::io::Seek>(
    wb: Result<calamine::Sheets<RS>, calamine::Error>,
    limits: IngestLimits,
) -> bool {
    use calamine::Reader;
    let Ok(mut wb) = wb else { return false };
    let Some(sheet) = wb.sheet_names().first().cloned() else { return false };
    let Ok(range) = wb.worksheet_range(&sheet) else { return false };
    let (height, width) = range.get_size();
    if height > limits.max_rows + 1 || width > limits.max_cols {
        return false;
    }
    let mut total = 0usize;
    for row in range.rows() {
        for cell in row {
            total += clamp_cell(&cell_to_string(cell)).0.len();
            if total > limits.max_total_chars {
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

fn parse_spreadsheet(
    path: &Path,
    file_name: String,
    limits: IngestLimits,
) -> Result<SupplementaryEvidence, GaplyError> {
    spreadsheet_evidence(
        calamine::open_workbook_auto(path),
        file_name,
        &RowsRead::to(None),
        limits,
    )
}

/// The spreadsheet parser after the workbook is opened, from a path or from
/// bytes.
fn spreadsheet_evidence<RS: Read + std::io::Seek>(
    wb: Result<calamine::Sheets<RS>, calamine::Error>,
    file_name: String,
    counted: &RowsRead<'_>,
    limits: IngestLimits,
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
    if height > limits.max_rows + 1 {
        return Err(GaplyError::Validation(format!(
            "spreadsheet has {height} rows; cap is {}",
            limits.max_rows
        )));
    }
    if width > limits.max_cols {
        return Err(GaplyError::Validation(format!(
            "spreadsheet has {width} columns; cap is {}",
            limits.max_cols
        )));
    }
    // The cells, from the range's size, before any cell is converted
    // (Research OS, Phase 8F-1, decision 282). calamine has built the range
    // by now; the ingest's inflation cap is the guard before it.
    if height.saturating_sub(1).saturating_mul(width) > limits.max_cells {
        return Ok(over_the_cells(file_name, SupplementaryKind::Spreadsheet));
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
            if total > limits.max_total_chars {
                return Err(GaplyError::Validation(format!(
                    "extracted content exceeds the {}-char cap",
                    limits.max_total_chars
                )));
            }
            out_row.push(val);
        }
        if i == 0 {
            headers = out_row;
        } else {
            rows.push(out_row);
            counted.row(rows.len());
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
        variables_held: None,
        sheet_name: Some(sheet),
        cells_over: false,
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
fn parse_mat(
    path: &Path,
    file_name: String,
    limits: IngestLimits,
) -> Result<SupplementaryEvidence, GaplyError> {
    let bytes = std::fs::read(path)
        .map_err(|e| GaplyError::Validation(format!("cannot read .mat: {e}")))?;
    mat_evidence(&bytes, file_name, &RowsRead::to(None), limits)
}

/// The `.mat` parser on the file's bytes, read from a path or already read.
fn mat_evidence(
    bytes: &[u8],
    file_name: String,
    counted: &RowsRead<'_>,
    limits: IngestLimits,
) -> Result<SupplementaryEvidence, GaplyError> {
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
    // The cells: the rows kept (the longest variable's, at most max_rows)
    // times the variables kept, from each array's length before any value
    // is rendered (Research OS, Phase 8F-1, decision 282).
    let kept = mat.arrays().len().min(limits.max_cols);
    let depth = mat
        .arrays()
        .iter()
        .take(kept)
        .map(matlab_len)
        .max()
        .unwrap_or(0);
    let cells_over = depth.min(limits.max_rows).saturating_mul(kept) > limits.max_cells;
    for arr in mat.arrays().iter().take(limits.max_cols) {
        let (declared_type, cells, truncated) = if cells_over {
            (
                matlab_type(arr),
                Vec::new(),
                matlab_len(arr) > limits.max_rows,
            )
        } else {
            matlab_cells(arr, limits)
        };
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
    if mat.arrays().len() > limits.max_cols {
        notes.push(format!(
            "{} of {} variables kept; cap is {}",
            limits.max_cols,
            mat.arrays().len(),
            limits.max_cols
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
    let rows = columns_to_rows(&columns, &mut notes, counted, limits);
    Ok(SupplementaryEvidence {
        file_name,
        kind: SupplementaryKind::Matlab,
        text_summary: format!("MATLAB v5 file with {} variable(s)", headers.len()),
        tables: vec![Table { headers, rows }],
        notes,
        variables,
        weight_variable: None,
        rows_truncated,
        variables_held: (mat.arrays().len() > limits.max_cols).then_some(mat.arrays().len()),
        sheet_name: None,
        cells_over,
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
/// the array held more than `limits.max_rows` values and only the first are
/// kept.
/// Numeric data is rendered with `serde_json`-style shortest round-trip so a
/// value is never re-formatted into something the author did not write
/// (ONTOLOGY §4.20, TEXT).
fn matlab_cells(arr: &matfile::Array, limits: IngestLimits) -> (String, Vec<String>, bool) {
    use matfile::NumericData;
    macro_rules! render {
        ($v:expr) => {
            (
                $v.iter()
                    .take(limits.max_rows)
                    .map(|x| x.to_string())
                    .collect::<Vec<_>>(),
                $v.len() > limits.max_rows,
            )
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
    (matlab_type(arr), cells, truncated)
}

/// A MATLAB array's declared type, `matlab:` and its dimensions.
fn matlab_type(arr: &matfile::Array) -> String {
    let dims = arr
        .size()
        .iter()
        .map(|d| d.to_string())
        .collect::<Vec<_>>()
        .join("x");
    format!("matlab:{dims}")
}

/// The values a MATLAB array holds, counted without rendering one (decision
/// 282).
fn matlab_len(arr: &matfile::Array) -> usize {
    use matfile::NumericData;
    match arr.data() {
        NumericData::Double { real, .. } => real.len(),
        NumericData::Single { real, .. } => real.len(),
        NumericData::Int8 { real, .. } => real.len(),
        NumericData::UInt8 { real, .. } => real.len(),
        NumericData::Int16 { real, .. } => real.len(),
        NumericData::UInt16 { real, .. } => real.len(),
        NumericData::Int32 { real, .. } => real.len(),
        NumericData::UInt32 { real, .. } => real.len(),
        NumericData::Int64 { real, .. } => real.len(),
        NumericData::UInt64 { real, .. } => real.len(),
    }
}

/// SPSS `.sav`, via `ambers` (pure Rust), after [`sav_header`]'s check of
/// the file's first [`SAV_HEADER_READ`] bytes. A file whose head cannot be
/// read goes to ambers, which refuses it as before.
fn parse_sav(
    path: &Path,
    file_name: String,
    limits: IngestLimits,
) -> Result<SupplementaryEvidence, GaplyError> {
    let mut head = Vec::with_capacity(SAV_HEADER_READ);
    let checked = std::fs::File::open(path)
        .and_then(|f| f.take(SAV_HEADER_READ as u64).read_to_end(&mut head))
        .map_or(SavHeader::Read, |_| sav_header(&head));
    let counted = RowsRead::to(None);
    match checked {
        SavHeader::Read => sav_evidence(
            ambers::scan_sav(path).and_then(|s| read_sav_rows_with(s, limits)),
            file_name,
            &counted,
            limits,
        ),
        SavHeader::NoVariables => sav_evidence(Ok(no_variables()), file_name, &counted, limits),
        SavHeader::CaseSizeNotPositive => Err(case_size_not_positive()),
        SavHeader::BigEndian => Err(big_endian()),
    }
}

/// What a `.sav`'s header says before ambers reads it (Research OS, Phase
/// 8F-2, decision 291). ambers 0.4.4 divides by the header's nominal case
/// size × 8 on each of its three read paths (`scanner.rs`), so a file with no
/// variables, or one whose case size is 0 or less, ended its read in a
/// panic. PSPP's "System File Format" chapter says some software writes -1 or
/// 0 there, so a file can have variables and still say so.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SavHeader {
    /// Go to ambers, unchanged: a proper header, or one ambers refuses itself
    /// (too short, not `$FL2` or `$FL3`, a layout code of neither order).
    Read,
    /// The record right after the 176-byte header is the dictionary's end
    /// (type 999): no variable records, which PSPP's chapter puts first.
    NoVariables,
    /// Variables, and a nominal case size of 0 or less.
    CaseSizeNotPositive,
    /// Variables, a positive case size, and the layout code in big-endian
    /// order (Research OS, Phase 8F-3, decision 295). ambers 0.4.4 reads such
    /// a file's header and dictionary in its order but an uncompressed
    /// file's values, a compressed file's raw values (after bytecode 253)
    /// and its value labels as little-endian, so 1.5 reads as 3.13984e-319
    /// with no warning. A compressed file's whole numbers from the bias'
    /// table read rightly, but nothing in the header says which values took
    /// which path, so every big-endian file is refused.
    BigEndian,
}

/// The bytes [`sav_header`] reads: the 176-byte header and the first
/// record's type.
const SAV_HEADER_READ: usize = 180;

/// Reads two numbers from the header (the layout code at 64, the nominal
/// case size at 68) and the first record's type (at 176), never a value,
/// each in the file's own byte order, found from the layout code as ambers'
/// `detect_endianness` finds it: 2 or 3 read little-endian, else 2 or 3 read
/// big-endian. Read in the wrong order a big-endian case size of 128 is
/// negative and a type 2 is 0x02000000, so files that read today would be
/// refused. The dictionary's end is checked first.
fn sav_header(bytes: &[u8]) -> SavHeader {
    if !matches!(bytes.get(..4), Some(b"$FL2") | Some(b"$FL3")) {
        return SavHeader::Read;
    }
    let word = |at: usize| -> Option<[u8; 4]> { bytes.get(at..at + 4)?.try_into().ok() };
    let Some(layout) = word(64) else {
        return SavHeader::Read;
    };
    let little = if matches!(i32::from_le_bytes(layout), 2 | 3) {
        true
    } else if matches!(i32::from_be_bytes(layout), 2 | 3) {
        false
    } else {
        return SavHeader::Read;
    };
    let read = |at: usize| -> Option<i32> {
        word(at).map(|w| {
            if little {
                i32::from_le_bytes(w)
            } else {
                i32::from_be_bytes(w)
            }
        })
    };
    if read(176) == Some(999) {
        return SavHeader::NoVariables;
    }
    match read(68) {
        Some(case_size) if case_size <= 0 => SavHeader::CaseSizeNotPositive,
        _ if !little => SavHeader::BigEndian,
        _ => SavHeader::Read,
    }
}

/// **Level 1 and the methods (Research OS, Phase 8F-3, decision 295): is a
/// `.sav` in big-endian byte order?** Called only after a parse has FAILED,
/// like [`xls_not_compound_document_bytes`], so a file that opens is never
/// affected; it reads the header as [`sav_header`] does, never the error's
/// text. A big-endian file with no variables or a case size of 0 or less
/// keeps 8F-2's outcome (decision 291), so this is false for it.
pub fn sav_big_endian_bytes(bytes: &[u8], ext: &str) -> bool {
    ext == "sav" && sav_header(bytes) == SavHeader::BigEndian
}

/// What ambers would have read from a file with no variables: no columns,
/// no rows, and no metadata, so [`sav_evidence`] builds the empty SPSS table
/// a file with no columns gives.
fn no_variables() -> (arrow::record_batch::RecordBatch, ambers::SpssMetadata) {
    (
        arrow::record_batch::RecordBatch::new_empty(std::sync::Arc::new(
            arrow::datatypes::Schema::empty(),
        )),
        ambers::SpssMetadata::default(),
    )
}

/// The refusal of a `.sav` whose header gives variables and a case size of
/// 0 or less: Gaply cannot read it. In [`sav_evidence`]'s words, so the
/// ingest classifies it as malformed, as any other unreadable `.sav`.
fn case_size_not_positive() -> GaplyError {
    GaplyError::Validation(
        "malformed or unsupported .sav file: its header gives a case size of 0 or less".into(),
    )
}

/// The refusal of a big-endian `.sav` (decision 295), in [`sav_evidence`]'s
/// words, so the reviewer agent's files and the legacy lane give it as
/// malformed and the ingest tells it apart with [`sav_big_endian_bytes`].
fn big_endian() -> GaplyError {
    GaplyError::Validation(
        "malformed or unsupported .sav file: it is in big-endian byte order, which Gaply cannot read yet".into(),
    )
}

/// [`read_sav_rows_with`] under [`IngestLimits::SUPPLEMENTARY`], for the test
/// that reads one row past the cap.
#[cfg(test)]
fn read_sav_rows<R: Read + std::io::Seek>(
    scanner: ambers::Scanner<R>,
) -> Result<(arrow::record_batch::RecordBatch, ambers::SpssMetadata), ambers::error::SpssError> {
    read_sav_rows_with(scanner, IngestLimits::SUPPLEMENTARY)
}

/// The rows of an opened `.sav` and its metadata: at most `limits.max_rows`
/// + 1 rows, or one more than the cell cap allows for the variables kept
/// when that is fewer (decision 282), enough to keep what the limits allow
/// and to know there were more (part C
/// follow-up, item 4). A byte-coded or zlib-compressed (`.zsav`) file holds
/// rows far beyond its size on disk, and `ambers::read_sav` reads every one:
/// measured, 40 million rows in a 40 KB file peaked at 330 MB. The output is
/// unchanged: it was already cut to `max_rows` rows.
fn read_sav_rows_with<R: Read + std::io::Seek>(
    mut scanner: ambers::Scanner<R>,
    limits: IngestLimits,
) -> Result<(arrow::record_batch::RecordBatch, ambers::SpssMetadata), ambers::error::SpssError> {
    let meta = scanner.metadata().clone();
    // Research OS, Phase 8F-1 (decision 282): no further than the rows the
    // cell cap allows for the variables kept, plus one, when that is fewer.
    let kept = meta.variable_names.len().min(limits.max_cols);
    scanner.limit(rows_the_cells_allow(kept, limits) + 1);
    let batch = scanner.collect_single()?;
    Ok((batch, meta))
}

/// The `.sav` parser after `ambers` has read the file, from a path or from
/// bytes.
fn sav_evidence(
    read: Result<(arrow::record_batch::RecordBatch, ambers::SpssMetadata), ambers::error::SpssError>,
    file_name: String,
    counted: &RowsRead<'_>,
    limits: IngestLimits,
) -> Result<SupplementaryEvidence, GaplyError> {
    let (batch, meta) =
        read.map_err(|e| GaplyError::Validation(format!("malformed or unsupported .sav file: {e}")))?;

    let mut notes = Vec::new();
    let mut headers = Vec::new();
    let mut variables = Vec::new();
    for name in meta.variable_names.iter().take(limits.max_cols) {
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
    if meta.variable_names.len() > limits.max_cols {
        notes.push(format!(
            "{} of {} variables kept; cap is {}",
            limits.max_cols,
            meta.variable_names.len(),
            limits.max_cols
        ));
    }
    // The cells: the rows kept times the variables kept, before any row is
    // converted (Research OS, Phase 8F-1, decision 282).
    let cells_over = batch
        .num_rows()
        .min(limits.max_rows)
        .saturating_mul(headers.len())
        > limits.max_cells;
    let columns: Vec<Vec<String>> = if cells_over {
        Vec::new()
    } else {
        (0..headers.len())
            .map(|i| {
                let col = batch.column(i);
                (0..col.len().min(limits.max_rows))
                    .map(|r| arrow_cell(col, r))
                    .collect()
            })
            .collect()
    };
    let rows = columns_to_rows(&columns, &mut notes, counted, limits);
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
        rows_truncated: batch.num_rows() > limits.max_rows,
        variables_held: (meta.variable_names.len() > limits.max_cols)
            .then_some(meta.variable_names.len()),
        sheet_name: None,
        cells_over,
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
fn columns_to_rows(
    columns: &[Vec<String>],
    notes: &mut Vec<String>,
    counted: &RowsRead<'_>,
    limits: IngestLimits,
) -> Vec<Vec<String>> {
    let depth = columns
        .iter()
        .map(|c| c.len())
        .max()
        .unwrap_or(0)
        .min(limits.max_rows);
    if columns.iter().any(|c| c.len() != depth) {
        notes.push("variables have differing lengths; short ones padded".into());
    }
    (0..depth)
        .map(|r| {
            let row = columns
                .iter()
                .map(|c| c.get(r).cloned().unwrap_or_default())
                .collect();
            counted.row(r + 1);
            row
        })
        .collect()
}

fn parse_csv(
    path: &Path,
    file_name: String,
    delim: u8,
    limits: IngestLimits,
) -> Result<SupplementaryEvidence, GaplyError> {
    let file = std::fs::File::open(path)
        .map_err(|e| GaplyError::Validation(format!("cannot open csv: {e}")))?;
    csv_evidence(
        std::io::BufReader::new(file),
        file_name,
        delim,
        &RowsRead::to(None),
        limits,
    )
}

/// The CSV parser on a reader: an opened file, or bytes already read.
fn csv_evidence(
    reader: impl Read,
    file_name: String,
    delim: u8,
    counted: &RowsRead<'_>,
    limits: IngestLimits,
) -> Result<SupplementaryEvidence, GaplyError> {
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
    let mut width = 0usize;

    // Streaming: only ever one record + our capped output is resident.
    for (i, rec) in rdr.records().enumerate() {
        if i > limits.max_rows {
            return Err(GaplyError::Validation(format!(
                "csv has more than {0} rows; cap is {0}",
                limits.max_rows
            )));
        }
        let rec = rec.map_err(|e| GaplyError::Validation(format!("malformed csv at row {i}: {e}")))?;
        if rec.len() > limits.max_cols {
            return Err(GaplyError::Validation(format!(
                "csv row {i} has {} columns; cap is {}",
                rec.len(),
                limits.max_cols
            )));
        }
        // The cells: `i` data rows of the header's width, checked after this
        // record's rows and columns and before its text (Research OS, Phase
        // 8F-1, decision 282). The first record over the cap stops the read.
        if i == 0 {
            width = rec.len();
        } else if i.saturating_mul(width) > limits.max_cells {
            return Ok(over_the_cells(file_name, SupplementaryKind::Csv));
        }
        let mut out_row = Vec::with_capacity(rec.len());
        for field in rec.iter() {
            let (val, was_clamped) = clamp_cell(field);
            if was_clamped {
                clamped += 1;
            }
            total += val.len();
            if total > limits.max_total_chars {
                return Err(GaplyError::Validation(format!(
                    "extracted content exceeds the {}-char cap",
                    limits.max_total_chars
                )));
            }
            out_row.push(val);
        }
        if i == 0 {
            headers = out_row;
        } else {
            rows.push(out_row);
            counted.row(rows.len());
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
        variables_held: None,
        sheet_name: None,
        cells_over: false,
    })
}

fn parse_text(
    path: &Path,
    file_name: String,
    limits: IngestLimits,
) -> Result<SupplementaryEvidence, GaplyError> {
    let file = std::fs::File::open(path)
        .map_err(|e| GaplyError::Validation(format!("cannot open text file: {e}")))?;
    text_evidence(std::io::BufReader::new(file), file_name, limits)
}

/// The text parser on a reader: an opened file, or bytes already read.
fn text_evidence(
    reader: impl Read,
    file_name: String,
    limits: IngestLimits,
) -> Result<SupplementaryEvidence, GaplyError> {
    // Read at most max_total_chars + 1 bytes — bounded regardless of file size.
    let max = limits.max_total_chars;
    let mut buf = Vec::new();
    reader
        .take(max as u64 + 1)
        .read_to_end(&mut buf)
        .map_err(|e| GaplyError::Validation(format!("cannot read text: {e}")))?;

    let mut notes = Vec::new();
    let truncated = buf.len() > max;
    buf.truncate(max);
    // Lossy decode so a non-UTF-8 byte can't error; still bounded.
    let mut text = String::from_utf8_lossy(&buf).into_owned();
    if truncated {
        text.truncate(max);
        notes.push(format!("text truncated to {max} chars"));
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
        variables_held: None,
        sheet_name: None,
        cells_over: false,
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

    /// Research OS, Phase 8F-1 (decision 282): a `.sav` is read no further
    /// than the rows its cell cap allows, plus one, when that is fewer than
    /// its row limit allows: max_cells divided by the variables kept. Three
    /// variables and a cap of 12 cells allow 4 rows, so 5 are read, of
    /// 200,000; and none is converted.
    /// Decision 282: zero variables hold zero cells, so only the row limit
    /// applies, through `checked_div`, never a division by zero.
    #[test]
    fn zero_columns_allow_the_row_limit_and_never_divide_by_zero() {
        let limits = IngestLimits {
            max_rows: 50,
            max_cells: 12,
            ..IngestLimits::METHODS
        };
        assert_eq!(rows_the_cells_allow(0, limits), 50);
        assert_eq!(rows_the_cells_allow(3, limits), 4);
        assert_eq!(rows_the_cells_allow(5, limits), 2);
        assert_eq!(rows_the_cells_allow(13, limits), 0);
        assert_eq!(rows_the_cells_allow(1, limits), 12);
        assert_eq!(
            rows_the_cells_allow(
                1,
                IngestLimits {
                    max_cells: 1_000,
                    ..limits
                }
            ),
            50,
            "the rows bind first"
        );
    }

    #[test]
    fn a_sav_is_read_no_further_than_one_row_past_what_its_cells_allow() {
        use arrow::array::Float64Array;
        use arrow::datatypes::{DataType, Field, Schema};
        let rows = 200_000;
        let fields: Vec<Field> = ["x", "y", "z"]
            .iter()
            .map(|n| Field::new(*n, DataType::Float64, false))
            .collect();
        let columns: Vec<std::sync::Arc<dyn arrow::array::Array>> = (0..3)
            .map(|_| std::sync::Arc::new(Float64Array::from(vec![1.0; rows])) as _)
            .collect();
        let batch = arrow::record_batch::RecordBatch::try_new(
            std::sync::Arc::new(Schema::new(fields)),
            columns,
        )
        .unwrap();
        let mut buf = std::io::Cursor::new(Vec::new());
        ambers::write_sav_to_writer(&mut buf, &batch, &ambers::SpssMetadata::default(), ambers::Compression::Zlib, None).unwrap();
        let bytes = buf.into_inner();
        let limits = IngestLimits { max_cells: 12, ..IngestLimits::METHODS };
        let scanner = ambers::scan_sav_from_reader(std::io::Cursor::new(&bytes[..]), SAV_BATCH_ROWS).unwrap();
        let (read, _) = read_sav_rows_with(scanner, limits).unwrap();
        assert_eq!(read.num_rows(), 5, "the 4 rows 12 cells allow, plus one");
        let ev = parse_supplementary_bytes_with(&bytes, "sav", limits).unwrap();
        assert!(ev.cells_over && !ev.rows_truncated);
        assert!(ev.tables[0].rows.is_empty(), "no row converted");
        // Its row limit still reads one past itself when the cells allow more.
        let scanner =
            ambers::scan_sav_from_reader(std::io::Cursor::new(&bytes[..]), SAV_BATCH_ROWS).unwrap();
        let (read, _) = read_sav_rows_with(scanner, IngestLimits::METHODS).unwrap();
        assert_eq!(read.num_rows(), MAX_ROWS + 1);
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
