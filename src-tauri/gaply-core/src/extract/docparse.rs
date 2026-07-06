//! Format adapters: turn uploaded manuscript files into plaintext. Pure,
//! offline parsing — `pdf-extract` and `zip`+`quick-xml` read local bytes
//! only; no network access.

use std::io::Read;
use std::path::Path;

use quick_xml::events::Event;
use quick_xml::Reader;

use crate::error::GaplyError;

/// Parse a file into plaintext, dispatching on its extension.
pub fn parse_path(path: &Path) -> Result<String, GaplyError> {
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
    pdf_extract::extract_text(path)
        .map_err(|e| GaplyError::Internal(format!("pdf parse failed: {e}")))
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
        assert!(matches!(
            parse_path(Path::new("/tmp/x.rtf")),
            Err(GaplyError::Validation(_))
        ));
    }
}
