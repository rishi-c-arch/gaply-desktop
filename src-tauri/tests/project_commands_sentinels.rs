//! **The project commands put user text only where decisions 101, 125 and 127
//! say it may (Research OS, Phase 5B-2).**
//!
//! One session through the commands, with sentinels planted in every place
//! user text can come from: a data file's cells, its level labels and its
//! headers; the file's name and the folder it is in; the project's name; and
//! an Excel file whose author, last editor and last-saved folder are
//! sentinels (decision 127). It opens both files, runs a summary and a t-test
//! on each, saves all four runs into one project, lists, opens, reads every
//! run back, renders both exports, provokes a refusal of every kind it can
//! reach, and deletes a second project. Then every byte the session left is
//! read: every command response, every refusal's text, every tracing line at
//! every level from every thread, both exports, and every file under the
//! root, `project.db`, `-wal` and `-shm` included.
//!
//! * Cells, labels and headers appear only inside artifact files and in a
//!   run's result read back.
//! * The project's name appears only in `project.title` (read column by
//!   column from every table), in the list and open responses, and as the
//!   report's title.
//! * The file name, the folder and the Excel file's author and folder appear
//!   nowhere outside the data file's own copy: the artifact whose bytes are
//!   the file's bytes.
//!
//! One test in its own binary, with a global subscriber: a run's work goes
//! on its own thread, and a thread-default subscriber would not see it.

mod project_support;

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use app_lib::project_commands::{self as pc, SaveTarget};
use app_lib::stats_desc_commands as desc;
use gaply_core::GaplyError;

use project_support::*;

// Letters and digits only: the report escapes punctuation, so a sentinel
// holding any would leak there in a form a plain search does not find.
const CELL: &str = "CELLSENTc41";
const LEVEL_A: &str = "LEVELSENTa52";
const LEVEL_B: &str = "LEVELSENTb53";
const HEADER: &str = "HDRSENTh63";
const NAME: &str = "NAMESENTn74";
const FILE: &str = "FILESENTf85";
const FOLDER: &str = "PATHSENTp96";
const AUTHOR: &str = "AUTHORSENTx17";
const SAVED_IN: &str = "SAVEDINSENTy28";

#[derive(Clone, Default)]
struct Capture(Arc<Mutex<Vec<u8>>>);

