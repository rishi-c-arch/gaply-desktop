//! **The related rank tests' commands at their boundary (Research OS, Phase
//! 8Z-1, decision 641).** Every call goes through Tauri's IPC on the mock
//! runtime, with the webview's argument names, into the state `AppState::new`
//! builds, and every assertion reads the JSON the webview reads.
//!
//! * The flow: open → validate → run → status → result → export (CSV and
//!   manifest) → close, for the Wilcoxon test, the Friedman test and the
//!   columns the other way round, the numbers equal to `stats_related_ranks`'s
//!   on the same dataset, the CSV equal to its export, and one output hash
//!   pinned on macOS aarch64.
//! * The progress counts its one check point.
//! * A cancel at the check point stops the run with no result; the run
//!   manager's ceiling stops a run whose check point "sleeps" past it, on an
//!   injected clock, never 30 s of real time.
//! * Refusals, each with its exact code, and the registry's `capability.*`
//!   codes as they are (decision 74), before the slot is read. One refusal is
//!   not sent here: `missing_code_not_finite`, since JSON has no NaN or infinity
//!   to send; `related_ranks_cases.rs` holds it.
//! * Statistics inactive: all six commands refuse `workspace.inactive`.
//! * `run.wrong_kind` both ways against the summary, and against the ANOVA's
//!   and the rank tests' commands.
//!
//! Every file is synthetic and in a folder the test creates and removes.

mod desc_support;
mod prep_support;
mod related_ranks_support;

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
use gaply_core::stats_related_ranks::export::{manifest_json, result_csv};
use gaply_core::stats_related_ranks::{self, RelatedRanksCode, RelatedRanksParams};
use gaply_core::Database;
use related_ranks_support::{app, app_with, rrspec, until_terminal_related};

/* ---- fixtures --------------------------------------------------------------- */

const HEADERS: [&str; 8] = ["pre", "post", "later", "note", "copy", "odd", "even", "bad"];

