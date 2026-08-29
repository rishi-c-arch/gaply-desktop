//! The batch job store (migration v16, plan §11 D37/D38).
//!
//! A thesis audit is 50–250 sequential model calls at ~65 s each on CPU. That
//! is hours, in a desktop app the user can quit, update or crash. **Resume is
//! not robustness polish here; it is the feature**, and this module is shaped
//! entirely by it.
//!
//! # The one guarantee
//!
//! A completed item's result, its `status='done'`, and the job's `done_items`
//! increment are written in a SINGLE transaction. There is no window in which a
//! result exists without the status that retires it, so:
//!
//! | crash point | on restart |
//! |---|---|
//! | mid-inference | item is `running` with a NULL result → reset to `queued`, re-run, **one** result |
//! | after commit | item is `done` → never selected again |
//! | between the two | **cannot happen — there is no between** |
//!
//! "Zero duplicate results" therefore follows from the transaction boundary
//! rather than from anybody remembering to check.
//!
//! # Why resetting `running` is safe
//!
//! [`resume_job`] resets stale `running` items to `queued`. That is only
//! correct because the single-inference invariant (plan §7) means exactly ONE
//! runner exists — a second would reset an item the first was actively working.
//! The invariant is enforced in the app crate; this module documents its
//! dependence on it rather than assuming it.

use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};

use crate::db::Database;
use crate::now_epoch;
use crate::GaplyError;

/// What an item asks the engine to do. Stored as text, matched exhaustively.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ItemKind {
    /// An uncited sentence: does it need a citation?
    CitationNeed,
    /// A cited sentence whose source IS in the library, indexed and embedded.
    CitationSupport,
    /// A cited sentence whose source is NOT available to check against.
    ///
    /// **A result, not a failure** (§11 D40). It costs zero model calls, it is
    /// a true finding about the manuscript, and recording it as an error would
    /// both misreport the audit and retry something that cannot succeed.
    Unverifiable,
}

impl ItemKind {
    pub fn as_str(self) -> &'static str {
        match self {
            ItemKind::CitationNeed => "citation_need",
            ItemKind::CitationSupport => "citation_support",
            ItemKind::Unverifiable => "unverifiable",
        }
    }
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "citation_need" => Some(ItemKind::CitationNeed),
            "citation_support" => Some(ItemKind::CitationSupport),
            "unverifiable" => Some(ItemKind::Unverifiable),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JobStatus {
    Queued,
    Running,
    Done,
    Failed,
    Cancelled,
}

impl JobStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            JobStatus::Queued => "queued",
            JobStatus::Running => "running",
            JobStatus::Done => "done",
            JobStatus::Failed => "failed",
            JobStatus::Cancelled => "cancelled",
        }
    }
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "queued" => Some(JobStatus::Queued),
            "running" => Some(JobStatus::Running),
            "done" => Some(JobStatus::Done),
            "failed" => Some(JobStatus::Failed),
            "cancelled" => Some(JobStatus::Cancelled),
            _ => None,
        }
    }
}

/// One unit of work, as the planner produced it.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JobItem {
    pub id: i64,
    pub job_id: i64,
    pub seq: i64,
    pub kind: ItemKind,
    pub chunk_id: Option<i64>,
    pub page: Option<u32>,
    pub sentence: String,
    pub payload_json: String,
    pub status: String,
    pub attempts: i64,
    pub result_json: Option<String>,
    pub error: Option<String>,
}

