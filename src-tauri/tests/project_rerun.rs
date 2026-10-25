//! **Reruns through the commands (Research OS, Phase 5C-1a; decisions 185 to
//! 190).** Every test works in a temporary folder given as the root, with
//! small caps, and calls the commands' plain functions with a window's
//! workspace session, as the Tauri wrappers do.
//!
//! * Each of `project_rerun`'s refusals, in its order, leaves the project's
//!   files and rows as they were; the one write a refusal may make is the
//!   damage mark that reading a damaged artifact makes, and it is named.
//! * While a rerun is going, the actions of decision 190 are refused, and
//!   once its save at the end has returned they are allowed. Leaving
//!   Statistics is refused after the rerun's run has ended and before its
//!   save at the end has returned, which no run-manager check can see: a
//!   second connection holds `project.db`'s write lock, as a second copy of
//!   the app saving would, so the save waits there.
//! * The ends: a rerun that Succeeds is saved beside the run it repeats, on
//!   the same data file artifact, with an output hash equal to the original's
//!   (decision 15); a quit mid-run, a cancel and a refusal write nothing; a
//!   rerun that Fails with its code is saved with its failure record. A
//!   rerun's own run is never saved by hand, and its dataset evicts no open
//!   one.
//!
//! The store's fault points (a crash before the commit of the save at the
//! end, a full disk) can be armed only inside the app crate, so those two
//! tests are in `src/project_store.rs`'s own tests.

mod project_support;

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::time::{Duration, Instant};

use rusqlite::{Connection, OpenFlags};
use serde_json::Value;

use app_lib::project_commands::{self as pc, RerunProgress, RerunView, SaveTarget};
use app_lib::project_store::{ArtifactInput, ProjectRoot, RunToSave, SaveRequest};
use app_lib::runs::{CheckpointHook, Clock, Point, RunKind, Runs, RUN_CEILING_MS};
use app_lib::stats_desc_commands as desc;
use app_lib::workspace::{readiness, RunsFactory, WorkspaceSession};
use gaply_core::contracts::codes::Closed;
use gaply_core::contracts::{
    CapabilityId, ContentHash, OperationId, OperationSpec, ParamKey, ParamValue, RunId, RunState,
    WorkspaceId,
};
use gaply_core::research_project::{ArtifactHash, ArtifactKind, DataFormat, UsedRole};
use gaply_core::stats_desc::{Alias, DescribeParams};
use gaply_core::stats_l1::dataset::ColumnAlias;
use gaply_core::stats_ttest::{self, TTestParams, Variant};

use project_support::*;

fn new_project(name: &str) -> SaveTarget {
    SaveTarget::NewProject {
        name: name.to_string(),
    }
}

fn into_open(id: &str) -> SaveTarget {
    SaveTarget::OpenProject {
        project_id: id.to_string(),
    }
}

/// A world whose project, open, holds one saved summary of `CSV`: (project
/// id, the in-memory run that was saved).
fn with_a_summary(w: &World) -> (String, u64) {
    let slot = w.open("a.csv", CSV);
    let run = w.describe(slot);
    let id = pc::save_run(&w.root(), &w.ws, run, new_project("Reruns"))
        .expect("saved")
        .project_id;
    (id, run)
}

fn project_dir(w: &World, id: &str) -> PathBuf {
    w.root().join("projects").join(id)
}

/// The project as it stands on disk: every file by its path, with its size
/// and SHA-256 (SQLite's `-wal` and `-shm` left out, which come and go with
/// connections), then each table's row count, then the number of artifact
/// rows marked damaged.
fn state_of(w: &World, id: &str) -> (BTreeMap<String, (u64, String)>, Vec<i64>) {
    let dir = project_dir(w, id);
    let mut f = files(&dir);
    f.retain(|k, _| !k.ends_with("-wal") && !k.ends_with("-shm"));
    let c = Connection::open_with_flags(dir.join("project.db"), OpenFlags::SQLITE_OPEN_READ_ONLY)
        .expect("project.db");
    let mut counts: Vec<i64> = [
        "project",
        "artifacts",
        "runs",
        "used",
        "generated",
        "derived",
    ]
    .iter()
    .map(|t| {
        c.query_row(&format!("SELECT COUNT(*) FROM {t}"), [], |r| r.get(0))
            .unwrap()
    })
    .collect();
    counts.push(
        c.query_row(
            "SELECT COUNT(*) FROM artifacts WHERE damage IS NOT NULL",
            [],
            |r| r.get(0),
        )
        .unwrap(),
    );
    (f, counts)
}

