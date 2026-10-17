//! **The window's workspace session, through IPC (Research OS, Phase 3).**
//! The four `workspace_*` commands and the eight `stats_desc_*` commands over
//! Tauri's mock runtime, called with the webview's argument names:
//!
//! * the list: three descriptors, Statistics available, Compute and Design
//!   missing, `entitled` left `null` for the frontend;
//! * activation refused for a missing workspace and for a failing readiness
//!   check, the state `NoWorkspace` afterwards and no engine held;
//! * every `stats_desc_*` command refused while `NoWorkspace` and while
//!   `Activating` (a readiness check parked mid-activation);
//! * a forged spec naming Compute refused, with no record made;
//! * deactivation refused during a parked run, with the run's id, the state
//!   still `Active`; allowed once the run is cancelled; the old run id and slot
//!   unknown after reactivating;
//! * activating Statistics asks about and builds nothing for Compute or
//!   Design, and leaving it drops its engine.

mod desc_support;

use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex, Weak};
use std::time::{Duration, Instant};

use serde_json::{json, Value};
use tauri::Manager;

use app_lib::runs::{CheckpointHook, Clock, Point, Runs};
use app_lib::state::AppState;
use app_lib::workspace::{Health, HealthCheck, RunsFactory};
use desc_support::*;
use gaply_core::contracts::{RunId, WorkspaceId};
use gaply_core::Database;

struct SinceTest(Instant);
impl Clock for SinceTest {
    fn now_ms(&self) -> u64 {
        self.0.elapsed().as_millis() as u64
    }
}

struct World {
    dir: TempDir,
    app: tauri::App<tauri::test::MockRuntime>,
    w: tauri::WebviewWindow<tauri::test::MockRuntime>,
}

fn world_with(runs: RunsFactory, health: HealthCheck) -> World {
    let dir = TempDir::new("ws");
    let app_data = dir.0.join("appdata");
    std::fs::create_dir_all(&app_data).unwrap();
    let db = Arc::new(Database::in_memory().unwrap());
    let (app, w) = app_with(&app_data, db, runs, health);
    World { dir, app, w }
}

fn csv(w: &World) -> String {
    let body = delimited(
        &["score", "group"],
        &(1..=6)
            .map(|i| {
                vec![
                    C::N(["1.5", "2.5", "3.5", "4.5", "5.5", "6.5"][i - 1]),
                    C::T(if i % 2 == 0 { "a" } else { "b" }),
                ]
            })
            .collect::<Vec<_>>(),
        ",",
        "\n",
    );
    w.dir
        .write("data.csv", &body)
        .to_string_lossy()
        .into_owned()
}

fn state(w: &World) -> Value {
    call(&w.w, "workspace_state", json!({})).expect("state")
}

fn activate(w: &World, id: &str) -> Result<Value, Value> {
    call(&w.w, "workspace_activate", json!({ "id": id }))
}

fn deactivate(w: &World) -> Result<Value, Value> {
    call(&w.w, "workspace_deactivate", json!({}))
}

/// Every `stats_desc_*` command, with plausible arguments.
fn every_desc_call(w: &World, path: &str) -> Vec<(&'static str, Result<Value, Value>)> {
    let spec = json!({
        "schemaVersion": 1,
        "operationId": 1,
        "workspaceId": "statistics",
        "capabilityId": "stats.describe",
        "capabilityVersion": 1,
        "inputs": [],
        "parameters": { "columns": [], "missingCodes": [] },
        "randomSeed": null,
    });
    vec![
        (
            "stats_desc_open",
            call(&w.w, "stats_desc_open", json!({ "path": path })),
        ),
        (
            "stats_desc_validate",
            call(
                &w.w,
                "stats_desc_validate",
                json!({ "slot": 1, "spec": spec }),
            ),
        ),
        (
            "stats_desc_run",
            call(&w.w, "stats_desc_run", json!({ "slot": 1, "spec": spec })),
        ),
        (
            "stats_desc_status",
            call(&w.w, "stats_desc_status", json!({ "runId": 1 })),
        ),
        (
            "stats_desc_result",
            call(&w.w, "stats_desc_result", json!({ "runId": 1 })),
        ),
        (
            "stats_desc_cancel",
            call(&w.w, "stats_desc_cancel", json!({ "runId": 1 })),
        ),
        (
            "stats_desc_export",
            call(
                &w.w,
                "stats_desc_export",
                json!({ "runId": 1, "format": "csv" }),
            ),
        ),
        (
            "stats_desc_close",
            call(&w.w, "stats_desc_close", json!({ "slot": 1 })),
        ),
    ]
}

