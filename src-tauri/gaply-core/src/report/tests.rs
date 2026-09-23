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

    let report = compile_report(&outcome, &validation, Some(&verification), Some(&plag), None, TEST_YEAR, checklist, &[], &[]);

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
    // **This line asserted `passed` and that was the claim §11 D181 withdrew.**
    // Nothing parses headed subsections, so a present abstract makes the item
    // UNDECIDED, not passed — asserting the pass is what kept the overclaim
    // alive. `unevaluable` is the third state, and the row must not read as a
    // pass while carrying it.
    assert!(by_req("structured abstract").unevaluable);
    assert!(!by_req("structured abstract").passed);
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
    let report = compile_report(&outcome, &validation, None, None, None, TEST_YEAR, vec![], &[], &[]);

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
    let report = compile_report(&outcome, &validation, Some(&final_verification), None, None, TEST_YEAR, vec![], &[], &[]);

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
    let report = compile_report(&outcome, &validation, None, Some(&pr), None, TEST_YEAR, vec![], &[], &[]);

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
    let report = compile_report(&outcome, &validation, None, Some(&pr), None, TEST_YEAR, vec![], &[], &[]);
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
    let report = compile_report(&outcome, &validation, None, None, None, TEST_YEAR, vec![], &[], &[]);

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
    let without = compile_report(&outcome, &validation, None, None, None, TEST_YEAR, vec![], &[], &[]);
    let with = compile_report(&outcome, &validation, None, None, Some(&ex), TEST_YEAR, vec![], &[], &[]);
    assert!(
        with.findings.len() > without.findings.len(),
        "passing the extraction must ADD findings, else the wiring is dead"
    );
    for signal in ["tables", "citation_recency", "citation_density"] {
        assert!(by_signal(&without, signal).is_empty(), "None must add no {signal} finding");
    }
}

/// **The table finding is WITHDRAWN, and this pins the withdrawal — §11 D167.**
///
/// It used to assert the count and the caption tally. Both were wrong in the
/// same way and neither was visibly wrong: `extract::detect_table` fires on any
/// paragraph opening `Table N`, so a thesis contents page is a run of matches
/// (178 of 414 detections over 20 real manuscripts — §11 D168-C corrects the
/// 237 first recorded), and each such row carries
/// the rest of its line as a "caption" — so `complete` could read TRUE on a
/// document whose real tables have none.
///
/// **The fixture below is why the old test never caught it.** Its two tables
/// ARE both real, in a Results section, with no front matter — a hand-written
/// input carrying its author's premise, which is the CLAUDE.md entry's exact
/// shape. It agreed with the function because it was built from the same idea
/// of what a document looks like. The corpus disagreed.
///
/// So this asserts the two things that must stay true while the finding is
/// gone, and names what restores it:
///
/// 1. **Nothing reaches the report**, on a document where the old finding
///    definitely fired — not on an empty one, which would pass vacuously.
/// 2. **The extractor still populates `tables`.** The withdrawal is at the
///    REPORT layer; `ExtractionResult::tables` is unchanged and other readers
///    (the lens layer, RT4's precondition) still see it. A withdrawal that
///    silently emptied the extraction would be a different and larger change.
#[test]
fn the_table_finding_is_withdrawn_and_the_extraction_is_not() {
    let outcome = minimal_outcome();

    // The fixture the OLD test used, and on which it asserted a Minor finding.
    let ex = crate::extract::extract_from_text(
        "T\n\nAbstract\nA.\n\nResults\nTable 1 Outcomes by arm\n\nTable 2\n",
    );
    // (2) precondition AND the containment: extraction is untouched.
    assert_eq!(ex.table_mentions.len(), 2, "the extractor still finds both tables");

    let validation = crate::validate::validate(&ex);
    let report =
        compile_report(&outcome, &validation, None, None, Some(&ex), TEST_YEAR, vec![], &[], &[]);

    // (1) and nothing about them reaches the user.
    assert!(
        by_signal(&report, "tables").is_empty(),
        "the table finding is withdrawn — if a table finding is being shown again, \
         D167 must be reopened with the extractor fix and its measurement: {:?}",
        report.findings.iter().map(|f| &f.title).collect::<Vec<_>>()
    );
    assert!(
        !report.findings.iter().any(|f| f.title.contains("table(s) detected")),
        "no table COUNT may be shown under any signal: {:?}",
        report.findings.iter().map(|f| &f.title).collect::<Vec<_>>()
    );
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
    let report = compile_report(&outcome, &validation, None, None, Some(&ex), TEST_YEAR, vec![], &[], &[]);

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
    let r2 = compile_report(&outcome, &v2, None, None, Some(&ex2), TEST_YEAR, vec![], &[], &[]);
    assert_eq!(by_signal(&r2, "citation_recency")[0].severity, FindingSeverity::Info);

    // No reference list -> no finding (the structural checklist covers that).
    let ex3 = crate::extract::extract_from_text("T\n\nAbstract\nA.\n");
    let v3 = crate::validate::validate(&ex3);
    let r3 = compile_report(&outcome, &v3, None, None, Some(&ex3), TEST_YEAR, vec![], &[], &[]);
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
    let report = compile_report(&outcome, &validation, None, None, Some(&ex), TEST_YEAR, vec![], &[], &[]);

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
    let r2 = compile_report(&outcome, &v2, None, None, Some(&ex2), TEST_YEAR, vec![], &[], &[]);
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
    let report = compile_report(&outcome, &validation, None, None, Some(&ex), TEST_YEAR, vec![], &[], &[]);

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
        &minimal_outcome(), &validation, None, None, Some(&ex), TEST_YEAR, vec![], &[], &[],
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
        &minimal_outcome(), &validation, None, None, Some(&ex), TEST_YEAR, vec![], &registry, &[],
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
        &minimal_outcome(), &validation, None, None, Some(&ex), TEST_YEAR, vec![], &[], &[],
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
        &minimal_outcome(), &validation, Some(&v), None, Some(&ex), TEST_YEAR, vec![], &[], &[],
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
        &minimal_outcome(), &validation, Some(&v), None, Some(&ex), TEST_YEAR, vec![], &[], &[],
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
        &minimal_outcome(), &validation, Some(&v), None, Some(&ex), TEST_YEAR, vec![], &[], &[],
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
        compile_report(&outcome, &validation, None, Some(&crowded), Some(&ex), TEST_YEAR, vec![], &[], &[]);

    // The finding EXISTS locally — this test is about the payload, not the report.
    //
    // This used the TABLE finding as its Info/Minor exemplar. That finding was
    // withdrawn (§11 D167 — its count was inflated by contents-page rows), so
    // the exemplar is now `citation_density`, which this fixture still
    // produces as Minor. The property under test is unchanged: an Info/Minor
    // extraction-derived family must yield at the cap and arrive under it.
    assert_eq!(
        by_signal(&report_a, "citation_density").len(),
        1,
        "the citation-density finding is in the local report"
    );
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
    for family in ["citation_recency", "citation_density"] {
        assert!(
            !payload_has_signal(&payload_a, family),
            "{family} is Info/Minor and must NOT displace a Major finding at the cap"
        );
    }

    // --- CASE B: room to spare ---------------------------------------------
    // 2 Major findings, so the extraction-derived families fit under the cap.
    let sparse = plagiarism_with_n_major_matches(2);
    let report_b =
        compile_report(&outcome, &validation, None, Some(&sparse), Some(&ex), TEST_YEAR, vec![], &[], &[]);
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
        payload_has_signal(&payload_b, "citation_density"),
        "with room to spare the Minor extraction-derived finding MUST reach the reviewer: {:?}",
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
    let report = compile_report(&outcome, &validation, None, None, Some(&ex), TEST_YEAR, vec![], &[], &[]);

    let major = report
        .findings
        .iter()
        .position(|f| f.severity == FindingSeverity::Major)
        .expect("the soft concern must compile to a Major finding");
    // Was `signal:tables`; that finding is withdrawn (§11 D167). Any
    // extraction-derived Minor serves — the property is severity-before-
    // insertion-order, not which family sits below the Major.
    let table = report
        .findings
        .iter()
        .position(|f| f.provenance.iter().any(|p| p == "signal:citation_density"))
        .expect("an extraction-derived Minor finding must be present");
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
// Fix C — what a deterministic rule's finding may claim
// ===========================================================================

/// **WIRING ONLY — each validation finding reads its label from
/// `vocabulary::rule_certainty_label`, and keeps its tier. §11 D217, D219.**
///
/// This test says nothing about the WORDS; each rule's wording is pinned at its
/// source, in `vocabulary.rs`. Reverting a wording there (and regenerating the
/// mirror) must not redden this test; reverting the flags loop to the tier's
/// label must. The tier assertion pins that D217 moved only the claim.
#[test]
fn validation_findings_read_their_label_from_the_vocabulary() {
    use crate::validate::{Flag, RuleId, Severity, StatsValidityReport};
    let rules = [
        RuleId::TestGroupMismatch,
        RuleId::PValueOverclaim,
        RuleId::MissingEffectSize,
        RuleId::MissingConfidenceInterval,
    ];
    let validation = StatsValidityReport {
        passed: false,
        checks: vec![],
        flags: rules
            .iter()
            .enumerate()
            .map(|(i, r)| Flag {
                rule: *r,
                severity: Severity::Major,
                location: Location::in_section(SectionKind::Results, 0, i),
                explanation: format!("e{i}"),
            })
            .collect(),
    };
    let report =
        compile_report(&minimal_outcome(), &validation, None, None, None, TEST_YEAR, vec![], &[], &[]);
    for r in rules {
        let tag = format!("rule:{r:?} ");
        let f = report
            .findings
            .iter()
            .find(|f| f.provenance.iter().any(|p| p.starts_with(&tag)))
            .unwrap_or_else(|| panic!("no finding for {r:?}"));
        assert_eq!(f.certainty_label, crate::vocabulary::rule_certainty_label(r), "{r:?}");
        assert_eq!(f.tier, CertaintyTier::MathematicallyCertain, "{r:?}: the tier is unchanged");
    }
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
        &[],
    );

    // **`location` PLUS `also_at`, because grouping is what makes this test
    // load-bearing.** §11 D180 merges rows a reader cannot tell apart, so the
    // three `MissingEffectSize` flags here become ONE finding. Collecting only
    // `f.location` would then compare 1 against 3 — and "fix" it by asserting
    // the smaller set, which would silently license losing two locations. The
    // invariant is that EVERY flag's location still reaches a finding; where it
    // lands is the grouping's business.
    let mut from_findings: Vec<Location> = report
        .findings
        .iter()
        .flat_map(|f| f.location.clone().into_iter().chain(f.also_at.iter().cloned()))
        .collect();
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
        let loc = Location { section, paragraph, section_index: None };
        let text = crate::extract::paragraph_at(&ex, &loc)
            .unwrap_or_else(|| panic!("{loc:?} must resolve"));
        assert!(text.contains(expected), "{loc:?} resolved to {text:?}, missing {expected:?}");
        assert!(
            !text.contains(forbidden),
            "{loc:?} resolved to {text:?}, which is another paragraph's text"
        );
    }
}