/// One column of `project.db`, for each row a query gives.
fn column(w: &World, id: &str, sql: &str) -> Vec<String> {
    let c = Connection::open_with_flags(
        project_dir(w, id).join("project.db"),
        OpenFlags::SQLITE_OPEN_READ_ONLY,
    )
    .unwrap();
    let mut st = c.prepare(sql).unwrap();
    let rows = st
        .query_map([], |r| r.get::<_, String>(0))
        .unwrap()
        .map(|r| r.unwrap())
        .collect();
    rows
}

/// The rerun as `project_current` shows it, once it is no longer going.
fn settled(w: &World) -> RerunView {
    let start = Instant::now();
    loop {
        if let Some(r) = pc::current(&w.ws).unwrap().rerun {
            if r.status != RerunProgress::Going {
                return r;
            }
        }
        assert!(
            start.elapsed() < Duration::from_secs(20),
            "the rerun never settled"
        );
        std::thread::sleep(Duration::from_millis(2));
    }
}

fn hash(bytes: &[u8]) -> ArtifactHash {
    ArtifactHash::parse(ContentHash::of_bytes(bytes).as_hex()).unwrap()
}

fn json_art(kind: ArtifactKind, bytes: &[u8]) -> ArtifactInput {
    ArtifactInput {
        kind,
        format: DataFormat::Json,
        schema_version: 1,
        expected: hash(bytes),
        bytes: bytes.to_vec(),
    }
}

/// Saves a run row through the store, on a second handle, as a test-built
/// stored run: `state`, its capability and version, and its data file, spec,
/// bundle and manifest. Its stored id.
fn store_a_row(
    w: &World,
    id: &str,
    capability: (&str, u32),
    state: RunState,
    data: (&[u8], DataFormat),
    spec: &[u8],
) -> u64 {
    let store = ProjectRoot::open_root_with(&w.root(), SMALL).unwrap();
    let mut h = store.open(id).unwrap();
    let (bundle, manifest) = (b"{\"test\":\"bundle\"}", b"{\"test\":\"manifest\"}");
    let req = SaveRequest {
        run: RunToSave {
            capability_id: CapabilityId::parse(capability.0).unwrap(),
            capability_version: capability.1,
            state,
            started_at: 1_792_396_340,
            ended_at: 1_792_396_341,
            used: vec![(hash(data.0), UsedRole::Data), (hash(spec), UsedRole::Spec)],
            generated: vec![hash(bundle), hash(manifest)],
            derived: Vec::new(),
        },
        artifacts: vec![
            ArtifactInput {
                kind: ArtifactKind::DataFile,
                format: data.1,
                schema_version: 1,
                expected: hash(data.0),
                bytes: data.0.to_vec(),
            },
            json_art(ArtifactKind::OperationSpec, spec),
            json_art(ArtifactKind::ResultBundle, bundle),
            json_art(ArtifactKind::RunManifest, manifest),
        ],
    };
    h.save(req).expect("a test row").0
}

/// Saves a rerun row of stored run `of` through the store, on a second
/// handle, with the data file and spec of run `of` and `manifest` as its
/// manifest: how a test makes a rerun whose result differs. Its stored id.
fn store_a_rerun_row(w: &World, id: &str, of: u64, manifest: &[u8]) -> u64 {
    let original = pc::run_result(&w.root(), &w.ws, id, of).unwrap();
    let spec = original.spec_json.into_bytes();
    let bundle = b"{\"test\":\"another bundle\"}";
    let store = ProjectRoot::open_root_with(&w.root(), SMALL).unwrap();
    let mut h = store.open(id).unwrap();
    let data = ArtifactHash::parse(&original.data_file.sha256).unwrap();
    let req = SaveRequest {
        run: RunToSave {
            capability_id: CapabilityId::parse(&original.capability_id).unwrap(),
            capability_version: 1,
            state: RunState::Succeeded,
            started_at: 1_792_396_340,
            ended_at: 1_792_396_341,
            used: vec![(data, UsedRole::Data), (hash(&spec), UsedRole::Spec)],
            generated: vec![hash(bundle), hash(manifest)],
            derived: Vec::new(),
        },
        artifacts: vec![
            json_art(ArtifactKind::OperationSpec, &spec),
            json_art(ArtifactKind::ResultBundle, bundle),
            json_art(ArtifactKind::RunManifest, manifest),
        ],
    };
    h.save_rerun(req, gaply_core::research_project::StoredRunId(of))
        .expect("a test rerun row")
        .0
}

/* ---- a run manager that parks a run when told -------------------------------- */

#[derive(Default)]
struct Ticks(AtomicU64);

impl Clock for Ticks {
    fn now_ms(&self) -> u64 {
        self.0.fetch_add(1, Ordering::SeqCst)
    }
}

/// Runs that stop at `BeforeStart` while `armed` is set, until `go` sends.
struct Parking {
    runs: Arc<Runs>,
    armed: Arc<AtomicBool>,
    reached: mpsc::Receiver<()>,
    go: mpsc::Sender<()>,
}

