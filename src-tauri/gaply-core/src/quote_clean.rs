//! Making a machine-extracted source quote readable WITHOUT changing what it
//! says (§11 D147).
//!
//! # The problem this exists for
//!
//! The audit report quotes source text and tells the reader that reading it IS
//! the check. What it was quoting is a retrieval chunk straight out of PDF
//! extraction: words broken across line breaks (`regres- sion`), and journal
//! running headers sitting inside the prose (`Alkenbrack et al. BMC Health
//! Services Research (2015) 15:473 DOI 10.1186/s12913-015-1132-5 and their
//! dependents.`).
//!
//! # The rule this module will not break
//!
//! Altering quoted evidence in a research-integrity tool is the failure the tool
//! exists to prevent, so **every edit is either visible or disclosed**:
//!
//! * an elided run is replaced by a visible `[...]`, never removed silently;
//! * de-hyphenation is invisible by nature, so it is covered by a standing
//!   disclosure printed with the quotes rather than by a marker per word.
//!
//! Nothing here selects WHICH sentences matter. That would be re-deciding what
//! supports the claim, which is the judgement this engine declines to let a
//! model make silently.

/// A second fragment that means the hyphen is SUSPENDED, not broken.
///
/// "low- and middle-income" is correct English. Joining it gives "lowand".
/// MEASURED on 1,623 real candidates: 15 of them are this shape, and all 15 are
/// genuine (`low- and`, `inter- and`, `low- or`, `closed- and`).
const SUSPENDED_BEFORE: [&str; 7] = ["and", "or", "to", "nor", "but", "versus", "vs"];

/// Rejoin words broken across a line break by PDF extraction.
///
/// # The measurement that chose this rule
///
/// Run over 1,276 indexed chunks (131k words) of the real corpus:
///
/// | rule | joins | left alone | verdict |
/// |---|---|---|---|
/// | join unless the second fragment is a conjunction | 1596 | 15 | SHIPPED |
/// | also keep the hyphen when the first fragment is a prefix | 1454 | 15 + 142 | REJECTED |
/// | join only when a fragment is not a standalone word | 832 | 779 | REJECTED |
///
/// The **prefix** variant was rejected by its own output: of its 142 cases, most
/// are ordinary words whose first syllable merely looks like a prefix, so it
/// produced `de-scribed`, `Pro-ceedings`, `multi-ple` and `Co-hen`. It would
/// have corrupted about 110 correct joins to rescue about 30 real compounds.
///
/// The **vocabulary** variant was rejected for leaving 779 genuine breaks
/// unfixed (`ef- fect`, `be- tween`, `How- ever`). Its vocabulary is built from
/// the same corpus, and the trailing fragments of broken words (`tion`, `sion`)
/// are preceded by a space, so they enter the vocabulary as standalone words and
/// teach the rule that the break is legitimate.
///
/// # What the shipped rule still gets wrong
///
/// A genuinely hyphenated compound that happens to break AT its own hyphen
/// loses that hyphen: `intra- annotator` becomes `intraannotator`. Measured at
/// roughly 2% of joins, and the result stays readable. The standing disclosure
/// covers it.
pub fn rejoin_line_break_hyphens(s: &str) -> String {
    let b: Vec<char> = s.chars().collect();
    let mut out = String::with_capacity(s.len());
    let mut i = 0usize;
    while i < b.len() {
        if b[i] != '-' {
            out.push(b[i]);
            i += 1;
            continue;
        }
        // A letter must precede the hyphen, and at least two of them: "e- mail"
        // is left alone, and so is any "- " that follows punctuation or a digit.
        let left_ok = out.chars().next_back().is_some_and(|c| c.is_alphabetic())
            && out.chars().rev().take(2).filter(|c| c.is_alphabetic()).count() == 2;
        // Whitespace, then a lowercase word.
        let mut j = i + 1;
        let mut spaces = 0;
        while j < b.len() && (b[j] == ' ' || b[j] == '\t') {
            j += 1;
            spaces += 1;
        }
        let word: String = b[j..].iter().take_while(|c| c.is_alphabetic()).collect();
        let next_lower = word.chars().count() >= 2 && word.chars().all(|c| c.is_lowercase());
        let suspended = SUSPENDED_BEFORE.contains(&word.to_lowercase().as_str());

        if left_ok && spaces > 0 && next_lower && !suspended {
            // Drop the hyphen AND the space: the two halves are one word.
            i = j;
            continue;
        }
        out.push('-');
        i += 1;
    }
    out
}

