//! Box 4 — Reviewer Synthesis, app-crate integration (Stage 1: additive shadow).
//!
//! The PURE synthesis logic (aggregate / build request / gate / synthesize)
//! lives in `gaply_core::reviewer_agent`. This module is the I/O bridge: it
//! assembles a [`ReviewerInput`] by JOINING the Evidence Store's per-finding
//! verdicts (`evidence_by_run`) with the compiled report's presentation fields
//! (titles / checklist / journal / verdict), then drives the pure pipeline.
//!
//! # D1, used correctly
//! `verified_findings` and `local_findings` both come from the SAME store query
//! (`evidence_by_run`). They are NOT two queries and NOT partitioned here — the
//! flat `ReviewerInput.findings` carries every row, and consumers distinguish
//! escalated from local purely via `ReviewerFinding::verification_state()`
//! (the single partition path).
//!
//! # Join
//! Store rows carry `finding_id` (`f{N}`); the report carries titles positionally
//! (`report.findings[N-1]`). We join by PARSING `N` out of the id, so the join is
//! robust to store row order — not by assuming the vecs line up.
//!
//! # Stage 1 (this commit): ADDITIVE SHADOW
//! `run_shadow_synthesis` runs ALONGSIDE the wholesale reviewer; its letter is
//! returned as `PublishReadyOutcome.shadow_reviewer` for comparison. The
//! wholesale reviewer remains authoritative. Stage 2 (switch) and Stage 3
//! (remove wholesale) are separate future decisions, gated on the gaply-proxy
//! escalate endpoint + reviewer-quality spike landing.

use serde_json::Value;

use gaply_core::evidence::ClaimKind;
use gaply_core::evidence_store::evidence_by_run;
use gaply_core::report::FindingSeverity;
use gaply_core::reviewer_agent::VerdictWithheld;
use gaply_core::swarm::AgentKind;
use gaply_core::reviewer_agent::{
    aggregate_reviewer_verdict, build_reviewer_request, gate_reviewer_narrative,
    synthesize_reviewer_letter, ReviewerEvaluation, ReviewerFinding, ReviewerInput, ReviewerMeta,
    ReviewerNarrative, TargetJournal, VerdictAggregation,
};
use gaply_core::verify_agent::ProxyClient;
use gaply_core::{Database, GaplyError};

/// Parse the positional index out of an `f{N}` finding id (1-based → 0-based).
fn parse_finding_index(id: &str) -> Option<usize> {
    id.strip_prefix('f')?.parse::<usize>().ok().and_then(|n| n.checked_sub(1))
}

/// Parse a stored `AgentKind` back from the store's TEXT column.
///
/// `None` on failure, and that is TYPED ABSENCE rather than a fallback: unlike
/// severity, an unrecognised family has NO safe default — it can be assumed
/// neither eligible nor ineligible (§21). Empty for rows written before
/// migration 13.
///
/// **This function is the fix for the loss that started ARCHITECTURE_TRACE
/// §22:** `AgentKind` was written as an enum, persisted as a string by
/// `enum_text`, and never restored here.
fn parse_agent(s: &str) -> Option<AgentKind> {
    serde_json::from_value(Value::String(s.to_string())).ok()
}

/// Parse a stored `ClaimKind`. `None` for rows written before migration 13,
/// whose claim is genuinely unrecorded — never defaulted to `ManuscriptDefect`,
/// which would assert something that was never stored (§26.4).
fn parse_claim(s: &str) -> Option<ClaimKind> {
    serde_json::from_value(Value::String(s.to_string())).ok()
}

/// Parse a stored severity string (snake_case, written from `FindingSeverity`)
/// back to the enum. Defensive fallback to `Info` — stored values are always
/// valid, so this only guards against corruption.
fn parse_severity(s: &str) -> FindingSeverity {
    serde_json::from_value(Value::String(s.to_string())).unwrap_or(FindingSeverity::Info)
}

