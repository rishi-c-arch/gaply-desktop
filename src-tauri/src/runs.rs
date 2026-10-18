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
//!   reads the cancel flag, then the clock, and a run past its wall-clock
//!   ceiling ([`RUN_CEILING_MS`], 30 s) stops there.
//! * Every state change goes through `contracts::transition`. A cancelled run
//!   is never reported as succeeded, timed out or failed: the worker reads the
//!   cancel flag again under the same lock as its last transition, and a set
//!   flag decides the end, so a cancel that the screen saw accepted while the
//!   run was running always ends `Cancelled`, even when it landed after the
//!   last check point or as the ceiling passed (evaluation of Phases 2 and 3,
//!   F1 and F2).
//! * A record keeps its terminal state for as long as it is held. Nothing the
//!   webview sends changes a finished record: a cancel of one returns its
//!   state unchanged.
//!
//! The shape is `L1Runs`' (a `VecDeque` bounded by a constant, behind a
//! `Mutex`); the code is not. Nothing that holds data is `Serialize` or
//! `Debug`: a slot holds the user's rows and the file's name. The one
//! `Serialize` type is [`RunKind`], a closed word. Nothing here writes a
//! file (`tests/stats_desc_writes_nothing.rs`).
//!
//! Phase 4B-2 (decision 75): a record holds one of two kinds of run, the
//! descriptive summary or the independent-samples t-test, as a closed enum
//! of result kinds. [`Runs::start`] keeps its signature and
//! [`Runs::start_ttest`] is its sibling. One foreground run at a time across
//! both kinds; eviction, the run ids and the slot counter are shared; the
//! worker's last step, where a set cancel flag decides the end, is one for
//! both. A kind's `snapshot`, `cancel` and `result` refuse a run of the
//! other kind with `run.wrong_kind` (decision 74). A t-test calls the check
//! point once, before its arithmetic.
//!
//! For tests, [`Runs::with_hooks`] takes a clock, a hook the check point
//! calls first (a test parks a run there at column k) and the ceiling.
//!
//! Phase 3: the run manager and the slots belong to the Statistics workspace
//! (`crate::workspace`), built when it is activated and dropped when it is
//! left. [`Runs::seal`] is how it is left: under the records' lock it either
//! names the unfinished run, or closes the manager so no run can start after.

use std::collections::VecDeque;
use std::ops::ControlFlow;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Instant;

use gaply_core::contracts::codes::{Closed, RunCode};
use gaply_core::contracts::{
    transition, Diagnostic, OperationId, OperationSpec, Platform, ResultBundle, RunEvent, RunId,
    RunRecord, RunState, SchemaVersion, ValidationReport, ValidationStatus,
};
use gaply_core::stats_adapters;
use gaply_core::stats_desc::{self, DescCode, DescribeParams, DescribeResult, Halt};
use gaply_core::stats_l1::dataset::Dataset;
use gaply_core::stats_ttest::{self, TTestCode, TTestParams, TTestResult};
use serde::Serialize;

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
    /// The check point at `k` let the run go on: the cancel flag and the
    /// clock have been read, and column `k` is about to be summarised. A
    /// test parks here to land a cancel after the last check point.
    Passed(usize),
}

/// Called at each [`Point`]: at `BeforeStart` and `Column(k)` before the
/// run's state, cancel flag or clock is read there, at `Passed(k)` after.
/// Tests only: a test parks a run here.
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
    /// The last slot number handed out. Shared by every engine one workspace
    /// session builds ([`DatasetSlots::numbered_by`]), so an open that lands
    /// in an engine already dropped can never take a number the next engine
    /// hands out (evaluation of Phases 2 and 3, F8).
    next: Arc<AtomicU64>,
    slots: Mutex<VecDeque<(u64, Arc<Slot>)>>,
}

impl DatasetSlots {
    /// Slots numbered from `counter`, which other slot sets may share.
    pub fn numbered_by(counter: Arc<AtomicU64>) -> DatasetSlots {
        DatasetSlots {
            next: counter,
            slots: Mutex::new(VecDeque::new()),
        }
    }

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

