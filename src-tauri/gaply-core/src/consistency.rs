//! Deterministic manuscript consistency checks (§11 D94).
//!
//! **NOT an AI feature.** No model, no embedder, no network — arithmetic over
//! text the parser already produced. It must work for a user who has never
//! downloaded a model, which is why it lives here and not under `ai_engine`.
//!
//! It runs BEFORE an audit is queued. The reason is one finding: a reference
//! list that is off by one silently invalidates most of the audit's
//! resolutions, so every `citation_support` verdict would be checking a claim
//! against the wrong paper. Reporting that at the END means the run was wasted
//! and the reader cannot tell which findings survived.
//!
//! # Every finding names its evidence
//!
//! The marker, the entry, the numbers. "Possible inconsistency detected" is not
//! a finding — it tells a researcher to go and re-derive what we already knew.
use std::collections::BTreeMap;

use regex::Regex;
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

use crate::ai_engine::audit_prepass::{BibEntry, MarkerStyle, PrepassReport};
use crate::extract::docparse::PagedBlock;

/// Does this finding make the audit's OUTPUT untrustworthy, or is it a defect
/// in the paper that leaves every verdict intact?
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    /// Resolution may be wrong — the user must acknowledge before auditing.
    Structural,
    /// Worth fixing, gates nothing.
    Cosmetic,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConsistencyFinding {
    /// Stable identifier, for tests and for the UI to group by.
    pub kind: String,
    pub severity: Severity,
    /// Names the evidence. Never a category.
    pub message: String,
    /// What the reader should do, when it is not obvious from the message.
    pub action: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConsistencyReport {
    pub findings: Vec<ConsistencyFinding>,
    pub structural: usize,
    pub cosmetic: usize,
}

impl ConsistencyReport {
    fn push(&mut self, kind: &'static str, severity: Severity, message: String, action: Option<String>) {
        match severity {
            Severity::Structural => self.structural += 1,
            Severity::Cosmetic => self.cosmetic += 1,
        }
        self.findings.push(ConsistencyFinding { kind: kind.to_string(), severity, message, action });
    }
    /// Does the user have to acknowledge something before auditing?
    pub fn blocks_audit(&self) -> bool {
        self.structural > 0
    }
}

/// Minimum words for a block to be considered a "paragraph" worth comparing.
/// Below this, a repeat is a heading or a table cell, not duplicated prose.
const MIN_PARAGRAPH_WORDS: usize = 25;

fn caption_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    // "Table II.", "Fig. 3:", "Figure 12 —" at the start of a block.
    RE.get_or_init(|| {
        Regex::new(r"(?i)^\s*(table|fig\.?|figure)\s+([ivxlcdm]+|\d{1,3})\b")
            .expect("caption regex")
    })
}

fn figref_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    // A reference to a figure INSIDE prose: "in Fig. 6", "see Figure 2".
    RE.get_or_init(|| Regex::new(r"(?i)\b(fig\.?|figure)\s+(\d{1,3})\b").expect("figref regex"))
}

fn section_letter_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    // "A. Dataset Acquisition", "B. Deep Learning…"
    RE.get_or_init(|| Regex::new(r"^\s*([A-H])\.\s+(\S.*)$").expect("section letter regex"))
}

/// A reference entry that does not look like a reference.
///
/// THE OFF-BY-ONE DETECTOR. A page-range continuation numbered as its own entry
/// — the defect that made `[7]` resolve to an SVM paper — has neither an author
/// nor a year, because it is the tail of the entry above it.
fn entry_is_malformed(e: &BibEntry) -> bool {
    // NO AUTHOR is the discriminator, on its own. A reference begins with who
    // wrote it; a continuation does not.
    //
    // The first rule tried was "no author AND no year", and it MISSED the real
    // defect: `[6] pp. 436-465, 2013.` carries a year, because a page range
    // ends with one. Requiring both let the very entry this check exists for
    // through. The year is no help here — entry [5] of the same paper is a
    // genuine reference whose year did not parse.
    e.lead_author.is_none()
}

/// Corroboration for the message, not for the decision: the shape that says
/// "this is the tail of the entry above".
fn looks_like_page_range(raw: &str) -> bool {
    let t = raw.trim_start().to_lowercase();
    t.starts_with("pp.") || t.starts_with("p.") || t.starts_with("vol.") || t.starts_with("no.")
}

