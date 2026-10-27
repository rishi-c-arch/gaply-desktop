//! **The independent-samples t-test's commands at their boundary (Research
//! OS, Phase 4B-2).** Every call goes through Tauri's IPC on the mock
//! runtime, with the webview's argument names, into the state
//! `AppState::new` builds, and every assertion reads the JSON the webview
//! reads. The run-manager tests drive `Runs` with its test constructor.
//!
//! * The flow: open → validate → run → status → result → export → close,
//!   the numbers equal to `stats_ttest::compute` on the same dataset.
//! * Refusals, each with its exact code: an unchosen variant and more than
//!   two levels with none chosen (NeedsUserInput), a one-level group, an
//!   outcome that holds text, a group of one, an unknown capability id, a
//!   wrong capability version (the registry's `capability.*` code, as it is:
//!   decision 74), and Statistics inactive.
//! * `run.wrong_kind` both ways, for status, result, cancel and export.
//! * Every transition the commands can reach, by the record's path.
//! * The run manager (decision 75): a run parked at its one check point and
//!   cancelled; a cancel and the ceiling at the same check point, 1,000
//!   times; a cancel during the arithmetic; one foreground run across both
//!   kinds; run ids and slot numbers unique across both kinds.
//! * The level positions: labels whose UTF-16 order and code-point order
//!   differ keep the profile's order end to end.
//! * Through the real parser: one table as csv, tsv and xlsx, and with a BOM
//!   and CRLF line endings, gives the same result numbers, output hash and
//!   CSV bytes.

mod desc_support;
mod ttest_support;

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex, OnceLock, Weak};
use std::time::{Duration, Instant};

use serde_json::{json, Value};

use app_lib::runs::{CheckpointHook, Clock, DatasetSlots, Point, RunKind, Runs};
use app_lib::stats_l1_ingest::load_dataset;
use desc_support::{call, delimited, refusal, spec as desc_spec, until_terminal, xlsx, TempDir, C};
use gaply_core::contracts::codes::Closed;
use gaply_core::contracts::{OperationId, RunId, RunState};
use gaply_core::stats_desc::{Alias, ColumnRequest, DescribeParams, TreatAs};
use gaply_core::stats_l1::dataset::{
    build_dataset, ColumnAlias, DataFingerprint, RawTable, TableRef,
};
use gaply_core::stats_ttest::{self, TTestParams, Variant};
use gaply_core::Database;
use ttest_support::{app, app_with, tspec, until_terminal_t};

/* ---- fixtures --------------------------------------------------------------- */

const HEADERS: [&str; 7] = ["score", "arm", "site", "note", "one", "rare", "flat"];

fn rows() -> Vec<Vec<C<'static>>> {
    let score = [
        "44.1257", "10.5", "95.5", "61.8129", "23.25", "50", "82", "31", "70.75", "12",
    ];
    let arm = [
        "ctrl", "trt", "ctrl", "trt", "ctrl", "trt", "ctrl", "trt", "ctrl", "trt",
    ];
    let site = ["n", "s", "e", "n", "s", "e", "n", "s", "e", "n"];
    let rare = ["a", "a", "a", "a", "a", "a", "a", "a", "a", "b"];
    (0..10)
        .map(|i| {
            vec![
                C::N(score[i]),
                C::T(arm[i]),
                C::T(site[i]),
                C::T(["t1", "t2", "t3", "t4", "t5", "t6", "t7", "t8", "t9", "t10"][i]),
                C::T("x"),
                C::T(rare[i]),
                C::N(if arm[i] == "ctrl" { "5" } else { "7" }),
            ]
        })
        .collect()
}

fn csv() -> Vec<u8> {
    delimited(&HEADERS, &rows(), ",", "\n")
}

struct World {
    _dir: TempDir,
    app: tauri::App<tauri::test::MockRuntime>,
    w: tauri::WebviewWindow<tauri::test::MockRuntime>,
    data: std::path::PathBuf,
}

fn world_with(runs: Option<Arc<Runs>>, name: &str, bytes: &[u8]) -> World {
    let dir = TempDir::new("ttest");
    let data = dir.write(name, bytes);
    let app_data = dir.0.join("appdata");
    std::fs::create_dir_all(&app_data).unwrap();
    let db = Arc::new(Database::in_memory().unwrap());
    let (app, w) = app(&app_data, db, runs);
    World {
        _dir: dir,
        app,
        w,
        data,
    }
}

fn world(runs: Option<Arc<Runs>>) -> World {
    world_with(runs, "data.csv", &csv())
}

fn open(w: &World) -> Value {
    call(
        &w.w,
        "stats_desc_open",
        json!({ "path": w.data.to_string_lossy() }),
    )
    .expect("open")
}

/// The analysis the flow runs: score by arm, Welch.
fn welch(o: &Value, op: u64) -> Value {
    tspec(o, op, "C1", "C2", None, Some("welch"), &[])
}

fn start(w: &World, o: &Value, spec: Value) -> Result<u64, Value> {
    call(
        &w.w,
        "stats_ttest_run",
        json!({ "slot": o["slot"], "spec": spec }),
    )
    .map(|v| v["runId"].as_u64().expect("runId"))
}

fn code_of(err: &Value) -> String {
    refusal(err)["code"].as_str().unwrap().to_string()
}

/* ---- the check-point hook --------------------------------------------------- */

struct SinceTest(Instant);
impl Clock for SinceTest {
    fn now_ms(&self) -> u64 {
        self.0.elapsed().as_millis() as u64
    }
}

/// A hook that parks the worker wherever `when` says, reports the run it
/// parked, and waits for one message on the returned sender.
fn park_when(
    when: impl Fn(RunId, Point) -> bool + Send + Sync + 'static,
) -> (CheckpointHook, Receiver<RunId>, Sender<()>) {
    let (parked_tx, parked_rx) = mpsc::channel();
    let (go_tx, go_rx) = mpsc::channel::<()>();
    let go_rx = Mutex::new(go_rx);
    let hook: CheckpointHook = Arc::new(move |run, point| {
        if when(run, point) {
            parked_tx.send(run).ok();
            go_rx
                .lock()
                .unwrap()
                .recv_timeout(Duration::from_secs(20))
                .ok();
        }
    });
    (hook, parked_rx, go_tx)
}

