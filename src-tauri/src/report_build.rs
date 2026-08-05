//! Builds the ENGINE layer's `LocalReportModel` from what the pipeline produced.
//!
//! Lives in the app crate because `PipelineResult` does. This is the ONLY place
//! the two sources §31.2 found unreachable — `ExtractionResult` and
//! `PlagiarismReport` — are read into the report, and it exists so the threading
//! added in Milestone 3 has a consumer rather than becoming a sixth instance of
//! the projection pattern (§22.4): values produced, carried, and dropped.
//!
//! It computes NO judgement. Every field is a count, a copy, or a rendering of
//! something the engine already decided.

use gaply_core::extract::stats::Stat;
use gaply_core::extract::SectionKind;
use gaply_core::plagiarism::MatchSource;
use gaply_core::report_model::{
    LocalFinding, LocalReportModel, ManuscriptFacts, ReportedStatistic, SimilarityRegion,
};

use crate::pipeline::PipelineResult;

/// Human-readable section name for a `Location`.
fn section_name(kind: SectionKind) -> &'static str {
    match kind {
        SectionKind::Abstract => "Abstract",
        SectionKind::Introduction => "Introduction",
        SectionKind::Methods => "Methods",
        SectionKind::Results => "Results",
        SectionKind::Discussion => "Discussion",
        SectionKind::Conclusion => "Conclusion",
        SectionKind::References => "References",
        // Front matter or an unrecognised heading. Named for a reader rather
        // than by the variant, per ONTOLOGY §4.21.
        SectionKind::Other => "Unlabelled section",
    }
}

/// `(kind, reported)` for one extracted statistic. `reported` is the RAW string
/// as the manuscript wrote it — never reformatted, because a re-rendered value
/// is altered evidence (ONTOLOGY §4.20, TEXT).
fn describe(stat: &Stat) -> (String, String) {
    match stat {
        Stat::PValue { raw, .. } => ("p-value".into(), raw.clone()),
        Stat::ConfidenceInterval { raw, .. } => ("Confidence interval".into(), raw.clone()),
        Stat::SampleSize { raw, .. } => ("Sample size".into(), raw.clone()),
        Stat::Test { name, raw } => (name.clone(), raw.clone()),
    }
}

/// Whether an effect size accompanies this statistic.
///
/// **TYPED ABSENCE, honestly reported (§4.12).** The extractor has no effect-size
/// category today, so this is `false` for every statistic and the composer's
/// "Reported without an effect size" list is currently the full list. That is
/// the CORRECT reading of what the engine knows — an effect size was not found —
/// and it is not the same claim as "the manuscript reports none". Milestone 4's
/// engine work (`Stat::EffectSize`) is what makes the distinction real; until
/// then this function is the single place that changes.
fn has_effect_size(_stat: &Stat) -> bool {
    false
}

impl PipelineResult {
    /// Project the run into the report's engine model.
    ///
    /// `journal_name` and `guidelines_url` come from the CALLER because they are
    /// the user's stated intent, propagated rather than reconstructed from
    /// corpus state (§18.6's rule, applied again here).
    pub fn into_report_model(
        self,
        journal_name: Option<String>,
        guidelines_url: Option<String>,
    ) -> LocalReportModel {
        let word_count = self.text.split_whitespace().count();

        let statistics = self
            .extraction
            .statistics
            .iter()
            .map(|s| {
                let (kind, reported) = describe(&s.stat);
                ReportedStatistic {
                    kind,
                    reported,
                    location: format!(
                        "{}, paragraph {}",
                        section_name(s.location.section),
                        s.location.paragraph + 1
                    ),
                    effect_size_present: has_effect_size(&s.stat),
                }
            })
            .collect();

        let similarity = self
            .plagiarism
            .corpus_matches
            .iter()
            .chain(self.plagiarism.self_matches.iter())
            .map(|m| SimilarityRegion {
                excerpt: m.manuscript_excerpt.clone(),
                similarity: m.similarity,
                source: match &m.source {
                    MatchSource::Corpus { title, .. } => title.clone(),
                    MatchSource::SelfManuscript { .. } => "another passage in this manuscript".into(),
                },
                self_match: matches!(m.source, MatchSource::SelfManuscript { .. }),
            })
            .collect();

        let findings = self
            .report
            .findings
            .iter()
            .enumerate()
            .map(|(i, f)| LocalFinding {
                id: format!("f{}", i + 1),
                severity: f.severity,
                tier: f.tier,
                claim: f.claim,
                agent: f.agent,
                title: f.title.clone(),
                detail: f.detail.clone(),
                confidence: f.confidence,
                provenance: f.provenance.clone(),
                // TYPED ABSENCE, not an empty string: `Finding` carries no
                // `Location`, so there is nothing to look up text near. Milestone
                // 4 adds it; until then the report simply does not show a
                // manuscript quotation, which is honest.
                nearby_text: None,
            })
            .collect();

        LocalReportModel {
            run_id: self.report_id,
            manuscript: ManuscriptFacts {
                title: self.extraction.title.clone(),
                word_count,
                section_count: self.extraction.sections.len(),
                table_count: self.extraction.tables.len(),
                reference_count: self.extraction.references.len(),
                statistics,
            },
            journal_name,
            guidelines_url,
            findings,
            verdict: self.report.verdict.clone(),
            combined_confidence: self.report.combined_confidence,
            checklist: self.report.checklist.clone(),
            similarity,
            corpus_chunks_available: self.plagiarism.corpus_chunks_available,
            lanes: self.lanes,
            disclaimer: self.report.disclaimer.clone(),
        }
    }
}
