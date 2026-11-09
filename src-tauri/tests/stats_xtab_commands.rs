//! **Crosstabs' commands at their boundary (Research OS, Phase 8X-1,
//! decision 580).** Every call goes through Tauri's IPC on the mock runtime,
//! with the webview's argument names, into the state `AppState::new`
//! builds, and every assertion reads the JSON the webview reads.
//!
//! * The flow: open → validate → run → status → result → export (CSV and
//!   manifest) → close, the numbers equal to `stats_xtab`'s on the same
//!   dataset, the CSV equal to its export, and one output hash pinned.
//! * The progress counts its one check point: the total is
//!   `stats_xtab::check_points`, and a result has done it.
//! * A cancel at the check point stops the run with no result; the run
//!   manager's ceiling stops a run whose check point "sleeps" past it, on an
//!   injected clock, never 30 s of real time.
//! * Refusals, each with its exact code, and the registry's `capability.*`
//!   codes as they are (decision 74), before the slot is read.
//! * Statistics inactive: all six commands refuse `workspace.inactive`.
//! * `run.wrong_kind` both ways against the summary and the ANOVA.
//!
//! Every file is synthetic and in a folder the test creates and removes.

mod desc_support;
mod xtab_support;

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::{json, Value};

use app_lib::runs::{CheckpointHook, Clock, Point, Runs};
use app_lib::stats_l1_ingest::load_dataset;
use desc_support::{call, delimited, refusal, spec as desc_spec, until_terminal, TempDir, C};
use gaply_core::contracts::{ContentHash, RunManifest};
use gaply_core::stats_desc::Alias;
use gaply_core::stats_l1::dataset::ColumnAlias;
use gaply_core::stats_xtab::export::{manifest_json, result_csv};
use gaply_core::stats_xtab::{self, CrosstabsParams, XtabCode};
use gaply_core::Database;
use xtab_support::{app, app_with, until_terminal_xtab, xspec};

/* ---- fixtures --------------------------------------------------------------- */

const HEADERS: [&str; 6] = ["arm", "outcome", "score", "one_level", "wide", "coded"];

/// 30 rows: arm a, b and c by outcome yes and no; a numeric column; a column
/// of one level; a column of 51 levels over 51 rows past the 30; and a
/// column of yes and no where every "no" is the code 9.
fn rows() -> Vec<Vec<C<'static>>> {
    const ARMS: [&str; 3] = ["a", "b", "c"];
    const WIDE: [&str; 51] = [
        "w00", "w01", "w02", "w03", "w04", "w05", "w06", "w07", "w08", "w09", "w10", "w11", "w12",
        "w13", "w14", "w15", "w16", "w17", "w18", "w19", "w20", "w21", "w22", "w23", "w24", "w25",
        "w26", "w27", "w28", "w29", "w30", "w31", "w32", "w33", "w34", "w35", "w36", "w37", "w38",
        "w39", "w40", "w41", "w42", "w43", "w44", "w45", "w46", "w47", "w48", "w49", "w50",
    ];
    (0..51)
        .map(|i| {
            let in_table = i < 30;
            let yes = i % 4 == 0 || (i % 3 == 0 && i < 15);
            vec![
                if in_table { C::T(ARMS[i % 3]) } else { C::Empty },
                if in_table { C::T(if yes { "yes" } else { "no" }) } else { C::Empty },
                C::N(["1.5", "2", "3.25", "4"][i % 4]),
                C::T("only"),
                C::T(WIDE[i]),
                if yes { C::T("yes") } else { C::N("9") },
            ]
        })
        .collect()
}

fn csv() -> Vec<u8> {
    delimited(&HEADERS, &rows(), ",", "\n")
}

struct World {
    _dir: TempDir,
    _app: tauri::App<tauri::test::MockRuntime>,
    w: tauri::WebviewWindow<tauri::test::MockRuntime>,
    data: std::path::PathBuf,
}

