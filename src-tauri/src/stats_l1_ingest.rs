//! **The Level 1 data path, WITHOUT Tauri: a file on disk to a typed
//! [`Dataset`] with a fingerprint, through the existing ingest and nothing
//! else.**
//!
//! Same seam as Level 0's `run_stats_consistency_measured` (in
//! `stats_consistency.rs`): a plain function a command (later) and a test
//! (now) both call, so what is measured is the path the command will take.
//! That module's view guard scans every source file for the Level 0 view's
//! module path, so this file names the module by its file name instead.
//!
//! In order:
//! 0. The path must be a REGULAR file, judged on the opened handle
//!    ([`import_guard::open_regular_file`]; evaluation fixes, part C, M-10):
//!    a FIFO, a device or a socket is refused at once, before a byte is read.
//! 1. `import_guard`'s byte ceiling, from metadata alone. **Not its
//!    `preflight`**: that parses the file as a manuscript, and a data file is
//!    not one — measured, `preflight` refuses every `.csv` with "unsupported
//!    manuscript format" (`import_guards_preflight_refuses_a_csv_so_only_its_byte_rule_is_applied`
//!    pins it). So the byte rule is applied through [`import_guard::assess`]
//!    and the document parser is never reached. This is the conservative
//!    reading of "reading goes through `import_guard`", recorded in the
//!    Step 2 report.
//! 2. The ingest's own size and type limits, then
//!    [`supplementary::parse_supplementary`], the one parser.
//! 3. The file is read ONCE, through the opened handle with a bound, and
//!    those bytes are what is hashed and what is parsed
//!    ([`supplementary::parse_supplementary_bytes`]): the path is never
//!    opened again, so the file cannot change between the check and the
//!    parse (part C follow-up, item 3; until then the parser reopened the
//!    path and a second read compared the bytes). A zip-based workbook whose
//!    central directory declares more than [`MAX_WORKBOOK_UNCOMPRESSED_BYTES`]
//!    is refused here, before any part is inflated (part C, M-9): a 1.5 MB
//!    file that inflated to 1.5 GB peaked at 1.4 GB in memory, and an
//!    allocation failure aborts the process. Then every part is inflated
//!    into a counter, and a part that inflates past its declared size, or
//!    parts past the limit, are refused the same way before parsing
//!    ([`import_guard::count_zip_inflation`], part C follow-up, item 2).
//! 4. [`build_dataset`] in `gaply_core`.
//!
//! A file over the ingest row cap is [`L1IngestRefusal::FileTooLarge`]: CSV
//! and spreadsheets refuse at the parser, and `.sav`/`.mat`, which the parser
//! truncates, are refused here on `rows_truncated`. Level 1 never computes on
//! a truncated table, and the cap is not raised (decisions.md).
//!
//! A CSV or spreadsheet with more than 2 MiB of cell text
//! (`supplementary::MAX_TOTAL_CHARS`, UTF-8 bytes after each cell is clamped)
//! is [`L1IngestRefusal::ContentTooLarge`], told apart by a typed check like
//! the row and column caps (evaluation of Research OS Phases 2 and 3, F4).
//!
//! Every refusal is a typed value with no path, no file name and no message
//! text from the parser: the only things in it are a byte count or a row cap.
//!
//! Since Step 3, [`compute_from_path`] is the computation's seam: it reads the
//! file again through [`load_dataset`] for every computation, so the bytes
//! computed on are the bytes fingerprinted at that moment.

use std::path::Path;

use gaply_core::extract::docparse::DocumentShape;
use std::io::Read;

use gaply_core::import_guard::{self, EstimateBasis, NotOpened, ZipInflation};
use gaply_core::stats_l1::dataset::{
    build_dataset, DataFingerprint, Dataset, DatasetRefusal, RawTable, RawVariable, SourceMetadata, TableRef,
};

use gaply_core::stats_l1::claim::ClaimId;
use gaply_core::stats_l1::compute::{self, L1Computation, L1Refusal};
use gaply_core::stats_l1::run_state::L1RunState;
use gaply_core::stats_l1::token::ConfirmationToken;

use crate::supplementary::{self, SupplementaryKind, MAX_FILE_SIZE_BYTES, MAX_ROWS, PARSER_VERSION};

/// Defined in `gaply_core` since Phase 2, Step 4, so a Level 1 verdict can
/// carry it as its "data unreadable" reason; re-exported here unchanged.
pub use gaply_core::stats_l1::dataset::L1IngestRefusal;

/// Extensions the ingest turns into a table. Text formats parse but hold no
/// table and are refused before being read.
/// Public so `tests/capability_registry.rs` can pin the registry's table
/// formats to it (Research OS, Phase 4A).
pub const TABLE_EXTENSIONS: &[&str] = &["sav", "mat", "xlsx", "xls", "xlsb", "ods", "xlsm", "csv", "tsv"];

/// The workbook formats that are zip archives (`.xls` is a compound
/// document, not a zip).
const ZIP_EXTENSIONS: &[&str] = &["xlsx", "xlsm", "xlsb", "ods"];

/// The most uncompressed bytes a workbook's parts may declare, in all
/// (evaluation fixes, part C, M-9). Measured 17 Oct 2026 on every real
/// workbook in `~/gaply-corpus/l1/papers/` and `~/gaply-corpus/eval2/fresh/`
/// (27 files, `partc/xlsx/declared.py`, central directories only): the
/// largest declares 13,248,850 bytes (a 2.26 MB file). 64 MiB is about five
/// times that. The deep evaluation's bomb declares 1,572,865,485.
///
/// An archive that declares small sizes and inflates to more is caught by
/// counting what each part actually inflates to before parsing (part C
/// follow-up, item 2): the reader (zip 2.4.2, through calamine) bounds the
/// compressed bytes it reads, not the inflated output.
pub const MAX_WORKBOOK_UNCOMPRESSED_BYTES: u64 = 64 * 1024 * 1024;

/// The deepest element nesting a worksheet may have (F14, decision 52). A
/// rich-text cell goes eight deep; this leaves room for any extension.
pub const MAX_SHEET_XML_DEPTH: usize = 64;

/// The file at `path` as a Level 1 dataset.
#[doc(hidden)]
#[tracing::instrument(skip(path))]
pub fn load_dataset(path: &Path) -> Result<Dataset, L1IngestRefusal> {
    dataset_from_checked(check_file(path)?)
}

/// A data file opened, checked and read once: its bytes and its extension.
/// It holds no path, so nothing after the check can open the file again (part
/// C follow-up, item 3). Never `Debug`: the bytes are the user's data.
#[doc(hidden)]
pub struct CheckedFile {
    bytes: Vec<u8>,
    ext: String,
}

