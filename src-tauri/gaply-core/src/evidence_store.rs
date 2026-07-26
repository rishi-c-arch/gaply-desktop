//! Evidence Store (Box 3) — local persistence for the Evidence Model.
//!
//! RUN IDENTITY INVARIANT: a PublishReady run has exactly one `run_id`
//! (== manuscript_id). All escalation, evidence storage, reviewer synthesis, and
//! chat interactions for that run operate on this same identity. Re-running
//! analysis creates a new run_id.
//!
//! DOMAIN vs PERSISTENCE: [`crate::evidence::EvidenceRecord`] is the transient
//! domain object; [`EvidenceRow`] here is the stored shape (a superset — the
//! record's honesty fields + escalation outcome + run identity + timestamp).
//! They intentionally differ, so the domain never needs `Deserialize`; rows are
//! mapped from sqlite BY HAND.
//!
//! TWO-PHASE WRITE:
//!   1. `evidence_persist` — once per run, right after compile_report: INSERT
//!      every record (escalation columns NULL) + prune.
//!   2. `evidence_record_escalation` — later, per escalated finding: an explicit
//!      UPDATE of only the escalation columns (never REPLACE, which would null
//!      the Phase-1 domain fields this call does not hold).
//!
//! Provider-blind: `provider`/`provider_model` are RECORDED from the proxy's
//! uniform JSON, never branched on.

use rusqlite::{params, OptionalExtension};

use crate::evidence::EvidenceRecord;
use crate::{Database, GaplyError};

/// Rows older than this are pruned (amortized, on each `evidence_persist`).
const EVIDENCE_RETENTION_SECS: i64 = 90 * 24 * 3600;
/// Keep at most this many most-recent runs (amortized, on each `evidence_persist`).
const EVIDENCE_MAX_RUNS: i64 = 10;

const COLS: &str = "run_id, finding_id, agent, severity, confidence, confidence_kind, \
    routing_hint, provenance, evidence_refs, limitations, llm_verdict, llm_rationale, \
    verified, gate_flags, provider, provider_model, evidence_schema_version, created_at";

/// The escalation outcome (Box 2 produces this). Gated + structured — never
/// manuscript text. `verified` = false means the grounding gate rejected it,
/// true means it passed; a not-yet-escalated row stores NULL, a distinct state.
pub struct EscalationOutcome {
    pub llm_verdict: String,
    pub llm_rationale: String,
    pub verified: bool,
    pub gate_flags: Vec<String>,
    pub provider: String,
    pub provider_model: String,
}

/// A stored evidence row (persistence DTO; distinct from the domain record).
#[derive(Debug, Clone, PartialEq)]
pub struct EvidenceRow {
    pub run_id: String,
    pub finding_id: String,
    pub agent: String,
    pub severity: String,
    pub confidence: f64,
    pub confidence_kind: String,
    pub routing_hint: String,
    pub provenance: Vec<String>,
    pub evidence_refs: Vec<String>,
    pub limitations: Option<String>,
    // --- escalation columns: None until the finding is escalated (Phase 2) ---
    pub llm_verdict: Option<String>,
    pub llm_rationale: Option<String>,
    /// THREE-STATE: `None` = not escalated; `Some(false)` = escalated +
    /// gate-rejected; `Some(true)` = escalated + passed. `None` and `Some(false)`
    /// MUST NEVER be conflated in query logic.
    pub verified: Option<bool>,
    pub gate_flags: Option<Vec<String>>,
    pub provider: Option<String>,
    pub provider_model: Option<String>,
    pub evidence_schema_version: u32,
    pub created_at: i64,
}

/// Serialize a serde enum to its snake_case TEXT repr (e.g. AgentKind -> "rag").
fn enum_text<T: serde::Serialize>(v: &T) -> String {
    serde_json::to_value(v).ok().and_then(|j| j.as_str().map(String::from)).unwrap_or_default()
}

fn json_arr(v: &[String]) -> String {
    serde_json::to_string(v).unwrap_or_else(|_| "[]".to_string())
}

fn json_vec(s: &str) -> Vec<String> {
    serde_json::from_str(s).unwrap_or_default()
}

