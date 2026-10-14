//! Findings from Phase 2, Step 8, section 4 (real papers with open data), and
//! the ingest messages Step 8b added for them. Finding 1's test was written
//! failing in Step 8 and passes since Step 8b.

use std::path::PathBuf;

use app_lib::stats_l1_ingest::load_dataset;
use gaply_core::stats_l1::dataset::{DatasetIssue, DatasetRefusal, L1IngestRefusal, Separator};

/// A file of this test's own, in a folder of its own.
fn write(name: &str, bytes: &[u8]) -> (PathBuf, PathBuf) {
    use std::sync::atomic::{AtomicUsize, Ordering};
    static N: AtomicUsize = AtomicUsize::new(0);
    let dir = std::env::temp_dir().join(format!("gaply-l1-s4-{}-{}", std::process::id(), N.fetch_add(1, Ordering::SeqCst)));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join(name);
    std::fs::write(&path, bytes).unwrap();
    (dir, path)
}

/// Section 4, PLOS ONE 0245226 (S2 file, 404 columns): Level 1 told the user
/// the file was `malformed`. The file is well formed; it is wider than the
/// ingest's 128-column cap, which `supplementary.rs` enforces before Level 1's
/// own schema builder sees it, and `load_dataset` mapped every parse failure
/// except the row cap to `Malformed`.
#[test]
fn a_data_file_over_the_column_cap_is_refused_as_too_wide_not_malformed() {
    let cols = 129;
    let mut s = (1..=cols).map(|i| format!("v{i}")).collect::<Vec<_>>().join(",") + "\n";
    for r in 0..3 {
        s += &((1..=cols).map(|i| format!("{}", i + r)).collect::<Vec<_>>().join(",") + "\n");
    }
    let (dir, path) = write("wide.csv", s.as_bytes());
    let got = load_dataset(&path).err();
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(got, Some(L1IngestRefusal::Dataset(DatasetRefusal::TooWide { columns: 129, limit: 128 })));
}

/// Section 4, PLOS ONE 0114578: an Excel 2.x (BIFF2) `.xls`, which is not a
/// compound document. The fixture is synthetic: a BIFF2 BOF record (type
/// 0x0009, length 4, version 2, worksheet 0x0010) and an EOF record, the
/// shape of that file's first bytes, holding no data.
#[test]
fn an_old_excel_format_is_refused_as_xls_not_readable() {
    let biff2: &[u8] = &[0x09, 0x00, 0x04, 0x00, 0x02, 0x00, 0x10, 0x00, 0x0A, 0x00, 0x00, 0x00];
    let (dir, path) = write("old.xls", biff2);
    let got = load_dataset(&path).err();
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(got, Some(L1IngestRefusal::XlsNotReadable));
    // Not every unreadable spreadsheet is this: a broken .xlsx stays malformed.
    let (dir, path) = write("broken.xlsx", b"this is not a zip");
    let got = load_dataset(&path).err();
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(got, Some(L1IngestRefusal::Malformed));
}

/// Section 4, PLOS ONE 0286503: semicolon-separated CSVs read as one text
/// column. Step 8b, rule 3: not parsed, but flagged so the screen can say so.
#[test]
fn a_one_column_csv_holding_semicolons_or_tabs_carries_a_looks_separated_issue() {
    let (dir, path) = write("semi.csv", "BS Blue;BS Red\n-3,5571;0,46767\n-6,9356;0,82106\n2,1;1,3\n".as_bytes());
    let d = load_dataset(&path).expect("it reads, as one column");
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(d.schema.columns.len(), 1);
    assert!(d.schema.issues.contains(&DatasetIssue::LooksSeparatedBy { separator: Separator::Semicolon }), "{:?}", d.schema.issues);

    let (dir, path) = write("tabbed.csv", "a\tb\n1\t2\n3\t4\n".as_bytes());
    let d = load_dataset(&path).expect("it reads");
    let _ = std::fs::remove_dir_all(&dir);
    assert!(d.schema.issues.contains(&DatasetIssue::LooksSeparatedBy { separator: Separator::Tab }), "{:?}", d.schema.issues);

    // A normal two-column CSV, and a one-column CSV without the character,
    // carry no such issue.
    for (name, text) in [("ok.csv", "a,b\n1,2\n3,4\n"), ("one.csv", "score\n1.5\n2.5\n")] {
        let (dir, path) = write(name, text.as_bytes());
        let d = load_dataset(&path).expect("it reads");
        let _ = std::fs::remove_dir_all(&dir);
        assert!(!d.schema.issues.iter().any(|i| matches!(i, DatasetIssue::LooksSeparatedBy { .. })), "{name}: {:?}", d.schema.issues);
    }
}