/// **Rows a reader cannot tell apart become ONE row, and no location is lost.**
/// §11 D180.
///
/// Measured on a real manuscript through the product path: 21 findings of which
/// the first FOURTEEN were three sentences repeated, and the one finding unique
/// to that paper ranked 15th. `ReportViewerPage` renders `findings.map` with no
/// dedupe, so that is what a researcher opened.
///
/// The golden fixture cannot guard this: all five of its findings are distinct,
/// so nothing groups there and `skip_serializing_if` keeps `also_at` off the
/// wire entirely. That is correct behaviour and it is why this test exists.
#[test]
fn identical_rows_group_into_one_carrying_every_location() {
    let ex = crate::extract::extract_from_text(TWO_LOCATED_PARAGRAPHS);
    let validation = crate::validate::validate(&ex);
    let report = compile_report(
        &minimal_outcome(), &validation, None, None, None, TEST_YEAR, vec![], &[], &[],
    );

    let effect: Vec<&Finding> = report
        .findings
        .iter()
        .filter(|f| f.title.starts_with("statistical rule failed: missing effect size"))
        .collect();
    assert_eq!(effect.len(), 1, "the repeats must be ONE row: {:?}", effect);

    let f = effect[0];
    let places = 1 + f.also_at.len();
    assert!(places >= 2, "fixture must produce a repeat to group: {f:?}");
    assert!(
        f.title.ends_with(&format!("(raised at {places} places)")),
        "the count must be in the title a reader reads: {:?}",
        f.title
    );
    // **No "all are quoted" at THIS layer.** It holds locations; they become
    // quotations in `report_build` and not all resolve. Claiming otherwise is
    // the truncated-span defect from the other side.
    assert!(!f.title.contains("all are quoted"), "{:?}", f.title);

    // The wire carries the extra locations; an UNGROUPED row carries no key at
    // all, which is what keeps stored reports byte-identical.
    let json = serde_json::to_string(f).expect("serialise");
    assert!(json.contains("\"also_at\""), "a grouped row must carry its locations: {json}");
    let solo = report
        .findings
        .iter()
        .find(|x| x.also_at.is_empty())
        .expect("an ungrouped finding");
    let solo_json = serde_json::to_string(solo).expect("serialise");
    assert!(
        !solo_json.contains("also_at"),
        "an ungrouped row must not gain a key: {solo_json}"
    );
}

/// **The plural is the whole bug, and the sentence is from the corpus.** §11 D181.
///
/// `Revised Health Economics Paper FINAL (1).docx` writes exactly this and was
/// told to add a conflict-of-interest declaration, because the shipping check
/// was `text_lower.contains("conflict of interest")` — singular. `synonyms_for`
/// lists the plural and `statement_in_text` returns the sentence; both existed,
/// both were called only from `checklist_from_requirements`, which has no
/// production caller.
#[test]
fn a_plural_conflicts_declaration_is_found_and_quoted() {
    const DECLARED: &str = "Methods\n\nWe surveyed 222 firms.\n\nConflicts of Interest: \
        The authors declare no conflicts of interest.\n";
    let ex = crate::extract::extract_from_text(DECLARED);
    // The journal sentence must state the REQUIREMENT, not merely contain the
    // word: the trigger fired on PLOS via a sample-reference title, "Amino acid
    // metabolism conflicts with protein diversity."
    let g = guideline_hit("Competing interests must be declared by all authors.");
    let items = checklist_from_guidelines(&ex, DECLARED, &[g]);

    let coi = items
        .iter()
        .find(|i| i.requirement == "conflict-of-interest declaration")
        .expect("the requirement sentence must raise the item");
    assert!(coi.passed, "the declaration IS in the manuscript: {:?}", coi.detail);
    assert!(
        coi.detail.contains("declare no conflicts of interest"),
        "a 'found' row must quote the sentence, or it cannot be checked: {:?}",
        coi.detail
    );

    // The negative control: without the statement the row must still FAIL, or
    // this test is satisfied by a check that passes on everything.
    const ABSENT: &str = "Methods\n\nWe surveyed 222 firms.\n\nResults\n\nProvision rose.\n";
    let ex2 = crate::extract::extract_from_text(ABSENT);
    let g2 = guideline_hit("Competing interests must be declared by all authors.");
    let missing = checklist_from_guidelines(&ex2, ABSENT, &[g2]);
    let coi2 = missing
        .iter()
        .find(|i| i.requirement == "conflict-of-interest declaration")
        .expect("item still raised");
    assert!(!coi2.passed, "a manuscript with no declaration must FAIL");
}