fn parking() -> Parking {
    let armed = Arc::new(AtomicBool::new(false));
    let (reached_tx, reached) = mpsc::channel::<()>();
    let (go, gate) = mpsc::channel::<()>();
    let gate = Mutex::new(gate);
    let a = Arc::clone(&armed);
    let hook: CheckpointHook = Arc::new(move |_run, p| {
        if p == Point::BeforeStart && a.load(Ordering::SeqCst) {
            reached_tx.send(()).unwrap();
            let _ = gate.lock().unwrap().recv();
        }
    });
    Parking {
        runs: Arc::new(Runs::with_hooks(
            Arc::new(Ticks::default()),
            Some(hook),
            RUN_CEILING_MS,
        )),
        armed,
        reached,
        go,
    }
}

/* ---- the refusals (decisions 185, 188 and 190) ------------------------------- */

#[test]
fn each_refusal_leaves_the_project_as_it_was_but_for_a_damage_mark() {
    // 1. Statistics is not active.
    {
        let w = World::new("rr-inactive");
        let (id, _) = with_a_summary(&w);
        w.ws.deactivate().unwrap();
        let before = state_of(&w, &id);
        let e = pc::rerun(&w.root(), &w.ws, 1).unwrap_err();
        assert_eq!(code(&e), "workspace.inactive");
        assert_eq!(state_of(&w, &id), before);
    }
    // 2. No project is open in this copy.
    {
        let w = World::new("rr-not-open");
        let (id, _) = with_a_summary(&w);
        pc::close(&w.root(), &w.ws).unwrap();
        let before = state_of(&w, &id);
        let e = pc::rerun(&w.root(), &w.ws, 1).unwrap_err();
        assert_eq!(code(&e), "project.not_open");
        assert_eq!(state_of(&w, &id), before);
    }
    // 3, 4, 5, 8, 9 and the data, in one project.
    let w = World::new("rr-refusals");
    let (id, _) = with_a_summary(&w);
    let spec_json = pc::run_result(&w.root(), &w.ws, &id, 1).unwrap().spec_json;
    let spec: OperationSpec<DescribeParams> = serde_json::from_str(&spec_json).unwrap();
    let canonical = serde_json::to_vec(&spec).unwrap();
    assert_eq!(
        canonical,
        spec_json.as_bytes(),
        "a Save stores canonical bytes"
    );

    let constant = w.open("constant.csv", CONSTANT);
    let failed = w.ttest(constant);
    assert_eq!(w.wait(failed), RunState::Failed);
    let failed_row = pc::save_run(&w.root(), &w.ws, failed, into_open(&id))
        .unwrap()
        .stored_run_id;
    let other_version = store_a_row(
        &w,
        &id,
        ("stats.describe", 2),
        RunState::Succeeded,
        (CSV, DataFormat::Csv),
        &canonical,
    );
    let unknown = store_a_row(
        &w,
        &id,
        ("stats.unknown_method", 1),
        RunState::Succeeded,
        (CSV, DataFormat::Csv),
        &canonical,
    );
    let mut spaced = b"{ ".to_vec();
    spaced.extend_from_slice(&canonical[1..]);
    let not_canonical = store_a_row(
        &w,
        &id,
        ("stats.describe", 1),
        RunState::Succeeded,
        (CSV, DataFormat::Csv),
        &spaced,
    );
    let mut no_column = spec.clone();
    no_column.parameters.columns[0].alias = Alias(ColumnAlias(9));
    let refused = store_a_row(
        &w,
        &id,
        ("stats.describe", 1),
        RunState::Succeeded,
        (CSV, DataFormat::Csv),
        &serde_json::to_vec(&no_column).unwrap(),
    );
    let unreadable = store_a_row(
        &w,
        &id,
        ("stats.describe", 1),
        RunState::Succeeded,
        (b"not a workbook", DataFormat::Xlsx),
        &canonical,
    );
    for (stored, want) in [
        (99, "project.stored_run_unknown"),
        (failed_row, "project.rerun_not_succeeded"),
        (other_version, "project.rerun_unsupported"),
        (unknown, "project.rerun_unsupported"),
        (not_canonical, "project.rerun_unsupported"),
        (unreadable, "project.rerun_data_unreadable"),
        (refused, "project.rerun_refused"),
    ] {
        let before = state_of(&w, &id);
        let e = pc::rerun(&w.root(), &w.ws, stored).unwrap_err();
        assert_eq!(code(&e), want, "stored run {stored}");
        assert_eq!(state_of(&w, &id), before, "{want}: nothing was written");
        assert_eq!(
            w.ws.projects().rerun_going(),
            None,
            "{want}: nothing is going"
        );
    }

    // 7. An artifact that does not read back: its damage mark is the one
    // write, and the refusal is its own code.
    let data = pc::run_result(&w.root(), &w.ws, &id, 1)
        .unwrap()
        .data_file
        .sha256;
    std::fs::remove_file(project_dir(&w, &id).join("artifacts").join(&data)).unwrap();
    let (files_before, mut counts_before) = state_of(&w, &id);
    let e = pc::rerun(&w.root(), &w.ws, 1).unwrap_err();
    assert_eq!(code(&e), "project.artifact_missing");
    let (files_after, counts_after) = state_of(&w, &id);
    assert_eq!(files_after, files_before, "no file was written");
    *counts_before.last_mut().unwrap() += 1;
    assert_eq!(
        counts_after, counts_before,
        "every row as it was but one damage mark"
    );
    assert_eq!(
        column(
            &w,
            &id,
            &format!("SELECT damage FROM artifacts WHERE hash = '{data}'")
        ),
        ["project.artifact_missing"]
    );
    assert_eq!(pc::current(&w.ws).unwrap().rerun.map(|r| r.run_id), None);
}

