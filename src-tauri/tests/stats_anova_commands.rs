//! **The one-way ANOVA's commands at their boundary (Research OS, Phase
//! 8G-2, decisions 318, 328 and 329).** Every call goes through Tauri's IPC
//! on the mock runtime, with the webview's argument names, into the state
//! `AppState::new` builds, and every assertion reads the JSON the webview
//! reads.
//!
//! * The flow: open → group levels → validate → run → status → result →
//!   export (CSV and manifest) → close, the numbers equal to
//!   `stats_anova`'s on the same dataset, and the CSV equal to its export.
//! * The progress counts check points (decision 328): the record's total is
//!   `stats_anova::check_points` for the included levels, from the spec when
//!   it names them and from the dataset when it does not; every check point
//!   reaches the hook in order; a result leaves done equal to the total.
//! * A cancel during the post hoc tests stops the run there, with no result;
//!   the run manager's ceiling stops a run whose check point "sleeps" past
//!   it, on an injected clock, never 30 s of real time.
//! * Refusals, each with its exact code, and the registry's `capability.*`
//!   codes as they are (decision 74), before the slot is read.
//! * Statistics inactive: all seven commands refuse `workspace.inactive`.
//! * `run.wrong_kind` both ways against the summary and the t-test.
//!
//! Every file is synthetic and in a folder the test creates and removes.

mod anova_support;
mod desc_support;

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::{json, Value};

use anova_support::{app, app_with, aspec, until_terminal_anova};
use app_lib::runs::{CheckpointHook, Clock, Point, Runs};
use app_lib::stats_l1_ingest::load_dataset;
use desc_support::{call, delimited, refusal, spec as desc_spec, until_terminal, TempDir, C};
use gaply_core::contracts::{ContentHash, RunManifest};
use gaply_core::stats_anova::export::{manifest_json, result_csv};
use gaply_core::stats_anova::{self, AnovaCode, AnovaParams, PostHoc};
use gaply_core::stats_desc::Alias;
use gaply_core::stats_l1::dataset::ColumnAlias;
use gaply_core::Database;

/* ---- fixtures --------------------------------------------------------------- */

const HEADERS: [&str; 6] = ["score", "arm", "note", "one_level", "tiny", "wide"];

