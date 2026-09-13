//! Report Compiler tests — offline; the only "network" is a MockProxyClient.

use serde_json::json;

/// Fixed "now" for the reference-recency count. Injected rather than read from
/// the clock so these assertions never change with the calendar.
const TEST_YEAR: i32 = 2025;

use super::*;
use crate::extract::citations::Reference;
use crate::refverify::{ExistenceCheck, Provenance, ReferenceVerification, UntrustedText};
use crate::swarm::{
    adapters, run_debate, DebateConfig, Opinion, PrecomputedAgent, RevisingVerificationAgent,
    SwarmAgent, ANSWER_PASS,
};
use crate::verify_agent::{verify_citations, CitationVerdict, MockProxyClient, VerificationReport, Verdict};

/// The Prompt-19 e2e sample manuscript (stats-clean, one DOI-bearing reference).
const MANUSCRIPT: &str = "Sleep and Memory\n\nAbstract\nA randomized trial (n = 96) found \
    improved recall (p < 0.01, d = 0.62, 95% CI [0.31, 0.93]).\n\nMethods\nWe used a paired \
    t-test. The authors declare no conflict of interest.\n\nResults\nRecall improved \
    significantly in the extended-sleep group.\n\nReferences\nDoe, J. (2022). Sleep and \
    memory. Journal of Rest, 1(1), 1-9. https://doi.org/10.1/zzz\n";

fn prov(url: &str) -> Provenance {
    Provenance {
        source: "crossref".into(),
        url: url.into(),
        fetched_at: 1_000,
        checksum: "c".repeat(64),
        from_cache: false,
    }
}

fn verification_items(reference: Reference) -> Vec<(Reference, ReferenceVerification)> {
    let p = prov("https://api.crossref.org/works/10.1/zzz");
    let rv = ReferenceVerification {
        reference_raw: reference.raw.clone(),
        exists: Some(ExistenceCheck {
            source: "crossref",
            found: true,
            doi: Some("10.1/zzz".into()),
            title: Some(UntrustedText::new("Sleep and memory", p.clone())),
            matched_authors: None,
            matched_year: None,
            is_retracted_hint: None,
            provenance: p.clone(),
        }),
        retraction: None,
        open_access: None,
        enrichment: None,
        provenance: vec![p],
        warnings: vec![],
    };
    vec![(reference, rv)]
}

fn opinion(agent: AgentKind, answer: &str, confidence: f64) -> Opinion {
    Opinion {
        agent,
        answer: answer.into(),
        explanation: format!("{agent:?} says {answer}"),
        confidence,
        hard_constraint: false,
        gate_passed: true,
    }
}

fn guideline_hit(content: &str) -> RagHit {
    RagHit {
        chunk_id: 1,
        document_id: 1,
        seq: 0,
        content: content.into(),
        distance: 0.1,
        source_type: "journal_guideline".into(),
        title: "Journal of Rest — Author Guidelines".into(),
        source_url: "https://rest.example/authors".into(),
        fetched_at: 1_000,
        checksum: "d".repeat(64),
    }
}

// --- golden report over the Prompt-19 sample manuscript ------------------------

#[test]
fn golden_report_for_sample_manuscript() {
    // Real local agents, exactly as in the Prompt-19 e2e.
    let extraction = crate::extract::extract_from_text(MANUSCRIPT);
    let validation = crate::validate::validate(&extraction);
    assert!(validation.passed, "fixture must be stats-clean: {:?}", validation.flags);
    let model = crate::ai_detect::HeuristicModel::gpt2_like();
    let ai = crate::ai_detect::detect_text(&model, crate::ai_detect::DeepKind::Absent, MANUSCRIPT);
    let db = crate::Database::in_memory().unwrap();
    let embedder = crate::embed::HashEmbedder;
    let mut session = crate::plagiarism::PlagiarismSession::new().unwrap();
    session.ingest_manuscript(&embedder, MANUSCRIPT).unwrap();
    let plag = session.report(&db, None).unwrap();
    let hits = crate::rag::search(&db, &embedder, "reporting standards", 3, None).unwrap();

    let proxy = MockProxyClient::returning(json!({"verdicts": [{
        "citation_id": "c1", "verdict": "SUPPORTED", "confidence": 0.85,
        "rationale": "DOI and title match", "evidence_refs": ["ev-c1-0"]
    }]}));
    let items = verification_items(extraction.references[0].clone());
    let verification = verify_citations(&proxy, &items).unwrap();

    let mut agents: Vec<Box<dyn SwarmAgent>> = vec![
        Box::new(PrecomputedAgent::new(adapters::from_extraction(&extraction))),
        Box::new(PrecomputedAgent::new(adapters::from_validation(&validation))),
        Box::new(PrecomputedAgent::new(adapters::from_ai_detection(&ai))),
        Box::new(PrecomputedAgent::new(adapters::from_plagiarism(&plag))),
        Box::new(PrecomputedAgent::new(adapters::from_rag_hits(&hits))),
        Box::new(PrecomputedAgent::new(adapters::from_verification_report(&verification))),
    ];
    let outcome = run_debate(&mut agents, &DebateConfig::default()).unwrap();

    // Checklist from a sample guideline (word limit + COI + structured abstract).
    let guideline = guideline_hit(
        "Authors must supply structured abstracts and declare all conflicts of interest. \
         Manuscripts have a limit of 3000 words.",
    );
    let checklist = checklist_from_guidelines(&extraction, MANUSCRIPT, &[guideline]);

    let report = compile_report(&outcome, &validation, Some(&verification), Some(&plag), None, TEST_YEAR, checklist, &[]);

    // Golden expectations (stable, structural — not a brittle full snapshot).
    assert_eq!(report.verdict, ANSWER_PASS);
    assert!(report.combined_confidence > 0.0 && report.combined_confidence <= 1.0);
    assert!(!report.debate.overridden_by_constraint);
    assert!(report.debate.converged);
    assert!(report.disclaimer.contains("mathematically certain"));
    assert!(report.disclaimer.contains("never"), "must disclaim definitive proof");

    // A clean manuscript: no CRITICAL findings; the supported citation is Info.
    assert!(report.findings.iter().all(|f| f.severity != FindingSeverity::Critical));
    let citation = report
        .findings
        .iter()
        .find(|f| f.agent == AgentKind::Verification)
        .expect("citation finding present");
    assert_eq!(citation.severity, FindingSeverity::Info);
    assert_eq!(citation.tier, CertaintyTier::AiAssessedModerate);

    // Checklist: all four structural sections pass; guideline items present + pass.
    let by_req = |needle: &str| {
        report
            .checklist
            .iter()
            .find(|c| c.requirement.contains(needle))
            .unwrap_or_else(|| panic!("missing checklist item: {needle}"))
    };
    for section in ["Abstract", "Methods", "Results", "References"] {
        assert!(by_req(section).passed, "{section} should pass");
    }
    // Word-limit detector disabled — see checklist_from_guidelines.
    assert!(!report.checklist.iter().any(|c| c.requirement.contains("word limit")));
    assert!(by_req("conflict-of-interest").passed);
    assert!(by_req("structured abstract").passed);
    // guideline-derived items carry the guideline's provenance URL
    // Provenance is still asserted — on a detector that is still enabled.
    assert_eq!(
        by_req("conflict-of-interest").guideline_source.as_deref(),
        Some("https://rest.example/authors")
    );
}

// --- hard constraints always rank first -----------------------------------------

#[test]
fn hard_constraint_findings_rank_first_regardless_of_soft_confidence() {
    // A manuscript that FAILS a deterministic rule (p-value, no effect size/CI).
    let text = "A Trial\n\nAbstract\nWe found an effect (p < 0.001).\n\nMethods\nA t-test \
                was used.\n\nResults\nStrong effect.\n";
    let extraction = crate::extract::extract_from_text(text);
    let validation = crate::validate::validate(&extraction);
    assert!(!validation.passed, "fixture must trip a deterministic rule");

    // Soft agents scream pass with MAXIMAL confidence.
    let mut agents: Vec<Box<dyn SwarmAgent>> = vec![
        Box::new(PrecomputedAgent::new(opinion(AgentKind::Extraction, ANSWER_PASS, 1.0))),
        Box::new(PrecomputedAgent::new(opinion(AgentKind::AiDetection, ANSWER_PASS, 1.0))),
        Box::new(PrecomputedAgent::new(opinion(AgentKind::Plagiarism, ANSWER_PASS, 1.0))),
        Box::new(PrecomputedAgent::new(adapters::from_validation(&validation))),
    ];
    let outcome = run_debate(&mut agents, &DebateConfig::default()).unwrap();
    // Synthetic plagiarism OPINION only (no PlagiarismReport) → no per-match
    // findings; the opinion still votes in consensus but yields no finding.
    let report = compile_report(&outcome, &validation, None, None, None, TEST_YEAR, vec![], &[]);

    // THE PROPERTY: hard-constraint findings lead, ahead of 1.0-confidence soft
    // findings. The EPISTEMIC marker is the tier, not the severity.
    //
    // This previously asserted `severity == Critical`, which was asserting the
    // ACCIDENT rather than the property — every validation flag was hardcoded
    // Critical regardless of its rule's own severity. Severity now comes from
    // the rule, and the property survives unchanged because the sort is
    // severity -> tier -> confidence, so MathematicallyCertain still leads its
    // band. Ordering never depended on the conflation.
    assert!(!report.findings.is_empty());
    let n_hard = validation.flags.len();
    for f in &report.findings[..n_hard] {
        assert_eq!(f.tier, CertaintyTier::MathematicallyCertain, "hard constraint first: {:?}", f.title);
        assert_eq!(f.agent, AgentKind::ValidationMaths);
        // Severity mirrors the RULE, and the finding's own provenance says so.
        let sev = format!("{:?}", f.severity).to_uppercase();
        assert!(
            f.provenance.iter().any(|p| p.contains(&sev)),
            "severity must match the rule severity printed in provenance: {:?} / {:?}",
            f.severity, f.provenance
        );
    }
    assert!(report.findings[n_hard..]
        .iter()
        .all(|f| f.tier != CertaintyTier::MathematicallyCertain),
        "no hard-constraint finding may appear after the leading block");
    // and the debate itself was overridden by the constraint
    assert!(report.debate.overridden_by_constraint);
    assert_eq!(report.verdict, crate::swarm::ANSWER_CONCERN);
}

// --- checklist against a sample journal guideline --------------------------------

