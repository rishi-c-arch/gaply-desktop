//! The RENDERER layer — "how do these blocks become pages?"
//!
//! Takes `&[Block]` and returns PDF bytes. **Forbidden: evidence,
//! recommendation, filtering, ranking, sorting, truncation.** The input type
//! carries no severity to re-rank and no finding it could drop; a renderer that
//! wants to omit something must ask the composer to stop emitting it.
//!
//! It lives in `gaply_core` rather than the app crate because `render_pdf` is a
//! PURE function — `&[Block] -> Vec<u8>`, no filesystem, no clock, no network.
//! The actual I/O (writing the file, handing bytes to IPC) stays in the app
//! crate. Keeping it here also puts it beside `pdf-extract`, which the
//! render-path instrument at the bottom of this file needs as its oracle.
//!
//! # v1 font: base-14 Helvetica, `/WinAnsiEncoding`, AFM widths
//!
//! No font is embedded, so the PDF stays a few kilobytes and needs no
//! redistribution licence. Bold is NOT used: Helvetica-Bold needs its own AFM
//! width table, and wrong metrics produce silently wrong wrapping. Hierarchy is
//! carried by size and spacing instead. Embedding Noto (and with it bold, and
//! full Unicode) is the documented next step — see ARCHITECTURE_TRACE §31.18.

use crate::report_compose::{Block, NOTE_MARKED, NOTE_SIMPLIFIED};

const PAGE_W: f64 = 595.0;
const PAGE_H: f64 = 842.0;
const MARGIN: f64 = 56.0;
const TEXT_W: f64 = PAGE_W - 2.0 * MARGIN;

/// What happened to a character on its way to the page.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CharFate {
    /// Written byte-for-byte through WinAnsi.
    Intact,
    /// Folded to its Latin base letter because base-14 cannot represent it.
    Simplified,
    /// No Latin base exists; a visible mark stands in its place.
    Marked,
}

/// Which fates occurred across a whole document.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct EncodingOutcome {
    pub any_simplified: bool,
    pub any_marked: bool,
}

impl EncodingOutcome {
    fn record(&mut self, fate: CharFate) {
        match fate {
            CharFate::Intact => {}
            CharFate::Simplified => self.any_simplified = true,
            CharFate::Marked => self.any_marked = true,
        }
    }
}

/// The mark for a character with no Latin base. `?` is the conventional
/// stand-in and no viewer hides it.
const MARK: u8 = b'?';

/// WinAnsi's `0x80`–`0x9F` block, which is NOT Latin-1 — the range where a
/// decoder assuming ISO-8859-1 goes wrong, and the range this table exists to
/// get right.
const WINANSI_HIGH: [(char, u8); 27] = [
    ('\u{20AC}', 0x80), ('\u{201A}', 0x82), ('\u{0192}', 0x83), ('\u{201E}', 0x84),
    ('\u{2026}', 0x85), ('\u{2020}', 0x86), ('\u{2021}', 0x87), ('\u{02C6}', 0x88),
    ('\u{2030}', 0x89), ('\u{0160}', 0x8A), ('\u{2039}', 0x8B), ('\u{0152}', 0x8C),
    ('\u{017D}', 0x8E), ('\u{2018}', 0x91), ('\u{2019}', 0x92), ('\u{201C}', 0x93),
    ('\u{201D}', 0x94), ('\u{2022}', 0x95), ('\u{2013}', 0x96), ('\u{2014}', 0x97),
    ('\u{02DC}', 0x98), ('\u{2122}', 0x99), ('\u{0161}', 0x9A), ('\u{203A}', 0x9B),
    ('\u{0153}', 0x9C), ('\u{017E}', 0x9E), ('\u{0178}', 0x9F),
];

/// The exact WinAnsi byte for a character, or `None` if it has none.
pub fn winansi_byte(ch: char) -> Option<u8> {
    let c = ch as u32;
    // Printable ASCII, then the Latin-1 upper half, both identity-mapped.
    if (0x20..0x7F).contains(&c) || (0xA0..=0xFF).contains(&c) {
        return Some(c as u8);
    }
    WINANSI_HIGH.iter().find(|(k, _)| *k == ch).map(|(_, b)| *b)
}

