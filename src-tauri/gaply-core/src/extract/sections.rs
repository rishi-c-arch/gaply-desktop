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
    /// **The OTHER sections this heading also names — the "mixed" label.**
    ///
    /// `"Results and Discussion"` is one section in the document and two in the
    /// IMRaD vocabulary. `kind` carries the one that governs it; this carries
    /// the rest, so a consumer asking "does this manuscript have a Discussion"
    /// is not told no because the author combined two headings.
    ///
    /// Empty for an ordinary heading. Additive and `serde(default)`, so every
    /// report cached before this field existed still parses.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub also_covers: Vec<SectionKind>,
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

/// **Every IMRaD section a heading names, in the order it names them.**
///
/// The lexicon is EXACT-MATCH by design: a sentence merely starting "Methods…"
/// is not a heading. That is right, and it meant a heading which names a
/// section alongside anything else named none at all.
///
/// Measured 22 Sep 2026 (docs/PROBLEM_DOSSIER.md A2): R PAPER's
/// `"IV. EXPERIMENTAL SETUP AND RESULTS"` is 4 words, so it passes the shape
/// gate, and `"experimental setup and results"` is not a lexicon phrase, so it
/// was not a heading. Its 126 paragraphs — the paper's entire results, tables
/// and figures — were absorbed into the preceding Methods section, and the
/// checklist reported **"Results section missing"** on a paper that has one.
/// **Three of the six corpus manuscripts had no detected Results section.**
///
/// # Why splitting is safe here when it was not for run-in headings
///
/// This splits on CONJUNCTIONS only, and asks the SAME exact-match lexicon
/// about each part. A part that is not a section name contributes nothing, so
/// the rule cannot invent a section from arbitrary prose — the five-word shape
/// gate still fires first, and `"Related Work and Results of Prior Studies"` is
/// rejected twice over: seven words, and `"results of prior studies"` is not a
/// lexicon phrase.
fn classify_heading_parts(phrase: &str) -> Vec<SectionKind> {
    if let Some(k) = classify_heading(phrase) {
        return vec![k];
    }
    // A TABLE-OF-CONTENTS LINE IS NOT A HEADING, and splitting makes one look
    // like one. Measured 22 Sep 2026 on `final final L.pdf`: the contents line
    //
    //     "RESULTS AND DISCUSSION ....................................... 260"
    //
    // is five whitespace-separated tokens, so it passes the shape gate, and
    // splitting on " and " isolates a clean "results" from the dotted leader —
    // inventing a Results section 200 pages before the real one. §11 D182 is
    // the same hazard: the earliest mention of a topic in a thesis is its table
    // of contents.
    //
    // A real heading names sections and nothing else, so a dot leader or a page
    // number disqualifies the whole line from the compound path. The exact-match
    // path above is untouched and still admits "Results and Discussion".
    if phrase.contains("..") || phrase.chars().any(|c| c.is_ascii_digit()) {
        return Vec::new();
    }
    let mut out = Vec::new();
    for part in phrase.split(" and ").flat_map(|p| p.split(" & ")).flat_map(|p| p.split('/')) {
        if let Some(k) = classify_heading(part.trim()) {
            if !out.contains(&k) {
                out.push(k);
            }
        }
    }
    out
}