impl std::io::Write for Capture {
    fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(b);
        Ok(b.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

fn crc32(data: &[u8]) -> u32 {
    let mut c = 0xFFFF_FFFFu32;
    for &b in data {
        c ^= u32::from(b);
        for _ in 0..8 {
            c = if c & 1 != 0 {
                (c >> 1) ^ 0xEDB8_8320
            } else {
                c >> 1
            };
        }
    }
    !c
}

/// A zip of stored (uncompressed) parts, so every sentinel is in its bytes.
fn stored_zip(parts: &[(&str, Vec<u8>)]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut central = Vec::new();
    for (name, data) in parts {
        let offset = out.len() as u32;
        let (crc, size) = (crc32(data), data.len() as u32);
        out.extend_from_slice(&[0x50, 0x4b, 0x03, 0x04, 20, 0, 0, 0, 0, 0, 0, 0, 0x21, 0]);
        for v in [crc, size, size] {
            out.extend_from_slice(&v.to_le_bytes());
        }
        out.extend_from_slice(&(name.len() as u16).to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(name.as_bytes());
        out.extend_from_slice(data);
        central.extend_from_slice(&[
            0x50, 0x4b, 0x01, 0x02, 20, 0, 20, 0, 0, 0, 0, 0, 0, 0, 0x21, 0,
        ]);
        for v in [crc, size, size] {
            central.extend_from_slice(&v.to_le_bytes());
        }
        central.extend_from_slice(&(name.len() as u16).to_le_bytes());
        central.extend_from_slice(&[0; 12]);
        central.extend_from_slice(&offset.to_le_bytes());
        central.extend_from_slice(name.as_bytes());
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

/// A workbook as Excel writes one: its sheet, and in its document
/// properties an author and a last editor, and in the workbook part the
/// folder it was last saved in.
fn workbook() -> Vec<u8> {
    let rows = [
        [HEADER.to_string(), "grp".to_string(), "note".to_string()],
        ["1.5".into(), LEVEL_A.into(), CELL.into()],
        ["2.5".into(), LEVEL_A.into(), "x".into()],
        ["3.0".into(), LEVEL_A.into(), "y".into()],
        ["3.5".into(), LEVEL_B.into(), "z".into()],
        ["4.5".into(), LEVEL_B.into(), "w".into()],
        ["6.0".into(), LEVEL_B.into(), "v".into()],
    ];
    let mut sheet = String::from(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><sheetData>"#,
    );
    for (r, row) in rows.iter().enumerate() {
        sheet.push_str(&format!(r#"<row r="{}">"#, r + 1));
        for (c, v) in row.iter().enumerate() {
            let at = format!("{}{}", (b'A' + c as u8) as char, r + 1);
            if r > 0 && c == 0 {
                sheet.push_str(&format!(r#"<c r="{at}"><v>{v}</v></c>"#));
            } else {
                sheet.push_str(&format!(
                    r#"<c r="{at}" t="inlineStr"><is><t>{v}</t></is></c>"#
                ));
            }
        }
        sheet.push_str("</row>");
    }
    sheet.push_str("</sheetData></worksheet>");
    let core = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><cp:coreProperties xmlns:cp="http://schemas.openxmlformats.org/package/2006/metadata/core-properties" xmlns:dc="http://purl.org/dc/elements/1.1/"><dc:creator>{AUTHOR}</dc:creator><cp:lastModifiedBy>{AUTHOR}</cp:lastModifiedBy></cp:coreProperties>"#
    );
    let workbook = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006" mc:Ignorable="x15"><mc:AlternateContent><mc:Choice Requires="x15"><x15ac:absPath xmlns:x15ac="http://schemas.microsoft.com/office/spreadsheetml/2010/11/ac" url="C:\Users\{SAVED_IN}\Documents\"/></mc:Choice></mc:AlternateContent><sheets><sheet name="Sheet1" sheetId="1" r:id="rId1"/></sheets></workbook>"#
    );
    stored_zip(&[
        (
            "[Content_Types].xml",
            br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/><Override PartName="/xl/worksheets/sheet1.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/><Override PartName="/docProps/core.xml" ContentType="application/vnd.openxmlformats-package.core-properties+xml"/></Types>"#.to_vec(),
        ),
        (
            "_rels/.rels",
            br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="xl/workbook.xml"/><Relationship Id="rId2" Type="http://schemas.openxmlformats.org/package/2006/relationships/metadata/core-properties" Target="docProps/core.xml"/></Relationships>"#.to_vec(),
        ),
        ("docProps/core.xml", core.into_bytes()),
        ("xl/workbook.xml", workbook.into_bytes()),
        (
            "xl/_rels/workbook.xml.rels",
            br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet1.xml"/></Relationships>"#.to_vec(),
        ),
        ("xl/worksheets/sheet1.xml", sheet.into_bytes()),
    ])
}

fn csv() -> Vec<u8> {
    format!(
        "{HEADER},grp,note\n1.5,{LEVEL_A},{CELL}\n2.5,{LEVEL_A},x\n3.0,{LEVEL_A},y\n3.5,{LEVEL_B},z\n4.5,{LEVEL_B},w\n6.0,{LEVEL_B},v\n"
    )
    .into_bytes()
}

fn contains(hay: &[u8], needle: &str) -> bool {
    hay.windows(needle.len()).any(|w| w == needle.as_bytes())
}

fn files_under(dir: &Path, out: &mut Vec<PathBuf>) {
    for e in std::fs::read_dir(dir).unwrap().flatten() {
        let p = e.path();
        let ft = e.file_type().unwrap();
        if ft.is_dir() {
            files_under(&p, out);
        } else if ft.is_file() {
            out.push(p);
        }
    }
}

/// Every text cell of every table of the database at `db`, as
/// (table.column, value).
fn every_text_cell(db: &Path) -> Vec<(String, String)> {
    let c = rusqlite::Connection::open_with_flags(db, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
        .unwrap();
    let tables: Vec<String> = {
        let mut st = c
            .prepare("SELECT name FROM sqlite_master WHERE type = 'table'")
            .unwrap();
        let names = st
            .query_map([], |r| r.get(0))
            .unwrap()
            .map(|r| r.unwrap())
            .collect();
        names
    };
    assert!(tables.len() >= 6, "{tables:?}");
    let mut out = Vec::new();
    for t in tables {
        let cols: Vec<String> = {
            let mut st = c
                .prepare(&format!("SELECT name FROM pragma_table_info('{t}')"))
                .unwrap();
            let names = st
                .query_map([], |r| r.get(0))
                .unwrap()
                .map(|r| r.unwrap())
                .collect();
            names
        };
        for col in cols {
            let mut st = c
                .prepare(&format!("SELECT CAST(\"{col}\" AS TEXT) FROM \"{t}\""))
                .unwrap();
            for v in st.query_map([], |r| r.get::<_, Option<String>>(0)).unwrap() {
                if let Some(v) = v.unwrap() {
                    out.push((format!("{t}.{col}"), v));
                }
            }
        }
    }
    out
}

fn new_project(name: &str) -> SaveTarget {
    SaveTarget::NewProject {
        name: name.to_string(),
    }
}

fn into_open(id: &str) -> SaveTarget {
    SaveTarget::OpenProject {
        project_id: id.to_string(),
    }
}

#[test]
fn user_text_stays_where_decisions_101_125_and_127_put_it() {
    let capture = Capture::default();
    let writer = capture.clone();
    tracing::subscriber::set_global_default(
        tracing_subscriber::fmt()
            .with_max_level(tracing::Level::TRACE)
            .with_ansi(false)
            .with_writer(move || writer.clone())
            .finish(),
    )
    .expect("the one subscriber of this binary");

    let w = World::new("sentinels");
    let folder = w.dir.0.join(FOLDER);
    std::fs::create_dir_all(&folder).unwrap();
    let csv_bytes = csv();
    let xlsx_bytes = workbook();
    let csv_path = folder.join(format!("{FILE}.csv"));
    let xlsx_path = folder.join(format!("{FILE}.xlsx"));
    std::fs::write(&csv_path, &csv_bytes).unwrap();
    std::fs::write(&xlsx_path, &xlsx_bytes).unwrap();

    // Every response and every refusal, as the webview would read them.
    let mut responses: Vec<(String, String)> = Vec::new();
    let mut refusals: Vec<String> = Vec::new();
    let mut reads: Vec<String> = Vec::new();
    let mut note = |what: &str, v: String| responses.push((what.to_string(), v));
    let mut refused = |e: GaplyError| {
        refusals.push(format!("{e} | {e:?}"));
    };

    let root = w.root();
    let e = w.engine();
    let mut runs = Vec::new();
    for p in [&csv_path, &xlsx_path] {
        let slot = desc::open(&e.datasets, p).expect("opens").slot;
        runs.push(w.describe(slot));
        runs.push(w.ttest(slot));
    }

    // Names and states that are refused.
    for bad in ["", "evil\u{202E}x", &"n".repeat(101)] {
        refused(pc::save_run(&root, &w.ws, runs[0], new_project(bad)).unwrap_err());
    }
    refused(pc::save_run(&root, &w.ws, 999, new_project(NAME)).unwrap_err());
    refused(pc::runs(&root, &w.ws, &"f".repeat(32)).unwrap_err());

    let first = pc::save_run(&root, &w.ws, runs[0], new_project(NAME)).unwrap();
    note("save", serde_json::to_string(&first).unwrap());
    let id = first.project_id.clone();
    for r in &runs[1..] {
        note(
            "save into",
            serde_json::to_string(&pc::save_run(&root, &w.ws, *r, into_open(&id)).unwrap())
                .unwrap(),
        );
    }
    refused(pc::save_run(&root, &w.ws, runs[1], into_open(&id)).unwrap_err());
    let listed = pc::list(&root, &w.ws).unwrap();
    let list_json = serde_json::to_string(&listed).unwrap();
    note(
        "sizes",
        serde_json::to_string(&pc::sizes(&root, &w.ws).unwrap()).unwrap(),
    );
    pc::close(&root, &w.ws).unwrap();
    let opened = serde_json::to_string(&pc::open(&root, &w.ws, &id).unwrap()).unwrap();
    note(
        "runs",
        serde_json::to_string(&pc::runs(&root, &w.ws, &id).unwrap()).unwrap(),
    );
    for n in 1..=4 {
        let back = pc::run_result(&root, &w.ws, &id, n).unwrap();
        reads.push(serde_json::to_string(&back).unwrap());
    }
    refused(pc::run_result(&root, &w.ws, &id, 99).unwrap_err());
    let report = pc::report(&root, &w.ws, &id).unwrap();
    let prov = pc::provenance(&root, &w.ws, &id).unwrap();
    note(
        "report view",
        serde_json::to_string(
            &serde_json::json!({ "fileName": report.file_name, "sha256": report.sha256 }),
        )
        .unwrap(),
    );
    note(
        "provenance view",
        serde_json::to_string(
            &serde_json::json!({ "fileName": prov.file_name, "sha256": prov.sha256 }),
        )
        .unwrap(),
    );
    let report_text = String::from_utf8(report.bytes.clone()).unwrap();

    // A second project, deleted, with its refusals.
    let slot = desc::open(&e.datasets, &csv_path).unwrap().slot;
    let gone = w.describe(slot);
    let second = pc::save_run(&root, &w.ws, gone, new_project("Second")).unwrap();
    refused(pc::delete(&root, &w.ws, &second.project_id, None).unwrap_err());
    let c = pc::prepare_delete(&root, &w.ws, &second.project_id).unwrap();
    note("prepare delete", serde_json::to_string(&c).unwrap());
    note(
        "delete",
        serde_json::to_string(
            &pc::delete(&root, &w.ws, &second.project_id, Some(&c.confirmation)).unwrap(),
        )
        .unwrap(),
    );
    refused(pc::delete(&root, &w.ws, &second.project_id, Some(&c.confirmation)).unwrap_err());

    let logs = String::from_utf8(capture.0.lock().unwrap().clone()).unwrap();
    // The instrument is live, and saw the runs' own threads.
    assert!(logs.lines().count() >= 20, "{} lines", logs.lines().count());
    assert!(logs.contains("project: run saved"));
    assert!(
        logs.contains("stats_desc: run ended"),
        "a worker thread's event"
    );
    assert!(refusals.len() >= 8, "{}", refusals.len());

    // ---- the file name, the folder, the author and the saved-in folder ----
    let nowhere = [FILE, FOLDER, AUTHOR, SAVED_IN];
    let mut files = Vec::new();
    files_under(&root, &mut files);
    assert!(files.len() >= 10, "{} files", files.len());
    let data_copies: Vec<&PathBuf> = files
        .iter()
        .filter(|p| {
            let b = std::fs::read(p).unwrap();
            b == csv_bytes || b == xlsx_bytes
        })
        .collect();
    assert_eq!(
        data_copies.len(),
        2,
        "each data file saved once, byte for byte"
    );
    assert!(
        data_copies
            .iter()
            .any(|p| contains(&std::fs::read(p).unwrap(), AUTHOR)),
        "the copy is the user's file exactly, author included (decision 127)"
    );
    let mut leaks = Vec::new();
    for f in &files {
        if data_copies.contains(&f) {
            continue;
        }
        let bytes = std::fs::read(f).unwrap();
        let rel = f.strip_prefix(&root).unwrap().display().to_string();
        assert!(
            !rel.contains(FILE) && !rel.contains(NAME),
            "a path names user text: {rel}"
        );
        for s in nowhere {
            if contains(&bytes, s) {
                leaks.push(format!("{s} in {rel}"));
            }
        }
    }
    let surfaces: Vec<(&str, &str)> = responses
        .iter()
        .map(|(w, v)| (w.as_str(), v.as_str()))
        .chain(std::iter::once(("list", list_json.as_str())))
        .chain(std::iter::once(("open", opened.as_str())))
        .chain(reads.iter().map(|r| ("run result", r.as_str())))
        .chain(refusals.iter().map(|r| ("refusal", r.as_str())))
        .chain(std::iter::once(("report", report_text.as_str())))
        .chain(std::iter::once((
            "provenance",
            std::str::from_utf8(&prov.bytes).unwrap(),
        )))
        .chain(std::iter::once(("logs", logs.as_str())))
        .collect();
    for (what, text) in &surfaces {
        for s in nowhere {
            if text.contains(s) {
                leaks.push(format!("{s} in {what}"));
            }
        }
    }
    assert!(
        leaks.is_empty(),
        "outside the data file's copy:\n{}",
        leaks.join("\n")
    );

    // ---- cells, labels and headers ----
    let content = [CELL, LEVEL_A, LEVEL_B, HEADER];
    let mut leaks = Vec::new();
    let artifacts_dir = root.join("projects").join(&id).join("artifacts");
    let mut found_in_artifacts = 0;
    for f in &files {
        let bytes = std::fs::read(f).unwrap();
        let in_artifacts = f.starts_with(&artifacts_dir);
        for s in content {
            if contains(&bytes, s) {
                if in_artifacts {
                    found_in_artifacts += 1;
                } else {
                    leaks.push(format!("{s} in {}", f.display()));
                }
            }
        }
    }
    assert!(
        found_in_artifacts >= 8,
        "the sweep sees the artifacts: {found_in_artifacts}"
    );
    for (what, text) in &surfaces {
        if *what == "run result" {
            continue;
        }
        for s in content {
            if text.contains(s) {
                leaks.push(format!("{s} in {what}"));
            }
        }
    }
    assert!(
        reads
            .iter()
            .any(|r| r.contains(HEADER) && r.contains(LEVEL_A)),
        "a run's result read back holds its headers and labels"
    );
    assert!(
        leaks.is_empty(),
        "cells, labels or headers outside the artifacts:\n{}",
        leaks.join("\n")
    );

    // ---- the project's name ----
    let mut leaks = Vec::new();
    let project_dir = root.join("projects").join(&id);
    for (col, v) in every_text_cell(&project_dir.join("project.db")) {
        if v.contains(NAME) && col != "project.title" {
            leaks.push(format!("the name in {col}"));
        }
    }
    for f in &files {
        let name = f.file_name().unwrap().to_string_lossy().into_owned();
        let in_db = f.parent() == Some(project_dir.as_path())
            && (name == "project.db" || name == "project.db-wal" || name == "project.db-shm");
        if !in_db && contains(&std::fs::read(f).unwrap(), NAME) {
            leaks.push(format!("the name in {}", f.display()));
        }
    }
    for (what, text) in &surfaces {
        let allowed = match *what {
            "list" | "open" => true,
            "report" => {
                assert_eq!(text.lines().next(), Some(format!("# {NAME}").as_str()));
                text.matches(NAME).count() == 1
            }
            _ => false,
        };
        if !allowed && text.contains(NAME) {
            leaks.push(format!("the name in {what}"));
        }
    }
    assert!(list_json.contains(NAME) && opened.contains(NAME));
    assert!(leaks.is_empty(), "{}", leaks.join("\n"));

    // The deleted project left nothing.
    let mut all = Vec::new();
    files_under(&root, &mut all);
    assert!(all
        .iter()
        .all(|p| !p.display().to_string().contains(&second.project_id)));
}
