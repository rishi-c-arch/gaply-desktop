//! **The Research OS run manager and dataset slots (Phase 2B): in memory
//! only, bounded, one foreground run at a time.**
//!
//! * [`DatasetSlots`] holds at most [`MAX_SLOTS`] opened datasets, each with
//!   its file's display name (the base name, never a path; decision 4).
//!   Opening a fifth drops the oldest. A slot is also dropped when it is
//!   closed, and every slot when the app exits.
//! * [`Runs`] holds at most [`MAX_RUNS`] run records (decision 6). Adding a
//!   ninth drops the oldest FINISHED record; a queued or running record is
//!   never dropped.
//! * A run is detached: [`Runs::start`] creates the record, validates the
//!   spec, queues it, starts the work on its own thread and returns the run
//!   id at once. The screen polls [`Runs::snapshot`].
//! * One foreground run at a time: while any record is unfinished, a second
//!   start is refused with `run.run_busy`.
//! * Cancel is cooperative. The describe loop calls this manager's check
//!   point before each column (`stats_desc::describe_until`); the check point
//!   reads the cancel flag and the clock, and a run past its wall-clock
//!   ceiling ([`RUN_CEILING_MS`], 30 s) ends `TimedOut` there.
//! * Every state change goes through `contracts::transition`. A cancelled run
//!   is never reported as succeeded: the worker reads the cancel flag under
//!   the same lock as its last transition, so a cancel that the screen saw
//!   accepted while the run was running always ends `Cancelled`.
//! * A record keeps its terminal state for as long as it is held. Nothing the
//!   webview sends changes a finished record: a cancel of one returns its
//!   state unchanged.
//!
//! The shape is `L1Runs`' (a `VecDeque` bounded by a constant, behind a
//! `Mutex`); the code is not. Nothing here is `Serialize` or `Debug`: a slot
//! holds the user's rows and the file's name. Nothing here writes a file
//! (`tests/stats_desc_writes_nothing.rs`).
//!
//! For tests, [`Runs::with_hooks`] takes a clock, a hook the check point
//! calls first (a test parks a run there at column k) and the ceiling.

use std::collections::VecDeque;
use std::ops::ControlFlow;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Instant;

use gaply_core::contracts::codes::{Closed, RunCode};
use gaply_core::contracts::{
    transition, Diagnostic, OperationSpec, Platform, ResultBundle, RunEvent, RunId, RunRecord,
    RunState, SchemaVersion, ValidationReport, ValidationStatus,
};
use gaply_core::stats_desc::{self, DescCode, DescribeParams, DescribeResult, Halt};
use gaply_core::stats_l1::dataset::Dataset;

/// Run records held at once (decision 6).
pub const MAX_RUNS: usize = 8;
/// Dataset slots held at once (decision 6).
pub const MAX_SLOTS: usize = 4;
/// A run's wall-clock ceiling, from its start (Queued → Running).
pub const RUN_CEILING_MS: u64 = 30_000;

/// Milliseconds on a clock that never goes back. The run records'
/// `startedMs` and `endedMs` are read from it: in production, milliseconds
/// since the manager was made, never a date.
pub trait Clock: Send + Sync {
    fn now_ms(&self) -> u64;
}

struct SinceStart(Instant);

impl Clock for SinceStart {
    fn now_ms(&self) -> u64 {
        u64::try_from(self.0.elapsed().as_millis()).unwrap_or(u64::MAX)
    }
}

/// Where a hook is called: just before the worker takes a queued run, and at
/// each check point with the number of columns done.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Point {
    BeforeStart,
    Column(usize),
}

/// Called at each [`Point`], before the run's state, cancel flag or clock is
/// read there. Tests only: a test parks a run here.
pub type CheckpointHook = Arc<dyn Fn(RunId, Point) + Send + Sync>;

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

/* ------------------------------------------------------------------------ *
 *  Dataset slots
 * ------------------------------------------------------------------------ */

