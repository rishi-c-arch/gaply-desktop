//! The COMPOSER layer — "how should a researcher read this?"
//!
//! Owns section ordering, grouping, what belongs together, and EVERY
//! user-facing string that is not a `vocabulary` label. Owns no facts: see
//! `report_model`'s module docs for the three-layer contract.
//!
//! # Why `Block` is FLAT — a refinement of the approved design
//!
//! The design proposal listed `FindingGroup { severity, findings }` and
//! `ChecklistTable { rows }` as block variants. **Both were dropped**, because
//! either one hands the renderer a question it is forbidden to answer: *what
//! does a finding look like?* A renderer receiving `FindingGroup` must decide
//! whether the title is bold, whether the detail is indented, whether provenance
//! is shown — presentation decisions, made below the composer.
//!
//! With a flat block set the composer EXPANDS a group into headings, paragraphs
//! and bullets, and the renderer's whole job reduces to turning a line of text
//! at a size into glyphs at a position. **That is a stricter reading of the
//! contract than the proposal's, not a looser one**, and it is what makes a
//! second renderer (HTML, DOCX) cheap.
//!
//! A new `Block` variant still breaks every renderer's `match` — the totality
//! discipline is unchanged.

use crate::report::FindingSeverity;
use crate::report_model::LocalReportModel;
use crate::vocabulary::{claim_label, severity_label, tier_label};

/// One unit of composed output. Deliberately close to "a line of text with a
/// role" — see the module docs for why it is not richer.
#[derive(Debug, Clone, PartialEq)]
pub enum Block {
    /// Document title page. `meta` is label/value pairs.
    Cover { title: String, subtitle: String, meta: Vec<(String, String)> },
    /// `level` 1 = section, 2 = subsection, 3 = finding title.
    Heading { text: String, level: u8 },
    Paragraph { text: String },
    /// `indent` 0 = flush bullet, 1 = nested.
    Bullet { text: String, indent: u8 },
    /// A caveat, disclosure or limitation. Rendered de-emphasised.
    Note { text: String },
    /// A verdict, as a label with a SEMANTIC tone rather than a colour.
    ///
    /// The tone is the composer's (it knows what "contradicts" means); the
    /// colour is each renderer's (only it knows its medium). A composer that
    /// said `#c0392b` would be deciding presentation, which is the layering
    /// this block set exists to prevent — and it would be wrong in a
    /// monochrome print anyway.
    Badge { text: String, tone: Tone },
    PageBreak,
}

/// How a badge should read, not how it should look.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tone {
    /// Checked and supported.
    Good,
    /// Needs the author's attention.
    Warn,
    /// Wrong, contradicted, or failed.
    Bad,
    /// Stated without judgement.
    Neutral,
}

/// Disclosure for characters FOLDED to a Latin base letter (`Śarmā` → `Sarma`).
///
/// # Why these two strings live here but are emitted by the renderer
///
/// Only the renderer knows what happened to a character — the outcome depends
/// on the font, which the composer is forbidden to know about. But the WORDING
/// is a user-facing string, which is the composer's. So ownership splits along
/// that line: **the composer owns what is said, the renderer owns whether it
/// applies.** Disclosing a renderer's own limitation is not a decision about
/// content; it is the render-path form of §4.14's absence attribution.
///
/// # Why TWO strings and not one
///
/// Under the folding rule a character is INTACT, SIMPLIFIED, or MARKED, and the
/// last two are different facts:
///
/// | | Outcome | What the reader must understand |
/// |---|---|---|
/// | `Śarmā` → `Sarma` | **simplified** | the name is ALTERED and readable |
/// | `हिन्दी` → `??????` | **marked** | the text is ABSENT and visible |
///
/// One note covering both would blur them. **A reader judging whether to trust
/// a name needs the first specifically** — "some characters could not be
/// displayed" gives them no reason to doubt a name that is right there on the
/// page, spelled wrongly.
/// # A DISCLOSURE CANNOT USE AN UNRENDERABLE EXAMPLE
///
/// The first draft read *"for example, a name written Śarmā appears here as
/// Sarma"*. **`Śarmā` is exactly what this renderer cannot represent**, so the
/// sentence rendered as *"a name written Sarma appears here as Sarma"* — a note
/// explaining an alteration, silently altered, into nonsense.
///
/// Caught by a `debug_assert` that the disclosure strings are themselves ASCII,
/// on the instrument's first run. **The wording therefore names the CLASS of
/// change rather than showing an instance of it**: showing one is impossible in
/// the medium doing the showing.
pub const NOTE_SIMPLIFIED: &str =
    "Some characters in this report were written in their closest basic Latin form: a letter \
     carrying an accent appears as its plain equivalent, and a Greek letter or mathematical \
     symbol appears as its name spelled out. Your manuscript is unchanged.";