/// An item as the planner hands it over, before it has an id.
#[derive(Debug, Clone)]
pub struct NewItem {
    pub seq: i64,
    pub kind: ItemKind,
    pub chunk_id: Option<i64>,
    pub page: Option<u32>,
    pub sentence: String,
    pub payload_json: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JobRow {
    pub id: i64,
    pub kind: String,
    pub status: JobStatus,
    pub document_id: Option<i64>,
    pub total_items: i64,
    pub done_items: i64,
    pub model_id: Option<String>,
    pub prompt_version: String,
    pub error: Option<String>,
    pub summary_json: Option<String>,
    pub created_at: i64,
    pub started_at: Option<i64>,
    pub finished_at: Option<i64>,
}

/// Create a job and ALL of its items in one transaction.
///
/// Atomic by decision: a job whose item rows are half-written is a job that
/// would resume into a silently short audit. Either the whole plan is durable
/// or none of it is.
pub fn create_job(
    db: &Database,
    kind: &str,
    document_id: Option<i64>,
    prompt_version: &str,
    items: &[NewItem],
) -> Result<i64, GaplyError> {
    let now = now_epoch();
    let mut conn = db.conn()?;
    let tx = conn.transaction()?;
    tx.execute(
        "INSERT INTO ai_jobs (kind, status, document_id, total_items, done_items,
                              prompt_version, created_at)
         VALUES (?1, 'queued', ?2, ?3, 0, ?4, ?5)",
        params![kind, document_id, items.len() as i64, prompt_version, now],
    )?;
    let job_id = tx.last_insert_rowid();
    {
        let mut stmt = tx.prepare(
            "INSERT INTO ai_job_items
                 (job_id, seq, kind, chunk_id, page, sentence, payload_json, status, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'queued', ?8)",
        )?;
        for it in items {
            stmt.execute(params![
                job_id,
                it.seq,
                it.kind.as_str(),
                it.chunk_id,
                it.page.map(i64::from),
                it.sentence,
                it.payload_json,
                now,
            ])?;
        }
    }
    tx.commit()?;
    Ok(job_id)
}

pub fn get_job(db: &Database, job_id: i64) -> Result<Option<JobRow>, GaplyError> {
    let conn = db.conn()?;
    let row = conn
        .query_row(
            "SELECT id, kind, status, document_id, total_items, done_items, model_id,
                    prompt_version, error, summary_json, created_at, started_at, finished_at
             FROM ai_jobs WHERE id = ?1",
            params![job_id],
            |r| {
                Ok(JobRow {
                    id: r.get(0)?,
                    kind: r.get(1)?,
                    status: JobStatus::parse(&r.get::<_, String>(2)?).unwrap_or(JobStatus::Failed),
                    document_id: r.get(3)?,
                    total_items: r.get(4)?,
                    done_items: r.get(5)?,
                    model_id: r.get(6)?,
                    prompt_version: r.get(7)?,
                    error: r.get(8)?,
                    summary_json: r.get(9)?,
                    created_at: r.get(10)?,
                    started_at: r.get(11)?,
                    finished_at: r.get(12)?,
                })
            },
        )
        .optional()?;
    Ok(row)
}

pub fn set_job_status(db: &Database, job_id: i64, status: JobStatus) -> Result<(), GaplyError> {
    let conn = db.conn()?;
    let now = now_epoch();
    match status {
        JobStatus::Running => conn.execute(
            "UPDATE ai_jobs SET status = ?2,
                                started_at = COALESCE(started_at, ?3)
             WHERE id = ?1",
            params![job_id, status.as_str(), now],
        )?,
        JobStatus::Done | JobStatus::Failed | JobStatus::Cancelled => conn.execute(
            "UPDATE ai_jobs SET status = ?2, finished_at = ?3 WHERE id = ?1",
            params![job_id, status.as_str(), now],
        )?,
        JobStatus::Queued => conn.execute(
            "UPDATE ai_jobs SET status = ?2 WHERE id = ?1",
            params![job_id, status.as_str()],
        )?,
    };
    Ok(())
}

pub fn set_job_summary(db: &Database, job_id: i64, summary_json: &str) -> Result<(), GaplyError> {
    let conn = db.conn()?;
    conn.execute(
        "UPDATE ai_jobs SET summary_json = ?2 WHERE id = ?1",
        params![job_id, summary_json],
    )?;
    Ok(())
}

/// Jobs left `running` by a crash — what startup has to pick back up.
pub fn interrupted_jobs(db: &Database) -> Result<Vec<i64>, GaplyError> {
    let conn = db.conn()?;
    let mut stmt =
        conn.prepare("SELECT id FROM ai_jobs WHERE status = 'running' ORDER BY created_at")?;
    let ids = stmt.query_map([], |r| r.get(0))?.collect::<Result<Vec<i64>, _>>()?;
    Ok(ids)
}

/// Prepare a job to continue: any item a crash left `running` goes back to
/// `queued`.
///
/// Safe ONLY because one runner exists (plan §7). Returns how many were reset,
/// which is the honest measure of what the crash cost — at ~65 s an item, a
/// number worth reporting rather than swallowing.
pub fn resume_job(db: &Database, job_id: i64) -> Result<usize, GaplyError> {
    let conn = db.conn()?;
    let n = conn.execute(
        "UPDATE ai_job_items SET status = 'queued'
         WHERE job_id = ?1 AND status = 'running'",
        params![job_id],
    )?;
    Ok(n)
}

