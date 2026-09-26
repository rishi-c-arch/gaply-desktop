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
    ///
    /// LOWERCASED because it is a MATCH KEY: `AlJohani`, `Aljohani` and
    /// `ALJOHANI` are one author. Use [`Marker::lead_display`] for anything a
    /// reader sees — lowercasing a surname and printing it back is its own small
    /// defect (§11 D131).
    pub lead_author: Option<String>,
    /// The lead surname AS WRITTEN, for display only. Never a key.
    pub lead_display: Option<String>,
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
    /// The AUTHOR-YEAR reference list, when the paper uses that style (§11
    /// D132). Empty for a numbered paper, and `bibliography` is empty for an
    /// author-year one — a manuscript has one reference list, in one style.
    pub author_year_bibliography: Vec<AuthorYearEntry>,
    /// Entries the author-year parser could not read. Reported, not dropped:
    /// a marker cannot resolve against an entry nobody could parse.
    pub author_year_unreadable: Vec<String>,
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

/// One entry from an AUTHOR-YEAR reference list.
///
/// # Why this lives here and not in `consistency`
///
/// It began there, serving the orphan-marker checks, which need only a surname
/// and a year. The fetch path needs the DOI and the title as well, and §11 D129
/// is the reason this is a MOVE rather than a second parser: two definitions of
/// "what this reference list says" is exactly how one half of a report came to
/// contradict the other. `consistency` now calls this.
///
/// Deliberately not a full CSL parse, for the same reason [`BibEntry`] is not:
/// reference formats vary by publisher and a strict grammar fails on most of
/// them. This takes the parts that are recognisable and leaves `raw`.
#[derive(Debug, Clone, PartialEq, Eq, Default, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthorYearEntry {
    /// Lower-cased leading surname, as `markers_in` reports a marker's — a MATCH
    /// KEY, never display text (§11 D131).
    pub surname: String,
    pub year: Option<i32>,
    /// The DOI the entry prints, if any. **26 of 35 entries in one real
    /// author-year paper carry one and none were read**, because this list had no
    /// parser and the numbered one does not apply (§11 D132).
    pub doi: Option<String>,
    /// The work's title, for display and for the title-lookup path that piece 2
    /// will add. Extracted from between the year and the venue.
    pub title: Option<String>,
    pub raw: String,
}

/// The author-year reference list: everything after the references heading, one
/// entry per block.
///
/// Returns `(parsed, malformed_raws)` — an entry yielding no surname+year is
/// REPORTED rather than dropped, because markers cannot resolve against it.
pub fn parse_author_year_entries(
    blocks: &[crate::extract::docparse::PagedBlock],
) -> (Vec<AuthorYearEntry>, Vec<String>) {
    let Some(start) = blocks.iter().position(|b| is_references_heading(&b.text)) else {
        return (Vec::new(), Vec::new());
    };

    let mut ok = Vec::new();
    let mut bad = Vec::new();
    for b in blocks.iter().skip(start + 1) {
        let t = b.text.split_whitespace().collect::<Vec<_>>().join(" ");
        let t = t.trim();
        // Too short to be a reference: a page number, a running header.
        if t.split_whitespace().count() < 5 {
            continue;
        }
        match ay_entry_re().captures(t) {
            Some(c) => ok.push(AuthorYearEntry {
                surname: c["surname"].to_lowercase(),
                year: c.name("year").or_else(|| c.name("bare")).and_then(|m| m.as_str().parse().ok()),
                // SHARED with the numbered parser — one definition of what a
                // DOI looks like.
                doi: doi_in_re()
                    .find(t)
                    .map(|m| m.as_str().trim_end_matches(|c| c == '.' || c == ',').to_string()),
                title: title_after_year(t, c.get(0).map(|m| m.end()).unwrap_or(0)),
                raw: t.to_string(),
            }),
            None => bad.push(t.to_string()),
        }
    }
    (ok, bad)
}

/// The title, taken from just after the `(Year)` the entry opens with.
///
/// APA puts the title immediately after the year and ends it with a full stop
/// before the venue:
///
/// ```text
/// Alkenbrack, S., … (2015). Evasion of “mandatory” social health insurance …. BMC Health Serv Res, 15, 473.
///                           ^------------------- title -----------------^
/// ```
///
/// A title containing ". " would be cut short. That is a visible, bounded
/// wrongness — a short title — and NOT the kind that matters here: this string
/// is display text and, later, a search query whose match is scored against the
/// year and surname too. It is never an identifier on its own.
fn title_after_year(entry: &str, year_end: usize) -> Option<String> {
    let rest = entry.get(year_end..)?.trim_start_matches(['.', ' ']);
    let end = rest.find(". ").unwrap_or(rest.len());
    let t = rest[..end].trim().trim_end_matches('.').trim();
    (t.split_whitespace().count() >= 2).then(|| t.to_string())
}

