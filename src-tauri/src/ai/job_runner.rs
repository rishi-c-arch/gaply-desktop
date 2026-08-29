//! The batch job runner (plan §11 D38/D39).
//!
//! An audit is 50–250 sequential model calls at ~65 s each. Everything here is
//! shaped by the fact that the user can quit, cancel, or want the model for
//! something else while it runs.
//!
//! # Preemption is a counter, not semaphore fairness (§11 D39)
//!
//! The tempting implementation is to lean on `tokio::sync::Semaphore` being
//! FIFO: an interactive request queues, the runner's next item queues behind
//! it, the human wins. That is probably true and is a bad thing to build a
//! user-visible guarantee on — it depends on a dependency's queueing
//! discipline, it is invisible at the call site, and it cannot be tested
//! without racing.
//!
//! Instead [`InteractivePriority`] counts in-flight interactive requests. The
//! runner checks it **between items** and waits while it is non-zero.
//!
//! **A job never yields mid-inference.** Killing a half-finished generation
//! throws away the 60+ seconds already spent and produces nothing; the human
//! waits at most one item.

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;

use gaply_core::ai_engine::jobs::{self, ItemKind, JobItem, JobStatus};
use gaply_core::Database;
use serde::Serialize;

use crate::ai::model_manager::ModelManager;

/// How many interactive requests are in flight. The runner waits while > 0.
#[derive(Clone, Default)]
pub struct InteractivePriority(Arc<AtomicUsize>);

/// Held for the lifetime of an interactive request; decrements on drop so an
/// early return or a `?` cannot strand the counter above zero and wedge every
/// job on the machine.
pub struct InteractiveGuard(Arc<AtomicUsize>);

impl Drop for InteractiveGuard {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
    }
}

impl InteractivePriority {
    pub fn new() -> Self {
        Self::default()
    }
    /// Take priority. Acquire this BEFORE touching the inference gate, so the
    /// window is never narrower than the request itself.
    pub fn acquire(&self) -> InteractiveGuard {
        self.0.fetch_add(1, Ordering::SeqCst);
        InteractiveGuard(self.0.clone())
    }
    pub fn busy(&self) -> bool {
        self.0.load(Ordering::SeqCst) > 0
    }
}

/// Controls for one running job.
#[derive(Clone)]
pub struct JobControl {
    pub job_id: i64,
    /// Honored between items AND handed to the model as the per-token cancel,
    /// so a cancel lands within one decode step rather than one item.
    pub cancel: Arc<AtomicBool>,
    pub paused: Arc<AtomicBool>,
}

impl JobControl {
    pub fn new(job_id: i64) -> Self {
        Self {
            job_id,
            cancel: Arc::new(AtomicBool::new(false)),
            paused: Arc::new(AtomicBool::new(false)),
        }
    }
    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::SeqCst);
    }
    pub fn pause(&self) {
        self.paused.store(true, Ordering::SeqCst);
    }
    pub fn resume(&self) {
        self.paused.store(false, Ordering::SeqCst);
    }
    pub fn is_cancelled(&self) -> bool {
        self.cancel.load(Ordering::SeqCst)
    }
    pub fn is_paused(&self) -> bool {
        self.paused.load(Ordering::SeqCst)
    }
}

/// Streamed after every completed item.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JobProgress {
    pub job_id: i64,
    pub completed: i64,
    pub total: i64,
    pub current_category: String,
    pub latest_item_summary: String,
}

/// Why a run stopped. All three are normal; none is an error.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunOutcome {
    Completed,
    Cancelled,
    Paused,
}

/// One item's outcome, before it is written.
struct ItemOutcome {
    status: &'static str,
    result_json: String,
    error: Option<String>,
    summary: String,
}

/// Run a job to completion, cancellation, or pause.
///
/// `emit` is called after each completed item. It is a plain closure rather
/// than a Tauri type so the runner is testable without an app handle.
/// Embeds a claim for retrieval. Injected rather than reached for: it keeps the
/// runner testable without a 134 MB model, and it makes "no embedder installed"
/// a value the caller supplies rather than a global the runner hopes for.
pub type QueryEmbedder<'a> = &'a (dyn Fn(&str) -> Result<Vec<f32>, gaply_core::GaplyError> + Send + Sync);

