//! **The rank tests' commands at their boundary (Research OS, Phase 8Y-1,
//! decision 607).** Every call goes through Tauri's IPC on the mock runtime,
//! with the webview's argument names, into the state `AppState::new`
//! builds, and every assertion reads the JSON the webview reads.
//!
//! * The flow: open → levels → validate → run → status → result → export
//!   (CSV and manifest) → close, the numbers equal to `stats_ranks`'s on the
//!   same dataset, the CSV equal to its export, and one output hash pinned.
//! * The progress counts its one check point: the total is
//!   `stats_ranks::check_points`, and a result has done it.
//! * A cancel at the check point stops the run with no result; the run
//!   manager's ceiling stops a run whose check point "sleeps" past it, on an
//!   injected clock, never 30 s of real time.
//! * Refusals, each with its exact code, the levels command's among them,
//!   and the registry's `capability.*` codes as they are (decision 74),
//!   before the slot is read.
//! * Statistics inactive: all seven commands refuse `workspace.inactive`.
//! * `run.wrong_kind` both ways against the summary and the ANOVA.
//! * One choice of levels gives one parameters hash.
//!
//! Every file is synthetic and in a folder the test creates and removes.

mod desc_support;
mod ranks_support;

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
use gaply_core::stats_ranks::export::{manifest_json, result_csv};
use gaply_core::stats_ranks::{self, IndependentRanksParams, RanksCode};
use gaply_core::Database;
use ranks_support::{app, app_with, rspec, until_terminal_ranks};

/* ---- fixtures --------------------------------------------------------------- */

const HEADERS: [&str; 6] = ["score", "arm", "note", "one_level", "wide", "pair"];