/// Return `(kind, original_heading)` if `line` is a section heading.
///
/// A heading is a short line that — after stripping leading numbering
/// ("1.", "2)", "IV.") and a trailing colon — matches a known phrase
/// exactly. The exact-match requirement prevents sentences that merely start
/// with "Methods…" from being treated as headings.
pub(crate) fn detect_heading(line: &str) -> Option<(SectionKind, String, Vec<SectionKind>)> {
    let trimmed = line.trim();
    if trimmed.is_empty() || trimmed.split_whitespace().count() > 5 {
        return None;
    }
    let stripped = regexes().heading_number.replace(trimmed, "");
    let phrase = stripped.trim().trim_end_matches(':').trim().to_lowercase();
    let mut kinds = classify_heading_parts(&phrase);
    if kinds.is_empty() {
        return None;
    }
    // The FIRST named section governs, matching the lexicon this rule extends:
    // "results and discussion" is already an entry and already resolves to
    // Results. Taking the last instead made "Results & Discussion" a Discussion
    // section while "Results and Discussion" stayed Results — the same heading
    // classified two ways by which conjunction the author typed.
    //
    // For a heading whose other half names no section at all — "Experimental
    // Setup and Results" — there is only one candidate and the choice does not
    // arise.
    let kind = kinds.remove(0);
    Some((kind, trimmed.to_string(), kinds))
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
    let (kind, heading, _also) = detect_heading(prefix)?;
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
    detect_heading(line).map(|(k, h, _)| (k, h))
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
    // The fifth element is the MIXED label: the other IMRaD sections a compound
    // heading also names ("Results and Discussion" is one section and two names).
    let mut heads: Vec<(usize, SectionKind, String, Option<String>, Vec<SectionKind>)> =
        Vec::new();
    for (i, l) in lines.iter().enumerate() {
        if let Some((k, h, also)) = detect_heading(l) {
            heads.push((i, k, h, None, also));
            continue;
        }
        // PREAMBLE ONLY. See `detect_runin_heading`: unrestricted this admits 37
        // lines across the corpus, 18 of them one parameter table.
        if heads.is_empty() {
            if let Some((k, h, rest)) = detect_runin_heading(l) {
                heads.push((i, k, h, Some(rest), Vec::new()));
            }
        }
    }

    let first_head = heads.first().map(|(i, _, _, _, _)| *i).unwrap_or(lines.len());

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
                    also_covers: Vec::new(),
                });
            }
        }
    }

    // Each heading owns the lines up to the next heading.
    for (idx, (line_i, kind, heading, runin, also)) in heads.iter().enumerate() {
        let body_start = line_i + 1;
        let body_end = heads.get(idx + 1).map(|(n, _, _, _, _)| *n).unwrap_or(lines.len());
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
        sections.push(Section {
            kind: *kind,
            heading: heading.clone(),
            paragraphs,
            also_covers: also.clone(),
        });
    }

    (title, sections)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **A heading that names Results alongside something else IS a Results
    /// heading.** docs/PROBLEM_DOSSIER.md A2: R PAPER's
    /// "IV. EXPERIMENTAL SETUP AND RESULTS" named none, so its 126 paragraphs
    /// of results and tables were absorbed into the preceding Methods section
    /// and the checklist reported "Results section missing" on a paper that has
    /// one. Three of the six corpus manuscripts had no detected Results section.
    #[test]
    fn a_compound_heading_names_the_section_it_contains() {
        for line in [
            "EXPERIMENTAL SETUP AND RESULTS",
            "IV. EXPERIMENTAL SETUP AND RESULTS",
            "Experimental Setup and Results",
            "Results & Discussion",
        ] {
            let got = detect_heading(line);
            assert!(got.is_some(), "not recognised as a heading at all: {line:?}");
            assert_eq!(
                got.as_ref().map(|(k, _, _)| *k),
                Some(SectionKind::Results),
                "compound heading did not resolve to Results: {line:?} -> {got:?}"
            );
        }
    }

    /// **The MIXED label: the other sections the heading also names.**
    /// "Results and Discussion" is one section in the document and two in the
    /// IMRaD vocabulary; `kind` carries the governing one and `also_covers`
    /// carries the rest, so asking "does this paper have a Discussion" is not
    /// answered no because the author combined two headings.
    #[test]
    fn a_compound_heading_records_the_other_sections_it_names() {
        // NOT "Results and Discussion": that is an EXACT lexicon entry, so it
        // never reaches the compound path and carries no mixed label. A
        // deletion test proved the point — disabling the compound path left an
        // earlier version of this assertion GREEN, because it accepted both an
        // empty label and a populated one. An assertion that cannot fail is not
        // a test.
        let (kind, _, also) = detect_heading("Results & Discussion").expect("a heading");
        assert_eq!(kind, SectionKind::Results, "the first named section governs");
        assert_eq!(
            also,
            vec![SectionKind::Discussion],
            "the other section the heading names must be recorded"
        );
        // An ordinary heading is not mixed.
        let (_, _, plain) = detect_heading("Methods").expect("a heading");
        assert!(plain.is_empty(), "an ordinary heading must carry no mixed label: {plain:?}");
    }

    /// **NEGATIVE CONTROL: a heading about OTHER people's results is not this
    /// manuscript's Results section.**
    ///
    /// Rejected twice over, and both reasons are load-bearing: seven words
    /// exceeds the five-word shape gate, and "results of prior studies" is not
    /// a lexicon phrase, so the compound path finds nothing either.
    #[test]
    fn a_heading_about_other_work_does_not_become_this_papers_results() {
        for line in [
            "Related Work and Results of Prior Studies",
            "Comparison with Results Reported Elsewhere",
            "Discussion of Results in the Literature",
        ] {
            assert_ne!(
                detect_heading(line).map(|(k, _, _)| k),
                Some(SectionKind::Results),
                "invented a Results section from a heading about other work: {line:?}"
            );
        }
    }

    /// **NEGATIVE CONTROL: a table-of-contents line is not a heading.**
    ///
    /// This one is a REGRESSION THE CORPUS CAUGHT. The first version of the
    /// compound rule admitted `final final L.pdf`'s contents line — five
    /// whitespace tokens, so it passed the shape gate — and split a clean
    /// "results" out of the dotted leader, inventing a Results section 200 pages
    /// before the real one. §11 D182 is the same hazard.
    #[test]
    fn a_table_of_contents_line_is_not_a_compound_heading() {
        for line in [
            "RESULTS AND DISCUSSION .................................................. 260",
            "Materials and Methods ........ 41",
            "Results and Discussion 260",
        ] {
            assert_eq!(
                detect_heading(line).map(|(k, _, _)| k),
                None,
                "a contents line became a heading: {line:?}"
            );
        }
        // …while the real heading, with no leader and no page number, still is.
        assert_eq!(
            detect_heading("RESULTS AND DISCUSSION").map(|(k, _, _)| k),
            Some(SectionKind::Results)
        );
    }

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