/// **"structured abstract" is not "an abstract exists".** §11 D181.
///
/// `passed: true` on any Abstract section claimed the structure had been
/// verified; nothing here parses headed subsections. 6 of 20 corpus manuscripts
/// would have taken that PASS. The third state is the honest one.
#[test]
fn a_structured_abstract_item_is_undecided_not_passed() {
    const WITH: &str = "Abstract\n\nWe surveyed firms and provision rose.\n\nMethods\n\nA survey.\n";
    let ex = crate::extract::extract_from_text(WITH);
    let g = guideline_hit("Authors must supply structured abstracts.");
    let items = checklist_from_guidelines(&ex, WITH, &[g]);
    let it = items.iter().find(|i| i.requirement == "structured abstract").expect("raised");
    assert!(it.unevaluable, "presence is not structure: {:?}", it.detail);
    assert!(!it.passed, "an undecided item must not read as a pass: {it:?}");

    // A MISSING abstract is still decidable, and still a real failure.
    const WITHOUT: &str = "Methods\n\nA survey.\n\nResults\n\nProvision rose.\n";
    let ex2 = crate::extract::extract_from_text(WITHOUT);
    let g2 = guideline_hit("Authors must supply structured abstracts.");
    let items2 = checklist_from_guidelines(&ex2, WITHOUT, &[g2]);
    let it2 = items2.iter().find(|i| i.requirement == "structured abstract").expect("raised");
    assert!(!it2.unevaluable && !it2.passed, "a missing abstract is a decided FAIL: {it2:?}");
}

/// A `SectionRequired` requirement naming a statement, as the crawler stores it.
fn stmt_req(value: &str) -> crate::journal_store::StoredRequirement {
    crate::journal_store::StoredRequirement {
        kind: crate::journal_extract::RequirementKind::SectionRequired,
        value: value.into(),
        article_type: None,
        status: "VERIFIED".into(),
        source_url: "https://example.test/authors".into(),
        source_heading: "Required statements".into(),
        source_span: "Manuscripts must include this statement.".into(),
        conflict_id: None,
    }
}

/// **The earliest mention of a topic in a thesis is its TABLE OF CONTENTS.**
/// §11 D182.
///
/// `statement_in_text` took `.min()` over the needle positions, so it returned
/// the first mention rather than a declaration. Measured over 20 manuscripts it
/// was right about funding 1 time in 6. Every string here is from the corpus.
#[test]
fn a_contents_entry_is_not_a_funding_statement() {
    // The TOC line appears BEFORE the real statement, which is the whole bug:
    // `.min()` preferred it by position.
    const M: &str = "Contents\n\n7  Funding and Financial Constraints\t45\n\n        Methods\n\nWe surveyed 222 firms.\n\n        Funding: Ministry of Health, Oman (Grant MOH/CSR/24/29387).\n";
    let ex = crate::extract::extract_from_text(M);
    let reqs = [stmt_req("funding statement")];
    let items = crate::report::checklist_from_requirements(&ex, M, 40, &reqs, &[]);
    let f = items
        .iter()
        .find(|i| i.requirement.contains("funding"))
        .expect("the requirement raises the item");
    assert!(f.passed, "the manuscript HAS a funding statement: {:?}", f.detail);
    assert!(
        f.detail.contains("Ministry of Health"),
        "it must quote the declaration, not the contents entry: {:?}",
        f.detail
    );
    assert!(
        !f.detail.contains("Financial Constraints"),
        "the contents entry must not be quoted as the statement: {:?}",
        f.detail
    );

    // **The two guards catch DIFFERENT things, and the first version of this
    // test only exercised one.** Deleting the position rule left it GREEN,
    // because the contents line above is rejected for ENDING IN A PAGE NUMBER.
    // This is the case only the position rule catches: prose where the statement
    // word sits deep inside a sentence that is not TOC-shaped. From the corpus:
    // "Under ODOP, each of UP's 75 districts was designated a unique traditional
    // product for promotion, branding, marketing and funding support."
    const PROSE: &str = "Methods\n\nWe surveyed firms.\n\n        Under ODOP, each of the districts was designated a unique traditional product \
        for promotion, branding, marketing and funding support.\n";
    let ex2 = crate::extract::extract_from_text(PROSE);
    let items2 = crate::report::checklist_from_requirements(&ex2, PROSE, 40, &reqs, &[]);
    let f2 = items2.iter().find(|i| i.requirement.contains("funding")).expect("raised");
    assert!(
        !f2.passed,
        "a sentence merely mentioning funding is not a funding statement: {:?}",
        f2.detail
    );
}

/// **A statement name inside a reference TITLE is not a declaration.** §11 D182.
/// From the corpus: *"The ethics of ChatGPT: Exploring the ethical issues of an
/// emerging technology."* satisfied an ethics-statement requirement.
#[test]
fn a_reference_title_is_not_an_ethics_statement() {
    const M: &str = "Methods\n\nWe surveyed 222 firms.\n\n        References\n\nThe ethics of ChatGPT: Exploring the ethical issues of an         emerging technology.\n";
    let ex = crate::extract::extract_from_text(M);
    let reqs = [stmt_req("ethics statement")];
    let items = crate::report::checklist_from_requirements(&ex, M, 40, &reqs, &[]);
    let e = items
        .iter()
        .find(|i| i.requirement.contains("ethics"))
        .expect("the requirement raises the item");
    assert!(!e.passed, "a citation is not the author's declaration: {:?}", e.detail);

    // The negative control: the SAME sentence in the body is still found, so
    // this test cannot pass by the check having stopped working.
    const BODY: &str = "Methods\n\nWe surveyed 222 firms.\n\n        Ethical Approval: Ministry of Health, Oman.\n";
    let ex2 = crate::extract::extract_from_text(BODY);
    let items2 = crate::report::checklist_from_requirements(&ex2, BODY, 40, &reqs, &[]);
    let e2 = items2.iter().find(|i| i.requirement.contains("ethics")).expect("raised");
    assert!(e2.passed, "a real declaration must still be found: {:?}", e2.detail);
}

/// **A row that judged nothing must not render as a pass.** §11 D182.
/// `design_independent` removes the standard rows for the pipeline, but any
/// caller passing bindings still sees these, and green on
/// *"CHEERS applies to an economic evaluation … Gaply has no evaluator"* is
/// `Unevaluable` rendered as `Met` for the third time in this log.
#[test]
fn a_standard_binding_row_is_undecided_not_passed() {
    const M: &str = "Methods\n\nWe surveyed 222 firms.\n";
    let ex = crate::extract::extract_from_text(M);
    let b = crate::journal_standards::StandardBinding {
        standard: crate::journal_standards::Standard::Cheers,
        design: "economic evaluation".into(),
        source_span: "Economic evaluations must follow CHEERS.".into(),
    };
    let items = crate::report::checklist_from_requirements(&ex, M, 40, &[], &[b]);
    let row = items
        .iter()
        .find(|i| i.requirement.contains("applies to"))
        .expect("the binding raises a row");
    assert!(!row.passed, "it judged nothing: {:?}", row.detail);
    assert!(row.unevaluable, "and undecided is the state the type carries");

    // And the pipeline's filter removes it entirely.
    let kept = crate::report::design_independent(items);
    assert!(
        !kept.iter().any(|i| i.requirement.contains("applies to")),
        "design-dependent rows must not reach the design-independent checklist"
    );
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
        crate::extract::paragraph_at(&ex, &Location { section: SectionKind::Results, paragraph: 99, section_index: None }),
        None,
        "a paragraph index past the end must not resolve"
    );
    assert_eq!(
        crate::extract::paragraph_at(&ex, &Location { section: SectionKind::Discussion, paragraph: 0, section_index: None }),
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
        &minimal_outcome(), &validation, Some(&verdicts), None, None, TEST_YEAR, vec![], &reg, &[]);

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
        &minimal_outcome(), &validation, Some(&verdicts), None, None, TEST_YEAR, vec![], &[], &[]);
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
    let report = compile_report(&outcome, &validation, None, None, None, TEST_YEAR, vec![], &[], &[]);

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
    let report = compile_report(&outcome, &validation, None, None, None, TEST_YEAR, vec![], &[], &[]);
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
    let report = compile_report(&outcome, &validation, None, None, None, TEST_YEAR, vec![], &[], &[]);

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
    let report = compile_report(&outcome, &validation, None, None, None, TEST_YEAR, vec![], &[], &[]);
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
    let report = compile_report(&outcome, &validation, None, None, None, TEST_YEAR, vec![], &[], &[]);

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
        compile_report(&outcome, &validation, None, None, Some(&ex), TEST_YEAR, vec![], &[], &[]);

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
    let report = compile_report(&outcome, &validation, None, None, None, TEST_YEAR, vec![], &[], &[]);
    let f = report.findings.iter().find(|f| f.claim == ClaimKind::AuthorshipSignal).unwrap();
    assert_eq!(f.severity, FindingSeverity::Info);
}

