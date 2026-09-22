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
#[derive(Debug, Clone, Serialize)]
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
    /// Per-variable metadata, when the format carries any. Empty for CSV and
    /// plain text, which have none to lose.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub variables: Vec<VariableMeta>,
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
        variables: Vec::new(),
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
    if is_matlab_73(&bytes) {
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
    let mat = matfile::MatFile::parse(std::io::Cursor::new(&bytes)).map_err(|e| {
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
    for arr in mat.arrays().iter().take(MAX_COLS) {
        let (declared_type, cells) = matlab_cells(arr);
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
    let declared = count_mat_variables(&bytes);
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

/// One MATLAB array as `(declared_type, cells)`. Numeric data is rendered with
/// `serde_json`-style shortest round-trip so a value is never re-formatted into
/// something the author did not write (ONTOLOGY §4.20, TEXT).
fn matlab_cells(arr: &matfile::Array) -> (String, Vec<String>) {
    use matfile::NumericData;
    let dims = arr
        .size()
        .iter()
        .map(|d| d.to_string())
        .collect::<Vec<_>>()
        .join("x");
    macro_rules! render {
        ($v:expr) => {
            $v.iter().take(MAX_ROWS).map(|x| x.to_string()).collect::<Vec<_>>()
        };
    }
    let cells = match arr.data() {
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
    (format!("matlab:{dims}"), cells)
}

/// SPSS `.sav`, via `ambers` (pure Rust).
fn parse_sav(path: &Path, file_name: String) -> Result<SupplementaryEvidence, GaplyError> {
    let (batch, meta) = ambers::read_sav(path)
        .map_err(|e| GaplyError::Validation(format!("malformed or unsupported .sav file: {e}")))?;

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
    })
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
        variables: Vec::new(),
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
        variables: Vec::new(),
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