/// "Alkenbrack, S., Hanson, K., & Lindelow, M. (2015). Title…", the
/// organisational form "P4H Network. (2024).", and the UNBRACKETED year
/// "Göncü E and Parlak O. 2011. Title…".
///
/// The unbracketed form is `. YYYY. `: the author list's closing full stop,
/// the year, and a full stop. Two styles in the corpus print it (IJAS's
/// `Surname I. 2011.` and the final-L thesis's `Surname, Given. 2007.`).
/// Requiring `(YYYY)` read all 26 IJAS entries as unparseable. Same proximity
/// rule as the bracketed year. The full stops on both sides are what keep a
/// sample size ("of 2000 households") or a page range ("1885–1914.") from
/// reading as a year; a sentence-shaped ". 2000. " that is not one would still
/// be read as one, and no such line was found in the corpus.
///
/// Requiring the comma reported both organisational entries in a real paper as
/// unreadable, which is the check calling correct APA wrong. Digits belong in a
/// name: "P4H Network" is an organisation, and requiring a letter after the
/// capital called it unreadable.
fn ay_entry_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(
            r"^\s*(?P<surname>[A-Z][\p{L}\p{N}'’\-]*)[^()]{0,300}?(?:\((?P<year>(?:1[6-9]|20)\d{2})[a-z]?\)|\.\s(?P<bare>(?:1[6-9]|20)\d{2})[a-z]?\.\s)",
        )
        .expect("author-year entry regex")
    })
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
            lead_display: c.name("lead").map(|m| m.as_str().to_string()),
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
            lead_display: c.name("lead").map(|m| m.as_str().to_string()),
            year: c.name("year").and_then(|m| m.as_str().parse::<i32>().ok()),
            raw: whole.to_string(),
            style: MarkerStyle::AuthorYear,
            numbers: Vec::new(),
        });
    }
    for m in numeric_re().find_iter(sentence) {
        let raw = m.as_str().to_string();
        let numbers: Vec<u32> = number_run_re()
            .find_iter(&raw)
            .filter_map(|d| d.as_str().parse::<u32>().ok())
            .collect();
        // INTERVAL NOTATION IS NOT A CITATION (§11 D130).
        //
        // "standardised to [0, 1] by (score − 1)/4" was reported as a cited
        // sentence, so it left the queue and was never examined, and the report
        // told the researcher it was "cited, but not checkable — numeric
        // citation style". Two such in one paper.
        //
        // A ZERO is the safe discriminator and it is safe for a structural
        // reason, not a statistical one: a numbered reference list starts at
        // [1], so no citation marker can contain 0. Measured across both
        // audited papers — R PAPER parses 21 numeric markers and NOT ONE
        // contains a zero, so the rule costs nothing where numeric citations
        // are real.
        //
        // A comma-separated PAIR was considered as a second rule and rejected:
        // `[5, 7]` is a standard multi-citation, and dropping it would silently
        // un-cite a real claim. Neither paper contains one (0 of 0), so there is
        // no evidence for the rule and a real cost if it is wrong. The
        // document-level check in `prepass_blocks` covers the same cases more
        // safely.
        if numbers.iter().any(|n| *n == 0) {
            continue;
        }
        out.push(Marker {
            raw,
            style: MarkerStyle::Numeric,
            lead_author: None,
            lead_display: None,
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
    // §11 D106. The label does not always LEAD. PDF reflow routinely leaves it
    // at the end — "Accuracy and F1-Score Comparison on SemEval-2018 Fig. 3."
    // — and that sentence was surviving as a claim and being offered for
    // labelling, where nothing in the source could ever adjudicate it.
    //
    // Prose that legitimately cites a figure reaches it through a preposition
    // ("depicted in Fig. 1", "shown in Table III"); a caption abuts its label.
    // That is the discriminator, measured on R PAPER: of the five planned
    // sentences ending in a label, it drops the one caption and keeps the four
    // real cross-references.
    if ends_with_bare_figure_label(&flat) {
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

/// The fewest words a block must have before it may ABSORB the one after it.
///
/// Table cells and stray labels are one or two words and sit next to each other
/// in reading order; without a floor, "Dataset" swallows "Source".
const MIN_WORDS_TO_JOIN: usize = 4;

/// Does this block stop mid-sentence?
///
/// Deliberately NOT abbreviation-aware. `ends_with_abbreviation` would call
/// "…proposed by Smith et al." open, and in PROSE that is far more often a real
/// sentence end than a wrap. The three wrapped blocks this exists for
/// ("(6 GB", "six evaluation", "GloVe costs") end on no punctuation at all, so
/// the abbreviation clause buys nothing and costs false joins.
fn ends_open(text: &str) -> bool {
    let t = text.trim_end();
    if t.is_empty() {
        return false;
    }
    // A terminator behind a closing quote or bracket still closes.
    let core = t.trim_end_matches(|c| matches!(c, '"' | '\'' | ')' | ']' | '\u{201d}' | '\u{2019}'));
    !matches!(core.chars().next_back(), Some('.' | '!' | '?' | ':' | ';'))
}

/// Does this block read as the CONTINUATION of the one before it?
fn continues_sentence(text: &str) -> bool {
    let t = text.trim_start();
    match t.chars().next() {
        Some(c) if c.is_lowercase() => true,
        Some(')' | ']' | ',' | ';') => true,
        _ => starts_with_numeric_literal(t),
    }
}

/// Does `t` open with a bare number followed by a space?
///
/// This is the clause that separates "3.3 points; SMOTE-Text…" (the tail of a
/// wrapped sentence) from "1,3,4Department of Computer Science" (a superscript
/// affiliation marker) and "1. Introduction" (a numbered heading). Only a
/// numeric literal that a SPACE closes counts.
fn starts_with_numeric_literal(t: &str) -> bool {
    let b = t.as_bytes();
    let mut i = 0;
    let mut seen_dot = false;
    while i < b.len() {
        if b[i].is_ascii_digit() {
            i += 1;
        } else if b[i] == b'.'
            && !seen_dot
            && b.get(i + 1).is_some_and(u8::is_ascii_digit)
        {
            seen_dot = true;
            i += 1;
        } else {
            break;
        }
    }
    i > 0 && matches!(b.get(i), Some(c) if c.is_ascii_whitespace())
}

/// An unclosed `(` is a wrap that no punctuation signals.
fn has_unclosed_paren(text: &str) -> bool {
    text.matches('(').count() > text.matches(')').count()
}

/// May block `a` absorb block `b`?
///
/// # A BREAK THE READER CAN SEE IS THE DOCUMENT'S; ONE THEY CANNOT IS OURS
///
/// This is the whole rule, and it is why `style_is_list` is refused here.
///
/// In body prose a paragraph break is INVISIBLE — Word renders the wrap and the
/// paragraph identically, so a sentence split across two blocks is our problem
/// to repair, and leaving it produces fragments that reach the model as if they
/// were claims ("RAM), Python 3.9, TensorFlow 2.10, and NLTK 3.7.").
///
/// In an AUTO-NUMBERED LIST the same break is VISIBLE: Word puts a number in
/// front of it, so the reader's reference [6] really is the page range that
/// wrapped. Repairing that would delete a true finding — §11 D67 settled this
/// on the same paper, and §11 D122 records the run that proved the rejoin does
/// not reach it.
fn may_absorb(a: &crate::extract::docparse::PagedBlock, b: &crate::extract::docparse::PagedBlock) -> bool {
    let (at, bt) = (a.text.trim(), b.text.trim());
    if at.is_empty() || bt.is_empty() {
        return false;
    }
    for s in [a.style.as_deref(), b.style.as_deref()] {
        if style_is_heading(s) || style_is_table(s) || style_is_list(s) {
            return false;
        }
    }
    if at.split_whitespace().count() < MIN_WORDS_TO_JOIN || !ends_open(at) {
        return false;
    }
    // The paren clause must name the block that CLOSES the paren, not merely
    // the next one. Without `bt.contains(')')` the caption
    // "TABLE III. PER-EMOTION PERFORMANCE (HEFCSO-" absorbed 40 consecutive
    // table cells — "Emotion", "Precision (%)", "Joy", "96.55" — because the
    // unclosed paren survived every join and re-armed the rule each time. Those
    // cells carry NO declared style in this file, so `style_is_table` never saw
    // them; the word floor caught them only once the caption stopped chaining.
    continues_sentence(bt) || (has_unclosed_paren(at) && bt.contains(')'))
}

/// Rejoin blocks that a wrapped line split mid-sentence.
///
/// # THE ABSORBED BLOCK IS BLANKED, NEVER REMOVED
///
/// `prepass_blocks` counts a paragraph ordinal over EVERY block, skipped ones
/// included, because that locator has to match what the reader counts in their
/// own document. Dropping a block would shift every ordinal after it. So the
/// text moves and the block stays: an empty block yields no sentences and still
/// takes its number.
///
/// Stops at the references heading. Everything past it is a bibliography, which
/// `prepass_blocks` accumulates whole on a different branch — see `may_absorb`
/// for why that must stay untouched.
fn rejoin_wrapped_blocks(
    blocks: &[crate::extract::docparse::PagedBlock],
) -> Vec<crate::extract::docparse::PagedBlock> {
    let mut out = blocks.to_vec();
    let mut i = 0usize;
    // Defence in depth behind the `may_absorb` rules: a sentence that wraps
    // across more than three blocks is not a wrap, it is a rule that has
    // stopped discriminating. Bounded chaining turns that into three wrong
    // joins instead of forty.
    const MAX_CHAIN: usize = 3;
    let mut chain = 0usize;
    while i < out.len() {
        if out[i].text.lines().any(is_references_heading) {
            break;
        }
        let Some(j) = (i + 1..out.len()).find(|k| !out[*k].text.trim().is_empty()) else {
            break;
        };
        if out[j].text.lines().any(is_references_heading) {
            break;
        }
        if chain < MAX_CHAIN && may_absorb(&out[i], &out[j]) {
            let joined = format!("{} {}", out[i].text.trim_end(), out[j].text.trim_start());
            out[i].text = joined;
            out[j].text = String::new();
            // Do NOT advance: a sentence may wrap across three blocks.
            chain += 1;
            continue;
        }
        i = j;
        chain = 0;
    }
    out
}

/// Is this bibliography entry not a reference at all?
///
/// NO AUTHOR is the discriminator, on its own. A reference begins with who wrote
/// it; a continuation of the entry above does not.
///
/// The first rule tried was "no author AND no year", and it MISSED the real
/// defect: `[6] pp. 436–465, 2013.` carries a year, because a page range ends
/// with one. Requiring both let the very entry this check exists for through.
/// The year is no help: entry [5] of the same paper is a genuine reference whose
/// year did not parse.
///
/// LIVES HERE, beside [`BibEntry`], because TWO callers need it and they must
/// agree: `consistency::check_reference_list` raises the structural finding, and
/// [`first_unreliable_entry`] stops marker resolution trusting the numbering it
/// describes. A second definition would let the report contradict itself
/// (§11 D129).
pub(crate) fn entry_is_malformed(e: &BibEntry) -> bool {
    e.lead_author.is_none()
}

/// The LOWEST entry number at or above which the reference list's numbering
/// cannot be trusted, if any.
///
/// An auto-numbered list takes its ordinals from POSITION, so one entry that is
/// really the wrapped tail of the entry above shifts every ordinal after it by
/// one. Entries BELOW the break are unaffected and still resolve correctly;
/// the malformed entry itself and everything above it do not.
///
/// This is the fact `resolve_marker_with` needs and did not have. See §11 D129
/// for what printing a confident resolution without it looked like.
pub fn first_unreliable_entry(bibliography: &BTreeMap<u32, BibEntry>) -> Option<u32> {
    bibliography.values().filter(|e| entry_is_malformed(e)).map(|e| e.number).min()
}

/// The `[n]` a reference entry already carries, if any.
fn leading_bib_marker(text: &str) -> Option<u32> {
    let t = text.trim_start();
    let rest = t.strip_prefix('[')?;
    let end = rest.find(']')?;
    rest[..end].trim().parse::<u32>().ok()
}

/// Is this sentence worth a model call at all?
/// Does this end in a figure/table label that labels rather than refers?
///
/// "…depicted in Fig. 1." is prose about a figure and stays. "…Comparison on
/// SemEval-2018 Fig. 3." is a caption whose label migrated to the end and goes.
///
/// TWO signals must agree before anything is dropped, because the directions
/// are not symmetric: keeping a caption costs a noisy labelling candidate,
/// while dropping a claim costs a check the audit exists to perform. The first
/// draft of this dropped whenever the word before the label was not a known
/// connective — i.e. it dropped BY DEFAULT on unfamiliar phrasing, which is the
/// wrong way round and is what its own test caught (§11 D106).
///
/// So: the label must not be introduced by a connective, AND the sentence must
/// carry no finite verb. A caption is a noun phrase; prose about a figure says
/// that something *is depicted*, *are presented*, *shows*.
fn ends_with_bare_figure_label(flat: &str) -> bool {
    /// Words that introduce a reference TO a figure, rather than label one.
    const CONNECTIVES: &[&str] = &[
        "in", "see", "from", "of", "to", "on", "at", "by", "and", "with", "per", "cf", "cf.",
        "versus", "vs", "vs.", "into", "within", "under", "above", "below", "via",
    ];
    /// Enough of a finite verb to say this is a sentence, not a label.
    const VERBS: &[&str] = &[
        "is", "are", "was", "were", "be", "been", "has", "have", "had", "can", "may", "will",
        "shows", "show", "presents", "present", "presented", "depicts", "depicted", "shown",
        "illustrates", "illustrated", "summarises", "summarizes", "summarised", "summarized",
        "compares", "compared", "reports", "reported", "gives", "given", "demonstrates",
        "uses", "used", "lists", "listed", "provides", "provided", "indicates", "indicated",
        "displays", "displayed", "describes", "described", "contains", "highlights",
    ];
    let w: Vec<&str> = flat.split_whitespace().collect();
    // A finite verb anywhere means this reads as a sentence, whatever it ends
    // with. Checked first because it is the signal that protects real claims.
    if w.iter().any(|t| {
        let t = t.to_lowercase();
        VERBS.contains(&t.trim_matches(|c: char| !c.is_alphanumeric()))
    }) {
        return false;
    }
    // A bare "Fig. 3." is already caught by the leading rule and by TooShort;
    // this needs a word before the label to judge.
    if w.len() < 3 {
        return false;
    }
    let number = w[w.len() - 1].trim_end_matches(['.', ':', ')']);
    let looks_numbered = !number.is_empty()
        && number.chars().all(|c| c.is_ascii_digit() || "IVXLCDM".contains(c.to_ascii_uppercase()));
    if !looks_numbered {
        return false;
    }
    let label = w[w.len() - 2].to_lowercase();
    let label = label.trim_end_matches('.');
    if !matches!(label, "fig" | "figure" | "table") {
        return false;
    }
    let before = w[w.len() - 3].to_lowercase();
    let before = before.trim_end_matches([',', ';']);
    !CONNECTIVES.contains(&before)
}

pub fn is_significant(sentence: &str) -> bool {
    skip_reason(sentence).is_none()
}

/// Run the whole deterministic pass over paged blocks.
///
/// Takes `(page, text)` pairs rather than a path so it stays pure and testable
/// — the caller does the parsing with `extract::docparse::parse_path_paged`.
/// The structural skeleton `audit_report::compose_audit` always emits (§11 D80).
///
/// Deliberately the report's FURNITURE — its cover title, its page footer and
/// its section headings — and not any sentence it happens to contain. Furniture
/// is what a Gaply report has and a manuscript does not; a sentence about
/// citations is something both can have.
///
/// Each entry must be text `compose_audit` emits verbatim. The test
/// `every_marker_appears_in_a_real_composed_report` holds that against the real
/// composer, so a heading renamed there fails here instead of silently
/// weakening the guard to the point where it stops firing.
pub const GAPLY_REPORT_MARKERS: &[&str] = &[
    "Thesis citation audit",
    "PublishReady",
    "At a glance",
    "The counts",
    // §11 D108 renamed this section. The OLD heading stays in the list: reports
    // exported before that change are still Gaply reports, and the guard exists
    // to recognise them.
    "Claims checked against their source",
    "The source passages behind each cited claim",
    "Cited, but not checkable",
    "Worth a second look",
    "Not judged",
];

/// How many DISTINCT markers must appear before a document is refused.
///
/// Not one. A genuine manuscript may quote one of these headings, and a paper
/// about this tool would name several deliberately — refusing on a single hit
/// would block real work to prevent a mistake. Our own report emits every
/// marker in the list, so three is comfortably clear of anything a manuscript
/// reaches by accident while still leaving the guard no way to miss a report.
pub const GAPLY_REPORT_MIN_MARKERS: usize = 3;

/// Which of the skeleton's markers this text contains, in list order.
///
/// Case-sensitive on purpose: these are rendered headings, not prose, and
/// lowercasing would match a sentence that merely mentions "the counts".
pub fn gaply_report_markers_in(text: &str) -> Vec<&'static str> {
    GAPLY_REPORT_MARKERS.iter().copied().filter(|m| text.contains(m)).collect()
}

/// Is this document one of OUR reports rather than a manuscript?
///
/// Returns the markers that decided, so the refusal can name them and a false
/// positive is arguable rather than mysterious.
pub fn detect_gaply_report(
    blocks: &[crate::extract::docparse::PagedBlock],
) -> Option<Vec<&'static str>> {
    let text = blocks.iter().map(|b| b.text.as_str()).collect::<Vec<_>>().join("\n");
    let found = gaply_report_markers_in(&text);
    (found.len() >= GAPLY_REPORT_MIN_MARKERS).then_some(found)
}

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
    // A wrapped line that Word recorded as its own paragraph is repaired FIRST,
    // so the loop below never sees half a sentence (§11 D122). Ordinals are
    // preserved: the absorbed block stays, emptied.
    let rejoined = rejoin_wrapped_blocks(blocks);
    for block in &rejoined {
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

    // A DOCUMENT-LEVEL RULE WAS CONSIDERED HERE AND REJECTED (§11 D130).
    //
    // "A `[n]` in a paper with no numbered reference list is not a citation"
    // looks stronger than the lexical zero rule and is more dangerous. It was
    // implemented, and the planted-marker fixture caught it immediately: that
    // chapter carries numeric markers and no parsed list, so the rule dropped
    // ALL TWELVE. The same thing happens to a real manuscript whose reference
    // list simply fails to parse — every numerically-cited sentence silently
    // becomes "not examined", and the audit quietly does nothing.
    //
    // It also buys nothing measurable: the zero rule alone already takes the
    // health-economics paper from 2 false numeric markers to 0. A rule whose
    // benefit is unmeasurable and whose failure mode is "the audit silently
    // checks nothing" is the wrong trade, and §11 D127's asymmetry is what says
    // so — a missed marker costs one unexamined sentence, a rule like this costs
    // the whole document.
    // The author-year list, parsed only when the numbered one yielded nothing.
    // Both are never populated: a manuscript has ONE reference list in one
    // style, and trying both and keeping whichever is larger would invent a
    // second source of truth about the same text (§11 D132).
    if report.bibliography.is_empty() {
        let (ok, bad) = parse_author_year_entries(blocks);
        report.author_year_bibliography = ok;
        report.author_year_unreadable = bad;
    }

    report
}

/// The manuscript's reference list, whichever style it is in.
///
/// One manuscript has ONE list; the unused side is empty (§11 D132). Passed as a
/// pair rather than as "the numbered map" because resolution needs the
/// author-year side too: that is where an author-year marker's DOI lives, and the
/// DOI is how a fetched source is found (§11 D133).
#[derive(Debug, Clone, Copy)]
pub struct Bibliography<'a> {
    pub numbered: &'a BTreeMap<u32, BibEntry>,
    pub author_year: &'a [AuthorYearEntry],
}

impl<'a> Bibliography<'a> {
    /// A numbered list alone — the shape most callers and every numeric test has.
    pub fn numbered(numbered: &'a BTreeMap<u32, BibEntry>) -> Self {
        Self { numbered, author_year: &[] }
    }

    /// Both sides, as a real pre-pass produces them.
    pub fn of(report: &'a PrepassReport) -> Self {
        Self { numbered: &report.bibliography, author_year: &report.author_year_bibliography }
    }

    /// The DOI this marker's own reference entry prints, if any.
    ///
    /// The bridge between a marker and a fetched PDF. An author-year marker
    /// carries a surname and a year and NOT a DOI; the entry carries the DOI and
    /// not the marker's spelling. Neither alone can find the file.
    pub fn doi_for(&self, marker: &Marker) -> Option<&'a str> {
        if let Some(n) = marker.numbers.first() {
            return self.numbered.get(n)?.doi.as_deref();
        }
        let lead = marker.lead_author.as_deref()?;
        self.author_year
            .iter()
            .find(|e| e.surname == lead && (marker.year.is_none() || e.year == marker.year))
            .and_then(|e| e.doi.as_deref())
    }
}

