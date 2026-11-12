//! **The factor analysis's commands at their boundary (Research OS, Phase 9B-1,
//! decision 704)**, on `stats_linear_regression_commands.rs`'s pattern. Every
//! call goes through Tauri's IPC on the mock runtime, with the webview's argument
//! names, into the state `AppState::new` builds, and every assertion reads the
//! JSON the webview reads.
//!
//! * The flow: open → validate → run → status → result → export (CSV and
//!   manifest) → close, the numbers equal to `stats_factor`'s on the same dataset
//!   and the CSV equal to its export.
//! * The progress counts every check point, `stats_factor::check_points`.
//! * A cancel mid-correlations and a cancel during principal axis iterations
//!   each stop the run with no result; the run manager's ceiling stops a run
//!   whose extraction iterations "sleep" past it, on an injected clock, never
//!   30 s of real time, and the same spec on a still clock gives a result.
//! * Refusals, each with its exact code, the run's own among them
//!   (`correlation_out_of_range`, `no_eigenvalue_over_one`,
//!   `correlations_singular`, `extraction_not_converged`), and the registry's
//!   `capability.*` codes as they are (decision 74), before the slot is read.
//!   One refusal is not sent here: `missing_code_not_finite`, since JSON has no
//!   NaN or infinity to send; `factor_cases.rs` holds it.
//! * Statistics inactive: all six commands refuse `workspace.inactive`.
//! * `run.wrong_kind` both ways against the summary, and against the ANOVA's,
//!   the rank tests', the related rank tests' and the linear regression's
//!   commands.
//!
//! Every file is synthetic and in a folder the test creates and removes.

mod desc_support;
mod factor_support;
mod prep_support;

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::{json, Value};

use app_lib::runs::{CheckpointHook, Clock, Point, Runs};
use app_lib::stats_l1_ingest::load_dataset;
use desc_support::{call, delimited, refusal, spec as desc_spec, until_terminal, TempDir, C};
use factor_support::{app, app_with, fspec, until_terminal_factor};
use gaply_core::contracts::{ContentHash, RunManifest};
use gaply_core::stats_desc::Alias;
use gaply_core::stats_factor::export::{manifest_json, result_csv};
use gaply_core::stats_factor::{self, Extraction, FactorCode, FactorParams, Rotation};
use gaply_core::stats_l1::dataset::ColumnAlias;
use gaply_core::Database;

/* ---- fixtures --------------------------------------------------------------- */

const HEADERS: [&str; 17] = [
    "a", "b", "c", "d", "e", "f", "note", "sum", "bad", "flat", "huge", "few", "h1", "h2", "h3", "odd", "even",
];

/// 40 rows: six items on two factors (an integer formula); a text column; the sum
/// of the first two items (exactly singular with them); one written-out infinity;
/// a constant; the first item times 1e300, whose correlations overflow; a column
/// of two values; three columns of a Hadamard matrix of order 8, repeated (exactly
/// uncorrelated, every eigenvalue 1); and two columns never both filled.
fn rows() -> Vec<Vec<String>> {
    let item = |i: u64, j: u64| {
        let f = (j - 1) % 2;
        let score = ((i * 7907 + f * 15485863) % 1013) as i64 - 506;
        let noise = ((i * 104729 + j * 7919 + i * j * 31) % 997) as i64 - 498;
        2 * score + noise
    };
    let had = [[1, 1, 1], [-1, 1, 1], [1, -1, 1], [-1, -1, 1], [1, 1, -1], [-1, 1, -1], [1, -1, -1], [-1, -1, -1]];
    (1..=40u64)
        .map(|i| {
            let mut r: Vec<String> = (1..=6).map(|j| item(i, j).to_string()).collect();
            r.push(format!("n{}", i % 4));
            r.push((item(i, 1) + item(i, 2)).to_string());
            r.push(if i == 1 { "inf".into() } else { "1".into() });
            r.push("2".into());
            r.push(format!("{}e300", item(i, 1)));
            r.push(if i <= 2 { i.to_string() } else { String::new() });
            for k in 0..3 {
                r.push(had[(i as usize - 1) % 8][k].to_string());
            }
            r.push(if i % 2 == 1 { item(i, 1).to_string() } else { String::new() });
            r.push(if i % 2 == 0 { item(i, 1).to_string() } else { String::new() });
            r
        })
        .collect()
}