/// One opened dataset and its file's display name. Never `Debug`, never
/// `Serialize`: it holds the user's rows.
pub struct Slot {
    display_name: String,
    dataset: Dataset,
}

impl Slot {
    pub fn display_name(&self) -> &str {
        &self.display_name
    }
    pub fn dataset(&self) -> &Dataset {
        &self.dataset
    }
}

#[derive(Default)]
pub struct DatasetSlots {
    next: AtomicU64,
    slots: Mutex<VecDeque<(u64, Arc<Slot>)>>,
}

impl DatasetSlots {
    /// Holds the dataset and returns its slot. A fifth drops the oldest; a run
    /// already using it keeps its own handle until it ends.
    pub fn insert(&self, display_name: String, dataset: Dataset) -> u64 {
        let id = self.next.fetch_add(1, Ordering::SeqCst) + 1;
        let mut slots = lock(&self.slots);
        slots.push_back((
            id,
            Arc::new(Slot {
                display_name,
                dataset,
            }),
        ));
        while slots.len() > MAX_SLOTS {
            slots.pop_front();
        }
        id
    }

    pub fn get(&self, slot: u64) -> Result<Arc<Slot>, RunCode> {
        lock(&self.slots)
            .iter()
            .find(|(s, _)| *s == slot)
            .map(|(_, d)| d.clone())
            .ok_or(RunCode::UnknownInput)
    }

    /// Drops the slot. True if it was held.
    pub fn close(&self, slot: u64) -> bool {
        let mut slots = lock(&self.slots);
        let before = slots.len();
        slots.retain(|(s, _)| *s != slot);
        slots.len() != before
    }

    pub fn held(&self) -> usize {
        lock(&self.slots).len()
    }
}

/* ------------------------------------------------------------------------ *
 *  Runs
 * ------------------------------------------------------------------------ */

struct Entry {
    record: RunRecord,
    /// Every state the record has been in, in order.
    path: Vec<RunState>,
    columns_total: usize,
    columns_done: usize,
    cancel_requested: bool,
    validation: Option<ValidationReport<DescCode>>,
    bundle: Option<ResultBundle<DescribeResult, DescCode>>,
    failure: Option<Diagnostic<DescCode>>,
}

/// What may be known about a run: a copy, never a handle.
#[derive(Clone)]
pub struct RunSnapshot {
    pub record: RunRecord,
    /// Every state the record has been in, in order, from `Draft`.
    pub path: Vec<RunState>,
    pub columns_done: usize,
    pub columns_total: usize,
    pub cancel_requested: bool,
    /// Since the record was made; to its end once it has ended.
    pub elapsed_ms: u64,
    /// The validation's errors, open questions and warnings, then a failure's
    /// code: closed codes with numeric parameters.
    pub diagnostics: Vec<Diagnostic<DescCode>>,
}

/// Why the check point stopped a run.
enum Stop {
    Cancel,
    TimeOut,
}

pub struct Runs {
    next: AtomicU64,
    entries: Mutex<VecDeque<Entry>>,
    clock: Arc<dyn Clock>,
    hook: Option<CheckpointHook>,
    ceiling_ms: u64,
}

impl Default for Runs {
    fn default() -> Runs {
        Runs::with_hooks(Arc::new(SinceStart(Instant::now())), None, RUN_CEILING_MS)
    }
}

impl Runs {
    /// A manager with an injected clock, check-point hook and ceiling.
    pub fn with_hooks(
        clock: Arc<dyn Clock>,
        hook: Option<CheckpointHook>,
        ceiling_ms: u64,
    ) -> Runs {
        Runs {
            next: AtomicU64::new(0),
            entries: Mutex::new(VecDeque::new()),
            clock,
            hook,
            ceiling_ms,
        }
    }