#[allow(clippy::too_many_arguments)]
pub async fn run_job(
    db: &Database,
    manager: &ModelManager,
    control: &JobControl,
    priority: &InteractivePriority,
    embed_query: QueryEmbedder<'_>,
    emit: &(dyn Fn(JobProgress) + Send + Sync),
) -> Result<RunOutcome, gaply_core::GaplyError> {
    let job_id = control.job_id;
    jobs::set_job_status(db, job_id, JobStatus::Running)?;

    loop {
        if control.is_cancelled() {
            jobs::set_job_status(db, job_id, JobStatus::Cancelled)?;
            return Ok(RunOutcome::Cancelled);
        }
        if control.is_paused() {
            // Paused jobs stay 'running' in the DB on purpose: a pause is not a
            // finished state, and marking it otherwise would make the next
            // startup treat a deliberate pause as a crash.
            return Ok(RunOutcome::Paused);
        }

        // §11 D39 — PREEMPTION POINT. Between items only.
        while priority.busy() {
            if control.is_cancelled() {
                jobs::set_job_status(db, job_id, JobStatus::Cancelled)?;
                return Ok(RunOutcome::Cancelled);
            }
            tokio::time::sleep(std::time::Duration::from_millis(25)).await;
        }

        let Some(item) = jobs::claim_next_item(db, job_id)? else {
            jobs::set_job_status(db, job_id, JobStatus::Done)?;
            return Ok(RunOutcome::Completed);
        };

        let outcome = run_item(db, manager, &item, control, embed_query).await;

        // A cancel that landed DURING the item: the item is retired with what
        // it produced, then the job stops. Discarding the work would mean
        // re-running 60 s of inference for nothing.
        jobs::complete_item(
            db,
            item.id,
            outcome.status,
            Some(&outcome.result_json),
            outcome.error.as_deref(),
        )?;

        let job = jobs::get_job(db, job_id)?;
        let (completed, total) = job.map(|j| (j.done_items, j.total_items)).unwrap_or((0, 0));
        emit(JobProgress {
            job_id,
            completed,
            total,
            current_category: item.kind.as_str().to_string(),
            latest_item_summary: outcome.summary,
        });
    }
}

/// Execute one item. Never panics; a failure is a recorded result.
async fn run_item(
    db: &Database,
    manager: &ModelManager,
    item: &JobItem,
    control: &JobControl,
    embed_query: QueryEmbedder<'_>,
) -> ItemOutcome {
    match item.kind {
        // §11 D40 — a RESULT, and it costs zero model calls.
        ItemKind::Unverifiable => {
            let reason = serde_json::from_str::<serde_json::Value>(&item.payload_json)
                .ok()
                .and_then(|v| v.get("reason").and_then(|r| r.as_str()).map(str::to_string))
                .unwrap_or_else(|| "source not in library".to_string());
            ItemOutcome {
                status: "done",
                result_json: serde_json::json!({
                    "category": "unverifiable",
                    "reason": reason,
                })
                .to_string(),
                error: None,
                summary: format!("unverifiable: {reason}"),
            }
        }
        ItemKind::CitationNeed => run_citation_need(manager, item, control).await,
        ItemKind::CitationSupport => {
            run_citation_support(db, manager, item, control, embed_query).await
        }
    }
}

async fn run_citation_need(
    manager: &ModelManager,
    item: &JobItem,
    control: &JobControl,
) -> ItemOutcome {
    use crate::ai::task::run_task;
    use crate::ai::tasks::citation_need::{CitationNeedInput, CitationNeedTask};

    let task = CitationNeedTask::new(CitationNeedInput {
        sentence: item.sentence.clone(),
        preceding_sentence: String::new(),
        following_sentence: String::new(),
        section: String::new(),
    });
    // The job's cancel IS the per-token cancel: a cancel lands within one
    // decode step rather than waiting out the item.
    match run_task(manager, &task, &Default::default(), control.cancel.clone(), None).await {
        Ok(run) => {
            let out = serde_json::to_value(&run.output).unwrap_or(serde_json::Value::Null);
            let summary = out
                .get("needs_citation")
                .map(|v| format!("needs_citation={v}"))
                .unwrap_or_else(|| "ok".into());
            ItemOutcome {
                status: "done",
                result_json: serde_json::json!({
                    "category": "citation_need",
                    "output": out,
                    "retried": run.retried,
                    "stopReasons": run.stop_reasons,
                })
                .to_string(),
                error: None,
                summary,
            }
        }
        Err(e) => failed_outcome("citation_need", e),
    }
}