#[test]
fn checklist_flags_violations_against_journal_guideline() {
    // Over the word limit, no COI statement, author-year (not numbered) refs.
    let text = "A Paper\n\nAbstract\nShort.\n\nMethods\nWe did things.\n\nResults\nGood.\n\n\
                References\nDoe, J. (2020). Title. Journal, 1(1), 1-2.\n";
    let extraction = crate::extract::extract_from_text(text);
    let guideline = guideline_hit(
        "References follow the numbered Vancouver citation style. Authors must declare all \
         conflicts of interest. Manuscripts have a limit of 10 words.",
    );
    let items = checklist_from_guidelines(&extraction, text, &[guideline]);

    let by_req = |needle: &str| {
        items.iter().find(|c| c.requirement.contains(needle)).unwrap_or_else(|| {
            panic!("missing checklist item: {needle} — have {:?}",
                items.iter().map(|c| &c.requirement).collect::<Vec<_>>())
        })
    };
    // The word-limit detector is DISABLED — it scraped an abstract limit and
    // applied it to the whole manuscript ("5144 words, limit 300" on PLOS ONE).
    // Pinned so re-enabling it is a deliberate act with its own evidence.
    assert!(
        !items.iter().any(|c| c.requirement.contains("word limit")),
        "word-limit detector must stay disabled: {:?}",
        items.iter().map(|c| &c.requirement).collect::<Vec<_>>()
    );
    assert!(!by_req("conflict-of-interest").passed, "no COI statement in fixture");
    assert!(!by_req("Vancouver").passed, "author-year refs are not numbered");
    assert!(by_req("required section: Methods").passed);
    // every guideline-derived item carries provenance
    for item in items.iter().filter(|i| i.guideline_source.is_some()) {
        assert_eq!(item.guideline_source.as_deref(), Some("https://rest.example/authors"));
    }
}

#[test]
fn checklist_without_guidelines_shows_structural_checks_only_no_fake_failure() {
    // Honest degradation (H4): with NO guideline docs in the store, the checklist
    // must NOT fabricate a passed:false "journal guidelines available: FAILED"
    // item — that misread "the user didn't provide guidelines" as a manuscript
    // failure. It returns ONLY the always-on structural checks; absence of
    // guideline-derived items (guideline_source == None on every item) is the
    // neutral signal the UI reads to show an honest "add your guidelines" note.
    let extraction = crate::extract::extract_from_text("T\n\nAbstract\nx.\n");
    let items = checklist_from_guidelines(&extraction, "T Abstract x.", &[]);

    // no fabricated guidelines-availability failure
    assert!(
        !items.iter().any(|i| i.requirement.contains("guidelines available")),
        "empty store must NOT emit a fake guidelines-availability item, got {:?}",
        items.iter().map(|c| &c.requirement).collect::<Vec<_>>()
    );
    // the always-on structural checks still run, and every item is structural
    // (no guideline provenance) — the honest "not provided" state
    assert!(!items.is_empty(), "structural checks still run with no guidelines");
    assert!(
        items.iter().all(|i| i.guideline_source.is_none()),
        "with no guidelines every item must be a structural check"
    );
    assert!(
        items.iter().any(|i| i.requirement.contains("required section: Abstract")),
        "the structural section checks are present"
    );
}

// --- provenance + certainty tiers on every finding, incl. reconsidered ----------

#[test]
fn every_finding_carries_provenance_and_correct_tier_including_reconsidered() {
    // Debate in which the Verification agent RECONSIDERS (as in Prompt 20).
    let proxy = MockProxyClient::returning_sequence(vec![
        json!({"verdicts": [{
            "citation_id": "c1", "verdict": "SUPPORTED", "confidence": 0.9,
            "rationale": "matches", "evidence_refs": ["ev-c1-0"]
        }]}),
        json!({"verdicts": [{
            "citation_id": "c1", "verdict": "UNKNOWN", "confidence": 0.3,
            "rationale": "peer similarity finding undermines support",
            "evidence_refs": ["ev-c1-0"]
        }]}),
    ]);
    let reference = Reference {
        raw: "Doe, J. (2022). A paper. https://doi.org/10.1/abc".into(),
        authors: "Doe, J.".into(),
        year: Some(2022),
        title: Some("A paper".into()),
        doi: Some("10.1/abc".into()),
    };
    let items = verification_items(reference);
    let initial = verify_citations(&proxy, &items).unwrap();
    let reviser = RevisingVerificationAgent::new(&proxy, items, initial);

    let mut agents: Vec<Box<dyn SwarmAgent + '_>> = vec![
        Box::new(PrecomputedAgent::new(opinion(AgentKind::Extraction, ANSWER_PASS, 0.9))),
        Box::new(PrecomputedAgent::new(opinion(
            AgentKind::Plagiarism,
            crate::swarm::ANSWER_CONCERN,
            0.92,
        ))),
        Box::new(reviser),
    ];
    let outcome = run_debate(&mut agents, &DebateConfig::default()).unwrap();
    assert!(outcome.revised_agents.contains(&AgentKind::Verification), "reconsideration ran");

    // The compiler needs the FINAL (reconsidered) verification report; rebuild
    // it the way a caller would (2nd proxy response is what the agent adopted).
    // We reconstruct via reconsider_citations directly for the same items.
    let items2 = verification_items(Reference {
        raw: "Doe, J. (2022). A paper. https://doi.org/10.1/abc".into(),
        authors: "Doe, J.".into(),
        year: Some(2022),
        title: Some("A paper".into()),
        doi: Some("10.1/abc".into()),
    });
    let proxy2 = MockProxyClient::returning(json!({"verdicts": [{
        "citation_id": "c1", "verdict": "UNKNOWN", "confidence": 0.3,
        "rationale": "peer similarity finding undermines support",
        "evidence_refs": ["ev-c1-0"]
    }]}));
    let final_verification = verify_citations(&proxy2, &items2).unwrap();

    let validation = crate::validate::validate(&crate::extract::extract_from_text(
        "T\n\nAbstract\nNo stats here.\n",
    ));
    let report = compile_report(&outcome, &validation, Some(&final_verification), None, None, TEST_YEAR, vec![], &[]);

    // EVERY finding: non-empty provenance + a label matching its tier.
    assert!(!report.findings.is_empty());
    for f in &report.findings {
        assert!(!f.provenance.is_empty(), "finding without provenance: {:?}", f.title);
        assert_eq!(f.certainty_label, f.tier.label(), "label/tier mismatch: {:?}", f.title);
    }

    // The reconsidered Verification finding carries the third tier + evidence.
    let vf = report
        .findings
        .iter()
        .find(|f| f.agent == AgentKind::Verification)
        .expect("verification finding present");
    assert_eq!(vf.tier, CertaintyTier::ReconsideredAfterPeerReview);
    assert_eq!(vf.certainty_label, "reconsidered after peer review");
    assert!(vf.provenance.iter().any(|p| p.contains("evidence:ev-c1-0")));
    assert_eq!(vf.severity, FindingSeverity::Minor, "UNKNOWN maps to Minor");

    // Tier labels are the three distinct strings, end to end.
    assert!(report.debate.revised_agents.contains(&AgentKind::Verification));
    assert_eq!(CertaintyTier::MathematicallyCertain.label(), "mathematically certain");
    assert_eq!(CertaintyTier::AiAssessedModerate.label(), "AI-assessed, moderate confidence");
}

// --- Step 0b: plagiarism fans into ONE finding PER MATCH -------------------------

#[test]
fn plagiarism_matches_fan_into_per_match_findings() {
    use crate::plagiarism::{MatchSource, MatchSpan, PlagiarismReport};
    let pr = PlagiarismReport {
        corpus_chunks_available: 10,
        chunk_count: 5,
        threshold: 0.80,
        corpus_matches: vec![MatchSpan {
            manuscript_chunk_seq: 2,
            manuscript_excerpt: "the effect was significant".into(),
            similarity: 0.91,
            source: MatchSource::Corpus {
                document_id: 1,
                chunk_id: 3,
                title: "Smith 2019".into(),
                source_url: "https://ex/smith".into(),
                source_type: "corpus".into(),
                excerpt: "prior text".into(),
            },
        }],
        self_matches: vec![MatchSpan {
            manuscript_chunk_seq: 7,
            manuscript_excerpt: "as noted above".into(),
            similarity: 0.62,
            source: MatchSource::SelfManuscript { other_chunk_seq: 1, excerpt: "earlier".into() },
        }],
        note: "test".into(),
    };
    // A plagiarism OPINION is also in the debate — it must still VOTE in consensus
    // but must NOT produce an aggregate finding (only the per-match findings show).
    let mut agents: Vec<Box<dyn SwarmAgent>> = vec![
        Box::new(PrecomputedAgent::new(opinion(
            AgentKind::Plagiarism,
            crate::swarm::ANSWER_CONCERN,
            0.91,
        ))),
        Box::new(PrecomputedAgent::new(opinion(AgentKind::Extraction, ANSWER_PASS, 0.9))),
    ];
    let outcome = run_debate(&mut agents, &DebateConfig::default()).unwrap();
    let validation =
        crate::validate::validate(&crate::extract::extract_from_text("T\n\nAbstract\nNo stats.\n"));
    let report = compile_report(&outcome, &validation, None, Some(&pr), None, TEST_YEAR, vec![], &[]);

    let plag: Vec<_> = report.findings.iter().filter(|f| f.agent == AgentKind::Plagiarism).collect();
    // One finding PER MATCH (2), aggregate opinion NOT double-counted.
    assert_eq!(plag.len(), 2, "one finding per match; opinion not double-counted");

    let corpus =
        plag.iter().find(|f| f.title.contains("high word overlap")).expect("corpus finding");
    assert_eq!(corpus.confidence, 0.91, "RAW cosine similarity, not swarm-rescaled");
    assert_eq!(corpus.severity, FindingSeverity::Major, ">= threshold ⇒ Major");
    assert_eq!(corpus.tier, CertaintyTier::AiAssessedModerate);
    assert!(corpus.provenance.iter().any(|p| p == "similarity:0.910"));
    assert!(corpus.provenance.iter().any(|p| p == "source:corpus"));

    let selfm =
        plag.iter().find(|f| f.title.contains("same manuscript")).expect("self finding");
    assert_eq!(selfm.confidence, 0.62);
    assert_eq!(selfm.severity, FindingSeverity::Minor, "< threshold ⇒ Minor");
    assert!(selfm.provenance.iter().any(|p| p == "source:self_manuscript"));
}