/// The Latin base letter(s) for a character base-14 cannot represent.
///
/// # A TABLE, not NFD stripping — and it is strictly better here
///
/// The obvious implementation decomposes with NFD and drops combining marks.
/// **That fails on exactly the case that motivated the whole rule: `Ł` has no
/// decomposition**, so NFD leaves it unmapped and `Łukasz` still renders
/// `?ukasz`. A table maps `Ł → L` directly.
///
/// It also needs no new dependency: `gaply_core` has no `unicode-normalization`
/// (nor `unicode-segmentation` — every path to that crate in the workspace runs
/// through the app crate), and this range is small, closed and slow-moving.
///
/// Covers Latin Extended-A (U+0100–U+017F) and the four Romanian comma-below
/// letters from Extended-B. **Latin Extended Additional (Vietnamese) is a known
/// gap** and falls through to `Marked`.
fn latin_base(ch: char) -> Option<&'static str> {
    Some(match ch {
        'Ā' | 'Ă' | 'Ą' => "A",
        'ā' | 'ă' | 'ą' => "a",
        'Ć' | 'Ĉ' | 'Ċ' | 'Č' => "C",
        'ć' | 'ĉ' | 'ċ' | 'č' => "c",
        'Ď' | 'Đ' => "D",
        'ď' | 'đ' => "d",
        'Ē' | 'Ĕ' | 'Ė' | 'Ę' | 'Ě' => "E",
        'ē' | 'ĕ' | 'ė' | 'ę' | 'ě' => "e",
        'Ĝ' | 'Ğ' | 'Ġ' | 'Ģ' => "G",
        'ĝ' | 'ğ' | 'ġ' | 'ģ' => "g",
        'Ĥ' | 'Ħ' => "H",
        'ĥ' | 'ħ' => "h",
        'Ĩ' | 'Ī' | 'Ĭ' | 'Į' | 'İ' => "I",
        'ĩ' | 'ī' | 'ĭ' | 'į' | 'ı' => "i",
        'Ĳ' => "IJ",
        'ĳ' => "ij",
        'Ĵ' => "J",
        'ĵ' => "j",
        'Ķ' => "K",
        'ķ' | 'ĸ' => "k",
        'Ĺ' | 'Ļ' | 'Ľ' | 'Ŀ' | 'Ł' => "L",
        'ĺ' | 'ļ' | 'ľ' | 'ŀ' | 'ł' => "l",
        'Ń' | 'Ņ' | 'Ň' | 'Ŋ' => "N",
        'ń' | 'ņ' | 'ň' | 'ŉ' | 'ŋ' => "n",
        'Ō' | 'Ŏ' | 'Ő' => "O",
        'ō' | 'ŏ' | 'ő' => "o",
        'Ŕ' | 'Ŗ' | 'Ř' => "R",
        'ŕ' | 'ŗ' | 'ř' => "r",
        'Ś' | 'Ŝ' | 'Ş' | 'Ș' => "S",
        'ś' | 'ŝ' | 'ş' | 'ș' => "s",
        'Ţ' | 'Ť' | 'Ŧ' | 'Ț' => "T",
        'ţ' | 'ť' | 'ŧ' | 'ț' => "t",
        'Ũ' | 'Ū' | 'Ŭ' | 'Ů' | 'Ű' | 'Ų' => "U",
        'ũ' | 'ū' | 'ŭ' | 'ů' | 'ű' | 'ų' => "u",
        'Ŵ' => "W",
        'ŵ' => "w",
        'Ŷ' => "Y",
        'ŷ' => "y",
        'Ź' | 'Ż' => "Z",
        'ź' | 'ż' => "z",
        'ſ' => "s",
        _ => return None,
    })
}