fn snippet(s: &str, n: usize) -> String {
    let t = s.split_whitespace().collect::<Vec<_>>().join(" ");
    if t.chars().count() <= n {
        return t;
    }
    let cut: String = t.chars().take(n).collect();
    format!("{cut}…")
}

/// Run every check. Pure: same inputs, same findings, no clock and no I/O.
pub fn check_consistency(blocks: &[PagedBlock], report: &PrepassReport) -> ConsistencyReport {
    let mut out = ConsistencyReport::default();
    check_reference_list(&mut out, report);
    check_orphan_markers(&mut out, report);
    check_captions(&mut out, blocks);
    check_section_letters(&mut out, blocks);
    check_repeated_paragraphs(&mut out, blocks);
    check_mixed_styles(&mut out, report);
    out
}

/// (1) Entries that are not references, and (3) markers that resolve to one.
fn check_reference_list(out: &mut ConsistencyReport, report: &PrepassReport) {
    let bad: Vec<&BibEntry> = report.bibliography.values().filter(|e| entry_is_malformed(e)).collect();
    for e in &bad {
        let above = report.bibliography.keys().filter(|n| **n > e.number).count();
        out.push(
            "reference-entry-malformed",
            Severity::Structural,
            format!(
                "[{}] does not look like a reference entry — it names no author{}: “{}”. \
                 If it is a continuation of [{}], then the {} entries numbered above it are each \
                 shifted by one, and every marker pointing at them resolves to the wrong paper.",
                e.number,
                if looks_like_page_range(&e.raw) { ", and reads as a page range" } else { "" },
                snippet(&e.raw, 90),
                e.number.saturating_sub(1),
                above
            ),
            Some(
                "Renumber the reference list so this text joins the entry above it, then re-run. \
                 Auditing before you do will check claims against the wrong sources."
                    .to_string(),
            ),
        );
    }

    let bad_numbers: Vec<u32> = bad.iter().map(|e| e.number).collect();
    if bad_numbers.is_empty() {
        return;
    }
    // (3) Which markers actually land on one of those entries.
    let mut hits: BTreeMap<u32, Vec<String>> = BTreeMap::new();
    for p in &report.planned {
        for m in &p.markers {
            for n in &m.numbers {
                if bad_numbers.contains(n) {
                    hits.entry(*n).or_default().push(snippet(&p.sentence, 60));
                }
            }
        }
    }
    for (n, sentences) in hits {
        out.push(
            "marker-resolves-to-malformed-entry",
            Severity::Structural,
            format!(
                "{} sentence{} [{n}], which is not a usable reference entry. First: “{}”",
                sentences.len(),
                if sentences.len() == 1 { " cites" } else { "s cite" },
                sentences.first().map(String::as_str).unwrap_or("")
            ),
            None,
        );
    }
}

/// (2) `[n]` with no entry `n`.
fn check_orphan_markers(out: &mut ConsistencyReport, report: &PrepassReport) {
    if report.bibliography.is_empty() {
        return; // author-year paper, or no references heading — not an orphan
    }
    let lo = report.bibliography.keys().min().copied().unwrap_or(0);
    let hi = report.bibliography.keys().max().copied().unwrap_or(0);
    let mut orphans: BTreeMap<u32, usize> = BTreeMap::new();
    for p in &report.planned {
        for m in &p.markers {
            for n in &m.numbers {
                if !report.bibliography.contains_key(n) {
                    *orphans.entry(*n).or_default() += 1;
                }
            }
        }
    }
    for (n, count) in orphans {
        out.push(
            "orphan-marker",
            Severity::Structural,
            format!(
                "[{n}] is cited {count} time{} but the reference list has no entry {n} — it runs \
                 from [{lo}] to [{hi}].",
                if count == 1 { "" } else { "s" }
            ),
            Some("Add the missing reference, or correct the marker.".to_string()),
        );
    }
}