/// Disclosure for characters this renderer cannot represent at all.
///
/// **States no count.** The marks are per code point, which is not the unit a
/// reader counts: `हिन्दी` is six code points but three visual clusters, so any
/// length claim would assert a character count nobody would recognise. Getting
/// that right needs grapheme segmentation, which `gaply_core` does not depend
/// on. **The marks show THAT something is missing, not how much** — see
/// ARCHITECTURE_TRACE §31.
pub const NOTE_MARKED: &str =
    "Some characters could not be displayed in this report and appear as a question \
     mark. This is a limitation of the report's font, not of the analysis — nothing \
     was skipped or removed.";

/// How much of the quoted manuscript paragraph a finding's bullet shows.
///
/// # MEASURED, not reasoned — on the frozen reference manuscript
///
/// The engine carries the paragraph WHOLE (a fact); this is where it is cut,
/// because truncation is a presentation decision and `report_model`'s contract
/// names it among the RENDERER's prohibitions. The number comes from the
/// project's reference document (`sha256 859880647c…`), parsed through
/// `parse_path` → `extract_from_text`:
///
/// | | chars |
/// |---|---|
/// | body paragraphs | 36 |
/// | median / mean | **728 / 749** |
/// | p90 / max | 1269 / 2131 |
/// | over 300 chars | **32 of 36 (89%)** |
///
/// **Truncation is the NORMAL case, not the exception** — no cap avoids it, so
/// the cap cannot be chosen to minimise how often it fires. What it can be
/// chosen for is whether the reader gets a whole opening sentence, since the
/// snippet's job is to be searchable in their own document:
///
/// | | first sentence, chars |
/// |---|---|
/// | median / mean | 189 / 244 |
/// | **p75** | **270** |
/// | p90 | 439 |
///
/// **350 is where that curve flattens.** Paragraphs receiving less than their
/// first full sentence: 8 at a 300 cap, **5 at 350, and still 5 at 400** — the
/// extra 50 characters buy nothing. At 350 the p75 opening sentence (270) fits
/// whole, and the reader gets it plus the start of the next.
///
/// # PROVENANCE AND ITS WIDTH — n = 1
///
/// **Thirty-six paragraphs from ONE manuscript.** Measured rather than reasoned,
/// which is why it is 350 and not the 300 the design argued for — but measured
/// on a single document, so a second manuscript could shift the first-sentence
/// distribution and move where the curve flattens. **Stated so the figure is not
/// read as a population estimate**, the same discipline the promotion delta was
/// held to. Not a reason to change it; a reason to re-measure before defending
/// it against a second corpus.
const NEARBY_TEXT_CHARS: usize = 350;

