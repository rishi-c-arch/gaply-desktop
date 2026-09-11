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

use crate::report_compose::{Align, Block, Tone};

/// One passage the model rested a verdict on. `page` comes from the store, not
/// from the model — D18's validated half.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReportEvidence {
    pub chunk_id: String,
    pub page: Option<u32>,
    /// The passage itself. This is what makes the prose checkable.
    ///
    /// CLEANED for display by `quote_clean` (§11 D147): words broken across a
    /// PDF line break are rejoined, and a journal running header is replaced by
    /// a visible `[...]`. Nothing is selected, shortened or reordered.
    pub quote: String,
    /// What the MODEL said this chunk contributes, verbatim from its answer.
    ///
    /// §11 D147. The reader was handed a 380-word retrieval chunk with no
    /// indication of which part mattered, while the one field that says so was
    /// being discarded by the export. Leading with it is a DISPLAY change: it
    /// shows the model's own pointer and keeps the chunk underneath as the
    /// context it is. Selecting which sentences of the chunk matter would be
    /// re-deciding what supports the claim, which is the judgement this engine
    /// declines to let a model make silently.
    ///
    /// Empty when the task did not record one (v1 asks for `why`; only v2 asks
    /// for a verbatim quote).
    #[serde(default)]
    pub why: String,
    /// Whether `quote` had a run elided. The report says so where it shows.
    #[serde(default)]
    pub elided: bool,
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
    /// The source checked was an ABSTRACT, not the full text (§11 D138).
    ///
    /// Shown on the item, not filtered out of it: an abstract legitimately
    /// supports some claims, and refusing it would drop real checks. What it may
    /// not do is read as a full-text check, so it says which.
    pub abstract_only: bool,
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
/// §11 D140. Said in full on an item when it is the only one, and once for the
/// section when several items share it: forty words repeated six times is the
/// defect this report was redesigned to remove, and the fact is the same fact.
const ABSTRACT_ONLY_LONG: &str =
    "Only an ABSTRACT was available for the cited work, not its full text. An abstract carries \
     too little to quote for most claims, so this is a limit of what could be obtained rather \
     than a judgement about the sentence.";
const ABSTRACT_ONLY_SHORT: &str = "Checked against an ABSTRACT only, not the full text.";

/// `hoisted` names the caveats the SECTION has already stated once, so an item
/// does not repeat them (defect 1).
#[derive(Debug, Clone, Copy, Default)]
struct Hoisted {
    /// The abstract-only explanation is above this item.
    abstract_note: bool,
    /// Every item in the section carries the same reason, printed above.
    reason: bool,
}

fn emit_finding(out: &mut Vec<Block>, item: &ReportItem, has_pages: bool, hoisted: Hoisted) {
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

    // §11 D140. WHY it could not be judged, when the answer is the SOURCE rather
    // than the model.
    //
    // §11 D138 stopped an abstract-backed SUCCESS looking like a full-text one.
    // This is the same rule broken in the other direction: an abstract-backed
    // FAILURE looking like a model failure. Measured on a real run, 6 of 7 "could
    // not be judged" items had only an abstract to work from, and a reader seeing
    // "7 could not be judged" concludes the model is unreliable when the honest
    // statement is that six had too little to cite.
    //
    // The distinction was in the payload, survived into `m.failed`, and was
    // dropped HERE — the same shape as the export defaulting `abstract_only` to
    // false (§11 D138).
    if item.abstract_only {
        // DEFECT 1. When the section says it once, the item still has to be
        // IDENTIFIABLE as one of them, or the hoisted sentence applies to
        // nothing a reader can point at. A short marker does that in six words
        // instead of forty.
        out.push(para(if hoisted.abstract_note {
            ABSTRACT_ONLY_SHORT.to_string()
        } else {
            ABSTRACT_ONLY_LONG.to_string()
        }));
    }

    if let Some(r) = &item.reason {
        if !hoisted.reason {
            out.push(para(r.trim().to_string()));
        }
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
                format!("{} · {}: “{}”", e.chunk_id, page_label(e.page, has_pages), e.quote.trim()),
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
    //
    // §11 D138. An abstract-backed check SAYS SO, on the item, where a reader who
    // lands on item 19 sees it. An abstract legitimately supports some claims —
    // "GoEmotions reports 46% macro-F1" is in the abstract — so it is not
    // refused; it is labelled, because it must not read as a full-text check.
    out.push(para(if item.abstract_only {
        "passages located IN THE ABSTRACT ONLY. The full text was not available, so only \
         what the abstract states could be checked · the source text below is the finding · \
         Gaply does not grade how well it supports the sentence"
    } else {
        "passages located · the source text below is the finding · Gaply does not grade \
         how well it supports the sentence"
    }));
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
        out.push(para("From the cited source. Read this and judge it yourself:"));
        for e in &item.evidence {
            // §11 D147. The MODEL'S POINTER first, then the passage it points
            // into. The reader used to meet an undifferentiated 380-word
            // retrieval chunk with nothing saying which part mattered, while
            // this field was being discarded by the export.
            //
            // It leads, and it is labelled as the model's, because it is the
            // one thing here that a model wrote. The passage underneath is the
            // source's own words and is what the check rests on.
            if !e.why.is_empty() {
                out.push(para(format!(
                    "What the model points to in {}: {}",
                    e.chunk_id,
                    e.why.trim()
                )));
            }
            out.push(bullet(
                format!("{} · {}: \u{201c}{}\u{201d}", e.chunk_id, page_label(e.page, has_pages), e.quote.trim()),
                1,
            ));
        }
        // §11 D147. THE STANDING DISCLOSURE, printed with the passages rather
        // than buried at the end, because it is about the text directly above
        // it. De-hyphenation cannot be marked inline without making the quote
        // unreadable, so it is disclosed here instead; an elision IS marked,
        // and this says what the marker means.
        let any_elided = item.evidence.iter().any(|e| e.elided);
        out.push(Block::Note {
            text: format!(
                "These passages are machine-extracted from a PDF. Words broken across a line \
                 break have been rejoined, so a quote may differ from the page in that respect. \
                 {}If you intend to rely on a quote, check it against the source.",
                if any_elided {
                    "A [...] marks a journal running header or DOI line removed from inside the \
                     passage. "
                } else {
                    ""
                }
            ),
        });
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
            out.push(bullet(format!("{}: {mark}", el.element.trim()), 1));
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

    // DEFECT 1, applied to this section's two repeating caveats.
    let mut hoisted = Hoisted::default();
    if items.iter().filter(|i| i.abstract_only).count() >= 2 {
        out.push(Block::Note { text: ABSTRACT_ONLY_LONG.to_string() });
        hoisted.abstract_note = true;
    }
    // When every item failed the same way, the reason is a property of the
    // SECTION. Printing it per item made seven identical paragraphs.
    let reasons: Vec<String> =
        items.iter().filter_map(|i| i.reason.clone()).map(|r| r.trim().to_string()).collect();
    if reasons.len() == items.len() && items.len() >= 2 {
        let first = &reasons[0];
        if reasons.iter().all(|r| r == first) {
            out.push(Block::Note {
                text: format!("Every item below reports the same thing: {first}"),
            });
            hoisted.reason = true;
        }
    }

    // Most severe first, then document order within a severity, so the reader
    // meets what most needs them at the top of each section.
    let mut sorted: Vec<&ReportItem> = items.iter().collect();
    // §11 D108. Was `(attention_rank(i), i.seq)`. `attention_rank` read the
    // support verdict; the sections still using this emitter carry no verdict
    // at all, so document order is the honest one.
    sorted.sort_by_key(|i| i.seq);
    for item in sorted {
        emit_finding(out, item, has_pages, hoisted);
    }
}

/// One cited work that nothing could be checked against, and everything that
/// depends on it.
struct BlockedSource<'a> {
    /// The grouping KEY: the reference entry when one was parsed, else the
    /// resolution reason. Not what the reader is shown: see `cited_work_label`.
    entry: String,
    /// The parsed reference entry, when there was one.
    parsed_entry: Option<String>,
    items: Vec<&'a ReportItem>,
    next_step: Option<String>,
}

