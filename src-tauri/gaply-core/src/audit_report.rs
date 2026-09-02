//! Composes a Thesis Health report into the shared [`Block`] vocabulary.
//!
//! Same three-layer contract as `report_compose`: this layer owns section
//! ordering, grouping and every user-facing string; it owns no facts and knows
//! nothing about fonts, pages or HTML. The renderers (`report_pdf::render_pdf`,
//! `report_html::render_html`) turn the same blocks into two formats, which is
//! exactly the cheapness `report_compose`'s docs predicted from a flat block
//! set.
//!
//! # D18 IS ENFORCED HERE, STRUCTURALLY
//!
//! §11 D18 records that the grounding guarantee covers IDENTIFIERS, not prose:
//! `chunk_id` and `page` are validated against the store, while `explanation`
//! and `why` are free text that nothing ties to the evidence. The smoke seed
//! that made this concrete had a model assert the exact OPPOSITE of its
//! evidence, in fluent prose, and it was rejected only incidentally.
//!
//! So in this report a model's prose is never printed on its own. Every
//! explanation is emitted immediately beneath the quoted evidence and page it
//! is supposed to rest on, so a reader can check it against the source in one
//! glance — and when an item has NO evidence, the prose is not printed at all
//! and the item says why instead. That is [`emit_finding`]'s only real job, and
//! `d18_prose_never_appears_without_its_evidence` is the test that holds it.

use serde::{Deserialize, Serialize};

use crate::report_compose::Block;

/// One passage the model rested a verdict on. `page` comes from the store, not
/// from the model — D18's validated half.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReportEvidence {
    pub chunk_id: String,
    pub page: Option<u32>,
    /// The passage itself, as stored. This is what makes the prose checkable.
    pub quote: String,
}

/// A judged item: one manuscript sentence and what the audit concluded.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReportItem {
    pub seq: i64,
    pub page: Option<u32>,
    pub sentence: String,
    /// `strong` | `partial` | … for support; `needs_citation` / `no_citation_needed`
    /// for need; absent when the item was never judged.
    pub verdict: Option<String>,
    /// The model's prose. NEVER rendered unless `evidence` is non-empty.
    pub explanation: Option<String>,
    /// Deterministic, non-model text — the reason an item could not be checked.
    /// Printed unconditionally: it is not a model's assertion about a source.
    pub reason: Option<String>,
    pub evidence: Vec<ReportEvidence>,
    /// For unverifiable items: the parsed reference entry, when there is one.
    pub reference_entry: Option<String>,
    /// What the reader can do about it. Composed by the caller, printed here.
    pub next_step: Option<String>,
}

/// Everything the report states. Assembled by the caller from the job rows.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuditReportModel {
    pub manuscript_name: String,
    /// Rendered date string. Passed in rather than read from a clock, so a
    /// report is reproducible and the tests are not time-dependent.
    pub generated_on: String,
    pub model_id: String,
    pub prompt_version: String,
    pub total_sentences: usize,
    pub checked: usize,
    /// `kind` → count, e.g. `citation_need` → 65.
    pub counts_by_category: Vec<(String, usize)>,
    /// What the model actually SAID, e.g. `needs_citation` → 52.
    pub verdict_counts: Vec<(String, usize)>,
    /// Why items could not be judged, and how many.
    pub skipped_reasons: Vec<(String, usize)>,
    pub supported: Vec<ReportItem>,
    pub needs_citation: Vec<ReportItem>,
    pub unverifiable: Vec<ReportItem>,
    /// Items whose model answer failed validation twice.
    pub failed: Vec<ReportItem>,
}

fn para(text: impl Into<String>) -> Block {
    Block::Paragraph { text: text.into() }
}
fn bullet(text: impl Into<String>, indent: u8) -> Block {
    Block::Bullet { text: text.into(), indent }
}
fn heading(text: impl Into<String>, level: u8) -> Block {
    Block::Heading { text: text.into(), level }
}

/// The page label for a passage. `None` is stated, never guessed — a fabricated
/// page number is worse than an absent one, because it looks checkable.
fn page_label(page: Option<u32>) -> String {
    match page {
        Some(p) => format!("p.{p}"),
        None => "page unknown".to_string(),
    }
}

