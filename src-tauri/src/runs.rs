//! **The Research OS run manager and dataset slots (Phase 2B): in memory
//! only, bounded, one foreground run at a time.**
//!
//! * [`DatasetSlots`] holds at most [`MAX_SLOTS`] opened datasets, each with
//!   its file's display name (the base name, never a path; decision 4).
//!   Opening a fifth drops the oldest. A slot is also dropped when it is
//!   closed, and every slot when the app exits.
//! * [`Runs`] holds at most [`MAX_RUNS`] run records: 40 since ADR-012 raised
//!   decision 6's 8 (decision 468), so the Output's 32 entries and a going
//!   rerun all keep theirs. Adding one past that drops a rerun's finished
//!   record first, then the oldest FINISHED record of any kind; a queued or
//!   running record is never dropped.
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
//! Phase 5B-2 (decision 123): a slot opened through `stats_desc_open` also
//! holds the original file's bytes, with their SHA-256 taken at ingest and
//! their format ([`SourceFile`]), for as long as the slot is held: closing or
//! evicting the slot drops them. Each record keeps its spec's canonical bytes
//! and the slot and hash of the data it read ([`RunInput`]), and
//! [`Runs::copy_for_save`] hands a project Save a copy of what it keeps.
//!
//! Phase 5C-1a (decisions 186 and 190): a rerun starts with
//! [`Runs::start_then`] or [`Runs::start_ttest_then`], on a [`Slot`] made by
//! [`Slot::for_one_run`] that no [`DatasetSlots`] holds, and with an
//! [`EndHook`]. The hook is kept with the record and taken out at whichever
//! step ends it: a refusal in `queue`, a failed spawn, a cancel while queued,
//! or the worker's last step. It is called once, after the records' lock is
//! released, with the copy taken under that lock. Runs started with
//! [`Runs::start`] and [`Runs::start_ttest`] have no hook and behave as
//! before. This module still writes nothing: the hook is its caller's.
//!
//! Research OS, Phase 8G-2 (decisions 317 and 328): a record may hold a
//! one-way ANOVA, started with [`Runs::start_anova`]. Its check points are
//! `stats_anova::run`'s, one before the arithmetic and one before each post
//! hoc pair, and the record's `columns_total` is `stats_anova::check_points`
//! for the levels the run includes, so its progress counts check points. A
//! method's `columns_total` reads the slot's dataset as well as the spec,
//! since with no levels named the ANOVA includes every level the dataset's
//! group holds; the other four ignore it.
//!
//! Research OS, Phase 8H-2 (decision 355): a record may hold correlations,
//! started with [`Runs::start_corr`]. Their check points are
//! `stats_corr::run`'s, one before the arithmetic and one before each pair of
//! each chosen coefficient, and the record's `columns_total` is
//! `stats_corr::check_points`, from the spec alone, so their progress counts
//! check points too. A spec over the budget is refused by validation, so it
//! never starts.
//!
//! Research OS, Phase 8I-2 (decision 380): a record may hold Cronbach's
//! alpha, started with [`Runs::start_alpha`]. Its check points are
//! `stats_alpha::run`'s, one before the arithmetic, one before each item's
//! row and one before each inter-item correlation, and the record's
//! `columns_total` is `stats_alpha::check_points`, from the spec alone. It has
//! no budget (decision 371); the ceiling applies, as to every kind. A spec
//! validation refuses never starts.
//!
//! Research OS, Phase 8X-1 (decision 580): a record may hold crosstabs, started
//! with [`Runs::start_xtab`]. Its one check point is `stats_xtab::run`'s, before
//! the arithmetic, and the record's `columns_total` is
//! `stats_xtab::check_points`, one. Since Phase 8X-2 (decision 591) the
//! Statistics window's Crosstabs… dialog starts it, and a project reruns it.
//!
//! Research OS, Phase 8Y-1 (decision 607): a record may hold the rank tests,
//! the Mann-Whitney and Kruskal-Wallis tests, started with
//! [`Runs::start_ranks`]. Its one check point is `stats_ranks::run`'s, before
//! the arithmetic, and the record's `columns_total` is
//! `stats_ranks::check_points`, one. Since Phase 8Y-2 (decision 622) the
//! Statistics window's Independent samples… dialog starts it, and a project
//! reruns it.
//!
//! Research OS, Phase 8Z-1 (decision 641): a record may hold the related rank
//! tests, the Wilcoxon signed-rank and Friedman tests, started with
//! [`Runs::start_related_ranks`]. Its one check point is
//! `stats_related_ranks::run`'s, before the arithmetic, and the record's
//! `columns_total` is `stats_related_ranks::check_points`, one. Since Phase 8Z-2
//! (decision 656) the Statistics window's Related samples… dialog starts it, and a
//! project reruns it.
//!
//! Research OS, Phase 9A-1 (decision 674): a record may hold the linear
//! regression, started with [`Runs::start_linear_regression`]. Its one check
//! point is `stats_linear_regression::run`'s, before the arithmetic, and the
//! record's `columns_total` is `stats_linear_regression::check_points`, one.
//! Since Phase 9A-2 (decision 687) the Statistics window's Regression, Linear…
//! dialog starts it, and a project reruns it.
//!
//! Research OS, Phase 9B-1 (decision 704): a record may hold the factor
//! analysis, started with [`Runs::start_factor`]. Its check points are
//! `stats_factor::run`'s: before the arithmetic, before each item's row, before
//! each correlation, before the extraction and before the rotation, every
//! extraction and rotation iteration calling its point again at the same count,
//! so a cancel or the ceiling stops a long one; the record's `columns_total` is
//! `stats_factor::check_points`. Since Phase 9B-3 (decision 732) the Statistics
//! window's Dimension reduction, Factor… dialog starts it, and a project reruns it
//! through [`Runs::start_factor_then`].
//!
//! Phase 3: the run manager and the slots belong to the Statistics workspace
//! (`crate::workspace`), built when it is activated and dropped when it is
//! left. [`Runs::seal`] is how it is left: under the records' lock it either
//! names the unfinished run, or closes the manager so no run can start after.
//!
//! **A prepared slot (Research OS, Phase 8M-3, decision 424).** A slot holds the
//! file's table or a recipe's prepared table, never both, under one number.
//! [`DatasetSlots::hold_prepared`] and [`DatasetSlots::hold_original`] replace
//! its table only after the new one exists, moving it to the back and running
//! the eviction, so a make can drop an older slot and never its own. A slot's
//! bytes are shared between its versions by their `Arc`, and a run or a Save
//! holding the old version keeps it whole. [`Slot::preparation`] is the one
//! place a spec's preparation is built. Nothing here writes a file.

use std::collections::{BTreeMap, VecDeque};
use std::ops::ControlFlow;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Instant;

use gaply_core::contracts::capability::Build;
use gaply_core::contracts::codes::{Closed, RunCode};
use gaply_core::contracts::{
    transition, CapabilityId, ContentHash, Diagnostic, OperationId, OperationSpec, Platform,
    Preparation, ResultBundle, RunEvent, RunId, RunManifest, RunRecord, RunState, SchemaVersion,
    ValidationReport, ValidationStatus,
};
use gaply_core::research_project::DataFormat;
use gaply_core::sav_findings::SavFinding;
use gaply_core::stats_adapters;
use gaply_core::stats_alpha::{self, AlphaCode, AlphaOutcome, AlphaParams};
use gaply_core::stats_anova::{self, AnovaCode, AnovaOutcome, AnovaParams};
use gaply_core::stats_corr::{self, CorrCode, CorrOutcome, CorrParams};
use gaply_core::stats_desc::{
    self, DatasetProfile, DescCode, DescribeParams, DescribeResult, Halt,
};
use gaply_core::stats_l1::dataset::{DataFingerprint, Dataset};
use gaply_core::stats_onesample::{
    self, OneSampleCode, OneSampleParams, OneSampleResult, PairedParams, PairedResult,
};
use gaply_core::stats_prep::{self, Prepared};
use gaply_core::stats_ranks::{
    self, IndependentRanksOutcome, IndependentRanksParams, RanksCode,
};
use gaply_core::stats_related_ranks::{
    self, RelatedRanksCode, RelatedRanksOutcome, RelatedRanksParams,
};
use gaply_core::stats_factor::{self, FactorCode, FactorOutcome, FactorParams};
use gaply_core::stats_linear_regression::{
    self, LinearRegressionCode, LinearRegressionOutcome, LinearRegressionParams,
};
use gaply_core::stats_xtab::{self, CrosstabsOutcome, CrosstabsParams, XtabCode};
use gaply_core::stats_ttest::{self, TTestCode, TTestParams, TTestResult};
use serde::Serialize;

/// Run records held at once (decision 6; 40 since decision 468).
pub const MAX_RUNS: usize = 40;
/// Dataset slots held at once (decision 6).
pub const MAX_SLOTS: usize = 4;
/// The cells [`DatasetSlots::default`] and [`DatasetSlots::numbered_by`]
/// hold in all (Research OS, Phase 8F-1, decision 282): [`MAX_SLOTS`]
/// datasets at `IngestLimits::METHODS`'s cell cap, 2,560,000. Since 8F-2 the
/// Statistics engine's slots take this computer's budget instead
/// (`memory_tier::MethodLimits::slot_budget_cells`, through
/// [`DatasetSlots::numbered_with_budget`]; decision 288): 10,000,000 cells on
/// 8 GB, 51,200,000 on 16 and 32 GB.
pub const SLOT_BUDGET_CELLS: u64 =
    MAX_SLOTS as u64 * crate::supplementary::IngestLimits::METHODS.max_cells as u64;
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

/// Called once a run started with it has ended, whatever the end (Research
/// OS, Phase 5C-1a, decision 186): with the run's id and the [`RunCopy`]
/// taken under the records' lock at its terminal step, after that lock is
/// released. Exactly once per record: it is taken out of the manager at the
/// terminal step. A start that makes no record (`Busy`, `Sealed`) never
/// calls it. This module only calls it; what it does is its caller's.
pub type EndHook = Box<dyn FnOnce(RunId, RunCopy) + Send>;

/// The number of a [`Slot`] that no [`DatasetSlots`] holds (Phase 5C-1a,
/// decision 190). `DatasetSlots` numbers from 1, so no open dataset has it.
pub const ONE_RUN_SLOT: u64 = 0;

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

/* ------------------------------------------------------------------------ *
 *  Dataset slots
 * ------------------------------------------------------------------------ */

/// The original file's bytes, as the ingest read them, with their SHA-256
/// taken at ingest and their format (Phase 5B-2, decision 123): what a
/// project Save copies. Never `Debug`, never `Serialize`.
pub struct SourceFile {
    bytes: Vec<u8>,
    sha256: ContentHash,
    format: DataFormat,
}

impl SourceFile {
    pub fn new(bytes: Vec<u8>, sha256: ContentHash, format: DataFormat) -> SourceFile {
        SourceFile {
            bytes,
            sha256,
            format,
        }
    }
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
    pub fn sha256(&self) -> &ContentHash {
        &self.sha256
    }
    pub fn format(&self) -> DataFormat {
        self.format
    }
}

/// What a slot holds: the file's table, or the prepared table in its place
/// (Research OS, Phase 8M-3, decision 424). There is never both, so a prepared
/// slot is one copy; the original comes back from it with no parse.
enum Table {
    File(Dataset),
    Prepared {
        /// The Dataset, the recipe and the counts.
        prepared: Prepared,
        /// The original's fingerprint, which the prepared table's own, over
        /// the recipe, does not give back.
        original: DataFingerprint,
    },
}

/// One opened dataset, its slot number, its file's display name and, when
/// the ingest kept them, the original bytes and what the reader noted about
/// the file. Never `Debug`, never `Serialize`: it holds the user's rows.
///
/// A recipe gives a new `Slot` under the same number, which shares the bytes
/// through their `Arc`: a Save may hold this one (decision 424), so it cannot
/// be taken apart.
///
/// Every slot [`DatasetSlots`] holds keeps the profile of its file's own
/// columns, shared through a make and a clear as the bytes are, so a view
/// profiles only the columns a recipe added (Research OS, Phase 8T-1b,
/// decision 498). It holds headers and level labels, so it is the user's data
/// too. A one-run slot holds none: no view reads it.
pub struct Slot {
    id: u64,
    display_name: String,
    table: Table,
    source: Option<Arc<SourceFile>>,
    file_findings: Vec<SavFinding>,
    file_profile: Option<Arc<DatasetProfile>>,
}

impl Slot {
    /// A dataset held by one run only (Phase 5C-1a, decision 190): a rerun's
    /// copy of a project's data file. No [`DatasetSlots`] holds it, so it is
    /// never among the open datasets, evicts none of them, and no screen can
    /// reach it by a slot number. It has no display name. It is dropped with
    /// the last handle the run's worker and its end hook hold.
    pub fn for_one_run(dataset: Dataset, source: SourceFile) -> Slot {
        Slot {
            id: ONE_RUN_SLOT,
            display_name: String::new(),
            table: Table::File(dataset),
            source: Some(Arc::new(source)),
            file_findings: Vec::new(),
            file_profile: None,
        }
    }

    /// [`Slot::for_one_run`] for a rerun on prepared data (Research OS, Phase
    /// 8M-4, decision 434): the table a stored recipe made, in place of the file's,
    /// and the file's fingerprint, so its [`Slot::preparation`] is the one the live
    /// slot gave. One table, as a live slot holds.
    pub fn for_one_run_prepared(
        prepared: Prepared,
        original: DataFingerprint,
        source: SourceFile,
    ) -> Slot {
        Slot {
            id: ONE_RUN_SLOT,
            display_name: String::new(),
            table: Table::Prepared { prepared, original },
            source: Some(Arc::new(source)),
            file_findings: Vec::new(),
            file_profile: None,
        }
    }

    pub fn id(&self) -> u64 {
        self.id
    }
    pub fn display_name(&self) -> &str {
        &self.display_name
    }
    /// The table every method reads: the prepared one when a recipe is held.
    pub fn dataset(&self) -> &Dataset {
        match &self.table {
            Table::File(dataset) => dataset,
            Table::Prepared { prepared, .. } => &prepared.dataset,
        }
    }
    pub fn source(&self) -> Option<&SourceFile> {
        self.source.as_deref()
    }
    /// What the SPSS reader noted when the file was opened (decision 365).
    pub fn file_findings(&self) -> &[SavFinding] {
        &self.file_findings
    }
    /// The prepared table, its recipe and its counts, when a recipe is held.
    pub fn prepared(&self) -> Option<&Prepared> {
        match &self.table {
            Table::File(_) => None,
            Table::Prepared { prepared, .. } => Some(prepared),
        }
    }
    /// The profile of the file's own columns, as `stats_desc::profile` gave it
    /// when the slot was first held (decision 498). `Some` for every slot
    /// [`DatasetSlots`] holds, `None` for a one-run slot.
    pub fn file_profile(&self) -> Option<&DatasetProfile> {
        self.file_profile.as_deref()
    }
    /// The file's fingerprint, whether or not a recipe is held.
    pub fn original_fingerprint(&self) -> &DataFingerprint {
        match &self.table {
            Table::File(dataset) => &dataset.schema.fingerprint,
            Table::Prepared { original, .. } => original,
        }
    }
    /// **What every method's spec on this slot copies (decision 424, 8M-2's
    /// question 4): the one place a [`Preparation`] is built.** `None` on a
    /// file's own table. The source's hash is the original's fingerprint, never
    /// the `SourceFile`'s, which a slot may lack.
    pub fn preparation(&self) -> Option<Preparation> {
        let Table::Prepared { prepared, original } = &self.table else {
            return None;
        };
        Some(Preparation {
            source_hash: ContentHash::from_hex(original.sha256_hex())
                .expect("a fingerprint is SHA-256 hex"),
            recipe_hash: ContentHash::from_hex(&stats_prep::recipe_hash(&prepared.recipe))
                .expect("a recipe's hash is SHA-256 hex"),
            source_columns: prepared.recipe.source_columns,
            capability_version: stats_prep::CAPABILITY_VERSION,
            engine_version: stats_prep::ENGINE_VERSION,
        })
    }
}

pub struct DatasetSlots {
    /// The last slot number handed out. Shared by every engine one workspace
    /// session builds ([`DatasetSlots::numbered_by`]), so an open that lands
    /// in an engine already dropped can never take a number the next engine
    /// hands out (evaluation of Phases 2 and 3, F8).
    next: Arc<AtomicU64>,
    slots: Mutex<VecDeque<(u64, Arc<Slot>)>>,
    /// The cells the slots may hold in all, the newest dataset included
    /// (decision 282). Cells only: not the file bytes a slot keeps, nor the
    /// profile of its file's columns (decision 498), nor a rerun's one-run
    /// slot.
    budget_cells: u64,
}

/// Written by hand for the production budget (decision 282).
impl Default for DatasetSlots {
    fn default() -> DatasetSlots {
        DatasetSlots::numbered_by(Arc::default())
    }
}

impl DatasetSlots {
    /// Slots numbered from `counter`, which other slot sets may share, with
    /// the production budget, [`SLOT_BUDGET_CELLS`].
    pub fn numbered_by(counter: Arc<AtomicU64>) -> DatasetSlots {
        DatasetSlots::numbered_with_budget(counter, SLOT_BUDGET_CELLS)
    }

    /// Slots numbered from `counter`, with a budget of `budget_cells`: the
    /// Statistics engine's, at this computer's slot budget
    /// (`memory_tier::MethodLimits::slot_budget_cells`; Research OS, Phase
    /// 8F-2, decision 288).
    pub fn numbered_with_budget(counter: Arc<AtomicU64>, budget_cells: u64) -> DatasetSlots {
        DatasetSlots {
            next: counter,
            slots: Mutex::new(VecDeque::new()),
            budget_cells,
        }
    }