fn runs_with(hook: Option<CheckpointHook>, ceiling_ms: u64) -> Arc<Runs> {
    Arc::new(Runs::with_hooks(
        Arc::new(SinceTest(Instant::now())),
        hook,
        ceiling_ms,
    ))
}

fn path(runs: &Runs, run: u64) -> Vec<RunState> {
    runs.ttest_snapshot(RunId(run)).expect("held").path
}

use RunState as S;

const SUCCEEDED: [RunState; 6] = [
    S::Draft,
    S::Validating,
    S::Validated,
    S::Queued,
    S::Running,
    S::Succeeded,
];

/* ---- the flow --------------------------------------------------------------- */

#[test]
fn open_validate_run_status_result_export_close_through_ipc() {
    let w = world(None);
    let o = open(&w);
    // The dataset is describe's view, unchanged: arm's two levels in the
    // order the t-test indexes them.
    assert_eq!(
        o["profile"]["columns"][1]["levels"],
        json!([{"label": "ctrl", "count": 5}, {"label": "trt", "count": 5}])
    );

    let v = call(
        &w.w,
        "stats_ttest_validate",
        json!({ "slot": o["slot"], "spec": welch(&o, 1) }),
    )
    .unwrap();
    assert_eq!(v["status"], "valid", "{v}");
    assert_eq!(v["errors"], json!([]));
    assert_eq!(v["missingInformation"], json!([]));
    assert_eq!(
        v["estimatedResources"],
        json!({"rows": 10, "columns": 2, "cells": 20})
    );

    let run = start(&w, &o, welch(&o, 2)).unwrap();
    let s = until_terminal_t(&w.w, run);
    assert_eq!(s["state"], "succeeded", "{s}");
    assert_eq!(s["terminal"], true);
    assert_eq!(s["cancelRequested"], false);
    assert!(
        s.get("columnsDone").is_none() && s.get("columnsTotal").is_none(),
        "a t-test's state view has no column progress: {s}"
    );

    let r = call(&w.w, "stats_ttest_result", json!({ "runId": run })).unwrap();
    assert_eq!(r["runId"], run);
    assert_eq!(r["status"], "succeeded");
    // The numbers are stats_ttest's on the same dataset, bit for bit.
    let dataset = load_dataset(&w.data).expect("the same file");
    let params = TTestParams {
        outcome: Alias(ColumnAlias(1)),
        group: Alias(ColumnAlias(2)),
        levels: None,
        variant: Some(Variant::Welch),
        missing_codes: vec![],
    };
    let direct = stats_ttest::compute(&dataset, &params).expect("computes");
    // The view went to JSON text and back. serde_json was built without
    // `float_roundtrip` until Phase 8D-2b (decision 267), and its parse of that
    // text could land one ulp off (measured: p 0.10922441798986729 read back
    // as …728). The view is still compared with the direct result through the
    // same text; the bit-for-bit claim rests on the output hash and the CSV
    // bytes below, which never pass through a float parse. A webview's
    // JSON.parse rounds correctly.
    let through_text: Value =
        serde_json::from_str(&serde_json::to_string(&direct).unwrap()).unwrap();
    assert_eq!(r["result"], through_text);
    assert_eq!(r["result"]["groups"][0]["label"], "ctrl");
    assert_eq!(r["result"]["groups"][1]["label"], "trt");
    assert_eq!(r["result"]["variant"], "welch");
    assert_eq!(
        r["result"]["limitations"],
        json!([
            "stats.ttest.normality_not_assessed",
            "stats.ttest.cohens_d_has_no_interval"
        ])
    );
    let m = &r["manifest"];
    assert_eq!(m["capabilityId"], "stats.analyze.independent_t");
    assert_eq!(m["capabilityVersion"], 1);
    assert_eq!(m["rowsRead"], 10);
    assert_eq!(m["rowsUsed"], 10);
    assert_eq!(m["inputs"], json!([o["inputRef"]["contentHash"]]));
    assert_eq!(
        m["outputHash"],
        gaply_core::contracts::ContentHash::of_bytes(&direct.canonical_bytes())
            .as_hex()
            .to_string()
    );

    let csv = call(
        &w.w,
        "stats_ttest_export",
        json!({ "runId": run, "format": "csv" }),
    )
    .unwrap();
    let bytes: Vec<u8> = serde_json::from_value(csv["bytes"].clone()).unwrap();
    assert_eq!(bytes, stats_ttest::export::result_csv(&direct));
    assert_eq!(
        csv["sha256"],
        gaply_core::contracts::ContentHash::of_bytes(&bytes)
            .as_hex()
            .to_string()
    );
    let mj = call(
        &w.w,
        "stats_ttest_export",
        json!({ "runId": run, "format": "manifestJson" }),
    )
    .unwrap();
    let mj_bytes: Vec<u8> = serde_json::from_value(mj["bytes"].clone()).unwrap();
    let parsed: Value = serde_json::from_slice(&mj_bytes).expect("JSON");
    assert_eq!(parsed["outputHash"], m["outputHash"]);

    assert_eq!(
        call(&w.w, "stats_desc_close", json!({ "slot": o["slot"] })).unwrap(),
        json!(true)
    );
    assert_eq!(
        code_of(
            &call(
                &w.w,
                "stats_ttest_validate",
                json!({ "slot": o["slot"], "spec": welch(&o, 3) })
            )
            .unwrap_err()
        ),
        "run.unknown_input"
    );
}

/* ---- refusals --------------------------------------------------------------- */

/// The one finding a validation reports, with its status.
fn validated(w: &World, o: &Value, spec: Value) -> Value {
    call(
        &w.w,
        "stats_ttest_validate",
        json!({ "slot": o["slot"], "spec": spec }),
    )
    .expect("a validation view")
}

