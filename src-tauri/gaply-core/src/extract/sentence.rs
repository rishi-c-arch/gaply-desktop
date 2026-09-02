//! Sentence-level lexical knowledge, and the one invariant every consumer of it
//! must satisfy.
//!
//! # THE INVARIANT — ONTOLOGY §4.24
//!
//! > **A SENTENCE BOUNDARY MAY NEVER FALL INSIDE A NUMERIC LITERAL.**
//!
//! This is §4.20's TEXT class caught before it shipped rather than after. A
//! boundary inside `0.05` yields `p ≤ 0.` — a value silently changed while being
//! read, which then looks like a value. The invariant belongs to ANY future
//! sentence consumer, not only to the one that motivated it: the implementation
//! below may be replaced, the property may not.
//!
//! # Why this module exists rather than reusing `docparse::ends_sentence`
//!
//! **`ends_sentence` FAILS the invariant, and that was established by RUNNING it,
//! not by reading it.** Measured:
//!
//! ```text
//! ends_sentence("improved recall (p < 0")   = false
//! ends_sentence("improved recall (p < 0.")  = TRUE   ← inside the decimal
//! ends_sentence("the value was 12.")        = TRUE
//! ends_sentence("reported by Smith et al.") = false  ← abbreviations DO work
//! ```
//!
//! *(The threshold phrasings that motivated this are deliberately NOT quoted
//! here: `threshold_markers_are_owned_by_extraction` proves the marker
//! vocabulary appears in exactly one file, and a doc example is a copy like any
//! other. The test caught this comment on its first run.)*
//!
//! **It is not wrong for its caller.** `ends_sentence` decides whether a physical
//! LINE closes a reference-list block during PDF reflow, and a line does not end
//! mid-number. Changing it would be a behaviour change to a pipeline whose
//! effect was measured (the reflow work: 81 parsed references down to 20), for a
//! defect it does not exhibit there. **It is left alone deliberately.**
//!
//! **What IS shared is the abbreviation knowledge**, which lives here and which
//! `docparse` imports — one definition of "what is an abbreviation", two callers
//! with different boundary needs. Sharing the boundary DECISION would force one
//! of the two to accept the other's failure mode.

/// Tokens ending in '.' that do NOT end a sentence.
///
/// **ONE DEFINITION, two callers**: `docparse::ends_sentence` (line/block
/// boundaries during PDF reflow) and [`sentence_containing`] (sentence bounds for
/// classification). A second list would drift.
pub(crate) const ABBREVIATIONS: &[&str] = &[
    "et al.", "e.g.", "i.e.", "cf.", "vs.", "approx.", "ca.", "etc.", "Fig.", "Figs.", "Tab.",
    "No.", "Dr.", "Prof.", "Mr.", "Mrs.", "Ms.", "St.", "Jr.", "Sr.", "Eq.", "Eqs.", "ref.",
    "refs.", "Ref.", "min.", "max.", "sec.", "wt.", "vol.", "conc.", "temp.", "spp.", "sp.",
    "subsp.", "var.", "p.", "pp.", "ed.", "eds.", "Inc.", "Ltd.", "Co.", "U.S.", "U.K.",
];

/// True when `tail` ends with an abbreviation **as a whole token** — the
/// preceding character must be non-alphanumeric or the string start. Without
/// this, `"unaffected."` matches the `"ed."` abbreviation and a real sentence
/// boundary is suppressed.
pub(crate) fn ends_with_abbreviation(tail: &str) -> bool {
    // Compare against whitespace-NORMALISED text. PDF extraction routinely
    // emits "et  al." with a doubled space, and `ABBREVIATIONS` holds
    // "et al." with one — so the literal match failed on exactly the text this
    // guard exists for. On the first real audited paper, 1 of the 4 "et al."
    // occurrences was doubled, and that is precisely the one that split:
    // "…was proposed by Wang  et  al." became an uncited sentence and
    // "(2016) to incorporate…" became an orphan fragment, so one whitespace
    // artifact manufactured two false findings out of one real citation.
    let flat: String = tail.split_whitespace().collect::<Vec<_>>().join(" ");
    // A trailing space is meaningful to `ends_with`, and `split_whitespace`
    // discards it; restore it so "cf. " still matches "cf.".
    let flat = if tail.ends_with(char::is_whitespace) { flat + " " } else { flat };
    ABBREVIATIONS.iter().any(|a| {
        flat.ends_with(a) && {
            let before = flat.len() - a.len();
            before == 0
                || !flat[..before].chars().next_back().map(|c| c.is_alphanumeric()).unwrap_or(false)
        }
    })
}

