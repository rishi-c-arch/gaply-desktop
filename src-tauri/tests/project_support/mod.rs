//! Shared by the project command tests (Research OS, Phase 5B-2): a
//! temporary folder the test makes and removes, a window's workspace session
//! with a project session of small caps and a clock the test moves, data
//! files written in the test, runs started through the real commands and
//! waited on, and readers for a command's refusal and the files under a root.
//! The commands are called as plain functions over a root and the session,
//! as their Tauri wrappers call them; no `AppHandle` is needed.
#![allow(dead_code)]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

use serde_json::Value;
use sha2::{Digest, Sha256};

use app_lib::project_commands::ProjectSession;
use app_lib::runs::{CheckpointHook, Clock, Point, Runs};
use app_lib::stats_desc_commands as desc;
use app_lib::stats_onesample_commands as onesample;
use app_lib::stats_ttest_commands as ttest;
use app_lib::workspace::{readiness, RunsFactory, StatisticsEngine, WorkspaceSession};
use gaply_core::contracts::{OperationId, RunId, RunState, WorkspaceId};
use gaply_core::research_project::Caps;
use gaply_core::stats_desc::{self, Alias, DescribeParams};
use gaply_core::stats_l1::dataset::ColumnAlias;
use gaply_core::stats_onesample::{self, OneSampleParams, PairedParams};
use gaply_core::stats_ttest::{self, TTestParams, Variant};
use gaply_core::GaplyError;

/* ---- folders ---------------------------------------------------------------- */

static DIRS: AtomicUsize = AtomicUsize::new(0);

/// A folder under the OS temp folder, removed when dropped. Projects go in
/// its `root/`; data files in its `data/`.
pub struct TempDir(pub PathBuf);