#[test]
fn another_run_going_refuses_a_rerun_and_writes_nothing() {
    let p = parking();
    let w = World::with("rr-busy", SMALL, Some(Arc::clone(&p.runs)));
    let (id, _) = with_a_summary(&w);
    let slot = w.open("b.csv", CSV);
    p.armed.store(true, Ordering::SeqCst);
    let parked = w.start_describe(slot);
    p.reached.recv().unwrap();
    let before = state_of(&w, &id);
    // 6. Another run is going.
    let e = pc::rerun(&w.root(), &w.ws, 1).unwrap_err();
    assert_eq!(code(&e), "project.rerun_busy");
    assert_eq!(state_of(&w, &id), before);
    assert_eq!(w.ws.projects().rerun_going(), None);
    p.armed.store(false, Ordering::SeqCst);
    p.go.send(()).unwrap();
    w.wait(parked);
}

/* ---- going (decision 190) ---------------------------------------------------- */

#[test]
fn while_a_rerun_is_going_decision_190s_actions_are_refused_then_allowed() {
    let p = parking();
    let w = World::with("rr-going", SMALL, Some(Arc::clone(&p.runs)));
    let (a, run) = with_a_summary(&w);
    // A second project, B, and A open again.
    let slot = w.open("b.csv", CSV);
    let b_run = w.describe(slot);
    let b = pc::save_run(&w.root(), &w.ws, b_run, new_project("B"))
        .unwrap()
        .project_id;
    pc::open(&w.root(), &w.ws, &a).unwrap();
    let spare = w.describe(slot);

    p.armed.store(true, Ordering::SeqCst);
    let started = pc::rerun(&w.root(), &w.ws, 1).expect("the rerun starts");
    assert_eq!(started.kind, RunKind::Describe);
    p.reached.recv().unwrap();
    p.armed.store(false, Ordering::SeqCst);
    assert_eq!(w.ws.projects().rerun_going(), Some(started.run_id));
    let now = pc::current(&w.ws).unwrap();
    assert_eq!(now.project_id.as_deref(), Some(a.as_str()));
    let going = now.rerun.expect("the rerun is shown");
    assert_eq!(
        (going.run_id, going.rerun_of, going.status),
        (started.run_id, 1, RerunProgress::Going)
    );

    let r = w.root();
    let in_progress = |e: gaply_core::GaplyError| code(&e);
    assert_eq!(
        in_progress(pc::close(&r, &w.ws).unwrap_err()),
        "project.rerun_in_progress"
    );
    assert_eq!(
        in_progress(pc::open(&r, &w.ws, &b).unwrap_err()),
        "project.rerun_in_progress"
    );
    assert_eq!(
        in_progress(pc::save_run(&r, &w.ws, spare, new_project("C")).unwrap_err()),
        "project.rerun_in_progress"
    );
    let c = pc::prepare_delete(&r, &w.ws, &a).unwrap();
    assert_eq!(
        in_progress(pc::delete(&r, &w.ws, &a, Some(&c.confirmation)).unwrap_err()),
        "project.rerun_in_progress"
    );
    assert_eq!(
        in_progress(pc::rerun(&r, &w.ws, 1).unwrap_err()),
        "project.rerun_in_progress"
    );
    let e = w.ws.deactivate().unwrap_err();
    assert_eq!(e.code.wire(), "workspace.run_active");
    assert_eq!(
        pc::current(&w.ws).unwrap().project_id.as_deref(),
        Some(a.as_str()),
        "A is still open"
    );
    // Allowed meanwhile: a Save into the open project, and another
    // project's delete.
    pc::save_run(&r, &w.ws, spare, into_open(&a)).expect("a Save into A");
    let cb = pc::prepare_delete(&r, &w.ws, &b).unwrap();
    pc::delete(&r, &w.ws, &b, Some(&cb.confirmation)).expect("B is not the open project");

    p.go.send(()).unwrap();
    let done = settled(&w);
    assert_eq!(done.status, RerunProgress::Saved);
    assert_eq!(w.ws.projects().rerun_going(), None);
    // Once its save at the end has returned, each is allowed.
    pc::rerun(&r, &w.ws, 1).expect("a second rerun");
    settled(&w);
    pc::save_run(&r, &w.ws, run, new_project("C")).expect("a Save into a new project");
    pc::open(&r, &w.ws, &a).expect("open another project");
    pc::close(&r, &w.ws).expect("close");
    pc::open(&r, &w.ws, &a).unwrap();
    let c = pc::prepare_delete(&r, &w.ws, &a).unwrap();
    pc::delete(&r, &w.ws, &a, Some(&c.confirmation)).expect("delete the open project");
    w.ws.deactivate().expect("leave Statistics");
}

