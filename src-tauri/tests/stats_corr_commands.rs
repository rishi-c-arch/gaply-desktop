//! **The correlations' commands at their boundary (Research OS, Phase 8H-2,
//! decisions 355 and 356).** Every call goes through Tauri's IPC on the mock
//! runtime, with the webview's argument names, into the state
//! `AppState::new` builds, and every assertion reads the JSON the webview
//! reads.
//!
//! * The flow: open → validate → run → status → result → export (CSV and
//!   manifest) → close, the numbers equal to `stats_corr`'s on the same
//!   dataset, and the CSV equal to its export.
//! * The progress counts check points (decision 355): the record's total is
//!   `stats_corr::check_points`, from the spec alone; every check point
//!   reaches the hook in order; a result leaves done equal to the total.
//! * A cancel during the pairs stops the run there, with no result; the run
//!   manager's ceiling stops a run whose check point "sleeps" past it, on an
//!   injected clock, never 30 s of real time.
//! * A spec over the budget is refused by validation with its pairs and rows,
//!   and `stats_corr_run` never starts it: the run is `refused` and no check
//!   point is reached.
//! * Refusals, each with its exact code, and the registry's `capability.*`
//!   codes as they are (decision 74), before the slot is read.
//! * Statistics inactive: all six commands refuse `workspace.inactive`.
//! * `run.wrong_kind` both ways against the summary, the t-test and the
//!   ANOVA.
//!
//! Every file is synthetic and in a folder the test creates and removes.

mod corr_support;
mod desc_support;

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::{json, Value};

use app_lib::runs::{CheckpointHook, Clock, Point, Runs};
use app_lib::stats_l1_ingest::load_dataset;
use corr_support::{app, app_with, cspec, until_terminal_corr};
use desc_support::{call, delimited, refusal, spec as desc_spec, until_terminal, TempDir, C};
use gaply_core::contracts::{ContentHash, RunManifest};
use gaply_core::stats_corr::export::{manifest_json, result_csv};
use gaply_core::stats_corr::{self, Coefficient, CorrCode, CorrParams, MissingHandling};
use gaply_core::stats_desc::Alias;
use gaply_core::stats_l1::dataset::ColumnAlias;
use gaply_core::Database;

/* ---- fixtures --------------------------------------------------------------- */

const HEADERS: [&str; 6] = ["a", "b", "c", "d", "note", "flat"];

/// Four numeric columns of twelve rows with cells missing in different rows
/// and one -9, a text column and a constant column.
fn rows() -> Vec<Vec<C<'static>>> {
    const A: [&str; 12] = [
        "1.5", "2.25", "", "4.5", "5", "6.75", "7", "8.5", "9", "10.25", "11", "12",
    ];
    const B: [&str; 12] = [
        "2", "1.5", "3.25", "", "6", "5.5", "8", "7.25", "-9", "11.5", "10", "13",
    ];
    const CC: [&str; 12] = [
        "9", "8.5", "7", "6.25", "", "4", "3.5", "2", "1.25", "0.5", "-1", "-2.5",
    ];
    const D: [&str; 12] = ["3", "7", "1", "8", "2", "9", "4", "", "6", "5", "10", "0"];
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
    let dir = TempDir::new("corr");
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

const ALL: [&str; 3] = ["pearson", "spearman", "kendall"];
const FOUR: [&str; 4] = ["C1", "C2", "C3", "C4"];

/// The analysis the flow runs: the four numeric columns, all three
/// coefficients, pairwise, -9 ticked.
fn full(o: &Value, op: u64) -> Value {
    cspec(o, op, &FOUR, &ALL, "pairwise", &[-9.0])
}