#[test]
fn empty_plagiarism_yields_zero_findings() {
    use crate::plagiarism::PlagiarismReport;
    let pr = PlagiarismReport {
        corpus_chunks_available: 10,
        chunk_count: 3,
        threshold: 0.80,
        corpus_matches: vec![],
        self_matches: vec![],
        note: "clean".into(),
    };
    let mut agents: Vec<Box<dyn SwarmAgent>> = vec![
        Box::new(PrecomputedAgent::new(opinion(AgentKind::Plagiarism, ANSWER_PASS, 0.7))),
        Box::new(PrecomputedAgent::new(opinion(AgentKind::Extraction, ANSWER_PASS, 0.9))),
    ];
    let outcome = run_debate(&mut agents, &DebateConfig::default()).unwrap();
    let validation =
        crate::validate::validate(&crate::extract::extract_from_text("T\n\nAbstract\nNo stats.\n"));
    let report = compile_report(&outcome, &validation, None, Some(&pr), None, TEST_YEAR, vec![], &[]);
    // No matches ⇒ no plagiarism findings (a non-event is not a finding).
    assert!(
        !report.findings.iter().any(|f| f.agent == AgentKind::Plagiarism),
        "empty plagiarism must yield zero findings"
    );
}

// --- Box 0 wiring: compile_report produces EvidenceRecords 1:1 with findings ----

#[test]
fn compile_report_produces_evidence_1to1_with_correct_kinds() {
    use crate::evidence::{ConfidenceKind, RoutingHint};
    // A RAG soft opinion with a KNOWN raw confidence (0.75) + an extraction one.
    let mut agents: Vec<Box<dyn SwarmAgent>> = vec![
        Box::new(PrecomputedAgent::new(opinion(AgentKind::Rag, ANSWER_PASS, 0.75))),
        Box::new(PrecomputedAgent::new(opinion(AgentKind::Extraction, ANSWER_PASS, 0.9))),
    ];
    let outcome = run_debate(&mut agents, &DebateConfig::default()).unwrap();
    let validation =
        crate::validate::validate(&crate::extract::extract_from_text("T\n\nAbstract\nNo stats.\n"));
    let report = compile_report(&outcome, &validation, None, None, None, TEST_YEAR, vec![], &[]);

    // 1:1, same order; ids are f1..fN.
    assert_eq!(report.evidence.len(), report.findings.len());
    assert!(!report.evidence.is_empty());
    for (i, ev) in report.evidence.iter().enumerate() {
        assert_eq!(ev.id, format!("f{}", i + 1));
        assert_eq!(ev.agent, report.findings[i].agent, "evidence[i] tracks findings[i]");
    }

    // RAG record: WiredReal + ContextOnly, carrying the RAW op.confidence (0.75)
    // — NOT the swarm-rescaled weight the Finding shows.
    let rag_i =
        report.findings.iter().position(|f| f.agent == AgentKind::Rag).expect("rag finding");
    let rag_ev = &report.evidence[rag_i];
    assert_eq!(rag_ev.confidence_kind, ConfidenceKind::WiredReal);
    assert_eq!(rag_ev.routing_hint, RoutingHint::ContextOnly);
    assert_eq!(rag_ev.confidence, 0.75, "evidence carries RAW op.confidence");
    assert_ne!(
        report.findings[rag_i].confidence, 0.75,
        "the Finding still carries the rescaled weight"
    );

    // Extraction record: NoSignal + HeldOut, never silently routable.
    let ext_i = report
        .findings
        .iter()
        .position(|f| f.agent == AgentKind::Extraction)
        .expect("extraction finding");
    let ext_ev = &report.evidence[ext_i];
    assert_eq!(ext_ev.confidence_kind, ConfidenceKind::NoSignal);
    assert_eq!(ext_ev.routing_hint, RoutingHint::HeldOut);
    assert!(ext_ev.limitations.is_some());
}

// ============================================================================
// Extraction-derived findings (stylometry / tables / reference recency)
// ============================================================================

/// One minimal debate outcome + clean validation, so these tests assert ONLY on
/// the extraction-derived findings the `extraction` argument adds.
fn minimal_outcome() -> crate::swarm::DebateOutcome {
    let mut agents: Vec<Box<dyn SwarmAgent>> =
        vec![Box::new(PrecomputedAgent::new(opinion(AgentKind::Rag, ANSWER_PASS, 0.75)))];
    run_debate(&mut agents, &DebateConfig::default()).unwrap()
}

/// Findings carrying a given `signal:` provenance tag.
fn by_signal<'a>(report: &'a PublishReadyReport, signal: &str) -> Vec<&'a Finding> {
    let tag = format!("signal:{signal}");
    report.findings.iter().filter(|f| f.provenance.iter().any(|p| *p == tag)).collect()
}

/// `extraction: None` must add NOTHING — the parameter is purely additive, so
/// every existing caller keeps its exact previous output.
#[test]
fn extraction_none_adds_no_findings() {
    let outcome = minimal_outcome();
    let ex = crate::extract::extract_from_text(
        "T\n\nAbstract\nNo stats.\n\nResults\nTable 1 Outcomes by arm\n",
    );
    let validation = crate::validate::validate(&ex);
    let without = compile_report(&outcome, &validation, None, None, None, TEST_YEAR, vec![], &[]);
    let with = compile_report(&outcome, &validation, None, None, Some(&ex), TEST_YEAR, vec![], &[]);
    assert!(
        with.findings.len() > without.findings.len(),
        "passing the extraction must ADD findings, else the wiring is dead"
    );
    for signal in ["tables", "citation_recency", "citation_density"] {
        assert!(by_signal(&without, signal).is_empty(), "None must add no {signal} finding");
    }
}

/// Tables: a structural COUNT under Extraction — NoSignal/HeldOut by
/// construction, 0.0 confidence (never a number implying precision), Minor only
/// when a caption is missing, and NOTHING at all when there are no tables.
#[test]
fn table_findings_count_captions_and_stay_structural() {
    use crate::evidence::{ConfidenceKind, RoutingHint};
    let outcome = minimal_outcome();

    // Two tables, one captioned -> Minor.
    let ex = crate::extract::extract_from_text(
        "T\n\nAbstract\nA.\n\nResults\nTable 1 Outcomes by arm\n\nTable 2\n",
    );
    let validation = crate::validate::validate(&ex);
    let report = compile_report(&outcome, &validation, None, None, Some(&ex), TEST_YEAR, vec![], &[]);
    let tables = by_signal(&report, "tables");
    assert_eq!(tables.len(), 1, "exactly one aggregate table finding");
    let f = tables[0];
    assert_eq!(f.agent, AgentKind::Extraction);
    assert_eq!(f.severity, FindingSeverity::Minor, "a missing caption is Minor");
    assert_eq!(f.confidence, 0.0, "a count is not a probability");
    assert!(f.title.contains("2 table(s)"), "title reports the count: {}", f.title);
    assert!(
        f.provenance.iter().any(|p| p.starts_with("evidence:tables=2;captioned=")),
        "structured count in provenance: {:?}",
        f.provenance
    );
    // The EvidenceRecord's kind/hint are DERIVED from the agent, not hand-set.
    let i = report.findings.iter().position(|x| std::ptr::eq(x, f)).unwrap();
    assert_eq!(report.evidence[i].confidence_kind, ConfidenceKind::NoSignal);
    assert_eq!(report.evidence[i].routing_hint, RoutingHint::HeldOut);

    // No tables -> no finding (a non-event is not a finding).
    let ex2 = crate::extract::extract_from_text("T\n\nAbstract\nA.\n\nResults\nNo tables here.\n");
    let v2 = crate::validate::validate(&ex2);
    let r2 = compile_report(&outcome, &v2, None, None, Some(&ex2), TEST_YEAR, vec![], &[]);
    assert!(by_signal(&r2, "tables").is_empty(), "zero tables must produce zero findings");
}

/// Reference recency: deterministic arithmetic over locally-parsed
/// `Reference.year`. Attributed to Extraction (no connector ran), Minor only
/// above the staleness share, and undated references are reported separately
/// rather than silently counted either way.
#[test]
fn reference_recency_counts_old_and_undated_separately() {
    use crate::evidence::{ConfidenceKind, RoutingHint};
    let outcome = minimal_outcome();
    // 3 of 4 dated references are pre-2015 (TEST_YEAR 2025 - 10y), 1 undated.
    let text = "T\n\nAbstract\nA.\n\nReferences\n\
                1. Old A. Ancient work. 1991.\n\
                2. Old B. Older work. 1999.\n\
                3. Old C. Still old. 2004.\n\
                4. New D. Recent work. 2024.\n\
                5. Nodate E. Undated work.\n";
    let ex = crate::extract::extract_from_text(text);
    let validation = crate::validate::validate(&ex);
    let report = compile_report(&outcome, &validation, None, None, Some(&ex), TEST_YEAR, vec![], &[]);

    let hits = by_signal(&report, "citation_recency");
    assert_eq!(hits.len(), 1, "one aggregate recency finding");
    let f = hits[0];
    assert_eq!(f.agent, AgentKind::Extraction, "local years, no connector — not Verification");
    assert_eq!(f.confidence, 0.0);
    assert_eq!(f.severity, FindingSeverity::Minor, "3 of 4 dated refs are stale (>50%)");
    assert!(f.detail.contains("pre-date 2015"), "cutoff is stated: {}", f.detail);
    assert!(f.detail.contains("no parseable year"), "undated reported separately: {}", f.detail);
    assert!(
        f.provenance.iter().any(|p| p.starts_with("evidence:references=5;dated=4;undated=1")),
        "structured counts: {:?}",
        f.provenance
    );
    let i = report.findings.iter().position(|x| std::ptr::eq(x, f)).unwrap();
    assert_eq!(report.evidence[i].confidence_kind, ConfidenceKind::NoSignal);
    assert_eq!(report.evidence[i].routing_hint, RoutingHint::HeldOut);

    // A current bibliography stays Info, not Minor.
    let fresh = "T\n\nAbstract\nA.\n\nReferences\n\
                 1. New A. Recent work. 2022.\n\
                 2. New B. Recent work. 2023.\n";
    let ex2 = crate::extract::extract_from_text(fresh);
    let v2 = crate::validate::validate(&ex2);
    let r2 = compile_report(&outcome, &v2, None, None, Some(&ex2), TEST_YEAR, vec![], &[]);
    assert_eq!(by_signal(&r2, "citation_recency")[0].severity, FindingSeverity::Info);

    // No reference list -> no finding (the structural checklist covers that).
    let ex3 = crate::extract::extract_from_text("T\n\nAbstract\nA.\n");
    let v3 = crate::validate::validate(&ex3);
    let r3 = compile_report(&outcome, &v3, None, None, Some(&ex3), TEST_YEAR, vec![], &[]);
    assert!(by_signal(&r3, "citation_recency").is_empty());
}

