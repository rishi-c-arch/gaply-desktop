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
    // WHICH model is answering. `model_id()` is the configured loader and
    // `acquire()` has no silent fallback (§11 D51), so recording it here is a
    // true statement about what judged this job — and it is what the exported
    // report's cover reads.
    let _ = jobs::set_job_model(db, job_id, &manager.model_id());

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

    // The SECTION, from the planner's payload. This was `String::new()`, and
    // the prompt's "the same sentence in Results is probably the author's own
    // finding" rule therefore never fired once — which is why the first real
    // audit flagged the authors' own F1 scores and their hardware setup as
    // needing citations.
    let section = serde_json::from_str::<serde_json::Value>(&item.payload_json)
        .ok()
        .and_then(|v| v.get("section")?.as_str().map(str::to_string))
        .unwrap_or_default();
    let task = CitationNeedTask::new(CitationNeedInput {
        sentence: item.sentence.clone(),
        preceding_sentence: String::new(),
        following_sentence: String::new(),
        section,
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

            // PERSISTENCE DISCIPLINE, unchanged from the interactive path: a
            // card is written ONLY for a run that passed validation. Reaching
            // here means the validator already proved every chunk_id was sent
            // and every page matches the store, so the join rows below cannot
            // point at anything the model invented. A rejected run returns via
            // the Err arm and never reaches this code at all.
            let card_id = persist_card(db, document_id, &item.sentence, &run, &bundle);

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
                    "cardId": card_id,
                    "persisted": card_id.is_some(),
                })
                .to_string(),
                error: None,
                summary: format!("verdict={verdict}"),
            }
        }
        Err(e) => failed_outcome("citation_support", e),
    }
}