/// 12 rows: three numeric columns, the second empty in the last row; a text
/// column; a copy of the first; two columns that are never both filled; and a
/// column holding one written-out infinity.
fn rows() -> Vec<Vec<C<'static>>> {
    const PRE: [&str; 12] = [
        "3.5", "1", "4.25", "2", "7", "3.5", "5.5", "0.5", "6", "2", "8.75", "4",
    ];
    const POST: [&str; 11] = [
        "4", "2.5", "4", "5", "6.5", "6", "8", "3", "6.25", "5.5", "9",
    ];
    const LATER: [&str; 12] = [
        "5", "3", "7", "4", "9", "4.5", "9.5", "1", "7.5", "6", "10", "5.5",
    ];
    const NOTES: [&str; 4] = ["n1", "n2", "n3", "n4"];
    (0..12)
        .map(|i| {
            vec![
                C::N(PRE[i]),
                if i < 11 { C::N(POST[i]) } else { C::Empty },
                C::N(LATER[i]),
                C::T(NOTES[i % 4]),
                C::N(PRE[i]),
                if i % 2 == 1 { C::N(PRE[i]) } else { C::Empty },
                if i % 2 == 0 { C::N(PRE[i]) } else { C::Empty },
                C::N(if i == 0 { "inf" } else { "1" }),
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
    let dir = TempDir::new("related-ranks");
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

/// The analysis most tests run: the first two columns.
fn two(o: &Value, op: u64) -> Value {
    rrspec(o, op, &["C1", "C2"], &[])
}

fn start(w: &World, o: &Value, spec: Value) -> Result<u64, Value> {
    call(
        &w.w,
        "stats_related_ranks_run",
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

fn direct_params(columns: &[u32]) -> RelatedRanksParams {
    RelatedRanksParams {
        columns: columns.iter().map(|c| Alias(ColumnAlias(*c))).collect(),
        missing_codes: vec![],
    }
}

/// The flow's output hash, pinned on macOS aarch64 (10 Nov 2026): the run the
/// app holds is the module's, and the module's is this one. Its p comes through
/// the platform's maths library, as the export's pin says, so another platform
/// checks only that the app's hash is the module's, until its bytes are
/// measured.
const FLOW_OUTPUT_SHA256: &str = "891ba1f1274a9e8564c614d04e6dbff93949725df9493e3e01aaebc987c6fd5f";

/* ---- the flow --------------------------------------------------------------- */

#[test]
fn open_validate_run_status_result_export_close_through_ipc() {
    let w = world(None);
    let o = open(&w);
    let dataset = load_dataset(&w.data).expect("the same file");

    let v = call(
        &w.w,
        "stats_related_ranks_validate",
        json!({ "slot": o["slot"], "spec": two(&o, 1) }),
    )
    .unwrap();
    assert_eq!(v["status"], "valid", "{v}");
    assert_eq!(v["errors"], json!([]));
    assert_eq!(v["missingInformation"], json!([]));
    assert_eq!(
        v["estimatedResources"],
        json!({ "rows": 12, "columns": 2, "cells": 24 })
    );

    let run = start(&w, &o, two(&o, 2)).unwrap();
    let s = until_terminal_related(&w.w, run);
    assert_eq!(s["state"], "succeeded", "{s}");
    assert_eq!(s["cancelRequested"], false);
    // One check point, before the arithmetic, and a result has done it.
    assert_eq!(s["stepsTotal"], 1, "{s}");
    assert_eq!(s["stepsDone"], 1, "{s}");
    assert!(s.get("columnsDone").is_none(), "steps, never columns: {s}");

    let r = call(&w.w, "stats_related_ranks_result", json!({ "runId": run })).unwrap();
    assert_eq!(r["runId"], run);
    assert_eq!(r["status"], "succeeded");
    let direct = stats_related_ranks::compute(&dataset, &direct_params(&[1, 2])).expect("computes");
    assert_eq!(r["result"], through_text(&direct));
    let t = &r["result"]["test"]["wilcoxon"];
    assert_eq!(t["first"], "C1");
    assert_eq!(t["second"], "C2");
    assert_eq!(r["result"]["rowsUsed"], 11);
    assert_eq!(
        r["result"]["exclusions"],
        json!([{ "code": "stats.related_ranks.missing_value", "count": 1, "params": { "column": 2 } }])
    );
    assert_eq!(
        r["result"]["limitations"],
        json!([
            "stats.related_ranks.no_pairwise_comparisons",
            "stats.related_ranks.two_sided_only",
            "stats.related_ranks.no_location_estimate",
            "stats.related_ranks.exact_without_ties_or_zeros_only",
            "stats.related_ranks.pratt_not_offered",
            "stats.related_ranks.no_further_related_tests",
            "stats.related_ranks.no_weights",
            "stats.related_ranks.column_order",
        ])
    );
    let m = &r["manifest"];
    assert_eq!(m["capabilityId"], "stats.analyze.related_ranks");
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
    let manifest: RunManifest<RelatedRanksCode> = serde_json::from_value(m.clone()).unwrap();
    let e = call(
        &w.w,
        "stats_related_ranks_export",
        json!({ "runId": run, "format": "csv" }),
    )
    .unwrap();
    let bytes: Vec<u8> = serde_json::from_value(e["bytes"].clone()).unwrap();
    assert_eq!(bytes, result_csv(&direct, &manifest));
    assert_eq!(e["sha256"], ContentHash::of_bytes(&bytes).as_hex());
    let j = call(
        &w.w,
        "stats_related_ranks_export",
        json!({ "runId": run, "format": "manifestJson" }),
    )
    .unwrap();
    let jb: Vec<u8> = serde_json::from_value(j["bytes"].clone()).unwrap();
    assert_eq!(jb, manifest_json(&manifest));

    // Three columns: the Friedman test.
    let run = start(&w, &o, rrspec(&o, 3, &["C1", "C2", "C3"], &[])).unwrap();
    assert_eq!(until_terminal_related(&w.w, run)["state"], "succeeded");
    let r = call(&w.w, "stats_related_ranks_result", json!({ "runId": run })).unwrap();
    let direct = stats_related_ranks::compute(&dataset, &direct_params(&[1, 2, 3])).unwrap();
    assert_eq!(r["result"], through_text(&direct));
    assert_eq!(r["result"]["test"]["friedman"]["df"], 2.0);
    assert_eq!(
        r["result"]["test"]["friedman"]["columns"]
            .as_array()
            .unwrap()
            .len(),
        3
    );

    // The columns the other way round: the differences' signs follow the list.
    let forward = call(
        &w.w,
        "stats_related_ranks_result",
        json!({ "runId": start(&w, &o, two(&o, 4)).map(|r| { until_terminal_related(&w.w, r); r }).unwrap() }),
    )
    .unwrap();
    let back = start(&w, &o, rrspec(&o, 5, &["C2", "C1"], &[])).unwrap();
    assert_eq!(until_terminal_related(&w.w, back)["state"], "succeeded");
    let back = call(&w.w, "stats_related_ranks_result", json!({ "runId": back })).unwrap();
    let (f, b) = (
        &forward["result"]["test"]["wilcoxon"],
        &back["result"]["test"]["wilcoxon"],
    );
    assert_eq!(b["first"], "C2");
    assert_eq!(b["v"], f["tMinus"], "V and T- swap");
    assert_eq!(
        b["z"]["value"].as_f64().unwrap(),
        -f["z"]["value"].as_f64().unwrap()
    );

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
            "stats_related_ranks_validate",
            json!({ "slot": o["slot"], "spec": spec }),
        )
        .unwrap()
    };
    let fifty_one: Vec<String> = (1..=51).map(|c| format!("C{c}")).collect();
    let fifty_one: Vec<&str> = fifty_one.iter().map(String::as_str).collect();
    let mut no_input = two(&o, 21);
    no_input["inputs"] = json!([]);
    let mut other_input = two(&o, 22);
    other_input["inputs"][0]["contentHash"] = json!("cd".repeat(32));
    for (spec, code, params) in [
        (
            no_input,
            "stats.related_ranks.input_count",
            json!({ "count": 0 }),
        ),
        (
            other_input,
            "stats.related_ranks.input_mismatch",
            json!({}),
        ),
        (
            rrspec(&o, 23, &["C1", "C8"], &[]),
            "stats.related_ranks.column_non_finite",
            json!({ "column": 8, "count": 1 }),
        ),
        (
            rrspec(&o, 1, &["C1"], &[]),
            "stats.related_ranks.too_few_columns",
            json!({ "count": 1 }),
        ),
        (
            rrspec(&o, 2, &fifty_one, &[]),
            "stats.related_ranks.too_many_columns",
            json!({ "count": 51, "limit": 50 }),
        ),
        (
            rrspec(&o, 3, &["C1", "C1"], &[]),
            "stats.related_ranks.column_repeated",
            json!({ "column": 1 }),
        ),
        (
            rrspec(&o, 4, &["C1", "C9"], &[]),
            "stats.related_ranks.unknown_column",
            json!({ "column": 9 }),
        ),
        (
            rrspec(&o, 5, &["C1", "C4"], &[]),
            "stats.related_ranks.column_holds_text",
            json!({ "column": 4, "count": 12 }),
        ),
        (
            rrspec(&o, 6, &["C6", "C7"], &[]),
            "stats.related_ranks.no_complete_rows",
            json!({}),
        ),
    ] {
        let v = validate(spec);
        assert_eq!(v["status"], "invalid", "{code}: {v}");
        assert_eq!(v["errors"][0]["code"], code, "{v}");
        assert_eq!(v["errors"][0]["params"], params, "{code}: {v}");
    }
    // A refused run, as a view: the run is made and refused by its check.
    let bad = start(&w, &o, rrspec(&o, 7, &["C1", "C1"], &[])).unwrap();
    let s = until_terminal_related(&w.w, bad);
    assert_eq!(s["state"], "refused", "{s}");
    assert_eq!(
        s["diagnostics"][0]["code"],
        "stats.related_ranks.column_repeated"
    );
    // Not computable is a result, not a refusal: a column and its copy.
    let same = start(&w, &o, rrspec(&o, 8, &["C1", "C5"], &[])).unwrap();
    assert_eq!(until_terminal_related(&w.w, same)["state"], "succeeded");
    let r = call(&w.w, "stats_related_ranks_result", json!({ "runId": same })).unwrap();
    assert_eq!(
        r["result"]["test"]["wilcoxon"]["z"]["not_computable"],
        "stats.related_ranks.all_differences_zero",
        "{r}"
    );
    // The registry before the slot: a version mismatch is refused as such
    // for a slot no dataset has, as the registry words it, and makes no run.
    let mut s = two(&o, 11);
    s["capabilityVersion"] = json!(2);
    for cmd in ["stats_related_ranks_validate", "stats_related_ranks_run"] {
        let e = call(&w.w, cmd, json!({ "slot": 999, "spec": s })).unwrap_err();
        assert_eq!(code_of(&e), "capability.version_mismatch", "{cmd}");
    }
    // A spec of another capability through this door.
    for other in ["stats.describe", "stats.analyze.independent_ranks"] {
        let mut t = two(&o, 12);
        t["capabilityId"] = json!(other);
        let e = call(
            &w.w,
            "stats_related_ranks_run",
            json!({ "slot": o["slot"], "spec": t }),
        )
        .unwrap_err();
        assert!(code_of(&e).starts_with("capability."), "{other}: {e}");
    }
    // A slot no dataset has.
    let e = call(
        &w.w,
        "stats_related_ranks_validate",
        json!({ "slot": 999, "spec": two(&o, 13) }),
    )
    .unwrap_err();
    assert_eq!(code_of(&e), "run.unknown_input");
}

/// The two refusals only a prepared table can reach, through IPC: a score made
/// with `stats_prep_make`, C9, the sum of the first three columns.
#[test]
fn a_prepared_table_refuses_a_code_on_a_score_and_a_spec_without_its_preparation() {
    let w = world(None);
    let o = open(&w);
    let recipe = prep_support::recipe(8, "total", "sum", &["C1", "C2", "C3"]);
    let view = prep_support::make(&w.w, &o["slot"], &recipe);
    let prepared = |op, columns: &[&str], codes: &[f64]| {
        let mut s = rrspec(&view, op, columns, codes);
        s["preparation"] = view["preparation"].clone();
        s
    };
    let validate = |spec: Value| {
        call(
            &w.w,
            "stats_related_ranks_validate",
            json!({ "slot": o["slot"], "spec": spec }),
        )
        .unwrap()
    };
    // A score is a column like any other.
    let v = validate(prepared(1, &["C1", "C9"], &[]));
    assert_eq!(v["status"], "valid", "{v}");
    // 12.5 is the first row's score, and no other's.
    let v = validate(prepared(2, &["C1", "C9"], &[12.5]));
    assert_eq!(v["status"], "invalid", "{v}");
    assert_eq!(
        v["errors"][0]["code"],
        "stats.related_ranks.code_equals_score",
        "{v}"
    );
    assert_eq!(v["errors"][0]["params"], json!({ "column": 9, "count": 1 }));
    // The prepared table's spec with no preparation named.
    let v = validate(rrspec(&view, 3, &["C1", "C9"], &[]));
    assert_eq!(v["status"], "invalid", "{v}");
    assert_eq!(
        v["errors"][0]["code"],
        "stats.related_ranks.preparation_mismatch",
        "{v}"
    );
}

#[test]
fn every_command_refuses_while_statistics_is_inactive() {
    let dir = TempDir::new("related-ranks-inactive");
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
    let spec = two(&o, 1);
    let mut checked = 0;
    for (cmd, body) in [
        (
            "stats_related_ranks_validate",
            json!({ "slot": 1, "spec": spec }),
        ),
        (
            "stats_related_ranks_run",
            json!({ "slot": 1, "spec": spec }),
        ),
        ("stats_related_ranks_status", json!({ "runId": 1 })),
        ("stats_related_ranks_result", json!({ "runId": 1 })),
        ("stats_related_ranks_cancel", json!({ "runId": 1 })),
        (
            "stats_related_ranks_export",
            json!({ "runId": 1, "format": "csv" }),
        ),
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
    let k = start(&w, &o, two(&o, 1)).unwrap();
    until_terminal_related(&w.w, k);
    let d = call(
        &w.w,
        "stats_desc_run",
        json!({ "slot": o["slot"], "spec": desc_spec(&o, 2, &[("C1", "numeric")], &[]) }),
    )
    .unwrap()["runId"]
        .as_u64()
        .unwrap();
    until_terminal(&w.w, d);
    let mut checked = 0;
    for (prefix, theirs) in [
        ("stats_desc", k),
        ("stats_anova", k),
        ("stats_ranks", k),
        ("stats_related_ranks", d),
    ] {
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
    assert_eq!(checked, 16);
    assert_eq!(
        code_of(&call(&w.w, "stats_related_ranks_status", json!({ "runId": 99 })).unwrap_err()),
        "run.unknown_run"
    );
    let s = call(&w.w, "workspace_state", json!({})).unwrap();
    assert_eq!(s["latestRun"], json!({ "runId": d, "kind": "describe" }));
    // A related rank tests run names its kind.
    let k2 = start(&w, &o, two(&o, 3)).unwrap();
    until_terminal_related(&w.w, k2);
    let s = call(&w.w, "workspace_state", json!({})).unwrap();
    assert_eq!(
        s["latestRun"],
        json!({ "runId": k2, "kind": "related_ranks" })
    );
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
    let s = until_terminal_related(&w.w, run);
    assert_eq!(s["state"], "succeeded", "{s}");
    assert_eq!(stats_related_ranks::check_points(), 1);
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
    let run = start(&w, &o, two(&o, 1)).unwrap();
    assert_eq!(
        parked
            .recv_timeout(Duration::from_secs(20))
            .expect("parked"),
        Point::Column(0)
    );
    let c = call(&w.w, "stats_related_ranks_cancel", json!({ "runId": run })).unwrap();
    assert_eq!(c["state"], "running", "{c}");
    assert_eq!(c["cancelRequested"], true);
    go.send(()).unwrap();
    let s = until_terminal_related(&w.w, run);
    assert_eq!(s["state"], "cancelled", "{s}");
    assert_eq!(s["stepsDone"], 0, "stopped at its one check point: {s}");
    assert_eq!(s["stepsTotal"], 1);
    for (cmd, body) in [
        ("stats_related_ranks_result", json!({ "runId": run })),
        (
            "stats_related_ranks_export",
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
    let run = start(&w, &o, two(&o, 1)).unwrap();
    let s = until_terminal_related(&w.w, run);
    assert_eq!(s["state"], "timed_out", "{s}");
    assert_eq!(
        code_of(&call(&w.w, "stats_related_ranks_result", json!({ "runId": run })).unwrap_err()),
        "run.timed_out"
    );
    // The same spec on a clock that never moves: a result.
    let (hook, _) = recording();
    let w = world(Some(runs_with(
        Arc::new(TestClock(AtomicU64::new(1_000))),
        hook,
    )));
    let o = open(&w);
    let run = start(&w, &o, two(&o, 1)).unwrap();
    assert_eq!(until_terminal_related(&w.w, run)["state"], "succeeded");
}