    /// Applies `event` through `transition`, stamping the end time when the
    /// state becomes terminal. False, and nothing changed, when the table has
    /// no such edge.
    fn step(entry: &mut Entry, event: RunEvent, now: u64) -> bool {
        match transition(entry.record.state, event) {
            Ok(to) => {
                entry.record.state = to;
                entry.path.push(to);
                if to.is_terminal() {
                    entry.record.ended_ms = Some(now);
                    tracing::info!(
                        run = entry.record.run_id.0,
                        state = to.wire(),
                        "stats_desc: run ended"
                    );
                }
                true
            }
            Err(d) => {
                tracing::warn!(code = ?d.code, "stats_desc: a run event had no edge");
                false
            }
        }
    }

    fn find(entries: &mut VecDeque<Entry>, run: RunId) -> Option<&mut Entry> {
        entries.iter_mut().find(|e| e.record.run_id == run)
    }

    /// Drops the oldest finished records past [`MAX_RUNS`]. An unfinished one
    /// is never dropped.
    fn evict(entries: &mut VecDeque<Entry>) {
        while entries.len() > MAX_RUNS {
            match entries.iter().position(|e| e.record.state.is_terminal()) {
                Some(i) => {
                    entries.remove(i);
                }
                None => break,
            }
        }
    }

    /// Creates the run, validates `spec` on the slot's dataset and, if it may
    /// run, queues it and starts it on its own thread. Returns at once.
    /// Refused with `run.run_busy` while another run is unfinished. A spec
    /// that fails validation still makes a record, ended `Refused` or
    /// `NeedsUserInput`, whose snapshot carries the codes.
    pub fn start(
        self: &Arc<Self>,
        slot: Arc<Slot>,
        spec: OperationSpec<DescribeParams>,
    ) -> Result<RunId, RunCode> {
        let now = self.clock.now_ms();
        let run = {
            let mut entries = lock(&self.entries);
            if entries.iter().any(|e| !e.record.state.is_terminal()) {
                return Err(RunCode::RunBusy);
            }
            let run = RunId(self.next.fetch_add(1, Ordering::SeqCst) + 1);
            let mut entry = Entry {
                record: RunRecord {
                    schema_version: SchemaVersion,
                    run_id: run,
                    operation_id: spec.operation_id,
                    state: RunState::Draft,
                    started_ms: now,
                    ended_ms: None,
                },
                path: vec![RunState::Draft],
                columns_total: spec.parameters.columns.len(),
                columns_done: 0,
                cancel_requested: false,
                validation: None,
                bundle: None,
                failure: None,
            };
            Self::step(&mut entry, RunEvent::BeginValidation, now);
            entries.push_back(entry);
            Self::evict(&mut entries);
            run
        };

        // Outside the lock, so a status poll does not wait on it; the record is
        // unfinished, so no second run can start meanwhile.
        let report = stats_desc::validate(slot.dataset(), &spec);
        let queued = {
            let now = self.clock.now_ms();
            let mut entries = lock(&self.entries);
            let entry =
                Self::find(&mut entries, run).expect("an unfinished record is never evicted");
            let queued = match report.status {
                ValidationStatus::Valid | ValidationStatus::ValidWithWarnings => {
                    Self::step(entry, RunEvent::PassValidation, now)
                        && Self::step(entry, RunEvent::Enqueue, now)
                }
                ValidationStatus::NeedsUserInput => {
                    Self::step(entry, RunEvent::RequestInput, now);
                    false
                }
                ValidationStatus::Unsupported | ValidationStatus::Invalid => {
                    Self::step(entry, RunEvent::Refuse, now);
                    false
                }
            };
            entry.validation = Some(report);
            queued
        };
        if queued {
            let me = Arc::clone(self);
            let spawned = std::thread::Builder::new()
                .name("stats-desc-run".into())
                .spawn(move || me.work(run, slot, spec));
            if spawned.is_err() {
                let now = self.clock.now_ms();
                let mut entries = lock(&self.entries);
                if let Some(entry) = Self::find(&mut entries, run) {
                    Self::step(entry, RunEvent::Refuse, now);
                }
            }
        }
        Ok(run)
    }

