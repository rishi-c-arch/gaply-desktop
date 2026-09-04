//! The deterministic pre-pass for a thesis citation audit (plan §11 D40).
//!
//! **No model runs here.** Sentence segmentation, citation-marker detection and
//! library matching all happen before the first inference, for a blunt reason:
//! at ~65 s an item, an audit that discovers at item 200 that the manuscript
//! was unparseable has wasted three hours.
//!
//! # What it decides
//!
//! Every surviving sentence becomes exactly one of:
//!
//! | kind | when |
//! |---|---|
//! | `CitationNeed` | no citation marker, and the sentence looks like a claim |
//! | `CitationSupport` | a marker resolving to a library entry with an indexed, embedded source |
//! | `Unverifiable` | a marker whose source is not available to check against |
//!
//! `Unverifiable` is a **report category, not an error**: a cited work whose
//! source is not in the library is a true and useful finding about the
//! manuscript, it costs zero model calls, and retrying it could never succeed.
//!
//! # The filter earns its place in hours
//!
//! Headings, the references section and sentences under six words are dropped
//! before anything is queued. At 65 s each, the difference between filtering
//! and not is measured in hours, not tidiness.

use regex::Regex;
use std::collections::BTreeMap;
use std::sync::OnceLock;

use super::jobs::ItemKind;

/// Sentences shorter than this are not claims worth a model call.
const MIN_CLAIM_WORDS: usize = 6;

/// Which citation style a marker was written in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MarkerStyle {
    /// `(Smith, 2019)`, `(Smith & Jones, 2020)`, `(Smith et al., 2019; Doe, 2021)`
    AuthorYear,
    /// `[3]`, `[3, 4]`, `[3-5]`
    Numeric,
}

/// One citation marker as it appears in the text.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Marker {
    pub raw: String,
    pub style: MarkerStyle,
    /// Lead surname, lowercased, for `AuthorYear`. `None` for numeric styles —
    /// a bare `[3]` names nobody without a numbered bibliography.
    pub lead_author: Option<String>,
    pub year: Option<i32>,
    /// Reference numbers for `Numeric`.
    pub numbers: Vec<u32>,
}

/// A sentence the planner intends to queue, before the library is consulted.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlannedSentence {
    pub page: Option<u32>,
    pub sentence: String,
    pub markers: Vec<Marker>,
    /// The nearest preceding heading, e.g. "RESULTS AND DISCUSSION".
    ///
    /// The citation_need prompt has always had a rule keyed on this — "the same
    /// sentence in Results is probably the author's own finding" — and the
    /// audit passed an EMPTY section, so it could never fire. That is why the
    /// first real run flagged the authors' own results, their hardware setup
    /// and their own F1 scores as needing citations.
    pub section: Option<String>,
    /// 1-based paragraph ordinal within the document, for sources that have no
    /// pages (§11 D65).
    ///
    /// A `.docx` has no pagination until something lays it out, so "page
    /// unknown" was reporting a defect where there is only a file format. A
    /// paragraph ordinal is a REAL locator the format does define, and it is
    /// counted from the document's own paragraph boundaries rather than
    /// estimated from character position. `None` for a PDF, which has the
    /// better locator already.
    pub paragraph: Option<u32>,
}

/// What the pre-pass measured, before any model call.
#[derive(Debug, Clone, Default, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PrepassReport {
    pub total_sentences: usize,
    pub cited: usize,
    pub uncited: usize,
    /// Dropped by the significance filter: headings, references, short lines.
    pub skipped: usize,
    /// WHY they were dropped, by reason. `skipped` alone cannot distinguish
    /// "the filter removed 40 table rows" from "the filter ate 40 claims",
    /// which are opposite facts about the same number.
    pub skipped_by_reason: BTreeMap<String, usize>,
    pub markers_found: usize,
    pub planned: Vec<PlannedSentence>,
    /// The paper's own numbered reference list, `[n]` → entry. Empty when the
    /// paper uses author-year, or when no references heading was found.
    pub bibliography: BTreeMap<u32, BibEntry>,
}

/// One entry from the paper's own numbered reference list.
///
/// Deliberately not a full CSL parse: what the resolver needs is a DOI if there
/// is one, a title-ish string, and a lead author with a year. Reference formats
/// vary by publisher and a strict grammar would fail on most of them, so this
/// takes the parts that are recognisable and leaves the rest as `raw`.
#[derive(Debug, Clone, PartialEq, Eq, Default, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BibEntry {
    pub number: u32,
    pub raw: String,
    pub doi: Option<String>,
    pub lead_author: Option<String>,
    pub year: Option<i32>,
}

fn bib_start_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    // "[12] Author…" or "12. Author…" at the start of a line.
    RE.get_or_init(|| Regex::new(r"^\s*(?:\[(\d{1,3})\]|(\d{1,3})\.)\s+(\S.*)$").expect("bib regex"))
}

fn doi_in_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"10\.\d{4,9}/[-._;()/:A-Za-z0-9]+").expect("doi regex"))
}

/// Parse the numbered reference list out of the text that FOLLOWS the
/// references heading.
///
/// # Why this exists
///
/// A numeric citation is an index into a list, and without the list `[5]` names
/// nothing. `resolve_marker` said exactly that — "numeric citation style: no
/// numbered bibliography was parsed" — and on the first real audited paper that
/// answer accounted for 15 of 17 unverifiable items. Every one of those
/// sentences genuinely cites a real work; the audit simply had no way to say
/// WHICH. The list is right there in the manuscript, a few lines below the
/// prose the pre-pass already stops at.
pub fn parse_numbered_bibliography(text: &str) -> BTreeMap<u32, BibEntry> {
    let mut out: BTreeMap<u32, BibEntry> = BTreeMap::new();

    // Bracketed form FIRST, and split on the markers wherever they fall rather
    // than at line starts. PDF extraction reflows the reference list into
    // paragraphs — on the first audited paper the entire list arrived as THREE
    // lines — so a line-anchored parser found 2 of 24 entries. The marker is
    // the only reliable delimiter.
    let marks: Vec<(usize, usize, u32)> = bib_marker_re()
        .captures_iter(text)
        .filter_map(|c| {
            let whole = c.get(0)?;
            let n = c.get(1)?.as_str().parse::<u32>().ok()?;
            Some((whole.start(), whole.end(), n))
        })
        .collect();

    if !marks.is_empty() {
        for (i, (_, body_start, n)) in marks.iter().enumerate() {
            let body_end = marks.get(i + 1).map(|(s, _, _)| *s).unwrap_or(text.len());
            let raw = text[*body_start..body_end].split_whitespace().collect::<Vec<_>>().join(" ");
            if raw.is_empty() {
                continue;
            }
            out.insert(*n, BibEntry { number: *n, raw, ..Default::default() });
        }
    } else {
        // `1. Author …` form. Only line-anchored: mid-paragraph "12." is far
        // more often the end of a sentence than the start of a reference.
        let mut current: Option<BibEntry> = None;
        for line in text.lines() {
            let flat = line.split_whitespace().collect::<Vec<_>>().join(" ");
            if let Some(c) = bib_start_re().captures(&flat) {
                if let Some(done) = current.take() {
                    out.insert(done.number, done);
                }
                let number =
                    c.get(2).and_then(|m| m.as_str().parse::<u32>().ok()).unwrap_or(0);
                let body = c.get(3).map(|m| m.as_str()).unwrap_or("").to_string();
                current = Some(BibEntry { number, raw: body, ..Default::default() });
            } else if let Some(cur) = current.as_mut() {
                if !flat.is_empty() {
                    cur.raw.push(' ');
                    cur.raw.push_str(&flat);
                }
            }
        }
        if let Some(done) = current.take() {
            out.insert(done.number, done);
        }
    }

    for e in out.values_mut() {
        e.doi = doi_in_re()
            .find(&e.raw)
            .map(|m| m.as_str().trim_end_matches(|c| c == '.' || c == ',').to_string());
        e.year = year_in_re().find(&e.raw).and_then(|m| m.as_str().parse::<i32>().ok());
        // The lead author is the first surname-shaped token. Entry formats
        // differ on initials-first vs surname-first, so this takes the first
        // token that LOOKS like a surname rather than assuming an order.
        e.lead_author = e
            .raw
            .split(|c: char| c == ',' || c == '.' || c == ' ')
            .map(str::trim)
            .find(|t| {
                t.len() >= 3
                    && t.chars().next().is_some_and(|c| c.is_uppercase())
                    && t.chars().all(|c| c.is_alphabetic())
            })
            .map(|t| t.to_lowercase());
    }
    out
}

