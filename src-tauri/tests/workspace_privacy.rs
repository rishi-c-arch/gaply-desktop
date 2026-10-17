//! **The workspace surfaces carry no user data (Research OS, Phase 3).** A
//! sentinel sweep: a dataset with a sentinel header, level label and text
//! cell, in a file and folder with sentinel names, is opened and run in the
//! Statistics workspace; then every workspace surface is collected:
//!
//! * every view `workspace_list`, `workspace_state`, `workspace_activate`
//!   and `workspace_deactivate` returned;
//! * every workspace refusal: `unavailable`, `illegal_transition`,
//!   `run_active` during a parked run, and `inactive` from all eight
//!   `stats_desc_*` commands after the workspace was left, `stats_desc_open`
//!   given the sentinel path;
//! * every tracing span and event of the process, from every thread.
//!
//! No sentinel may appear in any of them. Each surface must have been
//! collected (counted), so the sweep is not vacuous. Alone in its binary on
//! purpose: its subscriber is the process's global default.

mod desc_support;
mod l1_support;

use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde_json::{json, Value};

use app_lib::runs::{CheckpointHook, Clock, Point, Runs};
use app_lib::workspace::RunsFactory;
use desc_support::*;
use gaply_core::contracts::RunId;
use gaply_core::Database;

const HDR: &str = "HDR_SENTINEL_w31";
const LVL: &str = "LVL_SENTINEL_w52";
const TXT: &str = "TXT_SENTINEL_w73";
const FILE: &str = "FILE_SENTINEL_w14";
const PATH: &str = "PATH_SENTINEL_w96";
const SENTINELS: [&str; 5] = [HDR, LVL, TXT, FILE, PATH];

struct SinceTest(Instant);
impl Clock for SinceTest {
    fn now_ms(&self) -> u64 {
        self.0.elapsed().as_millis() as u64
    }
}

