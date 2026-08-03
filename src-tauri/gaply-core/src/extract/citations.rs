//! In-text citation extraction (parenthetical + narrative) and reference-list
//! parsing.

use serde::{Deserialize, Serialize};

use crate::extract::sections::Section;
use crate::extract::stats::regexes;
use crate::extract::Location;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CitationStyle {
    /// "(Smith, 2023)"
    Parenthetical,
    /// "Smith et al. (2023) showed…"
    Narrative,
    /// "[1]", "[1,2]", "[3–5]" — bracket-numeric (Vancouver/IEEE families). Has
    /// no author or year; the referenced numbers live in `Citation::numbers`.
    Numeric,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Citation {
    pub style: CitationStyle,
    /// Author string — empty for `Numeric` (bracket-numeric cites carry no author).
    pub authors: String,
    /// Publication year — `None` for `Numeric` (NO fake year; numeric markers
    /// carry no year). `Some` for the author-year styles.
    pub year: Option<i32>,
    /// The referenced reference-list numbers, for `Numeric` only (e.g. `[1,2]` ->
    /// `[1, 2]`, `[3–5]` -> `[3, 4, 5]`). Empty for the author-year styles.
    #[serde(default)]
    pub numbers: Vec<u32>,
    pub raw: String,
    pub location: Location,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Reference {
    pub raw: String,
    pub authors: String,
    pub year: Option<i32>,
    pub title: Option<String>,
    pub doi: Option<String>,
}

fn parse_year(s: &str) -> Option<i32> {
    s.trim_end_matches(|c: char| c.is_ascii_alphabetic()).parse().ok()
}

fn clean_authors(s: &str) -> String {
    s.trim().trim_end_matches(&[',', ';', ' '][..]).trim().to_string()
}

/// Extract both citation styles from one paragraph. Narrative citations are
/// matched first and their spans excluded so a narrative "Author (2023)" is
/// never double-counted by the parenthetical scanner.
pub fn extract_in_text(paragraph: &str, loc: &Location) -> Vec<Citation> {
    let re = regexes();
    let mut out = Vec::new();
    let mut narrative_spans: Vec<(usize, usize)> = Vec::new();

    for c in re.narrative_cite.captures_iter(paragraph) {
        let whole = c.get(0).unwrap();
        narrative_spans.push((whole.start(), whole.end()));
        if let Some(year) = parse_year(&c[2]) {
            out.push(Citation {
                style: CitationStyle::Narrative,
                authors: clean_authors(&c[1]),
                year: Some(year),
                numbers: Vec::new(),
                raw: whole.as_str().trim().to_string(),
                location: loc.clone(),
            });
        }
    }

    for group in re.paren_group.captures_iter(paragraph) {
        let whole = group.get(0).unwrap();
        // skip parens already consumed by a narrative "(year)" match
        if narrative_spans.iter().any(|(s, e)| whole.start() >= *s && whole.end() <= *e) {
            continue;
        }
        let inner = &group[1];
        for part in split_paren_citations(inner) {
            let part = part.as_str();
            if let Some((authors, year)) = parse_paren_part(part) {
                out.push(Citation {
                    style: CitationStyle::Parenthetical,
                    authors,
                    year: Some(year),
                    numbers: Vec::new(),
                    raw: format!("({})", part.trim()),
                    location: loc.clone(),
                });
            }
        }
    }

    // Bracket-numeric (Vancouver/IEEE): each `[...]` is ONE marker whose parsed
    // reference numbers live in `numbers` (so density counts them per token —
    // `[1,2]` = 2). `parse_numeric_group` rejects non-citation brackets
    // (`[Fig. 2]`, `[95% CI]`, `[data not shown]`, out-of-cap ranges).
    for group in re.bracket_numeric.captures_iter(paragraph) {
        let whole = group.get(0).unwrap();
        if let Some(numbers) = parse_numeric_group(&group[1]) {
            out.push(Citation {
                style: CitationStyle::Numeric,
                authors: String::new(),
                year: None,
                numbers,
                raw: whole.as_str().to_string(),
                location: loc.clone(),
            });
        }
    }

    out
}

/// Split the inside of a `(...)` group into individual citations.
///
/// `;` always separates. A COMMA separates only once the group so far already
/// carries a year — otherwise the comma is the one inside `"(Smith, 2020)"`,
/// which must not be split. Greedy accumulation handles both:
///   `"Smith, 2020"`                      -> one citation
///   `"A et al. 1993, B and C 1998"`      -> two citations
fn split_paren_citations(inner: &str) -> Vec<String> {
    let year = &regexes().year_bare;
    let mut out = Vec::new();
    for chunk in inner.split(';') {
        let mut cur = String::new();
        for tok in chunk.split(',') {
            if !cur.is_empty() {
                cur.push(',');
            }
            cur.push_str(tok);
            if year.is_match(&cur) {
                out.push(std::mem::take(&mut cur));
            }
        }
        if !cur.trim().is_empty() {
            out.push(cur);
        }
    }
    out
}

/// Ranges wider than this are treated as malformed (not a citation) — a real
/// bracket-numeric range spans a handful of references, not hundreds.
const MAX_EXPANDABLE_RANGE_WIDTH: u32 = 20;

/// Parse the inner text of a `[...]` group into its referenced numbers, or
/// `None` when it is NOT a bracket-numeric citation. Accepts ONLY digits,
/// commas, whitespace, and dashes (hyphen/en/em); comma-separated tokens; each
/// token a single number or an ascending range `a-b` expanded inclusively (up to
/// [`MAX_EXPANDABLE_RANGE_WIDTH`]). Any other content -> `None`.
pub fn parse_numeric_group(inner: &str) -> Option<Vec<u32>> {
    let dashes: &[char] = &['-', '\u{2013}', '\u{2014}'];
    if !inner
        .chars()
        .all(|c| c.is_ascii_digit() || c == ',' || dashes.contains(&c) || c.is_whitespace())
    {
        return None;
    }
    let mut nums: Vec<u32> = Vec::new();
    for tok in inner.split(',') {
        let tok = tok.trim();
        if tok.is_empty() {
            return None; // stray/trailing comma -> malformed
        }
        match tok.find(dashes) {
            Some(pos) => {
                let start: u32 = tok[..pos].trim().parse().ok()?;
                // skip the dash char (1 byte for '-', 3 for the en/em dashes)
                let after = pos + tok[pos..].chars().next().unwrap().len_utf8();
                let end: u32 = tok[after..].trim().parse().ok()?;
                if end < start || end - start > MAX_EXPANDABLE_RANGE_WIDTH {
                    return None; // descending or over-cap -> malformed
                }
                nums.extend(start..=end);
            }
            None => nums.push(tok.parse().ok()?),
        }
    }
    (!nums.is_empty()).then_some(nums)
}

/// Parse "Smith et al., 2023" / "Brown & Lee, 2020" into (authors, year).
/// Requires both an author token (letters) and a 19xx/20xx year.
fn parse_paren_part(part: &str) -> Option<(String, i32)> {
    let re = regexes();
    let ymatch = re.year_bare.find(part)?;
    let year: i32 = part[ymatch.start()..ymatch.end()].parse().ok()?;
    let authors = clean_authors(&part[..ymatch.start()]);
    if authors.chars().any(|c| c.is_alphabetic()) {
        Some((authors, year))
    } else {
        None
    }
}

/// Parse each paragraph of the References section into a structured entry.
pub fn parse_reference_list(references: &Section) -> Vec<Reference> {
    references.paragraphs.iter().map(|p| parse_reference(p)).collect()
}

fn parse_reference(entry: &str) -> Reference {
    let re = regexes();
    let entry = entry.trim();

    let doi = re
        .doi
        .captures(entry)
        .map(|c| c[1].trim_end_matches(&['.', ',', ';', ')'][..]).to_string());

    // Prefer a parenthesized year "(2023)"; fall back to the first bare year.
    let (year, year_end) = if let Some(m) = re.year_paren.find(entry) {
        let y = entry[m.start() + 1..m.end() - 1]
            .trim_end_matches(|c: char| c.is_ascii_alphabetic())
            .parse()
            .ok();
        (y, Some(m.end()))
    } else if let Some(m) = re.year_bare.find(entry) {
        (entry[m.start()..m.end()].parse().ok(), Some(m.end()))
    } else {
        (None, None)
    };

    let authors = year_end
        .map(|e| &entry[..e])
        .unwrap_or(entry)
        .split(|c| c == '(')
        .next()
        .map(clean_authors)
        .unwrap_or_default();

    // Title: text after the year up to the next sentence break.
    let title = year_end.and_then(|start| {
        let rest = entry[start..].trim_start_matches(&['.', ' ', ')'][..]);
        let end = rest.find(". ").unwrap_or(rest.len());
        let t = rest[..end].trim().trim_end_matches('.').trim();
        (!t.is_empty()).then(|| t.to_string())
    });

    Reference { raw: entry.to_string(), authors, year, title, doi }
}

// ---------------------------------------------------------------------------
// Uncited-reference matching
// ---------------------------------------------------------------------------
//
// Answers, for each bibliography entry: was it cited in the body?
//
// Governed by ONTOLOGY §4.6. The evidence is deterministic, but the MAPPING is
// not: a bibliography entry carries no number field, so joining the two sides
// means matching author + year, and that match can be ambiguous. Every ambiguous
// case is therefore refused rather than resolved.
//
// The asymmetry is the whole design. An author never told a reference is uncited
// loses a small check. An author WRONGLY told one is uncited is asked to delete a
// reference they did cite — the §4.4 class, on their own bibliography, and worse
// than the usual case because it invites a destructive edit. So `Uncited` is
// emitted only when the entry was successfully evaluated; everything else is
// `NotEvaluated` and is counted, not hidden.
//
// Measured need, on a real manuscript after `docparse::reflow_pdf_text`: 20
// parsed entries for ~16 real references. Four rows are parse fragments — two
// carry no year ("Central Silk Board, Bangalore."), two have a DOI URL where the
// surname should be. A naive matcher reports all four as uncited. Under the rules
// below all four land in `NotEvaluated`.

/// Share of citations that must share one style before the style is considered
/// determined. A heuristic — and acceptable under §4.6 question 3 only because
/// its failure mode is REFUSAL, not a wrong finding: an undetermined manuscript
/// yields "not evaluated", never "uncited". If the threshold is wrong the product
/// goes quiet, which is the safe direction.
pub const STYLE_DOMINANCE: f64 = 0.9;

/// Why one bibliography entry could not be evaluated.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotEvaluated {
    /// No publication year parsed — nothing to key on.
    Undated,
    /// The parsed author field does not look like a surname (a DOI URL, a
    /// journal name, a page range) — the entry is almost certainly a parse
    /// fragment rather than a real reference.
    UnparseableAuthor,
    /// Two or more entries share the same (surname, year) key, so a citation
    /// matching that key cannot be attributed to one of them.
    AmbiguousKey,
}

/// Verdict for one bibliography entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CitationUse {
    Cited,
    Uncited,
    NotEvaluated(NotEvaluated),
}

