//! **What a Level 0 method is given: one result, and what its document says
//! about all of them.**
//!
//! [`DocumentContext`] is read once per document. It holds the stated
//! significance thresholds (from Step 3's reader, never a second detector) and
//! two facts a p-value check needs from the prose: whether the document says
//! any test was one-tailed, and whether it names a correction for multiple
//! testing. Both are read from every section's heading and paragraphs except
//! References, the same scope the result reader scans.
//!
//! A document-level flag is coarse on purpose: it can only move a verdict away
//! from `Inconsistent` (to a noted `Consistent` or to `CouldNotCheck`), never
//! towards it.
//!
//! Deliberately NOT `Serialize`: the thresholds keep their printed text.

use std::sync::OnceLock;

use regex::Regex;

use super::method::TextPosition;
use super::number::ReportedNumber;
use super::result::{ReportedResult, ResultReading};
use crate::equation::rational::Rational;
use crate::extract::{ExtractionResult, SectionKind};

/// The document-wide facts a Level 0 method may use.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentContext {
    /// The distinct stated significance thresholds, as [`ResultReading`] holds them.
    pub thresholds: Vec<ReportedNumber>,
    /// Where "one-tailed", "one-sided" or "directional" first appears, if it
    /// does. "non-directional" and "bidirectional" do not count.
    pub one_tailed_at: Option<TextPosition>,
    /// Where a correction for multiple testing is first named, if one is.
    pub correction_at: Option<TextPosition>,
}

impl DocumentContext {
    pub fn one_tailed_stated(&self) -> bool {
        self.one_tailed_at.is_some()
    }
    pub fn correction_named(&self) -> bool {
        self.correction_at.is_some()
    }

    /// The alpha Level 0 judges against: every distinct stated threshold's
    /// printed value, or .05 when none is stated, with `true` for defaulted.
    /// The one definition: `p_consistency` and the outcome both read it.
    pub fn alpha(&self) -> (Vec<Rational>, bool) {
        if self.thresholds.is_empty() {
            (vec![Rational::new(5, 100).expect("1/20")], true)
        } else {
            (self.thresholds.iter().map(ReportedNumber::value).collect(), false)
        }
    }
}

/// One input to a Level 0 method.
#[derive(Debug, Clone, Copy)]
pub struct L0Input<'a> {
    pub result: &'a ReportedResult,
    pub context: &'a DocumentContext,
}

struct Patterns {
    one_tailed: Regex,
    directional: Regex,
    correction: Regex,
}

fn patterns() -> &'static Patterns {
    static P: OnceLock<Patterns> = OnceLock::new();
    P.get_or_init(|| Patterns {
        one_tailed: Regex::new(r"(?i)\bone[\s\-\u{2010}\u{2011}\u{2013}]?(?:tailed|sided)\b").unwrap(),
        // Group 1 is a negating prefix; a match with it is not a statement of
        // a one-tailed test.
        directional: Regex::new(r"(?i)(?:^|[^\p{L}])((?:non|bi)[\-\u{2010}\u{2011}\u{2013}]?)?directional\b").unwrap(),
        correction: Regex::new(
            r"(?i)\b(?:bonferroni|holm|[sš]id[aá]k|tukey|scheff[eé]|dunnett|benjamini|false[\s\-]discovery|greenhouse[\s\-\u{2010}\u{2011}\u{2013}]*geisser|huynh[\s\-\u{2010}\u{2011}\u{2013}]*feldt)\b",
        )
        .unwrap(),
    })
}

fn states_one_tailed(text: &str) -> bool {
    let p = patterns();
    p.one_tailed.is_match(text) || p.directional.captures_iter(text).any(|c| c.get(1).is_none())
}

impl DocumentContext {
    pub fn read(ex: &ExtractionResult, reading: &ResultReading) -> DocumentContext {
        // Each section's heading, then its paragraphs, in document order.
        let texts = || {
            ex.sections.iter().enumerate().filter(|(_, s)| s.kind != SectionKind::References).flat_map(|(i, s)| {
                std::iter::once((TextPosition { section: i, paragraph: None }, s.heading.as_str())).chain(
                    s.paragraphs
                        .iter()
                        .enumerate()
                        .map(move |(j, p)| (TextPosition { section: i, paragraph: Some(j) }, p.as_str())),
                )
            })
        };
        DocumentContext {
            thresholds: reading.thresholds.clone(),
            one_tailed_at: texts().find(|(_, t)| states_one_tailed(t)).map(|(at, _)| at),
            correction_at: texts().find(|(_, t)| patterns().correction.is_match(t)).map(|(at, _)| at),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_tailed_wording_is_recognised_and_its_negations_are_not() {
        for s in ["a one-tailed test", "One sided tests", "one\u{2013}tailed", "a directional hypothesis"] {
            assert!(states_one_tailed(s), "{s:?}");
        }
        for s in ["a non-directional hypothesis", "nondirectional tests", "bidirectional links", "two-tailed tests"] {
            assert!(!states_one_tailed(s), "{s:?}");
        }
    }

    #[test]
    fn the_first_mention_is_recorded_by_position_heading_or_paragraph() {
        use crate::extract::extract_from_text;
        use crate::stats_l0::result::{read_results, TableStructure};
        let mut ex = extract_from_text(
            "A Title\n\nMethods\nWe used t-tests.\n\nTests were one-tailed.\n\nResults\nScores rose, t(28) = 2.10, p = .023.\n",
        );
        let reading = read_results(&ex, TableStructure::NotAvailable);
        let ctx = DocumentContext::read(&ex, &reading);
        let methods = ex.sections.iter().position(|s| s.kind == SectionKind::Methods).expect("a Methods section");
        assert_eq!(ctx.one_tailed_at, Some(TextPosition { section: methods, paragraph: Some(1) }));
        assert_eq!(ctx.correction_at, None);
        // A heading that names a correction is found as the heading: no paragraph.
        let results = ex.sections.iter().position(|s| s.kind == SectionKind::Results).expect("a Results section");
        ex.sections[results].heading = "Post hoc tests (Tukey HSD)".into();
        let ctx = DocumentContext::read(&ex, &reading);
        assert_eq!(ctx.correction_at, Some(TextPosition { section: results, paragraph: None }));
    }

    #[test]
    fn every_named_correction_is_recognised_and_lookalikes_are_not() {
        for s in [
            "Bonferroni", "Holm", "Šidák", "Sidak", "Tukey HSD", "Scheffé", "Scheffe", "Dunnett", "Benjamini-Hochberg",
            "false discovery rate", "Greenhouse-Geisser", "Huynh–Feldt",
        ] {
            assert!(patterns().correction.is_match(s), "{s:?}");
        }
        for s in ["Holmes et al.", "a holmium laser"] {
            assert!(!patterns().correction.is_match(s), "{s:?}");
        }
    }
}