/// Emit ONE judged item, D18-safe.
///
/// The ordering is the guarantee: sentence, then verdict, then — only if there
/// is evidence — each quote with its page, and only then the model's prose. A
/// reader meets the source before the claim about it.
fn emit_finding(out: &mut Vec<Block>, item: &ReportItem) {
    out.push(heading(format!("Sentence {} · {}", item.seq, page_label(item.page)), 3));
    out.push(para(format!("“{}”", item.sentence.trim())));

    if let Some(v) = &item.verdict {
        out.push(para(format!("Verdict: {}", v.replace('_', " "))));
    }
    // Deterministic reasons are not model prose and carry no D18 risk.
    if let Some(r) = &item.reason {
        out.push(para(format!("Reason: {r}")));
    }
    if let Some(entry) = &item.reference_entry {
        out.push(bullet(format!("Reference list entry: {entry}"), 0));
    }

    if item.evidence.is_empty() {
        // THE D18 RULE. With nothing to check the prose against, the prose is
        // not printed — an unverifiable assertion in a PDF is indistinguishable
        // from a verified one, and the reader has no way to tell.
        if item.explanation.is_some() {
            out.push(Block::Note {
                text: "The model's explanation is withheld: no source passage was recorded for \
                       this item, so there is nothing to check it against."
                    .to_string(),
            });
        }
    } else {
        out.push(para("Evidence from the cited source:"));
        for e in &item.evidence {
            out.push(bullet(format!("{} · {} — “{}”", e.chunk_id, page_label(e.page), e.quote.trim()), 0));
        }
        if let Some(x) = &item.explanation {
            out.push(para(format!("The model's reading of that evidence: {}", x.trim())));
        }
    }

    if let Some(step) = &item.next_step {
        out.push(bullet(format!("What to do: {step}"), 0));
    }
}

fn emit_section(out: &mut Vec<Block>, title: &str, blurb: &str, items: &[ReportItem]) {
    out.push(heading(title, 1));
    if items.is_empty() {
        out.push(para(format!("{blurb} None found.")));
        return;
    }
    out.push(para(blurb));
    for item in items {
        emit_finding(out, item);
    }
}