    /// The worker: Queued → Running, the describe loop with its check point,
    /// then exactly one terminal transition.
    fn work(self: Arc<Self>, run: RunId, slot: Arc<Slot>, spec: OperationSpec<DescribeParams>) {
        if let Some(hook) = &self.hook {
            hook(run, Point::BeforeStart);
        }
        let started = self.clock.now_ms();
        {
            let mut entries = lock(&self.entries);
            let Some(entry) = Self::find(&mut entries, run) else {
                return;
            };
            // Cancelled while queued: it has ended, and nothing runs.
            if entry.record.state != RunState::Queued
                || !Self::step(entry, RunEvent::Start, started)
            {
                return;
            }
        }
        let outcome = catch_unwind(AssertUnwindSafe(|| {
            stats_desc::describe_until(slot.dataset(), &spec.parameters, |done| {
                self.checkpoint(run, done, started)
            })
        }));
        let now = self.clock.now_ms();
        let mut entries = lock(&self.entries);
        let Some(entry) = Self::find(&mut entries, run) else {
            return;
        };
        match outcome {
            // Every column was done, but the cancel came first: the work is
            // discarded and the run ends Cancelled, never Succeeded.
            Ok(Ok(result)) if entry.cancel_requested => {
                entry.columns_done = entry.columns_total;
                Self::step(entry, RunEvent::Cancel, now);
                drop(result);
            }
            Ok(Ok(result)) => {
                entry.columns_done = entry.columns_total;
                let manifest = stats_desc::manifest(
                    &spec,
                    slot.dataset(),
                    &result,
                    now.saturating_sub(started),
                    Platform::current(),
                );
                let diagnostics = entry
                    .validation
                    .as_ref()
                    .map(|v| v.warnings.clone())
                    .unwrap_or_default();
                if Self::step(entry, RunEvent::Succeed, now) {
                    entry.bundle = Some(ResultBundle {
                        schema_version: SchemaVersion,
                        run_id: run,
                        status: RunState::Succeeded,
                        primary_outputs: result,
                        diagnostics,
                        limitations: Vec::new(),
                        manifest,
                    });
                }
            }
            Ok(Err(Halt::Stopped { reason, done })) => {
                entry.columns_done = done;
                let event = match reason {
                    Stop::Cancel => RunEvent::Cancel,
                    Stop::TimeOut => RunEvent::TimeOut,
                };
                Self::step(entry, event, now);
            }
            Ok(Err(Halt::Refused(d))) => {
                entry.failure = Some(d);
                Self::step(entry, RunEvent::Fail, now);
            }
            Err(_) => {
                tracing::warn!("stats_desc: the describe loop panicked");
                Self::step(entry, RunEvent::Fail, now);
            }
        }
    }

    /// Before each column: the test hook, then the cancel flag, then the clock.
    fn checkpoint(&self, run: RunId, done: usize, started: u64) -> ControlFlow<Stop> {
        if let Some(hook) = &self.hook {
            hook(run, Point::Column(done));
        }
        let cancelled = {
            let mut entries = lock(&self.entries);
            match Self::find(&mut entries, run) {
                Some(entry) => {
                    entry.columns_done = done;
                    entry.cancel_requested
                }
                None => true,
            }
        };
        if cancelled {
            return ControlFlow::Break(Stop::Cancel);
        }
        if self.clock.now_ms().saturating_sub(started) >= self.ceiling_ms {
            return ControlFlow::Break(Stop::TimeOut);
        }
        ControlFlow::Continue(())
    }

    fn snapshot_of(&self, entry: &Entry) -> RunSnapshot {
        let end = entry.record.ended_ms.unwrap_or_else(|| self.clock.now_ms());
        let mut diagnostics = Vec::new();
        if let Some(v) = &entry.validation {
            diagnostics.extend(v.errors.iter().cloned());
            diagnostics.extend(v.missing_information.iter().cloned());
            diagnostics.extend(v.warnings.iter().cloned());
        }
        diagnostics.extend(entry.failure.iter().cloned());
        RunSnapshot {
            record: entry.record.clone(),
            path: entry.path.clone(),
            columns_done: entry.columns_done,
            columns_total: entry.columns_total,
            cancel_requested: entry.cancel_requested,
            elapsed_ms: end.saturating_sub(entry.record.started_ms),
            diagnostics,
        }
    }