#[test]
fn each_refusal_has_its_exact_code() {
    let w = world(None);
    let o = open(&w);

    // NeedsUserInput: the variant is never chosen for the user.
    let v = validated(&w, &o, tspec(&o, 1, "C1", "C2", None, None, &[]));
    assert_eq!(v["status"], "needs_user_input", "{v}");
    assert_eq!(
        v["missingInformation"][0]["code"],
        "stats.ttest.variant_not_chosen"
    );
    // NeedsUserInput: three levels, none chosen.
    let v = validated(&w, &o, tspec(&o, 2, "C1", "C3", None, Some("student"), &[]));
    assert_eq!(v["status"], "needs_user_input", "{v}");
    assert_eq!(
        v["missingInformation"][0]["code"],
        "stats.ttest.levels_not_chosen"
    );
    assert_eq!(v["missingInformation"][0]["params"]["count"], 3);
    // Invalid: a one-level group.
    let v = validated(&w, &o, tspec(&o, 3, "C1", "C5", None, Some("welch"), &[]));
    assert_eq!(v["status"], "invalid", "{v}");
    assert_eq!(v["errors"][0]["code"], "stats.ttest.group_one_level");
    // Invalid: an outcome that holds text.
    let v = validated(&w, &o, tspec(&o, 4, "C4", "C2", None, Some("welch"), &[]));
    assert_eq!(v["status"], "invalid", "{v}");
    assert_eq!(v["errors"][0]["code"], "stats.ttest.outcome_holds_text");
    assert_eq!(v["errors"][0]["params"]["count"], 10);
    // Invalid: a group of one ("b" has one row).
    let v = validated(&w, &o, tspec(&o, 5, "C1", "C6", None, Some("student"), &[]));
    assert_eq!(v["status"], "invalid", "{v}");
    assert_eq!(v["errors"][0]["code"], "stats.ttest.group_too_small");
    assert_eq!(v["errors"][0]["params"]["index"], 1);
    assert_eq!(v["errors"][0]["params"]["count"], 1);

    // The registry's refusals, as the registry words them (decision 74),
    // from validate and from run alike, and no record is made.
    let mut unknown = welch(&o, 6);
    unknown["capabilityId"] = json!("stats.analyze.nonexistent");
    let mut stale = welch(&o, 7);
    stale["capabilityVersion"] = json!(2);
    for cmd in ["stats_ttest_validate", "stats_ttest_run"] {
        let e = refusal(
            &call(
                &w.w,
                cmd,
                json!({ "slot": o["slot"], "spec": unknown.clone() }),
            )
            .unwrap_err(),
        );
        assert_eq!(e["code"], "capability.unknown_capability", "{cmd}: {e}");
        let e = refusal(
            &call(
                &w.w,
                cmd,
                json!({ "slot": o["slot"], "spec": stale.clone() }),
            )
            .unwrap_err(),
        );
        assert_eq!(e["code"], "capability.version_mismatch", "{cmd}: {e}");
        assert_eq!(e["params"], json!({ "expected": 1, "found": 2 }), "{cmd}");
    }
    // Another capability's spec is refused too: the describe spec names
    // stats.describe, which is not the t-test.
    let other = desc_spec(&o, 8, &[("C1", "numeric")], &[]);
    // Its parameters are not the t-test's, so it does not even read as a
    // t-test spec (`deny_unknown_fields`), and nothing runs.
    let e = call(
        &w.w,
        "stats_ttest_validate",
        json!({ "slot": o["slot"], "spec": other }),
    )
    .unwrap_err();
    assert!(e.to_string().contains("columns"), "{e}");
    // A run that needs input makes a record that ends NeedsUserInput.
    let run = start(&w, &o, tspec(&o, 9, "C1", "C2", None, None, &[])).unwrap();
    let s = until_terminal_t(&w.w, run);
    assert_eq!(s["state"], "needs_user_input");
    assert_eq!(
        s["diagnostics"][0]["code"],
        "stats.ttest.variant_not_chosen"
    );
}

#[test]
fn every_ttest_command_is_refused_while_statistics_is_not_active() {
    let dir = TempDir::new("ttest-inactive");
    let app_data = dir.0.join("appdata");
    std::fs::create_dir_all(&app_data).unwrap();
    let db = Arc::new(Database::in_memory().unwrap());
    let (_app, w) = app_with(
        &app_data,
        db,
        Arc::new(|| Arc::new(Runs::default())),
        Arc::new(app_lib::workspace::readiness),
    );
    let fake =
        json!({ "contentHash": "0".repeat(64), "rows": 1, "columns": 1, "semanticRole": "table" });
    let spec = tspec(
        &json!({ "inputRef": fake }),
        1,
        "C1",
        "C2",
        None,
        Some("welch"),
        &[],
    );
    for (cmd, body) in [
        ("stats_ttest_validate", json!({ "slot": 1, "spec": spec })),
        ("stats_ttest_run", json!({ "slot": 1, "spec": spec })),
        ("stats_ttest_status", json!({ "runId": 1 })),
        ("stats_ttest_result", json!({ "runId": 1 })),
        ("stats_ttest_cancel", json!({ "runId": 1 })),
        ("stats_ttest_export", json!({ "runId": 1, "format": "csv" })),
    ] {
        let e = refusal(&call(&w, cmd, body).unwrap_err());
        assert_eq!(e["code"], "workspace.inactive", "{cmd}: {e}");
    }
    // The selector's list needs no workspace (decision 76).
    let list = call(&w, "capability_list", json!({})).expect("the list");
    let ids: Vec<&str> = list
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["id"].as_str().unwrap())
        .collect();
    assert!(ids.contains(&"stats.analyze.independent_t"), "{ids:?}");
    let t = list
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["id"] == "stats.analyze.independent_t")
        .unwrap();
    assert_eq!(t["runnable"], true);
    assert_eq!(t["lifecycle"], "beta");
    assert_eq!(
        t["knownLimitations"],
        json!([
            "stats.capability.limit.table.text_files_utf8_only",
            "stats.capability.limit.table.cell_text_cap",
            "stats.capability.limit.analyze.levels_follow_level_1",
            "stats.capability.limit.analyze.normality_not_assessed",
            "stats.capability.limit.analyze.cohens_d_has_no_interval"
        ]),
        "the t-test's limitations are the registry's"
    );
}