/// Cut a quoted paragraph to [`NEARBY_TEXT_CHARS`], **snapping back to the last
/// whitespace** so no word or number is split, and marking the cut with `…`.
///
/// # The snap is load-bearing, and measured
///
/// A blind `chars().take(n)` lands mid-token on 23 of the 32 truncated
/// paragraphs of the reference manuscript, and **inside a NUMBER on 2 of them** —
/// which in a report about statistics is §4.20's TEXT class exactly: `p = 0.03`
/// shown as `p = 0.0` is altered evidence that still reads as evidence. The snap
/// removes the whole class rather than the two instances.
///
/// A token longer than the cap has no whitespace to snap to; it is cut and
/// marked, because showing nothing would be worse than showing a marked prefix.
///
/// # WHY NOT TRIM TO A SENTENCE — the obvious improvement, REFUSED
///
/// **No safe sentence splitter exists in this crate.** `ai_detect::
/// split_sentences` breaks on EVERY `.`, so `p < 0.001` becomes `p < 0.` — in a
/// report whose subject is statistics, that is §4.20's TEXT class committed by
/// the very function meant to make the quotation read better.
/// `docparse::ends_sentence` carries the correct abbreviation logic but is
/// line-oriented and private; reusing it is a shared utility with its own tests,
/// not a tidy-up.
///
/// **And the objection sentence-trimming would answer does not apply here.** The
/// similarity work found chunk-prefix excerpts read as mid-sentence fragments
/// because a CHUNK starts wherever token 448 lands. A PARAGRAPH starts at a
/// blank line, so its head is already a clean sentence start — only the tail is
/// ragged, and `…` marks it. Head-anchoring is also the better shape for the
/// job: the opening of a paragraph is the string the author can search for.
fn shorten(text: &str) -> String {
    let t = text.trim();
    if t.chars().count() <= NEARBY_TEXT_CHARS {
        return t.to_string();
    }
    let head: String = t.chars().take(NEARBY_TEXT_CHARS).collect();
    let cut = match head.rfind(char::is_whitespace) {
        Some(i) => &head[..i],
        None => head.as_str(),
    };
    format!("{}…", cut.trim_end())
}

/// The severities, in the order the report presents them. Explicit rather than
/// derived from an enum ordering, so a reordering is a visible edit.
const SEVERITY_ORDER: [FindingSeverity; 4] = [
    FindingSeverity::Critical,
    FindingSeverity::Major,
    FindingSeverity::Minor,
    FindingSeverity::Info,
];

/// Compose the report. Pure: same model in, same blocks out.
pub fn compose(model: &LocalReportModel) -> Vec<Block> {
    let mut out = Vec::new();
    cover(model, &mut out);
    summary(model, &mut out);
    findings(model, &mut out);
    statistics(model, &mut out);
    checklist(model, &mut out);
    similarity(model, &mut out);
    limitations(model, &mut out);
    out
}

fn cover(model: &LocalReportModel, out: &mut Vec<Block>) {
    // THE RECOMMENDATION LEADS. It is what the report exists to state, and the
    // cover said nothing about it until §52 — the model did not carry it.
    //
    // A missing recommendation is a REAL state (`VerdictWithheld`), so it is
    // said rather than omitted: an absent line reads as approval.
    let mut meta = vec![(
        "Recommendation".into(),
        match model.recommendation {
            Some(r) => crate::vocabulary::recommendation_label(r).to_string(),
            None => "No recommendation was produced for this run".to_string(),
        },
    )];
    meta.push(("Run".into(), model.run_id.clone()));
    if let Some(j) = &model.journal_name {
        meta.push(("Target journal".into(), j.clone()));
    }
    if let Some(g) = &model.guidelines_url {
        meta.push(("Guidelines".into(), g.clone()));
    }
    meta.push(("Findings".into(), model.findings.len().to_string()));
    out.push(Block::Cover {
        title: model.manuscript.title.clone().unwrap_or_else(|| "Untitled manuscript".into()),
        subtitle: "Gaply PublishReady report".into(),
        meta,
    });
}

fn summary(model: &LocalReportModel, out: &mut Vec<Block>) {
    out.push(Block::Heading { text: "Summary".into(), level: 1 });
    // NOT "Overall assessment" — that label belongs to the RECOMMENDATION, and
    // this is the round table's two-value consensus answer (§52.1). One label
    // over two concepts is what let the PDF read "pass" on a MajorRevision run.
    out.push(Block::Paragraph {
        text: format!("Agreement across the checks: {}.", model.verdict),
    });
    out.push(Block::Paragraph {
        text: format!(
            "This manuscript has {} sections, {} tables and {} references.",
            model.manuscript.section_count,
            model.manuscript.table_count,
            model.manuscript.reference_count
        ),
    });
    out.push(Block::Note { text: model.disclaimer.clone() });
}

