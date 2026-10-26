//! **The one-sample and paired t-tests' commands at their boundary
//! (Research OS, Phase 8D-2a, decision 263).** Every call goes through
//! Tauri's IPC on the mock runtime, with the webview's argument names, into
//! the state `AppState::new` builds, and every assertion reads the JSON the
//! webview reads.
//!
//! * The flow, for each test: open → validate → run → status → result →
//!   export (CSV and manifest) → close, the numbers equal to
//!   `stats_onesample`'s on the same dataset.
//! * Refusals, each with its exact code: an unchosen test value
//!   (NeedsUserInput), a column that holds text, an unknown column, too few
//!   values, the same column twice, too few pairs, constant values and
//!   differences, values and differences that are essentially constant
//!   (decision 259), and the registry's `capability.*` codes as they are
//!   (decision 74).
//! * Statistics inactive: all twelve commands refuse `workspace.inactive`.
//! * The registry before the slot: a version mismatch is refused as such
//!   even for a slot no dataset has, and makes no record.
//! * `run.wrong_kind` both ways against the summary, the independent t-test
//!   and each other, for status, result, cancel and export.
//! * Cancels: parked at the check point, and during the arithmetic.
//! * One foreground run across the kinds.
//!
//! Every file is synthetic and in a folder the test creates and removes.

mod desc_support;
mod onesample_support;

use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde_json::{json, Value};

use app_lib::runs::{CheckpointHook, Clock, Point, Runs};
use app_lib::stats_l1_ingest::load_dataset;
use desc_support::{call, delimited, refusal, spec as desc_spec, until_terminal, TempDir, C};
use gaply_core::contracts::{ContentHash, RunId};
use gaply_core::stats_desc::Alias;
use gaply_core::stats_l1::dataset::ColumnAlias;
use gaply_core::stats_onesample::{self, OneSampleParams, PairedParams};
use gaply_core::Database;
use onesample_support::{app, app_with, ospec, pspec, until_terminal_of};

/* ---- fixtures --------------------------------------------------------------- */

const HEADERS: [&str; 10] = [
    "before", "after", "arm", "note", "flat", "near", "a3", "b3", "minus1", "sparse",
];