/// Encode one string to WinAnsi bytes, folding or marking what it cannot hold.
///
/// **Latin-1 is byte-exact and untouched**: `Müller` stays `Müller`. Only what
/// base-14 cannot represent at all is folded, and only to its own base letter —
/// `?ukasz` is unusable while `Lukasz` is wrong but readable, and for a NAME
/// recognisability is the axis that matters to its owner.
pub fn encode_winansi(s: &str) -> (Vec<u8>, EncodingOutcome) {
    let mut bytes = Vec::with_capacity(s.len());
    let mut outcome = EncodingOutcome::default();
    for ch in s.chars() {
        if let Some(b) = winansi_byte(ch) {
            bytes.push(b);
            outcome.record(CharFate::Intact);
        } else if let Some(base) = latin_base(ch) {
            // The base letters are ASCII by construction, so this cannot recurse.
            bytes.extend(base.bytes());
            outcome.record(CharFate::Simplified);
        } else {
            bytes.push(MARK);
            outcome.record(CharFate::Marked);
        }
    }
    (bytes, outcome)
}

/// Adobe Helvetica AFM advance widths, in 1/1000 em, indexed by WinAnsi byte.
/// Zero for the byte values WinAnsi leaves undefined.
#[rustfmt::skip]
const HELVETICA_WIDTHS: [u16; 256] = [
    // 0x00–0x1F — undefined
    0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0, 0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,
    // 0x20–0x2F
    278,278,355,556,556,889,667,191,333,333,389,584,278,333,278,278,
    // 0x30–0x3F
    556,556,556,556,556,556,556,556,556,556,278,278,584,584,584,556,
    // 0x40–0x4F
    1015,667,667,722,722,667,611,778,722,278,500,667,556,833,722,778,
    // 0x50–0x5F
    667,778,722,667,611,722,667,944,667,667,611,278,278,278,469,556,
    // 0x60–0x6F
    333,556,556,500,556,556,278,556,556,222,222,500,222,833,556,556,
    // 0x70–0x7F
    556,556,333,500,278,556,500,722,500,500,500,334,260,334,584,0,
    // 0x80–0x8F
    556,0,222,556,333,1000,556,556,333,1000,667,333,1000,0,611,0,
    // 0x90–0x9F
    0,222,222,333,333,350,556,1000,333,1000,500,333,944,0,500,667,
    // 0xA0–0xAF
    278,333,556,556,556,556,260,556,333,737,370,556,584,333,737,333,
    // 0xB0–0xBF
    400,584,333,333,333,556,537,278,333,333,365,556,834,834,834,611,
    // 0xC0–0xCF
    667,667,667,667,667,667,1000,722,667,667,667,667,278,278,278,278,
    // 0xD0–0xDF
    722,722,778,778,778,778,778,584,778,722,722,722,722,667,667,611,
    // 0xE0–0xEF
    556,556,556,556,556,556,889,500,556,556,556,556,278,278,278,278,
    // 0xF0–0xFF
    556,556,556,556,556,556,556,584,611,556,556,556,556,500,556,500,
];

fn text_width(bytes: &[u8], size: f64) -> f64 {
    bytes.iter().map(|b| HELVETICA_WIDTHS[*b as usize] as f64).sum::<f64>() * size / 1000.0
}

/// One positioned line of already-encoded bytes.
struct Line {
    bytes: Vec<u8>,
    size: f64,
    x: f64,
    gray: f64,
    /// Extra space above this line.
    lead: f64,
}

/// Greedy word wrap on encoded bytes. Splits on ASCII space, which is exact:
/// every encoded byte is one character wide and `0x20` cannot occur inside a
/// multi-byte sequence — the encoding has none.
fn wrap(bytes: &[u8], size: f64, width: f64) -> Vec<Vec<u8>> {
    let mut lines: Vec<Vec<u8>> = Vec::new();
    let mut cur: Vec<u8> = Vec::new();
    for word in bytes.split(|b| *b == b' ') {
        if word.is_empty() {
            continue;
        }
        let mut trial = cur.clone();
        if !trial.is_empty() {
            trial.push(b' ');
        }
        trial.extend_from_slice(word);
        if text_width(&trial, size) <= width || cur.is_empty() {
            cur = trial;
        } else {
            lines.push(std::mem::take(&mut cur));
            cur = word.to_vec();
        }
    }
    if !cur.is_empty() {
        lines.push(cur);
    }
    if lines.is_empty() {
        lines.push(Vec::new());
    }
    lines
}