#[test]
fn no_header_label_cell_file_name_or_path_reaches_a_workspace_surface() {
    let lines = Arc::new(Mutex::new(Vec::<String>::new()));
    tracing::subscriber::set_global_default(l1_support::Capture {
        lines: lines.clone(),
    })
    .expect("the only subscriber");
    tracing::callsite::rebuild_interest_cache();

    let dir = TempDir::new("ws-privacy");
    let folder = dir.0.join(PATH);
    std::fs::create_dir_all(&folder).unwrap();
    let rows: Vec<Vec<C>> = (0..6)
        .map(|i| {
            vec![
                C::N(["1.5", "2.5", "3.5", "4.5", "5.5", "6.5"][i]),
                C::T(if i % 2 == 0 { LVL } else { "b" }),
                C::T(if i == 3 { TXT } else { "t" }),
            ]
        })
        .collect();
    let data = folder.join(format!("{FILE}.csv"));
    std::fs::write(&data, delimited(&[HDR, "group", "note"], &rows, ",", "\n")).unwrap();
    let path = data.to_string_lossy().into_owned();

    // Run 2 parks at column 1, so a deactivation meets it.
    let (parked_tx, parked_rx) = mpsc::channel();
    let (go_tx, go_rx) = mpsc::channel::<()>();
    let go_rx = Mutex::new(go_rx);
    let hook: CheckpointHook = Arc::new(move |run, point| {
        if run == RunId(2) && point == Point::Column(1) {
            parked_tx.send(run).ok();
            go_rx
                .lock()
                .unwrap()
                .recv_timeout(Duration::from_secs(20))
                .ok();
        }
    });
    let factory: RunsFactory = Arc::new(move || {
        Arc::new(Runs::with_hooks(
            Arc::new(SinceTest(Instant::now())),
            Some(hook.clone()),
            30_000,
        ))
    });
    let app_data = dir.0.join("appdata");
    std::fs::create_dir_all(&app_data).unwrap();
    let (_app, w) = app_with(
        &app_data,
        Arc::new(Database::in_memory().unwrap()),
        factory,
        Arc::new(app_lib::workspace::readiness),
    );

    let mut views: Vec<Value> = Vec::new();
    let mut refusals: Vec<Value> = Vec::new();
    let ws = |cmd: &str, body: Value| call(&w, cmd, body);

    views.push(ws("workspace_list", json!({})).unwrap());
    views.push(ws("workspace_state", json!({})).unwrap());
    refusals.push(ws("workspace_activate", json!({ "id": "compute" })).unwrap_err());
    views.push(ws("workspace_activate", json!({ "id": "statistics" })).unwrap());
    refusals.push(ws("workspace_activate", json!({ "id": "design" })).unwrap_err());

    // The data's own views do carry the header and label: that is allowed, and
    // they are not workspace surfaces.
    let o = call(&w, "stats_desc_open", json!({ "path": path })).expect("open");
    assert!(o.to_string().contains(HDR), "the sentinel table was read");
    let both = [("C1", "numeric"), ("C2", "categorical")];
    let first = call(
        &w,
        "stats_desc_run",
        json!({ "slot": o["slot"], "spec": spec(&o, 1, &both, &[]) }),
    )
    .expect("run 1")["runId"]
        .as_u64()
        .unwrap();
    assert_eq!(until_terminal(&w, first)["state"], "succeeded");
    views.push(ws("workspace_state", json!({})).unwrap());

    let second = call(
        &w,
        "stats_desc_run",
        json!({ "slot": o["slot"], "spec": spec(&o, 2, &both, &[]) }),
    )
    .expect("run 2")["runId"]
        .as_u64()
        .unwrap();
    assert_eq!(
        parked_rx.recv_timeout(Duration::from_secs(10)).unwrap(),
        RunId(second)
    );
    views.push(ws("workspace_state", json!({})).unwrap());
    refusals.push(ws("workspace_deactivate", json!({})).unwrap_err());
    call(&w, "stats_desc_cancel", json!({ "runId": second })).unwrap();
    go_tx.send(()).unwrap();
    assert_eq!(until_terminal(&w, second)["state"], "cancelled");
    views.push(ws("workspace_deactivate", json!({})).unwrap());
    views.push(ws("workspace_state", json!({})).unwrap());

    // Left: all eight refuse, the open with the sentinel path among them.
    let s = spec(&o, 3, &both, &[]);
    for (cmd, body) in [
        ("stats_desc_open", json!({ "path": path })),
        (
            "stats_desc_validate",
            json!({ "slot": o["slot"], "spec": s }),
        ),
        ("stats_desc_run", json!({ "slot": o["slot"], "spec": s })),
        ("stats_desc_status", json!({ "runId": first })),
        ("stats_desc_result", json!({ "runId": first })),
        ("stats_desc_cancel", json!({ "runId": second })),
        (
            "stats_desc_export",
            json!({ "runId": first, "format": "csv" }),
        ),
        ("stats_desc_close", json!({ "slot": o["slot"] })),
    ] {
        refusals.push(call(&w, cmd, body).unwrap_err());
    }
    refusals.push(ws("workspace_deactivate", json!({})).unwrap_err());
    drop(dir);

    let codes: Vec<String> = refusals
        .iter()
        .map(|e| refusal(e)["code"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(
        codes,
        [
            "workspace.unavailable",
            "workspace.illegal_transition",
            "workspace.run_active",
            "workspace.inactive",
            "workspace.inactive",
            "workspace.inactive",
            "workspace.inactive",
            "workspace.inactive",
            "workspace.inactive",
            "workspace.inactive",
            "workspace.inactive",
            "workspace.illegal_transition",
        ],
        "each refusal surface was collected"
    );
    assert_eq!(views.len(), 7, "each view surface was collected");

    let traced = lines.lock().unwrap().clone();
    for name in [
        "workspace_activate",
        "workspace_deactivate",
        "workspace_state",
        "workspace_list",
    ] {
        assert!(
            traced
                .iter()
                .any(|l| l.starts_with(&format!("span {name} "))),
            "{name}'s span was captured"
        );
    }
    let mut leaks = Vec::new();
    for (surface, text) in views
        .iter()
        .map(|v| ("view", v.to_string()))
        .chain(refusals.iter().map(|e| ("refusal", e.to_string())))
        .chain(traced.iter().map(|l| ("tracing", l.clone())))
    {
        for s in SENTINELS {
            if text.contains(s) {
                leaks.push(format!("{surface}: {s} in {text}"));
            }
        }
    }
    println!(
        "swept {} views, {} refusals, {} tracing lines",
        views.len(),
        refusals.len(),
        traced.len()
    );
    assert!(
        leaks.is_empty(),
        "user data on a workspace surface:\n{leaks:#?}"
    );
}