/// 51 rows: a numeric score with ties; arm a, b and c over the first 45
/// rows and empty after; a text column; a column of one level; a column of
/// 51 levels; and a column of two levels.
fn rows() -> Vec<Vec<C<'static>>> {
    const ARMS: [&str; 3] = ["a", "b", "c"];
    const SCORES: [&str; 12] = [
        "3.5", "1", "4.25", "2", "7", "3.5", "5.5", "0.5", "6", "2", "8.75", "4",
    ];
    const NOTES: [&str; 4] = ["n1", "n2", "n3", "n4"];
    const WIDE: [&str; 51] = [
        "w00", "w01", "w02", "w03", "w04", "w05", "w06", "w07", "w08", "w09", "w10", "w11", "w12",
        "w13", "w14", "w15", "w16", "w17", "w18", "w19", "w20", "w21", "w22", "w23", "w24", "w25",
        "w26", "w27", "w28", "w29", "w30", "w31", "w32", "w33", "w34", "w35", "w36", "w37", "w38",
        "w39", "w40", "w41", "w42", "w43", "w44", "w45", "w46", "w47", "w48", "w49", "w50",
    ];
    (0..51)
        .map(|i| {
            vec![
                C::N(SCORES[(i * 5 + i / 3) % 12]),
                if i < 45 { C::T(ARMS[i % 3]) } else { C::Empty },
                C::T(NOTES[i % 4]),
                C::T("only"),
                C::T(WIDE[i]),
                C::T(if i % 2 == 0 { "p" } else { "q" }),
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
    let dir = TempDir::new("ranks");
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

/// The analysis the flow runs: the score by arm, three groups.
fn full(o: &Value, op: u64) -> Value {
    rspec(o, op, "C1", "C2", None, &[])
}

fn start(w: &World, o: &Value, spec: Value) -> Result<u64, Value> {
    call(
        &w.w,
        "stats_ranks_run",
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

fn direct_params(levels: Option<Vec<u32>>) -> IndependentRanksParams {
    IndependentRanksParams {
        outcome: Alias(ColumnAlias(1)),
        group: Alias(ColumnAlias(2)),
        levels,
        missing_codes: vec![],
    }
}

/// The flow's output hash, pinned on macOS aarch64 (10 Nov 2026): the run the
/// app holds is the module's, and the module's is this one. Its p comes
/// through the platform's maths library, as the export's pin says, so another
/// platform checks only that the app's hash is the module's, until its bytes
/// are measured.
const FLOW_OUTPUT_SHA256: &str = "ce6565c36cfaa1fde42308f9ec5af45199b8d72e0598188085719edcdc7b8aba";

/* ---- the flow --------------------------------------------------------------- */

#[test]
fn open_levels_validate_run_status_result_export_close_through_ipc() {
    let w = world(None);
    let o = open(&w);
    let dataset = load_dataset(&w.data).expect("the same file");

    // The levels, as the rank tests index them, labels and counts.
    let l = call(
        &w.w,
        "stats_ranks_group_levels",
        json!({ "slot": o["slot"], "column": "C2" }),
    )
    .unwrap();
    assert_eq!(l["slot"], o["slot"]);
    assert_eq!(l["column"], "C2");
    assert_eq!(
        l["levels"],
        json!([
            { "index": 0, "label": "a", "count": 15 },
            { "index": 1, "label": "b", "count": 15 },
            { "index": 2, "label": "c", "count": 15 },
        ])
    );

    let v = call(
        &w.w,
        "stats_ranks_validate",
        json!({ "slot": o["slot"], "spec": full(&o, 1) }),
    )
    .unwrap();
    assert_eq!(v["status"], "valid", "{v}");
    assert_eq!(v["errors"], json!([]));
    assert_eq!(v["missingInformation"], json!([]));

    let run = start(&w, &o, full(&o, 2)).unwrap();
    let s = until_terminal_ranks(&w.w, run);
    assert_eq!(s["state"], "succeeded", "{s}");
    assert_eq!(s["cancelRequested"], false);
    // One check point, before the arithmetic, and a result has done it.
    assert_eq!(s["stepsTotal"], 1, "{s}");
    assert_eq!(s["stepsDone"], 1, "{s}");
    assert!(s.get("columnsDone").is_none(), "steps, never columns: {s}");

    let r = call(&w.w, "stats_ranks_result", json!({ "runId": run })).unwrap();
    assert_eq!(r["runId"], run);
    assert_eq!(r["status"], "succeeded");
    let direct = stats_ranks::compute(&dataset, &direct_params(None)).expect("computes");
    assert_eq!(r["result"], through_text(&direct));
    assert_eq!(r["result"]["test"]["kruskal_wallis"]["df"], 2.0);
    assert_eq!(
        r["result"]["limitations"],
        json!([
            "stats.ranks.no_pairwise_comparisons",
            "stats.ranks.two_sided_only",
            "stats.ranks.no_location_shift_estimate",
            "stats.ranks.exact_without_ties_only",
            "stats.ranks.no_further_k_sample_tests",
            "stats.ranks.no_weights",
            "stats.ranks.code_point_order",
        ])
    );
    // No warning from the validation, so the bundle's diagnostics are empty.
    assert_eq!(r["diagnostics"], json!([]));
    let m = &r["manifest"];
    assert_eq!(m["capabilityId"], "stats.analyze.independent_ranks");
    assert_eq!(
        m["outputHash"],
        ContentHash::of_bytes(&direct.canonical_bytes()).as_hex(),
        "bit for bit: the result the app holds is the module's"
    );
    if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
        assert_eq!(m["outputHash"], FLOW_OUTPUT_SHA256, "the pinned output hash");
    }
    assert_eq!(m["rowsUsed"], 45);
    assert_eq!(m["rowsRead"], 51);

    // The exports: the CSV is the module's export of that result and its
    // manifest, and the hash returned is the bytes'.
    let manifest: RunManifest<RanksCode> = serde_json::from_value(m.clone()).unwrap();
    let e = call(
        &w.w,
        "stats_ranks_export",
        json!({ "runId": run, "format": "csv" }),
    )
    .unwrap();
    let bytes: Vec<u8> = serde_json::from_value(e["bytes"].clone()).unwrap();
    assert_eq!(bytes, result_csv(&direct, &manifest));
    assert_eq!(e["sha256"], ContentHash::of_bytes(&bytes).as_hex());
    let j = call(
        &w.w,
        "stats_ranks_export",
        json!({ "runId": run, "format": "manifestJson" }),
    )
    .unwrap();
    let jb: Vec<u8> = serde_json::from_value(j["bytes"].clone()).unwrap();
    assert_eq!(jb, manifest_json(&manifest));

    // Two of the levels ticked: the Mann-Whitney test, a then c.
    let run = start(&w, &o, rspec(&o, 3, "C1", "C2", Some(&[0, 2]), &[])).unwrap();
    assert_eq!(until_terminal_ranks(&w.w, run)["state"], "succeeded");
    let r = call(&w.w, "stats_ranks_result", json!({ "runId": run })).unwrap();
    let direct = stats_ranks::compute(&dataset, &direct_params(Some(vec![0, 2]))).unwrap();
    assert_eq!(r["result"], through_text(&direct));
    assert_eq!(r["result"]["test"]["mann_whitney"]["first"], 0);
    assert_eq!(r["result"]["test"]["mann_whitney"]["second"], 2);

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
            "stats_ranks_validate",
            json!({ "slot": o["slot"], "spec": spec }),
        )
        .unwrap()
    };
    for (spec, code) in [
        (rspec(&o, 1, "C1", "C1", None, &[]), "stats.ranks.same_column"),
        (rspec(&o, 2, "C3", "C2", None, &[]), "stats.ranks.outcome_holds_text"),
        (rspec(&o, 3, "C1", "C4", None, &[]), "stats.ranks.group_one_level"),
        (rspec(&o, 4, "C1", "C5", None, &[]), "stats.ranks.too_many_levels"),
        (rspec(&o, 5, "C9", "C2", None, &[]), "stats.ranks.unknown_column"),
        (rspec(&o, 6, "C1", "C2", Some(&[2]), &[]), "stats.ranks.too_few_levels"),
        (
            rspec(&o, 7, "C1", "C2", Some(&[0, 9]), &[]),
            "stats.ranks.level_out_of_range",
        ),
        (
            rspec(&o, 8, "C1", "C2", Some(&[1, 1]), &[]),
            "stats.ranks.level_repeated",
        ),
    ] {
        let v = validate(spec);
        assert_eq!(v["status"], "invalid", "{code}: {v}");
        assert_eq!(v["errors"][0]["code"], code, "{v}");
    }
    // The 51 levels' refusal names the counts, never a label.
    let v = validate(rspec(&o, 9, "C1", "C5", None, &[]));
    assert_eq!(v["errors"][0]["params"], json!({ "count": 51, "limit": 50 }));
    // The levels command's own refusals, in the rank tests' codes.
    for (column, code, params) in [
        ("C9", "stats.ranks.unknown_column", json!({ "column": 9 })),
        (
            "C5",
            "stats.ranks.too_many_levels",
            json!({ "count": 51, "limit": 50 }),
        ),
    ] {
        let e = call(
            &w.w,
            "stats_ranks_group_levels",
            json!({ "slot": o["slot"], "column": column }),
        )
        .unwrap_err();
        assert_eq!(code_of(&e), code, "{column}");
        assert_eq!(refusal(&e)["params"], params, "{column}");
    }
    let e = call(
        &w.w,
        "stats_ranks_group_levels",
        json!({ "slot": 999, "column": "C2" }),
    )
    .unwrap_err();
    assert_eq!(code_of(&e), "run.unknown_input");
    // A refused run, as a view: the run is made and refused by its check.
    let bad = start(&w, &o, rspec(&o, 10, "C1", "C1", None, &[])).unwrap();
    let s = until_terminal_ranks(&w.w, bad);
    assert_eq!(s["state"], "refused", "{s}");
    assert_eq!(s["diagnostics"][0]["code"], "stats.ranks.same_column");
    // The registry before the slot: a version mismatch is refused as such
    // for a slot no dataset has, as the registry words it, and makes no run.
    let mut s = full(&o, 11);
    s["capabilityVersion"] = json!(2);
    for cmd in ["stats_ranks_validate", "stats_ranks_run"] {
        let e = call(&w.w, cmd, json!({ "slot": 999, "spec": s })).unwrap_err();
        assert_eq!(code_of(&e), "capability.version_mismatch", "{cmd}");
    }
    // A spec of another capability through the rank tests' door.
    let mut t = full(&o, 12);
    t["capabilityId"] = json!("stats.describe");
    let e = call(
        &w.w,
        "stats_ranks_run",
        json!({ "slot": o["slot"], "spec": t }),
    )
    .unwrap_err();
    assert!(code_of(&e).starts_with("capability."), "{e}");
    // A slot no dataset has.
    let e = call(
        &w.w,
        "stats_ranks_validate",
        json!({ "slot": 999, "spec": full(&o, 13) }),
    )
    .unwrap_err();
    assert_eq!(code_of(&e), "run.unknown_input");
}

#[test]
fn every_command_refuses_while_statistics_is_inactive() {
    let dir = TempDir::new("ranks-inactive");
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
        ("stats_ranks_validate", json!({ "slot": 1, "spec": spec })),
        ("stats_ranks_run", json!({ "slot": 1, "spec": spec })),
        ("stats_ranks_status", json!({ "runId": 1 })),
        ("stats_ranks_result", json!({ "runId": 1 })),
        ("stats_ranks_cancel", json!({ "runId": 1 })),
        ("stats_ranks_export", json!({ "runId": 1, "format": "csv" })),
        (
            "stats_ranks_group_levels",
            json!({ "slot": 1, "column": "C2" }),
        ),
    ] {
        assert_eq!(
            code_of(&call(&w, cmd, body).unwrap_err()),
            "workspace.inactive",
            "{cmd}"
        );
        checked += 1;
    }
    assert_eq!(checked, 7);
}

#[test]
fn another_kinds_run_is_refused_both_ways() {
    let w = world(None);
    let o = open(&w);
    let k = start(&w, &o, full(&o, 1)).unwrap();
    until_terminal_ranks(&w.w, k);
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
    for (prefix, theirs) in [("stats_desc", k), ("stats_anova", k), ("stats_ranks", d)] {
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
        code_of(&call(&w.w, "stats_ranks_status", json!({ "runId": 99 })).unwrap_err()),
        "run.unknown_run"
    );
    let s = call(&w.w, "workspace_state", json!({})).unwrap();
    assert_eq!(s["latestRun"], json!({ "runId": d, "kind": "describe" }));
    // A rank tests run names its kind.
    let k2 = start(&w, &o, full(&o, 3)).unwrap();
    until_terminal_ranks(&w.w, k2);
    let s = call(&w.w, "workspace_state", json!({})).unwrap();
    assert_eq!(
        s["latestRun"],
        json!({ "runId": k2, "kind": "independent_ranks" })
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
    let run = start(&w, &o, full(&o, 1)).unwrap();
    let s = until_terminal_ranks(&w.w, run);
    assert_eq!(s["state"], "succeeded", "{s}");
    assert_eq!(stats_ranks::check_points(), 1);
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
    let c = call(&w.w, "stats_ranks_cancel", json!({ "runId": run })).unwrap();
    assert_eq!(c["state"], "running", "{c}");
    assert_eq!(c["cancelRequested"], true);
    go.send(()).unwrap();
    let s = until_terminal_ranks(&w.w, run);
    assert_eq!(s["state"], "cancelled", "{s}");
    assert_eq!(s["stepsDone"], 0, "stopped at its one check point: {s}");
    assert_eq!(s["stepsTotal"], 1);
    for (cmd, body) in [
        ("stats_ranks_result", json!({ "runId": run })),
        (
            "stats_ranks_export",
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
    let s = until_terminal_ranks(&w.w, run);
    assert_eq!(s["state"], "timed_out", "{s}");
    assert_eq!(
        code_of(&call(&w.w, "stats_ranks_result", json!({ "runId": run })).unwrap_err()),
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
    assert_eq!(until_terminal_ranks(&w.w, run)["state"], "succeeded");
}

/// The levels are hashed as the spec names them: `stats_ranks::check` sorts
/// the named levels before it uses them and refuses only a repeat, so the
/// screen sends them sorted (decision 329, the ANOVA's), and this test does
/// too.
#[test]
fn one_choice_of_levels_gives_one_parameters_hash() {
    let w = world(None);
    let o = open(&w);
    let hash = |spec: Value| {
        let run = start(&w, &o, spec).unwrap();
        assert_eq!(until_terminal_ranks(&w.w, run)["state"], "succeeded");
        call(&w.w, "stats_ranks_result", json!({ "runId": run })).unwrap()["manifest"]
            ["parametersHash"]
            .clone()
    };
    let all = hash(full(&o, 1));
    let again = hash(full(&o, 2));
    assert_eq!(all, again);
    let named = hash(rspec(&o, 3, "C1", "C2", Some(&[0, 1, 2]), &[]));
    assert_ne!(
        all, named,
        "naming every level is a different spec: the screen sends null"
    );
    let pair = hash(rspec(&o, 4, "C1", "C2", Some(&[0, 2]), &[]));
    assert_eq!(pair, hash(rspec(&o, 5, "C1", "C2", Some(&[0, 2]), &[])));
    assert_ne!(all, pair);
}