/// Group the unverifiable items BY SOURCE.
///
/// The list was per-sentence, which showed the same missing paper five times
/// and asked the reader to notice. Grouped, each row is one thing to fix and
/// says how many sentences it unblocks — which is the number that decides
/// whether it is worth fixing.
use crate::report_compose::trim_at_word;

/// DEFECT 1: lift an explanation shared by every row out of the rows.
///
/// One caveat ran to 34 words and was repeated on ~20 consecutive rows, which is
/// the same sentence filling a third of a page. A reason that applies to all rows
/// is a heading; only what DIFFERS belongs in a row.
///
/// Generic on purpose, so this works for the next repeated caveat too: it finds
/// the longest tail every string shares, at a word boundary, and returns it
/// once with the per-row remainders.
///
/// Returns `(None, unchanged)` unless there are at least two rows and the shared
/// tail is long enough to be worth hoisting. Hoisting three words would cost
/// clarity and save nothing.
fn hoist_shared_tail(reasons: &[String]) -> (Option<String>, Vec<String>) {
    /// Below this, repetition is cheaper than an extra sentence of preamble.
    const MIN_TAIL_CHARS: usize = 40;
    if reasons.len() < 2 {
        return (None, reasons.to_vec());
    }
    let first: Vec<char> = reasons[0].chars().collect();
    let mut shared = 0usize;
    'outer: for n in 1..=first.len() {
        let tail: String = first[first.len() - n..].iter().collect();
        for r in reasons {
            if !r.ends_with(&tail) || r.chars().count() == n {
                break 'outer;
            }
        }
        shared = n;
    }
    if shared < MIN_TAIL_CHARS {
        return (None, reasons.to_vec());
    }
    let mut tail: String = first[first.len() - shared..].iter().collect();
    // Pull the cut back to a word boundary so the hoisted sentence starts on a
    // word and the remainders do not end mid-word.
    if let Some(i) = tail.find(' ') {
        if !tail.starts_with(' ') {
            tail = tail[i + 1..].to_string();
        }
    }
    let tail = tail.trim().to_string();
    if tail.chars().count() < MIN_TAIL_CHARS {
        return (None, reasons.to_vec());
    }
    let rest = reasons
        .iter()
        .map(|r| {
            let keep = r.len() - tail.len();
            r[..keep]
                .trim_end()
                .trim_end_matches([',', ';', ':'])
                .trim_end()
                .to_string()
        })
        .collect();
    (Some(tail), rest)
}

/// A short status for a row, from the reason the model layer recorded.
///
/// The reason says what happened in a sentence; a table column needs three
/// words. Both are kept: the status is the cell, the full reason is hoisted
/// above the table when every row shares it.
fn status_phrase(reason: &str) -> &'static str {
    let r = reason.to_lowercase();
    if r.contains("no library work matches") || r.contains("not in library") {
        "Not in your library"
    } else if r.contains("not indexed or not embedded") {
        "Linked, not yet indexed"
    } else if r.contains("no indexed document is linked") || r.contains("source is not indexed") {
        "In library, no file linked"
    } else if r.contains("numeric citation style") {
        "Reference list not readable"
    } else {
        "Could not be checked"
    }
}

/// DEFECT 1: what to call the cited work in a table row.
///
/// A row's first column must name the WORK, in a few words a reader recognises.
/// It was printing the whole resolution sentence, because an unmatched
/// author-year citation has no parsed reference entry and `group_by_source`
/// falls back to the reason. The reason is 34 words and identical on every row,
/// so the column held one repeated paragraph and no identifying information.
///
/// In order of preference:
///   1. the parsed reference entry, which is the work as the manuscript states it;
///   2. the marker the reason quotes (`\u{201C}Banerjee, 2021\u{201D}`), which is all
///      that exists for a citation nothing matched;
///   3. the reason itself, trimmed, when it is neither.
fn cited_work_label(entry: &str, parsed_entry: Option<&str>) -> String {
    if let Some(e) = parsed_entry {
        return trim_at_word(e, 120);
    }
    if let Some(q) = first_quoted(entry) {
        return q;
    }
    trim_at_word(entry, 90)
}

/// §11 D145. Is this "cited work" probably not a work at all?
///
/// Five of twenty rows in a real report were extraction artifacts rather than
/// missing sources: `RBV, 1991` (a theory acronym), `Authority, 2025` (from
/// "The Financial Services Authority"), `Weiner's, 2009` (a possessive),
/// `Dubai, 2006` (from a co-citation), and `Kutzins, 2013` two rows below
/// `Kutzin, 2013`. A reader who finds four nonsense rows in twenty stops
/// believing the other sixteen, and this table exists to say what to fetch.
///
/// The strongest signal costs nothing: **the report has already examined these
/// markers** in its deterministic consistency section. Repeating them a page
/// later as confident fetch targets is the report contradicting itself.
///
/// Returns the reason to SHOW, not a boolean, because the row must say why.
fn artifact_reason(label: &str, consistency: &[crate::consistency::ConsistencyFinding]) -> Option<String> {
    let (surname, year) = match label.rsplit_once(',') {
        Some((s, y)) => (s.trim(), y.trim()),
        None => (label.trim(), ""),
    };
    if surname.is_empty() {
        return None;
    }

    // 1. The deterministic checks already flagged this exact marker.
    //
    // Matched against the finding's SUBJECT (the marker it quotes first), as a
    // WHOLE WORD. Matching anywhere in the message flagged `Kutzin, 2013` — a
    // real work — because the finding about `Kutzins (2013)` quotes the entry it
    // was matched to, and "Kutzin" is a substring of "Kutzins". Calling a
    // researcher's genuine citation junk is the one thing this must never do.
    for f in consistency {
        let subject = first_quoted(&f.message).unwrap_or_else(|| f.message.clone());
        if !contains_word(&subject, surname) {
            continue;
        }
        if !year.is_empty() && !subject.contains(year) {
            continue;
        }
        match f.kind.as_str() {
            "uncertain-reference-match" => {
                return Some(
                    "may be a co-author or a mis-split name: the consistency checks matched it to \
                     an entry listed under a different name"
                        .to_string(),
                )
            }
            "orphan-author-year-marker" => {
                return Some(
                    "the consistency checks found no reference entry beginning with this name"
                        .to_string(),
                )
            }
            _ => {}
        }
    }

    // 2. An acronym, not a surname: RBV, WHO, OECD.
    let letters: Vec<char> = surname.chars().filter(|c| c.is_alphabetic()).collect();
    if letters.len() >= 2 && letters.len() <= 5 && letters.iter().all(|c| c.is_uppercase()) {
        return Some("reads as an acronym rather than an author's surname".to_string());
    }

    // 3. A possessive: "Weiner's (2009) model" names Weiner, and the apostrophe
    //    is the sentence's grammar, not part of the name.
    if surname.ends_with("'s") || surname.ends_with("\u{2019}s") {
        return Some("reads as a possessive form of a name rather than the name".to_string());
    }

    None
}