/// The registry, not the screen, says what is runnable (decision 76): every
/// entry's `runnable` is `require_runnable` with this build.
#[test]
fn capability_list_is_the_registry_with_rusts_runnable() {
    let registry = gaply_core::stats_registry::Registry::statistics();
    let list = app_lib::stats_ttest_commands::capabilities();
    assert_eq!(list.len(), registry.entries().len());
    let build = app_lib::workspace::build();
    for (view, entry) in list.iter().zip(registry.entries()) {
        let d = &entry.descriptor;
        assert_eq!(view.id, d.id);
        assert_eq!(view.version, d.version);
        assert_eq!(view.lifecycle, d.lifecycle);
        assert_eq!(view.title_code, d.title_code);
        assert_eq!(view.known_limitations, d.known_limitation_codes);
        assert_eq!(
            view.runnable,
            gaply_core::stats_registry::require_runnable(&d.id, d.version, build).is_ok(),
            "{}",
            d.id.as_str()
        );
        assert_eq!(view.runnable, d.lifecycle.runnable_in(build));
    }
}

/* ---- run.wrong_kind ----------------------------------------------------------- */

#[test]
fn each_kinds_commands_refuse_a_run_of_the_other_kind() {
    let w = world(None);
    let o = open(&w);
    let t = start(&w, &o, welch(&o, 1)).unwrap();
    assert_eq!(until_terminal_t(&w.w, t)["state"], "succeeded");
    let d = call(
        &w.w,
        "stats_desc_run",
        json!({ "slot": o["slot"], "spec": desc_spec(&o, 2, &[("C1", "numeric")], &[]) }),
    )
    .unwrap()["runId"]
        .as_u64()
        .unwrap();
    assert_eq!(until_terminal(&w.w, d)["state"], "succeeded");

    for (prefix, other) in [("stats_desc", t), ("stats_ttest", d)] {
        for (cmd, body) in [
            ("status", json!({ "runId": other })),
            ("result", json!({ "runId": other })),
            ("cancel", json!({ "runId": other })),
            ("export", json!({ "runId": other, "format": "csv" })),
        ] {
            let full = format!("{prefix}_{cmd}");
            assert_eq!(
                code_of(&call(&w.w, &full, body).unwrap_err()),
                "run.wrong_kind",
                "{full} given run {other}"
            );
        }
    }
    // Each kind still reads its own.
    assert_eq!(
        call(&w.w, "stats_ttest_status", json!({ "runId": t })).unwrap()["state"],
        "succeeded"
    );
    assert_eq!(
        call(&w.w, "stats_desc_status", json!({ "runId": d })).unwrap()["state"],
        "succeeded"
    );
    // And an id no record has is unknown, not of the wrong kind.
    assert_eq!(
        code_of(&call(&w.w, "stats_ttest_status", json!({ "runId": 99 })).unwrap_err()),
        "run.unknown_run"
    );
}

/* ---- transitions -------------------------------------------------------------- */

/// Every edge of the state table the t-test's commands can reach, each by the
/// record's full path. Not reachable, by design: anything through
/// `AwaitingConfirmation` (the t-test takes no confirmation), `ResourceLimit`
/// and `NumericalFailure` (a not-computable result ends `Failed`, with its
/// code, as the descriptive summary's refusals do), and `Queued –refuse→
/// Refused`, which only a failure to spawn the worker thread produces.
#[test]
fn every_transition_the_commands_can_reach() {
    // Succeeded, Refused, NeedsUserInput, Failed.
    let runs = runs_with(None, 30_000);
    let w = world(Some(runs.clone()));
    let o = open(&w);
    let ok = start(&w, &o, welch(&o, 1)).unwrap();
    assert_eq!(until_terminal_t(&w.w, ok)["state"], "succeeded");
    assert_eq!(path(&runs, ok), SUCCEEDED);

    let refused = start(&w, &o, tspec(&o, 2, "C4", "C2", None, Some("welch"), &[])).unwrap();
    assert_eq!(until_terminal_t(&w.w, refused)["state"], "refused");
    assert_eq!(path(&runs, refused), [S::Draft, S::Validating, S::Refused]);

    let asks = start(&w, &o, tspec(&o, 3, "C1", "C3", None, Some("welch"), &[])).unwrap();
    assert_eq!(until_terminal_t(&w.w, asks)["state"], "needs_user_input");
    assert_eq!(
        path(&runs, asks),
        [S::Draft, S::Validating, S::NeedsUserInput]
    );

    // Both groups constant: validation passes, the run is not computable.
    let flat = start(&w, &o, tspec(&o, 4, "C7", "C2", None, Some("student"), &[])).unwrap();
    let s = until_terminal_t(&w.w, flat);
    assert_eq!(s["state"], "failed", "{s}");
    assert_eq!(
        s["diagnostics"].as_array().unwrap().last().unwrap()["code"],
        "stats.ttest.both_groups_constant"
    );
    assert_eq!(
        path(&runs, flat),
        [
            S::Draft,
            S::Validating,
            S::Validated,
            S::Queued,
            S::Running,
            S::Failed
        ]
    );
    assert_eq!(
        code_of(&call(&w.w, "stats_ttest_result", json!({ "runId": flat })).unwrap_err()),
        "run.no_result"
    );

    // Cancelled while queued: parked before the worker takes it.
    let (hook, parked, go) = park_when(|_, p| p == Point::BeforeStart);
    let runs = runs_with(Some(hook), 30_000);
    let w = world(Some(runs.clone()));
    let o = open(&w);
    let queued = start(&w, &o, welch(&o, 1)).unwrap();
    parked
        .recv_timeout(Duration::from_secs(10))
        .expect("parked");
    let c = call(&w.w, "stats_ttest_cancel", json!({ "runId": queued })).unwrap();
    assert_eq!(c["state"], "cancelled", "{c}");
    go.send(()).unwrap();
    assert_eq!(
        path(&runs, queued),
        [
            S::Draft,
            S::Validating,
            S::Validated,
            S::Queued,
            S::Cancelled
        ]
    );

    // Timed out: a ceiling of 0.
    let runs = runs_with(None, 0);
    let w = world(Some(runs.clone()));
    let o = open(&w);
    let late = start(&w, &o, welch(&o, 1)).unwrap();
    assert_eq!(until_terminal_t(&w.w, late)["state"], "timed_out");
    assert_eq!(
        path(&runs, late),
        [
            S::Draft,
            S::Validating,
            S::Validated,
            S::Queued,
            S::Running,
            S::TimedOut
        ]
    );
    assert_eq!(
        code_of(&call(&w.w, "stats_ttest_result", json!({ "runId": late })).unwrap_err()),
        "run.timed_out"
    );
    // A finished record does not change: a cancel of it returns its state.
    let c = call(&w.w, "stats_ttest_cancel", json!({ "runId": late })).unwrap();
    assert_eq!(c["state"], "timed_out");
    assert_eq!(c["cancelRequested"], false);
}