/// Assemble the flat [`ReviewerInput`] for a run: JOIN the Evidence Store's
/// decided verdicts with the report's presentation fields.
///
/// The store is the SOLE source of per-finding verdicts (`report.evidence` is a
/// pre-escalation snapshot with none); the report supplies titles, checklist,
/// journal metadata and the overall verdict.
pub fn assemble_reviewer_input(
    db: &Database,
    run_id: &str,
    report: &Value,
    journal: &TargetJournal,
    supplementary: &[Value],
    withheld: Option<VerdictWithheld>,
) -> Result<ReviewerInput, GaplyError> {
    let rows = evidence_by_run(db, run_id)?;
    let empty: Vec<Value> = Vec::new();
    let report_findings = report["findings"].as_array().unwrap_or(&empty);

    let findings = rows
        .into_iter()
        .map(|row| {
            // Join the title by PARSED f{N} index (not row position).
            let title = parse_finding_index(&row.finding_id)
                .and_then(|idx| report_findings.get(idx))
                .and_then(|f| f["title"].as_str())
                .unwrap_or("")
                .to_string();
            ReviewerFinding {
                severity: parse_severity(&row.severity),
                confidence: row.confidence,
                title,
                evidence_refs: row.evidence_refs,
                verdict: row.llm_verdict,
                verified: row.verified,
                gate_flags: row.gate_flags.unwrap_or_default(),
                id: row.finding_id,
                agent: parse_agent(&row.agent),
                claim: parse_claim(&row.claim),
            }
        })
        .collect();

    let checklist = report["checklist"].as_array().cloned().unwrap_or_default();
    let metadata = ReviewerMeta {
        run_id: run_id.to_string(),
        overall_verdict: report["verdict"].as_str().unwrap_or("").to_string(),
        combined_confidence: report["combined_confidence"].as_f64().unwrap_or(0.0),
    };

    Ok(ReviewerInput {
        withheld,
        findings,
        checklist,
        supplementary: supplementary.to_vec(),
        journal: journal.clone(),
        metadata,
    })
}

/// Result of a shadow synthesis run. Carries the STRUCTURAL signals the
/// comparison harness needs — `narrative_available` is set at the point the
/// narrative is (or isn't) obtained, never inferred from the letter's body text.
pub struct ShadowOutcome {
    /// Carried so the harness can OBSERVE the withheld state rather than reading
    /// the letter's inert `Unknown`/0.0 filler as a real verdict.
    pub withheld: Option<VerdictWithheld>,
    pub letter: ReviewerEvaluation,
    pub aggregation: VerdictAggregation,
    /// True iff the narrative came from a real proxy response (not the honest
    /// degraded placeholder). The harness reads THIS, not `letter.body`.
    pub narrative_available: bool,
    /// Number of finding ids actually sent (issue-coverage denominator).
    pub findings_sent: usize,
}

/// Run the Stage-1 shadow synthesis end to end: assemble → deterministic verdict
/// → build request → (proxy narrative or honest degradation) → gated letter.
///
/// The verdict is deterministic, so a letter is produced even when `proxy` is
/// `None` or the call fails — only the narrative degrades (and `narrative_available`
/// records exactly that). `letter.available` stays true (the verdict is real).
pub fn run_shadow_synthesis(
    db: &Database,
    proxy: Option<&dyn ProxyClient>,
    run_id: &str,
    report: &Value,
    journal: &TargetJournal,
    supplementary: &[Value],
    withheld: Option<VerdictWithheld>,
) -> Result<ShadowOutcome, GaplyError> {
    let input = assemble_reviewer_input(db, run_id, report, journal, supplementary, withheld)?;

    // Deterministic verdict — from the already-decided severities, no LLM.
    let aggregation = aggregate_reviewer_verdict(&input);

    // The SOLE payload/SentIds pair-producer (D4 closed by construction).
    let request = build_reviewer_request(&input);
    let findings_sent = request.sent_finding_count();

    // narrative_available is set HERE, from the actual event — never parsed back
    // out of the letter body.
    let (narrative, narrative_available) = match proxy {
        Some(p) => match p.verify(request.payload()) {
            Ok(resp) => (gate_reviewer_narrative(&resp, &request), true),
            Err(e) => {
                tracing::warn!(error = %e, "shadow synthesis cloud narrative failed; degrading");
                (ReviewerNarrative::unavailable(), false)
            }
        },
        None => (ReviewerNarrative::unavailable(), false),
    };

    let letter = synthesize_reviewer_letter(&aggregation, narrative);
    Ok(ShadowOutcome { withheld, letter, aggregation, narrative_available, findings_sent })
}

