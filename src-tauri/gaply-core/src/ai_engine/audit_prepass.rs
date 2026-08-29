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
    pub markers_found: usize,
    pub planned: Vec<PlannedSentence>,
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

/// Is this sentence worth a model call at all?
pub fn is_significant(sentence: &str) -> bool {
    let words = sentence.split_whitespace().count();
    words >= MIN_CLAIM_WORDS && !looks_like_heading(sentence)
}

/// Run the whole deterministic pass over paged blocks.
///
/// Takes `(page, text)` pairs rather than a path so it stays pure and testable
/// — the caller does the parsing with `extract::docparse::parse_path_paged`.
pub fn prepass(blocks: &[(Option<u32>, String)]) -> PrepassReport {
    let mut report = PrepassReport::default();
    let mut in_references = false;

    for (page, text) in blocks {
        // Already past the bibliography: nothing after it is prose.
        if in_references {
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
                break;
            }
            offset += line.len();
        }
        let prose = &text[..prose_end];

        for sentence in crate::extract::sentence::sentences_in(prose) {
            report.total_sentences += 1;
            let markers = markers_in(sentence);
            report.markers_found += markers.len();
            if markers.is_empty() {
                report.uncited += 1;
            } else {
                report.cited += 1;
            }
            if !is_significant(sentence) {
                report.skipped += 1;
                continue;
            }
            report.planned.push(PlannedSentence {
                page: *page,
                sentence: sentence.to_string(),
                markers,
            });
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
    Unverifiable { reason: String },
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