fn start(w: &World, o: &Value, spec: Value) -> Result<u64, Value> {
    call(
        &w.w,
        "stats_corr_run",
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
    variables: &[u32],
    coefficients: &[Coefficient],
    missing: MissingHandling,
    codes: &[f64],
) -> CorrParams {
    CorrParams {
        variables: variables.iter().map(|v| Alias(ColumnAlias(*v))).collect(),
        coefficients: coefficients.to_vec(),
        missing,
        missing_codes: codes.to_vec(),
    }
}

const THREE: [Coefficient; 3] = [
    Coefficient::Pearson,
    Coefficient::Spearman,
    Coefficient::Kendall,
];

/* ---- the flow --------------------------------------------------------------- */

#[test]
fn open_validate_run_status_result_export_close_through_ipc() {
    let w = world(None);
    let o = open(&w);
    let dataset = load_dataset(&w.data).expect("the same file");

    let v = call(
        &w.w,
        "stats_corr_validate",
        json!({ "slot": o["slot"], "spec": full(&o, 1) }),
    )
    .unwrap();
    assert_eq!(v["status"], "valid", "{v}");
    assert_eq!(v["errors"], json!([]));
    assert_eq!(v["missingInformation"], json!([]));

    let run = start(&w, &o, full(&o, 2)).unwrap();
    let s = until_terminal_corr(&w.w, run);
    assert_eq!(s["state"], "succeeded", "{s}");
    assert_eq!(s["cancelRequested"], false);
    // Decision 355: the progress counts check points, and a result has done
    // them all. Four variables, three coefficients: 1 + 3 × 6.
    assert_eq!(s["stepsTotal"], 19, "{s}");
    assert_eq!(s["stepsDone"], 19, "{s}");
    assert!(s.get("columnsDone").is_none(), "steps, never columns: {s}");

    let r = call(&w.w, "stats_corr_result", json!({ "runId": run })).unwrap();
    assert_eq!(r["runId"], run);
    assert_eq!(r["status"], "succeeded");
    let direct = stats_corr::compute(
        &dataset,
        &direct_params(&[1, 2, 3, 4], &THREE, MissingHandling::Pairwise, &[-9.0]),
    )
    .expect("computes");
    assert_eq!(r["result"], through_text(&direct));
    assert_eq!(r["diagnostics"], json!([]));
    assert_eq!(
        r["result"]["limitations"],
        json!([
            "stats.corr.pearson_assumes_normality",
            "stats.corr.two_tailed_no_partial",
        ])
    );
    let m = &r["manifest"];
    assert_eq!(m["capabilityId"], "stats.analyze.correlations");
    assert_eq!(
        m["outputHash"],
        ContentHash::of_bytes(&direct.canonical_bytes()).as_hex(),
        "bit for bit: the result the app holds is the module's"
    );
    assert_eq!(m["rowsRead"], 12);

    // The exports: the CSV is the module's export of that result and its
    // manifest, and the hash returned is the bytes'.
    let manifest: RunManifest<CorrCode> = serde_json::from_value(m.clone()).unwrap();
    let e = call(
        &w.w,
        "stats_corr_export",
        json!({ "runId": run, "format": "csv" }),
    )
    .unwrap();
    let bytes: Vec<u8> = serde_json::from_value(e["bytes"].clone()).unwrap();
    assert_eq!(bytes, result_csv(&direct, &manifest));
    assert_eq!(e["sha256"], ContentHash::of_bytes(&bytes).as_hex());
    let j = call(
        &w.w,
        "stats_corr_export",
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
            "stats_corr_validate",
            json!({ "slot": o["slot"], "spec": spec }),
        )
        .unwrap()
    };
    for (spec, code) in [
        (
            cspec(&o, 1, &["C1"], &ALL, "pairwise", &[]),
            "stats.corr.too_few_variables",
        ),
        (
            cspec(&o, 2, &["C2", "C1"], &ALL, "pairwise", &[]),
            "stats.corr.variables_out_of_order",
        ),
        (
            cspec(&o, 3, &["C1", "C1"], &ALL, "pairwise", &[]),
            "stats.corr.variable_repeated",
        ),
        (
            cspec(&o, 4, &FOUR, &[], "pairwise", &[]),
            "stats.corr.no_coefficient",
        ),
        (
            cspec(&o, 5, &FOUR, &["kendall", "pearson"], "listwise", &[]),
            "stats.corr.coefficients_out_of_order",
        ),
        (
            cspec(&o, 6, &FOUR, &ALL, "pairwise", &[99.0, -9.0]),
            "stats.corr.missing_codes_out_of_order",
        ),
        (
            cspec(&o, 7, &["C1", "C5"], &ALL, "pairwise", &[]),
            "stats.corr.variable_holds_text",
        ),
        (
            cspec(&o, 8, &["C1", "C9"], &ALL, "pairwise", &[]),
            "stats.corr.unknown_column",
        ),
    ] {
        let v = validate(spec);
        assert_eq!(v["status"], "invalid", "{code}: {v}");
        assert_eq!(v["errors"][0]["code"], code, "{v}");
    }
    // The registry before the slot: a version mismatch is refused as such
    // for a slot no dataset has, as the registry words it, and makes no run.
    let mut s = full(&o, 9);
    s["capabilityVersion"] = json!(2);
    for cmd in ["stats_corr_validate", "stats_corr_run"] {
        let e = call(&w.w, cmd, json!({ "slot": 999, "spec": s })).unwrap_err();
        assert_eq!(code_of(&e), "capability.version_mismatch", "{cmd}");
    }
    // A spec of another capability through the correlations' door.
    let mut t = full(&o, 10);
    t["capabilityId"] = json!("stats.verify.pearson");
    let e = call(
        &w.w,
        "stats_corr_run",
        json!({ "slot": o["slot"], "spec": t }),
    )
    .unwrap_err();
    assert!(code_of(&e).starts_with("capability."), "{e}");
}