/// Compose the whole report.
pub fn compose_audit(m: &AuditReportModel) -> Vec<Block> {
    let mut out = Vec::new();

    out.push(Block::Cover {
        title: "Thesis citation audit".to_string(),
        subtitle: m.manuscript_name.clone(),
        meta: vec![
            ("Generated".to_string(), m.generated_on.clone()),
            ("Judged by".to_string(), m.model_id.clone()),
            ("Prompt version".to_string(), m.prompt_version.clone()),
            ("Sentences read".to_string(), m.total_sentences.to_string()),
            ("Sentences checked".to_string(), m.checked.to_string()),
        ],
    });

    // The provenance line is not decoration. A reader months from now needs to
    // know which model produced these verdicts and that it ran locally.
    out.push(Block::Note {
        text: format!(
            "Every verdict in this report was produced on this machine by {} ({}). \
             No manuscript text was sent anywhere.",
            m.model_id, m.prompt_version
        ),
    });

    out.push(heading("Summary", 1));
    out.push(para(format!(
        "{} sentences were read and {} were checked against a source or judged for whether they \
         need one.",
        m.total_sentences, m.checked
    )));
    for (kind, n) in &m.counts_by_category {
        out.push(bullet(format!("{n} {}", kind.replace('_', " ")), 0));
    }
    if !m.verdict_counts.is_empty() {
        out.push(para("What the model concluded:"));
        for (v, n) in &m.verdict_counts {
            out.push(bullet(format!("{n} {}", v.replace('_', " ")), 0));
        }
    }
    if !m.skipped_reasons.is_empty() {
        out.push(para("Not judged, and why:"));
        for (r, n) in &m.skipped_reasons {
            out.push(bullet(format!("{n} — {r}"), 0));
        }
    }

    out.push(Block::PageBreak);
    emit_section(
        &mut out,
        "Claims checked against their source",
        "Each sentence below cites a source Gaply could read. The quoted passage is what the \
         verdict rests on — read it before the verdict.",
        &m.supported,
    );

    out.push(Block::PageBreak);
    emit_section(
        &mut out,
        "Sentences that may need a citation",
        "These sentences carry no citation. Whether they need one is a judgement, not a fact, and \
         it is the model's.",
        &m.needs_citation,
    );

    out.push(Block::PageBreak);
    emit_section(
        &mut out,
        "Cited, but not checkable",
        "These sentences DO cite something. Gaply could not check them because the cited work is \
         not available to it — which is a gap in the library, not a defect in the writing.",
        &m.unverifiable,
    );

    if !m.failed.is_empty() {
        out.push(Block::PageBreak);
        emit_section(
            &mut out,
            "Not judged",
            "The model's answer for these did not pass Gaply's own checks, twice. They are \
             reported rather than hidden: an unanswered sentence is not a clean one.",
            &m.failed,
        );
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text_of(b: &Block) -> String {
        match b {
            Block::Cover { title, subtitle, meta } => format!(
                "{title} {subtitle} {}",
                meta.iter().map(|(k, v)| format!("{k}={v}")).collect::<Vec<_>>().join(" ")
            ),
            Block::Heading { text, .. } => text.clone(),
            Block::Paragraph { text } => text.clone(),
            Block::Bullet { text, .. } => text.clone(),
            Block::Note { text } => text.clone(),
            Block::PageBreak => String::new(),
        }
    }
    fn all_text(bs: &[Block]) -> String {
        bs.iter().map(text_of).collect::<Vec<_>>().join("\n")
    }

    fn item_with_evidence() -> ReportItem {
        ReportItem {
            seq: 3,
            page: Some(2),
            sentence: "Doctors were the largest affected category, at 37.5 per cent.".into(),
            verdict: Some("strong".into()),
            explanation: Some("The source reports 21 of 56 doctors, which is 37.5 per cent.".into()),
            evidence: vec![ReportEvidence {
                chunk_id: "c7".into(),
                page: Some(4),
                quote: "The work category of the 56 HCWs: 21 doctors (37.5%)".into(),
            }],
            ..Default::default()
        }
    }

    fn item_without_evidence() -> ReportItem {
        ReportItem {
            seq: 9,
            page: Some(3),
            sentence: "Reporting rates are uneven across institutions.".into(),
            verdict: Some("insufficient_evidence".into()),
            // Prose with nothing behind it — the case D18 exists for.
            explanation: Some("The evidence clearly supports this well-known finding.".into()),
            evidence: vec![],
            ..Default::default()
        }
    }

    fn model() -> AuditReportModel {
        AuditReportModel {
            manuscript_name: "R PAPER .pdf".into(),
            generated_on: "3 September 2026".into(),
            model_id: "qwen2.5-3b-instruct-q4km".into(),
            prompt_version: "citation_support-v1.4".into(),
            total_sentences: 114,
            checked: 84,
            counts_by_category: vec![("citation_need".into(), 65), ("unverifiable".into(), 19)],
            verdict_counts: vec![("needs_citation".into(), 52), ("no_citation_needed".into(), 27)],
            skipped_reasons: vec![("a table row or a figure caption".into(), 5)],
            supported: vec![item_with_evidence()],
            needs_citation: vec![],
            unverifiable: vec![],
            failed: vec![],
        }
    }

    #[test]
    fn d18_prose_never_appears_without_its_evidence() {
        // THE RULE. §11 D18: `explanation` is unvalidated free text — a model
        // once asserted the exact opposite of its evidence, fluently. Printed
        // alone in a PDF, such a sentence is indistinguishable from a checked
        // one, so it is not printed alone.
        let mut m = model();
        m.supported = vec![item_without_evidence()];
        let blocks = compose_audit(&m);
        let text = all_text(&blocks);

        assert!(
            !text.contains("The evidence clearly supports this well-known finding."),
            "ungrounded model prose was printed:\n{text}"
        );
        assert!(text.contains("no source passage was recorded"), "{text}");
        // The sentence and the verdict are still reported — withholding the
        // prose is not the same as hiding the item.
        assert!(text.contains("Reporting rates are uneven"), "{text}");
        assert!(text.contains("insufficient evidence"), "{text}");
    }

    #[test]
    fn evidence_is_printed_before_the_prose_that_rests_on_it() {
        // Order is the guarantee: a reader meets the source before the claim
        // about it, and can therefore disagree.
        let blocks = compose_audit(&model());
        let texts: Vec<String> = blocks.iter().map(text_of).collect();
        let quote = texts.iter().position(|t| t.contains("21 doctors (37.5%)")).expect("evidence");
        let prose = texts
            .iter()
            .position(|t| t.contains("which is 37.5 per cent"))
            .expect("explanation");
        assert!(quote < prose, "prose {prose} came before its evidence {quote}");
    }

    #[test]
    fn every_evidence_quote_carries_a_page_or_says_it_is_unknown() {
        // D18's validated half is `chunk_id` and `page`. A missing page is
        // stated, never silently dropped — an unlabelled quote cannot be found.
        let mut m = model();
        m.supported[0].evidence.push(ReportEvidence {
            chunk_id: "c9".into(),
            page: None,
            quote: "A passage with no recorded page.".into(),
        });
        let text = all_text(&compose_audit(&m));
        assert!(text.contains("c7 · p.4 —"), "{text}");
        assert!(text.contains("c9 · page unknown —"), "{text}");
    }

    #[test]
    fn the_cover_carries_the_paper_the_date_and_the_model() {
        let blocks = compose_audit(&model());
        let cover = blocks.first().expect("a cover");
        match cover {
            Block::Cover { title, subtitle, meta } => {
                assert_eq!(title, "Thesis citation audit");
                assert_eq!(subtitle, "R PAPER .pdf");
                let keys: Vec<&str> = meta.iter().map(|(k, _)| k.as_str()).collect();
                assert!(keys.contains(&"Generated"), "{keys:?}");
                assert!(keys.contains(&"Judged by"), "{keys:?}");
                assert!(keys.contains(&"Prompt version"), "{keys:?}");
            }
            other => panic!("first block must be the cover, got {other:?}"),
        }
        // Provenance, including that it ran locally.
        let text = all_text(&blocks);
        assert!(text.contains("produced on this machine"), "{text}");
        assert!(text.contains("No manuscript text was sent anywhere"), "{text}");
    }

    #[test]
    fn counts_by_category_and_what_the_model_said_are_both_reported() {
        // These are different numbers — 65 sentences were QUEUED as
        // citation_need and the model said "needs one" for 52 of them. A report
        // that showed only the first would overstate the finding by 25%.
        let text = all_text(&compose_audit(&model()));
        assert!(text.contains("65 citation need"), "{text}");
        assert!(text.contains("52 needs citation"), "{text}");
        assert!(text.contains("27 no citation needed"), "{text}");
        assert!(text.contains("5 — a table row or a figure caption"), "{text}");
    }

    #[test]
    fn unverifiable_items_carry_their_entry_and_what_to_do() {
        let mut m = model();
        m.unverifiable = vec![ReportItem {
            seq: 12,
            page: Some(1),
            sentence: "Classical approaches suffer from poor generalisation [5].".into(),
            reason: Some("[5] S. Mohammad and P. Turney, Crowdsourcing — not in your library".into()),
            reference_entry: Some("S. Mohammad and P. Turney, “Crowdsourcing a word-emotion association lexicon,” 2013".into()),
            next_step: Some("Fetch the open-access PDF, or attach a copy you have.".into()),
            ..Default::default()
        }];
        let text = all_text(&compose_audit(&m));
        assert!(text.contains("Reference list entry: S. Mohammad"), "{text}");
        assert!(text.contains("What to do: Fetch the open-access PDF"), "{text}");
        // And the section says whose fault it is not.
        assert!(text.contains("gap in the library, not a defect in the writing"), "{text}");
    }

    #[test]
    fn an_empty_section_says_none_found_rather_than_vanishing() {
        // A missing section reads as "not checked"; "None found" is the fact.
        let text = all_text(&compose_audit(&model()));
        assert!(text.contains("None found."), "{text}");
    }
}