/// Claim the next item, atomically.
///
/// The UPDATE *is* the lock: `WHERE status = 'queued'` means a zero-row result
/// is "somebody else took it", not an error. Ordered by `seq` so a resumed job
/// continues where it stopped rather than wherever the query planner fancies.
pub fn claim_next_item(db: &Database, job_id: i64) -> Result<Option<JobItem>, GaplyError> {
    let mut conn = db.conn()?;
    let tx = conn.transaction()?;
    let next: Option<i64> = tx
        .query_row(
            "SELECT id FROM ai_job_items
             WHERE job_id = ?1 AND status = 'queued'
             ORDER BY seq LIMIT 1",
            params![job_id],
            |r| r.get(0),
        )
        .optional()?;
    let Some(item_id) = next else {
        tx.commit()?;
        return Ok(None);
    };
    let updated = tx.execute(
        "UPDATE ai_job_items SET status = 'running', attempts = attempts + 1
         WHERE id = ?1 AND status = 'queued'",
        params![item_id],
    )?;
    if updated == 0 {
        tx.commit()?;
        return Ok(None);
    }
    let item = read_item(&tx, item_id)?;
    tx.commit()?;
    Ok(Some(item))
}

fn read_item(conn: &rusqlite::Connection, item_id: i64) -> Result<JobItem, GaplyError> {
    let item = conn.query_row(
        "SELECT id, job_id, seq, kind, chunk_id, page, sentence, payload_json,
                status, attempts, result_json, error
         FROM ai_job_items WHERE id = ?1",
        params![item_id],
        |r| {
            Ok(JobItem {
                id: r.get(0)?,
                job_id: r.get(1)?,
                seq: r.get(2)?,
                kind: ItemKind::parse(&r.get::<_, String>(3)?).unwrap_or(ItemKind::Unverifiable),
                chunk_id: r.get(4)?,
                page: r.get::<_, Option<i64>>(5)?.map(|p| p as u32),
                sentence: r.get(6)?,
                payload_json: r.get(7)?,
                status: r.get(8)?,
                attempts: r.get(9)?,
                result_json: r.get(10)?,
                error: r.get(11)?,
            })
        },
    )?;
    Ok(item)
}

/// Retire an item and advance the job — **in ONE transaction**.
///
/// This is the entire crash-safety story (§11 D38). Splitting it into "write
/// the result" and "mark it done" would create the one window in which a
/// duplicate could be produced.
pub fn complete_item(
    db: &Database,
    item_id: i64,
    status: &str,
    result_json: Option<&str>,
    error: Option<&str>,
) -> Result<(), GaplyError> {
    debug_assert!(matches!(status, "done" | "failed" | "skipped"), "not a terminal status");
    let now = now_epoch();
    let mut conn = db.conn()?;
    let tx = conn.transaction()?;
    tx.execute(
        "UPDATE ai_job_items
         SET status = ?2, result_json = ?3, error = ?4, finished_at = ?5
         WHERE id = ?1",
        params![item_id, status, result_json, error, now],
    )?;
    // done_items counts items that will never run again, which is what a
    // progress bar means and what resume relies on.
    tx.execute(
        "UPDATE ai_jobs
         SET done_items = (
             SELECT COUNT(*) FROM ai_job_items
             WHERE job_id = ai_jobs.id AND status IN ('done','failed','skipped')
         )
         WHERE id = (SELECT job_id FROM ai_job_items WHERE id = ?1)",
        params![item_id],
    )?;
    tx.commit()?;
    Ok(())
}