/// Six levels, a to f, four rows each, with unequal variances; a text
/// column; a group of one level; a group whose level `z` has one value; and
/// a group of 51 levels.
fn rows() -> Vec<Vec<C<'static>>> {
    const SCORES: [&str; 24] = [
        "1.5", "2.25", "3.75", "2.5", // a
        "4.5", "6.125", "7", "5.25", // b
        "9.5", "12.75", "8", "15.5", // c
        "3", "3.5", "4.25", "2.75", // d
        "11", "10.5", "13.25", "12", // e
        "6.5", "7.75", "5.5", "9", // f
    ];
    const ARMS: [&str; 6] = ["a", "b", "c", "d", "e", "f"];
    const WIDE: [&str; 51] = [
        "w00", "w01", "w02", "w03", "w04", "w05", "w06", "w07", "w08", "w09", "w10", "w11", "w12",
        "w13", "w14", "w15", "w16", "w17", "w18", "w19", "w20", "w21", "w22", "w23", "w24", "w25",
        "w26", "w27", "w28", "w29", "w30", "w31", "w32", "w33", "w34", "w35", "w36", "w37", "w38",
        "w39", "w40", "w41", "w42", "w43", "w44", "w45", "w46", "w47", "w48", "w49", "w50",
    ];
    (0..51)
        .map(|i| {
            let in_six = i < 24;
            vec![
                if in_six { C::N(SCORES[i]) } else { C::N("5") },
                if in_six { C::T(ARMS[i / 4]) } else { C::Empty },
                C::T("t"),
                C::T("only"),
                C::T(if i == 0 {
                    "z"
                } else if i % 2 == 0 {
                    "x"
                } else {
                    "y"
                }),
                C::T(WIDE[i]),
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
    let dir = TempDir::new("anova");
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

const ALL: [&str; 3] = ["tukey", "games_howell", "bonferroni"];

/// The analysis the flow runs: `score` by `arm`, every level, all three
/// tests.
fn full(o: &Value, op: u64) -> Value {
    aspec(o, op, "C1", "C2", None, &ALL, &[])
}

fn start(w: &World, o: &Value, spec: Value) -> Result<u64, Value> {
    call(
        &w.w,
        "stats_anova_run",
        json!({ "slot": o["slot"], "spec": spec }),
    )
    .map(|v| v["runId"].as_u64().expect("runId"))
}

fn code_of(err: &Value) -> String {
    refusal(err)["code"].as_str().unwrap().to_string()
}

/// JSON text and back, as the view went (`stats_onesample_commands.rs`'s
/// note): the bit-for-bit claim rests on the output hash and the CSV bytes.
fn through_text<T: serde::Serialize>(v: &T) -> Value {
    serde_json::from_str(&serde_json::to_string(v).unwrap()).unwrap()
}

fn direct_params(levels: Option<Vec<u32>>, post_hoc: Vec<PostHoc>) -> AnovaParams {
    AnovaParams {
        outcome: Alias(ColumnAlias(1)),
        group: Alias(ColumnAlias(2)),
        levels,
        post_hoc,
        missing_codes: vec![],
    }
}

const THREE: [PostHoc; 3] = [PostHoc::Tukey, PostHoc::GamesHowell, PostHoc::Bonferroni];

/* ---- the flow --------------------------------------------------------------- */

#[test]
fn open_levels_validate_run_status_result_export_close_through_ipc() {
    let w = world(None);
    let o = open(&w);
    let dataset = load_dataset(&w.data).expect("the same file");

    // The levels, as the ANOVA indexes them, labels and counts.
    let l = call(
        &w.w,
        "stats_anova_group_levels",
        json!({ "slot": o["slot"], "column": "C2" }),
    )
    .unwrap();
    assert_eq!(l["slot"], o["slot"]);
    assert_eq!(l["column"], "C2");
    let labels: Vec<&str> = l["levels"]
        .as_array()
        .unwrap()
        .iter()
        .map(|x| x["label"].as_str().unwrap())
        .collect();
    assert_eq!(labels, ["a", "b", "c", "d", "e", "f"]);
    assert_eq!(
        l["levels"][2],
        json!({ "index": 2, "label": "c", "count": 4 })
    );

    let v = call(
        &w.w,
        "stats_anova_validate",
        json!({ "slot": o["slot"], "spec": full(&o, 1) }),
    )
    .unwrap();
    assert_eq!(v["status"], "valid", "{v}");
    assert_eq!(v["errors"], json!([]));
    assert_eq!(v["missingInformation"], json!([]));

    let run = start(&w, &o, full(&o, 2)).unwrap();
    let s = until_terminal_anova(&w.w, run);
    assert_eq!(s["state"], "succeeded", "{s}");
    assert_eq!(s["cancelRequested"], false);
    // Decision 328: the progress counts check points, and a result has done
    // them all. Six levels, three tests: 1 + 3 × 15.
    assert_eq!(s["stepsTotal"], 46, "{s}");
    assert_eq!(s["stepsDone"], 46, "{s}");
    assert!(s.get("columnsDone").is_none(), "steps, never columns: {s}");

    let r = call(&w.w, "stats_anova_result", json!({ "runId": run })).unwrap();
    assert_eq!(r["runId"], run);
    assert_eq!(r["status"], "succeeded");
    let direct =
        stats_anova::compute(&dataset, &direct_params(None, THREE.to_vec())).expect("computes");
    assert_eq!(r["result"], through_text(&direct));
    assert_eq!(r["diagnostics"], json!([]));
    assert_eq!(
        r["result"]["limitations"],
        json!([
            "stats.anova.normality_not_assessed",
            "stats.anova.effect_sizes_have_no_interval",
            "stats.anova.no_contrasts",
        ])
    );
    let m = &r["manifest"];
    assert_eq!(m["capabilityId"], "stats.analyze.one_way_anova");
    assert_eq!(
        m["outputHash"],
        ContentHash::of_bytes(&direct.canonical_bytes()).as_hex(),
        "bit for bit: the result the app holds is the module's"
    );
    assert_eq!(m["rowsUsed"], 24);

    // The exports: the CSV is the module's export of that result and its
    // manifest, and the hash returned is the bytes'.
    let manifest: RunManifest<AnovaCode> = serde_json::from_value(m.clone()).unwrap();
    let e = call(
        &w.w,
        "stats_anova_export",
        json!({ "runId": run, "format": "csv" }),
    )
    .unwrap();
    let bytes: Vec<u8> = serde_json::from_value(e["bytes"].clone()).unwrap();
    assert_eq!(bytes, result_csv(&direct, &manifest));
    assert_eq!(e["sha256"], ContentHash::of_bytes(&bytes).as_hex());
    let j = call(
        &w.w,
        "stats_anova_export",
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
            "stats_anova_validate",
            json!({ "slot": o["slot"], "spec": spec }),
        )
        .unwrap()
    };
    for (spec, status, code) in [
        (
            aspec(&o, 1, "C1", "C1", None, &[], &[]),
            "invalid",
            "stats.anova.same_column",
        ),
        (
            aspec(&o, 2, "C3", "C2", None, &[], &[]),
            "invalid",
            "stats.anova.outcome_holds_text",
        ),
        (
            aspec(&o, 3, "C1", "C4", None, &[], &[]),
            "invalid",
            "stats.anova.group_one_level",
        ),
        (
            aspec(&o, 4, "C1", "C2", Some(&[2]), &[], &[]),
            "invalid",
            "stats.anova.too_few_levels",
        ),
        (
            aspec(&o, 5, "C1", "C2", Some(&[0, 9]), &[], &[]),
            "invalid",
            "stats.anova.level_out_of_range",
        ),
        (
            aspec(&o, 6, "C1", "C5", None, &[], &[]),
            "invalid",
            "stats.anova.group_too_small",
        ),
        (
            aspec(&o, 7, "C1", "C6", None, &[], &[]),
            "invalid",
            "stats.anova.too_many_levels",
        ),
        (
            aspec(&o, 8, "C1", "C9", None, &[], &[]),
            "invalid",
            "stats.anova.unknown_column",
        ),
    ] {
        let v = validate(spec);
        assert_eq!(v["status"], status, "{code}: {v}");
        assert_eq!(v["errors"][0]["code"], code, "{v}");
    }
    // The registry before the slot: a version mismatch is refused as such
    // for a slot no dataset has, as the registry words it, and makes no run.
    let mut s = full(&o, 9);
    s["capabilityVersion"] = json!(2);
    for cmd in ["stats_anova_validate", "stats_anova_run"] {
        let e = call(&w.w, cmd, json!({ "slot": 999, "spec": s })).unwrap_err();
        assert_eq!(code_of(&e), "capability.version_mismatch", "{cmd}");
    }
    // A spec of another capability through the ANOVA's door.
    let mut t = full(&o, 10);
    t["capabilityId"] = json!("stats.describe");
    let e = call(
        &w.w,
        "stats_anova_run",
        json!({ "slot": o["slot"], "spec": t }),
    )
    .unwrap_err();
    assert!(code_of(&e).starts_with("capability."), "{e}");
    // The levels command: an unknown column and a group of too many levels.
    for (column, code) in [
        ("C9", "stats.anova.unknown_column"),
        ("C6", "stats.anova.too_many_levels"),
    ] {
        let e = call(
            &w.w,
            "stats_anova_group_levels",
            json!({ "slot": o["slot"], "column": column }),
        )
        .unwrap_err();
        assert_eq!(code_of(&e), code, "{column}");
    }
    let e = call(
        &w.w,
        "stats_anova_group_levels",
        json!({ "slot": o["slot"], "column": "C6" }),
    )
    .unwrap_err();
    assert_eq!(refusal(&e)["params"], json!({ "count": 51, "limit": 50 }));
}

#[test]
fn every_command_refuses_while_statistics_is_inactive() {
    let dir = TempDir::new("anova-inactive");
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
        ("stats_anova_validate", json!({ "slot": 1, "spec": spec })),
        ("stats_anova_run", json!({ "slot": 1, "spec": spec })),
        ("stats_anova_status", json!({ "runId": 1 })),
        ("stats_anova_result", json!({ "runId": 1 })),
        ("stats_anova_cancel", json!({ "runId": 1 })),
        ("stats_anova_export", json!({ "runId": 1, "format": "csv" })),
        (
            "stats_anova_group_levels",
            json!({ "slot": 1, "column": "C2" }),
        ),
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
    let a = start(&w, &o, aspec(&o, 1, "C1", "C2", None, &[], &[])).unwrap();
    until_terminal_anova(&w.w, a);
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
    for (prefix, theirs) in [("stats_desc", a), ("stats_ttest", a), ("stats_anova", d)] {
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
        code_of(&call(&w.w, "stats_anova_status", json!({ "runId": 99 })).unwrap_err()),
        "run.unknown_run"
    );
    let s = call(&w.w, "workspace_state", json!({})).unwrap();
    assert_eq!(s["latestRun"], json!({ "runId": d, "kind": "describe" }));
}

/* ---- progress, cancel and the ceiling (decision 328) ---------------------------- */

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
fn the_progress_counts_check_points_from_the_spec_or_the_dataset() {
    let dataset = {
        let dir = TempDir::new("anova-progress-data");
        let p = dir.write("data.csv", &csv());
        load_dataset(&p).expect("loads")
    };
    for (levels, post_hoc, k) in [
        (None, &ALL[..], 6usize),
        (Some(&[0u32, 2, 4][..]), &ALL[..], 3),
        (None, &["games_howell"][..], 6),
        (Some(&[1u32, 5][..]), &["tukey", "bonferroni"][..], 2),
        (None, &[][..], 6),
    ] {
        let (hook, seen) = recording();
        let clock = Arc::new(TestClock(AtomicU64::new(0)));
        let w = world(Some(runs_with(clock, hook)));
        let o = open(&w);
        let run = start(&w, &o, aspec(&o, 1, "C1", "C2", levels, post_hoc, &[])).unwrap();
        let s = until_terminal_anova(&w.w, run);
        assert_eq!(s["state"], "succeeded", "{s}");
        // The core's own count, for the levels the run includes.
        let mut tests: Vec<PostHoc> = post_hoc
            .iter()
            .map(|t| serde_json::from_value(json!(t)).unwrap())
            .collect();
        tests.sort_unstable();
        let total = stats_anova::check_points(
            &direct_params(levels.map(<[u32]>::to_vec), tests.clone()),
            k,
        );
        assert_eq!(total, 1 + post_hoc.len() * k * (k - 1) / 2);
        assert_eq!(s["stepsTotal"], total as u64, "{levels:?} {post_hoc:?}");
        assert_eq!(s["stepsDone"], total as u64);
        // Every check point, in order, from 0.
        assert_eq!(
            *seen.lock().unwrap(),
            (0..total).collect::<Vec<_>>(),
            "{levels:?} {post_hoc:?}"
        );
        // And the run's result is the module's.
        let r = call(&w.w, "stats_anova_result", json!({ "runId": run })).unwrap();
        let direct =
            stats_anova::compute(&dataset, &direct_params(levels.map(<[u32]>::to_vec), tests))
                .unwrap();
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
fn a_cancel_during_the_post_hoc_tests_stops_the_run_with_no_result() {
    // Check point 20 is the fifth pair of Games-Howell (Tukey's are 1 to 15).
    let (hook, parked, go) = park_when(|p| p == Point::Column(20));
    let clock = Arc::new(TestClock(AtomicU64::new(0)));
    let w = world(Some(runs_with(clock, hook)));
    let o = open(&w);
    let run = start(&w, &o, full(&o, 1)).unwrap();
    assert_eq!(
        parked
            .recv_timeout(Duration::from_secs(20))
            .expect("parked"),
        Point::Column(20)
    );
    let c = call(&w.w, "stats_anova_cancel", json!({ "runId": run })).unwrap();
    assert_eq!(c["state"], "running", "{c}");
    assert_eq!(c["cancelRequested"], true);
    go.send(()).unwrap();
    let s = until_terminal_anova(&w.w, run);
    assert_eq!(s["state"], "cancelled", "{s}");
    assert_eq!(
        s["stepsDone"], 20,
        "stopped at the check point it was cancelled at: {s}"
    );
    assert_eq!(s["stepsTotal"], 46);
    for (cmd, body) in [
        ("stats_anova_result", json!({ "runId": run })),
        (
            "stats_anova_export",
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
    // At check point 30 (a Games-Howell pair), the check point "sleeps" 31 s
    // on the injected clock: the manager's own ceiling, 30 s, stops it there.
    let clock = Arc::new(TestClock(AtomicU64::new(1_000)));
    let c = clock.clone();
    let hook: CheckpointHook = Arc::new(move |_, point| {
        if point == Point::Column(30) {
            c.0.fetch_add(31_000, Ordering::SeqCst);
        }
    });
    let w = world(Some(runs_with(clock, hook)));
    let o = open(&w);
    let run = start(&w, &o, full(&o, 1)).unwrap();
    let s = until_terminal_anova(&w.w, run);
    assert_eq!(s["state"], "timed_out", "{s}");
    assert_eq!(s["stepsDone"], 30, "{s}");
    assert_eq!(
        code_of(&call(&w.w, "stats_anova_result", json!({ "runId": run })).unwrap_err()),
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
    assert_eq!(until_terminal_anova(&w.w, run)["state"], "succeeded");
}

/// Decision 329: one choice of levels, one spec, one parameters hash. Every
/// level ticked is `levels: null`; the same levels named in any order are
/// refused unless sorted; the post hoc tests, sorted.
#[test]
fn one_choice_of_levels_gives_one_parameters_hash() {
    let w = world(None);
    let o = open(&w);
    let hash = |spec: Value| {
        let run = start(&w, &o, spec).unwrap();
        assert_eq!(until_terminal_anova(&w.w, run)["state"], "succeeded");
        call(&w.w, "stats_anova_result", json!({ "runId": run })).unwrap()["manifest"]
            ["parametersHash"]
            .clone()
    };
    let all = hash(aspec(&o, 1, "C1", "C2", None, &ALL, &[]));
    let again = hash(aspec(&o, 2, "C1", "C2", None, &ALL, &[]));
    assert_eq!(all, again);
    let named = hash(aspec(
        &o,
        3,
        "C1",
        "C2",
        Some(&[0, 1, 2, 3, 4, 5]),
        &ALL,
        &[],
    ));
    assert_ne!(
        all, named,
        "naming every level is a different spec: the screen sends null"
    );
    let subset = hash(aspec(&o, 4, "C1", "C2", Some(&[0, 2, 5]), &ALL, &[]));
    assert_ne!(all, subset);
}
