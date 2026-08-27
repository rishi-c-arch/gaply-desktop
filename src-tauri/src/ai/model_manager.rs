//! Lifecycle for the GENERATIVE model only.
//!
//! The embedding engine is resident from startup and never unloaded (~130 MB,
//! and an idle timer would only add a stall). The generative model is the
//! opposite: ~1.1 GB for the bundled 0.5B and multiple GB for the production
//! model, loaded lazily on the first generation request and dropped once it has
//! been idle. Two models, two policies, deliberately.
//!
//! # State machine
//!
//! ```text
//!                     first generate()
//!   NotInstalled        (no model registered → stays here, honestly)
//!
//!   NotLoaded ──────────► Loading ──────────► Ready ◄─┐
//!       ▲                    │ load failed      │     │ acquire() before the
//!       │                    └──────────────────┤     │ timer fires
//!       │                       back to NotLoaded│     │
//!       │                                        ▼     │
//!       │                          refcount==0  Idle{since}
//!       │                                        │
//!       │            idle_timeout elapsed AND refcount still 0
//!       │                                        ▼
//!       └──────────────────────────────────  Unloading
//! ```
//!
//! - `acquire()` returns a [`ModelLease`] that decrements the refcount **on
//!   Drop**, so a cancelled, failed or panicking generation cannot leak it.
//! - `Idle → Ready` on a new acquire is the edge that matters: two generations a
//!   minute apart must not pay two multi-GB loads. The timer exists to release
//!   memory, not to enforce a cadence.
//! - `Unloading` re-checks the refcount under the lock immediately before
//!   dropping the weights, so an `acquire()` racing the timer wins.
//!
//! # Single-inference invariant
//!
//! Every generation passes through one `tokio::sync::Semaphore` with a single
//! permit, held for the whole call. Concurrent callers queue; the model is never
//! touched by two at once. The permit is separate from the refcount: the
//! refcount says "someone still needs the weights", the permit says "the model
//! is busy right now".

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use gaply_core::GaplyError;
use serde::Serialize;

use crate::ai::generative::{GenOutput, GenRequest, GenerationBackend, RamEstimate};

/// A per-token sink for streaming. Named because it appears in three
/// signatures; an inline `Option<Arc<dyn Fn(&str) + Send + Sync>>` in each is
/// what clippy's type_complexity is pointing at.
pub type TokenSink = Arc<dyn Fn(&str) + Send + Sync>;
/// The owned form the blocking backend call takes.
type BoxedTokenSink = Box<dyn Fn(&str) + Send + Sync>;

/// Idle before the generative model is dropped. Constant for now.
pub const DEFAULT_IDLE_TIMEOUT: Duration = Duration::from_secs(8 * 60);

/// Where the weights come from, and what they will cost.
///
/// A trait so the state machine, serialization and cancellation tests need no
/// real model — and so the engine is model-file-agnostic (§9.9): the production
/// model is a different loader, not a different manager.
pub trait BackendLoader: Send + Sync {
    /// Registry id of the model this loader provides.
    fn model_id(&self) -> String;
    /// Pre-flight cost, computed without loading (§5 — never RSS).
    fn ram_estimate(&self) -> Result<RamEstimate, GaplyError>;
    /// Actually load. Called at most once per NotLoaded → Ready transition.
    fn load(&self) -> Result<Arc<dyn GenerationBackend>, GaplyError>;
}

/// Stands in when no generative model resolves. Its `load` is unreachable —
/// the manager is put in `NotInstalled`, which short-circuits before any load —
/// but it returns an honest error rather than panicking if that ever changes.
pub struct NullLoader;

impl BackendLoader for NullLoader {
    fn model_id(&self) -> String {
        "none".to_string()
    }
    fn ram_estimate(&self) -> Result<RamEstimate, GaplyError> {
        Err(GaplyError::NotFound { entity: "generative model", id: "none installed".into() })
    }
    fn load(&self) -> Result<Arc<dyn GenerationBackend>, GaplyError> {
        Err(GaplyError::Validation("no generative model is installed".into()))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "state", rename_all = "camelCase")]
pub enum GenState {
    /// No generative model is registered/resolvable. Not an error: every
    /// deterministic feature works, and generation reports this rather than
    /// failing obscurely.
    NotInstalled,
    NotLoaded,
    Loading,
    Ready,
    Idle { idle_ms: u64 },
    Unloading,
}