    /// Slots with a budget of `budget_cells`, for the tests and the release
    /// probe (decision 282).
    pub fn with_cell_budget(budget_cells: u64) -> DatasetSlots {
        DatasetSlots {
            budget_cells,
            ..DatasetSlots::default()
        }
    }

    /// The cells the slots may hold in all.
    pub fn cell_budget(&self) -> u64 {
        self.budget_cells
    }

    /// The cells the slots hold now.
    pub fn cells_held(&self) -> u64 {
        lock(&self.slots)
            .iter()
            .map(|(_, s)| cells_of(s.dataset()))
            .sum()
    }

    /// Holds the dataset and returns its slot. A fifth drops the oldest, and
    /// so does an open that takes the cells held past the budget; the
    /// dataset just opened is never dropped. A run already using a dropped
    /// slot keeps its own handle until it ends, so its memory is freed then.
    pub fn insert(&self, display_name: String, dataset: Dataset) -> u64 {
        self.insert_with_source(display_name, dataset, None)
    }

    /// [`DatasetSlots::insert`], with the original file's bytes held beside
    /// the dataset for as long as the slot is (Phase 5B-2).
    pub fn insert_with_source(
        &self,
        display_name: String,
        dataset: Dataset,
        source: Option<SourceFile>,
    ) -> u64 {
        self.insert_opened(display_name, dataset, source, Vec::new())
    }

    /// [`DatasetSlots::insert_with_source`], with what the reader noted about
    /// the file, which a recipe's view shows as the open's does (decision 424).
    /// The table is profiled here, before the lock is taken, so the slot holds
    /// its file profile (decision 498).
    pub fn insert_opened(
        &self,
        display_name: String,
        dataset: Dataset,
        source: Option<SourceFile>,
        file_findings: Vec<SavFinding>,
    ) -> u64 {
        let profile = stats_desc::profile(&dataset);
        self.insert_profiled(display_name, dataset, profile, source, file_findings)
    }

    /// [`DatasetSlots::insert_opened`] with the file's profile already made:
    /// the open profiles the table for its view, so the slot keeps that one
    /// rather than profiling it twice (decision 498). `profile` must be
    /// `stats_desc::profile(&dataset)`.
    pub fn insert_profiled(
        &self,
        display_name: String,
        dataset: Dataset,
        profile: DatasetProfile,
        source: Option<SourceFile>,
        file_findings: Vec<SavFinding>,
    ) -> u64 {
        debug_assert_eq!(
            profile.columns.len(),
            dataset.schema.columns.len(),
            "the profile is of this table"
        );
        let id = self.next.fetch_add(1, Ordering::SeqCst) + 1;
        let mut slots = lock(&self.slots);
        slots.push_back((
            id,
            Arc::new(Slot {
                id,
                display_name,
                table: Table::File(dataset),
                source: source.map(Arc::new),
                file_findings,
                file_profile: Some(Arc::new(profile)),
            }),
        ));
        self.evict(&mut slots);
        id
    }

    /// The count first, then the budget in cells (decision 282); the slot at
    /// the back, the one just opened or just changed, is never dropped.
    fn evict(&self, slots: &mut VecDeque<(u64, Arc<Slot>)>) {
        while slots.len() > MAX_SLOTS {
            slots.pop_front();
        }
        let mut held: u64 = slots.iter().map(|(_, s)| cells_of(s.dataset())).sum();
        while slots.len() > 1 && held > self.budget_cells {
            if let Some((_, oldest)) = slots.pop_front() {
                held -= cells_of(oldest.dataset());
            }
        }
    }

    /// **`held`'s slot, its table replaced (decision 424): the same number,
    /// moved to the back so it is never the one dropped, sharing the bytes and
    /// the file's profile (decision 498) and keeping the file's findings.** Then the eviction runs, since a table
    /// with one more column may take the cells held past the budget.
    ///
    /// Only the slot `held` is may be replaced, so a make cannot overwrite what
    /// another make just held: another slot under that number is `RunBusy`,
    /// and no slot `UnknownInput`, each leaving everything as it was. A run
    /// holding `held` keeps it until it ends.
    fn replace(&self, held: &Arc<Slot>, table: Table) -> Result<Arc<Slot>, RunCode> {
        let mut slots = lock(&self.slots);
        let at = slots
            .iter()
            .position(|(s, _)| *s == held.id)
            .ok_or(RunCode::UnknownInput)?;
        if !Arc::ptr_eq(&slots[at].1, held) {
            return Err(RunCode::RunBusy);
        }
        let next = Arc::new(Slot {
            id: held.id,
            display_name: held.display_name.clone(),
            table,
            source: held.source.clone(),
            file_findings: held.file_findings.clone(),
            file_profile: held.file_profile.clone(),
        });
        slots.remove(at);
        slots.push_back((held.id, next.clone()));
        self.evict(&mut slots);
        Ok(next)
    }

    /// `held`'s slot now holds `prepared` in place of its table (decision 424).
    pub fn hold_prepared(
        &self,
        held: &Arc<Slot>,
        prepared: Prepared,
    ) -> Result<Arc<Slot>, RunCode> {
        let original = held.original_fingerprint().clone();
        self.replace(held, Table::Prepared { prepared, original })
    }