/// Phase 1: persist EVERY EvidenceRecord for a run (escalation columns NULL),
/// then prune — the single amortized prune trigger. Called once per PublishReady
/// run right after compile_report. Plain INSERT (not OR REPLACE): a fresh run_id
/// is expected (see the invariant), so a UNIQUE(run_id, finding_id) collision is
/// a double-persist caller bug and surfaces as an error rather than silently
/// nulling a row's escalation columns. `created_at` is the injected clock (also
/// the prune "now" — deterministic).
pub fn evidence_persist(
    db: &Database,
    run_id: &str,
    records: &[EvidenceRecord],
    created_at: i64,
) -> Result<(), GaplyError> {
    let mut conn = db.conn()?;
    let tx = conn.transaction()?;
    for r in records {
        tx.execute(
            "INSERT INTO evidence
             (run_id, finding_id, agent, severity, confidence, confidence_kind,
              routing_hint, provenance, evidence_refs, limitations,
              evidence_schema_version, created_at)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12)",
            params![
                run_id,
                r.id,
                enum_text(&r.agent),
                enum_text(&r.severity),
                r.confidence,
                enum_text(&r.confidence_kind),
                enum_text(&r.routing_hint),
                json_arr(&r.provenance),
                json_arr(&r.evidence_refs),
                r.limitations,
                r.schema_version as i64,
                created_at,
            ],
        )?;
    }
    prune(&tx, created_at)?;
    tx.commit()?;
    Ok(())
}

/// Phase 2: fill in one finding's escalation outcome via an EXPLICIT UPDATE of
/// ONLY the escalation columns — NEVER INSERT OR REPLACE (REPLACE would
/// delete+reinsert and NULL out the Phase-1 domain fields this call does not
/// hold). Errors if no matching row exists: escalation must always follow a
/// prior `evidence_persist`, so a miss is a caller bug, not a normal state.
pub fn evidence_record_escalation(
    db: &Database,
    run_id: &str,
    finding_id: &str,
    outcome: &EscalationOutcome,
) -> Result<(), GaplyError> {
    let n = db.conn()?.execute(
        "UPDATE evidence
            SET llm_verdict = ?3, llm_rationale = ?4, verified = ?5,
                gate_flags = ?6, provider = ?7, provider_model = ?8
          WHERE run_id = ?1 AND finding_id = ?2",
        params![
            run_id,
            finding_id,
            outcome.llm_verdict,
            outcome.llm_rationale,
            outcome.verified as i64, // Some(bool) -> 0/1; NULL only when never escalated
            json_arr(&outcome.gate_flags),
            outcome.provider,
            outcome.provider_model,
        ],
    )?;
    if n == 0 {
        return Err(GaplyError::NotFound {
            entity: "evidence",
            id: format!("{run_id}/{finding_id}"),
        });
    }
    Ok(())
}

/// Retention = the INTERSECTION of two bounds: keep a row ONLY if it is BOTH
/// newer than the TTL AND belongs to one of the `EVIDENCE_MAX_RUNS` most-recent
/// runs. Equivalently, DELETE a row failing EITHER bound. Runs inside
/// `evidence_persist`'s transaction; `now` is the injected clock (deterministic).
fn prune(tx: &rusqlite::Transaction, now: i64) -> Result<(), GaplyError> {
    let cutoff = now - EVIDENCE_RETENTION_SECS;
    tx.execute(
        "DELETE FROM evidence
          WHERE created_at < ?1
             OR run_id NOT IN (
                 SELECT run_id FROM evidence
                 GROUP BY run_id
                 ORDER BY MAX(created_at) DESC
                 LIMIT ?2
             )",
        params![cutoff, EVIDENCE_MAX_RUNS],
    )?;
    Ok(())
}

