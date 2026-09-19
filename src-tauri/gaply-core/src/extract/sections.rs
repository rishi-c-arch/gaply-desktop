//! Heuristic IMRaD section splitter for manuscript plaintext.

use serde::{Deserialize, Serialize};

use crate::extract::stats::regexes;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
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
pub(crate) fn detect_heading(line: &str) -> Option<(SectionKind, String)> {
    let trimmed = line.trim();
    if trimmed.is_empty() || trimmed.split_whitespace().count() > 5 {
        return None;
    }
    let stripped = regexes().heading_number.replace(trimmed, "");
    let phrase = stripped.trim().trim_end_matches(':').trim().to_lowercase();
    classify_heading(&phrase).map(|kind| (kind, trimmed.to_string()))
}

/// **A RUN-IN heading: the heading and the body share one line. §11 D189.**
///
/// `detect_heading` rejects at `split_whitespace().count() > 5` before the
/// lexicon is consulted, so `"Abstract- Emotion detection in social media text…"`
/// — 182 words — was never a heading and the manuscript was told its abstract
/// was missing while finding 1 quoted it.
///
/// **The hyphen was never the rule.** `"Abstract: …"` fails identically at 182
/// words; the word count fires before any separator is examined. So this splits
/// the line at the separator and asks the REAL [`detect_heading`] about the
/// prefix — same lexicon, same numbering strip, same five-word bound applied to
/// the heading rather than to the paragraph glued behind it.
///
/// Returns `(kind, heading, remainder)`; the remainder is the body text that
/// must open the section, or the run-in half is silently dropped.
///
/// # This is only safe in the PREAMBLE, and that is measured
///
/// Applied anywhere in a document it admits **37 lines across the 32-manuscript
/// corpus** — 18 of them one file's parameter table (`"Method: Thermometric;
/// APHA Method No.: 2550 B"`), which would split a single Methods section
/// eighteen times and move every statistic after each split into a section that
/// does not exist. A false heading is worse than a missed one.
///
/// Restricted to lines before the first accepted heading it admits **1 line in
/// 29,741**, which is the one it is for. The restriction is structural, not
/// fitted: an abstract is a paper's first section, so a run-in abstract heading
/// cannot follow one. [`split_document`] enforces it; this function does not
/// know where it is and must not be called elsewhere.
pub(crate) fn detect_runin_heading(line: &str) -> Option<(SectionKind, String, String)> {
    let caps = regexes().runin_heading.captures(line.trim())?;
    let prefix = caps.get(1)?.as_str().trim();
    let rest = caps.get(2)?.as_str().trim();
    if rest.is_empty() {
        // `"Abstract:"` with nothing after it is an ordinary heading with a
        // trailing colon, which `detect_heading` already handles.
        return None;
    }
    let (kind, heading) = detect_heading(prefix)?;
    Some((kind, heading, rest.to_string()))
}

/// [`detect_heading`], reachable from a probe.
///
/// `detect_heading` is `pub(crate)` and must stay that way — it is an internal
/// step of `split_document`, not an API. `examples/heading_vocab_probe.rs` needs
/// exactly it, though: a probe that reimplemented the shape gate would be
/// measuring its own copy of the rule against the corpus, and would agree with
/// the classifier by construction.
pub fn detect_heading_for_probe(line: &str) -> Option<(SectionKind, String)> {
    detect_heading(line)
}

