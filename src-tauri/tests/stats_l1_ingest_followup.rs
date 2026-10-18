//! **Level 1's ingest: two refusals told apart (Research OS, decision 52;
//! evaluation of Phases 2 and 3, F10 and F14).**
//!
//! * F10: a CSV or TSV in Latin-1 or UTF-16 was refused as `malformed`, so a
//!   user could not tell an encoding from a broken file. It is now
//!   `encoding_not_utf8`. Only a file the parser has ALREADY failed on is
//!   classified, so no file that opens today can change; and bytes that are
//!   not text in any encoding (control bytes) stay `malformed`.
//! * F14: a workbook whose sheet nests elements far beyond any SpreadsheetML
//!   (200,000 inside one cell, in the evaluation) opened as an empty table.
//!   It is now `malformed`. Ordinary workbooks, rich text included, open as
//!   before.
//!
//! Through `load_dataset`, the path every Level 1 and describe open takes,
//! and the wire code a screen reads.

mod desc_support;

use app_lib::stats_l1_ingest::load_dataset;
use desc_support::{delimited, xlsx, TempDir, C};
use gaply_core::stats_l1::codes::l1_ingest_refusal;

fn code_of(dir: &TempDir, name: &str, bytes: &[u8]) -> Result<(usize, usize), String> {
    let p = dir.write(name, bytes);
    match load_dataset(&p) {
        Ok(d) => Ok((d.rows(), d.schema.columns.len())),
        Err(r) => Err(l1_ingest_refusal(r).code.to_string()),
    }
}

fn utf16le(text: &str) -> Vec<u8> {
    let mut out = vec![0xFF, 0xFE];
    for u in text.encode_utf16() {
        out.extend_from_slice(&u.to_le_bytes());
    }
    out
}

/* ---- F10 --------------------------------------------------------------- */

#[test]
fn a_latin1_or_utf16_text_file_is_refused_as_not_utf8() {
    let dir = TempDir::new("enc");
    let latin1 = b"score,arm\n46.86,caf\xe9\n49.05,caf\xe9\n51.20,th\xe9\n".to_vec();
    assert_eq!(
        code_of(&dir, "latin1.csv", &latin1),
        Err("encoding_not_utf8".to_string())
    );
    let tsv = b"score\tarm\n1.5\tna\xefve\n2.5\tb\n".to_vec();
    assert_eq!(
        code_of(&dir, "latin1.tsv", &tsv),
        Err("encoding_not_utf8".to_string())
    );
    let utf16 = utf16le("score,arm\n62.52,a\n63.10,b\n");
    assert_eq!(
        code_of(&dir, "utf16.csv", &utf16),
        Err("encoding_not_utf8".to_string())
    );
    let mut utf16be = vec![0xFE, 0xFF];
    for u in "score,arm\n1,a\n".encode_utf16() {
        utf16be.extend_from_slice(&u.to_be_bytes());
    }
    assert_eq!(
        code_of(&dir, "utf16be.csv", &utf16be),
        Err("encoding_not_utf8".to_string())
    );
}

#[test]
fn bytes_that_are_not_text_stay_malformed() {
    let dir = TempDir::new("enc-binary");
    // Invalid UTF-8 and control bytes: no encoding makes this text.
    let mut binary = Vec::new();
    let mut x: u32 = 2026;
    for _ in 0..4096 {
        x = x.wrapping_mul(1_103_515_245).wrapping_add(12_345);
        binary.push((x >> 16) as u8);
    }
    assert_eq!(
        code_of(&dir, "binary.csv", &binary),
        Err("malformed".to_string())
    );
}

#[test]
fn utf8_files_that_open_today_open_as_before() {
    let dir = TempDir::new("enc-ok");
    let accented = "score,arm\n46.86,café\n49.05,thé\n".as_bytes().to_vec();
    assert_eq!(code_of(&dir, "utf8.csv", &accented), Ok((2, 2)));
    let mut bom = vec![0xEF, 0xBB, 0xBF];
    bom.extend_from_slice(b"score,arm\n1.5,a\n2.5,b\n3.5,a\n");
    assert_eq!(code_of(&dir, "bom.csv", &bom), Ok((3, 2)));
    let nul = b"a,b\n1,x\x00y\n2,z\n".to_vec();
    assert_eq!(code_of(&dir, "nul.csv", &nul), Ok((2, 2)));
    let plain = delimited(
        &["score", "group"],
        &[vec![C::N("1.5"), C::T("a")], vec![C::N("2.5"), C::T("b")]],
        ",",
        "\r\n",
    );
    assert_eq!(code_of(&dir, "crlf.csv", &plain), Ok((2, 2)));
}

/* ---- F14 --------------------------------------------------------------- */