async fn run_citation_support(
    db: &Database,
    manager: &ModelManager,
    item: &JobItem,
    control: &JobControl,
    embed_query: QueryEmbedder<'_>,
) -> ItemOutcome {
    use crate::ai::evidence::{assemble, Assembled, EVIDENCE_BUDGET_TOKENS};
    use crate::ai::task::run_task;
    use crate::ai::tasks::citation_support::CitationSupportTask;

    let payload: serde_json::Value =
        serde_json::from_str(&item.payload_json).unwrap_or(serde_json::Value::Null);
    let document_id = payload.get("documentId").and_then(|v| v.as_i64()).unwrap_or(-1);
    let cited_source =
        payload.get("citedSource").and_then(|v| v.as_str()).unwrap_or("unknown source");

    // Retrieval needs a query vector. The engine's embedder is loaded
    // separately; when it is absent this is NOT a model failure, it is a
    // missing precondition, and it is recorded as such.
    let query_vector = match embed_query(&item.sentence) {
        Ok(v) => v,
        Err(e) => {
            // NOT a model failure: a missing precondition. Recorded as skipped
            // so it is visibly different from "the model got it wrong".
            return ItemOutcome {
                status: "skipped",
                result_json: serde_json::json!({
                    "category": "citation_support",
                    "outcome": "noEmbedder",
                    "reason": e.to_string(),
                })
                .to_string(),
                error: None,
                summary: "skipped: evidence could not be retrieved".into(),
            };
        }
    };

    let bundle = match assemble(db, document_id, &item.sentence, &query_vector, EVIDENCE_BUDGET_TOKENS)
    {
        Ok(Assembled::Ready(b)) => b,
        Ok(Assembled::NoEvidence { reason }) => {
            // Short-circuit: NO generation, no persistence (plan §11 D15).
            return ItemOutcome {
                status: "done",
                result_json: serde_json::json!({
                    "category": "citation_support",
                    "outcome": "noEvidence",
                    "reason": reason,
                })
                .to_string(),
                error: None,
                summary: format!("no evidence: {reason}"),
            };
        }
        Err(e) => return failed_outcome("citation_support", e),
    };

    let task = CitationSupportTask {
        claim: item.sentence.clone(),
        cited_source: cited_source.to_string(),
        evidence: bundle.rendered.clone(),
    };
    match run_task(manager, &task, &bundle.ctx, control.cancel.clone(), None).await {
        Ok(run) => {
            let out = serde_json::to_value(&run.output).unwrap_or(serde_json::Value::Null);
            let verdict =
                out.get("verdict").and_then(|v| v.as_str()).unwrap_or("?").to_string();
            ItemOutcome {
                status: "done",
                result_json: serde_json::json!({
                    "category": "citation_support",
                    "outcome": "ok",
                    "output": out,
                    "chunksSent": bundle.chunks_sent,
                    "page": item.page,
                    "retried": run.retried,
                    "stopReasons": run.stop_reasons,
                })
                .to_string(),
                error: None,
                summary: format!("verdict={verdict}"),
            }
        }
        Err(e) => failed_outcome("citation_support", e),
    }
}