fn findings(model: &LocalReportModel, out: &mut Vec<Block>) {
    out.push(Block::PageBreak);
    // DECIDED (§31): "Fix these first" became "Issues by severity". The old
    // heading asserted a REMEDIATION ORDER the engine never computed —
    // ONTOLOGY §4.20's PRIORITY class, and its first recorded instance.
    out.push(Block::Heading { text: "Issues by severity".into(), level: 1 });
    out.push(Block::Paragraph {
        text: "Listed from most serious to least serious. The time needed to fix each \
               issue depends on your manuscript."
            .into(),
    });

    if model.findings.is_empty() {
        out.push(Block::Paragraph {
            text: "No issues were found by the checks that ran. The limitations section \
                   lists what was not examined."
                .into(),
        });
        return;
    }

    // NO CAP. `MAX_FINDINGS = 12` exists for the proxy's 8000-character budget
    // (reviewer_agent.rs) — a constraint a PDF does not have. See §31.22.
    let mut n = 0usize;
    for severity in SEVERITY_ORDER {
        let group = model.findings_by_severity(severity);
        if group.is_empty() {
            continue;
        }
        out.push(Block::Heading {
            text: format!("{} ({})", severity_label(severity), group.len()),
            level: 2,
        });
        for f in group {
            n += 1;
            out.push(Block::Heading { text: format!("{n}. {}", f.title), level: 3 });
            out.push(Block::Paragraph { text: f.detail.clone() });
            // The claim label says WHAT KIND of statement this is; absent for
            // ManuscriptDefect, where the finding speaks for itself.
            if let Some(c) = claim_label(f.claim) {
                out.push(Block::Bullet { text: format!("Type: {c}"), indent: 0 });
            }
            out.push(Block::Bullet {
                text: format!("Certainty: {}", tier_label(f.tier)),
                indent: 0,
            });
            // `if let Some` and no `else`: a finding with no location emits NO
            // bullet rather than an empty quotation. Most findings are about the
            // whole document and correctly have none.
            if let Some(near) = &f.nearby_text {
                out.push(Block::Bullet {
                    text: format!("In your manuscript: \"{}\"", shorten(near)),
                    indent: 1,
                });
            }
        }
    }
}

fn statistics(model: &LocalReportModel, out: &mut Vec<Block>) {
    if model.manuscript.statistics.is_empty() {
        return;
    }
    out.push(Block::PageBreak);
    out.push(Block::Heading { text: "Statistics reported".into(), level: 1 });

    let missing = model.statistics_missing_effect_size();
    let thresholds = model.significance_thresholds();
    let with_effect = model.manuscript.statistics.len() - missing.len() - thresholds.len();

    out.push(Block::Heading {
        text: format!("Reported with an effect size ({with_effect})"),
        level: 2,
    });
    if with_effect == 0 {
        out.push(Block::Paragraph { text: "None.".into() });
    } else {
        for s in
            model.manuscript.statistics.iter().filter(|s| !s.is_threshold && s.effect_size_present)
        {
            out.push(Block::Bullet {
                text: format!("{} — {} ({})", s.kind, s.reported, s.location),
                indent: 0,
            });
        }
    }

    out.push(Block::Heading {
        text: format!("Reported without an effect size ({})", missing.len()),
        level: 2,
    });
    if missing.is_empty() {
        out.push(Block::Paragraph { text: "None.".into() });
    } else {
        out.push(Block::Paragraph {
            text: "Many journals ask for an effect size alongside a significance test."
                .into(),
        });
        for s in missing {
            out.push(Block::Bullet {
                text: format!("{} — {} ({})", s.kind, s.reported, s.location),
                indent: 0,
            });
        }
    }

    // A DECLARED CRITERION IS NOT A REPORTED STATISTIC (§42). It is shown,
    // because it is a real fact about the manuscript and the author benefits
    // from seeing the engine read it correctly — but in its own block, because
    // filing it under either effect-size heading would advise adding an effect
    // size to a sentence that reports no result.
    if !thresholds.is_empty() {
        out.push(Block::Heading {
            text: format!("Significance criteria declared ({})", thresholds.len()),
            level: 2,
        });
        out.push(Block::Paragraph {
            text: "These state the threshold your analysis used. They are not results, \
                   and nothing below is a finding about them."
                .into(),
        });
        for s in thresholds {
            out.push(Block::Bullet { text: format!("{} ({})", s.reported, s.location), indent: 0 });
        }
    }
}