/// Render composed blocks to PDF bytes.
pub fn render_pdf(blocks: &[Block]) -> Vec<u8> {
    let mut outcome = EncodingOutcome::default();
    let mut lines: Vec<Option<Line>> = Vec::new(); // `None` = explicit page break

    let push_text = |lines: &mut Vec<Option<Line>>,
                         outcome: &mut EncodingOutcome,
                         text: &str,
                         size: f64,
                         indent: f64,
                         gray: f64,
                         lead: f64| {
        let (bytes, o) = encode_winansi(text);
        *outcome = EncodingOutcome {
            any_simplified: outcome.any_simplified || o.any_simplified,
            any_marked: outcome.any_marked || o.any_marked,
        };
        for (i, l) in wrap(&bytes, size, TEXT_W - indent).into_iter().enumerate() {
            lines.push(Some(Line {
                bytes: l,
                size,
                x: MARGIN + indent,
                gray,
                lead: if i == 0 { lead } else { 0.0 },
            }));
        }
    };

    for block in blocks {
        match block {
            Block::Cover { title, subtitle, meta } => {
                push_text(&mut lines, &mut outcome, subtitle, 11.0, 0.0, 0.45, 120.0);
                push_text(&mut lines, &mut outcome, title, 22.0, 0.0, 0.0, 16.0);
                for (k, v) in meta {
                    push_text(&mut lines, &mut outcome, &format!("{k}: {v}"), 9.5, 0.0, 0.4, 6.0);
                }
                lines.push(None);
            }
            Block::Heading { text, level } => {
                let (size, lead) = match level {
                    1 => (16.0, 22.0),
                    2 => (12.5, 16.0),
                    _ => (11.0, 12.0),
                };
                push_text(&mut lines, &mut outcome, text, size, 0.0, 0.0, lead);
            }
            Block::Paragraph { text } => {
                push_text(&mut lines, &mut outcome, text, 10.0, 0.0, 0.15, 7.0)
            }
            Block::Bullet { text, indent } => {
                let ind = 14.0 + f64::from(*indent) * 14.0;
                push_text(&mut lines, &mut outcome, &format!("- {text}"), 10.0, ind, 0.25, 3.0);
            }
            Block::Note { text } => {
                push_text(&mut lines, &mut outcome, text, 8.5, 0.0, 0.45, 10.0)
            }
            Block::PageBreak => lines.push(None),
        }
    }

    // The renderer discloses its OWN limitations. It is not deciding anything
    // about the content it was handed — it is reporting what it could not do
    // with it, which nothing upstream is in a position to know.
    let (simplified, marked) = (outcome.any_simplified, outcome.any_marked);
    // The notes are ASCII, so their own encoding outcome is discarded into a
    // scratch value rather than folded back — otherwise a note could describe
    // itself.
    let mut scratch = EncodingOutcome::default();
    if simplified {
        push_text(&mut lines, &mut scratch, NOTE_SIMPLIFIED, 8.5, 0.0, 0.45, 18.0);
    }
    if marked {
        push_text(&mut lines, &mut scratch, NOTE_MARKED, 8.5, 0.0, 0.45, 8.0);
    }
    debug_assert_eq!(scratch, EncodingOutcome::default(), "disclosure notes must be ASCII");

    paginate_and_write(lines)
}

fn paginate_and_write(lines: Vec<Option<Line>>) -> Vec<u8> {
    let mut pages: Vec<Vec<u8>> = Vec::new();
    let mut stream = Vec::new();
    let mut y = PAGE_H - MARGIN;
    let mut page_empty = true;

    for item in lines {
        let Some(line) = item else {
            if !page_empty {
                pages.push(std::mem::take(&mut stream));
                y = PAGE_H - MARGIN;
                page_empty = true;
            }
            continue;
        };
        let advance = line.size * 1.32;
        y -= line.lead + advance;
        if y < MARGIN {
            pages.push(std::mem::take(&mut stream));
            y = PAGE_H - MARGIN - advance;
        }
        page_empty = false;
        stream.extend_from_slice(format!("{:.3} g\nBT\n/F1 {:.2} Tf\n1 0 0 1 {:.2} {:.2} Tm\n(", line.gray, line.size, line.x, y).as_bytes());
        stream.extend_from_slice(&escape_pdf(&line.bytes));
        stream.extend_from_slice(b") Tj\nET\n");
    }
    if !page_empty {
        pages.push(stream);
    }
    if pages.is_empty() {
        pages.push(Vec::new());
    }
    write_pdf(&pages)
}