/// Decision 355: over the budget, validation refuses with the pairs and the
/// rows, and a run of that spec is refused before any arithmetic: no check
/// point is reached. Pearson alone on the same file is within it.
#[test]
fn a_spec_over_the_budget_is_refused_and_never_starts() {
    // 128 columns (the ingest's widest) of 7,000 rows: 8,128 pairs; all three
    // coefficients are estimated at about 17.3 s, Pearson alone at about
    // 0.74 s.
    let k = 128usize;
    let n = 7000usize;
    let headers: Vec<String> = (0..k).map(|j| format!("v{j}")).collect();
    let heads: Vec<&str> = headers.iter().map(String::as_str).collect();
    let values: Vec<Vec<String>> = (0..n)
        .map(|i| {
            (0..k)
                .map(|j| format!("{}", (i * 7 + j * 13) % 101))
                .collect()
        })
        .collect();
    let cells: Vec<Vec<C>> = values
        .iter()
        .map(|r| r.iter().map(|v| C::N(v.as_str())).collect())
        .collect();
    let (hook, seen) = recording();
    let w = world_on(
        Some(runs_with(Arc::new(TestClock(AtomicU64::new(0))), hook)),
        &delimited(&heads, &cells, ",", "\n"),
    );
    let o = open(&w);
    let aliases: Vec<String> = (1..=k).map(|j| format!("C{j}")).collect();
    let vars: Vec<&str> = aliases.iter().map(String::as_str).collect();
    let all = cspec(&o, 1, &vars, &ALL, "listwise", &[]);
    let p = direct_params(
        &(1..=k as u32).collect::<Vec<_>>(),
        &THREE,
        MissingHandling::Listwise,
        &[],
    );
    assert!(stats_corr::estimated_seconds(&p, n) > stats_corr::BUDGET_SECONDS);
    let v = call(
        &w.w,
        "stats_corr_validate",
        json!({ "slot": o["slot"], "spec": all }),
    )
    .unwrap();
    assert_eq!(v["status"], "invalid", "{v}");
    assert_eq!(v["errors"][0]["code"], "stats.corr.over_budget");
    assert_eq!(
        v["errors"][0]["params"],
        json!({ "count": 8128, "found": 7000 })
    );
    // Pearson alone: within the budget, so not refused for it.
    let pearson = cspec(&o, 2, &vars, &ALL[..1], "listwise", &[]);
    let v = call(
        &w.w,
        "stats_corr_validate",
        json!({ "slot": o["slot"], "spec": pearson }),
    )
    .unwrap();
    assert_eq!(v["status"], "valid", "{v}");
    // The run of the spec over the budget: refused, no check point reached,
    // no result.
    let run = start(&w, &o, cspec(&o, 3, &vars, &ALL, "listwise", &[])).unwrap();
    let s = until_terminal_corr(&w.w, run);
    assert_eq!(s["state"], "refused", "{s}");
    assert_eq!(s["stepsDone"], 0, "{s}");
    assert_eq!(s["diagnostics"][0]["code"], "stats.corr.over_budget", "{s}");
    assert!(
        seen.lock().unwrap().is_empty(),
        "no check point was reached"
    );
    assert!(call(&w.w, "stats_corr_result", json!({ "runId": run })).is_err());
}

