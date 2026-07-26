//! Targeted Escalation orchestration (Box 2) — the integrator.
//!
//! Threads `run_id`, deserializes `report["evidence"]`, routes it (Box 1 —
//! `route_evidence`), persists it (Box 3 Phase 1 — `evidence_persist`), batches
//! the escalation set by agent-kind (per-finding for CRITICAL), issues proxy
//! calls, records outcomes (Box 3 Phase 2 — `evidence_record_escalation`), and
//! DEGRADES HONESTLY on any failure (never fails the run).
//!
//! ⚠️ WHAT SHIPS vs. WHAT WORKS: this ships real routing, persistence and
//! idempotency infrastructure. Escalation CALLS degrade to "unavailable" until
//!   (a) gaply-proxy implements a `task:"escalate_findings"` endpoint
//!       (server-side, Python — out of this Rust rebuild's scope), AND
//!   (b) the reviewer-quality spike (does the LLM produce trustworthy, grounded
//!       per-finding verdicts on real papers — the architecture doc's Q12 risk,
//!       still unresolved) is run and passes.
//! This is a structural fact about what does/doesn't work after this commit, not
//! a "coming soon" feature note.
//!
//! IDEMPOTENCY (deep-link double-invocation lesson applied): at-most-once
//! escalation per finding is guaranteed STRUCTURALLY, not by per-finding locking:
//!   - `run_id` (== manuscript_id) is minted fresh per invocation, so two runs
//!     never share a `run_id`;
//!   - `evidence_persist` (plain INSERT of all records) is the run-level claim —
//!     a second persist of the same `run_id` fails loud on UNIQUE(run_id,
//!     finding_id) rather than double-escalating;
//!   - the escalate set is unique finding_ids processed sequentially (one call
//!     per batch, each finding in one batch);
//!   - `evidence_record_escalation` (UPDATE, errors on missing row) is idempotent
//!     (last-wins) and enforces persist-before-record ordering.
//! If a future version parallelizes batches or adds intra-run retry, a claim
//! column would be required — NOT needed for this sequential, no-retry flow.

use std::collections::HashSet;

use gaply_core::escalation::{build_escalation_payload, gate_escalation_response};
use gaply_core::evidence::EvidenceRecord;
use gaply_core::evidence_store::{evidence_persist, evidence_record_escalation, EscalationOutcome};
use gaply_core::orchestrator::{route_evidence, ContextAttachment, RoutingPolicy};
use gaply_core::report::FindingSeverity;
use gaply_core::swarm::AgentKind;
use gaply_core::verify_agent::ProxyClient;
use gaply_core::Database;
use serde_json::Value;

/// Honest tallies for one run.
#[derive(Debug, Default, PartialEq)]
pub struct EscalationSummary {
    /// Records persisted in Phase 1.
    pub persisted: usize,
    /// Findings that received a recorded verdict (verified OR gate-rejected).
    pub escalated: usize,
    /// Findings eligible for escalation whose call degraded (no proxy / error /
    /// omitted by the reply) — left at local confidence, `verified` NULL.
    pub unavailable: usize,
    /// Eligible-but-unselected (budget) — from `route_evidence`.
    pub omitted: usize,
}