    /// `held`'s slot back on the file's table, rebuilt from it (decision 424).
    pub fn hold_original(&self, held: &Arc<Slot>, original: Dataset) -> Result<Arc<Slot>, RunCode> {
        self.replace(held, Table::File(original))
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

    /// The slots' numbers, oldest first: the order the eviction drops them in.
    pub fn order(&self) -> Vec<u64> {
        lock(&self.slots).iter().map(|(id, _)| *id).collect()
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

/// A dataset's cells: its rows times its columns (decision 282).
fn cells_of(d: &Dataset) -> u64 {
    (d.rows() as u64).saturating_mul(d.schema.columns.len() as u64)
}

/* ------------------------------------------------------------------------ *
 *  The open's progress
 * ------------------------------------------------------------------------ */

/// **The methods' opens in flight, and each one's rows read (Research OS,
/// Phase 8F-1, decision 280).** `stats_desc_open` takes an optional
/// `progressId`, a number the screen chooses; while that open runs, the
/// parse stores its rows read into the counter [`OpenProgress::begin`]
/// gave it, and `stats_desc_open_progress` reads it. A count only, never a
/// value or a name. An open's entry is removed when the open returns, by the
/// [`ProgressEntry`] it holds being dropped. A second open with an id still
/// in flight takes its entry; each removes the entry on return only if it is
/// still its own.
#[derive(Default)]
pub struct OpenProgress {
    opens: Mutex<BTreeMap<u64, Arc<AtomicU64>>>,
}

impl OpenProgress {
    /// An entry for `id`, replacing any open's entry of the same id.
    pub fn begin(self: &Arc<Self>, id: u64) -> ProgressEntry {
        let rows = Arc::new(AtomicU64::new(0));
        lock(&self.opens).insert(id, Arc::clone(&rows));
        ProgressEntry {
            table: Arc::clone(self),
            id,
            rows,
        }
    }

    /// The rows read by the open in flight with `id`; `None` when no open
    /// has that id.
    pub fn rows_read(&self, id: u64) -> Option<u64> {
        lock(&self.opens)
            .get(&id)
            .map(|r| r.load(Ordering::Relaxed))
    }

    /// The opens in flight.
    pub fn in_flight(&self) -> usize {
        lock(&self.opens).len()
    }
}

/// One open's entry in [`OpenProgress`]: its counter, and its removal when
/// it is dropped, whether the open succeeded, was refused or panicked.
pub struct ProgressEntry {
    table: Arc<OpenProgress>,
    id: u64,
    rows: Arc<AtomicU64>,
}

impl ProgressEntry {
    /// The counter the parse stores its rows read into.
    pub fn counter(&self) -> &AtomicU64 {
        &self.rows
    }
}

impl Drop for ProgressEntry {
    fn drop(&mut self) {
        let mut opens = lock(&self.table.opens);
        if opens
            .get(&self.id)
            .is_some_and(|r| Arc::ptr_eq(r, &self.rows))
        {
            opens.remove(&self.id);
        }
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
    /// `stats.analyze.one_sample_t`, through the `stats_onesample_*`
    /// commands (Research OS, Phase 8D-2a, decision 263).
    OneSample,
    /// `stats.analyze.paired_t`, through the `stats_paired_*` commands.
    Paired,
    /// `stats.analyze.one_way_anova`, through the `stats_anova_*` commands
    /// (Research OS, Phase 8G-2, decision 318).
    Anova,
    /// `stats.analyze.correlations`, through the `stats_corr_*` commands
    /// (Research OS, Phase 8H-2, decision 355).
    Correlations,
    /// `stats.analyze.cronbach_alpha`, through the `stats_alpha_*` commands
    /// (Research OS, Phase 8I-2, decision 380).
    CronbachAlpha,
    /// `stats.analyze.crosstabs`, through the `stats_xtab_*` commands
    /// (Research OS, Phase 8X-1, decision 580).
    Crosstabs,
    /// `stats.analyze.independent_ranks`, through the `stats_ranks_*` commands
    /// (Research OS, Phase 8Y-1, decision 607).
    IndependentRanks,
    /// `stats.analyze.related_ranks`, through the `stats_related_ranks_*`
    /// commands (Research OS, Phase 8Z-1, decision 641).
    RelatedRanks,
    /// `stats.analyze.linear_regression`, through the
    /// `stats_linear_regression_*` commands (Research OS, Phase 9A-1, decision
    /// 674).
    LinearRegression,
    /// `stats.analyze.factor`, through the `stats_factor_*` commands (Research
    /// OS, Phase 9B-1, decision 704).
    Factor,
}

/// The data a run read: its slot, and the SHA-256 of the original bytes the
/// slot held when the run started (Phase 5B-2, decision 123); since Phase
/// 8T-1a (decision 491) also the recipe the slot held, so a Save writes the
/// recipe the run ran on, whatever the slot holds by then.
///
/// The recipe holds the score's typed name, so this is never `Debug` or
/// `Serialize` (pinned by `tests/run_record_guards.rs`). It lives as long as its
/// record: a forget, an eviction or leaving Statistics drops it.
#[derive(Clone)]
pub struct RunInput {
    pub slot: u64,
    pub sha256: ContentHash,
    /// The recipe's canonical bytes, through `stats_adapters::prep_recipe_bytes`,
    /// when the slot held one; `None` on a file's own table.
    pub recipe: Option<Vec<u8>>,
}

/// A record's spec, as canonical JSON, and the capability it names.
struct HeldSpec {
    json: Vec<u8>,
    capability_id: CapabilityId,
    capability_version: u32,
}

/// What one kind of run keeps: its validation, its result and the reason it
/// failed, in that kind's own codes; since Phase 5B-2 also its spec and the
/// data it read.
struct Outputs<R, C> {
    validation: Option<ValidationReport<C>>,
    bundle: Option<ResultBundle<R, C>>,
    failure: Option<Diagnostic<C>>,
    spec: Option<HeldSpec>,
    input: Option<RunInput>,
}

impl<R, C> Default for Outputs<R, C> {
    fn default() -> Self {
        Outputs {
            validation: None,
            bundle: None,
            failure: None,
            spec: None,
            input: None,
        }
    }
}

impl<R: Serialize, C: Serialize> Outputs<R, C> {
    /// The copy a Save takes, in this kind's codes made JSON.
    fn copy(&self) -> SavedParts {
        let json = "a record's parts are ids, words, numbers and the user's labels";
        SavedParts {
            spec: self.spec.as_ref().map(|s| {
                (
                    s.json.clone(),
                    s.capability_id.clone(),
                    s.capability_version,
                )
            }),
            input: self.input.clone(),
            bundle_json: self
                .bundle
                .as_ref()
                .map(|b| serde_json::to_vec(b).expect(json)),
            manifest_json: self
                .bundle
                .as_ref()
                .map(|b| serde_json::to_vec(&b.manifest).expect(json)),
            manifest_inputs: self
                .bundle
                .as_ref()
                .map(|b| b.manifest.inputs.clone())
                .unwrap_or_default(),
            failure: self
                .failure
                .as_ref()
                .map(|d| serde_json::to_value(d).expect(json)),
        }
    }
}

/// What [`Outputs::copy`] takes, before the record's own fields join it.
struct SavedParts {
    spec: Option<(Vec<u8>, CapabilityId, u32)>,
    input: Option<RunInput>,
    bundle_json: Option<Vec<u8>>,
    manifest_json: Option<Vec<u8>>,
    manifest_inputs: Vec<ContentHash>,
    failure: Option<serde_json::Value>,
}

/// A finished run as a project Save needs it (Phase 5B-2): copies, never
/// handles. The bundle's bytes hold the user's headers and level labels, so
/// this is never `Debug` or `Serialize`; it goes only into the store.
pub struct RunCopy {
    pub kind: RunKind,
    pub state: RunState,
    pub started_ms: u64,
    pub ended_ms: Option<u64>,
    /// The manager's clock when the copy was taken, so the run's times can be
    /// placed on the wall clock.
    pub now_ms: u64,
    /// The spec as canonical JSON, and the capability it names; `None` only
    /// before the record was queued.
    pub spec_json: Option<Vec<u8>>,
    pub capability: Option<(CapabilityId, u32)>,
    pub input: Option<RunInput>,
    pub bundle_json: Option<Vec<u8>>,
    pub manifest_json: Option<Vec<u8>>,
    /// The manifest's input hashes, in the spec's order.
    pub manifest_inputs: Vec<ContentHash>,
    /// A failed run's diagnostic: its closed code and parameters.
    pub failure: Option<serde_json::Value>,
}

/// A record's outputs, by its kind: the closed enum of result kinds
/// (decision 75).
enum Held {
    Describe(Outputs<DescribeResult, DescCode>),
    Ttest(Outputs<TTestResult, TTestCode>),
    OneSample(Outputs<OneSampleResult, OneSampleCode>),
    Paired(Outputs<PairedResult, OneSampleCode>),
    Anova(Outputs<AnovaOutcome, AnovaCode>),
    Correlations(Outputs<CorrOutcome, CorrCode>),
    CronbachAlpha(Outputs<AlphaOutcome, AlphaCode>),
    Crosstabs(Outputs<CrosstabsOutcome, XtabCode>),
    IndependentRanks(Outputs<IndependentRanksOutcome, RanksCode>),
    RelatedRanks(Outputs<RelatedRanksOutcome, RelatedRanksCode>),
    LinearRegression(Outputs<LinearRegressionOutcome, LinearRegressionCode>),
    Factor(Outputs<FactorOutcome, FactorCode>),
}

impl Held {
    fn kind(&self) -> RunKind {
        by_method!(self, Held, |M, _| M::KIND)
    }
}

struct Entry {
    record: RunRecord,
    /// Every state the record has been in, in order.
    path: Vec<RunState>,
    /// Describe's columns, or the ANOVA's, the correlations' or alpha's check
    /// points; a t-test has none, and its view does not carry them.
    columns_total: usize,
    columns_done: usize,
    cancel_requested: bool,
    /// Whether it was started with an end hook: a rerun. [`Runs::evict`] drops
    /// its finished record before any other finished one (decision 468).
    rerun: bool,
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
    OneSample(OperationSpec<OneSampleParams>),
    Paired(OperationSpec<PairedParams>),
    Anova(OperationSpec<AnovaParams>),
    Correlations(OperationSpec<CorrParams>),
    CronbachAlpha(OperationSpec<AlphaParams>),
    Crosstabs(OperationSpec<CrosstabsParams>),
    IndependentRanks(OperationSpec<IndependentRanksParams>),
    RelatedRanks(OperationSpec<RelatedRanksParams>),
    LinearRegression(OperationSpec<LinearRegressionParams>),
    Factor(OperationSpec<FactorParams>),
}

impl Job {
    /// The spec as canonical JSON, and the capability it names.
    fn held(&self) -> HeldSpec {
        let (json, id, version) = by_method!(self, Job, |_M, s| (
            serde_json::to_vec(s),
            s.capability_id.clone(),
            s.capability_version,
        ));
        HeldSpec {
            json: json.expect("a spec of ids, words and numbers serialises"),
            capability_id: id,
            capability_version: version,
        }
    }
}

/// A validation's report, in its kind's codes.
enum Report {
    Describe(ValidationReport<DescCode>),
    Ttest(ValidationReport<TTestCode>),
    OneSample(ValidationReport<OneSampleCode>),
    Paired(ValidationReport<OneSampleCode>),
    Anova(ValidationReport<AnovaCode>),
    Correlations(ValidationReport<CorrCode>),
    CronbachAlpha(ValidationReport<AlphaCode>),
    Crosstabs(ValidationReport<XtabCode>),
    IndependentRanks(ValidationReport<RanksCode>),
    RelatedRanks(ValidationReport<RelatedRanksCode>),
    LinearRegression(ValidationReport<LinearRegressionCode>),
    Factor(ValidationReport<FactorCode>),
}

impl Report {
    fn status(&self) -> ValidationStatus {
        by_method!(self, Report, |_M, r| r.status)
    }
}

/// What the work produced, with the spec it ran: the operation's own answer,
/// or a panic.
enum Done {
    Describe(Finished<Summary>),
    Ttest(Finished<IndependentT>),
    OneSample(Finished<OneSampleT>),
    Paired(Finished<PairedT>),
    Anova(Finished<OneWayAnova>),
    Correlations(Finished<Correlations>),
    CronbachAlpha(Finished<CronbachAlpha>),
    Crosstabs(Finished<Crosstabs>),
    IndependentRanks(Finished<IndependentRanks>),
    RelatedRanks(Finished<RelatedRanks>),
    LinearRegression(Finished<LinearRegression>),
    Factor(Finished<Factor>),
}

/* ------------------------------------------------------------------------ *
 *  The seam: one implementation per method (Research OS, Phase 8C)
 * ------------------------------------------------------------------------ */

/// THE one place a method's variants meet (decision 249). Every match on
/// [`Held`], [`Job`], [`Report`] or [`Done`] in this file is this one: each
/// arm binds the variant's state as `$x` and names that method's
/// implementation `$m` for `$body`. A new method adds one variant to each of
/// the four enums and one line here. It only dispatches: what a method does
/// is written out in its own `impl Method`.
macro_rules! by_method {
    ($value:expr, $enum:ident, |$m:ident, $x:pat_param| $body:expr) => {
        match $value {
            $enum::Describe($x) => {
                type $m = Summary;
                $body
            }
            $enum::Ttest($x) => {
                type $m = IndependentT;
                $body
            }
            $enum::OneSample($x) => {
                type $m = OneSampleT;
                $body
            }
            $enum::Paired($x) => {
                type $m = PairedT;
                $body
            }
            $enum::Anova($x) => {
                type $m = OneWayAnova;
                $body
            }
            $enum::Correlations($x) => {
                type $m = Correlations;
                $body
            }
            $enum::CronbachAlpha($x) => {
                type $m = CronbachAlpha;
                $body
            }
            $enum::Crosstabs($x) => {
                type $m = Crosstabs;
                $body
            }
            $enum::IndependentRanks($x) => {
                type $m = IndependentRanks;
                $body
            }
            $enum::RelatedRanks($x) => {
                type $m = RelatedRanks;
                $body
            }
            $enum::LinearRegression($x) => {
                type $m = LinearRegression;
                $body
            }
            $enum::Factor($x) => {
                type $m = Factor;
                $body
            }
        }
    };
}
use by_method;

/// What a method's work answered: its result, its halt, or a panic.
type Outcome<M> = std::thread::Result<Result<<M as Method>::Result, <M as Method>::Halt>>;

/// The work's answer with the spec it ran, of one method.
struct Finished<M: Method> {
    outcome: Outcome<M>,
    spec: OperationSpec<M::Params>,
}

/// A method's halt, as the finish reads it.
enum Halted<C> {
    Stopped(Stop),
    Refused(Diagnostic<C>),
}

/// What the run manager asks of one method (decision 249): what to check
/// before a run, what to compute through its check points, its manifest,
/// what its bundle carries, and what the work's end leaves in the record's
/// column count, a cancel's win included. The queue, the worker, the finish,
/// the snapshot and the result are written once, over this. Each
/// implementation calls its method's own façades, as the run manager did
/// before 8C.
trait Method: Sized {
    /// The record's closed word.
    const KIND: RunKind;
    type Params: Serialize;
    type Result: Serialize + Clone;
    type Code: Serialize + Clone;
    /// What its work answers when it gives no result.
    type Halt;

    /// What the record's `columns_total` is, for `spec` on the slot's
    /// `dataset` (decision 328).
    fn columns_total(spec: &OperationSpec<Self::Params>, dataset: &Dataset) -> usize;
    /// The check before a run, through the method's façade.
    fn validate(
        spec: &OperationSpec<Self::Params>,
        dataset: &Dataset,
    ) -> ValidationReport<Self::Code>;
    /// The work, through the method's façade, calling `checkpoint` at each of
    /// its check points.
    fn compute(
        spec: &OperationSpec<Self::Params>,
        build: Build,
        dataset: &Dataset,
        checkpoint: impl FnMut(usize) -> ControlFlow<Stop>,
    ) -> Result<Self::Result, Self::Halt>;
    /// A result's manifest.
    fn manifest(
        spec: &OperationSpec<Self::Params>,
        dataset: &Dataset,
        result: &Self::Result,
        elapsed: u64,
    ) -> RunManifest<Self::Code>;
    /// The limitations a result's bundle carries.
    fn limitations(result: &Self::Result) -> Vec<Diagnostic<Self::Code>>;
    /// The record's columns done once the work has ended, `total` being its
    /// `columns_total`, whether a cancel won or not; `None` leaves them as the
    /// check points set them.
    fn columns_done(total: usize, outcome: &Outcome<Self>) -> Option<usize>;
    /// Its halt, as the finish reads it.
    fn halted(halt: Self::Halt) -> Halted<Self::Code>;
    /// Says, in its own words, that its work panicked with no cancel.
    fn panicked();
    /// Its outputs in a record, when the record is of its kind.
    fn outputs(held: &Held) -> Option<&Outputs<Self::Result, Self::Code>>;
    fn outputs_mut(held: &mut Held) -> Option<&mut Outputs<Self::Result, Self::Code>>;
    /// Its variant of each closed enum.
    fn held(outputs: Outputs<Self::Result, Self::Code>) -> Held;
    fn job(spec: OperationSpec<Self::Params>) -> Job;
    fn report(report: ValidationReport<Self::Code>) -> Report;
    fn done(finished: Finished<Self>) -> Done;
}

/// `stats.describe`, through the `stats_desc_*` commands.
struct Summary;

impl Method for Summary {
    const KIND: RunKind = RunKind::Describe;
    type Params = DescribeParams;
    type Result = DescribeResult;
    type Code = DescCode;
    type Halt = Halt<Stop>;

    fn columns_total(spec: &OperationSpec<DescribeParams>, _: &Dataset) -> usize {
        spec.parameters.columns.len()
    }

    fn validate(
        spec: &OperationSpec<DescribeParams>,
        dataset: &Dataset,
    ) -> ValidationReport<DescCode> {
        // Through `stats.describe`'s façade: the capability registry first
        // (Research OS, Phase 4A), then `stats_desc::validate`, unchanged.
        stats_adapters::describe_validate(spec, crate::workspace::build(), dataset)
    }

    fn compute(
        spec: &OperationSpec<DescribeParams>,
        build: Build,
        dataset: &Dataset,
        checkpoint: impl FnMut(usize) -> ControlFlow<Stop>,
    ) -> Result<DescribeResult, Halt<Stop>> {
        // A check point before each column.
        stats_adapters::describe_until(spec, build, dataset, checkpoint)
    }

    fn manifest(
        spec: &OperationSpec<DescribeParams>,
        dataset: &Dataset,
        result: &DescribeResult,
        elapsed: u64,
    ) -> RunManifest<DescCode> {
        stats_desc::manifest(spec, dataset, result, elapsed, Platform::current())
    }

    fn limitations(_: &DescribeResult) -> Vec<Diagnostic<DescCode>> {
        Vec::new()
    }

    /// Every column on a result; the columns done when it stopped.
    fn columns_done(total: usize, outcome: &Outcome<Self>) -> Option<usize> {
        match outcome {
            Ok(Ok(_)) => Some(total),
            Ok(Err(Halt::Stopped { done, .. })) => Some(*done),
            Ok(Err(Halt::Refused(_))) | Err(_) => None,
        }
    }

    fn halted(halt: Halt<Stop>) -> Halted<DescCode> {
        match halt {
            Halt::Stopped { reason, .. } => Halted::Stopped(reason),
            Halt::Refused(d) => Halted::Refused(d),
        }
    }

    fn panicked() {
        tracing::warn!("stats_desc: the describe loop panicked");
    }

    fn outputs(held: &Held) -> Option<&Outputs<DescribeResult, DescCode>> {
        if let Held::Describe(o) = held {
            Some(o)
        } else {
            None
        }
    }

    fn outputs_mut(held: &mut Held) -> Option<&mut Outputs<DescribeResult, DescCode>> {
        if let Held::Describe(o) = held {
            Some(o)
        } else {
            None
        }
    }

    fn held(outputs: Outputs<DescribeResult, DescCode>) -> Held {
        Held::Describe(outputs)
    }

    fn job(spec: OperationSpec<DescribeParams>) -> Job {
        Job::Describe(spec)
    }

    fn report(report: ValidationReport<DescCode>) -> Report {
        Report::Describe(report)
    }

    fn done(finished: Finished<Self>) -> Done {
        Done::Describe(finished)
    }
}

/// `stats.analyze.independent_t`, through the `stats_ttest_*` commands.
struct IndependentT;

impl Method for IndependentT {
    const KIND: RunKind = RunKind::Ttest;
    type Params = TTestParams;
    type Result = TTestResult;
    type Code = TTestCode;
    type Halt = stats_ttest::Halt<Stop>;

    /// None: a t-test's state view carries no columns.
    fn columns_total(_: &OperationSpec<TTestParams>, _: &Dataset) -> usize {
        0
    }

    fn validate(
        spec: &OperationSpec<TTestParams>,
        dataset: &Dataset,
    ) -> ValidationReport<TTestCode> {
        // Through `stats.analyze.independent_t`'s façade: the registry first,
        // then `stats_ttest::validate`, unchanged.
        stats_adapters::ttest_validate(spec, crate::workspace::build(), dataset)
    }

    fn compute(
        spec: &OperationSpec<TTestParams>,
        build: Build,
        dataset: &Dataset,
        checkpoint: impl FnMut(usize) -> ControlFlow<Stop>,
    ) -> Result<TTestResult, stats_ttest::Halt<Stop>> {
        // The t-test's one check point, before its arithmetic.
        stats_adapters::ttest_run(spec, build, dataset, checkpoint)
    }

    fn manifest(
        spec: &OperationSpec<TTestParams>,
        dataset: &Dataset,
        result: &TTestResult,
        elapsed: u64,
    ) -> RunManifest<TTestCode> {
        stats_ttest::manifest(spec, dataset, result, elapsed, Platform::current())
    }

    /// The result's own limitations, as notes.
    fn limitations(result: &TTestResult) -> Vec<Diagnostic<TTestCode>> {
        result
            .limitations
            .iter()
            .map(|c| Diagnostic::note(*c))
            .collect()
    }

    /// Never: a t-test counts no columns, and its stop has none.
    fn columns_done(_: usize, _: &Outcome<Self>) -> Option<usize> {
        None
    }

    fn halted(halt: stats_ttest::Halt<Stop>) -> Halted<TTestCode> {
        match halt {
            stats_ttest::Halt::Stopped { reason } => Halted::Stopped(reason),
            stats_ttest::Halt::Refused(d) => Halted::Refused(d),
        }
    }

    fn panicked() {
        tracing::warn!("stats_desc: the t-test panicked");
    }

    fn outputs(held: &Held) -> Option<&Outputs<TTestResult, TTestCode>> {
        if let Held::Ttest(o) = held {
            Some(o)
        } else {
            None
        }
    }

    fn outputs_mut(held: &mut Held) -> Option<&mut Outputs<TTestResult, TTestCode>> {
        if let Held::Ttest(o) = held {
            Some(o)
        } else {
            None
        }
    }

    fn held(outputs: Outputs<TTestResult, TTestCode>) -> Held {
        Held::Ttest(outputs)
    }

    fn job(spec: OperationSpec<TTestParams>) -> Job {
        Job::Ttest(spec)
    }

    fn report(report: ValidationReport<TTestCode>) -> Report {
        Report::Ttest(report)
    }

    fn done(finished: Finished<Self>) -> Done {
        Done::Ttest(finished)
    }
}

/// `stats.analyze.one_sample_t`, through the `stats_onesample_*` commands
/// (Research OS, Phase 8D-2a, decision 263).
struct OneSampleT;

impl Method for OneSampleT {
    const KIND: RunKind = RunKind::OneSample;
    type Params = OneSampleParams;
    type Result = OneSampleResult;
    type Code = OneSampleCode;
    type Halt = stats_onesample::Halt<Stop>;

    /// None: its state view carries no columns.
    fn columns_total(_: &OperationSpec<OneSampleParams>, _: &Dataset) -> usize {
        0
    }

    fn validate(
        spec: &OperationSpec<OneSampleParams>,
        dataset: &Dataset,
    ) -> ValidationReport<OneSampleCode> {
        // Through `stats.analyze.one_sample_t`'s façade: the registry first,
        // then `stats_onesample::one_sample_validate`, unchanged.
        stats_adapters::one_sample_validate(spec, crate::workspace::build(), dataset)
    }

    fn compute(
        spec: &OperationSpec<OneSampleParams>,
        build: Build,
        dataset: &Dataset,
        checkpoint: impl FnMut(usize) -> ControlFlow<Stop>,
    ) -> Result<OneSampleResult, stats_onesample::Halt<Stop>> {
        // Its one check point, before its arithmetic.
        stats_adapters::one_sample_run(spec, build, dataset, checkpoint)
    }

    fn manifest(
        spec: &OperationSpec<OneSampleParams>,
        dataset: &Dataset,
        result: &OneSampleResult,
        elapsed: u64,
    ) -> RunManifest<OneSampleCode> {
        stats_onesample::one_sample_manifest(spec, dataset, result, elapsed, Platform::current())
    }

    /// The result's own limitations, as notes.
    fn limitations(result: &OneSampleResult) -> Vec<Diagnostic<OneSampleCode>> {
        result
            .limitations
            .iter()
            .map(|c| Diagnostic::note(*c))
            .collect()
    }

    /// Never: it counts no columns, and its stop has none.
    fn columns_done(_: usize, _: &Outcome<Self>) -> Option<usize> {
        None
    }

    fn halted(halt: stats_onesample::Halt<Stop>) -> Halted<OneSampleCode> {
        match halt {
            stats_onesample::Halt::Stopped { reason } => Halted::Stopped(reason),
            stats_onesample::Halt::Refused(d) => Halted::Refused(d),
        }
    }

    fn panicked() {
        tracing::warn!("stats_desc: the one-sample t-test panicked");
    }

    fn outputs(held: &Held) -> Option<&Outputs<OneSampleResult, OneSampleCode>> {
        if let Held::OneSample(o) = held {
            Some(o)
        } else {
            None
        }
    }

    fn outputs_mut(held: &mut Held) -> Option<&mut Outputs<OneSampleResult, OneSampleCode>> {
        if let Held::OneSample(o) = held {
            Some(o)
        } else {
            None
        }
    }

    fn held(outputs: Outputs<OneSampleResult, OneSampleCode>) -> Held {
        Held::OneSample(outputs)
    }

    fn job(spec: OperationSpec<OneSampleParams>) -> Job {
        Job::OneSample(spec)
    }

    fn report(report: ValidationReport<OneSampleCode>) -> Report {
        Report::OneSample(report)
    }

    fn done(finished: Finished<Self>) -> Done {
        Done::OneSample(finished)
    }
}

/// `stats.analyze.paired_t`, through the `stats_paired_*` commands (Research
/// OS, Phase 8D-2a, decision 263).
struct PairedT;

impl Method for PairedT {
    const KIND: RunKind = RunKind::Paired;
    type Params = PairedParams;
    type Result = PairedResult;
    type Code = OneSampleCode;
    type Halt = stats_onesample::Halt<Stop>;

    /// None: its state view carries no columns.
    fn columns_total(_: &OperationSpec<PairedParams>, _: &Dataset) -> usize {
        0
    }

    fn validate(
        spec: &OperationSpec<PairedParams>,
        dataset: &Dataset,
    ) -> ValidationReport<OneSampleCode> {
        // Through `stats.analyze.paired_t`'s façade: the registry first, then
        // `stats_onesample::paired_validate`, unchanged.
        stats_adapters::paired_validate(spec, crate::workspace::build(), dataset)
    }

    fn compute(
        spec: &OperationSpec<PairedParams>,
        build: Build,
        dataset: &Dataset,
        checkpoint: impl FnMut(usize) -> ControlFlow<Stop>,
    ) -> Result<PairedResult, stats_onesample::Halt<Stop>> {
        // Its one check point, before its arithmetic.
        stats_adapters::paired_run(spec, build, dataset, checkpoint)
    }

    fn manifest(
        spec: &OperationSpec<PairedParams>,
        dataset: &Dataset,
        result: &PairedResult,
        elapsed: u64,
    ) -> RunManifest<OneSampleCode> {
        stats_onesample::paired_manifest(spec, dataset, result, elapsed, Platform::current())
    }

    /// The result's own limitations, as notes.
    fn limitations(result: &PairedResult) -> Vec<Diagnostic<OneSampleCode>> {
        result
            .limitations
            .iter()
            .map(|c| Diagnostic::note(*c))
            .collect()
    }

    /// Never: it counts no columns, and its stop has none.
    fn columns_done(_: usize, _: &Outcome<Self>) -> Option<usize> {
        None
    }

    fn halted(halt: stats_onesample::Halt<Stop>) -> Halted<OneSampleCode> {
        match halt {
            stats_onesample::Halt::Stopped { reason } => Halted::Stopped(reason),
            stats_onesample::Halt::Refused(d) => Halted::Refused(d),
        }
    }

    fn panicked() {
        tracing::warn!("stats_desc: the paired t-test panicked");
    }

    fn outputs(held: &Held) -> Option<&Outputs<PairedResult, OneSampleCode>> {
        if let Held::Paired(o) = held {
            Some(o)
        } else {
            None
        }
    }

    fn outputs_mut(held: &mut Held) -> Option<&mut Outputs<PairedResult, OneSampleCode>> {
        if let Held::Paired(o) = held {
            Some(o)
        } else {
            None
        }
    }

    fn held(outputs: Outputs<PairedResult, OneSampleCode>) -> Held {
        Held::Paired(outputs)
    }

    fn job(spec: OperationSpec<PairedParams>) -> Job {
        Job::Paired(spec)
    }

    fn report(report: ValidationReport<OneSampleCode>) -> Report {
        Report::Paired(report)
    }

    fn done(finished: Finished<Self>) -> Done {
        Done::Paired(finished)
    }
}

/// `stats.analyze.one_way_anova`, through the `stats_anova_*` commands
/// (Research OS, Phase 8G-2, decisions 317 and 328).
struct OneWayAnova;

impl Method for OneWayAnova {
    const KIND: RunKind = RunKind::Anova;
    type Params = AnovaParams;
    type Result = AnovaOutcome;
    type Code = AnovaCode;
    type Halt = stats_anova::Halt<Stop>;

    /// `stats_anova::check_points` for the levels the run includes: those
    /// the spec names, or with none named every level of the dataset's group
    /// (`stats_ttest::group_levels`, the ANOVA's own level function). A group
    /// the dataset does not have counts no level; validation refuses it.
    fn columns_total(spec: &OperationSpec<AnovaParams>, dataset: &Dataset) -> usize {
        let p = &spec.parameters;
        let levels = match &p.levels {
            Some(named) => named.len(),
            None => gaply_core::stats_ttest::group_levels(dataset, p.group).map_or(0, |l| l.len()),
        };
        stats_anova::check_points(p, levels)
    }

    fn validate(
        spec: &OperationSpec<AnovaParams>,
        dataset: &Dataset,
    ) -> ValidationReport<AnovaCode> {
        // Through `stats.analyze.one_way_anova`'s façade: the registry
        // first, then `stats_anova::validate`, unchanged.
        stats_adapters::anova_validate(spec, crate::workspace::build(), dataset)
    }

    fn compute(
        spec: &OperationSpec<AnovaParams>,
        build: Build,
        dataset: &Dataset,
        checkpoint: impl FnMut(usize) -> ControlFlow<Stop>,
    ) -> Result<AnovaOutcome, stats_anova::Halt<Stop>> {
        // A check point before the arithmetic and before each post hoc pair.
        stats_adapters::anova_run(spec, build, dataset, checkpoint)
    }

    fn manifest(
        spec: &OperationSpec<AnovaParams>,
        dataset: &Dataset,
        result: &AnovaOutcome,
        elapsed: u64,
    ) -> RunManifest<AnovaCode> {
        stats_anova::manifest(spec, dataset, result, elapsed, Platform::current())
    }

    /// The result's own limitations, as notes.
    fn limitations(result: &AnovaOutcome) -> Vec<Diagnostic<AnovaCode>> {
        result
            .limitations
            .iter()
            .map(|c| Diagnostic::note(*c))
            .collect()
    }

    /// Every check point on a result, though a run whose post hoc tests are
    /// not computable calls fewer; a stop leaves the count the check points
    /// set.
    fn columns_done(total: usize, outcome: &Outcome<Self>) -> Option<usize> {
        match outcome {
            Ok(Ok(_)) => Some(total),
            Ok(Err(_)) | Err(_) => None,
        }
    }

    fn halted(halt: stats_anova::Halt<Stop>) -> Halted<AnovaCode> {
        match halt {
            stats_anova::Halt::Stopped { reason } => Halted::Stopped(reason),
            stats_anova::Halt::Refused(d) => Halted::Refused(d),
        }
    }

    fn panicked() {
        tracing::warn!("stats_desc: the one-way ANOVA panicked");
    }

    fn outputs(held: &Held) -> Option<&Outputs<AnovaOutcome, AnovaCode>> {
        if let Held::Anova(o) = held {
            Some(o)
        } else {
            None
        }
    }

    fn outputs_mut(held: &mut Held) -> Option<&mut Outputs<AnovaOutcome, AnovaCode>> {
        if let Held::Anova(o) = held {
            Some(o)
        } else {
            None
        }
    }

    fn held(outputs: Outputs<AnovaOutcome, AnovaCode>) -> Held {
        Held::Anova(outputs)
    }

    fn job(spec: OperationSpec<AnovaParams>) -> Job {
        Job::Anova(spec)
    }

    fn report(report: ValidationReport<AnovaCode>) -> Report {
        Report::Anova(report)
    }

    fn done(finished: Finished<Self>) -> Done {
        Done::Anova(finished)
    }
}

/// `stats.analyze.correlations`, through the `stats_corr_*` commands
/// (Research OS, Phase 8H-2, decision 355).
struct Correlations;

impl Method for Correlations {
    const KIND: RunKind = RunKind::Correlations;
    type Params = CorrParams;
    type Result = CorrOutcome;
    type Code = CorrCode;
    type Halt = stats_corr::Halt<Stop>;

    /// `stats_corr::check_points`: one before the arithmetic and one before
    /// each pair of each chosen coefficient, from the spec alone.
    fn columns_total(spec: &OperationSpec<CorrParams>, _: &Dataset) -> usize {
        stats_corr::check_points(&spec.parameters)
    }

    fn validate(spec: &OperationSpec<CorrParams>, dataset: &Dataset) -> ValidationReport<CorrCode> {
        // Through `stats.analyze.correlations`' façade: the registry first,
        // then `stats_corr::validate`, unchanged, which refuses a spec over
        // the budget.
        stats_adapters::corr_validate(spec, crate::workspace::build(), dataset)
    }

    fn compute(
        spec: &OperationSpec<CorrParams>,
        build: Build,
        dataset: &Dataset,
        checkpoint: impl FnMut(usize) -> ControlFlow<Stop>,
    ) -> Result<CorrOutcome, stats_corr::Halt<Stop>> {
        // A check point before the arithmetic and before each pair.
        stats_adapters::corr_run(spec, build, dataset, checkpoint)
    }

    fn manifest(
        spec: &OperationSpec<CorrParams>,
        dataset: &Dataset,
        result: &CorrOutcome,
        elapsed: u64,
    ) -> RunManifest<CorrCode> {
        stats_corr::manifest(spec, dataset, result, elapsed, Platform::current())
    }

    /// The result's own limitations, as notes.
    fn limitations(result: &CorrOutcome) -> Vec<Diagnostic<CorrCode>> {
        result
            .limitations
            .iter()
            .map(|c| Diagnostic::note(*c))
            .collect()
    }

    /// Every check point on a result; a stop leaves the count the check
    /// points set.
    fn columns_done(total: usize, outcome: &Outcome<Self>) -> Option<usize> {
        match outcome {
            Ok(Ok(_)) => Some(total),
            Ok(Err(_)) | Err(_) => None,
        }
    }

    fn halted(halt: stats_corr::Halt<Stop>) -> Halted<CorrCode> {
        match halt {
            stats_corr::Halt::Stopped { reason } => Halted::Stopped(reason),
            stats_corr::Halt::Refused(d) => Halted::Refused(d),
        }
    }

    fn panicked() {
        tracing::warn!("stats_desc: the correlations panicked");
    }

    fn outputs(held: &Held) -> Option<&Outputs<CorrOutcome, CorrCode>> {
        if let Held::Correlations(o) = held {
            Some(o)
        } else {
            None
        }
    }

    fn outputs_mut(held: &mut Held) -> Option<&mut Outputs<CorrOutcome, CorrCode>> {
        if let Held::Correlations(o) = held {
            Some(o)
        } else {
            None
        }
    }

    fn held(outputs: Outputs<CorrOutcome, CorrCode>) -> Held {
        Held::Correlations(outputs)
    }

    fn job(spec: OperationSpec<CorrParams>) -> Job {
        Job::Correlations(spec)
    }

    fn report(report: ValidationReport<CorrCode>) -> Report {
        Report::Correlations(report)
    }

    fn done(finished: Finished<Self>) -> Done {
        Done::Correlations(finished)
    }
}

/// `stats.analyze.cronbach_alpha`, through the `stats_alpha_*` commands
/// (Research OS, Phase 8I-2, decision 380).
struct CronbachAlpha;

impl Method for CronbachAlpha {
    const KIND: RunKind = RunKind::CronbachAlpha;
    type Params = AlphaParams;
    type Result = AlphaOutcome;
    type Code = AlphaCode;
    type Halt = stats_alpha::Halt<Stop>;

    /// `stats_alpha::check_points`: one before the arithmetic, one before
    /// each item's row and one before each inter-item correlation, from the
    /// spec alone.
    fn columns_total(spec: &OperationSpec<AlphaParams>, _: &Dataset) -> usize {
        stats_alpha::check_points(&spec.parameters)
    }

    fn validate(
        spec: &OperationSpec<AlphaParams>,
        dataset: &Dataset,
    ) -> ValidationReport<AlphaCode> {
        // Through `stats.analyze.cronbach_alpha`'s façade: the registry
        // first, then `stats_alpha::validate`, unchanged, which refuses a
        // reversed value outside the scale and too few complete rows.
        stats_adapters::alpha_validate(spec, crate::workspace::build(), dataset)
    }

    fn compute(
        spec: &OperationSpec<AlphaParams>,
        build: Build,
        dataset: &Dataset,
        checkpoint: impl FnMut(usize) -> ControlFlow<Stop>,
    ) -> Result<AlphaOutcome, stats_alpha::Halt<Stop>> {
        // A check point before the arithmetic, each item's row and each
        // inter-item correlation.
        stats_adapters::alpha_run(spec, build, dataset, checkpoint)
    }

    fn manifest(
        spec: &OperationSpec<AlphaParams>,
        dataset: &Dataset,
        result: &AlphaOutcome,
        elapsed: u64,
    ) -> RunManifest<AlphaCode> {
        stats_alpha::manifest(spec, dataset, result, elapsed, Platform::current())
    }

    /// The result's own limitations, as notes.
    fn limitations(result: &AlphaOutcome) -> Vec<Diagnostic<AlphaCode>> {
        result
            .limitations
            .iter()
            .map(|c| Diagnostic::note(*c))
            .collect()
    }

    /// Every check point on a result; a stop leaves the count the check
    /// points set.
    fn columns_done(total: usize, outcome: &Outcome<Self>) -> Option<usize> {
        match outcome {
            Ok(Ok(_)) => Some(total),
            Ok(Err(_)) | Err(_) => None,
        }
    }

    fn halted(halt: stats_alpha::Halt<Stop>) -> Halted<AlphaCode> {
        match halt {
            stats_alpha::Halt::Stopped { reason } => Halted::Stopped(reason),
            stats_alpha::Halt::Refused(d) => Halted::Refused(d),
        }
    }

    fn panicked() {
        tracing::warn!("stats_desc: Cronbach's alpha panicked");
    }

    fn outputs(held: &Held) -> Option<&Outputs<AlphaOutcome, AlphaCode>> {
        if let Held::CronbachAlpha(o) = held {
            Some(o)
        } else {
            None
        }
    }

    fn outputs_mut(held: &mut Held) -> Option<&mut Outputs<AlphaOutcome, AlphaCode>> {
        if let Held::CronbachAlpha(o) = held {
            Some(o)
        } else {
            None
        }
    }

    fn held(outputs: Outputs<AlphaOutcome, AlphaCode>) -> Held {
        Held::CronbachAlpha(outputs)
    }

    fn job(spec: OperationSpec<AlphaParams>) -> Job {
        Job::CronbachAlpha(spec)
    }

    fn report(report: ValidationReport<AlphaCode>) -> Report {
        Report::CronbachAlpha(report)
    }

    fn done(finished: Finished<Self>) -> Done {
        Done::CronbachAlpha(finished)
    }
}

/// `stats.analyze.crosstabs`, through the `stats_xtab_*` commands (Research OS,
/// Phase 8X-1, decision 580).
struct Crosstabs;

impl Method for Crosstabs {
    const KIND: RunKind = RunKind::Crosstabs;
    type Params = CrosstabsParams;
    type Result = CrosstabsOutcome;
    type Code = XtabCode;
    type Halt = stats_xtab::Halt<Stop>;

    /// `stats_xtab::check_points`: one, before the arithmetic.
    fn columns_total(_: &OperationSpec<CrosstabsParams>, _: &Dataset) -> usize {
        stats_xtab::check_points()
    }

    fn validate(
        spec: &OperationSpec<CrosstabsParams>,
        dataset: &Dataset,
    ) -> ValidationReport<XtabCode> {
        // Through `stats.analyze.crosstabs`' façade: the registry first,
        // then `stats_xtab::validate`, unchanged.
        stats_adapters::xtab_validate(spec, crate::workspace::build(), dataset)
    }

    fn compute(
        spec: &OperationSpec<CrosstabsParams>,
        build: Build,
        dataset: &Dataset,
        checkpoint: impl FnMut(usize) -> ControlFlow<Stop>,
    ) -> Result<CrosstabsOutcome, stats_xtab::Halt<Stop>> {
        // One check point, before the arithmetic.
        stats_adapters::xtab_run(spec, build, dataset, checkpoint)
    }

    fn manifest(
        spec: &OperationSpec<CrosstabsParams>,
        dataset: &Dataset,
        result: &CrosstabsOutcome,
        elapsed: u64,
    ) -> RunManifest<XtabCode> {
        stats_xtab::manifest(spec, dataset, result, elapsed, Platform::current())
    }

    /// The result's own limitations, as notes.
    fn limitations(result: &CrosstabsOutcome) -> Vec<Diagnostic<XtabCode>> {
        result
            .limitations
            .iter()
            .map(|c| Diagnostic::note(*c))
            .collect()
    }

    /// Its one check point on a result; a stop leaves the count the check
    /// point set.
    fn columns_done(total: usize, outcome: &Outcome<Self>) -> Option<usize> {
        match outcome {
            Ok(Ok(_)) => Some(total),
            Ok(Err(_)) | Err(_) => None,
        }
    }

    fn halted(halt: stats_xtab::Halt<Stop>) -> Halted<XtabCode> {
        match halt {
            stats_xtab::Halt::Stopped { reason } => Halted::Stopped(reason),
            stats_xtab::Halt::Refused(d) => Halted::Refused(d),
        }
    }

    fn panicked() {
        tracing::warn!("stats_desc: crosstabs panicked");
    }

    fn outputs(held: &Held) -> Option<&Outputs<CrosstabsOutcome, XtabCode>> {
        if let Held::Crosstabs(o) = held {
            Some(o)
        } else {
            None
        }
    }

    fn outputs_mut(held: &mut Held) -> Option<&mut Outputs<CrosstabsOutcome, XtabCode>> {
        if let Held::Crosstabs(o) = held {
            Some(o)
        } else {
            None
        }
    }

    fn held(outputs: Outputs<CrosstabsOutcome, XtabCode>) -> Held {
        Held::Crosstabs(outputs)
    }

    fn job(spec: OperationSpec<CrosstabsParams>) -> Job {
        Job::Crosstabs(spec)
    }

    fn report(report: ValidationReport<XtabCode>) -> Report {
        Report::Crosstabs(report)
    }

    fn done(finished: Finished<Self>) -> Done {
        Done::Crosstabs(finished)
    }
}

/// `stats.analyze.independent_ranks`, through the `stats_ranks_*` commands
/// (Research OS, Phase 8Y-1, decision 607).
struct IndependentRanks;

impl Method for IndependentRanks {
    const KIND: RunKind = RunKind::IndependentRanks;
    type Params = IndependentRanksParams;
    type Result = IndependentRanksOutcome;
    type Code = RanksCode;
    type Halt = stats_ranks::Halt<Stop>;

    /// `stats_ranks::check_points`: one, before the arithmetic.
    fn columns_total(_: &OperationSpec<IndependentRanksParams>, _: &Dataset) -> usize {
        stats_ranks::check_points()
    }

    fn validate(
        spec: &OperationSpec<IndependentRanksParams>,
        dataset: &Dataset,
    ) -> ValidationReport<RanksCode> {
        // Through `stats.analyze.independent_ranks`' façade: the registry
        // first, then `stats_ranks::validate`, unchanged.
        stats_adapters::ranks_validate(spec, crate::workspace::build(), dataset)
    }

    fn compute(
        spec: &OperationSpec<IndependentRanksParams>,
        build: Build,
        dataset: &Dataset,
        checkpoint: impl FnMut(usize) -> ControlFlow<Stop>,
    ) -> Result<IndependentRanksOutcome, stats_ranks::Halt<Stop>> {
        // One check point, before the arithmetic.
        stats_adapters::ranks_run(spec, build, dataset, checkpoint)
    }

    fn manifest(
        spec: &OperationSpec<IndependentRanksParams>,
        dataset: &Dataset,
        result: &IndependentRanksOutcome,
        elapsed: u64,
    ) -> RunManifest<RanksCode> {
        stats_ranks::manifest(spec, dataset, result, elapsed, Platform::current())
    }

    /// The result's own limitations, as notes.
    fn limitations(result: &IndependentRanksOutcome) -> Vec<Diagnostic<RanksCode>> {
        result
            .limitations
            .iter()
            .map(|c| Diagnostic::note(*c))
            .collect()
    }

    /// Its one check point on a result; a stop leaves the count the check
    /// point set.
    fn columns_done(total: usize, outcome: &Outcome<Self>) -> Option<usize> {
        match outcome {
            Ok(Ok(_)) => Some(total),
            Ok(Err(_)) | Err(_) => None,
        }
    }

    fn halted(halt: stats_ranks::Halt<Stop>) -> Halted<RanksCode> {
        match halt {
            stats_ranks::Halt::Stopped { reason } => Halted::Stopped(reason),
            stats_ranks::Halt::Refused(d) => Halted::Refused(d),
        }
    }

    fn panicked() {
        tracing::warn!("stats_desc: the rank tests panicked");
    }

    fn outputs(held: &Held) -> Option<&Outputs<IndependentRanksOutcome, RanksCode>> {
        if let Held::IndependentRanks(o) = held {
            Some(o)
        } else {
            None
        }
    }

    fn outputs_mut(held: &mut Held) -> Option<&mut Outputs<IndependentRanksOutcome, RanksCode>> {
        if let Held::IndependentRanks(o) = held {
            Some(o)
        } else {
            None
        }
    }

    fn held(outputs: Outputs<IndependentRanksOutcome, RanksCode>) -> Held {
        Held::IndependentRanks(outputs)
    }

    fn job(spec: OperationSpec<IndependentRanksParams>) -> Job {
        Job::IndependentRanks(spec)
    }

    fn report(report: ValidationReport<RanksCode>) -> Report {
        Report::IndependentRanks(report)
    }

    fn done(finished: Finished<Self>) -> Done {
        Done::IndependentRanks(finished)
    }
}

/// `stats.analyze.related_ranks`, through the `stats_related_ranks_*` commands
/// (Research OS, Phase 8Z-1, decision 641).
struct RelatedRanks;

impl Method for RelatedRanks {
    const KIND: RunKind = RunKind::RelatedRanks;
    type Params = RelatedRanksParams;
    type Result = RelatedRanksOutcome;
    type Code = RelatedRanksCode;
    type Halt = stats_related_ranks::Halt<Stop>;

    /// `stats_related_ranks::check_points`: one, before the arithmetic.
    fn columns_total(_: &OperationSpec<RelatedRanksParams>, _: &Dataset) -> usize {
        stats_related_ranks::check_points()
    }

    fn validate(
        spec: &OperationSpec<RelatedRanksParams>,
        dataset: &Dataset,
    ) -> ValidationReport<RelatedRanksCode> {
        // Through `stats.analyze.related_ranks`' façade: the registry
        // first, then `stats_related_ranks::validate`, unchanged.
        stats_adapters::related_ranks_validate(spec, crate::workspace::build(), dataset)
    }

    fn compute(
        spec: &OperationSpec<RelatedRanksParams>,
        build: Build,
        dataset: &Dataset,
        checkpoint: impl FnMut(usize) -> ControlFlow<Stop>,
    ) -> Result<RelatedRanksOutcome, stats_related_ranks::Halt<Stop>> {
        // One check point, before the arithmetic.
        stats_adapters::related_ranks_run(spec, build, dataset, checkpoint)
    }

    fn manifest(
        spec: &OperationSpec<RelatedRanksParams>,
        dataset: &Dataset,
        result: &RelatedRanksOutcome,
        elapsed: u64,
    ) -> RunManifest<RelatedRanksCode> {
        stats_related_ranks::manifest(spec, dataset, result, elapsed, Platform::current())
    }

    /// The result's own limitations, as notes.
    fn limitations(result: &RelatedRanksOutcome) -> Vec<Diagnostic<RelatedRanksCode>> {
        result
            .limitations
            .iter()
            .map(|c| Diagnostic::note(*c))
            .collect()
    }

    /// Its one check point on a result; a stop leaves the count the check
    /// point set.
    fn columns_done(total: usize, outcome: &Outcome<Self>) -> Option<usize> {
        match outcome {
            Ok(Ok(_)) => Some(total),
            Ok(Err(_)) | Err(_) => None,
        }
    }

    fn halted(halt: stats_related_ranks::Halt<Stop>) -> Halted<RelatedRanksCode> {
        match halt {
            stats_related_ranks::Halt::Stopped { reason } => Halted::Stopped(reason),
            stats_related_ranks::Halt::Refused(d) => Halted::Refused(d),
        }
    }

    fn panicked() {
        tracing::warn!("stats_desc: the related rank tests panicked");
    }

    fn outputs(held: &Held) -> Option<&Outputs<RelatedRanksOutcome, RelatedRanksCode>> {
        if let Held::RelatedRanks(o) = held {
            Some(o)
        } else {
            None
        }
    }

    fn outputs_mut(held: &mut Held) -> Option<&mut Outputs<RelatedRanksOutcome, RelatedRanksCode>> {
        if let Held::RelatedRanks(o) = held {
            Some(o)
        } else {
            None
        }
    }

    fn held(outputs: Outputs<RelatedRanksOutcome, RelatedRanksCode>) -> Held {
        Held::RelatedRanks(outputs)
    }

    fn job(spec: OperationSpec<RelatedRanksParams>) -> Job {
        Job::RelatedRanks(spec)
    }

    fn report(report: ValidationReport<RelatedRanksCode>) -> Report {
        Report::RelatedRanks(report)
    }

    fn done(finished: Finished<Self>) -> Done {
        Done::RelatedRanks(finished)
    }
}

/// `stats.analyze.linear_regression`, through the `stats_linear_regression_*`
/// commands (Research OS, Phase 9A-1, decision 674).
struct LinearRegression;

impl Method for LinearRegression {
    const KIND: RunKind = RunKind::LinearRegression;
    type Params = LinearRegressionParams;
    type Result = LinearRegressionOutcome;
    type Code = LinearRegressionCode;
    type Halt = stats_linear_regression::Halt<Stop>;

    /// `stats_linear_regression::check_points`: one, before the arithmetic.
    fn columns_total(_: &OperationSpec<LinearRegressionParams>, _: &Dataset) -> usize {
        stats_linear_regression::check_points()
    }

    fn validate(
        spec: &OperationSpec<LinearRegressionParams>,
        dataset: &Dataset,
    ) -> ValidationReport<LinearRegressionCode> {
        // Through `stats.analyze.linear_regression`' façade: the registry
        // first, then `stats_linear_regression::validate`, unchanged.
        stats_adapters::linear_regression_validate(spec, crate::workspace::build(), dataset)
    }

    fn compute(
        spec: &OperationSpec<LinearRegressionParams>,
        build: Build,
        dataset: &Dataset,
        checkpoint: impl FnMut(usize) -> ControlFlow<Stop>,
    ) -> Result<LinearRegressionOutcome, stats_linear_regression::Halt<Stop>> {
        // One check point, before the arithmetic.
        stats_adapters::linear_regression_run(spec, build, dataset, checkpoint)
    }

    fn manifest(
        spec: &OperationSpec<LinearRegressionParams>,
        dataset: &Dataset,
        result: &LinearRegressionOutcome,
        elapsed: u64,
    ) -> RunManifest<LinearRegressionCode> {
        stats_linear_regression::manifest(spec, dataset, result, elapsed, Platform::current())
    }

    /// The result's own limitations, as notes.
    fn limitations(result: &LinearRegressionOutcome) -> Vec<Diagnostic<LinearRegressionCode>> {
        result
            .limitations
            .iter()
            .map(|c| Diagnostic::note(*c))
            .collect()
    }

    /// Its one check point on a result; a stop leaves the count the check
    /// point set.
    fn columns_done(total: usize, outcome: &Outcome<Self>) -> Option<usize> {
        match outcome {
            Ok(Ok(_)) => Some(total),
            Ok(Err(_)) | Err(_) => None,
        }
    }

    fn halted(halt: stats_linear_regression::Halt<Stop>) -> Halted<LinearRegressionCode> {
        match halt {
            stats_linear_regression::Halt::Stopped { reason } => Halted::Stopped(reason),
            stats_linear_regression::Halt::Refused(d) => Halted::Refused(d),
        }
    }

    fn panicked() {
        tracing::warn!("stats_desc: the linear regression panicked");
    }

    fn outputs(held: &Held) -> Option<&Outputs<LinearRegressionOutcome, LinearRegressionCode>> {
        if let Held::LinearRegression(o) = held {
            Some(o)
        } else {
            None
        }
    }

    fn outputs_mut(held: &mut Held) -> Option<&mut Outputs<LinearRegressionOutcome, LinearRegressionCode>> {
        if let Held::LinearRegression(o) = held {
            Some(o)
        } else {
            None
        }
    }

    fn held(outputs: Outputs<LinearRegressionOutcome, LinearRegressionCode>) -> Held {
        Held::LinearRegression(outputs)
    }

    fn job(spec: OperationSpec<LinearRegressionParams>) -> Job {
        Job::LinearRegression(spec)
    }

    fn report(report: ValidationReport<LinearRegressionCode>) -> Report {
        Report::LinearRegression(report)
    }

    fn done(finished: Finished<Self>) -> Done {
        Done::LinearRegression(finished)
    }
}

/// `stats.analyze.factor`, through the `stats_factor_*` commands (Research OS,
/// Phase 9B-1, decision 704).
struct Factor;

impl Method for Factor {
    const KIND: RunKind = RunKind::Factor;
    type Params = FactorParams;
    type Result = FactorOutcome;
    type Code = FactorCode;
    type Halt = stats_factor::Halt<Stop>;

    /// `stats_factor::check_points` (decision 700): one before the arithmetic,
    /// one before each item's row and each correlation, one before the
    /// extraction and one before the rotation.
    fn columns_total(spec: &OperationSpec<FactorParams>, _: &Dataset) -> usize {
        stats_factor::check_points(&spec.parameters)
    }

    fn validate(spec: &OperationSpec<FactorParams>, dataset: &Dataset) -> ValidationReport<FactorCode> {
        // Through `stats.analyze.factor`'s façade: the registry first, then
        // `stats_factor::validate`, unchanged.
        stats_adapters::factor_validate(spec, crate::workspace::build(), dataset)
    }

    fn compute(
        spec: &OperationSpec<FactorParams>,
        build: Build,
        dataset: &Dataset,
        checkpoint: impl FnMut(usize) -> ControlFlow<Stop>,
    ) -> Result<FactorOutcome, stats_factor::Halt<Stop>> {
        // Every check point, the iterations' included: the manager's reads
        // the cancel flag and the clock at each call, whatever its count.
        stats_adapters::factor_run(spec, build, dataset, checkpoint)
    }

    fn manifest(
        spec: &OperationSpec<FactorParams>,
        dataset: &Dataset,
        result: &FactorOutcome,
        elapsed: u64,
    ) -> RunManifest<FactorCode> {
        stats_factor::manifest(spec, dataset, result, elapsed, Platform::current())
    }

    /// The result's own limitations, as notes.
    fn limitations(result: &FactorOutcome) -> Vec<Diagnostic<FactorCode>> {
        result.limitations.iter().map(|c| Diagnostic::note(*c)).collect()
    }

    /// Every check point on a result; a stop leaves the count the check point
    /// set.
    fn columns_done(total: usize, outcome: &Outcome<Self>) -> Option<usize> {
        match outcome {
            Ok(Ok(_)) => Some(total),
            Ok(Err(_)) | Err(_) => None,
        }
    }

    fn halted(halt: stats_factor::Halt<Stop>) -> Halted<FactorCode> {
        match halt {
            stats_factor::Halt::Stopped { reason } => Halted::Stopped(reason),
            stats_factor::Halt::Refused(d) => Halted::Refused(d),
        }
    }

    fn panicked() {
        tracing::warn!("stats_desc: the factor analysis panicked");
    }

    fn outputs(held: &Held) -> Option<&Outputs<FactorOutcome, FactorCode>> {
        if let Held::Factor(o) = held {
            Some(o)
        } else {
            None
        }
    }

    fn outputs_mut(held: &mut Held) -> Option<&mut Outputs<FactorOutcome, FactorCode>> {
        if let Held::Factor(o) = held {
            Some(o)
        } else {
            None
        }
    }

    fn held(outputs: Outputs<FactorOutcome, FactorCode>) -> Held {
        Held::Factor(outputs)
    }

    fn job(spec: OperationSpec<FactorParams>) -> Job {
        Job::Factor(spec)
    }

    fn report(report: ValidationReport<FactorCode>) -> Report {
        Report::Factor(report)
    }

    fn done(finished: Finished<Self>) -> Done {
        Done::Factor(finished)
    }
}

/// An end hook that is due, with what it is called with: called only once
/// the records' lock is released (Phase 5C-1a).
struct Due {
    end: EndHook,
    run: RunId,
    copy: RunCopy,
}

impl Due {
    fn call(self) {
        (self.end)(self.run, self.copy)
    }
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
    /// The end hooks of records started with one, by run id (Phase 5C-1a):
    /// added when the record is made and taken out at its terminal step, both
    /// under `entries`' lock, which is always taken first.
    ends: Mutex<BTreeMap<u64, EndHook>>,
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
            ends: Mutex::new(BTreeMap::new()),
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

    /// Drops finished records past [`MAX_RUNS`]: a rerun's first, oldest first
    /// (decision 468), then the oldest of any kind. An unfinished one is never
    /// dropped.
    fn evict(entries: &mut VecDeque<Entry>) {
        while entries.len() > MAX_RUNS {
            let finished_rerun = entries
                .iter()
                .position(|e| e.rerun && e.record.state.is_terminal());
            let oldest = entries.iter().position(|e| e.record.state.is_terminal());
            match finished_rerun.or(oldest) {
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
        self.begin::<Summary>(slot, spec, None)
    }

    /// [`Runs::start`], with an end hook (Phase 5C-1a): a rerun. The same
    /// record, façade and queue; the hook is kept with the record.
    pub fn start_then(
        self: &Arc<Self>,
        slot: Arc<Slot>,
        spec: OperationSpec<DescribeParams>,
        end: EndHook,
    ) -> Result<RunId, NotStarted> {
        self.begin::<Summary>(slot, spec, Some(end))
    }

    /// [`Runs::start`] for a t-test (decision 75): the same record, the same
    /// one foreground run, the same check point and the same last step.
    pub fn start_ttest(
        self: &Arc<Self>,
        slot: Arc<Slot>,
        spec: OperationSpec<TTestParams>,
    ) -> Result<RunId, NotStarted> {
        self.begin::<IndependentT>(slot, spec, None)
    }

    /// [`Runs::start_ttest`], with an end hook (Phase 5C-1a): a rerun.
    pub fn start_ttest_then(
        self: &Arc<Self>,
        slot: Arc<Slot>,
        spec: OperationSpec<TTestParams>,
        end: EndHook,
    ) -> Result<RunId, NotStarted> {
        self.begin::<IndependentT>(slot, spec, Some(end))
    }

    /// [`Runs::start`] for a one-sample t-test (Research OS, Phase 8D-2a,
    /// decision 263).
    pub fn start_one_sample(
        self: &Arc<Self>,
        slot: Arc<Slot>,
        spec: OperationSpec<OneSampleParams>,
    ) -> Result<RunId, NotStarted> {
        self.begin::<OneSampleT>(slot, spec, None)
    }

    /// [`Runs::start_one_sample`], with an end hook: a rerun.
    pub fn start_one_sample_then(
        self: &Arc<Self>,
        slot: Arc<Slot>,
        spec: OperationSpec<OneSampleParams>,
        end: EndHook,
    ) -> Result<RunId, NotStarted> {
        self.begin::<OneSampleT>(slot, spec, Some(end))
    }

    /// [`Runs::start`] for a paired t-test (Research OS, Phase 8D-2a,
    /// decision 263).
    pub fn start_paired(
        self: &Arc<Self>,
        slot: Arc<Slot>,
        spec: OperationSpec<PairedParams>,
    ) -> Result<RunId, NotStarted> {
        self.begin::<PairedT>(slot, spec, None)
    }

    /// [`Runs::start_paired`], with an end hook: a rerun.
    pub fn start_paired_then(
        self: &Arc<Self>,
        slot: Arc<Slot>,
        spec: OperationSpec<PairedParams>,
        end: EndHook,
    ) -> Result<RunId, NotStarted> {
        self.begin::<PairedT>(slot, spec, Some(end))
    }

    /// [`Runs::start`] for a one-way ANOVA (Research OS, Phase 8G-2,
    /// decision 318).
    pub fn start_anova(
        self: &Arc<Self>,
        slot: Arc<Slot>,
        spec: OperationSpec<AnovaParams>,
    ) -> Result<RunId, NotStarted> {
        self.begin::<OneWayAnova>(slot, spec, None)
    }

    /// [`Runs::start_anova`], with an end hook: a rerun.
    pub fn start_anova_then(
        self: &Arc<Self>,
        slot: Arc<Slot>,
        spec: OperationSpec<AnovaParams>,
        end: EndHook,
    ) -> Result<RunId, NotStarted> {
        self.begin::<OneWayAnova>(slot, spec, Some(end))
    }

    /// [`Runs::start`] for correlations (Research OS, Phase 8H-2, decision
    /// 355).
    pub fn start_corr(
        self: &Arc<Self>,
        slot: Arc<Slot>,
        spec: OperationSpec<CorrParams>,
    ) -> Result<RunId, NotStarted> {
        self.begin::<Correlations>(slot, spec, None)
    }

    /// [`Runs::start_corr`], with an end hook: a rerun.
    pub fn start_corr_then(
        self: &Arc<Self>,
        slot: Arc<Slot>,
        spec: OperationSpec<CorrParams>,
        end: EndHook,
    ) -> Result<RunId, NotStarted> {
        self.begin::<Correlations>(slot, spec, Some(end))
    }

    /// [`Runs::start`] for Cronbach's alpha (Research OS, Phase 8I-2,
    /// decision 380).
    pub fn start_alpha(
        self: &Arc<Self>,
        slot: Arc<Slot>,
        spec: OperationSpec<AlphaParams>,
    ) -> Result<RunId, NotStarted> {
        self.begin::<CronbachAlpha>(slot, spec, None)
    }

    /// [`Runs::start_alpha`], with an end hook: a rerun.
    pub fn start_alpha_then(
        self: &Arc<Self>,
        slot: Arc<Slot>,
        spec: OperationSpec<AlphaParams>,
        end: EndHook,
    ) -> Result<RunId, NotStarted> {
        self.begin::<CronbachAlpha>(slot, spec, Some(end))
    }

    /// Starts crosstabs (Research OS, Phase 8X-1, decision 580): its one check
    /// point before the arithmetic, the manager's ceiling and cancel, as every
    /// kind's.
    pub fn start_xtab(
        self: &Arc<Self>,
        slot: Arc<Slot>,
        spec: OperationSpec<CrosstabsParams>,
    ) -> Result<RunId, NotStarted> {
        self.begin::<Crosstabs>(slot, spec, None)
    }

    /// [`Runs::start_xtab`], with an end hook: a project's rerun (Research OS,
    /// Phase 8X-2, decision 590).
    pub fn start_xtab_then(
        self: &Arc<Self>,
        slot: Arc<Slot>,
        spec: OperationSpec<CrosstabsParams>,
        end: EndHook,
    ) -> Result<RunId, NotStarted> {
        self.begin::<Crosstabs>(slot, spec, Some(end))
    }

    /// Starts the rank tests (Research OS, Phase 8Y-1, decision 607): their one
    /// check point before the arithmetic, the manager's ceiling and cancel, as
    /// every kind's.
    pub fn start_ranks(
        self: &Arc<Self>,
        slot: Arc<Slot>,
        spec: OperationSpec<IndependentRanksParams>,
    ) -> Result<RunId, NotStarted> {
        self.begin::<IndependentRanks>(slot, spec, None)
    }

    /// [`Runs::start_ranks`], with an end hook, for a project's rerun (decisions
    /// 607 and 621).
    pub fn start_ranks_then(
        self: &Arc<Self>,
        slot: Arc<Slot>,
        spec: OperationSpec<IndependentRanksParams>,
        end: EndHook,
    ) -> Result<RunId, NotStarted> {
        self.begin::<IndependentRanks>(slot, spec, Some(end))
    }

    /// Starts the related rank tests (Research OS, Phase 8Z-1, decision 641):
    /// their one check point before the arithmetic, the manager's ceiling and
    /// cancel, as every kind's.
    pub fn start_related_ranks(
        self: &Arc<Self>,
        slot: Arc<Slot>,
        spec: OperationSpec<RelatedRanksParams>,
    ) -> Result<RunId, NotStarted> {
        self.begin::<RelatedRanks>(slot, spec, None)
    }

    /// [`Runs::start_related_ranks`], with an end hook, for the project rerun
    /// (decision 655).
    pub fn start_related_ranks_then(
        self: &Arc<Self>,
        slot: Arc<Slot>,
        spec: OperationSpec<RelatedRanksParams>,
        end: EndHook,
    ) -> Result<RunId, NotStarted> {
        self.begin::<RelatedRanks>(slot, spec, Some(end))
    }

    /// Starts the linear regression (Research OS, Phase 9A-1, decision 674): its
    /// one check point before the arithmetic, the manager's ceiling and cancel,
    /// as every kind's.
    pub fn start_linear_regression(
        self: &Arc<Self>,
        slot: Arc<Slot>,
        spec: OperationSpec<LinearRegressionParams>,
    ) -> Result<RunId, NotStarted> {
        self.begin::<LinearRegression>(slot, spec, None)
    }

    /// [`Runs::start_linear_regression`], with an end hook, for a project rerun
    /// (Research OS, Phase 9A-2, decision 686).
    pub fn start_linear_regression_then(
        self: &Arc<Self>,
        slot: Arc<Slot>,
        spec: OperationSpec<LinearRegressionParams>,
        end: EndHook,
    ) -> Result<RunId, NotStarted> {
        self.begin::<LinearRegression>(slot, spec, Some(end))
    }

    /// Starts the factor analysis (Research OS, Phase 9B-1, decision 704): its
    /// check points, the iterations' among them, the manager's ceiling and
    /// cancel, as every kind's.
    pub fn start_factor(self: &Arc<Self>, slot: Arc<Slot>, spec: OperationSpec<FactorParams>) -> Result<RunId, NotStarted> {
        self.begin::<Factor>(slot, spec, None)
    }

    /// [`Runs::start_factor`], with an end hook, for a project rerun (Research OS,
    /// Phase 9B-3, decision 728).
    pub fn start_factor_then(
        self: &Arc<Self>,
        slot: Arc<Slot>,
        spec: OperationSpec<FactorParams>,
        end: EndHook,
    ) -> Result<RunId, NotStarted> {
        self.begin::<Factor>(slot, spec, Some(end))
    }

    /// Every start, of any method (Research OS, Phase 8C): the record, then
    /// the method's check through its façade, then the queue.
    fn begin<M: Method>(
        self: &Arc<Self>,
        slot: Arc<Slot>,
        spec: OperationSpec<M::Params>,
        end: Option<EndHook>,
    ) -> Result<RunId, NotStarted> {
        let run = self.open_record(
            spec.operation_id,
            M::columns_total(&spec, slot.dataset()),
            M::held(Outputs::default()),
            end,
        )?;
        // Outside the lock, so a status poll does not wait on it; the record is
        // unfinished, so no second run can start meanwhile.
        let report = M::validate(&spec, slot.dataset());
        self.queue(run, slot, M::job(spec), M::report(report));
        Ok(run)
    }

    /// Makes the record, `Draft → Validating`: refused while another run of
    /// either kind is unfinished, and once the manager is sealed. Its end
    /// hook, if any, is kept under the same lock, so it exists exactly when
    /// the record does.
    fn open_record(
        &self,
        operation_id: OperationId,
        columns_total: usize,
        held: Held,
        end: Option<EndHook>,
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
            // Only a rerun passes an end hook, so every start function needs no
            // change for this (decision 468).
            rerun: end.is_some(),
            held,
        };
        Self::step(&mut entry, RunEvent::BeginValidation, now);
        entries.push_back(entry);
        if let Some(end) = end {
            lock(&self.ends).insert(run.0, end);
        }
        Self::evict(&mut entries);
        Ok(run)
    }

    /// After the validation: the record's next state from its report, and,
    /// if it may run, the worker on its own thread.
    fn queue(self: &Arc<Self>, run: RunId, slot: Arc<Slot>, job: Job, report: Report) {
        let held_spec = job.held();
        let input = slot.source().map(|s| RunInput {
            slot: slot.id(),
            sha256: s.sha256().clone(),
            recipe: slot
                .prepared()
                .map(|p| stats_adapters::prep_recipe_bytes(&p.recipe)),
        });
        let (queued, due) = {
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
            by_method!(report, Report, |M, r| {
                let o = M::outputs_mut(&mut entry.held)
                    .expect("a record's report is of the record's kind");
                o.validation = Some(r);
                o.spec = Some(held_spec);
                o.input = input;
            });
            // A refused or waiting record has ended here: its end hook, after
            // the lock is released, on this thread.
            let due = if queued {
                None
            } else {
                self.take_end(entry, now)
            };
            (queued, due)
        };
        if let Some(due) = due {
            due.call();
        }
        if queued {
            let me = Arc::clone(self);
            // Tests only: a spawn the test refuses, for the end that a failed
            // spawn makes (Phase 5C-1a).
            #[cfg(test)]
            let refused = tests::SPAWN_REFUSED.with(|r| r.replace(false));
            #[cfg(not(test))]
            let refused = false;
            let spawned = if refused {
                Err(())
            } else {
                std::thread::Builder::new()
                    .name("stats-desc-run".into())
                    .spawn(move || me.work(run, slot, job))
                    .map(|_| ())
                    .map_err(|_| ())
            };
            if spawned.is_err() {
                let due = {
                    let now = self.clock.now_ms();
                    let mut entries = lock(&self.entries);
                    Self::find(&mut entries, run).and_then(|entry| {
                        Self::step(entry, RunEvent::Refuse, now);
                        self.take_end(entry, now)
                    })
                };
                if let Some(due) = due {
                    due.call();
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
        // The method's work, through its façade, with this manager's check
        // point.
        let done = by_method!(job, Job, |M, spec| {
            let outcome = catch_unwind(AssertUnwindSafe(|| {
                M::compute(&spec, build, slot.dataset(), |done| {
                    self.checkpoint(run, done, started)
                })
            }));
            M::done(Finished { outcome, spec })
        });
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
            // What the method's end leaves in the record's column count.
            let panicked = by_method!(&done, Done, |M, d| {
                if let Some(n) = M::columns_done(entry.columns_total, &d.outcome) {
                    entry.columns_done = n;
                }
                d.outcome.is_err()
            });
            if panicked {
                tracing::warn!("stats_desc: the run's work panicked");
            }
            Self::step(entry, RunEvent::Cancel, now);
        } else {
            let elapsed = now.saturating_sub(started);
            // Each finish reads the record's outputs of its own kind: a
            // record's work is always of the record's kind (`begin` made both
            // from one job).
            by_method!(done, Done, |M, d| Self::finish::<M>(
                entry, run, d, &slot, elapsed, now
            ));
        }
        // Phase 5C-1a: the run has ended; its end hook, if any, is called on
        // this worker once the lock is released, with the copy taken here.
        let due = self.take_end(entry, now);
        drop(entries);
        if let Some(due) = due {
            due.call();
        }
    }

    /// A run's last transition, of any method, when no cancel was accepted.
    fn finish<M: Method>(
        entry: &mut Entry,
        run: RunId,
        finished: Finished<M>,
        slot: &Slot,
        elapsed: u64,
        now: u64,
    ) {
        let Finished { outcome, spec } = finished;
        if let Some(n) = M::columns_done(entry.columns_total, &outcome) {
            entry.columns_done = n;
        }
        match outcome {
            Ok(Ok(result)) => {
                let manifest = M::manifest(&spec, slot.dataset(), &result, elapsed);
                let succeeded = Self::step(entry, RunEvent::Succeed, now);
                let o = M::outputs_mut(&mut entry.held)
                    .expect("a record's work is of the record's kind");
                let diagnostics = o
                    .validation
                    .as_ref()
                    .map(|v| v.warnings.clone())
                    .unwrap_or_default();
                if succeeded {
                    let limitations = M::limitations(&result);
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
            Ok(Err(halt)) => match M::halted(halt) {
                Halted::Stopped(reason) => {
                    Self::step(entry, Self::stop_event(reason), now);
                }
                Halted::Refused(d) => {
                    if let Some(o) = M::outputs_mut(&mut entry.held) {
                        o.failure = Some(d);
                    }
                    Self::step(entry, RunEvent::Fail, now);
                }
            },
            Err(_) => {
                M::panicked();
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

    /// The snapshot of a record already found by `M`'s kind.
    fn snapshot_in<M: Method>(&self, entry: &Entry) -> RunSnapshot<M::Code> {
        self.snapshot_of(entry, M::outputs(&entry.held).expect("found by kind"))
    }

    /// A run of `M`'s kind; `run.wrong_kind` for another kind's.
    fn snapshot_as<M: Method>(&self, run: RunId) -> Result<RunSnapshot<M::Code>, RunCode> {
        let mut entries = lock(&self.entries);
        let entry = Self::find_kind(&mut entries, run, M::KIND)?;
        Ok(self.snapshot_in::<M>(entry))
    }

    /// A descriptive summary's run; `run.wrong_kind` for a t-test's.
    pub fn snapshot(&self, run: RunId) -> Result<RunSnapshot, RunCode> {
        self.snapshot_as::<Summary>(run)
    }

    /// A t-test's run; `run.wrong_kind` for a descriptive summary's.
    pub fn ttest_snapshot(&self, run: RunId) -> Result<RunSnapshot<TTestCode>, RunCode> {
        self.snapshot_as::<IndependentT>(run)
    }

    /// A one-sample t-test's run; `run.wrong_kind` for another kind's.
    pub fn one_sample_snapshot(&self, run: RunId) -> Result<RunSnapshot<OneSampleCode>, RunCode> {
        self.snapshot_as::<OneSampleT>(run)
    }

    /// A paired t-test's run; `run.wrong_kind` for another kind's.
    pub fn paired_snapshot(&self, run: RunId) -> Result<RunSnapshot<OneSampleCode>, RunCode> {
        self.snapshot_as::<PairedT>(run)
    }

    /// A one-way ANOVA's run; `run.wrong_kind` for another kind's.
    pub fn anova_snapshot(&self, run: RunId) -> Result<RunSnapshot<AnovaCode>, RunCode> {
        self.snapshot_as::<OneWayAnova>(run)
    }

    /// Correlations' run; `run.wrong_kind` for another kind's.
    pub fn corr_snapshot(&self, run: RunId) -> Result<RunSnapshot<CorrCode>, RunCode> {
        self.snapshot_as::<Correlations>(run)
    }

    /// Cronbach's alpha's run; `run.wrong_kind` for another kind's.
    pub fn alpha_snapshot(&self, run: RunId) -> Result<RunSnapshot<AlphaCode>, RunCode> {
        self.snapshot_as::<CronbachAlpha>(run)
    }

    /// [`Runs::snapshot`] for a crosstabs run (Phase 8X-1).
    pub fn xtab_snapshot(&self, run: RunId) -> Result<RunSnapshot<XtabCode>, RunCode> {
        self.snapshot_as::<Crosstabs>(run)
    }

    /// [`Runs::snapshot`] for a rank tests run (Phase 8Y-1).
    pub fn ranks_snapshot(&self, run: RunId) -> Result<RunSnapshot<RanksCode>, RunCode> {
        self.snapshot_as::<IndependentRanks>(run)
    }

    /// [`Runs::snapshot`] for a related rank tests run (Phase 8Z-1).
    pub fn related_ranks_snapshot(
        &self,
        run: RunId,
    ) -> Result<RunSnapshot<RelatedRanksCode>, RunCode> {
        self.snapshot_as::<RelatedRanks>(run)
    }

    /// [`Runs::snapshot`] for a linear regression run (Phase 9A-1).
    pub fn linear_regression_snapshot(
        &self,
        run: RunId,
    ) -> Result<RunSnapshot<LinearRegressionCode>, RunCode> {
        self.snapshot_as::<LinearRegression>(run)
    }

    /// [`Runs::snapshot`] for a factor analysis run (Phase 9B-1).
    pub fn factor_snapshot(&self, run: RunId) -> Result<RunSnapshot<FactorCode>, RunCode> {
        self.snapshot_as::<Factor>(run)
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

    /// Cancels a run of `M`'s kind; `run.wrong_kind` for another kind's.
    fn cancel_as<M: Method>(&self, run: RunId) -> Result<RunSnapshot<M::Code>, RunCode> {
        let now = self.clock.now_ms();
        let (snapshot, due) = {
            let mut entries = lock(&self.entries);
            let entry = Self::find_kind(&mut entries, run, M::KIND)?;
            Self::cancel_entry(entry, now);
            // A queued run has ended here (Phase 5C-1a): its end hook, after
            // the lock is released, on this thread.
            (self.snapshot_in::<M>(entry), self.take_end(entry, now))
        };
        if let Some(due) = due {
            due.call();
        }
        Ok(snapshot)
    }

    /// Cancels a descriptive summary's run; `run.wrong_kind` for a t-test's.
    pub fn cancel(&self, run: RunId) -> Result<RunSnapshot, RunCode> {
        self.cancel_as::<Summary>(run)
    }

    /// Cancels a t-test's run; `run.wrong_kind` for a descriptive summary's.
    pub fn ttest_cancel(&self, run: RunId) -> Result<RunSnapshot<TTestCode>, RunCode> {
        self.cancel_as::<IndependentT>(run)
    }

    /// Cancels a one-sample t-test's run; `run.wrong_kind` for another kind's.
    pub fn one_sample_cancel(&self, run: RunId) -> Result<RunSnapshot<OneSampleCode>, RunCode> {
        self.cancel_as::<OneSampleT>(run)
    }

    /// Cancels a paired t-test's run; `run.wrong_kind` for another kind's.
    pub fn paired_cancel(&self, run: RunId) -> Result<RunSnapshot<OneSampleCode>, RunCode> {
        self.cancel_as::<PairedT>(run)
    }

    /// Cancels a one-way ANOVA's run; `run.wrong_kind` for another kind's.
    pub fn anova_cancel(&self, run: RunId) -> Result<RunSnapshot<AnovaCode>, RunCode> {
        self.cancel_as::<OneWayAnova>(run)
    }

    /// Cancels correlations' run; `run.wrong_kind` for another kind's.
    pub fn corr_cancel(&self, run: RunId) -> Result<RunSnapshot<CorrCode>, RunCode> {
        self.cancel_as::<Correlations>(run)
    }

    /// Cancels Cronbach's alpha's run; `run.wrong_kind` for another kind's.
    pub fn alpha_cancel(&self, run: RunId) -> Result<RunSnapshot<AlphaCode>, RunCode> {
        self.cancel_as::<CronbachAlpha>(run)
    }

    /// [`Runs::cancel`] for a crosstabs run (Phase 8X-1).
    pub fn xtab_cancel(&self, run: RunId) -> Result<RunSnapshot<XtabCode>, RunCode> {
        self.cancel_as::<Crosstabs>(run)
    }

    /// [`Runs::cancel`] for a rank tests run (Phase 8Y-1).
    pub fn ranks_cancel(&self, run: RunId) -> Result<RunSnapshot<RanksCode>, RunCode> {
        self.cancel_as::<IndependentRanks>(run)
    }

    /// [`Runs::cancel`] for a related rank tests run (Phase 8Z-1).
    pub fn related_ranks_cancel(
        &self,
        run: RunId,
    ) -> Result<RunSnapshot<RelatedRanksCode>, RunCode> {
        self.cancel_as::<RelatedRanks>(run)
    }

    /// [`Runs::cancel`] for a linear regression run (Phase 9A-1).
    pub fn linear_regression_cancel(
        &self,
        run: RunId,
    ) -> Result<RunSnapshot<LinearRegressionCode>, RunCode> {
        self.cancel_as::<LinearRegression>(run)
    }

    /// [`Runs::cancel`] for a factor analysis run (Phase 9B-1).
    pub fn factor_cancel(&self, run: RunId) -> Result<RunSnapshot<FactorCode>, RunCode> {
        self.cancel_as::<Factor>(run)
    }

    /// The result of a run of `M`'s kind that succeeded. Otherwise the closed
    /// reason: `not_finished`, `cancelled`, `timed_out`, `no_result`, or
    /// `wrong_kind` for another kind's run.
    fn result_as<M: Method>(
        &self,
        run: RunId,
    ) -> Result<ResultBundle<M::Result, M::Code>, RunCode> {
        let mut entries = lock(&self.entries);
        let entry = Self::find_kind(&mut entries, run, M::KIND)?;
        let outputs = M::outputs(&entry.held).expect("found by kind");
        result_of(entry.record.state, &outputs.bundle)
    }

    /// The result of a descriptive summary that succeeded. Otherwise the
    /// closed reason: `not_finished`, `cancelled`, `timed_out`, `no_result`,
    /// or `wrong_kind` for a t-test's run.
    pub fn result(&self, run: RunId) -> Result<ResultBundle<DescribeResult, DescCode>, RunCode> {
        self.result_as::<Summary>(run)
    }

    /// [`Runs::result`] for a t-test; `wrong_kind` for a summary's run.
    pub fn ttest_result(
        &self,
        run: RunId,
    ) -> Result<ResultBundle<TTestResult, TTestCode>, RunCode> {
        self.result_as::<IndependentT>(run)
    }

    /// [`Runs::result`] for a one-sample t-test; `wrong_kind` for another
    /// kind's run.
    pub fn one_sample_result(
        &self,
        run: RunId,
    ) -> Result<ResultBundle<OneSampleResult, OneSampleCode>, RunCode> {
        self.result_as::<OneSampleT>(run)
    }

    /// [`Runs::result`] for a paired t-test; `wrong_kind` for another kind's
    /// run.
    pub fn paired_result(
        &self,
        run: RunId,
    ) -> Result<ResultBundle<PairedResult, OneSampleCode>, RunCode> {
        self.result_as::<PairedT>(run)
    }

    /// [`Runs::result`] for a one-way ANOVA; `wrong_kind` for another kind's
    /// run.
    pub fn anova_result(
        &self,
        run: RunId,
    ) -> Result<ResultBundle<AnovaOutcome, AnovaCode>, RunCode> {
        self.result_as::<OneWayAnova>(run)
    }

    /// [`Runs::result`] for correlations; `wrong_kind` for another kind's
    /// run.
    pub fn corr_result(&self, run: RunId) -> Result<ResultBundle<CorrOutcome, CorrCode>, RunCode> {
        self.result_as::<Correlations>(run)
    }

    /// [`Runs::result`] for Cronbach's alpha; `wrong_kind` for another
    /// kind's run.
    pub fn alpha_result(
        &self,
        run: RunId,
    ) -> Result<ResultBundle<AlphaOutcome, AlphaCode>, RunCode> {
        self.result_as::<CronbachAlpha>(run)
    }

    /// The result of a crosstabs run that succeeded (Phase 8X-1).
    pub fn xtab_result(
        &self,
        run: RunId,
    ) -> Result<ResultBundle<CrosstabsOutcome, XtabCode>, RunCode> {
        self.result_as::<Crosstabs>(run)
    }

    /// The result of a rank tests run that succeeded (Phase 8Y-1).
    pub fn ranks_result(
        &self,
        run: RunId,
    ) -> Result<ResultBundle<IndependentRanksOutcome, RanksCode>, RunCode> {
        self.result_as::<IndependentRanks>(run)
    }

    /// The result of a related rank tests run that succeeded (Phase 8Z-1).
    pub fn related_ranks_result(
        &self,
        run: RunId,
    ) -> Result<ResultBundle<RelatedRanksOutcome, RelatedRanksCode>, RunCode> {
        self.result_as::<RelatedRanks>(run)
    }

    /// The result of a linear regression run that succeeded (Phase 9A-1).
    pub fn linear_regression_result(
        &self,
        run: RunId,
    ) -> Result<ResultBundle<LinearRegressionOutcome, LinearRegressionCode>, RunCode> {
        self.result_as::<LinearRegression>(run)
    }

    /// The result of a factor analysis run that succeeded (Phase 9B-1).
    pub fn factor_result(&self, run: RunId) -> Result<ResultBundle<FactorOutcome, FactorCode>, RunCode> {
        self.result_as::<Factor>(run)
    }

    /// A copy of `run` for a project Save (Phase 5B-2), of either kind:
    /// `run.unknown_run` when no record has that id.
    pub fn copy_for_save(&self, run: RunId) -> Result<RunCopy, RunCode> {
        let now_ms = self.clock.now_ms();
        let mut entries = lock(&self.entries);
        let entry = Self::find(&mut entries, run).ok_or(RunCode::UnknownRun)?;
        Ok(Self::copy_of(entry, now_ms))
    }

    /// The copy of a record a Save takes, and the one an end hook is given,
    /// read under the records' lock.
    fn copy_of(entry: &Entry, now_ms: u64) -> RunCopy {
        let parts = by_method!(&entry.held, Held, |_M, o| o.copy());
        let (spec_json, capability) = match parts.spec {
            Some((json, id, version)) => (Some(json), Some((id, version))),
            None => (None, None),
        };
        RunCopy {
            kind: entry.held.kind(),
            state: entry.record.state,
            started_ms: entry.record.started_ms,
            ended_ms: entry.record.ended_ms,
            now_ms,
            spec_json,
            capability,
            input: parts.input,
            bundle_json: parts.bundle_json,
            manifest_json: parts.manifest_json,
            manifest_inputs: parts.manifest_inputs,
            failure: parts.failure,
        }
    }

    /// The end hook of a record that has just ended, with the copy of the
    /// record as it ended (Phase 5C-1a): taken out under the records' lock,
    /// so it is called at most once. `None` while the record is unfinished,
    /// or when it has no hook or its hook was taken already.
    fn take_end(&self, entry: &Entry, now: u64) -> Option<Due> {
        if !entry.record.state.is_terminal() {
            return None;
        }
        let end = lock(&self.ends).remove(&entry.record.run_id.0)?;
        Some(Due {
            end,
            run: entry.record.run_id,
            copy: Self::copy_of(entry, now),
        })
    }

    /// Whether this manager, or an engine before it, ever handed out `run`:
    /// an id that was issued and is not held was evicted, or its workspace
    /// was left.
    pub fn was_issued(&self, run: RunId) -> bool {
        run.0 >= 1 && run.0 <= self.last_id()
    }

    /// Drops a finished record, for Remove in the Output (decision 468):
    /// `NotFinished` for an unfinished one, which stays, and `UnknownRun` for
    /// none. The id stays issued, so a later Save reads it as evicted.
    pub fn forget(&self, run: RunId) -> Result<(), RunCode> {
        let mut entries = lock(&self.entries);
        let at = entries
            .iter()
            .position(|e| e.record.run_id == run)
            .ok_or(RunCode::UnknownRun)?;
        if !entries[at].record.state.is_terminal() {
            return Err(RunCode::NotFinished);
        }
        entries.remove(at);
        Ok(())
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
            rerun: false,
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

    /// Decision 468: a rerun's finished record goes before any other finished
    /// one, oldest first; only then the oldest finished record of any kind. An
    /// unfinished rerun is never dropped, so 40 reruns cannot push out a live
    /// result the Output still shows.
    #[test]
    fn eviction_drops_a_reruns_finished_record_first_and_never_an_unfinished_one() {
        // The Output holds 32, and a going rerun is one more (ADR-012 Part 7).
        let records = MAX_RUNS;
        assert!(records > 32, "the Output holds 32: {records} records");

        let rerun = |run, state| Entry {
            rerun: true,
            ..entry(run, state)
        };

        // A live finished record, oldest, then two finished reruns and an
        // unfinished rerun older than either: the older finished rerun goes.
        let mut entries: VecDeque<Entry> = VecDeque::new();
        entries.push_back(entry(1, RunState::Succeeded));
        entries.push_back(rerun(2, RunState::Running));
        entries.push_back(rerun(3, RunState::Succeeded));
        entries.push_back(rerun(4, RunState::Failed));
        for run in 5..=MAX_RUNS as u64 + 1 {
            entries.push_back(entry(run, RunState::Succeeded));
        }
        assert_eq!(entries.len(), MAX_RUNS + 1);
        Runs::evict(&mut entries);
        let held: Vec<u64> = entries.iter().map(|e| e.record.run_id.0).collect();
        assert_eq!(held.len(), MAX_RUNS);
        assert!(!held.contains(&3), "the older finished rerun: {held:?}");
        for kept in [1, 2, 4] {
            assert!(held.contains(&kept), "{kept} is kept: {held:?}");
        }

        // With only an unfinished rerun: the rule falls back to the oldest
        // finished record of any kind, and the rerun stays.
        let mut entries: VecDeque<Entry> = VecDeque::new();
        entries.push_back(rerun(1, RunState::Running));
        for run in 2..=MAX_RUNS as u64 + 1 {
            entries.push_back(entry(run, RunState::Succeeded));
        }
        Runs::evict(&mut entries);
        let held: Vec<u64> = entries.iter().map(|e| e.record.run_id.0).collect();
        assert_eq!(held.len(), MAX_RUNS);
        assert_eq!(held[0], 1, "the unfinished rerun stays");
        assert!(!held.contains(&2), "the oldest finished one: {held:?}");
    }

    /// Decision 468: the flag comes from the end hook, at the one place every
    /// start passes through, so a start function need not know about it.
    #[test]
    fn a_record_is_a_rerun_exactly_when_it_was_started_with_an_end_hook() {
        let runs = Arc::new(Runs::default());
        let calls = Arc::new(Mutex::new(Vec::new()));
        let flag = |run: RunId| {
            lock(&runs.entries)
                .iter()
                .find(|e| e.record.run_id == run)
                .map(|e| e.rerun)
                .expect("held")
        };

        let (slot, spec) = summary_of(1);
        let live = runs.start(slot, spec).unwrap();
        wait_for("the live run ended", || {
            runs.snapshot(live).unwrap().record.state.is_terminal()
        });
        assert!(!flag(live), "started with no hook");

        let (slot, spec) = summary_of(1);
        let rerun = runs
            .start_then(slot, spec, recorder(&runs, &calls))
            .unwrap();
        wait_for("the hook was called", || calls.lock().unwrap().len() == 1);
        assert!(flag(rerun), "started with a hook");
    }

    /// Decision 468: Remove drops its record. A finished one goes and is
    /// afterwards only an id that was issued; an unfinished one is refused and
    /// stays; a record that is not held is `UnknownRun`, which Remove treats
    /// as done.
    #[test]
    fn forget_drops_a_finished_record_refuses_an_unfinished_one_and_knows_no_other() {
        let runs = Arc::new(Runs::default());
        lock(&runs.entries).push_back(entry(1, RunState::Succeeded));
        lock(&runs.entries).push_back(entry(2, RunState::Running));
        lock(&runs.entries).push_back(entry(3, RunState::Queued));
        runs.next.store(3, Ordering::SeqCst);

        assert!(matches!(runs.forget(RunId(2)), Err(RunCode::NotFinished)));
        assert!(matches!(runs.forget(RunId(3)), Err(RunCode::NotFinished)));
        assert_eq!(runs.held(), 3, "an unfinished record stays");

        assert!(runs.forget(RunId(1)).is_ok());
        assert_eq!(runs.held(), 2, "the finished record went");
        assert!(matches!(runs.forget(RunId(1)), Err(RunCode::UnknownRun)));
        assert!(runs.was_issued(RunId(1)), "a Save reads it as evicted");
        assert!(matches!(runs.forget(RunId(99)), Err(RunCode::UnknownRun)));
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
            id: 1,
            display_name: "synthetic".into(),
            table: Table::File(dataset),
            source: None,
            file_findings: Vec::new(),
            file_profile: None,
        });
        assert_eq!(runs.start(slot, spec), Err(NotStarted::Sealed));
        assert_eq!(runs.held(), 1, "no record was made");
    }

    thread_local! {
        /// Set by a test: the next spawn on this thread is refused.
        pub(super) static SPAWN_REFUSED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    }

    /* ---- Phase 5C-1a: the end hook (decision 186) ------------------------ */

    use gaply_core::stats_desc::{Alias, ColumnRequest, TreatAs};
    use gaply_core::stats_l1::dataset::{
        build_dataset, ColumnAlias, DataFingerprint, RawTable, TableRef,
    };
    use std::sync::atomic::AtomicUsize;
    use std::sync::mpsc;
    use std::time::Duration;

    fn numbers() -> Dataset {
        build_dataset(
            RawTable::from_rows(
                vec!["x".into()],
                vec![vec!["1".into()], vec!["2".into()], vec!["4".into()]],
            ),
            DataFingerprint::of_bytes(b"synthetic", "unit", TableRef::FirstTable),
        )
        .expect("dataset")
    }

    /// A summary of column `column`: 1 exists, 9 does not.
    fn summary_of(column: u32) -> (Arc<Slot>, OperationSpec<DescribeParams>) {
        let dataset = numbers();
        let spec = stats_desc::spec(
            OperationId(1),
            &dataset,
            DescribeParams {
                columns: vec![ColumnRequest {
                    alias: Alias(ColumnAlias(column)),
                    treat_as: TreatAs::Numeric,
                }],
                missing_codes: vec![],
            },
        );
        let slot = Arc::new(Slot {
            id: 1,
            display_name: "synthetic".into(),
            table: Table::File(dataset),
            source: None,
            file_findings: Vec::new(),
            file_profile: None,
        });
        (slot, spec)
    }

    /// One call of an end hook: the run, its state in the copy, whether the
    /// copy holds a result, the thread it ran on, and whether the records'
    /// lock was free then.
    type Call = (RunId, RunState, bool, std::thread::ThreadId, bool);

    fn recorder(runs: &Arc<Runs>, calls: &Arc<Mutex<Vec<Call>>>) -> EndHook {
        let (runs, calls) = (Arc::clone(runs), Arc::clone(calls));
        Box::new(move |run, copy| {
            let free = runs.entries.try_lock().is_ok();
            calls.lock().unwrap().push((
                run,
                copy.state,
                copy.bundle_json.is_some(),
                std::thread::current().id(),
                free,
            ));
        })
    }

    fn wait_for(what: &str, done: impl Fn() -> bool) {
        let start = Instant::now();
        while !done() {
            assert!(start.elapsed() < Duration::from_secs(10), "{what}");
            std::thread::sleep(Duration::from_millis(1));
        }
    }

    /// A manager whose runs stop at `BeforeStart` until the test lets them go.
    fn parking() -> (Arc<Runs>, mpsc::Receiver<()>, mpsc::Sender<()>) {
        let (reached, at_start) = mpsc::channel::<()>();
        let (go, gate) = mpsc::channel::<()>();
        let gate = Mutex::new(gate);
        let hook: CheckpointHook = Arc::new(move |_run, p| {
            if p == Point::BeforeStart {
                reached.send(()).unwrap();
                let _ = gate.lock().unwrap().recv();
            }
        });
        let runs = Arc::new(Runs::with_hooks(
            Arc::new(SinceStart(Instant::now())),
            Some(hook),
            RUN_CEILING_MS,
        ));
        (runs, at_start, go)
    }

    /// The worker's last step: called once, on the worker, with the result in
    /// the copy, after the records' lock is released.
    #[test]
    fn an_end_hook_is_called_once_on_the_worker_after_a_run_ends() {
        let runs = Arc::new(Runs::default());
        let calls = Arc::new(Mutex::new(Vec::new()));
        let (slot, spec) = summary_of(1);
        let run = runs
            .start_then(slot, spec, recorder(&runs, &calls))
            .unwrap();
        wait_for("the hook was called", || calls.lock().unwrap().len() == 1);
        let (id, state, result, thread, free) = calls.lock().unwrap()[0];
        assert_eq!(
            (id, state, result, free),
            (run, RunState::Succeeded, true, true)
        );
        assert_ne!(thread, std::thread::current().id(), "on the worker");
        assert_eq!(
            runs.snapshot(run).unwrap().record.state,
            RunState::Succeeded
        );
        assert!(lock(&runs.ends).is_empty(), "taken out once called");
    }

    /// A refusal in `queue()`: called once, at once, on the thread that
    /// started the run.
    #[test]
    fn an_end_hook_is_called_once_when_validation_refuses_the_run() {
        let runs = Arc::new(Runs::default());
        let calls = Arc::new(Mutex::new(Vec::new()));
        let (slot, spec) = summary_of(9);
        let run = runs
            .start_then(slot, spec, recorder(&runs, &calls))
            .unwrap();
        let got = calls.lock().unwrap().clone();
        assert_eq!(got.len(), 1, "called before start returned");
        let (id, state, result, thread, free) = got[0];
        assert_eq!(
            (id, state, result, free),
            (run, RunState::Refused, false, true)
        );
        assert_eq!(thread, std::thread::current().id());
    }

    /// A failed spawn: called once, on the thread that started the run.
    #[test]
    fn an_end_hook_is_called_once_when_the_worker_cannot_be_spawned() {
        let runs = Arc::new(Runs::default());
        let calls = Arc::new(Mutex::new(Vec::new()));
        let (slot, spec) = summary_of(1);
        SPAWN_REFUSED.with(|r| r.set(true));
        let run = runs
            .start_then(slot, spec, recorder(&runs, &calls))
            .unwrap();
        let got = calls.lock().unwrap().clone();
        assert_eq!(got.len(), 1);
        let (id, state, result, thread, free) = got[0];
        assert_eq!(
            (id, state, result, free),
            (run, RunState::Refused, false, true)
        );
        assert_eq!(thread, std::thread::current().id());
    }

    /// A cancel while queued: called once, on the cancelling thread; the
    /// worker, let go afterwards, finds the run ended and calls nothing.
    #[test]
    fn an_end_hook_is_called_once_when_a_queued_run_is_cancelled() {
        let (runs, at_start, go) = parking();
        let calls = Arc::new(Mutex::new(Vec::new()));
        let (slot, spec) = summary_of(1);
        let held = Arc::downgrade(&slot);
        let run = runs
            .start_then(slot, spec, recorder(&runs, &calls))
            .unwrap();
        at_start.recv().unwrap();
        assert!(calls.lock().unwrap().is_empty(), "not before it ends");
        runs.cancel(run).unwrap();
        let got = calls.lock().unwrap().clone();
        assert_eq!(got.len(), 1);
        let (id, state, _, thread, free) = got[0];
        assert_eq!((id, state, free), (run, RunState::Cancelled, true));
        assert_eq!(thread, std::thread::current().id());
        go.send(()).unwrap();
        wait_for("the worker returned", || held.strong_count() == 0);
        assert_eq!(calls.lock().unwrap().len(), 1, "never twice");
    }

    /// A start that makes no record drops its hook unused; a run started
    /// without one ends as before and leaves nothing in the manager.
    #[test]
    fn a_start_that_makes_no_record_never_calls_its_hook() {
        struct Dropped(Arc<AtomicUsize>);
        impl Drop for Dropped {
            fn drop(&mut self) {
                self.0.fetch_add(1, Ordering::SeqCst);
            }
        }
        let (runs, at_start, go) = parking();
        let (slot, spec) = summary_of(1);
        let parked = runs.start(slot, spec).unwrap();
        at_start.recv().unwrap();
        let dropped = Arc::new(AtomicUsize::new(0));
        let called = Arc::new(AtomicUsize::new(0));
        let hook = |dropped: &Arc<AtomicUsize>, called: &Arc<AtomicUsize>| -> EndHook {
            let (d, c) = (Dropped(Arc::clone(dropped)), Arc::clone(called));
            Box::new(move |_, _| {
                let _keep = &d;
                c.fetch_add(1, Ordering::SeqCst);
            })
        };
        let (slot, spec) = summary_of(1);
        assert_eq!(
            runs.start_then(slot, spec, hook(&dropped, &called)),
            Err(NotStarted::Busy)
        );
        assert_eq!(
            (
                dropped.load(Ordering::SeqCst),
                called.load(Ordering::SeqCst)
            ),
            (1, 0)
        );
        go.send(()).unwrap();
        wait_for("the parked run ended", || {
            runs.snapshot(parked).unwrap().record.state.is_terminal()
        });
        assert_eq!(runs.seal(), Ok(()));
        let (slot, spec) = summary_of(1);
        assert_eq!(
            runs.start_then(slot, spec, hook(&dropped, &called)),
            Err(NotStarted::Sealed)
        );
        assert_eq!(
            (
                dropped.load(Ordering::SeqCst),
                called.load(Ordering::SeqCst)
            ),
            (2, 0)
        );
        assert!(lock(&runs.ends).is_empty());
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

    /* ---- Phase 8C: what each kind does at its check points (decision 249) -- */

    /// Two columns: `x`, numbers, and `g`, two groups of three.
    fn two_groups() -> Arc<Slot> {
        let rows = [
            ("1", "a"),
            ("2", "a"),
            ("4", "a"),
            ("3", "b"),
            ("5", "b"),
            ("8", "b"),
        ];
        let dataset = build_dataset(
            RawTable::from_rows(
                vec!["x".into(), "g".into()],
                rows.iter()
                    .map(|(x, g)| vec![x.to_string(), g.to_string()])
                    .collect(),
            ),
            DataFingerprint::of_bytes(b"synthetic two groups", "unit", TableRef::FirstTable),
        )
        .expect("dataset");
        Arc::new(Slot {
            id: 1,
            display_name: "synthetic".into(),
            table: Table::File(dataset),
            source: None,
            file_findings: Vec::new(),
            file_profile: None,
        })
    }

    /// A summary of both columns.
    fn summary_of_both(slot: &Slot) -> OperationSpec<DescribeParams> {
        stats_desc::spec(
            OperationId(1),
            slot.dataset(),
            DescribeParams {
                columns: vec![
                    ColumnRequest {
                        alias: Alias(ColumnAlias(1)),
                        treat_as: TreatAs::Numeric,
                    },
                    ColumnRequest {
                        alias: Alias(ColumnAlias(2)),
                        treat_as: TreatAs::Categorical,
                    },
                ],
                missing_codes: vec![],
            },
        )
    }

    /// Welch's t-test of `x` by `g`.
    fn ttest_of(slot: &Slot) -> OperationSpec<TTestParams> {
        stats_ttest::spec(
            OperationId(2),
            slot.dataset(),
            TTestParams {
                outcome: Alias(ColumnAlias(1)),
                group: Alias(ColumnAlias(2)),
                levels: Some([0, 1]),
                variant: Some(stats_ttest::Variant::Welch),
                missing_codes: vec![],
            },
        )
    }

    /// Every point a hook saw, with the name of the thread it saw it on.
    type Seen = Arc<Mutex<Vec<(Point, Option<String>)>>>;

    fn watching(seen: &Seen) -> Arc<Runs> {
        let seen = Arc::clone(seen);
        let hook: CheckpointHook = Arc::new(move |_run, p| {
            let name = std::thread::current().name().map(str::to_string);
            seen.lock().unwrap().push((p, name));
        });
        Arc::new(Runs::with_hooks(
            Arc::new(SinceStart(Instant::now())),
            Some(hook),
            RUN_CEILING_MS,
        ))
    }

    /// Differences 2, 9 and 10 of the 8C step: the summary checks once per
    /// column and counts its columns, ending with every one done; the t-test
    /// checks once, at 0, and has no columns. Both work on the thread named
    /// `stats-desc-run`.
    #[test]
    fn each_kind_reaches_its_own_check_points_on_the_run_thread() {
        let slot = two_groups();
        let thread = Some("stats-desc-run".to_string());

        let seen: Seen = Arc::default();
        let runs = watching(&seen);
        let run = runs
            .start(Arc::clone(&slot), summary_of_both(&slot))
            .unwrap();
        wait_for("the summary ended", || {
            runs.snapshot(run).unwrap().record.state.is_terminal()
        });
        let s = runs.snapshot(run).unwrap();
        assert_eq!(s.record.state, RunState::Succeeded);
        assert_eq!((s.columns_done, s.columns_total), (2, 2));
        let points: Vec<_> = seen.lock().unwrap().iter().map(|(p, _)| *p).collect();
        assert_eq!(
            points,
            [
                Point::BeforeStart,
                Point::Column(0),
                Point::Passed(0),
                Point::Column(1),
                Point::Passed(1),
            ]
        );
        assert!(seen.lock().unwrap().iter().all(|(_, n)| *n == thread));

        let seen: Seen = Arc::default();
        let runs = watching(&seen);
        let run = runs
            .start_ttest(Arc::clone(&slot), ttest_of(&slot))
            .unwrap();
        wait_for("the t-test ended", || {
            runs.ttest_snapshot(run).unwrap().record.state.is_terminal()
        });
        let s = runs.ttest_snapshot(run).unwrap();
        assert_eq!(s.record.state, RunState::Succeeded);
        assert_eq!((s.columns_done, s.columns_total), (0, 0));
        let points: Vec<_> = seen.lock().unwrap().iter().map(|(p, _)| *p).collect();
        assert_eq!(
            points,
            [Point::BeforeStart, Point::Column(0), Point::Passed(0)]
        );
        assert!(seen.lock().unwrap().iter().all(|(_, n)| *n == thread));
    }

    /// Difference 1 of the 8C step, for the t-test: a cancel that lands after
    /// its one check point wins, and leaves no column count in the record.
    #[test]
    fn a_ttest_cancel_after_its_check_point_wins_and_counts_no_columns() {
        let (reached, at_passed) = mpsc::channel::<()>();
        let (go, gate) = mpsc::channel::<()>();
        let gate = Mutex::new(gate);
        let hook: CheckpointHook = Arc::new(move |_run, p| {
            if p == Point::Passed(0) {
                reached.send(()).unwrap();
                let _ = gate.lock().unwrap().recv();
            }
        });
        let runs = Arc::new(Runs::with_hooks(
            Arc::new(SinceStart(Instant::now())),
            Some(hook),
            RUN_CEILING_MS,
        ));
        let slot = two_groups();
        let run = runs
            .start_ttest(Arc::clone(&slot), ttest_of(&slot))
            .unwrap();
        at_passed.recv().unwrap();
        runs.ttest_cancel(run).unwrap();
        go.send(()).unwrap();
        wait_for("the t-test ended", || {
            runs.ttest_snapshot(run).unwrap().record.state.is_terminal()
        });
        let s = runs.ttest_snapshot(run).unwrap();
        assert_eq!(s.record.state, RunState::Cancelled);
        assert_eq!((s.columns_done, s.columns_total), (0, 0));
        assert_eq!(runs.ttest_result(run).map(|_| ()), Err(RunCode::Cancelled));
    }

    /* ---- Phase 8D-2a: the one-sample and paired t-tests (decision 263) ---- */

    use gaply_core::stats_onesample::{self, OneSampleParams, PairedParams};

    /// Two numeric columns, `a` and `b`, whose differences vary.
    fn two_columns() -> Arc<Slot> {
        let rows = [
            ("1", "2"),
            ("2", "5"),
            ("4", "5"),
            ("3", "7"),
            ("5", "6"),
            ("8", "9"),
        ];
        let dataset = build_dataset(
            RawTable::from_rows(
                vec!["a".into(), "b".into()],
                rows.iter()
                    .map(|(a, b)| vec![a.to_string(), b.to_string()])
                    .collect(),
            ),
            DataFingerprint::of_bytes(b"synthetic two columns", "unit", TableRef::FirstTable),
        )
        .expect("dataset");
        Arc::new(Slot {
            id: 2,
            display_name: "synthetic pairs".into(),
            table: Table::File(dataset),
            source: None,
            file_findings: Vec::new(),
            file_profile: None,
        })
    }

    /// The one-sample t-test of `a` against 2.5.
    fn one_sample_of(slot: &Slot) -> OperationSpec<OneSampleParams> {
        stats_onesample::one_sample_spec(
            OperationId(3),
            slot.dataset(),
            OneSampleParams {
                outcome: Alias(ColumnAlias(1)),
                test_value: Some(2.5),
                missing_codes: vec![],
            },
        )
    }

    /// The paired t-test of `a` and `b`.
    fn paired_of(slot: &Slot) -> OperationSpec<PairedParams> {
        stats_onesample::paired_spec(
            OperationId(4),
            slot.dataset(),
            PairedParams {
                first: Alias(ColumnAlias(1)),
                second: Alias(ColumnAlias(2)),
                missing_codes: vec![],
            },
        )
    }

    /// Each new kind checks once, at 0, counts no columns and works on the
    /// thread named `stats-desc-run`, as the independent t-test does.
    #[test]
    fn the_one_sample_and_paired_kinds_reach_their_one_check_point_on_the_run_thread() {
        let slot = two_columns();
        let thread = Some("stats-desc-run".to_string());
        for paired in [false, true] {
            let seen: Seen = Arc::default();
            let runs = watching(&seen);
            let run = if paired {
                runs.start_paired(Arc::clone(&slot), paired_of(&slot))
            } else {
                runs.start_one_sample(Arc::clone(&slot), one_sample_of(&slot))
            }
            .unwrap();
            let snapshot = |r| {
                if paired {
                    runs.paired_snapshot(r)
                } else {
                    runs.one_sample_snapshot(r)
                }
            };
            wait_for("the run ended", || {
                snapshot(run).unwrap().record.state.is_terminal()
            });
            let s = snapshot(run).unwrap();
            assert_eq!(s.record.state, RunState::Succeeded, "paired {paired}");
            assert_eq!((s.columns_done, s.columns_total), (0, 0), "paired {paired}");
            let points: Vec<_> = seen.lock().unwrap().iter().map(|(p, _)| *p).collect();
            assert_eq!(
                points,
                [Point::BeforeStart, Point::Column(0), Point::Passed(0)],
                "paired {paired}"
            );
            assert!(seen.lock().unwrap().iter().all(|(_, n)| *n == thread));
        }
    }

    /// A cancel that lands after the one check point wins, for each new
    /// kind, and leaves no column count and no result.
    #[test]
    fn a_one_sample_or_paired_cancel_after_its_check_point_wins_and_counts_no_columns() {
        for paired in [false, true] {
            let (reached, at_passed) = mpsc::channel::<()>();
            let (go, gate) = mpsc::channel::<()>();
            let gate = Mutex::new(gate);
            let hook: CheckpointHook = Arc::new(move |_run, p| {
                if p == Point::Passed(0) {
                    reached.send(()).unwrap();
                    let _ = gate.lock().unwrap().recv();
                }
            });
            let runs = Arc::new(Runs::with_hooks(
                Arc::new(SinceStart(Instant::now())),
                Some(hook),
                RUN_CEILING_MS,
            ));
            let slot = two_columns();
            let run = if paired {
                runs.start_paired(Arc::clone(&slot), paired_of(&slot))
            } else {
                runs.start_one_sample(Arc::clone(&slot), one_sample_of(&slot))
            }
            .unwrap();
            at_passed
                .recv_timeout(Duration::from_secs(10))
                .expect("the run reached its check point");
            if paired {
                runs.paired_cancel(run).unwrap();
            } else {
                runs.one_sample_cancel(run).unwrap();
            }
            go.send(()).unwrap();
            let snapshot = |r| {
                if paired {
                    runs.paired_snapshot(r)
                } else {
                    runs.one_sample_snapshot(r)
                }
            };
            wait_for("the run ended", || {
                snapshot(run).unwrap().record.state.is_terminal()
            });
            let s = snapshot(run).unwrap();
            assert_eq!(s.record.state, RunState::Cancelled, "paired {paired}");
            assert_eq!((s.columns_done, s.columns_total), (0, 0), "paired {paired}");
            let result = if paired {
                runs.paired_result(run).map(|_| ())
            } else {
                runs.one_sample_result(run).map(|_| ())
            };
            assert_eq!(result, Err(RunCode::Cancelled), "paired {paired}");
        }
    }

    /// Every kind's snapshot, result and cancel, by the kind's index: 0 the
    /// summary, 1 the t-test, 2 the one-sample t-test, 3 the paired t-test.
    /// Each answers `Ok(())` or the run manager's code.
    fn read_as(runs: &Runs, kind: usize, run: RunId) -> [Result<(), RunCode>; 3] {
        match kind {
            0 => [
                runs.snapshot(run).map(drop),
                runs.result(run).map(drop),
                runs.cancel(run).map(drop),
            ],
            1 => [
                runs.ttest_snapshot(run).map(drop),
                runs.ttest_result(run).map(drop),
                runs.ttest_cancel(run).map(drop),
            ],
            2 => [
                runs.one_sample_snapshot(run).map(drop),
                runs.one_sample_result(run).map(drop),
                runs.one_sample_cancel(run).map(drop),
            ],
            _ => [
                runs.paired_snapshot(run).map(drop),
                runs.paired_result(run).map(drop),
                runs.paired_cancel(run).map(drop),
            ],
        }
    }

    /// `run.wrong_kind` between every pair of the four kinds, both ways, for
    /// status, result and cancel; each kind reads its own run.
    #[test]
    fn every_kind_refuses_every_other_kinds_run_as_wrong_kind() {
        let runs = Arc::new(Runs::default());
        let groups = two_groups();
        let pairs = two_columns();
        let mut ids = Vec::new();
        for kind in 0..4 {
            let run = match kind {
                0 => runs.start(Arc::clone(&groups), summary_of_both(&groups)),
                1 => runs.start_ttest(Arc::clone(&groups), ttest_of(&groups)),
                2 => runs.start_one_sample(Arc::clone(&pairs), one_sample_of(&pairs)),
                _ => runs.start_paired(Arc::clone(&pairs), paired_of(&pairs)),
            }
            .unwrap();
            wait_for("the run ended", || {
                runs.copy_for_save(run).unwrap().state.is_terminal()
            });
            assert_eq!(
                runs.copy_for_save(run).unwrap().state,
                RunState::Succeeded,
                "kind {kind}"
            );
            ids.push(run);
        }
        for (owner, run) in ids.iter().enumerate() {
            for reader in 0..4 {
                let got = read_as(&runs, reader, *run);
                if reader == owner {
                    assert_eq!(
                        got,
                        [Ok(()), Ok(()), Ok(())],
                        "kind {owner} reads its own run"
                    );
                } else {
                    assert_eq!(
                        got,
                        [Err(RunCode::WrongKind); 3],
                        "kind {reader} reads kind {owner}'s run"
                    );
                }
            }
        }
    }

    /// One foreground run across all four kinds: while a run of any kind is
    /// parked before its start, a start of every kind is refused as busy.
    #[test]
    fn one_run_at_a_time_across_all_four_kinds() {
        let groups = two_groups();
        let pairs = two_columns();
        let start = |runs: &Arc<Runs>, kind: usize| match kind {
            0 => runs.start(Arc::clone(&groups), summary_of_both(&groups)),
            1 => runs.start_ttest(Arc::clone(&groups), ttest_of(&groups)),
            2 => runs.start_one_sample(Arc::clone(&pairs), one_sample_of(&pairs)),
            _ => runs.start_paired(Arc::clone(&pairs), paired_of(&pairs)),
        };
        for parked in 0..4 {
            let (runs, at_start, go) = parking();
            let run = start(&runs, parked).unwrap();
            at_start
                .recv_timeout(Duration::from_secs(10))
                .expect("the run reached its start");
            for other in 0..4 {
                assert_eq!(
                    start(&runs, other).map(|_| ()),
                    Err(NotStarted::Busy),
                    "kind {other} while kind {parked} is parked"
                );
            }
            go.send(()).unwrap();
            wait_for("the parked run ended", || {
                runs.copy_for_save(run).unwrap().state.is_terminal()
            });
            assert_eq!(
                runs.copy_for_save(run).unwrap().state,
                RunState::Succeeded,
                "kind {parked}"
            );
        }
    }
}
