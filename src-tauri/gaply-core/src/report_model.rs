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
use crate::report::{CertaintyTier, ChecklistItem, FindingSeverity};
use crate::reviewer_agent::LaneExamination;
use crate::swarm::AgentKind;

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
    pub verdict: String,
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