/// Stylometry: soft signals under AI-detection — DeliberatelyCoarse +
/// PolicyEligible by construction, always Minor, and using the SAME coarse
/// confidence constants the AI-detection swarm opinion already uses.
#[test]
fn stylometry_findings_are_coarse_soft_signals() {
    use crate::evidence::{ConfidenceKind, RoutingHint};
    let outcome = minimal_outcome();
    // Uniform, highly repetitive prose with a reference list but no in-text
    // citations — trips low sentence-length variation and the citation-density
    // PARSE-FAILURE state (references present, nothing attributable).
    let body = "The system processes the data. The system processes the data well. \
                The system processes the data again. The system handles the data. "
        .repeat(12);
    let text = format!(
        "T\n\nAbstract\nA study.\n\nResults\n{body}\n\nReferences\n1. A. Work. 2020.\n"
    );
    let ex = crate::extract::extract_from_text(&text);
    let validation = crate::validate::validate(&ex);
    let report = compile_report(&outcome, &validation, None, None, Some(&ex), TEST_YEAR, vec![], &[]);

    let stylo: Vec<&Finding> =
        report.findings.iter().filter(|f| f.agent == AgentKind::AiDetection).collect();
    assert!(!stylo.is_empty(), "expected stylometric findings; got {:?}", report.findings);
    for f in &stylo {
        assert!(
            matches!(f.severity, FindingSeverity::Minor | FindingSeverity::Info),
            "soft signals are never Major/Critical: {:?}",
            f.severity
        );
        assert!(
            f.confidence == 0.6 || f.confidence == 0.5,
            "reuses the swarm's coarse constants, got {}",
            f.confidence
        );
        let i = report.findings.iter().position(|x| std::ptr::eq(x, *f)).unwrap();
        assert_eq!(report.evidence[i].confidence_kind, ConfidenceKind::DeliberatelyCoarse);
        assert_eq!(report.evidence[i].routing_hint, RoutingHint::PolicyEligible);
        assert!(
            report.evidence[i].limitations.as_deref().unwrap_or("").contains("never threshold"),
            "the coarse limitation must ride along"
        );
    }

    // References present but no attributable in-text citations is a PARSE
    // failure — reported as unavailable/Info, never as a false "zero citations".
    let density = by_signal(&report, "citation_density");
    assert_eq!(density.len(), 1);
    assert_eq!(density[0].severity, FindingSeverity::Info);
    assert!(density[0].title.contains("could not be measured"), "{}", density[0].title);
    assert!(density[0].provenance.iter().any(|p| p.contains("status=unavailable")));

    // The AI-AUTHORSHIP tells stay in AI Check — they must never appear here.
    for excluded in ["em_dash", "template_density", "function_word_ratio"] {
        assert!(
            by_signal(&report, excluded).is_empty(),
            "{excluded} is an AI-authorship tell and belongs to AI Check, not PublishReady"
        );
    }

    // Every finding above lands on the MODERATE constant. Cover the other branch
    // explicitly: prose with no in-text citations AND no reference list at all is
    // the genuinely-uncited case (distinct from the parse failure above), which
    // maps to the NOTABLE constant. Without this, the High path is untested and a
    // change to STYLO_CONF_HIGH would go unnoticed.
    let uncited = format!("T\n\nAbstract\nA study.\n\nResults\n{body}\n");
    let ex2 = crate::extract::extract_from_text(&uncited);
    let v2 = crate::validate::validate(&ex2);
    let r2 = compile_report(&outcome, &v2, None, None, Some(&ex2), TEST_YEAR, vec![], &[]);
    let d2 = by_signal(&r2, "citation_density");
    assert_eq!(d2.len(), 1, "an uncited document still reports density");
    assert_eq!(d2[0].severity, FindingSeverity::Minor, "genuinely uncited is Minor, not Info");
    assert_eq!(d2[0].confidence, 0.6, "the NOTABLE branch uses the swarm's High constant");
    assert!(
        !d2[0].provenance.iter().any(|p| p.contains("status=unavailable")),
        "no reference list means genuinely uncited, NOT an unmeasurable parse failure"
    );
}

/// Every new finding must carry only structured provenance and template text —
/// nothing that could put manuscript prose on the wire. Mirrors the discipline
/// `reviewer_agent::build_review_payload` enforces at the boundary.
#[test]
fn extraction_derived_findings_carry_only_structured_provenance() {
    let outcome = minimal_outcome();
    let text = "Sleep Study\n\nAbstract\nA study.\n\nResults\nTable 1 Outcomes\n\n\
                References\n1. A. Work. 1998.\n";
    let ex = crate::extract::extract_from_text(text);
    let validation = crate::validate::validate(&ex);
    let report = compile_report(&outcome, &validation, None, None, Some(&ex), TEST_YEAR, vec![], &[]);

    let derived: Vec<&Finding> = report
        .findings
        .iter()
        .filter(|f| f.provenance.iter().any(|p| p.starts_with("signal:")))
        .collect();
    assert!(!derived.is_empty());
    for f in derived {
        for p in &f.provenance {
            assert!(
                crate::evidence::is_structured_provenance(p),
                "every provenance tag must survive the payload filter: {p:?}"
            );
        }
    }
}

/// The reviewer payload's top-N cap (`reviewer_agent::MAX_FINDINGS`, private
/// there). Mirrored so this test can reason about the cutoff; the assertions
/// below check the payload ACTUALLY caps at this number, so if the production
/// const changes this test fails loudly instead of silently passing.
const REVIEWER_MAX_FINDINGS: usize = 12;

/// `n` plagiarism matches above threshold — each compiles to one MAJOR finding
/// (`compile_report` fans matches into per-match findings). A cheap way to fill
/// the severity band above the new extraction-derived families.
fn plagiarism_with_n_major_matches(n: usize) -> PlagiarismReport {
    PlagiarismReport {
        corpus_chunks_available: 10,
        chunk_count: n,
        threshold: 0.80,
        corpus_matches: (0..n)
            .map(|i| MatchSpan {
                manuscript_chunk_seq: i as i64,
                manuscript_excerpt: format!("excerpt {i}"),
                similarity: 0.90,
                source: MatchSource::Corpus {
                    document_id: 1,
                    chunk_id: i as i64,
                    title: format!("Prior {i}"),
                    source_url: format!("https://ex/{i}"),
                    source_type: "corpus".into(),
                    excerpt: "prior text".into(),
                },
            })
            .collect(),
        self_matches: Vec::new(),
        note: "test".into(),
    }
}

/// An extraction whose tables trip the caption finding (2 tables, 1 captioned),
/// so there is a known extraction-derived finding to look for.
fn extraction_with_uncaptioned_table() -> ExtractionResult {
    crate::extract::extract_from_text(
        "T\n\nAbstract\nA study.\n\nResults\nTable 1 Outcomes by arm\n\nTable 2\n",
    )
}

/// A manuscript citing Smith 2020 but not Jones 2019 — one genuinely uncited
/// reference, reached through the REAL extraction path (not a hand-built struct),
/// so the parser is exercised too.
fn extraction_with_one_uncited_reference() -> ExtractionResult {
    crate::extract::extract_from_text(
        "T\n\nAbstract\nWe build on prior work (Smith, 2020) in this study.\n\n         References\n\nSmith J. 2020. A cited paper. Journal of Things 1: 1-10.\n\n         Jones A. 2019. An uncited paper. Journal of Other Things 2: 11-20.\n",
    )
}

/// The builder is DELIBERATELY not wired into `compile_report` (see its doc
/// comment), so these tests call it directly. That is the point: the finding must
/// be provably correct BEFORE it can emit, not after.
fn uncited_finding(ex: &ExtractionResult) -> Option<Finding> {
    super::uncited_reference_findings(ex).into_iter().next().map(|rf| rf.finding)
}

#[test]
fn an_uncited_reference_is_reported_with_both_numbers() {
    let ex = extraction_with_one_uncited_reference();
    assert_eq!(ex.references.len(), 2, "fixture precondition: {:?}", ex.references);
    let f = uncited_finding(&ex).expect("uncited-reference finding missing");
    assert_eq!(f.severity, FindingSeverity::Minor);
    assert_eq!(f.tier, CertaintyTier::AiAssessedModerate);
    assert_eq!(f.agent, AgentKind::Extraction);
    // BOTH numbers must be present — M - N stated, not hidden.
    assert!(f.detail.contains("2 of 2 reference(s) were checked"), "detail: {}", f.detail);
    assert!(f.detail.contains("Jones 2019"), "should name the uncited entry: {}", f.detail);
    assert!(
        f.detail.contains("could not be checked and are NOT counted as uncited"),
        "detail must disclose the not-evaluated count: {}",
        f.detail
    );
}

#[test]
fn an_ambiguous_key_yields_no_uncited_finding_end_to_end() {
    // Two entries share (etebari, 2005). Neither may be called uncited, and since
    // there is nothing else to report the finding must not appear at all.
    let ex = crate::extract::extract_from_text(
        "T\n\nAbstract\nPrior work is relevant (Smith, 2020).\n\n         References\n\nEtebari K, Mirhoseini S. 2005. First paper. J One 1: 1-10.\n\n         Etebari K, Bizhannia A. 2005. Second paper. J Two 2: 11-20.\n",
    );
    assert_eq!(ex.references.len(), 2, "fixture precondition: {:?}", ex.references);
    assert!(
        uncited_finding(&ex).is_none(),
        "ambiguous keys must produce NO finding, got: {:?}",
        uncited_finding(&ex).map(|f| f.detail)
    );
}

#[test]
fn a_fully_cited_bibliography_produces_no_finding() {
    let ex = crate::extract::extract_from_text(
        "T\n\nAbstract\nWe build on prior work (Smith, 2020).\n\n         References\n\nSmith J. 2020. A cited paper. Journal of Things 1: 1-10.\n",
    );
    assert!(uncited_finding(&ex).is_none(), "a non-event is not a finding");
}

/// PIN: the builder must stay OUT of the report until its wiring precondition is
/// met. If someone wires it, this fails and they read the doc comment explaining
/// why precision was 0.00 on a real manuscript.
#[test]
fn the_uncited_reference_finding_is_not_emitted_by_compile_report() {
    let ex = extraction_with_one_uncited_reference();
    assert!(
        uncited_finding(&ex).is_some(),
        "precondition: the builder itself does produce a finding for this fixture"
    );
    let validation = crate::validate::validate(&ex);
    let report = compile_report(
        &minimal_outcome(), &validation, None, None, Some(&ex), TEST_YEAR, vec![], &[],
    );
    assert!(
        !report.findings.iter().any(|f| f.provenance.iter().any(|p| p == "signal:uncited_reference")),
        "uncited-reference findings must not reach the report yet (see uncited_reference_findings docs)"
    );
}