    /// The last slot number handed out.
    pub fn last_id(&self) -> u64 {
        self.next.load(Ordering::SeqCst)
    }

    /// Numbers the next slot after `last`, so a slot of an earlier engine is
    /// never reused.
    pub fn continue_after(&self, last: u64) {
        self.next.fetch_max(last, Ordering::SeqCst);
    }
}

/* ------------------------------------------------------------------------ *
 *  Runs
 * ------------------------------------------------------------------------ */

/// The kind of operation a run record holds (Research OS, Phase 4B-2,
/// decision 75): a closed word in the app crate, so `contracts` stays
/// domain-free. The shell reads a run through its kind's commands
/// (decision 80). A word, never data, so it may be `Serialize`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RunKind {
    /// `stats.describe`, through the `stats_desc_*` commands.
    Describe,
    /// `stats.analyze.independent_t`, through the `stats_ttest_*` commands.
    Ttest,
}

/// What one kind of run keeps: its validation, its result and the reason it
/// failed, in that kind's own codes.
struct Outputs<R, C> {
    validation: Option<ValidationReport<C>>,
    bundle: Option<ResultBundle<R, C>>,
    failure: Option<Diagnostic<C>>,
}

impl<R, C> Default for Outputs<R, C> {
    fn default() -> Self {
        Outputs {
            validation: None,
            bundle: None,
            failure: None,
        }
    }
}

/// A record's outputs, by its kind: the closed enum of result kinds
/// (decision 75).
enum Held {
    Describe(Outputs<DescribeResult, DescCode>),
    Ttest(Outputs<TTestResult, TTestCode>),
}

impl Held {
    fn kind(&self) -> RunKind {
        match self {
            Held::Describe(_) => RunKind::Describe,
            Held::Ttest(_) => RunKind::Ttest,
        }
    }
}

struct Entry {
    record: RunRecord,
    /// Every state the record has been in, in order.
    path: Vec<RunState>,
    /// Describe's columns; a t-test has none, and its view does not carry
    /// them.
    columns_total: usize,
    columns_done: usize,
    cancel_requested: bool,
    held: Held,
}

/// What may be known about a run: a copy, never a handle. `C` is its kind's
/// codes: [`Runs::snapshot`] gives a descriptive summary's, and
/// [`Runs::ttest_snapshot`] a t-test's.
#[derive(Clone)]
pub struct RunSnapshot<C = DescCode> {
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
    pub diagnostics: Vec<Diagnostic<C>>,
}

/// Why the check point stopped a run.
enum Stop {
    Cancel,
    TimeOut,
}

/// Why [`Runs::start`] made no record.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotStarted {
    /// Another run is unfinished: `run.run_busy`.
    Busy,
    /// The manager was sealed: its workspace has been left.
    Sealed,
}

/// What a worker runs: one spec of one kind.
enum Job {
    Describe(OperationSpec<DescribeParams>),
    Ttest(OperationSpec<TTestParams>),
}

/// A validation's report, in its kind's codes.
enum Report {
    Describe(ValidationReport<DescCode>),
    Ttest(ValidationReport<TTestCode>),
}

impl Report {
    fn status(&self) -> ValidationStatus {
        match self {
            Report::Describe(r) => r.status,
            Report::Ttest(r) => r.status,
        }
    }
}

/// What the work produced, with the spec it ran: the operation's own answer,
/// or a panic.
enum Done {
    Describe(
        std::thread::Result<Result<DescribeResult, Halt<Stop>>>,
        OperationSpec<DescribeParams>,
    ),
    Ttest(
        std::thread::Result<Result<TTestResult, stats_ttest::Halt<Stop>>>,
        OperationSpec<TTestParams>,
    ),
}

/// The newest record held and the unfinished one, each with its kind.
pub struct RunsBrief {
    pub unfinished: Option<(RunId, RunKind)>,
    pub latest: Option<(RunId, RunKind)>,
}

