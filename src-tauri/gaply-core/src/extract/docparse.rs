//! Format adapters: turn uploaded manuscript files into plaintext. Pure,
//! offline parsing — `pdf-extract` and `zip`+`quick-xml` read local bytes
//! only; no network access.

use std::io::Read;
use std::path::Path;

use quick_xml::events::Event;
use quick_xml::Reader;

use crate::error::GaplyError;

/// The minimum number of non-whitespace characters a parse must yield before we
/// treat it as real text. Below this, a PDF is almost certainly scanned /
/// image-only (no text layer) rather than a genuinely tiny manuscript.
const MIN_MEANINGFUL_CHARS: usize = 20;

/// True when extracted text has enough non-whitespace content to be a document
/// rather than an empty/scanned artifact.
fn has_extractable_text(text: &str) -> bool {
    text.chars().filter(|c| !c.is_whitespace()).count() >= MIN_MEANINGFUL_CHARS
}

/// Parse a file into plaintext, dispatching on its extension.
pub fn parse_path(path: &Path) -> Result<String, GaplyError> {
    // A missing file is the single most common real-world failure (e.g. a UI
    // that passed a bare filename instead of an absolute path). Name it clearly
    // instead of letting a downstream parser emit a cryptic error.
    if !path.exists() {
        return Err(GaplyError::Validation(format!(
            "file not found: {} — the desktop app must pass an absolute file path.",
            path.display()
        )));
    }
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase())
        .unwrap_or_default();
    match ext.as_str() {
        "pdf" => parse_pdf(path),
        "docx" => {
            let bytes = std::fs::read(path)?;
            parse_docx(&bytes)
        }
        "txt" | "md" | "text" => Ok(std::fs::read_to_string(path)?),
        other => Err(GaplyError::Validation(format!(
            "unsupported manuscript format: .{other} (expected pdf, docx, txt)"
        ))),
    }
}

fn parse_pdf(path: &Path) -> Result<String, GaplyError> {
    let text = pdf_extract::extract_text(path)
        .map_err(|e| GaplyError::Internal(format!("pdf parse failed: {e}")))?;
    // A PDF that parses but yields (almost) no text is scanned / image-only.
    // Say so specifically rather than proceeding with empty content.
    if !has_extractable_text(&text) {
        return Err(GaplyError::Validation(
            "This PDF has no extractable text — it looks scanned or image-only. \
             Extraction needs a text-based PDF (export from your editor, or run OCR first)."
                .to_string(),
        ));
    }
    Ok(text)
}

