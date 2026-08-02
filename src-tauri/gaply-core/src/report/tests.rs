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
use crate::verify_agent::{verify_citations, MockProxyClient};

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
    let ai = crate::ai_detect::detect_text(&model, MANUSCRIPT);
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

    let report = compile_report(&outcome, &validation, Some(&verification), Some(&plag), None, TEST_YEAR, checklist);

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
    assert!(by_req("word limit (3000 words)").passed);
    assert!(by_req("conflict-of-interest").passed);
    assert!(by_req("structured abstract").passed);
    // guideline-derived items carry the guideline's provenance URL
    assert_eq!(
        by_req("word limit").guideline_source.as_deref(),
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
    let report = compile_report(&outcome, &validation, None, None, None, TEST_YEAR, vec![]);

    // Every leading finding is the CRITICAL hard constraint tier, before any
    // 1.0-confidence soft finding.
    assert!(!report.findings.is_empty());
    let n_critical = validation.flags.len();
    for f in &report.findings[..n_critical] {
        assert_eq!(f.severity, FindingSeverity::Critical, "critical first: {:?}", f.title);
        assert_eq!(f.tier, CertaintyTier::MathematicallyCertain);
        assert_eq!(f.agent, AgentKind::ValidationMaths);
    }
    assert!(report.findings[n_critical..]
        .iter()
        .all(|f| f.severity != FindingSeverity::Critical));
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
    assert!(!by_req("word limit (10 words)").passed, "manuscript exceeds the tiny limit");
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
    let report = compile_report(&outcome, &validation, Some(&final_verification), None, None, TEST_YEAR, vec![]);

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
    let report = compile_report(&outcome, &validation, None, Some(&pr), None, TEST_YEAR, vec![]);

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
    let report = compile_report(&outcome, &validation, None, Some(&pr), None, TEST_YEAR, vec![]);
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
    let report = compile_report(&outcome, &validation, None, None, None, TEST_YEAR, vec![]);

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
    let without = compile_report(&outcome, &validation, None, None, None, TEST_YEAR, vec![]);
    let with = compile_report(&outcome, &validation, None, None, Some(&ex), TEST_YEAR, vec![]);
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
    let report = compile_report(&outcome, &validation, None, None, Some(&ex), TEST_YEAR, vec![]);
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
    let r2 = compile_report(&outcome, &v2, None, None, Some(&ex2), TEST_YEAR, vec![]);
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
    let report = compile_report(&outcome, &validation, None, None, Some(&ex), TEST_YEAR, vec![]);

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
    let r2 = compile_report(&outcome, &v2, None, None, Some(&ex2), TEST_YEAR, vec![]);
    assert_eq!(by_signal(&r2, "citation_recency")[0].severity, FindingSeverity::Info);

    // No reference list -> no finding (the structural checklist covers that).
    let ex3 = crate::extract::extract_from_text("T\n\nAbstract\nA.\n");
    let v3 = crate::validate::validate(&ex3);
    let r3 = compile_report(&outcome, &v3, None, None, Some(&ex3), TEST_YEAR, vec![]);
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
    let report = compile_report(&outcome, &validation, None, None, Some(&ex), TEST_YEAR, vec![]);

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
    let r2 = compile_report(&outcome, &v2, None, None, Some(&ex2), TEST_YEAR, vec![]);
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
    let report = compile_report(&outcome, &validation, None, None, Some(&ex), TEST_YEAR, vec![]);

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
        compile_report(&outcome, &validation, None, Some(&crowded), Some(&ex), TEST_YEAR, vec![]);

    // The finding EXISTS locally — this test is about the payload, not the report.
    assert_eq!(by_signal(&report_a, "tables").len(), 1, "the table finding is in the local report");
    assert!(
        report_a.findings.len() > REVIEWER_MAX_FINDINGS,
        "fixture must exceed the cap to test it, got {}",
        report_a.findings.len()
    );

    let json_a = serde_json::to_value(&report_a).unwrap();
    let (payload_a, sent_a) = build_review_payload(&json_a, &journal, &[], "run-a");
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
        compile_report(&outcome, &validation, None, Some(&sparse), Some(&ex), TEST_YEAR, vec![]);
    assert!(
        report_b.findings.len() <= REVIEWER_MAX_FINDINGS,
        "fixture must fit under the cap, got {}",
        report_b.findings.len()
    );

    let json_b = serde_json::to_value(&report_b).unwrap();
    let (payload_b, sent_b) = build_review_payload(&json_b, &journal, &[], "run-b");
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
    // An AiDetection CONCERN -> Major, compiled in the soft-opinion loop AFTER
    // the extraction-derived findings.
    let mut agents: Vec<Box<dyn SwarmAgent>> = vec![Box::new(PrecomputedAgent::new(opinion(
        AgentKind::AiDetection,
        crate::swarm::ANSWER_CONCERN,
        0.6,
    )))];
    let outcome = run_debate(&mut agents, &DebateConfig::default()).unwrap();
    let report = compile_report(&outcome, &validation, None, None, Some(&ex), TEST_YEAR, vec![]);

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