#[test]
fn a_delete_refused_for_a_going_rerun_leaves_its_confirmation_redeemable() {
    // F8 (decision 226): project_delete checks the rerun-going refusal before
    // it redeems the confirmation, so a delete refused that way does not use
    // the confirmation up — the same one deletes once the rerun has ended.
    let p = parking();
    let w = World::with("f8-confirm", SMALL, Some(Arc::clone(&p.runs)));
    let (a, _run) = with_a_summary(&w);
    let r = w.root();
    // A confirmation for A, issued before the rerun starts.
    let c = pc::prepare_delete(&r, &w.ws, &a).unwrap().confirmation;
    // Start a rerun and hold it going.
    p.armed.store(true, Ordering::SeqCst);
    pc::rerun(&r, &w.ws, 1).expect("the rerun starts");
    p.reached.recv().unwrap();
    p.armed.store(false, Ordering::SeqCst);
    assert!(w.ws.projects().rerun_going().is_some());
    // The delete is refused for the going rerun, and does NOT redeem `c`.
    assert_eq!(
        code(&pc::delete(&r, &w.ws, &a, Some(&c)).unwrap_err()),
        "project.rerun_in_progress"
    );
    // Let the rerun finish; the same confirmation still deletes A.
    p.go.send(()).unwrap();
    assert_eq!(settled(&w).status, RerunProgress::Saved);
    pc::delete(&r, &w.ws, &a, Some(&c)).expect("the confirmation is still good");
    assert!(
        !pc::list(&r, &w.ws)
            .unwrap()
            .iter()
            .any(|e| e.project_id == a),
        "A is gone"
    );
}

#[test]
fn each_command_loads_the_provenance_graph_once() {
    // F3 (5E-fix-1): project_runs, both exports and project_run_result each
    // load the graph exactly once, not once per run. The counter is
    // thread-local and incremented in load_graph; these commands run on this
    // thread, so it reads what they did.
    use app_lib::project_store::{graph_loads_on_this_thread, reset_graph_loads_on_this_thread};
    let w = World::new("f3-one-load");
    let (id, run) = with_a_summary(&w);
    // A rerun, so project_runs compares the two manifests (the old code loaded
    // the graph once per compare, and `runs` itself loaded it too).
    pc::rerun(&w.root(), &w.ws, 1).unwrap();
    assert_eq!(settled(&w).status, RerunProgress::Saved);
    let r = w.root();
    reset_graph_loads_on_this_thread();
    pc::runs(&r, &w.ws, &id).unwrap();
    assert_eq!(graph_loads_on_this_thread(), 1, "project_runs");
    reset_graph_loads_on_this_thread();
    pc::report(&r, &w.ws, &id).unwrap();
    assert_eq!(graph_loads_on_this_thread(), 1, "project_report");
    reset_graph_loads_on_this_thread();
    pc::provenance(&r, &w.ws, &id).unwrap();
    assert_eq!(graph_loads_on_this_thread(), 1, "project_provenance");
    reset_graph_loads_on_this_thread();
    pc::run_result(&r, &w.ws, &id, run).unwrap();
    assert_eq!(graph_loads_on_this_thread(), 1, "project_run_result");
}

