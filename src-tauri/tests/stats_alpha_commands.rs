//! **Cronbach's alpha's commands at their boundary (Research OS, Phase 8I-2,
//! decisions 380 and 381).** Every call goes through Tauri's IPC on the mock
//! runtime, with the webview's argument names, into the state
//! `AppState::new` builds, and every assertion reads the JSON the webview
//! reads.
//!
//! * The flow: open → validate → run → status → result → export (CSV and
//!   manifest) → close, the numbers equal to `stats_alpha`'s on the same
//!   dataset, and the CSV equal to its export.
//! * The progress counts check points (decision 380): the record's total is
//!   `stats_alpha::check_points`, from the spec alone; every check point
//!   reaches the hook in order; a result leaves done equal to the total.
//! * A cancel during the inter-item correlations stops the run there, with no
//!   result; the run manager's ceiling stops a run whose check point "sleeps"
//!   past it, on an injected clock, never 30 s of real time.
//! * What validation refuses never starts: a reversed item with a value
//!   outside the scale's ends is refused with its column and count, and
//!   `stats_alpha_run` of it is `refused` with no check point reached.
//! * Refusals, each with its exact code, and the registry's `capability.*`
//!   codes as they are (decision 74), before the slot is read.
//! * Statistics inactive: all six commands refuse `workspace.inactive`.
//! * `run.wrong_kind` both ways against the summary, the t-test, the ANOVA
//!   and the correlations.
//!
//! Every file is synthetic and in a folder the test creates and removes.

mod alpha_support;
mod desc_support;

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::{json, Value};

use alpha_support::{app, app_with, aspec, until_terminal_alpha};
use app_lib::runs::{CheckpointHook, Clock, Point, Runs};
use app_lib::stats_l1_ingest::load_dataset;
use desc_support::{call, delimited, refusal, spec as desc_spec, until_terminal, TempDir, C};
use gaply_core::contracts::{ContentHash, RunManifest};
use gaply_core::stats_alpha::export::{manifest_json, result_csv};
use gaply_core::stats_alpha::{self, AlphaCode, AlphaParams, ScaleEnds};
use gaply_core::stats_desc::Alias;
use gaply_core::stats_l1::dataset::ColumnAlias;
use gaply_core::Database;

/* ---- fixtures --------------------------------------------------------------- */

const HEADERS: [&str; 6] = ["a", "b", "c", "d", "note", "flat"];