/// True when the terminator at byte index `i` in `text` closes a sentence.
///
/// `text[i]` must be `.`, `!` or `?`. The three suppressions, in the order they
/// matter:
///
/// 1. **NUMERIC LITERAL — the invariant.** A `.` with a digit on both sides is a
///    decimal point. `0.05`, `1.96`, `12.4` are single tokens and are never cut.
/// 2. **Abbreviation.** `"Smith et al."` does not close a sentence.
/// 3. **Single-capital initial.** `"… A."` / `"Bombyx mori L."` do not either.
///
/// A closing terminator must also be followed by whitespace or the end of the
/// text — `"0.05."` at the end of a clause closes, `"v1.2beta"` does not.
///
/// # THE INVARIANT IS ENFORCED TWICE — established against a STRENGTHENED test
///
/// Removing (1) alone leaves the property holding, because a decimal point is
/// also followed by a digit and so fails the token-boundary rule. Removing the
/// token-boundary rule alone leaves it holding too, because (1) catches it.
/// **Only removing BOTH breaks the invariant, and the test catches that.**
///
/// **Both survivals were RE-RUN after the test's guard was widened**, so the
/// redundancy is not an artifact of a weak instrument — which is a stronger
/// claim than the first run could support. The control (both clauses removed)
/// is now caught through `0.05`, the literal this module exists to protect;
/// under the weaker guard it was caught only through `12.4`.
///
/// Both clauses are kept. They overlap on decimals and are independently
/// justified elsewhere — (1) states the invariant where a reader looks for it,
/// the token-boundary rule also rejects `"v1.2beta"`, which (1) does not.
/// **The property is held by the TEST, not by either clause**, which is the
/// honest reading and the reason a single-clause edit is safe and a two-clause
/// edit is not.
///
/// **THE REDUNDANCY IS INTENTIONAL AND RECORDED HERE** so a future reader does
/// not remove one clause as dead — finding the tests still green — and then
/// remove the other as equally dead.
pub(crate) fn closes_sentence(text: &str, i: usize) -> bool {
    let bytes = text.as_bytes();
    let terminator = bytes[i] as char;
    debug_assert!(matches!(terminator, '.' | '!' | '?'));

    let prev = text[..i].chars().next_back();
    let next = text[i + 1..].chars().next();

    // (1) THE INVARIANT. Never inside a numeric literal.
    if terminator == '.' {
        if let (Some(p), Some(n)) = (prev, next) {
            if p.is_ascii_digit() && n.is_ascii_digit() {
                return false;
            }
        }
    }

    // A terminator that is not followed by a break is inside a token.
    match next {
        None => {}
        Some(c) if c.is_whitespace() => {}
        _ => return false,
    }

    if terminator != '.' {
        return true;
    }

    // (2) abbreviations, and (3) single-capital initials.
    let head = &text[..=i];
    if ends_with_abbreviation(head) {
        return false;
    }
    let mut it = head.chars().rev();
    it.next(); // the '.'
    match (it.next(), it.next()) {
        (Some(c), Some(before)) if c.is_uppercase() && !before.is_alphanumeric() => false,
        (Some(c), None) if c.is_uppercase() => false,
        _ => true,
    }
}

/// The sentence of `text` containing byte offset `at`.
///
/// Returns the trimmed slice between the sentence boundaries either side of
/// `at`. When no boundary is found in a direction, that end of `text` is used —
/// a paragraph with no terminator is one sentence, which is the honest reading.
///
/// `at` must be a char boundary within `text`; an out-of-range offset yields the
/// whole trimmed text rather than panicking, because the caller's offset comes
/// from a regex match and a panic here would take down extraction.
pub fn sentence_containing(text: &str, at: usize) -> &str {
    if at >= text.len() || !text.is_char_boundary(at) {
        return text.trim();
    }

    let start = text[..at]
        .char_indices()
        .filter(|(i, c)| matches!(c, '.' | '!' | '?') && closes_sentence(text, *i))
        .map(|(i, c)| i + c.len_utf8())
        .next_back()
        .unwrap_or(0);

    let end = text[at..]
        .char_indices()
        .find(|(i, c)| matches!(c, '.' | '!' | '?') && closes_sentence(text, at + *i))
        .map(|(i, c)| at + i + c.len_utf8())
        .unwrap_or(text.len());

    text[start..end].trim()
}

/// Split a paragraph into sentences.
///
/// Returns the trimmed slice for each sentence. The same boundary rules as
/// [`sentence_containing`] apply, preserving the numeric-literal invariant.
pub fn sentences_in(text: &str) -> Vec<&str> {
    if text.is_empty() {
        return Vec::new();
    }

    let mut out = Vec::new();
    let mut start = 0usize;

    for (i, c) in text.char_indices() {
        if matches!(c, '.' | '!' | '?') && closes_sentence(text, i) {
            let end = i + c.len_utf8();
            let trimmed = text[start..end].trim();
            if !trimmed.is_empty() {
                out.push(trimmed);
            }
            start = end;
        }
    }

    if start < text.len() {
        let tail = text[start..].trim();
        if !tail.is_empty() {
            out.push(tail);
        }
    }

    out
}

