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
use gaply_core::extract::{has_effect_size_in, paragraph_at, ExtractionResult, Region, SectionKind};
use gaply_core::plagiarism::MatchSource;
use gaply_core::reviewer_agent::Recommendation;
use gaply_core::report_model::{
    LocalFinding, LocalReportModel, ManuscriptFacts, ReportedStatistic, SimilarityRegion,
};

use crate::pipeline::PipelineResult;

/// Human-readable section name for a `Location`.
/// **The quoted evidence for a located finding: the SENTENCE holding the
/// statistic, falling back to the whole paragraph.**
///
/// # The defect
///
/// `nearby_text`'s own doc says *"the manuscript SENTENCE this finding is
/// about"*, and it was populated with the whole paragraph. The composer then
/// head-anchors at `NEARBY_TEXT_CHARS = 350` (`report_compose::shorten`), which
/// is correct for a paragraph — its opening is the string an author can search
/// for — and which means a statistic past character 350 CANNOT appear in the
/// quotation.
///
/// Measured 22 Sep 2026 on R PAPER (docs/PROBLEM_DOSSIER.md A4): the flagged
/// p-value sits at **offset 1250 of a 1333-character abstract**, so the reader
/// saw "missing effect size" beside 350 characters of prose containing no
/// p-value at all. A finding a reader cannot check against its own evidence is
/// the truncated-span defect in the findings path.
///
/// # Why this is not a re-derivation
///
/// `paragraph_at` still resolves the paragraph — the same resolver the rule
/// used, over the same text. The narrowing is located by finding the
/// statistic's OWN verbatim text (`Stat::raw`) inside that paragraph, so the
/// quotation remains a substring of what the rule evaluated. When no statistic
/// of this finding's location occurs in the paragraph — every non-statistical
/// finding — the paragraph is returned unchanged, which is the previous
/// behaviour.
fn quoted_evidence_in(
    extraction: &ExtractionResult,
    loc: &gaply_core::extract::Location,
) -> Option<String> {
    let paragraph = paragraph_at(extraction, loc)?;
    // Matched on SECTION + PARAGRAPH, deliberately not on the whole `Location`.
    // A finding's location may carry `section_index: None` (pre-§11 D169 data,
    // and several in-tree producers) while every extracted claim carries
    // `Some`, and full equality then never matches — the narrowing would fall
    // back to the paragraph SILENTLY and this fix would do nothing. The
    // paragraph text is the real disambiguator: a claim from a different
    // paragraph simply will not be found inside this one.
    let at = extraction
        .statistics
        .iter()
        .filter(|s| s.location.section == loc.section && s.location.paragraph == loc.paragraph)
        .find_map(|s| paragraph.find(s.stat.raw()));
    Some(match at {
        Some(i) => gaply_core::extract::sentence::sentence_containing(paragraph, i).to_string(),
        None => paragraph.to_string(),
    })
}

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
        // FORCED DECISION (§42). Named for what it is. Presenting a criterion
        // as a reported statistic is ONTOLOGY §4.20's LABELS class — the value
        // is unchanged, the terminology asserts something the engine did not
        // find.
        Stat::SignificanceThreshold { raw, .. } => ("Significance threshold".into(), raw.clone()),
    }
}