#[test]
fn every_command_refuses_while_statistics_is_inactive() {
    let dir = TempDir::new("corr-inactive");
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
        ("stats_corr_validate", json!({ "slot": 1, "spec": spec })),
        ("stats_corr_run", json!({ "slot": 1, "spec": spec })),
        ("stats_corr_status", json!({ "runId": 1 })),
        ("stats_corr_result", json!({ "runId": 1 })),
        ("stats_corr_cancel", json!({ "runId": 1 })),
        ("stats_corr_export", json!({ "runId": 1, "format": "csv" })),
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
    let r = start(&w, &o, cspec(&o, 1, &FOUR, &ALL[..1], "listwise", &[])).unwrap();
    until_terminal_corr(&w.w, r);
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
        ("stats_desc", r),
        ("stats_ttest", r),
        ("stats_anova", r),
        ("stats_corr", d),
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
        code_of(&call(&w.w, "stats_corr_status", json!({ "runId": 99 })).unwrap_err()),
        "run.unknown_run"
    );
    let s = call(&w.w, "workspace_state", json!({})).unwrap();
    assert_eq!(s["latestRun"], json!({ "runId": d, "kind": "describe" }));
}

/* ---- progress, cancel and the ceiling (decision 355) ---------------------------- */

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
        let dir = TempDir::new("corr-progress-data");
        let p = dir.write("data.csv", &csv());
        load_dataset(&p).expect("loads")
    };
    for (vars, coefs, missing) in [
        (&FOUR[..], &ALL[..], "pairwise"),
        (&FOUR[..3], &ALL[..], "listwise"),
        (&FOUR[..], &["spearman"][..], "pairwise"),
        (&["C1", "C4"][..], &["pearson", "kendall"][..], "listwise"),
    ] {
        let (hook, seen) = recording();
        let clock = Arc::new(TestClock(AtomicU64::new(0)));
        let w = world(Some(runs_with(clock, hook)));
        let o = open(&w);
        let run = start(&w, &o, cspec(&o, 1, vars, coefs, missing, &[-9.0])).unwrap();
        let s = until_terminal_corr(&w.w, run);
        assert_eq!(s["state"], "succeeded", "{s}");
        // The core's own count, from the spec alone.
        let tests: Vec<Coefficient> = coefs
            .iter()
            .map(|c| serde_json::from_value(json!(c)).unwrap())
            .collect();
        let handling: MissingHandling = serde_json::from_value(json!(missing)).unwrap();
        let positions: Vec<u32> = vars.iter().map(|v| v[1..].parse().unwrap()).collect();
        let p = direct_params(&positions, &tests, handling, &[-9.0]);
        let total = stats_corr::check_points(&p);
        let k = vars.len();
        assert_eq!(total, 1 + coefs.len() * k * (k - 1) / 2);
        assert_eq!(s["stepsTotal"], total as u64, "{vars:?} {coefs:?}");
        assert_eq!(s["stepsDone"], total as u64);
        // Every check point, in order, from 0.
        assert_eq!(
            *seen.lock().unwrap(),
            (0..total).collect::<Vec<_>>(),
            "{vars:?} {coefs:?}"
        );
        // And the run's result is the module's.
        let r = call(&w.w, "stats_corr_result", json!({ "runId": run })).unwrap();
        let direct = stats_corr::compute(&dataset, &p).unwrap();
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
fn a_cancel_during_the_pairs_stops_the_run_with_no_result() {
    // Check point 10 is Spearman's fourth pair (Pearson's are 1 to 6).
    let (hook, parked, go) = park_when(|p| p == Point::Column(10));
    let clock = Arc::new(TestClock(AtomicU64::new(0)));
    let w = world(Some(runs_with(clock, hook)));
    let o = open(&w);
    let run = start(&w, &o, full(&o, 1)).unwrap();
    assert_eq!(
        parked
            .recv_timeout(Duration::from_secs(20))
            .expect("parked"),
        Point::Column(10)
    );
    let c = call(&w.w, "stats_corr_cancel", json!({ "runId": run })).unwrap();
    assert_eq!(c["state"], "running", "{c}");
    assert_eq!(c["cancelRequested"], true);
    go.send(()).unwrap();
    let s = until_terminal_corr(&w.w, run);
    assert_eq!(s["state"], "cancelled", "{s}");
    assert_eq!(
        s["stepsDone"], 10,
        "stopped at the check point it was cancelled at: {s}"
    );
    assert_eq!(s["stepsTotal"], 19);
    for (cmd, body) in [
        ("stats_corr_result", json!({ "runId": run })),
        (
            "stats_corr_export",
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
    // At check point 14 (a Kendall pair), the check point "sleeps" 31 s on
    // the injected clock: the manager's own ceiling, 30 s, stops it there.
    let clock = Arc::new(TestClock(AtomicU64::new(1_000)));
    let c = clock.clone();
    let hook: CheckpointHook = Arc::new(move |_, point| {
        if point == Point::Column(14) {
            c.0.fetch_add(31_000, Ordering::SeqCst);
        }
    });
    let w = world(Some(runs_with(clock, hook)));
    let o = open(&w);
    let run = start(&w, &o, full(&o, 1)).unwrap();
    let s = until_terminal_corr(&w.w, run);
    assert_eq!(s["state"], "timed_out", "{s}");
    assert_eq!(s["stepsDone"], 14, "{s}");
    assert_eq!(
        code_of(&call(&w.w, "stats_corr_result", json!({ "runId": run })).unwrap_err()),
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
    assert_eq!(until_terminal_corr(&w.w, run)["state"], "succeeded");
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
        assert_eq!(until_terminal_corr(&w.w, run)["state"], "succeeded");
        call(&w.w, "stats_corr_result", json!({ "runId": run })).unwrap()["manifest"]
            ["parametersHash"]
            .clone()
    };
    let one = hash(cspec(&o, 1, &FOUR, &ALL, "pairwise", &[-9.0]));
    let again = hash(cspec(&o, 2, &FOUR, &ALL, "pairwise", &[-9.0]));
    assert_eq!(one, again);
    let listwise = hash(cspec(&o, 3, &FOUR, &ALL, "listwise", &[-9.0]));
    assert_ne!(one, listwise);
    for spec in [
        cspec(&o, 4, &["C4", "C3", "C2", "C1"], &ALL, "pairwise", &[-9.0]),
        cspec(
            &o,
            5,
            &FOUR,
            &["spearman", "pearson", "kendall"],
            "pairwise",
            &[-9.0],
        ),
        cspec(&o, 6, &FOUR, &ALL, "pairwise", &[-9.0, -9.0]),
    ] {
        let v = call(
            &w.w,
            "stats_corr_validate",
            json!({ "slot": o["slot"], "spec": spec }),
        )
        .unwrap();
        assert_eq!(v["status"], "invalid", "{v}");
    }
}