fn assert_all_inactive(calls: Vec<(&str, Result<Value, Value>)>, phase: &str) {
    assert_eq!(calls.len(), 8, "every stats_desc_* command");
    for (name, r) in calls {
        let e = r.expect_err(name);
        let c = refusal(&e);
        assert_eq!(c["code"], "workspace.inactive", "{name}: {c}");
        assert_eq!(c["params"]["expected"], "statistics", "{name}");
        assert_eq!(c["params"]["from"], phase, "{name}");
    }
}

#[test]
fn the_list_has_three_descriptors_and_leaves_entitlement_to_the_frontend() {
    let w = world_with(fresh(), Arc::new(app_lib::workspace::readiness));
    let list = call(&w.w, "workspace_list", json!({})).expect("list");
    let rows = list.as_array().expect("an array");
    let summary: Vec<(String, String, Value)> = rows
        .iter()
        .map(|r| {
            (
                r["id"].as_str().unwrap().to_string(),
                r["availability"].as_str().unwrap().to_string(),
                r["entitled"].clone(),
            )
        })
        .collect();
    assert_eq!(
        summary,
        [
            ("statistics".into(), "available".into(), Value::Null),
            ("compute".into(), "missing".into(), Value::Null),
            ("design".into(), "missing".into(), Value::Null),
        ]
    );
    // Phase 4A (decision 47): the registry's ids, one list.
    assert_eq!(
        rows[0]["capabilityIds"],
        json!(gaply_core::stats_registry::Registry::statistics()
            .ids()
            .iter()
            .map(|i| i.as_str())
            .collect::<Vec<_>>())
    );
    assert_eq!(rows[0]["displayNameCode"], "workspace.name.statistics");
    assert_eq!(rows[1]["capabilityIds"], json!([]));
    assert_eq!(
        state(&w),
        json!({ "state": { "phase": "no_workspace" }, "activeRun": null, "latestRun": null })
    );
}

#[test]
fn a_missing_workspace_and_a_failing_readiness_check_are_refused_and_nothing_is_active() {
    let asked = Arc::new(Mutex::new(Vec::<WorkspaceId>::new()));
    let log = asked.clone();
    let failing: HealthCheck = Arc::new(move |id| {
        log.lock().unwrap().push(id);
        Health::NotReady
    });
    let w = world_with(fresh(), failing);
    let session = w.app.state::<AppState>().workspace.clone();

    for id in ["compute", "design"] {
        let c = refusal(&activate(&w, id).expect_err(id));
        assert_eq!(c["code"], "workspace.unavailable", "{id}: {c}");
        assert_eq!(c["params"]["workspace"], id);
        assert_eq!(state(&w)["state"], json!({ "phase": "no_workspace" }));
        assert!(session.engines_held().is_empty());
    }
    assert!(
        asked.lock().unwrap().is_empty(),
        "a missing workspace's readiness is never asked"
    );

    let c = refusal(&activate(&w, "statistics").expect_err("not ready"));
    assert_eq!(c["code"], "workspace.not_ready", "{c}");
    assert_eq!(c["params"]["workspace"], "statistics");
    assert_eq!(state(&w)["state"], json!({ "phase": "no_workspace" }));
    assert!(session.engines_held().is_empty(), "no engine was built");
    assert_eq!(*asked.lock().unwrap(), [WorkspaceId::Statistics]);

    // An unknown word is not a workspace at all.
    assert!(activate(&w, "geometry").is_err());
    assert_eq!(state(&w)["state"], json!({ "phase": "no_workspace" }));
}

