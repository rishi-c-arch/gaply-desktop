//! **The linear regression's commands at their boundary (Research OS, Phase 9A-1,
//! decision 674)**, on `stats_related_ranks_commands.rs`'s pattern. Every call
//! goes through Tauri's IPC on the mock runtime, with the webview's argument
//! names, into the state `AppState::new` builds, and every assertion reads the
//! JSON the webview reads.
//!
//! * The flow: open → validate → run → status → result → export (CSV and
//!   manifest) → close, the numbers equal to `stats_linear_regression`'s on the
//!   same dataset, the CSV equal to its export, and one output hash pinned on
//!   macOS aarch64.
//! * The progress counts its one check point.
//! * A cancel at the check point stops the run with no result; the run
//!   manager's ceiling stops a run whose check point "sleeps" past it, on an
//!   injected clock, never 30 s of real time.
//! * Refusals, each with its exact code, the run's own two among them
//!   (`predictors_collinear`, `fit_out_of_range`), and the registry's
//!   `capability.*` codes as they are (decision 74), before the slot is read.
//!   One refusal is not sent here: `missing_code_not_finite`, since JSON has no
//!   NaN or infinity to send; `linear_regression_cases.rs` holds it.
//! * Statistics inactive: all six commands refuse `workspace.inactive`.
//! * `run.wrong_kind` both ways against the summary, and against the ANOVA's,
//!   the rank tests' and the related rank tests' commands.
//!
//! Every file is synthetic and in a folder the test creates and removes.

mod desc_support;
mod linear_regression_support;
mod prep_support;

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
use gaply_core::stats_linear_regression::export::{manifest_json, result_csv};
use gaply_core::stats_linear_regression::{self, LinearRegressionCode, LinearRegressionParams};
use gaply_core::Database;
use linear_regression_support::{app, app_with, lrspec, until_terminal_regression};

/* ---- fixtures --------------------------------------------------------------- */

const HEADERS: [&str; 11] = ["y", "x1", "x2", "note", "copy", "odd", "even", "bad", "flat", "huge", "few"];

