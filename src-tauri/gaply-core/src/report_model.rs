//! The ENGINE layer of the report pipeline — "what exists?"
//!
//! ARCHITECTURE_TRACE §31, Milestone 3. Three layers, stated as a CONTRACT
//! rather than a convention, because this boundary will outlive the PDF:
//!
//! | Layer | Question | Forbidden |
//! |---|---|---|
//! | **ENGINE** (this module) | what exists? | naming a page, a font, a column, a heading-as-layout |
//! | **COMPOSER** (`report_compose`) | how should a researcher read this? | computing any manuscript fact |
//! | **RENDERER** (`report_pdf`) | how do these blocks become pages? | any decision — ordering, filtering, ranking, truncation |
//!
//! **The contract in one line: the ENGINE may not name a page, the COMPOSER may
//! not compute a fact, the RENDERER may not make a decision.**
//!
//! Each prohibition is STRUCTURAL rather than advisory:
//!
//! * This module imports nothing from `report_compose` or `report_pdf`, so a
//!   layout concept has no type in which it could be expressed here.
//! * `compose` takes `&LocalReportModel` and returns `Vec<Block>` — it holds no
//!   `Database`, no manuscript text beyond what the model carries, and nothing
//!   to compute a fact FROM. **If the composer needs a number the model does
//!   not have, the model is wrong, not the composer.**
//! * `render_pdf` takes `&[Block]` — it receives no severity to re-rank and no
//!   finding it could drop. A renderer that wants to omit something must ask the
//!   composer to stop emitting it.
//!
//! # NOT `Serialize`, ON PURPOSE — ONTOLOGY §4.22
//!
//! `LocalFinding::nearby_text`, `SimilarityRegion::excerpt` and
//! `ManuscriptFacts::title` are MANUSCRIPT PROSE. This type is therefore one of
//! the two in the codebase that carries what must never leave the machine (the
//! other is `pipeline::PipelineResult`).
//!
//! **The absence of a `Serialize` derive IS the guarantee.** No connector,
//! telemetry hook, export path or debug dump can serialize this by reaching for
//! a derive that happens to exist; adding one is a deliberate architectural
//! decision, which is exactly the friction wanted. This generalises what
//! `build_review_payload` achieves today by CONVENTION — a comment plus a pinned
//! key set — into something the compiler holds.

use crate::evidence::ClaimKind;
use crate::extract::Location;
use crate::report::{CertaintyTier, ChecklistItem, FindingSeverity};
use crate::reviewer_agent::{LaneExamination, Recommendation};
use crate::swarm::AgentKind;

/// **THE paragraph ordinal a reader sees, on every surface: 1-BASED.**
///
/// `Location::paragraph` is a 0-based index into `Section::paragraphs` and is
/// the right thing for code. It is the wrong thing for a person, who counts the
/// first paragraph as 1 — and it used to reach the screen raw.
///
/// # The defect
///
/// Measured 22 Sep 2026: the Inspector showed *"Abstract paragraph 0"* and
/// *"Methods paragraph 125"* for findings the exported PDF called *"paragraph
/// 1"* and *"paragraph 126"*. Same finding, same manuscript, two numbers. A
/// reader comparing the two surfaces has no way to tell which is the paragraph
/// to go and look at, and "paragraph 0" names something that does not exist in
/// any document.
///
/// Every other reader-facing surface was already 1-based and said so:
/// `validate.rs` and `extract/persist.rs` both write `¶{paragraph + 1}`, and
/// `ai_engine::audit_prepass` counts from 1 with a comment explaining that *"a
/// locator has to match what the reader counts in the document"*. Only the
/// provenance line was raw, in Rust and in its frontend mirror.
///
/// So this is not a new convention, it is the existing majority made
/// unavoidable: **both surface formatters live here and both go through this
/// function**, which is what lets one test render the same finding on both and
/// compare.
pub fn human_paragraph(loc: &Location) -> usize {
    loc.paragraph + 1
}

/// **Surface 1 — the Inspector's provenance line.** `Finding.provenance` is
/// rendered to the reader as written, so this string is reader-facing despite
/// looking machine-shaped.
pub fn provenance_location(loc: &Location) -> String {
    format!("location:{:?} paragraph {}", loc.section, human_paragraph(loc))
}