#[cfg(test)]
mod tests {
    use super::*;
    use gaply_core::evidence::EvidenceRecord;
    use gaply_core::evidence_store::{evidence_persist, evidence_record_escalation, EscalationOutcome};
    use gaply_core::report::FindingSeverity as Sev;
    use gaply_core::reviewer_agent::Recommendation;
    use gaply_core::swarm::AgentKind;
    use gaply_core::verify_agent::MockProxyClient;
    use serde_json::json;

    fn journal() -> TargetJournal {
        TargetJournal { name: "Nature".into(), quartile: "Q1".into() }
    }

    // Build a minimal report JSON whose findings line up positionally with the
    // f{N} ids we persist.
    fn report(titles: &[&str]) -> Value {
        let findings: Vec<Value> = titles.iter().map(|t| json!({ "title": t })).collect();
        json!({
            "verdict": "concern",
            "combined_confidence": 0.66,
            "findings": findings,
            "checklist": [ { "requirement": "IMRaD present", "passed": true } ],
        })
    }

    fn record(id: &str, agent: AgentKind, severity: Sev, confidence: f64) -> EvidenceRecord {
        EvidenceRecord::at_source(
            id.to_string(),
            agent,
            ClaimKind::ManuscriptDefect,
            severity,
            confidence,
            vec![],
        )
    }

    /// THE property whose absence started ARCHITECTURE_TRACE §22.
    ///
    /// `AgentKind` was written as an enum, persisted as a string by `enum_text`,
    /// and never restored — so the aggregator received `agent` as a `String` it
    /// never inspected, and family identity was not actionable. Covering
    /// `ClaimKind` alone would ship that restoration untested, which is why both
    /// enums are exercised here, over EVERY variant of each.
    ///
    /// Asserts VALUE PRESERVATION, not representational identity. The DB form is
    /// a snake_case string via `enum_text`, so asserting bit-for-bit sameness at
    /// every boundary would assert something false; what must hold is that the
    /// value written is the value that arrives.
    #[test]
    fn producer_and_claim_survive_the_write_persist_restore_round_trip() {
        use gaply_core::evidence::ClaimKind as CK;
        const AGENTS: &[AgentKind] = &[
            AgentKind::Extraction,
            AgentKind::ValidationMaths,
            AgentKind::AiDetection,
            AgentKind::Plagiarism,
            AgentKind::Rag,
            AgentKind::Verification,
        ];
        const CLAIMS: &[CK] = &[CK::ProcessState, CK::AuthorshipSignal, CK::ManuscriptDefect];

        for (i, agent) in AGENTS.iter().enumerate() {
            for (j, claim) in CLAIMS.iter().enumerate() {
                let db = Database::in_memory().unwrap();
                let run_id = format!("run-rt-{i}-{j}");
                let id = "f1";
                let written = EvidenceRecord::at_source(
                    id.to_string(),
                    *agent,
                    *claim,
                    Sev::Major,
                    0.5,
                    vec!["source:test".into()],
                );
                evidence_persist(&db, &run_id, &[written], gaply_core::now_epoch()).unwrap();

                let input =
                    assemble_reviewer_input(&db, &run_id, &report(&["t"]), &journal(), &[], None).unwrap();
                assert_eq!(input.findings.len(), 1, "one record in, one finding out");
                let got = &input.findings[0];

                assert_eq!(
                    got.agent,
                    Some(*agent),
                    "PRODUCER lost across write -> persist -> restore for {agent:?}"
                );
                assert_eq!(
                    got.claim,
                    Some(*claim),
                    "CLAIM lost across write -> persist -> restore for {claim:?}"
                );
            }
        }
    }

