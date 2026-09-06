//! Which page actually prints a sentence (§11 D95).
//!
//! The audit's page numbers came from a reflowed block, and a block that spans
//! a page break keeps one page for all of it — so 8% of locators were off by
//! one. The per-page text is still available; this puts a sentence back on the
//! page that prints it.
//!
//! # It refuses rather than guesses
//!
//! A wrong page costs a reader a search and teaches them the locators cannot be
//! trusted. Every path here either identifies the page exactly or returns
//! `None`, and the caller falls back to the stored value LABELLED AS
//! APPROXIMATE — never silently.
use std::collections::HashMap;

/// Sentences shorter than this cannot identify a page.
const MIN_LOCATE_CHARS: usize = 12;

/// Collapse the differences between the extractor's sentence text and the same
/// text taken straight off a page. Whitespace and typography only — folding
/// case or words would start matching sentences that merely resemble one
/// another, which is the failure this exists to prevent.
fn fold(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut prev_space = true;
    for ch in s.chars() {
        let ch = match ch {
            '\u{00AD}' => continue, // soft hyphen
            '\u{2010}'..='\u{2015}' => '-',
            '\u{2018}' | '\u{2019}' => '\'',
            '\u{201C}' | '\u{201D}' => '"',
            c => c,
        };
        if ch.is_whitespace() {
            if prev_space {
                continue;
            }
            prev_space = true;
            out.push(' ');
            continue;
        }
        prev_space = false;
        out.push(ch);
    }
    out.trim().to_string()
}

/// Resolves sentences to pages, in DOCUMENT ORDER.
///
/// Order is load-bearing: a manuscript that prints the same sentence twice —
/// R PAPER repeats three between its Discussion and Conclusion — is resolved by
/// giving the k-th planned copy the k-th printed occurrence. Exact, and only
/// possible because both sides are ordered.
pub struct PageLocator {
    pages: Vec<String>,
    used: HashMap<String, usize>,
}

impl PageLocator {
    pub fn new(page_texts: &[String]) -> Self {
        Self { pages: page_texts.iter().map(|p| fold(p)).collect(), used: HashMap::new() }
    }

    /// Is there any page text at all? A `.docx` has none.
    pub fn is_empty(&self) -> bool {
        self.pages.is_empty()
    }

    /// The 1-based page that prints this sentence, or `None` when it cannot be
    /// identified — which the caller must treat as "no derived page", never as
    /// "page 1".
    pub fn locate(&mut self, sentence: &str) -> Option<u32> {
        let needle = fold(sentence);
        if needle.chars().count() < MIN_LOCATE_CHARS || self.pages.is_empty() {
            return None;
        }
        let mut hits: Vec<(usize, usize)> = Vec::new();
        for (i, page) in self.pages.iter().enumerate() {
            let mut at = 0usize;
            while let Some(found) = page[at..].find(&needle) {
                let abs = at + found;
                hits.push((i, abs));
                at = abs + 1;
                if at >= page.len() {
                    break;
                }
            }
        }
        if hits.is_empty() {
            return None;
        }
        let k = *self.used.get(&needle).unwrap_or(&0);
        if k >= hits.len() {
            // More planned copies than printed occurrences: refuse rather than
            // reuse one, which would put two findings in the same place.
            return None;
        }
        self.used.insert(needle, k + 1);
        hits.sort_unstable();
        u32::try_from(hits[k].0 + 1).ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pages() -> Vec<String> {
        vec![
            "I. INTRODUCTION The meteoric rise of user generated content has made social media an instantaneous gauge.".to_string(),
            "III. METHODOLOGY In the course of this study three corpora from publicly available benchmarks were employed.".to_string(),
            "The highest F1-scores are for joy and trust. Convergence is reached almost 28% faster.".to_string(),
        ]
    }

    /// THE BUG (§11 D95): a reflowed block put this on page 2; it prints on 3.
    #[test]
    fn a_sentence_resolves_to_the_page_that_prints_it() {
        let mut l = PageLocator::new(&pages());
        assert_eq!(l.locate("Convergence is reached almost 28% faster."), Some(3));
        assert_eq!(l.locate("In the course of this study three corpora"), Some(2));
    }

    #[test]
    fn extraction_whitespace_and_typography_do_not_prevent_a_match() {
        let mut l = PageLocator::new(&pages());
        // The doubled spaces PDF extraction leaves behind.
        assert_eq!(l.locate("Convergence  is   reached  almost 28% faster."), Some(3));
    }

    /// Both lists are in document order, so copy k takes occurrence k.
    #[test]
    fn repeated_sentences_take_successive_occurrences() {
        let repeated = "A major drawback of this framework is that it is restricted.";
        let pages = vec![
            format!("Discussion. {repeated} And then some other text follows here."),
            format!("Conclusion. {repeated} And then some closing text follows."),
        ];
        let mut l = PageLocator::new(&pages);
        assert_eq!(l.locate(repeated), Some(1));
        assert_eq!(l.locate(repeated), Some(2));
        // A third copy has nowhere to go, and must NOT reuse page 2.
        assert_eq!(l.locate(repeated), None);
    }

    /// OMIT RATHER THAN APPROXIMATE — the same rule as the annotated view.
    #[test]
    fn a_sentence_that_prints_nowhere_gets_no_page() {
        let mut l = PageLocator::new(&pages());
        assert_eq!(l.locate("This sentence appears in no page of this document at all."), None);
    }

    #[test]
    fn a_sentence_too_short_to_identify_a_page_gets_none() {
        let mut l = PageLocator::new(&pages());
        assert_eq!(l.locate("Fig. 3."), None);
    }

    #[test]
    fn a_document_with_no_pages_locates_nothing() {
        let mut l = PageLocator::new(&[]);
        assert!(l.is_empty());
        assert_eq!(l.locate("Any sentence at all, of a reasonable length."), None);
    }
}