/// Four five-point items of twelve rows, the second reverse-worded, with
/// cells missing in different rows and one -9, a text column and a constant
/// column. With -9 ticked, seven rows have a value in every item.
fn rows() -> Vec<Vec<C<'static>>> {
    const A: [&str; 12] = ["1", "2", "", "4", "5", "2", "3", "4", "5", "1", "3", "4"];
    const B: [&str; 12] = ["5", "4", "3", "", "1", "4", "3", "2", "-9", "5", "3", "2"];
    const CC: [&str; 12] = ["2", "2", "3", "4", "", "3", "3", "5", "4", "1", "2", "4"];
    const D: [&str; 12] = ["1", "3", "2", "5", "4", "", "3", "4", "5", "2", "2", "3"];
    let cell = |s: &'static str| if s.is_empty() { C::Empty } else { C::N(s) };
    (0..12)
        .map(|i| {
            vec![
                cell(A[i]),
                cell(B[i]),
                cell(CC[i]),
                cell(D[i]),
                C::T("t"),
                C::N("4"),
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

fn world_on(runs: Option<Arc<Runs>>, bytes: &[u8]) -> World {
    let dir = TempDir::new("alpha");
    let data = dir.write("data.csv", bytes);
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

fn world(runs: Option<Arc<Runs>>) -> World {
    world_on(runs, &csv())
}

fn open(w: &World) -> Value {
    call(
        &w.w,
        "stats_desc_open",
        json!({ "path": w.data.to_string_lossy() }),
    )
    .expect("open")
}

const FOUR: [&str; 4] = ["C1", "C2", "C3", "C4"];

/// The analysis the flow runs: the four items, the second reverse-scored on
/// 1 to 5, -9 ticked.
fn full(o: &Value, op: u64) -> Value {
    aspec(o, op, &FOUR, &["C2"], Some((1.0, 5.0)), &[-9.0])
}

fn start(w: &World, o: &Value, spec: Value) -> Result<u64, Value> {
    call(
        &w.w,
        "stats_alpha_run",
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

fn direct_params(
    items: &[u32],
    reversed: &[u32],
    scale: Option<(f64, f64)>,
    codes: &[f64],
) -> AlphaParams {
    let aliases = |v: &[u32]| v.iter().map(|n| Alias(ColumnAlias(*n))).collect();
    AlphaParams {
        items: aliases(items),
        reversed: aliases(reversed),
        scale: scale.map(|(min, max)| ScaleEnds { min, max }),
        missing_codes: codes.to_vec(),
    }
}

/* ---- the flow --------------------------------------------------------------- */

#[test]
fn open_validate_run_status_result_export_close_through_ipc() {
    let w = world(None);
    let o = open(&w);
    let dataset = load_dataset(&w.data).expect("the same file");

    let v = call(
        &w.w,
        "stats_alpha_validate",
        json!({ "slot": o["slot"], "spec": full(&o, 1) }),
    )
    .unwrap();
    assert_eq!(v["status"], "valid", "{v}");
    assert_eq!(v["errors"], json!([]));
    assert_eq!(v["missingInformation"], json!([]));

    let run = start(&w, &o, full(&o, 2)).unwrap();
    let s = until_terminal_alpha(&w.w, run);
    assert_eq!(s["state"], "succeeded", "{s}");
    assert_eq!(s["cancelRequested"], false);
    // Decision 380: the progress counts check points, and a result has done
    // them all. Four items: 1 + 4 + 6.
    assert_eq!(s["stepsTotal"], 11, "{s}");
    assert_eq!(s["stepsDone"], 11, "{s}");
    assert!(s.get("columnsDone").is_none(), "steps, never columns: {s}");

    let r = call(&w.w, "stats_alpha_result", json!({ "runId": run })).unwrap();
    assert_eq!(r["runId"], run);
    assert_eq!(r["status"], "succeeded");
    let direct = stats_alpha::compute(
        &dataset,
        &direct_params(&[1, 2, 3, 4], &[2], Some((1.0, 5.0)), &[-9.0]),
    )
    .expect("computes");
    assert_eq!(r["result"], through_text(&direct));
    assert_eq!(r["result"]["rowsUsed"], 7);
    assert_eq!(r["result"]["items"][1]["reversed"], true);
    assert_eq!(r["diagnostics"], json!([]));
    assert_eq!(
        r["result"]["limitations"],
        json!(["stats.alpha.assumes_one_dimension", "stats.alpha.no_omega"])
    );
    let m = &r["manifest"];
    assert_eq!(m["capabilityId"], "stats.analyze.cronbach_alpha");
    assert_eq!(
        m["outputHash"],
        ContentHash::of_bytes(&direct.canonical_bytes()).as_hex(),
        "bit for bit: the result the app holds is the module's"
    );
    assert_eq!(m["rowsRead"], 12);

    // The exports: the CSV is the module's export of that result and its
    // manifest, and the hash returned is the bytes'.
    let manifest: RunManifest<AlphaCode> = serde_json::from_value(m.clone()).unwrap();
    let e = call(
        &w.w,
        "stats_alpha_export",
        json!({ "runId": run, "format": "csv" }),
    )
    .unwrap();
    let bytes: Vec<u8> = serde_json::from_value(e["bytes"].clone()).unwrap();
    assert_eq!(bytes, result_csv(&direct, &manifest));
    assert_eq!(e["sha256"], ContentHash::of_bytes(&bytes).as_hex());
    let j = call(
        &w.w,
        "stats_alpha_export",
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
            "stats_alpha_validate",
            json!({ "slot": o["slot"], "spec": spec }),
        )
        .unwrap()
    };
    let ends = Some((1.0, 5.0));
    for (spec, code) in [
        (
            aspec(&o, 1, &["C1"], &[], None, &[]),
            "stats.alpha.too_few_items",
        ),
        (
            aspec(&o, 2, &["C2", "C1"], &[], None, &[]),
            "stats.alpha.items_out_of_order",
        ),
        (
            aspec(&o, 3, &["C1", "C1"], &[], None, &[]),
            "stats.alpha.item_repeated",
        ),
        (
            aspec(&o, 4, &["C1", "C3"], &["C2"], ends, &[]),
            "stats.alpha.reversed_not_an_item",
        ),
        (
            aspec(&o, 5, &FOUR, &["C2"], None, &[-9.0]),
            "stats.alpha.scale_missing",
        ),
        (
            aspec(&o, 6, &FOUR, &[], ends, &[-9.0]),
            "stats.alpha.scale_without_reversed",
        ),
        (
            aspec(&o, 7, &FOUR, &["C2"], Some((5.0, 1.0)), &[-9.0]),
            "stats.alpha.scale_ends_out_of_order",
        ),
        (
            aspec(&o, 8, &FOUR, &[], None, &[99.0, -9.0]),
            "stats.alpha.missing_codes_out_of_order",
        ),
        (
            aspec(&o, 9, &["C1", "C5"], &[], None, &[]),
            "stats.alpha.item_holds_text",
        ),
        (
            aspec(&o, 10, &["C1", "C9"], &[], None, &[]),
            "stats.alpha.unknown_column",
        ),
    ] {
        let v = validate(spec);
        assert_eq!(v["status"], "invalid", "{code}: {v}");
        assert_eq!(v["errors"][0]["code"], code, "{v}");
    }
    // The registry before the slot: a version mismatch is refused as such
    // for a slot no dataset has, as the registry words it, and makes no run.
    let mut s = full(&o, 11);
    s["capabilityVersion"] = json!(2);
    for cmd in ["stats_alpha_validate", "stats_alpha_run"] {
        let e = call(&w.w, cmd, json!({ "slot": 999, "spec": s })).unwrap_err();
        assert_eq!(code_of(&e), "capability.version_mismatch", "{cmd}");
    }
    // A spec of another capability through alpha's door.
    let mut t = full(&o, 12);
    t["capabilityId"] = json!("stats.analyze.correlations");
    let e = call(
        &w.w,
        "stats_alpha_run",
        json!({ "slot": o["slot"], "spec": t }),
    )
    .unwrap_err();
    assert!(code_of(&e).starts_with("capability."), "{e}");
}

/// Decision 380: what validation refuses never starts. With -9 not ticked,
/// the reversed item's -9 lies outside the scale's ends: validation refuses
/// with its column and count, and a run of that spec is refused before any
/// arithmetic, with no check point reached and no result.
#[test]
fn a_reversed_value_outside_the_scale_is_refused_and_never_starts() {
    let (hook, seen) = recording();
    let w = world(Some(runs_with(
        Arc::new(TestClock(AtomicU64::new(0))),
        hook,
    )));
    let o = open(&w);
    let outside = aspec(&o, 1, &FOUR, &["C2"], Some((1.0, 5.0)), &[]);
    let v = call(
        &w.w,
        "stats_alpha_validate",
        json!({ "slot": o["slot"], "spec": outside }),
    )
    .unwrap();
    assert_eq!(v["status"], "invalid", "{v}");
    assert_eq!(v["errors"][0]["code"], "stats.alpha.reversed_outside_scale");
    assert_eq!(v["errors"][0]["params"], json!({ "column": 2, "count": 1 }));
    // One error: the reversed item's, and no other item's.
    assert_eq!(v["errors"].as_array().unwrap().len(), 1, "{v}");
    let run = start(&w, &o, aspec(&o, 2, &FOUR, &["C2"], Some((1.0, 5.0)), &[])).unwrap();
    let s = until_terminal_alpha(&w.w, run);
    assert_eq!(s["state"], "refused", "{s}");
    assert_eq!(s["stepsDone"], 0, "{s}");
    assert_eq!(
        s["diagnostics"][0]["code"], "stats.alpha.reversed_outside_scale",
        "{s}"
    );
    assert!(
        seen.lock().unwrap().is_empty(),
        "no check point was reached"
    );
    assert!(call(&w.w, "stats_alpha_result", json!({ "runId": run })).is_err());
    // Ticked, the same reversal runs.
    let run = start(&w, &o, full(&o, 3)).unwrap();
    assert_eq!(until_terminal_alpha(&w.w, run)["state"], "succeeded");
}

#[test]
fn every_command_refuses_while_statistics_is_inactive() {
    let dir = TempDir::new("alpha-inactive");
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
    for (cmd, body) in [
        ("stats_alpha_validate", json!({ "slot": 1, "spec": spec })),
        ("stats_alpha_run", json!({ "slot": 1, "spec": spec })),
        ("stats_alpha_status", json!({ "runId": 1 })),
        ("stats_alpha_result", json!({ "runId": 1 })),
        ("stats_alpha_cancel", json!({ "runId": 1 })),
        ("stats_alpha_export", json!({ "runId": 1, "format": "csv" })),
    ] {
        assert_eq!(
            code_of(&call(&w, cmd, body).unwrap_err()),
            "workspace.inactive",
            "{cmd}"
        );
    }
}

#[test]
fn another_kinds_run_is_refused_both_ways() {
    let w = world(None);
    let o = open(&w);
    let a = start(&w, &o, aspec(&o, 1, &["C1", "C3"], &[], None, &[])).unwrap();
    until_terminal_alpha(&w.w, a);
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
        ("stats_desc", a),
        ("stats_ttest", a),
        ("stats_anova", a),
        ("stats_corr", a),
        ("stats_alpha", d),
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
    assert_eq!(checked, 20);
    assert_eq!(
        code_of(&call(&w.w, "stats_alpha_status", json!({ "runId": 99 })).unwrap_err()),
        "run.unknown_run"
    );
    let s = call(&w.w, "workspace_state", json!({})).unwrap();
    assert_eq!(s["latestRun"], json!({ "runId": d, "kind": "describe" }));
}

/* ---- progress, cancel and the ceiling (decision 380) ---------------------------- */

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
fn the_progress_counts_check_points_from_the_spec() {
    let dataset = {
        let dir = TempDir::new("alpha-progress-data");
        let p = dir.write("data.csv", &csv());
        load_dataset(&p).expect("loads")
    };
    let ends = Some((1.0, 5.0));
    for (items, reversed, scale) in [
        (&FOUR[..], &["C2"][..], ends),
        (&FOUR[..3], &[][..], None),
        (&["C1", "C4"][..], &[][..], None),
        (&["C2", "C3", "C4"][..], &["C2", "C4"][..], ends),
    ] {
        let (hook, seen) = recording();
        let clock = Arc::new(TestClock(AtomicU64::new(0)));
        let w = world(Some(runs_with(clock, hook)));
        let o = open(&w);
        let run = start(&w, &o, aspec(&o, 1, items, reversed, scale, &[-9.0])).unwrap();
        let s = until_terminal_alpha(&w.w, run);
        assert_eq!(s["state"], "succeeded", "{s}");
        // The core's own count, from the spec alone.
        let at = |v: &[&str]| -> Vec<u32> { v.iter().map(|a| a[1..].parse().unwrap()).collect() };
        let p = direct_params(&at(items), &at(reversed), scale, &[-9.0]);
        let total = stats_alpha::check_points(&p);
        let k = items.len();
        assert_eq!(total, 1 + k + k * (k - 1) / 2);
        assert_eq!(s["stepsTotal"], total as u64, "{items:?}");
        assert_eq!(s["stepsDone"], total as u64);
        // Every check point, in order, from 0.
        assert_eq!(
            *seen.lock().unwrap(),
            (0..total).collect::<Vec<_>>(),
            "{items:?}"
        );
        // And the run's result is the module's.
        let r = call(&w.w, "stats_alpha_result", json!({ "runId": run })).unwrap();
        let direct = stats_alpha::compute(&dataset, &p).unwrap();
        assert_eq!(r["result"], through_text(&direct));
    }
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
fn a_cancel_during_the_inter_item_correlations_stops_the_run_with_no_result() {
    // Check point 7 is before the third inter-item correlation, (1, 4): the
    // arithmetic is 0 and the four items' rows are 1 to 4.
    let (hook, parked, go) = park_when(|p| p == Point::Column(7));
    let clock = Arc::new(TestClock(AtomicU64::new(0)));
    let w = world(Some(runs_with(clock, hook)));
    let o = open(&w);
    let run = start(&w, &o, full(&o, 1)).unwrap();
    assert_eq!(
        parked
            .recv_timeout(Duration::from_secs(20))
            .expect("parked"),
        Point::Column(7)
    );
    let c = call(&w.w, "stats_alpha_cancel", json!({ "runId": run })).unwrap();
    assert_eq!(c["state"], "running", "{c}");
    assert_eq!(c["cancelRequested"], true);
    go.send(()).unwrap();
    let s = until_terminal_alpha(&w.w, run);
    assert_eq!(s["state"], "cancelled", "{s}");
    assert_eq!(
        s["stepsDone"], 7,
        "stopped at the check point it was cancelled at: {s}"
    );
    assert_eq!(s["stepsTotal"], 11);
    for (cmd, body) in [
        ("stats_alpha_result", json!({ "runId": run })),
        (
            "stats_alpha_export",
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
    // At check point 9 (the inter-item correlation (2, 4)), the check point
    // "sleeps" 31 s on the injected clock: the manager's own ceiling, 30 s,
    // stops it there.
    let clock = Arc::new(TestClock(AtomicU64::new(1_000)));
    let c = clock.clone();
    let hook: CheckpointHook = Arc::new(move |_, point| {
        if point == Point::Column(9) {
            c.0.fetch_add(31_000, Ordering::SeqCst);
        }
    });
    let w = world(Some(runs_with(clock, hook)));
    let o = open(&w);
    let run = start(&w, &o, full(&o, 1)).unwrap();
    let s = until_terminal_alpha(&w.w, run);
    assert_eq!(s["state"], "timed_out", "{s}");
    assert_eq!(s["stepsDone"], 9, "{s}");
    assert_eq!(
        code_of(&call(&w.w, "stats_alpha_result", json!({ "runId": run })).unwrap_err()),
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
    assert_eq!(until_terminal_alpha(&w.w, run)["state"], "succeeded");
}

/// Decision 338: one choice, one spec, one parameters hash. The same spec
/// run twice gives the same hash, a different choice a different one, and
/// the same choice sent out of order is refused, never sorted silently.
#[test]
fn one_choice_gives_one_parameters_hash_and_another_order_is_refused() {
    let w = world(None);
    let o = open(&w);
    let hash = |spec: Value| {
        let run = start(&w, &o, spec).unwrap();
        assert_eq!(until_terminal_alpha(&w.w, run)["state"], "succeeded");
        call(&w.w, "stats_alpha_result", json!({ "runId": run })).unwrap()["manifest"]
            ["parametersHash"]
            .clone()
    };
    let one = hash(full(&o, 1));
    let again = hash(full(&o, 2));
    assert_eq!(one, again);
    let unreversed = hash(aspec(&o, 3, &FOUR, &[], None, &[-9.0]));
    assert_ne!(one, unreversed);
    for spec in [
        aspec(
            &o,
            4,
            &["C4", "C3", "C2", "C1"],
            &["C2"],
            Some((1.0, 5.0)),
            &[-9.0],
        ),
        aspec(&o, 5, &FOUR, &["C4", "C2"], Some((1.0, 5.0)), &[-9.0]),
        aspec(&o, 6, &FOUR, &["C2"], Some((1.0, 5.0)), &[-9.0, -9.0]),
    ] {
        let v = call(
            &w.w,
            "stats_alpha_validate",
            json!({ "slot": o["slot"], "spec": spec }),
        )
        .unwrap();
        assert_eq!(v["status"], "invalid", "{v}");
    }
}