fn rows() -> Vec<Vec<C<'static>>> {
    let before = [
        "12.25", "9.5", "14.125", "11", "10.75", "15.5", "13.25", "19", "16.75", "21.5",
    ];
    let after = [
        "13.5", "9.25", "15", "12.75", "11.5", "15.25", "14.5", "20.125", "16", "23",
    ];
    // `before` minus 1, exact in binary: the differences are all 1.
    let minus1 = [
        "11.25", "8.5", "13.125", "10", "9.75", "14.5", "12.25", "18", "15.75", "20.5",
    ];
    // A 3-decimal grid and the same minus 2: equal in decimal, an ulp apart in
    // binary on some rows (decision 259's paired case).
    let a3 = [
        "5.123", "4.871", "6.042", "3.987", "5.555", "4.444", "5.876", "6.213", "4.019", "5.731",
    ];
    let b3 = [
        "3.123", "2.871", "4.042", "1.987", "3.555", "2.444", "3.876", "4.213", "2.019", "3.731",
    ];
    (0..10)
        .map(|i| {
            vec![
                C::N(before[i]),
                C::N(after[i]),
                C::T(if i % 2 == 0 { "ctrl" } else { "trt" }),
                C::T(["t1", "t2", "t3", "t4", "t5", "t6", "t7", "t8", "t9", "t10"][i]),
                C::N("5"),
                C::N(if i == 3 { "0.30000000000000004" } else { "0.3" }),
                C::N(a3[i]),
                C::N(b3[i]),
                C::N(minus1[i]),
                if i == 0 { C::N("7") } else { C::Empty },
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
    let dir = TempDir::new("onesample");
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

/// The one-sample analysis the flow runs: `before` against 12.
fn one(o: &Value, op: u64) -> Value {
    ospec(o, op, "C1", Some(12.0), &[])
}

/// The paired analysis the flow runs: `before` and `after`.
fn pair(o: &Value, op: u64) -> Value {
    pspec(o, op, "C1", "C2", &[])
}

fn start(w: &World, o: &Value, prefix: &str, spec: Value) -> Result<u64, Value> {
    call(
        &w.w,
        &format!("{prefix}_run"),
        json!({ "slot": o["slot"], "spec": spec }),
    )
    .map(|v| v["runId"].as_u64().expect("runId"))
}

fn code_of(err: &Value) -> String {
    refusal(err)["code"].as_str().unwrap().to_string()
}

/// JSON text and back, as the view went: serde_json is built without
/// `float_roundtrip`, so the view is compared with the direct result through
/// the same text. The bit-for-bit claim rests on the output hash and the CSV
/// bytes, which never pass through a float parse.
fn through_text<T: serde::Serialize>(v: &T) -> Value {
    serde_json::from_str(&serde_json::to_string(v).unwrap()).unwrap()
}

/* ---- the flow --------------------------------------------------------------- */

#[test]
fn open_validate_run_status_result_export_close_through_ipc_for_both() {
    let w = world(None);
    let o = open(&w);
    let dataset = load_dataset(&w.data).expect("the same file");

    // The one-sample t-test.
    let v = call(
        &w.w,
        "stats_onesample_validate",
        json!({ "slot": o["slot"], "spec": one(&o, 1) }),
    )
    .unwrap();
    assert_eq!(v["status"], "valid", "{v}");
    assert_eq!(v["errors"], json!([]));
    assert_eq!(v["missingInformation"], json!([]));
    let run = start(&w, &o, "stats_onesample", one(&o, 2)).unwrap();
    let s = until_terminal_of(&w.w, "stats_onesample", run);
    assert_eq!(s["state"], "succeeded", "{s}");
    assert_eq!(s["cancelRequested"], false);
    assert!(
        s.get("columnsDone").is_none() && s.get("columnsTotal").is_none(),
        "no column progress: {s}"
    );
    let r = call(&w.w, "stats_onesample_result", json!({ "runId": run })).unwrap();
    assert_eq!(r["runId"], run);
    assert_eq!(r["status"], "succeeded");
    let direct = stats_onesample::one_sample_compute(
        &dataset,
        &OneSampleParams {
            outcome: Alias(ColumnAlias(1)),
            test_value: Some(12.0),
            missing_codes: vec![],
        },
    )
    .expect("computes");
    assert_eq!(r["result"], through_text(&direct));
    assert_eq!(r["result"]["testValue"], 12.0);
    // No warning in the validation; the limitations are the result's own.
    assert_eq!(r["diagnostics"], json!([]));
    assert_eq!(
        r["result"]["limitations"],
        json!([
            "stats.onesample.values_normality_not_assessed",
            "stats.onesample.cohens_d_has_no_interval"
        ])
    );
    let m = &r["manifest"];
    assert_eq!(m["capabilityId"], "stats.analyze.one_sample_t");
    assert_eq!(m["capabilityVersion"], 1);
    assert_eq!(m["rowsRead"], 10);
    assert_eq!(m["rowsUsed"], 10);
    assert_eq!(m["inputs"], json!([o["inputRef"]["contentHash"]]));
    assert_eq!(
        m["outputHash"],
        ContentHash::of_bytes(&direct.canonical_bytes())
            .as_hex()
            .to_string()
    );
    let csv = call(
        &w.w,
        "stats_onesample_export",
        json!({ "runId": run, "format": "csv" }),
    )
    .unwrap();
    let bytes: Vec<u8> = serde_json::from_value(csv["bytes"].clone()).unwrap();
    assert_eq!(
        bytes,
        stats_onesample::export::one_sample_result_csv(&direct)
    );
    assert_eq!(
        csv["sha256"],
        ContentHash::of_bytes(&bytes).as_hex().to_string()
    );
    let mj = call(
        &w.w,
        "stats_onesample_export",
        json!({ "runId": run, "format": "manifestJson" }),
    )
    .unwrap();
    let mj_bytes: Vec<u8> = serde_json::from_value(mj["bytes"].clone()).unwrap();
    let parsed: Value = serde_json::from_slice(&mj_bytes).expect("JSON");
    assert_eq!(parsed["outputHash"], m["outputHash"]);

    // The paired t-test.
    let v = call(
        &w.w,
        "stats_paired_validate",
        json!({ "slot": o["slot"], "spec": pair(&o, 3) }),
    )
    .unwrap();
    assert_eq!(v["status"], "valid", "{v}");
    let run = start(&w, &o, "stats_paired", pair(&o, 4)).unwrap();
    assert_eq!(
        until_terminal_of(&w.w, "stats_paired", run)["state"],
        "succeeded"
    );
    let r = call(&w.w, "stats_paired_result", json!({ "runId": run })).unwrap();
    let direct = stats_onesample::paired_compute(
        &dataset,
        &PairedParams {
            first: Alias(ColumnAlias(1)),
            second: Alias(ColumnAlias(2)),
            missing_codes: vec![],
        },
    )
    .expect("computes");
    assert_eq!(r["result"], through_text(&direct));
    assert_eq!(r["manifest"]["capabilityId"], "stats.analyze.paired_t");
    assert_eq!(r["manifest"]["rowsUsed"], 10);
    assert_eq!(
        r["manifest"]["outputHash"],
        ContentHash::of_bytes(&direct.canonical_bytes())
            .as_hex()
            .to_string()
    );
    let csv = call(
        &w.w,
        "stats_paired_export",
        json!({ "runId": run, "format": "csv" }),
    )
    .unwrap();
    let bytes: Vec<u8> = serde_json::from_value(csv["bytes"].clone()).unwrap();
    assert_eq!(bytes, stats_onesample::export::paired_result_csv(&direct));
    let mj = call(
        &w.w,
        "stats_paired_export",
        json!({ "runId": run, "format": "manifestJson" }),
    )
    .unwrap();
    let mj_bytes: Vec<u8> = serde_json::from_value(mj["bytes"].clone()).unwrap();
    assert_eq!(
        mj_bytes,
        stats_onesample::export::manifest_json(
            &serde_json::from_value(r["manifest"].clone()).expect("a manifest")
        ),
        "the manifest export is the result's manifest"
    );

    assert_eq!(
        call(&w.w, "stats_desc_close", json!({ "slot": o["slot"] })).unwrap(),
        json!(true)
    );
    for (cmd, spec) in [
        ("stats_onesample_validate", one(&o, 5)),
        ("stats_paired_validate", pair(&o, 6)),
    ] {
        assert_eq!(
            code_of(&call(&w.w, cmd, json!({ "slot": o["slot"], "spec": spec })).unwrap_err()),
            "run.unknown_input",
            "{cmd}"
        );
    }
}

/* ---- refusals --------------------------------------------------------------- */

fn validated(w: &World, o: &Value, cmd: &str, spec: Value) -> Value {
    call(&w.w, cmd, json!({ "slot": o["slot"], "spec": spec })).expect("a validation view")
}

/// A run that passes validation and ends `Failed`, with the code of its one
/// diagnostic.
fn failed_with(w: &World, o: &Value, prefix: &str, spec: Value) -> String {
    let run = start(w, o, prefix, spec).expect("starts");
    let s = until_terminal_of(&w.w, prefix, run);
    assert_eq!(s["state"], "failed", "{s}");
    s["diagnostics"][0]["code"].as_str().unwrap().to_string()
}

#[test]
fn each_refusal_has_its_exact_code() {
    let w = world(None);
    let o = open(&w);
    let ov = "stats_onesample_validate";
    let pv = "stats_paired_validate";

    // NeedsUserInput: the test value is never chosen for the user.
    let v = validated(&w, &o, ov, ospec(&o, 1, "C1", None, &[]));
    assert_eq!(v["status"], "needs_user_input", "{v}");
    assert_eq!(
        v["missingInformation"][0]["code"],
        "stats.onesample.test_value_not_chosen"
    );
    // Invalid: a column that holds text, for either test.
    let v = validated(&w, &o, ov, ospec(&o, 2, "C4", Some(0.0), &[]));
    assert_eq!(v["status"], "invalid", "{v}");
    assert_eq!(v["errors"][0]["code"], "stats.onesample.column_holds_text");
    assert_eq!(v["errors"][0]["params"]["count"], 10);
    let v = validated(&w, &o, pv, pspec(&o, 3, "C1", "C4", &[]));
    assert_eq!(
        v["errors"][0]["code"], "stats.onesample.column_holds_text",
        "{v}"
    );
    // Invalid: an unknown column.
    let v = validated(&w, &o, ov, ospec(&o, 4, "C99", Some(0.0), &[]));
    assert_eq!(
        v["errors"][0]["code"], "stats.onesample.unknown_column",
        "{v}"
    );
    assert_eq!(v["errors"][0]["params"]["column"], 99);
    // Invalid: one value.
    let v = validated(&w, &o, ov, ospec(&o, 5, "C10", Some(0.0), &[]));
    assert_eq!(
        v["errors"][0]["code"], "stats.onesample.too_few_values",
        "{v}"
    );
    assert_eq!(v["errors"][0]["params"]["count"], 1);
    // Invalid: the same column twice; one pair.
    let v = validated(&w, &o, pv, pspec(&o, 6, "C1", "C1", &[]));
    assert_eq!(v["errors"][0]["code"], "stats.onesample.same_column", "{v}");
    let v = validated(&w, &o, pv, pspec(&o, 7, "C1", "C10", &[]));
    assert_eq!(
        v["errors"][0]["code"], "stats.onesample.too_few_pairs",
        "{v}"
    );
    assert_eq!(v["errors"][0]["params"]["count"], 1);

    // Not computable: the run ends Failed with the arithmetic's code.
    assert_eq!(
        failed_with(
            &w,
            &o,
            "stats_onesample",
            ospec(&o, 8, "C5", Some(0.0), &[])
        ),
        "stats.onesample.values_constant"
    );
    assert_eq!(
        failed_with(
            &w,
            &o,
            "stats_onesample",
            ospec(&o, 9, "C6", Some(0.0), &[])
        ),
        "stats.onesample.values_essentially_constant"
    );
    assert_eq!(
        failed_with(&w, &o, "stats_paired", pspec(&o, 10, "C1", "C9", &[])),
        "stats.onesample.differences_constant"
    );
    assert_eq!(
        failed_with(&w, &o, "stats_paired", pspec(&o, 11, "C7", "C8", &[])),
        "stats.onesample.differences_essentially_constant"
    );

    // The registry's refusals, as the registry words them (decision 74),
    // from validate and from run alike.
    for (prefix, spec) in [
        ("stats_onesample", one(&o, 12)),
        ("stats_paired", pair(&o, 13)),
    ] {
        let mut unknown = spec.clone();
        unknown["capabilityId"] = json!("stats.analyze.nonexistent");
        let mut stale = spec.clone();
        stale["capabilityVersion"] = json!(2);
        for cmd in [format!("{prefix}_validate"), format!("{prefix}_run")] {
            let e = refusal(
                &call(
                    &w.w,
                    &cmd,
                    json!({ "slot": o["slot"], "spec": unknown.clone() }),
                )
                .unwrap_err(),
            );
            assert_eq!(e["code"], "capability.unknown_capability", "{cmd}: {e}");
            let e = refusal(
                &call(
                    &w.w,
                    &cmd,
                    json!({ "slot": o["slot"], "spec": stale.clone() }),
                )
                .unwrap_err(),
            );
            assert_eq!(e["code"], "capability.version_mismatch", "{cmd}: {e}");
            assert_eq!(e["params"], json!({ "expected": 1, "found": 2 }), "{cmd}");
        }
    }
    // Each test's spec is the other's wrong spec: their parameters differ, so
    // neither reads as the other (`deny_unknown_fields`).
    for (cmd, spec, field) in [
        ("stats_onesample_validate", pair(&o, 14), "first"),
        ("stats_paired_validate", one(&o, 15), "outcome"),
    ] {
        let e = call(&w.w, cmd, json!({ "slot": o["slot"], "spec": spec })).unwrap_err();
        assert!(e.to_string().contains(field), "{cmd}: {e}");
    }
    // A run that needs input makes a record that ends NeedsUserInput.
    let run = start(&w, &o, "stats_onesample", ospec(&o, 16, "C1", None, &[])).unwrap();
    let s = until_terminal_of(&w.w, "stats_onesample", run);
    assert_eq!(s["state"], "needs_user_input");
    assert_eq!(
        s["diagnostics"][0]["code"],
        "stats.onesample.test_value_not_chosen"
    );
}

#[test]
fn every_command_is_refused_while_statistics_is_not_active() {
    let dir = TempDir::new("onesample-inactive");
    let app_data = dir.0.join("appdata");
    std::fs::create_dir_all(&app_data).unwrap();
    let db = Arc::new(Database::in_memory().unwrap());
    let (_app, w) = app_with(
        &app_data,
        db,
        Arc::new(|| Arc::new(Runs::default())),
        Arc::new(app_lib::workspace::readiness),
    );
    let fake = json!({ "inputRef": { "contentHash": "0".repeat(64), "rows": 1, "columns": 1, "semanticRole": "table" } });
    let mut n = 0;
    for (prefix, spec) in [
        ("stats_onesample", ospec(&fake, 1, "C1", Some(0.0), &[])),
        ("stats_paired", pspec(&fake, 2, "C1", "C2", &[])),
    ] {
        for (cmd, body) in [
            ("validate", json!({ "slot": 1, "spec": spec })),
            ("run", json!({ "slot": 1, "spec": spec })),
            ("status", json!({ "runId": 1 })),
            ("result", json!({ "runId": 1 })),
            ("cancel", json!({ "runId": 1 })),
            ("export", json!({ "runId": 1, "format": "csv" })),
        ] {
            let full = format!("{prefix}_{cmd}");
            let e = refusal(&call(&w, &full, body).unwrap_err());
            assert_eq!(e["code"], "workspace.inactive", "{full}: {e}");
            n += 1;
        }
    }
    assert_eq!(n, 12);
}

/// Decision 74, as `methods_seam.rs` pins it for the t-test: the registry is
/// asked before the slot, so a version this build does not have is refused
/// as such for a slot no dataset has, and no record is made.
#[test]
fn both_ask_the_registry_before_they_read_the_slot() {
    let w = world(None);
    let o = open(&w);
    let before = call(&w.w, "workspace_state", json!({})).unwrap();
    for (prefix, spec) in [
        ("stats_onesample", one(&o, 1)),
        ("stats_paired", pair(&o, 2)),
    ] {
        let mut stale = spec.clone();
        stale["capabilityVersion"] = json!(2);
        for slot in [o["slot"].clone(), json!(999)] {
            for cmd in [format!("{prefix}_validate"), format!("{prefix}_run")] {
                let e = refusal(
                    &call(&w.w, &cmd, json!({ "slot": slot, "spec": stale.clone() })).unwrap_err(),
                );
                assert_eq!(
                    e["code"], "capability.version_mismatch",
                    "{cmd} slot {slot}: {e}"
                );
            }
        }
        // With the registry satisfied, the missing slot is what is refused.
        let e = call(
            &w.w,
            &format!("{prefix}_validate"),
            json!({ "slot": 999, "spec": spec }),
        )
        .unwrap_err();
        assert_eq!(code_of(&e), "run.unknown_input", "{prefix}");
    }
    let after = call(&w.w, "workspace_state", json!({})).unwrap();
    assert_eq!(
        after["latestRun"], before["latestRun"],
        "no record was made"
    );
    assert_eq!(after["latestRun"], Value::Null);
}

/* ---- run.wrong_kind ----------------------------------------------------------- */

#[test]
fn each_kinds_commands_refuse_a_run_of_another_kind() {
    let w = world(None);
    let o = open(&w);
    let d = call(
        &w.w,
        "stats_desc_run",
        json!({ "slot": o["slot"], "spec": desc_spec(&o, 1, &[("C1", "numeric")], &[]) }),
    )
    .unwrap()["runId"]
        .as_u64()
        .unwrap();
    assert_eq!(until_terminal(&w.w, d)["state"], "succeeded");
    let t = call(
        &w.w,
        "stats_ttest_run",
        json!({ "slot": o["slot"], "spec": {
            "schemaVersion": 1, "operationId": 2, "workspaceId": "statistics",
            "capabilityId": "stats.analyze.independent_t", "capabilityVersion": 1,
            "inputs": [o["inputRef"]],
            "parameters": { "outcome": "C1", "group": "C3", "levels": null, "variant": "welch", "missingCodes": [] },
            "randomSeed": null, "confirmationRefs": []
        } }),
    )
    .unwrap()["runId"]
        .as_u64()
        .unwrap();
    assert_eq!(
        until_terminal_of(&w.w, "stats_ttest", t)["state"],
        "succeeded"
    );
    let os = start(&w, &o, "stats_onesample", one(&o, 3)).unwrap();
    assert_eq!(
        until_terminal_of(&w.w, "stats_onesample", os)["state"],
        "succeeded"
    );
    let p = start(&w, &o, "stats_paired", pair(&o, 4)).unwrap();
    assert_eq!(
        until_terminal_of(&w.w, "stats_paired", p)["state"],
        "succeeded"
    );

    let kinds = [
        ("stats_desc", d),
        ("stats_ttest", t),
        ("stats_onesample", os),
        ("stats_paired", p),
    ];
    let mut checked = 0;
    for (prefix, own) in kinds {
        for (other_prefix, other) in kinds {
            if prefix == other_prefix {
                continue;
            }
            // The new kinds' runs read by every other kind, and every other
            // kind's run read by the new kinds; the summary and the t-test
            // between themselves are `stats_ttest_commands.rs`'s.
            let new = |p: &str| p == "stats_onesample" || p == "stats_paired";
            if !new(prefix) && !new(other_prefix) {
                continue;
            }
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
                    "{full} given {other_prefix}'s run {other}"
                );
                checked += 1;
            }
        }
        // Each kind still reads its own.
        assert_eq!(
            call(&w.w, &format!("{prefix}_status"), json!({ "runId": own })).unwrap()["state"],
            "succeeded",
            "{prefix}"
        );
    }
    // 10 ordered pairs that involve a new kind, 4 commands each.
    assert_eq!(checked, 40);
    for prefix in ["stats_onesample", "stats_paired"] {
        assert_eq!(
            code_of(&call(&w.w, &format!("{prefix}_status"), json!({ "runId": 99 })).unwrap_err()),
            "run.unknown_run"
        );
    }
}

/* ---- cancels and one foreground run --------------------------------------------- */

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

fn runs_with(hook: CheckpointHook) -> Arc<Runs> {
    Arc::new(Runs::with_hooks(
        Arc::new(SinceTest(Instant::now())),
        Some(hook),
        30_000,
    ))
}

/// Parked at the one check point and cancelled; and cancelled after the check
/// point has read the flag, while the arithmetic runs. Either way the run ends
/// `Cancelled` and its result and export are `run.cancelled`.
#[test]
fn a_cancel_at_or_after_the_check_point_ends_cancelled_with_no_result() {
    for point in [Point::Column(0), Point::Passed(0)] {
        for prefix in ["stats_onesample", "stats_paired"] {
            let (hook, parked, go) = park_when(move |_, p| p == point);
            let w = world(Some(runs_with(hook)));
            let o = open(&w);
            let spec = if prefix == "stats_paired" {
                pair(&o, 1)
            } else {
                one(&o, 1)
            };
            let run = start(&w, &o, prefix, spec).unwrap();
            parked
                .recv_timeout(Duration::from_secs(10))
                .expect("parked");
            let c = call(&w.w, &format!("{prefix}_cancel"), json!({ "runId": run })).unwrap();
            assert_eq!(c["state"], "running", "{prefix} {point:?}");
            assert_eq!(c["cancelRequested"], true);
            go.send(()).unwrap();
            let s = until_terminal_of(&w.w, prefix, run);
            assert_eq!(s["state"], "cancelled", "{prefix} {point:?}: {s}");
            for (cmd, body) in [
                ("result", json!({ "runId": run })),
                ("export", json!({ "runId": run, "format": "csv" })),
            ] {
                assert_eq!(
                    code_of(&call(&w.w, &format!("{prefix}_{cmd}"), body).unwrap_err()),
                    "run.cancelled",
                    "{prefix} {cmd} {point:?}"
                );
            }
        }
    }
}

#[test]
fn one_foreground_run_across_the_kinds() {
    for prefix in ["stats_onesample", "stats_paired"] {
        let (hook, parked, go) = park_when(|run, p| run == RunId(1) && p == Point::Column(0));
        let runs = runs_with(hook);
        let w = world(Some(runs.clone()));
        let o = open(&w);
        let spec = if prefix == "stats_paired" {
            pair(&o, 1)
        } else {
            one(&o, 1)
        };
        let run = start(&w, &o, prefix, spec).unwrap();
        parked
            .recv_timeout(Duration::from_secs(10))
            .expect("parked");
        // Every other start is refused, and nothing is recorded.
        assert_eq!(
            code_of(&start(&w, &o, "stats_onesample", one(&o, 2)).unwrap_err()),
            "run.run_busy"
        );
        assert_eq!(
            code_of(&start(&w, &o, "stats_paired", pair(&o, 3)).unwrap_err()),
            "run.run_busy"
        );
        let e = call(
            &w.w,
            "stats_desc_run",
            json!({ "slot": o["slot"], "spec": desc_spec(&o, 4, &[("C1", "numeric")], &[]) }),
        )
        .unwrap_err();
        assert_eq!(code_of(&e), "run.run_busy");
        assert_eq!(runs.held(), 1, "a refused start makes no record");
        // The shell names the parked run and its kind (decision 80).
        let kind = if prefix == "stats_paired" {
            "paired"
        } else {
            "one_sample"
        };
        let s = call(&w.w, "workspace_state", json!({})).unwrap();
        assert_eq!(s["activeRun"], json!({ "runId": run, "kind": kind }));
        go.send(()).unwrap();
        assert_eq!(until_terminal_of(&w.w, prefix, run)["state"], "succeeded");
        let s = call(&w.w, "workspace_state", json!({})).unwrap();
        assert_eq!(s["activeRun"], Value::Null);
        assert_eq!(s["latestRun"], json!({ "runId": run, "kind": kind }));
    }
}