/// What the library lookup concluded for one planned sentence.
#[derive(Debug, Clone, PartialEq)]
pub enum Resolution {
    /// The cited work has a source that is indexed + embedded.
    ///
    /// `via` says WHICH route it came in by (§11 D133): the user's library, or a
    /// reference staged from the manuscript (§11 D132). This was
    /// `library_id: String`, which could not express the second — so a staged
    /// source whose PDF had been fetched and embedded still resolved as
    /// unverifiable, and the whole staging path produced ZERO checkable
    /// sentences.
    Checkable {
        via: crate::source_ref::SourceRef,
        document_id: i64,
        /// The linked document is an ABSTRACT, not the full text (§11 D138).
        ///
        /// Carried rather than filtered. An abstract legitimately supports some
        /// claims — "GoEmotions reports 46% macro-F1" is in the abstract — so
        /// refusing it would drop real checks. What it must not do is LOOK like a
        /// full-text check: the distinction travels to the reader, and out of any
        /// count that implies full-text verification.
        abstract_only: bool,
    },
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
        Some((document_id, _)) => Ok(Some(Resolution::Checkable {
            via: crate::source_ref::SourceRef::Citation { citation_id: library_id },
            document_id,
            abstract_only: super::store::is_abstract_only(db, document_id)?,
        })),
        None => Ok(Some(Resolution::Unverifiable {
            reason: format!(
                "[{}] is in your library but its source is not indexed. Link the document.",
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
    resolve_marker_with(db, marker, &Bibliography::numbered(&BTreeMap::new()))
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
    bib: &Bibliography<'_>,
) -> Result<Resolution, crate::GaplyError> {
    use rusqlite::params;

    let bibliography = bib.numbered;

    // A numeric marker resolves through the bibliography, when there is one.
    if marker.lead_author.is_none() && !marker.numbers.is_empty() {
        if bibliography.is_empty() {
            return Ok(Resolution::Unverifiable {
                reason: "numeric citation style: no numbered bibliography was parsed".to_string(),
                library_id: None,
            });
        }
        // WHERE THE NUMBERING STOPS BEING TRUSTWORTHY (§11 D129).
        //
        // One entry that is really the wrapped tail of the entry above shifts
        // every ordinal after it by one, and `consistency` raises a STRUCTURAL
        // finding saying exactly that. Resolution used to ignore it and name the
        // entry sitting at position `n` anyway — so one report told a researcher
        // "every marker above [6] resolves to the wrong paper" and then, lower
        // down, "[9] Whitley, A genetic algorithm tutorial — not in your
        // library" for a sentence about Yang's firefly algorithm, with an action
        // attached. A dozen of those.
        //
        // The guard runs BEFORE `resolve_bib_entry`, which matters more than the
        // wording: had Whitley been in the library, indexed and embedded, the
        // claim would have been CHECKED against an unrelated paper and the
        // report would have quoted its passages as evidence. A wrong fetch
        // instruction is visible; a wrong verification is not.
        let unreliable_from = first_unreliable_entry(bibliography);
        let mut missing: Vec<String> = Vec::new();
        for n in &marker.numbers {
            if let Some(first) = unreliable_from {
                if *n >= first {
                    missing.push(format!(
                        "[{n}] cannot be resolved. Entry [{first}] of the reference list is not a \
                         reference, so every number from [{first}] up points at the wrong entry. \
                         Fix the numbering and re-run; until then Gaply will not name a work for \
                         this marker"
                    ));
                    continue;
                }
            }
            let Some(entry) = bibliography.get(n) else {
                missing.push(format!("[{n}] is not in the reference list"));
                continue;
            };
            match resolve_bib_entry(db, entry)? {
                Some(r) => return Ok(r),
                None => missing.push(format!(
                    "[{n}] {}: not in your library",
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
        // AN AUTHOR-YEAR MARKER IDENTIFIES A WORK ONLY APPROXIMATELY, AND THE
        // REPORT NOW SAYS SO (§11 D131).
        //
        // This used to embed `marker.raw`, which made the report wrong twice.
        //
        // (1) IT COUNTED ONE WORK AS SEVERAL. The raw text is the grouping key
        //     downstream, so `(AlJohani & Bugis, 2024)`, `(AlJohani and Bugis,
        //     2024)` and `AlJohani and Bugis (2024)` became three rows for one
        //     paper. Three spellings of Alkenbrack and three of Reka did the
        //     same, and "48 sources not in your library" overstated the work by
        //     roughly a third. The canonical surname-and-year below is one key
        //     for all spellings.
        //
        // (2) IT PRINTED AN UNCERTAIN EXTRACTION AS A CERTAIN WORK. The
        //     surname the regex captures is not reliably the lead author:
        //     measured on a real paper it produced "Authority (2025)" from "The
        //     Financial Services Authority (2025)", "Valletta (2011)" from
        //     "Buchmueller, DiNardo, and Valletta (2011)", "Saksena and Kutzin
        //     (2019)" from "Mathauer, Saksena and Kutzin (2019)", "Weiner's
        //     (2009)" with the possessive, and "(RBV; Barney, 1991)" with the
        //     abbreviation. `consistency` already reports this class as
        //     `uncertain-reference-match` rather than asserting it; this list was
        //     asserting it anyway, with a fetch action attached. Same rule as
        //     §11 D129: an uncertain resolution must not print as a certain one.
        // A SOURCE STAGED FROM THE MANUSCRIPT AND ALREADY FETCHED (§11 D133).
        //
        // Not in the library — staging deliberately never puts it there — but its
        // PDF may be on disk, indexed and embedded. Found by the DOI the
        // manuscript's OWN reference entry prints, which is why `Bibliography`
        // carries the author-year side: the marker has a surname and a year, the
        // entry has the DOI, and neither alone can find the file.
        //
        // Checked AFTER the library, matching the staging rule that the library
        // wins: the user's curated copy is preferred to the audit's own.
        if let Some(doi) = bib.doi_for(marker) {
            if let Some(document_id) = crate::staged_sources::checkable_document_for_doi(db, doi)? {
                return Ok(Resolution::Checkable {
                    via: crate::source_ref::SourceRef::Staged {
                        staged_id: crate::staged_sources::id_for_doi(db, doi)?.unwrap_or(0),
                    },
                    document_id,
                    abstract_only: super::store::is_abstract_only(db, document_id)?,
                });
            }
        }

        let surname = marker.lead_display.as_deref().unwrap_or(lead);
        let year = marker.year.map(|y| y.to_string()).unwrap_or_else(|| "no year".into());
        return Ok(Resolution::Unverifiable {
            reason: format!(
                "no library work matches “{surname}, {year}”: taken from the in-text marker, \
                 which gives a surname and a year and not the full author list, so this may be \
                 an incomplete or mis-split name rather than a missing source"
            ),
            library_id: None,
        });
    };

    match crate::citation_links::checkable_document_for_citation(db, &library_id)? {
        Some((document_id, _matched_by)) => {
            Ok(Resolution::Checkable {
                via: crate::source_ref::SourceRef::Citation { citation_id: library_id },
                document_id,
                abstract_only: super::store::is_abstract_only(db, document_id)?,
            })
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

    fn blk_ay(text: &str) -> crate::extract::docparse::PagedBlock {
        crate::extract::docparse::PagedBlock { page: None, style: None, text: text.to_string() }
    }

    fn blk(style: Option<&str>, text: &str) -> crate::extract::docparse::PagedBlock {
        crate::extract::docparse::PagedBlock {
            page: None,
            style: style.map(str::to_string),
            text: text.to_string(),
        }
    }

    /// The three wrapped sentences from `R PAPER .docx`, verbatim.
    ///
    /// Each reached the model as a fragment that reads like a claim, and two of
    /// the three were then FLAGGED as needing a citation (§11 D122).
    #[test]
    fn a_sentence_wrapped_across_two_blocks_is_rejoined() {
        let cases = [
            (
                "I carried out all the experiments on an Intel Core i7-11800H CPU, 16 GB RAM, NVIDIA GeForce RTX 3060 (6 GB",
                "RAM), Python 3.9, TensorFlow 2.10, and NLTK 3.7.",
            ),
            (
                "Figure 4 is an exemplary showpiece of a multi-metric radar plot aiding comparisons between HEFCSO-BiLSTM, Std. BiLSTM, and ATAE-LSTM across a total of six evaluation ",
                "metrics. HEFCSO-BiLSTM not only captures a significantly bigger area.",
            ),
            (
                "If the attention layer is removed it costs 2.1 points; GloVe costs",
                "3.3 points; SMOTE-Text costs 2.3 points.",
            ),
        ];
        for (a, b) in cases {
            let out = rejoin_wrapped_blocks(&[blk(None, a), blk(None, b)]);
            assert_eq!(out.len(), 2, "a block may never be REMOVED — ordinals shift");
            assert!(
                out[0].text.contains(a.trim()) && out[0].text.contains(b.trim()),
                "expected {a:?} + {b:?} to be rejoined, got {:?}",
                out[0].text
            );
            assert!(out[1].text.is_empty(), "the absorbed block is blanked, not dropped");
        }
    }

    /// THE CASE THAT MUST NOT BE "FIXED" — §11 D67, §11 D122.
    ///
    /// In an auto-numbered list Word renders a number in front of the wrapped
    /// line, so the reader's reference [6] REALLY IS the page range. Joining it
    /// would delete a true structural finding.
    #[test]
    fn a_wrapped_line_in_an_auto_numbered_list_is_left_alone() {
        let out = rejoin_wrapped_blocks(&[
            blk(
                Some("ListParagraph"),
                "S. Mohammad and P. Turney, \u{201c}Crowdsourcing a word-emotion association lexicon,\u{201d} Comput. Intell., vol. 29, no. 3,",
            ),
            blk(Some("ListParagraph"), "pp. 436\u{2013}465, 2013."),
        ]);
        assert_eq!(out[1].text, "pp. 436\u{2013}465, 2013.", "a VISIBLE break belongs to the document");
    }

    /// Everything after the references heading is a bibliography, and it is
    /// accumulated whole on another branch. The rejoin stops there even when
    /// the entries carry no list style.
    #[test]
    fn rejoining_stops_at_the_references_heading() {
        let out = rejoin_wrapped_blocks(&[
            blk(None, "References"),
            blk(None, "T. Cover and P. Hart, Nearest neighbor pattern classification, vol. 13, no. 1,"),
            blk(None, "pp. 21-27, 1967."),
        ]);
        assert_eq!(out[2].text, "pp. 21-27, 1967.");
    }

    /// THE CASCADE THIS RULE ALMOST SHIPPED.
    ///
    /// "TABLE III. …(HEFCSO-" holds an unclosed paren, and these cells carry NO
    /// declared style, so `style_is_table` cannot see them. Before the paren
    /// clause required the block that CLOSES it, this caption absorbed forty
    /// consecutive cells and 44 sentences vanished from the pre-pass.
    #[test]
    fn an_unclosed_paren_may_not_absorb_a_table_of_undeclared_cells() {
        let mut blocks = vec![blk(None, "TABLE III. PER-EMOTION PERFORMANCE (HEFCSO-")];
        for cell in ["Emotion", "Precision (%)", "Joy", "96.55", "93.33", "Sadness", "97.12"] {
            blocks.push(blk(None, cell));
        }
        let out = rejoin_wrapped_blocks(&blocks);
        assert_eq!(
            out[0].text, "TABLE III. PER-EMOTION PERFORMANCE (HEFCSO-",
            "the caption absorbed a table"
        );
        for (i, b) in out.iter().enumerate().skip(1) {
            assert!(!b.text.is_empty(), "cell {i} was absorbed");
        }
    }

    /// A superscript affiliation marker is not a wrapped sentence.
    #[test]
    fn a_leading_affiliation_digit_does_not_continue_the_line_above() {
        let out = rejoin_wrapped_blocks(&[
            blk(None, "Neha Yadav1, Rakhi Sharma2, Poonam Sharma3, Sarika Chaudhary4"),
            blk(None, "1,3,4Department of Computer Science & Engineering"),
        ]);
        assert_eq!(out[1].text, "1,3,4Department of Computer Science & Engineering");
    }

    #[test]
    fn a_numeric_literal_continues_but_a_numbered_heading_does_not() {
        assert!(starts_with_numeric_literal("3.3 points; SMOTE-Text costs 2.3 points."));
        assert!(!starts_with_numeric_literal("1,3,4Department of Computer Science"));
        assert!(!starts_with_numeric_literal("1. Introduction"));
        assert!(!starts_with_numeric_literal("2013."));
    }

    /// Bounded chaining, so a rule that stops discriminating fails small.
    ///
    /// The cap is PER STARTING BLOCK, not per document: eleven runaway blocks
    /// become several bounded joins rather than one block of eleven. What must
    /// never happen is the forty-cell cascade — one block swallowing a section.
    #[test]
    fn no_block_absorbs_more_than_the_chain_cap() {
        let blocks: Vec<_> = (0..11)
            .map(|n| blk(None, &format!("marker{n} the sentence runs onward without stopping")))
            .collect();
        let out = rejoin_wrapped_blocks(&blocks);
        for (i, b) in out.iter().enumerate() {
            let absorbed = (0..11).filter(|n| b.text.contains(&format!("marker{n}"))).count();
            assert!(
                absorbed <= MAX_CHAIN_TEST + 1,
                "block {i} absorbed {absorbed} blocks: {:?}",
                b.text
            );
        }
    }

    /// Mirrors `MAX_CHAIN` in `rejoin_wrapped_blocks`; a change there that is
    /// not reflected here fails the test above rather than passing quietly.
    const MAX_CHAIN_TEST: usize = 3;

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

    /// §11 D132. THE AUTHOR-YEAR LIST CARRIES DOIs AND NOBODY READ THEM.
    ///
    /// Measured on a real paper: 35 entries, **26 with a DOI**, and the audit
    /// saw zero of them because `parse_numbered_bibliography` does not apply to
    /// an author-year list and nothing else looked.
    #[test]
    fn an_author_year_entry_yields_its_surname_year_doi_and_title() {
        let (ok, bad) = parse_author_year_entries(&[
            blk_ay("References"),
            blk_ay(
                "Alkenbrack, S., Hanson, K., & Lindelow, M. (2015). Evasion of \u{201c}mandatory\u{201d} \
                 social health insurance for the formal sector: Evidence from Lao PDR. BMC Health \
                 Services Research, 15, 473. https://doi.org/10.1186/s12913-015-1132-5",
            ),
        ]);
        assert!(bad.is_empty(), "{bad:?}");
        assert_eq!(ok.len(), 1);
        let e = &ok[0];
        assert_eq!(e.surname, "alkenbrack", "a MATCH KEY, so lowercased");
        assert_eq!(e.year, Some(2015));
        assert_eq!(e.doi.as_deref(), Some("10.1186/s12913-015-1132-5"));
        assert_eq!(
            e.title.as_deref(),
            Some(
                "Evasion of \u{201c}mandatory\u{201d} social health insurance for the formal \
                 sector: Evidence from Lao PDR"
            ),
            "the title must stop at the venue, not run into it"
        );
    }

    /// An entry with no DOI still yields a title — that is what makes it a
    /// candidate for piece 2's title lookup rather than a dead end.
    #[test]
    fn an_entry_without_a_doi_still_yields_a_title() {
        let (ok, _) = parse_author_year_entries(&[
            blk_ay("References"),
            blk_ay(
                "Cashin, C., Bloom, D., Sparkes, S., & Barroy, H. (2017). Aligning public \
                 financial management and health financing. World Health Organization.",
            ),
        ]);
        assert_eq!(ok.len(), 1);
        assert_eq!(ok[0].doi, None);
        assert_eq!(
            ok[0].title.as_deref(),
            Some("Aligning public financial management and health financing")
        );
    }

    /// The organisational form. Requiring a comma-separated personal name
    /// reported correct APA as unreadable on a real paper.
    #[test]
    fn an_organisational_entry_is_readable() {
        let (ok, bad) = parse_author_year_entries(&[
            blk_ay("References"),
            blk_ay("P4H Network. (2024). Health financing progress matrix for Oman. P4H."),
        ]);
        assert!(bad.is_empty(), "{bad:?}");
        assert_eq!(ok.len(), 1);
        assert_eq!(ok[0].surname, "p4h");
        assert_eq!(ok[0].year, Some(2024));
    }

    /// An entry nobody can parse is REPORTED, not dropped — a marker cannot
    /// resolve against it, and silence would make that look like a clean list.
    #[test]
    fn an_unreadable_entry_is_reported_rather_than_dropped() {
        let (ok, bad) = parse_author_year_entries(&[
            blk_ay("References"),
            blk_ay("pp. 436-465, 2013, continued from the entry above somehow"),
        ]);
        assert!(ok.is_empty());
        assert_eq!(bad.len(), 1, "the unreadable entry vanished");
    }

    /// **AN UNBRACKETED YEAR IS A YEAR.** Two reference styles in the corpus
    /// print the year bare after the author list, `Authors. YYYY. Title`: IJAS
    /// (`Surname I and Surname I. 2011.`) and the final-L thesis (`Surname,
    /// Given and Surname, Given. 2007.`). Entries verbatim from both. Requiring
    /// `(YYYY)` read all 26 of IJAS's entries as unparseable, and every marker
    /// citing them as a work the list does not contain.
    #[test]
    fn an_unbracketed_year_entry_is_readable() {
        let (ok, bad) = parse_author_year_entries(&[
            blk_ay("References"),
            blk_ay("Göncü E and Parlak O. 2011. The influence of juvenile hormone analogue, fenoxycarb on the midgut remodeling in Bombyx mori (L., 1758) (Lepidoptera: Bombycidae) during larval– pupal metamorphosis. Turkish Journal of Entomology 35(2): 179–94."),
            blk_ay("Kamimura M and Kiuchi M. 1998. Effects of a juvenile hormone analogue, fenoxycarb, on 5th stadium larvae of the silkworm, Bombyx mori (Lepidoptera: Bombycidae). Applied Entomology and Zoology 33(2): 333–38. https://doi.org/10.1303/aez.33.333"),
            blk_ay("Drost, Wiebke, Matzke, Marianne and Backhaus, Thomas. 2007. `Heavy metal toxicity to Lemna minor: studies on the time dependence of growth inhibition and the recovery after exposure`, Chemosphere. 67:1: 36-43. DOI 10.1016/j.chemosphere.2006.10.018."),
        ]);
        assert!(bad.is_empty(), "{bad:?}");
        let got: Vec<(&str, Option<i32>)> = ok.iter().map(|e| (e.surname.as_str(), e.year)).collect();
        assert_eq!(got, vec![("göncü", Some(2011)), ("kamimura", Some(1998)), ("drost", Some(2007))]);
        assert_eq!(ok[1].doi.as_deref(), Some("10.1303/aez.33.333"));
        assert_eq!(ok[2].doi.as_deref(), Some("10.1016/j.chemosphere.2006.10.018"));
    }

    /// NEGATIVE CONTROL: a surname is not enough. An entry with no year in any
    /// form is still reported.
    #[test]
    fn an_entry_with_a_surname_and_no_year_is_still_unreadable() {
        let (ok, bad) = parse_author_year_entries(&[
            blk_ay("References"),
            blk_ay("Smith J and Jones K. A title that carries no year anywhere. Journal of Things 12: 1–5."),
        ]);
        assert!(ok.is_empty(), "{ok:?}");
        assert_eq!(bad.len(), 1);
    }

    /// NEGATIVE CONTROL: a four-digit number that is not a year. A sample size
    /// and a page range both sit in the year range (1600–2099); neither is
    /// `(YYYY)` or `. YYYY. `, so neither makes the entry parse.
    #[test]
    fn a_sample_size_or_page_range_is_not_read_as_a_year() {
        let (ok, bad) = parse_author_year_entries(&[
            blk_ay("References"),
            blk_ay("Smith J. A survey of 2000 households in two districts. Small Business Economics 58(4): 1885–1914."),
        ]);
        assert!(ok.is_empty(), "{ok:?}");
        assert_eq!(bad.len(), 1);
    }

    /// ONE LIST, ONE STYLE. Parsing both and keeping the larger would invent a
    /// second source of truth about the same text (§11 D132).
    #[test]
    fn the_author_year_list_is_parsed_only_when_there_is_no_numbered_one() {
        let numbered = prepass_blocks(&[
            blk_ay("Some prose that cites something [1] and says a thing about it."),
            blk_ay("References"),
            blk_ay("[1] A. Author, A real reference, 2001. https://doi.org/10.1/x"),
        ]);
        assert!(!numbered.bibliography.is_empty(), "numbered list not parsed");
        assert!(
            numbered.author_year_bibliography.is_empty(),
            "both lists were populated for one manuscript"
        );
    }

    /// §11 D133. THE WHOLE POINT: A FETCHED STAGED SOURCE MAKES A SENTENCE
    /// CHECKABLE.
    ///
    /// Before this, staging and fetching could both succeed and resolution still
    /// returned `Unverifiable`, because it only ever consulted
    /// `citation_library`. The staged source's PDF was on disk, indexed and
    /// embedded, and the sentence citing it was reported as not checkable —
    /// so the entire staging path produced ZERO checkable sentences.
    ///
    /// DOI-KEYED, not job-keyed. The marker carries a surname and a year; the
    /// manuscript's own reference entry carries the DOI; the staged row carries
    /// the document. Neither the marker nor the entry alone can find the file,
    /// and keying on the job would fail at preview time (no job exists), inside
    /// the planner (resolution runs before the job is created), and on a re-run
    /// (fresh job, so every PDF re-downloads).
    #[test]
    fn a_fetched_staged_source_makes_its_sentence_checkable() {
        let db = crate::Database::in_memory().unwrap();
        let doc = {
            let conn = db.conn().unwrap();
            conn.execute(
                "INSERT INTO ai_jobs (kind, status, total_items, prompt_version, created_at)
                 VALUES ('thesis_audit', 'queued', 0, 'v1', 1)",
                [],
            )
            .unwrap();
            let job = conn.last_insert_rowid();
            conn.execute(
                "INSERT INTO documents (source_type, title, source_url, fetched_at, checksum, status, created_at)
                 VALUES ('paper', 'Evasion of mandatory SHI', 'u', 1, 'ck', 'ingested', 1)",
                [],
            )
            .unwrap();
            let doc = conn.last_insert_rowid();
            conn.execute(
                "INSERT INTO ai_model_registry (id, kind, display_name, file_path, dim, registered_at)
                 VALUES ('m', 'embedding', 'm', '/dev/null', 384, 1)",
                [],
            )
            .unwrap();
            conn.execute(
                "INSERT INTO ai_chunks (document_id, page, section, char_start, char_end, content, token_estimate, content_hash, created_at)
                 VALUES (?1, 1, NULL, 0, 5, 'text', 1, 'h1', 1)",
                rusqlite::params![doc],
            )
            .unwrap();
            let chunk = conn.last_insert_rowid();
            conn.execute(
                "INSERT INTO ai_chunk_embeddings (chunk_id, model_id, preprocessing_version, dim, vector, created_at)
                 VALUES (?1, 'm', 'p', 384, X'00', 1)",
                rusqlite::params![chunk],
            )
            .unwrap();
            drop(conn);

            let entries = [AuthorYearEntry {
                surname: "alkenbrack".into(),
                year: Some(2015),
                doi: Some("https://doi.org/10.1186/s12913-015-1132-5".into()),
                title: Some("Evasion of mandatory social health insurance".into()),
                raw: "Alkenbrack, S. (2015). Evasion of mandatory social health insurance. BMC.".into(),
            }];
            crate::staged_sources::stage_entries(&db, job, &entries, 1).unwrap();
            let staged = crate::staged_sources::list_for_job(&db, job).unwrap();
            crate::staged_sources::link_document(&db, staged[0].id, doc, "doi").unwrap();
            doc
        };

        let numbered = BTreeMap::new();
        let entries = [AuthorYearEntry {
            surname: "alkenbrack".into(),
            year: Some(2015),
            // The reference entry prints the DOI in the prefixed form; the
            // staged row normalised it. One normaliser, both sides.
            doi: Some("https://doi.org/10.1186/s12913-015-1132-5".into()),
            title: None,
            raw: String::new(),
        }];
        let bib = Bibliography { numbered: &numbered, author_year: &entries };

        let m = &markers_in("Coverage rose sharply (Alkenbrack et al., 2015).")[0];
        match resolve_marker_with(&db, m, &bib).unwrap() {
            Resolution::Checkable { via, document_id, abstract_only } => {
                assert_eq!(document_id, doc);
                assert!(!abstract_only);
                assert!(
                    matches!(via, crate::source_ref::SourceRef::Staged { .. }),
                    "resolved via the library rather than the staged source: {via:?}"
                );
                assert_eq!(via.citation_id(), None, "a staged source has no library id");
            }
            other => panic!("a fetched staged source did not make the sentence checkable: {other:?}"),
        }

        // WITHOUT the author-year side of the bibliography there is no DOI to
        // key on, so the same marker cannot reach the same document. That is why
        // `Bibliography` carries both lists.
        let blind = Bibliography::numbered(&numbered);
        assert!(
            matches!(resolve_marker_with(&db, m, &blind).unwrap(), Resolution::Unverifiable { .. }),
            "resolution found the document without the reference entry that names its DOI"
        );
    }

    /// §11 D130. INTERVAL NOTATION IS NOT A CITATION.
    ///
    /// `standardised to [0, 1] by (score − 1)/4` was counted as a cited
    /// sentence: it left the queue unexamined and the report told the researcher
    /// it was "cited, but not checkable — numeric citation style". Statistical
    /// papers carry `[0, 1]`, `[0, 100]`, `[1, 5]` constantly.
    #[test]
    fn a_bracketed_interval_is_not_a_numeric_marker() {
        for s in [
            "Each item scored 1–5, standardised to [0, 1] by (score − 1)/4.",
            "Scores were rescaled to [0, 100] for comparability.",
            // U+202F NARROW NO-BREAK SPACE — the real document used one, and
            // `\s` matches it, which is how this reached the queue.
            "Standardised to [0,\u{202f}1] before averaging.",
        ] {
            let all = markers_in(s);
            let numeric: Vec<&Marker> =
                all.iter().filter(|m| m.style == MarkerStyle::Numeric).collect();
            assert!(numeric.is_empty(), "interval parsed as a citation in {s:?}: {numeric:?}");
        }
        // And a real numeric citation still parses, including a multi-citation —
        // the rule keys on a ZERO, not on a comma, for exactly this reason.
        for s in ["As shown previously [7].", "Both lines agree [5, 7]."] {
            assert!(
                markers_in(s).iter().any(|m| m.style == MarkerStyle::Numeric),
                "a real numeric citation stopped parsing: {s:?}"
            );
        }
    }

    /// §11 D131. ONE WORK, ONE ROW — whatever the in-text spelling.
    ///
    /// The reason string is the grouping key downstream, and it used to embed
    /// the raw marker, so three spellings of one paper became three "not in your
    /// library" rows and the source count overstated the work by roughly a third.
    #[test]
    fn author_year_spellings_of_one_work_produce_one_reason() {
        let db = crate::Database::in_memory().unwrap();
        let bib = BTreeMap::new();
        let reasons: std::collections::BTreeSet<String> = [
            "Coverage rose (AlJohani & Bugis, 2024).",
            "Coverage rose (AlJohani and Bugis, 2024).",
            "AlJohani and Bugis (2024) report a rise.",
        ]
        .iter()
        .map(|s| {
            let m = &markers_in(s)[0];
            match resolve_marker_with(&db, m, &Bibliography::numbered(&bib)).unwrap() {
                Resolution::Unverifiable { reason, .. } => reason,
                other => panic!("expected Unverifiable, got {other:?}"),
            }
        })
        .collect();
        assert_eq!(
            reasons.len(),
            1,
            "three spellings of one work produced {} distinct rows: {reasons:#?}",
            reasons.len()
        );
        let only = reasons.iter().next().unwrap();
        // It names the surname AS WRITTEN, not the lowercased match key.
        assert!(only.contains("AlJohani, 2024"), "{only}");
        // And it does NOT assert a work: the extraction is approximate.
        assert!(only.contains("not the full author list"), "{only}");
    }

    /// §11 D129. A SHIFTED REFERENCE LIST MUST NOT PRODUCE A CONFIDENT ANSWER.
    ///
    /// `R PAPER .docx` has one auto-numbered entry that is really the wrapped
    /// tail of the entry above, so Word renders 22 entries where there are 21
    /// and every ordinal from [6] up points one too high. `consistency` says so
    /// in a STRUCTURAL finding. Resolution used to ignore that and name the
    /// entry sitting at position `n` anyway, so one report said "every marker
    /// above [6] resolves to the wrong paper" and then, lower down, told the
    /// researcher to fetch "[9] Whitley, A genetic algorithm tutorial" for a
    /// sentence about Yang's firefly algorithm. A dozen such items.
    #[test]
    fn a_marker_above_a_malformed_entry_names_no_work() {
        let db = crate::Database::in_memory().unwrap();
        // [3] is the wrapped tail — no author — so [3] and up are unreliable.
        let bib = parse_numbered_bibliography(
            "[1] A. Author, A real reference, 2001.\n\
             [2] B. Bee, Another real reference, 2002.\n\
             [3] pp. 436–465, 2013.\n\
             [4] D. Dee, A fourth reference, 2004.",
        );
        assert_eq!(first_unreliable_entry(&bib), Some(3));

        // BELOW the break the numbering still holds, so the entry is named.
        let below = &markers_in("An early claim [2].")[0];
        match resolve_marker_with(&db, below, &Bibliography::numbered(&bib)).unwrap() {
            Resolution::Unverifiable { reason, .. } => assert!(
                reason.contains("Another real reference"),
                "an entry below the break must still be named: {reason}"
            ),
            other => panic!("expected Unverifiable, got {other:?}"),
        }

        // AT or ABOVE it, no work may be named and the reason must say why.
        for sentence in ["The malformed one [3].", "A later claim [4]."] {
            let m = &markers_in(sentence)[0];
            match resolve_marker_with(&db, m, &Bibliography::numbered(&bib)).unwrap() {
                Resolution::Unverifiable { reason, .. } => {
                    assert!(reason.contains("cannot be resolved"), "{reason}");
                    assert!(reason.contains("[3]"), "the break must be named: {reason}");
                    assert!(
                        !reason.contains("A fourth reference") && !reason.contains("not in your library"),
                        "a work was named for an unreliable marker: {reason}"
                    );
                }
                other => panic!("expected Unverifiable, got {other:?}"),
            }
        }
    }

    /// THE WORSE HALF OF §11 D129, and the reason the guard runs BEFORE the
    /// library lookup rather than only changing the wording.
    ///
    /// If the wrongly-pointed-at entry happens to be in the library, indexed and
    /// embedded, the old path returned `Checkable` — and the audit would quote
    /// an unrelated paper's passages as evidence for the claim. A wrong fetch
    /// instruction is visible to the reader; a wrong verification is not.
    #[test]
    fn an_unreliable_marker_is_never_checkable_even_when_the_entry_is_in_the_library() {
        let db = crate::Database::in_memory().unwrap();
        {
            let conn = db.conn().unwrap();
            conn.execute(
                "INSERT INTO citation_library (id, csl_json, doi, title, authors, year, tags, sync_status, created_at, updated_at)
                 VALUES ('lib-1', '{}', '10.1234/abc', 'A genetic algorithm tutorial', 'Whitley, D', 1994, '[]', 'local_only', 1, 1)",
                [],
            ).unwrap();
            conn.execute(
                "INSERT INTO documents (source_type, title, source_url, fetched_at, checksum, status, created_at)
                 VALUES ('paper', 'Whitley', '/tmp/a.pdf', 1, 'ck-1', 'ingested', 1)",
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
        let bib = parse_numbered_bibliography(
            "[1] A. Author, A real reference, 2001.\n\
             [2] pp. 436–465, 2013.\n\
             [3] D. Whitley, A genetic algorithm tutorial, 1994, doi:10.1234/abc",
        );
        assert_eq!(first_unreliable_entry(&bib), Some(2));
        let m = &markers_in("Yang formalized the firefly algorithm [3].")[0];
        let r = resolve_marker_with(&db, m, &Bibliography::numbered(&bib)).unwrap();
        assert!(
            matches!(r, Resolution::Unverifiable { .. }),
            "an unreliable marker resolved to a CHECKABLE source — the audit would have \
             quoted the wrong paper as evidence: {r:?}"
        );
    }

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
        match resolve_marker_with(&db, m, &Bibliography::numbered(&bib)).unwrap() {
            Resolution::Checkable { via, document_id, abstract_only } => {
                // §11 D133: a library citation says so, rather than the id being
                // assumed present on every checkable resolution.
                assert_eq!(via.citation_id(), Some("lib-1"));
                assert_eq!(document_id, 1);
                assert!(!abstract_only, "a full-text document must not read as an abstract");
            }
            other => panic!("expected Checkable, got {other:?}"),
        }

        // … and by lead author + year when it does not.
        let bib2 = parse_numbered_bibliography(
            "[1] M. De Choudhury, S. Counts, Mining large-scale social media, ICWSM, 2013, pp. 1-10.",
        );
        assert!(matches!(
            resolve_marker_with(&db, m, &Bibliography::numbered(&bib2)).unwrap(),
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
        match resolve_marker_with(&db, m, &Bibliography::numbered(&bib)).unwrap() {
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

    /// §11 D106. Designed against R PAPER's real sentences, not invented ones:
    /// of the five planned sentences ending in a figure/table label, exactly one
    /// is a caption. A rule that drops the other four would cost real claims.
    #[test]
    fn a_caption_whose_label_trails_is_still_a_caption() {
        // THE one this was built for — the label migrated to the end, and the
        // sentence was being offered as a claim about SemEval-2018.
        assert_eq!(
            skip_reason("Accuracy and F1-Score Comparison on SemEval-2018 Fig. 3."),
            Some(SkipReason::TableOrFigure)
        );
        assert_eq!(
            skip_reason("Ablation Study Accuracy and MCC per Component Table 4"),
            Some(SkipReason::TableOrFigure)
        );

        // And the four it must NOT touch — prose reaches a figure through a
        // preposition. Each of these is a real R PAPER sentence.
        for keep in [
            "The complete-flow architecture is depicted in Fig. 1.",
            "Accuracy and F1-score comparisons across all methods are depicted in Fig. 2.",
            "Per-emotion precision, recall, and F1-score for SemEval-2018 are presented in Table III.",
            "The attention example for a representative sad-labelled tweet is shown in Fig. 8.",
        ] {
            assert_eq!(skip_reason(keep), None, "wrongly dropped a cross-reference: {keep:?}");
        }

        // An unfamiliar connective must err towards KEEPING the claim.
        assert_eq!(
            skip_reason("The distribution of emotions is summarised throughout Table 9."),
            None
        );
    }

    #[test]
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
            // AS WRITTEN. Copying the lowercased key here is the exact defect
            // the field exists to prevent, and doing it made this test fail.
            lead_display: Some(lead.to_string()),
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

        // (a) nothing in the library at all. §11 D131: the reason names the
        // surname and year the marker actually gave, and says the extraction is
        // approximate — it no longer asserts a specific work.
        let r = resolve_marker(&db, &marker("Nobody", 1999)).unwrap();
        assert!(
            matches!(&r, Resolution::Unverifiable { reason, .. }
                if reason.contains("no library work matches")
                    && reason.contains("Nobody, 1999")
                    && reason.contains("not the full author list")),
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
            lead_display: None,
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