/// (4) duplicate table/figure numbers and (5) figures cited but never captioned.
fn check_captions(out: &mut ConsistencyReport, blocks: &[PagedBlock]) {
    let mut seen: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut figure_labels: Vec<String> = Vec::new();
    for b in blocks {
        if let Some(c) = caption_re().captures(&b.text) {
            let kind = c.get(1).map(|m| m.as_str().to_lowercase()).unwrap_or_default();
            let kind = if kind.starts_with("fig") { "Figure" } else { "Table" };
            let num = c.get(2).map(|m| m.as_str().to_uppercase()).unwrap_or_default();
            let key = format!("{kind} {num}");
            if kind == "Figure" {
                figure_labels.push(num.clone());
            }
            seen.entry(key).or_default().push(snippet(&b.text, 70));
        }
    }
    for (label, caps) in &seen {
        if caps.len() > 1 {
            out.push(
                "duplicate-caption-number",
                Severity::Cosmetic,
                format!(
                    "{label} is used {} times. First: “{}”. Also: “{}”",
                    caps.len(),
                    caps[0],
                    caps[1]
                ),
                Some("Renumber one of them — a cross-reference cannot say which you mean.".to_string()),
            );
        }
    }

    // A figure referred to in prose that no caption defines.
    let mut referenced: BTreeMap<String, usize> = BTreeMap::new();
    for b in blocks {
        if caption_re().is_match(&b.text) {
            continue; // a caption is a definition, not a reference
        }
        for c in figref_re().captures_iter(&b.text) {
            if let Some(n) = c.get(2) {
                *referenced.entry(n.as_str().to_string()).or_default() += 1;
            }
        }
    }
    for (n, count) in referenced {
        if !figure_labels.contains(&n) {
            out.push(
                "figure-referenced-but-absent",
                Severity::Cosmetic,
                format!(
                    "Figure {n} is referred to {count} time{} but no caption defines it. \
                     Captions found: {}",
                    if count == 1 { "" } else { "s" },
                    if figure_labels.is_empty() {
                        "none".to_string()
                    } else {
                        figure_labels.join(", ")
                    }
                ),
                None,
            );
        }
    }
}

/// (6) `A, A, B, C` — a lettered run that does not advance by one.
fn check_section_letters(out: &mut ConsistencyReport, blocks: &[PagedBlock]) {
    let mut run: Vec<(char, String)> = Vec::new();
    for b in blocks {
        if let Some(c) = section_letter_re().captures(&b.text) {
            let ch = c.get(1).and_then(|m| m.as_str().chars().next()).unwrap_or('?');
            let title = c.get(2).map(|m| snippet(m.as_str(), 40)).unwrap_or_default();
            run.push((ch, title));
        }
    }
    for w in run.windows(2) {
        let (prev, next) = (w[0].0, w[1].0);
        // 'A' after 'C' is a NEW section's first subsection, which is correct —
        // but 'A' after 'A' is the defect itself, not a restart, so the
        // allowance requires the previous letter to be a LATER one.
        if next as u8 == prev as u8 + 1 || (next == 'A' && prev != 'A') {
            continue;
        }
        out.push(
            "section-letters-out-of-sequence",
            Severity::Cosmetic,
            format!(
                "Subsection “{}. {}” follows “{}. {}” — the letters do not advance by one.",
                next, w[1].1, prev, w[0].1
            ),
            None,
        );
    }
}

/// (7) The same paragraph, printed twice.
fn check_repeated_paragraphs(out: &mut ConsistencyReport, blocks: &[PagedBlock]) {
    let mut seen: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    for (i, b) in blocks.iter().enumerate() {
        let norm = b.text.split_whitespace().collect::<Vec<_>>().join(" ").to_lowercase();
        if norm.split_whitespace().count() < MIN_PARAGRAPH_WORDS {
            continue;
        }
        seen.entry(norm).or_default().push(i);
    }
    for (norm, at) in seen {
        if at.len() > 1 {
            out.push(
                "repeated-paragraph",
                Severity::Cosmetic,
                format!(
                    "A paragraph appears {} times, at blocks {}: “{}”",
                    at.len(),
                    at.iter().map(|i| i.to_string()).collect::<Vec<_>>().join(" and "),
                    snippet(&norm, 80)
                ),
                Some("Delete the duplicate, or rewrite one of them.".to_string()),
            );
        }
    }
}

