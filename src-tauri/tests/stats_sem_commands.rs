//! **The structural equation models' commands at their boundary (Research OS,
//! Phase 9C-4, decision 803)**, on `stats_cfa_commands.rs`'s pattern. Every call
//! goes through Tauri's IPC on the mock runtime, with the webview's argument
//! names, into the state `AppState::new` builds, and every assertion reads the
//! JSON the webview reads.
//!
//! * The flow: open → validate → run → status → result → export (CSV and
//!   manifest) → close, the numbers equal to `stats_sem`'s on the same dataset
//!   and the CSV equal to its export.
//! * The progress counts every check point, `stats_sem::check_points`, and the
//!   iterations and the check at the solution call the estimation's again.
//! * A cancel mid-covariances and one during the iterations each stop the run
//!   with no result; the run manager's ceiling stops a run whose iterations
//!   "sleep" past it, on an injected clock, never 30 s of real time, and the same
//!   spec on a still clock gives a result.
//! * Refusals, each with its exact code, the run's own among them
//!   (`sample_covariance_singular`, `overflow`, `sem_not_converged` on the CFA
//!   cases' Walsh columns), and the registry's `capability.*` codes as they are
//!   (decision 74), before the slot is read. Two refusals are not sent here:
//!   `missing_code_not_finite`, since JSON has no NaN or infinity to send
//!   (`sem_cases.rs` holds it), and `solution_not_identified`, which no known
//!   dataset reaches through the attempts (decision 798; `stats_sem`'s unit test
//!   holds it at a constructed point).
//! * Statistics inactive: all six commands refuse `workspace.inactive`.
//! * `run.wrong_kind` both ways against the summary, and against the ANOVA's,
//!   the rank tests', the related rank tests', the linear regression's, the
//!   factor analysis's and the confirmatory factor analysis's commands.
//!
//! Every file is synthetic and in a folder the test creates and removes.

mod desc_support;
mod prep_support;
mod sem_support;

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::{json, Value};

use app_lib::runs::{CheckpointHook, Clock, Point, Runs};
use app_lib::stats_l1_ingest::load_dataset;
use desc_support::{call, delimited, refusal, spec as desc_spec, until_terminal, TempDir, C};
use gaply_core::contracts::{ContentHash, RunManifest};
use gaply_core::stats_sem::export::{manifest_json, result_csv};
use gaply_core::stats_sem::{self, SemCode, SemParams};
use gaply_core::Database;
use sem_support::{app, app_with, factors, sspec, until_terminal_sem};

/* ---- fixtures --------------------------------------------------------------- */

const HEADERS: [&str; 17] = [
    "a", "b", "c", "d", "e", "f", "note", "sum", "bad", "flat", "huge", "few", "h1", "h2", "h3", "odd", "even",
];

