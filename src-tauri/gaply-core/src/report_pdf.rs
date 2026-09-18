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

use crate::report_compose::{Align, Block, Tone, NOTE_MARKED, NOTE_SIMPLIFIED};

const PAGE_W: f64 = 595.28; // A4
const PAGE_H: f64 = 841.89;
/// 25mm left and right, 24mm top and bottom, in points.
const MARGIN_X: f64 = 70.87;
const MARGIN_Y: f64 = 68.03;
const TEXT_W: f64 = PAGE_W - 2.0 * MARGIN_X;
/// §11 D93. The bar row's three columns: label, drawn track, count.
const BAR_LABEL_W: f64 = 190.0;
const BAR_COUNT_W: f64 = 78.0;
const BAR_TRACK_H: f64 = 7.0;
/// Space reserved for the running header and footer inside the margins.
const HEADER_H: f64 = 22.0;
const FOOTER_H: f64 = 18.0;

// ---------------------------------------------------------------------------
// Palette — exact hex from the reviewed specimen, as PDF RGB triples.
// ---------------------------------------------------------------------------
type Rgb = [f64; 3];
const fn rgb(r: u8, g: u8, b: u8) -> Rgb {
    [r as f64 / 255.0, g as f64 / 255.0, b as f64 / 255.0]
}
const INK: Rgb = rgb(0x14, 0x14, 0x14);
const BODY: Rgb = rgb(0x2E, 0x2E, 0x2E);
const MUTED: Rgb = rgb(0x70, 0x70, 0x70);
const FAINT: Rgb = rgb(0x9A, 0x9A, 0x9A);
const RULE: Rgb = rgb(0xD5, 0xD5, 0xD5);
const HAIR: Rgb = rgb(0xE8, 0xE8, 0xE8);
const PANEL: Rgb = rgb(0xF6, 0xF6, 0xF5);
const QUOTE_BG: Rgb = rgb(0xFB, 0xF7, 0xEF);
const QRULE: Rgb = rgb(0xD8, 0xC9, 0xA8);

/// Which base-14 face a line is set in. Oblique shares Helvetica's advance
/// widths (the AFM tables are identical — oblique is a shear of the same
/// glyphs), so only Bold needs its own metric table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Face {
    Regular,
    Bold,
    Oblique,
}

impl Face {
    fn resource(self) -> &'static str {
        match self {
            Face::Regular => "/F1",
            Face::Bold => "/F2",
            Face::Oblique => "/F3",
        }
    }
}

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
        // ── MATHEMATICAL TYPOGRAPHY ──────────────────────────────────────
        //
        // These are NOT unrepresentable — they are ordinary notation with an
        // exact ASCII spelling, so folding them is the same class as the
        // em-dash and curly-quote folding already done, not a loss.
        //
        // Measured on the reference manuscript: `p ≤ 0.05` rendered as
        // `p ? 0.05`, which made EVERY significance criterion unreadable in a
        // report whose subject is statistics (§52.4). The sentinel was working;
        // the table was incomplete.
        // ── GREEK ────────────────────────────────────────────────────────
        //
        // Same argument as the maths above, one step further: a Greek letter in
        // a methods sentence is notation with a conventional English spelling,
        // and `α_t = SoftMax(v_a^T tanh(W_a h_t))` rendered as
        // `?_t = SoftMax(...)` loses the variable the sentence is ABOUT. The
        // audit report's attention sections are full of these — every equation
        // line from the reference paper hit it.
        //
        // Spelled out rather than approximated to a Latin lookalike: `a` for α
        // would read as a different variable, which is worse than a longer one.
        '\u{03B1}' => "alpha",  '\u{0391}' => "Alpha",
        '\u{03B2}' => "beta",   '\u{0392}' => "Beta",
        '\u{03B3}' => "gamma",  '\u{0393}' => "Gamma",
        '\u{03B4}' => "delta",  '\u{0394}' => "Delta",
        '\u{03B5}' => "epsilon",
        '\u{03B8}' => "theta",  '\u{0398}' => "Theta",
        '\u{03BB}' => "lambda", '\u{039B}' => "Lambda",
        '\u{03BC}' => "mu",
        '\u{03C0}' => "pi",     '\u{03A0}' => "Pi",
        '\u{03C1}' => "rho",
        '\u{03C3}' => "sigma",  '\u{03A3}' => "Sigma",
        '\u{03C4}' => "tau",
        '\u{03C6}' => "phi",    '\u{03A6}' => "Phi",
        '\u{03C7}' => "chi",
        '\u{03C8}' => "psi",    '\u{03A8}' => "Psi",
        '\u{03C9}' => "omega",  '\u{03A9}' => "Omega",
        '\u{2211}' => "sum",    // ∑
        '\u{220F}' => "prod",   // ∏
        '\u{221A}' => "sqrt",   // √
        '\u{221E}' => "inf",    // ∞
        '\u{2248}' => "~=",     // ≈
        '\u{00B1}' => "+/-",    // ±  (in Latin-1, but spelled for clarity)
        '\u{2264}' => "<=", // ≤
        '\u{2265}' => ">=", // ≥
        '\u{2260}' => "!=", // ≠
        '\u{2212}' => "-",  // − MINUS SIGN, not HYPHEN-MINUS
        '\u{2032}' => "'",  // ′ prime
        '\u{2033}' => "\"", // ″ double prime
        '\u{2192}' => "->", // →
        '\u{2190}' => "<-", // ←

        // ── TYPOGRAPHIC LIGATURES ────────────────────────────────────────
        //
        // §11 D70. These are the characters REAL fetched papers broke the table
        // with, immediately. The Greek and maths above were added from
        // synthetic examples; the first open-access PDF ever pulled through the
        // OA path (SemEval-2018, OpenAlex) rendered `classification` as
        // `classi?cation` and `~1,500` as `?1,500`, because typesetters emit
        // U+FB01 and U+223C and nothing here covered them.
        //
        // A ligature is PURELY presentational — `ﬁ` IS `fi`, with no semantic
        // content whatsoever — so folding it is lossless, not an approximation.
        // A `?` in the middle of a word, by contrast, is the one outcome a
        // reader cannot interpret: it hides which word was written.
        '\u{FB00}' => "ff",
        '\u{FB01}' => "fi",
        '\u{FB02}' => "fl",
        '\u{FB03}' => "ffi",
        '\u{FB04}' => "ffl",
        '\u{FB05}' => "st",   // ﬅ long-s+t, archaic but present in scans
        '\u{FB06}' => "st",   // ﬆ

        // ── DASHES AND HYPHENS NOT IN WINANSI ────────────────────────────
        //
        // En-dash, em-dash and the four curly quotes are already WinAnsi code
        // points (0x96, 0x97, 0x91-0x94) and need no entry — these are the ones
        // that are not.
        '\u{2010}' => "-",  // ‐ HYPHEN (distinct from HYPHEN-MINUS)
        '\u{2011}' => "-",  // ‑ non-breaking hyphen
        '\u{2012}' => "-",  // ‒ figure dash
        '\u{2015}' => "--", // ― horizontal bar
        '\u{2053}' => "~",  // ⁓ swung dash

        // ── QUOTES AND BRACKETS NOT IN WINANSI ───────────────────────────
        '\u{201B}' => "'",  // ‛ single high-reversed-9
        '\u{201F}' => "\"", // ‟ double high-reversed-9
        '\u{2039}' => "<",  // ‹
        '\u{203A}' => ">",  // ›
        '\u{27E8}' => "<",  // ⟨ mathematical angle bracket
        '\u{27E9}' => ">",  // ⟩

        // ── SPACES ───────────────────────────────────────────────────────
        //
        // PDF extraction emits these constantly for justified text. Folding to
        // an ordinary space is exact: they differ only in width.
        '\u{2002}' | '\u{2003}' | '\u{2004}' | '\u{2005}' | '\u{2006}'
        | '\u{2007}' | '\u{2008}' | '\u{2009}' | '\u{200A}' | '\u{202F}'
        | '\u{205F}' => " ",
        '\u{200B}' | '\u{FEFF}' => "",  // zero-width space / BOM: emit nothing

        // ── FURTHER MATHS SEEN IN FETCHED PAPERS ─────────────────────────
        '\u{223C}' => "~",   // ∼ TILDE OPERATOR — "~1,500 tweets"
        '\u{223D}' => "~",   // ∽
        '\u{2243}' => "~=",  // ≃
        '\u{2245}' => "~=",  // ≅
        '\u{221D}' => "prop to", // ∝
        '\u{226A}' => "<<",  // ≪
        '\u{226B}' => ">>",  // ≫
        '\u{22C5}' => "*",   // ⋅ dot operator
        '\u{00D7}' => "x",   // × (WinAnsi has it, but "x" reads better in prose)
        '\u{2191}' => "^",   // ↑
        '\u{2193}' => "v",   // ↓
        '\u{2194}' => "<->", // ↔
        '\u{21D2}' => "=>",  // ⇒
        '\u{21D0}' => "<=",  // ⇐
        '\u{2208}' => "in",  // ∈
        '\u{2205}' => "{}",  // ∅

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
/// Characters that ARE representable, at a different codepoint.
///
/// GREEK SMALL LETTER MU and WinAnsi's MICRO SIGN are the same mark; a PDF may
/// carry either. Canonicalising BEFORE the WinAnsi lookup makes the result
/// `Intact` rather than `Simplified`, which is correct — nothing was altered,
/// so no fold disclosure is owed.
///
/// This is deliberately NOT part of `latin_base`, whose contract is that every
/// base it returns is ASCII: returning `"\u{00B5}"` there would push its two
/// UTF-8 bytes into a WinAnsi stream. Caught by a test asserting the encoded
/// byte, which the first attempt failed.
fn canonical(ch: char) -> char {
    match ch {
        '\u{03BC}' => '\u{00B5}', // μ → µ
        other => other,
    }
}