// --- the checklist must not upgrade the journal's own modality -------------
//
// Nature Medicine's real pages carry both forms. "Observational studies …
// must be reported according to the STROBE statement" is a requirement;
// "We recommend following the ARRIVE 2.0 reporting guidelines" is not.
// Rendering both as "requires" states something the journal did not.
#[test]
fn a_recommended_standard_is_not_reported_as_required() {
    use crate::journal_standards::{Standard, StandardBinding};

    let bindings = vec![
        StandardBinding {
            standard: Standard::Strobe,
            design: "observational study".into(),
            source_span: "Observational studies (cohort, case-control or cross-sectional \
                          designs) must be reported according to the STROBE statement."
                .into(),
        },
        StandardBinding {
            standard: Standard::Arrive,
            design: "animal study".into(),
            source_span: "We recommend following the ARRIVE 2.0 reporting guidelines when \
                          documenting animal studies"
                .into(),
        },
    ];
    let ex = ExtractionResult::default();
    let items = crate::report::checklist_from_requirements(&ex, "", 1000, &[], &bindings);

    let strobe = items.iter().find(|i| i.requirement.starts_with("STROBE")).unwrap();
    assert!(strobe.detail.contains("requires STROBE"), "{}", strobe.detail);

    let arrive = items.iter().find(|i| i.requirement.starts_with("ARRIVE")).unwrap();
    assert!(
        arrive.detail.contains("recommends ARRIVE"),
        "a recommendation was reported as a requirement: {}",
        arrive.detail
    );
    // And the article agrees with the design it introduces.
    assert!(arrive.requirement.contains("an animal study"), "{}", arrive.requirement);
    assert!(
        strobe.requirement.contains("an observational study"),
        "{}",
        strobe.requirement
    );
}

/// **Every item row carries the coverage fraction, in the row.**
///
/// The fraction used to live in one binding row's prose. A reader who scrolled
/// past it then saw "STROBE item 16a — passed" and had been told the manuscript
/// met STROBE, when 20 of its 22 items were never read. Restating the fraction
/// on every row is what makes that reading unavailable.
#[test]
fn every_standard_item_row_states_the_fraction_of_the_standard_checked() {
    use crate::journal_standards::{Standard, StandardBinding};

    let bindings = vec![StandardBinding {
        standard: Standard::Strobe,
        design: "cross-sectional study".into(),
        source_span: "Observational studies (cohort, case-control or cross-sectional designs) \
                      must be reported according to the STROBE statement."
            .into(),
    }];
    let ex = crate::extract::extract_from_text(
        "Abstract\n\nMethods: Cross-sectional employer survey (n = 222). Multivariable \
         logistic regression was used.\n\nResults\n\nProvision was 36.5% (95% CI: \
         30.2-43.1%).\n",
    );
    let items = crate::report::checklist_from_requirements(&ex, "", 1000, &[], &bindings);

    let rows: Vec<&crate::report::ChecklistItem> =
        items.iter().filter(|i| i.requirement.contains("STROBE item")).collect();
    assert_eq!(rows.len(), 5, "one row per item, not one row per standard");
    for r in &rows {
        assert!(
            r.requirement.contains("checks 2 of STROBE's 22 numbered items"),
            "the fraction must be in the ROW, not only in a header: {}",
            r.requirement
        );
        // The journal's own sentence travels with every item it justifies.
        assert!(
            r.source_span.as_deref().is_some_and(|s| s.contains("must be reported")),
            "every item must carry the binding sentence: {:?}",
            r.source_span
        );
    }

    // **`Unevaluable` must not render as passed.** Three of STROBE's five items
    // read only the declined layer; a tick on any of them is a compliance claim
    // nobody made.
    let passed: Vec<&str> = rows
        .iter()
        .filter(|r| r.passed)
        .map(|r| r.requirement.as_str())
        .collect();
    assert_eq!(passed.len(), 2, "only the two decidable items may pass: {passed:?}");
    for r in rows.iter().filter(|r| r.detail.contains("not checked")) {
        assert!(!r.passed, "an unevaluable item rendered as passed: {}", r.requirement);
    }

    // And a met row shows the manuscript sentence that decided it.
    let met = rows.iter().find(|r| r.passed && r.requirement.contains("16a")).unwrap();
    assert!(met.detail.contains("95% CI"), "a met row must show its evidence: {}", met.detail);
}

mod evaluator_tests {
    #![allow(clippy::module_inception)]
    //! **The manuscript text here is from the real corpus**, not invented to
    //! match: the Methods and Results sentences are `Revised Health Economics
    //! Paper FINAL (1).docx`'s own, which is the manuscript the probe runs on.

    use crate::extract;
    use crate::journal_standards::{items_for, Standard};
    use crate::report::{evaluate, ItemStatus};

    const REAL: &str = "Abstract\n\nMethods: Cross-sectional employer survey (n = 222) \
        conducted prior to full Dhamani enforcement, using stratified purposive sampling by \
        firm size and sector. Multivariable logistic regression and bootstrapped mediation \
        analysis were used.\n\nResults\n\nFormal provision was 36.5% overall (95% CI: \
        30.2-43.1%), ranging from 10.8% among micro-enterprises to 81.0% among large firms.\n";

    fn ex(text: &str) -> extract::ExtractionResult {
        extract::extract_from_text(text)
    }

    /// **The coverage number is EVALUABLE over PUBLISHED, not implemented over
    /// published.** STROBE ships 5 items and can decide 2; saying "5 of 22"
    /// would overstate the examination by 2.5x, and it is the number a reader
    /// uses to decide how much a pass is worth.
    #[test]
    fn the_coverage_phrase_counts_what_can_be_decided_not_what_is_listed() {
        let e = evaluate(Standard::Strobe, &ex(REAL), REAL);
        assert_eq!(e.items_implemented, 5, "five items are listed");
        assert_eq!(e.items_evaluable, 2, "two can actually be decided");
        assert_eq!(e.published_items.unwrap().numbered, 22);
        // **The unit is named, and the mismatch is declared rather than hidden.**
        // STROBE's sub-item total is not recorded here, and Gaply's items
        // include sub-items (`6a`, `12a`, `16a`), so the denominator on offer is
        // the wrong unit — the phrase says so instead of inventing a number.
        let phrase = e.coverage_phrase();
        assert_eq!(
            phrase,
            "checks 2 of STROBE's 22 numbered items — Gaply's items include sub-items, so the true fraction is smaller"
        );
        assert!(
            !phrase.contains("of STROBE's 5"),
            "the listed count must not become the numerator: {phrase}"
        );
    }

    /// Per standard, so the ratio is visible as data rather than as prose.
    /// **Four of five standards can decide two items or fewer.**
    #[test]
    fn the_evaluable_fraction_is_small_and_stated_per_standard() {
        let expected = [
            (Standard::Consort, 4, 25),
            (Standard::Prisma, 2, 27),
            (Standard::Strobe, 2, 22),
            (Standard::Arrive, 2, 21),
            (Standard::Tripod, 1, 22),
        ];
        for (s, evaluable, published) in expected {
            let e = evaluate(s, &ex(REAL), REAL);
            assert_eq!(e.items_evaluable, evaluable, "{s:?} evaluable");
            assert_eq!(e.published_items.unwrap().numbered, published, "{s:?} numbered");
        }
    }

