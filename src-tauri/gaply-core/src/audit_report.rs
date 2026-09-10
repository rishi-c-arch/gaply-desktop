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

use crate::report_compose::{Block, Tone};

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

/// One element of a decomposed claim, and whether the source carried it
/// (§11 D108).
///
/// This is the part of `citation_support` that MEASURED correct while the
/// verdict did not: on `cs-label-005` the model found "46%", "27 categories"
/// and "fine-tuned BERT" present and marked "95%" absent — the right reading —
/// and then answered `weak` anyway. So the decomposition is promoted to the
/// report and the verdict is withdrawn from it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClaimElement {
    pub element: String,
    /// `found` | `absent` | `different`, as the model reported it.
    pub status: String,
}

/// A judged item: one manuscript sentence and what the audit concluded.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReportItem {
    pub seq: i64,
    pub page: Option<u32>,
    /// Paragraph ordinal, for a source with no pages (§11 D65). Never set
    /// alongside `page` — a PDF has the better locator, and offering two is a
    /// reader deciding which to trust.
    #[serde(default)]
    pub paragraph: Option<u32>,
    /// §11 D95. The page could not be derived from the text that prints this
    /// sentence, so `page` is the reflowed block's — a hint, shown as `~p.N`.
    #[serde(default)]
    pub page_approximate: bool,
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
    /// §11 D108. The claim broken into parts, each marked against the source.
    /// Printed for `citation_support` INSTEAD of the verdict.
    #[serde(default)]
    pub claim_elements: Vec<ClaimElement>,
}

/// Everything the report states. Assembled by the caller from the job rows.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuditReportModel {
    pub manuscript_name: String,
    /// Whether the source format HAS page numbers at all.
    ///
    /// A PDF does; a .docx does not — extraction yields 253 blocks with no page
    /// on any of them. Printing "page unknown" beside all 84 items of a Word
    /// document says something is missing when nothing is: the format has no
    /// pages to report. The report says which case it is, once, instead.
    pub has_pages: bool,
    /// Rendered date string. Passed in rather than read from a clock, so a
    /// report is reproducible and the tests are not time-dependent.
    pub generated_on: String,
    pub model_id: String,
    pub prompt_version: String,
    pub total_sentences: usize,
    /// Items the engine RETURNED AN ANSWER FOR — not items it examined and
    /// cleared (§11 D74).
    ///
    /// A `citation_need` item answering "no citation needed" is `done`, so this
    /// counted 83 for a manuscript where 65 of those were the model declining to
    /// flag anything. "Sentences checked: 83" invited the reading that 83
    /// sentences were meaningfully assessed. The cover now says "answered", and
    /// the at-a-glance section says how many of those answers were declinations.
    pub checked: usize,
    /// `kind` → count, e.g. `citation_need` → 65.
    pub counts_by_category: Vec<(String, usize)>,
    /// What the model actually SAID, e.g. `needs_citation` → 52.
    pub verdict_counts: Vec<(String, usize)>,
    /// §11 D110. Cited works a retraction registry has CONFIRMED retracted, as
    /// `(reference entry or marker, sentences citing it)`.
    ///
    /// Deterministic and registry-backed. Kept out of `supported` and out of
    /// every verdict tally on purpose: this is not something the model decided,
    /// and filing it among model findings would rank the most serious fact in
    /// the report below things with an error rate.
    #[serde(default)]
    pub retracted_sources: Vec<(String, usize)>,
    /// Why items could not be judged, and how many.
    pub skipped_reasons: Vec<(String, usize)>,
    pub supported: Vec<ReportItem>,
    /// RETIRED and always empty (§11 D128). Kept so a stored report model from
    /// before the retirement still deserialises; nothing populates it and
    /// nothing renders it.
    pub needs_citation: Vec<ReportItem>,
    pub unverifiable: Vec<ReportItem>,
    /// Items whose model answer failed validation twice.
    pub failed: Vec<ReportItem>,
    /// §11 D94. Deterministic consistency findings, so a researcher has the
    /// list to work from rather than only the gate's warning.
    #[serde(default)]
    pub consistency: Vec<crate::consistency::ConsistencyFinding>,
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
fn page_label(page: Option<u32>, has_pages: bool) -> String {
    match (page, has_pages) {
        (Some(p), _) => format!("p.{p}"),
        // The document is paginated and THIS item lost its page — genuinely
        // unknown, and worth saying.
        (None, true) => "page unknown".to_string(),
        // The document has no pagination. Saying "unknown" here would report a
        // defect where there is only a file format.
        (None, false) => "no page numbers".to_string(),
    }
}

/// Where a finding IS, in the best terms the source supports (§11 D65).
///
/// A PDF has pages. A Word file does not — but it does have paragraphs, and a
/// paragraph ordinal is a real locator rather than an absence. Before this, 84
/// items of a Word manuscript all read "no page numbers", which told a reader
/// where nothing was.
///
/// Page WINS when both exist. Two locators for one sentence is a reader
/// deciding which to trust, and the page is the one they can act on.
fn locator(item: &ReportItem, has_pages: bool) -> String {
    match (item.page, item.paragraph) {
        // §11 D95. `~` is not decoration: it is the difference between "here"
        // and "somewhere near here". The derived page comes from the text that
        // PRINTS the sentence; the approximate one is a reflowed block's page,
        // which is off by one about 8% of the time.
        (Some(p), _) if item.page_approximate => format!("~p.{p}"),
        (Some(p), _) => format!("p.{p}"),
        (None, Some(par)) => format!("¶{par}"),
        (None, None) => page_label(None, has_pages),
    }
}

/// How a verdict should read. The composer knows what the words mean; the
/// renderers decide what the tone looks like in their medium.
fn tone_of(verdict: Option<&str>) -> Tone {
    match verdict.unwrap_or("") {
        "strong" => Tone::Good,
        "no_citation_needed" => Tone::Good,
        "contradicts" => Tone::Bad,
        "not determined" | "" => Tone::Neutral,
        // partial, weak, insufficient_evidence, needs_citation
        _ => Tone::Warn,
    }
}

// §11 D108. `health_score` and `attention_list` lived here and are GONE.
//
// Both read `attention_rank`, which reads the support verdict. That verdict is
// a constant — 14 of 14 valid outputs across two runs were `weak` — so the
// score was 0/100 on every manuscript and the "needs your attention" list was
// every checked claim in document order. Neither described a manuscript.
//
// Do not reinstate either from the support verdict. A score needs an input that
// can come out differently for a good paper and a bad one; §11 D85 is the rule
// (a number that cannot respond to the thing it names is worse than absent).

/// The fewest evidence-backed findings a /100 score may be computed from.
pub const MIN_SCOREABLE: usize = 10;

// §11 D123. ADVISORY_RECALL_PCT / ADVISORY_PRECISION_PCT LIVED HERE AND ARE GONE.
//
// They printed "82% recall, 43% precision" into every report. Both were real
// measurements of a real eval — of a labelled set drawn almost entirely from
// one paper's Introduction, Related Work and early Methodology. The shipped
// audit judges the WHOLE document, and on the paper that exposed this, 31 of
// the 46 suggestions came from Experimental Configuration onward, where the
// labelled set has ZERO cases. Restricted to the sentences the audit actually
// selects, precision measured 25%, not 43%; against the author's own read of
// the full report it was nearer 9%.
//
// The number was not wrong about its sample. It was wrong about its SUBJECT.
//
// DO NOT REINSTATE EITHER FROM A RE-RUN OF THE SAME SET. A replacement needs a
// labelled sample of the population the audit judges — Results, Discussion and
// Conclusion sentences included, which are mostly the authors' own work and
// mostly need no citation. `a_printed_advisory_rate_needs_a_representative_set`
// in `tests/ai_eval_cli.rs` arms itself the moment a constant of this shape
// comes back.

