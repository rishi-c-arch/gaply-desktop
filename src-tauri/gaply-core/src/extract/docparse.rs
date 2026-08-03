//! Format adapters: turn uploaded manuscript files into plaintext. Pure,
//! offline parsing — `pdf-extract` and `zip`+`quick-xml` read local bytes
//! only; no network access.

use std::io::Read;
use std::path::Path;

use quick_xml::events::Event;
use quick_xml::Reader;

use crate::error::GaplyError;
use crate::extract::sections;

/// The minimum number of non-whitespace characters a parse must yield before we
/// treat it as real text. Below this, a PDF is almost certainly scanned /
/// image-only (no text layer) rather than a genuinely tiny manuscript.
const MIN_MEANINGFUL_CHARS: usize = 20;

/// True when extracted text has enough non-whitespace content to be a document
/// rather than an empty/scanned artifact.
fn has_extractable_text(text: &str) -> bool {
    text.chars().filter(|c| !c.is_whitespace()).count() >= MIN_MEANINGFUL_CHARS
}

// ---------------------------------------------------------------------------
// PDF paragraph reflow
// ---------------------------------------------------------------------------
//
// ACCURACY CORRECTION. `pdf_extract::extract_text` emits a line break — and
// usually a BLANK line — at every *rendered* line, so a double-spaced manuscript
// arrives with a blank line inside every sentence. `sections::paragraphs_of`
// correctly treats a blank line as a paragraph break, so on such a PDF one
// `Section.paragraph` is one physical LINE, and `Location.paragraph` stops
// meaning what it says.
//
// That value is user-visible: it renders in the provenance inspector
// (`ReportViewerPage.tsx:186-187`) and is written into the EXPORTED PDF
// (`exportPdf.ts:32`), and the reference count derived from the same
// segmentation is reported to the author (`report.rs:790-817`). Measured on one
// real manuscript, before this reflow: 81 `Reference` rows for 20 references
// (page furniture parsed as references), 24 citations against 28, and four
// detected tables against three.
//
// Scope is deliberately narrow: rejoin wrapped lines and drop page furniture.
// Footnote splicing and multi-column ordering are DIFFERENT root causes with a
// destructive failure mode (they can delete real manuscript text, which joining
// cannot) and are deliberately left out — shipping them together would make a
// regression unattributable.

/// A line repeated at least this many times is a running header/footer.
const FURNITURE_MIN_REPEATS: usize = 3;

/// Repeated lines longer than this are treated as content, not furniture — a
/// running header is short. Guards against deleting a genuinely repeated
/// sentence.
const FURNITURE_MAX_CHARS: usize = 80;

/// Tokens ending in '.' that do NOT end a sentence.
const ABBREVIATIONS: &[&str] = &[
    "et al.", "e.g.", "i.e.", "cf.", "vs.", "approx.", "ca.", "etc.", "Fig.", "Figs.", "Tab.",
    "No.", "Dr.", "Prof.", "Mr.", "Mrs.", "Ms.", "St.", "Jr.", "Sr.", "Eq.", "Eqs.", "ref.",
    "refs.", "Ref.", "min.", "max.", "sec.", "wt.", "vol.", "conc.", "temp.", "spp.", "sp.",
    "subsp.", "var.", "p.", "pp.", "ed.", "eds.", "Inc.", "Ltd.", "Co.", "U.S.", "U.K.",
];

/// True when `tail` ends with `abbrev` **as a whole token** — the preceding
/// character must be a non-alphanumeric or the string start. Without this,
/// `"unaffected."` matches the `"ed."` abbreviation and a real sentence boundary
/// is suppressed.
fn ends_with_abbreviation(tail: &str) -> bool {
    ABBREVIATIONS.iter().any(|a| {
        tail.ends_with(a) && {
            let before = tail.len() - a.len();
            before == 0
                || !tail[..before].chars().next_back().map(|c| c.is_alphanumeric()).unwrap_or(false)
        }
    })
}