fn world(runs: Option<Arc<Runs>>) -> World {
    let dir = TempDir::new("xtab");
    let data = dir.write("data.csv", &csv());
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

fn open(w: &World) -> Value {
    call(
        &w.w,
        "stats_desc_open",
        json!({ "path": w.data.to_string_lossy() }),
    )
    .expect("open")
}

/// The analysis the flow runs: arm by outcome.
fn full(o: &Value, op: u64) -> Value {
    xspec(o, op, "C1", "C2", &[])
}

fn start(w: &World, o: &Value, spec: Value) -> Result<u64, Value> {
    call(
        &w.w,
        "stats_xtab_run",
        json!({ "slot": o["slot"], "spec": spec }),
    )
    .map(|v| v["runId"].as_u64().expect("runId"))
}

fn code_of(err: &Value) -> String {
    refusal(err)["code"].as_str().unwrap().to_string()
}

/// JSON text and back, as the view went: the bit-for-bit claim rests on the
/// output hash and the CSV bytes.
fn through_text<T: serde::Serialize>(v: &T) -> Value {
    serde_json::from_str(&serde_json::to_string(v).unwrap()).unwrap()
}

fn direct_params() -> CrosstabsParams {
    CrosstabsParams {
        rows: Alias(ColumnAlias(1)),
        columns: Alias(ColumnAlias(2)),
        missing_codes: vec![],
    }
}

/// The flow's output hash, pinned on macOS aarch64 (9 Nov 2026): the run the
/// app holds is the module's, and the module's is this one. Its p values come
/// through the platform's maths library, as the export's pin says, so another
/// platform checks only that the app's hash is the module's, until its bytes
/// are measured.
const FLOW_OUTPUT_SHA256: &str = "9b414b9b2125d10fe87cff4ffa06559f72c2fab4347876d5b4b5f88a453381a2";

/* ---- the flow --------------------------------------------------------------- */

#[test]
fn open_validate_run_status_result_export_close_through_ipc() {
    let w = world(None);
    let o = open(&w);
    let dataset = load_dataset(&w.data).expect("the same file");

    let v = call(
        &w.w,
        "stats_xtab_validate",
        json!({ "slot": o["slot"], "spec": full(&o, 1) }),
    )
    .unwrap();
    assert_eq!(v["status"], "valid", "{v}");
    assert_eq!(v["errors"], json!([]));
    assert_eq!(v["missingInformation"], json!([]));

    let run = start(&w, &o, full(&o, 2)).unwrap();
    let s = until_terminal_xtab(&w.w, run);
    assert_eq!(s["state"], "succeeded", "{s}");
    assert_eq!(s["cancelRequested"], false);
    // One check point, before the arithmetic, and a result has done it.
    assert_eq!(s["stepsTotal"], 1, "{s}");
    assert_eq!(s["stepsDone"], 1, "{s}");
    assert!(s.get("columnsDone").is_none(), "steps, never columns: {s}");

    let r = call(&w.w, "stats_xtab_result", json!({ "runId": run })).unwrap();
    assert_eq!(r["runId"], run);
    assert_eq!(r["status"], "succeeded");
    let direct = stats_xtab::compute(&dataset, &direct_params()).expect("computes");
    assert_eq!(r["result"], through_text(&direct));
    assert_eq!(
        r["result"]["limitations"],
        json!([
            "stats.xtab.no_layers",
            "stats.xtab.fisher_two_by_two_only",
            "stats.xtab.no_further_nominal_measures",
            "stats.xtab.no_weights",
            "stats.xtab.code_point_order",
        ])
    );
    // No warning from the validation, so the bundle's diagnostics are empty.
    assert_eq!(r["diagnostics"], json!([]));
    let m = &r["manifest"];
    assert_eq!(m["capabilityId"], "stats.analyze.crosstabs");
    assert_eq!(
        m["outputHash"],
        ContentHash::of_bytes(&direct.canonical_bytes()).as_hex(),
        "bit for bit: the result the app holds is the module's"
    );
    if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
        assert_eq!(m["outputHash"], FLOW_OUTPUT_SHA256, "the pinned output hash");
    }
    assert_eq!(m["rowsUsed"], 30);
    assert_eq!(m["rowsRead"], 51);

    // The exports: the CSV is the module's export of that result and its
    // manifest, and the hash returned is the bytes'.
    let manifest: RunManifest<XtabCode> = serde_json::from_value(m.clone()).unwrap();
    let e = call(
        &w.w,
        "stats_xtab_export",
        json!({ "runId": run, "format": "csv" }),
    )
    .unwrap();
    let bytes: Vec<u8> = serde_json::from_value(e["bytes"].clone()).unwrap();
    assert_eq!(bytes, result_csv(&direct, &manifest));
    assert_eq!(e["sha256"], ContentHash::of_bytes(&bytes).as_hex());
    let j = call(
        &w.w,
        "stats_xtab_export",
        json!({ "runId": run, "format": "manifestJson" }),
    )
    .unwrap();
    let jb: Vec<u8> = serde_json::from_value(j["bytes"].clone()).unwrap();
    assert_eq!(jb, manifest_json(&manifest));

    assert_eq!(
        call(&w.w, "stats_desc_close", json!({ "slot": o["slot"] })).unwrap(),
        true
    );
}

