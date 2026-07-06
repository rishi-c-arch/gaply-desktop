//! Heuristic IMRaD section splitter for manuscript plaintext.

use serde::{Deserialize, Serialize};

use crate::extract::stats::regexes;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SectionKind {
    Abstract,
    Introduction,
    Methods,
    Results,
    Discussion,
    Conclusion,
    References,
    /// Front matter (authors, affiliations) or unrecognized headings.
    Other,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Section {
    pub kind: SectionKind,
    pub heading: String,
    pub paragraphs: Vec<String>,
}

/// Map a cleaned, lowercased heading phrase to a section kind.
fn classify_heading(phrase: &str) -> Option<SectionKind> {
    let kind = match phrase {
        "abstract" | "summary" => SectionKind::Abstract,
        "introduction" | "background" => SectionKind::Introduction,
        "methods" | "method" | "materials and methods" | "materials & methods"
        | "methodology" | "experimental methods" | "materials and methods." => {
            SectionKind::Methods
        }
        "results" | "findings" | "results and discussion" => SectionKind::Results,
        "discussion" => SectionKind::Discussion,
        "conclusion" | "conclusions" | "concluding remarks" => SectionKind::Conclusion,
        "references" | "bibliography" | "works cited" | "literature cited" => {
            SectionKind::References
        }
        _ => return None,
    };
    Some(kind)
}

/// Return `(kind, original_heading)` if `line` is a section heading.
///
/// A heading is a short line that — after stripping leading numbering
/// ("1.", "2)", "IV.") and a trailing colon — matches a known phrase
/// exactly. The exact-match requirement prevents sentences that merely start
/// with "Methods…" from being treated as headings.
fn detect_heading(line: &str) -> Option<(SectionKind, String)> {
    let trimmed = line.trim();
    if trimmed.is_empty() || trimmed.split_whitespace().count() > 5 {
        return None;
    }
    let stripped = regexes().heading_number.replace(trimmed, "");
    let phrase = stripped.trim().trim_end_matches(':').trim().to_lowercase();
    classify_heading(&phrase).map(|kind| (kind, trimmed.to_string()))
}

/// Split a body block into paragraphs: separated by blank lines, with
/// wrapped lines inside a paragraph re-joined by spaces.
fn paragraphs_of(lines: &[&str]) -> Vec<String> {
    let mut paragraphs = Vec::new();
    let mut current: Vec<&str> = Vec::new();
    for line in lines {
        if line.trim().is_empty() {
            if !current.is_empty() {
                paragraphs.push(current.join(" ").split_whitespace().collect::<Vec<_>>().join(" "));
                current.clear();
            }
        } else {
            current.push(line.trim());
        }
    }
    if !current.is_empty() {
        paragraphs.push(current.join(" ").split_whitespace().collect::<Vec<_>>().join(" "));
    }
    paragraphs
}

/// Parse a document into an optional title plus ordered sections.
pub fn split_document(text: &str) -> (Option<String>, Vec<Section>) {
    let lines: Vec<&str> = text.lines().collect();

    // Locate heading lines.
    let heads: Vec<(usize, SectionKind, String)> = lines
        .iter()
        .enumerate()
        .filter_map(|(i, l)| detect_heading(l).map(|(k, h)| (i, k, h)))
        .collect();

    let first_head = heads.first().map(|(i, _, _)| *i).unwrap_or(lines.len());

    // Preamble: everything before the first heading. First non-empty line is
    // the title; the rest (authors, affiliations) becomes an `Other` section.
    let mut title = None;
    let mut sections = Vec::new();
    {
        let preamble = &lines[..first_head];
        let mut title_idx = None;
        for (i, l) in preamble.iter().enumerate() {
            if !l.trim().is_empty() {
                title = Some(l.trim().to_string());
                title_idx = Some(i);
                break;
            }
        }
        if let Some(ti) = title_idx {
            let rest = paragraphs_of(&preamble[ti + 1..]);
            if !rest.is_empty() {
                sections.push(Section {
                    kind: SectionKind::Other,
                    heading: String::new(),
                    paragraphs: rest,
                });
            }
        }
    }

    // Each heading owns the lines up to the next heading.
    for (idx, (line_i, kind, heading)) in heads.iter().enumerate() {
        let body_start = line_i + 1;
        let body_end = heads.get(idx + 1).map(|(n, _, _)| *n).unwrap_or(lines.len());
        let body = &lines[body_start..body_end];
        // Reference lists are one entry per line — don't merge wrapped lines,
        // or adjacent references would collapse into a single paragraph.
        let paragraphs = if *kind == SectionKind::References {
            body.iter().map(|l| l.trim()).filter(|l| !l.is_empty()).map(String::from).collect()
        } else {
            paragraphs_of(body)
        };
        sections.push(Section { kind: *kind, heading: heading.clone(), paragraphs });
    }

    (title, sections)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbered_headings_are_recognized() {
        let text = "Title Line\n\n1. Introduction\nHello world.\n\n2. Methods\nWe did things.";
        let (title, secs) = split_document(text);
        assert_eq!(title.as_deref(), Some("Title Line"));
        assert_eq!(secs.iter().filter(|s| s.kind == SectionKind::Introduction).count(), 1);
        assert_eq!(secs.iter().filter(|s| s.kind == SectionKind::Methods).count(), 1);
    }

    #[test]
    fn sentence_starting_with_keyword_is_not_a_heading() {
        let text = "T\n\nMethods were applied carefully to every sample in the cohort study.";
        let (_, secs) = split_document(text);
        assert!(secs.iter().all(|s| s.kind != SectionKind::Methods));
    }

    #[test]
    fn paragraphs_split_on_blank_lines() {
        let text = "T\n\nResults\nPara one line one\nline two.\n\nPara two.";
        let (_, secs) = split_document(text);
        let results = secs.iter().find(|s| s.kind == SectionKind::Results).unwrap();
        assert_eq!(results.paragraphs.len(), 2);
        assert_eq!(results.paragraphs[0], "Para one line one line two.");
    }
}
