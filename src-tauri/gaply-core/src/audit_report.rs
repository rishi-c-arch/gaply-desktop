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

/// Lower sorts first: the reader meets what most needs them at the top, not
/// sentence 0 in document order.
fn attention_rank(item: &ReportItem) -> u8 {
    match item.verdict.as_deref().unwrap_or("") {
        "contradicts" => 0,
        "weak" => 1,
        "insufficient_evidence" => 2,
        "partial" => 3,
        "needs_citation" => 4,
        _ => 9,
    }
}

/// A single number for "how much of this needs work", 0-100.
///
/// Deliberately simple and stated in the report: checked-and-supported counts
/// full, an item needing attention counts nothing, and an item that could not
/// be checked counts nothing either but is reported separately so the score is
/// never mistaken for a clean bill. A score with a hidden formula is worse than
/// no score, so the denominator is printed beside it.
pub fn health_score(m: &AuditReportModel) -> Option<u32> {
    let attention = m.supported.iter().filter(|i| attention_rank(i) < 9).count();
    let judged = m.supported.len();
    // Fewer than this and a percentage is noise dressed as a measurement: with
    // two checked claims, one weak verdict is "50%".
    if judged < MIN_SCOREABLE {
        return None;
    }
    Some((((judged - attention) as f64 / judged as f64) * 100.0).round() as u32)
}

/// The fewest evidence-backed findings a /100 score may be computed from.
pub const MIN_SCOREABLE: usize = 10;

/// `citation_need`'s MEASURED precision, stated in the report so the advisory
/// claim is checkable (§11 D78).
///
/// 41 cold labelled cases, `citation_need-v4`, 3B on Metal: recall 82%,
/// precision 43% — 14 of 32 flagged sentences actually needed a citation. These
/// are numbers about a MODEL and they expire; re-measure before changing them.
pub const ADVISORY_RECALL_PCT: u32 = 82;
pub const ADVISORY_PRECISION_PCT: u32 = 43;

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

/// Emit ONE SUGGESTION (§11 D89).
///
/// Deliberately not `emit_finding`. A suggestion is not a finding: nothing was
/// checked against any source, and the measured precision is 43% — so it must
/// not arrive in the badge a `strong` verdict wears, at the weight a verdict
/// carries.
///
/// The rate rides with the ITEM. The section's caveat is a paragraph at the
/// top, and a reader who lands on item 19 never sees it; by then the label is
/// the only thing on the page telling them what this is.
fn emit_suggestion(out: &mut Vec<Block>, item: &ReportItem, has_pages: bool) {
    out.push(heading(format!("Sentence {} · {}", item.seq, locator(item, has_pages)), 3));
    // NO Block::Badge. A badge is the report's verdict vocabulary and this is
    // not a verdict. NO severity either — it is a constant (§11 D82) and would
    // read as triage.
    out.push(para(
        "suggestion · not checked against any source · about 4 in 10 of these are real",
    ));
    out.push(para(format!("\u{201c}{}\u{201d}", item.sentence.trim())));
    if let Some(r) = &item.reason {
        out.push(para(r.trim().to_string()));
    }
}

