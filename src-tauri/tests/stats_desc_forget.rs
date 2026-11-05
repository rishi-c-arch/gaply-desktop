//! **`stats_desc_forget_run`, through IPC (Research OS, Phase 8R, decision 468;
//! ADR-012 Part 7).** Remove in the Output drops its record, so a removed
//! result's newer record cannot later push out the oldest entry still shown.
//! A finished run goes; an unfinished one is refused with `run.not_finished`
//! and keeps running; a record that is not held is `run.unknown_run`, which
//! the screen treats as done; and a forgotten run reads afterwards as
//! `project.run_evicted`, as any dropped one does.

mod desc_support;
mod project_support;

use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::{json, Value};

use app_lib::project_commands::{self as pc, SaveTarget};
use app_lib::runs::{CheckpointHook, Clock, Point, Runs};
use desc_support::{app, call, refusal, spec, until_terminal, TempDir};
use gaply_core::Database;

const CSV: &[u8] = b"a,b\n1,2\n2,3\n3,5\n4,4\n";

struct Now;
impl Clock for Now {
    fn now_ms(&self) -> u64 {
        0
    }
}

struct World {
    _dir: TempDir,
    _app: tauri::App<tauri::test::MockRuntime>,
    w: tauri::WebviewWindow<tauri::test::MockRuntime>,
    data: std::path::PathBuf,
}

fn world(runs: Option<Arc<Runs>>) -> World {
    let dir = TempDir::new("forget");
    let data = dir.write("data.csv", CSV);
    let app_data = dir.0.join("appdata");
    std::fs::create_dir_all(&app_data).unwrap();
    let db = Arc::new(Database::in_memory().unwrap());
    let (app, w) = app(&app_data, db, runs);
    World {
        _dir: dir,
        _app: app,
        w,
        data,
    }
}

fn start(w: &World) -> (Value, u64) {
    let o = call(
        &w.w,
        "stats_desc_open",
        json!({ "path": w.data.to_string_lossy() }),
    )
    .expect("open");
    let run = call(
        &w.w,
        "stats_desc_run",
        json!({ "slot": o["slot"], "spec": spec(&o, 1, &[("C1", "numeric")], &[]) }),
    )
    .expect("run")["runId"]
        .as_u64()
        .expect("runId");
    (o, run)
}

fn forget(w: &World, run: u64) -> Result<Value, Value> {
    call(&w.w, "stats_desc_forget_run", json!({ "runId": run }))
}

#[test]
fn a_finished_run_is_forgotten_and_a_second_forget_is_unknown_run() {
    let w = world(None);
    let (_o, run) = start(&w);
    assert_eq!(until_terminal(&w.w, run)["state"], "succeeded");

    assert_eq!(forget(&w, run).expect("forgotten"), Value::Null);

    let status = call(&w.w, "stats_desc_status", json!({ "runId": run })).unwrap_err();
    assert_eq!(refusal(&status)["code"], "run.unknown_run");
    let again = forget(&w, run).unwrap_err();
    assert_eq!(refusal(&again)["code"], "run.unknown_run");
    let never = forget(&w, 99).unwrap_err();
    assert_eq!(refusal(&never)["code"], "run.unknown_run");
}

#[test]
fn an_unfinished_run_is_refused_and_keeps_running() {
    let (parked_tx, parked_rx) = mpsc::channel();
    let (go_tx, go_rx) = mpsc::channel::<()>();
    let go_rx = Mutex::new(go_rx);
    let hook: CheckpointHook = Arc::new(move |_run, point| {
        if point == Point::Column(0) {
            parked_tx.send(()).ok();
            go_rx
                .lock()
                .unwrap()
                .recv_timeout(Duration::from_secs(20))
                .ok();
        }
    });
    let runs = Arc::new(Runs::with_hooks(Arc::new(Now), Some(hook), 30_000));
    let w = world(Some(runs.clone()));
    let (_o, run) = start(&w);
    parked_rx
        .recv_timeout(Duration::from_secs(10))
        .expect("parked");

    let e = forget(&w, run).unwrap_err();
    assert_eq!(refusal(&e)["code"], "run.not_finished");
    assert_eq!(runs.held(), 1, "the record stays");

    go_tx.send(()).unwrap();
    assert_eq!(until_terminal(&w.w, run)["state"], "succeeded");
    assert_eq!(forget(&w, run).expect("now it can go"), Value::Null);
    assert_eq!(runs.held(), 0);
}

#[test]
fn a_forgotten_run_reads_as_evicted_at_a_save() {
    let p = project_support::World::new("forget-save");
    let slot = p.open("a.csv", CSV);
    let run = p.describe(slot);
    p.engine()
        .runs
        .forget(gaply_core::contracts::RunId(run))
        .expect("forgotten");
    let e = pc::save_run(
        &p.root(),
        &p.ws,
        run,
        SaveTarget::NewProject { name: "p".into() },
    )
    .unwrap_err();
    assert_eq!(project_support::code(&e), "project.run_evicted");
}

/// ADR-012 Part 7: after a close an entry's export still works, from its record,
/// and its Save is refused `project.data_unavailable`, which is what the Output's
/// disabled button says. Rust stays the boundary whatever the screen shows.
#[test]
fn after_a_close_the_export_holds_and_the_save_is_refused() {
    let p = project_support::World::new("forget-close");
    let slot = p.open("a.csv", CSV);
    let run = p.describe(slot);
    let e = p.engine();
    assert!(app_lib::stats_desc_commands::close(&e.datasets, slot));
    assert!(app_lib::stats_desc_commands::export(
        &e.runs,
        run,
        app_lib::stats_desc_commands::ExportFormat::Csv
    )
    .is_ok());
    let refused = pc::save_run(
        &p.root(),
        &p.ws,
        run,
        SaveTarget::NewProject { name: "p".into() },
    )
    .unwrap_err();
    assert_eq!(project_support::code(&refused), "project.data_unavailable");
}