/// [`detect_runin_heading`], for the same reason as above.
pub fn detect_runin_heading_for_probe(line: &str) -> Option<(SectionKind, String, String)> {
    detect_runin_heading(line)
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
    // The fourth element is a RUN-IN remainder: body text that shared the
    // heading's line and must open the section (§11 D189). `None` for every
    // ordinary heading, which is all of them after the first.
    let mut heads: Vec<(usize, SectionKind, String, Option<String>)> = Vec::new();
    for (i, l) in lines.iter().enumerate() {
        if let Some((k, h)) = detect_heading(l) {
            heads.push((i, k, h, None));
            continue;
        }
        // PREAMBLE ONLY. See `detect_runin_heading`: unrestricted this admits 37
        // lines across the corpus, 18 of them one parameter table.
        if heads.is_empty() {
            if let Some((k, h, rest)) = detect_runin_heading(l) {
                heads.push((i, k, h, Some(rest)));
            }
        }
    }

    let first_head = heads.first().map(|(i, _, _, _)| *i).unwrap_or(lines.len());

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
    for (idx, (line_i, kind, heading, runin)) in heads.iter().enumerate() {
        let body_start = line_i + 1;
        let body_end = heads.get(idx + 1).map(|(n, _, _, _)| *n).unwrap_or(lines.len());
        let body = &lines[body_start..body_end];
        // Reference lists are one entry per line — don't merge wrapped lines,
        // or adjacent references would collapse into a single paragraph.
        let paragraphs = if *kind == SectionKind::References {
            body.iter().map(|l| l.trim()).filter(|l| !l.is_empty()).map(String::from).collect()
        } else {
            paragraphs_of(body)
        };
        // The run-in remainder IS the section's first paragraph. Dropping it
        // would recognise the heading and lose the abstract behind it, which is
        // the same denial one step later.
        let paragraphs = match runin {
            Some(rest) => std::iter::once(rest.clone()).chain(paragraphs).collect(),
            None => paragraphs,
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

    /// **§11 D189 — the abstract the product used to deny while quoting it.**
    /// The heading and 182 words of body share one line; the word-count gate
    /// fired before any separator was examined, so a colon would have failed
    /// identically and the hyphen was never the rule.
    #[test]
    fn a_run_in_abstract_is_a_heading_and_keeps_its_body() {
        let text = "PAPER TITLE\n\nAbstract- Emotion detection in social media text must deal \
with five challenges.\n\nIntroduction\nPrior work exists.";
        let (_, secs) = split_document(text);
        let a = secs.iter().find(|s| s.kind == SectionKind::Abstract).expect("abstract found");
        // THE BODY COMES WITH IT. Recognising the heading and dropping the text
        // behind it is the same denial one step later.
        assert!(
            a.paragraphs.first().is_some_and(|p| p.starts_with("Emotion detection")),
            "the run-in remainder opens the section: {:?}",
            a.paragraphs
        );
    }

    /// A colon fails the same way a hyphen does, which is the point.
    #[test]
    fn the_separator_was_never_what_mattered() {
        for sep in ["-", ":", ".", "\u{2013}"] {
            let text = format!("T\n\nAbstract{sep} Body text here.\n\nIntroduction\nx.");
            let (_, secs) = split_document(&text);
            assert!(
                secs.iter().any(|s| s.kind == SectionKind::Abstract),
                "separator {sep:?} must behave like the others"
            );
        }
    }

    /// **THE PREAMBLE RESTRICTION IS THE WHOLE SAFETY ARGUMENT.** Unrestricted,
    /// this rule admits 37 lines across the 32-manuscript corpus — 18 of them
    /// one file's parameter table. The identical line must be inert once a
    /// heading has been seen.
    #[test]
    fn a_run_in_line_after_a_heading_is_not_a_heading() {
        let text = "T\n\nMethods\nWe measured things.\n\nMethod: Thermometric; APHA Method No.: 2550 B; Units: degrees Celsius.";
        let (_, secs) = split_document(text);
        assert_eq!(
            secs.iter().filter(|s| s.kind == SectionKind::Methods).count(),
            1,
            "the parameter row must not open a second Methods section: {:?}",
            secs.iter().map(|s| (&s.kind, &s.heading)).collect::<Vec<_>>()
        );
    }

    /// §11 D189 rule 4: numbering glued to the word with no space.
    #[test]
    fn numbering_glued_to_the_heading_is_stripped() {
        let text = "T\n\nIV.Introduction\nx.\n\nVI.CONCLUSION\nDone.";
        let (_, secs) = split_document(text);
        assert!(secs.iter().any(|s| s.kind == SectionKind::Conclusion), "{secs:?}");
        assert!(secs.iter().any(|s| s.kind == SectionKind::Introduction), "{secs:?}");
    }

    /// **THE TRAP the mandatory `[.)]` exists for — and the input that catches it.**
    ///
    /// With the separator optional (`[.)]?\s*`), `[IVXLCM]+` eats leading
    /// numeral-letters off ANY word. Measured: `"Methods"` -> `"ethods"`,
    /// `"Conclusion"` -> `"onclusion"`, `"Introduction"` -> `"ntroduction"`. So
    /// the damage is not mainly to prose — **every heading beginning with I, V,
    /// X, L, C or M stops being recognised**, which is most of the lexicon.
    ///
    /// **The first version of this test asserted only that `"IVMethods"` and
    /// `"CLIMATE"` are not headings, and the deletion test showed both hold
    /// under the trap too** (`IVM` -> `"ethods"`, `CLIM` -> `"ATE"`): true
    /// statements that discriminate nothing. The three below are what separate
    /// the two regexes.
    #[test]
    fn numeral_letters_are_not_eaten_off_the_front_of_headings() {
        // THE DISCRIMINATORS: each fails under `[.)]?\s*`.
        assert!(detect_heading("Methods").is_some(), "the M must survive");
        assert!(detect_heading("Conclusion").is_some(), "the C must survive");
        assert!(detect_heading("Introduction").is_some(), "the I must survive");
        // Non-discriminating, kept as the plain statement of the rule.
        assert!(detect_heading("IVMethods").is_none(), "no separator, no stripping");
        assert!(detect_heading("CLIMATE").is_none());
        // …and the legitimate numbered forms keep working.
        assert!(detect_heading("IV. Methods").is_some());
        assert!(detect_heading("IV.Methods").is_some());
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