/// Registry evidence for one reference: a resolved year, optionally a count.
fn rv(raw: &str, matched_year: Option<i32>, citation_count: Option<i64>) -> ReferenceVerification {
    use crate::refverify::{Enrichment, ExistenceCheck, Provenance};
    let prov = Provenance {
        source: "test".into(),
        url: String::new(),
        fetched_at: 0,
        checksum: String::new(),
        from_cache: false,
    };
    ReferenceVerification {
        reference_raw: raw.into(),
        exists: matched_year.map(|y| ExistenceCheck {
            source: "test".into(),
            found: true,
            doi: None,
            title: None,
            matched_authors: None,
            matched_year: Some(y),
            is_retracted_hint: Some(false),
            provenance: prov.clone(),
        }),
        retraction: None,
        open_access: None,
        enrichment: citation_count.map(|c| Enrichment {
            citation_count: Some(c),
            influential_citation_count: None,
            abstract_text: None,
            venue: None,
            provenance: prov.clone(),
        }),
        provenance: vec![prov],
        warnings: vec![],
    }
}

fn extraction_with_two_dated_references() -> ExtractionResult {
    crate::extract::extract_from_text(
        "T\n\nAbstract\nWe build on prior work (Smith, 2020).\n\n\
         References\n\nSmith J. 2020. A paper. Journal of Things 1: 1-10.\n\n\
         Jones A. 2019. Another paper. Journal of Other Things 2: 11-20.\n",
    )
}

fn finding_with_signal<'a>(r: &'a PublishReadyReport, sig: &str) -> Option<&'a Finding> {
    let tag = format!("signal:{sig}");
    r.findings.iter().find(|f| f.provenance.iter().any(|p| *p == tag))
}




#[test]
fn the_citation_count_summary_is_descriptive_and_states_what_it_omits() {
    let ex = extraction_with_two_dated_references();
    let raws: Vec<String> = ex.references.iter().map(|r| r.raw.clone()).collect();
    let registry = vec![rv(&raws[0], None, Some(42)), rv(&raws[1], None, None)];
    let validation = crate::validate::validate(&ex);
    let report = compile_report(
        &minimal_outcome(), &validation, None, None, Some(&ex), TEST_YEAR, vec![], &registry,
    );
    let f = finding_with_signal(&report, "citation_count").expect("missing finding");
    assert_eq!(f.severity, FindingSeverity::Info, "descriptive only — never above Info");
    assert!(f.detail.contains("median citation count 42"), "{}", f.detail);
    assert!(
        f.detail.contains("1 reference(s) were not resolved and are not described here"),
        "unresolved count must be disclosed, not hidden: {}", f.detail
    );
}

#[test]
fn no_registry_evidence_produces_no_citation_count_finding() {
    let ex = extraction_with_two_dated_references();
    let validation = crate::validate::validate(&ex);
    let report = compile_report(
        &minimal_outcome(), &validation, None, None, Some(&ex), TEST_YEAR, vec![], &[],
    );
    assert!(finding_with_signal(&report, "citation_count").is_none());
}

fn verification_with(verdicts: Vec<(&str, Verdict, &str)>) -> VerificationReport {
    VerificationReport {
        verdicts: verdicts
            .into_iter()
            .map(|(id, verdict, refs)| CitationVerdict {
                citation_id: id.into(),
                verdict,
                confidence: 0.5,
                rationale: "model returned no verdict for this citation".into(),
                evidence_refs: if refs.is_empty() { vec![] } else { vec![refs.into()] },
                gate_flags: vec![],
            })
            .collect(),
        warnings: vec![],
    }
}

#[test]
fn many_unknown_verdicts_collapse_into_one_finding() {
    // The defect this fixes: 28 identical findings, 58% of a real report.
    let v = verification_with(
        (1..=28).map(|i| (Box::leak(format!("c{i}").into_boxed_str()) as &str, Verdict::Unknown, "")).collect(),
    );
    let ex = extraction_with_uncaptioned_table();
    let validation = crate::validate::validate(&ex);
    let report = compile_report(
        &minimal_outcome(), &validation, Some(&v), None, Some(&ex), TEST_YEAR, vec![], &[],
    );
    let unchecked: Vec<&Finding> = report
        .findings
        .iter()
        .filter(|f| f.provenance.iter().any(|p| p == "signal:citations_unchecked"))
        .collect();
    assert_eq!(unchecked.len(), 1, "28 Unknown verdicts must yield ONE finding");
    let f = unchecked[0];
    assert!(f.title.contains("28 of 28"), "count and denominator in the title: {}", f.title);
    // The collapse must not name INTERNAL INDICES. With no registry nothing
    // resolves, so the list is OMITTED rather than rendered as a repeated
    // placeholder — the count still carries the fact.
    assert!(!f.detail.contains("c1"), "internal index leaked: {}", f.detail);
    assert!(!f.detail.contains("a reference, a reference"), "placeholder repeated: {}", f.detail);
    assert!(f.detail.contains("28 of 28"), "the count survives: {}", f.detail);
    // Author-facing wording — our infrastructure must not appear in their report.
    assert!(!f.detail.contains("model returned no verdict"), "infra wording leaked: {}", f.detail);
    assert!(f.detail.contains("not a finding about your references"), "{}", f.detail);
    // And no per-citation UNKNOWN findings remain.
    assert!(
        !report.findings.iter().any(|f| f.title.contains("could not be verified")),
        "per-citation UNKNOWN findings must be gone"
    );
}

#[test]
fn refuted_and_supported_still_fan_out_per_citation() {
    let v = verification_with(vec![
        ("c1", Verdict::Refuted, "ev-c1-0"),
        ("c2", Verdict::Supported, "ev-c2-0"),
        ("c3", Verdict::Unknown, ""),
    ]);
    let ex = extraction_with_uncaptioned_table();
    let validation = crate::validate::validate(&ex);
    let report = compile_report(
        &minimal_outcome(), &validation, Some(&v), None, Some(&ex), TEST_YEAR, vec![], &[],
    );
    // Per-citation fan-out survives; the TITLES no longer name c1/c2.
    assert!(
        report.findings.iter().any(|f| f.title.contains("refuted by the literature")),
        "Refuted stays per-citation"
    );
    assert!(
        report.findings.iter().any(|f| f.title.contains("supported by the literature")),
        "Supported stays per-citation"
    );
    assert!(
        !report.findings.iter().any(|f| f.title.contains("c1") || f.title.contains("c2")),
        "no title may name an internal index"
    );
    assert_eq!(
        report.findings.iter().filter(|f| f.provenance.iter().any(|p| p == "signal:citations_unchecked")).count(),
        1
    );
}

#[test]
fn the_collapse_preserves_evidence_grounding() {
    // Collapsing PRESENTATION must not collapse PROVENANCE.
    let v = verification_with(vec![
        ("c1", Verdict::Unknown, "ev-c1-0"),
        ("c2", Verdict::Unknown, "ev-c2-0"),
    ]);
    let ex = extraction_with_uncaptioned_table();
    let validation = crate::validate::validate(&ex);
    let report = compile_report(
        &minimal_outcome(), &validation, Some(&v), None, Some(&ex), TEST_YEAR, vec![], &[],
    );
    let f = report
        .findings
        .iter()
        .find(|f| f.provenance.iter().any(|p| p == "signal:citations_unchecked"))
        .unwrap();
    assert!(f.provenance.iter().any(|p| p == "evidence:ev-c1-0"), "{:?}", f.provenance);
    assert!(f.provenance.iter().any(|p| p == "evidence:ev-c2-0"), "{:?}", f.provenance);
}

/// Does the reviewer payload carry a finding bearing this `signal:` tag?
/// `build_review_payload` filters provenance to structured prefixes and re-emits
/// them under `evidence`, so a `signal:` tag is how a family is identified there.
fn payload_has_signal(payload: &serde_json::Value, signal: &str) -> bool {
    let tag = format!("signal:{signal}");
    payload["summary"]["findings"]
        .as_array()
        .map(|fs| {
            fs.iter().any(|f| {
                f["evidence"]
                    .as_array()
                    .map(|es| es.iter().any(|e| e.as_str() == Some(tag.as_str())))
                    .unwrap_or(false)
            })
        })
        .unwrap_or(false)
}

/// THE CAP INTERACTION. Severity-first ordering must keep the new
/// extraction-derived families (Info/Minor) BELOW the reviewer's top-N cutoff
/// whenever anything more urgent exists — and let them through when nothing does.
///
/// Asserted end-to-end through the real path (`compile_report` -> serialize ->
/// `build_review_payload`), not by inspecting the sort.
#[test]
fn extraction_derived_findings_yield_to_more_urgent_findings_at_the_reviewer_cap() {
    use crate::reviewer_agent::{build_review_payload, TargetJournal};
    let journal = TargetJournal { name: "J".into(), quartile: "Q1".into() };
    let ex = extraction_with_uncaptioned_table();
    let validation = crate::validate::validate(&ex);
    let outcome = minimal_outcome();

    // --- CASE A: more MAJOR findings than the cap --------------------------
    // 14 plagiarism matches -> 14 Major findings, which alone exceed the 12-item
    // cap, so every Info/Minor extraction-derived finding must be squeezed out.
    let crowded = plagiarism_with_n_major_matches(14);
    let report_a =
        compile_report(&outcome, &validation, None, Some(&crowded), Some(&ex), TEST_YEAR, vec![], &[]);

    // The finding EXISTS locally — this test is about the payload, not the report.
    assert_eq!(by_signal(&report_a, "tables").len(), 1, "the table finding is in the local report");
    assert!(
        report_a.findings.len() > REVIEWER_MAX_FINDINGS,
        "fixture must exceed the cap to test it, got {}",
        report_a.findings.len()
    );

    // The typed report is passed directly: the to_value round-trip existed only
    // because build_review_payload could not accept it.
    let (payload_a, sent_a) = build_review_payload(&report_a, &journal, &[], "run-a");
    assert_eq!(sent_a.findings.len(), REVIEWER_MAX_FINDINGS, "payload caps at top-N");
    assert!(
        payload_a["summary"]["findings_omitted"].as_u64().unwrap_or(0) > 0,
        "the drop must be reported honestly via findings_omitted"
    );
    // The cutoff is severity-driven: only Major plagiarism findings made it.
    for f in payload_a["summary"]["findings"].as_array().unwrap() {
        assert_eq!(
            f["severity"], "major",
            "the top-12 must be the urgent band, got {f:?}"
        );
    }
    for family in ["tables", "citation_recency", "citation_density"] {
        assert!(
            !payload_has_signal(&payload_a, family),
            "{family} is Info/Minor and must NOT displace a Major finding at the cap"
        );
    }

    // --- CASE B: room to spare ---------------------------------------------
    // 2 Major findings, so the extraction-derived families fit under the cap.
    let sparse = plagiarism_with_n_major_matches(2);
    let report_b =
        compile_report(&outcome, &validation, None, Some(&sparse), Some(&ex), TEST_YEAR, vec![], &[]);
    assert!(
        report_b.findings.len() <= REVIEWER_MAX_FINDINGS,
        "fixture must fit under the cap, got {}",
        report_b.findings.len()
    );

    let (payload_b, sent_b) = build_review_payload(&report_b, &journal, &[], "run-b");
    assert_eq!(sent_b.findings.len(), report_b.findings.len(), "nothing dropped when it fits");
    assert_eq!(
        payload_b["summary"]["findings_omitted"].as_u64().unwrap_or(0),
        0,
        "nothing omitted when everything fits"
    );
    assert!(
        payload_has_signal(&payload_b, "tables"),
        "with room to spare the table finding MUST reach the reviewer: {:?}",
        payload_b["summary"]["findings"]
    );
}