/// `[12]` followed by whitespace — a reference-list marker wherever it appears.
fn bib_marker_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"\[(\d{1,3})\]\s+").expect("bib marker regex"))
}

fn year_in_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?:1[6-9]|20)\d{2}").expect("year regex"))
}

fn author_year_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        // A capitalised surname, optional co-authors / "et al.", then a 4-digit
        // year with an optional disambiguating letter. Anchored to the whole
        // parenthetical so "(in 2019 we sampled)" cannot match: something
        // author-shaped must precede the year.
        Regex::new(
            r"\(\s*(?P<lead>[A-Z][\p{L}'’\-]+)(?:[^()]{0,80}?)?,?\s*(?P<year>(?:1[6-9]|20)\d{2})[a-z]?\s*(?:;[^()]{0,200})?\)",
        )
        .expect("author-year regex")
    })
}

/// NARRATIVE author-year: `Tang et al. (2016)`, `Wang and Liu (2019)`.
///
/// The audit was blind to this form. `author_year_re` requires the author
/// INSIDE the parentheses, so `(Smith, 2019)` matched and `Smith (2019)` did
/// not — and on the first real paper that meant sentences citing Tang, Wang and
/// Demszky were counted as UNCITED and sent to the model as "does this need a
/// citation?", which is precisely the question they already answer.
///
/// The pattern mirrors `extract::stats`'s canonical `narrative_cite`, including
/// its `et\s+al\.` — `\s+`, not a literal space, because PDF extraction emits
/// `et  al.` with a doubled space often enough to matter.
fn narrative_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(
            r"(?P<lead>[A-Z][\p{L}'’.\-]+)(?:\s+(?:et\s+al\.?|(?:and|&)\s+[A-Z][\p{L}'’.\-]+|,\s+[A-Z][\p{L}'’.\-]+)+)?\s*\(\s*(?P<year>(?:1[6-9]|20)\d{2})[a-z]?\s*\)",
        )
        .expect("narrative regex")
    })
}

fn numeric_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"\[\s*\d{1,3}(?:\s*[,;–—-]\s*\d{1,3})*\s*\]").expect("numeric regex")
    })
}

fn number_run_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"\d{1,3}").expect("digit regex"))
}

/// Every citation marker in one sentence, in both supported styles.
pub fn markers_in(sentence: &str) -> Vec<Marker> {
    let mut out = Vec::new();
    for c in author_year_re().captures_iter(sentence) {
        let raw = c.get(0).map(|m| m.as_str()).unwrap_or_default().to_string();
        out.push(Marker {
            lead_author: c.name("lead").map(|m| m.as_str().to_lowercase()),
            year: c.name("year").and_then(|m| m.as_str().parse::<i32>().ok()),
            raw,
            style: MarkerStyle::AuthorYear,
            numbers: Vec::new(),
        });
    }
    // Narrative form, skipping any span the parenthetical scanner already took
    // so `(Smith, 2019)` is never counted twice.
    for c in narrative_re().captures_iter(sentence) {
        let whole = c.get(0).map(|m| m.as_str()).unwrap_or_default();
        if out.iter().any(|m: &Marker| whole.contains(&m.raw)) {
            continue;
        }
        out.push(Marker {
            lead_author: c.name("lead").map(|m| m.as_str().to_lowercase()),
            year: c.name("year").and_then(|m| m.as_str().parse::<i32>().ok()),
            raw: whole.to_string(),
            style: MarkerStyle::AuthorYear,
            numbers: Vec::new(),
        });
    }
    for m in numeric_re().find_iter(sentence) {
        let raw = m.as_str().to_string();
        let numbers = number_run_re()
            .find_iter(&raw)
            .filter_map(|d| d.as_str().parse::<u32>().ok())
            .collect();
        out.push(Marker {
            raw,
            style: MarkerStyle::Numeric,
            lead_author: None,
            year: None,
            numbers,
        });
    }
    out
}

/// Does this line start the references section? Everything after it is
/// bibliography, not prose, and must never become an audit item.
pub fn is_references_heading(line: &str) -> bool {
    let t = line.trim().trim_end_matches(':').trim();
    if t.len() > 40 {
        return false;
    }
    let lower = t.to_lowercase();
    let lower = lower.trim_start_matches(|c: char| c.is_ascii_digit() || c == '.' || c == ' ');
    matches!(
        lower,
        "references" | "bibliography" | "works cited" | "reference list" | "literature cited"
    )
}

/// A heading is short, unpunctuated prose — a title, not a claim.
pub fn looks_like_heading(sentence: &str) -> bool {
    let t = sentence.trim();
    if t.is_empty() {
        return true;
    }
    // Ends with sentence punctuation → it is a sentence, not a heading.
    if t.ends_with('.') || t.ends_with('?') || t.ends_with('!') {
        // "3.2 Methods." is still a heading; a numbered stub with few words is
        // the giveaway.
        let words = t.split_whitespace().count();
        let numbered = t.chars().next().is_some_and(|c| c.is_ascii_digit());
        return numbered && words <= 4;
    }
    let words: Vec<&str> = t.split_whitespace().collect();
    words.len() <= 8
}

/// Why a line was not worth a model call. Typed rather than boolean so the
/// report can say WHAT it dropped — "83 sentences need a citation" and "17 of
/// those were table rows" are very different messages.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SkipReason {
    TooShort,
    Heading,
    /// Title block, authors, affiliations, emails, abstract/keyword labels.
    FrontMatter,
    /// A table row, a table caption, or a figure caption.
    TableOrFigure,
    /// Acknowledgements, funding, conflicts, copyright/DOI furniture.
    BackMatter,
    /// A numbered or lettered step in a procedure list.
    ListItem,
    /// An equation or a line of symbolic notation.
    Notation,
}

impl SkipReason {
    pub fn as_str(self) -> &'static str {
        match self {
            SkipReason::TooShort => "too short to be a claim",
            SkipReason::Heading => "a heading, not a claim",
            SkipReason::FrontMatter => "front matter (title, authors, abstract or keywords)",
            SkipReason::TableOrFigure => "a table row or a figure caption",
            SkipReason::BackMatter => "acknowledgements or publication furniture",
            SkipReason::ListItem => "a numbered step in a procedure list",
            SkipReason::Notation => "an equation, not prose",
        }
    }
}

/// Fraction of the cased letters that are uppercase. A title set in caps is not
/// a claim, and on PDF-extracted text this is far more reliable than looking
/// for a heading's punctuation, which extraction routinely mangles.
fn uppercase_ratio(t: &str) -> f32 {
    let cased: Vec<char> = t.chars().filter(|c| c.is_alphabetic()).collect();
    if cased.len() < 8 {
        return 0.0;
    }
    let upper = cased.iter().filter(|c| c.is_uppercase()).count();
    upper as f32 / cased.len() as f32
}

/// Proportion of whitespace-separated tokens that are numeric-ish. Table rows
/// are mostly numbers and short labels; prose is not.
fn numeric_token_ratio(t: &str) -> f32 {
    let toks: Vec<&str> = t.split_whitespace().collect();
    if toks.len() < 4 {
        return 0.0;
    }
    let numeric = toks
        .iter()
        .filter(|w| {
            let w = w.trim_matches(|c: char| !c.is_alphanumeric());
            !w.is_empty() && w.chars().any(|c| c.is_ascii_digit())
        })
        .count();
    numeric as f32 / toks.len() as f32
}