/// Emit ONE judged item, D18-safe.
///
/// The ordering is the guarantee: sentence, then verdict, then — only if there
/// is evidence — each quote with its page, and only then the model's prose. A
/// reader meets the source before the claim about it.
fn emit_finding(out: &mut Vec<Block>, item: &ReportItem, has_pages: bool) {
    // COMPACT: locator, badge, sentence, one line of reason. The previous shape
    // spent four paragraphs per item, which turned 65 findings into something
    // to wade through rather than read.
    out.push(heading(
        format!("Sentence {} · {}", item.seq, locator(item, has_pages)),
        3,
    ));
    if let Some(v) = &item.verdict {
        out.push(Block::Badge {
            text: v.replace('_', " "),
            tone: tone_of(Some(v)),
        });
    }
    out.push(para(format!("“{}”", item.sentence.trim())));

    if let Some(r) = &item.reason {
        out.push(para(r.trim().to_string()));
    }
    if let Some(entry) = &item.reference_entry {
        out.push(bullet(format!("Reference list entry: {entry}"), 0));
    }

    if item.evidence.is_empty() {
        // THE D18 RULE, unchanged. With nothing to check the prose against, the
        // prose is not printed — an unverifiable assertion in a PDF is
        // indistinguishable from a verified one, and the reader cannot tell.
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
            out.push(bullet(
                format!("{} · {} — “{}”", e.chunk_id, page_label(e.page, has_pages), e.quote.trim()),
                1,
            ));
        }
        if let Some(x) = &item.explanation {
            out.push(para(format!("The model's reading of that evidence: {}", x.trim())));
        }
    }

    if let Some(step) = &item.next_step {
        out.push(bullet(format!("What to do: {step}"), 0));
    }
}

/// Emit ONE EVIDENCE ITEM for `citation_support` (§11 D108).
///
/// Deliberately not `emit_finding`, for the reason D89 split suggestions out:
/// the badge is the report's verdict vocabulary, and this verdict is not
/// earned. Measured across two runs on the 6-case cold set, every one of
/// FOURTEEN valid outputs was `weak` — a constant `weak` predictor scores the
/// same 38% agreement the model does. The verdict carries no information.
///
/// What IS earned stays and is promoted: the passages the claim rests on, with
/// their page links, and the model's own decomposition of the claim. On
/// `cs-label-005` that decomposition was exactly right while the verdict was
/// wrong, which is what distinguishes "cannot read the source" from "reads it
/// correctly and answers regardless".
///
/// The caveat rides with the ITEM, not only the section: a reader who lands on
/// item 19 never saw the heading.
fn emit_evidence_item(out: &mut Vec<Block>, item: &ReportItem, has_pages: bool) {
    out.push(heading(format!("Sentence {} · {}", item.seq, locator(item, has_pages)), 3));
    // NO Block::Badge. The five-class verdict is withheld from the report.
    out.push(para(
        "passages located · the source text below is the finding · Gaply does not grade          how well it supports the sentence",
    ));
    out.push(para(format!("\u{201c}{}\u{201d}", item.sentence.trim())));

    if let Some(entry) = &item.reference_entry {
        out.push(bullet(format!("Reference list entry: {entry}"), 0));
    }

    if item.evidence.is_empty() {
        // THE D18 RULE, unchanged: with nothing to check the prose against, the
        // prose is not printed.
        if item.explanation.is_some() {
            out.push(Block::Note {
                text: "The model's explanation is withheld: no source passage was recorded for \
                       this item, so there is nothing to check it against."
                    .to_string(),
            });
        }
    } else {
        out.push(para("From the cited source — read this and judge it yourself:"));
        for e in &item.evidence {
            out.push(bullet(
                format!("{} · {} — \u{201c}{}\u{201d}", e.chunk_id, page_label(e.page, has_pages), e.quote.trim()),
                1,
            ));
        }
        // The prose stays, AFTER the evidence it rests on — D18's rule is that
        // a model's reading may be printed when there is a quote to check it
        // against, and that is satisfied here. It is the five-class VERDICT
        // that is withdrawn, not everything the model said.
        if let Some(x) = &item.explanation {
            out.push(para(format!("The model's reading of that evidence: {}", x.trim())));
        }
    }

    // The decomposition, which is the half that measured correct.
    if !item.claim_elements.is_empty() {
        out.push(para("What the sentence claims, part by part:"));
        for el in &item.claim_elements {
            let mark = match el.status.as_str() {
                "found" => "in the source",
                "absent" => "NOT in the passages read",
                "different" => "the source says otherwise",
                other => other,
            };
            out.push(bullet(format!("{} — {mark}", el.element.trim()), 1));
        }
        out.push(Block::Note {
            text: "These marks are the model's reading of the passages above, not Gaply's \
                   conclusion. Check them against the quoted text."
                .to_string(),
        });
    }

    if let Some(step) = &item.next_step {
        out.push(bullet(format!("What to do: {step}"), 0));
    }
}

/// The evidence list, emitted with `emit_evidence_item` (§11 D108).
fn emit_evidence_items(out: &mut Vec<Block>, title: &str, blurb: &str, items: &[ReportItem], has_pages: bool) {
    out.push(heading(title, 1));
    if items.is_empty() {
        out.push(para(format!("{blurb} None found.")));
        return;
    }
    out.push(para(blurb));
    // Document order. The previous sort was by `attention_rank`, which reads
    // the VERDICT — ordering by a signal we have just withdrawn would put the
    // verdict back into the report as a ranking (§11 D108).
    let mut sorted: Vec<&ReportItem> = items.iter().collect();
    sorted.sort_by_key(|i| i.seq);
    for item in sorted {
        emit_evidence_item(out, item, has_pages);
    }
}

fn emit_section(
    out: &mut Vec<Block>,
    title: &str,
    blurb: &str,
    items: &[ReportItem],
    has_pages: bool,
) {
    out.push(heading(title, 1));
    if items.is_empty() {
        out.push(para(format!("{blurb} None found.")));
        return;
    }
    out.push(para(blurb));
    // Most severe first, then document order within a severity — so the reader
    // meets what most needs them at the top of each section.
    let mut sorted: Vec<&ReportItem> = items.iter().collect();
    // §11 D108. Was `(attention_rank(i), i.seq)`. `attention_rank` read the
    // support verdict; the sections still using this emitter carry no verdict
    // at all, so document order is the honest one.
    sorted.sort_by_key(|i| i.seq);
    for item in sorted {
        emit_finding(out, item, has_pages);
    }
}

/// One cited work that nothing could be checked against, and everything that
/// depends on it.
struct BlockedSource<'a> {
    entry: String,
    items: Vec<&'a ReportItem>,
    next_step: Option<String>,
}