/// Extract text from a DOCX (a zip of XML). Paragraphs (`<w:p>`) become
/// blank-line-separated blocks; `<w:tab>`/`<w:br>` become whitespace.
pub fn parse_docx(bytes: &[u8]) -> Result<String, GaplyError> {
    let cursor = std::io::Cursor::new(bytes);
    let mut zip = zip::ZipArchive::new(cursor)
        .map_err(|e| GaplyError::Validation(format!("not a valid docx (zip): {e}")))?;
    let mut xml = String::new();
    zip.by_name("word/document.xml")
        .map_err(|e| GaplyError::Validation(format!("docx missing document.xml: {e}")))?
        .read_to_string(&mut xml)?;

    let mut reader = Reader::from_str(&xml);
    // Lenient: we only harvest text, so don't fail the whole document on a
    // mismatched/idiosyncratic end tag (real-world DOCX is not always strict).
    let cfg = reader.config_mut();
    cfg.trim_text(false);
    cfg.check_end_names = false;
    let mut out = String::new();
    let mut in_text = false;

    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) if e.local_name().as_ref() == b"t" => in_text = true,
            Ok(Event::End(e)) if e.local_name().as_ref() == b"t" => in_text = false,
            Ok(Event::Text(t)) if in_text => {
                out.push_str(&t.unescape().unwrap_or_default());
            }
            Ok(Event::Empty(e)) | Ok(Event::Start(e)) => match e.local_name().as_ref() {
                b"tab" => out.push('\t'),
                b"br" | b"cr" => out.push('\n'),
                _ => {}
            },
            // paragraph end → blank line so the section splitter sees breaks
            Ok(Event::End(e)) if e.local_name().as_ref() == b"p" => out.push_str("\n\n"),
            Ok(Event::Eof) => break,
            Err(e) => return Err(GaplyError::Internal(format!("docx xml error: {e}"))),
            _ => {}
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    /// Build a minimal valid .docx in memory with the given paragraphs.
    fn make_docx(paragraphs: &[&str]) -> Vec<u8> {
        let mut body = String::new();
        for p in paragraphs {
            body.push_str(&format!(
                "<w:p><w:r><w:t xml:space=\"preserve\">{p}</w:t></w:r></w:p>"
            ));
        }
        let doc = format!(
            "<?xml version=\"1.0\"?><w:document xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"><w:body>{body}</w:body></w:document>"
        );
        let mut buf = Vec::new();
        {
            let mut zip = zip::ZipWriter::new(std::io::Cursor::new(&mut buf));
            let opts: zip::write::FileOptions<()> =
                zip::write::FileOptions::default().compression_method(zip::CompressionMethod::Stored);
            zip.start_file("word/document.xml", opts).unwrap();
            zip.write_all(doc.as_bytes()).unwrap();
            zip.finish().unwrap();
        }
        buf
    }

    #[test]
    fn docx_paragraphs_become_blank_separated_text() {
        let bytes = make_docx(&["Introduction", "We used a paired t-test (p < 0.01)."]);
        let text = parse_docx(&bytes).unwrap();
        assert!(text.contains("Introduction"));
        assert!(text.contains("paired t-test"));
        // blank line between paragraphs
        assert!(text.contains("Introduction\n\n"));
    }

    #[test]
    fn non_docx_bytes_are_rejected() {
        assert!(matches!(parse_docx(b"not a zip"), Err(GaplyError::Validation(_))));
    }

    #[test]
    fn unsupported_extension_is_validation_error() {
        // Must EXIST so we reach the extension branch (not the not-found guard).
        let p = std::env::temp_dir().join(format!("gaply_docparse_{}.rtf", std::process::id()));
        std::fs::write(&p, b"{\\rtf1}").unwrap();
        let res = parse_path(&p);
        let _ = std::fs::remove_file(&p);
        match res {
            Err(GaplyError::Validation(m)) => assert!(m.contains("unsupported"), "got: {m}"),
            other => panic!("expected unsupported-format validation error, got {other:?}"),
        }
    }

    #[test]
    fn missing_file_is_named_clearly() {
        // The Stage-2 real-world bug: a bare filename (no directory) reached the
        // core. Now it fails with a specific, honest message — not a cryptic
        // downstream parser error.
        let res = parse_path(Path::new("Gaply Remediation Plan.pdf"));
        match res {
            Err(GaplyError::Validation(m)) => {
                assert!(m.contains("file not found"), "got: {m}");
                assert!(m.contains("absolute file path"), "should hint the real cause: {m}");
            }
            other => panic!("expected file-not-found validation error, got {other:?}"),
        }
    }

    #[test]
    fn txt_file_is_read() {
        let p = std::env::temp_dir().join(format!("gaply_docparse_{}.txt", std::process::id()));
        std::fs::write(&p, "Methods\nWe ran a paired t-test.").unwrap();
        let text = parse_path(&p).unwrap();
        let _ = std::fs::remove_file(&p);
        assert!(text.contains("paired t-test"));
    }

    #[test]
    fn scanned_or_empty_text_is_detected() {
        // The image-only / scanned case: parses but yields ~no text.
        assert!(!has_extractable_text(""));
        assert!(!has_extractable_text("   \n\t  \n "));
        assert!(!has_extractable_text("a b c")); // below the meaningful threshold
        assert!(has_extractable_text("Methods: we recruited 48 participants and ran a t-test."));
    }

    #[test]
    fn real_text_pdf_extracts_content() {
        // A genuine text-layer PDF fixture (generated from plain text). Proves
        // pdf-extract works end-to-end and guards against a pdf-extract
        // regression — the Stage-2 failure was NOT pdf-extract, it was the path.
        let fixture = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/sample_text.pdf");
        let text = parse_path(Path::new(fixture)).expect("real text PDF should parse");
        assert!(text.to_lowercase().contains("methods"), "got: {text:?}");
        assert!(text.to_lowercase().contains("participants"), "got: {text:?}");
    }
}
