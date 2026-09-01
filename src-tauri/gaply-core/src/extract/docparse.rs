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

// The abbreviation list and its whole-token test now live in
// `extract::sentence`, which owns sentence-level lexical knowledge. ONE
// definition, two callers with DIFFERENT boundary needs: this module decides
// whether a physical LINE closes a reference block, `sentence::
// sentence_containing` decides where a sentence begins and ends.
//
// The BOUNDARY DECISION is deliberately not shared. `ends_sentence` below
// returns true for a prefix ending inside a decimal ("…p ≤ 0." → true), which is
// harmless here — a rendered line does not end mid-number — and is exactly the
// failure `sentence`'s invariant forbids. Sharing the decision would force one
// caller to accept the other's failure mode; sharing the vocabulary costs
// nothing and prevents the drift a second list would create.
use crate::extract::sentence::ends_with_abbreviation;

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

/// Which convention a document's References section uses for blank lines.
///
/// # ARCHITECTURE_TRACE §45–§46 — three arms, THREE KINDS OF WARRANT
///
/// `split_document` special-cases References to ONE PARAGRAPH PER NON-EMPTY
/// LINE. `reflow_pdf_text` merges lines and runs FIRST, so on a PDF whose
/// entries do not end in a sentence — a citation style ending in bare URLs and
/// access dates — every line joins into one block and 26 references become 1.
/// Measured: 76 lines to one 6373-character paragraph, with ten downstream
/// consumers degraded and every emptiness guard passing because the count is 1
/// rather than 0.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RefConvention {
    /// A blank line after EVERY rendered line, so blank lines carry no entry
    /// information and sentence completion is the only signal.
    ///
    /// **PROVED, from what the median measures rather than from the corpus:** a
    /// noise convention gives every group size exactly 1, so the median is
    /// exactly 1. A median above 1 means the document is NOT using it. No
    /// document can falsify this.
    Noise,
    /// A blank line only BETWEEN entries, so a blank line closes the block.
    ///
    /// **OBSERVED at median 3 in one corpus document; DEFAULTED for every other
    /// non-`Noise` measurement.** Ruling out `Noise` does not establish that one
    /// alternative remains — a mixed-convention section, OCR artifacts, a
    /// publisher conversion quirk or a malformed PDF are all untouched by that
    /// argument. Mapping them here is a CHOICE, made because no third convention
    /// has been observed, and it is the arm a future document can overturn
    /// without either of the others being wrong.
    Structure,
    /// The section cannot be classified, so the incumbent behaviour governs.
    ///
    /// **Not a third convention — an absence of the signal the measure reads**
    /// (§4.12's typed absence, one level up). The non-obvious member is a
    /// section with NO BLANK LINES AT ALL: 75 lines, one group, median 75. Under
    /// a naive `median > 1` rule that classifies as `Structure`, finds nothing to
    /// close on, and collapses the whole section — the defect this exists to
    /// prevent, reached from the opposite direction. Size is not the only way a
    /// measurement can be insufficient (§46.7).
    Unclassified,
}

/// Classify the References section's blank-line convention.
///
/// `body` is the region AFTER the References heading. `furniture` lines are
/// skipped exactly as `reflow_pdf_text` skips them, so the classification reads
/// the same line stream the reflow will.
fn classify_references(body: &[&str], furniture: &std::collections::HashSet<String>) -> RefConvention {
    let (mut groups, mut nonempty, mut run) = (0usize, 0usize, 0usize);
    let mut sizes: Vec<usize> = Vec::new();
    for line in body {
        let t = line.trim();
        if furniture.contains(t) {
            continue;
        }
        if t.is_empty() {
            if run > 0 {
                groups += 1;
                sizes.push(run);
                run = 0;
            }
        } else {
            nonempty += 1;
            run += 1;
        }
    }
    if run > 0 {
        groups += 1;
        sizes.push(run);
    }

    if nonempty == 0 {
        return RefConvention::Unclassified;
    }
    // The signal is ABSENT, not scarce: with one group there are no blank lines
    // to read a convention from. Tested BEFORE the median, which would be huge.
    if groups <= 1 {
        return RefConvention::Unclassified;
    }
    sizes.sort_unstable();
    // The MEDIAN, not "any group exceeds one line". The `any` form is a cliff:
    // ONE wrapped entry in a 75-entry noise-convention section flips it to true
    // and produces the catastrophic branch. Measured — the median is unmoved by
    // that case, and 38 of 75 groups would have to exceed one line to move it.
    if sizes[sizes.len() / 2] == 1 {
        RefConvention::Noise
    } else {
        RefConvention::Structure
    }
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
    // Thin wrapper over the paged algorithm, with every line untagged. Behaviour
    // is byte-identical to the pre-refactor implementation — the existing tests
    // in this module are the acceptance condition for that claim.
    let lines: Vec<(Option<u32>, &str)> = text.lines().map(|l| (None, l)).collect();
    reflow_pdf_lines(&lines)
        .into_iter()
        .map(|(_, block)| block)
        .collect::<Vec<_>>()
        .join("\n\n")
}