fn csv() -> Vec<u8> {
    let owned = rows();
    let rows: Vec<Vec<C>> = owned
        .iter()
        .map(|r| {
            r.iter()
                .enumerate()
                .map(|(j, c)| if c.is_empty() { C::Empty } else if j == 6 { C::T(c.as_str()) } else { C::N(c.as_str()) })
                .collect()
        })
        .collect();
    delimited(&HEADERS, &rows, ",", "\n")
}

struct World {
    _dir: TempDir,
    _app: tauri::App<tauri::test::MockRuntime>,
    w: tauri::WebviewWindow<tauri::test::MockRuntime>,
    data: std::path::PathBuf,
}

fn world(runs: Option<Arc<Runs>>) -> World {
    let dir = TempDir::new("factor");
    let data = dir.write("data.csv", &csv());
    let app_data = dir.0.join("appdata");
    std::fs::create_dir_all(&app_data).unwrap();
    let db = Arc::new(Database::in_memory().unwrap());
    let (app, w) = app(&app_data, db, runs);
    World { _dir: dir, _app: app, w, data }
}

fn open(w: &World) -> Value {
    call(&w.w, "stats_desc_open", json!({ "path": w.data.to_string_lossy() })).expect("open")
}

const SIX: [&str; 6] = ["C1", "C2", "C3", "C4", "C5", "C6"];

/// The analysis most tests run: principal axis, two factors, promax.
fn six(o: &Value, op: u64) -> Value {
    fspec(o, op, &SIX, "principal_axis", json!(2), "promax", &[])
}

fn start(w: &World, o: &Value, spec: Value) -> Result<u64, Value> {
    call(&w.w, "stats_factor_run", json!({ "slot": o["slot"], "spec": spec })).map(|v| v["runId"].as_u64().expect("runId"))
}

fn code_of(err: &Value) -> String {
    refusal(err)["code"].as_str().unwrap().to_string()
}

/// JSON text and back, as the view went.
fn through_text<T: serde::Serialize>(v: &T) -> Value {
    serde_json::from_str(&serde_json::to_string(v).unwrap()).unwrap()
}

fn direct_params() -> FactorParams {
    FactorParams {
        items: (1..=6).map(|j| Alias(ColumnAlias(j))).collect(),
        extraction: Extraction::PrincipalAxis,
        factors: Some(2),
        rotation: Rotation::Promax,
        iterations: 25,
        convergence: 0.001,
        missing_codes: vec![],
    }
}

/// The check point before the extraction for six items: 1 + 6 + 15.
const EXTRACTION_POINT: usize = 22;

/* ---- the flow --------------------------------------------------------------- */