pub fn encode_winansi(s: &str) -> (Vec<u8>, EncodingOutcome) {
    let mut bytes = Vec::with_capacity(s.len());
    let mut outcome = EncodingOutcome::default();
    for ch in s.chars().map(canonical) {
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

/// Adobe Helvetica-BOLD AFM advance widths, same indexing as above.
///
/// # This table is the risk in the typography pass, and it is stated
///
/// The module previously used no bold face precisely because "Helvetica-Bold
/// needs its own AFM width table, and wrong metrics produce silently wrong
/// wrapping". These values are transcribed from the Adobe AFM. They are
/// self-consistent with the wrapper — every test that wraps bold text uses this
/// table — so a transcription error would NOT be caught by a test that measures
/// with the same numbers it set. `bold_is_never_narrower_than_regular` checks
/// the one independent invariant available without a rasteriser.
#[rustfmt::skip]
const HELVETICA_BOLD_WIDTHS: [u16; 256] = [
    0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0, 0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,
    278,333,474,556,556,889,722,238,333,333,389,584,278,333,278,278,
    556,556,556,556,556,556,556,556,556,556,333,333,584,584,584,611,
    975,722,722,722,722,667,611,778,722,278,556,722,611,833,722,778,
    667,778,722,667,611,722,667,944,667,667,611,333,278,333,584,556,
    333,556,611,556,611,556,333,611,611,278,278,556,278,889,611,611,
    611,611,389,556,333,611,556,778,556,556,500,389,280,389,584,0,
    556,0,278,556,500,1000,556,556,333,1000,667,333,1000,0,611,0,
    0,278,278,500,500,350,556,1000,333,1000,556,333,889,0,500,667,
    278,333,556,556,556,556,280,556,333,737,370,556,584,333,737,333,
    400,584,333,333,333,611,556,278,333,333,365,556,834,834,834,611,
    722,722,722,722,722,722,1000,722,667,667,667,667,278,278,278,278,
    722,722,778,778,778,778,778,584,778,722,722,722,722,667,667,611,
    556,556,556,556,556,556,889,556,556,556,556,556,278,278,278,278,
    611,611,611,611,611,611,611,584,611,611,611,611,611,556,611,556,
];

fn widths_for(face: Face) -> &'static [u16; 256] {
    match face {
        // Helvetica-Oblique's AFM advances are identical to Helvetica's.
        Face::Regular | Face::Oblique => &HELVETICA_WIDTHS,
        Face::Bold => &HELVETICA_BOLD_WIDTHS,
    }
}

fn text_width_in(bytes: &[u8], size: f64, face: Face) -> f64 {
    let w = widths_for(face);
    bytes.iter().map(|b| w[*b as usize] as f64).sum::<f64>() * size / 1000.0
}

/// One positioned line of already-encoded bytes.
#[derive(Clone)]
struct Line {
    bytes: Vec<u8>,
    face: Face,
    size: f64,
    /// Baseline-to-baseline distance for this line. Set from the type scale
    /// rather than derived from size — the single biggest defect in the
    /// previous output was leading equal to size, which reads as cramped.
    leading: f64,
    indent: f64,
    rgb: Rgb,
    /// Extra space above this line.
    lead: f64,
    /// Extra width added to every space on this line, for JUSTIFICATION
    /// (defect 5). PDF's `Tw` operator, so the spaces stretch and the glyphs
    /// do not: a line is justified by the gaps between words, never by
    /// distorting the letters.
    ///
    /// Zero on the LAST line of a paragraph and on anything that is not body
    /// copy. A justified last line is the classic giveaway of a tool that
    /// justified by rule rather than by typography.
    word_space: f64,
}

/// Background and rule treatment for a group of lines.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Decor {
    None,
    /// Shaded panel: PANEL fill, 11pt padding all four sides, full column.
    Panel,
    /// Quotation: QUOTE fill, 10pt padding, 2pt QRULE bar on the LEFT EDGE ONLY.
    Quote,
    /// A section heading followed by a rule of the given weight and colour.
    /// That single rule is what creates the hierarchy.
    RuleUnder(f64, Rgb),
    /// A drawn proportion (§11 D93): a track, a filled portion, and the count
    /// right-aligned in its own column. `f64` is value/total, already clamped.
    Bar(f64, Rgb),
    /// A verdict pill: a tinted fill sized to the TEXT, not to the column.
    ///
    /// Full-column is what `Panel` is for, and a verdict drawn that way reads
    /// as a section rather than as a label on one. A badge has to be scannable
    /// down the left edge of a page of findings, which means it has to end
    /// where its word ends.
    Pill(Rgb),
}

/// DEFECT 5: justify a run of body lines to the column.
///
/// Sets PDF word spacing per line so the spaces stretch and the glyphs keep
/// their drawn widths. The LAST line is left alone, which is the whole
/// difference between justified text and text that has been stretched by rule.
///
/// A line is left ragged when the stretch would be visible as a river: more
/// than `MAX_WORD_STRETCH` added to a single space reads worse than an uneven
/// right edge, and a line with one or no spaces cannot be stretched at all.
fn justify(lines: &mut [Line], column: f64) {
    /// Points of extra space per gap before ragged beats justified.
    const MAX_WORD_STRETCH: f64 = 2.6;
    let n = lines.len();
    if n < 2 {
        return;
    }
    for line in lines.iter_mut().take(n - 1) {
        let spaces = line.bytes.iter().filter(|b| **b == b' ').count();
        if spaces == 0 {
            continue;
        }
        let natural = text_width_in(&line.bytes, line.size, line.face);
        let slack = column - line.indent - natural;
        if slack <= 0.0 {
            continue;
        }
        let per_space = slack / spaces as f64;
        if per_space <= MAX_WORD_STRETCH {
            line.word_space = per_space;
        }
    }
}

/// The ink a semantic tone is drawn in. ONE definition, so a `Good` bar and a
/// `Good` chart segment cannot drift to different greens.
fn tone_ink(tone: Tone) -> Rgb {
    match tone {
        Tone::Good => rgb(0x1C, 0x73, 0x40),
        Tone::Warn => rgb(0xB5, 0x6B, 0x0D),
        Tone::Bad => rgb(0xB3, 0x21, 0x21),
        Tone::Neutral => rgb(0x6B, 0x6B, 0x6B),
    }
}

/// A rectangle placed relative to a group's own text top (defect 3).
///
/// The escape hatch that makes charts possible without teaching `Decor` a new
/// variant per chart type. The COMPOSER still supplies only facts: every number
/// here was computed by the renderer from a `StackedBar`'s counts.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Draw {
    /// From the left text margin.
    dx: f64,
    /// DOWN from the group's text top.
    dy: f64,
    w: f64,
    h: f64,
    rgb: Rgb,
}

/// Where a group sits in a table, so a table split across pages keeps its
/// header (defect 2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TablePart {
    /// The header row. Re-emitted at the top of each continuation page.
    Header,
    /// A data row. Triggers the re-emit when it lands on a fresh page.
    Row,
}

/// A group of lines drawn together, with its decoration. Groups are the unit
/// of pagination: a panel is never split across a page boundary.
#[derive(Clone)]
struct Elem {
    lines: Vec<Line>,
    decor: Decor,
    /// Rectangles drawn under this group's text: chart geometry.
    draws: Vec<Draw>,
    /// Set on table header and data rows only.
    table: Option<TablePart>,
    /// A page break requested by the composer. HONOURED unless the current page
    /// is nearly empty — see `paginate_and_write`.
    soft_break: bool,
    /// A break that must always happen: only the cover asks for one.
    hard_break: bool,
}

impl Elem {
    fn br(soft: bool, hard: bool) -> Self {
        Elem {
            lines: Vec::new(),
            decor: Decor::None,
            soft_break: soft,
            hard_break: hard,
            draws: Vec::new(),
            table: None,
        }
    }
    /// Total vertical space this group needs, decoration included.
    fn height(&self) -> f64 {
        let text: f64 = self.lines.iter().map(|l| l.lead + l.leading).sum();
        text + match self.decor {
            Decor::Panel => 22.0,
            Decor::Quote => 20.0 + 16.0,
            Decor::RuleUnder(..) => 11.0 + 6.0,
            Decor::Pill(_) => 3.0 + 5.0,
            Decor::Bar(..) => 6.0,
            Decor::None => 0.0,
        }
    }
}