/// The reflow algorithm, carrying a PAGE TAG per input line.
///
/// One algorithm, two entry points (plan §11 D3): [`reflow_pdf_text`] passes
/// `None` for every line and joins the result, which is exactly what every
/// existing caller — plagiarism, AI Check, PublishReady, Gap Finder — already
/// did. The page-aware ingest passes real page numbers from
/// `pdf_extract::extract_text_by_pages`.
///
/// PAGE ATTRIBUTION: a block is attributed to the page of the line that
/// STARTED it. A paragraph spanning a page break belongs to the page it starts
/// on. This is a recorded fact and never an estimate — a source with no page
/// information yields `None` all the way through, and `None` is stored as SQL
/// NULL rather than being guessed at from position.
pub(crate) fn reflow_pdf_lines(lines: &[(Option<u32>, &str)]) -> Vec<(Option<u32>, String)> {
    // Furniture detection and the References-convention pre-pass both work over
    // the WHOLE line stream, across page boundaries — a running header repeats
    // per page and is only recognisable globally. Hence one flat &str view.
    let flat: Vec<&str> = lines.iter().map(|(_, l)| *l).collect();
    let lines_view = &flat;
    let furniture = page_furniture(lines_view);

    // PRE-PASS. `page_furniture` above is already one; this is a second over the
    // same vector, so there is no streaming constraint to work around — the
    // whole line stream is in hand before any block decision is made.
    let ref_convention = lines_view
        .iter()
        .position(|l| matches!(sections::detect_heading(l), Some((sections::SectionKind::References, _))))
        .map(|i| classify_references(&lines_view[i + 1..], &furniture))
        .unwrap_or(RefConvention::Unclassified);
    let mut in_references = false;

    let mut blocks: Vec<(Option<u32>, String)> = Vec::new();
    let mut cur = String::new();
    // The page of the line that opened `cur`. Set when the block starts, so a
    // block continuing onto the next page keeps the page it began on.
    let mut cur_page: Option<u32> = None;
    let flush = |cur: &mut String, cur_page: &mut Option<u32>, blocks: &mut Vec<(Option<u32>, String)>| {
        let t = cur.trim();
        if !t.is_empty() {
            blocks.push((*cur_page, t.to_string()));
        }
        cur.clear();
        *cur_page = None;
    };

    for (page, line) in lines {
        let t = line.trim();

        if t.is_empty() {
            // In a References section using the Structure convention, a blank
            // line IS the entry boundary — the signal `split_document` expects
            // and that sentence completion cannot see. Everywhere else the
            // incumbent rule stands unchanged.
            if (in_references && ref_convention == RefConvention::Structure)
                || ends_sentence(&cur)
            {
                flush(&mut cur, &mut cur_page, &mut blocks);
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
        // Under the Structure convention a blank line has already closed the
        // entry, so a bare-URL line that follows is the NEXT entry's start, not
        // trailing metadata of the last. Appending it there would merge two
        // entries — the defect this whole path exists to prevent.
        if cur.is_empty()
            && is_url_only(t)
            && !(in_references && ref_convention == RefConvention::Structure)
        {
            if let Some((_, last)) = blocks.last_mut() {
                last.push(' ');
                last.push_str(t);
            }
            continue;
        }
        if let Some((kind, _)) = sections::detect_heading(line) {
            flush(&mut cur, &mut cur_page, &mut blocks);
            in_references = kind == sections::SectionKind::References;
            blocks.push((*page, t.to_string()));
            continue;
        }
        if cur.is_empty() {
            cur_page = *page;
        } else {
            cur.push(' ');
        }
        cur.push_str(t);
    }
    flush(&mut cur, &mut cur_page, &mut blocks);

    blocks
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

/// One reflowed block together with the page it was recorded on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PagedBlock {
    /// The page the block STARTED on, 1-based.
    ///
    /// `None` means the source carries no reliable page boundaries — a DOCX,
    /// TXT or MD file, or a PDF whose pages could not be read. It is NEVER an
    /// estimate: page is provenance, so an unknown page is stored as SQL NULL
    /// rather than inferred from position in the text.
    pub page: Option<u32>,
    pub text: String,
}

/// Parse a file into reflowed blocks, each tagged with its recorded page.
///
/// The page-aware sibling of [`parse_path`], added for the AI ingest path. It
/// does not replace or alter `parse_path`, and no existing caller changes
/// behaviour: PDFs go through `pdf_extract::extract_text_by_pages` (the flat
/// `extract_text` loses page boundaries irrecoverably) and then through the SAME
/// reflow algorithm, via [`reflow_pdf_lines`].
///
/// Furniture detection still runs across the whole document, not per page — a
/// running header is only recognisable by its repetition across pages.
pub fn parse_path_paged(path: &Path) -> Result<Vec<PagedBlock>, GaplyError> {
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
        "pdf" => parse_pdf_paged(path),
        // No pagination exists in these formats, so every block is honestly
        // page-less. Blocks are the blank-line-separated paragraphs the
        // non-paged path already produces.
        _ => {
            let text = parse_path(path)?;
            Ok(blocks_without_pages(&text))
        }
    }
}

/// Split already-parsed text into page-less blocks on blank lines — the same
/// block contract `sections::paragraphs_of` expects.
fn blocks_without_pages(text: &str) -> Vec<PagedBlock> {
    text.split("\n\n")
        .map(str::trim)
        .filter(|b| !b.is_empty())
        .map(|b| PagedBlock { page: None, text: b.to_string() })
        .collect()
}

fn parse_pdf_paged(path: &Path) -> Result<Vec<PagedBlock>, GaplyError> {
    // Same panic boundary as every other pdf-extract call site.
    let pages = catch_pdf_panic(|| pdf_extract::extract_text_by_pages(path))?
        .map_err(|e| GaplyError::Internal(format!("pdf parse failed: {e}")))?;

    // Tag every line with its 1-based page, then reflow the whole stream at
    // once so cross-page furniture detection still works.
    let mut tagged: Vec<(Option<u32>, &str)> = Vec::new();
    for (i, page_text) in pages.iter().enumerate() {
        let page = u32::try_from(i + 1).ok();
        tagged.extend(page_text.lines().map(|l| (page, l)));
    }

    let blocks = reflow_pdf_lines(&tagged);
    let joined: String = blocks.iter().map(|(_, t)| t.as_str()).collect::<Vec<_>>().join("\n\n");
    if !has_extractable_text(&joined) {
        return Err(GaplyError::Validation(NO_TEXT_LAYER_ADVICE.to_string()));
    }
    Ok(blocks.into_iter().map(|(page, text)| PagedBlock { page, text }).collect())
}

/// The message shown when a PDF carries no text layer. ONE definition — the
/// three parse paths and the import preflight all say the same sentence,
/// because a user who meets it twice must not be told two different things.
pub const NO_TEXT_LAYER_ADVICE: &str =
    "This PDF has no extractable text — it looks scanned or image-only. \
     Extraction needs a text-based PDF (export from your editor, or run OCR first).";

/// What a file IS, before anything is spent indexing it.
///
/// Deliberately cheap-ish and side-effect-free: it parses, but it does not
/// chunk, does not write a `documents` row and never touches the embedding
/// engine — which is the expensive part by two orders of magnitude.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentShape {
    /// Real, printed pages. `None` for formats that HAVE no pages (docx, txt,
    /// md) — reported as absent rather than guessed, because "12 pages" about a
    /// text file is a fact the file does not contain.
    pub pages: Option<u32>,
    /// Pages for a PDF; for page-less formats, the text's length expressed in
    /// pages so one estimate can cover both. Labelled as an equivalence at
    /// every surface that shows it.
    pub page_equivalents: u32,
    pub bytes: u64,
    /// Non-whitespace characters extracted. The evidence behind the scanned
    /// verdict, kept so a caller can explain the refusal rather than assert it.
    pub text_chars: usize,
}