    /// **`Unevaluable` is not `NotFound`.** "We looked and it is missing" and
    /// "we cannot look" are opposite messages to a researcher, and collapsing
    /// them invents a compliance failure.
    #[test]
    fn an_item_reading_only_the_declined_layer_is_unevaluable_not_missing() {
        let e = evaluate(Standard::Strobe, &ex(REAL), REAL);
        let v = e.verdicts.iter().find(|v| v.item == "4").expect("STROBE 4");
        assert_eq!(v.status, ItemStatus::Unevaluable);
        assert_ne!(v.status, ItemStatus::NotFound, "absence of a check is not absence of evidence");
        assert!(v.detail.contains("D165"), "the reason must name the record: {}", v.detail);
        assert!(v.evidence_span.is_none(), "nothing to quote, so nothing is quoted");
    }

    /// A met item carries the paragraph it was decided from, whole.
    #[test]
    fn a_met_item_carries_its_manuscript_span() {
        let e = evaluate(Standard::Strobe, &ex(REAL), REAL);
        let v = e.verdicts.iter().find(|v| v.item == "16a").expect("STROBE 16a");
        assert_eq!(v.status, ItemStatus::Met, "a 95% CI is reported: {}", v.detail);
        let span = v.evidence_span.as_ref().expect("a met item must quote its evidence");
        assert!(span.contains("95% CI"), "the span must contain the evidence: {span}");
        assert!(
            span.len() > 60,
            "the span is the paragraph, not a fragment — a truncated span is not a span: {span}"
        );
    }

    /// The negative control for the pair above: with no interval reported, the
    /// same item is `NotFound` and quotes nothing.
    #[test]
    fn the_same_item_is_not_found_when_the_evidence_is_absent() {
        let bare = "Abstract\n\nWe surveyed firms.\n\nResults\n\nProvision was common.\n";
        let e = evaluate(Standard::Strobe, &ex(bare), bare);
        let v = e.verdicts.iter().find(|v| v.item == "16a").unwrap();
        assert_eq!(v.status, ItemStatus::NotFound);
        assert!(v.evidence_span.is_none(), "nothing found, so nothing quoted");
        assert_eq!(e.met(), 0, "and nothing is met on a manuscript carrying no statistics");
    }

    /// **No check may read the declined layer.** The guard, rather than the
    /// convention: an item added later with a `science.*` check would put a
    /// compliance verdict on a Turnitin page footer (§11 D165).
    #[test]
    fn no_implemented_check_sources_evidence_from_the_declined_layer() {
        for s in [
            Standard::Consort,
            Standard::Prisma,
            Standard::Strobe,
            Standard::Arrive,
            Standard::Tripod,
        ] {
            for it in items_for(s) {
                if it.check.is_evaluable() {
                    assert!(
                        it.reads.iter().any(|f| !f.starts_with("science.")),
                        "{s:?} item {} has a check but reads only {:?} — it would decide from \
                         the declined layer",
                        it.item,
                        it.reads
                    );
                }
            }
        }
    }
}

/// **A standard the journal did not bind is reported, and the two shapes differ.**
///
/// The spans are Nature Medicine's own, from the 15 Sep 2026 crawl: STARD is
/// named with mandatory force and binds nothing because "biomarkers" is not a
/// design phrase; STROBE binds normally on the same site.
mod unbound_standards {
    use crate::journal_extract::RequirementKind;
    use crate::journal_standards::{Standard, StandardBinding};
    use crate::journal_store::StoredRequirement;

    fn req(value: &str, span: &str) -> StoredRequirement {
        StoredRequirement {
            kind: RequirementKind::ReportingStandard,
            value: value.into(),
            article_type: None,
            status: "VERIFIED".into(),
            source_url: "https://www.nature.com/nm/editorial-policies/clinicalresearch".into(),
            source_heading: "Reporting guidelines".into(),
            source_span: span.into(),
            conflict_id: None,
        }
    }

    const STARD_SPAN: &str = "Studies reporting biomarkers in association with clinical \
                              outcomes must follow the STARD guidelines or relevant STARD \
                              extensions.";
    const STROBE_SPAN: &str = "Observational studies (cohort, case-control or cross-sectional \
                               designs) must be reported according to the STROBE statement.";

    fn run(reqs: &[StoredRequirement], bindings: &[StandardBinding]) -> Vec<crate::report::ChecklistItem> {
        let ex = crate::extract::extract_from_text("Abstract\n\nA paper.\n");
        crate::report::checklist_from_requirements(&ex, "", 1000, reqs, bindings)
    }

    /// Named, mandatory, and unroutable: reported with the journal's sentence so
    /// a reader can check whether we read it wrong.
    #[test]
    fn a_standard_named_without_a_design_is_reported_with_its_sentence() {
        let items = run(&[req("STARD", STARD_SPAN)], &[]);
        let row = items
            .iter()
            .find(|i| i.requirement.starts_with("STARD:"))
            .expect("STARD must be reported");
        assert!(row.requirement.contains("no study design stated"), "{}", row.requirement);
        assert_eq!(
            row.source_span.as_deref(),
            Some(STARD_SPAN),
            "the journal's own sentence must travel with the finding"
        );
        assert!(row.detail.contains("we read it wrong"), "the finding must invite correction");
        // It is NOT a compliance failure against the manuscript.
        // **This asserted `passed` and that was the claim §11 D182 withdrew.**
        // The rationale — "a statement about the journal must not mark the
        // manuscript down" — is right, and `passed: true` was the wrong half of
        // it: green reads as "my manuscript satisfies this" on a row that
        // judged nothing. `unevaluable` marks nothing down AND claims nothing.
        assert!(!row.passed, "a row that judged nothing must not render as a pass");
        assert!(row.unevaluable, "it is undecided, which is a state the type carries");
    }

    /// Never named, and we could have checked it: a fact about the journal,
    /// with the page count that makes it checkable.
    #[test]
    fn a_standard_never_named_is_reported_as_not_required_with_the_page_count() {
        let items = run(&[req("STROBE", STROBE_SPAN)], &[]);
        let row = items
            .iter()
            .find(|i| i.requirement.starts_with("CONSORT:"))
            .expect("CONSORT must be reported as not required");
        assert!(row.requirement.contains("not required by this journal"), "{}", row.requirement);
        assert!(row.detail.contains("1 page(s) we read"), "{}", row.detail);
        assert!(row.source_span.is_none(), "there is no sentence to quote for an absence");
    }

    /// **The negative control.** A BOUND standard must produce no such row, or
    /// the finding fires on every journal and means nothing — the filter that
    /// catches everything.
    #[test]
    fn a_bound_standard_produces_no_absence_finding() {
        let bindings = vec![StandardBinding {
            standard: Standard::Strobe,
            design: "cross-sectional study".into(),
            source_span: STROBE_SPAN.into(),
        }];
        let items = run(&[req("STROBE", STROBE_SPAN)], &bindings);
        assert!(
            !items.iter().any(|i| i.requirement.starts_with("STROBE:")),
            "STROBE is bound; an absence finding for it would be false"
        );
        // And the others still report, so the suppression is specific.
        assert!(items.iter().any(|i| i.requirement.starts_with("CONSORT:")));
    }

    /// A standard with no evaluator and no mention is NOT reported. Saying
    /// "this journal does not require CHEERS" when nothing could have checked
    /// CHEERS is clutter, not information.
    #[test]
    fn a_standard_with_no_evaluator_and_no_mention_is_not_reported() {
        let items = run(&[req("STROBE", STROBE_SPAN)], &[]);
        assert!(!items.iter().any(|i| i.requirement.starts_with("CHEERS")), "CHEERS has no evaluator");
        assert!(!items.iter().any(|i| i.requirement.starts_with("SPIRIT")), "SPIRIT has no evaluator");
    }