/// The cap test above cannot, on its own, prove the ordering is SEVERITY-driven:
/// inside `compile_report` everything more severe than the extraction-derived
/// families (Critical validation, Major plagiarism) also happens to be pushed
/// BEFORE them, so insertion order alone would produce the same cutoff. Proven
/// by mutation — deleting `severity.rank()` from the sort key left that test
/// green.
///
/// A soft round-table CONCERN is the one Major finding compiled AFTER the
/// extraction block, so it inverts insertion order and isolates the severity key:
/// it must still sort ahead of the Minor table finding.
#[test]
fn severity_beats_insertion_order_for_extraction_derived_findings() {
    let ex = extraction_with_uncaptioned_table();
    let validation = crate::validate::validate(&ex);
    // A soft CONCERN -> Major, compiled in the soft-opinion loop AFTER the
    // extraction-derived findings.
    //
    // The vehicle was AiDetection until §11 D153 capped the authorship claim at
    // `info`, which left this test with no Major to order and made it fail on a
    // PRECONDITION rather than on its subject. Rag is the replacement: its
    // opinion carries `ClaimKind::ProcessState`, which the cap does not touch,
    // so a concern still compiles to Major in the same loop at the same point.
    // The assertion below is unchanged — this test is about severity beating
    // insertion order, and which agent supplies the Major was always incidental.
    let mut agents: Vec<Box<dyn SwarmAgent>> = vec![Box::new(PrecomputedAgent::new(opinion(
        AgentKind::Rag,
        crate::swarm::ANSWER_CONCERN,
        0.6,
    )))];
    let outcome = run_debate(&mut agents, &DebateConfig::default()).unwrap();
    let report = compile_report(&outcome, &validation, None, None, Some(&ex), TEST_YEAR, vec![], &[]);

    let major = report
        .findings
        .iter()
        .position(|f| f.severity == FindingSeverity::Major)
        .expect("the soft concern must compile to a Major finding");
    let table = report
        .findings
        .iter()
        .position(|f| f.provenance.iter().any(|p| p == "signal:tables"))
        .expect("the table finding must be present");
    assert!(
        major < table,
        "a Major finding compiled AFTER the extraction block must still SORT before its \
         Minor findings — otherwise the ordering is insertion order, not severity \
         (major at {major}, table at {table})"
    );

    // And the whole report is non-decreasing in severity, which is the property
    // the reviewer's top-N truncation actually relies on.
    let ranks: Vec<u8> = report.findings.iter().map(|f| f.severity.rank()).collect();
    assert!(
        ranks.windows(2).all(|w| w[0] <= w[1]),
        "findings must be ordered by severity for top-N truncation to mean anything: {ranks:?}"
    );
}

/// PIN the plagiarism wording to what the algorithm actually measures, and keep
/// it in lockstep with the frontend mirror (`matchTypeLabel` in
/// checks/adapters.ts, pinned by the same literals in
/// checks/plagiarism.vitest.tsx).
///
/// `similarity` is cosine over HashEmbedder — a bag-of-words encoder
/// (embed.rs:33-53). The words below claim word overlap and nothing more. The
/// previous set ("verbatim"/"near-verbatim"/"paraphrase") claimed verbatim
/// identity and paraphrase detection, neither of which a bag-of-words cosine
/// can support; "paraphrase" in particular named the inverse of the signal.
#[test]
fn match_type_label_wording_is_supported_by_the_algorithm() {
    let corpus_at = |sim: f64| MatchSpan {
        manuscript_chunk_seq: 0,
        manuscript_excerpt: "x".into(),
        similarity: sim,
        source: MatchSource::Corpus {
            document_id: 1,
            chunk_id: 1,
            title: "T".into(),
            source_url: "u".into(),
            source_type: "corpus".into(),
            excerpt: "e".into(),
        },
    };

    // Self-matches: the source enum knows WHERE the match is, not that the reuse
    // was illegitimate — so the label states location only, never a determination.
    let self_at = |sim: f64| MatchSpan {
        manuscript_chunk_seq: 0,
        manuscript_excerpt: "x".into(),
        similarity: sim,
        source: MatchSource::SelfManuscript { other_chunk_seq: 3, excerpt: "e".into() },
    };
    for sim in [1.00, 0.90, 0.81] {
        let label = super::match_type_label(&self_at(sim));
        assert_eq!(label, "internal duplication (same manuscript)");
        assert!(
            !label.contains("plagiarism"),
            "the label must not assert plagiarism — ISOLATION_NOTE disclaims exactly that"
        );
    }

    // The three bands, at and just under each boundary.
    assert_eq!(super::match_type_label(&corpus_at(1.00)), "near-identical wording");
    assert_eq!(super::match_type_label(&corpus_at(0.98)), "near-identical wording");
    assert_eq!(super::match_type_label(&corpus_at(0.97)), "high word overlap");
    assert_eq!(super::match_type_label(&corpus_at(0.85)), "high word overlap");
    assert_eq!(super::match_type_label(&corpus_at(0.84)), "partial lexical overlap");

    // No band may claim verbatim identity, paraphrase detection, or semantics —
    // a bag-of-words cosine supports none of the three.
    for sim in [1.00, 0.98, 0.90, 0.84, 0.80] {
        let label = super::match_type_label(&corpus_at(sim));
        for forbidden in ["paraphrase", "semantic"] {
            assert!(
                !label.contains(forbidden),
                "label {label:?} at similarity {sim} claims {forbidden:?}, which cosine over a \
                 bag-of-words encoder cannot support"
            );
        }
        assert_ne!(label, "verbatim", "word-order-blind cosine cannot verify verbatim identity");
    }

    // The isolation note must not call the signal semantic either.
    let note = crate::plagiarism::ISOLATION_NOTE;
    assert!(note.contains("lexical-overlap signal"), "note must name the real signal: {note}");
    assert!(!note.contains("semantic"), "note must not claim semantics: {note}");
}

// ===========================================================================
// Step 3 — the finding's location, and the quotation it resolves to
// ===========================================================================

/// Two Results paragraphs, each reporting a p-value with no effect size, so
/// `MissingEffectSize` fires TWICE at two DIFFERENT paragraph indices. A
/// resolver that ignores the index, or is off by one, or ignores the section,
/// produces prose from the wrong paragraph — which is what these tests catch.
const TWO_LOCATED_PARAGRAPHS: &str = "Located Findings\n\nMethods\nThe gamma \
    marker was measured (p = 0.04).\n\nResults\nThe ALPHAMARKER improved recall \
    (p = 0.01).\n\nThe BETAMARKER reduced errors (p = 0.02).\n";

/// **The location a finding carries is the location its RULE evaluated.**
///
/// Not "a location that looks right": `validate.rs` reads
/// `extract::paragraph_at(result, &flag.location)` to decide whether the rule
/// fires, so carrying `flag.location` unchanged means the quotation the report
/// shows is the exact string that produced the finding. Asserted as a MULTISET
/// against `validation.flags` rather than positionally, so the sort order of
/// `compile_report` is not silently under test here.
#[test]
fn validation_findings_carry_the_location_their_rule_evaluated() {
    let ex = crate::extract::extract_from_text(TWO_LOCATED_PARAGRAPHS);
    let validation = crate::validate::validate(&ex);
    assert!(
        validation.flags.len() >= 2,
        "fixture must produce at least two located flags: {:?}",
        validation.flags
    );

    let report = compile_report(
        &minimal_outcome(),
        &validation,
        None,
        None,
        None,
        TEST_YEAR,
        vec![],
        &[],
    );

    let mut from_findings: Vec<Location> =
        report.findings.iter().filter_map(|f| f.location.clone()).collect();
    let mut from_flags: Vec<Location> =
        validation.flags.iter().map(|f| f.location.clone()).collect();
    from_findings.sort();
    from_flags.sort();
    assert_eq!(
        from_findings, from_flags,
        "every validation flag's location must reach its finding unchanged"
    );

    // And each distinct location must resolve to ITS OWN paragraph. The markers
    // are unique, so a wrong index or a dropped section filter shows up as the
    // other paragraph's marker rather than as an absence.
    let want = [
        (SectionKind::Results, 0usize, "ALPHAMARKER", "BETAMARKER"),
        (SectionKind::Results, 1, "BETAMARKER", "ALPHAMARKER"),
        (SectionKind::Methods, 0, "gamma marker", "ALPHAMARKER"),
    ];
    for (section, paragraph, expected, forbidden) in want {
        let loc = Location { section, paragraph };
        let text = crate::extract::paragraph_at(&ex, &loc)
            .unwrap_or_else(|| panic!("{loc:?} must resolve"));
        assert!(text.contains(expected), "{loc:?} resolved to {text:?}, missing {expected:?}");
        assert!(
            !text.contains(forbidden),
            "{loc:?} resolved to {text:?}, which is another paragraph's text"
        );
    }
}

