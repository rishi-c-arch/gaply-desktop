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
//! 3. SHA-256 over the exact file bytes, read before the parse and again
//!    after it: a file that changed between the two is refused rather than
//!    fingerprinted as something it no longer is.
//! 4. [`build_dataset`] in `gaply_core`.
//!
//! A file over the ingest row cap is [`L1IngestRefusal::FileTooLarge`]: CSV
//! and spreadsheets refuse at the parser, and `.sav`/`.mat`, which the parser
//! truncates, are refused here on `rows_truncated`. Level 1 never computes on
//! a truncated table, and the cap is not raised (decisions.md).
//!
//! Every refusal is a typed value with no path, no file name and no message
//! text from the parser: the only things in it are a byte count or a row cap.

use std::path::Path;

use gaply_core::extract::docparse::DocumentShape;
use gaply_core::import_guard::{self, EstimateBasis};
use gaply_core::stats_l1::dataset::{
    build_dataset, DataFingerprint, Dataset, DatasetRefusal, RawTable, RawVariable, SourceMetadata, TableRef,
};

use crate::supplementary::{self, SupplementaryKind, MAX_FILE_SIZE_BYTES, MAX_ROWS, PARSER_VERSION};

/// Why a file did not become a dataset. No path, no name, no parser text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum L1IngestRefusal {
    /// The file could not be stat'ed or read.
    NotReadable,
    /// Over `import_guard`'s byte ceiling or the ingest's own size cap.
    TooLarge { bytes: u64, limit: u64 },
    /// Not a format the ingest turns into a table.
    UnsupportedFormat,
    /// The parser could not read the file.
    Malformed,
    /// The file parsed and holds no table.
    NoTable,
    /// More rows than the ingest cap. Level 1 does not compute on a
    /// truncated table.
    FileTooLarge { limit_rows: usize },
    /// The bytes changed between the hash and the parse.
    FileChangedWhileReading,
    /// The parsed table could not become a schema.
    Dataset(DatasetRefusal),
}

/// Extensions the ingest turns into a table. Text formats parse but hold no
/// table and are refused before being read.
const TABLE_EXTENSIONS: &[&str] = &["sav", "mat", "xlsx", "xls", "xlsb", "ods", "xlsm", "csv", "tsv"];

/// The file at `path` as a Level 1 dataset.
#[doc(hidden)]
#[tracing::instrument(skip(path))]
pub fn load_dataset(path: &Path) -> Result<Dataset, L1IngestRefusal> {
    let bytes_len = std::fs::metadata(path).map_err(|_| L1IngestRefusal::NotReadable)?.len();

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
    let before = std::fs::read(path).map_err(|_| L1IngestRefusal::NotReadable)?;
    // A failed parse is classified by a typed check of its own
    // (`exceeds_row_cap`), never by reading the parser's message, which the
    // existing Stats Verifier shows unchanged.
    let evidence = supplementary::parse_supplementary(path).map_err(|_| {
        if supplementary::exceeds_row_cap(path).unwrap_or(false) {
            L1IngestRefusal::FileTooLarge { limit_rows: MAX_ROWS }
        } else {
            L1IngestRefusal::Malformed
        }
    })?;
    let after = std::fs::read(path).map_err(|_| L1IngestRefusal::NotReadable)?;
    if before != after {
        return Err(L1IngestRefusal::FileChangedWhileReading);
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use gaply_core::stats_l1::dataset::{Cell, ColumnAlias, ColumnType, DatasetIssue};
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

    /// The generator for `tests/fixtures/l1_weighted.sav`. Ignored: run once
    /// with `--ignored` to (re)write the committed fixture. Six rows.
    #[test]
    #[ignore]
    fn generate_l1_weighted_sav_fixture() {
        write_sav(&fixture("l1_weighted.sav"), 6, true);
    }
}
