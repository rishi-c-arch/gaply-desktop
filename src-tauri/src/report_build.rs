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
use gaply_core::extract::{ExtractionResult, Location, SectionKind};
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
        Stat::TestStatistic { name, df, raw, .. } => {
            let dfs: Vec<String> = df.iter().map(|d| d.to_string()).collect();
            (format!("{name}({})", dfs.join(", ")), raw.clone())
        }
        Stat::EffectSize { name, raw, .. } => (name.clone(), raw.clone()),
    }
}

/// Locations at which an effect size was extracted.
///
/// # The join is BY PARAGRAPH, matching the rule it mirrors
///
/// `validate.rs`'s MissingEffectSize fires when no effect size appears in the
/// SAME PARAGRAPH as a p-value. This uses the same unit, so the report's
/// Reported/Missing split and the finding it explains can never disagree — a
/// statistic listed as "reported without an effect size" is exactly one the rule
/// would flag.
///
/// **Milestone 4 replaced a function that returned `false` unconditionally.**
/// The extractor had no effect-size category, so every statistic was reported as
/// missing one — the CORRECT reading of what the engine knew, and a different
/// claim from "the manuscript reports none" (§4.12).
fn effect_size_locations(extraction: &ExtractionResult) -> Vec<Location> {
    extraction
        .statistics
        .iter()
        .filter(|s| matches!(s.stat, Stat::EffectSize { .. }))
        .map(|s| s.location.clone())
        .collect()
}

/// Project every extracted statistic into the report's `ReportedStatistic`.
///
/// Split out of `into_report_model` so the Reported/Missing join is TESTABLE
/// WITHOUT constructing a whole `PipelineResult`. It was inline until a mutation
/// — hard-coding `effect_size_present: false` again — SURVIVED the test suite:
/// the test asserted on `effect_size_locations` rather than on what the model
/// actually carried, so it verified the helper and not the wiring.
pub fn reported_statistics(extraction: &ExtractionResult) -> Vec<ReportedStatistic> {
    let effect_locs = effect_size_locations(extraction);
    extraction
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
                effect_size_present: effect_locs.contains(&s.location),
            }
        })
        .collect()
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

        let statistics = reported_statistics(&self.extraction);

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

#[cfg(test)]
mod tests {
    use super::*;
    use gaply_core::extract::stats::Stat;

    /// The join that makes `effect_size_present` real, end to end from raw text.
    ///
    /// **Same unit as the rule it mirrors:** `validate.rs`'s MissingEffectSize
    /// fires when no effect size appears in the same PARAGRAPH as a p-value, so
    /// a statistic the report lists as "reported without an effect size" is
    /// exactly one that rule would flag. If these used different units the
    /// report would contradict its own finding.
    #[test]
    fn an_effect_size_in_the_same_paragraph_marks_the_p_value_as_accompanied() {
        let text = "Results\n\nThe intervention improved outcomes significantly, \
                    p = 0.01, Cohen's d = 0.42.\n\nA secondary measure was not \
                    significant, p = 0.44.\n";
        let ex = gaply_core::extract::extract_from_text(text);
        assert!(
            ex.statistics.iter().any(|c| matches!(c.stat, Stat::EffectSize { .. })),
            "an effect size must be extracted from the fixture"
        );

        // Asserted on what the MODEL CARRIES, not on the helper — a mutation
        // hard-coding `effect_size_present: false` must fail this.
        let reported = reported_statistics(&ex);
        let p_values: Vec<&ReportedStatistic> =
            reported.iter().filter(|r| r.kind == "p-value").collect();
        assert_eq!(p_values.len(), 2, "fixture must yield two p-values: {reported:?}");
        assert_eq!(
            p_values.iter().filter(|r| r.effect_size_present).count(),
            1,
            "exactly the p-value beside Cohen's d is accompanied: {reported:?}"
        );
    }
}