/// Shown when the run consulted no journal guidelines at all.
///
/// # Why the heading alone was not enough
///
/// `build_checklist` with `guidelines_url: None` returns the ALWAYS-ON
/// structural checks and nothing else — the honest empty case. But the section
/// was headed "Guideline checklist" either way, so a reader saw a list of met
/// requirements under a heading promising a journal comparison that never
/// happened. §4.12's typed absence, one layer up: the checklist was present,
/// plausible, and not what it appeared to be.
pub const NOTE_NO_GUIDELINES: &str =
    "No journal guidelines were consulted for this run, so nothing below compares your \
     manuscript against a specific journal's requirements. These are Gaply's always-on \
     structural checks. Their silence is not a journal's approval.";

fn checklist(model: &LocalReportModel, out: &mut Vec<Block>) {
    if model.checklist.is_empty() {
        return;
    }
    // The DATA already distinguishes the two kinds: `guideline_source` is the
    // RAG source URL an item came from, and `None` marks the always-on
    // structural checks. Nothing read it.
    let from_guidelines = model.checklist.iter().any(|i| i.guideline_source.is_some());

    out.push(Block::PageBreak);
    out.push(Block::Heading {
        text: if from_guidelines { "Guideline checklist".into() } else { "Structural checks".into() },
        level: 1,
    });
    if !from_guidelines {
        out.push(Block::Note { text: NOTE_NO_GUIDELINES.into() });
    }
    for item in &model.checklist {
        // In a MIXED list the structural items must not read as the journal's
        // requirements either.
        let origin = if from_guidelines && item.guideline_source.is_none() {
            " (structural check, not a journal requirement)"
        } else {
            ""
        };
        out.push(Block::Bullet {
            text: format!(
                "[{}] {} — {}{origin}",
                if item.passed { "met" } else { "not met" },
                item.requirement,
                item.detail
            ),
            indent: 0,
        });
    }
}

fn similarity(model: &LocalReportModel, out: &mut Vec<Block>) {
    out.push(Block::PageBreak);
    out.push(Block::Heading { text: "Text similarity".into(), level: 1 });

    // §26 PR-4's distinction, stated on the page: "no matches" and "nothing to
    // compare against" are different facts and must not share a sentence.
    if model.corpus_chunks_available == 0 {
        out.push(Block::Paragraph {
            text: "No reference corpus was available on this machine, so this manuscript \
                   was not compared against other documents. This is not a statement that \
                   no overlap exists."
                .into(),
        });
    } else if model.similarity.is_empty() {
        out.push(Block::Paragraph {
            text: format!(
                "Compared against {} reference passages. No passage exceeded the \
                 similarity threshold.",
                model.corpus_chunks_available
            ),
        });
    } else {
        out.push(Block::Paragraph {
            text: format!(
                "Compared against {} reference passages. {} passage(s) exceeded the \
                 similarity threshold. Similarity is not plagiarism — quoted, standard \
                 or methodological wording scores highly and is often correct.",
                model.corpus_chunks_available,
                model.similarity.len()
            ),
        });
        for r in &model.similarity {
            out.push(Block::Bullet {
                text: format!(
                    "{:.0}% similar to {}{}",
                    r.similarity * 100.0,
                    r.source,
                    if r.self_match { " (elsewhere in this manuscript)" } else { "" }
                ),
                indent: 0,
            });
            out.push(Block::Bullet { text: format!("\"{}\"", r.excerpt), indent: 1 });
        }
    }
}