/// True when `s` looks like a completed sentence: ends in `.`/`!`/`?`, and the
/// terminator is not an abbreviation dot or a single-capital initial ("… J.").
///
/// A TRAILING URL is stripped before the test. A bibliography entry often ends
/// with a bare DOI — `"… Journal of Insect Science 12: Article 82, 1–14.
/// https://doi.org/10.1673/031.012.8201"` — which ends in a digit, so without
/// this the block never closes and the NEXT entry is absorbed. Measured on the
/// evaluation manuscript: eight entries merged this way, in one chain of four,
/// and two more were left carrying the previous entry's DOI where the author
/// belongs. Every entry WITHOUT a DOI parsed cleanly.
///
/// This is a different defect from the abbreviation guard below, not a variant of
/// it: that one suppresses a FALSE boundary ("unaffected." is not the "ed."
/// abbreviation), this one restores a MISSED one. They are opposite failure
/// directions of the same proxy — sentence completion standing in for block
/// boundary — and neither fix implies the other.
fn ends_sentence(s: &str) -> bool {
    let t = strip_trailing_url(s.trim_end());
    let t = t.trim_end();
    let last = match t.chars().next_back() {
        Some(c) => c,
        None => return false,
    };
    if last == '!' || last == '?' {
        return true;
    }
    if last != '.' {
        return false;
    }
    if ends_with_abbreviation(t) {
        return false;
    }
    // Single-capital initial: "… A." / "Bombyx mori L."
    let mut it = t.chars().rev();
    it.next(); // the '.'
    match (it.next(), it.next()) {
        (Some(c), Some(prev)) if c.is_uppercase() && !prev.is_alphanumeric() => false,
        (Some(c), None) if c.is_uppercase() => false,
        _ => true,
    }
}

/// Drop a URL that sits at the very end of `s`, so the text before it can be
/// tested for sentence completion. Returns `s` unchanged when it does not end
/// with one.
fn strip_trailing_url(s: &str) -> &str {
    const SCHEMES: &[&str] = &["https://", "http://", "www.", "doi:"];
    match s.rsplit_once(char::is_whitespace) {
        Some((head, last)) if SCHEMES.iter().any(|p| last.starts_with(p)) => head,
        _ => s,
    }
}

/// True when the whole line is a single URL.
fn is_url_only(s: &str) -> bool {
    const SCHEMES: &[&str] = &["https://", "http://", "www.", "doi:"];
    let t = s.trim();
    !t.contains(char::is_whitespace) && SCHEMES.iter().any(|p| t.starts_with(p))
}

/// Lines that are running headers/footers rather than manuscript content.
fn page_furniture(lines: &[&str]) -> std::collections::HashSet<String> {
    let mut counts: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();
    for l in lines {
        let t = l.trim();
        if !t.is_empty() {
            *counts.entry(t).or_insert(0) += 1;
        }
    }
    counts
        .into_iter()
        .filter(|(t, n)| {
            if t.len() > FURNITURE_MAX_CHARS {
                return false;
            }
            if *n >= FURNITURE_MIN_REPEATS {
                return true;
            }
            // "Page 3 of 13" appears once per page with a different number, so
            // repetition alone never catches it.
            let l = t.to_lowercase();
            l.starts_with("page ")
                && l.contains(" of ")
                && l.split_whitespace().count() == 4
                && l.split_whitespace().nth(1).map(|w| w.chars().all(|c| c.is_ascii_digit())).unwrap_or(false)
        })
        .map(|(t, _)| t.to_string())
        .collect()
}

/// Rejoin PDF text that `pdf-extract` broke at rendered-line boundaries, so the
/// section splitter sees real paragraphs.
///
/// Rules, in order:
/// 1. **Page furniture is dropped** and does not close a block — a running
///    header landing inside a sentence must not split it.
/// 2. **A recognised heading is its own block.** Delegated to
///    [`sections::detect_heading`] so the heading vocabulary has exactly one
///    definition; joining a heading into the following text would destroy
///    section splitting.
/// 3. **A blank line closes the block only if the text so far ends a sentence.**
///    This is the whole correction: in a well-formed PDF a blank line follows a
///    completed sentence and the real paragraph break is preserved; in a
///    line-broken PDF the blank line falls mid-sentence and is a wrap artifact,
///    so it is ignored.
/// 4. Otherwise lines accumulate, joined by a single space.
///
/// Blocks are emitted `\n\n`-separated, which is the contract
/// `sections::paragraphs_of` already expects.
pub(crate) fn reflow_pdf_text(text: &str) -> String {
    let lines: Vec<&str> = text.lines().collect();
    let furniture = page_furniture(&lines);

    let mut blocks: Vec<String> = Vec::new();
    let mut cur = String::new();
    let flush = |cur: &mut String, blocks: &mut Vec<String>| {
        let t = cur.trim();
        if !t.is_empty() {
            blocks.push(t.to_string());
        }
        cur.clear();
    };

    for line in &lines {
        let t = line.trim();

        if t.is_empty() {
            if ends_sentence(&cur) {
                flush(&mut cur, &mut blocks);
            }
            continue;
        }
        if furniture.contains(t) {
            continue;
        }
        // A line that is NOTHING but a URL is trailing metadata of the block that
        // just closed, not the start of a new one. Same trailing-DOI mechanism as
        // `strip_trailing_url`, different manifestation: when the entry's last
        // prose line already ended a sentence, the block closes and the bare DOI
        // line would otherwise become the HEAD of the next entry — putting a URL
        // where the author belongs. Measured: two entries lost that way.
        if cur.is_empty() && is_url_only(t) {
            if let Some(last) = blocks.last_mut() {
                last.push(' ');
                last.push_str(t);
            }
            continue;
        }
        if sections::detect_heading(line).is_some() {
            flush(&mut cur, &mut blocks);
            blocks.push(t.to_string());
            continue;
        }
        if !cur.is_empty() {
            cur.push(' ');
        }
        cur.push_str(t);
    }
    flush(&mut cur, &mut blocks);

    blocks.join("\n\n")
}