/// Why this line is not a claim, or `None` if it is one.
///
/// # Why this got much stricter
///
/// The first real audit queued 83 of 100 lines from an IEEE-style PDF as
/// "needs a citation", and among them were the paper's TITLE, the author list
/// with an email address, the abstract, the keyword line, `TABLE I. DATASET
/// STATISTICS …`, figure captions, section headers and the acknowledgement.
/// None of those is a claim, and each one cost a model call to be told so.
///
/// The old filter was "six words and not a short unpunctuated line", which
/// catches `3.2 Methods` and nothing else. PDF extraction produces long,
/// unpunctuated, whitespace-mangled lines for exactly the furniture that needs
/// dropping, so every test here is written against that kind of text rather
/// than against clean prose.
pub fn skip_reason(sentence: &str) -> Option<SkipReason> {
    let t = sentence.trim();
    // Collapse the runs of spaces PDF extraction leaves behind, so every rule
    // below sees the words rather than the layout.
    let flat = t.split_whitespace().collect::<Vec<_>>().join(" ");
    let lower = flat.to_lowercase();
    let words = flat.split_whitespace().count();

    if words < MIN_CLAIM_WORDS {
        return Some(SkipReason::TooShort);
    }

    // --- front matter -----------------------------------------------------
    // An email address never appears inside a claim; it appears in an author
    // block, which extraction glues to whatever surrounds it.
    if flat.contains('@') && flat.split_whitespace().any(|w| w.contains('@') && w.contains('.')) {
        return Some(SkipReason::FrontMatter);
    }
    for label in ["abstract", "keywords", "key words", "index terms"] {
        // "Abstract-", "Keywords:", "Index Terms —" all start the line.
        if lower.starts_with(label)
            && lower[label.len()..].starts_with(|c: char| !c.is_alphanumeric())
        {
            return Some(SkipReason::FrontMatter);
        }
    }
    // A title or an ALL-CAPS running header.
    if uppercase_ratio(&flat) >= 0.7 {
        return Some(SkipReason::FrontMatter);
    }

    // --- tables and figures ----------------------------------------------
    if lower.starts_with("table ") || lower.starts_with("fig.") || lower.starts_with("figure ") {
        return Some(SkipReason::TableOrFigure);
    }
    // A row of mostly numbers, with no verb-bearing prose to speak of.
    if numeric_token_ratio(&flat) >= 0.4 {
        return Some(SkipReason::TableOrFigure);
    }

    // --- back matter ------------------------------------------------------
    for label in ["acknowledg", "conflict of interest", "funding statement", "©", "doi:", "arxiv:"] {
        if lower.starts_with(label) {
            return Some(SkipReason::BackMatter);
        }
    }

    // --- procedure lists --------------------------------------------------
    // "2) Emoji-to-text replacement …", "3. Tokenization …". A pipeline step is
    // a description of what THIS paper did, not a claim about the literature.
    // Anchored to the very start so a mid-sentence "(2)" cannot trip it.
    if flat
        .split_once(|c: char| c == ')' || c == '.')
        .is_some_and(|(head, rest)| {
            !head.is_empty()
                && head.len() <= 3
                && head.chars().all(|c| c.is_ascii_digit())
                && rest.starts_with(' ')
        })
    {
        return Some(SkipReason::ListItem);
    }

    // --- notation ---------------------------------------------------------
    // An equation is not prose. Counted as symbol density rather than matched
    // as a pattern, because extraction renders maths as whatever it can.
    let symbols = flat
        .chars()
        // Brackets are DELIBERATELY absent: `[1]` is a numeric citation, and a
        // rule that counted it as maths dropped "…points to mental health
        // monitoring [1]" — a genuinely cited claim — as an equation. A filter
        // that discards cited sentences is failing at the one thing it must
        // never do.
        .filter(|c| "=+*/^_∑Σαβθπ≤≥".contains(*c))
        .count();
    if symbols >= 6 && symbols as f32 / flat.chars().count().max(1) as f32 >= 0.04 {
        return Some(SkipReason::Notation);
    }

    if looks_like_heading(&flat) {
        return Some(SkipReason::Heading);
    }
    None
}

/// The section heading this line IS, if it is one.
///
/// Deliberately narrow: an ALL-CAPS or numbered short line, which is what IEEE
/// and thesis templates produce and what extraction preserves. A loose rule
/// here would relabel ordinary sentences and mislead every judgement beneath
/// them, which is worse than having no section at all.
pub fn heading_of(sentence: &str) -> Option<String> {
    let t = sentence.split_whitespace().collect::<Vec<_>>().join(" ");
    let t = t.trim();
    if t.is_empty() || t.split_whitespace().count() > 8 {
        return None;
    }
    // Strip a leading "IV." / "4.2" / "B." enumerator before judging the words.
    let body = t
        .trim_start_matches(|c: char| c.is_ascii_digit() || c == '.' || c == ')' || c == ' ')
        .trim_start_matches(|c: char| "IVXLC".contains(c) && t.starts_with(|f: char| f.is_uppercase()))
        .trim_start_matches(['.', ')', ' ']);
    let letters: Vec<char> = body.chars().filter(|c| c.is_alphabetic()).collect();
    if letters.len() < 3 {
        return None;
    }
    let upper = letters.iter().filter(|c| c.is_uppercase()).count();
    (upper as f32 / letters.len() as f32 >= 0.8).then(|| body.trim().to_string())
}

/// Does this Word style DECLARE the paragraph a heading, and at what level?
///
/// `Heading1`/`Heading2`/`Heading3` are what Word writes when an author uses the
/// heading styles, and they are exact — unlike `heading_of`, which infers from
/// ALL-CAPS shape and produced `"SEAR"` and `"HEFCSO-BILSTM: A HYBRID"` on a
/// real paper. `Title` counts too: it names the document, and the sentences
/// under it before the first real heading belong to it.
pub fn style_is_heading(style: Option<&str>) -> bool {
    matches!(style, Some(s) if s.starts_with("Heading") || s == "Title" || s == "Subtitle")
}

/// Does this Word style DECLARE the paragraph a table cell?
///
/// `TableParagraph` is written for every cell of every table. The heuristic it
/// replaces guessed from digit density, which cannot tell a table row from a
/// sentence that happens to quote several numbers — and a Results paragraph
/// full of percentages is exactly that sentence.
pub fn style_is_table(style: Option<&str>) -> bool {
    matches!(style, Some(s) if s.contains("Table"))
}

/// Does this Word style DECLARE the paragraph a numbered-list item?
///
/// Word puts auto-list NUMBERS in `numbering.xml`, never in the paragraph text,
/// so `<w:t>` extraction of a numbered reference list yields entries with NO
/// number at all (§11 D67). On `R PAPER .docx` that made 22 of 25 references
/// invisible to the marker parser while the PDF rendered from the same file
/// parsed all 25 — the numbers exist only once something lays the list out.
pub fn style_is_list(style: Option<&str>) -> bool {
    matches!(style, Some(s) if s.contains("List"))
}

/// The `[n]` a reference entry already carries, if any.
fn leading_bib_marker(text: &str) -> Option<u32> {
    let t = text.trim_start();
    let rest = t.strip_prefix('[')?;
    let end = rest.find(']')?;
    rest[..end].trim().parse::<u32>().ok()
}

/// Is this sentence worth a model call at all?
pub fn is_significant(sentence: &str) -> bool {
    skip_reason(sentence).is_none()
}

/// Run the whole deterministic pass over paged blocks.
///
/// Takes `(page, text)` pairs rather than a path so it stays pure and testable
/// — the caller does the parsing with `extract::docparse::parse_path_paged`.
pub fn prepass(blocks: &[(Option<u32>, String)]) -> PrepassReport {
    let owned: Vec<crate::extract::docparse::PagedBlock> = blocks
        .iter()
        .map(|(page, text)| crate::extract::docparse::PagedBlock {
            page: *page,
            style: None,
            text: text.clone(),
        })
        .collect();
    prepass_blocks(&owned)
}