#[test]
fn every_stats_desc_command_is_refused_while_no_workspace_and_while_activating() {
    // Parks the readiness check until released, so the state stays Activating.
    let (entered_tx, entered_rx) = mpsc::channel::<()>();
    let (go_tx, go_rx) = mpsc::channel::<()>();
    let go_rx = Mutex::new(go_rx);
    let parked: HealthCheck = Arc::new(move |_| {
        entered_tx.send(()).ok();
        go_rx
            .lock()
            .unwrap()
            .recv_timeout(Duration::from_secs(20))
            .ok();
        Health::Ready
    });
    let w = world_with(fresh(), parked);
    let path = csv(&w);

    assert_all_inactive(every_desc_call(&w, &path), "no_workspace");
    assert!(w
        .app
        .state::<AppState>()
        .workspace
        .engines_held()
        .is_empty());

    std::thread::scope(|s| {
        let webview = w.w.clone();
        let activating = s.spawn(move || {
            call(
                &webview,
                "workspace_activate",
                json!({ "id": "statistics" }),
            )
        });
        entered_rx
            .recv_timeout(Duration::from_secs(10))
            .expect("the readiness check ran");
        assert_eq!(
            state(&w)["state"],
            json!({ "phase": "activating", "workspace": "statistics" })
        );
        assert_all_inactive(every_desc_call(&w, &path), "activating");
        // A second activation or a deactivation has no edge from Activating.
        let c = refusal(&activate(&w, "statistics").expect_err("twice"));
        assert_eq!(c["code"], "workspace.illegal_transition", "{c}");
        let c = refusal(&deactivate(&w).expect_err("while activating"));
        assert_eq!(c["code"], "workspace.illegal_transition", "{c}");
        go_tx.send(()).unwrap();
        let v = activating.join().unwrap().expect("activated");
        assert_eq!(
            v["state"],
            json!({ "phase": "active", "workspace": "statistics" })
        );
    });

    // Active: the same commands reach the run manager.
    let o = call(&w.w, "stats_desc_open", json!({ "path": path })).expect("open");
    assert_eq!(o["profile"]["rows"], 6);
}

#[test]
fn a_spec_forged_for_compute_is_refused_and_makes_no_record() {
    let runs = Arc::new(Runs::default());
    let w = world_with(the(runs.clone()), Arc::new(app_lib::workspace::readiness));
    activate(&w, "statistics").expect("active");
    let path = csv(&w);
    let o = call(&w.w, "stats_desc_open", json!({ "path": path })).expect("open");

    let mut forged = spec(&o, 1, &[("C1", "numeric")], &[]);
    forged["workspaceId"] = json!("compute");
    for cmd in ["stats_desc_validate", "stats_desc_run"] {
        let e = call(&w.w, cmd, json!({ "slot": o["slot"], "spec": forged })).expect_err(cmd);
        let c = refusal(&e);
        assert_eq!(c["code"], "workspace.wrong_workspace", "{cmd}: {c}");
        assert_eq!(c["params"]["expected"], "statistics");
        assert_eq!(c["params"]["found"], "compute");
    }
    assert_eq!(runs.held(), 0, "no run record was made");

    // A capability of a workspace this build does not have cannot even be
    // named: the id is refused on the way in.
    let mut alien = spec(&o, 2, &[("C1", "numeric")], &[]);
    alien["capabilityId"] = json!("compute.solve");
    assert!(call(
        &w.w,
        "stats_desc_run",
        json!({ "slot": o["slot"], "spec": alien })
    )
    .is_err());
    assert_eq!(runs.held(), 0);

    // The honest spec runs.
    let ok = spec(&o, 3, &[("C1", "numeric")], &[]);
    let run = call(
        &w.w,
        "stats_desc_run",
        json!({ "slot": o["slot"], "spec": ok }),
    )
    .expect("run")["runId"]
        .as_u64()
        .unwrap();
    assert_eq!(until_terminal(&w.w, run)["state"], "succeeded");
}