/// 12 rows: the outcome, empty in the last row; two predictors; a text column; a
/// copy of x1 (collinear with it); two columns never both filled; one written-out
/// infinity; a constant; values whose sums of squares overflow; and a column of
/// three values.
fn rows() -> Vec<Vec<C<'static>>> {
    const Y: [&str; 11] = ["3.5", "1", "4.25", "2", "7", "3.5", "5.5", "0.5", "6", "2", "8.75"];
    const X1: [&str; 12] = ["4", "2.5", "4", "5", "6.5", "6", "8", "3", "6.25", "5.5", "9", "4.5"];
    const X2: [&str; 12] = ["5", "3", "7", "4", "9", "4.5", "9.5", "1", "7.5", "6", "10", "5.5"];
    const HUGE: [&str; 12] = ["1e200", "3e200", "2e200", "5e200", "4e200", "7e200", "6e200", "9e200", "8e200", "1.1e201", "1e201", "1.2e201"];
    const FEW: [&str; 3] = ["1", "2", "4"];
    const NOTES: [&str; 4] = ["n1", "n2", "n3", "n4"];
    (0..12)
        .map(|i| {
            vec![
                if i < 11 { C::N(Y[i]) } else { C::Empty },
                C::N(X1[i]),
                C::N(X2[i]),
                C::T(NOTES[i % 4]),
                C::N(X1[i]),
                if i % 2 == 1 { C::N(X1[i]) } else { C::Empty },
                if i % 2 == 0 { C::N(X1[i]) } else { C::Empty },
                C::N(if i == 0 { "inf" } else { "1" }),
                C::N("2"),
                C::N(HUGE[i]),
                if i < 3 { C::N(FEW[i]) } else { C::Empty },
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
    let dir = TempDir::new("linear-regression");
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
    call(&w.w, "stats_desc_open", json!({ "path": w.data.to_string_lossy() })).expect("open")
}

/// The analysis most tests run: y on x1 and x2.
fn two(o: &Value, op: u64) -> Value {
    lrspec(o, op, "C1", &["C2", "C3"], &[])
}

fn start(w: &World, o: &Value, spec: Value) -> Result<u64, Value> {
    call(&w.w, "stats_linear_regression_run", json!({ "slot": o["slot"], "spec": spec }))
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

fn direct_params(outcome: u32, predictors: &[u32]) -> LinearRegressionParams {
    LinearRegressionParams {
        outcome: Alias(ColumnAlias(outcome)),
        predictors: predictors.iter().map(|c| Alias(ColumnAlias(*c))).collect(),
        missing_codes: vec![],
    }
}

/// The flow's output hash, pinned on macOS aarch64 (11 Nov 2026): the run the
/// app holds is the module's, and the module's is this one. Its p values come
/// through the platform's maths library, as the export's pin says, so another
/// platform checks only that the app's hash is the module's, until its bytes are
/// measured.
const FLOW_OUTPUT_SHA256: &str = "06839100534e253622b68f0323d8905882b8a3dc455d40f02b99b018bc886006";

/* ---- the flow --------------------------------------------------------------- */

#[test]
fn open_validate_run_status_result_export_close_through_ipc() {
    let w = world(None);
    let o = open(&w);
    let dataset = load_dataset(&w.data).expect("the same file");

    let v = call(&w.w, "stats_linear_regression_validate", json!({ "slot": o["slot"], "spec": two(&o, 1) })).unwrap();
    assert_eq!(v["status"], "valid", "{v}");
    assert_eq!(v["errors"], json!([]));
    assert_eq!(v["missingInformation"], json!([]));
    assert_eq!(v["estimatedResources"], json!({ "rows": 12, "columns": 3, "cells": 36 }));

    let run = start(&w, &o, two(&o, 2)).unwrap();
    let s = until_terminal_regression(&w.w, run);
    assert_eq!(s["state"], "succeeded", "{s}");
    assert_eq!(s["cancelRequested"], false);
    // One check point, before the arithmetic, and a result has done it.
    assert_eq!(s["stepsTotal"], 1, "{s}");
    assert_eq!(s["stepsDone"], 1, "{s}");
    assert!(s.get("columnsDone").is_none(), "steps, never columns: {s}");

    let r = call(&w.w, "stats_linear_regression_result", json!({ "runId": run })).unwrap();
    assert_eq!(r["runId"], run);
    assert_eq!(r["status"], "succeeded");
    let direct = stats_linear_regression::compute(&dataset, &direct_params(1, &[2, 3])).expect("computes");
    assert_eq!(r["result"], through_text(&direct));
    assert_eq!(r["result"]["outcome"], "C1");
    assert_eq!(r["result"]["predictors"][0]["column"], "C2");
    assert_eq!(r["result"]["predictors"][1]["column"], "C3");
    assert_eq!(r["result"]["rowsUsed"], 11);
    assert_eq!(
        r["result"]["exclusions"],
        json!([{ "code": "stats.linear_regression.missing_value", "count": 1, "params": { "column": 1 } }])
    );
    assert_eq!(r["result"]["anova"]["residual"]["df"], 8);
    assert_eq!(
        r["result"]["limitations"],
        json!([
            "stats.linear_regression.enter_only",
            "stats.linear_regression.no_categorical_predictors",
            "stats.linear_regression.no_interaction_terms",
            "stats.linear_regression.intercept_always",
            "stats.linear_regression.listwise_only",
            "stats.linear_regression.no_robust_standard_errors",
            "stats.linear_regression.no_casewise_diagnostics",
            "stats.linear_regression.durbin_watson_row_order",
            "stats.linear_regression.intervals_95_only",
            "stats.linear_regression.no_weights",
        ])
    );
    let m = &r["manifest"];
    assert_eq!(m["capabilityId"], "stats.analyze.linear_regression");
    assert_eq!(
        m["outputHash"],
        ContentHash::of_bytes(&direct.canonical_bytes()).as_hex(),
        "bit for bit: the result the app holds is the module's"
    );
    if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
        assert_eq!(m["outputHash"], FLOW_OUTPUT_SHA256, "the pinned output hash");
    }
    assert_eq!(m["rowsUsed"], 11);
    assert_eq!(m["rowsRead"], 12);

    // The exports: the CSV is the module's export of that result and its
    // manifest, and the hash returned is the bytes'.
    let manifest: RunManifest<LinearRegressionCode> = serde_json::from_value(m.clone()).unwrap();
    let e = call(&w.w, "stats_linear_regression_export", json!({ "runId": run, "format": "csv" })).unwrap();
    let bytes: Vec<u8> = serde_json::from_value(e["bytes"].clone()).unwrap();
    assert_eq!(bytes, result_csv(&direct, &manifest));
    assert_eq!(e["sha256"], ContentHash::of_bytes(&bytes).as_hex());
    let j = call(&w.w, "stats_linear_regression_export", json!({ "runId": run, "format": "manifestJson" })).unwrap();
    let jb: Vec<u8> = serde_json::from_value(j["bytes"].clone()).unwrap();
    assert_eq!(jb, manifest_json(&manifest));

    // The predictors the other way round: the coefficients follow the list.
    let back = start(&w, &o, lrspec(&o, 3, "C1", &["C3", "C2"], &[])).unwrap();
    assert_eq!(until_terminal_regression(&w.w, back)["state"], "succeeded");
    let back = call(&w.w, "stats_linear_regression_result", json!({ "runId": back })).unwrap();
    assert_eq!(back["result"]["predictors"][0]["column"], "C3");
    let (f, b) = (&r["result"]["predictors"], &back["result"]["predictors"]);
    let close = |a: &Value, b: &Value| (a.as_f64().unwrap() - b.as_f64().unwrap()).abs() <= 1e-12 * a.as_f64().unwrap().abs().max(1.0);
    assert!(close(&f[0]["b"]["value"], &b[1]["b"]["value"]), "{f} {b}");
    assert!(close(&f[1]["b"]["value"], &b[0]["b"]["value"]), "{f} {b}");

    assert_eq!(call(&w.w, "stats_desc_close", json!({ "slot": o["slot"] })).unwrap(), true);
}

/* ---- refusals --------------------------------------------------------------- */

#[test]
fn each_refusal_has_its_code() {
    let w = world(None);
    let o = open(&w);
    let validate = |spec: Value| call(&w.w, "stats_linear_regression_validate", json!({ "slot": o["slot"], "spec": spec })).unwrap();
    let fifty_one: Vec<String> = (2..=52).map(|c| format!("C{c}")).collect();
    let fifty_one: Vec<&str> = fifty_one.iter().map(String::as_str).collect();
    let mut no_input = two(&o, 21);
    no_input["inputs"] = json!([]);
    let mut other_input = two(&o, 22);
    other_input["inputs"][0]["contentHash"] = json!("cd".repeat(32));
    let mut checked = 0;
    for (spec, code, params) in [
        (no_input, "stats.linear_regression.input_count", json!({ "count": 0 })),
        (other_input, "stats.linear_regression.input_mismatch", json!({})),
        (lrspec(&o, 1, "C1", &[], &[]), "stats.linear_regression.no_predictors", json!({})),
        (lrspec(&o, 2, "C1", &fifty_one, &[]), "stats.linear_regression.too_many_predictors", json!({ "count": 51, "limit": 50 })),
        (lrspec(&o, 3, "C1", &["C2", "C3", "C2"], &[]), "stats.linear_regression.column_repeated", json!({ "column": 2 })),
        (lrspec(&o, 4, "C1", &["C2", "C1"], &[]), "stats.linear_regression.column_repeated", json!({ "column": 1 })),
        (lrspec(&o, 5, "C1", &["C2", "C99"], &[]), "stats.linear_regression.unknown_column", json!({ "column": 99 })),
        (lrspec(&o, 6, "C1", &["C2", "C8"], &[]), "stats.linear_regression.column_non_finite", json!({ "column": 8, "count": 1 })),
        (lrspec(&o, 7, "C1", &["C2", "C4"], &[]), "stats.linear_regression.column_holds_text", json!({ "column": 4, "count": 12 })),
        (lrspec(&o, 8, "C1", &["C6", "C7"], &[]), "stats.linear_regression.no_complete_rows", json!({})),
        (lrspec(&o, 9, "C1", &["C2", "C11"], &[]), "stats.linear_regression.too_few_rows", json!({ "count": 3, "limit": 4 })),
        (lrspec(&o, 10, "C9", &["C2"], &[]), "stats.linear_regression.outcome_constant", json!({})),
        (lrspec(&o, 11, "C1", &["C2", "C9"], &[]), "stats.linear_regression.predictor_constant", json!({ "column": 9 })),
    ] {
        let v = validate(spec);
        assert_eq!(v["status"], "invalid", "{code}: {v}");
        assert_eq!(v["errors"][0]["code"], code, "{v}");
        assert_eq!(v["errors"][0]["params"], params, "{code}: {v}");
        checked += 1;
    }
    assert_eq!(checked, 13);
    // A refused run, as a view: the run is made and refused by its check.
    let bad = start(&w, &o, lrspec(&o, 12, "C1", &["C2", "C2"], &[])).unwrap();
    let s = until_terminal_regression(&w.w, bad);
    assert_eq!(s["state"], "refused", "{s}");
    assert_eq!(s["diagnostics"][0]["code"], "stats.linear_regression.column_repeated");
    // The run's own two refusals: validation passes, and the run's work refuses,
    // which the run manager records as a failed run carrying the refusal
    // (runs.rs's worker, RunEvent::Fail), as for every kind.
    for (op, predictors, code, params) in [
        (13, ["C2", "C5"], "stats.linear_regression.predictors_collinear", json!({ "column": 5 })),
        (14, ["C3", "C10"], "stats.linear_regression.fit_out_of_range", json!({})),
    ] {
        let spec = lrspec(&o, op, "C1", &predictors, &[]);
        assert_eq!(validate(spec.clone())["status"], "valid", "{code}");
        let run = start(&w, &o, spec).unwrap();
        let s = until_terminal_regression(&w.w, run);
        assert_eq!(s["state"], "failed", "{code}: {s}");
        assert_eq!(s["diagnostics"][0]["code"], code, "{s}");
        assert_eq!(s["diagnostics"][0]["params"], params, "{s}");
    }
    // The registry before the slot: a version mismatch is refused as such for a
    // slot no dataset has, as the registry words it, and makes no run.
    let mut s = two(&o, 15);
    s["capabilityVersion"] = json!(2);
    for cmd in ["stats_linear_regression_validate", "stats_linear_regression_run"] {
        let e = call(&w.w, cmd, json!({ "slot": 999, "spec": s })).unwrap_err();
        assert_eq!(code_of(&e), "capability.version_mismatch", "{cmd}");
    }
    // A spec of another capability through this door.
    for other in ["stats.describe", "stats.verify.ols", "stats.analyze.related_ranks"] {
        let mut t = two(&o, 16);
        t["capabilityId"] = json!(other);
        let e = call(&w.w, "stats_linear_regression_run", json!({ "slot": o["slot"], "spec": t })).unwrap_err();
        assert!(code_of(&e).starts_with("capability."), "{other}: {e}");
    }
    // A slot no dataset has.
    let e = call(&w.w, "stats_linear_regression_validate", json!({ "slot": 999, "spec": two(&o, 17) })).unwrap_err();
    assert_eq!(code_of(&e), "run.unknown_input");
}

/// The two refusals only a prepared table can reach, through IPC: a score made
/// with `stats_prep_make`, C12, the sum of the first three columns.
#[test]
fn a_prepared_table_refuses_a_code_on_a_score_and_a_spec_without_its_preparation() {
    let w = world(None);
    let o = open(&w);
    let recipe = prep_support::recipe(11, "total", "sum", &["C1", "C2", "C3"]);
    let view = prep_support::make(&w.w, &o["slot"], &recipe);
    let prepared = |op, predictors: &[&str], codes: &[f64]| {
        let mut s = lrspec(&view, op, "C2", predictors, codes);
        s["preparation"] = view["preparation"].clone();
        s
    };
    let validate = |spec: Value| call(&w.w, "stats_linear_regression_validate", json!({ "slot": o["slot"], "spec": spec })).unwrap();
    // A score is a predictor like any other.
    let v = validate(prepared(1, &["C3", "C12"], &[]));
    assert_eq!(v["status"], "valid", "{v}");
    // 12.5 is the first row's score, and no other's.
    let v = validate(prepared(2, &["C3", "C12"], &[12.5]));
    assert_eq!(v["status"], "invalid", "{v}");
    assert_eq!(v["errors"][0]["code"], "stats.linear_regression.code_equals_score", "{v}");
    assert_eq!(v["errors"][0]["params"], json!({ "column": 12, "count": 1 }));
    // The prepared table's spec with no preparation named.
    let v = validate(lrspec(&view, 3, "C2", &["C3", "C12"], &[]));
    assert_eq!(v["status"], "invalid", "{v}");
    assert_eq!(v["errors"][0]["code"], "stats.linear_regression.preparation_mismatch", "{v}");
}

#[test]
fn every_command_refuses_while_statistics_is_inactive() {
    let dir = TempDir::new("linear-regression-inactive");
    let app_data = dir.0.join("appdata");
    std::fs::create_dir_all(&app_data).unwrap();
    let db = Arc::new(Database::in_memory().unwrap());
    let (_app, w) = app_with(&app_data, db, Arc::new(|| Arc::new(Runs::default())), Arc::new(app_lib::workspace::readiness));
    let o = json!({ "slot": 1, "inputRef": { "contentHash": "ab".repeat(32), "rows": 1, "columns": 1, "semanticRole": "table" } });
    let spec = two(&o, 1);
    let mut checked = 0;
    for (cmd, body) in [
        ("stats_linear_regression_validate", json!({ "slot": 1, "spec": spec })),
        ("stats_linear_regression_run", json!({ "slot": 1, "spec": spec })),
        ("stats_linear_regression_status", json!({ "runId": 1 })),
        ("stats_linear_regression_result", json!({ "runId": 1 })),
        ("stats_linear_regression_cancel", json!({ "runId": 1 })),
        ("stats_linear_regression_export", json!({ "runId": 1, "format": "csv" })),
    ] {
        assert_eq!(code_of(&call(&w, cmd, body).unwrap_err()), "workspace.inactive", "{cmd}");
        checked += 1;
    }
    assert_eq!(checked, 6);
}

#[test]
fn another_kinds_run_is_refused_both_ways() {
    let w = world(None);
    let o = open(&w);
    let k = start(&w, &o, two(&o, 1)).unwrap();
    until_terminal_regression(&w.w, k);
    let d = call(&w.w, "stats_desc_run", json!({ "slot": o["slot"], "spec": desc_spec(&o, 2, &[("C1", "numeric")], &[]) }))
        .unwrap()["runId"]
        .as_u64()
        .unwrap();
    until_terminal(&w.w, d);
    let mut checked = 0;
    for (prefix, theirs) in [
        ("stats_desc", k),
        ("stats_anova", k),
        ("stats_ranks", k),
        ("stats_related_ranks", k),
        ("stats_linear_regression", d),
    ] {
        for cmd in ["status", "result", "cancel"] {
            assert_eq!(
                code_of(&call(&w.w, &format!("{prefix}_{cmd}"), json!({ "runId": theirs })).unwrap_err()),
                "run.wrong_kind",
                "{prefix}_{cmd}"
            );
            checked += 1;
        }
        assert_eq!(
            code_of(&call(&w.w, &format!("{prefix}_export"), json!({ "runId": theirs, "format": "csv" })).unwrap_err()),
            "run.wrong_kind",
            "{prefix}_export"
        );
        checked += 1;
    }
    assert_eq!(checked, 20);
    assert_eq!(
        code_of(&call(&w.w, "stats_linear_regression_status", json!({ "runId": 99 })).unwrap_err()),
        "run.unknown_run"
    );
    let s = call(&w.w, "workspace_state", json!({})).unwrap();
    assert_eq!(s["latestRun"], json!({ "runId": d, "kind": "describe" }));
    // A linear regression run names its kind.
    let k2 = start(&w, &o, two(&o, 3)).unwrap();
    until_terminal_regression(&w.w, k2);
    let s = call(&w.w, "workspace_state", json!({})).unwrap();
    assert_eq!(s["latestRun"], json!({ "runId": k2, "kind": "linear_regression" }));
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
    let run = start(&w, &o, two(&o, 1)).unwrap();
    let s = until_terminal_regression(&w.w, run);
    assert_eq!(s["state"], "succeeded", "{s}");
    assert_eq!(stats_linear_regression::check_points(), 1);
    assert_eq!(s["stepsTotal"], 1);
    assert_eq!(s["stepsDone"], 1);
    assert_eq!(*seen.lock().unwrap(), [0]);
}

/// A hook that parks the worker wherever `when` says, reports the point it
/// parked at, and waits for one message on the returned sender.
fn park_when(when: impl Fn(Point) -> bool + Send + Sync + 'static) -> (CheckpointHook, Receiver<Point>, Sender<()>) {
    let (parked_tx, parked_rx) = mpsc::channel();
    let (go_tx, go_rx) = mpsc::channel::<()>();
    let go_rx = Mutex::new(go_rx);
    let hook: CheckpointHook = Arc::new(move |_, point| {
        if when(point) {
            parked_tx.send(point).ok();
            go_rx.lock().unwrap().recv_timeout(Duration::from_secs(20)).ok();
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
    let run = start(&w, &o, two(&o, 1)).unwrap();
    assert_eq!(parked.recv_timeout(Duration::from_secs(20)).expect("parked"), Point::Column(0));
    let c = call(&w.w, "stats_linear_regression_cancel", json!({ "runId": run })).unwrap();
    assert_eq!(c["state"], "running", "{c}");
    assert_eq!(c["cancelRequested"], true);
    go.send(()).unwrap();
    let s = until_terminal_regression(&w.w, run);
    assert_eq!(s["state"], "cancelled", "{s}");
    assert_eq!(s["stepsDone"], 0, "stopped at its one check point: {s}");
    assert_eq!(s["stepsTotal"], 1);
    for (cmd, body) in [
        ("stats_linear_regression_result", json!({ "runId": run })),
        ("stats_linear_regression_export", json!({ "runId": run, "format": "csv" })),
    ] {
        assert_eq!(code_of(&call(&w.w, cmd, body).unwrap_err()), "run.cancelled", "{cmd}");
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
    let run = start(&w, &o, two(&o, 1)).unwrap();
    let s = until_terminal_regression(&w.w, run);
    assert_eq!(s["state"], "timed_out", "{s}");
    assert_eq!(
        code_of(&call(&w.w, "stats_linear_regression_result", json!({ "runId": run })).unwrap_err()),
        "run.timed_out"
    );
    // The same spec on a clock that never moves: a result.
    let (hook, _) = recording();
    let w = world(Some(runs_with(Arc::new(TestClock(AtomicU64::new(1_000))), hook)));
    let o = open(&w);
    let run = start(&w, &o, two(&o, 1)).unwrap();
    assert_eq!(until_terminal_regression(&w.w, run)["state"], "succeeded");
}