/// Project every extracted statistic into the report's `ReportedStatistic`.
///
/// Split out of `into_report_model` so the Reported/Missing join is TESTABLE
/// WITHOUT constructing a whole `PipelineResult`. It was inline until a mutation
/// — hard-coding `effect_size_present: false` again — SURVIVED the test suite:
/// the test asserted on `effect_size_locations` rather than on what the model
/// actually carried, so it verified the helper and not the wiring.
pub fn reported_statistics(extraction: &ExtractionResult) -> Vec<ReportedStatistic> {
    extraction
        .statistics
        .iter()
        .map(|s| {
            let (kind, reported) = describe(&s.stat);
            ReportedStatistic {
                kind,
                reported,
                location: gaply_core::report_model::reported_statistic_location(
                    section_name(s.location.section),
                    &s.location,
                ),
                // THE SAME QUESTION THE RULE ASKS, through the same function
                // (§49). Before shape 3 the rule scanned TEXT while this joined
                // on typed locations, so the report could list a statistic as
                // "reported without an effect size" that the rule had declined
                // to flag. Both now read `has_effect_size_in` over the same
                // region, so they cannot disagree.
                effect_size_present: has_effect_size_in(extraction, &Region::paragraph(&s.location)),
                // A criterion belongs in NEITHER effect-size bucket: "reported
                // without an effect size" would tell the author to add one to a
                // sentence that reports nothing.
                is_threshold: matches!(s.stat, Stat::SignificanceThreshold { .. }),
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
    /// Project the run into the report's engine model, BORROWING it.
    ///
    /// `into_report_model` consumes `self` and is what callers that own the run
    /// use. This variant exists because the PDF must be rendered DURING the run
    /// — the model carries manuscript prose and is deliberately not
    /// serializable (§4.22), so it cannot be rebuilt later from anything
    /// persisted. The body already cloned nearly every field; this makes the
    /// remaining two clones explicit rather than moving.
    pub fn report_model(
        &self,
        journal_name: Option<String>,
        guidelines_url: Option<String>,
        recommendation: Option<Recommendation>,
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
                // The report's own label, carried — not `tier.label()`, which
                // is the claim D214 withdrew. §11 D220.
                certainty_label: f.certainty_label.clone(),
                claim: f.claim,
                agent: f.agent,
                title: f.title.clone(),
                detail: f.detail.clone(),
                confidence: f.confidence,
                provenance: f.provenance.clone(),
                // TYPED ABSENCE, not an empty string, at BOTH steps: a finding
                // with no `Location` is about the whole document, and a
                // `Location` that does not resolve produces no quotation rather
                // than an empty one.
                //
                // THE SAME RESOLVER THE RULE USED. `paragraph_at` is the
                // promoted body of `validate::paragraph`, which is the text the
                // five deterministic rules evaluate. The quotation is therefore
                // the string that produced the finding, not a re-derivation of
                // where we think the finding meant — a distinction §4.20's TEXT
                // class exists to protect, in the field built to serve it.
                //
                // NOT TRUNCATED HERE. Truncation is a presentation decision and
                // belongs to the composer (`report_model`'s three-layer
                // contract names it among the RENDERER's prohibitions, and the
                // ENGINE holds facts). The model carries the paragraph whole.
                nearby_text: f
                    .location
                    .as_ref()
                    .and_then(|loc| quoted_evidence_in(&self.extraction, loc)),
                // Resolved through the SAME `paragraph_at`, so a grouped row's
                // other occurrences are quoted exactly as its first is. A
                // location that does not resolve is SKIPPED rather than quoted
                // empty — `Some("")` is a row claiming evidence and showing
                // none. §11 D180.
                also_nearby: f
                    .also_at
                    .iter()
                    .filter_map(|loc| quoted_evidence_in(&self.extraction, loc))
                    .filter(|p| !p.trim().is_empty())
                    .collect(),
            })
            .collect();

        LocalReportModel {
            run_id: self.report_id.clone(),
            manuscript: ManuscriptFacts {
                title: self.extraction.title.clone(),
                word_count,
                section_count: self.extraction.sections.len(),
                // **§11 D213: count TABLES, not sightings.** This is the one
                // number a user reads ("This manuscript has N tables"), and it
                // was the sighting count — 0 for R PAPER, whose five tables are
                // captioned "TABLE II.", and 6 for a paper with 5. The rule
                // lives in `ExtractionResult::table_count` so the corpus probe
                // measures the function this line calls, not a copy of it.
                table_count: self.extraction.table_count(),
                reference_count: self.extraction.references.len(),
                statistics,
            },
            journal_name,
            guidelines_url,
            findings,
            verdict: self.report.verdict.clone(),
            recommendation,
            combined_confidence: self.report.combined_confidence,
            checklist: self.report.checklist.clone(),
            similarity,
            corpus_chunks_available: self.plagiarism.corpus_chunks_available,
            lanes: self.lanes,
            disclaimer: self.report.disclaimer.clone(),
        }
    }

    /// Consuming form, kept for callers that own the run and are done with it.
    pub fn into_report_model(
        self,
        journal_name: Option<String>,
        guidelines_url: Option<String>,
        recommendation: Option<Recommendation>,
    ) -> LocalReportModel {
        self.report_model(journal_name, guidelines_url, recommendation)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gaply_core::extract::Location;
    use gaply_core::extract::stats::Stat;


    use gaply_core::extract::SectionKind;
    use gaply_core::plagiarism::PlagiarismReport;
    use gaply_core::report::{
        CertaintyTier, DebateSummary, Finding, FindingSeverity, PublishReadyReport,
    };
    use gaply_core::reviewer_agent::LaneExamination;
    use gaply_core::swarm::AgentKind;
    use gaply_core::evidence::ClaimKind;

    /// Two Results paragraphs with distinguishable markers, so a quotation
    /// resolved from the wrong index shows the OTHER paragraph's text.
    const TWO_PARAGRAPHS: &str = "Located\n\nMethods\nThe gamma marker was \
        measured (p = 0.04).\n\nResults\nThe ALPHAMARKER improved recall \
        (p = 0.01).\n\nThe BETAMARKER reduced errors (p = 0.02).\n";

    /// A Results paragraph whose statistic sits PAST the composer's 350-char
    /// display window — the shape measured on R PAPER, where the flagged
    /// p-value was at offset 1250 of a 1333-character abstract.
    const STAT_PAST_THE_WINDOW: &str = "Located\n\nResults\nWe evaluated the \
        proposed architecture against eight published baselines across four \
        public corpora. Every configuration was repeated with five random \
        seeds and we report the mean of the held-out folds. The preprocessing, \
        the tokenisation, the embedding initialisation and the optimiser \
        schedule are described in the order they were applied so that an \
        independent group can repeat the work. We further conducted ablation \
        studies over each component in turn to isolate its contribution. The \
        headline comparison against the strongest baseline was ZETAMARKER \
        (p = 0.03).\n";


    fn located(location: Option<gaply_core::extract::Location>) -> Finding {
        Finding {
            also_at: Vec::new(),
            severity: FindingSeverity::Major,
            tier: CertaintyTier::MathematicallyCertain,
            certainty_label: CertaintyTier::MathematicallyCertain.label().into(),
            agent: AgentKind::ValidationMaths,
            claim: ClaimKind::ManuscriptDefect,
            title: "statistical rule failed".into(),
            detail: "d".into(),
            confidence: 1.0,
            provenance: vec!["rule:MissingEffectSize (MAJOR)".into()],
            location,
        }
    }

    /// A whole `PipelineResult`, because the join under test lives in
    /// `into_report_model` and the assertion must be made on WHAT THE MODEL
    /// CARRIES. The doc comment on `reported_statistics` records why: a mutation
    /// once survived because a test asserted on the helper instead.
    fn pipeline_with(findings: Vec<Finding>) -> PipelineResult {
        pipeline_from(TWO_PARAGRAPHS, findings)
    }

    fn pipeline_from(text: &str, findings: Vec<Finding>) -> PipelineResult {
        PipelineResult {
            report_id: "r1".into(),
            // Additive field; this fixture exercises the report join, not the
            // specialist stage.
            specialists: Vec::new(),
            report: PublishReadyReport {
                verdict: "Minor revision".into(),
                combined_confidence: 0.5,
                findings,
                evidence: vec![],
                checklist: vec![],
                debate: DebateSummary {
                    rounds_run: 1,
                    converged: true,
                    overridden_by_constraint: false,
                    rejected_agents: vec![],
                    revised_agents: vec![],
                },
                harness_notes: vec![],
                disclaimer: "d".into(),
            },
            // Derived from the same empty extraction this fixture uses, so the
            // fixture stays a projection of its own inputs rather than carrying
            // a hand-written state that could disagree with them.
            research_state: gaply_core::research_state::ResearchState::from_extraction(
                &Default::default(),
            ),
            lanes: LaneExamination {
                verification_examined: false,
                validation_examined: true,
                plagiarism_examined: false,
                ai_detection_examined: false,
                extraction_examined: true,
            },
            extraction: gaply_core::extract::extract_from_text(text),
            plagiarism: PlagiarismReport {
                chunk_count: 0,
                corpus_chunks_available: 0,
                threshold: 0.8,
                corpus_matches: vec![],
                self_matches: vec![],
                note: "n".into(),
            },
            text: text.into(),
        }
    }

    /// **§11 D213: "This manuscript has N tables" counts tables.** Four
    /// sightings naming two tables — a contents row, a caption and a sentence
    /// for Table 1, a roman caption for Table II — are two. This line showed
    /// the sighting count until D213: 4 here, 0 for R PAPER's five tables.
    #[test]
    fn the_table_count_a_reader_sees_is_tables_not_sightings() {
        let p = pipeline_from(
            "Contents\n\nTable 1 Outcomes by arm 12\n\nResults\n\nTable 1 Outcomes by arm\n\n\
             Table 1 shows the outcomes.\n\nTABLE II. COMPARATIVE PERFORMANCE\n",
            vec![],
        );
        assert_eq!(p.extraction.table_mentions.len(), 4, "precondition: four sightings");
        assert_eq!(p.report_model(None, None, None).manuscript.table_count, 2);
    }

    /// **The model carries each finding's OWN certainty label. §11 D220.**
    ///
    /// The PDF's "Certainty:" line reads `LocalFinding::certainty_label`, and
    /// this is the only place it is filled. The fixture's tier is the certain
    /// one and its label is the rule's, so filling it from `tier.label()` — what
    /// the PDF effectively did until D220 — fails the equality.
    #[test]
    fn the_model_carries_each_findings_own_certainty_label() {
        let mut f = located(None);
        f.certainty_label = gaply_core::vocabulary::rule_certainty_label(
            gaply_core::validate::RuleId::MissingEffectSize,
        )
        .into();
        assert_ne!(f.certainty_label, f.tier.label(), "precondition: label and tier must differ");
        let model = pipeline_with(vec![f.clone()]).report_model(None, None, None);
        assert_eq!(model.findings.len(), 1);
        assert_eq!(model.findings[0].certainty_label, f.certainty_label);
    }

    fn loc(section: SectionKind, paragraph: usize) -> Option<Location> {
        Some(Location { section, paragraph, section_index: None })
    }

    /// **Each located finding quotes ITS OWN paragraph.**
    ///
    /// The mutations this is built to kill all leave the plumbing intact and the
    /// resolution wrong: `paragraphs.get(0)` (index ignored), `paragraph + 1` /
    /// `paragraph - 1` (off by one), and dropping the `find(kind)` filter
    /// (section ignored). Each produces a real, plausible sentence from the
    /// wrong place — which is the failure `nearby_text` exists to prevent.
    /// **A finding must quote the sentence holding the statistic it is about.**
    ///
    /// Measured 22 Sep 2026 on R PAPER (docs/PROBLEM_DOSSIER.md A4): the flagged
    /// p-value sat at offset 1250 of a 1333-character abstract while the
    /// composer head-anchors at `NEARBY_TEXT_CHARS = 350`, so the reader was
    /// shown "missing effect size" beside 350 characters containing no p-value.
    /// The quotation could not be checked against the claim it supported.
    ///
    /// Note what is NOT changed: `is_significance_criterion` and the rule
    /// itself. This makes the evidence visible; whether the rule should have
    /// fired at all is the separate, measured lexicon gap (§11).
    #[test]
    fn a_located_finding_quotes_the_sentence_holding_its_statistic() {
        let model = pipeline_from(STAT_PAST_THE_WINDOW, vec![located(loc(SectionKind::Results, 0))])
            .into_report_model(None, None, None);
        let near = model.findings[0].nearby_text.as_deref().expect("a quotation");

        // 1. ACCEPTANCE: the flagged value is IN the quoted evidence.
        assert!(
            near.contains("p = 0.03"),
            "the quotation does not contain the statistic the finding is about: {near:?}"
        );
        // 2. It is the sentence, not the paragraph: the paragraph's opening
        //    prose is no longer dragged along.
        assert!(
            !near.contains("eight published baselines"),
            "the whole paragraph is still being quoted: {near:?}"
        );
        // 3. And it survives the composer's 350-char window, which is the whole
        //    point — a sentence the renderer clips has not been fixed.
        assert!(
            near.chars().count() <= 350,
            "the quoted sentence is {} chars and will be clipped by the renderer: {near:?}",
            near.chars().count()
        );
    }

    /// **NEGATIVE CONTROL: a finding whose statistic already sits inside the
    /// quoted window renders unchanged.** If this moves, the change is not
    /// narrowing evidence, it is rewriting quotations generally.
    #[test]
    fn a_finding_whose_statistic_is_already_in_view_renders_unchanged() {
        let model = pipeline_with(vec![
            located(loc(SectionKind::Results, 0)),
            located(loc(SectionKind::Methods, 0)),
        ])
        .into_report_model(None, None, None);
        let near: Vec<&str> = model
            .findings
            .iter()
            .map(|f| f.nearby_text.as_deref().unwrap_or("<none>"))
            .collect();
        assert!(near[0].contains("ALPHAMARKER") && near[0].contains("p = 0.01"), "{near:?}");
        assert!(near[1].contains("gamma marker") && near[1].contains("p = 0.04"), "{near:?}");
        // Still each paragraph's own text — the section/index filter is intact.
        assert!(!near[0].contains("BETAMARKER"), "{near:?}");
        assert!(!near[1].contains("ALPHAMARKER"), "{near:?}");
    }

    fn each_located_finding_quotes_its_own_paragraph() {
        let model = pipeline_with(vec![
            located(loc(SectionKind::Results, 0)),
            located(loc(SectionKind::Results, 1)),
            located(loc(SectionKind::Methods, 0)),
        ])
        .into_report_model(None, None, None);

        let near: Vec<&str> = model
            .findings
            .iter()
            .map(|f| f.nearby_text.as_deref().unwrap_or("<none>"))
            .collect();

        assert!(near[0].contains("ALPHAMARKER"), "Results ¶1: {near:?}");
        assert!(!near[0].contains("BETAMARKER"), "Results ¶1 quoted ¶2: {near:?}");
        assert!(near[1].contains("BETAMARKER"), "Results ¶2: {near:?}");
        assert!(!near[1].contains("ALPHAMARKER"), "Results ¶2 quoted ¶1: {near:?}");
        assert!(near[2].contains("gamma marker"), "Methods ¶1: {near:?}");
        assert!(
            !near[2].contains("ALPHAMARKER") && !near[2].contains("BETAMARKER"),
            "Methods ¶1 quoted a Results paragraph — the section filter was dropped: {near:?}"
        );
    }

    /// **Absence stays typed at BOTH steps.** No location, and an unresolvable
    /// location, must each yield `None` — never `Some("")`, which would render
    /// as an empty quotation asserting the manuscript says nothing.
    #[test]
    fn an_absent_or_unresolvable_location_yields_no_quotation() {
        let model = pipeline_with(vec![
            located(None),
            located(loc(SectionKind::Results, 99)),
            located(loc(SectionKind::Discussion, 0)),
        ])
        .into_report_model(None, None, None);

        for (i, f) in model.findings.iter().enumerate() {
            assert_eq!(f.nearby_text, None, "finding {i} must carry no quotation");
        }
    }

    /// The quotation is VERBATIM — the engine carries the fact, the composer
    /// cuts it. A model that pre-truncated would put a presentation decision in
    /// the engine and make the fact unrecoverable downstream.
    ///
    /// **Narrowed 22 Sep 2026, and this fixture is why it still reads as
    /// "whole":** the quotation is now the SENTENCE holding the finding's
    /// statistic (`quoted_evidence_in`), because a statistic past the
    /// composer's 350-char window could not appear in the quotation at all
    /// (docs/PROBLEM_DOSSIER.md A4). `TWO_PARAGRAPHS`' Results ¶0 is a single
    /// sentence, so sentence and paragraph coincide here and the assertion is
    /// unchanged — which is exactly the negative control
    /// `a_finding_whose_statistic_is_already_in_view_renders_unchanged` states
    /// deliberately. What is still verbatim is that nothing is TRUNCATED: the
    /// engine emits a complete sentence, never an ellipsis.
    #[test]
    fn the_model_carries_the_quotation_untruncated() {
        let ex = gaply_core::extract::extract_from_text(TWO_PARAGRAPHS);
        let expected = gaply_core::extract::paragraph_at(
            &ex,
            &Location { section: SectionKind::Results, paragraph: 0, section_index: None },
        )
        .unwrap();

        let model =
            pipeline_with(vec![located(loc(SectionKind::Results, 0))]).into_report_model(None, None, None);
        assert_eq!(model.findings[0].nearby_text.as_deref(), Some(expected));
    }

    /// **THE BORROWING BUILDER AND THE CONSUMING ONE AGREE.**
    ///
    /// `report_model` exists so the PDF can be rendered DURING the run without
    /// consuming the result, and `into_report_model` DELEGATES to it.
    ///
    /// # What this guards, and what it CANNOT
    ///
    /// Because of the delegation the two share one body, so **a defect in that
    /// body changes both and this test still passes** — verified by mutation:
    /// dropping `nearby_text` from the builder leaves this green. It is not
    /// vacuous, but its guarantee is narrower than it looks: **it fires only if
    /// someone gives `into_report_model` its own implementation again**, which
    /// is the re-duplication that would reintroduce drift.
    ///
    /// **The body itself is covered by
    /// `the_rendered_report_contains_the_blocks_the_summary_export_lacks`**,
    /// which the same mutation DOES fail.
    #[test]
    fn the_borrowing_and_consuming_model_builders_agree() {
        let a = pipeline_with(vec![located(loc(SectionKind::Results, 0))])
            .report_model(Some("J".into()), None, None);
        let b = pipeline_with(vec![located(loc(SectionKind::Results, 0))])
            .into_report_model(Some("J".into()), None, None);

        assert_eq!(a.run_id, b.run_id);
        assert_eq!(a.verdict, b.verdict);
        assert_eq!(a.journal_name, b.journal_name);
        assert_eq!(a.manuscript.word_count, b.manuscript.word_count);
        assert_eq!(a.manuscript.section_count, b.manuscript.section_count);
        assert_eq!(a.manuscript.statistics.len(), b.manuscript.statistics.len());
        assert_eq!(a.findings.len(), b.findings.len());
        assert_eq!(
            a.findings.iter().map(|f| f.nearby_text.clone()).collect::<Vec<_>>(),
            b.findings.iter().map(|f| f.nearby_text.clone()).collect::<Vec<_>>(),
            "the quotations must survive the borrowing path — they are the reason it exists"
        );
    }

    /// **THE RENDERED REPORT CARRIES WHAT THE SUMMARY EXPORT CANNOT.**
    ///
    /// The TS `exportPdf.ts` reads only `report.findings`. This asserts the
    /// Rust path emits the blocks that motivated wiring it at all, so a
    /// regression that quietly reduced it to a findings list would fail here.
    #[test]
    fn the_rendered_report_contains_the_blocks_the_summary_export_lacks() {
        use gaply_core::report_compose::{compose, Block};
        let model =
            pipeline_with(vec![located(loc(SectionKind::Results, 0))]).report_model(None, None, None);
        let headings: Vec<String> = compose(&model)
            .iter()
            .filter_map(|b| match b {
                Block::Heading { text, .. } => Some(text.clone()),
                _ => None,
            })
            .collect();
        for expected in ["Statistics reported", "Text similarity", "What was not examined"] {
            assert!(
                headings.iter().any(|h| h == expected),
                "the composed report must contain {expected:?}: {headings:?}"
            );
        }
        let quoted = compose(&model).iter().any(|b| matches!(
            b, Block::Bullet { text, .. } if text.starts_with("In your manuscript:")));
        assert!(quoted, "the quotation block must reach the rendered report");
    }

    /// A rendered report is real bytes, not an empty buffer.
    #[test]
    fn the_run_produces_a_non_trivial_pdf() {
        use gaply_core::{report_compose::compose, report_pdf::render_pdf};
        let model =
            pipeline_with(vec![located(loc(SectionKind::Results, 0))]).report_model(None, None, None);
        let bytes = render_pdf(&compose(&model));
        assert!(bytes.starts_with(b"%PDF"), "must be a PDF");
        assert!(bytes.len() > 2000, "a real report is not a stub: {} bytes", bytes.len());
    }

    /// **A DECLARED CRITERION REACHES THE REPORT AS A CRITERION**, end to end
    /// from raw text — §42. Asserted on what the MODEL carries, not on
    /// `describe`, for the reason recorded on `reported_statistics`.
    #[test]
    fn a_declared_criterion_reaches_the_report_as_a_criterion() {
        let text = "Study\n\nMethods\nStatistical significance was determined at p < 0.05.\n\n\
                    Results\nThe intervention improved recall (p = 0.002).\n";
        let ex = gaply_core::extract::extract_from_text(text);
        let reported = reported_statistics(&ex);

        let thresholds: Vec<&ReportedStatistic> =
            reported.iter().filter(|r| r.is_threshold).collect();
        assert_eq!(thresholds.len(), 1, "exactly one criterion: {reported:?}");
        assert_eq!(thresholds[0].kind, "Significance threshold");
        assert_eq!(thresholds[0].reported, "p < 0.05", "the value as WRITTEN, unaltered");

        let results: Vec<&ReportedStatistic> =
            reported.iter().filter(|r| !r.is_threshold && r.kind == "p-value").collect();
        assert_eq!(results.len(), 1, "the reported p-value is untouched: {reported:?}");
        assert_eq!(results[0].reported, "p = 0.002");
    }

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