/* ---- refusals --------------------------------------------------------------- */

#[test]
fn each_refusal_has_its_code() {
    let w = world(None);
    let o = open(&w);
    let validate = |spec: Value| {
        call(
            &w.w,
            "stats_xtab_validate",
            json!({ "slot": o["slot"], "spec": spec }),
        )
        .unwrap()
    };
    for (spec, code) in [
        (xspec(&o, 1, "C1", "C1", &[]), "stats.xtab.same_column"),
        (xspec(&o, 2, "C4", "C2", &[]), "stats.xtab.too_few_levels"),
        (xspec(&o, 3, "C5", "C3", &[]), "stats.xtab.too_many_levels"),
        (xspec(&o, 4, "C9", "C2", &[]), "stats.xtab.unknown_column"),
        (xspec(&o, 5, "C1", "C6", &[9.0]), "stats.xtab.too_few_levels"),
        (
            xspec(&o, 6, "C1", "C2", &[9.0, 1.0]),
            "stats.xtab.missing_codes_out_of_order",
        ),
        (
            xspec(&o, 7, "C1", "C2", &[9.0, 9.0]),
            "stats.xtab.missing_code_repeated",
        ),
    ] {
        let v = validate(spec);
        assert_eq!(v["status"], "invalid", "{code}: {v}");
        assert_eq!(v["errors"][0]["code"], code, "{v}");
    }
    // The 51 levels' refusal names the column and the counts, never a label.
    let v = validate(xspec(&o, 8, "C5", "C3", &[]));
    assert_eq!(
        v["errors"][0]["params"],
        json!({ "column": 5, "count": 51, "limit": 50 })
    );
    // The coded column: without its code, two levels (9 and yes), and it
    // is valid.
    let v = validate(xspec(&o, 9, "C1", "C6", &[]));
    assert_eq!(v["status"], "valid", "{v}");
    // A refused run, as a view: the run is made and refused by its check.
    let bad = start(&w, &o, xspec(&o, 10, "C1", "C1", &[])).unwrap();
    let s = until_terminal_xtab(&w.w, bad);
    assert_eq!(s["state"], "refused", "{s}");
    assert_eq!(s["diagnostics"][0]["code"], "stats.xtab.same_column");
    // The registry before the slot: a version mismatch is refused as such
    // for a slot no dataset has, as the registry words it, and makes no run.
    let mut s = full(&o, 11);
    s["capabilityVersion"] = json!(2);
    for cmd in ["stats_xtab_validate", "stats_xtab_run"] {
        let e = call(&w.w, cmd, json!({ "slot": 999, "spec": s })).unwrap_err();
        assert_eq!(code_of(&e), "capability.version_mismatch", "{cmd}");
    }
    // A spec of another capability through crosstabs' door.
    let mut t = full(&o, 12);
    t["capabilityId"] = json!("stats.describe");
    let e = call(
        &w.w,
        "stats_xtab_run",
        json!({ "slot": o["slot"], "spec": t }),
    )
    .unwrap_err();
    assert!(code_of(&e).starts_with("capability."), "{e}");
    // A slot no dataset has.
    let e = call(
        &w.w,
        "stats_xtab_validate",
        json!({ "slot": 999, "spec": full(&o, 13) }),
    )
    .unwrap_err();
    assert_eq!(code_of(&e), "run.unknown_input");
}