impl TempDir {
    pub fn new(tag: &str) -> TempDir {
        let n = DIRS.fetch_add(1, Ordering::SeqCst);
        let p = std::env::temp_dir().join(format!("gaply-5b2-{}-{n}-{tag}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(p.join("data")).expect("temp dir");
        TempDir(p)
    }
    pub fn root(&self) -> PathBuf {
        self.0.join("root")
    }
    pub fn write(&self, name: &str, bytes: &[u8]) -> PathBuf {
        let p = self.0.join("data").join(name);
        std::fs::write(&p, bytes).expect("write fixture");
        p
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        // The store's artifacts are read-only; the folders are the owner's.
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/* ---- the clock -------------------------------------------------------------- */

/// A clock the test moves.
#[derive(Default)]
pub struct TestClock(pub AtomicU64);

impl TestClock {
    pub fn advance(&self, ms: u64) {
        self.0.fetch_add(ms, Ordering::SeqCst);
    }
}

impl Clock for TestClock {
    fn now_ms(&self) -> u64 {
        self.0.load(Ordering::SeqCst)
    }
}

/* ---- the session ------------------------------------------------------------ */

/// Small caps, so a test can pass them with a few bytes.
pub const SMALL: Caps = Caps {
    project: 10 * 1024 * 1024,
    all_projects: 40 * 1024 * 1024,
    db_allowance: 64 * 1024,
};

pub struct World {
    pub dir: TempDir,
    pub ws: Arc<WorkspaceSession>,
    pub clock: Arc<TestClock>,
}

impl World {
    /// A session with `caps`, Statistics active, and `runs` as its run
    /// manager (a fresh default one when `None`).
    pub fn with(tag: &str, caps: Caps, runs: Option<Arc<Runs>>) -> World {
        let w = World::inactive(tag, caps, runs);
        w.ws.activate(WorkspaceId::Statistics)
            .expect("Statistics activates");
        w
    }

    pub fn new(tag: &str) -> World {
        World::with(tag, SMALL, None)
    }

    /// The same, with no workspace active.
    pub fn inactive(tag: &str, caps: Caps, runs: Option<Arc<Runs>>) -> World {
        let clock = Arc::new(TestClock::default());
        let projects = Arc::new(ProjectSession::with(caps, clock.clone()));
        let factory: RunsFactory =
            Arc::new(move || runs.clone().unwrap_or_else(|| Arc::new(Runs::default())));
        let ws = Arc::new(WorkspaceSession::with_projects(
            Arc::new(readiness),
            factory,
            projects,
        ));
        World {
            dir: TempDir::new(tag),
            ws,
            clock,
        }
    }

    pub fn root(&self) -> PathBuf {
        self.dir.root()
    }

    pub fn engine(&self) -> Arc<StatisticsEngine> {
        self.ws.require_statistics().expect("Statistics is active")
    }

    /// Opens a data file through `stats_desc_open`'s body; its slot.
    pub fn open(&self, name: &str, bytes: &[u8]) -> u64 {
        let p = self.dir.write(name, bytes);
        desc::open(&self.engine().datasets, &p)
            .unwrap_or_else(|e| panic!("open {name}: {e}"))
            .slot
    }

    /// A descriptive summary of every column of `slot`, run to its end.
    pub fn describe(&self, slot: u64) -> u64 {
        let id = self.start_describe(slot);
        self.wait(id);
        id
    }

    pub fn start_describe(&self, slot: u64) -> u64 {
        let e = self.engine();
        let held = e.datasets.get(slot).expect("the slot");
        let profile = stats_desc::profile(held.dataset());
        let spec = stats_desc::spec(
            OperationId(1),
            held.dataset(),
            DescribeParams::defaults_for(&profile),
        );
        desc::run(&e.runs, &e.datasets, slot, spec)
            .expect("the run starts")
            .run_id
    }

    /// A Welch t-test of column 1 by column 2 of `slot`, run to its end.
    pub fn ttest(&self, slot: u64) -> u64 {
        let e = self.engine();
        let held = e.datasets.get(slot).expect("the slot");
        let spec = stats_ttest::spec(
            OperationId(2),
            held.dataset(),
            TTestParams {
                outcome: Alias(ColumnAlias(1)),
                group: Alias(ColumnAlias(2)),
                levels: None,
                variant: Some(Variant::Welch),
                missing_codes: Vec::new(),
            },
        );
        let id = ttest::run(&e.runs, &e.datasets, slot, spec)
            .expect("the run starts")
            .run_id;
        self.wait(id);
        id
    }

    /// A one-sample t-test of column `column` of `slot` against
    /// `test_value`, run to its end (Research OS, Phase 8D-2a).
    pub fn one_sample(&self, slot: u64, column: u32, test_value: f64) -> u64 {
        let e = self.engine();
        let held = e.datasets.get(slot).expect("the slot");
        let spec = stats_onesample::one_sample_spec(
            OperationId(3),
            held.dataset(),
            OneSampleParams {
                outcome: Alias(ColumnAlias(column)),
                test_value: Some(test_value),
                missing_codes: Vec::new(),
            },
        );
        let id = onesample::run_one_sample(&e.runs, &e.datasets, slot, spec)
            .expect("the run starts")
            .run_id;
        self.wait_new_kind(id);
        id
    }

    /// A paired t-test of columns `first` and `second` of `slot`, run to its
    /// end (Research OS, Phase 8D-2a).
    pub fn paired(&self, slot: u64, first: u32, second: u32) -> u64 {
        let e = self.engine();
        let held = e.datasets.get(slot).expect("the slot");
        let spec = stats_onesample::paired_spec(
            OperationId(4),
            held.dataset(),
            PairedParams {
                first: Alias(ColumnAlias(first)),
                second: Alias(ColumnAlias(second)),
                missing_codes: Vec::new(),
            },
        );
        let id = onesample::run_paired(&e.runs, &e.datasets, slot, spec)
            .expect("the run starts")
            .run_id;
        self.wait_new_kind(id);
        id
    }

    /// Waits until a one-sample or paired run has ended; its state.
    pub fn wait_new_kind(&self, run: u64) -> RunState {
        let e = self.engine();
        let start = Instant::now();
        loop {
            let state = e
                .runs
                .one_sample_snapshot(RunId(run))
                .map(|s| s.record.state)
                .or_else(|_| e.runs.paired_snapshot(RunId(run)).map(|s| s.record.state))
                .expect("the run is held");
            if state.is_terminal() {
                return state;
            }
            assert!(
                start.elapsed() < Duration::from_secs(20),
                "the run never ended"
            );
            std::thread::sleep(Duration::from_millis(2));
        }
    }

    /// Waits until the run has ended; its state.
    pub fn wait(&self, run: u64) -> RunState {
        let e = self.engine();
        let start = Instant::now();
        loop {
            let state = e
                .runs
                .snapshot(RunId(run))
                .map(|s| s.record.state)
                .or_else(|_| e.runs.ttest_snapshot(RunId(run)).map(|s| s.record.state))
                .expect("the run is held");
            if state.is_terminal() {
                return state;
            }
            assert!(
                start.elapsed() < Duration::from_secs(20),
                "the run never ended"
            );
            std::thread::sleep(Duration::from_millis(2));
        }
    }
}

/* ---- data ------------------------------------------------------------------- */

/// Two groups that differ: a t-test succeeds.
pub const CSV: &[u8] = b"score,group\n1.5,a\n2.5,a\n3.0,a\n3.5,b\n4.0,b\n5.5,b\n";

/// Each group constant: a t-test runs and fails with
/// `stats.ttest.both_groups_constant`.
pub const CONSTANT: &[u8] = b"score,group\n1,a\n1,a\n2,b\n2,b\n";

/* ---- a parked run ----------------------------------------------------------- */

/// A gate a run's check point waits at, at one point, until it is opened.
pub struct Gate {
    open: Mutex<bool>,
    turn: Condvar,
    pub reached: Mutex<bool>,
    pub reached_turn: Condvar,
}

impl Gate {
    pub fn new() -> Arc<Gate> {
        Arc::new(Gate {
            open: Mutex::new(false),
            turn: Condvar::new(),
            reached: Mutex::new(false),
            reached_turn: Condvar::new(),
        })
    }
    pub fn hook(self: &Arc<Gate>, at: Point) -> CheckpointHook {
        let g = Arc::clone(self);
        Arc::new(move |_run, p| {
            if p == at {
                *g.reached.lock().unwrap() = true;
                g.reached_turn.notify_all();
                let mut open = g.open.lock().unwrap();
                while !*open {
                    open = g.turn.wait(open).unwrap();
                }
            }
        })
    }
    pub fn wait_reached(&self) {
        let mut r = self.reached.lock().unwrap();
        while !*r {
            r = self.reached_turn.wait(r).unwrap();
        }
    }
    pub fn release(&self) {
        *self.open.lock().unwrap() = true;
        self.turn.notify_all();
    }
}

/// A run manager whose runs wait at `at` until the gate opens.
pub fn gated_runs(gate: &Arc<Gate>, at: Point) -> Arc<Runs> {
    #[derive(Default)]
    struct Real(AtomicU64);
    impl Clock for Real {
        fn now_ms(&self) -> u64 {
            self.0.fetch_add(1, Ordering::SeqCst)
        }
    }
    Arc::new(Runs::with_hooks(
        Arc::new(Real::default()),
        Some(gate.hook(at)),
        app_lib::runs::RUN_CEILING_MS,
    ))
}

/* ---- reading results -------------------------------------------------------- */

/// A command's refusal as the webview reads it.
pub fn refusal(e: &GaplyError) -> Value {
    match e {
        GaplyError::Validation(s) => serde_json::from_str(s).expect("a refusal is JSON"),
        other => panic!("not a refusal: {other}"),
    }
}

/// The refusal's code.
pub fn code(e: &GaplyError) -> String {
    refusal(e)["code"]
        .as_str()
        .expect("a refusal has a code")
        .to_string()
}

/// Every file under `dir`, by its path relative to it, with its size and
/// SHA-256. A SQLite `-shm` file is listed by name only: a read may touch it.
pub fn files(dir: &Path) -> BTreeMap<String, (u64, String)> {
    let mut out = BTreeMap::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&d) else {
            continue;
        };
        for e in rd.flatten() {
            let p = e.path();
            let ft = e.file_type().unwrap();
            if ft.is_dir() {
                stack.push(p);
                continue;
            }
            let rel = p.strip_prefix(dir).unwrap().to_string_lossy().into_owned();
            if rel.ends_with("-shm") {
                out.insert(rel, (0, String::new()));
                continue;
            }
            let bytes = std::fs::read(&p).unwrap_or_default();
            let sha: String = Sha256::digest(&bytes)
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect();
            out.insert(rel, (bytes.len() as u64, sha));
        }
    }
    out
}

/// Every name under `dir`, folders included.
pub fn names(dir: &Path) -> Vec<String> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&d) else {
            continue;
        };
        for e in rd.flatten() {
            out.push(e.file_name().to_string_lossy().into_owned());
            if e.file_type().unwrap().is_dir() {
                stack.push(e.path());
            }
        }
    }
    out.sort();
    out
}