/// The in-text citation styles that carry an author and a year.
fn is_author_year(style: CitationStyle) -> bool {
    matches!(style, CitationStyle::Parenthetical | CitationStyle::Narrative)
}

/// Tokens that appear in an author list but are not surnames.
const AUTHOR_NOISE: &[&str] = &["et", "al", "and", "the", "of", "for"];

/// Markers that an author string absorbed a URL or DOI — positive evidence the
/// entry is a PARSE FRAGMENT (a split bibliography entry carrying the previous
/// entry's tail), not a reference whose identity we have established.
const URL_MARKERS: &[&str] = &["://", "doi.org", "www."];

/// A surname-shaped token: begins with a capital, at least two characters,
/// letters only apart from hyphens and apostrophes, and not author-list noise.
///
/// The capital is load-bearing. Without it, a DOI URL that leaked into the author
/// field yields `doi` and `org` as "surnames", which silently converts an entry
/// we cannot identify into one we think we can — the exact refusal guard this
/// module exists to hold.
fn plausible_surname(s: &str) -> bool {
    let t = s.trim().trim_end_matches(&[',', '.', ';'][..]);
    t.chars().count() >= 2
        && t.chars().next().map(|c| c.is_uppercase()).unwrap_or(false)
        && t.chars().all(|c| c.is_alphabetic() || c == '-' || c == '\'')
        && !AUTHOR_NOISE.contains(&t.to_lowercase().as_str())
}

