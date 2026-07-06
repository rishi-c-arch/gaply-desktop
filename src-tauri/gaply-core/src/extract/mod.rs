//! Extraction Agent: parse a manuscript into structured sections and pull
//! out statistical claims, in-text citations, and reference-list entries.
//!
//! Pure, offline Rust — the parsers (`pdf-extract`, `zip`+`quick-xml`) read
//! local bytes only; nothing here makes a network call. Everything operates
//! on plaintext, so the format adapters in [`docparse`] are the only
//! format-specific code.

pub mod citations;
pub mod docparse;
pub mod persist;
pub mod sections;
pub mod stats;

use serde::{Deserialize, Serialize};

pub use citations::{Citation, CitationStyle, Reference};
pub use sections::{Section, SectionKind};
pub use stats::{Stat, StatClaim};

/// A location within the parsed document: which section, and which paragraph
/// (0-based) inside that section.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Location {
    pub section: SectionKind,
    pub paragraph: usize,
}

/// A referenced table (e.g. "Table 1") and its caption, if the paragraph is
/// the caption itself.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TableRef {
    pub label: String,
    pub caption: Option<String>,
    pub location: Location,
}

/// The full typed result of extracting one manuscript.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExtractionResult {
    pub title: Option<String>,
    pub sections: Vec<Section>,
    pub statistics: Vec<StatClaim>,
    pub citations: Vec<Citation>,
    pub references: Vec<Reference>,
    pub tables: Vec<TableRef>,
}

/// Extract structure and claims from already-parsed manuscript plaintext.
#[tracing::instrument(skip(text), fields(len = text.len()))]
pub fn extract_from_text(text: &str) -> ExtractionResult {
    let (title, secs) = sections::split_document(text);

    let mut statistics = Vec::new();
    let mut citations = Vec::new();
    let mut tables = Vec::new();
    let mut references = Vec::new();

    for section in &secs {
        if section.kind == SectionKind::References {
            references = citations::parse_reference_list(section);
            // reference-list entries are parsed structurally; don't also scan
            // them as in-text citations or statistics.
            continue;
        }
        for (p_idx, paragraph) in section.paragraphs.iter().enumerate() {
            let loc = Location { section: section.kind, paragraph: p_idx };
            statistics.extend(stats::extract(paragraph, &loc));
            citations.extend(citations::extract_in_text(paragraph, &loc));
            if let Some(t) = detect_table(paragraph, &loc) {
                tables.push(t);
            }
        }
    }

    ExtractionResult { title, sections: secs, statistics, citations, references, tables }
}

/// A paragraph that begins "Table N ..." is treated as that table's caption.
fn detect_table(paragraph: &str, loc: &Location) -> Option<TableRef> {
    let re = &stats::regexes().table_caption;
    let caps = re.captures(paragraph.trim())?;
    let label = format!("Table {}", &caps[1]);
    let caption = caps.get(2).map(|m| m.as_str().trim()).filter(|s| !s.is_empty());
    Some(TableRef {
        label,
        caption: caption.map(String::from),
        location: loc.clone(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "\
Effect of Sleep on Memory Consolidation

Jane A. Smith, John Doe

Abstract
We tested whether sleep improves recall. In a randomized trial (n = 120),
sleep-deprived participants performed worse (p < 0.001).

Methods
Participants were assigned to two groups. We used an independent t-test to
compare recall scores. A one-way ANOVA assessed dose effects (n = 120).

Results
Table 1 summarizes the outcomes. Recall improved with sleep (95% CI: 1.2 to
3.4; p = 0.03). Smith et al. (2023) reported a similar effect. Earlier work
found no effect (Jones, 2019; Brown & Lee, 2020).

Discussion
Our findings align with prior reports (Smith et al., 2023).

References
Smith, J., Adams, R., & Clark, T. (2023). Sleep and memory in adults. Journal of Sleep, 12(3), 45-67. https://doi.org/10.1234/jsleep.2023.045
Jones, P. (2019). Memory under deprivation. Cognitive Science, 5(1), 10-20.
";

    #[test]
    fn detects_all_imrad_sections() {
        let r = extract_from_text(SAMPLE);
        let kinds: Vec<_> = r.sections.iter().map(|s| s.kind).collect();
        assert!(kinds.contains(&SectionKind::Abstract));
        assert!(kinds.contains(&SectionKind::Methods));
        assert!(kinds.contains(&SectionKind::Results));
        assert!(kinds.contains(&SectionKind::Discussion));
        assert!(kinds.contains(&SectionKind::References));
        assert_eq!(r.title.as_deref(), Some("Effect of Sleep on Memory Consolidation"));
    }

    #[test]
    fn extracts_pvalues_with_location() {
        let r = extract_from_text(SAMPLE);
        let pvals: Vec<_> = r
            .statistics
            .iter()
            .filter_map(|s| match &s.stat {
                Stat::PValue { value, .. } => Some((*value, s.location.section)),
                _ => None,
            })
            .collect();
        // p < 0.001 in Abstract, p = 0.03 in Results
        assert!(pvals.iter().any(|(v, sec)| (*v - 0.001).abs() < 1e-9
            && *sec == SectionKind::Abstract));
        assert!(pvals.iter().any(|(v, sec)| (*v - 0.03).abs() < 1e-9
            && *sec == SectionKind::Results));
    }

    #[test]
    fn extracts_test_names_ci_and_sample_size() {
        let r = extract_from_text(SAMPLE);
        let tests: Vec<_> = r
            .statistics
            .iter()
            .filter_map(|s| match &s.stat {
                Stat::Test { name, .. } => Some(name.clone()),
                _ => None,
            })
            .collect();
        assert!(tests.iter().any(|t| t == "t-test"));
        assert!(tests.iter().any(|t| t == "ANOVA"));

        assert!(r.statistics.iter().any(|s| matches!(&s.stat,
            Stat::ConfidenceInterval { low, high, .. } if (*low - 1.2).abs() < 1e-9 && (*high - 3.4).abs() < 1e-9)));
        assert!(r.statistics.iter().any(|s| matches!(&s.stat,
            Stat::SampleSize { n, .. } if *n == 120)));
    }

    #[test]
    fn extracts_both_citation_styles() {
        let r = extract_from_text(SAMPLE);
        // narrative: "Smith et al. (2023)"
        assert!(r.citations.iter().any(|c| c.style == CitationStyle::Narrative
            && c.authors.contains("Smith")
            && c.year == 2023));
        // parenthetical: "(Jones, 2019; Brown & Lee, 2020)"
        assert!(r.citations.iter().any(|c| c.style == CitationStyle::Parenthetical
            && c.authors.contains("Jones")
            && c.year == 2019));
        assert!(r.citations.iter().any(|c| c.style == CitationStyle::Parenthetical
            && c.authors.contains("Brown")
            && c.year == 2020));
    }

    #[test]
    fn parses_reference_list_with_doi() {
        let r = extract_from_text(SAMPLE);
        assert_eq!(r.references.len(), 2);
        let first = &r.references[0];
        assert!(first.authors.contains("Smith"));
        assert_eq!(first.year, Some(2023));
        assert_eq!(first.doi.as_deref(), Some("10.1234/jsleep.2023.045"));
        assert!(first.title.as_deref().unwrap().contains("Sleep and memory"));
        // second reference has no DOI
        assert_eq!(r.references[1].doi, None);
    }

    #[test]
    fn detects_table_reference() {
        let r = extract_from_text(SAMPLE);
        assert!(r.tables.iter().any(|t| t.label == "Table 1"));
    }
}