/// The real implementation, over blocks that may carry their Word style.
///
/// `prepass` remains as the `(page, text)` shim so every existing caller and
/// test is unchanged; a source with no style information behaves exactly as it
/// did, because `style: None` takes every heuristic path it took before.
pub fn prepass_blocks(blocks: &[crate::extract::docparse::PagedBlock]) -> PrepassReport {
    let mut report = PrepassReport::default();
    let mut in_references = false;
    // Everything after the references heading, kept rather than discarded: a
    // numeric citation is an index into this list, and without it `[5]` names
    // nothing.
    let mut references_text = String::new();
    let mut current_section: Option<String> = None;
    // §11 D67. The same reference list, with ordinals synthesised for
    // auto-numbered items — kept SEPARATELY and used only if it stays
    // consistent with the entries that carry a literal marker.
    let mut synthesised_text = String::new();
    let mut synth_next: u32 = 1;
    let mut synthesis_consistent = true;

    // 1-based paragraph ordinal, counted over EVERY block including the ones
    // that get skipped — a locator has to match what the reader counts in the
    // document, not what survived the filter.
    let mut paragraph: u32 = 0;
    for block in blocks {
        let (page, text, style) = (&block.page, &block.text, block.style.as_deref());
        paragraph += 1;
        // Already past the bibliography: nothing after it is prose, but it IS
        // the reference list.
        if in_references {
            references_text.push('\n');
            references_text.push_str(text);

            synthesised_text.push('\n');
            match leading_bib_marker(text) {
                // A literal marker is GROUND TRUTH and also the check: if the
                // document's own number disagrees with our count, the count is
                // wrong and every synthesised ordinal is suspect.
                Some(n) => {
                    if n != synth_next {
                        synthesis_consistent = false;
                    }
                    synth_next = n + 1;
                }
                // An auto-numbered item whose number lives in numbering.xml.
                // Position IS the number Word displays — including, on this
                // paper, an author's own wrapped line that Word counts as its
                // own entry. Reproducing that is correct: the reader's [6] is
                // whatever their document shows, not what they meant.
                None if style_is_list(style) => {
                    synthesised_text.push_str(&format!("[{synth_next}] "));
                    synth_next += 1;
                }
                None => {}
            }
            synthesised_text.push_str(text);
            continue;
        }
        // The heading can sit INSIDE a block — a whole chapter is often one
        // block — so split at it rather than judging the block as a whole.
        // Latching (not per-line) matters because a reference entry reads
        // exactly like prose and would otherwise generate hundreds of nonsense
        // items.
        let mut prose_end = text.len();
        let mut offset = 0usize;
        for line in text.split_inclusive('\n') {
            if is_references_heading(line) {
                prose_end = offset;
                in_references = true;
                // The list starts on this block, after the heading.
                references_text.push('\n');
                references_text.push_str(&text[offset..]);
                break;
            }
            offset += line.len();
        }
        let prose = &text[..prose_end];

        // STRUCTURE FIRST, heuristics second. A style the file declares is a
        // fact; ALL-CAPS shape and digit density are guesses about one.
        if style_is_heading(style) {
            // The whole paragraph is the heading. Take it verbatim rather than
            // re-deriving it — `heading_of` would truncate "Experimental Setup
            // and Results" to nothing, since it is neither short nor upper.
            let h = text.split_whitespace().collect::<Vec<_>>().join(" ");
            if !h.is_empty() {
                current_section = Some(h);
            }
            report.total_sentences += 1;
            report.skipped += 1;
            *report
                .skipped_by_reason
                .entry(SkipReason::Heading.as_str().to_string())
                .or_default() += 1;
            continue;
        }
        if style_is_table(style) {
            // Every cell of every table. The heuristic could not tell one from
            // a Results sentence quoting several numbers.
            report.total_sentences += 1;
            report.skipped += 1;
            *report
                .skipped_by_reason
                .entry(SkipReason::TableOrFigure.as_str().to_string())
                .or_default() += 1;
            continue;
        }

        for sentence in crate::extract::sentence::sentences_in(prose) {
            // A heading is not a claim, but it TELLS us what the claims under
            // it are. Tracked as the scan passes rather than looked up later,
            // because "the nearest preceding heading" is a property of reading
            // order and nothing downstream can reconstruct it.
            if let Some(h) = heading_of(sentence) {
                current_section = Some(h);
            }
            report.total_sentences += 1;
            let markers = markers_in(sentence);
            report.markers_found += markers.len();
            if markers.is_empty() {
                report.uncited += 1;
            } else {
                report.cited += 1;
            }
            if let Some(reason) = skip_reason(sentence) {
                report.skipped += 1;
                *report.skipped_by_reason.entry(reason.as_str().to_string()).or_default() += 1;
                continue;
            }
            report.planned.push(PlannedSentence {
                page: *page,
                // Only where there is no page. A PDF has the better locator
                // and two competing ones is a reader deciding which to trust.
                paragraph: page.is_none().then_some(paragraph),
                section: current_section.clone(),
                sentence: sentence.to_string(),
                markers,
            });
        }
    }

    // Prefer the synthesised list ONLY when it stayed consistent with every
    // literal marker AND actually found more. A wrong reference number is worse
    // than a missing one: it resolves a citation to the wrong paper, which is
    // the failure this engine exists to prevent.
    report.bibliography = parse_numbered_bibliography(&references_text);
    if synthesis_consistent {
        let synth = parse_numbered_bibliography(&synthesised_text);
        if synth.len() > report.bibliography.len() {
            report.bibliography = synth;
        }
    }
    report
}

/// What the library lookup concluded for one planned sentence.
#[derive(Debug, Clone, PartialEq)]
pub enum Resolution {
    /// The cited work is in the library AND its source is indexed + embedded.
    Checkable { library_id: String, document_id: i64 },
    /// Cited, but nothing to check against.
    ///
    /// `library_id` is `Some` when the marker DID resolve to a library entry
    /// and only the source is missing. The distinction is load-bearing for any
    /// caller filtering by citation: "this sentence cites Smith 2019, whose PDF
    /// is not indexed" and "this sentence cites nobody we know" are different
    /// facts, and collapsing them would drop the first sentence out of a
    /// per-citation view that should be showing it with a Link/Index prompt.
    Unverifiable { reason: String, library_id: Option<String> },
    /// No marker at all.
    Uncited,
}

impl Resolution {
    pub fn kind(&self) -> ItemKind {
        match self {
            Resolution::Checkable { .. } => ItemKind::CitationSupport,
            Resolution::Unverifiable { .. } => ItemKind::Unverifiable,
            Resolution::Uncited => ItemKind::CitationNeed,
        }
    }
}

/// Can this marker's source actually be checked against evidence?
///
/// # Reads the LINK TABLE (Phase 8b, migration v17)
///
/// Phase 8 joined `citation_library` to `documents` by normalised title, inline
/// and recomputed on every audit, and recorded the consequence: a source
/// indexed under a different title reported `Unverifiable` even though its text
/// was present.
///
/// Linking now happens once, in [`crate::citation_links`], with DOI matches
/// preferred over title matches and the method recorded per link. This function
/// asks that table rather than re-deriving a heuristic, so an audit's verdict
/// about availability is the same one the user can see and correct.
/// Match one reference-list entry against the citation library.
///
/// `None` means the work is genuinely not in the library — a different answer
/// from "we could not read the reference", and the caller words them
/// differently.
fn resolve_bib_entry(
    db: &crate::db::Database,
    entry: &BibEntry,
) -> Result<Option<Resolution>, crate::GaplyError> {
    use rusqlite::params;

    let library_id: Option<String> = {
        let conn = db.conn()?;
        // DOI first: an identifier beats a name, and a reference list that
        // prints a DOI has given us the unambiguous answer.
        let by_doi = entry.doi.as_deref().and_then(|doi| {
            let norm = crate::citation_links::normalise_doi(doi)?;
            conn.query_row(
                "SELECT id FROM citation_library WHERE LOWER(doi) = ?1 LIMIT 1",
                params![norm],
                |r| r.get::<_, String>(0),
            )
            .ok()
        });
        match by_doi {
            Some(id) => Some(id),
            None => match (entry.lead_author.as_deref(), entry.year) {
                (Some(a), Some(y)) => conn
                    .query_row(
                        "SELECT id FROM citation_library
                         WHERE LOWER(authors) LIKE ?1 AND year = ?2 LIMIT 1",
                        params![format!("%{a}%"), y],
                        |r| r.get::<_, String>(0),
                    )
                    .ok(),
                _ => None,
            },
        }
    };

    let Some(library_id) = library_id else { return Ok(None) };
    match crate::citation_links::checkable_document_for_citation(db, &library_id)? {
        Some((document_id, _)) => Ok(Some(Resolution::Checkable { library_id, document_id })),
        None => Ok(Some(Resolution::Unverifiable {
            reason: format!(
                "[{}] is in your library but its source is not indexed — link the document",
                entry.number
            ),
            library_id: Some(library_id),
        })),
    }
}