/// Escape a PDF literal string. Bytes outside printable ASCII are written as
/// octal escapes so no parser has to guess at the encoding of a raw high byte.
fn escape_pdf(bytes: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(bytes.len() + 8);
    for b in bytes {
        match b {
            b'\\' | b'(' | b')' => {
                out.push(b'\\');
                out.push(*b);
            }
            0x20..=0x7E => out.push(*b),
            _ => out.extend_from_slice(format!("\\{:03o}", b).as_bytes()),
        }
    }
    out
}

/// Assemble the object graph. Object numbering: 1 catalog, 2 pages, 3 font,
/// then two objects per page (page dict, content stream).
fn write_pdf(pages: &[Vec<u8>]) -> Vec<u8> {
    let n = pages.len();
    let mut objs: Vec<Vec<u8>> = Vec::new();

    let kids: Vec<String> = (0..n).map(|i| format!("{} 0 R", 4 + i * 2)).collect();
    objs.push(b"<< /Type /Catalog /Pages 2 0 R >>".to_vec());
    objs.push(format!("<< /Type /Pages /Kids [{}] /Count {n} >>", kids.join(" ")).into_bytes());
    objs.push(
        b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding >>"
            .to_vec(),
    );
    for (i, content) in pages.iter().enumerate() {
        objs.push(
            format!(
                "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {PAGE_W:.0} {PAGE_H:.0}] \
                 /Resources << /Font << /F1 3 0 R >> >> /Contents {} 0 R >>",
                5 + i * 2
            )
            .into_bytes(),
        );
        let mut stream = format!("<< /Length {} >>\nstream\n", content.len()).into_bytes();
        stream.extend_from_slice(content);
        stream.extend_from_slice(b"endstream");
        objs.push(stream);
    }

    let mut out: Vec<u8> = b"%PDF-1.4\n".to_vec();
    let mut offsets = Vec::with_capacity(objs.len());
    for (i, body) in objs.iter().enumerate() {
        offsets.push(out.len());
        out.extend_from_slice(format!("{} 0 obj\n", i + 1).as_bytes());
        out.extend_from_slice(body);
        out.extend_from_slice(b"\nendobj\n");
    }
    let xref_at = out.len();
    out.extend_from_slice(format!("xref\n0 {}\n0000000000 65535 f \n", objs.len() + 1).as_bytes());
    for off in &offsets {
        out.extend_from_slice(format!("{off:010} 00000 n \n").as_bytes());
    }
    out.extend_from_slice(
        format!(
            "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref_at}\n%%EOF\n",
            objs.len() + 1
        )
        .as_bytes(),
    );
    out
}