/// **Surface 2 — the report model's human location** ("Results, paragraph 3"),
/// which is what the exported PDF prints. Takes the section name already
/// resolved, because the reader-facing names live with the renderer
/// (`Other` reads "Unlabelled section", not "Other").
pub fn reported_statistic_location(section_name: &str, loc: &Location) -> String {
    format!("{section_name}, paragraph {}", human_paragraph(loc))
}

/// Facts about the manuscript itself, all COUNTED rather than judged.
pub struct ManuscriptFacts {
    pub title: Option<String>,
    pub word_count: usize,
    pub section_count: usize,
    pub table_count: usize,
    pub reference_count: usize,
    /// Every statistic the extractor found, with whether an effect size
    /// accompanied it. Feeds the Reported/Missing block — the report's most
    /// actionable content, and unreachable before §31's threading.
    pub statistics: Vec<ReportedStatistic>,
}

/// One extracted statistical claim, as reported by the manuscript.
///
/// `Debug` but NOT `Serialize` — §4.22 governs serialization, and its own text
/// already records that `Debug` still prints, so this widens nothing the rule
/// claimed to close. `reported` holds a statistic (`p = 0.01`), not prose.
#[derive(Debug)]
pub struct ReportedStatistic {
    /// Test or quantity name as the extractor classified it ("p-value", "t").
    pub kind: String,
    /// The value as WRITTEN in the manuscript, never reformatted — a rendered
    /// "p = .04" that the author wrote as "p<0.05" is altered evidence
    /// (ONTOLOGY §4.20's TEXT class).
    pub reported: String,
    /// Human-readable position, e.g. "Results, paragraph 3".
    pub location: String,
    /// Whether an effect size was found alongside this statistic.
    pub effect_size_present: bool,
    /// True when this is a significance CRITERION (`p ≤ 0.05` as a decision
    /// rule) rather than a reported result. Such a row belongs in neither
    /// effect-size bucket — see `statistics_missing_effect_size`.
    pub is_threshold: bool,
}

/// A finding plus the manuscript context that never crossed the proxy.
pub struct LocalFinding {
    /// `f{N}`, matching the report's evidence ids.
    pub id: String,
    pub severity: FindingSeverity,
    pub tier: CertaintyTier,
    pub claim: ClaimKind,
    pub agent: AgentKind,
    pub title: String,
    pub detail: String,
    pub confidence: f64,
    pub provenance: Vec<String>,
    /// The manuscript sentence this finding is about. LOCAL ONLY — this is the
    /// field whose existence makes this type non-`Serialize`.
    pub nearby_text: Option<String>,
    /// **The other places' sentences, when this row groups several.** A grouped
    /// row says "raised at 8 places"; without these it would show ONE quote and
    /// the other seven would be unreachable, which trades checkable evidence for
    /// a shorter list. `nearby_text` plus these is every place that resolved.
    /// Local-only, like `nearby_text`, and for the same reason. §11 D180.
    pub also_nearby: Vec<String>,
}

/// One similarity region, local-only for the same reason.
pub struct SimilarityRegion {
    pub excerpt: String,
    /// Cosine similarity in [0, 1].
    pub similarity: f64,
    pub source: String,
    /// Self-overlap within the manuscript rather than corpus overlap.
    pub self_match: bool,
}

/// Everything the report is built from. Constructed once, then IMMUTABLE — the
/// composer takes `&LocalReportModel`.
pub struct LocalReportModel {
    pub run_id: String,
    pub manuscript: ManuscriptFacts,
    pub journal_name: Option<String>,
    pub guidelines_url: Option<String>,
    pub findings: Vec<LocalFinding>,
    /// The DEBATE's consensus answer — `"pass"` / `"concern"` — which is a
    /// two-value categorical from the round table, NOT a publication
    /// recommendation. It wore the label "Overall assessment" until §52; one
    /// label over two concepts is what this record keeps finding.
    pub verdict: String,
    /// THE RECOMMENDATION — what the report exists to state.
    ///
    /// Produced by `aggregate_reviewer_verdict` from the severity breakdown,
    /// and absent from this model entirely until §52, which is why the PDF read
    /// "Overall assessment: pass" on a run the log recorded as MajorRevision.
    /// `None` when no recommendation was produced, which is a real state
    /// (`VerdictWithheld`) and must not be rendered as an approval.
    pub recommendation: Option<Recommendation>,
    pub combined_confidence: f64,
    pub checklist: Vec<ChecklistItem>,
    pub similarity: Vec<SimilarityRegion>,
    /// Without this, "no similarity regions" cannot be told apart from "there
    /// was nothing to compare against" — §26 PR-4's distinction, carried
    /// through to the page so the report can state which one happened.
    pub corpus_chunks_available: usize,
    /// Which lanes examined anything, so the report can say what it did NOT
    /// look at rather than implying completeness (ONTOLOGY §4.20, COMPLETENESS).
    pub lanes: LaneExamination,
    pub disclaimer: String,
}