#[test]
fn open_validate_run_status_result_export_close_through_ipc() {
    let w = world(None);
    let o = open(&w);
    let dataset = load_dataset(&w.data).expect("the same file");

    let v = call(&w.w, "stats_factor_validate", json!({ "slot": o["slot"], "spec": six(&o, 1) })).unwrap();
    assert_eq!(v["status"], "valid", "{v}");
    assert_eq!(v["errors"], json!([]));
    assert_eq!(v["estimatedResources"], json!({ "rows": 40, "columns": 6, "cells": 240 }));

    let run = start(&w, &o, six(&o, 2)).unwrap();
    let s = until_terminal_factor(&w.w, run);
    assert_eq!(s["state"], "succeeded", "{s}");
    assert_eq!(s["stepsTotal"], stats_factor::check_points(&direct_params()) as u64, "{s}");
    assert_eq!(s["stepsDone"], s["stepsTotal"], "{s}");

    let r = call(&w.w, "stats_factor_result", json!({ "runId": run })).unwrap();
    assert_eq!((r["runId"].as_u64(), r["status"].as_str()), (Some(run), Some("succeeded")));
    let direct = stats_factor::compute(&dataset, &direct_params()).expect("computes");
    assert_eq!(r["result"], through_text(&direct));
    assert_eq!(r["result"]["items"], json!(SIX));
    assert_eq!(r["result"]["rowsUsed"], 40);
    assert_eq!(r["result"]["rotation"]["method"], "promax");
    assert_eq!(r["result"]["limitations"].as_array().unwrap().len(), 9);
    let m = &r["manifest"];
    assert_eq!(m["capabilityId"], "stats.analyze.factor");
    assert_eq!(m["outputHash"], ContentHash::of_bytes(&direct.canonical_bytes()).as_hex(), "bit for bit");
    assert_eq!((m["rowsUsed"].as_u64(), m["rowsRead"].as_u64()), (Some(40), Some(40)));

    let manifest: RunManifest<FactorCode> = serde_json::from_value(m.clone()).unwrap();
    let e = call(&w.w, "stats_factor_export", json!({ "runId": run, "format": "csv" })).unwrap();
    let bytes: Vec<u8> = serde_json::from_value(e["bytes"].clone()).unwrap();
    assert_eq!(bytes, result_csv(&direct, &manifest));
    assert_eq!(e["sha256"], ContentHash::of_bytes(&bytes).as_hex());
    let j = call(&w.w, "stats_factor_export", json!({ "runId": run, "format": "manifestJson" })).unwrap();
    let jb: Vec<u8> = serde_json::from_value(j["bytes"].clone()).unwrap();
    assert_eq!(jb, manifest_json(&manifest));

    assert_eq!(call(&w.w, "stats_desc_close", json!({ "slot": o["slot"] })).unwrap(), true);
}

/* ---- refusals --------------------------------------------------------------- */

