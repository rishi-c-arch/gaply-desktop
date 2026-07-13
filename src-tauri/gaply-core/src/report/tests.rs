//! Report Compiler tests — offline; the only "network" is a MockProxyClient.

use serde_json::json;

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

    let report = compile_report(&outcome, &validation, Some(&verification), checklist);

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
    let report = compile_report(&outcome, &validation, None, vec![]);

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
    let report = compile_report(&outcome, &validation, Some(&final_verification), vec![]);

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