/// Run a `pdf-extract` call behind a panic boundary.
///
/// `pdf-extract` is a third-party parser that can *panic* (not merely return
/// `Err`) on malformed / truncated / hostile PDFs. The `.map_err()` at the call
/// sites only handles the `Err` variant — a panic unwinds straight past it. The
/// callers that funnel through here (`stats_validity` / `stats_prefill_p` and
/// the Gap Finder corpus builder) run inside *synchronous* Tauri commands on the
/// main thread, so an unguarded panic crashes the whole app instead of the
/// intended honest "couldn't parse" degradation. (The main analysis pipeline is
/// already protected by `spawn_blocking`'s `JoinError`; this covers the sync
/// callers too.)
///
/// Catching the unwind here — at the single shared choke point — turns a
/// panicking PDF into the same honest `Validation` error the `Err` path returns,
/// so every caller is protected without any per-caller change. `AssertUnwindSafe`
/// is sound: the closure only *reads* its captured path/bytes and returns a
/// fresh value, so a caught panic leaves no observable state behind the boundary.
fn catch_pdf_panic<T>(f: impl FnOnce() -> T) -> Result<T, GaplyError> {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(f)).map_err(|_| {
        GaplyError::Validation(
            "This PDF could not be parsed — it looks malformed or corrupted. \
             Try re-exporting it from your editor (or use a different file)."
                .to_string(),
        )
    })
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
    // `?` on the outer guard: a *panic* inside pdf-extract becomes an honest
    // Validation error; the inner `.map_err(...)?` keeps the existing handling
    // of a normal parse Err unchanged.
    let text = catch_pdf_panic(|| pdf_extract::extract_text(path))?
        .map_err(|e| GaplyError::Internal(format!("pdf parse failed: {e}")))?;
    let text = reflow_pdf_text(&text);
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