/// EVERY surname in an author string, lowercased. Empty when the string carries a
/// URL/DOI marker — such an entry is refused wholesale rather than mined for
/// whichever token happens to look like a name.
///
/// Not just the first surname. The narrative-citation regex (`stats.rs:84`)
/// matches `and`/`&` without consuming the surname that follows, so `"Gordon and
/// Burford (1984)"` is extracted with authors `"Burford"` — the SECOND author.
/// Keying on the first surname alone would report `"Gordon R and Burford I R.
/// 1984."` as uncited, which is false. A citation naming ANY of an entry's
/// authors is evidence that entry was cited.
fn surnames(authors: &str) -> Vec<String> {
    if URL_MARKERS.iter().any(|m| authors.contains(m)) {
        return Vec::new();
    }
    authors
        .split(|c: char| !(c.is_alphabetic() || c == '-' || c == '\''))
        .filter(|t| plausible_surname(t))
        .map(|t| t.to_lowercase())
        .collect()
}

/// Every match key for a bibliography entry, or the reason it has none.
fn reference_keys(r: &Reference) -> Result<Vec<(String, i32)>, NotEvaluated> {
    let year = r.year.ok_or(NotEvaluated::Undated)?;
    let names = surnames(&r.authors);
    if names.is_empty() {
        return Err(NotEvaluated::UnparseableAuthor);
    }
    Ok(names.into_iter().map(|n| (n, year)).collect())
}

