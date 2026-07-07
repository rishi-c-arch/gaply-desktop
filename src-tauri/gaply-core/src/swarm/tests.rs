//! Swarm debate tests — fully offline; the only "network" is a MockProxyClient.

use super::*;
use crate::GaplyError;

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

/// Scripted agent: starts with `initial`, and from `align_round` on aligns with
/// the current majority answer (a real "persuaded in discussion" behaviour).
struct Persuadable {
    initial: Opinion,
    align_round: usize,
}

impl SwarmAgent for Persuadable {
    fn kind(&self) -> AgentKind {
        self.initial.agent
    }
    fn opine(&mut self) -> Result<Opinion, GaplyError> {
        Ok(self.initial.clone())
    }
    fn revise(&mut self, own: &Opinion, others: &[Opinion], round: usize) -> Option<Opinion> {
        if round < self.align_round {
            return None;
        }
        // majority answer among the others
        let mut counts: Vec<(&str, usize)> = Vec::new();
        for o in others {
            match counts.iter_mut().find(|(a, _)| *a == o.answer) {
                Some((_, n)) => *n += 1,
                None => counts.push((&o.answer, 1)),
            }
        }
        let majority = counts.iter().max_by_key(|(_, n)| *n)?.0.to_string();
        if majority == own.answer {
            return None; // already aligned -> no revision
        }
        let mut new = own.clone();
        new.answer = majority;
        new.explanation = format!("persuaded by majority in round {round}");
        Some(new)
    }
}

/// Scripted agent that revises EVERY round (never settles) — exercises the
/// forced-termination ceiling.
struct Oscillator {
    base: Opinion,
    flips: usize,
}

impl SwarmAgent for Oscillator {
    fn kind(&self) -> AgentKind {
        self.base.agent
    }
    fn opine(&mut self) -> Result<Opinion, GaplyError> {
        Ok(self.base.clone())
    }
    fn revise(&mut self, own: &Opinion, _others: &[Opinion], _round: usize) -> Option<Opinion> {
        self.flips += 1;
        let mut new = own.clone();
        new.answer = if own.answer == ANSWER_PASS { ANSWER_CONCERN.into() } else { ANSWER_PASS.into() };
        Some(new)
    }
}

fn boxed(agents: Vec<Box<dyn SwarmAgent>>) -> Vec<Box<dyn SwarmAgent>> {
    agents
}

// --- convergence within 3 rounds ---------------------------------------------

#[test]
fn debate_converges_within_three_rounds() {
    // Two settled "pass" agents + one dissenter that lets itself be persuaded
    // from round 2. Round 1: dissenter still holds -> wait, align_round=2 means
    // round 1 no change BY THE DISSENTER, and nobody else ever revises, so the
    // debate would converge in round 1. Use align_round=1: it aligns in round 1,
    // then round 2 has zero revisions -> converged, well within the ceiling.
    let mut agents = boxed(vec![
        Box::new(PrecomputedAgent::new(opinion(AgentKind::Extraction, ANSWER_PASS, 0.9))),
        Box::new(PrecomputedAgent::new(opinion(AgentKind::Plagiarism, ANSWER_PASS, 0.8))),
        Box::new(Persuadable {
            initial: opinion(AgentKind::AiDetection, ANSWER_CONCERN, 0.6),
            align_round: 1,
        }),
    ]);
    let outcome = run_debate(&mut agents, &DebateConfig::default()).unwrap();

    assert!(outcome.converged, "debate should converge");
    assert!(outcome.rounds_run <= MAX_ROUNDS, "within the ceiling");
    assert_eq!(outcome.result.answer, ANSWER_PASS);
    // the persuaded agent's final opinion reflects the discussion
    let ai = outcome.opinions.iter().find(|o| o.agent == AgentKind::AiDetection).unwrap();
    assert_eq!(ai.answer, ANSWER_PASS);
    assert!(ai.explanation.contains("persuaded"));
}

#[test]
fn debate_is_force_terminated_at_three_rounds() {
    let mut agents = boxed(vec![
        Box::new(PrecomputedAgent::new(opinion(AgentKind::Extraction, ANSWER_PASS, 0.9))),
        Box::new(Oscillator { base: opinion(AgentKind::AiDetection, ANSWER_PASS, 0.6), flips: 0 }),
    ]);
    let outcome = run_debate(&mut agents, &DebateConfig::default()).unwrap();
    assert_eq!(outcome.rounds_run, MAX_ROUNDS, "must stop at the ceiling");
    assert!(!outcome.converged, "an oscillator never converges");
}