/// Run Box 2 for one PublishReady run. ADDITIVE + never-fails: persists evidence
/// and attempts escalation; returns a summary (errors are logged + degraded).
/// `proxy == None` (or a caller-filtered unreachable proxy) → every escalation
/// degrades to unavailable, while Phase-1 persistence still happens.
pub fn run_targeted_escalation(
    db: &Database,
    proxy: Option<&dyn ProxyClient>,
    run_id: &str,
    report: &Value,
    policy: &dyn RoutingPolicy,
    now: i64,
) -> EscalationSummary {
    let records: Vec<EvidenceRecord> =
        serde_json::from_value(report["evidence"].clone()).unwrap_or_default();
    if records.is_empty() {
        return EscalationSummary::default();
    }

    let routed = route_evidence(&records, policy);

    // Phase 1: persist ALL records once (the run-level claim). On failure (e.g. a
    // duplicate run_id — a double-persist bug), degrade honestly: skip escalation,
    // never fail the run.
    if let Err(e) = evidence_persist(db, run_id, &records, now) {
        tracing::warn!(run_id, error = %e, "evidence_persist failed; skipping escalation");
        return EscalationSummary { omitted: routed.omitted_count, ..Default::default() };
    }
    let persisted = records.len();

    let escalate: HashSet<&str> = routed.escalate.iter().map(String::as_str).collect();
    if escalate.is_empty() {
        return EscalationSummary { persisted, omitted: routed.omitted_count, ..Default::default() };
    }

    let mut escalated = 0usize;
    let mut unavailable = 0usize;
    for batch in batch_escalation(&records, &escalate) {
        match escalate_batch(proxy, run_id, &batch, &routed.context_pool) {
            Some(outcomes) => {
                let recorded: HashSet<&str> = outcomes.iter().map(|(id, _)| id.as_str()).collect();
                for (fid, outcome) in &outcomes {
                    match evidence_record_escalation(db, run_id, fid, outcome) {
                        Ok(()) => escalated += 1,
                        Err(e) => {
                            tracing::warn!(run_id, finding = %fid, error = %e, "record_escalation failed");
                            unavailable += 1;
                        }
                    }
                }
                // Findings sent but not addressed by the reply stay NULL (unavailable).
                unavailable += batch.iter().filter(|r| !recorded.contains(r.id.as_str())).count();
            }
            None => unavailable += batch.len(), // no proxy / call error / gate error
        }
    }

    EscalationSummary { persisted, escalated, unavailable, omitted: routed.omitted_count }
}

/// Each CRITICAL finding → its own call; the rest grouped by agent (deterministic
/// order). Only records whose id is in `escalate` are batched.
fn batch_escalation<'a>(
    records: &'a [EvidenceRecord],
    escalate: &HashSet<&str>,
) -> Vec<Vec<&'a EvidenceRecord>> {
    let mut critical: Vec<Vec<&EvidenceRecord>> = Vec::new();
    let mut by_agent: Vec<(AgentKind, Vec<&EvidenceRecord>)> = Vec::new();
    for r in records.iter().filter(|r| escalate.contains(r.id.as_str())) {
        if r.severity == FindingSeverity::Critical {
            critical.push(vec![r]);
            continue;
        }
        match by_agent.iter_mut().find(|(a, _)| *a == r.agent) {
            Some((_, b)) => b.push(r),
            None => by_agent.push((r.agent, vec![r])),
        }
    }
    critical.into_iter().chain(by_agent.into_iter().map(|(_, b)| b)).collect()
}