/// A minimal STORED zip of `parts`.
fn stored_zip(parts: &[(&str, Vec<u8>)]) -> Vec<u8> {
    fn crc32(bytes: &[u8]) -> u32 {
        let mut c = 0xFFFF_FFFFu32;
        for &b in bytes {
            c ^= u32::from(b);
            for _ in 0..8 {
                c = if c & 1 == 1 {
                    (c >> 1) ^ 0xEDB8_8320
                } else {
                    c >> 1
                };
            }
        }
        !c
    }
    let mut out = Vec::new();
    let mut central = Vec::new();
    for (name, data) in parts {
        let (crc, len, n, offset) = (
            crc32(data),
            data.len() as u32,
            name.len() as u16,
            out.len() as u32,
        );
        out.extend_from_slice(&0x0403_4B50u32.to_le_bytes());
        for v in [20u16, 0, 0, 0, 0] {
            out.extend_from_slice(&v.to_le_bytes());
        }
        for v in [crc, len, len] {
            out.extend_from_slice(&v.to_le_bytes());
        }
        out.extend_from_slice(&n.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(name.as_bytes());
        out.extend_from_slice(data);
        central.extend_from_slice(&0x0201_4B50u32.to_le_bytes());
        for v in [20u16, 20, 0, 0, 0, 0] {
            central.extend_from_slice(&v.to_le_bytes());
        }
        for v in [crc, len, len] {
            central.extend_from_slice(&v.to_le_bytes());
        }
        central.extend_from_slice(&n.to_le_bytes());
        for v in [0u16, 0, 0, 0] {
            central.extend_from_slice(&v.to_le_bytes());
        }
        central.extend_from_slice(&0u32.to_le_bytes());
        central.extend_from_slice(&offset.to_le_bytes());
        central.extend_from_slice(name.as_bytes());
    }
    let (cd_offset, cd_size) = (out.len() as u32, central.len() as u32);
    out.extend_from_slice(&central);
    out.extend_from_slice(&0x0605_4B50u32.to_le_bytes());
    for v in [0u16, 0, parts.len() as u16, parts.len() as u16] {
        out.extend_from_slice(&v.to_le_bytes());
    }
    out.extend_from_slice(&cd_size.to_le_bytes());
    out.extend_from_slice(&cd_offset.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out
}

/// A one-sheet workbook whose sheet data is `rows_xml`.
fn workbook(rows_xml: &str) -> Vec<u8> {
    let sheet = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><sheetData>{rows_xml}</sheetData></worksheet>"#
    );
    stored_zip(&[
        (
            "[Content_Types].xml",
            br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/><Override PartName="/xl/worksheets/sheet1.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/></Types>"#.to_vec(),
        ),
        (
            "_rels/.rels",
            br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="xl/workbook.xml"/></Relationships>"#.to_vec(),
        ),
        (
            "xl/workbook.xml",
            br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><sheets><sheet name="Sheet1" sheetId="1" r:id="rId1"/></sheets></workbook>"#.to_vec(),
        ),
        (
            "xl/_rels/workbook.xml.rels",
            br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet1.xml"/></Relationships>"#.to_vec(),
        ),
        ("xl/worksheets/sheet1.xml", sheet.into_bytes()),
    ])
}

#[test]
fn a_workbook_that_nests_elements_inside_a_cell_is_refused_as_malformed() {
    let dir = TempDir::new("deep");
    for depth in [200, 5_000] {
        let nested = format!("{}deep{}", "<x>".repeat(depth), "</x>".repeat(depth));
        let rows =
            format!(r#"<row r="1"><c r="A1" t="inlineStr"><is><t>{nested}</t></is></c></row>"#);
        assert_eq!(
            code_of(&dir, &format!("deep_{depth}.xlsx"), &workbook(&rows)),
            Err("malformed".to_string()),
            "{depth} nested elements in one cell"
        );
    }
}

#[test]
fn ordinary_and_rich_text_workbooks_open_as_before() {
    let dir = TempDir::new("deep-ok");
    let plain = xlsx(
        &["score", "group"],
        &[vec![C::N("1.5"), C::T("a")], vec![C::N("2.5"), C::T("b")]],
    );
    assert_eq!(code_of(&dir, "plain.xlsx", &plain), Ok((2, 2)));
    // Rich text: runs with properties, the deepest a real cell goes (8).
    let rich = concat!(
        r#"<row r="1"><c r="A1" t="inlineStr"><is><r><rPr><b/><sz val="11"/></rPr><t>score</t></r></is></c>"#,
        r#"<c r="B1" t="inlineStr"><is><t>group</t></is></c></row>"#,
        r#"<row r="2"><c r="A2"><v>1.5</v></c><c r="B2" t="inlineStr"><is><r><rPr><i/></rPr><t>a</t></r></is></c></row>"#,
        r#"<row r="3"><c r="A3"><v>2.5</v></c><c r="B3" t="inlineStr"><is><t>b</t></is></c></row>"#,
    );
    assert_eq!(code_of(&dir, "rich.xlsx", &workbook(rich)), Ok((2, 2)));
}
