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
use super::negation::{negation_of, Negation};
use super::number::ReportedNumber;
use super::result::{ReportedResult, ResultReading};
use crate::equation::rational::Rational;
use crate::extract::{ExtractionResult, SectionKind};

/// The document-wide facts a Level 0 method may use.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentContext {
    /// The distinct stated significance thresholds, as [`ResultReading`] holds them.
    pub thresholds: Vec<ReportedNumber>,
    /// Where explicit one-tailed wording first appears, if it does:
    /// "one-tailed", "one tailed", "1-tailed", "one-sided", "one sided", in
    /// any case, with a hyphen, a space or a Unicode hyphen. The word
    /// "directional" on its own never counts: it means something else in
    /// ecology, neuroscience and modelling, and the evaluation found it in
    /// five of fifteen corpus papers, none of which stated a one-tailed test.
    pub one_tailed_at: Option<TextPosition>,
    /// Where a correction for MULTIPLE TESTING is first named, if one is.
    /// Sphericity and variance corrections (Greenhouse-Geisser, Huynh-Feldt,
    /// Welch, Satterthwaite, Yates, Brown-Forsythe) adjust df or the
    /// statistic, never the p for multiplicity, and are not on the list.
    pub correction_at: Option<TextPosition>,
}

impl DocumentContext {
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