#[test]
fn leaving_statistics_is_refused_until_the_save_at_the_end_has_returned() {
    let w = World::new("rr-leave");
    let (id, _) = with_a_summary(&w);
    // A second connection holds the database's write lock, as a second copy
    // of the app saving would: the save at the end waits on it.
    let db = Connection::open(project_dir(&w, &id).join("project.db")).unwrap();
    db.execute_batch("BEGIN IMMEDIATE").unwrap();
    let started = pc::rerun(&w.root(), &w.ws, 1).unwrap();
    assert!(w.wait(started.run_id).is_terminal());
    assert_eq!(
        w.engine().runs.unfinished(),
        None,
        "the run has ended: the seal alone would let Statistics go"
    );
    std::thread::sleep(Duration::from_millis(300));
    let asked = Instant::now();
    let now = pc::current(&w.ws).unwrap();
    assert!(
        asked.elapsed() < Duration::from_secs(1),
        "project_current answered without waiting for the save: {:?}",
        asked.elapsed()
    );
    assert_eq!(now.rerun.map(|r| r.status), Some(RerunProgress::Going));
    let e = w.ws.deactivate().unwrap_err();
    assert_eq!(e.code.wire(), "workspace.run_active");
    assert_eq!(
        e.params.get(&ParamKey::Run),
        Some(&ParamValue::Int(started.run_id as i64)),
        "the rerun's run"
    );
    db.execute_batch("ROLLBACK").unwrap();
    drop(db);
    let done = settled(&w);
    assert_eq!(done.status, RerunProgress::Saved);
    w.ws.deactivate()
        .expect("allowed once the save has returned");
}

/* ---- the ends (decisions 186 and 187) ----------------------------------------- */

#[test]
fn a_rerun_that_succeeds_is_saved_beside_the_run_it_repeats() {
    let w = World::new("rr-saved");
    let slot = w.open("a.csv", CSV);
    let d = w.describe(slot);
    let t = w.ttest(slot);
    let id = pc::save_run(&w.root(), &w.ws, d, new_project("Reruns"))
        .unwrap()
        .project_id;
    pc::save_run(&w.root(), &w.ws, t, into_open(&id)).unwrap();

    let a = pc::rerun(&w.root(), &w.ws, 1).unwrap();
    assert_eq!(a.kind, RunKind::Describe);
    let ra = settled(&w);
    assert_eq!(
        (ra.run_id, ra.rerun_of, ra.status, ra.stored_run_id, ra.code),
        (a.run_id, 1, RerunProgress::Saved, Some(3), None)
    );
    let b = pc::rerun(&w.root(), &w.ws, 2).unwrap();
    assert_eq!(b.kind, RunKind::Ttest);
    assert_eq!(settled(&w).stored_run_id, Some(4));
    // A rerun of a rerun (decision 185).
    pc::rerun(&w.root(), &w.ws, 3).unwrap();
    assert_eq!(settled(&w).stored_run_id, Some(5));

    let rows: Vec<(u64, Option<u64>, RunState, u32)> = pc::runs(&w.root(), &w.ws, &id)
        .unwrap()
        .iter()
        .map(|r| (r.stored_run_id, r.rerun_of, r.state, r.capability_version))
        .collect();
    assert_eq!(
        rows,
        [
            (1, None, RunState::Succeeded, 1),
            (2, None, RunState::Succeeded, 1),
            (3, Some(1), RunState::Succeeded, 1),
            (4, Some(2), RunState::Succeeded, 1),
            (5, Some(3), RunState::Succeeded, 1),
        ]
    );

    // Decision 15, for both kinds on this platform: the same output bytes.
    let read = |n: u64| pc::run_result(&w.root(), &w.ws, &id, n).unwrap();
    let manifest =
        |n: u64| -> Value { serde_json::from_str(&read(n).manifest_json.unwrap()).unwrap() };
    for (original, rerun) in [(1, 3), (2, 4), (1, 5)] {
        assert_eq!(
            manifest(rerun)["outputHash"],
            manifest(original)["outputHash"],
            "run {rerun}'s output bytes are run {original}'s"
        );
        assert_eq!(manifest(rerun)["engineVersion"], Value::from(1));
        assert_eq!(
            read(rerun).data_file.sha256,
            read(original).data_file.sha256
        );
        assert_ne!(
            read(rerun).bundle_json,
            read(original).bundle_json,
            "its own bundle"
        );
    }
    // The data file was reused, never copied: one artifact for five runs.
    assert_eq!(
        column(
            &w,
            &id,
            "SELECT hash FROM artifacts WHERE kind = 'data_file'"
        )
        .len(),
        1
    );
    for n in [3, 4, 5] {
        let mut made = column(
            &w,
            &id,
            &format!("SELECT a.kind FROM generated g JOIN artifacts a ON a.hash = g.hash WHERE g.run_id = {n} ORDER BY a.kind"),
        );
        made.dedup();
        assert_eq!(made, ["result_bundle", "run_manifest"], "run {n}");
    }
}

/* ---- the comparison (decision 189) -------------------------------------------- */