// ============================================================================
// THE RENDER-PATH INSTRUMENT — ONTOLOGY §4.20
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::evidence::ClaimKind;
    use crate::report::{CertaintyTier, ChecklistItem, FindingSeverity};
    use crate::report_compose::compose;
    use crate::report_model::{
        LocalFinding, LocalReportModel, ManuscriptFacts, ReportedStatistic,
    };
    use crate::reviewer_agent::LaneExamination;
    use crate::swarm::AgentKind;

    /// Read the text back OUT OF THE PDF BYTES.
    ///
    /// **This is what makes the instrument an artifact assertion.** Asserting on
    /// `Vec<Block>` would pass while the PDF said `arm` — precisely the gap
    /// §31.15 named. `pdf-extract` was verified as an oracle before being
    /// trusted: a hand-written WinAnsi PDF round-tripped `0xFC → ü`, `0xE9 → é`,
    /// `0x8A → Š` and `0x9E → ž`, the last two proving it decodes CP1252's
    /// `0x80`–`0x9F` block rather than assuming ISO-8859-1.
    fn text_from_pdf(bytes: &[u8]) -> String {
        pdf_extract::extract_text_from_mem(bytes).expect("pdf-extract could not read our own PDF")
    }

    /// `pdf-extract` may normalise or reorder whitespace, so needles are matched
    /// against a whitespace-free projection. Still an assertion about the
    /// ARTIFACT, never about an intermediate structure.
    fn squash(s: &str) -> String {
        s.chars().filter(|c| !c.is_whitespace()).collect()
    }

    fn finding(id: &str, title: &str, severity: FindingSeverity) -> LocalFinding {
        LocalFinding {
            id: id.into(),
            severity,
            tier: CertaintyTier::MathematicallyCertain,
            claim: ClaimKind::ManuscriptDefect,
            agent: AgentKind::ValidationMaths,
            title: title.into(),
            detail: "A plain ASCII detail sentence.".into(),
            confidence: 1.0,
            provenance: vec!["rule:test".into()],
            nearby_text: None,
        }
    }

    fn model(title: Option<&str>, findings: Vec<LocalFinding>) -> LocalReportModel {
        LocalReportModel {
            run_id: "run-1".into(),
            manuscript: ManuscriptFacts {
                title: title.map(str::to_string),
                word_count: 4000,
                section_count: 5,
                table_count: 2,
                reference_count: 30,
                statistics: vec![ReportedStatistic {
                    kind: "p-value".into(),
                    reported: "p = 0.03".into(),
                    location: "Results, paragraph 2".into(),
                    effect_size_present: false,
                }],
            },
            journal_name: None,
            guidelines_url: None,
            findings,
            verdict: "Major revision".into(),
            combined_confidence: 0.8,
            checklist: vec![ChecklistItem {
                requirement: "Structured abstract".into(),
                passed: true,
                detail: "Found.".into(),
                guideline_source: None,
            }],
            similarity: vec![],
            corpus_chunks_available: 0,
            lanes: LaneExamination {
                verification_examined: true,
                validation_examined: true,
                plagiarism_examined: false,
                ai_detection_examined: true,
                extraction_examined: true,
            },
            disclaimer: "Model-assisted assessment.".into(),
        }
    }

    fn render(m: &LocalReportModel) -> String {
        text_from_pdf(&render_pdf(&compose(m)))
    }

    /// THE fixture: a Devanagari title (no Latin base), a Latin-Extended-A
    /// author name (has one), a Latin-1 name (byte-exact), and ASCII findings.
    #[test]
    fn the_render_path_preserves_latin1_folds_extended_latin_and_marks_the_rest() {
        let m = model(
            Some("हिन्दी"),
            vec![
                finding("f1", "Reported by Śarmā and Müller", FindingSeverity::Critical),
                finding("f2", "Sample size is not stated", FindingSeverity::Major),
            ],
        );
        let text = squash(&render(&m));

        // 1. SIMPLIFIED — folded to its Latin base, not marked.
        assert!(text.contains("Sarma"), "Śarmā must fold to Sarma; got: {text}");
        assert!(!text.contains("Śarmā"), "base-14 cannot represent Ś or ā");
        assert!(!text.contains("?arm?"), "a foldable name must never be marked");

        // 2. INTACT — Latin-1 survives WinAnsi byte-exact, NOT transliterated.
        //    This is the assertion miniPdf's sanitizer test cannot make.
        assert!(text.contains("Müller"), "Müller must round-trip exactly; got: {text}");
        assert!(!text.contains("Muller"), "Latin-1 must not be folded");

        // 3. MARKED — no Latin base, so a visible mark and NO length claim.
        assert!(!text.contains("हिन्दी"), "Devanagari cannot be represented");
        assert!(text.contains('?'), "unrepresentable text must be visibly marked");

        // 4. ASCII findings pass through completely unaltered.
        assert!(text.contains(squash("Sample size is not stated").as_str()));
        assert!(text.contains(squash("A plain ASCII detail sentence.").as_str()));

        // 5. BOTH disclosures, because both outcomes occurred.
        assert!(text.contains(&squash(NOTE_SIMPLIFIED)), "simplified note missing");
        assert!(text.contains(&squash(NOTE_MARKED)), "marked note missing");
    }

    /// The "and ONLY when" half — a note that always fires discloses nothing.
    #[test]
    fn an_ascii_only_report_carries_neither_disclosure() {
        let m = model(Some("A Study of Things"), vec![finding("f1", "Ordinary title", FindingSeverity::Minor)]);
        let text = squash(&render(&m));
        assert!(!text.contains(&squash(NOTE_SIMPLIFIED)), "simplified note fired with nothing simplified");
        assert!(!text.contains(&squash(NOTE_MARKED)), "marked note fired with nothing marked");
    }

    /// Each disclosure fires INDEPENDENTLY: folding alone must not claim that
    /// something could not be displayed.
    #[test]
    fn folding_alone_discloses_only_simplification() {
        let m = model(Some("Study by Łukasz"), vec![finding("f1", "Ordinary title", FindingSeverity::Minor)]);
        let text = squash(&render(&m));
        assert!(text.contains("Lukasz"), "Ł must fold to L — the case NFD cannot handle");
        assert!(text.contains(&squash(NOTE_SIMPLIFIED)));
        assert!(!text.contains(&squash(NOTE_MARKED)), "nothing was marked");
    }

    /// The sentinel states no count, so the note must carry no number. A length
    /// claim would assert a character count nobody would recognise: `हिन्दी` is
    /// six code points but three visual clusters.
    #[test]
    fn the_marked_disclosure_makes_no_length_claim() {
        assert!(
            !NOTE_MARKED.chars().any(|c| c.is_ascii_digit()),
            "the marked disclosure must not state how much is missing"
        );
    }

    #[test]
    fn winansi_covers_latin1_exactly_and_seven_extended_a_characters() {
        for c in '\u{A0}'..='\u{FF}' {
            assert_eq!(winansi_byte(c), Some(c as u8), "Latin-1 must map identically");
        }
        let seven = ['Œ', 'œ', 'Š', 'š', 'Ÿ', 'Ž', 'ž'];
        for c in seven {
            assert!(winansi_byte(c).is_some(), "{c} is one of WinAnsi's seven Extended-A characters");
        }
        let mut found = 0;
        for c in '\u{100}'..='\u{17F}' {
            if winansi_byte(c).is_some() {
                assert!(seven.contains(&c), "unexpected Extended-A character {c} in WinAnsi");
                found += 1;
            }
        }
        assert_eq!(found, 7, "WinAnsi contains exactly seven Latin Extended-A characters");
    }

    /// The PDF must be structurally valid, not merely readable by one library.
    #[test]
    fn the_document_is_a_well_formed_multi_page_pdf() {
        let m = model(Some("A Study"), (0..40).map(|i| finding(&format!("f{i}"), &format!("Finding number {i} with a reasonably long title to force wrapping"), FindingSeverity::Major)).collect());
        let bytes = render_pdf(&compose(&m));
        assert!(bytes.starts_with(b"%PDF-1.4"), "missing header");
        assert!(bytes.ends_with(b"%%EOF\n"), "missing trailer");
        let s = String::from_utf8_lossy(&bytes);
        let startxref: usize = s.rsplit("startxref").next().unwrap().trim().lines().next().unwrap().parse().unwrap();
        assert_eq!(&bytes[startxref..startxref + 4], b"xref", "startxref does not point at the xref table");
        assert!(s.matches("/Type /Page ").count() > 1, "40 findings must paginate");
    }

    /// NO CAP: every finding reaches the page. `MAX_FINDINGS = 12` exists for
    /// the proxy's character budget, which a PDF does not have — so the report
    /// and the reviewer payload deliberately see different sets (§31.22).
    #[test]
    fn every_finding_reaches_the_page_regardless_of_count() {
        let n = 40;
        let m = model(Some("A Study"), (0..n).map(|i| finding(&format!("f{i}"), &format!("Distinct issue {i} zzq"), FindingSeverity::Major)).collect());
        let text = squash(&render(&m));
        for i in 0..n {
            assert!(text.contains(&squash(&format!("Distinct issue {i} zzq"))), "finding {i} was dropped");
        }
    }
}