fn failed_outcome<E: std::fmt::Display>(category: &str, e: E) -> ItemOutcome {
    let msg = e.to_string();
    ItemOutcome {
        status: "failed",
        result_json: serde_json::json!({ "category": category, "outcome": "error" }).to_string(),
        error: Some(msg.clone()),
        summary: format!("failed: {msg}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::generative::{GenOutput, GenRequest, GenerationBackend, RamEstimate, StopReason};
    use crate::ai::model_manager::BackendLoader;
    use gaply_core::ai_engine::jobs::{create_job, job_results, NewItem};
    use gaply_core::GaplyError;
    use std::sync::atomic::AtomicU32;
    use std::sync::Mutex as StdMutex;

    /// Counts generations so "never reached the model" is provable, and can
    /// stall so a cancel has something to interrupt.
    #[derive(Default)]
    struct Stats {
        calls: AtomicU32,
    }

    struct MockBackend {
        stats: Arc<Stats>,
        reply: String,
        /// Honour the per-token cancel like the real backend does.
        stall_ms: u64,
    }

    impl GenerationBackend for MockBackend {
        fn generate(&self, req: GenRequest<'_>) -> Result<GenOutput, GaplyError> {
            self.stats.calls.fetch_add(1, Ordering::SeqCst);
            // Poll the cancel exactly as the real decode loop does.
            let steps = (self.stall_ms / 5).max(1);
            for _ in 0..steps {
                if req.cancel.load(Ordering::SeqCst) {
                    return Err(GaplyError::Cancelled);
                }
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
            Ok(GenOutput {
                text: self.reply.clone(),
                tokens: 1,
                stop_reason: StopReason::EndOfTurn,
                elapsed_ms: 1,
                prompt_tokens: 1,
                prefill_ms: 1,
                decode_ms: 1,
                decode_tokens_per_sec: 1.0,
            })
        }
    }

    struct MockLoader {
        stats: Arc<Stats>,
        reply: String,
        stall_ms: u64,
    }

    impl BackendLoader for MockLoader {
        fn model_id(&self) -> String {
            "mock".into()
        }
        fn ram_estimate(&self) -> Result<RamEstimate, GaplyError> {
            Ok(RamEstimate {
                weights_bytes: 1,
                kv_cache_bytes: 1,
                total_bytes: 2,
                context_length: 1,
                model_max_context: 1,
                layers: 1,
                kv_heads: 1,
                head_dim: 1,
            })
        }
        fn load(&self) -> Result<Arc<dyn GenerationBackend>, GaplyError> {
            Ok(Arc::new(MockBackend {
                stats: self.stats.clone(),
                reply: self.reply.clone(),
                stall_ms: self.stall_ms,
            }))
        }
    }

    /// A legal citation_need reply, so items complete rather than fail.
    const NEED_OK: &str = r#"{"needs_citation":true,"sentence_type":"empirical_claim","severity":"high","reason":"states an empirical finding without attribution","search_query":"organic management soil invertebrate richness"}"#;

    fn manager(stats: Arc<Stats>, stall_ms: u64) -> ModelManager {
        ModelManager::new(Arc::new(MockLoader {
            stats,
            reply: NEED_OK.to_string(),
            stall_ms,
        }))
    }

    fn need_items(n: i64) -> Vec<NewItem> {
        (0..n)
            .map(|seq| NewItem {
                seq,
                kind: ItemKind::CitationNeed,
                chunk_id: None,
                page: Some(1),
                sentence: format!("Organic management increased soil richness markedly, case {seq}."),
                payload_json: "{}".into(),
            })
            .collect()
    }

    fn no_embedder(_: &str) -> Result<Vec<f32>, GaplyError> {
        Err(GaplyError::Validation("no embedding model is installed".into()))
    }

    fn seeded(n: i64) -> (Database, i64) {
        let db = Database::in_memory().unwrap();
        let job =
            create_job(&db, "thesis_audit", None, "citation_need-v2", &need_items(n)).unwrap();
        (db, job)
    }

    #[tokio::test]
    async fn a_job_runs_every_item_and_finishes_done() {
        let (db, job) = seeded(3);
        let stats = Arc::new(Stats::default());
        let m = manager(stats.clone(), 0);
        let control = JobControl::new(job);
        let seen = Arc::new(StdMutex::new(Vec::new()));
        let seen2 = seen.clone();

        let outcome = run_job(
            &db,
            &m,
            &control,
            &InteractivePriority::new(),
            &no_embedder,
            &move |p| seen2.lock().unwrap().push(p),
        )
        .await
        .unwrap();

        assert_eq!(outcome, RunOutcome::Completed);
        assert_eq!(stats.calls.load(Ordering::SeqCst), 3);
        assert_eq!(jobs::get_job(&db, job).unwrap().unwrap().status, JobStatus::Done);
        // one progress event per completed item, counting up
        let events = seen.lock().unwrap();
        assert_eq!(events.len(), 3);
        assert_eq!(events.iter().map(|e| e.completed).collect::<Vec<_>>(), vec![1, 2, 3]);
        assert!(events.iter().all(|e| e.total == 3));
    }

    /// Cancel BETWEEN items: the job stops and is recorded cancelled, and the
    /// items it never reached stay queued rather than being lost.
    #[tokio::test]
    async fn cancelling_between_items_stops_the_job_and_leaves_the_rest_queued() {
        let (db, job) = seeded(5);
        let stats = Arc::new(Stats::default());
        let m = manager(stats.clone(), 0);
        let control = JobControl::new(job);
        let ctl = control.clone();
        let calls = stats.clone();

        let outcome = run_job(
            &db,
            &m,
            &control,
            &InteractivePriority::new(),
            &no_embedder,
            &move |_| {
                // cancel as soon as the first item has been retired
                if calls.calls.load(Ordering::SeqCst) >= 1 {
                    ctl.cancel();
                }
            },
        )
        .await
        .unwrap();

        assert_eq!(outcome, RunOutcome::Cancelled);
        assert_eq!(jobs::get_job(&db, job).unwrap().unwrap().status, JobStatus::Cancelled);
        let items = job_results(&db, job, 0, 100).unwrap();
        assert_eq!(items.iter().filter(|i| i.status == "done").count(), 1);
        assert_eq!(
            items.iter().filter(|i| i.status == "queued").count(),
            4,
            "cancelled work must remain queued so a resume can finish it"
        );
    }

    /// Cancel MID-ITEM: the job's cancel is the model's per-token cancel, so it
    /// lands inside the generation rather than after it. The interrupted item
    /// is retired as failed — with its reason — and never silently re-queued.
    #[tokio::test]
    async fn cancelling_mid_item_interrupts_the_generation_itself() {
        let (db, job) = seeded(3);
        let stats = Arc::new(Stats::default());
        // long enough that the cancel below lands inside the first generation
        let m = manager(stats.clone(), 2000);
        let control = JobControl::new(job);
        let ctl = control.clone();

        tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(80)).await;
            ctl.cancel();
        });

        let started = std::time::Instant::now();
        let outcome =
            run_job(&db, &m, &control, &InteractivePriority::new(), &no_embedder, &|_| {})
                .await
                .unwrap();
        let elapsed = started.elapsed();

        assert_eq!(outcome, RunOutcome::Cancelled);
        assert!(
            elapsed < std::time::Duration::from_millis(1500),
            "the cancel waited for the whole generation ({elapsed:?}) instead of interrupting it"
        );
        assert_eq!(stats.calls.load(Ordering::SeqCst), 1, "no item started after the cancel");
        let items = job_results(&db, job, 0, 100).unwrap();
        let first = &items[0];
        assert_eq!(first.status, "failed", "the interrupted item must be retired, not left running");
        assert!(first.error.is_some(), "an interrupted item must record why");
    }

    /// §11 D39. An interactive request holds priority; the runner must not
    /// start the next item until it is released — and must not abandon the job.
    #[tokio::test]
    async fn an_interactive_request_preempts_the_job_between_items() {
        let (db, job) = seeded(4);
        let stats = Arc::new(Stats::default());
        let m = manager(stats.clone(), 0);
        let control = JobControl::new(job);
        let priority = InteractivePriority::new();

        // the human arrives before the runner starts
        let guard = priority.acquire();
        assert!(priority.busy());

        // Two concurrent futures rather than a spawned task: neither Database
        // nor ModelManager is Clone, and join! needs no shared ownership.
        let observed = Arc::new(AtomicU32::new(u32::MAX));
        let observed2 = observed.clone();
        let calls = stats.clone();
        let releaser = async move {
            tokio::time::sleep(std::time::Duration::from_millis(150)).await;
            // snapshot BEFORE releasing: this is the assertion that matters
            observed2.store(calls.calls.load(Ordering::SeqCst), Ordering::SeqCst);
            drop(guard);
        };
        let runner = run_job(&db, &m, &control, &priority, &no_embedder, &|_| {});
        let (outcome, ()) = tokio::join!(runner, releaser);

        assert_eq!(
            observed.load(Ordering::SeqCst),
            0,
            "the job ran an item while an interactive request was in flight"
        );
        assert_eq!(outcome.unwrap(), RunOutcome::Completed, "the job did not resume after preemption");
        assert_eq!(stats.calls.load(Ordering::SeqCst), 4);
    }

    /// The guard decrements on drop even when the request unwinds, so a
    /// panicking interactive command cannot wedge every job on the machine.
    #[test]
    fn a_dropped_priority_guard_always_releases() {
        let p = InteractivePriority::new();
        {
            let _g = p.acquire();
            assert!(p.busy());
        }
        assert!(!p.busy());

        let p2 = p.clone();
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _g = p2.acquire();
            panic!("interactive request blew up");
        }));
        assert!(!p.busy(), "a panicking request stranded the priority counter");
    }

    /// §11 D40. An unverifiable item is a RESULT: it retires cleanly and the
    /// model is never asked.
    #[tokio::test]
    async fn an_unverifiable_item_never_reaches_the_model() {
        let db = Database::in_memory().unwrap();
        let job = create_job(
            &db,
            "thesis_audit",
            None,
            "citation_support-v1.4",
            &[NewItem {
                seq: 0,
                kind: ItemKind::Unverifiable,
                chunk_id: None,
                page: Some(2),
                sentence: "Richness rose sharply (Smith, 2019).".into(),
                payload_json: r#"{"reason":"cited work not in library: (Smith, 2019)"}"#.into(),
            }],
        )
        .unwrap();
        let stats = Arc::new(Stats::default());
        let m = manager(stats.clone(), 0);

        run_job(
            &db,
            &m,
            &JobControl::new(job),
            &InteractivePriority::new(),
            &no_embedder,
            &|_| {},
        )
        .await
        .unwrap();

        assert_eq!(stats.calls.load(Ordering::SeqCst), 0, "an unverifiable item ran the model");
        let it = &job_results(&db, job, 0, 10).unwrap()[0];
        assert_eq!(it.status, "done");
        assert!(it.error.is_none(), "unverifiable is a finding, not an error");
        assert!(it.result_json.as_deref().unwrap().contains("not in library"));
    }

    /// A support item with no embedder is SKIPPED with a stated reason — a
    /// missing precondition, visibly different from the model getting it wrong.
    #[tokio::test]
    async fn a_support_item_without_an_embedder_is_skipped_not_failed() {
        let db = Database::in_memory().unwrap();
        let job = create_job(
            &db,
            "thesis_audit",
            None,
            "citation_support-v1.4",
            &[NewItem {
                seq: 0,
                kind: ItemKind::CitationSupport,
                chunk_id: None,
                page: Some(1),
                sentence: "Organic management increased richness by about 31 percent.".into(),
                payload_json: r#"{"documentId":1,"citedSource":"Smith (2019)"}"#.into(),
            }],
        )
        .unwrap();
        let stats = Arc::new(Stats::default());
        let m = manager(stats.clone(), 0);

        run_job(
            &db,
            &m,
            &JobControl::new(job),
            &InteractivePriority::new(),
            &no_embedder,
            &|_| {},
        )
        .await
        .unwrap();

        assert_eq!(stats.calls.load(Ordering::SeqCst), 0, "generation ran without evidence");
        let it = &job_results(&db, job, 0, 10).unwrap()[0];
        assert_eq!(it.status, "skipped");
        assert!(it.result_json.as_deref().unwrap().contains("noEmbedder"));
    }

    /// Pause is not a finished state: the job stays `running` so the next
    /// startup treats it as resumable rather than as a crash or a completion.
    #[tokio::test]
    async fn pausing_leaves_the_job_resumable() {
        let (db, job) = seeded(4);
        let stats = Arc::new(Stats::default());
        let m = manager(stats.clone(), 0);
        let control = JobControl::new(job);
        control.pause();

        let outcome =
            run_job(&db, &m, &control, &InteractivePriority::new(), &no_embedder, &|_| {})
                .await
                .unwrap();

        assert_eq!(outcome, RunOutcome::Paused);
        assert_eq!(stats.calls.load(Ordering::SeqCst), 0);
        assert_eq!(jobs::get_job(&db, job).unwrap().unwrap().status, JobStatus::Running);
        assert_eq!(jobs::remaining_items(&db, job).unwrap(), 4);
    }
}