/// A factory whose run managers park their worker at column 1 of run 1, and
/// the newest manager it built, held weakly so the test cannot keep it alive.
fn parked_runs() -> (
    RunsFactory,
    Arc<Mutex<Weak<Runs>>>,
    Receiver<RunId>,
    Sender<()>,
) {
    let (parked_tx, parked_rx) = mpsc::channel();
    let (go_tx, go_rx) = mpsc::channel::<()>();
    let go_rx = Mutex::new(go_rx);
    let hook: CheckpointHook = Arc::new(move |run, point| {
        if run == RunId(1) && point == Point::Column(1) {
            parked_tx.send(run).ok();
            go_rx
                .lock()
                .unwrap()
                .recv_timeout(Duration::from_secs(20))
                .ok();
        }
    });
    let newest = Arc::new(Mutex::new(Weak::new()));
    let keep = newest.clone();
    let factory: RunsFactory = Arc::new(move || {
        let runs = Arc::new(Runs::with_hooks(
            Arc::new(SinceTest(Instant::now())),
            Some(hook.clone()),
            30_000,
        ));
        *keep.lock().unwrap() = Arc::downgrade(&runs);
        runs
    });
    (factory, newest, parked_rx, go_tx)
}

#[test]
fn deactivation_is_refused_during_a_run_and_allowed_after_it_is_cancelled() {
    let (factory, newest, parked, go) = parked_runs();
    let w = world_with(factory, Arc::new(app_lib::workspace::readiness));
    activate(&w, "statistics").expect("active");
    let weak: Weak<Runs> = newest.lock().unwrap().clone();
    assert!(weak.upgrade().is_some(), "the engine's run manager exists");
    let path = csv(&w);
    let o = call(&w.w, "stats_desc_open", json!({ "path": path })).expect("open");
    let slot = o["slot"].as_u64().unwrap();
    let both = [("C1", "numeric"), ("C2", "categorical")];
    let run = call(
        &w.w,
        "stats_desc_run",
        json!({ "slot": slot, "spec": spec(&o, 1, &both, &[]) }),
    )
    .expect("run")["runId"]
        .as_u64()
        .unwrap();
    assert_eq!(
        parked.recv_timeout(Duration::from_secs(10)).unwrap(),
        RunId(run)
    );

    // Refused, with the run's id; nothing changed.
    let c = refusal(&deactivate(&w).expect_err("a run is going"));
    assert_eq!(c["code"], "workspace.run_active", "{c}");
    assert_eq!(c["params"]["run"], run);
    assert_eq!(c["params"]["workspace"], "statistics");
    let s = state(&w);
    assert_eq!(
        s["state"],
        json!({ "phase": "active", "workspace": "statistics" })
    );
    assert_eq!(s["activeRun"], run);
    assert_eq!(s["latestRun"], run);
    assert_eq!(
        call(&w.w, "stats_desc_status", json!({ "runId": run })).unwrap()["state"],
        "running",
        "the run was not touched"
    );

    // Cancel, as the dialog's Cancel does; the run ends Cancelled.
    call(&w.w, "stats_desc_cancel", json!({ "runId": run })).expect("cancel");
    go.send(()).unwrap();
    assert_eq!(until_terminal(&w.w, run)["state"], "cancelled");
    let s = state(&w);
    assert_eq!(s["activeRun"], Value::Null);
    assert_eq!(s["latestRun"], run);

    let v = deactivate(&w).expect("no run is going");
    assert_eq!(v["state"], json!({ "phase": "no_workspace" }));
    assert_eq!(v["latestRun"], Value::Null);
    let session = w.app.state::<AppState>().workspace.clone();
    assert!(session.engines_held().is_empty(), "the engine was dropped");
    // The run manager itself is gone once its worker thread has returned.
    let until = Instant::now() + Duration::from_secs(10);
    while weak.upgrade().is_some() {
        assert!(Instant::now() < until, "the old run manager is still held");
        std::thread::sleep(Duration::from_millis(10));
    }

    // Left: refused, not unknown.
    let c = refusal(&call(&w.w, "stats_desc_status", json!({ "runId": run })).unwrap_err());
    assert_eq!(c["code"], "workspace.inactive");

    // Back in: the old run id and the old slot are unknown, and new ids come
    // after them.
    activate(&w, "statistics").expect("active again");
    let c = refusal(&call(&w.w, "stats_desc_status", json!({ "runId": run })).unwrap_err());
    assert_eq!(c["code"], "run.unknown_run", "{c}");
    let c = refusal(
        &call(
            &w.w,
            "stats_desc_validate",
            json!({ "slot": slot, "spec": spec(&o, 2, &both, &[]) }),
        )
        .unwrap_err(),
    );
    assert_eq!(c["code"], "run.unknown_input", "{c}");
    let o2 = call(&w.w, "stats_desc_open", json!({ "path": path })).expect("open");
    assert!(o2["slot"].as_u64().unwrap() > slot, "{o2}");
}