/// An unresolvable location yields `None`, never `Some("")`.
///
/// The rule path wants `""` (no regex matches it, so no rule fires); the report
/// path wants `None` (no quotation is shown). `paragraph_at` returns the typed
/// absence and `validate::paragraph` adds the `""` — so one resolver serves both
/// without either faking a result for the other.
#[test]
fn an_unresolvable_location_resolves_to_a_typed_absence() {
    let ex = crate::extract::extract_from_text(TWO_LOCATED_PARAGRAPHS);
    assert_eq!(
        crate::extract::paragraph_at(&ex, &Location { section: SectionKind::Results, paragraph: 99 }),
        None,
        "a paragraph index past the end must not resolve"
    );
    assert_eq!(
        crate::extract::paragraph_at(&ex, &Location { section: SectionKind::Discussion, paragraph: 0 }),
        None,
        "a section the document does not have must not resolve"
    );
}

/// **The set of families carrying a location is PINNED.**
///
/// `None` is the correct answer for a finding about the whole document, and a
/// gap for a finding that has a place the type does not carry. Both look
/// identical at runtime, so this fixes the wired set: a new family that acquires
/// a location, or an existing one that quietly loses it, fails here and the
/// decision has to be made in the open rather than defaulting to silence.
#[test]
fn only_located_families_carry_a_location() {
    let ex = crate::extract::extract_from_text(TWO_LOCATED_PARAGRAPHS);
    let validation = crate::validate::validate(&ex);
    let plag = plagiarism_with_n_major_matches(2);
    let verdicts = VerificationReport {
        verdicts: vec![CitationVerdict {
            citation_id: "c1".into(),
            verdict: Verdict::Refuted,
            confidence: 0.7,
            rationale: "no match".into(),
            evidence_refs: vec![],
            gate_flags: vec![],
        }],
        warnings: vec![],
    };
    let report = compile_report(
        &minimal_outcome(),
        &validation,
        Some(&verdicts),
        Some(&plag),
        Some(&extraction_with_two_dated_references()),
        TEST_YEAR,
        vec![],
        &[],
    );

    let mut families: Vec<(AgentKind, bool)> =
        report.findings.iter().map(|f| (f.agent, f.location.is_some())).collect();
    families.sort_by_key(|(a, l)| (format!("{a:?}"), *l));
    families.dedup();

    for (agent, located) in &families {
        assert_eq!(
            *located,
            *agent == AgentKind::ValidationMaths,
            "agent {agent:?} located={located}: exactly ValidationMaths carries a location \
             today. If this family gained one, wire it and update this pin; if it lost one, \
             that is a regression."
        );
    }
    assert!(
        families.iter().any(|(a, l)| *a == AgentKind::ValidationMaths && *l),
        "the fixture must exercise at least one located finding: {families:?}"
    );
    assert!(
        families.len() > 1,
        "the fixture must exercise more than one family: {families:?}"
    );
}

/// **THE LOCATION DOES NOT CROSS THE PROXY BOUNDARY, AND NEITHER DOES THE PROSE
/// IT RESOLVES TO.**
///
/// `build_review_payload` is a positive construction — a `json!` literal naming
/// seven keys — so a new field on `Finding` cannot leak by being carried along.
/// This asserts that property rather than trusting it: the finding's location
/// names a real section and paragraph of a real manuscript, and neither the key,
/// the section name, nor any word of the quoted paragraph appears in the bytes
/// that would be sent.
///
/// The stronger half is structural and lives elsewhere: `LocalReportModel`,
/// which holds the resolved paragraph, has NO `Serialize` derive, so the type
/// carrying the prose cannot be serialized at all (ONTOLOGY §4.22).
#[test]
fn a_findings_location_never_reaches_the_reviewer_payload() {
    use crate::reviewer_agent::{build_review_payload, TargetJournal};
    let ex = crate::extract::extract_from_text(TWO_LOCATED_PARAGRAPHS);
    let validation = crate::validate::validate(&ex);
    let report = compile_report(
        &minimal_outcome(),
        &validation,
        None,
        None,
        None,
        TEST_YEAR,
        vec![],
        &[],
    );
    assert!(
        report.findings.iter().any(|f| f.location.is_some()),
        "the fixture must produce a located finding"
    );

    let (payload, _sent) = build_review_payload(
        &report,
        &TargetJournal { name: "J".into(), quartile: "Q1".into() },
        &[],
        "run-loc",
    );
    let bytes = serde_json::to_string(&payload).unwrap();

    assert!(!bytes.contains("location"), "the location key reached the payload: {bytes}");
    assert!(!bytes.contains("paragraph"), "a paragraph index reached the payload: {bytes}");
    // The manuscript markers are unique to the quoted paragraphs.
    for marker in ["ALPHAMARKER", "BETAMARKER", "gamma marker"] {
        assert!(!bytes.contains(marker), "manuscript prose {marker:?} reached the payload: {bytes}");
    }
}

/// **A FINDING MUST NOT NAME AN INTERNAL INDEX.** `citation_id` is `c{i+1}`
/// over a slice `pipeline.rs` builds by SKIPPING failed lookups, so it is not
/// even the author's bibliography position. The registry carries the entry as
/// the manuscript wrote it, in the same order.
#[test]
fn a_citation_finding_names_the_reference_not_its_index() {
    let ex = crate::extract::extract_from_text(TWO_LOCATED_PARAGRAPHS);
    let validation = crate::validate::validate(&ex);
    let verdicts = VerificationReport {
        verdicts: vec![CitationVerdict {
            citation_id: "c1".into(),
            verdict: Verdict::Refuted,
            confidence: 0.7,
            rationale: "no match".into(),
            evidence_refs: vec![],
            gate_flags: vec![],
        }],
        warnings: vec![],
    };
    let reg = vec![rv("Smith J. 2020. A paper about things. Journal of Things 1: 1-9.", None, None)];
    let report = compile_report(
        &minimal_outcome(), &validation, Some(&verdicts), None, None, TEST_YEAR, vec![], &reg);

    let title = report
        .findings
        .iter()
        .find(|f| f.agent == AgentKind::Verification && f.severity == FindingSeverity::Major)
        .map(|f| f.title.clone())
        .expect("a refuted-citation finding");
    assert!(title.contains("Smith J. 2020"), "must name the reference: {title}");
    assert!(!title.contains("c1"), "must NOT name the internal index: {title}");
}

/// When the id cannot be resolved the finding says so rather than falling back
/// to the index or inventing a reference (§4.12).
#[test]
fn an_unresolvable_citation_id_yields_a_typed_absence_not_an_index() {
    let ex = crate::extract::extract_from_text(TWO_LOCATED_PARAGRAPHS);
    let validation = crate::validate::validate(&ex);
    let verdicts = VerificationReport {
        verdicts: vec![CitationVerdict {
            citation_id: "c9".into(),
            verdict: Verdict::Refuted,
            confidence: 0.7,
            rationale: "no match".into(),
            evidence_refs: vec![],
            gate_flags: vec![],
        }],
        warnings: vec![],
    };
    let report = compile_report(
        &minimal_outcome(), &validation, Some(&verdicts), None, None, TEST_YEAR, vec![], &[]);
    let title = report
        .findings
        .iter()
        .find(|f| f.agent == AgentKind::Verification && f.severity == FindingSeverity::Major)
        .map(|f| f.title.clone())
        .expect("a refuted-citation finding");
    assert!(title.contains("a reference"), "typed absence: {title}");
    assert!(!title.contains("c9"), "must not leak the index: {title}");
}

// ============================================================================
// BLOCKER 4 — the disclaimer describes the tiers the report actually has
// ============================================================================

/// **A tier no finding carries must not be explained as though it did.**
///
/// The disclaimer named three certainty tiers unconditionally, including
/// *"'reconsidered after peer review' findings were revised by the verification
/// agent after seeing other agents' evidence"*. Nothing in production can
/// revise: every participant is a `PrecomputedAgent` and `SwarmAgent::revise`
/// returns `None`, so `revised_agents` is empty on every run — measured as empty
/// in **22 of 22** stored reports.
///
/// A researcher reading that sentence would reasonably conclude a peer-review
/// step had happened and found nothing to revise. No such step ran. The clause
/// describes a mechanism that exists in the code and did not execute, which is a
/// worse failure than a missing explanation: it is an explanation of something
/// that did not occur, printed beside findings that did.
#[test]
fn the_disclaimer_omits_the_revision_tier_when_nothing_was_revised() {
    let outcome = minimal_outcome();
    assert!(
        outcome.revised_agents.is_empty(),
        "fixture precondition: this outcome must have no revisions"
    );
    let ex = crate::extract::extract_from_text("T\n\nAbstract\nA.\n\nResults\nR.\n");
    let validation = crate::validate::validate(&ex);
    let report = compile_report(&outcome, &validation, None, None, None, TEST_YEAR, vec![], &[]);

    assert!(
        !report.disclaimer.contains("reconsidered after peer review"),
        "the disclaimer explains a tier no finding carries: {:?}",
        report.disclaimer
    );
    // The two tiers that ARE reachable must still be explained — the fix is to
    // drop a clause, never to drop the disclaimer.
    assert!(report.disclaimer.contains("mathematically certain"), "{:?}", report.disclaimer);
    assert!(report.disclaimer.contains("AI-assessed"), "{:?}", report.disclaimer);
    assert!(!report.disclaimer.is_empty());

    // And no finding may claim the tier either — the disclaimer and the
    // findings must agree about which tiers this report has.
    assert!(
        !report
            .findings
            .iter()
            .any(|f| f.tier == CertaintyTier::ReconsideredAfterPeerReview),
        "a finding carries the revision tier on a run with no revisions"
    );
}