    /// **With nothing read, "this journal does not require X" is a claim about
    /// an empty crawl wearing a claim about a journal.**
    #[test]
    fn nothing_is_claimed_when_no_page_was_read() {
        let items = run(&[], &[]);
        assert!(
            !items.iter().any(|i| i.requirement.contains("not required by this journal")),
            "an empty crawl must assert nothing about the journal"
        );
    }
}

/// **Three defects found by running node 2 against a real journal, pinned.**
///
/// Nature Medicine binds CONSORT to three designs through six sentences, and the
/// manuscript under test is a cross-sectional survey. Each assertion below
/// corresponds to a row that was wrong on the first real run.
#[test]
fn standard_item_rows_are_evaluated_once_carry_the_conditional_and_do_not_flag_the_undecided() {
    use crate::journal_standards::{Standard, StandardBinding};

    let span = "Randomized trials must conform to CONSORT 2025 guidelines.";
    let bindings: Vec<StandardBinding> = ["clinical trial", "randomised trial", "trial protocol"]
        .iter()
        .map(|d| StandardBinding {
            standard: Standard::Consort,
            design: (*d).into(),
            source_span: span.into(),
        })
        .collect();
    let ex = crate::extract::extract_from_text(
        "Abstract\n\nA cross-sectional employer survey (n = 222).\n\nResults\n\nProvision was \
         36.5% (95% CI: 30.2-43.1%).\n",
    );
    let items = crate::report::checklist_from_requirements(&ex, "", 1000, &[], &bindings);
    let rows: Vec<&crate::report::ChecklistItem> =
        items.iter().filter(|i| i.requirement.contains("CONSORT item")).collect();

    // 1. ONE evaluation per standard, not one per binding. Three bindings
    //    previously produced fifteen rows, ten of them identical.
    assert_eq!(rows.len(), 5, "CONSORT's five items, once: got {}", rows.len());

    // 2. Every item row carries the conditional, naming ALL three designs —
    //    whether the manuscript IS one of them is not knowable here.
    for r in &rows {
        for d in ["clinical trial", "randomised trial", "trial protocol"] {
            assert!(
                r.requirement.contains(d),
                "item row must name every design the standard binds: {}",
                r.requirement
            );
        }
        assert!(r.requirement.contains("if your study is"), "{}", r.requirement);
    }

    // 3. An undecidable item is UNEVALUABLE, not failed. `passed: false` alone
    //    renders as a flag against a manuscript that has done nothing wrong.
    let six_a = rows.iter().find(|r| r.requirement.contains("item 6a")).expect("CONSORT 6a");
    assert!(six_a.unevaluable, "6a reads only the declined layer, so it cannot be decided");
    assert!(!six_a.passed, "and it is not a pass either");

    let one_b = rows.iter().find(|r| r.requirement.contains("item 1b")).expect("CONSORT 1b");
    assert!(one_b.passed, "an Abstract is present");
    assert!(!one_b.unevaluable, "a decided item is never unevaluable");

    // The negative control for the flag: a decidable item with absent evidence
    // is failed AND evaluable — the state `unevaluable` must not swallow.
    let seventeen_a =
        rows.iter().find(|r| r.requirement.contains("item 17a")).expect("CONSORT 17a");
    assert!(!seventeen_a.passed, "no effect size is reported");
    assert!(
        !seventeen_a.unevaluable,
        "an effect-size check CAN be decided; calling it unevaluable would hide a real gap"
    );
}

/// **A three-state value must be three states AT THE WIRE, and the test is
/// rendering the default rather than trusting it.**
///
/// `ChecklistItem` carries compliance in two bools: `passed` and `unevaluable`.
/// A stored report written before `unevaluable` existed has no such key, so
/// `serde(default)` supplies `false` — and an item that was never decided then
/// deserialises as `passed: false, unevaluable: false`, which every renderer
/// reads as **FAIL**. A missing value read as a stated one; the third time this
/// week.
///
/// This does not assert the default is `false` — that is trusting it. It
/// deserialises a payload with the key ABSENT and asserts what a renderer would
/// draw, which is the only form of this test that can fail.
#[test]
fn an_absent_unevaluable_key_renders_as_a_verdict_and_the_risk_is_named() {
    // A row as an older report stored it: no `unevaluable` key at all.
    let legacy = r#"{
        "requirement": "CONSORT item 6a: pre-specified outcome measures",
        "passed": false,
        "detail": "not checked",
        "guideline_source": null
    }"#;
    let item: crate::report::ChecklistItem =
        serde_json::from_str(legacy).expect("legacy rows must still parse");

    // What a renderer draws, computed the way a renderer computes it.
    fn render(i: &crate::report::ChecklistItem) -> &'static str {
        if i.unevaluable {
            "UNEVALUABLE"
        } else if i.passed {
            "OK"
        } else {
            "FAIL"
        }
    }
    assert_eq!(
        render(&item),
        "FAIL",
        "this is the DEFECT, pinned rather than hidden: an undecided item in a pre-`unevaluable` \
         report renders as a compliance failure, because two bools cannot carry three states \
         across a wire where one of them may be absent"
    );

    // The containment: no such report exists. `unevaluable` shipped WITH the
    // evaluator, so every stored report predating the key also predates any
    // item that could be undecided. If an evaluator is ever added whose items
    // can be undecided while this type still carries two bools, that is the
    // moment this becomes live — and this test is where it is written down.
    let current = serde_json::to_string(&crate::report::ChecklistItem {
        also_from: Vec::new(),
        requirement: "x".into(),
        passed: false,
        detail: "not checked".into(),
        guideline_source: None,
        source_span: None,
        article_type: None,
        checked_field: None,
        unevaluable: true,
    })
    .unwrap();
    assert!(
        current.contains("\"unevaluable\":true"),
        "a TRUE flag must always reach the wire, or the state is unrecoverable: {current}"
    );
    let round: crate::report::ChecklistItem = serde_json::from_str(&current).unwrap();
    assert_eq!(render(&round), "UNEVALUABLE", "and it must survive the round trip");
}


/// **A required statement is decided from the FULL TEXT, not from a heading.**
///
/// Measured against Nature Medicine and `Revised Health Economics Paper
/// FINAL (1).docx`: the heading check produced FOUR false compliance failures
/// on one real manuscript, because the paper writes its statements as inline
/// run-in labels. The strings below are that manuscript's own words.
///
/// §4.5 [v8] is the rule this enforces: `extraction.sections` is a MEDIATED
/// field, and a compliance failure may not be concluded from one being empty.
#[test]
fn a_required_statement_written_as_a_run_in_label_is_found() {
    use crate::journal_extract::RequirementKind;
    use crate::journal_store::StoredRequirement;

    // The manuscript's own words, verbatim. Not one of them is a heading.
    let text = "Results\n\nThe estimate was 36.5%.\n\nEthical Approval: Ministry of \
                Health, Oman (Grant MOH/CSR/24/29387). Conducted in accordance with the \
                Declaration of Helsinki. Participant consent obtained prior to \
                interview.\nData Availability: Available from the corresponding author on \
                reasonable request.\nConflicts of Interest: The authors declare no \
                conflicts of interest.\nFunding: Ministry of Health, Oman (Grant \
                MOH/CSR/24/29387).\n";
    let ex = crate::extract::extract_from_text(text);
    assert!(
        !ex.sections.iter().any(|s| s.heading.to_lowercase().contains("data availability")),
        "precondition: the classifier produces no such HEADING — which is why the old \
         check failed"
    );

    let req = |kind: RequirementKind, value: &str| StoredRequirement {
        kind,
        value: value.to_string(),
        article_type: None,
        status: "verified".into(),
        source_url: "https://www.nature.com/nm/for-authors".into(),
        source_heading: "Preparing your submission".into(),
        source_span: "all submissions must include a data availability statement".into(),
        conflict_id: None,
    };
    let reqs = vec![
        req(RequirementKind::DataPolicy, "data sharing required"),
        req(RequirementKind::SectionRequired, "competing interests statement"),
        req(RequirementKind::SectionRequired, "funding statement"),
    ];

    let items = crate::report::checklist_from_requirements(&ex, text, 1000, &reqs, &[]);

    let da = items.iter().find(|i| i.requirement == "data availability statement").unwrap();
    assert!(da.passed, "{da:?}");
    assert!(
        da.detail.contains("Available from the corresponding author"),
        "the item must quote the manuscript's own sentence, not assert: {}",
        da.detail
    );
    assert_eq!(da.checked_field.as_deref(), Some("manuscript full text"));

    // **The synonym case.** The journal says "competing interests"; the
    // manuscript says "Conflicts of Interest". Checking only the journal's word
    // reports a missing statement that is on the page.
    let ci = items.iter().find(|i| i.requirement == "competing interests statement").unwrap();
    assert!(ci.passed, "{ci:?}");
    assert!(ci.detail.contains("declare no conflicts of interest"), "{}", ci.detail);

    let f = items.iter().find(|i| i.requirement == "funding statement").unwrap();
    assert!(f.passed, "{f:?}");
}