/// Steps 0 to 3's checks, and the bytes read through the opened handle.
#[doc(hidden)]
pub fn check_file(path: &Path) -> Result<CheckedFile, L1IngestRefusal> {
    // 0. A regular file, judged on the opened handle; read through it.
    let (file, bytes_len) = open_regular(path)?;

    // 1. import_guard's byte rule, from metadata, through the pure decision.
    let shape = DocumentShape { pages: None, page_equivalents: 0, bytes: bytes_len, text_chars: 0 };
    let pre = import_guard::assess(&shape, import_guard::SEED_SECONDS_PER_PAGE, EstimateBasis::Seeded);
    if pre.is_refused() {
        return Err(L1IngestRefusal::TooLarge { bytes: bytes_len, limit: import_guard::MAX_BYTES });
    }
    // 2. The ingest's own caps, before any content is read.
    if bytes_len > MAX_FILE_SIZE_BYTES {
        return Err(L1IngestRefusal::TooLarge { bytes: bytes_len, limit: MAX_FILE_SIZE_BYTES });
    }
    let ext = path.extension().and_then(|e| e.to_str()).map(|e| e.to_ascii_lowercase()).unwrap_or_default();
    if !TABLE_EXTENSIONS.contains(&ext.as_str()) {
        return Err(L1IngestRefusal::UnsupportedFormat);
    }

    // 3. Hash the exact bytes, parse, hash again.
    let before = read_bounded(file)?;
    if ZIP_EXTENSIONS.contains(&ext.as_str()) {
        if let Some(declared) = declared_uncompressed_total(&before) {
            if declared > MAX_WORKBOOK_UNCOMPRESSED_BYTES {
                return Err(L1IngestRefusal::ArchiveTooLarge { declared, limit: MAX_WORKBOOK_UNCOMPRESSED_BYTES });
            }
        }
        // What the parts DECLARE is the archive's own claim. Count what they
        // actually inflate to, into a sink, before any parser sees them (part
        // C follow-up, item 2).
        if let ZipInflation::Over { counted } = import_guard::count_zip_inflation(&before, MAX_WORKBOOK_UNCOMPRESSED_BYTES) {
            return Err(L1IngestRefusal::ArchiveTooLarge { declared: counted, limit: MAX_WORKBOOK_UNCOMPRESSED_BYTES });
        }
        // F14 (decision 52): a sheet that nests far past any SpreadsheetML
        // is not one; the parser read it as an empty table.
        if import_guard::worksheet_nests_deeper_than(&before, MAX_SHEET_XML_DEPTH) {
            return Err(L1IngestRefusal::Malformed);
        }
    }
    // A .zsav's trailer, read before ambers allocates from it (part C
    // follow-up 2, item 3).
    if ext == "sav" {
        match supplementary::zsav_trailer(&before) {
            supplementary::ZsavTrailer::Malformed => return Err(L1IngestRefusal::Malformed),
            supplementary::ZsavTrailer::Declared { inflated } if inflated > MAX_WORKBOOK_UNCOMPRESSED_BYTES => {
                return Err(L1IngestRefusal::ArchiveTooLarge { declared: inflated, limit: MAX_WORKBOOK_UNCOMPRESSED_BYTES });
            }
            _ => {}
        }
    }
    // A .mat file's compressed elements, counted the same way before matfile
    // inflates them (part C follow-up 2).
    if ext == "mat" {
        if let Err(counted) = supplementary::count_mat_inflation(&before, MAX_WORKBOOK_UNCOMPRESSED_BYTES) {
            return Err(L1IngestRefusal::ArchiveTooLarge { declared: counted, limit: MAX_WORKBOOK_UNCOMPRESSED_BYTES });
        }
    }
    Ok(CheckedFile { bytes: before, ext })
}

/// Steps 3 and 4 on a checked file: the parse reads the checked bytes, never
/// the path, so they are the bytes fingerprinted.
#[doc(hidden)]
pub fn dataset_from_checked(checked: CheckedFile) -> Result<Dataset, L1IngestRefusal> {
    let CheckedFile { bytes: before, ext } = checked;
    // A failed parse is classified by a typed check of its own
    // (`exceeds_row_cap`), never by reading the parser's message, which the
    // existing Stats Verifier shows unchanged.
    let evidence = supplementary::parse_supplementary_bytes(&before, &ext).map_err(|_| {
        if supplementary::exceeds_row_cap_bytes(&before, &ext) {
            L1IngestRefusal::FileTooLarge { limit_rows: MAX_ROWS }
        } else if let Some(columns) = supplementary::exceeds_col_cap_bytes(&before, &ext) {
            // Step 8b, section 4's finding 1: too wide, not malformed.
            L1IngestRefusal::Dataset(DatasetRefusal::TooWide { columns, limit: supplementary::MAX_COLS })
        } else if supplementary::exceeds_content_cap_bytes(&before, &ext) {
            // Research OS evaluation, F4: more cell text than the ingest
            // keeps, not malformed.
            L1IngestRefusal::ContentTooLarge { limit_bytes: supplementary::MAX_TOTAL_CHARS }
        } else if supplementary::xls_not_compound_document_bytes(&before, &ext) {
            L1IngestRefusal::XlsNotReadable
        } else if supplementary::not_utf8_text_bytes(&before, &ext) {
            // F10 (decision 52): text in another encoding, not malformed.
            L1IngestRefusal::EncodingNotUtf8
        } else {
            L1IngestRefusal::Malformed
        }
    })?;
    if evidence.rows_truncated {
        return Err(L1IngestRefusal::FileTooLarge { limit_rows: MAX_ROWS });
    }
    let table_ref = match evidence.kind {
        SupplementaryKind::Csv => TableRef::FirstTable,
        SupplementaryKind::Spreadsheet => TableRef::Sheet { index: 0, name: evidence.sheet_name.clone().unwrap_or_default() },
        SupplementaryKind::Spss => TableRef::SpssCases,
        SupplementaryKind::Matlab => TableRef::MatlabVariables,
        SupplementaryKind::Text => return Err(L1IngestRefusal::NoTable),
    };
    let fingerprint = DataFingerprint::of_bytes(&before, PARSER_VERSION, table_ref);

    // 4. The table, with whatever metadata the format carried.
    let table = evidence.tables.into_iter().next().ok_or(L1IngestRefusal::NoTable)?;
    let metadata = match evidence.kind {
        SupplementaryKind::Spss => SourceMetadata::Read,
        _ => SourceMetadata::NotCarried,
    };
    let variables: Vec<RawVariable> = table
        .headers
        .iter()
        .map(|h| {
            evidence
                .variables
                .iter()
                .find(|v| v.name == *h)
                .map(|v| RawVariable {
                    user_missing: v.user_missing_rules.clone(),
                    value_labels: v.value_label_codes.clone(),
                    declared_text: Some(!v.is_numeric),
                })
                .unwrap_or_default()
        })
        .collect();
    let raw = RawTable {
        headers: table.headers,
        rows: table.rows,
        variables,
        weight_variable: evidence.weight_variable,
        metadata,
    };
    let dataset = build_dataset(raw, fingerprint).map_err(L1IngestRefusal::Dataset)?;
    tracing::debug!(rows = dataset.rows(), columns = dataset.schema.columns.len(), "stats_l1: dataset loaded");
    Ok(dataset)
}

/// The path opened as a regular file, and its length, as a refusal code.
fn open_regular(path: &Path) -> Result<(std::fs::File, u64), L1IngestRefusal> {
    import_guard::open_regular_file(path).map_err(|e| match e {
        NotOpened::NotRegular => L1IngestRefusal::NotARegularFile,
        NotOpened::Unreadable => L1IngestRefusal::NotReadable,
    })
}

/// Every byte of an opened file, but never more than the ingest's size cap:
/// a file that grew past it since its `stat` is refused as too large.
fn read_bounded(file: std::fs::File) -> Result<Vec<u8>, L1IngestRefusal> {
    let mut bytes = Vec::new();
    file.take(MAX_FILE_SIZE_BYTES + 1).read_to_end(&mut bytes).map_err(|_| L1IngestRefusal::NotReadable)?;
    if bytes.len() as u64 > MAX_FILE_SIZE_BYTES {
        return Err(L1IngestRefusal::TooLarge { bytes: bytes.len() as u64, limit: MAX_FILE_SIZE_BYTES });
    }
    Ok(bytes)
}