/// True when author-year matching applies to this manuscript at all: there are
/// citations to reason from, and at least [`STYLE_DOMINANCE`] of them carry an
/// author and a year. Mixed or numeric-dominant manuscripts are refused —
/// numeric-style needs reference NUMBERS parsed from the entries, which
/// `parse_reference` does not do (ARCHITECTURE_TRACE §11.4).
pub fn author_year_style_determined(citations: &[Citation]) -> bool {
    if citations.is_empty() {
        return false;
    }
    let ay = citations.iter().filter(|c| is_author_year(c.style)).count();
    ay as f64 / citations.len() as f64 >= STYLE_DOMINANCE
}

/// Per-entry verdicts, in `references` order.
///
/// Returns an empty vector when the style is not determined, so the caller emits
/// nothing rather than guessing.
pub fn classify_reference_use(references: &[Reference], citations: &[Citation]) -> Vec<CitationUse> {
    if !author_year_style_determined(citations) {
        return Vec::new();
    }

    let keys: Vec<Result<Vec<(String, i32)>, NotEvaluated>> =
        references.iter().map(reference_keys).collect();

    // A key shared by two entries cannot attribute a citation to either of them.
    let mut owners: std::collections::HashMap<(String, i32), usize> = std::collections::HashMap::new();
    for ks in keys.iter().flatten() {
        for k in ks {
            *owners.entry(k.clone()).or_insert(0) += 1;
        }
    }

    let cited: std::collections::HashSet<(String, i32)> = citations
        .iter()
        .filter(|c| is_author_year(c.style))
        .flat_map(|c| {
            let year = c.year;
            surnames(&c.authors)
                .into_iter()
                .filter_map(move |n| year.map(|y| (n, y)))
        })
        .collect();

    keys.iter()
        .map(|k| match k {
            Err(reason) => CitationUse::NotEvaluated(*reason),
            Ok(ks) if ks.iter().any(|k| owners.get(k).copied().unwrap_or(0) > 1) => {
                CitationUse::NotEvaluated(NotEvaluated::AmbiguousKey)
            }
            Ok(ks) if ks.iter().any(|k| cited.contains(k)) => CitationUse::Cited,
            Ok(_) => CitationUse::Uncited,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::extract::SectionKind;

    fn loc() -> Location {
        Location { section: SectionKind::Results, paragraph: 0 }
    }

    // -----------------------------------------------------------------
    // Uncited-reference matching (ONTOLOGY §4.6 — refuse, do not guess)
    // -----------------------------------------------------------------

    fn refr(authors: &str, year: Option<i32>) -> Reference {
        Reference { raw: format!("{authors} {year:?}"), authors: authors.into(), year, title: None, doi: None }
    }
    fn cite(authors: &str, year: i32) -> Citation {
        Citation {
            style: CitationStyle::Parenthetical,
            authors: authors.into(),
            year: Some(year),
            numbers: Vec::new(),
            raw: format!("({authors}, {year})"),
            location: loc(),
        }
    }
    fn numeric_cite(n: u32) -> Citation {
        Citation {
            style: CitationStyle::Numeric,
            authors: String::new(),
            year: None,
            numbers: vec![n],
            raw: format!("[{n}]"),
            location: loc(),
        }
    }

    #[test]
    fn a_reference_with_no_matching_citation_is_uncited() {
        let refs = vec![refr("Smith J", Some(2020)), refr("Jones A", Some(2019))];
        let cites = vec![cite("Smith", 2020)];
        assert_eq!(
            classify_reference_use(&refs, &cites),
            vec![CitationUse::Cited, CitationUse::Uncited]
        );
    }

    #[test]
    fn narrative_and_parenthetical_citations_both_count_as_use() {
        let refs = vec![refr("Smith J", Some(2020))];
        let mut c = cite("Smith", 2020);
        c.style = CitationStyle::Narrative;
        assert_eq!(classify_reference_use(&refs, &[c]), vec![CitationUse::Cited]);
    }

    #[test]
    fn an_undated_reference_is_never_reported_uncited() {
        // Real fragment from a parsed bibliography: "Central Silk Board, Bangalore."
        let refs = vec![refr("Central Silk Board, Bangalore.", None)];
        let cites = vec![cite("Smith", 2020)];
        assert_eq!(
            classify_reference_use(&refs, &cites),
            vec![CitationUse::NotEvaluated(NotEvaluated::Undated)]
        );
    }

    #[test]
    fn a_doi_url_where_the_surname_should_be_is_never_reported_uncited() {
        // Real fragment: a split entry leaves the previous entry's DOI in `authors`.
        let refs = vec![refr("https://doi.org/10.1007/s00018-023-04996-1 Liu X", Some(2023))];
        let cites = vec![cite("Smith", 2020)];
        assert_eq!(
            classify_reference_use(&refs, &cites),
            vec![CitationUse::NotEvaluated(NotEvaluated::UnparseableAuthor)]
        );
    }

    #[test]
    fn a_journal_fragment_without_a_year_is_not_evaluated() {
        // Real fragment: "Analytical Chemistry 31(3): 426–28."
        let refs = vec![refr("Analytical Chemistry 31(3): 426–28.", None)];
        assert_eq!(
            classify_reference_use(&refs, &[cite("Smith", 2020)]),
            vec![CitationUse::NotEvaluated(NotEvaluated::Undated)]
        );
    }

    #[test]
    fn an_ambiguous_key_produces_no_finding_for_either_entry() {
        // Two entries share (etebari, 2005): a citation cannot be attributed to
        // one of them, so NEITHER may be called uncited.
        let refs = vec![
            refr("Etebari K, Mirhoseini S Z", Some(2005)),
            refr("Etebari K, Bizhannia A R", Some(2005)),
        ];
        let cites = vec![cite("Smith", 2020)];
        assert_eq!(
            classify_reference_use(&refs, &cites),
            vec![
                CitationUse::NotEvaluated(NotEvaluated::AmbiguousKey),
                CitationUse::NotEvaluated(NotEvaluated::AmbiguousKey),
            ]
        );
    }

    #[test]
    fn same_surname_different_years_are_not_ambiguous() {
        let refs = vec![refr("Etebari K", Some(2005)), refr("Etebari K", Some(2007))];
        let cites = vec![cite("Etebari", 2007)];
        assert_eq!(
            classify_reference_use(&refs, &cites),
            vec![CitationUse::Uncited, CitationUse::Cited]
        );
    }

    #[test]
    fn surname_matching_is_case_insensitive() {
        // An all-caps bibliography entry matches a normally-cased citation.
        // (Both sides must still be surname-SHAPED — capital-initial — so a
        // lowercase URL component can never act as a name; see `plausible_surname`.)
        let refs = vec![refr("SMITH J", Some(2020))];
        assert_eq!(classify_reference_use(&refs, &[cite("Smith", 2020)]), vec![CitationUse::Cited]);
    }

    #[test]
    fn numeric_dominant_manuscripts_are_refused_entirely() {
        // Numeric style needs reference NUMBERS parsed from the entries, which
        // parse_reference does not do. Refuse rather than key on position.
        let refs = vec![refr("Smith J", Some(2020))];
        let cites: Vec<Citation> = (1..=10).map(numeric_cite).collect();
        assert!(!author_year_style_determined(&cites));
        assert!(classify_reference_use(&refs, &cites).is_empty());
    }

    #[test]
    fn a_mixed_style_manuscript_is_refused() {
        let refs = vec![refr("Smith J", Some(2020))];
        // 8 author-year + 2 numeric = 0.8, below STYLE_DOMINANCE.
        let mut cites: Vec<Citation> = (0..8).map(|i| cite("Smith", 2000 + i)).collect();
        cites.push(numeric_cite(1));
        cites.push(numeric_cite(2));
        assert!(!author_year_style_determined(&cites));
        assert!(classify_reference_use(&refs, &cites).is_empty());
    }

    #[test]
    fn a_dominant_author_year_manuscript_is_evaluated_despite_one_numeric_marker() {
        let refs = vec![refr("Smith J", Some(2020))];
        let mut cites: Vec<Citation> = (0..19).map(|i| cite("Smith", 2000 + i)).collect();
        cites.push(numeric_cite(1)); // 19/20 = 0.95
        assert!(author_year_style_determined(&cites));
        assert!(!classify_reference_use(&refs, &cites).is_empty());
    }

    #[test]
    fn no_citations_at_all_is_refused_not_treated_as_all_uncited() {
        let refs = vec![refr("Smith J", Some(2020)), refr("Jones A", Some(2019))];
        assert!(classify_reference_use(&refs, &[]).is_empty());
    }

    #[test]
    fn narrative_single_and_etal() {
        let cites = extract_in_text("Smith et al. (2023) and Jones (2019) both agree.", &loc());
        assert_eq!(cites.len(), 2);
        assert!(cites.iter().all(|c| c.style == CitationStyle::Narrative));
        assert!(cites.iter().any(|c| c.authors == "Smith et al." && c.year == Some(2023)));
        assert!(cites.iter().any(|c| c.authors == "Jones" && c.year == Some(2019)));
    }

    #[test]
    fn parenthetical_multiple_in_one_group() {
        let cites = extract_in_text("prior work (Jones, 2019; Brown & Lee, 2020).", &loc());
        assert_eq!(cites.len(), 2);
        assert!(cites.iter().all(|c| c.style == CitationStyle::Parenthetical));
        assert!(cites.iter().any(|c| c.authors == "Jones" && c.year == Some(2019)));
        assert!(cites.iter().any(|c| c.authors == "Brown & Lee" && c.year == Some(2020)));
    }

    #[test]
    fn bracket_numeric_positives_with_expected_counts() {
        // (input, expected reference-number count) — the decision's fixtures.
        let cases = [
            ("As shown [1].", 1usize),
            ("prior work [1,2].", 2),
            ("see [1\u{2013}3].", 3),      // en-dash range 1–3
            ("refs [1,3,5].", 3),
            ("multiple [1\u{2013}3,7].", 4), // 1,2,3 + 7
            ("earlier [12,15\u{2013}18].", 5), // 12 + 15,16,17,18
            ("hyphen range [1-3].", 3),      // ASCII hyphen too
        ];
        for (text, expect) in cases {
            let cites = extract_in_text(text, &loc());
            assert_eq!(cites.len(), 1, "{text:?}: exactly one numeric marker");
            assert_eq!(cites[0].style, CitationStyle::Numeric);
            assert!(cites[0].year.is_none(), "{text:?}: numeric carries NO year");
            assert!(cites[0].authors.is_empty(), "{text:?}: numeric carries no author");
            assert_eq!(cites[0].numbers.len(), expect, "{text:?}: reference-token count");
        }
    }

    #[test]
    fn bracket_numeric_negatives_are_not_citations() {
        // Statistical, figure/table, and malformed brackets must NOT parse.
        for text in [
            "the CI was wide [95% CI].",
            "significant [p < 0.05].",
            "see [Fig. 2].",
            "in [Table 1].",
            "detail [Supplementary Fig. S1].",
            "from [Eq. 3].",
            "results [data not shown].",
            "point [i] of the list.",
            "absurd range [1-999].", // over the 20-wide cap -> malformed
        ] {
            let cites = extract_in_text(text, &loc());
            assert!(cites.is_empty(), "{text:?} must NOT parse as a citation, got {cites:?}");
        }
    }

    #[test]
    fn parse_numeric_group_edges() {
        assert_eq!(parse_numeric_group("1,2"), Some(vec![1, 2]));
        assert_eq!(parse_numeric_group("12,15\u{2013}18"), Some(vec![12, 15, 16, 17, 18]));
        assert_eq!(parse_numeric_group("1-999"), None, "range width > 20 cap");
        assert_eq!(parse_numeric_group("5-3"), None, "descending range");
        assert_eq!(parse_numeric_group("1,"), None, "trailing comma");
        assert_eq!(parse_numeric_group("i"), None, "roman numeral");
        assert_eq!(parse_numeric_group("Fig. 2"), None, "has letters");
    }

    #[test]
    fn narrative_not_double_counted_as_parenthetical() {
        // "Smith (2020)" must yield exactly one narrative citation, not also a
        // parenthetical one for "(2020)".
        let cites = extract_in_text("As Smith (2020) reported, the effect held.", &loc());
        assert_eq!(cites.len(), 1);
        assert_eq!(cites[0].style, CitationStyle::Narrative);
    }

    #[test]
    fn ci_parentheses_are_not_citations() {
        let cites = extract_in_text("the estimate (95% CI: 1.2 to 3.4) was stable.", &loc());
        assert!(cites.is_empty());
    }

    #[test]
    fn reference_with_doi() {
        let section = Section {
            kind: SectionKind::References,
            heading: "References".into(),
            paragraphs: vec![
                "Smith, J., & Clark, T. (2023). Sleep and memory in adults. Journal of Sleep, 12(3), 45-67. https://doi.org/10.1234/jsleep.2023.045".into(),
            ],
        };
        let refs = parse_reference_list(&section);
        assert_eq!(refs.len(), 1);
        let r = &refs[0];
        assert!(r.authors.starts_with("Smith"));
        assert_eq!(r.year, Some(2023));
        assert_eq!(r.title.as_deref(), Some("Sleep and memory in adults"));
        assert_eq!(r.doi.as_deref(), Some("10.1234/jsleep.2023.045"));
    }

    #[test]
    fn reference_without_doi() {
        let section = Section {
            kind: SectionKind::References,
            heading: "References".into(),
            paragraphs: vec!["Jones, P. (2019). Memory under deprivation. Cognitive Science, 5(1), 10-20.".into()],
        };
        let r = &parse_reference_list(&section)[0];
        assert_eq!(r.doi, None);
        assert_eq!(r.year, Some(2019));
        assert_eq!(r.title.as_deref(), Some("Memory under deprivation"));
    }
}