#[test]
fn activating_statistics_builds_nothing_for_compute_or_design() {
    let asked = Arc::new(Mutex::new(Vec::<WorkspaceId>::new()));
    let log = asked.clone();
    let health: HealthCheck = Arc::new(move |id| {
        log.lock().unwrap().push(id);
        app_lib::workspace::readiness(id)
    });
    let built = Arc::new(Mutex::new(0usize));
    let count = built.clone();
    let dir = TempDir::new("ws-build");
    let app_data = dir.0.join("appdata");
    std::fs::create_dir_all(&app_data).unwrap();
    let (app, w) = app_with(
        &app_data,
        Arc::new(Database::in_memory().unwrap()),
        fresh(),
        health,
    );
    // Count Statistics engines through a factory that counts.
    let session = Arc::new(app_lib::workspace::WorkspaceSession::with_hooks(
        Arc::new(app_lib::workspace::readiness),
        Arc::new(move || {
            *count.lock().unwrap() += 1;
            Arc::new(Runs::default())
        }),
    ));
    assert!(
        session.engines_held().is_empty(),
        "nothing before activation"
    );
    session.activate(WorkspaceId::Statistics).expect("active");
    assert_eq!(session.engines_held(), [WorkspaceId::Statistics]);
    assert_eq!(*built.lock().unwrap(), 1, "one Statistics engine");
    session.deactivate().expect("left");
    assert!(session.engines_held().is_empty());

    // Through IPC: readiness is asked about Statistics only.
    let v = call(&w, "workspace_activate", json!({ "id": "statistics" })).expect("active");
    assert_eq!(v["state"]["workspace"], "statistics");
    assert_eq!(*asked.lock().unwrap(), [WorkspaceId::Statistics]);
    assert_eq!(
        app.state::<AppState>().workspace.engines_held(),
        [WorkspaceId::Statistics]
    );
    // Another workspace cannot be activated beside it.
    let c = refusal(&call(&w, "workspace_activate", json!({ "id": "compute" })).unwrap_err());
    assert_eq!(c["code"], "workspace.illegal_transition", "{c}");
    assert_eq!(c["params"]["workspace"], "statistics");
    assert_eq!(*asked.lock().unwrap(), [WorkspaceId::Statistics]);
}

/// Evaluation of Phases 2 and 3, F8. An open that passed the workspace check
/// before a deactivation inserts into the engine being dropped. Its slot
/// number must not be one the next engine hands out, or the webview could
/// address the next engine's dataset by an old number. Reproduced without
/// timing: the old engine is held, as the in-flight open holds it.
#[test]
fn an_open_landing_in_a_dropped_engine_never_shares_a_slot_number_with_the_next() {
    use app_lib::workspace::WorkspaceSession;
    use gaply_core::stats_l1::dataset::{build_dataset, DataFingerprint, RawTable, TableRef};

    let dataset = || {
        build_dataset(
            RawTable::from_rows(vec!["x".into()], vec![vec!["1".into()], vec!["2".into()]]),
            DataFingerprint::of_bytes(b"synthetic", "unit", TableRef::FirstTable),
        )
        .expect("dataset")
    };
    let session = WorkspaceSession::default();
    session.activate(WorkspaceId::Statistics).expect("active");
    let old = session.require_statistics().expect("engine");
    let before = old.datasets.insert("before".into(), dataset());
    session.deactivate().expect("left");
    session
        .activate(WorkspaceId::Statistics)
        .expect("active again");
    let new = session.require_statistics().expect("engine");
    assert!(!Arc::ptr_eq(&old, &new), "a new engine");

    // The in-flight open lands in the old engine after the deactivation.
    let late = old.datasets.insert("late".into(), dataset());
    let fresh = new.datasets.insert("fresh".into(), dataset());
    assert_ne!(
        late, fresh,
        "the late open and the next engine's first share a number"
    );
    assert!(fresh > before && late > before, "{before} {late} {fresh}");
    assert!(
        new.datasets.get(late).is_err(),
        "the late slot is not the new engine's"
    );
}