impl LocalReportModel {
    /// Findings in the order the engine produced them — severity-ordered by
    /// `compile_report`. **Not re-sorted here**: a second sort would be a
    /// presentation decision made in the engine.
    pub fn findings_by_severity(&self, severity: FindingSeverity) -> Vec<&LocalFinding> {
        self.findings.iter().filter(|f| f.severity == severity).collect()
    }

    /// Statistics reported WITHOUT an accompanying effect size.
    ///
    /// **Excludes significance criteria.** A declared threshold reports no
    /// result, so "add an effect size to it" is advice about a sentence that
    /// makes no claim — the same mistake `MissingEffectSize` made before §42.
    pub fn statistics_missing_effect_size(&self) -> Vec<&ReportedStatistic> {
        self.manuscript
            .statistics
            .iter()
            .filter(|s| !s.is_threshold && !s.effect_size_present)
            .collect()
    }

    /// Significance criteria the manuscript declared. A fact about the paper,
    /// reported as itself rather than as a statistic it is not.
    pub fn significance_thresholds(&self) -> Vec<&ReportedStatistic> {
        self.manuscript.statistics.iter().filter(|s| s.is_threshold).collect()
    }
}

#[cfg(test)]
mod paragraph_numbering_tests {
    use super::*;
    use crate::extract::SectionKind;

    fn loc(section: SectionKind, paragraph: usize) -> Location {
        Location { section, paragraph, section_index: Some(0) }
    }

    /// The number a surface prints after the word "paragraph".
    fn printed_ordinal(rendered: &str) -> usize {
        let after = rendered
            .split("paragraph ")
            .nth(1)
            .unwrap_or_else(|| panic!("no paragraph ordinal in {rendered:?}"));
        after
            .chars()
            .take_while(char::is_ascii_digit)
            .collect::<String>()
            .parse()
            .unwrap_or_else(|e| panic!("unparseable ordinal in {rendered:?}: {e}"))
    }

    /// **THE SAME FINDING, RENDERED ON BOTH SURFACES, COMPARED.**
    ///
    /// Measured 22 Sep 2026: the Inspector said *"Abstract paragraph 0"* and
    /// *"Methods paragraph 125"* where the exported PDF said *"paragraph 1"*
    /// and *"paragraph 126"* — same finding, same manuscript, two numbers, and
    /// "paragraph 0" names nothing that exists in a document.
    ///
    /// Both cases from that report are in the table below, so the test fails on
    /// the exact inputs the defect was seen with rather than on invented ones.
    #[test]
    fn both_surfaces_give_the_same_finding_the_same_paragraph_number() {
        let cases = [
            (SectionKind::Abstract, 0usize, "Abstract"),
            (SectionKind::Methods, 125, "Methods"),
            (SectionKind::Results, 1, "Results"),
            (SectionKind::Other, 7, "Unlabelled section"),
        ];
        for (section, paragraph, name) in cases {
            let l = loc(section, paragraph);
            let inspector = provenance_location(&l);
            let pdf = reported_statistic_location(name, &l);
            let (a, b) = (printed_ordinal(&inspector), printed_ordinal(&pdf));
            assert_eq!(
                a, b,
                "the same finding is numbered differently on two surfaces:\n                   Inspector: {inspector}\n  PDF:       {pdf}"
            );
            assert_eq!(
                a,
                paragraph + 1,
                "the convention is 1-BASED on every surface a reader sees;                  index {paragraph} must print as {}, got {a} in {inspector:?}",
                paragraph + 1
            );
        }
    }

    /// The first paragraph of a section is "paragraph 1". Pinned separately
    /// because it is the case that makes the bug visible to a reader: no
    /// document has a paragraph 0.
    #[test]
    fn the_first_paragraph_of_a_section_is_never_numbered_zero() {
        let l = loc(SectionKind::Abstract, 0);
        assert_eq!(human_paragraph(&l), 1);
        for rendered in [provenance_location(&l), reported_statistic_location("Abstract", &l)] {
            assert!(
                !rendered.contains("paragraph 0"),
                "a surface printed a paragraph that does not exist: {rendered}"
            );
        }
    }
}