/// Group the unverifiable items BY SOURCE.
///
/// The list was per-sentence, which showed the same missing paper five times
/// and asked the reader to notice. Grouped, each row is one thing to fix and
/// says how many sentences it unblocks — which is the number that decides
/// whether it is worth fixing.
fn group_by_source(items: &[ReportItem]) -> Vec<BlockedSource<'_>> {
    let mut order: Vec<String> = Vec::new();
    let mut map: std::collections::HashMap<String, BlockedSource> = Default::default();
    for it in items {
        // The reference entry identifies the WORK; the reason is the fallback
        // when no entry was parsed.
        let key = it
            .reference_entry
            .clone()
            .or_else(|| it.reason.clone())
            .unwrap_or_else(|| "unidentified source".to_string());
        if !map.contains_key(&key) {
            order.push(key.clone());
            map.insert(
                key.clone(),
                BlockedSource {
                    entry: key.clone(),
                    items: Vec::new(),
                    next_step: it.next_step.clone(),
                },
            );
        }
        map.get_mut(&key).expect("just inserted").items.push(it);
    }
    let mut out: Vec<BlockedSource> = order.into_iter().filter_map(|k| map.remove(&k)).collect();
    // The source blocking the most sentences is the one worth fixing first.
    out.sort_by_key(|g| std::cmp::Reverse(g.items.len()));
    out
}

fn emit_blocked_sources(out: &mut Vec<Block>, items: &[ReportItem], has_pages: bool) {
    out.push(heading("Cited, but not checkable", 1));
    if items.is_empty() {
        out.push(para("Every cited source was available to check against. None found."));
        return;
    }
    let groups = group_by_source(items);
    out.push(para(format!(
        "{} sentence{} cite {} source{} Gaply could not read. This is a gap in your library, \
         not a defect in the writing — each source below is one action away from being checkable.",
        items.len(),
        if items.len() == 1 { "" } else { "s" },
        groups.len(),
        if groups.len() == 1 { "" } else { "s" },
    )));

    for g in groups {
        out.push(heading(
            format!(
                "{} — blocks {} sentence{}",
                g.entry.chars().take(90).collect::<String>(),
                g.items.len(),
                if g.items.len() == 1 { "" } else { "s" }
            ),
            2,
        ));
        out.push(Block::Badge { text: "not checkable".into(), tone: Tone::Warn });
        if let Some(step) = &g.next_step {
            out.push(para(format!("Fix: {step}")));
        }
        for it in &g.items {
            out.push(bullet(
                format!(
                    "{} — “{}”",
                    locator(it, has_pages),
                    it.sentence.trim().chars().take(110).collect::<String>()
                ),
                0,
            ));
        }
    }
}

