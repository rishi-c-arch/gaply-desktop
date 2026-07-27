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

use gaply_core::evidence_store::evidence_by_run;
use gaply_core::report::FindingSeverity;
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
                agent: row.agent,
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
) -> Result<ShadowOutcome, GaplyError> {
    let input = assemble_reviewer_input(db, run_id, report, journal, supplementary)?;

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
    Ok(ShadowOutcome { letter, aggregation, narrative_available, findings_sent })
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
        EvidenceRecord::at_source(id.to_string(), agent, severity, confidence, vec![])
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
            assemble_reviewer_input(&db, run_id, &report(&["refuted citation", "impossible SD"]), &journal(), &[])
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
            run_shadow_synthesis(&db, None, run_id, &report(&["impossible SD"]), &journal(), &[]).unwrap();
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