#[test]
fn each_refusal_has_its_code() {
    let w = world(None);
    let o = open(&w);
    let validate = |spec: Value| call(&w.w, "stats_factor_validate", json!({ "slot": o["slot"], "spec": spec })).unwrap();
    let pc = |op, items: &[&str], factors: Value| fspec(&o, op, items, "principal_components", factors, "none", &[]);
    let mut no_input = six(&o, 21);
    no_input["inputs"] = json!([]);
    let mut other_input = six(&o, 22);
    other_input["inputs"][0]["contentHash"] = json!("cd".repeat(32));
    let mut iterations = six(&o, 23);
    iterations["parameters"]["iterations"] = json!(0);
    let mut convergence = six(&o, 24);
    convergence["parameters"]["convergence"] = json!(0.5);
    let mut checked = 0;
    for (spec, code, params) in [
        (no_input, "stats.factor.input_count", json!({ "count": 0 })),
        (other_input, "stats.factor.input_mismatch", json!({})),
        (pc(1, &["C1", "C2"], json!(null)), "stats.factor.too_few_items", json!({ "count": 2, "limit": 3 })),
        (pc(2, &["C2", "C1", "C3"], json!(null)), "stats.factor.items_out_of_order", json!({ "column": 1 })),
        (pc(3, &["C1", "C2", "C2"], json!(null)), "stats.factor.item_repeated", json!({ "column": 2 })),
        (pc(4, &["C1", "C2", "C99"], json!(null)), "stats.factor.unknown_column", json!({ "column": 99 })),
        (pc(5, &["C1", "C2", "C9"], json!(null)), "stats.factor.column_non_finite", json!({ "column": 9, "count": 1 })),
        (pc(6, &["C1", "C2", "C7"], json!(null)), "stats.factor.column_holds_text", json!({ "column": 7, "count": 40 })),
        (pc(7, &["C1", "C2", "C3"], json!(4)), "stats.factor.factors_out_of_range", json!({ "count": 4, "limit": 3 })),
        (iterations, "stats.factor.iterations_out_of_range", json!({ "count": 0, "limit": 1000 })),
        (convergence, "stats.factor.convergence_out_of_range", json!({})),
        (pc(8, &["C1", "C16", "C17"], json!(null)), "stats.factor.no_complete_rows", json!({})),
        (pc(9, &["C1", "C2", "C12"], json!(null)), "stats.factor.too_few_rows", json!({ "count": 2, "limit": 3 })),
        (pc(10, &["C1", "C2", "C10"], json!(null)), "stats.factor.item_constant", json!({ "column": 10 })),
    ] {
        let v = validate(spec);
        assert_eq!(v["status"], "invalid", "{code}: {v}");
        assert_eq!(v["errors"][0]["code"], code, "{v}");
        assert_eq!(v["errors"][0]["params"], params, "{code}: {v}");
        checked += 1;
    }
    assert_eq!(checked, 14);
    // A refused run, as a view: the run is made and refused by its check.
    let bad = start(&w, &o, pc(11, &["C1", "C1", "C2"], json!(null))).unwrap();
    let s = until_terminal_factor(&w.w, bad);
    assert_eq!(s["state"], "refused", "{s}");
    assert_eq!(s["diagnostics"][0]["code"], "stats.factor.item_repeated");
    // The run's own refusals: validation passes, and the run's work refuses,
    // recorded as a failed run carrying the refusal, as for every kind.
    let mut few_iterations = six(&o, 15);
    few_iterations["parameters"]["iterations"] = json!(1);
    few_iterations["parameters"]["convergence"] = json!(1e-12);
    for (spec, code, params) in [
        // C11 is the item whose arithmetic overflows (decision 714), though the
        // pair (C1, C11) is the one that fails first.
        (pc(12, &["C1", "C2", "C11"], json!(null)), "stats.factor.correlation_out_of_range", json!({ "column": 11 })),
        (pc(13, &["C13", "C14", "C15"], json!(null)), "stats.factor.no_eigenvalue_over_one", json!({})),
        (fspec(&o, 14, &["C1", "C2", "C8"], "principal_axis", json!(1), "none", &[]), "stats.factor.correlations_singular", json!({ "column": 8 })),
        (few_iterations, "stats.factor.extraction_not_converged", json!({ "limit": 1 })),
    ] {
        assert_eq!(validate(spec.clone())["status"], "valid", "{code}");
        let run = start(&w, &o, spec).unwrap();
        let s = until_terminal_factor(&w.w, run);
        assert_eq!(s["state"], "failed", "{code}: {s}");
        assert_eq!(s["diagnostics"][0]["code"], code, "{s}");
        assert_eq!(s["diagnostics"][0]["params"], params, "{s}");
    }
    // The registry before the slot.
    let mut s = six(&o, 16);
    s["capabilityVersion"] = json!(2);
    for cmd in ["stats_factor_validate", "stats_factor_run"] {
        let e = call(&w.w, cmd, json!({ "slot": 999, "spec": s })).unwrap_err();
        assert_eq!(code_of(&e), "capability.version_mismatch", "{cmd}");
    }
    for other in ["stats.describe", "stats.verify.ols", "stats.analyze.linear_regression"] {
        let mut t = six(&o, 17);
        t["capabilityId"] = json!(other);
        let e = call(&w.w, "stats_factor_run", json!({ "slot": o["slot"], "spec": t })).unwrap_err();
        assert!(code_of(&e).starts_with("capability."), "{other}: {e}");
    }
    let e = call(&w.w, "stats_factor_validate", json!({ "slot": 999, "spec": six(&o, 18) })).unwrap_err();
    assert_eq!(code_of(&e), "run.unknown_input");
}