/// (8) One section citing `(Smith, 2019)` in a `[n]` paper.
fn check_mixed_styles(out: &mut ConsistencyReport, report: &PrepassReport) {
    let mut numeric = 0usize;
    let mut author_year = 0usize;
    // Per section, so the finding can name where.
    let mut by_section: BTreeMap<String, (usize, usize)> = BTreeMap::new();
    for p in &report.planned {
        let section = p.section.clone().unwrap_or_else(|| "(no section)".to_string());
        let e = by_section.entry(section).or_default();
        for m in &p.markers {
            match m.style {
                MarkerStyle::Numeric => {
                    numeric += 1;
                    e.0 += 1;
                }
                MarkerStyle::AuthorYear => {
                    author_year += 1;
                    e.1 += 1;
                }
            }
        }
    }
    if numeric == 0 || author_year == 0 {
        return; // one style throughout — nothing mixed
    }
    let document_is_numeric = numeric >= author_year;
    let (doc_style, odd_style) = if document_is_numeric {
        ("numeric [n]", "author-year")
    } else {
        ("author-year", "numeric [n]")
    };
    for (section, (n, a)) in by_section {
        let (odd, same) = if document_is_numeric { (a, n) } else { (n, a) };
        if odd > 0 && odd > same {
            out.push(
                "mixed-citation-style",
                Severity::Cosmetic,
                format!(
                    "“{section}” uses {odd_style} citations ({odd} of them) while the rest of the \
                     paper uses {doc_style}."
                ),
                Some("Convert them to the document's style so every marker resolves the same way.".to_string()),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai_engine::audit_prepass::prepass_blocks;

    fn blocks(texts: &[&str]) -> Vec<PagedBlock> {
        texts
            .iter()
            .map(|t| PagedBlock { page: Some(1), style: None, text: (*t).to_string() })
            .collect()
    }
    fn run(texts: &[&str]) -> ConsistencyReport {
        let b = blocks(texts);
        let r = prepass_blocks(&b);
        check_consistency(&b, &r)
    }
    fn kinds(r: &ConsistencyReport) -> Vec<String> {
        r.findings.iter().map(|f| f.kind.clone()).collect()
    }
    fn message(r: &ConsistencyReport, kind: &str) -> String {
        r.findings.iter().find(|f| f.kind == kind).map(|f| f.message.clone()).unwrap_or_default()
    }

    /// THE ONE THIS EXISTS FOR (§11 D94). Verbatim from `R PAPER .pdf`: entry
    /// [6] is a page-range continuation numbered as its own reference, so every
    /// marker above it resolves one paper too far.
    #[test]
    fn a_page_range_numbered_as_a_reference_is_structural() {
        let r = run(&[
            "Classical lexical-based approaches [5] suffer from a lack of reasoning about words.",
            "The BiLSTM architecture [7] handles long-range dependencies in social media text.",
            "References",
            "[5] S. Mohammad and P. Turney, \"Crowdsourcing a word-emotion association lexicon,\" 2013.",
            "[6] pp. 436-465, 2013.",
            "[7] C. Cortes and V. Vapnik, \"Support-vector networks,\" Mach. Learn., vol. 20, 1995.",
        ]);
        assert!(r.blocks_audit(), "an off-by-one reference list must gate the audit");
        assert!(kinds(&r).iter().any(|k| k == "reference-entry-malformed"), "{:?}", kinds(&r));

        let m = message(&r, "reference-entry-malformed");
        // NAMES THE EVIDENCE: the ordinal, the text, and the consequence.
        assert!(m.contains("[6]"), "{m}");
        assert!(m.contains("pp. 436-465"), "{m}");
        assert!(m.contains("shifted by one"), "{m}");
        assert!(m.contains("resolves to the wrong paper"), "{m}");
    }

    /// The rule that was tried FIRST and missed it: "no author AND no year".
    /// A page range ends with a year, so that let the defect through — while a
    /// genuine entry whose year fails to parse must NOT be flagged.
    #[test]
    fn absence_of_an_author_decides_it_not_absence_of_a_year() {
        let with_year = BibEntry {
            number: 6,
            raw: "pp. 436-465, 2013.".into(),
            doi: None,
            lead_author: None,
            year: Some(2013),
        };
        assert!(entry_is_malformed(&with_year), "a page range with a year must still be caught");

        let no_year = BibEntry {
            number: 5,
            raw: "S. Mohammad and P. Turney, \"Crowdsourcing a lexicon\".".into(),
            doi: None,
            lead_author: Some("mohammad".into()),
            year: None,
        };
        assert!(!entry_is_malformed(&no_year), "a real reference whose year did not parse is fine");
    }

    #[test]
    fn a_marker_landing_on_a_malformed_entry_is_named() {
        let r = run(&[
            "Emotion detection in social media text relies on lexical resources [6] for polarity.",
            "References",
            "[5] S. Mohammad, \"Crowdsourcing a word-emotion lexicon,\" 2013.",
            "[6] pp. 436-465, 2013.",
        ]);
        let m = message(&r, "marker-resolves-to-malformed-entry");
        assert!(m.contains("[6]"), "{m}");
        assert!(m.contains("cites"), "singular verb: {m}");
    }

    #[test]
    fn an_orphan_marker_names_the_range_the_list_covers() {
        let r = run(&[
            "The proposed approach outperforms every published baseline on this corpus [42].",
            "References",
            "[1] T. Cover and P. Hart, \"Nearest neighbor pattern classification,\" 1967.",
            "[2] D. Whitley, \"A genetic algorithm tutorial,\" Stat. Comput., 1994.",
        ]);
        assert!(r.blocks_audit());
        let m = message(&r, "orphan-marker");
        assert!(m.contains("[42]"), "{m}");
        assert!(m.contains("[1]") && m.contains("[2]"), "must name the real range: {m}");
    }

    #[test]
    fn a_paper_with_no_numbered_list_produces_no_orphans() {
        // Author-year paper: every numeric check must stay silent rather than
        // reporting every marker as an orphan.
        let r = run(&[
            "Emotion detection has been studied widely (Smith, 2019) across many corpora.",
            "The approach follows Tang et al. (2016) in its treatment of aspect terms.",
        ]);
        assert!(!kinds(&r).iter().any(|x| x == "orphan-marker"), "{:?}", kinds(&r));
        assert!(!r.blocks_audit());
    }

    #[test]
    fn duplicate_table_numbers_name_both_captions() {
        let r = run(&[
            "TABLE II. HEFCSO HYPERPARAMETER CONFIGURATION Parameter Value Tuned By Range",
            "TABLE II. COMPARATIVE PERFORMANCE ON SEMEVAL-2018 Method Accuracy F1 MCC",
        ]);
        let m = message(&r, "duplicate-caption-number");
        assert!(m.contains("Table II"), "{m}");
        assert!(m.contains("HYPERPARAMETER"), "{m}");
        assert!(m.contains("COMPARATIVE"), "both captions must be named: {m}");
        // Cosmetic: a duplicate number invalidates no verdict.
        assert!(!r.blocks_audit());
    }

    #[test]
    fn a_figure_cited_but_never_captioned_lists_the_ones_that_exist() {
        let r = run(&[
            "The convergence behaviour of the optimiser is shown in Fig. 6 across all datasets.",
            "Fig. 1. Proposed HEFCSO-BiLSTM System Architecture",
            "Fig. 3. Accuracy and F1-Score Comparison on the benchmark",
        ]);
        let m = message(&r, "figure-referenced-but-absent");
        assert!(m.contains("Figure 6"), "{m}");
        assert!(m.contains("1, 3"), "must list the captions that DO exist: {m}");
    }

    #[test]
    fn a_captioned_figure_is_not_reported_as_absent() {
        let r = run(&[
            "The architecture is depicted in Fig. 1 and described in the following section.",
            "Fig. 1. Proposed HEFCSO-BiLSTM System Architecture",
        ]);
        assert!(!kinds(&r).iter().any(|x| x == "figure-referenced-but-absent"), "{:?}", kinds(&r));
    }

    #[test]
    fn section_letters_that_do_not_advance_are_reported() {
        let r = run(&[
            "A. Dataset Acquisition Three corpora from publicly available benchmarks were employed.",
            "A. Preprocessing Pipeline Emoji were replaced with their textual descriptions first.",
            "B. GloVe Embeddings Each token is mapped to a 200-dimensional pretrained vector.",
        ]);
        let m = message(&r, "section-letters-out-of-sequence");
        assert!(m.contains("A."), "{m}");
        assert!(m.contains("do not advance"), "{m}");
    }

    #[test]
    fn a_new_sections_first_subsection_is_not_out_of_sequence() {
        // C then A is a NEW section starting, which is correct.
        let r = run(&[
            "A. Dataset Acquisition Three corpora from public benchmarks were employed here.",
            "B. GloVe Embeddings Each token maps to a 200-dimensional pretrained vector space.",
            "C. Proposed Algorithm The optimiser treats each configuration as a vector.",
            "A. Ablation Results Removing the attention layer costs two accuracy points.",
        ]);
        assert!(!kinds(&r).iter().any(|x| x == "section-letters-out-of-sequence"), "{:?}", kinds(&r));
    }

    #[test]
    fn a_paragraph_printed_twice_is_reported_with_both_locations() {
        let para = "A major drawback of this framework is that it has been restricted to single label \
                    problems and does not yet handle the multi label case that real corpora present.";
        let r = run(&[para, "An unrelated paragraph that says something else entirely about the data.", para]);
        let m = message(&r, "repeated-paragraph");
        assert!(m.contains("2 times"), "{m}");
        assert!(m.contains("0 and 2"), "must name where: {m}");
    }

    #[test]
    fn a_short_repeated_line_is_not_a_repeated_paragraph() {
        // Headings and table cells repeat legitimately.
        let r = run(&["Results", "Results", "Discussion"]);
        assert!(!kinds(&r).iter().any(|x| x == "repeated-paragraph"), "{:?}", kinds(&r));
    }

    #[test]
    fn a_section_citing_in_the_minority_style_is_reported() {
        let r = run(&[
            "II. RELATED WORK",
            "B. Deep Learning Architectures The TD-LSTM was introduced by Tang et al. (2016) to model aspects.",
            "The ATAE-LSTM model was proposed by Wang et al. (2016) to incorporate aspect information.",
            "III. METHODOLOGY",
            "Recurrent network parameters have been tuned by PSO [19] and Genetic Algorithms [8].",
            "Classical lexical approaches [5] suffer from a lack of reasoning about vocabulary.",
            "The BiLSTM architecture [7] handles the long-range dependencies of informal text.",
        ]);
        let m = message(&r, "mixed-citation-style");
        assert!(m.contains("author-year"), "{m}");
        assert!(m.contains("numeric"), "{m}");
        assert!(!r.blocks_audit(), "a style mismatch invalidates no verdict");
    }

    #[test]
    fn one_style_throughout_is_never_reported_as_mixed() {
        let r = run(&[
            "Classical lexical approaches [5] suffer from a lack of reasoning about vocabulary.",
            "The BiLSTM architecture [7] handles the long-range dependencies of informal text.",
        ]);
        assert!(!kinds(&r).iter().any(|x| x == "mixed-citation-style"), "{:?}", kinds(&r));
    }

    /// A clean manuscript must produce NOTHING. A check that fires on everything
    /// is noise a researcher learns to skip.
    #[test]
    fn a_consistent_manuscript_produces_no_findings() {
        let r = run(&[
            "Emotion detection in social media text is a long-standing problem in the field [1].",
            "The BiLSTM architecture [2] handles the long-range dependencies of informal text.",
            "Fig. 1. Proposed system architecture",
            "The architecture is depicted in Fig. 1 and described in the following section.",
            "References",
            "[1] M. De Choudhury, S. Counts, and E. Horvitz, \"Mining social media,\" 2013.",
            "[2] M. Schuster and K. Paliwal, \"Bidirectional recurrent neural networks,\" 1997.",
        ]);
        assert_eq!(r.findings.len(), 0, "clean manuscript flagged: {:?}", r.findings);
        assert!(!r.blocks_audit());
    }

    /// Severity is the gate, and only resolution-invalidating findings hold it.
    #[test]
    fn only_structural_findings_gate_the_audit() {
        for kind in ["reference-entry-malformed", "orphan-marker", "marker-resolves-to-malformed-entry"] {
            let f = ConsistencyFinding {
                kind: kind.to_string(),
                severity: Severity::Structural,
                message: String::new(),
                action: None,
            };
            assert_eq!(f.severity, Severity::Structural, "{kind}");
        }
        let mut r = ConsistencyReport::default();
        r.push("duplicate-caption-number", Severity::Cosmetic, String::new(), None);
        assert!(!r.blocks_audit(), "a cosmetic finding must not gate");
        r.push("orphan-marker", Severity::Structural, String::new(), None);
        assert!(r.blocks_audit());
    }
}