/// Internal state; `GenState` is the serializable projection of it.
enum Slot {
    NotInstalled,
    NotLoaded,
    Loading,
    Ready(Arc<dyn GenerationBackend>),
    Idle { backend: Arc<dyn GenerationBackend>, since: Instant },
    Unloading,
}

pub struct ModelManager {
    loader: Arc<dyn BackendLoader>,
    slot: Mutex<Slot>,
    /// In-flight generations. Only 0 permits an unload.
    refcount: Arc<AtomicUsize>,
    /// THE single-inference invariant.
    gate: tokio::sync::Semaphore,
    /// Signals a completed load to callers that waited through `Loading`.
    load_done: tokio::sync::Notify,
    idle_timeout: Duration,
}

/// Holds the refcount up for as long as a caller needs the weights.
///
/// RAII rather than manual bookkeeping: a generation that returns early, errors,
/// is cancelled, or panics still releases, because Drop always runs.
pub struct ModelLease {
    backend: Arc<dyn GenerationBackend>,
    refcount: Arc<AtomicUsize>,
}

impl ModelLease {
    pub fn backend(&self) -> &Arc<dyn GenerationBackend> {
        &self.backend
    }
}

impl Drop for ModelLease {
    fn drop(&mut self) {
        self.refcount.fetch_sub(1, Ordering::SeqCst);
    }
}

/// What the locked inspection of the slot decided. Exists so the `MutexGuard`
/// is released BEFORE any `.await` — a guard held across an await would make the
/// whole future non-Send and, worse, hold the lock across a multi-second load.
enum AcquireStep {
    Ready(Arc<dyn GenerationBackend>),
    NeedLoad,
    WaitForOtherLoad,
    NotInstalled,
}

impl ModelManager {
    pub fn new(loader: Arc<dyn BackendLoader>) -> Self {
        Self::with_idle_timeout(loader, DEFAULT_IDLE_TIMEOUT)
    }

    /// Constructor taking the idle timeout so tests can exercise the unload
    /// edge without waiting eight minutes. Production always uses the default.
    pub fn with_idle_timeout(loader: Arc<dyn BackendLoader>, idle_timeout: Duration) -> Self {
        Self {
            loader,
            slot: Mutex::new(Slot::NotLoaded),
            refcount: Arc::new(AtomicUsize::new(0)),
            gate: tokio::sync::Semaphore::new(1),
            load_done: tokio::sync::Notify::new(),
            idle_timeout,
        }
    }

    /// Mark that no model is available at all.
    pub fn set_not_installed(&self) {
        *self.slot.lock().expect("slot poisoned") = Slot::NotInstalled;
    }

    pub fn state(&self) -> GenState {
        match &*self.slot.lock().expect("slot poisoned") {
            Slot::NotInstalled => GenState::NotInstalled,
            Slot::NotLoaded => GenState::NotLoaded,
            Slot::Loading => GenState::Loading,
            Slot::Ready(_) => GenState::Ready,
            Slot::Idle { since, .. } => {
                GenState::Idle { idle_ms: since.elapsed().as_millis() as u64 }
            }
            Slot::Unloading => GenState::Unloading,
        }
    }

    pub fn in_flight(&self) -> usize {
        self.refcount.load(Ordering::SeqCst)
    }

    pub fn ram_estimate(&self) -> Result<RamEstimate, GaplyError> {
        self.loader.ram_estimate()
    }

    pub fn model_id(&self) -> String {
        self.loader.model_id()
    }