#[test]
fn every_command_refuses_while_statistics_is_inactive() {
    let dir = TempDir::new("xtab-inactive");
    let app_data = dir.0.join("appdata");
    std::fs::create_dir_all(&app_data).unwrap();
    let db = Arc::new(Database::in_memory().unwrap());
    let (_app, w) = app_with(
        &app_data,
        db,
        Arc::new(|| Arc::new(Runs::default())),
        Arc::new(app_lib::workspace::readiness),
    );
    let o = json!({ "slot": 1, "inputRef": { "contentHash": "ab".repeat(32), "rows": 1, "columns": 1, "semanticRole": "table" } });
    let spec = full(&o, 1);
    let mut checked = 0;
    for (cmd, body) in [
        ("stats_xtab_validate", json!({ "slot": 1, "spec": spec })),
        ("stats_xtab_run", json!({ "slot": 1, "spec": spec })),
        ("stats_xtab_status", json!({ "runId": 1 })),
        ("stats_xtab_result", json!({ "runId": 1 })),
        ("stats_xtab_cancel", json!({ "runId": 1 })),
        ("stats_xtab_export", json!({ "runId": 1, "format": "csv" })),
    ] {
        assert_eq!(
            code_of(&call(&w, cmd, body).unwrap_err()),
            "workspace.inactive",
            "{cmd}"
        );
        checked += 1;
    }
    assert_eq!(checked, 6);
}

#[test]
fn another_kinds_run_is_refused_both_ways() {
    let w = world(None);
    let o = open(&w);
    let x = start(&w, &o, full(&o, 1)).unwrap();
    until_terminal_xtab(&w.w, x);
    let d = call(
        &w.w,
        "stats_desc_run",
        json!({ "slot": o["slot"], "spec": desc_spec(&o, 2, &[("C3", "numeric")], &[]) }),
    )
    .unwrap()["runId"]
        .as_u64()
        .unwrap();
    until_terminal(&w.w, d);
    let mut checked = 0;
    for (prefix, theirs) in [("stats_desc", x), ("stats_anova", x), ("stats_xtab", d)] {
        for cmd in ["status", "result", "cancel"] {
            assert_eq!(
                code_of(
                    &call(&w.w, &format!("{prefix}_{cmd}"), json!({ "runId": theirs }))
                        .unwrap_err()
                ),
                "run.wrong_kind",
                "{prefix}_{cmd}"
            );
            checked += 1;
        }
        assert_eq!(
            code_of(
                &call(
                    &w.w,
                    &format!("{prefix}_export"),
                    json!({ "runId": theirs, "format": "csv" })
                )
                .unwrap_err()
            ),
            "run.wrong_kind",
            "{prefix}_export"
        );
        checked += 1;
    }
    assert_eq!(checked, 12);
    assert_eq!(
        code_of(&call(&w.w, "stats_xtab_status", json!({ "runId": 99 })).unwrap_err()),
        "run.unknown_run"
    );
    let s = call(&w.w, "workspace_state", json!({})).unwrap();
    assert_eq!(s["latestRun"], json!({ "runId": d, "kind": "describe" }));
    // A crosstabs run names its kind.
    let x2 = start(&w, &o, full(&o, 3)).unwrap();
    until_terminal_xtab(&w.w, x2);
    let s = call(&w.w, "workspace_state", json!({})).unwrap();
    assert_eq!(s["latestRun"], json!({ "runId": x2, "kind": "crosstabs" }));
}

/* ---- progress, cancel and the ceiling ------------------------------------------- */

/// A clock a test moves: a check point that "sleeps" moves it on.
struct TestClock(AtomicU64);
impl Clock for TestClock {
    fn now_ms(&self) -> u64 {
        self.0.load(Ordering::SeqCst)
    }
}

/// A hook that records every check point it sees.
fn recording() -> (CheckpointHook, Arc<Mutex<Vec<usize>>>) {
    let seen = Arc::new(Mutex::new(Vec::new()));
    let s = seen.clone();
    let hook: CheckpointHook = Arc::new(move |_, point| {
        if let Point::Column(i) = point {
            s.lock().unwrap().push(i);
        }
    });
    (hook, seen)
}

fn runs_with(clock: Arc<dyn Clock>, hook: CheckpointHook) -> Arc<Runs> {
    Arc::new(Runs::with_hooks(clock, Some(hook), 30_000))
}