/// The sum of the uncompressed sizes a zip's central directory declares,
/// ZIP64 included; `None` when the bytes are not a zip this can read, which
/// is left to the parser to refuse. Reads the directory only: nothing is
/// inflated.
pub fn declared_uncompressed_total(bytes: &[u8]) -> Option<u64> {
    let u16_at = |i: usize| -> Option<u64> { Some(u64::from(u16::from_le_bytes(bytes.get(i..i + 2)?.try_into().ok()?))) };
    let u32_at = |i: usize| -> Option<u64> { Some(u64::from(u32::from_le_bytes(bytes.get(i..i + 4)?.try_into().ok()?))) };
    let u64_at = |i: usize| -> Option<u64> { Some(u64::from_le_bytes(bytes.get(i..i + 8)?.try_into().ok()?)) };
    let sig = |i: usize, s: [u8; 4]| bytes.get(i..i + 4) == Some(&s[..]);
    // The end-of-central-directory record: 22 bytes and a comment of up to
    // 65,535, so it starts in the last 65,557 bytes.
    let last = bytes.len().checked_sub(22)?;
    let eocd = (last.saturating_sub(65_535)..=last).rev().find(|&i| sig(i, [0x50, 0x4b, 0x05, 0x06]))?;
    let (mut entries, mut directory) = (u16_at(eocd + 10)?, u32_at(eocd + 16)?);
    if entries == 0xFFFF || directory == 0xFFFF_FFFF {
        // ZIP64: the locator sits just before the record and points at the
        // ZIP64 end-of-central-directory record.
        let locator = eocd.checked_sub(20)?;
        if !sig(locator, [0x50, 0x4b, 0x06, 0x07]) {
            return None;
        }
        let z = usize::try_from(u64_at(locator + 8)?).ok()?;
        if !sig(z, [0x50, 0x4b, 0x06, 0x06]) {
            return None;
        }
        entries = u64_at(z + 32)?;
        directory = u64_at(z + 48)?;
    }
    let mut at = usize::try_from(directory).ok()?;
    let mut total: u64 = 0;
    for _ in 0..entries {
        if !sig(at, [0x50, 0x4b, 0x01, 0x02]) {
            return None;
        }
        let (name, extra, comment) = (u16_at(at + 28)? as usize, u16_at(at + 30)? as usize, u16_at(at + 32)? as usize);
        let mut size = u32_at(at + 24)?;
        if size == 0xFFFF_FFFF {
            // The ZIP64 extra field (id 1) holds the real size first.
            let field = bytes.get(at + 46 + name..at + 46 + name + extra)?;
            let mut i = 0;
            let mut found = None;
            while i + 4 <= field.len() {
                let (id, len) = (u16::from_le_bytes([field[i], field[i + 1]]), usize::from(u16::from_le_bytes([field[i + 2], field[i + 3]])));
                if id == 1 {
                    found = Some(u64::from_le_bytes(field.get(i + 4..i + 12)?.try_into().ok()?));
                }
                i += 4 + len;
            }
            size = found?;
        }
        total = total.saturating_add(size);
        at = at.checked_add(46 + name + extra + comment)?;
    }
    Some(total)
}

/// Why a Level 1 computation from a file produced nothing: the file could
/// not be read again as a dataset, or the computation refused. Typed, with no
/// path, name or parser text, like both halves.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum L1ComputeError {
    Ingest(L1IngestRefusal),
    Refused(L1Refusal),
}

/// Level 1's computation for one claim, with the data READ AGAIN from the
/// file's bytes for this call (Phase 2, Step 3). The token is checked before
/// the file is opened, so nothing is read without a live confirmation; the
/// fresh dataset's fingerprint is then checked against the confirmed one by
/// `compute`, so an edited file is refused rather than computed on.
#[doc(hidden)]
#[tracing::instrument(skip(state, token, path))]
pub fn compute_from_path(state: &L1RunState, token: &ConfirmationToken, claim: ClaimId, path: &Path) -> Result<L1Computation, L1ComputeError> {
    state.gate(token, claim).map_err(|g| L1ComputeError::Refused(L1Refusal::Gate(g)))?;
    let reread = load_dataset(path).map_err(L1ComputeError::Ingest)?;
    compute::compute(state, token, claim, &reread).map_err(L1ComputeError::Refused)
}

#[cfg(test)]
mod tests {
    use super::*;
    use gaply_core::stats_l1::dataset::{Cell, ColumnType, DatasetIssue, DatasetRefusal};
    use std::io::Write;

    /// A directory whose name carries a sentinel, so a path leaking into a
    /// refusal is visible.
    fn dir() -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("gaply_l1_PATH_SECRET_c4f1_{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn write(name: &str, bytes: &[u8]) -> std::path::PathBuf {
        let p = dir().join(name);
        std::fs::File::create(&p).unwrap().write_all(bytes).unwrap();
        p
    }