/// A journal running header or footer that PDF extraction left inside the prose.
///
/// Deliberately narrow: a run is only elided when it carries a DOI or the
/// `Journal (Year) Volume:Pages` shape, both of which are typography rather than
/// something an author wrote into a sentence.
fn header_runs(s: &str) -> Vec<(usize, usize)> {
    let mut spans = Vec::new();
    let bytes = s.as_bytes();
    let lower = s.to_lowercase();

    // A DOI, plus the citation furniture that usually precedes it on the same
    // line. Bounded so it can never swallow a paragraph.
    let mut from = 0usize;
    while let Some(rel) = lower[from..].find("doi") {
        let at = from + rel;
        let after = &s[at..];
        let is_doi = after.len() > 8
            && after[3..].trim_start().starts_with("10.")
            || after.starts_with("doi.org/10.");
        if is_doi {
            // Back up to the start of the citation furniture, at most 90 chars,
            // and never across a sentence end.
            let mut start = at;
            let mut budget = 90;
            while start > 0 && budget > 0 {
                let prev = s[..start].chars().next_back().unwrap();
                if prev == '.' && s[..start].ends_with(". ") {
                    break;
                }
                start -= prev.len_utf8();
                budget -= 1;
            }
            // Forward to the end of the DOI token.
            let mut end = at;
            for c in s[at..].chars() {
                if c.is_whitespace() {
                    break;
                }
                end += c.len_utf8();
            }
            // Include the DOI's own value, which follows a space in "DOI 10.x".
            let rest = &s[end..];
            if rest.starts_with(' ') {
                let mut e2 = end + 1;
                for c in rest[1..].chars() {
                    if c.is_whitespace() {
                        break;
                    }
                    e2 += c.len_utf8();
                }
                if s[end..e2].contains("10.") {
                    end = e2;
                }
            }
            spans.push((start, end));
        }
        from = at + 3;
        if from >= bytes.len() {
            break;
        }
    }
    spans
}

/// Clean a quoted passage for display.
///
/// Returns the text and whether anything was ELIDED, so the caller can say so.
/// De-hyphenation is not reported per call: it is covered by the standing
/// disclosure, because marking every rejoined word would make the quote
/// unreadable for a change that restores what the page said.
pub fn clean_quote(s: &str) -> (String, bool) {
    let spans = header_runs(s);
    let mut out = String::with_capacity(s.len());
    let mut cut = false;
    let mut last = 0usize;
    for (a, b) in spans {
        if a < last || b <= a || b > s.len() {
            continue;
        }
        out.push_str(&s[last..a]);
        out.push_str("[...] ");
        cut = true;
        last = b;
    }
    out.push_str(&s[last..]);
    let joined = rejoin_line_break_hyphens(&out);
    // Collapse the whitespace the elision and the joins leave behind.
    let collapsed = joined.split_whitespace().collect::<Vec<_>>().join(" ");
    (collapsed, cut)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn line_break_hyphens_are_rejoined() {
        assert_eq!(rejoin_line_break_hyphens("regres- sion analysis"), "regression analysis");
        assert_eq!(rejoin_line_break_hyphens("the Govern- ment of Lao"), "the Government of Lao");
        assert_eq!(rejoin_line_break_hyphens("mul- tiple and motiv- ation"), "multiple and motivation");
    }

    /// The 15 real cases that must survive untouched (§11 D147's measurement).
    #[test]
    fn a_suspended_hyphen_is_not_a_broken_word() {
        for s in ["low- and middle-income", "inter- and intra-group", "low- or no-cost", "closed- and open-ended"] {
            assert_eq!(rejoin_line_break_hyphens(s), s, "a suspended hyphen was joined");
        }
    }

    /// Shapes that are NOT a line break and must be left alone.
    #[test]
    fn ordinary_hyphens_survive() {
        for s in [
            "cross-sectional study",      // no space: not a line break
            "COVID- 19 cases",            // digits: not a word
            "e- mail",                    // one letter before the hyphen
            "well-being and self-esteem", // ordinary compounds
            "pp. 436-465, 2013",          // a page range
        ] {
            assert_eq!(rejoin_line_break_hyphens(s), s, "{s} was altered");
        }
    }

    #[test]
    fn a_journal_header_is_elided_visibly_and_never_silently() {
        let raw = "Alkenbrack et al. BMC Health Services Research (2015) 15:473 DOI \
                   10.1186/s12913-015-1132-5 and their dependents.";
        let (out, cut) = clean_quote(raw);
        assert!(cut, "the header was not detected");
        assert!(out.contains("[...]"), "an elision happened with no marker: {out}");
        assert!(!out.contains("10.1186"), "the DOI survived: {out}");
        assert!(out.contains("and their dependents."), "real prose was eaten: {out}");
    }

    #[test]
    fn a_quote_with_nothing_to_clean_is_returned_unchanged() {
        let raw = "Species richness rose 31% under organic management (p<0.01).";
        let (out, cut) = clean_quote(raw);
        assert_eq!(out, raw);
        assert!(!cut);
    }
}