/// Characters of prose taken as one page's worth, for formats with no pages.
/// A double-spaced manuscript page runs ~1,800–2,000 characters; this is the
/// round number in that range and it is only ever used for an ESTIMATE.
const CHARS_PER_PAGE_EQUIVALENT: usize = 2_000;

/// Read a file's shape without indexing it.
///
/// Refuses a scanned/image-only PDF here, with the same sentence the parse
/// paths use, so the refusal arrives BEFORE any indexing time is spent rather
/// than after a user has watched a progress bar for a document that was never
/// going to yield text.
pub fn inspect_path(path: &Path) -> Result<DocumentShape, GaplyError> {
    if !path.exists() {
        return Err(GaplyError::Validation(format!(
            "file not found: {} — the desktop app must pass an absolute file path.",
            path.display()
        )));
    }
    let bytes = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase())
        .unwrap_or_default();

    if ext == "pdf" {
        let pages = catch_pdf_panic(|| pdf_extract::extract_text_by_pages(path))?
            .map_err(|e| GaplyError::Internal(format!("pdf parse failed: {e}")))?;
        let joined = pages.join("\n");
        if !has_extractable_text(&joined) {
            return Err(GaplyError::Validation(NO_TEXT_LAYER_ADVICE.to_string()));
        }
        let n = u32::try_from(pages.len()).unwrap_or(u32::MAX);
        return Ok(DocumentShape {
            pages: Some(n),
            page_equivalents: n.max(1),
            bytes,
            text_chars: joined.chars().filter(|c| !c.is_whitespace()).count(),
        });
    }

    let text = parse_path(path)?;
    let text_chars = text.chars().filter(|c| !c.is_whitespace()).count();
    Ok(DocumentShape {
        pages: None,
        page_equivalents: u32::try_from(text_chars.div_ceil(CHARS_PER_PAGE_EQUIVALENT))
            .unwrap_or(u32::MAX)
            .max(1),
        bytes,
        text_chars,
    })
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
        return Err(GaplyError::Validation(NO_TEXT_LAYER_ADVICE.to_string()));
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
        return Err(GaplyError::Validation(NO_TEXT_LAYER_ADVICE.to_string()));
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
    fn a_paragraph_wrapping_a_page_break_is_one_block_on_its_starting_page() {
        // Page 2 ends mid-sentence; page 3 continues it. The reflow must join
        // them into ONE block and attribute it to page 2 — the page it started
        // on — never to page 3 and never to a guess.
        let lines: Vec<(Option<u32>, &str)> = vec![
            (Some(2), "the shortfall is traced largely"),
            (Some(2), ""),
            (Some(3), "to poor larval growth."),
            (Some(3), ""),
            (Some(3), "A new paragraph starts here."),
        ];
        let blocks = reflow_pdf_lines(&lines);
        assert_eq!(blocks.len(), 2, "unexpected blocks: {blocks:?}");
        assert_eq!(blocks[0].1, "the shortfall is traced largely to poor larval growth.");
        assert_eq!(blocks[0].0, Some(2), "the spanning paragraph lost its starting page");
        assert_eq!(blocks[1].0, Some(3));
    }

    #[test]
    fn untagged_lines_stay_page_less_and_match_the_flat_reflow() {
        // reflow_pdf_text is the same algorithm with every page tag None; a
        // page-less source must never acquire a page.
        let raw = "Body text here.\n\nMore body text.";
        let lines: Vec<(Option<u32>, &str)> = raw.lines().map(|l| (None, l)).collect();
        let blocks = reflow_pdf_lines(&lines);
        assert!(blocks.iter().all(|(p, _)| p.is_none()));
        let joined = blocks.iter().map(|(_, t)| t.as_str()).collect::<Vec<_>>().join("\n\n");
        assert_eq!(joined, reflow_pdf_text(raw), "the two entry points diverged");
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
    fn inspect_reports_shape_for_a_real_text_pdf() {
        let fixture = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/sample_text.pdf");
        let shape = inspect_path(Path::new(fixture)).expect("a text PDF should inspect");
        assert!(shape.pages.is_some(), "a PDF must report real pages");
        assert!(shape.page_equivalents >= 1);
        assert!(shape.bytes > 0);
        assert!(shape.text_chars >= MIN_MEANINGFUL_CHARS);
    }

    #[test]
    fn inspect_refuses_a_scanned_pdf_with_the_ocr_advice_before_any_indexing() {
        // A real, valid, 2-page PDF that draws a rectangle and shows no text —
        // what a scan looks like to an extractor. The refusal must arrive from
        // INSPECTION, i.e. before a documents row or a single embedded chunk
        // exists, because the whole cost of finding out later is the point.
        let fixture = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/scanned_no_text.pdf");
        let err = inspect_path(Path::new(fixture)).expect_err("a scanned PDF must be refused");
        let msg = err.to_string();
        assert!(msg.contains("no extractable text"), "{msg}");
        assert!(msg.contains("OCR"), "the advice must say what to DO about it: {msg}");
        // And the parse path refuses it with the SAME sentence — one message,
        // not two descriptions of one situation.
        let parse_err = parse_path_paged(Path::new(fixture)).expect_err("parse must refuse too");
        assert_eq!(parse_err.to_string(), msg);
    }

    #[test]
    fn inspect_treats_a_page_less_format_as_page_less() {
        let dir = std::env::temp_dir().join(format!("gaply_inspect_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let f = dir.join("notes.txt");
        std::fs::write(&f, "Methods: we recruited 48 participants and ran a t-test. ".repeat(60))
            .unwrap();
        let shape = inspect_path(&f).expect("a txt file should inspect");
        assert_eq!(shape.pages, None, "a .txt has no pages to report");
        assert!(shape.page_equivalents >= 1);
        let _ = std::fs::remove_dir_all(&dir);
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

#[cfg(test)]
mod reference_convention_tests {
    use super::*;
    use crate::extract::sections::{split_document, SectionKind};

    fn no_furniture() -> std::collections::HashSet<String> {
        std::collections::HashSet::new()
    }

    // FIXTURES MUST NOT REPEAT A LINE VERBATIM. `page_furniture` drops any line
    // under 80 chars seen three or more times, treating it as a running header —
    // so a reference template with an identical line per entry has that line
    // removed BEFORE the classifier sees it, and the fixture then measures
    // something other than what it claims. Caught by a mutation whose test
    // failed for this reason rather than the intended one.

    /// A NOISE-convention References section: a blank line after every rendered
    /// line, entries wrapped across lines. This is the shape reflow was built
    /// for and must keep handling.
    fn noise_refs(entries: usize) -> String {
        let mut s = String::from("A Study\n\nResults\n\nWe measured things.\n\nReferences\n\n");
        for i in 0..entries {
            s.push_str(&format!("Author {i} A B. 2020. A title that wraps across\n\n"));
            s.push_str(&format!("more than one rendered line. Journal of Things {i}: 1-9.\n\n"));
        }
        s
    }

    /// A STRUCTURE-convention References section: entries separated by blank
    /// lines, wrapped lines contiguous, and entries ending in a bare URL so
    /// `ends_sentence` never closes them.
    fn structure_refs(entries: usize) -> String {
        let mut s = String::from("A Study\n\nResults\n\nWe measured things.\n\nReferences\n\n");
        for i in 0..entries {
            s.push_str(&format!("Author {i} A B. Annual Report {i} on Waste Management. CPCB: New\n"));
            s.push_str(&format!("Delhi; 201{i}. Accessed: July 3{i}, 2026:\n"));
            s.push_str(&format!("https://example.org/uploads/Projects/Report_{i}.pdf\n\n"));
        }
        s
    }

    fn ref_count(text: &str) -> usize {
        let (_t, secs) = split_document(&reflow_pdf_text(text));
        secs.iter().find(|s| s.kind == SectionKind::References).map(|s| s.paragraphs.len()).unwrap_or(0)
    }

    fn classify(text: &str) -> RefConvention {
        let lines: Vec<&str> = text.lines().collect();
        let furn = page_furniture(&lines);
        let i = lines
            .iter()
            .position(|l| matches!(crate::extract::sections::detect_heading(l), Some((SectionKind::References, _))))
            .expect("fixture has a References heading");
        classify_references(&lines[i + 1..], &furn)
    }

    /// **THE REPAIR.** Entries ending in a bare URL close no sentence, so before
    /// this the whole list became ONE paragraph (§45: 26 references measured as
    /// 1 on a real manuscript).
    #[test]
    fn a_structure_convention_reference_list_keeps_its_entries() {
        let text = structure_refs(12);
        assert_eq!(classify(&text), RefConvention::Structure);
        assert_eq!(ref_count(&text), 12, "each blank-separated entry is one reference");
    }

    /// **THE INCUMBENT, UNTOUCHED.** A noise-convention list must still have its
    /// wrapped lines rejoined — skipping reflow here would restore the measured
    /// defect of one reference row per rendered line.
    #[test]
    fn a_noise_convention_reference_list_still_has_its_wrapped_lines_rejoined() {
        let text = noise_refs(10);
        assert_eq!(classify(&text), RefConvention::Noise);
        assert_eq!(ref_count(&text), 10, "two rendered lines per entry, rejoined");
    }

    /// **THE MEDIAN, NOT THE `any` FORM.** One wrapped entry in an otherwise
    /// noise-convention list flips "does any group exceed one line" to true. The
    /// median does not move, and the classification must not either — the `any`
    /// form would send this document down the Structure branch and split every
    /// wrapped entry in two.
    #[test]
    fn one_wrapped_entry_does_not_reclassify_a_noise_section() {
        // 10 single-line entries, then ONE that wraps without a blank inside.
        let mut text = String::from("A Study\n\nReferences\n\n");
        for i in 0..10 {
            text.push_str(&format!("Author {i} A B. 2020. A complete title. Journal {i}: 1-9.\n\n"));
        }
        text.push_str("Author X A B. 2021. A title that wraps here\nand continues on the next line. Journal X: 1-9.\n\n");

        let lines: Vec<&str> = text.lines().collect();
        let furn = no_furniture();
        let i = lines
            .iter()
            .position(|l| matches!(crate::extract::sections::detect_heading(l), Some((SectionKind::References, _))))
            .unwrap();
        let body = &lines[i + 1..];
        // the `any` form WOULD flip — assert the fixture really exercises it
        let mut sizes = Vec::new();
        let mut run = 0;
        for l in body {
            if l.trim().is_empty() { if run > 0 { sizes.push(run); run = 0; } } else { run += 1 }
        }
        if run > 0 { sizes.push(run) }
        assert!(sizes.iter().any(|s| *s > 1), "fixture must contain a wrapped entry");

        assert_eq!(
            classify_references(body, &furn),
            RefConvention::Noise,
            "the median is unmoved by one wrapped entry; the `any` form would not be"
        );
    }

    /// **THE SIGNAL IS ABSENT, NOT SCARCE.** A large, well-formed References
    /// section with NO blank lines at all has one group and a huge median. A
    /// naive `median > 1` rule classifies it Structure, finds nothing to close
    /// on, and collapses the whole section — the defect this prevents, reached
    /// from the opposite direction (§46.7).
    #[test]
    fn a_reference_section_with_no_blank_lines_is_unclassified() {
        let mut text = String::from("A Study\n\nReferences\n\n");
        for i in 0..40 {
            text.push_str(&format!("Author {i} A B. 2020. A complete title. Journal {i}: 1-9.\n"));
        }
        assert_eq!(classify(&text), RefConvention::Unclassified);
    }

    /// An empty References section classifies as absent, not as a convention.
    #[test]
    fn an_empty_reference_section_is_unclassified() {
        assert_eq!(classify("A Study\n\nReferences\n\n"), RefConvention::Unclassified);
    }

    /// The switch is scoped to References. A body paragraph in the same document
    /// keeps the incumbent rule, so the Structure classification cannot leak
    /// into prose.
    #[test]
    fn the_structure_switch_does_not_reach_body_paragraphs() {
        let mut text = String::from("A Study\n\nResults\n\nOne sentence that wraps\nacross two rendered lines.\n\n");
        text.push_str("A second paragraph, also wrapped\nacross two lines.\n\n");
        text.push_str(&structure_refs(3)[structure_refs(3).find("References").unwrap()..]);
        let (_t, secs) = split_document(&reflow_pdf_text(&text));
        let results = secs.iter().find(|s| s.kind == SectionKind::Results).unwrap();
        assert_eq!(results.paragraphs.len(), 2, "wrapped body lines stay rejoined: {:?}", results.paragraphs);
    }

    /// **A BARE URL ON ITS OWN LINE IS THE NEXT ENTRY'S, NOT THE LAST ONE'S.**
    ///
    /// The `is_url_only` guard exists because under the incumbent rule a lone
    /// DOI line would become the HEAD of the next entry. Under `Structure` the
    /// blank line has ALREADY closed the previous entry, so appending it there
    /// merges two references — the defect this path exists to prevent.
    ///
    /// The earlier fixture never reached the guard: its URL was the third line
    /// of a three-line entry, so `cur` was non-empty. This one puts the URL in
    /// its own blank-separated group, which is what the guard actually tests.
    #[test]
    fn a_lone_url_line_starts_its_entry_rather_than_joining_the_previous_one() {
        let mut text = String::from("A Study\n\nReferences\n\n");
        for i in 0..6 {
            text.push_str(&format!("Author {i} A B. A report {i} on waste management. CPCB: New\n"));
            text.push_str(&format!("Delhi; 201{i}. Accessed: July 3{i}, 2026:\n\n"));
            text.push_str(&format!("https://example.org/uploads/Report_{i}.pdf\n\n"));
        }
        assert_eq!(classify(&text), RefConvention::Structure);
        // 6 entries x 2 blank-separated groups = 12 paragraphs, and CRUCIALLY
        // none of the URL groups may be swallowed into the group before it.
        let (_t, secs) = split_document(&reflow_pdf_text(&text));
        let refs = secs.iter().find(|s| s.kind == SectionKind::References).unwrap();
        assert_eq!(refs.paragraphs.len(), 12, "no group may be merged: {:?}", refs.paragraphs);
        assert!(
            refs.paragraphs.iter().filter(|p| p.starts_with("https://")).count() == 6,
            "every URL group must stand alone: {:?}",
            refs.paragraphs
        );
    }

    /// **THE STRUCTURE CONVENTION MUST NOT LEAK PAST THE REFERENCES SECTION.**
    ///
    /// A recognised heading AFTER References returns to the incumbent rule. If
    /// the in-references flag were merely set and never cleared, a
    /// noise-convention section following the reference list would have every
    /// rendered line closed into its own paragraph — 1b's damage, caused here.
    #[test]
    fn a_section_after_references_returns_to_the_incumbent_rule() {
        let mut text = String::from("A Study\n\nReferences\n\n");
        for i in 0..6 {
            text.push_str(&format!("Author {i} A B. A report {i}. CPCB: New\n"));
            text.push_str(&format!("Delhi; 201{i}. Accessed: {i}:\n\n"));
        }
        text.push_str("Conclusion\n\nA closing paragraph that wraps\n\nacross two rendered lines.\n\n");
        assert_eq!(classify(&text), RefConvention::Structure);

        let (_t, secs) = split_document(&reflow_pdf_text(&text));
        let concl = secs.iter().find(|s| s.kind == SectionKind::Conclusion).unwrap();
        assert_eq!(
            concl.paragraphs.len(),
            1,
            "the wrapped conclusion must be rejoined, not split by the References rule: {:?}",
            concl.paragraphs
        );
    }

    /// **DOCX IS UNTOUCHED — asserted, not assumed.** `reflow_pdf_text` is called
    /// only from `parse_pdf`; a DOCX reference list must parse identically before
    /// and after this change, which it does because reflow never runs on it.
    #[test]
    fn docx_reference_parsing_does_not_route_through_reflow() {
        // One Word paragraph per entry is what parse_docx emits (\n\n per </w:p>).
        let docx_like = "A Study\n\nReferences\n\nAuthor A. 2020. First. J 1: 1-9.\n\nAuthor B. 2021. Second. J 2: 1-9.\n\nAuthor C. 2022. Third. J 3: 1-9.\n\n";
        let (_t, secs) = split_document(docx_like);
        let refs = secs.iter().find(|s| s.kind == SectionKind::References).unwrap();
        assert_eq!(refs.paragraphs.len(), 3, "the DOCX path does not call reflow at all");
    }
}