    /// A row written before migration 13 carries an empty `claim` (the column's
    /// DEFAULT ''). It must restore as `None` — typed absence — never as
    /// `ManuscriptDefect`, which would assert something that was never stored
    /// (§26.4). Asserted at the mapping, since the app crate has no SQL escape
    /// hatch to age a row; the round-trip test above covers the populated path.
    #[test]
    fn an_unrecorded_claim_restores_as_absent_not_defaulted() {
        assert_eq!(parse_claim(""), None, "migration 13's default is UNKNOWN, not a claim");
        assert_eq!(parse_claim("not_a_claim"), None, "an unrecognised claim is absent");
        assert_eq!(parse_claim("process_state"), Some(ClaimKind::ProcessState));
        // The producer has no safe default either — §21: an unknown family can be
        // assumed neither eligible nor ineligible.
        assert_eq!(parse_agent(""), None);
        assert_eq!(parse_agent("grammar"), None, "an unknown family is absent, not guessed");
        assert_eq!(parse_agent("verification"), Some(AgentKind::Verification));
    }

    /// PR-1 carries identity end to end and must change NO verdict. Pinned so a
    /// data regression and a decision regression stay attributable: if this
    /// fails, the identity work changed behaviour it was not supposed to touch.
    #[test]
    fn pr1_identity_does_not_change_the_recommendation() {
        use gaply_core::reviewer_agent::aggregate_reviewer_verdict;
        let db = Database::in_memory().unwrap();
        let run_id = "run-verdict-pin";
        // 1 Major + 2 Minor: Major dominates, so the tree yields MajorRevision —
        // and the two Minors sit either side of MINOR_REVISION_THRESHOLD, so a
        // miscount would move the result.
        let records = vec![
            EvidenceRecord::at_source("f1".to_string(), AgentKind::AiDetection, ClaimKind::AuthorshipSignal, Sev::Major, 0.6, vec![]),
            EvidenceRecord::at_source("f2".to_string(), AgentKind::Verification, ClaimKind::ProcessState, Sev::Minor, 0.5, vec![]),
            EvidenceRecord::at_source("f3".to_string(), AgentKind::Extraction, ClaimKind::ManuscriptDefect, Sev::Minor, 0.5, vec![]),
        ];
        evidence_persist(&db, run_id, &records, gaply_core::now_epoch()).unwrap();
        let input =
            assemble_reviewer_input(&db, run_id, &report(&["a", "b", "c"]), &journal(), &[], None).unwrap();
        let agg = aggregate_reviewer_verdict(&input);
        assert_eq!(
            agg.verdict.recommendation(),
            Some(Recommendation::MajorRevision),
            "PR-1 must not change the verdict; the resolver lands in PR-3"
        );
        assert_eq!(agg.verdict.probability(), Some(0.30));
        // The claim is CARRIED but not yet CONSULTED — f2 is a process claim and
        // still counts, which is precisely what PR-3 changes.
        assert_eq!(agg.breakdown.minor.total(), 2);
    }

    /// §25.10's defect, closed. Malformed evidence must WITHHOLD, never yield
    /// `Accept` at 0.92 — the silent wrong answer this PR exists to remove.
    #[test]
    fn uninterpretable_evidence_withholds_rather_than_accepting() {
        use gaply_core::reviewer_agent::{aggregate_reviewer_verdict, VerdictWithheld};
        let db = Database::in_memory().unwrap();
        let run_id = "run-withheld";
        // Nothing persisted — exactly what escalation.rs:72's failure path leaves
        // behind. Before this PR that produced zero findings and Accept at 0.92.
        let input = assemble_reviewer_input(
            &db,
            run_id,
            &report(&[]),
            &journal(),
            &[],
            Some(VerdictWithheld::EvidenceUninterpretable),
        )
        .unwrap();
        let agg = aggregate_reviewer_verdict(&input);

        assert_eq!(agg.verdict.recommendation(), None, "no recommendation may be produced");
        assert_eq!(agg.verdict.probability(), None, "and no probability — not 0.05, not 0.92");
        assert_eq!(
            agg.verdict.withheld_reason(),
            Some(VerdictWithheld::EvidenceUninterpretable),
            "the absence must carry its reason (§4.14)"
        );
        // The breakdown still reports what WAS counted — all-zero is a fact, and
        // it must never be overloaded to carry the withheld reason.
        assert_eq!(agg.breakdown.critical.total(), 0);
        assert_eq!(agg.breakdown.minor.total(), 0);
    }