fn limitations(model: &LocalReportModel, out: &mut Vec<Block>) {
    out.push(Block::PageBreak);
    out.push(Block::Heading { text: "What was not examined".into(), level: 1 });

    let l = &model.lanes;
    let unexamined: Vec<&str> = [
        (!l.verification_examined, "Reference checking — no references were parsed."),
        (!l.validation_examined, "Statistical checking — no statistics were found."),
        (!l.plagiarism_examined, "Text similarity — nothing was available to compare against."),
        (!l.ai_detection_examined, "AI writing signals — the manuscript was too short to score."),
        (!l.extraction_examined, "Table and reference checks — neither was found."),
    ]
    .iter()
    .filter(|(unex, _)| *unex)
    .map(|(_, s)| *s)
    .collect();

    if unexamined.is_empty() {
        out.push(Block::Paragraph { text: "Every check ran on this manuscript.".into() });
    } else {
        // ONTOLOGY §4.20, COMPLETENESS: a findings list with no statement of
        // what did not run reads as "this is everything".
        out.push(Block::Paragraph {
            text: "These checks did not run. Their silence is not a pass — it means \
                   nothing was looked at."
                .into(),
        });
        for u in unexamined {
            out.push(Block::Bullet { text: u.into(), indent: 0 });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::report_model::{LocalReportModel, ManuscriptFacts, ReportedStatistic};
    use crate::reviewer_agent::LaneExamination;

    fn model_with(stats: Vec<ReportedStatistic>) -> LocalReportModel {
        LocalReportModel {
            run_id: "r".into(),
            manuscript: ManuscriptFacts {
                title: None,
                word_count: 100,
                section_count: 1,
                table_count: 0,
                reference_count: 0,
                statistics: stats,
            },
            journal_name: None,
            guidelines_url: None,
            findings: vec![],
            verdict: "Minor revision".into(),
            recommendation: None,
            combined_confidence: 0.5,
            checklist: vec![],
            similarity: vec![],
            corpus_chunks_available: 0,
            lanes: LaneExamination {
                verification_examined: true,
                validation_examined: true,
                plagiarism_examined: true,
                ai_detection_examined: true,
                extraction_examined: true,
            },
            disclaimer: "d".into(),
        }
    }

    fn stat(kind: &str, reported: &str, effect: bool) -> ReportedStatistic {
        ReportedStatistic {
            kind: kind.into(),
            reported: reported.into(),
            location: "Results, paragraph 1".into(),
            effect_size_present: effect,
            is_threshold: false,
        }
    }

    fn headings(blocks: &[Block]) -> Vec<String> {
        blocks
            .iter()
            .filter_map(|b| match b {
                Block::Heading { text, .. } => Some(text.clone()),
                _ => None,
            })
            .collect()
    }

    /// **The block Milestone 4 exists to fill.** Before `Stat::EffectSize`,
    /// `has_effect_size` returned `false` unconditionally, so this heading read
    /// "Reported with an effect size (0)" followed by "None." on every report
    /// ever produced.
    #[test]
    fn the_reported_with_an_effect_size_block_can_now_fill() {
        let blocks = compose(&model_with(vec![
            stat("p-value", "p = 0.01", true),
            stat("Cohen's d", "d = 0.42", true),
            stat("p-value", "p = 0.20", false),
        ]));
        let h = headings(&blocks);
        assert!(
            h.iter().any(|t| t == "Reported with an effect size (2)"),
            "the with-effect-size block must fill: {h:?}"
        );
        assert!(h.iter().any(|t| t == "Reported without an effect size (1)"), "{h:?}");
        // And "None." must NOT appear under the filled block.
        let text: Vec<String> = blocks
            .iter()
            .filter_map(|b| match b {
                Block::Paragraph { text } => Some(text.clone()),
                _ => None,
            })
            .collect();
        assert_eq!(text.iter().filter(|t| *t == "None.").count(), 0, "{text:?}");
    }

    /// The other direction still reports honestly.
    #[test]
    fn a_report_with_no_effect_sizes_still_says_none() {
        let blocks = compose(&model_with(vec![stat("p-value", "p = 0.01", false)]));
        let h = headings(&blocks);
        assert!(h.iter().any(|t| t == "Reported with an effect size (0)"), "{h:?}");
    }

    fn finding_with(nearby: Option<&str>) -> crate::report_model::LocalFinding {
        crate::report_model::LocalFinding {
            id: "f1".into(),
            severity: FindingSeverity::Major,
            tier: crate::report::CertaintyTier::MathematicallyCertain,
            claim: crate::evidence::ClaimKind::ManuscriptDefect,
            agent: crate::swarm::AgentKind::ValidationMaths,
            title: "statistical rule failed: missing effect size".into(),
            detail: "A p-value is reported without an accompanying effect size.".into(),
            confidence: 1.0,
            provenance: vec!["rule:MissingEffectSize (MAJOR)".into()],
            nearby_text: nearby.map(String::from),
        }
    }

    fn bullets(blocks: &[Block]) -> Vec<String> {
        blocks
            .iter()
            .filter_map(|b| match b {
                Block::Bullet { text, .. } => Some(text.clone()),
                _ => None,
            })
            .collect()
    }

    fn quoted(blocks: &[Block]) -> Vec<String> {
        bullets(blocks).into_iter().filter(|t| t.starts_with("In your manuscript:")).collect()
    }

    /// **An absent quotation emits NO bullet — not an empty one.**
    ///
    /// Most findings are about the whole document and correctly carry no
    /// location. A composer that rendered `In your manuscript: ""` would turn
    /// that correct absence into a claim that the manuscript says nothing.
    #[test]
    fn a_finding_without_a_quotation_emits_no_quotation_bullet() {
        let mut model = model_with(vec![]);
        model.findings = vec![finding_with(None)];
        let blocks = compose(&model);
        assert!(
            quoted(&blocks).is_empty(),
            "no quotation bullet may appear: {:?}",
            bullets(&blocks)
        );
        // The finding itself must still render.
        assert!(
            bullets(&blocks).iter().any(|b| b.starts_with("Certainty:")),
            "the finding must still render its other bullets"
        );
    }

    /// The bullet the report gains, nested under the finding it illustrates.
    #[test]
    fn a_finding_with_a_quotation_emits_it_nested() {
        let mut model = model_with(vec![]);
        model.findings = vec![finding_with(Some("Recall improved with sleep (p = 0.03)."))];
        let blocks = compose(&model);
        let q = quoted(&blocks);
        assert_eq!(q.len(), 1, "{:?}", bullets(&blocks));
        assert_eq!(q[0], "In your manuscript: \"Recall improved with sleep (p = 0.03).\"");
        let indent = blocks.iter().find_map(|b| match b {
            Block::Bullet { text, indent } if text.starts_with("In your manuscript:") => Some(*indent),
            _ => None,
        });
        assert_eq!(indent, Some(1), "the quotation nests under its finding");
    }

    /// **TRUNCATION MUST NOT SPLIT A TOKEN — and a NUMBER is the case that
    /// matters.**
    ///
    /// Measured on the reference manuscript: a blind `chars().take(350)` lands
    /// mid-token on 23 of 32 truncated paragraphs and inside a NUMBER on 2. In a
    /// report whose subject is statistics, `p = 0.03` shown as `p = 0.0` is
    /// §4.20's TEXT class — altered evidence that still reads as evidence.
    #[test]
    fn a_truncated_quotation_never_splits_a_token() {
        // Build a paragraph whose cap-th character falls INSIDE "0.0125":
        // pad to exactly cap-3 chars, then " 0.0125" puts '0' at cap-2, '.' at
        // cap-1 and '0' at cap, so a blind cut yields a trailing "0.".
        let mut pad = String::new();
        while pad.chars().count() < NEARBY_TEXT_CHARS - 3 {
            pad.push_str("word ");
        }
        let pad: String = pad.chars().take(NEARBY_TEXT_CHARS - 3).collect();
        let para = format!("{pad} 0.0125 and more text follows here to force a cut.");
        assert!(para.chars().count() > NEARBY_TEXT_CHARS);
        assert!(
            !para.chars().nth(NEARBY_TEXT_CHARS - 1).unwrap().is_whitespace()
                && !para.chars().nth(NEARBY_TEXT_CHARS).unwrap().is_whitespace(),
            "the fixture must place the raw cut inside a token"
        );

        let mut model = model_with(vec![]);
        model.findings = vec![finding_with(Some(&para))];
        let q = quoted(&compose(&model));
        assert_eq!(q.len(), 1);
        let shown = &q[0];

        assert!(shown.ends_with("…\""), "a truncated quotation must be marked: {shown}");
        // The partial number must not appear. Either the whole value is shown or
        // none of it is; "0.0" or "0.01" as the final token is the defect.
        for partial in ["0.0…", "0.01…", "0.012…"] {
            assert!(!shown.contains(partial), "a number was split: {shown}");
        }
        // And the cut landed on a word boundary of the ORIGINAL text.
        let inner = shown
            .trim_start_matches("In your manuscript: \"")
            .trim_end_matches("…\"");
        assert!(para.starts_with(inner), "the shown prefix must be verbatim: {inner:?}");
        assert!(
            para[inner.len()..].starts_with(char::is_whitespace),
            "the cut must land on a whitespace boundary: {inner:?}"
        );
    }

    fn threshold(reported: &str) -> ReportedStatistic {
        ReportedStatistic {
            kind: "Significance threshold".into(),
            reported: reported.into(),
            location: "Methods, paragraph 1".into(),
            effect_size_present: false,
            is_threshold: true,
        }
    }

    /// **A DECLARED CRITERION IS NOT FILED UNDER EITHER EFFECT-SIZE HEADING.**
    ///
    /// Filing it under "Reported without an effect size" would tell the author
    /// to add an effect size to a sentence that reports no result — the same
    /// mistake `MissingEffectSize` made before §42, reappearing one layer up.
    #[test]
    fn a_significance_criterion_gets_its_own_block_not_the_missing_bucket() {
        let blocks =
            compose(&model_with(vec![stat("p-value", "p = 0.01", false), threshold("p < 0.05")]));
        let h = headings(&blocks);
        assert!(
            h.iter().any(|t| t == "Significance criteria declared (1)"),
            "the criterion must have its own block: {h:?}"
        );
        assert!(
            h.iter().any(|t| t == "Reported without an effect size (1)"),
            "only the real p-value counts as missing one: {h:?}"
        );
        assert!(
            h.iter().any(|t| t == "Reported with an effect size (0)"),
            "and the criterion must not inflate the accompanied count either: {h:?}"
        );
    }


    fn chk(req: &str, source: Option<&str>) -> crate::report::ChecklistItem {
        crate::report::ChecklistItem {
            requirement: req.into(),
            passed: true,
            detail: "d".into(),
            guideline_source: source.map(String::from),
        }
    }

    /// **A CHECKLIST BUILT FROM NO GUIDELINES MUST SAY SO.**
    ///
    /// `build_checklist` with `guidelines_url: None` returns the always-on
    /// structural checks — the honest empty case — but the section was headed
    /// "Guideline checklist" regardless, so a reader saw met requirements under
    /// a heading promising a journal comparison that never happened.
    #[test]
    fn a_checklist_with_no_guidelines_is_labelled_structural_and_discloses_it() {
        let mut m = model_with(vec![]);
        m.checklist = vec![chk("required section: Abstract", None), chk("has references", None)];
        let blocks = compose(&m);
        let h = headings(&blocks);
        assert!(h.iter().any(|t| t == "Structural checks"), "heading must not promise guidelines: {h:?}");
        assert!(!h.iter().any(|t| t == "Guideline checklist"), "{h:?}");
        assert!(
            blocks.iter().any(|b| matches!(b, Block::Note { text } if text == NOTE_NO_GUIDELINES)),
            "the disclosure must be emitted"
        );
    }

    /// The real thing still reads as the real thing.
    #[test]
    fn a_checklist_built_from_guidelines_keeps_its_heading_and_no_disclosure() {
        let mut m = model_with(vec![]);
        m.checklist = vec![chk("word limit 5000", Some("https://journal/guide"))];
        let blocks = compose(&m);
        assert!(headings(&blocks).iter().any(|t| t == "Guideline checklist"));
        assert!(
            !blocks.iter().any(|b| matches!(b, Block::Note { text } if text == NOTE_NO_GUIDELINES)),
            "no disclosure when guidelines WERE consulted"
        );
    }

    /// **IN A MIXED LIST the structural items must not read as the journal's.**
    #[test]
    fn structural_items_are_marked_inside_a_guideline_checklist() {
        let mut m = model_with(vec![]);
        m.checklist =
            vec![chk("word limit 5000", Some("https://journal/guide")), chk("required section: Abstract", None)];
        let bullets = bullets(&compose(&m));
        assert!(
            bullets.iter().any(|b| b.contains("word limit 5000") && !b.contains("structural check")),
            "a real requirement is unmarked: {bullets:?}"
        );
        assert!(
            bullets.iter().any(|b| b.contains("Abstract") && b.contains("structural check, not a journal requirement")),
            "the structural item must be marked: {bullets:?}"
        );
    }
    /// A quotation at or under the cap is shown whole, with no ellipsis.
    #[test]
    fn a_short_quotation_is_not_marked_as_truncated() {
        let mut model = model_with(vec![]);
        model.findings = vec![finding_with(Some("A short sentence (p = 0.03)."))];
        let q = quoted(&compose(&model));
        assert!(!q[0].contains('…'), "an untruncated quotation carries no ellipsis: {}", q[0]);
    }
}
