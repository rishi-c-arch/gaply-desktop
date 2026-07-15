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
        for part in inner.split(';') {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::extract::SectionKind;

    fn loc() -> Location {
        Location { section: SectionKind::Results, paragraph: 0 }
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