/// 40 rows: six columns on two factors (cfa_r_golden.rs's integer formula, C1,
/// C3, C5 on one and C2, C4, C6 on the other); a text column; the sum of the
/// first two (exactly singular with them); one written-out infinity; a constant;
/// the first column times 1e300; a column of two values; three columns of a
/// Hadamard matrix of order 8, repeated; and two columns never both filled.
fn rows() -> Vec<Vec<String>> {
    let item = |i: u64, j: u64| {
        let f = (j - 1) % 2;
        let score = ((i * (7907 + 104729 * f)) % 1013) as i64 - 506;
        let shared = ((i * 2654435761) % 1009) as i64 - 504;
        let noise = ((i * 104729 + j * 7919 + i * j * 31) % 997) as i64 - 498;
        (2 + (j % 3) as i64) * (score + shared) + noise
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


/// The CFA cases' Walsh columns (cfa_cases.rs's the_independence_models_cmin_is_never_below_0):
/// 32 rows of three columns, on which every start puts the loadings at exactly 0,
/// so one factor of the three refuses as sem_not_converged at the cap (decision
/// 798; sem_cases.rs).
fn walsh() -> Vec<u8> {
    let cols = [
        [2.0, 0.0, 2.0, 0.0, 2.0, 0.0, 2.0, 0.0, 1.0, -1.0, 1.0, -1.0, 1.0, -1.0, 1.0, -1.0, 1.0, -1.0, 1.0, -1.0, 1.0, -1.0, 1.0, -1.0, 0.0, -2.0, 0.0, -2.0, 0.0, -2.0, 0.0, -2.0],
        [-0.5, -0.5, 1.5, 1.5, -1.5, -1.5, 0.5, 0.5, -0.5, -0.5, 1.5, 1.5, -1.5, -1.5, 0.5, 0.5, -0.5, -0.5, 1.5, 1.5, -1.5, -1.5, 0.5, 0.5, -0.5, -0.5, 1.5, 1.5, -1.5, -1.5, 0.5, 0.5],
        [0.5, 1.5, 0.5, 1.5, 0.5, 1.5, 0.5, 1.5, -1.5, -0.5, -1.5, -0.5, -1.5, -0.5, -1.5, -0.5, 0.5, 1.5, 0.5, 1.5, 0.5, 1.5, 0.5, 1.5, -1.5, -0.5, -1.5, -0.5, -1.5, -0.5, -1.5, -0.5],
    ];
    let mut s = String::from("x1,x2,x3\n");
    for r in 0..32 {
        s.push_str(&format!("{:?},{:?},{:?}\n", cols[0][r], cols[1][r], cols[2][r]));
    }
    s.into_bytes()
}

struct World {
    _dir: TempDir,
    _app: tauri::App<tauri::test::MockRuntime>,
    w: tauri::WebviewWindow<tauri::test::MockRuntime>,
    data: std::path::PathBuf,
    stuck: std::path::PathBuf,
}

fn world(runs: Option<Arc<Runs>>) -> World {
    let dir = TempDir::new("sem");
    let data = dir.write("data.csv", &csv());
    let stuck = dir.write("stuck.csv", &walsh());
    let app_data = dir.0.join("appdata");
    std::fs::create_dir_all(&app_data).unwrap();
    let db = Arc::new(Database::in_memory().unwrap());
    let (app, w) = app(&app_data, db, runs);
    World { _dir: dir, _app: app, w, data, stuck }
}

fn open(w: &World) -> Value {
    call(&w.w, "stats_desc_open", json!({ "path": w.data.to_string_lossy() })).expect("open")
}

const FACTOR_1: [&str; 3] = ["C1", "C3", "C5"];
const FACTOR_2: [&str; 3] = ["C2", "C4", "C6"];

/// The analysis most tests run: two correlated factors of three columns, the
/// second's regression on the first written as a path.
fn two_params() -> Value {
    json!({
        "indicators": [FACTOR_1, FACTOR_2],
        "paths": [{ "to": { "factor": 2 }, "from": { "factor": 1 } }],
        "covariances": [],
        "equal": [],
        "missingCodes": [],
    })
}

fn two(o: &Value, op: u64) -> Value {
    sspec(o, op, two_params())
}

fn start(w: &World, o: &Value, spec: Value) -> Result<u64, Value> {
    call(&w.w, "stats_sem_run", json!({ "slot": o["slot"], "spec": spec })).map(|v| v["runId"].as_u64().expect("runId"))
}

fn code_of(err: &Value) -> String {
    refusal(err)["code"].as_str().unwrap().to_string()
}

/// JSON text and back, as the view went.
fn through_text<T: serde::Serialize>(v: &T) -> Value {
    serde_json::from_str(&serde_json::to_string(v).unwrap()).unwrap()
}

fn direct_params() -> SemParams {
    serde_json::from_value(two_params()).unwrap()
}

/// The check point before the estimation for six columns: 1 + 6.
const ESTIMATION_POINT: usize = 7;

/* ---- the flow --------------------------------------------------------------- */

#[test]
fn open_validate_run_status_result_export_close_through_ipc() {
    let w = world(None);
    let o = open(&w);
    let dataset = load_dataset(&w.data).expect("the same file");

    let v = call(&w.w, "stats_sem_validate", json!({ "slot": o["slot"], "spec": two(&o, 1) })).unwrap();
    assert_eq!(v["status"], "valid", "{v}");
    assert_eq!(v["errors"], json!([]));
    assert_eq!(v["estimatedResources"], json!({ "rows": 40, "columns": 6, "cells": 240 }));

    let run = start(&w, &o, two(&o, 2)).unwrap();
    let s = until_terminal_sem(&w.w, run);
    assert_eq!(s["state"], "succeeded", "{s}");
    assert_eq!(s["stepsTotal"], stats_sem::check_points(&direct_params()) as u64, "{s}");
    assert_eq!(s["stepsDone"], s["stepsTotal"], "{s}");

    let r = call(&w.w, "stats_sem_result", json!({ "runId": run })).unwrap();
    assert_eq!((r["runId"].as_u64(), r["status"].as_str()), (Some(run), Some("succeeded")));
    let direct = stats_sem::compute(&dataset, &direct_params()).expect("computes");
    assert_eq!(r["result"], through_text(&direct));
    assert_eq!(r["result"]["columns"], json!(["C1", "C2", "C3", "C4", "C5", "C6"]));
    assert_eq!(r["result"]["rowsUsed"], 40);
    assert_eq!(r["result"]["effects"], json!([through_text(&direct.effects[0])]), "one structural pair");
    assert_eq!(r["result"]["limitations"].as_array().unwrap().len(), 15);
    let m = &r["manifest"];
    assert_eq!(m["capabilityId"], "stats.analyze.sem");
    assert_eq!(m["outputHash"], ContentHash::of_bytes(&direct.canonical_bytes()).as_hex(), "bit for bit");
    assert_eq!((m["rowsUsed"].as_u64(), m["rowsRead"].as_u64()), (Some(40), Some(40)));

    let manifest: RunManifest<SemCode> = serde_json::from_value(m.clone()).unwrap();
    let e = call(&w.w, "stats_sem_export", json!({ "runId": run, "format": "csv" })).unwrap();
    let bytes: Vec<u8> = serde_json::from_value(e["bytes"].clone()).unwrap();
    assert_eq!(bytes, result_csv(&direct, &manifest));
    assert_eq!(e["sha256"], ContentHash::of_bytes(&bytes).as_hex());
    let j = call(&w.w, "stats_sem_export", json!({ "runId": run, "format": "manifestJson" })).unwrap();
    let jb: Vec<u8> = serde_json::from_value(j["bytes"].clone()).unwrap();
    assert_eq!(jb, manifest_json(&manifest));

    assert_eq!(call(&w.w, "stats_desc_close", json!({ "slot": o["slot"] })).unwrap(), true);
}

/* ---- refusals --------------------------------------------------------------- */

#[test]
fn each_refusal_has_its_code() {
    let w = world(None);
    let o = open(&w);
    let validate = |spec: Value| call(&w.w, "stats_sem_validate", json!({ "slot": o["slot"], "spec": spec })).unwrap();
    let f = |op, indicators: &[&[&str]], codes: &[f64]| sspec(&o, op, factors(indicators, true, codes));
    let one = |op, cols: &[&str]| sspec(&o, op, factors(&[cols], true, &[]));
    let paths = |op, p: Value| sspec(&o, op, json!({ "indicators": [], "paths": p, "covariances": [], "equal": [], "missingCodes": [] }));
    let mut no_input = two(&o, 21);
    no_input["inputs"] = json!([]);
    let mut other_input = two(&o, 22);
    other_input["inputs"][0]["contentHash"] = json!("cd".repeat(32));
    let mut checked = 0;
    for (spec, code, params) in [
        (no_input, "stats.sem.input_count", json!({ "count": 0 })),
        (other_input, "stats.sem.input_mismatch", json!({})),
        (f(1, &[&["C1", "C3", "C5"]], &[2.0, 2.0]), "stats.sem.missing_code_repeated", json!({ "index": 1 })),
        (f(2, &[], &[]), "stats.sem.empty_model", json!({})),
        (f(3, &[&["C1"], &["C2", "C4"]], &[]), "stats.sem.factor_too_few_items", json!({ "index": 1, "count": 1, "limit": 2 })),
        (one(5, &["C3", "C1", "C5"]), "stats.sem.items_out_of_order", json!({ "column": 1 })),
        (f(6, &[&["C2", "C4"], &["C1", "C3"]], &[]), "stats.sem.factors_out_of_order", json!({ "index": 2 })),
        (f(7, &[&["C1", "C3"], &["C3", "C5"]], &[]), "stats.sem.item_repeated", json!({ "column": 3 })),
        (paths(25, json!([{ "to": { "factor": 9 }, "from": { "column": "C1" } }])), "stats.sem.unknown_factor", json!({ "index": 1 })),
        (paths(26, json!([{ "to": { "column": "C1" }, "from": { "column": "C1" } }])), "stats.sem.path_to_itself", json!({ "index": 1 })),
        (one(8, &["C1", "C3", "C99"]), "stats.sem.unknown_column", json!({ "column": 99 })),
        (one(9, &["C1", "C3", "C9"]), "stats.sem.column_non_finite", json!({ "column": 9, "count": 1 })),
        (one(10, &["C1", "C3", "C7"]), "stats.sem.column_holds_text", json!({ "column": 7, "count": 40 })),
        // A factor of two columns alone: 4 parameters over 3 moments, rank 3.
        (one(17, &["C1", "C3"]), "stats.sem.model_not_identified", json!({ "count": 1 })),
        (one(11, &["C1", "C16", "C17"]), "stats.sem.no_complete_rows", json!({})),
        (one(12, &["C1", "C3", "C12"]), "stats.sem.too_few_rows", json!({ "count": 2, "limit": 4 })),
        (one(13, &["C1", "C3", "C10"]), "stats.sem.column_constant", json!({ "column": 10 })),
    ] {
        let v = validate(spec);
        assert_eq!(v["status"], "invalid", "{code}: {v}");
        assert_eq!(v["errors"][0]["code"], code, "{v}");
        assert_eq!(v["errors"][0]["params"], params, "{code}: {v}");
        checked += 1;
    }
    assert_eq!(checked, 17);
    // A refused run, as a view: the run is made and refused by its check.
    let bad = start(&w, &o, one(14, &["C1", "C1", "C3"])).unwrap();
    let s = until_terminal_sem(&w.w, bad);
    assert_eq!(s["state"], "refused", "{s}");
    assert_eq!(s["diagnostics"][0]["code"], "stats.sem.item_repeated");
    // The run's own refusals: validation passes, and the run's work refuses,
    // recorded as a failed run carrying the refusal, as for every kind.
    for (spec, code, params) in [
        (one(15, &["C1", "C2", "C8"]), "stats.sem.sample_covariance_singular", json!({ "column": 8 })),
        (one(16, &["C1", "C3", "C11"]), "stats.sem.overflow", json!({ "column": 11 })),
    ] {
        assert_eq!(validate(spec.clone())["status"], "valid", "{code}");
        let run = start(&w, &o, spec).unwrap();
        let s = until_terminal_sem(&w.w, run);
        assert_eq!(s["state"], "failed", "{code}: {s}");
        assert_eq!(s["diagnostics"][0]["code"], code, "{s}");
        assert_eq!(s["diagnostics"][0]["params"], params, "{s}");
    }
    let stuck = call(&w.w, "stats_desc_open", json!({ "path": w.stuck.to_string_lossy() })).expect("open");
    let spec = sspec(&stuck, 18, factors(&[&["C1", "C2", "C3"]], true, &[]));
    let run = call(&w.w, "stats_sem_run", json!({ "slot": stuck["slot"], "spec": spec })).unwrap()["runId"].as_u64().unwrap();
    let s = until_terminal_sem(&w.w, run);
    assert_eq!(s["state"], "failed", "{s}");
    assert_eq!(s["diagnostics"][0]["code"], "stats.sem.sem_not_converged", "{s}");
    assert_eq!(s["diagnostics"][0]["params"], json!({ "count": 500, "limit": 500 }), "{s}");
    // The registry before the slot.
    let mut s = two(&o, 19);
    s["capabilityVersion"] = json!(2);
    for cmd in ["stats_sem_validate", "stats_sem_run"] {
        let e = call(&w.w, cmd, json!({ "slot": 999, "spec": s })).unwrap_err();
        assert_eq!(code_of(&e), "capability.version_mismatch", "{cmd}");
    }
    for other in ["stats.describe", "stats.verify.ols", "stats.analyze.cfa"] {
        let mut t = two(&o, 20);
        t["capabilityId"] = json!(other);
        let e = call(&w.w, "stats_sem_run", json!({ "slot": o["slot"], "spec": t })).unwrap_err();
        assert!(code_of(&e).starts_with("capability."), "{other}: {e}");
    }
    let e = call(&w.w, "stats_sem_validate", json!({ "slot": 999, "spec": two(&o, 23) })).unwrap_err();
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
        let mut s = sspec(&view, op, factors(&[&["C4", "C5", "C18"]], true, codes));
        s["preparation"] = view["preparation"].clone();
        s
    };
    let validate = |spec: Value| call(&w.w, "stats_sem_validate", json!({ "slot": o["slot"], "spec": spec })).unwrap();
    let v = validate(prepared(1, &[]));
    assert_eq!(v["status"], "valid", "a score is a column like any other: {v}");
    let v = validate(prepared(2, &[score as f64]));
    assert_eq!(v["status"], "invalid", "{v}");
    assert_eq!(v["errors"][0]["code"], "stats.sem.code_equals_score", "{v}");
    assert_eq!(v["errors"][0]["params"]["column"], 18);
    let v = validate(sspec(&view, 3, factors(&[&["C4", "C5", "C18"]], true, &[])));
    assert_eq!(v["errors"][0]["code"], "stats.sem.preparation_mismatch", "{v}");
}

#[test]
fn every_command_refuses_while_statistics_is_inactive() {
    let dir = TempDir::new("sem-inactive");
    let app_data = dir.0.join("appdata");
    std::fs::create_dir_all(&app_data).unwrap();
    let db = Arc::new(Database::in_memory().unwrap());
    let (_app, w) = app_with(&app_data, db, Arc::new(|| Arc::new(Runs::default())), Arc::new(app_lib::workspace::readiness));
    let o = json!({ "slot": 1, "inputRef": { "contentHash": "ab".repeat(32), "rows": 1, "columns": 1, "semanticRole": "table" } });
    let spec = two(&o, 1);
    let mut checked = 0;
    for (cmd, body) in [
        ("stats_sem_validate", json!({ "slot": 1, "spec": spec })),
        ("stats_sem_run", json!({ "slot": 1, "spec": spec })),
        ("stats_sem_status", json!({ "runId": 1 })),
        ("stats_sem_result", json!({ "runId": 1 })),
        ("stats_sem_cancel", json!({ "runId": 1 })),
        ("stats_sem_export", json!({ "runId": 1, "format": "csv" })),
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
    until_terminal_sem(&w.w, k);
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
        ("stats_factor", k),
        ("stats_cfa", k),
        ("stats_sem", d),
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
    assert_eq!(checked, 32);
    assert_eq!(code_of(&call(&w.w, "stats_sem_status", json!({ "runId": 99 })).unwrap_err()), "run.unknown_run");
    let k2 = start(&w, &o, two(&o, 3)).unwrap();
    until_terminal_sem(&w.w, k2);
    let s = call(&w.w, "workspace_state", json!({})).unwrap();
    assert_eq!(s["latestRun"], json!({ "runId": k2, "kind": "sem" }));
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
    let run = start(&w, &o, two(&o, 1)).unwrap();
    let s = until_terminal_sem(&w.w, run);
    assert_eq!(s["state"], "succeeded", "{s}");
    assert_eq!(s["stepsTotal"], 11);
    assert_eq!(s["stepsDone"], 11);
    let seen = seen.lock().unwrap().clone();
    let mut distinct = seen.clone();
    distinct.dedup();
    assert_eq!(distinct, (0..11).collect::<Vec<usize>>());
    let r = call(&w.w, "stats_sem_result", json!({ "runId": run })).unwrap();
    let iterations = r["result"]["iterations"].as_u64().unwrap();
    assert!(iterations >= 2, "the premise: more than one step");
    assert_eq!(r["result"]["attempt"], 1);
    assert_eq!(
        seen.iter().filter(|i| **i == ESTIMATION_POINT).count() as u64,
        1 + iterations + 1,
        "one at the start, one per step, one before the check at the solution: {seen:?}"
    );
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
    let run = start(&w, &o, two(&o, 1)).unwrap();
    assert_eq!(parked.recv_timeout(Duration::from_secs(20)).expect("parked"), at);
    let c = call(&w.w, "stats_sem_cancel", json!({ "runId": run })).unwrap();
    assert_eq!((c["state"].as_str(), c["cancelRequested"].as_bool()), (Some("running"), Some(true)), "{c}");
    go.send(()).unwrap();
    let s = until_terminal_sem(&w.w, run);
    assert_eq!(s["state"], "cancelled", "{s}");
    assert_eq!(s["stepsDone"], steps_done, "{s}");
    for (cmd, body) in [
        ("stats_sem_result", json!({ "runId": run })),
        ("stats_sem_export", json!({ "runId": run, "format": "csv" })),
    ] {
        assert_eq!(code_of(&call(&w.w, cmd, body).unwrap_err()), "run.cancelled", "{cmd}");
    }
}

#[test]
fn a_cancel_mid_covariances_stops_the_run_with_no_result() {
    // The third column's check point, 1 + 2.
    cancel_at(|p| p == Point::Column(3), Point::Column(3), 3);
}

#[test]
fn a_cancel_during_the_iterations_stops_the_run_with_no_result() {
    // The estimation point's third call: the second step.
    let calls = Arc::new(AtomicU64::new(0));
    let c = calls.clone();
    cancel_at(
        move |p| p == Point::Column(ESTIMATION_POINT) && c.fetch_add(1, Ordering::SeqCst) == 2,
        Point::Column(ESTIMATION_POINT),
        ESTIMATION_POINT as u64,
    );
}

#[test]
fn the_ceiling_stops_a_run_whose_iterations_sleep_past_it() {
    // Each call of the estimation's check point "sleeps" 11 s on the injected
    // clock: the one before the estimation and the first step's leave 22 s, the
    // second step's passes the manager's 30 s ceiling.
    let clock = Arc::new(TestClock(AtomicU64::new(1_000)));
    let c = clock.clone();
    let hook: CheckpointHook = Arc::new(move |_, point| {
        if point == Point::Column(ESTIMATION_POINT) {
            c.0.fetch_add(11_000, Ordering::SeqCst);
        }
    });
    let w = world(Some(runs_with(clock, hook)));
    let o = open(&w);
    let run = start(&w, &o, two(&o, 1)).unwrap();
    let s = until_terminal_sem(&w.w, run);
    assert_eq!(s["state"], "timed_out", "{s}");
    assert_eq!(code_of(&call(&w.w, "stats_sem_result", json!({ "runId": run })).unwrap_err()), "run.timed_out");
    // The same spec on a clock that never moves: a result, using more than two
    // iterations, so the ceiling above was reached during them.
    let (hook, _) = recording();
    let w = world(Some(runs_with(Arc::new(TestClock(AtomicU64::new(1_000))), hook)));
    let o = open(&w);
    let run = start(&w, &o, two(&o, 1)).unwrap();
    assert_eq!(until_terminal_sem(&w.w, run)["state"], "succeeded");
    let r = call(&w.w, "stats_sem_result", json!({ "runId": run })).unwrap();
    assert!(r["result"]["iterations"].as_u64().unwrap() >= 2, "{}", r["result"]["iterations"]);
}