/// Extract text from in-memory PDF bytes — the bytes-API sibling of
/// [`parse_docx`], for callers that fetched a PDF over the network (Gap
/// Finder paper links) rather than from disk. Same scanned/image-only check
/// as the path-based parser.
pub fn parse_pdf_bytes(bytes: &[u8]) -> Result<String, GaplyError> {
    // Same panic boundary as the path-based parser (see `catch_pdf_panic`).
    let text = catch_pdf_panic(|| pdf_extract::extract_text_from_mem(bytes))?
        .map_err(|e| GaplyError::Internal(format!("pdf parse failed: {e}")))?;
    let text = reflow_pdf_text(&text);
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

    // ---------------------------------------------------------------
    // PDF paragraph reflow (accuracy correction)
    // ---------------------------------------------------------------

    #[test]
    fn blank_line_mid_sentence_is_a_wrap_artifact_and_is_joined() {
        // The exact shape pdf-extract produces for a double-spaced manuscript.
        let raw = "All three analogues raised total\n\nfree amino acids and total protein\n\nabove both controls.";
        assert_eq!(
            reflow_pdf_text(raw),
            "All three analogues raised total free amino acids and total protein above both controls."
        );
    }

    #[test]
    fn blank_line_after_a_completed_sentence_closes_the_paragraph() {
        let raw = "First paragraph ends here.\n\nSecond paragraph starts here.";
        assert_eq!(
            reflow_pdf_text(raw),
            "First paragraph ends here.\n\nSecond paragraph starts here."
        );
    }

    #[test]
    fn consecutive_sentences_without_a_blank_line_stay_one_paragraph() {
        // Closing on every terminator would make each SENTENCE a paragraph —
        // a different wrong answer than one paragraph per line.
        let raw = "One sentence here.\nA second sentence follows.\nAnd a third.";
        assert_eq!(
            reflow_pdf_text(raw),
            "One sentence here. A second sentence follows. And a third."
        );
    }

    #[test]
    fn abbreviation_does_not_close_a_block() {
        let raw = "Reported by Bizhannia et al.\n\n(2005) in an earlier study.";
        assert_eq!(
            reflow_pdf_text(raw),
            "Reported by Bizhannia et al. (2005) in an earlier study."
        );
    }

    #[test]
    fn abbreviation_match_is_word_boundary_aware() {
        // REGRESSION: "unaffected." must not match the "ed." abbreviation. Without
        // the word-boundary check this joins into one block.
        assert!(ends_sentence("the transaminases were unaffected."));
        let raw = "The transaminases were unaffected.\n\nThe carbohydrate fractions followed.";
        assert_eq!(
            reflow_pdf_text(raw),
            "The transaminases were unaffected.\n\nThe carbohydrate fractions followed."
        );
    }

    #[test]
    fn single_capital_initial_does_not_close_a_block() {
        let raw = "grown on Bombyx mori L.\n\nunder controlled conditions.";
        assert_eq!(
            reflow_pdf_text(raw),
            "grown on Bombyx mori L. under controlled conditions."
        );
    }

    #[test]
    fn repeated_running_header_is_dropped() {
        let raw = "JOURNAL BANNER\n\nBody line one.\n\nJOURNAL BANNER\n\nBody line two.\n\nJOURNAL BANNER\n\nBody line three.";
        let out = reflow_pdf_text(raw);
        assert!(!out.contains("JOURNAL BANNER"), "banner survived: {out}");
        assert!(out.contains("Body line one."));
        assert!(out.contains("Body line three."));
    }

    #[test]
    fn page_number_footer_is_dropped_even_though_each_is_unique() {
        let raw = "Body text here.\n\nPage 1 of 13\n\nMore body text.\n\nPage 2 of 13\n\nFinal body text.";
        let out = reflow_pdf_text(raw);
        assert!(!out.contains("Page 1 of 13"), "footer survived: {out}");
        assert!(!out.contains("Page 2 of 13"), "footer survived: {out}");
    }

    #[test]
    fn furniture_landing_mid_sentence_does_not_split_it() {
        // A running header between two halves of one sentence must be removed
        // WITHOUT closing the block.
        let raw = "the shortfall is traced largely\n\nPage 2 of 13\n\nto poor larval growth.";
        assert_eq!(
            reflow_pdf_text(raw),
            "the shortfall is traced largely to poor larval growth."
        );
    }

    #[test]
    fn a_long_repeated_line_is_treated_as_content_not_furniture() {
        let long = "This sentence is deliberately longer than the furniture character cap so that it is never mistaken for a running header.";
        assert!(long.len() > FURNITURE_MAX_CHARS);
        let raw = format!("{long}\n\n{long}\n\n{long}");
        assert!(reflow_pdf_text(&raw).contains(long));
    }

    #[test]
    fn recognised_heading_stays_its_own_block() {
        // Joining a heading into the following text would destroy section splitting.
        let raw = "ABSTRACT\n\nA field investigation was carried out during\n\nthe spring season.";
        assert_eq!(
            reflow_pdf_text(raw),
            "ABSTRACT\n\nA field investigation was carried out during the spring season."
        );
    }

    #[test]
    fn reflow_restores_paragraph_semantics_end_to_end() {
        // The defect, stated as a test: before the reflow this text yields three
        // one-line "paragraphs"; after it, one real paragraph.
        let raw = "ABSTRACT\n\nThe response was dose dependent but not\n\nmonotonic: values rose from 0.01 to 0.1 and fell\n\nagain at 1.0 in both seasons.";
        let (_, before) = sections::split_document(raw);
        let abstract_before = before.iter().find(|s| s.kind == crate::extract::SectionKind::Abstract).unwrap();
        assert_eq!(abstract_before.paragraphs.len(), 3, "precondition: the defect");

        let (_, after) = sections::split_document(&reflow_pdf_text(raw));
        let abstract_after = after.iter().find(|s| s.kind == crate::extract::SectionKind::Abstract).unwrap();
        assert_eq!(abstract_after.paragraphs.len(), 1);
        assert!(abstract_after.paragraphs[0].contains("dose dependent but not monotonic"));
    }

    #[test]
    fn a_trailing_doi_url_does_not_prevent_a_block_from_closing() {
        // A bibliography entry ending in a bare DOI ends in a DIGIT, so without
        // stripping the URL the blank line never closes and the NEXT entry is
        // absorbed. Measured: eight entries merged this way, one chain of four.
        let raw = "Rahmathulla V K and Suresh H M. 2012. Seasonal variation. Journal of Insect Science 12: 1-14. https://doi.org/10.1673/031.012.8201\n\nReitman S and Frankel S. 1957. A colorimetric method.";
        let out = reflow_pdf_text(raw);
        assert_eq!(out.split("\n\n").count(), 2, "entries must not merge: {out}");
        assert!(out.split("\n\n").nth(1).unwrap().starts_with("Reitman"));
    }

    #[test]
    fn a_url_only_line_attaches_to_the_previous_block() {
        // Same mechanism, other manifestation: the entry's prose already ended a
        // sentence, so the block closed and the bare DOI line would otherwise
        // become the HEAD of the next entry — a URL where the author belongs.
        let raw = "Liu S and Wang H. 2023. Juvenile hormone regulates silk gene expression. Cellular Life Sciences 80: Article 331.\n\nhttps://doi.org/10.1007/s00018-023-04996-1\n\nMamatha D M and Rao M R. 2006. Studies on cocoons.";
        let out = reflow_pdf_text(raw);
        let blocks: Vec<&str> = out.split("\n\n").collect();
        assert_eq!(blocks.len(), 2, "url-only line must not start a block: {out}");
        assert!(blocks[0].ends_with("s00018-023-04996-1"), "url joins the entry it belongs to");
        assert!(blocks[1].starts_with("Mamatha"), "next entry keeps its author: {}", blocks[1]);
    }

    #[test]
    fn a_sentence_ending_before_a_trailing_url_still_closes_normally() {
        assert!(ends_sentence("An entry ends here. https://doi.org/10.1/x"));
        assert!(!ends_sentence("An entry continues https://doi.org/10.1/x"));
    }

    #[test]
    fn well_formed_text_is_not_damaged() {
        // A DOCX-shaped input (already correct) must pass through unchanged.
        let raw = "Introduction text, first paragraph. It has two sentences.\n\nSecond paragraph here.";
        assert_eq!(reflow_pdf_text(raw), raw);
    }

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

    // ---- C1: pdf-extract panic guard -------------------------------------

    #[test]
    fn catch_pdf_panic_turns_a_panic_into_an_honest_validation_error() {
        // Deterministic proof of the boundary itself: a closure that PANICS is
        // caught and mapped to Validation — the unwind does NOT propagate past
        // catch_pdf_panic (if it did, this test process would abort).
        let prev = std::panic::take_hook();
        std::panic::set_hook(Box::new(|_| {})); // silence the expected panic on stderr
        let result = catch_pdf_panic(|| -> String { panic!("simulated pdf-extract panic") });
        std::panic::set_hook(prev);
        match result {
            Err(GaplyError::Validation(m)) => {
                assert!(m.contains("could not be parsed"), "honest message expected, got: {m}");
            }
            other => panic!("expected Validation from a caught panic, got {other:?}"),
        }
    }

    #[test]
    fn catch_pdf_panic_passes_through_a_non_panicking_value() {
        // The guard is transparent when nothing panics — no behavior change for
        // the normal (valid) path.
        let ok = catch_pdf_panic(|| 42_usize).expect("no panic → Ok");
        assert_eq!(ok, 42);
    }

    #[test]
    fn malformed_pdf_bytes_degrade_honestly_without_crashing() {
        // A PDF-looking but broken byte sequence. Whether pdf-extract panics
        // (→ Validation, via the guard) or returns Err (→ Internal), the caller
        // gets an honest GaplyError and the process does NOT crash. Before the
        // guard, a panic here would unwind through the sync command onto the
        // main thread.
        let junk: &[u8] = b"%PDF-1.5\n1 0 obj<</Type/Catalog>>endobj\nxref\nbroken trailer \x00\x01\x02\xff";
        match parse_pdf_bytes(junk) {
            Err(GaplyError::Validation(_)) | Err(GaplyError::Internal(_)) => {}
            other => panic!("expected an honest parse error, got {other:?}"),
        }
    }

    #[test]
    fn valid_pdf_bytes_still_parse_through_the_guard() {
        // Regression: the panic boundary must not change the happy path. Feed the
        // real fixture through the bytes API (the guarded parse_pdf_bytes) and
        // confirm content still extracts exactly as before.
        let fixture = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/sample_text.pdf");
        let bytes = std::fs::read(fixture).expect("fixture readable");
        let text = parse_pdf_bytes(&bytes).expect("valid PDF bytes should parse through the guard");
        assert!(text.to_lowercase().contains("methods"), "got: {text:?}");
        assert!(text.to_lowercase().contains("participants"), "got: {text:?}");
    }
}