    /// Get the weights, loading them if needed. Lazy: this is the ONLY thing
    /// that ever triggers a load, and it is reached only from a generation
    /// request — never from startup.
    pub async fn acquire(&self) -> Result<ModelLease, GaplyError> {
        loop {
            // The guard lives ONLY in this block. Everything slow happens after
            // it is released.
            let step = {
                let mut slot = self.slot.lock().expect("slot poisoned");
                match &*slot {
                    Slot::NotInstalled => AcquireStep::NotInstalled,
                    Slot::Ready(b) => {
                        let b = b.clone();
                        self.refcount.fetch_add(1, Ordering::SeqCst);
                        AcquireStep::Ready(b)
                    }
                    // Idle -> Ready. This is why a second generation a minute
                    // later does not pay another load.
                    Slot::Idle { backend, .. } => {
                        let b = backend.clone();
                        *slot = Slot::Ready(b.clone());
                        self.refcount.fetch_add(1, Ordering::SeqCst);
                        AcquireStep::Ready(b)
                    }
                    Slot::NotLoaded | Slot::Unloading => {
                        *slot = Slot::Loading;
                        AcquireStep::NeedLoad
                    }
                    Slot::Loading => AcquireStep::WaitForOtherLoad,
                }
            };

            match step {
                AcquireStep::NotInstalled => {
                    return Err(GaplyError::Validation(
                        "no generative model is installed — every deterministic feature \
                         works without it"
                            .into(),
                    ))
                }
                AcquireStep::Ready(backend) => return Ok(self.lease(backend)),
                AcquireStep::WaitForOtherLoad => {
                    // Someone else is loading. Wait for their result rather than
                    // starting a second load of the same weights.
                    self.load_done.notified().await;
                    continue;
                }
                AcquireStep::NeedLoad => {}
            }

            // Load OFF the async runtime and OUTSIDE the lock.
            let loader = self.loader.clone();
            let loaded = tokio::task::spawn_blocking(move || loader.load())
                .await
                .map_err(|e| GaplyError::Internal(format!("model load task panicked: {e}")))?;

            let outcome = {
                let mut slot = self.slot.lock().expect("slot poisoned");
                match loaded {
                    Ok(backend) => {
                        *slot = Slot::Ready(backend.clone());
                        self.refcount.fetch_add(1, Ordering::SeqCst);
                        Ok(backend)
                    }
                    Err(e) => {
                        // Back to NotLoaded, never stuck in Loading — a failed
                        // load must leave the manager retryable.
                        *slot = Slot::NotLoaded;
                        Err(e)
                    }
                }
            };
            self.load_done.notify_waiters();
            return outcome.map(|b| self.lease(b));
        }
    }

    fn lease(&self, backend: Arc<dyn GenerationBackend>) -> ModelLease {
        ModelLease { backend, refcount: self.refcount.clone() }
    }

    /// Called when a lease is released, to enter Idle at refcount 0.
    fn note_release(&self) {
        if self.refcount.load(Ordering::SeqCst) != 0 {
            return;
        }
        let mut slot = self.slot.lock().expect("slot poisoned");
        if let Slot::Ready(backend) = &*slot {
            *slot = Slot::Idle { backend: backend.clone(), since: Instant::now() };
        }
    }

    /// Drop the weights if they have been idle long enough AND nothing is in
    /// flight. Called by the timer task; exposed so tests drive it directly
    /// instead of sleeping.
    pub fn tick_idle(&self) -> bool {
        // Re-check the refcount UNDER the lock, immediately before dropping, so
        // an acquire() racing the timer wins.
        let mut slot = self.slot.lock().expect("slot poisoned");
        if self.refcount.load(Ordering::SeqCst) != 0 {
            return false;
        }
        // Promote a Ready slot nobody is using into Idle. generate() already
        // does this on release; doing it here too means a lease dropped by any
        // other path still becomes reclaimable rather than pinning the weights
        // forever.
        if let Slot::Ready(backend) = &*slot {
            *slot = Slot::Idle { backend: backend.clone(), since: Instant::now() };
            return false;
        }
        let elapsed_enough = match &*slot {
            Slot::Idle { since, .. } => since.elapsed() >= self.idle_timeout,
            _ => false,
        };
        if !elapsed_enough {
            return false;
        }
        *slot = Slot::Unloading;
        // Dropping the Arc releases the weights once the last clone goes.
        *slot = Slot::NotLoaded;
        true
    }