    fn fixture(name: &str) -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures").join(name)
    }

    const CSV: &[u8] = b"score,group,note\n1.5,1,CELL_SECRET_7d9\n2.5,2,b\n3.5,1,LEVEL_SECRET_b18\n,2,b\n";

    /// The premise "reading goes through `import_guard`" measured against the
    /// code: `preflight` parses the file as a manuscript and refuses a CSV
    /// outright. So only its byte rule is applied here, through `assess`.
    #[test]
    fn import_guards_preflight_refuses_a_csv_so_only_its_byte_rule_is_applied() {
        let p = write("PATH_SECRET_c4f1.csv", CSV);
        let db = gaply_core::Database::in_memory().unwrap();
        let pre = import_guard::preflight(&db, &p, 0).unwrap();
        assert!(pre.is_refused(), "{:?}", pre.verdict);
        assert!(pre.summary.contains("unsupported manuscript format"), "{}", pre.summary);
        // And the same file loads here.
        assert!(load_dataset(&p).is_ok());
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn a_csv_loads_with_a_fingerprint_over_its_exact_bytes_and_one_byte_changes_it() {
        let p = write("a.csv", CSV);
        let d = load_dataset(&p).unwrap();
        // The same hash the manuscript hasher computes over the same file.
        assert_eq!(d.schema.fingerprint.sha256_hex(), crate::harness_log::manuscript_sha256(&p));
        assert_eq!(d.schema.fingerprint.parser_version(), PARSER_VERSION);
        assert_eq!(*d.schema.fingerprint.table(), TableRef::FirstTable);
        assert_eq!(d.rows(), 4);
        assert_eq!(d.schema.columns[0].column_type, ColumnType::Numeric);
        assert_eq!(d.schema.columns[1].column_type, ColumnType::Integer);
        assert!(d.schema.columns[0].has_missing);
        assert_eq!(d.schema.source_metadata, SourceMetadata::NotCarried);
        // The same bytes give the same schema.
        let again = load_dataset(&p).unwrap();
        assert_eq!(d, again);
        // One changed byte: another fingerprint.
        let mut changed = CSV.to_vec();
        changed[0] = b'S';
        let q = write("b.csv", &changed);
        let e = load_dataset(&q).unwrap();
        assert_ne!(e.schema.fingerprint.sha256_hex(), d.schema.fingerprint.sha256_hex());
        assert_ne!(e.schema.fingerprint.digest(), d.schema.fingerprint.digest());
        let _ = std::fs::remove_file(&p);
        let _ = std::fs::remove_file(&q);
    }

    #[test]
    fn every_refusal_is_typed_and_names_no_path_cell_or_level() {
        let cases: Vec<(std::path::PathBuf, L1IngestRefusal)> = vec![
            (write("x.parquet", b"PAR1"), L1IngestRefusal::UnsupportedFormat),
            (write("notes.txt", b"n = 48"), L1IngestRefusal::UnsupportedFormat),
            (write("fake.xlsx", b"this is not a zip"), L1IngestRefusal::Malformed),
            (write("broken.sav", b"$FL2 not really an spss file at all"), L1IngestRefusal::Malformed),
            (dir().join("missing.csv"), L1IngestRefusal::NotReadable),
            (write("empty.csv", b""), L1IngestRefusal::Dataset(DatasetRefusal::NoColumns)),
        ];
        for (p, want) in &cases {
            let got = load_dataset(p).unwrap_err();
            assert_eq!(got, *want, "{}", p.display());
            let text = format!("{got:?}");
            assert!(!text.contains("PATH_SECRET"), "{text}");
            assert!(!text.contains("CELL_SECRET") && !text.contains("LEVEL_SECRET"), "{text}");
            let _ = std::fs::remove_file(p);
        }
        // Over the ingest size cap, before any content is read.
        let big = vec![b'0'; (MAX_FILE_SIZE_BYTES + 1024) as usize];
        let p = write("huge.csv", &big);
        assert_eq!(load_dataset(&p).unwrap_err(), L1IngestRefusal::TooLarge { bytes: MAX_FILE_SIZE_BYTES + 1024, limit: MAX_FILE_SIZE_BYTES });
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn a_csv_over_the_row_cap_is_file_too_large_not_a_truncated_table() {
        let mut data = String::from("id\n");
        for i in 0..(MAX_ROWS + 50) {
            data.push_str(&format!("{i}\n"));
        }
        let p = write("manyrows.csv", data.as_bytes());
        assert_eq!(load_dataset(&p).unwrap_err(), L1IngestRefusal::FileTooLarge { limit_rows: MAX_ROWS });
        // The existing parser's refusal is exactly what it was before Phase 2:
        // same variant, same text, no prefix.
        match supplementary::parse_supplementary(&p).unwrap_err() {
            gaply_core::GaplyError::Validation(m) => assert_eq!(m, format!("csv has more than {MAX_ROWS} rows; cap is {MAX_ROWS}")),
            other => panic!("{other:?}"),
        }
        assert!(supplementary::exceeds_row_cap(&p).unwrap());
        // At the cap: header + MAX_ROWS - 1 data rows parse, and the check agrees.
        let mut fits = String::from("id\n");
        for i in 0..(MAX_ROWS - 1) {
            fits.push_str(&format!("{i}\n"));
        }
        let q = write("fits.csv", fits.as_bytes());
        assert!(supplementary::parse_supplementary(&q).is_ok());
        assert!(!supplementary::exceeds_row_cap(&q).unwrap());
        let _ = std::fs::remove_file(&p);
        let _ = std::fs::remove_file(&q);
    }

    /* ---- evaluation fixes, part C: files ---------------------------------- */

    fn crc32(data: &[u8]) -> u32 {
        let mut c = 0xFFFF_FFFFu32;
        for &b in data {
            c ^= u32::from(b);
            for _ in 0..8 {
                c = if c & 1 != 0 { (c >> 1) ^ 0xEDB8_8320 } else { c >> 1 };
            }
        }
        !c
    }

    /// A zip of stored (uncompressed) parts. `declare` overrides, in the
    /// central directory only, one part's declared uncompressed size; with a
    /// size of 0xFFFFFFFF or more it is written the ZIP64 way, in an extra
    /// field, as Python's `force_zip64` does.
    fn stored_zip(parts: &[(&str, Vec<u8>)], declare: Option<(usize, u64)>) -> Vec<u8> {
        let mut out = Vec::new();
        let mut central = Vec::new();
        for (i, (name, data)) in parts.iter().enumerate() {
            let offset = out.len() as u32;
            let crc = crc32(data);
            let size = data.len() as u32;
            out.extend_from_slice(&[0x50, 0x4b, 0x03, 0x04, 20, 0, 0, 0, 0, 0, 0, 0, 0x21, 0]);
            for v in [crc, size, size] {
                out.extend_from_slice(&v.to_le_bytes());
            }
            out.extend_from_slice(&(name.len() as u16).to_le_bytes());
            out.extend_from_slice(&0u16.to_le_bytes());
            out.extend_from_slice(name.as_bytes());
            out.extend_from_slice(data);
            let declared = declare.filter(|(at, _)| *at == i).map_or(u64::from(size), |(_, d)| d);
            let zip64 = declared >= 0xFFFF_FFFF;
            let extra: Vec<u8> = if zip64 { [1u16.to_le_bytes().to_vec(), 8u16.to_le_bytes().to_vec(), declared.to_le_bytes().to_vec()].concat() } else { Vec::new() };
            central.extend_from_slice(&[0x50, 0x4b, 0x01, 0x02, 45, 0, 45, 0, 0, 0, 0, 0, 0, 0, 0x21, 0]);
            central.extend_from_slice(&crc.to_le_bytes());
            central.extend_from_slice(&size.to_le_bytes());
            central.extend_from_slice(&(if zip64 { 0xFFFF_FFFF } else { declared as u32 }).to_le_bytes());
            central.extend_from_slice(&(name.len() as u16).to_le_bytes());
            central.extend_from_slice(&(extra.len() as u16).to_le_bytes());
            // Comment length, disk, internal and external attributes.
            central.extend_from_slice(&[0; 10]);
            central.extend_from_slice(&offset.to_le_bytes());
            central.extend_from_slice(name.as_bytes());
            central.extend_from_slice(&extra);
        }
        let start = out.len() as u32;
        out.extend_from_slice(&central);
        out.extend_from_slice(&[0x50, 0x4b, 0x05, 0x06, 0, 0, 0, 0]);
        out.extend_from_slice(&(parts.len() as u16).to_le_bytes());
        out.extend_from_slice(&(parts.len() as u16).to_le_bytes());
        out.extend_from_slice(&(central.len() as u32).to_le_bytes());
        out.extend_from_slice(&start.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out
    }

    /// A minimal workbook: one sheet, a header row and `rows` data rows.
    fn xlsx(rows: usize, declare_sheet: Option<u64>) -> Vec<u8> {
        let mut sheet = String::from(r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><sheetData><row r="1"><c r="A1" t="inlineStr"><is><t>id</t></is></c></row>"#);
        for i in 0..rows {
            sheet.push_str(&format!(r#"<row r="{r}"><c r="A{r}"><v>{i}</v></c></row>"#, r = i + 2));
        }
        sheet.push_str("</sheetData></worksheet>");
        let parts: Vec<(&str, Vec<u8>)> = vec![
            ("[Content_Types].xml", br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/><Override PartName="/xl/worksheets/sheet1.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/></Types>"#.to_vec()),
            ("_rels/.rels", br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="xl/workbook.xml"/></Relationships>"#.to_vec()),
            ("xl/workbook.xml", br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><sheets><sheet name="Sheet1" sheetId="1" r:id="rId1"/></sheets></workbook>"#.to_vec()),
            ("xl/_rels/workbook.xml.rels", br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet1.xml"/></Relationships>"#.to_vec()),
            ("xl/worksheets/sheet1.xml", sheet.into_bytes()),
        ];
        stored_zip(&parts, declare_sheet.map(|d| (4, d)))
    }

    /// M-10: a FIFO named as the data file used to block `open` forever (the
    /// deep evaluation's probe: still blocked 5 s after a writer appeared).
    /// Now it is refused at once with a closed code, and so is a device and
    /// a directory; the manuscript's preflight refuses a FIFO too.
    #[cfg(unix)]
    #[test]
    fn a_fifo_a_device_or_a_directory_is_refused_at_once_as_not_a_regular_file() {
        let fifo = dir().join("PATH_SECRET_fifo.csv");
        let _ = std::fs::remove_file(&fifo);
        assert!(std::process::Command::new("mkfifo").arg(&fifo).status().unwrap().success());
        let (tx, rx) = std::sync::mpsc::channel();
        let f = fifo.clone();
        std::thread::spawn(move || {
            let _ = tx.send(load_dataset(&f).map(|_| ()));
        });
        let got = rx.recv_timeout(std::time::Duration::from_secs(5)).expect("load_dataset returned within 5 s");
        assert_eq!(got, Err(L1IngestRefusal::NotARegularFile));
        let (tx, rx) = std::sync::mpsc::channel();
        let f = fifo.clone();
        std::thread::spawn(move || {
            let db = gaply_core::Database::in_memory().unwrap();
            let _ = tx.send(import_guard::preflight(&db, &f, 0).map(|p| p.is_refused()));
        });
        assert!(matches!(rx.recv_timeout(std::time::Duration::from_secs(5)), Ok(Ok(true))), "a FIFO manuscript is refused at once");
        assert_eq!(load_dataset(std::path::Path::new("/dev/null")).map(|_| ()), Err(L1IngestRefusal::NotARegularFile));
        assert_eq!(load_dataset(&dir()).map(|_| ()), Err(L1IngestRefusal::NotARegularFile));
        let _ = std::fs::remove_file(&fifo);
    }

    /// M-9: a workbook whose parts declare more than the limit is refused
    /// before any part is inflated, ZIP64 sizes included. The declared size
    /// is read from the central directory, so the stored part here is small.
    #[test]
    fn a_workbook_declaring_more_than_the_limit_is_refused_before_parsing() {
        let over = MAX_WORKBOOK_UNCOMPRESSED_BYTES + 1;
        for (name, declared) in [("bomb.xlsx", over), ("bomb64.xlsx", 0x1_0000_0000)] {
            let bytes = xlsx(3, Some(declared));
            let total = declared_uncompressed_total(&bytes).expect("a zip");
            assert!(total > MAX_WORKBOOK_UNCOMPRESSED_BYTES);
            let p = write(name, &bytes);
            assert_eq!(load_dataset(&p).map(|_| ()), Err(L1IngestRefusal::ArchiveTooLarge { declared: total, limit: MAX_WORKBOOK_UNCOMPRESSED_BYTES }), "{name}");
            let _ = std::fs::remove_file(&p);
        }
        // Not a zip: left to the parser.
        assert_eq!(declared_uncompressed_total(b"not a zip at all"), None);
        // The workbook fixture declares far less, and still opens.
        let real = std::fs::read(fixture("sample.xlsx")).unwrap();
        assert!(declared_uncompressed_total(&real).unwrap() < MAX_WORKBOOK_UNCOMPRESSED_BYTES);
        assert!(load_dataset(&fixture("sample.xlsx")).is_ok());
    }

    /* ---- part C follow-up: a workbook that lies, and one read ------------- */

    /// A raw deflate stream, fixed Huffman codes, that inflates to `1 + 258 *
    /// matches` zero bytes: one literal 0, then `matches` copies of length
    /// 258 at distance 1 (13 bits each), then the end of the block.
    fn deflated_zeros(matches: usize) -> Vec<u8> {
        let (mut out, mut acc, mut n) = (Vec::new(), 0u64, 0u32);
        let mut bits = |v: u64, count: u32, out: &mut Vec<u8>| {
            acc |= v << n;
            n += count;
            while n >= 8 {
                out.push(acc as u8);
                acc >>= 8;
                n -= 8;
            }
        };
        // A Huffman code goes in most significant bit first.
        let code = |c: u64, len: u32| (0..len).fold(0u64, |r, i| r | (((c >> i) & 1) << (len - 1 - i)));
        bits(1, 1, &mut out); // BFINAL
        bits(1, 2, &mut out); // BTYPE 01: fixed codes
        bits(code(0x30, 8), 8, &mut out); // literal 0
        for _ in 0..matches {
            bits(code(0xC5, 8), 8, &mut out); // length 258 (symbol 285)
            bits(0, 5, &mut out); // distance 1 (code 0)
        }
        bits(0, 7, &mut out); // end of block (256)
        bits(0, 7, &mut out); // flush
        out
    }

    /// Part C's minimal workbook with its sheet replaced by `matches` worth of
    /// deflated zeros that DECLARES `declared` uncompressed bytes, in the
    /// local header and the central directory alike.
    fn lying_xlsx(matches: usize, declared: u32) -> Vec<u8> {
        let honest = xlsx(1, None);
        // Every part but the sheet, as Part C's helper stored them.
        let mut out = Vec::new();
        let mut central = Vec::new();
        let mut at = 0usize;
        let mut count = 0u16;
        while honest.get(at..at + 4) == Some(&[0x50, 0x4b, 0x03, 0x04][..]) {
            let u16_at = |i: usize| usize::from(u16::from_le_bytes([honest[i], honest[i + 1]]));
            let size = u32::from_le_bytes(honest[at + 18..at + 22].try_into().unwrap()) as usize;
            let name_len = u16_at(at + 26);
            let end = at + 30 + name_len + u16_at(at + 28) + size;
            let name = &honest[at + 30..at + 30 + name_len];
            if name != b"xl/worksheets/sheet1.xml" {
                let offset = out.len() as u32;
                out.extend_from_slice(&honest[at..end]);
                central.extend_from_slice(&[0x50, 0x4b, 0x01, 0x02, 45, 0, 20, 0, 0, 0, 0, 0, 0, 0, 0x21, 0]);
                central.extend_from_slice(&honest[at + 14..at + 26]); // crc, sizes
                central.extend_from_slice(&(name_len as u16).to_le_bytes());
                central.extend_from_slice(&[0; 12]);
                central.extend_from_slice(&offset.to_le_bytes());
                central.extend_from_slice(name);
                count += 1;
            }
            at = end;
        }
        let name = b"xl/worksheets/sheet1.xml";
        let data = deflated_zeros(matches);
        let offset = out.len() as u32;
        let mut header = vec![0u8; 0];
        header.extend_from_slice(&[0, 0, 0, 0]); // crc: never reached
        header.extend_from_slice(&(data.len() as u32).to_le_bytes());
        header.extend_from_slice(&declared.to_le_bytes());
        out.extend_from_slice(&[0x50, 0x4b, 0x03, 0x04, 20, 0, 0, 0, 8, 0, 0, 0, 0x21, 0]);
        out.extend_from_slice(&header);
        out.extend_from_slice(&(name.len() as u16).to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(name);
        out.extend_from_slice(&data);
        central.extend_from_slice(&[0x50, 0x4b, 0x01, 0x02, 45, 0, 20, 0, 0, 0, 8, 0, 0, 0, 0x21, 0]);
        central.extend_from_slice(&header);
        central.extend_from_slice(&(name.len() as u16).to_le_bytes());
        central.extend_from_slice(&[0; 12]);
        central.extend_from_slice(&offset.to_le_bytes());
        central.extend_from_slice(name);
        count += 1;
        let start = out.len() as u32;
        out.extend_from_slice(&central);
        out.extend_from_slice(&[0x50, 0x4b, 0x05, 0x06, 0, 0, 0, 0]);
        out.extend_from_slice(&count.to_le_bytes());
        out.extend_from_slice(&count.to_le_bytes());
        out.extend_from_slice(&(central.len() as u32).to_le_bytes());
        out.extend_from_slice(&start.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out
    }

    /// Part C follow-up, item 2: a workbook that DECLARES a 4 KiB sheet and
    /// inflates it to about 77 MB (300,000 copies of 258 bytes, past the
    /// 64 MiB limit) passes part C's declared-size check, so it reached the
    /// parser. It is now refused with the same closed code as soon as the
    /// inflated count passes the part's declared size, and nothing inflated is
    /// kept. Part C's own minimal workbook still opens.
    #[test]
    fn a_workbook_that_declares_small_sizes_and_inflates_past_them_is_refused() {
        let bytes = lying_xlsx(300_000, 4096);
        assert!(declared_uncompressed_total(&bytes).unwrap() < MAX_WORKBOOK_UNCOMPRESSED_BYTES, "declares small sizes");
        assert!(bytes.len() < 1024 * 1024, "{} bytes on disk", bytes.len());
        let p = write("lying.xlsx", &bytes);
        let started = std::time::Instant::now();
        let got = load_dataset(&p).map(|_| ());
        assert!(
            matches!(got, Err(L1IngestRefusal::ArchiveTooLarge { declared, limit: MAX_WORKBOOK_UNCOMPRESSED_BYTES }) if declared <= MAX_WORKBOOK_UNCOMPRESSED_BYTES),
            "{got:?}"
        );
        assert!(started.elapsed() < std::time::Duration::from_secs(2), "{:?}", started.elapsed());
        let honest = write("honest.xlsx", &xlsx(3, None));
        assert_eq!(load_dataset(&honest).map(|d| d.rows()), Ok(3));
        for f in [p, honest] {
            let _ = std::fs::remove_file(f);
        }
    }

    /// Part C follow-up, item 3: the file is opened and read ONCE. Replacing
    /// or removing it after the check changes nothing: the parse reads the
    /// bytes that were checked and hashed, never the path again.
    #[test]
    fn the_parse_reads_the_checked_bytes_after_the_file_is_replaced_or_removed() {
        for replace in [true, false] {
            let p = write("PATH_SECRET_once.csv", CSV);
            let checked = check_file(&p).expect("checked");
            let _ = std::fs::remove_file(&p);
            if replace {
                std::fs::write(&p, b"other,columns\n9,9\n").unwrap();
            }
            let d = dataset_from_checked(checked).unwrap_or_else(|e| panic!("replace={replace}: {e:?}"));
            assert_eq!(d.schema.fingerprint, DataFingerprint::of_bytes(CSV, PARSER_VERSION, TableRef::FirstTable), "replace={replace}");
            assert_eq!(d.schema.columns.iter().map(|c| c.header.as_str()).collect::<Vec<_>>(), vec!["score", "group", "note"]);
            assert_eq!(d.rows(), 4);
            let _ = std::fs::remove_file(&p);
        }
    }

    /// A MATLAB v5 file holding one compressed double matrix of `n` zeros,
    /// as `save('-v7')` writes a large zero vector: the miMATRIX element
    /// deflated inside a miCOMPRESSED (15) element.
    fn compressed_mat(n: usize) -> Vec<u8> {
        use std::io::Write as _;
        let real = (n * 8) as u32;
        let mut m = Vec::new();
        for v in [14u32, 48 + real, 6, 8, 6, 0, 5, 8, n as u32, 1, 1, 1] {
            m.extend_from_slice(&v.to_le_bytes());
        }
        m.extend_from_slice(b"x\0\0\0\0\0\0\0");
        m.extend_from_slice(&9u32.to_le_bytes());
        m.extend_from_slice(&real.to_le_bytes());
        let mut enc = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::fast());
        enc.write_all(&m).unwrap();
        let chunk = vec![0u8; 1 << 20];
        let mut left = n * 8;
        while left > 0 {
            let k = left.min(chunk.len());
            enc.write_all(&chunk[..k]).unwrap();
            left -= k;
        }
        let z = enc.finish().unwrap();
        let mut out = vec![b' '; 116];
        out[..19].copy_from_slice(b"MATLAB 5.0 MAT-file");
        out.extend_from_slice(&[0u8; 8]);
        out.extend_from_slice(&0x0100u16.to_le_bytes());
        out.extend_from_slice(b"IM");
        out.extend_from_slice(&15u32.to_le_bytes());
        out.extend_from_slice(&(z.len() as u32).to_le_bytes());
        out.extend_from_slice(&z);
        out
    }

    /// Part C follow-up 2: matfile inflates a compressed element with
    /// `read_to_end`, so a 7.7 MB file reached a 1 GB peak. Every compressed
    /// element is now inflated into a counter first, and past the limit the
    /// file is refused with the workbook's code before matfile sees it. The
    /// fixtures still open.
    #[test]
    fn a_mat_whose_compressed_elements_inflate_past_the_limit_is_refused_before_matfile_reads_it() {
        // 9 million doubles: 72 MB inflated, about 70 KB on disk.
        let bytes = compressed_mat(9_000_000);
        assert!(bytes.len() < 1024 * 1024, "{} bytes", bytes.len());
        let p = write("big.mat", &bytes);
        let started = std::time::Instant::now();
        let got = load_dataset(&p).map(|_| ());
        assert!(matches!(got, Err(L1IngestRefusal::ArchiveTooLarge { limit: MAX_WORKBOOK_UNCOMPRESSED_BYTES, .. })), "{got:?}");
        assert!(started.elapsed() < std::time::Duration::from_secs(2), "{:?}", started.elapsed());
        let small = write("small.mat", &compressed_mat(1000));
        assert!(!matches!(load_dataset(&small), Err(L1IngestRefusal::ArchiveTooLarge { .. })));
        for f in ["analysis_v7.mat", "analysis_mixed.mat"] {
            assert!(load_dataset(&fixture(f)).is_ok(), "{f}");
        }
        for f in [p, small] {
            let _ = std::fs::remove_file(f);
        }
    }

    /// An honest 100-row zlib-compressed SPSS file (`$FL3`), from ambers'
    /// own writer, and the offset of its trailer (found from the zlib header,
    /// whose first field is its own offset).
    fn zsav() -> (Vec<u8>, usize) {
        use arrow::array::Float64Array;
        use arrow::datatypes::{DataType, Field, Schema};
        let schema = std::sync::Arc::new(Schema::new(vec![Field::new("x", DataType::Float64, false)]));
        let batch = arrow::record_batch::RecordBatch::try_new(schema, vec![std::sync::Arc::new(Float64Array::from(vec![1.0; 100]))]).unwrap();
        let mut buf = std::io::Cursor::new(Vec::new());
        ambers::write_sav_to_writer(&mut buf, &batch, &ambers::SpssMetadata::default(), ambers::Compression::Zlib, None).unwrap();
        let b = buf.into_inner();
        let i64_at = |o: usize| i64::from_le_bytes(b[o..o + 8].try_into().unwrap());
        let zh = (0..b.len() - 24).find(|&p| i64_at(p) == p as i64 && i64_at(p + 8) as usize + i64_at(p + 16) as usize == b.len()).unwrap();
        let zt = i64_at(zh + 8) as usize;
        (b, zt)
    }

    /// Part C follow-up 2, item 3: ambers allocates each zlib block at the
    /// size the trailer declares, and the trailer's entries at the count it
    /// declares. Measured: a trailer declaring a 2 GiB block made a 2,048 MB
    /// allocation, one declaring 2^31 - 1 blocks a 49,152 MB one, and a
    /// negative size panicked. The trailer is now read before ambers runs:
    /// sizes past the limit are refused with the workbook's code, a trailer
    /// that cannot be what it declares as malformed. Honest files still open.
    #[test]
    fn a_zsav_whose_trailer_lies_is_refused_before_ambers_reads_it() {
        let (honest, zt) = zsav();
        assert_eq!(&honest[..4], b"$FL3");
        let p = write("honest.sav", &honest);
        assert_eq!(load_dataset(&p).map(|d| d.rows()), Ok(100));
        let entry = zt + 24;
        let lies: [(&str, usize, i32, bool); 3] = [
            ("a 2 GiB block", entry + 16, i32::MAX, true),
            ("a negative block", entry + 16, -1, false),
            ("2^31 - 1 blocks", zt + 20, i32::MAX, false),
        ];
        for (name, at, value, too_large) in lies {
            let mut b = honest.clone();
            b[at..at + 4].copy_from_slice(&value.to_le_bytes());
            let q = write("lie.sav", &b);
            let got = std::panic::catch_unwind(|| load_dataset(&q).map(|_| ()));
            let Ok(got) = got else { panic!("{name}: the reader panicked") };
            if too_large {
                assert!(matches!(got, Err(L1IngestRefusal::ArchiveTooLarge { limit: MAX_WORKBOOK_UNCOMPRESSED_BYTES, .. })), "{name}: {got:?}");
            } else {
                assert_eq!(got, Err(L1IngestRefusal::Malformed), "{name}");
            }
            let _ = std::fs::remove_file(&q);
        }
        assert!(load_dataset(&fixture("analysis.sav")).is_ok());
        let _ = std::fs::remove_file(&p);
    }

    /// m-7: the header is not a row. A file with exactly 5,000 data rows was
    /// refused as "over the 5,000-row limit" (the deep evaluation's
    /// `narrow_5000.csv`); 5,001 data rows are still refused.
    #[test]
    fn exactly_5000_data_rows_are_accepted_and_5001_are_not() {
        let csv = |n: usize| {
            let mut s = String::from("id\n");
            for i in 0..n {
                s.push_str(&format!("{i}\n"));
            }
            s
        };
        let p = write("rows_5000.csv", csv(MAX_ROWS).as_bytes());
        assert_eq!(load_dataset(&p).map(|d| d.rows()), Ok(MAX_ROWS));
        let q = write("rows_5001.csv", csv(MAX_ROWS + 1).as_bytes());
        assert_eq!(load_dataset(&q).map(|_| ()), Err(L1IngestRefusal::FileTooLarge { limit_rows: MAX_ROWS }));
        let x = write("rows_5000.xlsx", &xlsx(MAX_ROWS, None));
        assert_eq!(load_dataset(&x).map(|d| d.rows()), Ok(MAX_ROWS));
        let y = write("rows_5001.xlsx", &xlsx(MAX_ROWS + 1, None));
        assert_eq!(load_dataset(&y).map(|_| ()), Err(L1IngestRefusal::FileTooLarge { limit_rows: MAX_ROWS }));
        for f in [p, q, x, y] {
            let _ = std::fs::remove_file(f);
        }
    }

    #[test]
    fn the_xlsx_fixture_records_its_sheet_and_the_mat_fixture_its_variables() {
        let d = load_dataset(&fixture("sample.xlsx")).unwrap();
        assert!(matches!(d.schema.fingerprint.table(), TableRef::Sheet { index: 0, name } if !name.is_empty()));
        assert_eq!(d.schema.columns.iter().map(|c| c.header.as_str()).collect::<Vec<_>>(), vec!["id", "mean", "sd"]);
        let m = load_dataset(&fixture("analysis_v7.mat")).unwrap();
        assert_eq!(*m.schema.fingerprint.table(), TableRef::MatlabVariables);
        assert!(m.schema.columns.iter().any(|c| c.header == "reaction_ms"));
        assert_eq!(m.schema.source_metadata, SourceMetadata::NotCarried);
    }

    /// `analysis.sav` was written by pyreadstat 1.2.7 (an independent writer):
    /// `group` declares 99 user-missing and labels its codes. On the Level 1
    /// path the 99 is MISSING and the labels name the levels.
    #[test]
    fn a_sav_applies_user_missing_codes_and_carries_value_labels() {
        let d = load_dataset(&fixture("analysis.sav")).unwrap();
        assert_eq!(*d.schema.fingerprint.table(), TableRef::SpssCases);
        assert_eq!(d.schema.source_metadata, SourceMetadata::Read);
        let group = d.schema.columns.iter().find(|c| c.header == "group").expect("group");
        let cells = d.column_cells(group.alias).unwrap();
        assert!(cells.iter().any(Cell::is_missing), "the user-missing 99 must be missing: {cells:?}");
        assert!(!cells.iter().any(|c| c.number() == Some(99.0)), "99 must not survive as a number: {cells:?}");
        let levels = group.levels.as_ref().expect("levels");
        assert!(levels.levels.iter().any(|l| l.label.as_deref() == Some("control")), "{levels:?}");
        assert!(!levels.codes().any(|c| c == "99"), "{levels:?}");
        // A string variable stays a group even where its cells look numeric.
        let site = d.schema.columns.iter().find(|c| c.header == "site").expect("site");
        assert_eq!(site.column_type, ColumnType::Categorical);
        assert_eq!(d.schema.weight_variable, None);
    }

    /// `l1_weighted.sav` was written by `ambers` 0.4.4's own writer (the
    /// `generate_l1_weighted_sav_fixture` test below), so this is a round
    /// trip through one crate, not a file SPSS wrote: it pins that a declared
    /// weight reaches the schema, and nothing about SPSS compatibility.
    #[test]
    fn a_sav_with_a_declared_weight_records_it_so_step_4_can_refuse() {
        let d = load_dataset(&fixture("l1_weighted.sav")).unwrap();
        let wt = d.schema.columns.iter().find(|c| c.header == "wt").expect("wt");
        assert_eq!(d.schema.weight_variable, Some(wt.alias));
        assert!(d.schema.issues.contains(&DatasetIssue::WeightVariableDeclared { column: wt.alias }));
        let group = d.schema.columns.iter().find(|c| c.header == "group").expect("group");
        assert!(group.has_missing, "9 is declared user-missing");
        let levels = group.levels.as_ref().unwrap();
        assert_eq!(levels.levels.iter().map(|l| (l.code.as_str(), l.label.as_deref())).collect::<Vec<_>>(), vec![("1", Some("control")), ("2", Some("treatment"))]);
    }

    /// A `.sav` over the row cap: the parser truncates, and Level 1 refuses.
    #[test]
    fn a_sav_over_the_row_cap_is_file_too_large() {
        let p = dir().join("tall.sav");
        write_sav(&p, MAX_ROWS + 1, false);
        assert_eq!(load_dataset(&p).unwrap_err(), L1IngestRefusal::FileTooLarge { limit_rows: MAX_ROWS });
        let q = dir().join("fits.sav");
        write_sav(&q, MAX_ROWS, false);
        assert_eq!(load_dataset(&q).unwrap().rows(), MAX_ROWS);
        let _ = std::fs::remove_file(&p);
        let _ = std::fs::remove_file(&q);
    }

    /// Writes a `.sav` with `ambers`: `subject`, `group` (1/2/9, 9 declared
    /// user-missing, 1 and 2 labelled), `score`, and `wt` declared the weight
    /// when `weighted`.
    fn write_sav(path: &Path, rows: usize, weighted: bool) {
        use arrow::array::Float64Array;
        use arrow::datatypes::{DataType, Field, Schema};
        use arrow::record_batch::RecordBatch;
        use std::sync::Arc;
        let subject: Vec<f64> = (1..=rows).map(|i| i as f64).collect();
        let group: Vec<f64> = (0..rows).map(|i| [1.0, 2.0, 9.0][i % 3]).collect();
        let score: Vec<f64> = (0..rows).map(|i| 10.0 + (i % 7) as f64).collect();
        let wt: Vec<f64> = (0..rows).map(|i| 0.5 + (i % 4) as f64 * 0.25).collect();
        let schema = Arc::new(Schema::new(vec![
            Field::new("subject", DataType::Float64, false),
            Field::new("group", DataType::Float64, false),
            Field::new("score", DataType::Float64, false),
            Field::new("wt", DataType::Float64, false),
        ]));
        let batch = RecordBatch::try_new(
            schema.clone(),
            vec![
                Arc::new(Float64Array::from(subject)),
                Arc::new(Float64Array::from(group)),
                Arc::new(Float64Array::from(score)),
                Arc::new(Float64Array::from(wt)),
            ],
        )
        .unwrap();
        let mut meta = ambers::SpssMetadata::from_arrow_schema(&schema);
        meta.variable_missing_values.entry("group".into()).or_default().push(ambers::MissingSpec::Value(9.0));
        let labels = meta.variable_value_labels.entry("group".into()).or_default();
        labels.insert(ambers::Value::Numeric(1.0), "control".into());
        labels.insert(ambers::Value::Numeric(2.0), "treatment".into());
        if weighted {
            meta.weight_variable = Some("wt".into());
        }
        ambers::write_sav(path, &batch, &meta, ambers::Compression::None, None).unwrap();
    }

    /// The five fields Step 2 added to the ingest types are `serde(skip)`:
    /// `SupplementaryEvidence` is serialised toward the PublishReady reviewer
    /// (`commands.rs`, `serde_json::to_value(&ev)`), and that payload must not
    /// gain a weight name, a sheet name, typed missing rules or label codes.
    #[test]
    fn the_publishready_payload_does_not_carry_the_level_1_fields() {
        let ev = supplementary::parse_supplementary(&fixture("l1_weighted.sav")).unwrap();
        assert!(ev.weight_variable.is_some() && !ev.variables[1].user_missing_rules.is_empty(), "the fields must be populated for the check to mean anything");
        let json = serde_json::to_string(&ev).unwrap();
        for key in ["weight_variable", "rows_truncated", "sheet_name", "user_missing_rules", "value_label_codes"] {
            assert!(!json.contains(key), "{key} reached the PublishReady payload: {json}");
        }
        assert!(json.contains("user_missing"), "the pre-existing field is still there");
    }

    /// A run with one user claim and `draft` confirmed against the file at
    /// `path`: the state, the token and the claim.
    fn confirmed_from_path(
        path: &Path,
        draft: impl FnOnce(&Dataset) -> gaply_core::stats_l1::binding::BindingDraft,
    ) -> (L1RunState, ConfirmationToken, ClaimId) {
        use gaply_core::extract::SectionKind;
        use gaply_core::stats_l0::number::Relation;
        use gaply_core::stats_l0::result::Family;
        use gaply_core::stats_l1::binding::ProposalSource;
        use gaply_core::stats_l1::claim::{ClaimLocation, UserClaim, UserP};
        let data = load_dataset(path).unwrap();
        let d = draft(&data);
        let mut state = L1RunState::new();
        state.load_dataset(data);
        let claim = state
            .add_claim(&UserClaim {
                family: Family::T,
                statistic: "2.00",
                df: vec!["2"],
                n: None,
                p: UserP::Reported { raw: ".05", relation: Relation::Eq },
                location: ClaimLocation { section_index: 0, section: SectionKind::Results, paragraph: 0, sentence: 0 },
                sentence: None,
            })
            .unwrap();
        let id = state.add_proposal(claim, d, ProposalSource::Manual { edited_from: None }).unwrap();
        let token = state.confirm(id).unwrap().1;
        (state, token, claim)
    }

    fn t_on(outcome: &str, group: &str) -> impl FnOnce(&Dataset) -> gaply_core::stats_l1::binding::BindingDraft {
        use gaply_core::stats_l1::binding::BindingDraft;
        use gaply_core::stats_l1::catalogue::{L1MethodId, RoleName, VariantChoice, VariantKind};
        let (outcome, group) = (outcome.to_string(), group.to_string());
        move |d: &Dataset| {
            let alias = |h: &str| d.schema.columns.iter().find(|c| c.header == h).unwrap().alias;
            BindingDraft::new(L1MethodId::IndependentT, VariantChoice::not_stated(VariantKind::T))
                .role(RoleName::Outcome, &[alias(&outcome)])
                .role(RoleName::Group, &[alias(&group)])
        }
    }

    /// The file is read again for the computation, and the result is the
    /// one the confirm pass counted for.
    #[test]
    fn a_confirmed_binding_computes_from_the_file_read_again() {
        let p = write("compute.csv", b"score,group\n1.5,1\n2.5,1\n3.0,1\n4.5,2\n6.0,2\n,2\n");
        let (state, token, claim) = confirmed_from_path(&p, t_on("score", "group"));
        let c = compute_from_path(&state, &token, claim, &p).unwrap();
        assert_eq!((c.counts.rows_read, c.counts.n_used), (6, 5));
        assert_eq!(c.results.len(), 2, "Student and Welch");
        let _ = std::fs::remove_file(&p);
    }

    /// One byte edited after the confirmation: the token is still live, and
    /// the computation refuses because the bytes are not the confirmed bytes.
    #[test]
    fn a_file_edited_after_confirmation_is_refused_not_computed_on() {
        let p = write("edited.csv", b"score,group\n1.5,1\n2.5,1\n3.0,1\n4.5,2\n6.0,2\n");
        let (state, token, claim) = confirmed_from_path(&p, t_on("score", "group"));
        std::fs::write(&p, b"score,group\n1.5,1\n2.5,1\n3.0,1\n4.5,2\n9.0,2\n").unwrap();
        assert_eq!(compute_from_path(&state, &token, claim, &p).unwrap_err(), L1ComputeError::Refused(L1Refusal::FingerprintChanged));
        let _ = std::fs::remove_file(&p);
    }

    /// A token that is not live is refused before the file is opened: a path
    /// that does not exist gives the gate's refusal, not `NotReadable`.
    #[test]
    fn no_file_is_read_without_a_live_token() {
        let p = write("gate.csv", b"score,group\n1.5,1\n2.5,1\n4.5,2\n6.0,2\n");
        let (state, _token, claim) = confirmed_from_path(&p, t_on("score", "group"));
        let forged = ConfirmationToken::presented(&"0".repeat(64));
        let missing = dir().join("never_written.csv");
        assert_eq!(
            compute_from_path(&state, &forged, claim, &missing).unwrap_err(),
            L1ComputeError::Refused(L1Refusal::Gate(gaply_core::stats_l1::run_state::GateRejection::Unknown))
        );
        let _ = std::fs::remove_file(&p);
    }

    /// `l1_weighted.sav` declares `wt` the weight: Level 1 refuses it.
    #[test]
    fn a_weighted_sav_is_refused_on_the_level_1_path() {
        let p = fixture("l1_weighted.sav");
        let (state, token, claim) = confirmed_from_path(&p, t_on("score", "group"));
        assert_eq!(compute_from_path(&state, &token, claim, &p).unwrap_err(), L1ComputeError::Refused(L1Refusal::WeightedFile));
    }

    /// The generator for `tests/fixtures/l1_weighted.sav`. Ignored: run once
    /// with `--ignored` to (re)write the committed fixture. Six rows.
    #[test]
    #[ignore]
    fn generate_l1_weighted_sav_fixture() {
        write_sav(&fixture("l1_weighted.sav"), 6, true);
    }
}