#[test]
fn requested_rounds_are_clamped_to_the_ceiling() {
    let mut agents = boxed(vec![
        Box::new(PrecomputedAgent::new(opinion(AgentKind::Extraction, ANSWER_PASS, 0.9))),
        Box::new(Oscillator { base: opinion(AgentKind::AiDetection, ANSWER_PASS, 0.6), flips: 0 }),
    ]);
    let outcome = run_debate(&mut agents, &DebateConfig { max_rounds: 50 }).unwrap();
    assert_eq!(outcome.rounds_run, MAX_ROUNDS, "50 requested, 3 enforced");
}

// --- maths hard constraint overrides majority consensus -----------------------

#[test]
fn maths_verdict_overrides_majority_consensus() {
    // Four soft agents vote "pass" with high confidence. The deterministic
    // Validation agent says "concern" (a rule failed — mathematically certain).
    let mut validation = opinion(AgentKind::ValidationMaths, ANSWER_CONCERN, 1.0);
    validation.hard_constraint = true;

    let mut agents = boxed(vec![
        Box::new(PrecomputedAgent::new(opinion(AgentKind::Extraction, ANSWER_PASS, 0.95))),
        Box::new(PrecomputedAgent::new(opinion(AgentKind::AiDetection, ANSWER_PASS, 0.95))),
        Box::new(PrecomputedAgent::new(opinion(AgentKind::Plagiarism, ANSWER_PASS, 0.95))),
        Box::new(PrecomputedAgent::new(opinion(AgentKind::Rag, ANSWER_PASS, 0.95))),
        Box::new(PrecomputedAgent::new(validation)),
    ]);
    let outcome = run_debate(&mut agents, &DebateConfig::default()).unwrap();

    assert_eq!(outcome.result.answer, ANSWER_CONCERN, "hard constraint must win");
    assert!(outcome.result.overridden_by_constraint, "the soft majority was displaced");
    assert_eq!(outcome.result.combined_confidence, 1.0, "deterministic verdicts are certain");
    assert!(outcome.result.constraint_note.as_ref().unwrap().contains("hard constraint"));
    // the soft weights are still reported for transparency
    assert_eq!(outcome.result.weights.len(), 4);
}

#[test]
fn conflicting_hard_constraints_are_an_error() {
    let mut a = opinion(AgentKind::ValidationMaths, ANSWER_PASS, 1.0);
    a.hard_constraint = true;
    let mut b = opinion(AgentKind::ValidationMaths, ANSWER_CONCERN, 1.0);
    b.hard_constraint = true;
    let mut agents = boxed(vec![
        Box::new(PrecomputedAgent::new(a)),
        Box::new(PrecomputedAgent::new(b)),
    ]);
    let err = run_debate(&mut agents, &DebateConfig::default()).unwrap_err();
    assert!(matches!(err, GaplyError::Conflict(_)), "{err:?}");
}

// --- confidence rescaling ------------------------------------------------------

#[test]
fn confidence_rescaling_shrinks_overconfident_agents_toward_half() {
    // Same raw confidence, different calibration: heuristic/LLM agents keep less.
    let raw = 0.95;
    let det = rescale_confidence(AgentKind::ValidationMaths, raw);
    let mech = rescale_confidence(AgentKind::Extraction, raw);
    let emb = rescale_confidence(AgentKind::Plagiarism, raw);
    let heur = rescale_confidence(AgentKind::AiDetection, raw);
    let llm = rescale_confidence(AgentKind::Verification, raw);

    assert_eq!(det, raw, "deterministic agent passes through");
    assert!(mech < det && emb < mech && heur < emb, "shrinkage orders by calibration");
    assert_eq!(heur, llm, "AI-Detection and Verification share the strongest shrink");
    assert!((heur - (0.5 + 0.45 * 0.6)).abs() < 1e-12, "0.5 + (0.95-0.5)*0.6");

    // 0.5 is a fixed point for every agent kind.
    for kind in [
        AgentKind::Extraction,
        AgentKind::ValidationMaths,
        AgentKind::AiDetection,
        AgentKind::Plagiarism,
        AgentKind::Rag,
        AgentKind::Verification,
    ] {
        assert_eq!(rescale_confidence(kind, 0.5), 0.5);
    }

    // under-confidence is shrunk toward 0.5 symmetrically, and inputs clamp.
    assert!(rescale_confidence(AgentKind::AiDetection, 0.1) > 0.1);
    assert_eq!(rescale_confidence(AgentKind::AiDetection, 7.0), rescale_confidence(AgentKind::AiDetection, 1.0));
}