#[test]
fn the_runs_view_compares_each_rerun_by_its_output_hash_and_never_judges_it() {
    use gaply_core::research_project::RerunComparison::{Different, Identical, NotCompared};
    let w = World::new("rr-compare");
    let (id, _) = with_a_summary(&w);
    pc::rerun(&w.root(), &w.ws, 1).unwrap();
    assert_eq!(settled(&w).stored_run_id, Some(2));
    // A rerun whose stored output hash differs, on another engine version,
    // and one whose manifest has no output hash.
    let other = format!(
        "{{\"outputHash\":\"{}\",\"engineVersion\":2}}",
        "0".repeat(64)
    );
    let different = store_a_rerun_row(&w, &id, 1, other.as_bytes());
    let unread = store_a_rerun_row(&w, &id, 1, b"{\"engineVersion\":1}");
    let view = |n: u64| {
        let r = pc::runs(&w.root(), &w.ws, &id)
            .unwrap()
            .into_iter()
            .find(|r| r.stored_run_id == n)
            .unwrap();
        (r.rerun_of, r.comparison, r.engine_version_differs)
    };
    assert_eq!(view(1), (None, None, None));
    assert_eq!(view(2), (Some(1), Some(Identical), Some(false)));
    assert_eq!(view(different), (Some(1), Some(Different), Some(true)));
    assert_eq!(view(unread), (Some(1), Some(NotCompared), None));

    // The repeated run's manifest no longer reads: nothing is compared, and
    // reading it marks it damaged, as any read does.
    let manifest = pc::run_result(&w.root(), &w.ws, &id, 1)
        .unwrap()
        .manifest_json
        .unwrap();
    let file = ContentHash::of_bytes(manifest.as_bytes())
        .as_hex()
        .to_string();
    std::fs::remove_file(project_dir(&w, &id).join("artifacts").join(&file)).unwrap();
    assert_eq!(view(2), (Some(1), Some(NotCompared), None));
    assert_eq!(
        column(
            &w,
            &id,
            &format!("SELECT damage FROM artifacts WHERE hash = '{file}'")
        ),
        ["project.artifact_missing"]
    );
}

#[test]
fn a_quit_mid_run_writes_nothing() {
    let p = parking();
    let w = World::with("rr-quit", SMALL, Some(Arc::clone(&p.runs)));
    let (id, _) = with_a_summary(&w);
    let before = state_of(&w, &id);
    p.armed.store(true, Ordering::SeqCst);
    let started = pc::rerun(&w.root(), &w.ws, 1).unwrap();
    p.reached.recv().unwrap();
    p.armed.store(false, Ordering::SeqCst);

    // The app quits: the session goes, with the worker parked.
    let World { dir, ws, clock } = w;
    let gone = Arc::downgrade(ws.projects());
    drop(ws);
    // The rerun's end hook holds the session weakly, so nothing keeps it
    // once the app has dropped it. Asserted here, not only by the disk
    // below: the late end's save takes its own time, so a disk read after
    // it is a race that can miss it.
    assert_eq!(
        gone.strong_count(),
        0,
        "nothing keeps the dropped session alive"
    );
    let projects = Arc::new(app_lib::project_commands::ProjectSession::with(
        SMALL,
        clock.clone(),
    ));
    let factory: RunsFactory = Arc::new(|| Arc::new(Runs::default()));
    let ws = Arc::new(WorkspaceSession::with_projects(
        Arc::new(readiness),
        factory,
        projects,
    ));
    ws.activate(WorkspaceId::Statistics).unwrap();
    let w = World { dir, ws, clock };
    pc::open(&w.root(), &w.ws, &id).expect("the project opens again");
    assert_eq!(
        pc::runs(&w.root(), &w.ws, &id).unwrap().len(),
        1,
        "no new row"
    );
    pc::close(&w.root(), &w.ws).unwrap();
    assert_eq!(state_of(&w, &id), before, "no new file");

    // The parked run, let go, ends with its session gone: still nothing.
    p.go.send(()).unwrap();
    let start = Instant::now();
    while !p
        .runs
        .snapshot(RunId(started.run_id))
        .unwrap()
        .record
        .state
        .is_terminal()
    {
        assert!(start.elapsed() < Duration::from_secs(10));
        std::thread::sleep(Duration::from_millis(2));
    }
    std::thread::sleep(Duration::from_millis(200));
    assert_eq!(state_of(&w, &id), before, "nothing after it ended either");
}

#[test]
fn a_cancelled_rerun_writes_nothing_and_says_so() {
    let p = parking();
    let w = World::with("rr-cancel", SMALL, Some(Arc::clone(&p.runs)));
    let (id, _) = with_a_summary(&w);
    let before = state_of(&w, &id);
    p.armed.store(true, Ordering::SeqCst);
    let started = pc::rerun(&w.root(), &w.ws, 1).unwrap();
    p.reached.recv().unwrap();
    p.armed.store(false, Ordering::SeqCst);
    let snap = desc::cancel(&w.engine().runs, started.run_id).unwrap();
    assert_eq!(snap.state, RunState::Cancelled);
    let r = pc::current(&w.ws).unwrap().rerun.unwrap();
    assert_eq!(
        (r.run_id, r.status, r.stored_run_id, r.code),
        (
            started.run_id,
            RerunProgress::NotSaved,
            None,
            Some(gaply_core::research_project::ProjectCode::RunCancelled)
        )
    );
    assert_eq!(state_of(&w, &id), before);
    assert_eq!(w.ws.projects().rerun_going(), None);
    p.go.send(()).unwrap();
    pc::close(&w.root(), &w.ws).expect("no longer going");
    assert_eq!(state_of(&w, &id), before);
}