    /// The same thresholds as [`Self::alpha`] returns, as the paper printed
    /// them (`.05`, `0.01`, `.05/6`), one per value and in the same order;
    /// `.05` when none is stated. For the card, which shows a fraction as the
    /// paper wrote it beside its decimal.
    pub fn alpha_printed(&self) -> Vec<String> {
        if self.thresholds.is_empty() {
            vec![".05".to_string()]
        } else {
            self.thresholds.iter().map(|t| t.raw().to_string()).collect()
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
    correction: Regex,
}

fn patterns() -> &'static Patterns {
    static P: OnceLock<Patterns> = OnceLock::new();
    P.get_or_init(|| Patterns {
        // Explicit wording only. `1-tailed` is the digit form some journals
        // print; `\b` keeps "someone sided" and "one-tailored" out.
        one_tailed: Regex::new(r"(?i)\b(?:one|1)[\s\-\u{2010}\u{2011}\u{2013}]?(?:tailed|sided)\b").unwrap(),
        // The closed list of corrections for MULTIPLE TESTING (evaluation
        // fixes, part A). Terms: FDR, false discovery rate, corrected or
        // adjusted for multiple comparisons or testing, a multiple-comparison
        // correction, familywise error, adjusted p, p-adj, padj, q values,
        // Bonferroni. A surname counts only beside a qualifier (test,
        // correction, corrected, adjusted, procedure, method, step-up,
        // step-down, HSD, post hoc), joined by a space or a hyphen, or in the
        // pairs Benjamini-Hochberg, Benjamini and Hochberg,
        // Benjamini-Yekutieli and Holm-Bonferroni: a surname alone, as in a
        // citation, never counts. Greenhouse-Geisser, Huynh-Feldt, Welch,
        // Satterthwaite, Yates, continuity, Brown-Forsythe and bias-corrected
        // are not on it.
        correction: Regex::new(concat!(
            r"(?i)",
            r"\bfdr\b|\bfalse[\s\-\u{2010}\u{2011}]discovery[\s\-\u{2010}\u{2011}]rates?\b",
            r"|\b(?:corrected|adjusted)\s+for\s+multiple\s+(?:comparisons?|testing|tests)\b",
            r"|\bmultiple[\s\-\u{2010}\u{2011}]comparisons?[\s\-\u{2010}\u{2011}]corrections?\b",
            r"|\bfamily[\s\-\u{2010}\u{2011}]?wise\s+error\b",
            r"|\badjusted\s+p\b|\bp[\-_]adj\b|\bpadj\b|\bq[\s\-\u{2010}\u{2011}]values?\b",
            r"|\bbonferroni\b",
            r"|\bbenjamini(?:[\s\-\u{2010}\u{2011}\u{2013}\u{2014}]+|\s+and\s+)hochberg\b",
            r"|\bbenjamini[\s\-\u{2010}\u{2011}\u{2013}\u{2014}]+yekutieli\b",
            r"|\b(?:hochberg|hommel|holm|[sš]id[aá]k|tukey|scheff[eé]|games[\s\-\u{2010}\u{2011}\u{2013}\u{2014}]+howell|dunnett|dunn)",
            r"(?:['\u{2019}]s)?[\s\-\u{2010}\u{2011}]+",
            r"(?:tests?|corrections?|corrected|adjusted|procedures?|methods?|step[\s\-\u{2010}\u{2011}]?(?:up|down)|hsd|post[\s\-\u{2010}\u{2011}]?hoc)\b",
            r"|\bpost[\s\-\u{2010}\u{2011}]?hoc\s+(?:hochberg|hommel|holm|[sš]id[aá]k|tukey|scheff[eé]|games[\s\-\u{2010}\u{2011}\u{2013}\u{2014}]+howell|dunnett|dunn)\b",
        ))
        .unwrap(),
    })
}

/// Whether `text` states a one-tailed test: some match the shared negation
/// rule finds not negated. Clearly negated and unclear mentions are not a
/// statement, so a halved p stays `Concern(OneTailedUnstated)`.
pub(crate) fn states_one_tailed(text: &str) -> bool {
    patterns()
        .one_tailed
        .find_iter(text)
        .any(|m| !describes_an_interval(&text[m.end()..]) && negation_of(text, m.start(), m.end()) == Negation::Not)
}

/// "one-sided" or "one-tailed" followed within three words by an interval
/// word describes an interval, not a test (review follow-up, item 2; the
/// evaluation's B09).
fn describes_an_interval(rest: &str) -> bool {
    rest.split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .take(3)
        .any(|w| matches!(w.to_lowercase().as_str(), "interval" | "intervals" | "ci" | "cis" | "bound" | "bounds" | "confidence"))
}

/// Whether `text` names a correction for multiple testing: some match that
/// is not CLEARLY negated. An unclear negation keeps the p values adjusted,
/// so `CouldNotCheck(AdjustedP)` stays; only "not corrected for multiple
/// comparisons" and the like make them raw.
pub(crate) fn names_correction(text: &str) -> bool {
    patterns().correction.find_iter(text).any(|m| negation_of(text, m.start(), m.end()) != Negation::Clearly)
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
            correction_at: texts().find(|(_, t)| names_correction(t)).map(|(at, _)| at),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_tailed_wording_is_recognised_only_when_explicit() {
        for s in [
            "a one-tailed test",
            "One sided tests",
            "one\u{2013}tailed",
            "one\u{2010}sided",
            "1-tailed",
            "ONE TAILED",
            "one-tailed tests were used",
        ] {
            assert!(states_one_tailed(s), "{s:?}");
        }
        // "directional" on its own, in any phrase, is not a statement of a
        // one-tailed test: the evaluation found it in five of fifteen corpus
        // papers ("directional coupling", "bi- directional arrow", "directional
        // trend") and none stated one.
        for s in [
            "a directional hypothesis",
            "the directional coupling of brain regions",
            "the bi-directional arrow",
            "the bi- directional arrow",
            "a directional trend toward higher amplitudes",
            "a non-directional hypothesis",
            "nondirectional tests",
            "bidirectional links",
            "two-tailed tests",
            "one-tailored",
            "someone sided with",
        ] {
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
            "Bonferroni", "Holm procedure", "Šidák correction", "Sidak-adjusted", "Tukey HSD", "Scheffé test", "Scheffe post hoc",
            "Dunnett's test", "Benjamini-Hochberg", "false discovery rate",
        ] {
            assert!(patterns().correction.is_match(s), "{s:?}");
        }
        // A surname alone, as in a citation, never counts (evaluation fixes,
        // part A); "Holm", "Šidák", "Scheffé" and "Dunnett" alone did before.
        for s in ["Holm", "Šidák", "Scheffé", "Dunnett", "(Benjamini & Hochberg, 1995)"] {
            assert!(!patterns().correction.is_match(s), "{s:?}");
        }
        // Sphericity and variance corrections adjust df or the statistic, not
        // the p for multiple testing; none of them makes a larger p an
        // adjusted p.
        for s in [
            "Holmes et al.", "a holmium laser", "Greenhouse-Geisser", "Huynh–Feldt", "Welch", "Satterthwaite", "Yates",
            "continuity correction", "Brown-Forsythe",
        ] {
            assert!(!patterns().correction.is_match(s), "{s:?}");
        }
    }
}