#[test]
fn the_progress_counts_the_one_check_point() {
    let (hook, seen) = recording();
    let clock = Arc::new(TestClock(AtomicU64::new(0)));
    let w = world(Some(runs_with(clock, hook)));
    let o = open(&w);
    let run = start(&w, &o, full(&o, 1)).unwrap();
    let s = until_terminal_xtab(&w.w, run);
    assert_eq!(s["state"], "succeeded", "{s}");
    assert_eq!(stats_xtab::check_points(), 1);
    assert_eq!(s["stepsTotal"], 1);
    assert_eq!(s["stepsDone"], 1);
    assert_eq!(*seen.lock().unwrap(), [0]);
}

/// A hook that parks the worker wherever `when` says, reports the point it
/// parked at, and waits for one message on the returned sender.
fn park_when(
    when: impl Fn(Point) -> bool + Send + Sync + 'static,
) -> (CheckpointHook, Receiver<Point>, Sender<()>) {
    let (parked_tx, parked_rx) = mpsc::channel();
    let (go_tx, go_rx) = mpsc::channel::<()>();
    let go_rx = Mutex::new(go_rx);
    let hook: CheckpointHook = Arc::new(move |_, point| {
        if when(point) {
            parked_tx.send(point).ok();
            go_rx
                .lock()
                .unwrap()
                .recv_timeout(Duration::from_secs(20))
                .ok();
        }
    });
    (hook, parked_rx, go_tx)
}

#[test]
fn a_cancel_at_the_check_point_stops_the_run_with_no_result() {
    let (hook, parked, go) = park_when(|p| p == Point::Column(0));
    let clock = Arc::new(TestClock(AtomicU64::new(0)));
    let w = world(Some(runs_with(clock, hook)));
    let o = open(&w);
    let run = start(&w, &o, full(&o, 1)).unwrap();
    assert_eq!(
        parked
            .recv_timeout(Duration::from_secs(20))
            .expect("parked"),
        Point::Column(0)
    );
    let c = call(&w.w, "stats_xtab_cancel", json!({ "runId": run })).unwrap();
    assert_eq!(c["state"], "running", "{c}");
    assert_eq!(c["cancelRequested"], true);
    go.send(()).unwrap();
    let s = until_terminal_xtab(&w.w, run);
    assert_eq!(s["state"], "cancelled", "{s}");
    assert_eq!(s["stepsDone"], 0, "stopped at its one check point: {s}");
    assert_eq!(s["stepsTotal"], 1);
    for (cmd, body) in [
        ("stats_xtab_result", json!({ "runId": run })),
        (
            "stats_xtab_export",
            json!({ "runId": run, "format": "csv" }),
        ),
    ] {
        assert_eq!(
            code_of(&call(&w.w, cmd, body).unwrap_err()),
            "run.cancelled",
            "{cmd}"
        );
    }
}

#[test]
fn the_ceiling_stops_a_run_whose_check_point_sleeps_past_it() {
    // At its check point the run "sleeps" 31 s on the injected clock: the
    // manager's own ceiling, 30 s, stops it there.
    let clock = Arc::new(TestClock(AtomicU64::new(1_000)));
    let c = clock.clone();
    let hook: CheckpointHook = Arc::new(move |_, point| {
        if point == Point::Column(0) {
            c.0.fetch_add(31_000, Ordering::SeqCst);
        }
    });
    let w = world(Some(runs_with(clock, hook)));
    let o = open(&w);
    let run = start(&w, &o, full(&o, 1)).unwrap();
    let s = until_terminal_xtab(&w.w, run);
    assert_eq!(s["state"], "timed_out", "{s}");
    assert_eq!(
        code_of(&call(&w.w, "stats_xtab_result", json!({ "runId": run })).unwrap_err()),
        "run.timed_out"
    );
    // The same spec on a clock that never moves: a result.
    let (hook, _) = recording();
    let w = world(Some(runs_with(
        Arc::new(TestClock(AtomicU64::new(1_000))),
        hook,
    )));
    let o = open(&w);
    let run = start(&w, &o, full(&o, 1)).unwrap();
    assert_eq!(until_terminal_xtab(&w.w, run)["state"], "succeeded");
}