fn row_to_evidence(row: &rusqlite::Row) -> rusqlite::Result<EvidenceRow> {
    let provenance: String = row.get(7)?;
    let evidence_refs: String = row.get(8)?;
    let verified: Option<i64> = row.get(12)?;
    let gate_flags: Option<String> = row.get(13)?;
    Ok(EvidenceRow {
        run_id: row.get(0)?,
        finding_id: row.get(1)?,
        agent: row.get(2)?,
        severity: row.get(3)?,
        confidence: row.get(4)?,
        confidence_kind: row.get(5)?,
        routing_hint: row.get(6)?,
        provenance: json_vec(&provenance),
        evidence_refs: json_vec(&evidence_refs),
        limitations: row.get(9)?,
        llm_verdict: row.get(10)?,
        llm_rationale: row.get(11)?,
        verified: verified.map(|v| v != 0),
        gate_flags: gate_flags.as_deref().map(json_vec),
        provider: row.get(14)?,
        provider_model: row.get(15)?,
        evidence_schema_version: row.get::<_, i64>(16)? as u32,
        created_at: row.get(17)?,
    })
}

/// All evidence for a run, in report order (insertion order == f1..fN).
pub fn evidence_by_run(db: &Database, run_id: &str) -> Result<Vec<EvidenceRow>, GaplyError> {
    let conn = db.conn()?;
    let mut stmt =
        conn.prepare(&format!("SELECT {COLS} FROM evidence WHERE run_id = ?1 ORDER BY rowid"))?;
    let rows = stmt.query_map(params![run_id], row_to_evidence)?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

/// One finding's evidence, or None if absent.
pub fn evidence_by_finding(
    db: &Database,
    run_id: &str,
    finding_id: &str,
) -> Result<Option<EvidenceRow>, GaplyError> {
    let conn = db.conn()?;
    Ok(conn
        .query_row(
            &format!("SELECT {COLS} FROM evidence WHERE run_id = ?1 AND finding_id = ?2"),
            params![run_id, finding_id],
            row_to_evidence,
        )
        .optional()?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::evidence::EvidenceRecord;
    use crate::report::FindingSeverity;
    use crate::swarm::AgentKind;

    fn rec(id: &str, agent: AgentKind, conf: f64) -> EvidenceRecord {
        EvidenceRecord::at_source(id, agent, FindingSeverity::Major, conf, vec![
            format!("agent:{}", enum_text(&agent)),
        ])
    }

    fn outcome() -> EscalationOutcome {
        EscalationOutcome {
            llm_verdict: "supported".into(),
            llm_rationale: "evidence confirms".into(),
            verified: true,
            gate_flags: vec!["grounded".into()],
            provider: "openai".into(),
            provider_model: "gpt-5.5".into(),
        }
    }

    #[test]
    fn phase1_insert_and_read_back_roundtrip_escalation_cols_null() {
        let db = Database::in_memory().unwrap();
        let records = vec![rec("f1", AgentKind::Plagiarism, 0.91), rec("f2", AgentKind::Rag, 0.7)];
        evidence_persist(&db, "run-1", &records, 1_000).unwrap();

        let rows = evidence_by_run(&db, "run-1").unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].finding_id, "f1"); // rowid order == report order
        assert_eq!(rows[0].confidence, 0.91);
        assert_eq!(rows[0].confidence_kind, "wired_real");
        assert_eq!(rows[0].agent, "plagiarism");
        // Escalation columns are NULL (three-state None), NOT Some(false).
        assert!(rows[0].verified.is_none());
        assert!(rows[0].llm_verdict.is_none());
        assert!(rows[0].provider.is_none());
    }

    #[test]
    fn phase2_update_fills_escalation_without_touching_domain_fields() {
        // Regression test for the REPLACE-vs-UPDATE bug.
        let db = Database::in_memory().unwrap();
        evidence_persist(&db, "run-1", &[rec("f1", AgentKind::Verification, 0.42)], 1_000).unwrap();

        let before = evidence_by_finding(&db, "run-1", "f1").unwrap().unwrap();
        evidence_record_escalation(&db, "run-1", "f1", &outcome()).unwrap();
        let after = evidence_by_finding(&db, "run-1", "f1").unwrap().unwrap();

        // Escalation columns now filled.
        assert_eq!(after.llm_verdict.as_deref(), Some("supported"));
        assert_eq!(after.verified, Some(true));
        assert_eq!(after.provider.as_deref(), Some("openai"));
        assert_eq!(after.gate_flags, Some(vec!["grounded".to_string()]));
        // Domain fields UNCHANGED by the UPDATE (the whole point).
        assert_eq!(after.confidence, before.confidence);
        assert_eq!(after.severity, before.severity);
        assert_eq!(after.confidence_kind, before.confidence_kind);
        assert_eq!(after.provenance, before.provenance);
        assert_eq!(after.agent, before.agent);
    }

    #[test]
    fn escalation_errors_on_missing_row() {
        let db = Database::in_memory().unwrap();
        evidence_persist(&db, "run-1", &[rec("f1", AgentKind::Rag, 0.5)], 1_000).unwrap();
        // Wrong finding_id → no row → error (caller bug, not a normal state).
        let err = evidence_record_escalation(&db, "run-1", "f99", &outcome());
        assert!(matches!(err, Err(GaplyError::NotFound { entity: "evidence", .. })));
        // Wrong run_id → also errors.
        assert!(evidence_record_escalation(&db, "run-x", "f1", &outcome()).is_err());
    }

    #[test]
    fn prune_removes_rows_failing_either_bound() {
        let db = Database::in_memory().unwrap();
        let day = 24 * 3600;

        // (a) TTL bound: an OLD run persisted at t=0; then a fresh run 100 days
        //     later prunes the old one (created_at < now - 90d) even though only
        //     two runs exist (well under the 10-run cap).
        evidence_persist(&db, "old", &[rec("f1", AgentKind::Rag, 0.5)], 0).unwrap();
        evidence_persist(&db, "new", &[rec("f1", AgentKind::Rag, 0.6)], 100 * day).unwrap();
        assert!(evidence_by_run(&db, "old").unwrap().is_empty(), "old run pruned by TTL");
        assert_eq!(evidence_by_run(&db, "new").unwrap().len(), 1);

        // (b) Count bound: 11 runs all within the TTL window; the oldest of the 11
        //     falls outside the 10 most-recent and is pruned even though it's fresh.
        let db2 = Database::in_memory().unwrap();
        for i in 0..11 {
            let t = 200 * day + i as i64; // all recent, strictly increasing
            evidence_persist(&db2, &format!("r{i}"), &[rec("f1", AgentKind::Rag, 0.5)], t).unwrap();
        }
        assert!(evidence_by_run(&db2, "r0").unwrap().is_empty(), "oldest of 11 pruned by count cap");
        assert_eq!(evidence_by_run(&db2, "r10").unwrap().len(), 1, "newest retained");
        // Exactly the 10 most-recent runs remain.
        let remaining: i64 = db2
            .conn()
            .unwrap()
            .query_row("SELECT COUNT(DISTINCT run_id) FROM evidence", [], |r| r.get(0))
            .unwrap();
        assert_eq!(remaining, 10);
    }

    #[test]
    fn by_run_and_by_finding_include_non_escalated_rows() {
        let db = Database::in_memory().unwrap();
        let records = vec![
            rec("f1", AgentKind::Plagiarism, 0.91), // will be escalated
            rec("f2", AgentKind::Extraction, 0.9),  // stays non-escalated (HeldOut)
        ];
        evidence_persist(&db, "run-1", &records, 1_000).unwrap();
        evidence_record_escalation(&db, "run-1", "f1", &outcome()).unwrap();

        // by_run returns BOTH — escalated and not.
        let rows = evidence_by_run(&db, "run-1").unwrap();
        assert_eq!(rows.len(), 2);
        let f2 = evidence_by_finding(&db, "run-1", "f2").unwrap().unwrap();
        assert!(f2.verified.is_none(), "non-escalated row present with NULL escalation state");
        assert_eq!(f2.routing_hint, "held_out");
        let f1 = evidence_by_finding(&db, "run-1", "f1").unwrap().unwrap();
        assert_eq!(f1.verified, Some(true));
        // A finding that never existed → None (not an error).
        assert!(evidence_by_finding(&db, "run-1", "nope").unwrap().is_none());
    }
}