/// The other direction: a manuscript that really has no such statement is still
/// flagged, and the item says how many phrasings were searched.
#[test]
fn a_statement_that_is_genuinely_absent_is_still_flagged_with_its_lexicon() {
    use crate::journal_extract::RequirementKind;
    use crate::journal_store::StoredRequirement;
    let text = "Results\n\nThe estimate was 36.5%.\n";
    let ex = crate::extract::extract_from_text(text);
    let reqs = vec![StoredRequirement {
        kind: RequirementKind::DataPolicy,
        value: "data sharing required".into(),
        article_type: None,
        status: "verified".into(),
        source_url: "u".into(),
        source_heading: "h".into(),
        source_span: "must include a data availability statement".into(),
        conflict_id: None,
    }];
    let items = crate::report::checklist_from_requirements(&ex, text, 1000, &reqs, &[]);
    let da = items.iter().find(|i| i.requirement == "data availability statement").unwrap();
    assert!(!da.passed);
    assert!(da.detail.contains("phrasings was found"), "{}", da.detail);
}


/// **A heading the classifier does not know is not a missing section.**
///
/// Measured over 20 real manuscripts: 29 section-absence verdicts, 11 of them
/// contradicted by the manuscript's own text. Every string below is one of
/// those manuscripts' own heading lines, and each became a MAJOR concern in the
/// reviewer report.
#[test]
fn a_heading_the_classifier_does_not_know_is_not_a_missing_section() {
    use crate::journal_standards::Standard;
    use crate::report::ItemStatus;

    // "IV. EXPERIMENTS AND RESULTS" USED TO BE IN THIS LIST and is now handled
    // by the classifier itself (§11 A2, 22 Sep 2026): a heading naming Results
    // alongside something else is now a Results heading, so the precondition
    // below — that the classifier misses it — is no longer true of it. It moved
    // to `a_compound_heading_is_now_found_by_the_classifier_not_the_fallback`,
    // which asserts the section exists rather than that the fallback rescued it.
    //
    // This fallback is NOT obsolete: "4.8 INTERPRETATION OF RESULTS" names one
    // section and a preposition, so no conjunction split reaches it and the
    // classifier still misses it. The 20-manuscript measurement in this doc
    // comment is what the fallback is for, and it still is.
    for (heading, body) in [
        ("4.8 INTERPRETATION OF RESULTS", "Provision rose with firm size."),
        ("5 DISCUSSION OF THE RESULTS", "The trend held across both cohorts."),
    ] {
        let text = format!("A Title\n\n1. Background\n\nSome prose.\n\n{heading}\n{body}\n");
        let ex = crate::extract::extract_from_text(&text);
        assert!(
            !ex.sections.iter().any(|s| s.kind == crate::extract::SectionKind::Results),
            "precondition: the classifier misses {heading:?}"
        );
        let e = crate::report::evaluate(Standard::Prisma, &ex, &text);
        let v = e.verdicts.iter().find(|v| v.item == "16a").expect("PRISMA 16a");
        assert_eq!(
            v.status,
            ItemStatus::Met,
            "{heading:?} is a Results heading: {}",
            v.detail
        );
        assert!(
            v.detail.contains("fact about the extractor"),
            "and the row says whose fact it is: {}",
            v.detail
        );
    }
}

/// **A compound heading is now found by the CLASSIFIER, not by the fallback.**
///
/// `"IV. EXPERIMENTS AND RESULTS"` used to reach PRISMA 16a through
/// `report`'s heading-shaped fallback scan, whose detail says the absence is
/// "a fact about the extractor". After §11 A2 the extractor finds it, so the
/// row is Met because the manuscript HAS a Results section — a stronger and
/// more honest basis for the same verdict.
///
/// Both halves are asserted, because the point is which mechanism answered.
#[test]
fn a_compound_heading_is_now_found_by_the_classifier_not_the_fallback() {
    use crate::journal_standards::Standard;
    use crate::report::ItemStatus;
    let text = "A Title\n\n1. Background\n\nSome prose.\n\nIV. EXPERIMENTS AND RESULTS\n\
                Accuracy reached 96.42% on the combined set.\n";
    let ex = crate::extract::extract_from_text(text);
    assert!(
        ex.sections.iter().any(|s| s.kind == crate::extract::SectionKind::Results),
        "the classifier must now find a compound Results heading"
    );
    let e = crate::report::evaluate(Standard::Prisma, &ex, text);
    let v = e.verdicts.iter().find(|v| v.item == "16a").expect("PRISMA 16a");
    assert_eq!(v.status, ItemStatus::Met, "{}", v.detail);
    assert!(
        !v.detail.contains("fact about the extractor"),
        "answered by the fallback, not the classifier — the fix did not take: {}",
        v.detail
    );
}

/// The other direction: a manuscript with genuinely no Results section is still
/// `NotFound`, and the row says what it searched for.
#[test]
fn a_manuscript_with_no_results_heading_anywhere_is_still_not_found() {
    use crate::journal_standards::Standard;
    use crate::report::ItemStatus;
    let text = "A Title\n\n1. Background\n\nThis chapter reviews the literature on \
                sediment chemistry and says nothing about what was measured.\n";
    let ex = crate::extract::extract_from_text(text);
    let e = crate::report::evaluate(Standard::Prisma, &ex, text);
    let v = e.verdicts.iter().find(|v| v.item == "16a").expect("PRISMA 16a");
    assert_eq!(v.status, ItemStatus::NotFound, "{}", v.detail);
    assert!(v.detail.contains("no heading-shaped line matching"), "{}", v.detail);
}

/// **A sentence mentioning results is not a heading.** The fallback scan has to
/// be heading-SHAPED or it re-admits everything it was added to exclude.
#[test]
fn a_sentence_mentioning_results_is_not_a_heading() {
    use crate::journal_standards::Standard;
    use crate::report::ItemStatus;
    for prose in [
        "The results of the survey were inconclusive and are discussed below.",
        "Table 3 summarises our findings for each of the three lakes studied.",
    ] {
        let text = format!("A Title\n\n1. Background\n\n{prose}\n");
        let ex = crate::extract::extract_from_text(&text);
        let e = crate::report::evaluate(Standard::Prisma, &ex, &text);
        let v = e.verdicts.iter().find(|v| v.item == "16a").expect("PRISMA 16a");
        assert_eq!(v.status, ItemStatus::NotFound, "{prose:?} -> {}", v.detail);
    }
}