/// Attempt one batch. `None` = degraded (no proxy, call error, or gate error).
/// Stamps `run_id` on the payload so the proxy can meter at-most-once per run
/// (server-side dedup — one run = one use; a proxy contract, out of Rust scope).
fn escalate_batch(
    proxy: Option<&dyn ProxyClient>,
    run_id: &str,
    batch: &[&EvidenceRecord],
    context: &[ContextAttachment],
) -> Option<Vec<(String, EscalationOutcome)>> {
    let proxy = proxy?;
    let (mut payload, sent) = build_escalation_payload(batch, context);
    payload["run_id"] = Value::String(run_id.to_string());
    match proxy.verify(&payload).and_then(|resp| gate_escalation_response(&resp, &sent)) {
        Ok(outcomes) => Some(outcomes),
        Err(e) => {
            tracing::warn!(error = %e, "escalation batch failed; findings stay unavailable");
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gaply_core::error::GaplyError;
    use gaply_core::orchestrator::DefaultRoutingPolicy;
    use gaply_core::verify_agent::MockProxyClient;
    use serde_json::json;

    /// A proxy that always errors (simulates timeout/5xx) — MockProxyClient only
    /// returns Ok.
    struct ErrProxy;
    impl ProxyClient for ErrProxy {
        fn verify(&self, _p: &Value) -> Result<Value, GaplyError> {
            Err(GaplyError::Internal("simulated proxy timeout".into()))
        }
    }

    // A report Value carrying two escalation-eligible verification findings.
    fn report_with_two_low_conf_verifications() -> Value {
        let evidence = vec![
            EvidenceRecord::at_source(
                "f1",
                AgentKind::Verification,
                FindingSeverity::Minor,
                0.60,
                vec!["evidence:ev-1".into()],
            ),
            EvidenceRecord::at_source(
                "f2",
                AgentKind::Verification,
                FindingSeverity::Major,
                0.55,
                vec!["evidence:ev-2".into()],
            ),
        ];
        json!({ "evidence": serde_json::to_value(&evidence).unwrap() })
    }

    #[test]
    fn successful_batch_records_verified_outcomes() {
        let db = Database::in_memory().unwrap();
        let report = report_with_two_low_conf_verifications();
        let proxy = MockProxyClient::returning(json!({
            "provider": "openai", "provider_model": "gpt-5.5",
            "verdicts": [
                { "finding_id": "f1", "verdict": "REFUTED", "rationale": "r", "evidence_refs": ["evidence:ev-1"] },
                { "finding_id": "f2", "verdict": "UNKNOWN", "rationale": "r", "evidence_refs": ["evidence:ev-2"] }
            ]
        }));
        let summary = run_targeted_escalation(
            &db, Some(&proxy), "run-1", &report, &DefaultRoutingPolicy::default(), 1_000,
        );
        assert_eq!(summary.persisted, 2);
        assert_eq!(summary.escalated, 2);
        assert_eq!(summary.unavailable, 0);
        // Both persisted with verified=true.
        let f1 = gaply_core::evidence_store::evidence_by_finding(&db, "run-1", "f1").unwrap().unwrap();
        assert_eq!(f1.verified, Some(true));
        assert_eq!(f1.llm_verdict.as_deref(), Some("REFUTED"));
        assert_eq!(f1.provider.as_deref(), Some("openai"));
    }

    #[test]
    fn gate_rejected_batch_persists_verified_false() {
        let db = Database::in_memory().unwrap();
        let report = report_with_two_low_conf_verifications();
        // f1 cites an UNGROUNDED ref → gate-rejected; f2 grounded → verified.
        let proxy = MockProxyClient::returning(json!({
            "provider": "openai", "provider_model": "gpt-5.5",
            "verdicts": [
                { "finding_id": "f1", "verdict": "REFUTED", "rationale": "r", "evidence_refs": ["evidence:made-up"] },
                { "finding_id": "f2", "verdict": "SUPPORTED", "rationale": "r", "evidence_refs": ["evidence:ev-2"] }
            ]
        }));
        run_targeted_escalation(&db, Some(&proxy), "run-1", &report, &DefaultRoutingPolicy::default(), 1_000);
        let f1 = gaply_core::evidence_store::evidence_by_finding(&db, "run-1", "f1").unwrap().unwrap();
        assert_eq!(f1.verified, Some(false), "ungrounded → escalated + gate-rejected (0), not NULL");
        let f2 = gaply_core::evidence_store::evidence_by_finding(&db, "run-1", "f2").unwrap().unwrap();
        assert_eq!(f2.verified, Some(true));
    }

    #[test]
    fn proxy_error_leaves_findings_unavailable_and_run_continues() {
        let db = Database::in_memory().unwrap();
        let report = report_with_two_low_conf_verifications();
        let summary = run_targeted_escalation(
            &db, Some(&ErrProxy), "run-1", &report, &DefaultRoutingPolicy::default(), 1_000,
        );
        // Phase 1 still persisted; escalation degraded — nothing recorded.
        assert_eq!(summary.persisted, 2);
        assert_eq!(summary.escalated, 0);
        assert_eq!(summary.unavailable, 2);
        // Rows exist (persisted) with NULL escalation state — NOT verified=false.
        let f1 = gaply_core::evidence_store::evidence_by_finding(&db, "run-1", "f1").unwrap().unwrap();
        assert!(f1.verified.is_none(), "degraded → NULL (not escalated), distinct from gate-rejected 0");
        assert!(f1.llm_verdict.is_none());
    }

    #[test]
    fn no_proxy_persists_but_degrades_all() {
        let db = Database::in_memory().unwrap();
        let report = report_with_two_low_conf_verifications();
        let summary =
            run_targeted_escalation(&db, None, "run-1", &report, &DefaultRoutingPolicy::default(), 1_000);
        assert_eq!(summary.persisted, 2);
        assert_eq!(summary.escalated, 0);
        assert_eq!(summary.unavailable, 2);
        // Every record is still queryable (Phase 1 done, escalation NULL).
        assert_eq!(gaply_core::evidence_store::evidence_by_run(&db, "run-1").unwrap().len(), 2);
    }
}