pub struct Runs {
    next: AtomicU64,
    /// Set by [`Runs::seal`], only while `entries` is locked.
    sealed: AtomicBool,
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

/// The result of a run that succeeded. Otherwise the closed reason:
/// `not_finished`, `cancelled`, `timed_out`, or `no_result`.
fn result_of<R: Clone, C: Clone>(
    state: RunState,
    bundle: &Option<ResultBundle<R, C>>,
) -> Result<ResultBundle<R, C>, RunCode> {
    match (state, bundle) {
        (RunState::Succeeded, Some(bundle)) => Ok(bundle.clone()),
        (RunState::Cancelled, _) => Err(RunCode::Cancelled),
        (RunState::TimedOut, _) => Err(RunCode::TimedOut),
        (RunState::ResourceLimit, _) => Err(RunCode::ResourceLimit),
        (state, _) if !state.is_terminal() => Err(RunCode::NotFinished),
        _ => Err(RunCode::NoResult),
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
            sealed: AtomicBool::new(false),
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

    /// The record of `run` if it is of `kind`: `run.unknown_run` when no
    /// record has that id, `run.wrong_kind` when it is of the other kind
    /// (decision 74).
    fn find_kind(
        entries: &mut VecDeque<Entry>,
        run: RunId,
        kind: RunKind,
    ) -> Result<&mut Entry, RunCode> {
        let entry = Self::find(entries, run).ok_or(RunCode::UnknownRun)?;
        if entry.held.kind() == kind {
            Ok(entry)
        } else {
            Err(RunCode::WrongKind)
        }
    }

    /// Drops the oldest finished records past [`MAX_RUNS`], whatever their
    /// kind. An unfinished one is never dropped.
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
    /// Refused while another run of either kind is unfinished, and once the
    /// manager is sealed. A spec that fails validation still makes a record,
    /// ended `Refused` or `NeedsUserInput`, whose snapshot carries the codes.
    pub fn start(
        self: &Arc<Self>,
        slot: Arc<Slot>,
        spec: OperationSpec<DescribeParams>,
    ) -> Result<RunId, NotStarted> {
        let held = Held::Describe(Outputs::default());
        let run = self.open_record(spec.operation_id, spec.parameters.columns.len(), held)?;
        // Outside the lock, so a status poll does not wait on it; the record is
        // unfinished, so no second run can start meanwhile.
        // Through `stats.describe`'s façade: the capability registry first
        // (Research OS, Phase 4A), then `stats_desc::validate`, unchanged.
        let report =
            stats_adapters::describe_validate(&spec, crate::workspace::build(), slot.dataset());
        self.queue(run, slot, Job::Describe(spec), Report::Describe(report));
        Ok(run)
    }

    /// [`Runs::start`] for a t-test (decision 75): the same record, the same
    /// one foreground run, the same check point and the same last step.
    pub fn start_ttest(
        self: &Arc<Self>,
        slot: Arc<Slot>,
        spec: OperationSpec<TTestParams>,
    ) -> Result<RunId, NotStarted> {
        let run = self.open_record(spec.operation_id, 0, Held::Ttest(Outputs::default()))?;
        // Through `stats.analyze.independent_t`'s façade: the registry first,
        // then `stats_ttest::validate`, unchanged.
        let report =
            stats_adapters::ttest_validate(&spec, crate::workspace::build(), slot.dataset());
        self.queue(run, slot, Job::Ttest(spec), Report::Ttest(report));
        Ok(run)
    }

    /// Makes the record, `Draft → Validating`: refused while another run of
    /// either kind is unfinished, and once the manager is sealed.
    fn open_record(
        &self,
        operation_id: OperationId,
        columns_total: usize,
        held: Held,
    ) -> Result<RunId, NotStarted> {
        let now = self.clock.now_ms();
        let mut entries = lock(&self.entries);
        if self.sealed.load(Ordering::SeqCst) {
            return Err(NotStarted::Sealed);
        }
        if entries.iter().any(|e| !e.record.state.is_terminal()) {
            return Err(NotStarted::Busy);
        }
        let run = RunId(self.next.fetch_add(1, Ordering::SeqCst) + 1);
        let mut entry = Entry {
            record: RunRecord {
                schema_version: SchemaVersion,
                run_id: run,
                operation_id,
                state: RunState::Draft,
                started_ms: now,
                ended_ms: None,
            },
            path: vec![RunState::Draft],
            columns_total,
            columns_done: 0,
            cancel_requested: false,
            held,
        };
        Self::step(&mut entry, RunEvent::BeginValidation, now);
        entries.push_back(entry);
        Self::evict(&mut entries);
        Ok(run)
    }

    /// After the validation: the record's next state from its report, and,
    /// if it may run, the worker on its own thread.
    fn queue(self: &Arc<Self>, run: RunId, slot: Arc<Slot>, job: Job, report: Report) {
        let queued = {
            let now = self.clock.now_ms();
            let mut entries = lock(&self.entries);
            let entry =
                Self::find(&mut entries, run).expect("an unfinished record is never evicted");
            let queued = match report.status() {
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
            match (&mut entry.held, report) {
                (Held::Describe(o), Report::Describe(r)) => o.validation = Some(r),
                (Held::Ttest(o), Report::Ttest(r)) => o.validation = Some(r),
                _ => unreachable!("a record's report is of the record's kind"),
            }
            queued
        };
        if queued {
            let me = Arc::clone(self);
            let spawned = std::thread::Builder::new()
                .name("stats-desc-run".into())
                .spawn(move || me.work(run, slot, job));
            if spawned.is_err() {
                let now = self.clock.now_ms();
                let mut entries = lock(&self.entries);
                if let Some(entry) = Self::find(&mut entries, run) {
                    Self::step(entry, RunEvent::Refuse, now);
                }
            }
        }
    }

    /// The worker: Queued → Running, the operation with its check point,
    /// then exactly one terminal transition, `Cancel` whenever the flag is
    /// set, whatever the kind.
    fn work(self: Arc<Self>, run: RunId, slot: Arc<Slot>, job: Job) {
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
        let build = crate::workspace::build();
        let done = match job {
            Job::Describe(spec) => {
                let outcome = catch_unwind(AssertUnwindSafe(|| {
                    stats_adapters::describe_until(&spec, build, slot.dataset(), |done| {
                        self.checkpoint(run, done, started)
                    })
                }));
                Done::Describe(outcome, spec)
            }
            // The t-test's one check point, before its arithmetic.
            Job::Ttest(spec) => {
                let outcome = catch_unwind(AssertUnwindSafe(|| {
                    stats_adapters::ttest_run(&spec, build, slot.dataset(), |done| {
                        self.checkpoint(run, done, started)
                    })
                }));
                Done::Ttest(outcome, spec)
            }
        };
        let now = self.clock.now_ms();
        let mut entries = lock(&self.entries);
        let Some(entry) = Self::find(&mut entries, run) else {
            return;
        };
        // THE cancel-wins check, one for both kinds (F1, F2, decision 75). A
        // cancel accepted at any time before this lock decides the end,
        // whatever the work produced: a result (the cancel landed after the
        // last check point), the ceiling (it landed after the check point
        // read the flag), a refusal or a panic. The work is discarded and the
        // run ends Cancelled, never Succeeded, TimedOut or Failed. `cancel`
        // accepts only while the run is unfinished, under this same lock, so
        // a cancel that comes later finds the run ended and changes nothing.
        if entry.cancel_requested {
            let panicked = match &done {
                Done::Describe(outcome, _) => {
                    match outcome {
                        Ok(Ok(_)) => entry.columns_done = entry.columns_total,
                        Ok(Err(Halt::Stopped { done, .. })) => entry.columns_done = *done,
                        Ok(Err(Halt::Refused(_))) | Err(_) => {}
                    }
                    outcome.is_err()
                }
                Done::Ttest(outcome, _) => outcome.is_err(),
            };
            if panicked {
                tracing::warn!("stats_desc: the run's work panicked");
            }
            Self::step(entry, RunEvent::Cancel, now);
            return;
        }
        let elapsed = now.saturating_sub(started);
        // Each finish reads the record's outputs of its own kind: a record's
        // work is always of the record's kind (`begin` made both from one
        // job).
        match done {
            Done::Describe(outcome, spec) => {
                Self::finish_describe(entry, run, outcome, &spec, &slot, elapsed, now)
            }
            Done::Ttest(outcome, spec) => {
                Self::finish_ttest(entry, run, outcome, &spec, &slot, elapsed, now)
            }
        }
    }

    /// A describe run's last transition, when no cancel was accepted.
    fn finish_describe(
        entry: &mut Entry,
        run: RunId,
        outcome: std::thread::Result<Result<DescribeResult, Halt<Stop>>>,
        spec: &OperationSpec<DescribeParams>,
        slot: &Slot,
        elapsed: u64,
        now: u64,
    ) {
        match outcome {
            Ok(Ok(result)) => {
                entry.columns_done = entry.columns_total;
                let manifest = stats_desc::manifest(
                    spec,
                    slot.dataset(),
                    &result,
                    elapsed,
                    Platform::current(),
                );
                let succeeded = Self::step(entry, RunEvent::Succeed, now);
                let Held::Describe(o) = &mut entry.held else {
                    unreachable!("a describe record")
                };
                let diagnostics = o
                    .validation
                    .as_ref()
                    .map(|v| v.warnings.clone())
                    .unwrap_or_default();
                if succeeded {
                    o.bundle = Some(ResultBundle {
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
                Self::step(entry, Self::stop_event(reason), now);
            }
            Ok(Err(Halt::Refused(d))) => {
                if let Held::Describe(o) = &mut entry.held {
                    o.failure = Some(d);
                }
                Self::step(entry, RunEvent::Fail, now);
            }
            Err(_) => {
                tracing::warn!("stats_desc: the describe loop panicked");
                Self::step(entry, RunEvent::Fail, now);
            }
        }
    }

    /// A t-test run's last transition, when no cancel was accepted.
    fn finish_ttest(
        entry: &mut Entry,
        run: RunId,
        outcome: std::thread::Result<Result<TTestResult, stats_ttest::Halt<Stop>>>,
        spec: &OperationSpec<TTestParams>,
        slot: &Slot,
        elapsed: u64,
        now: u64,
    ) {
        match outcome {
            Ok(Ok(result)) => {
                let manifest = stats_ttest::manifest(
                    spec,
                    slot.dataset(),
                    &result,
                    elapsed,
                    Platform::current(),
                );
                let succeeded = Self::step(entry, RunEvent::Succeed, now);
                let Held::Ttest(o) = &mut entry.held else {
                    unreachable!("a t-test record")
                };
                let diagnostics = o
                    .validation
                    .as_ref()
                    .map(|v| v.warnings.clone())
                    .unwrap_or_default();
                if succeeded {
                    let limitations = result
                        .limitations
                        .iter()
                        .map(|c| Diagnostic::note(*c))
                        .collect();
                    o.bundle = Some(ResultBundle {
                        schema_version: SchemaVersion,
                        run_id: run,
                        status: RunState::Succeeded,
                        primary_outputs: result,
                        diagnostics,
                        limitations,
                        manifest,
                    });
                }
            }
            Ok(Err(stats_ttest::Halt::Stopped { reason })) => {
                Self::step(entry, Self::stop_event(reason), now);
            }
            Ok(Err(stats_ttest::Halt::Refused(d))) => {
                if let Held::Ttest(o) = &mut entry.held {
                    o.failure = Some(d);
                }
                Self::step(entry, RunEvent::Fail, now);
            }
            Err(_) => {
                tracing::warn!("stats_desc: the t-test panicked");
                Self::step(entry, RunEvent::Fail, now);
            }
        }
    }

    fn stop_event(reason: Stop) -> RunEvent {
        match reason {
            Stop::Cancel => RunEvent::Cancel,
            Stop::TimeOut => RunEvent::TimeOut,
        }
    }

    /// Before each column of a summary, and once before a t-test's
    /// arithmetic: the test hook, then the cancel flag, then the clock.
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
        if let Some(hook) = &self.hook {
            hook(run, Point::Passed(done));
        }
        ControlFlow::Continue(())
    }

    fn snapshot_of<R, C: Clone>(&self, entry: &Entry, outputs: &Outputs<R, C>) -> RunSnapshot<C> {
        let end = entry.record.ended_ms.unwrap_or_else(|| self.clock.now_ms());
        let mut diagnostics = Vec::new();
        if let Some(v) = &outputs.validation {
            diagnostics.extend(v.errors.iter().cloned());
            diagnostics.extend(v.missing_information.iter().cloned());
            diagnostics.extend(v.warnings.iter().cloned());
        }
        diagnostics.extend(outputs.failure.iter().cloned());
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

    fn describe_snapshot(&self, entry: &Entry) -> RunSnapshot {
        match &entry.held {
            Held::Describe(o) => self.snapshot_of(entry, o),
            Held::Ttest(_) => unreachable!("found by kind"),
        }
    }

    fn ttest_snapshot_of(&self, entry: &Entry) -> RunSnapshot<TTestCode> {
        match &entry.held {
            Held::Ttest(o) => self.snapshot_of(entry, o),
            Held::Describe(_) => unreachable!("found by kind"),
        }
    }

    /// A descriptive summary's run; `run.wrong_kind` for a t-test's.
    pub fn snapshot(&self, run: RunId) -> Result<RunSnapshot, RunCode> {
        let mut entries = lock(&self.entries);
        let entry = Self::find_kind(&mut entries, run, RunKind::Describe)?;
        Ok(self.describe_snapshot(entry))
    }

    /// A t-test's run; `run.wrong_kind` for a descriptive summary's.
    pub fn ttest_snapshot(&self, run: RunId) -> Result<RunSnapshot<TTestCode>, RunCode> {
        let mut entries = lock(&self.entries);
        let entry = Self::find_kind(&mut entries, run, RunKind::Ttest)?;
        Ok(self.ttest_snapshot_of(entry))
    }

    /// Asks the run to stop. Queued: cancelled now. Running: the flag is set,
    /// and the next check point, or the worker's last step if no check point
    /// is left, ends it `Cancelled`. Finished: unchanged.
    fn cancel_entry(entry: &mut Entry, now: u64) {
        match entry.record.state {
            RunState::Queued => {
                entry.cancel_requested = true;
                Self::step(entry, RunEvent::Cancel, now);
            }
            state if !state.is_terminal() => entry.cancel_requested = true,
            _ => {}
        }
    }

    /// Cancels a descriptive summary's run; `run.wrong_kind` for a t-test's.
    pub fn cancel(&self, run: RunId) -> Result<RunSnapshot, RunCode> {
        let now = self.clock.now_ms();
        let mut entries = lock(&self.entries);
        let entry = Self::find_kind(&mut entries, run, RunKind::Describe)?;
        Self::cancel_entry(entry, now);
        Ok(self.describe_snapshot(entry))
    }

    /// Cancels a t-test's run; `run.wrong_kind` for a descriptive summary's.
    pub fn ttest_cancel(&self, run: RunId) -> Result<RunSnapshot<TTestCode>, RunCode> {
        let now = self.clock.now_ms();
        let mut entries = lock(&self.entries);
        let entry = Self::find_kind(&mut entries, run, RunKind::Ttest)?;
        Self::cancel_entry(entry, now);
        Ok(self.ttest_snapshot_of(entry))
    }

    /// The result of a descriptive summary that succeeded. Otherwise the
    /// closed reason: `not_finished`, `cancelled`, `timed_out`, `no_result`,
    /// or `wrong_kind` for a t-test's run.
    pub fn result(&self, run: RunId) -> Result<ResultBundle<DescribeResult, DescCode>, RunCode> {
        let mut entries = lock(&self.entries);
        let entry = Self::find_kind(&mut entries, run, RunKind::Describe)?;
        match &entry.held {
            Held::Describe(o) => result_of(entry.record.state, &o.bundle),
            Held::Ttest(_) => unreachable!("found by kind"),
        }
    }

    /// [`Runs::result`] for a t-test; `wrong_kind` for a summary's run.
    pub fn ttest_result(
        &self,
        run: RunId,
    ) -> Result<ResultBundle<TTestResult, TTestCode>, RunCode> {
        let mut entries = lock(&self.entries);
        let entry = Self::find_kind(&mut entries, run, RunKind::Ttest)?;
        match &entry.held {
            Held::Ttest(o) => result_of(entry.record.state, &o.bundle),
            Held::Describe(_) => unreachable!("found by kind"),
        }
    }

    /// How many records are held.
    pub fn held(&self) -> usize {
        lock(&self.entries).len()
    }

    /// The run that is queued or running, if any (at most one: one
    /// foreground run at a time).
    pub fn unfinished(&self) -> Option<RunId> {
        lock(&self.entries)
            .iter()
            .find(|e| !e.record.state.is_terminal())
            .map(|e| e.record.run_id)
    }

    /// The newest record held, finished or not.
    pub fn latest(&self) -> Option<RunId> {
        lock(&self.entries).back().map(|e| e.record.run_id)
    }

    /// [`Runs::unfinished`] and [`Runs::latest`], each with its kind, read
    /// under one lock (decision 80).
    pub fn brief(&self) -> RunsBrief {
        let entries = lock(&self.entries);
        let named = |e: &Entry| (e.record.run_id, e.held.kind());
        RunsBrief {
            unfinished: entries
                .iter()
                .find(|e| !e.record.state.is_terminal())
                .map(named),
            latest: entries.back().map(named),
        }
    }

    /// Closes the manager to new runs, unless one is unfinished: then it is
    /// left open and that run's id is the answer. One step under the records'
    /// lock, so no run can start between the check and the seal.
    pub fn seal(&self) -> Result<(), RunId> {
        let entries = lock(&self.entries);
        if let Some(e) = entries.iter().find(|e| !e.record.state.is_terminal()) {
            return Err(e.record.run_id);
        }
        self.sealed.store(true, Ordering::SeqCst);
        Ok(())
    }

    /// The last run id handed out.
    pub fn last_id(&self) -> u64 {
        self.next.load(Ordering::SeqCst)
    }

    /// Numbers the next run after `last`, so a run id of an earlier engine is
    /// never reused.
    pub fn continue_after(&self, last: u64) {
        self.next.fetch_max(last, Ordering::SeqCst);
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
            held: Held::Describe(Outputs::default()),
        }
    }

    /// A t-test's record, as [`entry`] makes a descriptive summary's.
    fn ttest_entry(run: u64, state: RunState) -> Entry {
        Entry {
            held: Held::Ttest(Outputs::default()),
            columns_total: 0,
            ..entry(run, state)
        }
    }

    /// Phase 4B-2 (decision 75): eviction reads the state, never the kind. A
    /// t-test parked oldest is kept among finished summaries, the oldest
    /// finished record goes whichever kind it is, and a parked summary is
    /// kept among finished t-tests.
    #[test]
    fn eviction_keeps_a_parked_entry_and_drops_the_oldest_finished_one_whatever_its_kind() {
        let kinds = |entries: &VecDeque<Entry>| -> Vec<(u64, RunKind)> {
            entries
                .iter()
                .map(|e| (e.record.run_id.0, e.held.kind()))
                .collect()
        };

        // A parked t-test, oldest, then finished records of both kinds.
        let mut entries: VecDeque<Entry> = VecDeque::new();
        entries.push_back(ttest_entry(1, RunState::Running));
        entries.push_back(ttest_entry(2, RunState::Succeeded));
        for run in 3..=MAX_RUNS as u64 + 1 {
            entries.push_back(entry(run, RunState::Cancelled));
        }
        Runs::evict(&mut entries);
        let held = kinds(&entries);
        assert_eq!(held.len(), MAX_RUNS);
        assert_eq!(held[0], (1, RunKind::Ttest), "the parked t-test is kept");
        assert!(
            !held.iter().any(|(r, _)| *r == 2),
            "the oldest finished record, a t-test, is dropped: {held:?}"
        );

        // A parked summary, oldest, and the oldest finished record a summary
        // among t-tests.
        let mut entries: VecDeque<Entry> = VecDeque::new();
        entries.push_back(entry(1, RunState::Queued));
        entries.push_back(entry(2, RunState::Failed));
        for run in 3..=MAX_RUNS as u64 + 1 {
            entries.push_back(ttest_entry(run, RunState::Succeeded));
        }
        Runs::evict(&mut entries);
        let held = kinds(&entries);
        assert_eq!(held.len(), MAX_RUNS);
        assert_eq!(
            held[0],
            (1, RunKind::Describe),
            "the parked summary is kept"
        );
        assert!(
            !held.iter().any(|(r, _)| *r == 2),
            "the oldest finished record, a summary, is dropped: {held:?}"
        );
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

    /// Phase 3. Leaving the Statistics workspace seals its run manager. With a
    /// run unfinished the seal is refused with that run's id and the manager
    /// stays open; once sealed, a start makes no record. The seal and the
    /// start take the same lock, so there is no gap between them.
    #[test]
    fn a_seal_names_the_unfinished_run_and_a_sealed_manager_starts_nothing() {
        use gaply_core::stats_desc::{Alias, ColumnRequest, TreatAs};
        use gaply_core::stats_l1::dataset::{
            build_dataset, ColumnAlias, DataFingerprint, RawTable, TableRef,
        };

        let runs = Arc::new(Runs::default());
        lock(&runs.entries).push_back(entry(7, RunState::Running));
        assert_eq!(runs.seal(), Err(RunId(7)));
        assert_eq!(runs.unfinished(), Some(RunId(7)));
        assert!(!runs.sealed.load(Ordering::SeqCst), "left open");

        lock(&runs.entries).clear();
        lock(&runs.entries).push_back(entry(8, RunState::Cancelled));
        assert_eq!(runs.seal(), Ok(()));
        assert_eq!(runs.latest(), Some(RunId(8)));

        let dataset = build_dataset(
            RawTable::from_rows(vec!["x".into()], vec![vec!["1".into()], vec!["2".into()]]),
            DataFingerprint::of_bytes(b"synthetic", "unit", TableRef::FirstTable),
        )
        .expect("dataset");
        let spec = stats_desc::spec(
            OperationId(1),
            &dataset,
            DescribeParams {
                columns: vec![ColumnRequest {
                    alias: Alias(ColumnAlias(1)),
                    treat_as: TreatAs::Numeric,
                }],
                missing_codes: vec![],
            },
        );
        let slot = Arc::new(Slot {
            display_name: "synthetic".into(),
            dataset,
        });
        assert_eq!(runs.start(slot, spec), Err(NotStarted::Sealed));
        assert_eq!(runs.held(), 1, "no record was made");
    }

    #[test]
    fn ids_continue_after_an_earlier_engine() {
        let runs = Runs::default();
        runs.continue_after(5);
        assert_eq!(runs.last_id(), 5);
        runs.continue_after(3);
        assert_eq!(runs.last_id(), 5, "never goes back");
        let slots = DatasetSlots::default();
        slots.continue_after(4);
        assert_eq!(slots.last_id(), 4);
    }
}