pub fn resolve_marker(
    db: &crate::db::Database,
    marker: &Marker,
) -> Result<Resolution, crate::GaplyError> {
    resolve_marker_with(db, marker, &BTreeMap::new())
}

/// `resolve_marker`, with the paper's own reference list available.
///
/// A numeric `[5]` is an index into that list. Given the list, the entry it
/// names can be matched against the library by DOI, then by lead author + year
/// — the same evidence order `citation_links` uses, for the same reason: a DOI
/// is an identifier and a name is a heuristic.
pub fn resolve_marker_with(
    db: &crate::db::Database,
    marker: &Marker,
    bibliography: &BTreeMap<u32, BibEntry>,
) -> Result<Resolution, crate::GaplyError> {
    use rusqlite::params;

    // A numeric marker resolves through the bibliography, when there is one.
    if marker.lead_author.is_none() && !marker.numbers.is_empty() {
        if bibliography.is_empty() {
            return Ok(Resolution::Unverifiable {
                reason: "numeric citation style: no numbered bibliography was parsed".to_string(),
                library_id: None,
            });
        }
        // A sentence may cite several works; the FIRST that resolves decides,
        // matching the author-year path's rule.
        let mut missing: Vec<String> = Vec::new();
        for n in &marker.numbers {
            let Some(entry) = bibliography.get(n) else {
                missing.push(format!("[{n}] is not in the reference list"));
                continue;
            };
            match resolve_bib_entry(db, entry)? {
                Some(r) => return Ok(r),
                None => missing.push(format!(
                    "[{n}] {} — not in your library",
                    entry.raw.chars().take(60).collect::<String>()
                )),
            }
        }
        return Ok(Resolution::Unverifiable {
            reason: missing.join("; "),
            library_id: None,
        });
    }

    let Some(lead) = marker.lead_author.as_deref() else {
        return Ok(Resolution::Unverifiable {
            reason: "numeric citation style: no numbered bibliography was parsed".to_string(),
            library_id: None,
        });
    };

    // Scoped so the pooled connection is released before the link lookup below
    // asks for one of its own — the pool is small, and holding one across the
    // call deadlocks rather than failing loudly.
    let hits: Vec<(String, String)> = {
        let conn = db.conn()?;
        let like = format!("%{lead}%");
        let mut stmt = conn.prepare(
            "SELECT id, title FROM citation_library
             WHERE LOWER(authors) LIKE ?1 AND (?2 IS NULL OR year = ?2)
             LIMIT 2",
        )?;
        let rows = stmt
            .query_map(params![like, marker.year], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<Result<Vec<_>, _>>()?;
        drop(stmt);
        rows
    };

    let Some((library_id, title)) = hits.first().cloned() else {
        return Ok(Resolution::Unverifiable {
            reason: format!("cited work not in library: {}", marker.raw),
            library_id: None,
        });
    };

    match crate::citation_links::checkable_document_for_citation(db, &library_id)? {
        Some((document_id, _matched_by)) => {
            Ok(Resolution::Checkable { library_id, document_id })
        }
        None => Ok(Resolution::Unverifiable {
            // Two different states, named differently, because they need
            // different things from the user: link the file, or index it.
            reason: if crate::citation_links::documents_for_citation(db, &library_id)?.is_empty() {
                format!("no indexed document is linked to this work: {title}")
            } else {
                format!("the linked document is not indexed or not embedded: {title}")
            },
            library_id: Some(library_id),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn author_year_markers_are_found_in_their_common_forms() {
        let cases = [
            ("Richness rose sharply (Smith, 2019).", "smith", 2019),
            ("Effects were mixed (Smith & Jones, 2020).", "smith", 2020),
            ("This is established (Smith et al., 2019).", "smith", 2019),
            ("Consistent with prior work (Von Braun, 1998).", "von", 1998),
            ("Disambiguated years work (Smith, 2019a).", "smith", 2019),
        ];
        for (text, lead, year) in cases {
            let m = markers_in(text);
            assert_eq!(m.len(), 1, "expected exactly one marker in {text:?}, got {m:?}");
            assert_eq!(m[0].style, MarkerStyle::AuthorYear);
            assert_eq!(m[0].lead_author.as_deref(), Some(lead), "{text}");
            assert_eq!(m[0].year, Some(year), "{text}");
        }
    }

    #[test]
    fn numeric_markers_are_found_in_their_common_forms() {
        for (text, expected) in [
            ("Richness rose sharply [3].", vec![3u32]),
            ("Several studies agree [3, 4].", vec![3, 4]),
            ("A range is cited [3-5].", vec![3, 5]),
        ] {
            let m = markers_in(text);
            assert_eq!(m.len(), 1, "{text}");
            assert_eq!(m[0].style, MarkerStyle::Numeric);
            assert_eq!(m[0].numbers, expected, "{text}");
        }
    }

    /// The precision half: things that LOOK like citations and are not.
    /// A false marker turns a citation_need item into a citation_support one
    /// and sends the audit looking for evidence that was never cited.
    #[test]
    fn prose_that_merely_contains_numbers_or_parentheses_is_not_a_marker() {
        for text in [
            "In 2019 the fields were resampled.",
            "Yields rose (by about 31 percent) across the trial.",
            "The pH was low (4.5) in every plot.",
            "See Figure 3 for the distribution.",
            "Temperatures ranged from 3-5 degrees.",
            "We sampled twelve paired fields (six organic).",
        ] {
            assert!(markers_in(text).is_empty(), "false marker found in {text:?}: {:?}", markers_in(text));
        }
    }

    #[test]
    fn a_multi_citation_parenthetical_is_one_marker() {
        let m = markers_in("Both agree (Smith, 2019; Jones, 2020).");
        assert_eq!(m.len(), 1, "a semicolon list is one parenthetical: {m:?}");
        assert_eq!(m[0].lead_author.as_deref(), Some("smith"));
    }

    #[test]
    fn references_headings_latch_and_variants_are_recognised() {
        for h in ["References", "REFERENCES", "Bibliography", "  Works Cited  ", "7. References"] {
            assert!(is_references_heading(h), "{h:?}");
        }
        for h in ["Reference to the method", "The references were checked carefully"] {
            assert!(!is_references_heading(h), "{h:?}");
        }
    }

    #[test]
    #[test]
    fn a_numbered_reference_list_is_parsed_from_extraction_whitespace() {
        // Real IEEE shape, with the re-flow a PDF gives it: entries wrap, and
        // the wrapped lines belong to the entry above them.
        let refs = "\n[1] R. Plutchik, \"A general psychoevolutionary theory of emotion,\"\n                    Theories of Emotion, 1980, doi:10.1016/B978-0-12-558701-3.50007-7\n                    [2] S. Hochreiter and J. Schmidhuber, Long short-term memory,\n                    Neural Computation, 1997.\n                    [12] A. Vaswani, Attention is all you need, NeurIPS 2017.\n";
        let bib = parse_numbered_bibliography(refs);
        assert_eq!(bib.len(), 3, "{bib:?}");

        let one = &bib[&1];
        assert_eq!(one.doi.as_deref(), Some("10.1016/B978-0-12-558701-3.50007-7"));
        assert_eq!(one.year, Some(1980));
        assert_eq!(one.lead_author.as_deref(), Some("plutchik"));
        // The wrapped continuation line was joined, not dropped.
        assert!(one.raw.contains("Theories of Emotion"), "{}", one.raw);

        // Numbers are the entry's own, not positional.
        assert_eq!(bib[&12].year, Some(2017));
        assert_eq!(bib[&2].lead_author.as_deref(), Some("Hochreiter".to_lowercase().as_str()));
    }

    #[test]
    fn prepass_keeps_the_reference_list_it_stops_prose_at() {
        // The pre-pass already stops prose at the references heading. It used
        // to throw the list away, which is why `[5]` could never name anything.
        let blocks = vec![(
            Some(1u32),
            "Emotion detection matters a great deal in practice [1].\n             REFERENCES\n             [1] R. Plutchik, A general theory of emotion, 1980.\n"
                .to_string(),
        )];
        let report = prepass(&blocks);
        assert_eq!(report.bibliography.len(), 1);
        assert_eq!(report.bibliography[&1].year, Some(1980));
        // …and the reference entry itself is still not queued as prose.
        assert!(
            report.planned.iter().all(|p| !p.sentence.contains("Plutchik")),
            "a reference entry leaked into the prose plan"
        );
    }

    #[test]
    /// §11 D67. Word keeps auto-list numbers in `numbering.xml`, never in the
    /// paragraph text, so a numbered reference list extracts with NO markers.
    #[test]
    fn auto_numbered_reference_entries_get_the_ordinal_word_would_display() {
        let b = |style: Option<&str>, text: &str| crate::extract::docparse::PagedBlock {
            page: None,
            style: style.map(str::to_string),
            text: text.to_string(),
        };
        let blocks = vec![
            b(None, "A claim about the field [2]."),
            b(Some("BodyText"), "References"),
            b(Some("ListParagraph"), "First, A. Paper one. J. One, 2001."),
            b(Some("ListParagraph"), "Second, B. Paper two. J. Two, 2002."),
            b(Some("ListParagraph"), "Third, C. Paper three. J. Three, 2003."),
        ];
        let r = prepass_blocks(&blocks);
        assert_eq!(r.bibliography.len(), 3, "auto-numbered entries were invisible");
        assert!(r.bibliography[&1].raw.contains("Paper one"));
        assert!(r.bibliography[&2].raw.contains("Paper two"));
        assert!(r.bibliography[&3].raw.contains("Paper three"));
    }

    /// A LITERAL marker is ground truth AND the check. When the document's own
    /// number disagrees with the running count, every synthesised ordinal is
    /// suspect and the synthesis is abandoned wholesale — a wrong reference
    /// number resolves a citation to the WRONG paper, which is worse than a
    /// missing one and is the failure this engine exists to prevent.
    #[test]
    fn a_disagreeing_literal_marker_abandons_the_synthesis_entirely() {
        let b = |style: Option<&str>, text: &str| crate::extract::docparse::PagedBlock {
            page: None,
            style: style.map(str::to_string),
            text: text.to_string(),
        };
        let blocks = vec![
            b(Some("BodyText"), "References"),
            b(Some("ListParagraph"), "First, A. Paper one. J. One, 2001."),
            b(Some("ListParagraph"), "Second, B. Paper two. J. Two, 2002."),
            // The document says this is [9]; our count says 3. One of us is
            // wrong, and it is not the document.
            b(None, "[9] Ninth, D. Paper nine. J. Nine, 2009."),
        ];
        let r = prepass_blocks(&blocks);
        // Only the literal entry survives. Synthesised 1 and 2 are discarded.
        assert_eq!(r.bibliography.len(), 1, "kept ordinals it could not vouch for");
        assert!(r.bibliography.contains_key(&9));
    }

    /// The consistent case: auto-numbered items followed by literal ones that
    /// AGREE. This is `R PAPER .docx` — 22 auto-numbered entries then a literal
    /// [23] — and the whole list is kept.
    #[test]
    fn agreeing_literal_markers_confirm_the_synthesis_and_the_list_is_kept() {
        let b = |style: Option<&str>, text: &str| crate::extract::docparse::PagedBlock {
            page: None,
            style: style.map(str::to_string),
            text: text.to_string(),
        };
        let mut blocks = vec![b(Some("BodyText"), "References")];
        for i in 1..=3 {
            blocks.push(b(Some("ListParagraph"), &format!("Author{i}, A. Paper {i}. J, 200{i}.")));
        }
        blocks.push(b(None, "[4] Fourth, D. Paper four. J. Four, 2004."));
        let r = prepass_blocks(&blocks);
        assert_eq!(r.bibliography.len(), 4, "the confirmed synthesis was discarded");
        assert!(r.bibliography[&1].raw.contains("Paper 1"));
        assert!(r.bibliography[&4].raw.contains("Paper four"));
    }

    /// A list item that ALREADY carries its marker must not be double-numbered.
    #[test]
    fn a_list_item_with_its_own_marker_is_not_renumbered() {
        let b = |style: Option<&str>, text: &str| crate::extract::docparse::PagedBlock {
            page: None,
            style: style.map(str::to_string),
            text: text.to_string(),
        };
        let blocks = vec![
            b(Some("BodyText"), "References"),
            b(Some("ListParagraph"), "[1] First, A. Paper one. J. One, 2001."),
            b(Some("ListParagraph"), "[2] Second, B. Paper two. J. Two, 2002."),
        ];
        let r = prepass_blocks(&blocks);
        assert_eq!(r.bibliography.len(), 2);
        assert!(r.bibliography[&1].raw.contains("Paper one"), "{:?}", r.bibliography[&1].raw);
        assert!(!r.bibliography[&1].raw.contains("[1]"), "the marker was left in the body");
    }

    #[test]
    fn a_numeric_marker_resolves_through_the_bibliography_to_a_checkable_source() {
        // The whole point: [1] is an index into the paper's own list, the list
        // names a work, and the work is in the library with an indexed source.
        let db = crate::Database::in_memory().unwrap();
        {
            let conn = db.conn().unwrap();
            conn.execute(
                "INSERT INTO citation_library (id, csl_json, doi, title, authors, year, tags, sync_status, created_at, updated_at)
                 VALUES ('lib-1', '{}', '10.1234/abc', 'Mining large-scale social media', 'De Choudhury, M', 2013, '[]', 'local_only', 1, 1)",
                [],
            ).unwrap();
            conn.execute(
                "INSERT INTO documents (source_type, title, source_url, fetched_at, checksum, status, created_at)
                 VALUES ('paper', 'Mining', '/tmp/a.pdf', 1, 'ck-1', 'ingested', 1)",
                [],
            ).unwrap();
            conn.execute(
                "INSERT INTO citation_documents (citation_id, document_id, matched_by, created_at)
                 VALUES ('lib-1', 1, 'manual', 1)",
                [],
            ).unwrap();
            conn.execute(
                "INSERT INTO ai_model_registry (id, kind, display_name, file_path, dim, registered_at)
                 VALUES ('m', 'embedding', 'm', '/dev/null', 384, 1)",
                [],
            ).unwrap();
            conn.execute(
                "INSERT INTO ai_chunks (document_id, page, section, char_start, char_end, content, token_estimate, content_hash, created_at)
                 VALUES (1, 1, NULL, 0, 5, 'text', 1, 'h1', 1)",
                [],
            ).unwrap();
            conn.execute(
                "INSERT INTO ai_chunk_embeddings (chunk_id, model_id, preprocessing_version, dim, vector, created_at)
                 VALUES (1, 'm', 'p', 384, X'00', 1)",
                [],
            ).unwrap();
        }

        // Matched by DOI when the entry prints one …
        let bib = parse_numbered_bibliography(
            "[1] M. De Choudhury, Mining large-scale social media, 2013, doi:10.1234/abc",
        );
        let m = &markers_in("Emotion detection matters [1].")[0];
        match resolve_marker_with(&db, m, &bib).unwrap() {
            Resolution::Checkable { library_id, document_id } => {
                assert_eq!(library_id, "lib-1");
                assert_eq!(document_id, 1);
            }
            other => panic!("expected Checkable, got {other:?}"),
        }

        // … and by lead author + year when it does not.
        let bib2 = parse_numbered_bibliography(
            "[1] M. De Choudhury, S. Counts, Mining large-scale social media, ICWSM, 2013, pp. 1-10.",
        );
        assert!(matches!(
            resolve_marker_with(&db, m, &bib2).unwrap(),
            Resolution::Checkable { .. }
        ));
    }

    #[test]
    fn an_unresolved_number_names_the_work_it_could_not_find() {
        // The old answer was "no numbered bibliography was parsed" for every
        // numeric citation in the paper — true once, and useless. With the list
        // parsed, the reason can say WHICH work is missing, which is the thing
        // the user can act on.
        let db = crate::Database::in_memory().unwrap();
        let bib = parse_numbered_bibliography("[5] S. Mohammad and P. Turney, Crowdsourcing, 2013.");
        let m = &markers_in("Lexicons help [5].")[0];
        match resolve_marker_with(&db, m, &bib).unwrap() {
            Resolution::Unverifiable { reason, library_id } => {
                assert!(reason.contains("[5]"), "{reason}");
                assert!(reason.contains("Mohammad"), "{reason}");
                assert!(reason.contains("not in your library"), "{reason}");
                assert!(library_id.is_none());
            }
            other => panic!("expected Unverifiable, got {other:?}"),
        }
    }

    #[test]
    fn the_dotted_form_is_parsed_only_when_no_bracketed_markers_exist() {
        // A list uses ONE style. Mid-paragraph "12." is far more often the end
        // of a sentence than the start of a reference, so the dotted form is
        // read only line-anchored and only when no bracket markers are present.
        let dotted = "1. R. Plutchik, A general theory of emotion, 1980.\n                      2. S. Hochreiter, Long short-term memory, 1997.\n";
        let bib = parse_numbered_bibliography(dotted);
        assert_eq!(bib.len(), 2, "{bib:?}");
        assert_eq!(bib[&1].year, Some(1980));

        // With brackets present, a stray "12." inside an entry stays part of it
        // rather than starting a phantom reference.
        let mixed = "[1] R. Plutchik, A general theory, 1980, pp. 12. and following.";
        let bib2 = parse_numbered_bibliography(mixed);
        assert_eq!(bib2.len(), 1, "{bib2:?}");
    }

    #[test]
    fn a_reference_list_reflowed_into_one_paragraph_still_parses() {
        // How extraction actually delivers it: the whole list on one line.
        let one_line = "[1]  M. De Choudhury, Mining, 2013, pp. 1-10. [2]  Rudra, Extracting                         situational information, 2015, pp. 583-592. [3]  Liu, Sentiment Analysis.                         Morgan & Claypool, 2012.";
        let bib = parse_numbered_bibliography(one_line);
        assert_eq!(bib.len(), 3, "{bib:?}");
        assert_eq!(bib[&2].lead_author.as_deref(), Some("rudra"));
        assert_eq!(bib[&3].year, Some(2012));
    }

    #[test]
    fn a_numeric_marker_without_a_bibliography_says_so_and_nothing_more() {
        let m = &markers_in("Emotion detection matters [5].")[0];
        assert_eq!(m.numbers, vec![5]);
        assert!(m.lead_author.is_none());
    }

    #[test]
    fn narrative_citations_are_markers_too() {
        // The audit was blind to these, so every sentence citing "Tang et al.
        // (2016)" was counted UNCITED and asked whether it needed a citation.
        for text in [
            "The TD-LSTM was introduced by Tang et al. (2016) to model aspects.",
            "GoEmotions by Demszky et al. (2020) fine-tuned BERT and reached 46%.",
            "Gordon and Burford (1984) reported the opposite.",
            // Doubled space: PDF extraction emits this, and a literal-space
            // pattern misses it.
            "The ATAE-LSTM model was proposed by Wang  et  al.  (2016) for gates.",
        ] {
            let m = markers_in(text);
            assert_eq!(m.len(), 1, "no marker found in {text:?}");
            assert_eq!(m[0].style, MarkerStyle::AuthorYear);
            assert!(m[0].year.is_some(), "no year in {text:?}");
            assert!(m[0].lead_author.is_some(), "no lead author in {text:?}");
        }
    }

    #[test]
    fn a_parenthetical_citation_is_not_counted_twice() {
        let m = markers_in("Yields rose sharply (Smith, 2019) in the trial.");
        assert_eq!(m.len(), 1, "double-counted: {m:?}");
        assert_eq!(m[0].lead_author.as_deref(), Some("smith"));
    }

    #[test]
    fn a_bare_year_in_parentheses_is_not_a_citation() {
        // "(2016) to incorporate …" is the tail of a sentence the splitter cut
        // in half. It names nobody, so it must not resolve to anything.
        assert!(markers_in("(2016) to incorporate the aspect information.").is_empty());
        assert!(markers_in("The study ran from 1998 to (2004) without issue.").is_empty());
    }

    #[test]
    fn the_filter_drops_the_furniture_of_a_real_pdf() {
        use SkipReason::*;
        // Every one of these is a real line from the first audited paper, with
        // its extraction whitespace intact.
        let cases: &[(&str, SkipReason)] = &[
            ("HEFCSO-BILSTM: A HYBRID FIREFLY-CROW SEARCH OPTIMIZED BIDIRECTIONAL LSTM FOR EMOTION DETECTION", FrontMatter),
            ("yadav.neha109@gmail.com Abstract- Emotion detection in social media text must deal with five challenges", FrontMatter),
            ("Keywords- emotion detection, social media NLP, BiLSTM, Firefly Algorithm, Crow Search", FrontMatter),
            ("TABLE I. DATASET STATISTICS Dataset Source Samples Emotions Avg.", TableOrFigure),
            ("Fig. 2. The complete architecture of the proposed system pipeline", TableOrFigure),
            ("ACKNOWLEDGEMENT The authors thank the Department of Computer Science & Engineering", BackMatter),
            ("2) Emoji-to- text replacement followed by the Python-emoji emoji description library.", ListItem),
            ("An attention mechanism computes c = Σ_t α_t h_t, where α_t = SoftMax(v_a^T tanh(W_a h_t))", Notation),
            ("3.2 Methods", TooShort),
        ];
        for (text, want) in cases {
            assert_eq!(skip_reason(text), Some(*want), "wrong verdict for {text:?}");
        }
    }

    #[test]
    fn the_filter_never_drops_a_real_claim_or_a_cited_sentence() {
        // The one thing it must not do. A rule that counted `[1]` as maths
        // dropped the first of these — a genuinely cited claim.
        for keep in [
            "Exact profiling of microemotional states from such  text  carries  points  to  mental  health  monitoring  [1]",
            "In  the  combined  data  set,  the  results  achieved  96.42% accuracy, 95.00% F1-score",
            "The TD-LSTM was introduced by Tang et al. (2016) to model aspects of the sentiment.",
            "Organic management increased soil invertebrate species richness by about 31 percent.",
        ] {
            assert_eq!(skip_reason(keep), None, "wrongly dropped: {keep:?}");
        }
    }

    fn the_significance_filter_drops_headings_and_stubs() {
        assert!(!is_significant("3.2 Methods"));
        assert!(!is_significant("Introduction"));
        assert!(!is_significant("Yields rose."));
        assert!(is_significant("Organic management increased soil invertebrate richness markedly."));
    }

    /// PRECISION on a realistic chapter: 20 markers planted across BOTH styles,
    /// with decoys chosen to be exactly the things a naive regex trips on —
    /// a bare year in prose, a parenthetical aside, a measurement in brackets,
    /// a figure reference, a numeric range outside brackets, and a references
    /// section whose entries look like citations because they are.
    ///
    /// All 20 found, zero false markers. Both halves matter: a missed marker
    /// turns a support check into a spurious "needs citation", and a false
    /// marker sends the audit hunting evidence that was never cited.
    #[test]
    fn the_fixture_chapter_yields_exactly_its_twenty_planted_markers() {
        let text = include_str!("testdata/thesis_chapter.txt");
        let report = prepass(&[(Some(1u32), text.to_string())]);

        assert_eq!(
            report.markers_found, 20,
            "expected 20 markers, found {}: {:#?}",
            report.markers_found,
            report
                .planned
                .iter()
                .flat_map(|p| p.markers.iter().map(|m| m.raw.clone()))
                .collect::<Vec<_>>()
        );

        let all: Vec<&Marker> = report.planned.iter().flat_map(|p| p.markers.iter()).collect();
        let author_year = all.iter().filter(|m| m.style == MarkerStyle::AuthorYear).count();
        let numeric = all.iter().filter(|m| m.style == MarkerStyle::Numeric).count();
        assert_eq!(author_year, 12, "author-year markers");
        assert_eq!(numeric, 8, "numeric markers");

        // the decoys, named individually so a regression says WHICH one broke
        for decoy in [
            "In 2019, the present trial",
            "six organic, six conventional",
            "was low (4.5)",
            "See Figure 3 and Table 2",
            "ranged from 3-5 degrees",
        ] {
            let hit = report
                .planned
                .iter()
                .find(|p| p.sentence.contains(decoy))
                .unwrap_or_else(|| panic!("decoy sentence missing from the plan: {decoy}"));
            assert!(
                hit.markers.is_empty(),
                "decoy {decoy:?} produced a false marker: {:?}",
                hit.markers
            );
        }

        // and the bibliography contributed nothing, despite looking like prose
        assert!(
            !report.planned.iter().any(|p| p.sentence.contains("Journal of Soil Biology")),
            "a reference entry became an audit item"
        );
        assert!(report.cited >= 20 - 1, "cited count disagrees with markers: {}", report.cited);
        assert!(report.uncited > 0, "the chapter has uncited claims and they were not counted");
    }

    /// Run setup SQL and RELEASE the connection. The pool is small, and
    /// `resolve_marker` needs one of its own — holding one across the call
    /// deadlocks rather than failing loudly.
    fn exec(db: &crate::db::Database, sql: &str, p: &[&dyn rusqlite::ToSql]) -> i64 {
        let conn = db.conn().unwrap();
        conn.execute(sql, p).unwrap();
        conn.last_insert_rowid()
    }

    fn marker(lead: &str, year: i32) -> Marker {
        Marker {
            raw: format!("({lead}, {year})"),
            style: MarkerStyle::AuthorYear,
            lead_author: Some(lead.to_lowercase()),
            year: Some(year),
            numbers: Vec::new(),
        }
    }

    /// §11 D40. The three ways a cited sentence can end up, and the fact that
    /// only ONE of them is checkable. Each Unverifiable reason is distinct
    /// because "not in your library" and "in your library but not indexed" are
    /// different things for the reader to do something about.
    #[test]
    fn resolution_distinguishes_missing_unindexed_and_checkable_sources() {
        let db = crate::db::Database::in_memory().unwrap();

        // (a) nothing in the library at all
        let r = resolve_marker(&db, &marker("Nobody", 1999)).unwrap();
        assert!(
            matches!(&r, Resolution::Unverifiable { reason, .. } if reason.contains("not in library")),
            "{r:?}"
        );
        assert_eq!(r.kind(), ItemKind::Unverifiable);

        // (b) in the library, but its source was never indexed
        exec(
            &db,
            "INSERT INTO citation_library (id, csl_json, title, authors, year, created_at, updated_at)
             VALUES ('lib-1', '{}', 'Organic Management and Soil Life', 'Smith, J.', 2019, 1, 1)",
            &[],
        );
        // In the library, but nothing is linked to it yet.
        let r = resolve_marker(&db, &marker("Smith", 2019)).unwrap();
        assert!(
            matches!(&r, Resolution::Unverifiable { reason, .. } if reason.contains("no indexed document is linked")),
            "{r:?}"
        );

        // (c) indexed AND embedded → checkable
        let doc = exec(
            &db,
            "INSERT INTO documents (source_type, title, fetched_at, checksum, status, created_at)
             VALUES ('pdf', 'organic management and soil life', 1, 'ck1', 'ready', 1)",
            &[],
        );
        let chunk = exec(
            &db,
            "INSERT INTO ai_chunks (document_id, page, char_start, char_end, content,
                                    token_estimate, content_hash, created_at)
             VALUES (?1, 1, 0, 10, 'richness rose', 3, 'h1', 1)",
            &[&doc],
        );
        exec(
            &db,
            "INSERT INTO ai_model_registry (id, kind, display_name, file_path, registered_at)
             VALUES ('bge-small-en-v1.5', 'embedding', 'bge', '/x', 1)",
            &[],
        );
        exec(
            &db,
            "INSERT INTO ai_chunk_embeddings (chunk_id, model_id, preprocessing_version, dim, vector, created_at)
             VALUES (?1, 'bge-small-en-v1.5', 'bge-v1.5-p2', 1, X'00', 1)",
            &[&chunk],
        );

        // Phase 8b: availability is a LINK, established once, not a title
        // join recomputed inside every audit.
        crate::citation_links::link_citations(&db).unwrap();

        let r = resolve_marker(&db, &marker("Smith", 2019)).unwrap();
        assert!(matches!(&r, Resolution::Checkable { document_id, .. } if *document_id == doc), "{r:?}");
        assert_eq!(r.kind(), ItemKind::CitationSupport);
    }

    /// Chunks without vectors are NOT checkable: retrieval needs the vectors,
    /// and a document with none returns NoEvidence for every claim — which
    /// would read as a model failure rather than a missing index.
    #[test]
    fn an_indexed_but_unembedded_source_is_unverifiable_not_checkable() {
        let db = crate::db::Database::in_memory().unwrap();
        exec(
            &db,
            "INSERT INTO citation_library (id, csl_json, title, authors, year, created_at, updated_at)
             VALUES ('lib-2', '{}', 'Paired Fields', 'Jones, A.', 2020, 1, 1)",
            &[],
        );
        let doc = exec(
            &db,
            "INSERT INTO documents (source_type, title, fetched_at, checksum, status, created_at)
             VALUES ('pdf', 'paired fields', 1, 'ck2', 'ready', 1)",
            &[],
        );
        exec(
            &db,
            "INSERT INTO ai_chunks (document_id, page, char_start, char_end, content,
                                    token_estimate, content_hash, created_at)
             VALUES (?1, 1, 0, 5, 'text', 1, 'h2', 1)",
            &[&doc],
        );
        crate::citation_links::link_citations(&db).unwrap();
        let r = resolve_marker(&db, &marker("Jones", 2020)).unwrap();
        assert!(
            matches!(&r, Resolution::Unverifiable { reason, .. } if reason.contains("not indexed or not embedded")),
            "chunks without vectors must not be called checkable: {r:?}"
        );
    }

    /// A numeric marker names nobody without a numbered bibliography, and this
    /// phase does not parse one. Said plainly rather than guessed at.
    #[test]
    fn a_numeric_marker_is_unverifiable_by_construction() {
        let db = crate::db::Database::in_memory().unwrap();
        let m = Marker {
            raw: "[3]".into(),
            style: MarkerStyle::Numeric,
            lead_author: None,
            year: None,
            numbers: vec![3],
        };
        let r = resolve_marker(&db, &m).unwrap();
        assert!(
            matches!(&r, Resolution::Unverifiable { reason, .. } if reason.contains("numeric")),
            "{r:?}"
        );
    }

    #[test]
    fn the_references_section_never_becomes_items() {
        let blocks = vec![
            (Some(1u32), "Organic management increased soil invertebrate species richness here.\n".to_string()),
            (Some(9), "References\nSmith, J. (2019). A paper about soil. Journal of Soil, 4, 1-10.\n".to_string()),
        ];
        let r = prepass(&blocks);
        assert_eq!(r.planned.len(), 1, "a reference entry leaked into the plan: {:?}", r.planned);
        assert!(r.planned[0].sentence.starts_with("Organic management"));
    }
}