/* ---- the run manager ---------------------------------------------------------- */

#[test]
fn a_run_parked_at_its_check_point_and_cancelled_ends_cancelled() {
    let (hook, parked, go) = park_when(|_, p| p == Point::Column(0));
    let runs = runs_with(Some(hook), 30_000);
    let w = world(Some(runs.clone()));
    let o = open(&w);
    let run = start(&w, &o, welch(&o, 1)).unwrap();
    parked
        .recv_timeout(Duration::from_secs(10))
        .expect("parked at the check point");
    let c = call(&w.w, "stats_ttest_cancel", json!({ "runId": run })).unwrap();
    assert_eq!(c["state"], "running");
    assert_eq!(c["cancelRequested"], true);
    go.send(()).unwrap();
    let s = until_terminal_t(&w.w, run);
    assert_eq!(s["state"], "cancelled", "{s}");
    assert_eq!(
        path(&runs, run),
        [
            S::Draft,
            S::Validating,
            S::Validated,
            S::Queued,
            S::Running,
            S::Cancelled
        ]
    );
    for (cmd, body) in [
        ("stats_ttest_result", json!({ "runId": run })),
        (
            "stats_ttest_export",
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

/// F1 for the t-test: a cancel that lands after the one check point has read
/// the flag, while the arithmetic runs, finds no check point left. The work
/// completes and is discarded: the run ends `Cancelled`, with no result.
#[test]
fn a_cancel_during_the_arithmetic_ends_cancelled_with_no_result() {
    let (hook, parked, go) = park_when(|_, p| p == Point::Passed(0));
    let runs = runs_with(Some(hook), 30_000);
    let w = world(Some(runs.clone()));
    let o = open(&w);
    let run = start(&w, &o, welch(&o, 1)).unwrap();
    parked
        .recv_timeout(Duration::from_secs(10))
        .expect("parked after the check point, before the arithmetic");
    let c = call(&w.w, "stats_ttest_cancel", json!({ "runId": run })).unwrap();
    assert_eq!(c["state"], "running", "accepted while running: {c}");
    assert_eq!(c["cancelRequested"], true);
    go.send(()).unwrap();
    let s = until_terminal_t(&w.w, run);
    assert_eq!(s["state"], "cancelled", "{s}");
    assert_eq!(path(&runs, run).last(), Some(&S::Cancelled));
    for (cmd, body) in [
        ("stats_ttest_result", json!({ "runId": run })),
        (
            "stats_ttest_export",
            json!({ "runId": run, "format": "manifestJson" }),
        ),
    ] {
        assert_eq!(
            code_of(&call(&w.w, cmd, body).unwrap_err()),
            "run.cancelled",
            "{cmd}"
        );
    }
}

/// A clock that stands still until a hook arms it. Armed, its next reading
/// on the armed thread (the check point's reading of the ceiling, which
/// comes AFTER its reading of the cancel flag) accepts a cancel of the run
/// through `Runs::ttest_cancel` and moves time on by the whole ceiling, so
/// the cancel and the ceiling land at the same check point by construction.
struct CancelAtTheCeiling {
    now: AtomicU64,
    ceiling: u64,
    armed: Mutex<Option<(std::thread::ThreadId, RunId)>>,
    runs: OnceLock<Weak<Runs>>,
    accepted: Mutex<Vec<RunState>>,
}

impl Clock for CancelAtTheCeiling {
    fn now_ms(&self) -> u64 {
        let fire = {
            let mut armed = self.armed.lock().unwrap();
            match *armed {
                Some((thread, run)) if thread == std::thread::current().id() => {
                    *armed = None;
                    Some(run)
                }
                _ => None,
            }
        };
        if let Some(run) = fire {
            self.now.fetch_add(self.ceiling, Ordering::SeqCst);
            let runs = self.runs.get().and_then(Weak::upgrade).expect("runs");
            let snap = runs.ttest_cancel(run).expect("the run is held");
            assert!(snap.cancel_requested);
            self.accepted.lock().unwrap().push(snap.record.state);
        }
        self.now.load(Ordering::SeqCst)
    }
}

fn synthetic_slot(slots: &DatasetSlots) -> Arc<app_lib::runs::Slot> {
    let dataset = build_dataset(
        RawTable::from_rows(
            vec!["y".into(), "g".into()],
            (1..=8)
                .map(|i| {
                    vec![
                        (i * 3 % 7).to_string(),
                        if i % 2 == 0 { "a" } else { "b" }.to_string(),
                    ]
                })
                .collect(),
        ),
        DataFingerprint::of_bytes(b"synthetic", "unit", TableRef::FirstTable),
    )
    .expect("dataset");
    slots
        .get(slots.insert("synthetic".into(), dataset))
        .expect("slot")
}

fn synthetic_params() -> TTestParams {
    TTestParams {
        outcome: Alias(ColumnAlias(1)),
        group: Alias(ColumnAlias(2)),
        levels: None,
        variant: Some(Variant::Student),
        missing_codes: vec![],
    }
}

/// F2 for the t-test (decision 75: the one shared check covers both kinds).
/// A cancel accepted while running ends the run `Cancelled` even when the
/// ceiling passes at the same check point. 1,000 runs, each deterministic.
#[test]
fn a_cancel_and_the_ceiling_at_the_same_check_point_end_cancelled_every_time() {
    const CEILING: u64 = 1_000;
    const RUNS: usize = 1_000;
    let clock = Arc::new(CancelAtTheCeiling {
        now: AtomicU64::new(0),
        ceiling: CEILING,
        armed: Mutex::new(None),
        runs: OnceLock::new(),
        accepted: Mutex::new(Vec::new()),
    });
    let arm = clock.clone();
    let hook: CheckpointHook = Arc::new(move |run, p| {
        if p == Point::Column(0) {
            *arm.armed.lock().unwrap() = Some((std::thread::current().id(), run));
        }
    });
    let runs = Arc::new(Runs::with_hooks(clock.clone(), Some(hook), CEILING));
    clock
        .runs
        .set(Arc::downgrade(&runs))
        .ok()
        .expect("set once");
    let slots = DatasetSlots::default();
    let slot = synthetic_slot(&slots);

    let mut ended = std::collections::BTreeMap::<String, usize>::new();
    for i in 0..RUNS as u64 {
        let spec = stats_ttest::spec(OperationId(i + 1), slot.dataset(), synthetic_params());
        let run = runs.start_ttest(slot.clone(), spec).expect("started");
        let deadline = Instant::now() + Duration::from_secs(10);
        let s = loop {
            let s = runs.ttest_snapshot(run).expect("held");
            if s.record.state.is_terminal() {
                break s;
            }
            assert!(Instant::now() < deadline, "run {i} did not end");
            std::thread::yield_now();
        };
        *ended
            .entry(format!(
                "{} cancel_requested={}",
                s.record.state.wire(),
                s.cancel_requested
            ))
            .or_default() += 1;
        assert!(runs.ttest_result(run).is_err(), "no result");
    }
    let accepted = clock.accepted.lock().unwrap().clone();
    // Not vacuous: every run's cancel was accepted while it ran, after the
    // check point had read the flag, so the ceiling stopped it there.
    assert_eq!(accepted.len(), RUNS, "a cancel was accepted in every run");
    assert!(
        accepted.iter().all(|s| *s == RunState::Running),
        "{accepted:?}"
    );
    assert_eq!(
        ended.into_iter().collect::<Vec<_>>(),
        [("cancelled cancel_requested=true".to_string(), RUNS)],
        "every run must end Cancelled"
    );
}

#[test]
fn one_foreground_run_across_both_kinds() {
    // A t-test started while a summary is parked is refused, and nothing is
    // recorded.
    let (hook, parked, go) = park_when(|run, p| run == RunId(1) && p == Point::Column(0));
    let runs = runs_with(Some(hook), 30_000);
    let w = world(Some(runs.clone()));
    let o = open(&w);
    let d = call(
        &w.w,
        "stats_desc_run",
        json!({ "slot": o["slot"], "spec": desc_spec(&o, 1, &[("C1", "numeric")], &[]) }),
    )
    .unwrap()["runId"]
        .as_u64()
        .unwrap();
    parked
        .recv_timeout(Duration::from_secs(10))
        .expect("parked");
    assert_eq!(
        code_of(&start(&w, &o, welch(&o, 2)).unwrap_err()),
        "run.run_busy"
    );
    assert_eq!(runs.held(), 1, "a refused start makes no record");
    go.send(()).unwrap();
    assert_eq!(until_terminal(&w.w, d)["state"], "succeeded");

    // The reverse: a summary started while a t-test is parked.
    let (hook, parked, go) = park_when(|run, p| run == RunId(1) && p == Point::Column(0));
    let runs = runs_with(Some(hook), 30_000);
    let w = world(Some(runs.clone()));
    let o = open(&w);
    let t = start(&w, &o, welch(&o, 1)).unwrap();
    parked
        .recv_timeout(Duration::from_secs(10))
        .expect("parked");
    let e = call(
        &w.w,
        "stats_desc_run",
        json!({ "slot": o["slot"], "spec": desc_spec(&o, 2, &[("C1", "numeric")], &[]) }),
    )
    .unwrap_err();
    assert_eq!(code_of(&e), "run.run_busy");
    assert_eq!(runs.held(), 1);
    // The shell names the parked run and its kind (decision 80).
    let s = call(&w.w, "workspace_state", json!({})).unwrap();
    assert_eq!(s["activeRun"], json!({ "runId": t, "kind": "ttest" }));
    go.send(()).unwrap();
    assert_eq!(until_terminal_t(&w.w, t)["state"], "succeeded");
    let s = call(&w.w, "workspace_state", json!({})).unwrap();
    assert_eq!(s["activeRun"], Value::Null);
    assert_eq!(s["latestRun"], json!({ "runId": t, "kind": "ttest" }));
}

/// F8 across both kinds: one run counter and one slot counter, so no id is
/// handed out twice, whichever kind asked, and none is reused after the
/// workspace is left and opened again.
#[test]
fn run_ids_and_slot_numbers_stay_unique_across_both_kinds() {
    let w = world(None);
    let mut slots = Vec::new();
    let mut runs = Vec::new();
    for round in 0..2 {
        for i in 0..3u64 {
            let o = open(&w);
            slots.push(o["slot"].as_u64().unwrap());
            let t = start(&w, &o, welch(&o, 10 * round + i)).unwrap();
            assert_eq!(until_terminal_t(&w.w, t)["state"], "succeeded");
            let d = call(
                &w.w,
                "stats_desc_run",
                json!({ "slot": o["slot"], "spec": desc_spec(&o, 100 + i, &[("C1", "numeric")], &[]) }),
            )
            .unwrap()["runId"]
                .as_u64()
                .unwrap();
            assert_eq!(until_terminal(&w.w, d)["state"], "succeeded");
            runs.extend([(t, RunKind::Ttest), (d, RunKind::Describe)]);
        }
        call(&w.w, "workspace_deactivate", json!({})).expect("leaves");
        call(&w.w, "workspace_activate", json!({ "id": "statistics" })).expect("opens again");
    }
    let mut ids: Vec<u64> = runs.iter().map(|(r, _)| *r).collect();
    let n = ids.len();
    ids.sort_unstable();
    ids.dedup();
    assert_eq!(ids.len(), n, "a run id was handed out twice: {runs:?}");
    assert!(
        runs.windows(2).all(|p| p[0].0 < p[1].0),
        "run ids increase across kinds: {runs:?}"
    );
    let mut s = slots.clone();
    s.sort_unstable();
    s.dedup();
    assert_eq!(s.len(), slots.len(), "a slot number was reused: {slots:?}");
    // After reopening, an old run of either kind reads as unknown.
    for (run, kind) in &runs[..2] {
        let cmd = match kind {
            RunKind::Ttest => "stats_ttest_status",
            RunKind::Describe => "stats_desc_status",
            RunKind::OneSample => "stats_onesample_status",
            RunKind::Paired => "stats_paired_status",
        };
        assert_eq!(
            code_of(&call(&w.w, cmd, json!({ "runId": run })).unwrap_err()),
            "run.unknown_run"
        );
    }
}

/* ---- level positions ------------------------------------------------------------ */

/// Decision 78. "ｚ" (U+FF5A) comes before "😀" (U+1F600) in code points,
/// and after it in UTF-16 code units, which is how JavaScript sorts strings.
/// The profile lists them in code-point order, the t-test indexes them in
/// the same order, and the result's group 1 is the profile's position 0.
#[test]
fn level_positions_follow_the_profile_where_utf16_order_differs() {
    assert!("ｚ" < "😀", "code-point order");
    let utf16 = |s: &str| s.encode_utf16().collect::<Vec<u16>>();
    assert!(utf16("😀") < utf16("ｚ"), "UTF-16 order is the reverse");
    let rows: Vec<Vec<C>> = (0..8)
        .map(|i| {
            vec![
                C::N(["1", "2", "3", "4", "10", "11", "13", "15"][i]),
                C::T(if i < 4 { "😀" } else { "ｚ" }),
            ]
        })
        .collect();
    let w = world_with(
        None,
        "levels.csv",
        &delimited(&["y", "g"], &rows, ",", "\n"),
    );
    let o = open(&w);
    let levels = &o["profile"]["columns"][1]["levels"];
    assert_eq!(levels[0]["label"], "ｚ");
    assert_eq!(levels[1]["label"], "😀");
    for (levels, first) in [([0u32, 1u32], "ｚ"), ([1, 0], "😀")] {
        let run = start(
            &w,
            &o,
            tspec(
                &o,
                u64::from(levels[0]) + 1,
                "C1",
                "C2",
                Some(levels),
                Some("student"),
                &[],
            ),
        )
        .unwrap();
        assert_eq!(until_terminal_t(&w.w, run)["state"], "succeeded");
        let r = call(&w.w, "stats_ttest_result", json!({ "runId": run })).unwrap();
        assert_eq!(r["result"]["groups"][0]["label"], first, "{levels:?}");
        assert_eq!(r["result"]["groups"][0]["level"], levels[0]);
    }
}

/* ---- through the real parser ------------------------------------------------------ */

/// Opens `bytes` as `name`, runs the t-test, and returns the result, its
/// output hash and the CSV export.
fn ttest_file(name: &str, bytes: &[u8]) -> (Value, Value, Vec<u8>) {
    let w = world_with(None, name, bytes);
    let o = open(&w);
    let run = start(&w, &o, welch(&o, 1)).unwrap();
    assert_eq!(until_terminal_t(&w.w, run)["state"], "succeeded", "{name}");
    let r = call(&w.w, "stats_ttest_result", json!({ "runId": run })).unwrap();
    let csv = call(
        &w.w,
        "stats_ttest_export",
        json!({ "runId": run, "format": "csv" }),
    )
    .unwrap();
    let bytes: Vec<u8> = serde_json::from_value(csv["bytes"].clone()).unwrap();
    (
        r["result"].clone(),
        r["manifest"]["outputHash"].clone(),
        bytes,
    )
}

#[test]
fn the_same_table_through_every_format_gives_the_same_result_hash_and_csv() {
    let csv_bytes = csv();
    let mut bom_crlf = vec![0xEF, 0xBB, 0xBF];
    bom_crlf.extend(delimited(&HEADERS, &rows(), ",", "\r\n"));
    let cases = [
        ("data.csv", csv_bytes.clone()),
        ("data.tsv", delimited(&HEADERS, &rows(), "\t", "\n")),
        ("data.xlsx", xlsx(&HEADERS, &rows())),
        ("bom_crlf.csv", bom_crlf),
    ];
    let (want_result, want_hash, want_csv) = ttest_file(cases[0].0, &cases[0].1);
    assert!(want_result["t"].is_number());
    for (name, bytes) in &cases[1..] {
        let (result, hash, csv) = ttest_file(name, bytes);
        assert_eq!(result, want_result, "{name}: the result numbers");
        assert_eq!(hash, want_hash, "{name}: the output hash");
        assert_eq!(csv, want_csv, "{name}: the CSV bytes");
    }
}

/* ---- the describe run manager is unchanged ---------------------------------------- */

/// Describe's `start` keeps its signature and its record (decision 75).
#[test]
fn a_summary_started_through_runs_start_is_still_a_summary() {
    let runs = Arc::new(Runs::default());
    let slots = DatasetSlots::default();
    let slot = synthetic_slot(&slots);
    let spec = gaply_core::stats_desc::spec(
        OperationId(1),
        slot.dataset(),
        DescribeParams {
            columns: vec![ColumnRequest {
                alias: Alias(ColumnAlias(1)),
                treat_as: TreatAs::Numeric,
            }],
            missing_codes: vec![],
        },
    );
    let run = runs.start(slot, spec).expect("started");
    let deadline = Instant::now() + Duration::from_secs(10);
    while !runs.snapshot(run).unwrap().record.state.is_terminal() {
        assert!(Instant::now() < deadline);
        std::thread::yield_now();
    }
    assert_eq!(
        runs.snapshot(run).unwrap().record.state,
        RunState::Succeeded
    );
    assert_eq!(
        runs.ttest_snapshot(run).err(),
        Some(gaply_core::contracts::codes::RunCode::WrongKind)
    );
    assert!(runs.result(run).is_ok());
}

/* ---- a group coded in numbers (decision 81) ---------------------------------- */

/// A table whose group is coded in numbers: `0`/`1` in C2, `1`/`2`/`3` in C3.
/// Both columns read as numeric, so `DatasetView`'s profile lists no levels
/// for them; `stats_ttest_group_levels` does.
fn coded_table() -> Vec<u8> {
    let y = [
        "4.5", "6", "5.25", "7", "3", "8.5", "4", "9", "5", "6.75", "2", "7.5",
    ];
    let rows: Vec<Vec<C>> = (0..12)
        .map(|i| {
            vec![
                C::N(y[i]),
                C::N(if i % 2 == 0 { "0" } else { "1" }),
                C::N(["1", "2", "3"][i % 3]),
            ]
        })
        .collect();
    delimited(&["y", "treated", "dose"], &rows, ",", "\n")
}

fn group_levels(w: &World, o: &Value, column: &str) -> Result<Value, Value> {
    call(
        &w.w,
        "stats_ttest_group_levels",
        json!({ "slot": o["slot"], "column": column }),
    )
}

#[test]
fn a_group_coded_0_1_is_offered_and_runs_with_its_levels_in_the_cores_order() {
    let w = world_with(None, "coded.csv", &coded_table());
    let o = open(&w);
    // The profile reads the column as numeric and lists no levels.
    assert_eq!(o["profile"]["columns"][1]["kind"], "numeric");
    assert_eq!(o["profile"]["columns"][1]["levels"], Value::Null);
    assert_eq!(o["profile"]["columns"][1]["distinct"], 2);

    let g = group_levels(&w, &o, "C2").expect("the group's levels");
    assert_eq!(g["slot"], o["slot"]);
    assert_eq!(g["column"], "C2");
    assert_eq!(
        g["levels"],
        json!([
            { "index": 0, "label": "0", "count": 6 },
            { "index": 1, "label": "1", "count": 6 }
        ])
    );
    // Run with each order: group 1 is the level at the position sent, so the
    // command's list is the core's indexing.
    for (levels, first) in [([0u32, 1u32], "0"), ([1, 0], "1")] {
        let run = start(
            &w,
            &o,
            tspec(
                &o,
                u64::from(levels[0]) + 1,
                "C1",
                "C2",
                Some(levels),
                Some("welch"),
                &[],
            ),
        )
        .unwrap();
        assert_eq!(until_terminal_t(&w.w, run)["state"], "succeeded");
        let r = call(&w.w, "stats_ttest_result", json!({ "runId": run })).unwrap();
        let groups = r["result"]["groups"].as_array().unwrap();
        assert_eq!(groups[0]["label"], first, "{levels:?}");
        for gs in groups {
            let i = gs["level"].as_u64().unwrap() as usize;
            assert_eq!(gs["label"], g["levels"][i]["label"], "index {i}");
            assert_eq!(gs["n"], g["levels"][i]["count"], "index {i}");
        }
    }
}

#[test]
fn a_group_coded_1_2_and_3_needs_two_chosen() {
    let w = world_with(None, "coded.csv", &coded_table());
    let o = open(&w);
    let g = group_levels(&w, &o, "C3").expect("the group's levels");
    let labels: Vec<&str> = g["levels"]
        .as_array()
        .unwrap()
        .iter()
        .map(|l| l["label"].as_str().unwrap())
        .collect();
    assert_eq!(labels, ["1", "2", "3"]);
    let v = call(
        &w.w,
        "stats_ttest_validate",
        json!({ "slot": o["slot"], "spec": tspec(&o, 1, "C1", "C3", None, Some("student"), &[]) }),
    )
    .unwrap();
    assert_eq!(v["status"], "needs_user_input", "{v}");
    assert_eq!(
        v["missingInformation"][0]["code"],
        "stats.ttest.levels_not_chosen"
    );
    assert_eq!(v["missingInformation"][0]["params"]["count"], 3);
    // Two chosen by their positions in the command's list: "3" against "1".
    let run = start(
        &w,
        &o,
        tspec(&o, 2, "C1", "C3", Some([2, 0]), Some("student"), &[]),
    )
    .unwrap();
    assert_eq!(until_terminal_t(&w.w, run)["state"], "succeeded");
    let r = call(&w.w, "stats_ttest_result", json!({ "runId": run })).unwrap();
    assert_eq!(r["result"]["groups"][0]["label"], "3");
    assert_eq!(r["result"]["groups"][1]["label"], "1");
    assert_eq!(
        r["result"]["exclusions"],
        json!([{ "code": "stats.ttest.other_level", "count": 4, "params": { "column": 3 } }])
    );
}

#[test]
fn group_levels_checks_the_workspace_first_and_refuses_with_closed_codes() {
    let w = world_with(None, "coded.csv", &coded_table());
    let o = open(&w);
    // A text column reads too; an unknown column and slot are refused.
    assert_eq!(
        code_of(&group_levels(&w, &o, "C9").unwrap_err()),
        "stats.ttest.unknown_column"
    );
    assert_eq!(
        refusal(&group_levels(&w, &o, "C9").unwrap_err())["params"],
        json!({ "column": 9 })
    );
    assert_eq!(
        code_of(
            &call(
                &w.w,
                "stats_ttest_group_levels",
                json!({ "slot": 99, "column": "C2" })
            )
            .unwrap_err()
        ),
        "run.unknown_input"
    );
    call(&w.w, "workspace_deactivate", json!({})).expect("leaves");
    assert_eq!(
        code_of(&group_levels(&w, &o, "C2").unwrap_err()),
        "workspace.inactive"
    );
}

/// Decision 87: a group column has between 2 and 50 levels. One with 51 is
/// refused with its own code, so a continuous column's values never reach
/// the screen as a list; one with exactly 50 is listed.
#[test]
fn a_group_with_51_levels_is_refused_and_one_with_50_is_listed() {
    let rows: Vec<Vec<String>> = (1..=51)
        .map(|i: u32| {
            vec![
                format!("{}.5", i % 7),
                i.to_string(),
                // 1..=50, the 51st row repeating 50.
                i.min(50).to_string(),
            ]
        })
        .collect();
    let cells: Vec<Vec<C>> = rows
        .iter()
        .map(|r| r.iter().map(|c| C::N(c.as_str())).collect())
        .collect();
    let w = world_with(
        None,
        "wide.csv",
        &delimited(&["y", "fifty_one", "fifty"], &cells, ",", "\n"),
    );
    let o = open(&w);
    assert_eq!(o["profile"]["columns"][1]["distinct"], 51);
    assert_eq!(o["profile"]["columns"][2]["distinct"], 50);

    let e = refusal(&group_levels(&w, &o, "C2").unwrap_err());
    assert_eq!(e["code"], "stats.ttest.too_many_levels", "{e}");
    assert_eq!(e["params"], json!({ "count": 51, "limit": 50 }));

    let g = group_levels(&w, &o, "C3").expect("fifty levels are listed");
    assert_eq!(g["levels"].as_array().unwrap().len(), 50);
    assert_eq!(g["levels"][49]["index"], 49);
}
