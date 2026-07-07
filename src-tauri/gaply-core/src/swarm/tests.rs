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

// --- peer-informed reconsideration (RevisingVerificationAgent) ----------------------

mod reconsideration {
    use super::*;
    use crate::extract::citations::Reference;
    use crate::refverify::{ExistenceCheck, Provenance, ReferenceVerification, UntrustedText};
    use crate::verify_agent::{verify_citations, MockProxyClient, Verdict};
    use serde_json::json;

    /// One reference with real evidence (evidence key ev-c1-0), as in Prompt 17.
    fn items() -> Vec<(Reference, ReferenceVerification)> {
        let prov = Provenance {
            source: "crossref".into(),
            url: "https://api.crossref.org/works/10.1/abc".into(),
            fetched_at: 1_000,
            checksum: "c".repeat(64),
            from_cache: false,
        };
        let reference = Reference {
            raw: "Doe, J. (2022). A paper. Journal. https://doi.org/10.1/abc".into(),
            authors: "Doe, J.".into(),
            year: Some(2022),
            title: Some("A Study of Things".into()),
            doi: Some("10.1/abc".into()),
        };
        let rv = ReferenceVerification {
            reference_raw: reference.raw.clone(),
            exists: Some(ExistenceCheck {
                source: "crossref",
                found: true,
                doi: Some("10.1/abc".into()),
                title: Some(UntrustedText::new("A Study of Things", prov.clone())),
                is_retracted_hint: None,
                provenance: prov.clone(),
            }),
            retraction: None,
            open_access: None,
            enrichment: None,
            provenance: vec![prov],
            warnings: vec![],
        };
        vec![(reference, rv)]
    }

    fn supported_response() -> serde_json::Value {
        json!({"verdicts": [{
            "citation_id": "c1", "verdict": "SUPPORTED", "confidence": 0.9,
            "rationale": "doi and title match", "evidence_refs": ["ev-c1-0"]
        }]})
    }

    #[test]
    fn peer_contradiction_triggers_grounded_reconsideration_to_cautious_verdict() {
        // Round 2 (reconsideration): the model, seeing the plagiarism finding,
        // downgrades to UNKNOWN — GROUNDED in a real evidence ref, so the
        // harness gate confirms it and the revision stands.
        let proxy = MockProxyClient::returning_sequence(vec![
            supported_response(),
            json!({"verdicts": [{
                "citation_id": "c1", "verdict": "UNKNOWN", "confidence": 0.3,
                "rationale": "peer similarity finding undermines support; evidence insufficient",
                "evidence_refs": ["ev-c1-0"]
            }]}),
        ]);
        let items = items();
        let report = verify_citations(&proxy, &items).unwrap();
        assert_eq!(report.verdict_for("c1").unwrap().verdict, Verdict::Supported);

        let mut agent = RevisingVerificationAgent::new(&proxy, items, report);
        let own = agent.opine().unwrap();
        assert!((own.confidence - 0.9).abs() < 1e-9);

        // Plagiarism contradicts (high-similarity match on a supported passage).
        let peer = opinion(AgentKind::Plagiarism, ANSWER_CONCERN, 0.92);
        let revised = agent.revise(&own, &[peer], 1).expect("contradiction must trigger revision");

        // Verdict level: SUPPORTED -> UNKNOWN (more cautious), and it passed the
        // gate cleanly because it cited provided evidence.
        let v = agent.report().verdict_for("c1").unwrap();
        assert_eq!(v.verdict, Verdict::Unknown);
        assert!(v.gate_flags.is_empty(), "grounded revision must not be flagged: {:?}", v.gate_flags);
        assert!(revised.confidence < own.confidence, "opinion confidence must drop");

        // Exactly two proxy rounds, and the second is the reconsideration
        // payload: prior verdicts + peer findings, still structured-only.
        let payloads = proxy.sent_payloads();
        assert_eq!(payloads.len(), 2);
        let wire = serde_json::to_string(&payloads[1]).unwrap();
        assert!(wire.contains("citation_verification_reconsideration"));
        assert!(wire.contains("prior_verdicts"));
        assert!(wire.contains("peer_findings"));
        assert!(wire.contains("Plagiarism"));
        assert!(wire.contains("not authority"), "peers must be framed as context, not authority");
    }