/// The advisory list, emitted with `emit_suggestion` (§11 D89).
fn emit_suggestions(out: &mut Vec<Block>, title: &str, blurb: &str, items: &[ReportItem], has_pages: bool) {
    out.push(heading(title, 1));
    if items.is_empty() {
        out.push(para(format!("{blurb} None found.")));
        return;
    }
    out.push(para(blurb));
    let mut sorted: Vec<&ReportItem> = items.iter().collect();
    sorted.sort_by_key(|i| i.seq);
    for item in sorted {
        emit_suggestion(out, item, has_pages);
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
    sorted.sort_by_key(|i| (attention_rank(i), i.seq));
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

/// A one-line proportional bar.
///
/// Deliberately built from a repeated ASCII character rather than block-drawing
/// glyphs: the PDF renderer encodes WinAnsi, and a bar that folds to `?` in one
/// of the two output formats is worse than a plain one that survives both.
fn bar(n: usize, total: usize) -> String {
    if total == 0 {
        return String::new();
    }
    let width = ((n as f64 / total as f64) * 24.0).round() as usize;
    format!(
        "{:<24} {n:>4}  {:>3}%",
        "=".repeat(width.max(usize::from(n > 0))),
        (n as f64 * 100.0 / total as f64).round() as u32
    )
}

/// The items a reader should look at first, across every category.
///
/// Ordering by document position put sentence 0 at the top of a 65-item report
/// and buried the one contradiction on page 9. Severity decides the order here;
/// document position only breaks ties.
fn attention_list(m: &AuditReportModel) -> Vec<&ReportItem> {
    // EVIDENCE-BACKED ONLY (§11 D78). An advisory suggestion at 43% precision
    // must not appear in a list headed "most need your attention" — that is a
    // verdict's framing, and fewer than half of them are real.
    let mut v: Vec<&ReportItem> = m
        .supported
        .iter()
        .filter(|i| attention_rank(i) < 9)
        .collect();
    v.sort_by_key(|i| (attention_rank(i), i.seq));
    v
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
            // "answered", not "checked": see `AuditReportModel::checked`.
            ("Sentences answered".to_string(), m.checked.to_string()),
        ],
    });

    // ---- AT A GLANCE ------------------------------------------------------
    // The first page answers "how bad is it, and what do I do first". The
    // per-item detail is reference material behind it.
    let judged = m.supported.len() + m.needs_citation.len();
    let attention = attention_list(m);
    let score = health_score(m);

    out.push(heading("At a glance", 1));

    // THE SCORE IS ABOUT EVIDENCE-BACKED FINDINGS ONLY (§11 D78).
    //
    // It used to average in `citation_need`, whose measured precision is 43% —
    // fewer than half its flags are real. Folding an advisory signal into a
    // health number makes the number advisory too, and it was not labelled that
    // way: 65 declinations produced "97/100" on a paper with a real miscitation.
    let checked = m.supported.len();
    let failing = m.supported.iter().filter(|i| attention_rank(i) < 9).count();
    match health_score(m) {
        Some(score) => {
            out.push(Block::Badge {
                text: format!("health {score} / 100"),
                tone: match score {
                    80..=100 => Tone::Good,
                    50..=79 => Tone::Warn,
                    _ => Tone::Bad,
                },
            });
            out.push(para(format!(
                "{score}/100 — of {checked} claims checked against their cited source, {} held up \
                 and {failing} did not. The score covers ONLY claims Gaply could check against real \
                 evidence.",
                checked - failing,
            )));
        }
        None if checked == 0 => {
            out.push(Block::Badge { text: "no score".into(), tone: Tone::Neutral });
            out.push(para(
                "No score: not one claim could be checked against its cited source, so there is \
                 nothing to score. The sections below say why, and what would make a check \
                 possible.",
            ));
        }
        None => {
            out.push(Block::Badge { text: "too few to score".into(), tone: Tone::Neutral });
            out.push(para(format!(
                "No score: only {checked} claim{} could be checked against a cited source, and a \
                 percentage from that few would be noise rather than a measurement. {} held up, \
                 {failing} did not — the findings themselves are below.",
                if checked == 1 { "" } else { "s" },
                checked - failing,
            )));
        }
    }
    out.push(Block::Note {
        text: format!(
            "The score does NOT include the “worth a second look” suggestions. Those come from a \
             language model reading each sentence on its own, and on a labelled test set it \
             flagged {ADVISORY_RECALL_PCT}% of the sentences that genuinely needed a citation — \
             but only {ADVISORY_PRECISION_PCT}% of what it flagged actually did. They are a prompt \
             to look, not a finding, and averaging them into a score would make the score a guess."
        ),
    });

    // The breakdown. Evidence-backed and advisory are SEPARATE BARS, never
    // summed — a chart that adds a 43%-precision suggestion to a checked
    // finding is the same conflation the score just removed (§11 D78).
    let total_bar = checked + m.unverifiable.len() + m.failed.len() + m.needs_citation.len();
    out.push(para("The whole manuscript, proportionally:"));
    out.push(bullet(
        format!("checked, did not hold up  {}", bar(failing, total_bar)),
        0,
    ));
    out.push(bullet(
        format!("checked, held up          {}", bar(checked - failing, total_bar)),
        0,
    ));
    out.push(bullet(
        format!("could not be checked      {}", bar(m.unverifiable.len(), total_bar)),
        0,
    ));
    out.push(bullet(
        format!("suggestions only          {}", bar(m.needs_citation.len(), total_bar)),
        0,
    ));
    if !m.failed.is_empty() {
        out.push(bullet(format!("not judged                {}", bar(m.failed.len(), total_bar)), 0));
    }

    if !attention.is_empty() {
        const TOP: usize = 10;
        out.push(heading(
            format!(
                "The {} checked claim{} that most need your attention",
                attention.len().min(TOP),
                if attention.len().min(TOP) == 1 { "" } else { "s" }
            ),
            2,
        ));
        out.push(para(
            "Most serious first, and EVIDENCE-BACKED — every one was checked against the source \
             it cites. The suggestions are listed separately and are not ranked here.",
        ));
        for it in attention.iter().take(TOP) {
            out.push(bullet(
                format!(
                    "[{}] {} · sentence {} — “{}”",
                    it.verdict.as_deref().unwrap_or("?").replace('_', " "),
                    locator(it, m.has_pages),
                    it.seq,
                    it.sentence.trim().chars().take(96).collect::<String>()
                ),
                0,
            ));
        }
        if attention.len() > TOP {
            out.push(para(format!(
                "…and {} more in the sections below.",
                attention.len() - TOP
            )));
        }
    } else if judged > 0 {
        out.push(para(
            "Nothing was flagged for attention. That is a statement about the sentences Gaply              could judge — read the not-checkable section before treating it as a clean bill.",
        ));
    }

    // The provenance line is not decoration. A reader months from now needs to
    // know which model produced these verdicts and that it ran locally.
    out.push(Block::Note {
        text: format!(
            "Every verdict in this report was produced on this machine by {} ({}). \
             No manuscript text was sent anywhere.",
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
    if !m.verdict_counts.is_empty() {
        out.push(para("What the model concluded:"));
        for (v, n) in &m.verdict_counts {
            out.push(bullet(format!("{:<26} {}", v.replace('_', " "), bar(*n, judged.max(1))), 0));
        }
    }
    if !m.skipped_reasons.is_empty() {
        out.push(para("Not judged, and why:"));
        for (r, n) in &m.skipped_reasons {
            out.push(bullet(format!("{n} — {r}"), 0));
        }
    }

    // Order of the detail sections follows what the reader can act on: the
    // library gap is one batch action away from being fixed, so it comes before
    // the per-sentence reading.
    // ORDER (§11 D78): evidence-backed findings LEAD. `citation_support` checks
    // prose against a real source and is the half worth trusting; the advisory
    // list follows it rather than competing with it for the reader's attention.
    out.push(Block::PageBreak);
    emit_section(
        &mut out,
        "Claims checked against their source",
        "Each sentence below cites a source Gaply could read. The quoted passage is what the \
         verdict rests on — read it before the verdict. THIS IS THE EVIDENCE-BACKED SECTION.",
        &m.supported,
        m.has_pages,
    );

    out.push(Block::PageBreak);
    emit_blocked_sources(&mut out, &m.unverifiable, m.has_pages);

    // ONLY the sentences actually flagged. `needs_citation` carries every
    // judged uncited sentence, and on a real manuscript 65 of 65 came back
    // "no citation needed" — listing those under "worth a second look" would
    // present 65 non-suggestions as a list of things to review (§11 D78).
    let flagged: Vec<ReportItem> = m
        .needs_citation
        .iter()
        .filter(|i| i.verdict.as_deref() == Some("needs_citation"))
        .cloned()
        .collect();
    out.push(Block::PageBreak);
    emit_suggestions(
        &mut out,
        "Worth a second look — suggestions, not findings",
        "SUGGESTIONS. These sentences carry no citation and a language model thought they might \
         need one. It is right slightly under half the time, so treat this as a list to skim, not \
         a list of problems. Nothing here was checked against any source.",
        &flagged,
        m.has_pages,
    );

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
            Block::Cover { title, subtitle, meta } => format!(
                "{title} {subtitle} {}",
                meta.iter().map(|(k, v)| format!("{k}={v}")).collect::<Vec<_>>().join(" ")
            ),
            Block::Heading { text, .. } => text.clone(),
            Block::Paragraph { text } => text.clone(),
            Block::Bullet { text, .. } => text.clone(),
            Block::Note { text } => text.clone(),
            Block::Badge { text, .. } => text.clone(),
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
            skipped_reasons: vec![("a table row or a figure caption".into(), 5)],
            supported: vec![item_with_evidence()],
            needs_citation: vec![],
            unverifiable: vec![],
            failed: vec![],
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
        let missing: Vec<&str> = GAPLY_REPORT_MARKERS
            .iter()
            .copied()
            .filter(|m| *m != "PublishReady" && !found.contains(m))
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

    /// §11 D65. A Word file has no pages, but it HAS paragraphs — and a
    /// paragraph ordinal is a locator rather than an absence. 84 items reading
    /// "no page numbers" told a reader where nothing was.
    #[test]
    fn an_unpaginated_source_locates_findings_by_paragraph() {
        let mut m = model();
        m.has_pages = false;
        m.needs_citation = vec![ReportItem {
            seq: 4,
            page: None,
            paragraph: Some(12),
            sentence: "An uncited assertion.".into(),
            verdict: Some("needs_citation".into()),
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
        let without = health_score(&m);
        assert_eq!(without, Some(80));

        // Fifty advisory suggestions must not shift it by a point.
        m.needs_citation = (0..50)
            .map(|i| ReportItem {
                seq: 100 + i,
                sentence: format!("Suggestion {i}."),
                verdict: Some("needs_citation".into()),
                ..Default::default()
            })
            .collect();
        assert_eq!(health_score(&m), without, "advisory items moved the score");

        let text = all_text(&compose_audit(&m));
        // The score says what it covers.
        assert!(text.contains("checked against their cited source"), "{text}");
        // The advisory list is labelled as suggestions, never as findings.
        assert!(text.contains("suggestions, not findings"), "{text}");
        assert!(!text.contains("Sentences that may need a citation"), "old verdict heading:\n{text}");
        // And the measured precision is stated, so the claim is checkable.
        assert!(text.contains("43%"), "precision not disclosed:\n{text}");
        assert!(text.contains("82%"), "recall not disclosed:\n{text}");
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

        // The evidence-backed side KEEPS its badge — the fix must not flatten
        // both into the same undifferentiated grey.
        assert!(
            blocks.iter().any(|b| matches!(b, Block::Badge { text, .. } if text == "strong")),
            "the checked findings lost their verdict badge"
        );
        // The advisory side has none.
        assert!(
            !blocks
                .iter()
                .any(|b| matches!(b, Block::Badge { text, .. } if text.contains("needs citation"))),
            "a suggestion is still wearing a verdict badge"
        );
    }

    /// §11 D89. The rate must ride with the ITEM, not only sit in the intro.
    ///
    /// A reader who lands on item 19 never sees the paragraph at the top of the
    /// section, and by then the label is the only thing on the page telling
    /// them what they are looking at.
    #[test]
    fn every_suggestion_says_what_it_is_where_it_is() {
        let mut m = model();
        m.needs_citation = (0..3)
            .map(|i| ReportItem {
                seq: 100 + i,
                sentence: format!("Uncited assertion {i}."),
                verdict: Some("needs_citation".into()),
                ..Default::default()
            })
            .collect();
        let text = all_text(&compose_audit(&m));

        // Once per item, not once per section.
        assert_eq!(
            text.matches("not checked against any source").count(),
            3,
            "the label must appear beside EVERY suggestion:\n{text}"
        );
        assert_eq!(
            text.matches("about 4 in 10 of these are real").count(),
            3,
            "the measured rate must ride with every item:\n{text}"
        );
        // And no severity anywhere: it is a constant (§11 D82) and would read
        // as triage.
        for banned in ["severity", "HIGH", "MEDIUM"] {
            assert!(!text.contains(banned), "{banned:?} leaked into the report:\n{text}");
        }
    }

    /// §11 D78. The advisory section lists only what was actually FLAGGED.
    ///
    /// `needs_citation` carries every judged uncited sentence, and on a real
    /// manuscript 65 of 65 came back "no citation needed". Listing those under
    /// "worth a second look" presents 65 non-suggestions as a review list — the
    /// exact over-claim this section exists to avoid.
    #[test]
    fn the_advisory_section_lists_only_flagged_sentences() {
        let mut m = model();
        m.needs_citation = vec![
            ReportItem {
                seq: 1,
                sentence: "The model flagged this one.".into(),
                verdict: Some("needs_citation".into()),
                ..Default::default()
            },
            ReportItem {
                seq: 2,
                sentence: "The model said this needs nothing.".into(),
                verdict: Some("no_citation_needed".into()),
                ..Default::default()
            },
        ];
        let text = all_text(&compose_audit(&m));
        assert!(text.contains("The model flagged this one."), "flagged item missing:\n{text}");
        assert!(
            !text.contains("The model said this needs nothing."),
            "a NOT-flagged sentence appears under 'worth a second look':\n{text}"
        );
    }

    /// Too few checked claims to score is SAID, not rendered as 0 or 100.
    #[test]
    fn too_few_checked_claims_yields_no_score_rather_than_a_misleading_one() {
        let mut m = model();
        m.supported = vec![ReportItem {
            seq: 1,
            sentence: "The only checked claim.".into(),
            verdict: Some("weak".into()),
            ..Default::default()
        }];
        assert_eq!(health_score(&m), None, "a percentage from n=1 is noise");
        let text = all_text(&compose_audit(&m));
        assert!(text.contains("would be noise rather than a measurement"), "{text}");
        assert!(!text.contains("0 / 100"), "rendered a score anyway:\n{text}");

        // And with nothing checked at all, it says so.
        m.supported.clear();
        let text = all_text(&compose_audit(&m));
        assert!(text.contains("not one claim could be checked"), "{text}");
    }

    #[test]
    fn the_summary_leads_with_the_worst_CHECKED_items_first() {
        // The old report opened at sentence 0 in document order, which put a
        // contradiction on page 9 below thirty routine items. §11 D78 adds the
        // second property: the list is EVIDENCE-BACKED, so a 43%-precision
        // suggestion cannot appear under "most need your attention".
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
        let attention_hdr = text.find("most need your attention").expect("no attention list");
        let advisory_hdr = text.find("suggestions, not findings").expect("no advisory section");
        let uncited = text.find("An uncited assertion.").expect("advisory item absent");
        assert!(
            uncited > advisory_hdr,
            "an advisory suggestion appeared before its own section — it is being \
             presented as a finding:\n{text}"
        );
        assert!(attention_hdr < advisory_hdr, "advisory section preceded the attention list");

        // A clean checked item is not promoted into the attention list.
        let detail = text.find("Claims checked against their source").expect("no detail section");
        assert!(
            text.find("An ordinary supported claim.").unwrap() > detail,
            "a clean item was promoted into the attention list:\n{text}"
        );
        // Evidence-backed findings LEAD the advisory ones (§11 D78).
        assert!(detail < advisory_hdr, "the advisory section preceded the checked findings");
    }

    #[test]
    fn an_empty_section_says_none_found_rather_than_vanishing() {
        // A missing section reads as "not checked"; "None found" is the fact.
        let text = all_text(&compose_audit(&model()));
        assert!(text.contains("None found."), "{text}");
    }
}