/// Does `haystack` contain `word` as a WHOLE word?
///
/// Substring matching is what made `Kutzin` match a finding about `Kutzins`.
fn contains_word(haystack: &str, word: &str) -> bool {
    if word.is_empty() {
        return false;
    }
    let mut from = 0usize;
    while let Some(i) = haystack[from..].find(word) {
        let start = from + i;
        let end = start + word.len();
        let before_ok = haystack[..start].chars().next_back().map_or(true, |c| !c.is_alphabetic());
        let after_ok = haystack[end..].chars().next().map_or(true, |c| !c.is_alphabetic());
        if before_ok && after_ok {
            return true;
        }
        from = start + word.len().max(1);
        if from >= haystack.len() {
            break;
        }
    }
    false
}

/// The first curly- or straight-quoted run in a string, if any.
fn first_quoted(s: &str) -> Option<String> {
    for (open, close) in [('\u{201C}', '\u{201D}'), ('"', '"')] {
        if let Some(a) = s.find(open) {
            let rest = &s[a + open.len_utf8()..];
            if let Some(b) = rest.find(close) {
                let inner = rest[..b].trim();
                if !inner.is_empty() && inner.chars().count() <= 160 {
                    return Some(inner.to_string());
                }
            }
        }
    }
    None
}

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
                    parsed_entry: it.reference_entry.clone(),
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