#[test]
fn a_rerun_that_fails_with_its_code_is_saved_with_its_failure_record() {
    let w = World::new("rr-failed");
    let (id, _) = with_a_summary(&w);
    // A Succeeded row whose data fail at run time: both groups constant,
    // which passes validation.
    let slot = w.open("constant.csv", CONSTANT);
    let spec = {
        let e = w.engine();
        let held = e.datasets.get(slot).unwrap();
        stats_ttest::spec(
            OperationId(2),
            held.dataset(),
            TTestParams {
                outcome: Alias(ColumnAlias(1)),
                group: Alias(ColumnAlias(2)),
                levels: None,
                variant: Some(Variant::Welch),
                missing_codes: Vec::new(),
            },
        )
    };
    let stored = store_a_row(
        &w,
        &id,
        (stats_ttest::CAPABILITY_ID, 1),
        RunState::Succeeded,
        (CONSTANT, DataFormat::Csv),
        &serde_json::to_vec(&spec).unwrap(),
    );
    let started = pc::rerun(&w.root(), &w.ws, stored).unwrap();
    assert_eq!(started.kind, RunKind::Ttest);
    let r = settled(&w);
    assert_eq!(r.status, RerunProgress::Saved, "{:?}", r.code);
    let saved = r.stored_run_id.unwrap();
    let back = pc::run_result(&w.root(), &w.ws, &id, saved).unwrap();
    assert_eq!(back.state, RunState::Failed);
    assert!(back.bundle_json.is_none() && back.manifest_json.is_none());
    let failure: Value = serde_json::from_str(&back.failure_json.unwrap()).unwrap();
    assert_eq!(
        failure["failure"]["code"],
        "stats.ttest.both_groups_constant"
    );
    let row = pc::runs(&w.root(), &w.ws, &id)
        .unwrap()
        .into_iter()
        .find(|r| r.stored_run_id == saved)
        .unwrap();
    assert_eq!(row.rerun_of, Some(stored));
}

#[test]
fn a_reruns_own_run_is_never_saved_by_hand() {
    let p = parking();
    let w = World::with("rr-by-hand", SMALL, Some(Arc::clone(&p.runs)));
    let (id, _) = with_a_summary(&w);
    let saved = pc::rerun(&w.root(), &w.ws, 1).unwrap();
    settled(&w);
    p.armed.store(true, Ordering::SeqCst);
    let cancelled = pc::rerun(&w.root(), &w.ws, 1).unwrap();
    p.reached.recv().unwrap();
    p.armed.store(false, Ordering::SeqCst);
    desc::cancel(&w.engine().runs, cancelled.run_id).unwrap();
    p.go.send(()).unwrap();
    let before = state_of(&w, &id);
    for (run, want) in [
        (saved.run_id, "project.run_already_saved"),
        (cancelled.run_id, "project.data_unavailable"),
    ] {
        for target in [into_open(&id), new_project("By hand")] {
            let e = pc::save_run(&w.root(), &w.ws, run, target).unwrap_err();
            assert_eq!(code(&e), want, "run {run}");
        }
    }
    assert_eq!(state_of(&w, &id), before);
    assert_eq!(
        pc::list(&w.root(), &w.ws).unwrap().len(),
        1,
        "no project made"
    );
}

#[test]
fn a_reruns_dataset_evicts_no_open_dataset() {
    let w = World::new("rr-slots");
    let slots: Vec<u64> = (0..4).map(|i| w.open(&format!("s{i}.csv"), CSV)).collect();
    let run = w.describe(slots[0]);
    let id = pc::save_run(&w.root(), &w.ws, run, new_project("Slots"))
        .unwrap()
        .project_id;
    let e = w.engine();
    let last = e.datasets.last_id();
    pc::rerun(&w.root(), &w.ws, 1).unwrap();
    assert_eq!(settled(&w).status, RerunProgress::Saved);
    assert_eq!(e.datasets.held(), 4, "the same four slots");
    for s in &slots {
        assert!(e.datasets.get(*s).is_ok(), "slot {s} is still open");
    }
    assert_eq!(e.datasets.last_id(), last, "no slot number was handed out");
    assert_eq!(pc::runs(&w.root(), &w.ws, &id).unwrap().len(), 2);
}