    /// THE INVARIANT: if a verdict is withheld, EVERY consumer must observe that
    /// it is withheld. No consumer may see `recommendation: unavailable` while
    /// another sees `MajorRevision` from the same run.
    ///
    /// The risk is concrete and structural: the withheld state passes through a
    /// TRANSFORMATION before every consumer sees it. `synthesize_reviewer_letter`
    /// is the sole reader of the aggregation's verdict, the harness reads
    /// `s.letter.recommendation` rather than the aggregation, and the UI reads
    /// the letter. So it is asserted at every boundary, the way PR-1 asserted the
    /// round-trip at every boundary.
    #[test]
    fn a_withheld_verdict_is_observed_as_withheld_at_every_boundary() {
        use gaply_core::reviewer_agent::{
            aggregate_reviewer_verdict, synthesize_reviewer_letter, Recommendation,
            ReviewerNarrative, VerdictWithheld,
        };
        use gaply_core::reviewer_harness::{
            build_comparison_report, HarnessInputs, HarnessTiming, ShadowInputs,
        };

        let db = Database::in_memory().unwrap();
        let run_id = "run-invariant";
        let input = assemble_reviewer_input(
            &db,
            run_id,
            &report(&[]),
            &journal(),
            &[],
            Some(VerdictWithheld::EvidenceUninterpretable),
        )
        .unwrap();

        // BOUNDARY 1 — the aggregation.
        let agg = aggregate_reviewer_verdict(&input);
        assert_eq!(agg.verdict.recommendation(), None, "boundary 1: aggregation");

        // BOUNDARY 2 — the letter, which is what the other two read.
        let letter = synthesize_reviewer_letter(
            &agg,
            ReviewerNarrative { body: "ignored".into(), issues: vec![], warnings: vec![] },
        );
        assert!(!letter.available, "boundary 2: the letter must not present itself as available");
        assert_eq!(
            letter.publication_probability, None,
            "boundary 2: no probability at all — not 0.0, not PROB_REJECT's 0.05"
        );
        assert!(
            letter.body.contains("No recommendation was produced"),
            "boundary 2: the reason must be stated to the author, got {:?}",
            letter.body
        );
        assert!(
            letter.warnings.iter().any(|w| w.contains("evidence_uninterpretable")),
            "boundary 2: the machine-readable reason must survive"
        );

        // BOUNDARY 3 — the harness record.
        let wholesale = gaply_core::reviewer_agent::ReviewerEvaluation::unavailable_offline();
        let rec = build_comparison_report(&HarnessInputs {
            run_id,
            manuscript_sha256: "sha",
            recorded_at: 0,
            shadow: Some(ShadowInputs {
                withheld: Some(VerdictWithheld::EvidenceUninterpretable),
                letter: &letter,
                breakdown: &agg.breakdown,
                findings_sent: 0,
                narrative_available: false,
            }),
            wholesale: &wholesale,
            wholesale_findings_sent: 0,
            findings_projection_digest: "d",
            summary_digest: "s",
            summary_format_version: 1,
            journal_name: None,
            guidelines_url: None,
            timing: HarnessTiming::default(),
            proxy_meta: None,
        });
        let v = serde_json::to_value(&rec).unwrap();
        assert_eq!(
            v["shadow_recommendation"]["status"], "unavailable",
            "boundary 3: the record must not observe the letter's inert filler as a verdict"
        );
        assert_eq!(v["shadow_recommendation"]["requires"], "verdict_withheld");
        assert_eq!(
            v["verdict_withheld"]["value"], "evidence_uninterpretable",
            "boundary 3: breakdown all-zero beside an unavailable verdict must be unambiguous"
        );

        // BOUNDARY 4 — the UI payload. The frontend reads the letter, so what it
        // receives is what boundary 2 produced, serialized.
        let ui = serde_json::to_value(&letter).unwrap();
        assert_eq!(ui["available"], false, "boundary 4: UI payload");
        // STRUCTURAL, not a runtime guard: the key is OMITTED from the wire, so
        // no consumer can read a number for a run where none was computed. A
        // plain f64 emitted 0.0 into every artifact for such a run (§4.4).
        assert!(
            ui.get("publication_probability").is_none(),
            "boundary 4: the probability key must be absent, got {ui}"
        );
        assert_ne!(
            ui["recommendation"],
            serde_json::to_value(Recommendation::MajorRevision).unwrap(),
            "boundary 4: no consumer may read a real recommendation from a withheld run"
        );
    }