    pub fn snapshot(&self, run: RunId) -> Result<RunSnapshot, RunCode> {
        let mut entries = lock(&self.entries);
        let entry = Self::find(&mut entries, run).ok_or(RunCode::UnknownRun)?;
        Ok(self.snapshot_of(entry))
    }

    /// Asks the run to stop. Queued: cancelled now. Running: the flag is set
    /// and the next check point ends it `Cancelled`. Finished: unchanged.
    pub fn cancel(&self, run: RunId) -> Result<RunSnapshot, RunCode> {
        let now = self.clock.now_ms();
        let mut entries = lock(&self.entries);
        let entry = Self::find(&mut entries, run).ok_or(RunCode::UnknownRun)?;
        match entry.record.state {
            RunState::Queued => {
                entry.cancel_requested = true;
                Self::step(entry, RunEvent::Cancel, now);
            }
            state if !state.is_terminal() => entry.cancel_requested = true,
            _ => {}
        }
        Ok(self.snapshot_of(entry))
    }

    /// The result of a run that succeeded. Otherwise the closed reason:
    /// `not_finished`, `cancelled`, `timed_out`, or `no_result`.
    pub fn result(&self, run: RunId) -> Result<ResultBundle<DescribeResult, DescCode>, RunCode> {
        let mut entries = lock(&self.entries);
        let entry = Self::find(&mut entries, run).ok_or(RunCode::UnknownRun)?;
        match (entry.record.state, &entry.bundle) {
            (RunState::Succeeded, Some(bundle)) => Ok(bundle.clone()),
            (RunState::Cancelled, _) => Err(RunCode::Cancelled),
            (RunState::TimedOut, _) => Err(RunCode::TimedOut),
            (RunState::ResourceLimit, _) => Err(RunCode::ResourceLimit),
            (state, _) if !state.is_terminal() => Err(RunCode::NotFinished),
            _ => Err(RunCode::NoResult),
        }
    }

    /// How many records are held.
    pub fn held(&self) -> usize {
        lock(&self.entries).len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gaply_core::contracts::OperationId;

    fn entry(run: u64, state: RunState) -> Entry {
        Entry {
            record: RunRecord {
                schema_version: SchemaVersion,
                run_id: RunId(run),
                operation_id: OperationId(run),
                state,
                started_ms: 0,
                ended_ms: None,
            },
            path: vec![state],
            columns_total: 1,
            columns_done: 0,
            cancel_requested: false,
            validation: None,
            bundle: None,
            failure: None,
        }
    }

    /// Through the commands a running record is always the newest, because a
    /// second start is refused while one is unfinished. So the rule that an
    /// unfinished record is never dropped is pinned here, with the running
    /// record OLDEST, where only the rule can keep it.
    #[test]
    fn eviction_drops_the_oldest_finished_record_and_never_an_unfinished_one() {
        let mut entries: VecDeque<Entry> = VecDeque::new();
        entries.push_back(entry(1, RunState::Running));
        for run in 2..=MAX_RUNS as u64 + 1 {
            entries.push_back(entry(run, RunState::Succeeded));
        }
        assert_eq!(entries.len(), MAX_RUNS + 1);
        Runs::evict(&mut entries);
        let held: Vec<u64> = entries.iter().map(|e| e.record.run_id.0).collect();
        assert_eq!(held.len(), MAX_RUNS);
        assert_eq!(held[0], 1, "the running record is kept");
        assert!(
            !held.contains(&2),
            "the oldest finished record is dropped: {held:?}"
        );

        // All unfinished: nothing is dropped, rather than one that is running.
        let mut busy: VecDeque<Entry> = (1..=MAX_RUNS as u64 + 1)
            .map(|r| entry(r, RunState::Queued))
            .collect();
        Runs::evict(&mut busy);
        assert_eq!(busy.len(), MAX_RUNS + 1);
    }
}