/// Write the evidence card for an ACCEPTED support result.
///
/// Mirrors `ai_citation_support` exactly — same card shape, same chunk-role
/// join, same verdict mapping — because two persistence paths that drift are
/// two sets of provenance rules. Only reachable after validation passed.
///
/// A failure to persist is logged and reported as `persisted: false` rather
/// than failing the item: the judgement itself is still a real result, and
/// losing three hours of audit to one bad insert would be the wrong trade.
fn persist_card(
    db: &Database,
    document_id: i64,
    claim: &str,
    run: &crate::ai::task::TaskRun<crate::ai::tasks::citation_support::CitationSupportOutput>,
    bundle: &crate::ai::evidence::EvidenceBundle,
) -> Option<i64> {
    use crate::ai::tasks::citation_support::Verdict;
    use gaply_core::ai_engine::cards;

    let advisories: Vec<String> = run.advisories.iter().map(|a| a.to_string()).collect();
    let provenance = serde_json::json!({
        "advisories": advisories,
        "chunksSent": bundle.chunks_sent,
        "chunksDropped": bundle.chunks_dropped,
        "evidenceWords": bundle.words_estimated,
        "retrievalPath": bundle.retrieval_path,
        "supportingChunks": run.output.supporting_chunks,
        "source": "thesis_audit",
    });
    let parse_id = |c: &crate::ai::tasks::citation_support::SupportingChunk| {
        c.chunk_id.trim_start_matches('c').parse::<i64>().ok()
    };
    let refs: Vec<(i64, cards::ChunkRole)> = run
        .output
        .supporting_chunks
        .iter()
        .filter_map(parse_id)
        .map(|id| (id, cards::ChunkRole::Supporting))
        .collect();

    let card = cards::NewEvidenceCard {
        document_id,
        chunk_id: run.output.supporting_chunks.first().and_then(parse_id),
        page: run.output.supporting_chunks.first().and_then(|c| c.page),
        claim: claim.to_string(),
        evidence_text: run.output.explanation.clone(),
        evidence_type: match run.output.verdict {
            Verdict::Contradicts => "contradict".to_string(),
            Verdict::InsufficientEvidence => "context".to_string(),
            _ => "support".to_string(),
        },
        verdict: serde_json::to_value(run.output.verdict)
            .ok()
            .and_then(|v| v.as_str().map(str::to_string)),
        confidence: Some(run.output.confidence),
        model_id: run.model_id.clone(),
        model_version: crate::ai::generative::BUNDLED_GEN_MODEL_QUANT.to_string(),
        prompt_version: run.prompt_version.to_string(),
        provenance_json: provenance.to_string(),
    };
    match cards::insert_card(db, &card, &refs) {
        Ok(id) => Some(id),
        Err(e) => {
            tracing::warn!(%e, "audit result accepted but its evidence card could not be written");
            None
        }
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

    /// Seed a document that retrieval can actually find: chunks plus vectors in
    /// one registered embedding space.
    fn seed_indexed_document(db: &Database) -> i64 {
        use gaply_core::ai_engine::{embeddings as core_emb, registry, store};

        let doc = store::create_document(db, "Smith 2019", "/tmp/smith.pdf", "ck-smith").unwrap();
        store::index_chunks(
            db,
            doc,
            &[gaply_core::chunk::PagedChunk {
                seq: 0,
                content: "Species richness rose 31 percent under organic management.".into(),
                token_estimate: 9,
                page: Some(5),
                section: Some("Results".into()),
                char_start: 0,
                char_end: 57,
            }],
        )
        .unwrap();
        registry::register_model(
            db,
            registry::ModelRow {
                id: "test-emb".into(),
                kind: "embedding".into(),
                display_name: "test".into(),
                file_path: "/x".into(),
                sha256: None,
                dim: Some(3),
                quant: None,
            },
        )
        .unwrap();
        // The generative model must be registered too: ai_evidence_cards.model_id
        // is a foreign key into the registry, which is how a card is prevented
        // from claiming provenance it cannot substantiate.
        registry::register_model(
            db,
            registry::ModelRow {
                id: "mock".into(),
                kind: "generative".into(),
                display_name: "mock".into(),
                file_path: "/x".into(),
                sha256: None,
                dim: None,
                quant: Some("Q4_K_M".into()),
            },
        )
        .unwrap();
        let space = core_emb::EmbeddingSpace::new("test-emb", "test-p1");
        let pending = core_emb::chunks_missing_embeddings(db, None, "test-emb").unwrap();
        let rows: Vec<(i64, Vec<f32>)> =
            pending.iter().map(|p| (p.chunk_id, vec![1.0, 0.0, 0.0])).collect();
        core_emb::put_embeddings(db, &space, &rows).unwrap();
        doc
    }

    fn support_job(db: &Database, doc: i64) -> i64 {
        create_job(
            db,
            "thesis_audit",
            None,
            "citation_support-v1.4",
            &[NewItem {
                seq: 0,
                kind: ItemKind::CitationSupport,
                chunk_id: None,
                page: Some(5),
                sentence: "Organic management increased species richness by about 31 percent."
                    .into(),
                payload_json: serde_json::json!({
                    "documentId": doc,
                    "citedSource": "Smith (2019) — Soil life",
                })
                .to_string(),
            }],
        )
        .unwrap()
    }

    /// Counted through gaply-core's API, not SQL: the app crate cannot execute
    /// SQL at all (plan §3), and that constraint is the point rather than an
    /// inconvenience to work around in a test.
    fn cards_for(db: &Database, doc: i64) -> usize {
        gaply_core::ai_engine::cards::list_cards(db, doc).unwrap().len()
    }

    fn manager_replying(stats: Arc<Stats>, reply: &str) -> ModelManager {
        ModelManager::new(Arc::new(MockLoader {
            stats,
            reply: reply.to_string(),
            stall_ms: 0,
        }))
    }

    /// PERSISTENCE DISCIPLINE, unchanged (plan §9.9 / D9). A support result that
    /// passes validation writes exactly one evidence card; one that fails
    /// writes none. The audit path must obey the same rule as the interactive
    /// path, or provenance depends on which door the result came through.
    #[tokio::test]
    async fn only_a_valid_support_result_writes_an_evidence_card() {
        // ---- accepted ----
        let db = Database::in_memory().unwrap();
        let doc = seed_indexed_document(&db);
        let job = support_job(&db, doc);
        let valid = r#"{"verdict":"strong","confidence":0.9,
            "supporting_chunks":[{"chunk_id":"c1","page":5,"why":"Reports the richness increase the claim describes."}],
            "claim_elements":[{"element":"organic management","status":"found"},
                              {"element":"31 percent","status":"found"}],
            "explanation":"The chunk reports the increase the claim describes.",
            "suggested_rewrite":null}"#;
        let stats = Arc::new(Stats::default());
        let m = manager_replying(stats.clone(), valid);
        run_job(
            &db,
            &m,
            &JobControl::new(job),
            &InteractivePriority::new(),
            &|_| Ok(vec![1.0, 0.0, 0.0]),
            &|_| {},
        )
        .await
        .unwrap();

        let it = &job_results(&db, job, 0, 10).unwrap()[0];
        assert_eq!(it.status, "done");
        assert!(
            it.result_json.as_deref().unwrap().contains("\"persisted\":true"),
            "an accepted result did not persist: {:?}",
            it.result_json
        );
        assert_eq!(cards_for(&db, doc), 1, "an accepted support result wrote no card");

        // ---- rejected: an invented chunk_id, fatal in both attempts ----
        let db2 = Database::in_memory().unwrap();
        let doc2 = seed_indexed_document(&db2);
        let job2 = support_job(&db2, doc2);
        let invented = r#"{"verdict":"strong","confidence":0.9,
            "supporting_chunks":[{"chunk_id":"c999","page":5,"why":"Reports the increase."}],
            "claim_elements":[{"element":"organic management","status":"found"}],
            "explanation":"x","suggested_rewrite":null}"#;
        let stats2 = Arc::new(Stats::default());
        let m2 = manager_replying(stats2.clone(), invented);
        run_job(
            &db2,
            &m2,
            &JobControl::new(job2),
            &InteractivePriority::new(),
            &|_| Ok(vec![1.0, 0.0, 0.0]),
            &|_| {},
        )
        .await
        .unwrap();

        let it2 = &job_results(&db2, job2, 0, 10).unwrap()[0];
        assert_eq!(it2.status, "failed", "an invented chunk_id must not be accepted");
        assert_eq!(
            cards_for(&db2, doc2),
            0,
            "a REJECTED support result wrote an evidence card — the grounding guarantee is broken"
        );
        assert_eq!(stats2.calls.load(Ordering::SeqCst), 2, "expected one retry, then failure");
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

/// Env-gated end-to-end smoke over the REAL bundled 0.5B.
///
/// Tests the JOB MACHINERY — plan, run, stream, resume-safety, report — not
/// judgement quality. The 0.5B's answers are expected to be poor (§11 D34);
/// what is being checked is that hours-long batch work completes, retires every
/// item exactly once, and produces a readable report.
///
/// Off by default because it loads a real model and takes minutes:
/// `GAPLY_SMOKE_AUDIT=1 cargo test -p app smoke_audit -- --nocapture --ignored`
#[cfg(test)]
mod smoke {
    use super::*;
    use gaply_core::ai_engine::{jobs, thesis_audit};

    // multi_thread: the runner uses spawn_blocking for generation, and
    // block_in_place below needs more than the single-threaded test runtime.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    #[ignore = "loads the real 0.5B; run with GAPLY_SMOKE_AUDIT=1"]
    async fn smoke_audit_runs_ten_items_end_to_end_on_the_bundled_model() {
        if std::env::var("GAPLY_SMOKE_AUDIT").ok().as_deref() != Some("1") {
            eprintln!("skipped: set GAPLY_SMOKE_AUDIT=1 to run");
            return;
        }
        let Some(loader) = crate::ai::generative::BundledGenerativeLoader::resolve() else {
            panic!("no bundled generative model resolved — the smoke needs the 0.5B");
        };

        let dir = std::env::temp_dir().join(format!("gaply-smoke-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("chapter.txt");
        std::fs::write(
            &path,
            include_str!("../../gaply-core/src/ai_engine/testdata/thesis_chapter.txt"),
        )
        .unwrap();

        let db = Database::in_memory().unwrap();
        let plan = tokio::task::block_in_place(|| {
            thesis_audit::plan_thesis_audit(&db, &path, "citation_need-v2")
        })
        .unwrap();
        println!("\n=== PLAN ===\n{}", serde_json::to_string_pretty(&plan).unwrap());

        // Cap the run: this is a machinery test, not an audit.
        let capped = {
            let all = jobs::job_results(&db, plan.job_id, 0, 1000).unwrap();
            let keep: Vec<_> = all.iter().take(10).map(|i| i.seq).collect();
            for item in all.iter().filter(|i| !keep.contains(&i.seq)) {
                jobs::complete_item(&db, item.id, "skipped", Some(r#"{"capped":true}"#), None)
                    .unwrap();
            }
            keep.len()
        };

        let manager = ModelManager::new(Arc::new(loader));
        let control = JobControl::new(plan.job_id);
        let started = std::time::Instant::now();
        let outcome = run_job(
            &db,
            &manager,
            &control,
            &InteractivePriority::new(),
            &|_| Err(gaply_core::GaplyError::Validation("no embedder in the smoke".into())),
            &|p| println!("  progress {}/{} — {}", p.completed, p.total, p.latest_item_summary),
        )
        .await
        .unwrap();
        let elapsed = started.elapsed();

        assert_eq!(outcome, RunOutcome::Completed);
        assert_eq!(jobs::remaining_items(&db, plan.job_id).unwrap(), 0);

        let health = thesis_audit::thesis_health(&db, plan.job_id).unwrap();
        println!("\n=== WALL TIME ===\n{capped} items in {elapsed:?} ({:.1} s/item)",
            elapsed.as_secs_f64() / capped.max(1) as f64);
        println!("\n=== THESIS HEALTH ===\n{}", serde_json::to_string_pretty(&health).unwrap());

        // Every item retired exactly once — the machinery guarantee.
        let all = jobs::job_results(&db, plan.job_id, 0, 1000).unwrap();
        assert!(all.iter().all(|i| matches!(i.status.as_str(), "done" | "failed" | "skipped")));
        assert!(all.iter().all(|i| i.result_json.is_some()));

        let _ = std::fs::remove_dir_all(&dir);
    }
}