/// Results, paginated. The streaming requirement: readable MID-job.
pub fn job_results(
    db: &Database,
    job_id: i64,
    offset: i64,
    limit: i64,
) -> Result<Vec<JobItem>, GaplyError> {
    let conn = db.conn()?;
    let mut stmt = conn.prepare(
        "SELECT id, job_id, seq, kind, chunk_id, page, sentence, payload_json,
                status, attempts, result_json, error
         FROM ai_job_items
         WHERE job_id = ?1
         ORDER BY seq
         LIMIT ?3 OFFSET ?2",
    )?;
    let rows = stmt
        .query_map(params![job_id, offset, limit], |r| {
            Ok(JobItem {
                id: r.get(0)?,
                job_id: r.get(1)?,
                seq: r.get(2)?,
                kind: ItemKind::parse(&r.get::<_, String>(3)?).unwrap_or(ItemKind::Unverifiable),
                chunk_id: r.get(4)?,
                page: r.get::<_, Option<i64>>(5)?.map(|p| p as u32),
                sentence: r.get(6)?,
                payload_json: r.get(7)?,
                status: r.get(8)?,
                attempts: r.get(9)?,
                result_json: r.get(10)?,
                error: r.get(11)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// How many items are left to run. Zero means the job is finished.
pub fn remaining_items(db: &Database, job_id: i64) -> Result<i64, GaplyError> {
    let conn = db.conn()?;
    let n = conn.query_row(
        "SELECT COUNT(*) FROM ai_job_items
         WHERE job_id = ?1 AND status IN ('queued','running')",
        params![job_id],
        |r| r.get(0),
    )?;
    Ok(n)
}

/// Per-kind, per-status tallies for the summary — computed by SQL rather than
/// by re-deriving from results in memory.
pub fn item_tallies(db: &Database, job_id: i64) -> Result<Vec<(String, String, i64)>, GaplyError> {
    let conn = db.conn()?;
    let mut stmt = conn.prepare(
        "SELECT kind, status, COUNT(*) FROM ai_job_items
         WHERE job_id = ?1 GROUP BY kind, status ORDER BY kind, status",
    )?;
    let rows = stmt
        .query_map(params![job_id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn db() -> Database {
        Database::in_memory().unwrap()
    }

    fn items(n: i64) -> Vec<NewItem> {
        (0..n)
            .map(|seq| NewItem {
                seq,
                kind: if seq % 2 == 0 { ItemKind::CitationNeed } else { ItemKind::CitationSupport },
                chunk_id: None,
                page: Some(1),
                sentence: format!("sentence {seq}."),
                payload_json: "{}".into(),
            })
            .collect()
    }

    fn seeded(n: i64) -> (Database, i64) {
        let db = db();
        let job = create_job(&db, "thesis_audit", None, "citation_support-v1.4", &items(n)).unwrap();
        (db, job)
    }

    /// THE crash-resume proof (§11 D38).
    ///
    /// Runs three items, then simulates a kill in the middle of the fourth —
    /// which is exactly the state a crash leaves: `running`, no result. Restart
    /// resets it, the job continues from there, and every item ends with
    /// EXACTLY ONE result.
    #[test]
    fn a_killed_runner_resumes_from_the_first_unfinished_item_with_no_duplicates() {
        let (db, job) = seeded(6);
        set_job_status(&db, job, JobStatus::Running).unwrap();

        // three complete cleanly
        for _ in 0..3 {
            let it = claim_next_item(&db, job).unwrap().expect("an item");
            complete_item(&db, it.id, "done", Some(&format!(r#"{{"seq":{}}}"#, it.seq)), None)
                .unwrap();
        }
        // the fourth is claimed and then the process dies — no completion call
        let killed = claim_next_item(&db, job).unwrap().expect("a fourth item");
        assert_eq!(killed.seq, 3);
        assert_eq!(get_job(&db, job).unwrap().unwrap().done_items, 3);

        // ---- restart ----
        assert_eq!(interrupted_jobs(&db).unwrap(), vec![job], "the crash was not detectable");
        let reset = resume_job(&db, job).unwrap();
        assert_eq!(reset, 1, "the in-flight item was not returned to the queue");

        // the resumed runner picks up the SAME item, not the one after it
        let resumed = claim_next_item(&db, job).unwrap().expect("resumed item");
        assert_eq!(resumed.seq, 3, "resume skipped the interrupted item");
        assert_eq!(resumed.attempts, 2, "attempts must record that this item ran twice");
        complete_item(&db, resumed.id, "done", Some(r#"{"seq":3}"#), None).unwrap();

        // drain the rest
        while let Some(it) = claim_next_item(&db, job).unwrap() {
            complete_item(&db, it.id, "done", Some(r#"{}"#), None).unwrap();
        }

        // ---- the guarantee ----
        let all = job_results(&db, job, 0, 100).unwrap();
        assert_eq!(all.len(), 6, "item count changed across a crash");
        assert!(all.iter().all(|i| i.status == "done"));
        assert!(all.iter().all(|i| i.result_json.is_some()), "an item finished with no result");
        let seqs: Vec<i64> = all.iter().map(|i| i.seq).collect();
        assert_eq!(seqs, vec![0, 1, 2, 3, 4, 5], "duplicate or missing items after resume");
        assert_eq!(remaining_items(&db, job).unwrap(), 0);
        assert_eq!(get_job(&db, job).unwrap().unwrap().done_items, 6);
    }

    /// The claim IS the lock: a second claimant cannot take the same item.
    #[test]
    fn claiming_is_atomic_so_two_runners_cannot_take_one_item() {
        let (db, job) = seeded(2);
        let a = claim_next_item(&db, job).unwrap().unwrap();
        let b = claim_next_item(&db, job).unwrap().unwrap();
        assert_ne!(a.id, b.id, "the same item was handed out twice");
        assert!(claim_next_item(&db, job).unwrap().is_none(), "claimed an item that was not queued");
    }

    /// Results must be readable WHILE the job runs — that is the streaming
    /// requirement, and a report that only exists at the end is not one.
    #[test]
    fn results_are_queryable_mid_job() {
        let (db, job) = seeded(5);
        let it = claim_next_item(&db, job).unwrap().unwrap();
        complete_item(&db, it.id, "done", Some(r#"{"verdict":"weak"}"#), None).unwrap();

        let page = job_results(&db, job, 0, 2).unwrap();
        assert_eq!(page.len(), 2);
        assert_eq!(page[0].result_json.as_deref(), Some(r#"{"verdict":"weak"}"#));
        assert!(page[1].result_json.is_none(), "an unrun item must have no result");
        assert_eq!(job_results(&db, job, 3, 10).unwrap().len(), 2, "offset paging is wrong");
    }

    /// A failed item is still retired — otherwise a permanently failing item
    /// would be re-run forever and the job would never finish.
    #[test]
    fn a_failed_item_is_retired_not_retried_forever() {
        let (db, job) = seeded(2);
        let it = claim_next_item(&db, job).unwrap().unwrap();
        complete_item(&db, it.id, "failed", None, Some("the model produced nothing legal")).unwrap();
        assert_eq!(resume_job(&db, job).unwrap(), 0, "a failed item was resurrected by resume");
        let next = claim_next_item(&db, job).unwrap().unwrap();
        assert_ne!(next.id, it.id);
        assert_eq!(get_job(&db, job).unwrap().unwrap().done_items, 1, "failed items count as done");
    }

    /// `unverifiable` is a RESULT (§11 D40): it retires with a recorded reason
    /// and never reaches the model.
    #[test]
    fn an_unverifiable_item_completes_without_an_error() {
        let db = db();
        let job = create_job(
            &db,
            "thesis_audit",
            None,
            "citation_support-v1.4",
            &[NewItem {
                seq: 0,
                kind: ItemKind::Unverifiable,
                chunk_id: None,
                page: Some(3),
                sentence: "Organic management increases richness (Smith, 2019).".into(),
                payload_json: r#"{"marker":"(Smith, 2019)"}"#.into(),
            }],
        )
        .unwrap();
        let it = claim_next_item(&db, job).unwrap().unwrap();
        assert_eq!(it.kind, ItemKind::Unverifiable);
        complete_item(
            &db,
            it.id,
            "done",
            Some(r#"{"category":"unverifiable","reason":"source not in library"}"#),
            None,
        )
        .unwrap();
        let stored = &job_results(&db, job, 0, 10).unwrap()[0];
        assert_eq!(stored.status, "done", "unverifiable must not be a failure");
        assert!(stored.error.is_none(), "unverifiable must not record an error");
    }

    #[test]
    fn creating_a_job_is_atomic_in_its_items() {
        let (db, job) = seeded(4);
        let j = get_job(&db, job).unwrap().unwrap();
        assert_eq!(j.total_items, 4);
        assert_eq!(j.done_items, 0);
        assert_eq!(j.status, JobStatus::Queued);
        assert_eq!(job_results(&db, job, 0, 100).unwrap().len(), 4);
    }

    #[test]
    fn tallies_group_by_kind_and_status() {
        let (db, job) = seeded(4);
        let it = claim_next_item(&db, job).unwrap().unwrap();
        complete_item(&db, it.id, "done", Some("{}"), None).unwrap();
        let t = item_tallies(&db, job).unwrap();
        assert!(t.contains(&("citation_need".to_string(), "done".to_string(), 1)));
        assert!(t.contains(&("citation_need".to_string(), "queued".to_string(), 1)));
        assert!(t.contains(&("citation_support".to_string(), "queued".to_string(), 2)));
    }
}