/// Compose the whole report.
pub fn compose_audit(m: &AuditReportModel) -> Vec<Block> {
    let mut out = Vec::new();

    // §11 D93 made the cover ANSWER rather than describe. §11 D108 changes
    // what it may honestly answer WITH.
    //
    // The old headline was "{score} / 100 — N of M checked claims held up",
    // and every input to it came from the support verdict. That verdict is a
    // constant: fourteen valid outputs across two runs were all `weak`, and
    // `attention_rank("weak") < 9`, so EVERY checked claim counted as failing
    // and the cover read "0 / 100 — 0 of N held up" on any manuscript
    // whatsoever. A fabricated failing grade is worse than no grade, and §11
    // D85 already settled the principle: a number that cannot respond to the
    // thing it names is worse than absent.
    let cover_checked = m.supported.len();
    let cover_headline = Some(if cover_checked == 0 {
        (
            "No claim could be checked against its cited source".to_string(),
            Tone::Neutral,
        )
    } else {
        (
            {
                // The number of claims whose passages were actually quoted is
                // the honest second figure — not the count of claims, and not
                // a `.max(1)` floor, which would have reported "1 source" for
                // a report that quoted none.
                let with_passages =
                    m.supported.iter().filter(|i| !i.evidence.is_empty()).count();
                format!(
                    "{cover_checked} cited claim{} — source passages quoted for {with_passages}",
                    if cover_checked == 1 { "" } else { "s" },
                )
            },
            Tone::Neutral,
        )
    });
    out.push(Block::Cover {
        headline: cover_headline,
        title: "Thesis citation audit".to_string(),
        subtitle: m.manuscript_name.clone(),
        meta: vec![
            ("Generated".to_string(), m.generated_on.clone()),
            ("Judged by".to_string(), m.model_id.clone()),
            ("Prompt version".to_string(), m.prompt_version.clone()),
            ("Sentences read".to_string(), m.total_sentences.to_string()),
            // "answered", not "checked": see `AuditReportModel::checked`.
            ("Sentences answered".to_string(), m.checked.to_string()),
        ],
    });

    // ---- AT A GLANCE ------------------------------------------------------
    // The first page answers "how bad is it, and what do I do first". The
    // per-item detail is reference material behind it.
    // Advisory items are retired (§11 D128); "judged" is what was CHECKED.
    let judged = m.supported.len();

    out.push(heading("At a glance", 1));

    // §11 D108. THE SCORE IS WITHDRAWN.
    //
    // It was the fraction of checked claims whose verdict was not one of
    // contradicts/weak/insufficient/partial. The model answers `weak` to
    // everything — 14 of 14 valid outputs across two runs — so the numerator
    // was always zero and the score always 0/100, on every manuscript, whatever
    // the sources said. That is not a low score; it is not a measurement at
    // all. §11 D85: a number that cannot respond to the thing it names is worse
    // than absent.
    let checked = m.supported.len();
    out.push(Block::Badge { text: "no score".into(), tone: Tone::Neutral });
    if checked == 0 {
        out.push(para(
            "No claim could be checked against its cited source, so there is nothing to report \
             here. The sections below say why, and what would make a check possible.",
        ));
    } else {
        out.push(para(format!(
            "Gaply does not score how well your sources support your claims, and this report \
             gives no /100. It located the source passages behind {checked} cited claim{} and \
             quotes them below with their page — reading those is the check. The grader that \
             used to produce a score was measured on a labelled set and returned the SAME grade \
             for every case (14 of 14 outputs across two runs), so any number built from it \
             described the grader rather than your manuscript.",
            if checked == 1 { "" } else { "s" },
        )));
    }


    // The breakdown. Evidence-backed and advisory are SEPARATE BARS, never
    // summed — a chart that adds a 43%-precision suggestion to a checked
    // finding is the same conflation the score just removed (§11 D78).
    let total_bar = checked + m.unverifiable.len() + m.failed.len();
    out.push(para("The whole manuscript, proportionally:"));
    // §11 D93. DRAWN bars, and a tone each — the renderer decides the pixels.
    // These were `"=".repeat(n)` inside a bullet, which is a chart only in a
    // terminal; in a proportional font the alignment padding collapsed and the
    // count ran into the percent ("16 19%").
    let mut prop = |label: &str, n: usize, tone: Tone| {
        out.push(Block::Bar {
            label: label.to_string(),
            value: n,
            total: total_bar,
            tone,
        });
    };
    // §11 D108. Was "checked, held up" / "checked, did not hold up" — the
    // withdrawn verdict, split into two bars and coloured. One bar now, saying
    // only what is true: the passages were found.
    prop("source passages located", checked, Tone::Good);
    prop("could not be checked", m.unverifiable.len(), Tone::Neutral);
    if !m.failed.is_empty() {
        prop("not judged", m.failed.len(), Tone::Neutral);
    }

    // §11 D108. THE ATTENTION LIST IS WITHDRAWN.
    //
    // "The N checked claims that most need your attention" was ordered by
    // `attention_rank`, which reads the support verdict, and each bullet
    // printed that verdict in brackets. With a constant `weak` it listed EVERY
    // checked claim, in document order, each tagged `[weak]` — a ranking with
    // nothing ranking it, under a heading asserting these are the worst.
    //
    // Nothing replaces it. Ordering the reader's attention needs a signal that
    // discriminates, and this engine does not currently have one for support.
    // The deterministic consistency checks (§11 D94) lead instead: they carry
    // no error rate because no model produced them.

    // The provenance line is not decoration. A reader months from now needs to
    // know which model produced this and that it ran locally.
    out.push(Block::Note {
        text: format!(
            "Everything a language model contributed to this report was produced on this \
             machine by {} ({}). No manuscript text was sent anywhere.",
            m.model_id, m.prompt_version
        ),
    });

    if !m.has_pages {
        out.push(Block::Note {
            text: "This manuscript is a Word document, which has no page numbering, so findings \
                   are located by sentence rather than by page. Export it as a PDF and re-run to \
                   get page references."
                .to_string(),
        });
    }

    out.push(Block::PageBreak);
    out.push(heading("The counts", 1));
    out.push(para(format!(
        "{} sentences were read and {} were checked against a source or judged for whether they \
         need one.",
        m.total_sentences, m.checked
    )));
    for (kind, n) in &m.counts_by_category {
        out.push(bullet(format!("{n} {}", kind.replace('_', " ")), 0));
    }
    // §11 D108. The support VERDICTS are recorded but no longer summarised as
    // bars. "support: weak 8 (67%)" is the same withdrawn grade wearing a
    // headline, and a bar is the most emphatic thing on the page.
    let (support_counts, other_counts): (Vec<_>, Vec<_>) =
        m.verdict_counts.iter().partition(|(v, _)| v.starts_with("support:"));
    if !other_counts.is_empty() {
        out.push(para("What the model concluded:"));
        for (v, n) in &other_counts {
            // §11 D93. The second ASCII-bar site. `{:<26}` padded with spaces
            // that a proportional font does not align, so the label and the
            // count collided exactly as they did above.
            out.push(Block::Bar {
                label: v.replace('_', " "),
                value: *n,
                total: judged.max(1),
                tone: tone_of(Some(v)),
            });
        }
    }
    if !support_counts.is_empty() {
        let located: usize = support_counts.iter().map(|(_, n)| *n).sum();
        out.push(para(format!(
            "{located} cited sentence{} had its source passages located and quoted in this \
             report. Gaply does not grade how well a passage supports a sentence: on a 6-case \
             labelled set its grade was identical on all 14 outputs across two runs, so the \
             grade carries no information and is not reported. The passages are.",
            if located == 1 { "" } else { "s" }
        )));
    }
    if !m.skipped_reasons.is_empty() {
        out.push(para("Not judged, and why:"));
        for (r, n) in &m.skipped_reasons {
            out.push(bullet(format!("{n} — {r}"), 0));
        }
    }

    // §11 D110. RETRACTION LEADS. Two reasons it sits above even the
    // deterministic consistency checks:
    //
    //   * it is the most serious fact this report can carry — a cited work has
    //     been withdrawn by its own publisher — and it is settled, not judged;
    //   * it is registry-backed and costs nothing to establish, so burying it
    //     under three hours of model output inverts the cost of finding out.
    //
    // NOT a gate, and NOT filed among findings. Citing a retracted paper can be
    // entirely correct — a paper ABOUT the retraction must cite it — so this
    // reports and does not block. What it must never do is stay quiet.
    if !m.retracted_sources.is_empty() {
        out.push(heading("Retracted sources", 1));
        out.push(Block::Badge { text: "retracted".into(), tone: Tone::Bad });
        out.push(para(format!(
            "{} of the works this manuscript cites {} been RETRACTED by the publisher. This is \
             not a judgement of your writing and no language model was involved — a retraction \
             registry was asked and answered. Citing a retracted work can be legitimate when the \
             retraction is the point; otherwise the citation needs replacing.",
            m.retracted_sources.len(),
            if m.retracted_sources.len() == 1 { "has" } else { "have" },
        )));
        for (entry, citing) in &m.retracted_sources {
            out.push(bullet(
                format!("{entry} — cited by {citing} sentence{}", if *citing == 1 { "" } else { "s" }),
                0,
            ));
        }
        out.push(Block::Note {
            text: "Only works a registry actually answered about appear here. An entry that was \
                   never checked is not listed and is not clean — the Citation Manager says which \
                   is which."
                .to_string(),
        });
    }

    // Order of the detail sections follows what the reader can act on: the
    // library gap is one batch action away from being fixed, so it comes before
    // the per-sentence reading.
    // ORDER (§11 D78): evidence-backed findings LEAD. `citation_support` checks
    // prose against a real source and is the half worth trusting; the advisory
    // list follows it rather than competing with it for the reader's attention.
    // §11 D94. Deterministic — no model touched any of these, and that is
    // worth saying next to a report whose other half is a 3B's judgement.
    if !m.consistency.is_empty() {
        out.push(Block::PageBreak);
        out.push(heading("Consistency checks", 1));
        out.push(para(
            "Found by reading the manuscript's own structure — its reference list, markers, \
             captions and headings. No language model was involved, so unlike the sections \
             below these are not judgements and carry no error rate.",
        ));
        let structural: Vec<_> = m.consistency.iter().filter(|f| f.severity == crate::consistency::Severity::Structural).collect();
        let cosmetic: Vec<_> = m.consistency.iter().filter(|f| f.severity == crate::consistency::Severity::Cosmetic).collect();
        if !structural.is_empty() {
            out.push(heading("These affect whether the audit above can be trusted", 2));
            for f in structural {
                out.push(Block::Badge { text: "resolution".into(), tone: Tone::Bad });
                out.push(para(f.message.clone()));
                if let Some(a) = &f.action {
                    out.push(bullet(format!("What to do: {a}"), 0));
                }
            }
        }
        if !cosmetic.is_empty() {
            out.push(heading("Worth fixing; they change no verdict above", 2));
            for f in cosmetic {
                out.push(para(f.message.clone()));
                if let Some(a) = &f.action {
                    out.push(bullet(format!("What to do: {a}"), 0));
                }
            }
        }
    }

    out.push(Block::PageBreak);
    // §11 D108. Was "Claims checked against their source", led with a verdict
    // badge, and told the reader to read the passage "before the verdict".
    // There is no verdict here any more: fourteen valid outputs across two runs
    // were all `weak`, and a constant scores the same. The passages stay,
    // because those are what measured correct.
    emit_evidence_items(
        &mut out,
        "The source passages behind each cited claim",
        "Each sentence below cites a source Gaply could read, and the passages it rests on are \
         quoted underneath it with their page. THIS IS THE EVIDENCE-BACKED SECTION — and the \
         evidence is the finding. Gaply does NOT grade how well a passage supports a sentence: \
         measured on a 6-case labelled set, its grade was the same one every time (14 of 14 \
         outputs across two runs), so the grade carries no information and is not shown. \
         Reading the quoted passage is the check.",
        &m.supported,
        m.has_pages,
    );

    out.push(Block::PageBreak);
    emit_blocked_sources(&mut out, &m.unverifiable, m.has_pages);

    // THE ADVISORY SECTION IS GONE — §11 D128.
    //
    // "Worth a second look" listed sentences a language model thought might
    // need a citation. Measured population-weighted it scored 18.3% precision
    // against an 18.0% no-skill baseline — indistinguishable from a rule that
    // flags every sentence, not merely weak, and it flagged 83% of what it saw.
    // `thesis_audit` no longer plans those items, so `m.needs_citation` is
    // empty; the section is REMOVED rather than left to render "none found",
    // which would imply a check that no longer happens.
    //
    // Same treatment as the support verdict (§11 D85): the evidence and the
    // deterministic findings stand, the model's opinion about them does not.

    if !m.failed.is_empty() {
        out.push(Block::PageBreak);
        emit_section(
            &mut out,
            "Not judged",
            "The model's answer for these did not pass Gaply's own checks, twice. They are \
             reported rather than hidden: an unanswered sentence is not a clean one.",
            &m.failed,
            m.has_pages,
        );
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text_of(b: &Block) -> String {
        match b {
            Block::Cover { title, subtitle, meta, headline } => format!(
                "{title} {subtitle} {} {}",
                headline.as_ref().map(|(h, _)| h.as_str()).unwrap_or(""),
                meta.iter().map(|(k, v)| format!("{k}={v}")).collect::<Vec<_>>().join(" ")
            ),
            Block::Heading { text, .. } => text.clone(),
            Block::Paragraph { text } => text.clone(),
            Block::Bullet { text, .. } => text.clone(),
            Block::Note { text } => text.clone(),
            Block::Badge { text, .. } => text.clone(),
            // §11 D93. The bar's TEXT is its label and its numbers — what a
            // reader would see — so assertions about the report's wording keep
            // working across the ASCII-to-drawn change.
            Block::Bar { label, value, total, .. } => {
                let pct = if *total == 0 { 0 } else { ((*value as f64 / *total as f64) * 100.0).round() as u32 };
                format!("{label} {value} ({pct}%)")
            }
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
            has_pages: true,
            generated_on: "3 September 2026".into(),
            model_id: "qwen2.5-3b-instruct-q4km".into(),
            prompt_version: "citation_support-v1.4".into(),
            total_sentences: 114,
            checked: 84,
            counts_by_category: vec![("citation_need".into(), 65), ("unverifiable".into(), 19)],
            verdict_counts: vec![("needs_citation".into(), 52), ("no_citation_needed".into(), 27)],
            retracted_sources: vec![],
            skipped_reasons: vec![("a table row or a figure caption".into(), 5)],
            supported: vec![item_with_evidence()],
            needs_citation: vec![],
            unverifiable: vec![],
            failed: vec![],
            consistency: vec![],
        }
    }

    /// §11 D80. The guard's marker list must keep matching the real composer.
    ///
    /// The guard refuses a document carrying `GAPLY_REPORT_MIN_MARKERS` of
    /// these. Rename a heading in `compose_audit` and the list silently loses a
    /// marker; do that three times and the guard stops firing on the exact
    /// document it exists to catch, with every test still green. So the list is
    /// checked against real composed output rather than trusted.
    #[test]
    fn every_marker_appears_in_a_real_composed_report() {
        use crate::ai_engine::audit_prepass::{
            detect_gaply_report, gaply_report_markers_in, GAPLY_REPORT_MARKERS,
            GAPLY_REPORT_MIN_MARKERS,
        };

        let text = all_text(&compose_audit(&model()));
        let found = gaply_report_markers_in(&text);

        // `PublishReady` is page furniture drawn by `report_pdf`, not by
        // `compose_audit`, so it is the one marker this composer cannot show.
        // Named explicitly rather than skipped by count, so that if it ever
        // starts being emitted here the exemption is revisited.
        // §11 D108. "Claims checked against their source" is the RETIRED
        // heading, kept in the marker list so reports exported before the
        // rename are still recognised. A current report cannot emit it.
        //
        // §11 D128. "Worth a second look" is the same case for a different
        // reason: the advisory SECTION is retired, not renamed, but every report
        // exported before that is still a Gaply report and the guard must still
        // recognise it. Listing both exemptions by NAME rather than loosening
        // the count keeps the guard's real job intact — if a LIVE heading ever
        // stops being composed, this still fails.
        const RETIRED_HEADINGS: &[&str] =
            &["Claims checked against their source", "Worth a second look"];
        let missing: Vec<&str> = GAPLY_REPORT_MARKERS
            .iter()
            .copied()
            .filter(|m| {
                *m != "PublishReady" && !RETIRED_HEADINGS.contains(m) && !found.contains(m)
            })
            .collect();
        assert!(
            missing.is_empty(),
            "these markers no longer appear in a composed report, so the guard is weaker \
             than it reads: {missing:?}\n\n{text}"
        );

        // And the whole point: a real report trips the guard, with margin.
        let fired = detect_gaply_report(&[crate::extract::docparse::PagedBlock {
            page: Some(1),
            style: None,
            text: text.clone(),
        }]);
        assert!(fired.is_some(), "a real composed report did not trip the guard:\n{text}");
        assert!(
            found.len() > GAPLY_REPORT_MIN_MARKERS,
            "a real report trips the guard with no margin ({} markers, threshold {}); \
             one renamed heading would disarm it",
            found.len(),
            GAPLY_REPORT_MIN_MARKERS
        );
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
        // The SENTENCE is still reported — withholding the prose is not the
        // same as hiding the item. §11 D108: the verdict is no longer printed
        // beside it, so this no longer asserts one.
        assert!(text.contains("Reporting rates are uneven"), "{text}");
        assert!(!text.contains("insufficient evidence"), "a verdict was printed:\n{text}");
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

    /// §11 D65. A Word file has no pages, but it HAS paragraphs — and a
    /// paragraph ordinal is a locator rather than an absence. 84 items reading
    /// "no page numbers" told a reader where nothing was.
    #[test]
    fn an_unpaginated_source_locates_findings_by_paragraph() {
        let mut m = model();
        m.has_pages = false;
        // §11 D128. This used to assert the locator on an ADVISORY item. That
        // lane is retired and renders nothing, so the property is shown on an
        // item the report actually emits — which is the stronger place for it.
        m.failed = vec![ReportItem {
            seq: 4,
            page: None,
            paragraph: Some(12),
            sentence: "A sentence the model could not answer for.".into(),
            ..Default::default()
        }];
        let text = all_text(&compose_audit(&m));
        assert!(text.contains("¶12"), "no paragraph locator:\n{text}");
        assert!(
            !text.contains("no page numbers"),
            "fell back to the absence message despite having a locator:\n{text}"
        );
    }

    /// A page and a paragraph must never both show: two locators for one
    /// sentence is a reader deciding which to trust.
    #[test]
    fn a_page_wins_over_a_paragraph_when_both_exist() {
        let it = ReportItem {
            seq: 1,
            page: Some(7),
            paragraph: Some(12),
            ..Default::default()
        };
        assert_eq!(locator(&it, true), "p.7");
    }

    /// And an item with neither still says which KIND of absence it is.
    #[test]
    fn neither_locator_still_distinguishes_the_two_absences() {
        let bare = ReportItem { seq: 1, ..Default::default() };
        assert_eq!(locator(&bare, true), "page unknown");
        assert_eq!(locator(&bare, false), "no page numbers");
    }

    #[test]
    fn a_document_with_no_pagination_says_so_once_not_per_item() {
        // The exported report said "page unknown" on all 84 items of a Word
        // document. Nothing was missing — .docx has no pages. Saying "unknown"
        // 84 times reports a defect that does not exist.
        let mut m = model();
        m.has_pages = false;
        m.supported[0].page = None;
        m.supported[0].evidence[0].page = None;
        let text = all_text(&compose_audit(&m));
        assert!(!text.contains("page unknown"), "{text}");
        assert!(text.contains("no page numbers"), "{text}");
        assert_eq!(text.matches("Word document, which has no page numbering").count(), 1);
        assert!(text.contains("Export it as a PDF and re-run"), "the fix is named: {text}");
    }

    #[test]
    fn a_paginated_document_still_says_unknown_when_a_page_is_genuinely_missing() {
        // The honest reading only changes for formats that HAVE no pages.
        let mut m = model();
        m.supported[0].page = None;
        let text = all_text(&compose_audit(&m));
        assert!(text.contains("page unknown"), "{text}");
        assert!(!text.contains("Word document"), "{text}");
    }

    #[test]
    fn the_cover_carries_the_paper_the_date_and_the_model() {
        let blocks = compose_audit(&model());
        let cover = blocks.first().expect("a cover");
        match cover {
            Block::Cover { title, subtitle, meta, .. } => {
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
        // The verdict counts are drawn as proportional bars now, so the number
        // and its label are no longer adjacent — assert both, not the old
        // "52 needs citation" spelling.
        assert!(text.contains("needs citation"), "{text}");
        assert!(text.contains("52"), "{text}");
        assert!(text.contains("no citation needed"), "{text}");
        assert!(text.contains("27"), "{text}");
        assert!(text.contains("5 — a table row or a figure caption"), "{text}");
    }

    #[test]
    fn unverifiable_items_are_grouped_by_source_with_one_action_each() {
        // Grouped BY SOURCE, not by sentence: five sentences citing one missing
        // paper are ONE thing to fix, and listing it five times asked the
        // reader to work that out for themselves.
        let entry = "S. Mohammad and P. Turney, “Crowdsourcing a word-emotion association lexicon,” 2013";
        let item = |seq: i64, sentence: &str| ReportItem {
            seq,
            page: Some(1),
            sentence: sentence.into(),
            reason: Some("[5] not in your library".into()),
            reference_entry: Some(entry.into()),
            next_step: Some("Fetch the open-access PDF, or attach a copy you have.".into()),
            ..Default::default()
        };
        let mut m = model();
        m.unverifiable = vec![
            item(12, "Classical approaches suffer from poor generalisation [5]."),
            item(31, "Lexicon methods remain a strong baseline [5]."),
            item(44, "The association lexicon is widely used [5]."),
        ];
        let text = all_text(&compose_audit(&m));

        // ONE heading for the source, carrying the count that decides whether
        // fixing it is worth the reader's time.
        assert!(text.contains("blocks 3 sentences"), "{text}");
        assert_eq!(
            text.matches("Fix: Fetch the open-access PDF").count(),
            1,
            "the action was repeated per sentence instead of per source:\n{text}"
        );
        assert!(text.contains("S. Mohammad and P. Turney"), "{text}");
        // Every dependent sentence is still listed under it — grouping must not
        // hide which sentences are affected.
        for s in ["Classical approaches", "Lexicon methods", "association lexicon"] {
            assert!(text.contains(s), "missing dependent sentence {s}:\n{text}");
        }
        // And the section says whose fault it is not.
        assert!(text.contains("not a defect in the writing"), "{text}");
    }

    /// §11 D78. The score covers EVIDENCE-BACKED findings only.
    ///
    /// `citation_need` measures 43% precision — fewer than half its flags are
    /// real — so averaging it into a health number makes the number advisory
    /// too. Before this, 65 declinations produced "97/100" on a paper with a
    /// genuine miscitation.
    #[test]
    fn advisory_suggestions_do_not_move_the_score() {
        let mut m = model();
        // 10 checked claims, 2 of which failed -> 80.
        m.supported = (0..10)
            .map(|i| ReportItem {
                seq: i,
                sentence: format!("Checked claim {i}."),
                verdict: Some(if i < 2 { "weak" } else { "strong" }.into()),
                ..Default::default()
            })
            .collect();
        // §11 D108. There is no score to move any more, so the property is
        // stated where it now lives: the evidence section counts ONLY claims
        // checked against a source, and no /100 is rendered at all.
        let before = all_text(&compose_audit(&m));
        assert!(!before.contains("/ 100"), "a /100 survived the withdrawal:\n{before}");

        // Fifty advisory suggestions must not shift it by a point.
        m.needs_citation = (0..50)
            .map(|i| ReportItem {
                seq: 100 + i,
                sentence: format!("Suggestion {i}."),
                verdict: Some("needs_citation".into()),
                ..Default::default()
            })
            .collect();
        let text = all_text(&compose_audit(&m));
        assert!(!text.contains("/ 100"), "fifty suggestions produced a score:\n{text}");
        // The count of located claims is unmoved by advisory items.
        assert!(
            text.contains("10 cited claims"),
            "advisory items changed what the cover counts:\n{text}"
        );
        // §11 D128. The advisory lane is RETIRED: fifty items must not merely
        // fail to move the score, they must not appear at all. Neither its
        // section, nor its old heading, nor any of its sentences.
        for banned in ["suggestions, not findings", "Worth a second look",
                       "Sentences that may need a citation", "Suggestion 0."] {
            assert!(!text.contains(banned), "retired advisory surface {banned:?} is back:\n{text}");
        }
        // §11 D123. NO measured rate is stated. Both figures were real
        // measurements of a labelled set drawn from one paper's front third,
        // while the audit judges the whole document — so they described a
        // population the product never sees. The honest qualifier replaces
        // them, and nothing here may quietly reintroduce a percentage.
        for banned in ["43%", "82%", "4 in 10"] {
            assert!(!text.contains(banned), "{banned:?} is back in the report:\n{text}");
        }
    }

    /// §11 D95. A derived page and a guessed one must not look alike.
    #[test]
    fn an_approximate_page_is_marked_and_an_exact_one_is_not() {
        let exact = ReportItem { seq: 1, page: Some(4), page_approximate: false, ..Default::default() };
        assert_eq!(locator(&exact, true), "p.4");

        let approx = ReportItem { seq: 2, page: Some(3), page_approximate: true, ..Default::default() };
        assert_eq!(
            locator(&approx, true),
            "~p.3",
            "a reflowed block's page is off by one about 8% of the time and must say so"
        );
    }

    /// OMIT RATHER THAN APPROXIMATE. With no page at all, print none — a page
    /// the reader cannot rely on costs them a search and costs us their trust.
    #[test]
    fn no_page_and_no_paragraph_prints_neither() {
        let none = ReportItem { seq: 3, page: None, paragraph: None, ..Default::default() };
        let out = locator(&none, true);
        assert!(!out.contains("p.1"), "a missing page must not become page 1: {out}");
        assert!(!out.contains('~'), "{out}");
    }

    /// A Word manuscript has no pages at all, so the paragraph ordinal stands
    /// and is EXACT — it must never be marked approximate.
    #[test]
    fn a_paragraph_locator_is_never_marked_approximate() {
        let para = ReportItem { seq: 4, page: None, paragraph: Some(27), ..Default::default() };
        assert_eq!(locator(&para, false), "¶27");
    }

    /// §11 D93. The proportions are DRAWN, not typed.
    ///
    /// `"=".repeat(n)` is a bar chart in a terminal and a run of punctuation in
    /// a PDF, and the `{:<24}` padding meant to align the count does nothing in
    /// a proportional font — so "16" and "19%" arrived as "16 19%".
    #[test]
    fn proportions_are_bar_blocks_not_ascii() {
        let mut m = model();
        m.supported = (0..10)
            .map(|i| ReportItem { seq: i, verdict: Some("strong".into()), ..Default::default() })
            .collect();
        let blocks = compose_audit(&m);

        let bars: Vec<&Block> = blocks.iter().filter(|b| matches!(b, Block::Bar { .. })).collect();
        assert!(bars.len() >= 4, "the At-a-glance proportions are not Bar blocks: {}", bars.len());

        // And no ASCII bar survives anywhere in the document.
        let text = all_text(&blocks);
        assert!(!text.contains("===="), "an ASCII bar is still being emitted:\n{text}");
    }

    /// §11 D93. The count and the percent are separated, and stay separated.
    #[test]
    fn a_count_never_collides_with_its_percent() {
        let mut m = model();
        m.supported = (0..10)
            .map(|i| ReportItem { seq: i, verdict: Some("strong".into()), ..Default::default() })
            .collect();
        let text = all_text(&compose_audit(&m));
        // The shape that shipped: "1 1%", "0 0%", "16 19%".
        let bad = regex_like_collision(&text);
        assert!(bad.is_none(), "count and percent collide: {bad:?}\n{text}");
        // The shape that replaced it.
        assert!(text.contains("(0%)") || text.contains("(100%)"), "no parenthesised percent:\n{text}");
    }

    /// Crude but exact: "<digits> <digits>%" with a single space is the defect.
    fn regex_like_collision(text: &str) -> Option<String> {
        for line in text.lines() {
            let toks: Vec<&str> = line.split_whitespace().collect();
            for w in toks.windows(2) {
                let (a, b) = (w[0], w[1]);
                if a.chars().all(|c| c.is_ascii_digit())
                    && b.ends_with('%')
                    && b[..b.len() - 1].chars().all(|c| c.is_ascii_digit())
                {
                    return Some(line.to_string());
                }
            }
        }
        None
    }

    /// §11 D110. A retracted cited source is the most serious thing this
    /// report can carry, it is registry-backed rather than judged, and it used
    /// to be absent entirely — `grep -i retract` over the whole report path
    /// returned nothing while the Citation Manager held the fact.
    #[test]
    fn a_retracted_source_leads_the_report_and_is_not_a_model_finding() {
        let mut m = model();
        m.retracted_sources = vec![
            ("Wakefield et al., Ileal-lymphoid-nodular hyperplasia, Lancet 1998".into(), 3),
            ("Some Other Withdrawn Paper (2016)".into(), 1),
        ];
        let blocks = compose_audit(&m);
        let text = all_text(&blocks);

        assert!(text.contains("Retracted sources"), "no retraction section:\n{text}");
        assert!(text.contains("Wakefield"), "the work is not named:\n{text}");
        assert!(text.contains("cited by 3 sentences"), "{text}");
        assert!(text.contains("cited by 1 sentence"), "singular not agreed:\n{text}");

        // It LEADS: above the deterministic consistency checks, and far above
        // anything the model produced.
        let retr = text.find("Retracted sources").expect("section");
        let evidence = text
            .find("The source passages behind each cited claim")
            .expect("evidence section");
        assert!(retr < evidence, "a model finding preceded a retraction:\n{text}");
        if let Some(cons) = text.find("Consistency checks") {
            assert!(retr < cons, "retraction sat below the consistency checks:\n{text}");
        }

        // It says it is NOT a model judgement, and it does not claim the rest
        // are clean.
        assert!(text.contains("no language model was involved"), "{text}");
        assert!(text.contains("never checked is not listed and is not clean"), "{text}");

        // And it is absent when there are none — no "0 retracted" reassurance,
        // which would read as a clean bill the check cannot give.
        let none = compose_audit(&model());
        assert!(!all_text(&none).contains("Retracted sources"), "empty section rendered");
    }

    /// §11 D108. The whole contract, in one place: the support verdict is
    /// recorded but never rendered, the decomposition and the passages are, and
    /// the report states the measurement that justifies the withdrawal.
    #[test]
    fn the_support_verdict_is_withheld_and_the_evidence_is_promoted() {
        let mut m = model();
        m.supported = vec![ReportItem {
            seq: 7,
            page: Some(3),
            sentence: "GoEmotions reports 46% macro-F1 over 27 categories.".into(),
            // Recorded — the export and the counts keep it.
            verdict: Some("weak".into()),
            explanation: Some("The source states 46% over 27 categories.".into()),
            evidence: vec![ReportEvidence {
                chunk_id: "c32".into(),
                page: Some(1),
                quote: "we achieve an average F1-score of 46% over 27 emotion categories".into(),
            }],
            claim_elements: vec![
                ClaimElement { element: "46% macro-F1".into(), status: "found".into() },
                ClaimElement { element: "27 categories".into(), status: "found".into() },
                ClaimElement { element: "95% accuracy".into(), status: "absent".into() },
            ],
            ..Default::default()
        }];
        let blocks = compose_audit(&m);
        let text = all_text(&blocks);

        // NOT rendered: the five-class verdict, in any of its forms.
        assert!(
            !blocks.iter().any(|b| matches!(b, Block::Badge { text, .. } if text == "weak")),
            "the verdict is still a badge:\n{text}"
        );
        assert!(!text.contains("/ 100"), "a score returned:\n{text}");
        assert!(!text.contains("held up"), "verdict language returned:\n{text}");

        // RENDERED: the passage, its page, and the decomposition.
        assert!(text.contains("46% over 27 emotion categories"), "the passage is missing:\n{text}");
        assert!(text.contains("p.1"), "the page link is missing:\n{text}");
        assert!(text.contains("What the sentence claims, part by part"), "{text}");
        assert!(text.contains("46% macro-F1 — in the source"), "{text}");
        assert!(text.contains("95% accuracy — NOT in the passages read"), "{text}");
        // The marks are the model's, and the report says so.
        assert!(text.contains("not Gaply's conclusion"), "{text}");

        // And the measurement that justifies all of the above is STATED.
        assert!(text.contains("14 of 14"), "the measurement is not disclosed:\n{text}");
        assert!(text.contains("does not grade"), "{text}");

        // The verdict survives in the MODEL, so the measurement stays repeatable.
        assert_eq!(m.supported[0].verdict.as_deref(), Some("weak"));
    }

    /// §11 D108. The attention list is GONE, and this asserts its absence.
    ///
    /// It was "The N checked claims that most need your attention", ordered by
    /// `attention_rank`, which read the support verdict. With a constant `weak`
    /// it listed every checked claim, each bullet printing `[weak]` — a ranking
    /// with nothing ranking it, under a heading asserting these were the worst.
    #[test]
    fn no_attention_ranking_is_rendered_from_the_support_verdict() {
        let mut m = model();
        m.supported = vec![
            ReportItem {
                seq: 1,
                sentence: "One weak claim.".into(),
                verdict: Some("weak".into()),
                ..Default::default()
            },
            ReportItem {
                seq: 2,
                sentence: "Another weak claim.".into(),
                verdict: Some("contradicts".into()),
                ..Default::default()
            },
        ];
        let text = all_text(&compose_audit(&m));
        assert!(!text.contains("your attention"), "the attention list came back:\n{text}");
        // And no bullet prints the verdict in brackets, which is how that list
        // put the withdrawn grade in front of the reader.
        assert!(!text.contains("[weak]"), "a bracketed verdict survived:\n{text}");
        assert!(!text.contains("[contradicts]"), "a bracketed verdict survived:\n{text}");
        // The sentences themselves are still reported.
        assert!(text.contains("One weak claim."), "{text}");
    }

    /// §11 D93. Page 1 answers "how did it go?", not only "what was run".
    #[test]
    fn the_cover_carries_the_answer() {
        let mut m = model();
        m.supported = (0..10)
            .map(|i| ReportItem {
                seq: i,
                verdict: Some(if i < 2 { "weak" } else { "strong" }.into()),
                ..Default::default()
            })
            .collect();
        let blocks = compose_audit(&m);
        let Some(Block::Cover { headline, .. }) = blocks.first() else {
            panic!("no cover block");
        };
        let (text, _) = headline.as_ref().expect("the cover has no headline");
        // §11 D108. The cover used to read "80 / 100 — 8 of 10 checked claims
        // held up". Every input to that came from the support verdict, which
        // is a constant, so it said 0/100 for every real manuscript. It now
        // states what was actually done.
        assert!(!text.contains("/ 100"), "the cover still carries a score: {text}");
        assert!(!text.contains("held up"), "the cover still grades: {text}");
        assert!(text.contains("10 cited claims"), "{text}");

        // With nothing checkable it says so. (The old test used `model()`,
        // which has one supported item and used to fall through to "too few to
        // score" — a branch that no longer exists, so it must now actually be
        // empty to exercise this.)
        let mut none = model();
        none.supported.clear();
        let empty_blocks = compose_audit(&none);
        let Some(Block::Cover { headline, .. }) = empty_blocks.first() else {
            panic!("no cover")
        };
        assert!(headline.as_ref().unwrap().0.contains("No claim could be checked"), "{headline:?}");
    }

    /// §11 D89. A suggestion must not arrive dressed as a finding.
    ///
    /// `emit_finding` gives every item a `Block::Badge` carrying its verdict, so
    /// a 43%-precision suggestion wore the same badge, at the same weight, as a
    /// `strong` verdict backed by a quoted passage. At a glance they were the
    /// same kind of statement.
    #[test]
    fn a_suggestion_carries_no_verdict_badge_and_a_finding_still_does() {
        let mut m = model();
        m.supported = (0..10)
            .map(|i| ReportItem {
                seq: i,
                sentence: format!("Checked claim {i}."),
                // §11 D108. The evidence side is distinguished by QUOTING the
                // source, so these must carry a passage for the distinction to
                // be the thing under test.
                evidence: vec![ReportEvidence {
                    chunk_id: format!("c{i}"),
                    page: Some(2),
                    quote: "A passage from the cited source.".into(),
                }],
                verdict: Some("strong".into()),
                ..Default::default()
            })
            .collect();
        m.needs_citation = vec![ReportItem {
            seq: 77,
            sentence: "An uncited assertion about the world.".into(),
            verdict: Some("needs_citation".into()),
            reason: Some("States a fact a reader would want to check.".into()),
            ..Default::default()
        }];

        let blocks = compose_audit(&m);

        // §11 D108. NEITHER side carries a verdict badge now: the support
        // verdict was withdrawn after measuring as a constant. D89's original
        // worry — that the fix would flatten both into the same
        // undifferentiated grey — is answered below, not by a badge.
        assert!(
            !blocks.iter().any(|b| matches!(b, Block::Badge { text, .. } if text == "strong")),
            "the withdrawn verdict is still rendered as a badge"
        );
        // §11 D128. There is no advisory side left to badge — the stronger
        // check is that nothing of it is rendered.
        assert!(
            !blocks
                .iter()
                .any(|b| matches!(b, Block::Badge { text, .. } if text.contains("needs citation"))),
            "a suggestion is still wearing a verdict badge"
        );

        // THE DISTINCTION THAT MATTERS, and it is stronger than a badge: the
        // evidence side quotes source text and says the passage IS the finding;
        // the advisory side says nothing was checked against any source.
        let text = all_text(&blocks);
        assert!(text.contains("read this and judge it yourself"), "{text}");
        // §11 D128: the advisory sentence and its caveat are both gone.
        assert!(
            !text.contains("An uncited assertion about the world."),
            "a retired advisory item reached the report:\n{text}"
        );
        assert!(text.contains("does not grade"), "the withdrawal is not stated:\n{text}");
    }



    /// §11 D108. NO score is rendered, at any n. The old rule was "too few to
    /// score is said rather than rendered as 0"; the rule now is that the
    /// support verdict cannot produce a score at any sample size, because it
    /// returns the same value every time.
    #[test]
    fn no_health_score_is_rendered_at_any_size() {
        let mut m = model();
        m.supported = vec![ReportItem {
            seq: 1,
            sentence: "The only checked claim.".into(),
            verdict: Some("weak".into()),
            ..Default::default()
        }];
        let text = all_text(&compose_audit(&m));
        assert!(!text.contains("/ 100"), "rendered a score anyway:\n{text}");
        assert!(text.contains("does not score"), "the withdrawal is not explained:\n{text}");

        // And with plenty of checked claims it is STILL absent — this is the
        // half the old test could not express, because 20 items used to score.
        m.supported = (0..20)
            .map(|i| ReportItem {
                seq: i,
                sentence: format!("Checked claim {i}."),
                verdict: Some("strong".into()),
                ..Default::default()
            })
            .collect();
        let text = all_text(&compose_audit(&m));
        assert!(!text.contains("/ 100"), "twenty items brought the score back:\n{text}");
        assert!(!text.contains("held up"), "the verdict language returned:\n{text}");

        // And with nothing checked at all, it says so.
        m.supported.clear();
        let text = all_text(&compose_audit(&m));
        assert!(text.contains("No claim could be checked against its cited source"), "{text}");
    }

    #[test]
    fn the_summary_leads_with_the_worst_CHECKED_items_first() {
        // The old report opened at sentence 0 in document order, which put a
        // contradiction on page 9 below thirty routine items. §11 D78 adds the
        // second property: the list is EVIDENCE-BACKED, so a suggestion cannot
        // appear under "most need your attention".
        //
        // §11 D128 settles that by construction — the advisory lane is retired
        // and renders nothing anywhere. The item is still set here, and now
        // asserted ABSENT from the whole report, so this also pins the
        // retirement rather than merely the ordering.
        let mut m = model();
        m.supported = vec![
            ReportItem {
                seq: 2,
                sentence: "An ordinary supported claim.".into(),
                verdict: Some("strong".into()),
                ..Default::default()
            },
            ReportItem {
                seq: 40,
                sentence: "The contradicted claim.".into(),
                verdict: Some("contradicts".into()),
                ..Default::default()
            },
            ReportItem {
                seq: 41,
                sentence: "The weak claim.".into(),
                verdict: Some("weak".into()),
                ..Default::default()
            },
        ];
        m.needs_citation = vec![ReportItem {
            seq: 9,
            sentence: "An uncited assertion.".into(),
            verdict: Some("needs_citation".into()),
            ..Default::default()
        }];
        let text = all_text(&compose_audit(&m));

        // Most severe first among the CHECKED items.
        let worst = text.find("The contradicted claim.").expect("worst item absent");
        let next = text.find("The weak claim.").expect("second item absent");
        assert!(worst < next, "severity order not respected:\n{text}");

        // The advisory item is NOT in the attention list — it appears only in
        // its own section, after the evidence-backed one.
        //
        // §11 D108. The attention list is gone with the verdict that ordered
        // it, so the ranking half of this test is gone too. What survives is
        // D78's property, which never depended on the verdict: evidence-backed
        // material LEADS, and an advisory suggestion appears only inside its
        // own clearly-labelled section.
        // §11 D128. The advisory section is retired, so the half of this test
        // that checked its ORDER is replaced by the stronger claim: it is not
        // in the report at all, under any heading.
        assert!(
            !text.contains("suggestions, not findings") && !text.contains("An uncited assertion."),
            "a retired advisory item reached the report:\n{text}"
        );

        let detail = text
            .find("The source passages behind each cited claim")
            .expect("no evidence section");
        assert!(
            text.find("An ordinary supported claim.").unwrap() > detail,
            "a checked item appeared before its own section:\n{text}"
        );
        // D78's ordering property had a second half — evidence-backed findings
        // lead the advisory ones. With the advisory section gone there is
        // nothing left for them to lead, which is the retirement doing the work
        // the ordering used to.
    }

    #[test]
    fn an_empty_section_says_none_found_rather_than_vanishing() {
        // A missing section reads as "not checked"; "None found" is the fact.
        let text = all_text(&compose_audit(&model()));
        assert!(text.contains("None found."), "{text}");
    }
}