// --- gate filtering BEFORE the debate -------------------------------------------

#[test]
fn gate_failed_opinion_is_rejected_before_the_round_table() {
    let mut gated = opinion(AgentKind::Verification, ANSWER_CONCERN, 0.9);
    gated.gate_passed = false; // e.g. Prompt-17 harness flagged a hallucination
    let mut agents = boxed(vec![
        Box::new(PrecomputedAgent::new(opinion(AgentKind::Extraction, ANSWER_PASS, 0.9))),
        Box::new(PrecomputedAgent::new(gated)),
    ]);
    let outcome = run_debate(&mut agents, &DebateConfig::default()).unwrap();

    assert_eq!(outcome.rejected.len(), 1);
    assert_eq!(outcome.rejected[0].agent, AgentKind::Verification);
    assert_eq!(outcome.opinions.len(), 1, "rejected opinion never entered the debate");
    assert_eq!(outcome.result.answer, ANSWER_PASS);
    assert!(!outcome.result.weights.iter().any(|(k, _)| *k == AgentKind::Verification));
}

// --- offline guarantee ------------------------------------------------------------

#[test]
fn five_local_agents_run_fully_offline() {
    // Only Verification is a network agent — by declaration...
    for kind in [
        AgentKind::Extraction,
        AgentKind::ValidationMaths,
        AgentKind::AiDetection,
        AgentKind::Plagiarism,
        AgentKind::Rag,
    ] {
        assert!(!kind.requires_network(), "{kind:?} must be a local agent");
    }
    assert!(AgentKind::requires_network(&AgentKind::Verification));

    // ...and by construction: a full 5-agent debate over REAL local agent runs,
    // with no proxy/HTTP object in existence anywhere in this test.
    let text = "Sleep and Memory\n\nAbstract\nA randomized trial (n = 96) found improved recall \
                (p < 0.01).\n\nMethods\nWe used a paired t-test.\n\nResults\nRecall improved.\n\n\
                References\nDoe, J. (2022). Sleep. Journal, 1(1), 1-9. https://doi.org/10.1/zzz\n";
    let extraction = crate::extract::extract_from_text(text);
    let validation = crate::validate::validate(&extraction);
    let model = crate::ai_detect::HeuristicModel::gpt2_like();
    let ai = crate::ai_detect::detect_text(&model, text);

    let db = crate::Database::in_memory().unwrap();
    let embedder = crate::embed::HashEmbedder;
    let mut session = crate::plagiarism::PlagiarismSession::new().unwrap();
    session.ingest_manuscript(&embedder, text).unwrap();
    let plag = session.report(&db, None).unwrap();
    let hits = crate::rag::search(&db, &embedder, "sleep and memory guidance", 3, None).unwrap();

    let mut agents = boxed(vec![
        Box::new(PrecomputedAgent::new(adapters::from_extraction(&extraction))),
        Box::new(PrecomputedAgent::new(adapters::from_validation(&validation))),
        Box::new(PrecomputedAgent::new(adapters::from_ai_detection(&ai))),
        Box::new(PrecomputedAgent::new(adapters::from_plagiarism(&plag))),
        Box::new(PrecomputedAgent::new(adapters::from_rag_hits(&hits))),
    ]);
    let outcome = run_debate(&mut agents, &DebateConfig::default()).unwrap();
    assert_eq!(outcome.opinions.len(), 5, "all five local agents debated offline");
    assert!(outcome.converged);
}

// --- end-to-end: all six agents over a sample manuscript ---------------------------