/// DEFECTS 1, 2, 3 and 6 all landed in this section, because all four were worst
/// here: ~20 near-identical paragraphs, each repeating a 34-word caveat, each
/// with a heading truncated mid-word.
///
/// It is now: one sentence of shared explanation, a chart of which works block
/// the most sentences, a table of the works, and a table of where each is cited.
fn emit_blocked_sources(
    out: &mut Vec<Block>,
    items: &[ReportItem],
    has_pages: bool,
    consistency: &[crate::consistency::ConsistencyFinding],
) {
    out.push(heading("Cited, but not checkable", 1));
    if items.is_empty() {
        out.push(para("None found. Every cited source was available to check against."));
        return;
    }
    let groups = group_by_source(items);
    out.push(para(format!(
        "{} sentence{} cite {} source{} Gaply could not read. This is a gap in your library \
         rather than a fault in your writing. Each source listed below is one action away from \
         being checkable.",
        items.len(),
        if items.len() == 1 { "" } else { "s" },
        groups.len(),
        if groups.len() == 1 { "" } else { "s" },
    )));

    // DEFECT 1. The shared caveat, said ONCE.
    let reasons: Vec<String> = groups
        .iter()
        .map(|g| g.items.first().and_then(|i| i.reason.clone()).unwrap_or_default())
        .collect();
    let (shared, _rest) = hoist_shared_tail(&reasons);
    if let Some(tail) = shared {
        // Capitalised and closed, because it is now a sentence of its own rather
        // than the end of one.
        // A tail split off after a dash or colon still carries it. Strip the
        // joiner: this is a sentence now, and one opening with a dash reads as
        // a fragment of the sentence it was cut from.
        let mut sentence = tail
            .trim_start_matches(['\u{2014}', '\u{2013}', '-', ':', ',', ' '])
            .to_string();
        if let Some(c) = sentence.chars().next() {
            sentence = c.to_uppercase().collect::<String>() + &sentence[c.len_utf8()..];
        }
        if !sentence.ends_with('.') {
            sentence.push('.');
        }
        out.push(Block::Note {
            text: format!("This applies to every row below. {sentence}"),
        });
    }

    // DEFECT 3. Which gap is worth closing first, seen rather than counted.
    if groups.len() >= 2 {
        let top: Vec<(String, usize)> = groups
            .iter()
            .take(10)
            .map(|g| {
                (trim_at_word(&cited_work_label(&g.entry, g.parsed_entry.as_deref()), 44), g.items.len())
            })
            .collect();
        out.push(Block::BarChart {
            title: "Which missing sources block the most sentences".to_string(),
            category_axis: "one cited work".to_string(),
            value_axis: "how many of your sentences cite it".to_string(),
            bars: top,
        });
        if groups.len() > 10 {
            out.push(Block::Note {
                text: format!(
                    "The chart shows the ten works blocking the most sentences. All {} are in \
                     the table below.",
                    groups.len()
                ),
            });
        }
    }

    // §11 D145. TWO tables, because these are two different asks.
    //
    // The first is a shopping list: works to go and add. The second is a
    // checking list: names that may not be works at all, which the report must
    // not present as things to fetch.
    let mut real: Vec<(&BlockedSource, String)> = Vec::new();
    let mut suspect: Vec<(&BlockedSource, String)> = Vec::new();
    for g in &groups {
        let label = cited_work_label(&g.entry, g.parsed_entry.as_deref());
        match artifact_reason(&label, consistency) {
            Some(why) => suspect.push((g, why)),
            None => real.push((g, label)),
        }
    }

    // Where a work is cited, so the reader can go straight to it. This replaces
    // a "Status" column whose every value was the same six words: a column that
    // never varies is a heading, and it was occupying the width the one
    // actionable thing needed (§11 D145).
    let cited_at = |g: &BlockedSource| -> String {
        let mut locs: Vec<String> = g.items.iter().map(|it| locator(it, has_pages)).collect();
        locs.dedup();
        locs.join(", ")
    };

    if !real.is_empty() {
        // The status is stated ONCE, in the caption, because every row shares it.
        out.push(Block::Table {
            caption: Some(format!(
                "Not in your library. Add {} and re-run to have {} checked.",
                if real.len() == 1 { "this work" } else { "these works" },
                if real.len() == 1 { "its sentence" } else { "their sentences" },
            )),
            header: vec![
                "Cited work".to_string(),
                "Sentences".to_string(),
                if has_pages { "Cited on page".to_string() } else { "Cited at".to_string() },
            ],
            rows: real
                .iter()
                .map(|(g, label)| vec![label.clone(), g.items.len().to_string(), cited_at(g)])
                .collect(),
            align: vec![Align::Left, Align::Right, Align::Left],
        });
    }

    // What to do, once per distinct action rather than once per work, and ONLY
    // for the works that are works: telling a reader to add `RBV, 1991` to
    // their library is an instruction to do the wrong thing.
    let mut steps: Vec<String> = Vec::new();
    for (g, _) in &real {
        if let Some(st) = &g.next_step {
            if !steps.contains(st) {
                steps.push(st.clone());
            }
        }
    }
    if !steps.is_empty() {
        out.push(heading("What to do", 2));
        for st in steps {
            out.push(bullet(st, 0));
        }
    }

    if !suspect.is_empty() {
        out.push(heading("Names that may not be separate works", 2));
        out.push(para(format!(
            "{} of the {} names in this section came from an in-text marker that may not name a \
             work at all: an acronym, a possessive, or a name the consistency checks could not \
             match to a reference entry. They are listed apart because adding them to your \
             library would be the wrong action. Check the sentence first.",
            suspect.len(),
            groups.len(),
        )));
        out.push(Block::Table {
            caption: Some("Check these against your manuscript before doing anything.".to_string()),
            header: vec![
                "Name as cited".to_string(),
                "Sentences".to_string(),
                "Why it may not be a work".to_string(),
            ],
            rows: suspect
                .iter()
                .map(|(g, why)| {
                    vec![
                        cited_work_label(&g.entry, g.parsed_entry.as_deref()),
                        g.items.len().to_string(),
                        why.clone(),
                    ]
                })
                .collect(),
            align: vec![Align::Left, Align::Right, Align::Left],
        });
    }

    // DEFECT 6. The sentences, in a table, with the text WRAPPED and not cut.
    // A truncated sentence cannot be found in the manuscript, which is the only
    // thing this column is for.
    out.push(heading("Where each one is cited", 2));
    out.push(Block::Table {
        caption: Some(
            "Your own sentences, so you can find each one in the manuscript.".to_string(),
        ),
        header: vec![
            if has_pages { "Page".to_string() } else { "Sentence".to_string() },
            "Cited work".to_string(),
            "Your sentence".to_string(),
        ],
        rows: groups
            .iter()
            .flat_map(|g| {
                g.items.iter().map(move |it| {
                    vec![
                        locator(it, has_pages),
                        trim_at_word(&cited_work_label(&g.entry, g.parsed_entry.as_deref()), 40),
                        it.sentence.trim().to_string(),
                    ]
                })
            })
            .collect(),
        align: vec![Align::Left, Align::Left, Align::Left],
    });
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
    // §11 D145. THE DENOMINATOR, defined once and used everywhere it appears.
    //
    // Every sentence that cites a source is in exactly one of three states, and
    // the report previously named them with three different numbers in three
    // places: the cover said "5 cited claims", "The counts" said 39 were
    // "checked against a source", and the chart said 5. `checked` is ANSWERED
    // (46 minus the 7 that failed validation), which is not the same thing as
    // checked against a source and must never be printed as if it were.
    let citing_total = m.supported.len() + m.unverifiable.len() + m.failed.len();
    // Reached the cited source: a check was actually attempted against it.
    let reached_source = m.supported.len() + m.failed.len();
    let cover_checked = m.supported.len();
    let cover_headline = Some(if cover_checked == 0 {
        (
            if citing_total == 0 {
                // Nothing cited a source at all: a different fact from "none of
                // them could be checked", and "None of 0" is not a sentence.
                "No sentence in this manuscript cites a source".to_string()
            } else {
                format!(
                    "None of {citing_total} cited claim{} could be checked against {} source",
                    if citing_total == 1 { "" } else { "s" },
                    if citing_total == 1 { "its" } else { "their" },
                )
            },
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
                // §11 D138. An abstract-backed check is NOT full-text
                // verification, and the cover is the one line everyone reads.
                // Named separately rather than folded in, because a single
                // number cannot mean both and the stronger reading is the one a
                // reader would assume.
                let from_abstract = m
                    .supported
                    .iter()
                    .filter(|i| !i.evidence.is_empty() && i.abstract_only)
                    .count();
                let full_text = with_passages - from_abstract;
                // §11 D145. "5 cited claims" on a manuscript with 46 of them
                // announces the successes and hides the denominator, and page 1
                // is the page everyone reads. The failure is the finding here:
                // 34 of 46 could not be checked at all.
                let mut line = format!(
                    "{cover_checked} of {citing_total} cited claim{} checked against {} source. \
                     Passages quoted for {full_text}",
                    if citing_total == 1 { "" } else { "s" },
                    if citing_total == 1 { "its" } else { "their" },
                );
                if from_abstract > 0 {
                    line.push_str(&format!(
                        ", and for {from_abstract} from the abstract only"
                    ));
                }
                line
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
            // §11 D145. Was "Sentences answered: 39". `checked` counts every
            // item that returned an answer, INCLUDING the 34 whose answer was
            // "I cannot reach this source" — so beside a cover claiming checks
            // it read as 39 checks. These two say what actually happened.
            ("Cited source reached".to_string(), reached_source.to_string()),
            ("Passages quoted".to_string(), cover_checked.to_string()),
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
             gives no /100. It located the source passages behind {checked} of {citing_total} \
             cited claim{} and \
             quotes them below with their page. Reading those is the check. The grader that \
             used to produce a score was measured on a labelled set and returned the SAME grade \
             for every case (14 of 14 outputs across two runs), so any number built from it \
             described the grader rather than your manuscript.",
            if checked == 1 { "" } else { "s" },
        )));
    }


    // The breakdown. Evidence-backed and advisory are SEPARATE BARS, never
    // summed — a chart that adds a 43%-precision suggestion to a checked
    // finding is the same conflation the score just removed (§11 D78).
    // DEFECT 3. ONE stacked bar, because these three parts ARE the whole:
    // every sentence that reached a check is in exactly one of them. Three
    // separate bars invited reading them as unrelated quantities, and §11 D78's
    // warning about summing unlike things applies to the reader's eye too.
    //
    // §11 D93 established that the bars are DRAWN rather than typed: these were
    // `"=".repeat(n)` inside a bullet, which is a chart only in a terminal.
    let mut segments = vec![
        ("Source passages found".to_string(), checked, Tone::Good),
        ("Could not be checked".to_string(), m.unverifiable.len(), Tone::Neutral),
    ];
    if !m.failed.is_empty() {
        segments.push(("Not judged".to_string(), m.failed.len(), Tone::Warn));
    }
    out.push(Block::StackedBar {
        title: "What happened to every sentence that cited a source".to_string(),
        segments,
    });

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
        // §11 D128. This said "checked against a source OR judged for whether
        // they need one". The second clause was the advisory lane and is retired;
        // leaving it described a scope the run no longer has.
        // §11 D145. This printed the ANSWERED count as if it were the checked
        // count: it claimed 39 checks where 5 happened, in the section called
        // "The counts", contradicting the chart one page earlier. It is the
        // number a researcher would quote to a supervisor.
        "Gaply read {} sentences. {} of them cite a source. It could reach the cited source for \
         {} of those and quoted passages for {}; the remaining {} cite works it could not read.",
        m.total_sentences,
        citing_total,
        reached_source,
        m.supported.len(),
        m.unverifiable.len()
    )));

    // DEFECT 2. Was a run of bullets. WHAT WAS JUDGED, BY KIND: the row tally,
    // which is NOT the number of claims whose passages were found. One
    // `citation_support` item can be judged and yield no passage, so these two
    // numbers differ legitimately and the report says which is which rather than
    // leaving a reader to reconcile "4 citation support" against "passages
    // quoted for 3".
    if !m.counts_by_category.is_empty() {
        out.push(Block::Table {
            caption: Some(
                "What Gaply did with each sentence. Counts rows, not findings.".to_string(),
            ),
            header: vec!["Kind of check".to_string(), "Sentences".to_string()],
            rows: m
                .counts_by_category
                .iter()
                .map(|(kind, n)| vec![kind.replace('_', " "), n.to_string()])
                .collect(),
            align: vec![Align::Left, Align::Right],
        });
    }

    // §11 D108. The support VERDICTS are recorded but no longer summarised as
    // bars. "support: weak 8 (67%)" is the same withdrawn grade wearing a
    // headline, and a bar is the most emphatic thing on the page.
    let (support_counts, other_counts): (Vec<_>, Vec<_>) =
        m.verdict_counts.iter().partition(|(v, _)| v.starts_with("support:"));
    if !other_counts.is_empty() {
        out.push(Block::Table {
            caption: Some("What the model concluded.".to_string()),
            header: vec!["Conclusion".to_string(), "Sentences".to_string()],
            rows: other_counts
                .iter()
                .map(|(v, n)| vec![v.replace('_', " "), n.to_string()])
                .collect(),
            align: vec![Align::Left, Align::Right],
        });
    }
    if !support_counts.is_empty() {
        let located: usize = support_counts.iter().map(|(_, n)| *n).sum();
        out.push(para(format!(
            "{located} cited sentence{} had source passages located and quoted in this report. \
             Gaply does not grade how well a passage supports a sentence. On a 6-case labelled \
             set the grade was identical on all 14 outputs across two runs, so it carries no \
             information and is not reported. The passages are.",
            if located == 1 { "" } else { "s" },
        )));
    }
    if !m.skipped_reasons.is_empty() {
        // DEFECT 4. Was `bullet(format!("{n} — {r}"))`: an em dash doing a
        // table's job.
        out.push(Block::Table {
            // "Not judged" is a marker `detect_gaply_report` recognises, so the
            // wording keeps it rather than paraphrasing it away (§11 D80).
            caption: Some("Not judged, and why.".to_string()),
            header: vec!["Reason".to_string(), "Sentences".to_string()],
            rows: m
                .skipped_reasons
                .iter()
                .map(|(r, n)| vec![r.clone(), n.to_string()])
                .collect(),
            align: vec![Align::Left, Align::Right],
        });
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
             not a judgement of your writing, and no language model was involved. A retraction \
             registry was asked and answered. Citing a retracted work can be legitimate when the \
             retraction is the point; otherwise the citation needs replacing.",
            m.retracted_sources.len(),
            if m.retracted_sources.len() == 1 { "has" } else { "have" },
        )));
        out.push(Block::Table {
            caption: Some("Works a retraction registry reports as withdrawn.".to_string()),
            header: vec!["Retracted work".to_string(), "Your sentences citing it".to_string()],
            rows: m
                .retracted_sources
                .iter()
                .map(|(entry, citing)| vec![entry.clone(), citing.to_string()])
                .collect(),
            align: vec![Align::Left, Align::Right],
        });
        out.push(Block::Note {
            text: "Only works a registry actually answered about appear here. An entry that was \
                   never checked is not listed, and is not clean. The Citation Manager says which \
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
            "Found by reading the manuscript's own structure: its reference list, markers, \
             captions and headings. No language model was involved, so unlike the sections \
             below these are not judgements and carry no error rate.",
        ));
        // DEFECT 2. One table, with severity as a COLUMN rather than as two
        // headings and a badge per finding. Severity is what differs between
        // rows, so it is a cell; the reader scans one grid instead of two lists.
        let sev = |f: &crate::consistency::ConsistencyFinding| -> &'static str {
            match f.severity {
                crate::consistency::Severity::Structural => "Affects the audit",
                crate::consistency::Severity::Cosmetic => "Worth fixing",
            }
        };
        // Structural first: these decide whether the rest of the report can be
        // trusted, so they must not sort below a spacing nit.
        let mut ordered: Vec<&crate::consistency::ConsistencyFinding> = m.consistency.iter().collect();
        ordered.sort_by_key(|f| match f.severity {
            crate::consistency::Severity::Structural => 0,
            crate::consistency::Severity::Cosmetic => 1,
        });
        out.push(Block::Table {
            caption: Some(
                "\"Affects the audit\" means this could change what the rest of this report \
                 says. \"Worth fixing\" changes nothing above it."
                    .to_string(),
            ),
            header: vec![
                "What Gaply found".to_string(),
                "Severity".to_string(),
                "What to do".to_string(),
            ],
            rows: ordered
                .iter()
                .map(|f| {
                    vec![
                        f.message.clone(),
                        sev(f).to_string(),
                        f.action.clone().unwrap_or_else(|| "No action needed.".to_string()),
                    ]
                })
                .collect(),
            align: vec![Align::Left, Align::Left, Align::Left],
        });
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
         quoted underneath it with their page. THIS IS THE EVIDENCE-BACKED SECTION, and the \
         evidence is the finding. Gaply does NOT grade how well a passage supports a sentence: \
         measured on a 6-case labelled set, its grade was the same one every time (14 of 14 \
         outputs across two runs), so the grade carries no information and is not shown. \
         Reading the quoted passage is the check.",
        &m.supported,
        m.has_pages,
    );

    out.push(Block::PageBreak);
    emit_blocked_sources(&mut out, &m.unverifiable, m.has_pages, &m.consistency);

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
            // Defect 1 and 2. A table's text is its caption, its header and
            // every cell, so wording assertions keep working after prose became
            // rows.
            Block::Table { caption, header, rows, .. } => {
                let mut t = caption.clone().unwrap_or_default();
                t.push(' ');
                t.push_str(&header.join(" "));
                for r in rows {
                    t.push(' ');
                    t.push_str(&r.join(" "));
                }
                t
            }
            Block::StackedBar { title, segments } => {
                // The legend a reader sees prints the count AND the share, so
                // the text form carries both: the collision guard below is
                // asserting on what reaches the page.
                let total: usize = segments.iter().map(|(_, n, _)| *n).sum();
                let mut t = title.clone();
                for (label, n, _) in segments {
                    let pct = if total == 0 { 0 } else { ((*n as f64 / total as f64) * 100.0).round() as u32 };
                    t.push_str(&format!(" {label} {n} ({pct}%)"));
                }
                t
            }
            Block::BarChart { title, category_axis, value_axis, bars } => {
                let mut t = format!("{title} {category_axis} {value_axis}");
                for (label, n) in bars {
                    t.push_str(&format!(" {label} {n}"));
                }
                t
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
                why: String::new(),
                elided: false,
            }],
            ..Default::default()
        }
    }

    /// An item blocked on one cited work, identified by its reference entry.
    fn blocked_item(entry: &str) -> ReportItem {
        ReportItem {
            seq: 11,
            page: Some(5),
            sentence: "Uptake has risen steadily across the region since 2015.".into(),
            reference_entry: Some(entry.to_string()),
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

    /// DEFECT 4: NO EM DASHES ANYWHERE IN THE OUTPUT.
    ///
    /// An em dash is a typographer's join that a reader has to decode: it stands
    /// in for a colon, a comma, a parenthesis or a full stop without saying
    /// which. In a report read by researchers outside computing, the punctuation
    /// should not need interpreting, and two clauses joined by a dash are almost
    /// always clearer as two sentences.
    ///
    /// Checked on the COMPOSED TEXT and on BOTH RENDERED OUTPUTS, because a dash
    /// can enter at any of the three layers. The PDF check is on the encoded
    /// byte (WinAnsi `0x97`), not on the Rust string, since that is what a
    /// reader's viewer actually draws.
    ///
    /// A rich model on purpose: every section has to be reachable for this to
    /// mean anything, and a model with empty vectors would pass while the
    /// unreachable sections kept their dashes.
    #[test]
    fn no_em_dash_reaches_the_reader() {
        let m = rich_model();
        let blocks = compose_audit(&m);

        // 1. The composed text, with the offending string named. A bare
        //    "contains an em dash" failure would send the next person grepping
        //    a 1800-line file.
        let mut offenders: Vec<String> = Vec::new();
        for b in &blocks {
            let t = text_of(b);
            if t.contains('\u{2014}') {
                offenders.push(t.chars().take(160).collect());
            }
        }
        assert!(
            offenders.is_empty(),
            "em dash (U+2014) in composed output:\n  {}",
            offenders.join("\n  ")
        );

        // 2. The HTML, where it would arrive as UTF-8.
        let html = crate::report_html::render_html(&blocks, "t");
        assert!(!html.contains('\u{2014}'), "em dash in rendered HTML");

        // 3. The PDF. An em dash is WinAnsi 0x97, but `escape_pdf` writes any
        //    byte above 0x7F as an OCTAL ESCAPE, so the literal byte never
        //    appears in the file and `0x97` alone is a check for something the
        //    renderer cannot emit.
        //
        //    The first version of this assertion did exactly that. It passed on
        //    a report that carried SEVEN em dashes, which `pdftotext` found
        //    immediately. Both forms are checked now, and the escape is the one
        //    that does the work.
        let pdf = crate::report_pdf::render_pdf(&blocks);
        assert!(
            !pdf.windows(1).any(|w| w == [0x97]),
            "em dash (raw WinAnsi 0x97) in rendered PDF"
        );
        assert!(
            !pdf.windows(4).any(|w| w == b"\\227"),
            "em dash (octal escape \\227) in rendered PDF"
        );

        // THE NEGATIVE CONTROL (§11 D144). A guard for "X must not appear" is
        // worth nothing until it has been shown to FAIL on output that does
        // contain X. The byte-only version of this assertion was green for its
        // whole life on a report carrying seven em dashes, and nothing about a
        // passing run said so.
        let dashed = crate::report_pdf::render_pdf(&[Block::Paragraph {
            text: "a \u{2014} b".to_string(),
        }]);
        assert!(
            !dashed.windows(1).any(|w| w == [0x97]),
            "the renderer emits the RAW byte after all, so the escape check is \
             not the load-bearing one and this control needs rewriting"
        );
        assert!(
            dashed.windows(4).any(|w| w == b"\\227"),
            "the escape check cannot see an em dash that IS in the output: the \
             guard above proves nothing"
        );
    }

    /// DEFECT 4, at the SOURCE, because the output test could not see this.
    ///
    /// `no_em_dash_reaches_the_reader` renders a fixture, and a fixture only
    /// reaches the strings its own data triggers. Six dashes in `consistency`
    /// survived it and appeared in the regenerated report: the findings there are
    /// built from a real manuscript's structure, which no hand-written model
    /// reproduces.
    ///
    /// So this reads the modules that own reader-facing strings and fails on a
    /// literal em dash in any non-comment line. Regex lines are exempt: a
    /// character class matching a dash in someone else's text is not a dash in
    /// ours.
    #[test]
    fn no_module_that_writes_to_the_reader_contains_an_em_dash() {
        let sources = [
            ("audit_report.rs", include_str!("audit_report.rs")),
            ("consistency.rs", include_str!("consistency.rs")),
            ("report_html.rs", include_str!("report_html.rs")),
            ("report_pdf.rs", include_str!("report_pdf.rs")),
            ("report_compose.rs", include_str!("report_compose.rs")),
        ];
        let mut bad: Vec<String> = Vec::new();
        for (name, src) in sources {
            for (n, line) in src.lines().enumerate() {
                let t = line.trim_start();
                if !line.contains('\u{2014}') {
                    continue;
                }
                // Comments explain the rule and may quote the character. A
                // TRAILING comment counts too: `return; // ... \u{2014} ...` is a
                // comment, and the first version of this guard reported three of
                // them as defects.
                let code = match line.find("//") {
                    Some(i) => &line[..i],
                    None => line,
                };
                if !code.contains('\u{2014}') || t.starts_with("*") {
                    continue;
                }
                // A regex matching a dash in a MANUSCRIPT is not one in a report.
                if line.contains("Regex::new") || line.contains("regex") {
                    continue;
                }
                bad.push(format!("{name}:{}: {}", n + 1, t.chars().take(110).collect::<String>()));
            }
        }
        assert!(
            bad.is_empty(),
            "em dash in a module that writes to the reader:\n  {}",
            bad.join("\n  ")
        );
    }

    /// DEFECT 1, for the two caveats that repeat inside a SECTION.
    ///
    /// Six items checked against an abstract printed the same forty words six
    /// times, and seven items that failed the same way printed the same reason
    /// seven times. §11 D140 put the abstract sentence ON the item deliberately,
    /// so hoisting must not lose it: the section states it once and each item
    /// still says, briefly, that it is one of them.
    #[test]
    fn a_caveat_shared_by_a_whole_section_is_stated_once() {
        let mut m = model();
        let same = "the model's output failed validation twice: supporting_chunks is empty";
        m.failed = (0..6)
            .map(|i| ReportItem {
                seq: i,
                sentence: format!("Sentence {i}."),
                abstract_only: true,
                reason: Some(same.to_string()),
                ..Default::default()
            })
            .collect();
        let text = all_text(&compose_audit(&m));

        // The long explanation: ONCE for six items.
        assert_eq!(
            text.matches("An abstract carries").count(),
            1,
            "the abstract caveat was repeated per item:\n{text}"
        );
        // But every item is still identifiable as one of them.
        assert_eq!(
            text.matches("Checked against an ABSTRACT only").count(),
            6,
            "an item stopped saying it was abstract-backed:\n{text}"
        );
        // And the identical reason is stated once, not six times.
        assert_eq!(
            text.matches(same).count(),
            1,
            "the shared reason was repeated per item:\n{text}"
        );

        // A SINGLE abstract-backed item keeps the full sentence on the item:
        // there is nothing to hoist, and a one-line marker with no explanation
        // anywhere would be worse than what §11 D140 fixed.
        let mut one = model();
        one.failed = vec![ReportItem { abstract_only: true, ..Default::default() }];
        let t1 = all_text(&compose_audit(&one));
        assert_eq!(t1.matches("An abstract carries").count(), 1, "{t1}");
        assert!(!t1.contains("Checked against an ABSTRACT only"), "{t1}");
    }

    /// §11 D145. THE NUMBERS AGREE WITH EACH OTHER.
    #[test]
    fn the_checked_count_never_claims_more_than_the_chart() {
        let mut m = model();
        m.supported = vec![item_with_evidence(); 5];
        m.unverifiable = (0..34).map(|i| ReportItem { seq: i, ..Default::default() }).collect();
        m.failed = (0..7).map(|i| ReportItem { seq: i, ..Default::default() }).collect();
        m.checked = 39; // answered, i.e. 46 minus the 7 that failed validation
        let blocks = compose_audit(&m);
        let text = all_text(&blocks);

        assert!(
            !text.contains("39 of them cite a source and were checked"),
            "the answered count is being reported as checks:\n{text}"
        );
        let segments = blocks
            .iter()
            .find_map(|b| match b {
                Block::StackedBar { segments, .. } => Some(segments.clone()),
                _ => None,
            })
            .expect("no breakdown chart");
        let charted = segments
            .iter()
            .find(|(l, _, _)| l.contains("passages found"))
            .map(|(_, n, _)| *n)
            .expect("no checked segment");
        assert_eq!(charted, 5);
        assert!(text.contains("quoted passages for 5"), "{text}");

        let Some(Block::Cover { headline, meta, .. }) = blocks.first() else { panic!("no cover") };
        let h = &headline.as_ref().unwrap().0;
        assert!(h.contains("5 of 46"), "the cover hides the denominator: {h}");
        assert!(
            !meta.iter().any(|(k, _)| k == "Sentences answered"),
            "the misleading cover figure is back: {meta:?}"
        );
    }

    /// §11 D145. A name that may not be a work is not a shopping-list row.
    #[test]
    fn marker_artifacts_are_listed_apart_from_works_to_fetch() {
        let mut m = model();
        m.unverifiable = vec![
            // Distinct sentences, because real ones are: a fixture that reuses
            // one string trips the shared-value check below for a reason the
            // product does not have.
            ReportItem { seq: 1, sentence: "Uptake rose after 2015.".into(), next_step: Some("Add it to your library.".into()), ..blocked_item("Banerjee, 2021") },
            ReportItem { seq: 2, sentence: "Enrolment followed the same path.".into(), next_step: Some("Add it to your library.".into()), ..blocked_item("Banerjee, 2021") },
            ReportItem { seq: 3, sentence: "Firms differ in their resources.".into(), next_step: Some("Add it to your library.".into()), ..blocked_item("RBV, 1991") },
            ReportItem { seq: 4, sentence: "Attribution shapes the response.".into(), next_step: Some("Add it to your library.".into()), ..blocked_item("Weiner\u{2019}s, 2009") },
            ReportItem { seq: 5, sentence: "The platform processed three million transactions.".into(), next_step: Some("Add it to your library.".into()), ..blocked_item("Authority, 2025") },
        ];
        m.consistency = vec![crate::consistency::ConsistencyFinding {
            kind: "uncertain-reference-match".into(),
            severity: crate::consistency::Severity::Cosmetic,
            message: "\u{201C}Authority (2025)\u{201D} names \u{201C}authority\u{201D}, which appears in the entry \
                      beginning \u{201C}The Financial Services Authority. (2025)\u{201D}"
                .into(),
            action: None,
        }];
        let blocks = compose_audit(&m);

        let table_with = |first_header: &str| -> Vec<Vec<String>> {
            blocks
                .iter()
                .find_map(|b| match b {
                    Block::Table { header, rows, .. }
                        if header.first().map(String::as_str) == Some(first_header) =>
                    {
                        Some(rows.clone())
                    }
                    _ => None,
                })
                .unwrap_or_default()
        };
        let names = |rows: &[Vec<String>]| rows.iter().map(|r| r[0].clone()).collect::<Vec<_>>();
        assert_eq!(names(&table_with("Cited work")), vec!["Banerjee, 2021"], "a non-work is on the shopping list");
        let s = names(&table_with("Name as cited"));
        for expected in ["RBV, 1991", "Weiner\u{2019}s, 2009", "Authority, 2025"] {
            assert!(s.iter().any(|n| n == expected), "{expected} was presented as a work: {s:?}");
        }

        // DEFECT 3: no TEXT column whose every value is the same.
        //
        // Scoped to left-aligned columns after the first, which is where a
        // derived label like "Not in your library" lands. A COUNT column is
        // exempt: four works cited once each legitimately show "1" four times.
        for b in &blocks {
            if let Block::Table { header, rows, align, .. } = b {
                if rows.len() < 2 {
                    continue;
                }
                for (c, name) in header.iter().enumerate() {
                    if c == 0 || align.get(c) != Some(&Align::Left) {
                        continue;
                    }
                    let first = rows[0].get(c);
                    assert!(
                        !rows.iter().all(|r| r.get(c) == first),
                        "column {name:?} repeats {first:?} on every row: that is a heading, not a cell"
                    );
                }
            }
        }
    }

    /// §11 D147. The model's pointer LEADS its passage, and the disclosure is
    /// printed with the passages rather than buried.
    #[test]
    fn the_models_pointer_leads_the_passage_it_points_into() {
        let mut m = model();
        m.supported = vec![ReportItem {
            seq: 3,
            sentence: "Enrolment is low.".into(),
            evidence: vec![ReportEvidence {
                chunk_id: "c931".into(),
                page: Some(1),
                quote: "Background: the enrolment rate in the mandatory scheme is low.".into(),
                why: "Background: Lao PDR has low enrolment.".into(),
                elided: false,
            }],
            ..Default::default()
        }];
        let text = all_text(&compose_audit(&m));

        let pointer = text.find("What the model points to in c931").expect("no pointer");
        let passage = text.find("Background: the enrolment rate").expect("no passage");
        assert!(pointer < passage, "the pointer did not lead its passage:\n{text}");
        assert!(text.contains("Background: Lao PDR has low enrolment."), "the why was dropped");

        // The standing disclosure, because de-hyphenation cannot be marked inline.
        assert!(text.contains("machine-extracted from a PDF"), "no disclosure:\n{text}");
        assert!(text.contains("check it against the source"), "{text}");
        // Nothing was elided here, so the marker is NOT explained.
        assert!(
            !text.contains("[...] marks"),
            "an elision was explained where none happened:\n{text}"
        );
    }

    /// An elision is explained only where one occurred.
    #[test]
    fn an_elided_passage_says_what_the_marker_means() {
        let mut m = model();
        m.supported = vec![ReportItem {
            seq: 3,
            sentence: "Enrolment is low.".into(),
            evidence: vec![ReportEvidence {
                chunk_id: "c1".into(),
                page: Some(1),
                quote: "[...] and their dependents.".into(),
                why: String::new(),
                elided: true,
            }],
            ..Default::default()
        }];
        let text = all_text(&compose_audit(&m));
        assert!(text.contains("[...] marks"), "the marker was left unexplained:\n{text}");
    }

    /// A model that reaches EVERY section, for the whole-output guards.
    fn rich_model() -> AuditReportModel {
        let mut m = model();
        m.unverifiable = vec![
            ReportItem {
                reason: Some(
                    "no library work matches \u{201C}AlJohani, 2024\u{201D}: taken from the \
                     in-text marker, which gives a surname and a year and not the full author \
                     list, so this may be an incomplete or mis-split name rather than a missing \
                     source"
                        .into(),
                ),
                next_step: Some("Add the work to your library, then re-run.".into()),
                ..blocked_item("AlJohani, 2024")
            },
            ReportItem {
                reason: Some(
                    "no library work matches \u{201C}Banerjee, 2021\u{201D}: taken from the \
                     in-text marker, which gives a surname and a year and not the full author \
                     list, so this may be an incomplete or mis-split name rather than a missing \
                     source"
                        .into(),
                ),
                next_step: Some("Add the work to your library, then re-run.".into()),
                ..blocked_item("Banerjee, 2021")
            },
        ];
        m.failed = vec![ReportItem { reason: Some("the reply was not valid JSON".into()), ..blocked_item("x") }];
        m.retracted_sources = vec![("Smith, J. (2019). A withdrawn paper.".into(), 2)];
        m.consistency = vec![crate::consistency::ConsistencyFinding {
            kind: "reference_count".into(),
            severity: crate::consistency::Severity::Structural,
            message: "Reference 6 is numbered but the list has 5 entries.".into(),
            action: Some("Renumber the reference list.".into()),
        }];
        m
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
            why: String::new(),
            elided: false,
        });
        let text = all_text(&compose_audit(&m));
        assert!(text.contains("c7 · p.4:"), "{text}");
        assert!(text.contains("c9 · page unknown:"), "{text}");
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
        // DEFECT 2: these are table cells now, so the number follows its label
        // instead of preceding it.
        assert!(text.contains("citation need 65"), "{text}");
        // The verdict counts are drawn as proportional bars now, so the number
        // and its label are no longer adjacent — assert both, not the old
        // "52 needs citation" spelling.
        assert!(text.contains("needs citation"), "{text}");
        assert!(text.contains("52"), "{text}");
        assert!(text.contains("no citation needed"), "{text}");
        assert!(text.contains("27"), "{text}");
        assert!(text.contains("a table row or a figure caption"), "{text}");
        assert!(text.contains(" 5"), "{text}");
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

        // DEFECT 1 and 2. ONE ROW for the source, carrying the count that
        // decides whether fixing it is worth the reader's time. It was one
        // heading plus a repeated paragraph per source.
        let blocks = compose_audit(&m);
        let table = blocks.iter().find_map(|b| match b {
            Block::Table { header, rows, .. } if header.first().map(String::as_str) == Some("Cited work") => Some(rows),
            _ => None,
        });
        let rows = table.expect("no table of cited works");
        assert_eq!(rows.len(), 1, "three sentences on one work did not group to one row: {rows:?}");
        assert_eq!(rows[0][1], "3", "the row lost its sentence count: {rows:?}");
        // DEFECT 1. The action is listed ONCE per distinct action now, under
        // "What to do", rather than once per source with a "Fix:" prefix. Two
        // sources needing the same step print it once between them.
        assert_eq!(
            text.matches("Fetch the open-access PDF").count(),
            1,
            "the action was repeated instead of stated once:\n{text}"
        );
        assert!(text.contains("S. Mohammad and P. Turney"), "{text}");
        // Every dependent sentence is still listed under it — grouping must not
        // hide which sentences are affected.
        for s in ["Classical approaches", "Lexicon methods", "association lexicon"] {
            assert!(text.contains(s), "missing dependent sentence {s}:\n{text}");
        }
        // And the section says whose fault it is not.
        assert!(text.contains("rather than a fault in your writing"), "{text}");
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

    /// §11 D138. AN ABSTRACT-BACKED CHECK IS ALLOWED, AND MAY NOT LOOK LIKE A
    /// FULL-TEXT ONE.
    ///
    /// Measured on a real run: of 12 sentences that became checkable, 5 rested on
    /// full text and 7 on an abstract. Refusing abstracts would have dropped 7
    /// real checks — an abstract genuinely carries some claims ("GoEmotions
    /// reports 46% macro-F1" is in the abstract). Presenting them identically
    /// would have told a researcher 12 claims were verified against their
    /// sources, which is not what happened.
    #[test]
    fn an_abstract_backed_check_says_so_and_is_counted_apart() {
        let mut m = model();
        let ev = |q: &str| ReportEvidence {
            chunk_id: "c1".into(),
            page: Some(2),
            quote: q.to_string(),
            why: String::new(),
            elided: false,
        };
        m.supported = vec![
            ReportItem {
                seq: 1,
                sentence: "A claim checked against the full text.".into(),
                evidence: vec![ev("A passage from the body of the paper.")],
                verdict: Some("strong".into()),
                ..Default::default()
            },
            ReportItem {
                seq: 2,
                sentence: "A claim checked against the abstract only.".into(),
                evidence: vec![ev("The abstract reports 46% macro-F1.")],
                verdict: Some("strong".into()),
                abstract_only: true,
                ..Default::default()
            },
        ];
        let text = all_text(&compose_audit(&m));

        // The ITEM says so, where a reader who lands on it will see it.
        assert!(
            text.contains("IN THE ABSTRACT ONLY"),
            "an abstract-backed check did not say so:\n{text}"
        );
        // And the cover does not fold it into the full-text count.
        assert!(
            text.contains("Passages quoted for 1, and for 1 from the abstract only"),
            "the cover implied two full-text checks:\n{text}"
        );
        // The weaker thing is still PRESENT — not filtered out.
        assert!(text.contains("The abstract reports 46% macro-F1."), "{text}");
    }

    /// §11 D140. AN ABSTRACT-BACKED FAILURE MUST NOT LOOK LIKE A MODEL FAILURE.
    ///
    /// §11 D138 stopped an abstract-backed SUCCESS reading as a full-text one.
    /// This is the same rule in the other direction. Measured live: of 7 items
    /// that "could not be judged", SIX had only an abstract — a one-chunk source
    /// carries too little to quote, so the model answered "weak" and cited
    /// nothing, which the validator correctly refuses. A reader seeing "7 could
    /// not be judged" concludes the model is unreliable; the honest statement is
    /// that six had nothing to work from.
    #[test]
    fn a_not_judged_item_says_when_only_an_abstract_was_available() {
        let mut m = model();
        m.failed = vec![
            ReportItem {
                seq: 80,
                sentence: "A claim whose source had only an abstract.".into(),
                abstract_only: true,
                ..Default::default()
            },
            ReportItem {
                seq: 141,
                sentence: "A claim whose source was full text.".into(),
                ..Default::default()
            },
        ];
        let text = all_text(&compose_audit(&m));

        assert!(
            text.contains("Only an ABSTRACT was available"),
            "an abstract-backed failure did not say so:\n{text}"
        );
        // ONCE — on the abstract item, not on the full-text one beside it.
        assert_eq!(text.matches("Only an ABSTRACT was available").count(), 1, "{text}");
        // And it says whose limit it is: the source's, not the sentence's.
        assert!(
            text.contains("rather than a judgement about the sentence"),
            "the notice did not distinguish a source limit from a verdict:\n{text}"
        );
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

        // DEFECT 3. The three At-a-glance proportions are now ONE StackedBar,
        // because they partition the same whole; separate bars invited reading
        // them as unrelated quantities. The property under test is unchanged:
        // proportions are DRAWN blocks, never characters in a string.
        let drawn: Vec<&Block> = blocks
            .iter()
            .filter(|b| matches!(b, Block::StackedBar { .. } | Block::BarChart { .. } | Block::Bar { .. }))
            .collect();
        assert!(!drawn.is_empty(), "the At-a-glance proportions are not drawn blocks");
        let stacked = blocks.iter().find_map(|b| match b {
            Block::StackedBar { segments, .. } => Some(segments),
            _ => None,
        });
        let segments = stacked.expect("no stacked bar for the manuscript breakdown");
        assert!(
            segments.len() >= 2,
            "the breakdown collapsed to one segment: {segments:?}"
        );

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
        // DEFECT 2: a table, so the count is a cell and the header carries the
        // plural once. The old assertions pinned "cited by 3 sentences" and
        // "cited by 1 sentence" to catch a singular/plural slip; a column cannot
        // make that slip, so what is asserted now is that both counts survive
        // and are attributed to the right work.
        assert!(text.contains("Your sentences citing it"), "no count column:\n{text}");
        let rows: Vec<&Block> = blocks
            .iter()
            .filter(|b| matches!(b, Block::Table { header, .. } if header.iter().any(|h| h == "Retracted work")))
            .collect();
        assert_eq!(rows.len(), 1, "the retraction table is not there exactly once");
        if let Some(Block::Table { rows, .. }) = rows.first() {
            assert!(rows.iter().any(|r| r[0].contains("Wakefield") && r[1] == "3"), "{rows:?}");
            assert!(rows.iter().any(|r| r[1] == "1"), "the single-sentence work lost its count: {rows:?}");
        }

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
        assert!(text.contains("never checked is not listed, and is not clean"), "{text}");

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
                why: String::new(),
                elided: false,
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
        assert!(text.contains("46% macro-F1: in the source"), "{text}");
        assert!(text.contains("95% accuracy: NOT in the passages read"), "{text}");
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
        // §11 D145. The empty case carries the DENOMINATOR too, and
        // distinguishes "nothing cited a source" from "none could be checked".
        let h = &headline.as_ref().unwrap().0;
        assert!(h.contains("could be checked") || h.contains("cites a source"), "{headline:?}");
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
                    why: String::new(),
                    elided: false,
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
        assert!(text.contains("Read this and judge it yourself"), "{text}");
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