/// The two refusals only a prepared table can reach, through IPC: a score made
/// with `stats_prep_make`, C18, the sum of the first three columns.
#[test]
fn a_prepared_table_refuses_a_code_on_a_score_and_a_spec_without_its_preparation() {
    let w = world(None);
    let o = open(&w);
    let recipe = prep_support::recipe(17, "total", "sum", &["C1", "C2", "C3"]);
    let view = prep_support::make(&w.w, &o["slot"], &recipe);
    let score: i64 = rows()[0][..3].iter().map(|c| c.parse::<i64>().unwrap()).sum();
    let prepared = |op, codes: &[f64]| {
        let mut s = fspec(&view, op, &["C4", "C5", "C18"], "principal_components", json!(1), "none", codes);
        s["preparation"] = view["preparation"].clone();
        s
    };
    let validate = |spec: Value| call(&w.w, "stats_factor_validate", json!({ "slot": o["slot"], "spec": spec })).unwrap();
    let v = validate(prepared(1, &[]));
    assert_eq!(v["status"], "valid", "a score is an item like any other: {v}");
    let v = validate(prepared(2, &[score as f64]));
    assert_eq!(v["status"], "invalid", "{v}");
    assert_eq!(v["errors"][0]["code"], "stats.factor.code_equals_score", "{v}");
    assert_eq!(v["errors"][0]["params"]["column"], 18);
    let v = validate(fspec(&view, 3, &["C4", "C5", "C18"], "principal_components", json!(1), "none", &[]));
    assert_eq!(v["errors"][0]["code"], "stats.factor.preparation_mismatch", "{v}");
}