/// **§12.1's equation GAP, closed.** Equation findings shipped with
/// `location: None`, marked GAP at the construction site: the engine works over
/// lines and the OMML paragraph index does not compose with `Location`, whose
/// `paragraph` is an index WITHIN a section.
///
/// The fix is the one that has now worked three times in this crate — search
/// the text for the line rather than thread a second index through. The
/// manuscript below is `Revised Health Economics Paper FINAL (1).docx`'s real
/// defect in miniature: the products give 0.21968 where the next line writes
/// 0.219.
#[test]
fn an_equation_finding_is_anchored_to_the_paragraph_that_contains_it() {
    let text = "A Title\n\n1. Background\n\nSome prose about provision.\n\n\
                5. Results\n\nThe indirect effect was 0.413 * 0.5317 = 0.21968.\n";
    let ex = crate::extract::extract_from_text(text);
    let line = "The indirect effect was 0.413 * 0.5317 = 0.21968.";
    let at = crate::extract::locate_line(&ex, line).expect("the line is in exactly one paragraph");
    assert_eq!(at.section, crate::extract::SectionKind::Results);
    assert_eq!(crate::extract::paragraph_at(&ex, &at), Some(line));
}

/// **A line in two paragraphs anchors to neither.** Anchoring to the first
/// would be `paragraph_at`'s ambiguity defect, committed here rather than
/// inherited — and §9 [v5] already refused a reconstruction that looks subtly
/// wrong to the person who wrote the paper.
#[test]
fn a_line_appearing_twice_is_refused_rather_than_anchored_to_the_first() {
    let dup = "The conversion factor was 8000 mg per litre.";
    let text = format!("A Title\n\n1. Background\n\n{dup}\n\n5. Results\n\n{dup}\n");
    let ex = crate::extract::extract_from_text(&text);
    assert!(crate::extract::locate_line(&ex, dup).is_none());
}

/// A line too short to identify a place is refused. `n = x` occurs everywhere.
#[test]
fn a_line_shorter_than_the_floor_is_not_anchored() {
    let text = "A Title\n\n5. Results\n\nn = 5 and the rest of this paragraph.\n";
    let ex = crate::extract::extract_from_text(text);
    assert!(crate::extract::locate_line(&ex, "n = 5").is_none());
    assert!(crate::extract::locate_line(&ex, "   ").is_none());
}

// ---------------------------------------------------------------------------
// Article-type scoping: the first producer of `unevaluable` (§11 D188).
// Measurement: `docs/A3_APPLICABILITY_MEASUREMENT.md`.
// ---------------------------------------------------------------------------

/// A statement requirement the journal scoped to one article type.
fn stmt_req_for(value: &str, article_type: &str) -> crate::journal_store::StoredRequirement {
    crate::journal_store::StoredRequirement { article_type: Some(article_type.into()), ..stmt_req(value) }
}

/// **The row R PAPER got wrong: `author contributions statement`, harvested
/// from Nature Medicine's Matters Arising page, judged against a paper that is
/// not a Matters Arising piece.**
///
/// The manuscript genuinely has no such statement, so `passed: false` is a true
/// statement about the text. It is still the wrong row to show, because nobody
/// established that the requirement applies.
#[test]
fn a_requirement_scoped_to_an_article_type_is_not_decided() {
    let ex = crate::extract::extract_from_text("Title\n\nAbstract\nShort.\n\nMethods\nWe did work.\n");
    let items = checklist_from_requirements(
        &ex,
        "Title Abstract Short. Methods We did work.",
        7,
        &[stmt_req_for("author contributions statement", "Matters Arising")],
        &[],
    );
    let it = items
        .iter()
        .find(|i| i.requirement == "author contributions statement")
        .expect("the row is still emitted — scoping it does not drop the journal's requirement");
    assert!(it.unevaluable, "scoped to an article type nobody established: {it:?}");
    assert!(!it.passed, "and never a pass");
    assert!(
        it.detail.contains("which this analysis does not know"),
        "it must say WHY, in the wording the word-limit row already uses: {}",
        it.detail
    );
    assert!(
        it.detail.contains("Matters Arising"),
        "and name the type the journal scoped it to: {}",
        it.detail
    );
}

/// NEGATIVE CONTROL 1 — the journal stated it for everyone, so there is nothing
/// to scope and the row must still decide.
#[test]
fn a_requirement_with_no_article_type_is_still_decided() {
    let ex = crate::extract::extract_from_text("Title\n\nAbstract\nShort.\n\nMethods\nWe did work.\n");
    let items = checklist_from_requirements(
        &ex,
        "Title Abstract Short. Methods We did work.",
        7,
        &[stmt_req("competing interests statement")],
        &[],
    );
    let it = items
        .iter()
        .find(|i| i.requirement == "competing interests statement")
        .expect("raised");
    assert!(!it.unevaluable, "unscoped: nothing stops this being decided: {it:?}");
    assert!(!it.passed, "and the manuscript really does not declare one");
}

/// NEGATIVE CONTROL 2 — once the type IS known, a genuinely unmet requirement
/// must still fail.
///
/// Nothing establishes the manuscript's type today, so this exercises the rule
/// directly rather than inventing a detector to satisfy a test. The third case
/// is the one to watch: a known type that does NOT match the journal's scope
/// still returns `None`, because concluding NOT_APPLICABLE needs evidence this
/// rule does not have.
#[test]
fn a_known_article_type_decides_the_row_and_never_says_not_applicable() {
    assert_eq!(undecidable_for_article_type(None, None), None, "nothing to scope");
    assert_eq!(
        undecidable_for_article_type(Some("Article"), Some("Article")),
        None,
        "type known and matching: decide it"
    );
    assert_eq!(
        undecidable_for_article_type(Some("Brief Communication"), Some("Article")),
        None,
        "type known and DIFFERENT: still decided here — this rule never concludes \
         that a requirement does not apply"
    );
    assert!(
        undecidable_for_article_type(Some("Matters Arising"), None).is_some(),
        "scoped, and the manuscript's type is unknown: the only undecidable case"
    );
}

/// The abstract limit, which is R PAPER's other wrong row: 150 words is Nature
/// Medicine's Brief Communication format, and nobody said this is one.
#[test]
fn an_abstract_limit_scoped_to_an_article_type_is_not_decided() {
    let text = format!("Title\n\nAbstract\n{}\n\nMethods\nWe did work.\n", "word ".repeat(202));
    let ex = crate::extract::extract_from_text(&text);
    let req = crate::journal_store::StoredRequirement {
        kind: crate::journal_extract::RequirementKind::AbstractLimit,
        value: "150".into(),
        article_type: Some("Brief Communication".into()),
        status: "VERIFIED".into(),
        source_url: "https://example.test/content".into(),
        source_heading: "Format".into(),
        source_span: "Format Abstract – up to 150 words, unreferenced.".into(),
        conflict_id: None,
    };
    let items = checklist_from_requirements(&ex, &text, 210, &[req], &[]);
    let it = items.iter().find(|i| i.requirement.starts_with("abstract limit")).expect("raised");
    assert!(it.unevaluable, "{it:?}");
    assert!(
        it.detail.contains("202 words") && it.detail.contains("Brief Communication"),
        "the observation is kept beside the caveat, so the author still learns what was counted: {}",
        it.detail
    );
}

/// **Both renderers, on a row that until now no production path could produce.**
///
/// The third state must read as undecided in each — never as a pass, and never
/// as "not applicable", which would claim the requirement was ruled out.
#[test]
fn an_undecided_row_renders_as_undecided_in_the_shared_renderer() {
    let ex = crate::extract::extract_from_text("Title\n\nAbstract\nShort.\n\nMethods\nWe did work.\n");
    let items = checklist_from_requirements(
        &ex,
        "Title Abstract Short. Methods We did work.",
        7,
        &[stmt_req_for("author contributions statement", "Matters Arising")],
        &[],
    );
    let lines = crate::report_compose::checklist_lines(&items[0]);
    let first = &lines[0];
    assert!(first.contains("not decided"), "the shared renderer's third state: {first}");
    assert!(!first.contains("[met]"), "never a pass: {first}");
    assert!(
        !first.to_lowercase().contains("not applicable"),
        "undecided is not a ruling that the requirement does not apply: {first}"
    );
}