    #[test]
    fn assemble_joins_store_verdicts_with_report_titles() {
        let db = Database::in_memory().unwrap();
        let run_id = "run-assemble";
        let records = vec![
            record("f1", AgentKind::Verification, Sev::Major, 0.6),
            record("f2", AgentKind::ValidationMaths, Sev::Critical, 1.0),
        ];
        evidence_persist(&db, run_id, &records, gaply_core::now_epoch()).unwrap();
        // Escalate f1 -> gate-passed verdict; f2 stays local.
        evidence_record_escalation(
            &db,
            run_id,
            "f1",
            &EscalationOutcome {
                llm_verdict: "REFUTED".into(),
                llm_rationale: "citation not found".into(),
                verified: true,
                gate_flags: vec![],
                provider: "mock".into(),
                provider_model: "mock-1".into(),
            },
        )
        .unwrap();

        let input =
            assemble_reviewer_input(&db, run_id, &report(&["refuted citation", "impossible SD"]), &journal(), &[], None)
                .unwrap();

        assert_eq!(input.findings.len(), 2);
        let f1 = input.findings.iter().find(|f| f.id == "f1").unwrap();
        assert_eq!(f1.title, "refuted citation"); // joined by parsed f{N} index
        assert_eq!(f1.verified, Some(true));
        assert_eq!(f1.verdict.as_deref(), Some("REFUTED"));
        let f2 = input.findings.iter().find(|f| f.id == "f2").unwrap();
        assert_eq!(f2.title, "impossible SD");
        assert_eq!(f2.verified, None); // never escalated -> NotEscalated
        assert_eq!(input.metadata.run_id, run_id);
        assert_eq!(input.metadata.overall_verdict, "concern");
    }

    #[test]
    fn shadow_synthesis_produces_deterministic_verdict_without_proxy() {
        let db = Database::in_memory().unwrap();
        let run_id = "run-shadow-offline";
        let records = vec![record("f1", AgentKind::ValidationMaths, Sev::Critical, 1.0)];
        evidence_persist(&db, run_id, &records, gaply_core::now_epoch()).unwrap();

        // proxy = None -> narrative degrades, verdict still deterministic.
        let outcome =
            run_shadow_synthesis(&db, None, run_id, &report(&["impossible SD"]), &journal(), &[], None).unwrap();
        assert_eq!(outcome.letter.recommendation, Recommendation::Reject);
        assert!(outcome.letter.available);
        assert!(!outcome.narrative_available); // structural flag, not body-string
        assert!(outcome.letter.body.contains("narrative synthesis unavailable"));
    }

    #[test]
    fn shadow_synthesis_grounds_narrative_against_sent_ids() {
        let db = Database::in_memory().unwrap();
        let run_id = "run-shadow-mock";
        let records = vec![record("f1", AgentKind::Verification, Sev::Major, 0.7)];
        evidence_persist(&db, run_id, &records, gaply_core::now_epoch()).unwrap();

        // The model returns a body + one grounded (f1) and one hallucinated (f99)
        // issue; the gate keeps only the grounded one, and the verdict stays
        // deterministic regardless.
        let proxy = MockProxyClient::returning(json!({
            "body": "The manuscript has a refuted citation.",
            "issues": [
                { "finding_ref": "f1", "severity": "major", "rationale": "citation refuted" },
                { "finding_ref": "f99", "severity": "major", "rationale": "hallucinated" },
            ],
        }));
        let outcome = run_shadow_synthesis(
            &db,
            Some(&proxy as &dyn ProxyClient),
            run_id,
            &report(&["refuted citation"]),
            &journal(),
            &[],
            None,
        )
        .unwrap();
        // Deterministic verdict from the Major finding, regardless of narrative.
        assert_eq!(outcome.letter.recommendation, Recommendation::MajorRevision);
        assert_eq!(outcome.letter.issues.len(), 1);
        assert_eq!(outcome.letter.issues[0].finding_ref, "f1");
        assert!(outcome.letter.available);
        assert!(outcome.narrative_available); // a real proxy response arrived
        assert_eq!(outcome.findings_sent, 1);
    }
}