    #[test]
    fn ungrounded_peer_pressure_revision_is_caught_by_the_gate() {
        // Variant A: the reconsidered verdict flips to REFUTED citing evidence
        // we never provided -> gate 1 (potential hallucination).
        let proxy = MockProxyClient::returning_sequence(vec![
            supported_response(),
            json!({"verdicts": [{
                "citation_id": "c1", "verdict": "REFUTED", "confidence": 0.95,
                "rationale": "the plagiarism agent said so",
                "evidence_refs": ["ev-c1-99"]
            }]}),
        ]);
        let items_a = items();
        let report = verify_citations(&proxy, &items_a).unwrap();
        let mut agent = RevisingVerificationAgent::new(&proxy, items_a, report);
        let own = agent.opine().unwrap();
        agent.revise(&own, &[opinion(AgentKind::Plagiarism, ANSWER_CONCERN, 0.9)], 1);
        let v = agent.report().verdict_for("c1").unwrap();
        assert_eq!(v.verdict, Verdict::Unknown, "phantom-evidence flip must not survive");
        assert!(v.gate_flags.iter().any(|f| f.contains("potential_hallucination")), "{:?}", v.gate_flags);

        // Variant B: the flip cites NO evidence at all — blind deference to the
        // peer -> gate 3 (revision grounding).
        let proxy = MockProxyClient::returning_sequence(vec![
            supported_response(),
            json!({"verdicts": [{
                "citation_id": "c1", "verdict": "REFUTED", "confidence": 0.95,
                "rationale": "peers are confident", "evidence_refs": []
            }]}),
        ]);
        let items_b = items();
        let report = verify_citations(&proxy, &items_b).unwrap();
        let mut agent = RevisingVerificationAgent::new(&proxy, items_b, report);
        let own = agent.opine().unwrap();
        agent.revise(&own, &[opinion(AgentKind::Plagiarism, ANSWER_CONCERN, 0.9)], 1);
        let v = agent.report().verdict_for("c1").unwrap();
        assert_eq!(v.verdict, Verdict::Unknown, "ungrounded flip must not survive");
        assert!(v.gate_flags.iter().any(|f| f.contains("revision_not_grounded")), "{:?}", v.gate_flags);
    }

    #[test]
    fn only_the_verification_agent_reconsiders() {
        // Design boundary: measurements, not beliefs — a PrecomputedAgent never
        // revises, however loud the contradiction.
        let contradiction = vec![opinion(AgentKind::Verification, ANSWER_CONCERN, 0.99)];
        for kind in [
            AgentKind::Extraction,
            AgentKind::ValidationMaths,
            AgentKind::AiDetection,
            AgentKind::Plagiarism,
            AgentKind::Rag,
        ] {
            let own = opinion(kind, ANSWER_PASS, 0.9);
            let mut agent = PrecomputedAgent::new(own.clone());
            assert!(
                agent.revise(&own, &contradiction, 1).is_none(),
                "{kind:?} must never reconsider"
            );
        }
    }

    #[test]
    fn reconsideration_spends_a_debate_round_within_the_budget() {
        // In a full debate: round 1 = the reconsideration (a revision), round 2 =
        // quiet -> converged. One reconsideration max, ceiling respected.
        let proxy = MockProxyClient::returning_sequence(vec![
            supported_response(),
            json!({"verdicts": [{
                "citation_id": "c1", "verdict": "UNKNOWN", "confidence": 0.3,
                "rationale": "reconsidered", "evidence_refs": ["ev-c1-0"]
            }]}),
        ]);
        let items = items();
        let report = verify_citations(&proxy, &items).unwrap();
        let reviser = RevisingVerificationAgent::new(&proxy, items, report);

        let mut agents: Vec<Box<dyn SwarmAgent + '_>> = vec![
            Box::new(PrecomputedAgent::new(opinion(AgentKind::Extraction, ANSWER_PASS, 0.9))),
            Box::new(PrecomputedAgent::new(opinion(AgentKind::Plagiarism, ANSWER_CONCERN, 0.92))),
            Box::new(reviser),
        ];
        let outcome = run_debate(&mut agents, &DebateConfig::default()).unwrap();

        assert!(outcome.rounds_run <= MAX_ROUNDS);
        assert!(outcome.converged, "one reconsideration then quiet -> converged");
        assert_eq!(proxy.sent_payloads().len(), 2, "initial call + exactly one reconsideration");
        // the verification opinion in the outcome reflects the revision
        let vop = outcome.opinions.iter().find(|o| o.agent == AgentKind::Verification).unwrap();
        assert!(vop.confidence < 0.9, "revised (more cautious) confidence: {}", vop.confidence);
    }
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
