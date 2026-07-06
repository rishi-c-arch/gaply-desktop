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
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Citation {
    pub style: CitationStyle,
    pub authors: String,
    pub year: i32,
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
                year,
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
                    year,
                    raw: format!("({})", part.trim()),
                    location: loc.clone(),
                });
            }
        }
    }

    out
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
        assert!(cites.iter().any(|c| c.authors == "Smith et al." && c.year == 2023));
        assert!(cites.iter().any(|c| c.authors == "Jones" && c.year == 2019));
    }

    #[test]
    fn parenthetical_multiple_in_one_group() {
        let cites = extract_in_text("prior work (Jones, 2019; Brown & Lee, 2020).", &loc());
        assert_eq!(cites.len(), 2);
        assert!(cites.iter().all(|c| c.style == CitationStyle::Parenthetical));
        assert!(cites.iter().any(|c| c.authors == "Jones" && c.year == 2019));
        assert!(cites.iter().any(|c| c.authors == "Brown & Lee" && c.year == 2020));
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