/// The other direction, so the fix is not "delete the sentence". When an agent
/// DOES revise, the clause must come back — otherwise the tier would be carried
/// by findings and explained nowhere, which is the same defect mirrored.
///
/// Built the same way as
/// `every_finding_carries_provenance_and_correct_tier_including_reconsidered`:
/// a scripted two-response proxy, so the verification agent revises on seeing
/// the plagiarism agent's concern.
#[test]
fn the_disclaimer_restores_the_revision_tier_when_an_agent_revises() {
    let proxy = MockProxyClient::returning_sequence(vec![
        json!({"verdicts": [{
            "citation_id": "c1", "verdict": "SUPPORTED", "confidence": 0.9,
            "rationale": "matches", "evidence_refs": ["ev-c1-0"]
        }]}),
        json!({"verdicts": [{
            "citation_id": "c1", "verdict": "UNKNOWN", "confidence": 0.3,
            "rationale": "peer similarity finding undermines support",
            "evidence_refs": ["ev-c1-0"]
        }]}),
    ]);
    let reference = Reference {
        raw: "Doe, J. (2022). A paper. https://doi.org/10.1/abc".into(),
        authors: "Doe, J.".into(),
        year: Some(2022),
        title: Some("A paper".into()),
        doi: Some("10.1/abc".into()),
    };
    let items = verification_items(reference);
    let initial = verify_citations(&proxy, &items).unwrap();
    let mut agents: Vec<Box<dyn SwarmAgent + '_>> = vec![
        Box::new(PrecomputedAgent::new(opinion(
            AgentKind::Plagiarism,
            crate::swarm::ANSWER_CONCERN,
            0.92,
        ))),
        Box::new(RevisingVerificationAgent::new(&proxy, items, initial)),
    ];
    let outcome = run_debate(&mut agents, &DebateConfig::default()).unwrap();
    assert!(
        outcome.revised_agents.contains(&AgentKind::Verification),
        "fixture precondition: this outcome must contain a revision"
    );

    let ex = crate::extract::extract_from_text("T\n\nAbstract\nA.\n\nResults\nR.\n");
    let validation = crate::validate::validate(&ex);
    let report = compile_report(&outcome, &validation, None, None, None, TEST_YEAR, vec![], &[]);
    assert!(
        report.disclaimer.contains("reconsidered after peer review"),
        "a revised run must explain the tier its findings carry: {:?}",
        report.disclaimer
    );
}

// ============================================================================
// BLOCKER 5 — a statement about our run is not a finding about their paper
// ============================================================================

/// A debate in which the Verification agent's opinion fails its own internal
/// gate — the shape that produced `"Verification output rejected by its internal
/// gate"` in 20 of 22 stored reports.
fn outcome_with_a_gate_rejection() -> crate::swarm::DebateOutcome {
    let mut rejected = opinion(AgentKind::Verification, ANSWER_PASS, 0.8);
    rejected.gate_passed = false;
    rejected.explanation = "verdicts were not grounded in the supplied evidence".into();
    let mut agents: Vec<Box<dyn SwarmAgent>> = vec![
        Box::new(PrecomputedAgent::new(opinion(AgentKind::Rag, ANSWER_PASS, 0.75))),
        Box::new(PrecomputedAgent::new(rejected)),
    ];
    run_debate(&mut agents, &DebateConfig::default()).unwrap()
}

/// **The gate rejection is recorded, and it is not a finding.**
///
/// Both halves matter and the test would be wrong with either alone: dropping
/// the note entirely would hide from an author that a lane produced nothing
/// usable, and leaving it in `findings` puts a fact about our harness in the
/// list of things wrong with their manuscript, in the same severity vocabulary.
#[test]
fn a_gate_rejection_becomes_a_harness_note_and_never_a_finding() {
    use crate::report::HarnessNoteReason;
    let outcome = outcome_with_a_gate_rejection();
    assert_eq!(outcome.rejected.len(), 1, "fixture precondition: one rejected opinion");

    let ex = crate::extract::extract_from_text("T\n\nAbstract\nA.\n\nResults\nR.\n");
    let validation = crate::validate::validate(&ex);
    let report = compile_report(&outcome, &validation, None, None, None, TEST_YEAR, vec![], &[]);

    // Not in the findings list, by title or by provenance.
    assert!(
        !report.findings.iter().any(|f| f.title.contains("rejected by its internal gate")),
        "a harness statement is still rendered as a finding: {:?}",
        report.findings.iter().map(|f| &f.title).collect::<Vec<_>>()
    );
    assert!(
        !report
            .findings
            .iter()
            .any(|f| f.provenance.iter().any(|p| p.starts_with("swarm:rejected"))),
        "a rejected opinion still reaches findings by provenance"
    );
    // `findings` and `evidence` are unzipped from one source and must stay the
    // same length — removing an item from one without the other is the desync
    // the pairing exists to prevent.
    assert_eq!(report.findings.len(), report.evidence.len());

    // Recorded, with the agent, its own words, and a machine-readable reason.
    assert_eq!(report.harness_notes.len(), 1, "the note must survive the move");
    let note = &report.harness_notes[0];
    assert_eq!(note.agent, AgentKind::Verification);
    assert_eq!(note.reason, HarnessNoteReason::OutputRejectedByInternalGate);
    assert!(
        note.detail.contains("not grounded"),
        "the note must carry the agent's own explanation: {:?}",
        note.detail
    );

    // And the structured summary still names the agent, so the two agree.
    assert!(report.debate.rejected_agents.contains(&AgentKind::Verification));
}

/// A clean run has no notes — the field is not a place where something always
/// appears, which is what would make readers stop looking at it.
#[test]
fn a_run_with_no_rejections_has_no_harness_notes() {
    let outcome = minimal_outcome();
    let ex = crate::extract::extract_from_text("T\n\nAbstract\nA.\n\nResults\nR.\n");
    let validation = crate::validate::validate(&ex);
    let report = compile_report(&outcome, &validation, None, None, None, TEST_YEAR, vec![], &[]);
    assert!(report.harness_notes.is_empty());
}

/// Reports cached before `harness_notes` existed must still parse. The serde
/// drift this guards has bitten five times (§11 D53, D103, D109); a defaulted
/// field is only additive if something checks that old rows load.
#[test]
fn a_report_cached_before_harness_notes_still_parses() {
    let json = serde_json::json!({
        "verdict": "pass", "combined_confidence": 0.8,
        "findings": [], "evidence": [], "checklist": [],
        "debate": { "rounds_run": 1, "converged": true, "overridden_by_constraint": false,
                    "rejected_agents": [], "revised_agents": [] },
        "disclaimer": "Certainty tiers: ..."
    });
    let parsed: PublishReadyReport =
        serde_json::from_value(json).expect("a pre-field report must still parse");
    assert!(parsed.harness_notes.is_empty());
}

// ============================================================================
// §11 D153 — the authorship signal may never be louder than `info`
// ============================================================================

/// A debate whose AI-detection opinion is a CONCERN — the shape that produced
/// `[major] AiDetection: concern` in 19 of 22 stored reports.
fn outcome_with_an_ai_concern() -> crate::swarm::DebateOutcome {
    let mut agents: Vec<Box<dyn SwarmAgent>> = vec![
        Box::new(PrecomputedAgent::new(opinion(AgentKind::Rag, ANSWER_PASS, 0.75))),
        Box::new(PrecomputedAgent::new(opinion(
            AgentKind::AiDetection,
            crate::swarm::ANSWER_CONCERN,
            0.6,
        ))),
    ];
    run_debate(&mut agents, &DebateConfig::default()).unwrap()
}

/// **The cap.** An AI-detection concern lands at `info`, never `major`.
#[test]
fn an_authorship_concern_is_capped_at_info() {
    let outcome = outcome_with_an_ai_concern();
    let ex = crate::extract::extract_from_text("T\n\nAbstract\nA.\n\nResults\nR.\n");
    let validation = crate::validate::validate(&ex);
    let report = compile_report(&outcome, &validation, None, None, None, TEST_YEAR, vec![], &[]);

    let f = report
        .findings
        .iter()
        .find(|f| f.claim == ClaimKind::AuthorshipSignal)
        .expect("the authorship opinion must still produce a finding — it is not deleted");
    assert_eq!(f.severity, FindingSeverity::Info, "an authorship signal may not exceed info");
    assert_eq!(f.agent, AgentKind::AiDetection);

    // NOTHING in the report may carry AuthorshipSignal above info.
    for f in &report.findings {
        if f.claim == ClaimKind::AuthorshipSignal {
            assert_eq!(f.severity, FindingSeverity::Info, "uncapped authorship finding: {f:?}");
        }
    }
}

/// **THE ACCIDENT THIS GUARDS.** The cap is keyed on the CLAIM. Keyed on
/// `AgentKind::AiDetection` instead it would also demote the document stylometry
/// findings — which carry `ManuscriptDefect` at `Minor`, rest on lexical
/// diversity and citation density rather than on perplexity thresholds, and were
/// deliberately kept reviewer-relevant by `reviewer_agent.rs:1080-1084`.
///
/// Re-deciding that question by accident, in the opposite direction, is exactly
/// what a producer-keyed cap would have done — and it would have looked correct.
#[test]
fn the_cap_does_not_touch_stylometry_findings_from_the_same_agent() {
    let outcome = outcome_with_an_ai_concern();
    // Prose with low lexical diversity, so a stylometry finding is produced.
    let repetitive = format!(
        "Title\n\nAbstract\n{}\n\nResults\n{}\n",
        "the study the study the study of the study by the study ".repeat(40),
        "the result the result the result of the result by the result ".repeat(40)
    );
    let ex = crate::extract::extract_from_text(&repetitive);
    let validation = crate::validate::validate(&ex);
    let report =
        compile_report(&outcome, &validation, None, None, Some(&ex), TEST_YEAR, vec![], &[]);

    let stylo: Vec<&Finding> = report
        .findings
        .iter()
        .filter(|f| f.agent == AgentKind::AiDetection && f.claim == ClaimKind::ManuscriptDefect)
        .collect();
    assert!(
        !stylo.is_empty(),
        "fixture precondition: this prose must produce a stylometry finding, else the \
         guard asserts nothing; got {:?}",
        report.findings.iter().map(|f| (&f.title, f.claim)).collect::<Vec<_>>()
    );
    for f in &stylo {
        assert_eq!(
            f.severity,
            FindingSeverity::Minor,
            "a stylometry finding was demoted by the authorship cap: {f:?}"
        );
    }
    // And the same report still caps the authorship claim — both behaviours at once.
    assert!(report
        .findings
        .iter()
        .filter(|f| f.claim == ClaimKind::AuthorshipSignal)
        .all(|f| f.severity == FindingSeverity::Info));
}

/// A `pass` opinion was already `info`; the cap must not change it, so a green
/// test here cannot be mistaken for the cap working.
#[test]
fn a_passing_authorship_opinion_is_unchanged() {
    let mut agents: Vec<Box<dyn SwarmAgent>> = vec![Box::new(PrecomputedAgent::new(opinion(
        AgentKind::AiDetection,
        ANSWER_PASS,
        0.6,
    )))];
    let outcome = run_debate(&mut agents, &DebateConfig::default()).unwrap();
    let ex = crate::extract::extract_from_text("T\n\nAbstract\nA.\n\nResults\nR.\n");
    let validation = crate::validate::validate(&ex);
    let report = compile_report(&outcome, &validation, None, None, None, TEST_YEAR, vec![], &[]);
    let f = report.findings.iter().find(|f| f.claim == ClaimKind::AuthorshipSignal).unwrap();
    assert_eq!(f.severity, FindingSeverity::Info);
}