/// Greedy word wrap on encoded bytes. Splits on ASCII space, which is exact:
/// every encoded byte is one character wide and `0x20` cannot occur inside a
/// multi-byte sequence — the encoding has none.
fn wrap(bytes: &[u8], size: f64, width: f64, face: Face) -> Vec<Vec<u8>> {
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
        if text_width_in(&trial, size, face) <= width || cur.is_empty() {
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

/// The type scale, in (size, leading) pairs. Named rather than inlined so the
/// specimen's numbers appear once.
mod scale {
    pub const COVER_BRAND: (f64, f64) = (11.5, 14.0);
    pub const COVER_SUBTITLE: (f64, f64) = (10.2, 14.0);
    pub const FIELD_LABEL: (f64, f64) = (7.4, 10.0);
    pub const FIELD_VALUE: (f64, f64) = (10.6, 15.0);
    pub const H1: (f64, f64) = (15.5, 20.0);
    pub const H2: (f64, f64) = (11.8, 16.0);
    pub const H3: (f64, f64) = (9.6, 14.0);
    pub const BODY: (f64, f64) = (10.2, 14.4);
    pub const SMALL: (f64, f64) = (9.0, 13.0);
    pub const QUOTATION: (f64, f64) = (9.6, 14.5);
    pub const FURNITURE: (f64, f64) = (7.4, 10.0);
}

/// Render composed blocks to PDF bytes.
/// §11 D149. TWO PASSES, because a contents page states page numbers.
///
/// A page number is not knowable until the document has been paginated, and
/// paginating it requires the contents page to already be there. So the
/// document is laid out twice: the first pass learns which page each section
/// heading landed on, the second prints those numbers into the contents.
///
/// It terminates, and the numbers it prints are the numbers of the document it
/// prints, because the two passes lay out IDENTICALLY. A contents entry's page
/// number is a right-aligned run on the entry's own line, measured into place
/// rather than padded, and a line that advances nothing vertically cannot
/// change where anything else falls. Absent (pass one) or present (pass two),
/// every element has the same height and lands on the same page.
///
/// The cost is one extra layout pass over a document of a few dozen pages.
pub fn render_pdf(blocks: &[Block]) -> Vec<u8> {
    let (elems, sections) = layout(blocks, &[]);
    let first = paginate(elems, &sections);
    let (elems, sections) = layout(blocks, &first.section_pages);
    let settled = paginate(elems, &sections);
    debug_assert_eq!(
        first.section_pages, settled.section_pages,
        "the two passes disagree about where the sections are, so the contents page \
         is printing numbers from a document that is not this one"
    );
    furnish_and_write(settled.pages, running_title(blocks))
}

/// What the running header calls this document (§11 D149).
///
/// From the cover, which is the composer saying what it composed. A block list
/// with no cover is a fragment (every test in this module renders one), and a
/// fragment is furnished with the product's name rather than a guess.
fn running_title(blocks: &[Block]) -> String {
    blocks
        .iter()
        .find_map(|b| match b {
            Block::Cover { running_title, .. } => Some(running_title.clone()),
            _ => None,
        })
        .unwrap_or_else(|| "Gaply report".to_string())
}

/// Lay every block out into positioned elements.
///
/// `section_pages` holds the page number of each level-1 heading, in order, or
/// is empty on the first pass. Returns the elements, and the INDEX of each
/// level-1 heading's element so the paginator can report where they landed.
fn layout(blocks: &[Block], section_pages: &[usize]) -> (Vec<Elem>, Vec<usize>) {
    let mut outcome = EncodingOutcome::default();
    let mut elems: Vec<Elem> = Vec::new();
    // Which elements are section headings, in document order.
    let mut section_elems: Vec<usize> = Vec::new();
    // The contents IS this list: the composer does not restate it, so the two
    // cannot drift (see `Block::Contents`).
    let section_titles: Vec<&str> = blocks
        .iter()
        .filter_map(|b| match b {
            Block::Heading { text, level: 1 } => Some(text.as_str()),
            _ => None,
        })
        .collect();

    // Build the lines for one run of text, wrapped to the column it sits in.
    let mut lay = |out: &mut EncodingOutcome,
                   text: &str,
                   (size, leading): (f64, f64),
                   face: Face,
                   indent: f64,
                   rgb: Rgb,
                   lead: f64,
                   column: f64|
     -> Vec<Line> {
        let (bytes, o) = encode_winansi(text);
        *out = EncodingOutcome {
            any_simplified: out.any_simplified || o.any_simplified,
            any_marked: out.any_marked || o.any_marked,
        };
        wrap(&bytes, size, column - indent, face)
            .into_iter()
            .enumerate()
            .map(|(i, l)| Line {
                bytes: l,
                face,
                size,
                leading,
                indent,
                rgb,
                lead: if i == 0 { lead } else { 0.0 },
                word_space: 0.0,
            })
            .collect()
    };

    for block in blocks {
        match block {
            // `running_title` is furniture, read by `running_title()` above.
            Block::Cover { title, subtitle, meta, headline, running_title: _ } => {
                // 2.2pt INK rule flush across the top margin, then 34mm.
                let mut lines = Vec::new();
                // §11 D93. Was 96.4pt, which marooned the cover block in the middle
                // of an otherwise empty A4 page — the reader's first impression
                // of the report was four-fifths white.
                lines.extend(lay(&mut outcome, subtitle, scale::COVER_BRAND, Face::Bold, 0.0, INK, 34.0, TEXT_W));
                lines.extend(lay(&mut outcome, title, scale::COVER_SUBTITLE, Face::Regular, 0.0, BODY, 2.0, TEXT_W));
                // §11 D93. The ANSWER, set large — page 1 used to carry five
                // metadata pairs and four-fifths white paper.
                if let Some((h, tone)) = headline {
                        let ink = match tone {
                        Tone::Good => rgb(0x1C, 0x73, 0x40),
                        Tone::Warn => rgb(0xB5, 0x6B, 0x0D),
                        Tone::Bad => rgb(0xB3, 0x21, 0x21),
                        Tone::Neutral => BODY,
                    };
                    lines.extend(lay(&mut outcome, h, scale::H1, Face::Bold, 0.0, ink, 26.0, TEXT_W));
                }
                elems.push(Elem { lines, decor: Decor::RuleUnder(0.9, INK), soft_break: false, hard_break: false, draws: Vec::new(), table: None });

                // Metadata as label/value pairs, 10pt between pairs.
                for (k, v) in meta {
                    let mut pair = lay(&mut outcome, &k.to_uppercase(), scale::FIELD_LABEL, Face::Bold, 0.0, FAINT, 10.0, TEXT_W);
                    pair.extend(lay(&mut outcome, v, scale::FIELD_VALUE, Face::Regular, 0.0, INK, 1.0, TEXT_W));
                    elems.push(Elem { lines: pair, decor: Decor::None, soft_break: false, hard_break: false, draws: Vec::new(), table: None });
                }
                elems.push(Elem::br(false, true));
            }
            Block::Heading { text, level } => {
                let (sc, face, rgb, lead, decor) = match level {
                    1 => (scale::H1, Face::Bold, INK, 24.0, Decor::RuleUnder(0.9, INK)),
                    2 => (scale::H2, Face::Bold, INK, 18.0, Decor::None),
                    _ => (scale::H3, Face::Bold, INK, 14.0, Decor::None),
                };
                let lines = lay(&mut outcome, text, sc, face, 0.0, rgb, lead, TEXT_W);
                if *level == 1 {
                    section_elems.push(elems.len());
                }
                elems.push(Elem { lines, decor, soft_break: false, hard_break: false, draws: Vec::new(), table: None });
            }
            Block::Paragraph { text } => {
                let mut lines = lay(&mut outcome, text, scale::BODY, Face::Regular, 0.0, BODY, 8.0, TEXT_W);
                // DEFECT 5. Body copy is justified; headings and cells are not.
                justify(&mut lines, TEXT_W);
                elems.push(Elem { lines, decor: Decor::None, soft_break: false, hard_break: false, draws: Vec::new(), table: None });
            }
            Block::Bullet { text, indent } => {
                if *indent >= 1 {
                    // The only indent-1 bullets the composer emits are
                    // quotations — the manuscript excerpt and the similarity
                    // excerpt. Both are set as quotations.
                    let lines = lay(
                        &mut outcome, text, scale::QUOTATION, Face::Oblique,
                        12.0, BODY, 8.0, TEXT_W - 20.0,
                    );
                    elems.push(Elem { lines, decor: Decor::Quote, soft_break: false, hard_break: false, draws: Vec::new(), table: None });
                } else {
                    // DEFECT 5. A real bullet (U+2022, WinAnsi 0x95), hung in
                    // the margin so the text edge stays straight. "- " is a
                    // hyphen pretending to be a list marker.
                    let mut lines = lay(
                        &mut outcome, &format!("\u{2022}  {text}"), scale::BODY, Face::Regular,
                        14.0, BODY, 4.0, TEXT_W,
                    );
                    justify(&mut lines, TEXT_W);
                    elems.push(Elem { lines, decor: Decor::None, soft_break: false, hard_break: false, draws: Vec::new(), table: None });
                }
            }
            Block::Note { text } => {
                let lines = lay(&mut outcome, text, scale::SMALL, Face::Regular, 11.0, MUTED, 12.0, TEXT_W - 22.0);
                elems.push(Elem { lines, decor: Decor::Panel, soft_break: false, hard_break: false, draws: Vec::new(), table: None });
            }
            // §11 D93. Label on the left, a DRAWN track in the middle, the
            // count and percent right-aligned — three columns that cannot
            // collide, where "=".repeat(n) plus two numbers always did.
            Block::Bar { label, value, total, tone } => {
                let ink = tone_ink(*tone);
                let frac = if *total == 0 { 0.0 } else { (*value as f64 / *total as f64).clamp(0.0, 1.0) };
                let pct = (frac * 100.0).round() as u32;
                // Two separate lines in ONE element: the label, and the count
                // placed at a measured offset. Neither is padded with spaces.
                let mut lines = lay(&mut outcome, label, scale::BODY, Face::Regular, 0.0, BODY, 0.0, BAR_LABEL_W);
                lines.truncate(1); // a label that wraps would break the row
                let count = format!("{value}  ({pct}%)");
                let mut right = lay(&mut outcome, &count, scale::SMALL, Face::Regular, 0.0, MUTED, 0.0, TEXT_W);
                if let Some(r) = right.first_mut() {
                    // Right-align by MEASURING, not by padding with spaces.
                    let w = text_width_in(&r.bytes, r.size, r.face);
                    r.indent = TEXT_W - w;
                    // Sits on the SAME row as the label: no vertical advance.
                    r.leading = 0.0;
                    r.lead = 0.0;
                }
                lines.append(&mut right);
                elems.push(Elem { lines, decor: Decor::Bar(frac, ink), soft_break: false, hard_break: false, draws: Vec::new(), table: None });
            }
            Block::Badge { text, tone } => {
                // Ink and tint, not one colour: coloured text on white is a
                // word that happens to be red. The fill is what makes it a
                // badge you can find by scanning rather than by reading.
                let (ink, tint) = match tone {
                    Tone::Good => (rgb(0x1C, 0x73, 0x40), rgb(0xEE, 0xF7, 0xF1)),
                    Tone::Warn => (rgb(0xB5, 0x6B, 0x0D), rgb(0xFD, 0xF5, 0xE9)),
                    Tone::Bad => (rgb(0xB3, 0x21, 0x21), rgb(0xFB, 0xED, 0xED)),
                    Tone::Neutral => (MUTED, rgb(0xF2, 0xF2, 0xF0)),
                };
                let lines = lay(
                    &mut outcome,
                    &text.to_uppercase(),
                    scale::FIELD_LABEL,
                    Face::Bold,
                    0.0,
                    ink,
                    8.0,
                    TEXT_W,
                );
                elems.push(Elem {
                    lines,
                    decor: Decor::Pill(tint),
                    soft_break: false,
                    hard_break: false,
                    draws: Vec::new(),
                    table: None,
                });
            }
            // ---- DEFECT 2: a real ruled table ------------------------------
            //
            // Expanded into ONE ELEM PER ROW rather than one per table, so a
            // long table paginates at a row boundary instead of being refused
            // as an over-tall atomic group. `TablePart` is what lets the header
            // come back at the top of the continuation page.
            Block::Table { caption, header, rows, align } => {
                let cols = header.len().max(rows.iter().map(|r| r.len()).max().unwrap_or(0));
                if cols == 0 {
                    continue;
                }
                // Column widths from CONTENT, not from an even split: a
                // "Sentences" column of two-digit numbers does not need a
                // third of the page, and the names do.
                let gutter = 10.0;
                let mut natural = vec![0.0f64; cols];
                for c in 0..cols {
                    let mut w: f64 = header
                        .get(c)
                        .map(|h| text_width_in(&encode_winansi(h).0, scale::SMALL.0, Face::Bold))
                        .unwrap_or(0.0);
                    for r in rows {
                        if let Some(cell) = r.get(c) {
                            let cw = text_width_in(
                                &encode_winansi(cell).0, scale::SMALL.0, Face::Regular,
                            );
                            if cw > w {
                                w = cw;
                            }
                        }
                    }
                    natural[c] = w;
                }
                let avail = TEXT_W - gutter * (cols as f64 - 1.0);
                let total_natural: f64 = natural.iter().sum();
                let widths: Vec<f64> = if total_natural <= avail {
                    // Everything fits: give the slack to the FIRST left column,
                    // which is the one holding names that may still be long.
                    let mut w = natural.clone();
                    let slack = avail - total_natural;
                    let grow = align
                        .iter()
                        .position(|a| *a == Align::Left)
                        .unwrap_or(0);
                    w[grow] += slack;
                    w
                } else {
                    // Over-wide: numeric columns keep their natural width (they
                    // are short and must not wrap), and the text columns share
                    // what is left. This is what stops a long title from
                    // squeezing a count column to nothing.
                    let fixed: f64 = (0..cols)
                        .filter(|c| align.get(*c) == Some(&Align::Right))
                        .map(|c| natural[c])
                        .sum();
                    // Flex columns share what is left IN PROPORTION to what
                    // they need. An even split gave a column of paragraph
                    // numbers the same width as a column of whole sentences,
                    // so one wrapped to six lines while the other wasted two
                    // thirds of its space.
                    // A column of short labels ("Hadley, 2002") is effectively
                    // fixed: squeezing it wraps a name onto two lines and buys
                    // the long column almost nothing.
                    const SHORT_COL: f64 = 86.0;
                    let is_flex = |c: usize| {
                        align.get(c) != Some(&Align::Right) && natural[c] > SHORT_COL
                    };
                    let fixed = fixed
                        + (0..cols)
                            .filter(|c| align.get(*c) != Some(&Align::Right) && !is_flex(*c))
                            .map(|c| natural[c])
                            .sum::<f64>();
                    let flex_natural: f64 = (0..cols).filter(|c| is_flex(*c)).map(|c| natural[c]).sum();
                    let flex_avail = (avail - fixed).max(60.0);
                    (0..cols)
                        .map(|c| {
                            if !is_flex(c) {
                                natural[c]
                            } else if flex_natural <= 0.0 {
                                flex_avail
                            } else {
                                (flex_avail * natural[c] / flex_natural).max(46.0)
                            }
                        })
                        .collect()
                };
                let mut xs = Vec::with_capacity(cols);
                let mut x = 0.0;
                for w in &widths {
                    xs.push(x);
                    x += w + gutter;
                }

                if let Some(c) = caption {
                    let lines = lay(&mut outcome, c, scale::SMALL, Face::Regular, 0.0, MUTED, 12.0, TEXT_W);
                    elems.push(Elem { lines, decor: Decor::None, soft_break: false, hard_break: false, draws: Vec::new(), table: None });
                }

                // One row -> one Elem. Cells that wrap make the row taller;
                // every cell in a row starts at the SAME baseline because only
                // the first line of the row carries the vertical advance.
                let mut row_elem = |out: &mut EncodingOutcome,
                                    cells: &[String],
                                    face: Face,
                                    colour: Rgb,
                                    part: TablePart|
                 -> Elem {
                    let pad_top = 5.0;
                    let leading = scale::SMALL.1;
                    let mut wrapped: Vec<Vec<Vec<u8>>> = Vec::with_capacity(cols);
                    for c in 0..cols {
                        let text = cells.get(c).map(String::as_str).unwrap_or("");
                        let (bytes, o) = encode_winansi(text);
                        *out = EncodingOutcome {
                            any_simplified: out.any_simplified || o.any_simplified,
                            any_marked: out.any_marked || o.any_marked,
                        };
                        wrapped.push(wrap(&bytes, scale::SMALL.0, widths[c], face));
                    }
                    let depth = wrapped.iter().map(Vec::len).max().unwrap_or(1).max(1);
                    let mut lines: Vec<Line> = Vec::new();
                    for r in 0..depth {
                        // A zero-width carrier holds the row-line's advance, so
                        // a short cell in column 0 cannot swallow it.
                        lines.push(Line {
                            bytes: Vec::new(),
                            face,
                            size: scale::SMALL.0,
                            leading,
                            indent: 0.0,
                            rgb: colour,
                            lead: if r == 0 { pad_top } else { 0.0 },
                            word_space: 0.0,
                        });
                        for c in 0..cols {
                            let Some(bytes) = wrapped[c].get(r) else { continue };
                            let w = text_width_in(bytes, scale::SMALL.0, face);
                            let indent = if align.get(c) == Some(&Align::Right) {
                                // Right-aligned by MEASURING to the column's
                                // right edge: digits line up under digits.
                                xs[c] + widths[c] - w
                            } else {
                                xs[c]
                            };
                            lines.push(Line {
                                bytes: bytes.clone(),
                                face,
                                size: scale::SMALL.0,
                                leading: 0.0,
                                indent,
                                rgb: colour,
                                lead: 0.0,
                                word_space: 0.0,
                            });
                        }
                    }
                    // Bottom padding as a real carrier line, so `Elem::height`
                    // reserves it and the next row cannot ride up into it.
                    const PAD_BOTTOM: f64 = 4.0;
                    lines.push(Line {
                        bytes: Vec::new(),
                        face,
                        size: scale::SMALL.0,
                        leading: PAD_BOTTOM,
                        indent: 0.0,
                        rgb: colour,
                        lead: 0.0,
                        word_space: 0.0,
                    });
                    let height = pad_top + leading * depth as f64 + PAD_BOTTOM;
                    // The rule UNDER the row: heavy under the header (it
                    // separates head from body), a hairline between data rows.
                    let (rule_h, rule_c) = match part {
                        TablePart::Header => (1.1, INK),
                        TablePart::Row => (0.4, HAIR),
                    };
                    Elem {
                        lines,
                        decor: Decor::None,
                        soft_break: false,
                        hard_break: false,
                        draws: vec![Draw { dx: 0.0, dy: height - 1.0, w: TEXT_W, h: rule_h, rgb: rule_c }],
                        table: Some(part),
                    }
                };

                let e = row_elem(&mut outcome, header, Face::Bold, INK, TablePart::Header);
                elems.push(e);
                for row in rows {
                    let e = row_elem(&mut outcome, row, Face::Regular, BODY, TablePart::Row);
                    elems.push(e);
                }
            }
            // ---- DEFECT 3: charts -----------------------------------------
            //
            // One bar cut into parts that sum to the whole, with a legend that
            // NAMES each colour. Colour never carries the meaning alone: the
            // legend prints the label and the count beside its swatch, so the
            // chart survives a monochrome print and a colour-blind reader.
            Block::StackedBar { title, segments } => {
                let total: usize = segments.iter().map(|(_, n, _)| *n).sum();
                let bar_h = 14.0;
                let gap_above = 6.0;
                let mut lines = lay(&mut outcome, title, scale::H3, Face::Bold, 0.0, INK, 14.0, TEXT_W);
                let axis = format!(
                    "Whole bar = {total} sentences read. Each part is labelled below."
                );
                lines.extend(lay(&mut outcome, &axis, scale::SMALL, Face::Regular, 0.0, MUTED, 3.0, TEXT_W));
                // Vertical offset of the bar: the sum of what the text above it
                // advanced, plus a gap.
                let mut dy: f64 = lines.iter().map(|l| l.lead + l.leading).sum();
                dy += gap_above;
                // A blank line reserves the bar's own height so the legend does
                // not sit on top of it.
                lines.push(Line {
                    bytes: Vec::new(),
                    face: Face::Regular,
                    size: scale::SMALL.0,
                    leading: bar_h + gap_above + 4.0,
                    indent: 0.0,
                    rgb: BODY,
                    lead: 0.0,
                    word_space: 0.0,
                });
                let mut draws = vec![Draw { dx: 0.0, dy, w: TEXT_W, h: bar_h, rgb: PANEL }];
                let mut x = 0.0;
                for (_, n, tone) in segments {
                    if total == 0 {
                        break;
                    }
                    let w = TEXT_W * (*n as f64 / total as f64);
                    if w > 0.0 {
                        draws.push(Draw { dx: x, dy, w: w.max(1.0), h: bar_h, rgb: tone_ink(*tone) });
                    }
                    x += w;
                }
                // The legend: a swatch per part, with its label and count.
                let swatch = 8.0;
                for (label, n, tone) in segments {
                    let pct = if total == 0 { 0 } else { ((*n as f64 / total as f64) * 100.0).round() as u32 };
                    let text = format!("{label}: {n} ({pct}%)");
                    let mut l = lay(&mut outcome, &text, scale::SMALL, Face::Regular, swatch + 7.0, BODY, 3.0, TEXT_W);
                    let row_dy: f64 = lines.iter().map(|x| x.lead + x.leading).sum::<f64>()
                        + l.first().map(|f| f.lead + f.leading).unwrap_or(0.0);
                    draws.push(Draw {
                        dx: 0.0,
                        // Aligned to the legend line's own baseline, 1pt under
                        // it, so the swatch sits on the text rather than above.
                        dy: row_dy - swatch + 1.0,
                        w: swatch,
                        h: swatch,
                        rgb: tone_ink(*tone),
                    });
                    lines.append(&mut l);
                    let _ = tone;
                }
                elems.push(Elem { lines, decor: Decor::None, soft_break: false, hard_break: false, draws, table: None });
            }
            // Horizontal bars, reusing the row geometry `Bar` already has:
            // label, drawn track, value. Both axes are named in words above the
            // chart, because "top ten" is meaningless without saying ten of what.
            Block::BarChart { title, category_axis, value_axis, bars } => {
                let max = bars.iter().map(|(_, n)| *n).max().unwrap_or(0);
                let lines = lay(&mut outcome, title, scale::H3, Face::Bold, 0.0, INK, 16.0, TEXT_W);
                elems.push(Elem { lines, decor: Decor::None, soft_break: false, hard_break: false, draws: Vec::new(), table: None });
                let axis = format!("Each row is {category_axis}. Bar length is {value_axis}.");
                let lines = lay(&mut outcome, &axis, scale::SMALL, Face::Regular, 0.0, MUTED, 3.0, TEXT_W);
                elems.push(Elem { lines, decor: Decor::None, soft_break: false, hard_break: false, draws: Vec::new(), table: None });
                for (label, n) in bars {
                    let frac = if max == 0 { 0.0 } else { (*n as f64 / max as f64).clamp(0.0, 1.0) };
                    let mut lines = lay(&mut outcome, label, scale::SMALL, Face::Regular, 0.0, BODY, 0.0, BAR_LABEL_W);
                    lines.truncate(1);
                    let mut right = lay(&mut outcome, &n.to_string(), scale::SMALL, Face::Regular, 0.0, MUTED, 0.0, TEXT_W);
                    if let Some(r) = right.first_mut() {
                        let w = text_width_in(&r.bytes, r.size, r.face);
                        r.indent = TEXT_W - w;
                        r.leading = 0.0;
                        r.lead = 0.0;
                    }
                    lines.append(&mut right);
                    elems.push(Elem {
                        lines,
                        decor: Decor::Bar(frac, rgb(0x46, 0x62, 0x8C)),
                        soft_break: false,
                        hard_break: false,
                        draws: Vec::new(),
                        table: None,
                    });
                }
                // The value scale, stated rather than drawn as ticks: at this
                // size a tick row is noise, and the largest value is what a
                // reader needs to read the lengths against.
                let scale_note = format!("Longest bar = {max}.");
                let lines = lay(&mut outcome, &scale_note, scale::SMALL, Face::Regular, BAR_LABEL_W, MUTED, 4.0, TEXT_W);
                elems.push(Elem { lines, decor: Decor::None, soft_break: false, hard_break: false, draws: Vec::new(), table: None });
            }
            Block::Contents { title } => {
                let lines = lay(&mut outcome, title, scale::H2, Face::Bold, 0.0, INK, 20.0, TEXT_W);
                elems.push(Elem { lines, decor: Decor::None, soft_break: false, hard_break: false, draws: Vec::new(), table: None });
                for (i, t) in section_titles.iter().enumerate() {
                    // The entry, then its page number right-aligned on the same
                    // line. The number is measured into place and advances
                    // nothing, which is what lets the two passes agree.
                    let mut lines = lay(
                        &mut outcome, t, scale::BODY, Face::Regular, 0.0, BODY, 5.0,
                        TEXT_W - CONTENTS_NUM_W,
                    );
                    lines.truncate(1);
                    if let Some(n) = section_pages.get(i).filter(|n| **n > 0) {
                        let mut right = lay(
                            &mut outcome, &n.to_string(), scale::BODY, Face::Regular, 0.0,
                            MUTED, 0.0, TEXT_W,
                        );
                        if let Some(r) = right.first_mut() {
                            let w = text_width_in(&r.bytes, r.size, r.face);
                            r.indent = TEXT_W - w;
                            r.leading = 0.0;
                            r.lead = 0.0;
                        }
                        lines.append(&mut right);
                    }
                    elems.push(Elem { lines, decor: Decor::None, soft_break: false, hard_break: false, draws: Vec::new(), table: None });
                }
            }
            Block::PageBreak => elems.push(Elem::br(true, false)),
        }
    }

    // The renderer discloses its OWN limitations. It is not deciding anything
    // about the content it was handed — it is reporting what it could not do
    // with it, which nothing upstream is in a position to know.
    let (simplified, marked) = (outcome.any_simplified, outcome.any_marked);
    let mut scratch = EncodingOutcome::default();
    if simplified {
        let lines = lay(&mut scratch, NOTE_SIMPLIFIED, scale::FURNITURE, Face::Regular, 11.0, MUTED, 18.0, TEXT_W - 22.0);
        elems.push(Elem { lines, decor: Decor::Panel, soft_break: false, hard_break: false, draws: Vec::new(), table: None });
    }
    if marked {
        let lines = lay(&mut scratch, NOTE_MARKED, scale::FURNITURE, Face::Regular, 11.0, MUTED, 8.0, TEXT_W - 22.0);
        elems.push(Elem { lines, decor: Decor::Panel, soft_break: false, hard_break: false, draws: Vec::new(), table: None });
    }
    debug_assert_eq!(scratch, EncodingOutcome::default(), "disclosure notes must be ASCII");

    (elems, section_elems)
}

/// A page break is honoured only when the page already carries real content.
///
/// Two sections — "Text similarity" with no corpus, and "What was not
/// examined" — each carry a single paragraph, and each was starting a fresh
/// page. The renderer cannot identify a section (it has no content knowledge),
/// so the rule is stated in terms it CAN see: a break that would leave a nearly
/// empty page behind is spent as vertical space instead. It applies uniformly.
const SOFT_BREAK_MIN_FILL: f64 = 0.45;

/// The right-hand column a contents entry leaves for its page number, so a long
/// section name wraps before it collides with the digits (§11 D149).
const CONTENTS_NUM_W: f64 = 34.0;

/// A paginated document, and where each level-1 heading landed (§11 D149).
struct Paginated {
    pages: Vec<Vec<u8>>,
    /// One 1-based page number per section heading, in document order.
    section_pages: Vec<usize>,
}

/// `section_elems` holds indices into `elems` whose page number the caller
/// wants back: the contents page is built from them.
fn paginate(elems: Vec<Elem>, section_elems: &[usize]) -> Paginated {
    let mut section_pages: Vec<usize> = vec![0; section_elems.len()];
    let top = PAGE_H - MARGIN_Y - HEADER_H;
    let bottom = MARGIN_Y + FOOTER_H;
    let usable = top - bottom;

    let mut pages: Vec<Vec<u8>> = Vec::new();
    let mut stream: Vec<u8> = Vec::new();
    // The page's TEXT, kept apart from its decoration until the page is closed
    // (§11 D93).
    let mut text_stream: Vec<u8> = Vec::new();
    let mut y = top;
    #[allow(unused_assignments)]
    let mut page_empty = true;
    // DEFECT 2: a table that spills onto the next page takes its header with
    // it. Without this a reader meets a grid of unlabelled columns, which is
    // worse than prose.
    let mut table_header: Option<Elem> = None;
    let mut header_on_page = false;

    macro_rules! new_page {
        () => {{
            // §11 D93. Decoration for the WHOLE page first, then all its
            // text. Drawing them interleaved meant block N+1's panel fill
            // painted over block N's descenders — the letters were not clipped
            // by a box, they were covered by the next one.
            let mut page = std::mem::take(&mut stream);
            page.extend_from_slice(&std::mem::take(&mut text_stream));
            pages.push(page);
            y = top;
            page_empty = true;
            header_on_page = false;
        }};
    }

    // The index rides with the element because a table row can be pushed back
    // onto the queue behind a repeated header, and it is still the same
    // element when it comes round again.
    const NOT_AN_ORIGINAL: usize = usize::MAX;
    let mut queue: std::collections::VecDeque<(usize, Elem)> =
        elems.into_iter().enumerate().collect();
    while let Some((idx, el)) = queue.pop_front() {
        // Remember the live header, and re-issue it when a data row opens a
        // page without one.
        match el.table {
            Some(TablePart::Header) => {
                table_header = Some(el.clone());
                header_on_page = true;
            }
            // The re-emit cannot be decided HERE: whether this row opens a
            // new page is not known until its height is measured below. Doing
            // it here put the repeated header UNDER the first row of the
            // continuation page, which is worse than no header at all.
            Some(TablePart::Row) => {}
            None => {
                // A non-table block ends the table: the next row starts a new
                // one and must print its own header.
                if !el.soft_break && !el.hard_break && !el.lines.is_empty() {
                    table_header = None;
                }
            }
        }
        if el.hard_break {
            if !page_empty {
                new_page!();
            }
            continue;
        }
        if el.soft_break {
            let filled = (top - y) / usable;
            if !page_empty && filled >= SOFT_BREAK_MIN_FILL {
                new_page!();
            } else if !page_empty {
                y -= 18.0; // spend the break as space instead
            }
            continue;
        }
        if el.lines.is_empty() {
            continue;
        }

        // Groups are atomic: a panel or quotation is never split.
        let h = el.height();
        if !page_empty && y - h < bottom {
            new_page!();
        }
        // NOW the page is settled. A data row that finds itself on a page with
        // no header puts the header back in front of itself and returns.
        if matches!(el.table, Some(TablePart::Row)) && !header_on_page {
            if let Some(hd) = table_header.clone() {
                queue.push_front((idx, el));
                queue.push_front((NOT_AN_ORIGINAL, hd));
                continue;
            }
        }

        // The page is settled: this element is about to be drawn on it. A
        // 1-based number, because it is what the reader will see printed in
        // the footer.
        if let Some(k) = section_elems.iter().position(|&i| i == idx) {
            section_pages[k] = pages.len() + 1;
        }

        let block_top = y;
        let pad = match el.decor {
            Decor::Panel => 11.0,
            Decor::Quote => 10.0,
            Decor::Pill(_) => 3.0,
            _ => 0.0,
        };
        if pad > 0.0 {
            y -= pad;
        }
        let text_top = y;
        let mut body = Vec::new();
        for line in &el.lines {
            y -= line.lead + line.leading;
            if line.bytes.is_empty() {
                // A carrier line: it holds vertical space (a table row's
                // advance, a chart's reserved bar) and draws nothing.
                continue;
            }
            body.extend_from_slice(
                format!(
                    "{:.3} {:.3} {:.3} rg\nBT\n{} {:.2} Tf\n{:.3} Tw\n1 0 0 1 {:.2} {:.2} Tm\n(",
                    line.rgb[0], line.rgb[1], line.rgb[2],
                    line.face.resource(), line.size,
                    line.word_space,
                    MARGIN_X + line.indent, y
                )
                .as_bytes(),
            );
            body.extend_from_slice(&escape_pdf(&line.bytes));
            body.extend_from_slice(b") Tj\nET\n");
        }
        // DEFECT 3: chart geometry, positioned from this group's text top.
        // Drawn into the DECORATION stream so the page's text still lands on
        // top of every fill (§11 D93's ordering).
        for d in &el.draws {
            stream.extend_from_slice(&fill_rect(
                MARGIN_X + d.dx,
                text_top - d.dy - d.h,
                d.w,
                d.h,
                d.rgb,
            ));
        }
        let text_bottom = y;
        if pad > 0.0 {
            y -= pad;
        }

        // Decoration is drawn FIRST so text sits on top of any fill.
        match el.decor {
            Decor::Panel => {
                stream.extend_from_slice(&fill_rect(MARGIN_X, y, TEXT_W, block_top - y, PANEL));
            }
            Decor::Quote => {
                stream.extend_from_slice(&fill_rect(MARGIN_X, y, TEXT_W, block_top - y, QUOTE_BG));
                stream.extend_from_slice(&fill_rect(MARGIN_X, y, 2.0, block_top - y, QRULE));
            }
            Decor::RuleUnder(w, c) => {
                let _ = (text_top, text_bottom);
                y -= 11.0;
                stream.extend_from_slice(&fill_rect(MARGIN_X, y, TEXT_W, w, c));
                y -= 6.0;
            }
            // §11 D93. The track, then the filled proportion. Drawn on the
            // row's own baseline so the label, the bar and the count line up.
            Decor::Bar(frac, ink) => {
                let track_x = MARGIN_X + BAR_LABEL_W;
                let track_w = TEXT_W - BAR_LABEL_W - BAR_COUNT_W;
                // 2pt under the baseline puts the bar optically on the line
                // rather than floating above it.
                let by = text_bottom - 2.0;
                stream.extend_from_slice(&fill_rect(track_x, by, track_w, BAR_TRACK_H, PANEL));
                let filled = track_w * frac;
                if filled > 0.0 {
                    stream.extend_from_slice(&fill_rect(track_x, by, filled.max(1.5), BAR_TRACK_H, ink));
                }
            }
            Decor::Pill(tint) => {
                // Widest line, so a two-word verdict is not clipped. Padded 6pt
                // each side; the extra 2pt of height sits under the baseline so
                // descenders are not cut by the fill's lower edge.
                let w = el
                    .lines
                    .iter()
                    .map(|l| text_width_in(&l.bytes, l.size, l.face))
                    .fold(0.0_f64, f64::max);
                stream.extend_from_slice(&fill_rect(
                    MARGIN_X - 6.0,
                    y - 2.0,
                    w + 12.0,
                    block_top - y,
                    tint,
                ));
                y -= 5.0;
            }
            Decor::None => {}
        }
        text_stream.extend_from_slice(&body);
        page_empty = false;
    }
    if !page_empty {
        let mut page = stream;
        page.extend_from_slice(&text_stream);
        pages.push(page);
    }
    if pages.is_empty() {
        pages.push(Vec::new());
    }

    Paginated { pages, section_pages }
}

/// Draw the running header and footer on every page, and write the file.
fn furnish_and_write(pages: Vec<Vec<u8>>, running_title: String) -> Vec<u8> {
    let total = pages.len();
    let furnished: Vec<Vec<u8>> = pages
        .into_iter()
        .enumerate()
        .map(|(i, mut content)| {
            let mut page = Vec::new();
            // 2.2pt INK rule flush across the top margin of the cover only.
            if i == 0 {
                page.extend_from_slice(&fill_rect(MARGIN_X, PAGE_H - MARGIN_Y, TEXT_W, 2.2, INK));
            } else {
                // §11 D149. Was the hard-coded "PublishReady report", printed
                // on all eighteen pages of a document called "Thesis citation
                // audit". A renderer cannot know what it is rendering, so it
                // stopped guessing: the composer says, on the cover.
                page.extend_from_slice(&furniture_text(&running_title, MARGIN_X, PAGE_H - MARGIN_Y - 10.0));
                page.extend_from_slice(&fill_rect(MARGIN_X, PAGE_H - MARGIN_Y - 15.0, TEXT_W, 0.5, HAIR));
            }
            // The FOOTER names the product, which is true of every report this
            // renderer draws. The header names this one.
            page.extend_from_slice(&furniture_text("Gaply", MARGIN_X, MARGIN_Y));
            let n = format!("{}", i + 1);
            let (bytes, _) = encode_winansi(&n);
            let w = text_width_in(&bytes, scale::FURNITURE.0, Face::Regular);
            page.extend_from_slice(&furniture_text(&n, MARGIN_X + TEXT_W - w, MARGIN_Y));
            page.append(&mut content);
            page
        })
        .collect();
    let _ = total;

    write_pdf(&furnished)
}

/// A filled rectangle in the current palette colour.
fn fill_rect(x: f64, y: f64, w: f64, h: f64, c: Rgb) -> Vec<u8> {
    format!("{:.3} {:.3} {:.3} rg\n{x:.2} {y:.2} {w:.2} {h:.2} re f\n", c[0], c[1], c[2])
        .into_bytes()
}

/// One line of running header/footer text, always FAINT at the furniture size.
fn furniture_text(text: &str, x: f64, y: f64) -> Vec<u8> {
    let (bytes, _) = encode_winansi(text);
    let mut out = format!(
        "{:.3} {:.3} {:.3} rg\nBT\n/F1 {:.2} Tf\n1 0 0 1 {x:.2} {y:.2} Tm\n(",
        FAINT[0], FAINT[1], FAINT[2], scale::FURNITURE.0
    )
    .into_bytes();
    out.extend_from_slice(&escape_pdf(&bytes));
    out.extend_from_slice(b") Tj\nET\n");
    out
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
/// then two objects per page (page dict, content stream). Objects 3-5 are the
/// three base-14 faces: Helvetica, Helvetica-Bold, Helvetica-Oblique.
fn write_pdf(pages: &[Vec<u8>]) -> Vec<u8> {
    let n = pages.len();
    let mut objs: Vec<Vec<u8>> = Vec::new();

    let kids: Vec<String> = (0..n).map(|i| format!("{} 0 R", 6 + i * 2)).collect();
    objs.push(b"<< /Type /Catalog /Pages 2 0 R >>".to_vec());
    objs.push(format!("<< /Type /Pages /Kids [{}] /Count {n} >>", kids.join(" ")).into_bytes());
    objs.push(
        b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding >>"
            .to_vec(),
    );
    objs.push(
        b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica-Bold /Encoding /WinAnsiEncoding >>"
            .to_vec(),
    );
    objs.push(
        b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica-Oblique /Encoding /WinAnsiEncoding >>"
            .to_vec(),
    );
    for (i, content) in pages.iter().enumerate() {
        objs.push(
            format!(
                "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {PAGE_W:.0} {PAGE_H:.0}] \
                 /Resources << /Font << /F1 3 0 R /F2 4 0 R /F3 5 0 R >> >> /Contents {} 0 R >>",
                7 + i * 2
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
            also_nearby: Vec::new(),
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
            recommendation: None,
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
                    is_threshold: false,
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
                source_span: None,
                article_type: None,
                checked_field: None,
                unevaluable: false,
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
    /// §11 D70. The REAL strings that broke the table, taken verbatim from the
    /// first open-access paper ever pulled through the OA path.
    ///
    /// The Greek/maths entries below were added from examples I wrote; these
    /// two characters — a ligature and a tilde operator — appeared in the first
    /// real fetched PDF and neither was covered. A ligature is purely
    /// presentational, so folding it is lossless; a `?` in the middle of a word
    /// hides which word was written, which is the one outcome a reader cannot
    /// interpret.
    #[test]
    fn ligatures_and_typography_from_real_fetched_papers_survive() {
        // VERBATIM from SemEval-2018 via OpenAlex, chunks c469 and c474.
        let blocks = vec![Block::Paragraph {
            text: "The multi-label emotion classi\u{FB01}cation dataset included \u{223C}1,500                    tweets \u{2010} a\u{2009}sample \u{2014} of the o\u{FB03}cial set."
                .to_string(),
        }];
        let text = squash(&text_from_pdf(&render_pdf(&blocks)));

        assert!(text.contains("classification"), "fi-ligature lost:\n{text}");
        assert!(text.contains("officialset") || text.contains("official"), "ffi lost:\n{text}");
        assert!(text.contains("~1,500"), "tilde operator lost:\n{text}");
        assert!(!text.contains('?'), "a character still folded to `?`:\n{text}");
        // Nothing was LOST, so the marked-character disclosure must not fire.
        assert!(!text.contains(&squash(NOTE_MARKED)), "claimed a loss that did not happen");
    }

    /// Every ligature and space fold, exhaustively — these are the ones a
    /// typesetter emits and a synthetic fixture never contains.
    #[test]
    fn every_ligature_folds_to_its_letters() {
        for (ch, want) in [
            ('\u{FB00}', "ff"), ('\u{FB01}', "fi"), ('\u{FB02}', "fl"),
            ('\u{FB03}', "ffi"), ('\u{FB04}', "ffl"),
        ] {
            assert_eq!(latin_base(ch), Some(want), "ligature U+{:04X}", ch as u32);
        }
        // Width-only spaces fold to an ordinary one; zero-width ones vanish.
        assert_eq!(latin_base('\u{2009}'), Some(" "));
        assert_eq!(latin_base('\u{202F}'), Some(" "));
        assert_eq!(latin_base('\u{200B}'), Some(""));
        // En/em dash and curly quotes are WinAnsi code points already and need
        // NO fold — asserting that keeps someone from adding a redundant one.
        assert_eq!(latin_base('\u{2013}'), None);
        assert_eq!(latin_base('\u{2014}'), None);
        assert_eq!(latin_base('\u{201C}'), None);
    }

    #[test]
    fn greek_and_maths_are_spelled_out_rather_than_becoming_question_marks() {
        // A methods section is full of these — "the objective function J(θ)",
        // "search space Ω", "p = σ(x)". WinAnsi has none of them, so before the
        // fold table covered them every one rendered as `?`, which is the one
        // outcome a reader cannot interpret: they cannot tell a lost symbol
        // from a symbol the author never wrote.
        let blocks = vec![Block::Paragraph {
            text: "The objective J(θ) over Ω with p = σ(x), α ≈ β, and ∑ √n.".into(),
        }];
        let pdf = render_pdf(&blocks);
        let text = squash(&text_from_pdf(&pdf));
        for word in ["theta", "Omega", "sigma", "alpha", "beta", "sum", "sqrt"] {
            assert!(text.contains(word), "{word} was not spelled out:\n{text}");
        }
        assert!(!text.contains('?'), "a character still folded to `?`:\n{text}");
        // And the reader is told the substitution happened.
        assert!(text.contains(&squash(NOTE_SIMPLIFIED)), "no disclosure:\n{text}");
        assert!(!text.contains(&squash(NOTE_MARKED)), "claimed a loss that did not happen");
    }

    #[test]
    fn folding_alone_discloses_only_simplification() {
        let m = model(Some("Study by Łukasz"), vec![finding("f1", "Ordinary title", FindingSeverity::Minor)]);
        let text = squash(&render(&m));
        assert!(text.contains("Lukasz"), "Ł must fold to L: the case NFD cannot handle");
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

// ============================================================================
// TYPOGRAPHY PASS — what is assertable without seeing the page
// ============================================================================

#[cfg(test)]
mod layout_tests {
    use super::*;
    use crate::report_compose::{compose, Block};

    /// Every drawing operation the renderer emits, parsed back out of the
    /// content streams — the only way to check geometry without a rasteriser.
    fn ops(pdf: &[u8]) -> (Vec<(f64, f64, f64, f64)>, Vec<(f64, f64, f64)>) {
        let text = String::from_utf8_lossy(pdf);
        let mut rects = Vec::new();
        let mut tms = Vec::new();
        for line in text.lines() {
            if let Some(rest) = line.strip_suffix(" re f") {
                let n: Vec<f64> = rest.split_whitespace().filter_map(|t| t.parse().ok()).collect();
                if n.len() == 4 {
                    rects.push((n[0], n[1], n[2], n[3]));
                }
            }
            if line.starts_with("1 0 0 1 ") && line.ends_with(" Tm") {
                let n: Vec<f64> = line.split_whitespace().filter_map(|t| t.parse().ok()).collect();
                if n.len() >= 6 {
                    tms.push((n[4], n[5], 0.0));
                }
            }
        }
        (rects, tms)
    }

    /// Each page's content stream, in page order.
    ///
    /// The renderer writes one uncompressed stream per page and no other
    /// streams, so this is the page structure rather than a guess at it. It is
    /// what lets a test ask WHICH PAGE something landed on (§11 D149).
    fn page_streams(pdf: &[u8]) -> Vec<String> {
        let text = String::from_utf8_lossy(pdf).into_owned();
        // Split on the dictionary that introduces a content stream, never on
        // the bare keyword: "endstream" ends with it too, and splitting there
        // returns the object trailer as if it were a page.
        text.split(">>\nstream\n")
            .skip(1)
            .filter_map(|chunk| chunk.split("endstream").next().map(str::to_string))
            .collect()
    }

    /// The strings a page draws, in drawing order, each with the font resource
    /// it was drawn in. The font is what tells a SECTION HEADING (bold) from
    /// the contents entry naming it (regular), which are the same characters.
    fn drawn(page: &str) -> Vec<(String, String)> {
        let mut face = String::new();
        let mut out = Vec::new();
        for l in page.lines() {
            if l.ends_with(" Tf") {
                face = l.split_whitespace().next().unwrap_or_default().to_string();
            }
            if let Some(t) = l.strip_suffix(") Tj") {
                out.push((face.clone(), t.trim_start_matches('(').to_string()));
            }
        }
        out
    }

    /// The strings a page draws, in drawing order, as they were written into
    /// the content stream.
    fn drawn_text(page: &str) -> Vec<String> {
        drawn(page).into_iter().map(|(_, t)| t).collect()
    }

    /// §11 D149. THE RUNNING HEADER NAMES THIS DOCUMENT.
    ///
    /// It was the literal "PublishReady report", drawn on every page after the
    /// cover, including all eighteen pages of a document titled "Thesis
    /// citation audit". The renderer cannot know what it is rendering, so the
    /// composer says, on the cover.
    #[test]
    fn the_running_header_names_the_document_the_composer_composed() {
        let blocks = vec![
            Block::Cover {
                title: "Thesis citation audit".into(),
                subtitle: "R PAPER .docx".into(),
                meta: vec![],
                headline: None,
                running_title: "Thesis citation audit".into(),
            },
            Block::PageBreak,
            Block::Heading { text: "At a glance".into(), level: 1 },
            Block::Paragraph { text: "A sentence on the second page.".into() },
        ];
        let pdf = render_pdf(&blocks);
        let after_cover = &page_streams(&pdf)[1];
        assert!(
            drawn_text(after_cover).iter().any(|t| t == "Thesis citation audit"),
            "the running header does not name this document: {after_cover}"
        );
        assert!(
            !String::from_utf8_lossy(&pdf).contains("PublishReady"),
            "another product's name is still on the page"
        );

        // THE NEGATIVE CONTROL. The name was not simply deleted everywhere: the
        // report it was right for still carries it.
        let pr = render_pdf(&specimen());
        assert!(
            drawn_text(&page_streams(&pr)[1]).iter().any(|t| t == "PublishReady report"),
            "the PublishReady report lost its own name"
        );
    }

    /// §11 D149. THE CONTENTS PAGE STATES WHERE EACH SECTION ACTUALLY IS.
    ///
    /// The numbers come from a first layout pass and are printed by a second,
    /// so the thing to check is not that numbers appear but that each one is
    /// the page its section landed on in the file that was written. A contents
    /// page with confident wrong numbers is worse than none.
    #[test]
    fn the_contents_page_numbers_are_the_pages_the_sections_are_on() {
        let filler = "A paragraph of ordinary body text, long enough that a run of them fills \
                      a page and pushes the next section onto the one after it. ";
        let sections = ["At a glance", "The counts", "Cited, but not checkable", "Not judged"];
        let mut blocks = vec![
            Block::Cover {
                title: "Thesis citation audit".into(),
                subtitle: "R PAPER .docx".into(),
                meta: vec![],
                headline: None,
                running_title: "Thesis citation audit".into(),
            },
            Block::Contents { title: "What is in this report".into() },
            Block::PageBreak,
        ];
        for name in sections {
            blocks.push(Block::Heading { text: name.into(), level: 1 });
            for _ in 0..14 {
                blocks.push(Block::Paragraph { text: filler.repeat(3) });
            }
        }
        let pdf = render_pdf(&blocks);
        let pages = page_streams(&pdf);
        assert!(pages.len() >= 4, "the fixture did not spill across pages: {}", pages.len());

        // The cover ends with a break the renderer always honours, so the
        // contents opens the page after it.
        let contents_page = pages
            .iter()
            .position(|p| drawn_text(p).iter().any(|t| t == "What is in this report"))
            .expect("no contents page");
        let contents = drawn_text(&pages[contents_page]);
        for name in sections {
            // Where the contents SAYS it is: the first all-digits run drawn
            // after the entry.
            let at = contents.iter().position(|t| t == name).expect("no contents entry");
            let claimed: usize = contents[at + 1..]
                .iter()
                .find(|t| !t.is_empty() && t.chars().all(|c| c.is_ascii_digit()))
                .expect("the entry carries no page number")
                .parse()
                .unwrap();
            // Where it IS. The contents page names every section in regular
            // type; the heading itself is the bold one, and it can be on the
            // contents page (a short contents does not spend a whole sheet).
            let actual = pages
                .iter()
                .enumerate()
                .find(|(_, p)| {
                    drawn(p).iter().any(|(face, t)| face == "/F2" && t == name)
                })
                .map(|(i, _)| i + 1)
                .expect("a section in the contents is not in the document");
            assert_eq!(claimed, actual, "the contents sends the reader to the wrong page for {name:?}");
        }
    }

    /// A specimen exercising every decorated element: cover, h1/h2/h3,
    /// paragraphs, flush and indented bullets, a note, and page breaks.
    fn specimen() -> Vec<Block> {
        use crate::report::{CertaintyTier, FindingSeverity};
        use crate::report_model::{LocalFinding, LocalReportModel, ManuscriptFacts};
        use crate::reviewer_agent::LaneExamination;
        let m = LocalReportModel {
            run_id: "run-1".into(),
            manuscript: ManuscriptFacts {
                title: Some("A Study of Something Measurable".into()),
                word_count: 4000,
                section_count: 5,
                table_count: 1,
                reference_count: 20,
                statistics: vec![],
            },
            journal_name: Some("PLOS Medicine".into()),
            guidelines_url: None,
            findings: vec![LocalFinding {
                also_nearby: Vec::new(),
                id: "f1".into(),
                severity: FindingSeverity::Major,
                tier: CertaintyTier::MathematicallyCertain,
                claim: crate::evidence::ClaimKind::ManuscriptDefect,
                agent: crate::swarm::AgentKind::ValidationMaths,
                title: "statistical rule failed: missing effect size".into(),
                detail: "A p-value is reported without an accompanying effect size, so the \
                         magnitude of the effect is not stated anywhere in this paragraph."
                    .into(),
                confidence: 1.0,
                provenance: vec!["rule:MissingEffectSize (MAJOR)".into()],
                nearby_text: Some(
                    "Recall improved with sleep (95% CI: 1.2 to 3.4; p = 0.03), and the \
                     effect persisted at follow-up."
                        .into(),
                ),
            }],
            verdict: "concern".into(),
            recommendation: None,
            combined_confidence: 0.62,
            checklist: vec![crate::report::ChecklistItem {
                requirement: "structured abstract".into(),
                passed: true,
                detail: "found".into(),
                guideline_source: None,
                source_span: None,
                article_type: None,
                checked_field: None,
                unevaluable: false,
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
            disclaimer: "Certainty tiers: mathematically certain findings are deterministic.".into(),
        };
        compose(&m)
    }


    /// **`p ≤ 0.05` MUST NOT RENDER AS `p ? 0.05`.**
    ///
    /// Measured on the reference manuscript: every significance criterion was
    /// unreadable because U+2264 has no Latin base and hit the unrepresentable
    /// sentinel. These characters have an exact ASCII spelling — folding them
    /// is the em-dash class, not a loss.
    #[test]
    fn mathematical_notation_folds_rather_than_marking() {
        for (input, want) in [
            ("p \u{2264} 0.05", "p <= 0.05"),
            ("p \u{2265} 0.05", "p >= 0.05"),
            ("d \u{2260} 0", "d != 0"),
            ("\u{2212}0.092", "-0.092"),
            ("1\u{2032}", "1'"),
            ("2\u{2033}", "2\""),
            ("a \u{2192} b", "a -> b"),
        ] {
            let (bytes, o) = encode_winansi(input);
            assert_eq!(String::from_utf8_lossy(&bytes), want, "{input:?}");
            assert!(!o.any_marked, "{input:?} must not hit the sentinel");
            assert!(o.any_simplified, "{input:?} is a fold, and must be disclosed as one");
            assert!(bytes.iter().all(|b| b.is_ascii()), "a fold must emit ASCII, never UTF-8");
        }
    }

    /// Both mu codepoints survive: GREEK SMALL LETTER MU folds to WinAnsi's
    /// MICRO SIGN, which is the same mark at a different codepoint.
    #[test]
    fn both_mu_codepoints_reach_the_page() {
        for input in ["\u{03BC}g/cm2", "\u{00B5}g/cm2"] {
            let (bytes, o) = encode_winansi(input);
            assert_eq!(bytes[0], 0xB5, "{input:?} must encode as the micro sign");
            // The same mark at a different codepoint is NOT an alteration, so
            // no fold disclosure is owed for it.
            assert_eq!(o, EncodingOutcome::default(), "{input:?} is intact, not folded");
        }
    }

    /// `±` is already WinAnsi 0xB1 — the first attempt added a "+/-" fold for
    /// it, which was unreachable and would have been wrong.
    #[test]
    fn plus_minus_is_already_representable() {
        let (bytes, o) = encode_winansi("\u{00B1}0.1");
        assert_eq!(bytes[0], 0xB1);
        assert_eq!(o, EncodingOutcome::default());
    }

    /// Characters already IN WinAnsi must still pass through untouched — the
    /// new folds must not have caught them.
    #[test]
    fn superscript_two_and_multiply_still_pass_through_intact() {
        let (bytes, o) = encode_winansi("cm\u{00B2} 3 \u{00D7} 4");
        assert_eq!(bytes[2], 0xB2, "superscript two is WinAnsi 0xB2");
        assert_eq!(bytes[6], 0xD7, "multiplication sign is WinAnsi 0xD7");
        assert_eq!(o, EncodingOutcome::default(), "neither is folded or marked");
    }
    /// **NO DRAWN RULE OR FILL ESCAPES THE MARGINS.**
    #[test]
    fn every_rule_and_fill_sits_inside_the_page() {
        let (rects, _) = ops(&render_pdf(&specimen()));
        assert!(!rects.is_empty(), "the pass must draw rules and fills");
        for (x, y, w, h) in rects {
            assert!(x >= MARGIN_X - 0.01, "rect starts left of the margin: x={x}");
            assert!(x + w <= PAGE_W - MARGIN_X + 0.01, "rect runs past the right margin: {}", x + w);
            assert!(y >= 0.0, "rect below the page: y={y}");
            assert!(y + h <= PAGE_H + 0.01, "rect above the page: {}", y + h);
            assert!(w > 0.0 && h > 0.0, "degenerate rect {w}x{h}");
        }
    }

    /// **NO BASELINE GOES NEGATIVE OR OFF-PAGE.**
    #[test]
    fn every_baseline_is_on_the_page() {
        let (_, tms) = ops(&render_pdf(&specimen()));
        assert!(tms.len() > 20, "the specimen must produce real text");
        for (x, y, _) in tms {
            assert!(y > 0.0, "baseline below the page edge: y={y}");
            assert!(y < PAGE_H, "baseline above the page: y={y}");
            assert!(x >= MARGIN_X - 0.01, "text starts left of the margin: x={x}");
            assert!(x < PAGE_W - MARGIN_X, "text starts past the right margin: x={x}");
        }
    }

    /// **EVERY WRAPPED LINE FITS ITS COLUMN**, measured with the same AFM table
    /// the wrapper used — so this proves the wrapper is self-consistent, NOT
    /// that the bold table is transcribed correctly.
    #[test]
    fn every_wrapped_line_fits_the_column() {
        for face in [Face::Regular, Face::Bold, Face::Oblique] {
            for size in [scale::BODY.0, scale::H1.0, scale::QUOTATION.0] {
                let (bytes, _) = encode_winansi(
                    "A reasonably long line of manuscript prose with several polysyllabic                      words in it, long enough to wrap more than once at every size.",
                );
                for l in wrap(&bytes, size, TEXT_W, face) {
                    assert!(
                        text_width_in(&l, size, face) <= TEXT_W + 0.01,
                        "line overflows at {size}pt in {face:?}"
                    );
                }
            }
        }
    }

    /// The one INDEPENDENT check available on the bold table without a
    /// rasteriser: bold is wider than regular for every glyph EXCEPT two.
    ///
    /// # The exceptions are evidence, not a weakening
    ///
    /// `@` (1015 → 975) and `œ` (944 → 889) are genuinely NARROWER in
    /// Helvetica-Bold than in Helvetica — both are real Adobe AFM values. A
    /// fabricated or guessed table would almost certainly have been
    /// monotonically wider, so these two exceptions are the strongest evidence
    /// available here that the transcription came from the real metrics. They
    /// are PINNED: a third exception, or the loss of one of these, fails.
    #[test]
    fn bold_is_wider_everywhere_but_two_known_glyphs() {
        let mut narrower: Vec<usize> = Vec::new();
        for b in 0..256usize {
            let (r, bo) = (HELVETICA_WIDTHS[b], HELVETICA_BOLD_WIDTHS[b]);
            if r == 0 || bo == 0 {
                continue;
            }
            if bo < r {
                narrower.push(b);
            }
        }
        assert_eq!(narrower, vec![0x40, 0x9C], "unexpected bold/regular width inversion");
    }

    /// Every WinAnsi byte the regular table defines must be defined in bold too
    /// — a hole would silently measure as zero and overflow the column.
    #[test]
    fn the_bold_table_defines_every_glyph_the_regular_one_does() {
        for b in 0..256usize {
            if HELVETICA_WIDTHS[b] != 0 {
                assert_ne!(HELVETICA_BOLD_WIDTHS[b], 0, "bold width missing for byte {b:#04x}");
            }
        }
    }

    /// Text still extracts — the typography pass must not have produced a
    /// document whose content is unreadable.
    #[test]
    fn no_page_extracts_empty() {
        let pdf = render_pdf(&specimen());
        let text = pdf_extract::extract_text_from_mem(&pdf).expect("our own PDF must parse");
        assert!(text.contains("Issues by severity"), "section headings must survive");
        assert!(text.trim().len() > 200, "extraction is not empty: {}", text.len());
    }

    /// The soft break spends itself as space rather than leaving a near-empty
    /// page — the two one-paragraph sections merge.
    #[test]
    fn a_break_on_a_nearly_empty_page_does_not_start_a_new_one() {
        let blocks = vec![
            Block::Heading { text: "One".into(), level: 1 },
            Block::Paragraph { text: "A short paragraph.".into() },
            Block::PageBreak,
            Block::Heading { text: "Two".into(), level: 1 },
            Block::Paragraph { text: "Another short paragraph.".into() },
        ];
        let pdf = render_pdf(&blocks);
        let pages = String::from_utf8_lossy(&pdf).matches("/Type /Page ").count();
        assert_eq!(pages, 1, "two short sections must share a page");
    }

    /// A hard break — only the cover asks for one — always happens.
    #[test]
    fn the_cover_always_stands_alone() {
        let blocks = vec![
            Block::Cover {
                title: "T".into(),
                subtitle: "S".into(),
                meta: vec![],
                headline: None,
                running_title: "T".into(),
            },
            Block::Heading { text: "One".into(), level: 1 },
        ];
        let pdf = render_pdf(&blocks);
        let pages = String::from_utf8_lossy(&pdf).matches("/Type /Page ").count();
        assert_eq!(pages, 2, "the cover never shares a page");
    }
}