    /// One generation, start to finish: acquire (loading if needed), serialize
    /// through the gate, run off the async runtime, release.
    pub async fn generate(
        &self,
        prompt: String,
        max_tokens: usize,
        cancel: Arc<AtomicBool>,
        on_token: Option<TokenSink>,
    ) -> Result<GenOutput, GaplyError> {
        let lease = self.acquire().await?;

        // THE single-inference invariant. Held for the whole generation.
        let _permit = self
            .gate
            .acquire()
            .await
            .map_err(|_| GaplyError::Internal("inference gate closed".into()))?;

        // A request cancelled while it was QUEUED must not start. Checked after
        // the gate, because that is where the waiting happened.
        if cancel.load(Ordering::SeqCst) {
            drop(lease);
            self.note_release();
            return Err(GaplyError::Cancelled);
        }

        let backend = lease.backend().clone();
        let result = tokio::task::spawn_blocking(move || {
            let sink = on_token.clone();
            let cb: Option<BoxedTokenSink> =
                sink.map(|s| Box::new(move |t: &str| s(t)) as BoxedTokenSink);
            backend.generate(GenRequest {
                prompt,
                max_tokens,
                cancel: &cancel,
                on_token: cb.as_deref(),
            })
        })
        .await
        .map_err(|e| GaplyError::Internal(format!("generation task panicked: {e}")))?;

        drop(lease);
        self.note_release();
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::generative::StopReason;
    use std::sync::atomic::AtomicU32;

    /// Instrumented mock: counts loads, tracks CONCURRENT generations so the
    /// serialization test can prove none interleaved, and can yield tokens
    /// slowly so cancellation has something to interrupt.
    #[derive(Default)]
    struct MockStats {
        loads: AtomicU32,
        concurrent_now: AtomicU32,
        concurrent_max: AtomicU32,
        completed: AtomicU32,
    }

    struct MockBackend {
        stats: Arc<MockStats>,
        token_delay: Duration,
        tokens: usize,
        output: String,
    }

    impl GenerationBackend for MockBackend {
        fn generate(&self, req: GenRequest<'_>) -> Result<GenOutput, GaplyError> {
            let now = self.stats.concurrent_now.fetch_add(1, Ordering::SeqCst) + 1;
            self.stats.concurrent_max.fetch_max(now, Ordering::SeqCst);
            let mut stop = StopReason::EndOfTurn;
            for _ in 0..self.tokens {
                if req.cancel.load(Ordering::SeqCst) {
                    stop = StopReason::Cancelled;
                    break;
                }
                std::thread::sleep(self.token_delay);
            }
            self.stats.concurrent_now.fetch_sub(1, Ordering::SeqCst);
            self.stats.completed.fetch_add(1, Ordering::SeqCst);
            Ok(GenOutput {
                text: self.output.clone(),
                tokens: self.tokens,
                stop_reason: stop,
                elapsed_ms: 0,
                prompt_tokens: 0,
                prefill_ms: 0,
                decode_ms: 0,
                decode_tokens_per_sec: 0.0,
            })
        }
    }

    struct MockLoader {
        stats: Arc<MockStats>,
        token_delay: Duration,
        tokens: usize,
        output: String,
        fail: bool,
    }

    impl BackendLoader for MockLoader {
        fn model_id(&self) -> String {
            "mock-model".into()
        }
        fn ram_estimate(&self) -> Result<RamEstimate, GaplyError> {
            Ok(RamEstimate {
                weights_bytes: 1000,
                kv_cache_bytes: 500,
                total_bytes: 1500,
                context_length: 8,
                model_max_context: 8,
                layers: 2,
                kv_heads: 1,
                head_dim: 4,
            })
        }
        fn load(&self) -> Result<Arc<dyn GenerationBackend>, GaplyError> {
            self.stats.loads.fetch_add(1, Ordering::SeqCst);
            if self.fail {
                return Err(GaplyError::Validation("mock load failure".into()));
            }
            Ok(Arc::new(MockBackend {
                stats: self.stats.clone(),
                token_delay: self.token_delay,
                tokens: self.tokens,
                output: self.output.clone(),
            }))
        }
    }

    fn manager(
        stats: Arc<MockStats>,
        idle: Duration,
        token_delay: Duration,
        tokens: usize,
    ) -> ModelManager {
        ModelManager::with_idle_timeout(
            Arc::new(MockLoader {
                stats,
                token_delay,
                tokens,
                output: "{\"ok\":true}".into(),
                fail: false,
            }),
            idle,
        )
    }

    #[tokio::test]
    async fn loading_is_lazy_and_happens_once() {
        let stats = Arc::new(MockStats::default());
        let m = manager(stats.clone(), Duration::from_secs(60), Duration::ZERO, 1);

        // Constructing the manager must NOT load.
        assert_eq!(m.state(), GenState::NotLoaded);
        assert_eq!(stats.loads.load(Ordering::SeqCst), 0, "constructing loaded the model");

        m.generate("hi".into(), 4, Arc::new(AtomicBool::new(false)), None).await.unwrap();
        assert_eq!(stats.loads.load(Ordering::SeqCst), 1, "first generate did not load");

        // Second generation reuses the loaded weights.
        m.generate("hi".into(), 4, Arc::new(AtomicBool::new(false)), None).await.unwrap();
        assert_eq!(stats.loads.load(Ordering::SeqCst), 1, "reloaded weights that were still held");
    }

    #[tokio::test]
    async fn refcount_returns_to_zero_and_the_state_becomes_idle() {
        let stats = Arc::new(MockStats::default());
        let m = manager(stats, Duration::from_secs(60), Duration::ZERO, 1);
        m.generate("hi".into(), 4, Arc::new(AtomicBool::new(false)), None).await.unwrap();
        assert_eq!(m.in_flight(), 0, "a completed generation leaked its lease");
        assert!(matches!(m.state(), GenState::Idle { .. }), "state is {:?}", m.state());
    }

    #[tokio::test]
    async fn idle_unload_happens_only_at_refcount_zero() {
        let stats = Arc::new(MockStats::default());
        let m = Arc::new(manager(stats.clone(), Duration::from_millis(30), Duration::ZERO, 1));
        m.generate("hi".into(), 4, Arc::new(AtomicBool::new(false)), None).await.unwrap();
        assert!(matches!(m.state(), GenState::Idle { .. }));

        // Too soon: the timer has not elapsed.
        assert!(!m.tick_idle(), "unloaded before the idle timeout");
        tokio::time::sleep(Duration::from_millis(50)).await;
        assert!(m.tick_idle(), "did not unload after the idle timeout at refcount 0");
        assert_eq!(m.state(), GenState::NotLoaded);

        // And it reloads on the next request.
        m.generate("hi".into(), 4, Arc::new(AtomicBool::new(false)), None).await.unwrap();
        assert_eq!(stats.loads.load(Ordering::SeqCst), 2, "did not reload after unload");
    }

    #[tokio::test]
    async fn never_unloads_while_a_generation_is_in_flight() {
        let stats = Arc::new(MockStats::default());
        // Slow generation so we can tick the timer while it runs.
        let m = Arc::new(manager(stats, Duration::from_millis(1), Duration::from_millis(20), 10));
        let m2 = m.clone();
        let handle = tokio::spawn(async move {
            m2.generate("hi".into(), 10, Arc::new(AtomicBool::new(false)), None).await
        });
        tokio::time::sleep(Duration::from_millis(60)).await;
        assert_eq!(m.in_flight(), 1, "expected an in-flight generation");
        // Timeout is 1ms and long past, but refcount is 1 — must not unload.
        assert!(!m.tick_idle(), "unloaded while a generation was in flight");
        assert_ne!(m.state(), GenState::NotLoaded);
        handle.await.unwrap().unwrap();
    }

    #[tokio::test]
    async fn idle_returns_to_ready_without_a_second_load() {
        let stats = Arc::new(MockStats::default());
        let m = manager(stats.clone(), Duration::from_secs(60), Duration::ZERO, 1);
        m.generate("a".into(), 4, Arc::new(AtomicBool::new(false)), None).await.unwrap();
        assert!(matches!(m.state(), GenState::Idle { .. }));
        m.generate("b".into(), 4, Arc::new(AtomicBool::new(false)), None).await.unwrap();
        assert_eq!(stats.loads.load(Ordering::SeqCst), 1, "Idle -> Ready paid for a second load");
    }

    #[tokio::test]
    async fn concurrent_generations_are_serialized_and_never_interleave() {
        let stats = Arc::new(MockStats::default());
        let m = Arc::new(manager(stats.clone(), Duration::from_secs(60), Duration::from_millis(5), 6));

        let mut handles = Vec::new();
        for i in 0..5 {
            let m = m.clone();
            handles.push(tokio::spawn(async move {
                m.generate(format!("p{i}"), 6, Arc::new(AtomicBool::new(false)), None).await
            }));
        }
        for h in handles {
            h.await.unwrap().unwrap();
        }
        assert_eq!(stats.completed.load(Ordering::SeqCst), 5, "not every request completed");
        assert_eq!(
            stats.concurrent_max.load(Ordering::SeqCst),
            1,
            "two generations touched the model at once — the single-inference invariant is broken"
        );
        assert_eq!(stats.loads.load(Ordering::SeqCst), 1, "concurrent acquires each loaded");
    }

    #[tokio::test]
    async fn a_queued_request_cancelled_before_it_starts_never_runs() {
        let stats = Arc::new(MockStats::default());
        let m = Arc::new(manager(stats.clone(), Duration::from_secs(60), Duration::from_millis(10), 20));

        // Occupy the gate with a long generation.
        let m1 = m.clone();
        let busy = tokio::spawn(async move {
            m1.generate("long".into(), 20, Arc::new(AtomicBool::new(false)), None).await
        });
        tokio::time::sleep(Duration::from_millis(30)).await;

        // Queue a second, then cancel it while it is still waiting.
        let cancel = Arc::new(AtomicBool::new(false));
        let m2 = m.clone();
        let c2 = cancel.clone();
        let queued = tokio::spawn(async move { m2.generate("queued".into(), 20, c2, None).await });
        tokio::time::sleep(Duration::from_millis(10)).await;
        cancel.store(true, Ordering::SeqCst);

        busy.await.unwrap().unwrap();
        let err = queued.await.unwrap().unwrap_err();
        assert_eq!(err.code(), "cancelled", "queued request ran anyway: {err:?}");
        // 1 = only the first ever reached the backend.
        assert_eq!(stats.completed.load(Ordering::SeqCst), 1, "the cancelled request still ran");
        assert_eq!(m.in_flight(), 0, "cancelled request leaked its lease");
    }

    #[tokio::test]
    async fn a_running_generation_stops_mid_stream_when_cancelled() {
        let stats = Arc::new(MockStats::default());
        // 100 tokens at 5ms would be 500ms; cancelling at ~40ms must cut it short.
        let m = Arc::new(manager(stats, Duration::from_secs(60), Duration::from_millis(5), 100));
        let cancel = Arc::new(AtomicBool::new(false));
        let m2 = m.clone();
        let c2 = cancel.clone();
        let started = Instant::now();
        let h = tokio::spawn(async move { m2.generate("long".into(), 100, c2, None).await });
        tokio::time::sleep(Duration::from_millis(40)).await;
        cancel.store(true, Ordering::SeqCst);
        let out = h.await.unwrap().unwrap();
        assert_eq!(out.stop_reason, StopReason::Cancelled, "did not report a cancelled stop");
        assert!(
            started.elapsed() < Duration::from_millis(400),
            "cancel took {:?} — it is not checked per token",
            started.elapsed()
        );
        assert_eq!(m.in_flight(), 0);
    }

    #[tokio::test]
    async fn a_failed_load_leaves_the_manager_retryable_not_stuck_loading() {
        let stats = Arc::new(MockStats::default());
        let m = ModelManager::with_idle_timeout(
            Arc::new(MockLoader {
                stats: stats.clone(),
                token_delay: Duration::ZERO,
                tokens: 1,
                output: String::new(),
                fail: true,
            }),
            Duration::from_secs(60),
        );
        let err = m.generate("x".into(), 4, Arc::new(AtomicBool::new(false)), None).await.unwrap_err();
        assert_eq!(err.code(), "validation");
        assert_eq!(m.state(), GenState::NotLoaded, "a failed load left the manager stuck");
        assert_eq!(m.in_flight(), 0, "a failed load leaked a lease");
    }

    #[tokio::test]
    async fn not_installed_is_reported_not_a_confusing_failure() {
        let stats = Arc::new(MockStats::default());
        let m = manager(stats.clone(), Duration::from_secs(60), Duration::ZERO, 1);
        m.set_not_installed();
        assert_eq!(m.state(), GenState::NotInstalled);
        let err = m.generate("x".into(), 4, Arc::new(AtomicBool::new(false)), None).await.unwrap_err();
        assert!(err.to_string().contains("no generative model is installed"), "{err}");
        assert_eq!(stats.loads.load(Ordering::SeqCst), 0, "NotInstalled still attempted a load");
    }
}