#[test]
fn every_command_refuses_while_statistics_is_inactive() {
    let dir = TempDir::new("factor-inactive");
    let app_data = dir.0.join("appdata");
    std::fs::create_dir_all(&app_data).unwrap();
    let db = Arc::new(Database::in_memory().unwrap());
    let (_app, w) = app_with(&app_data, db, Arc::new(|| Arc::new(Runs::default())), Arc::new(app_lib::workspace::readiness));
    let o = json!({ "slot": 1, "inputRef": { "contentHash": "ab".repeat(32), "rows": 1, "columns": 1, "semanticRole": "table" } });
    let spec = six(&o, 1);
    let mut checked = 0;
    for (cmd, body) in [
        ("stats_factor_validate", json!({ "slot": 1, "spec": spec })),
        ("stats_factor_run", json!({ "slot": 1, "spec": spec })),
        ("stats_factor_status", json!({ "runId": 1 })),
        ("stats_factor_result", json!({ "runId": 1 })),
        ("stats_factor_cancel", json!({ "runId": 1 })),
        ("stats_factor_export", json!({ "runId": 1, "format": "csv" })),
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
    let k = start(&w, &o, six(&o, 1)).unwrap();
    until_terminal_factor(&w.w, k);
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
        ("stats_linear_regression", k),
        ("stats_factor", d),
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
    assert_eq!(checked, 24);
    assert_eq!(code_of(&call(&w.w, "stats_factor_status", json!({ "runId": 99 })).unwrap_err()), "run.unknown_run");
    let k2 = start(&w, &o, six(&o, 3)).unwrap();
    until_terminal_factor(&w.w, k2);
    let s = call(&w.w, "workspace_state", json!({})).unwrap();
    assert_eq!(s["latestRun"], json!({ "runId": k2, "kind": "factor" }));
}

/* ---- progress, cancel and the ceiling ------------------------------------------- */

/// A clock a test moves.
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
fn the_progress_counts_every_check_point_and_the_iterations_repeat_theirs() {
    let (hook, seen) = recording();
    let w = world(Some(runs_with(Arc::new(TestClock(AtomicU64::new(0))), hook)));
    let o = open(&w);
    let run = start(&w, &o, six(&o, 1)).unwrap();
    let s = until_terminal_factor(&w.w, run);
    assert_eq!(s["state"], "succeeded", "{s}");
    assert_eq!(s["stepsTotal"], 24);
    assert_eq!(s["stepsDone"], 24);
    let seen = seen.lock().unwrap().clone();
    let mut distinct = seen.clone();
    distinct.dedup();
    assert_eq!(distinct, (0..24).collect::<Vec<usize>>());
    assert!(seen.iter().filter(|i| **i == EXTRACTION_POINT).count() >= 2, "the extraction's iterations call its point: {seen:?}");
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

fn cancel_at(when: impl Fn(Point) -> bool + Send + Sync + 'static, at: Point, steps_done: u64) {
    let (hook, parked, go) = park_when(when);
    let w = world(Some(runs_with(Arc::new(TestClock(AtomicU64::new(0))), hook)));
    let o = open(&w);
    let run = start(&w, &o, six(&o, 1)).unwrap();
    assert_eq!(parked.recv_timeout(Duration::from_secs(20)).expect("parked"), at);
    let c = call(&w.w, "stats_factor_cancel", json!({ "runId": run })).unwrap();
    assert_eq!((c["state"].as_str(), c["cancelRequested"].as_bool()), (Some("running"), Some(true)), "{c}");
    go.send(()).unwrap();
    let s = until_terminal_factor(&w.w, run);
    assert_eq!(s["state"], "cancelled", "{s}");
    assert_eq!(s["stepsDone"], steps_done, "{s}");
    for (cmd, body) in [
        ("stats_factor_result", json!({ "runId": run })),
        ("stats_factor_export", json!({ "runId": run, "format": "csv" })),
    ] {
        assert_eq!(code_of(&call(&w.w, cmd, body).unwrap_err()), "run.cancelled", "{cmd}");
    }
}

#[test]
fn a_cancel_mid_correlations_stops_the_run_with_no_result() {
    // The fifth correlation's check point, 1 + 6 + 4.
    cancel_at(|p| p == Point::Column(11), Point::Column(11), 11);
}

/// Decision 715: every varimax cycle calls the rotation's check point at its
/// unchanged count, so a cancel at the point's second call (the first cycle,
/// after the call before the rotation) stops the run there with no result.
#[test]
fn a_cancel_during_the_rotations_cycles_stops_the_run_with_no_result() {
    let rotation_point = EXTRACTION_POINT + 1;
    let calls = Arc::new(AtomicU64::new(0));
    let c = calls.clone();
    cancel_at(
        move |p| p == Point::Column(rotation_point) && c.fetch_add(1, Ordering::SeqCst) == 1,
        Point::Column(rotation_point),
        rotation_point as u64,
    );
}

#[test]
fn a_cancel_during_principal_axis_iterations_stops_the_run_with_no_result() {
    // The extraction point's third call: its second iteration.
    let calls = Arc::new(AtomicU64::new(0));
    let c = calls.clone();
    cancel_at(
        move |p| p == Point::Column(EXTRACTION_POINT) && c.fetch_add(1, Ordering::SeqCst) == 2,
        Point::Column(EXTRACTION_POINT),
        EXTRACTION_POINT as u64,
    );
}

#[test]
fn the_ceiling_stops_a_run_whose_extraction_iterations_sleep_past_it() {
    // Each call of the extraction's check point "sleeps" 11 s on the injected
    // clock: the one before the extraction and the first iteration's leave 22 s,
    // the second iteration's passes the manager's 30 s ceiling. Without the
    // iterations' calls the run would sleep 11 s and end.
    let clock = Arc::new(TestClock(AtomicU64::new(1_000)));
    let c = clock.clone();
    let hook: CheckpointHook = Arc::new(move |_, point| {
        if point == Point::Column(EXTRACTION_POINT) {
            c.0.fetch_add(11_000, Ordering::SeqCst);
        }
    });
    let w = world(Some(runs_with(clock, hook)));
    let o = open(&w);
    let run = start(&w, &o, six(&o, 1)).unwrap();
    let s = until_terminal_factor(&w.w, run);
    assert_eq!(s["state"], "timed_out", "{s}");
    assert_eq!(code_of(&call(&w.w, "stats_factor_result", json!({ "runId": run })).unwrap_err()), "run.timed_out");
    // The same spec on a clock that never moves: a result, using more than two
    // iterations, so the ceiling above was reached during them.
    let (hook, _) = recording();
    let w = world(Some(runs_with(Arc::new(TestClock(AtomicU64::new(1_000))), hook)));
    let o = open(&w);
    let run = start(&w, &o, six(&o, 1)).unwrap();
    assert_eq!(until_terminal_factor(&w.w, run)["state"], "succeeded");
    let r = call(&w.w, "stats_factor_result", json!({ "runId": run })).unwrap();
    assert!(r["result"]["extraction"]["iterations"].as_u64().unwrap() >= 2, "{}", r["result"]["extraction"]);
}