#[cfg(test)]
mod tests {

    #[test]
    fn pdf_doubled_whitespace_does_not_defeat_an_abbreviation() {
        // The exact text from the first audited paper. One whitespace artifact
        // used to manufacture two false findings from one real citation: an
        // "uncited" sentence ending at "Wang  et  al." and an orphan fragment
        // starting "(2016) to incorporate…".
        let mangled = "The  ATAE-LSTM  model  was  proposed  by  Wang  et  al.  (2016)  to incorporate the aspect information.";
        let got = sentences_in(mangled);
        assert_eq!(got.len(), 1, "split at a doubled-space abbreviation: {got:?}");

        // Single-spaced still works, and so does the rest of the list.
        assert_eq!(sentences_in("Reported by Smith et al. (2019) in the trial.").len(), 1);
        assert!(ends_with_abbreviation("reported by Smith et  al."));
        assert!(ends_with_abbreviation("reported by Smith et al."));
        assert!(ends_with_abbreviation("see  Fig."));

        // And a real sentence boundary is still a boundary.
        assert_eq!(sentences_in("Yields rose.  The effect held.").len(), 2);
    }

    use super::*;

    /// **THE INVARIANT, asserted directly.** A boundary landing inside a decimal
    /// is the failure this module exists to prevent, and the failure
    /// `docparse::ends_sentence` exhibits.
    #[test]
    fn a_sentence_boundary_never_falls_inside_a_numeric_literal() {
        let cases = [
            "Group means were compared using a cut-off of p \u{2264} 0.05.",
            "Recall improved (p < 0.001) in the treated group.",
            "The mean was 12.4 and the SD was 3.16 across arms.",
            "Values of 0.05, 0.01 and 0.001 were used as cut-offs.",
        ];
        for text in cases {
            // Every offset in the text must resolve to a sentence that contains
            // each numeric literal WHOLE.
            for at in 0..text.len() {
                if !text.is_char_boundary(at) {
                    continue;
                }
                let s = sentence_containing(text, at);
                for lit in ["0.05", "0.001", "12.4", "3.16", "0.01"] {
                    // `text.contains(lit)` scopes the literal to the case that
                    // owns it — without it, `0.05`'s prefix would match inside
                    // `0.001` and report a split that did not happen.
                    if text.contains(lit) {
                        // If the sentence contains the literal's mantissa prefix,
                        // it must contain the WHOLE literal — never a truncation.
                        let prefix_only =
                            s.contains(&lit[..lit.find('.').unwrap() + 1]) && !s.contains(lit);
                        assert!(
                            !prefix_only,
                            "boundary split {lit:?} at offset {at} in {text:?} -> {s:?}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn the_decimal_case_docparse_gets_wrong() {
        // docparse::ends_sentence("…p ≤ 0.") returns TRUE. This must not.
        let text = "Group means were compared at p \u{2264} 0.05. Differences failed to reach it.";
        let at = text.find("0.05").unwrap();
        assert_eq!(
            sentence_containing(text, at),
            "Group means were compared at p \u{2264} 0.05.",
            "the decimal point must not close the sentence"
        );
    }

    #[test]
    fn abbreviations_do_not_close_a_sentence() {
        let text = "Reported by Smith et al. in a later trial. A second study followed.";
        let at = text.find("Smith").unwrap();
        assert_eq!(sentence_containing(text, at), "Reported by Smith et al. in a later trial.");
    }

    #[test]
    fn initials_do_not_close_a_sentence() {
        let text = "The host was Bombyx mori L. under standard rearing. Values followed.";
        let at = text.find("host").unwrap();
        assert_eq!(
            sentence_containing(text, at),
            "The host was Bombyx mori L. under standard rearing."
        );
    }

    #[test]
    fn boundaries_are_found_on_both_sides() {
        let text = "First sentence here. Second one has p = 0.03 in it. Third follows.";
        let at = text.find("0.03").unwrap();
        assert_eq!(sentence_containing(text, at), "Second one has p = 0.03 in it.");
    }

    #[test]
    fn a_paragraph_without_a_terminator_is_one_sentence() {
        let text = "NS, not significant at p \u{2264} 0.05";
        assert_eq!(sentence_containing(text, 4), text);
    }

    #[test]
    fn an_out_of_range_offset_yields_the_whole_text_rather_than_panicking() {
        let text = "Short text.";
        assert_eq!(sentence_containing(text, 9_999), "Short text.");
    }
}