#[test]
fn end_to_end_six_agents_produce_confidence_weighted_result() {
    use crate::refverify::{ExistenceCheck, Provenance, ReferenceVerification, UntrustedText};
    use crate::verify_agent::{verify_citations, MockProxyClient};
    use serde_json::json;

    // A small, stats-clean manuscript with one DOI-bearing reference. The
    // deterministic validator demands effect size + CI next to the p-value.
    let text = "Sleep and Memory\n\nAbstract\nA randomized trial (n = 96) found improved recall \
                (p < 0.01, d = 0.62, 95% CI [0.31, 0.93]).\n\nMethods\nWe used a paired \
                t-test.\n\nResults\nRecall improved significantly in the extended-sleep \
                group.\n\nReferences\nDoe, J. (2022). Sleep and memory. Journal of Rest, 1(1), \
                1-9. https://doi.org/10.1/zzz\n";

    // --- the five local agents (real runs, offline) --------------------------
    let extraction = crate::extract::extract_from_text(text);
    assert!(!extraction.references.is_empty(), "fixture must yield a reference");
    let validation = crate::validate::validate(&extraction);
    let model = crate::ai_detect::HeuristicModel::gpt2_like();
    let ai = crate::ai_detect::detect_text(&model, text);
    let db = crate::Database::in_memory().unwrap();
    let embedder = crate::embed::HashEmbedder;
    let mut session = crate::plagiarism::PlagiarismSession::new().unwrap();
    session.ingest_manuscript(&embedder, text).unwrap();
    let plag = session.report(&db, None).unwrap();
    let hits = crate::rag::search(&db, &embedder, "sleep study reporting standards", 3, None).unwrap();

    // --- the Verification agent (cloud hop mocked; evidence from refverify) --
    let prov = Provenance {
        source: "crossref".into(),
        url: "https://api.crossref.org/works/10.1/zzz".into(),
        fetched_at: 1_000,
        checksum: "c".repeat(64),
        from_cache: false,
    };
    let rv = ReferenceVerification {
        reference_raw: extraction.references[0].raw.clone(),
        exists: Some(ExistenceCheck {
            source: "crossref",
            found: true,
            doi: Some("10.1/zzz".into()),
            title: Some(UntrustedText::new("Sleep and memory", prov.clone())),
            is_retracted_hint: None,
            provenance: prov.clone(),
        }),
        retraction: None,
        open_access: None,
        enrichment: None,
        provenance: vec![prov],
        warnings: vec![],
    };
    let proxy = MockProxyClient::returning(json!({"verdicts": [{
        "citation_id": "c1", "verdict": "SUPPORTED", "confidence": 0.85,
        "rationale": "DOI and title match", "evidence_refs": ["ev-c1-0"]
    }]}));
    let verification =
        verify_citations(&proxy, &[(extraction.references[0].clone(), rv)]).unwrap();
    assert_eq!(proxy.sent_payloads().len(), 1, "exactly one proxied cloud call");

    // --- the round-table -------------------------------------------------------
    let mut agents = boxed(vec![
        Box::new(PrecomputedAgent::new(adapters::from_extraction(&extraction))),
        Box::new(PrecomputedAgent::new(adapters::from_validation(&validation))),
        Box::new(PrecomputedAgent::new(adapters::from_ai_detection(&ai))),
        Box::new(PrecomputedAgent::new(adapters::from_plagiarism(&plag))),
        Box::new(PrecomputedAgent::new(adapters::from_rag_hits(&hits))),
        Box::new(PrecomputedAgent::new(adapters::from_verification_report(&verification))),
    ]);
    let outcome = run_debate(&mut agents, &DebateConfig::default()).unwrap();

    // All six agents contributed; nothing was gate-rejected.
    assert_eq!(outcome.opinions.len(), 6);
    assert!(outcome.rejected.is_empty(), "{:?}", outcome.rejected);
    assert!(outcome.rounds_run <= MAX_ROUNDS);

    // Combined, confidence-weighted result: the validation hard constraint is
    // clean here, so it AGREES with (not overrides) the soft consensus.
    let validation_op =
        outcome.opinions.iter().find(|o| o.agent == AgentKind::ValidationMaths).unwrap();
    assert!(validation_op.hard_constraint);
    assert_eq!(outcome.result.answer, validation_op.answer);
    assert!(!outcome.result.overridden_by_constraint, "no disagreement to override");
    assert!(outcome.result.combined_confidence > 0.0 && outcome.result.combined_confidence <= 1.0);

    // Every non-hard agent has a rescaled weight in the result set.
    let weighted: Vec<AgentKind> = outcome.result.weights.iter().map(|(k, _)| *k).collect();
    for kind in [
        AgentKind::Extraction,
        AgentKind::AiDetection,
        AgentKind::Plagiarism,
        AgentKind::Rag,
        AgentKind::Verification,
    ] {
        assert!(weighted.contains(&kind), "missing weight for {kind:?}");
    }
    // ...and every weight is a valid rescaled confidence.
    for (_, w) in &outcome.result.weights {
        assert!((0.0..=1.0).contains(w));
    }
}
