//! **The confirmatory factor analysis's commands at their boundary (Research OS,
//! Phase 9C-1, decision 749)**, on `stats_factor_commands.rs`'s pattern. Every
//! call goes through Tauri's IPC on the mock runtime, with the webview's argument
//! names, into the state `AppState::new` builds, and every assertion reads the
//! JSON the webview reads.
//!
//! * The flow: open → validate → run → status → result → export (CSV and
//!   manifest) → close, the numbers equal to `stats_cfa`'s on the same dataset and
//!   the CSV equal to its export.
//! * The progress counts every check point, `stats_cfa::check_points`, and the
//!   iterations call the estimation's again.
//! * A cancel mid-covariances and one during the iterations each stop the run
//!   with no result; the run manager's ceiling stops a run whose iterations
//!   "sleep" past it, on an injected clock, never 30 s of real time, and the same
//!   spec on a still clock gives a result.
//! * Refusals, each with its exact code, the run's own among them
//!   (`sample_covariance_singular`, `model_not_identified`, `cfa_not_converged`),
//!   and the registry's `capability.*` codes as they are (decision 74), before the
//!   slot is read. One refusal is not sent here: `missing_code_not_finite`, since
//!   JSON has no NaN or infinity to send; `cfa_cases.rs` holds it.
//! * Statistics inactive: all six commands refuse `workspace.inactive`.
//! * `run.wrong_kind` both ways against the summary, and against the ANOVA's,
//!   the rank tests', the related rank tests', the linear regression's and the
//!   factor analysis's commands.
//!
//! Every file is synthetic and in a folder the test creates and removes.

mod cfa_support;
mod desc_support;
mod prep_support;

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::{json, Value};

use app_lib::runs::{CheckpointHook, Clock, Point, Runs};
use app_lib::stats_l1_ingest::load_dataset;
use cfa_support::{app, app_with, cspec, until_terminal_cfa};
use desc_support::{call, delimited, refusal, spec as desc_spec, until_terminal, TempDir, C};
use gaply_core::contracts::{ContentHash, RunManifest};
use gaply_core::stats_cfa::export::{manifest_json, result_csv};
use gaply_core::stats_cfa::{self, CfaCode, CfaParams};
use gaply_core::stats_desc::Alias;
use gaply_core::stats_l1::dataset::ColumnAlias;
use gaply_core::Database;

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

/// The bug hunt's dataset h041 (seed 20260746, ~/gaply-corpus/os/phase9/9c1/hunt/):
/// 34 rows of eight synthetic columns, the first factor C1, C2, C3, C7, C8 and the
/// second C4, C5, C6, correlated, on which decision 745's Fisher scoring from its
/// starts does not reach its stop within 500 iterations (R's own scoring from the
/// same starts does not either), so the run refuses as cfa_not_converged.
const NOT_CONVERGED: &[u8] = b"x1,x2,x3,x4,x5,x6,x7,x8\n-8.8984,59.5534,60.7944,83.9235,-85.0705,-41.29,62.6071,7.8669\n-9.0338,59.5501,60.7733,81.6645,-85.0717,-40.8426,6.535,-11.0439\n-4.5305,59.5538,60.7814,83.6228,-85.0721,-40.9012,73.5662,526.922\n-7.7741,59.5512,60.7409,86.282,-85.0672,-41.1175,77.0245,98.4211\n-11.1557,59.5545,60.8388,77.7212,-85.074,-41.1177,19.6049,312.6662\n-6.1488,59.5527,60.7671,87.0073,-85.0752,-40.7,60.3244,-120.9867\n-8.8571,59.5521,60.7952,83.8586,-85.075,-41.5756,76.3996,157.898\n-12.046,59.553,60.7905,77.9539,-85.0643,-41.4919,70.6684,176.3188\n-6.9381,59.5529,60.803,85.2376,-85.0723,-41.1737,107.5797,-66.9645\n-10.8926,59.5513,60.7661,84.802,-85.0774,-41.188,-14.9624,-162.4867\n-11.2668,59.5526,60.8141,82.107,-85.0724,-41.7805,32.3799,-198.494\n-10.2525,59.552,60.752,83.9345,-85.0717,-40.9361,68.6432,-248.7544\n-6.1454,59.5513,60.788,88.2049,-85.0756,-41.2264,41.6248,-282.0135\n-3.8053,59.551,60.7888,83.414,-85.0754,-40.4868,-9.7591,157.7591\n-8.0268,59.553,60.7955,85.9451,-85.0698,-40.8263,93.1988,-29.2516\n-4.4392,59.5533,60.8076,82.1094,-85.0692,-40.9765,165.0175,255.4041\n-12.4343,59.5522,60.7366,84.4989,-85.0752,-40.513,127.7898,174.4233\n-7.294,59.5511,60.8054,84.5239,-85.0739,-41.6268,66.8572,225.5601\n-9.9634,59.5525,60.7567,81.3878,-85.0752,-41.1567,17.7198,-75.1718\n-4.2156,59.5532,60.773,80.5079,-85.0757,-40.8708,71.5191,-40.2031\n-7.275,59.5517,60.7213,86.4525,-85.0666,-40.8999,18.4897,4.2818\n-8.2658,59.5524,60.736,81.3498,-85.0699,-40.7537,95.5567,-215.8202\n-9.5955,59.5525,60.7747,88.2441,-85.0743,-41.464,11.1069,105.5568\n-11.425,59.5534,60.7915,86.1603,-85.076,-40.6615,48.3462,-413.8615\n-4.8955,59.5521,60.7709,83.453,-85.075,-41.1096,42.5785,23.4984\n-8.0315,59.553,60.7997,86.1877,-85.0684,-41.0116,21.9465,19.2242\n-7.4517,59.5537,60.8565,85.8081,-85.0665,-40.762,33.5504,65.5634\n-5.0888,59.553,60.9056,83.2703,-85.0701,-41.193,29.463,385.5392\n-7.1619,59.5534,60.8471,83.2257,-85.0659,-41.4744,37.3595,221.4536\n-8.3704,59.5526,60.7853,78.109,-85.0726,-41.3302,4.788,-286.5376\n-8.4185,59.5544,60.8483,82.6384,-85.0782,-41.5338,-36.4768,317.7412\n-5.2108,59.552,60.7596,86.3038,-85.0728,-40.9152,44.3469,-60.2179\n-4.4497,59.5551,60.8364,88.9047,-85.0717,-41.1968,114.1039,213.4461\n-2.9546,59.557,60.8761,87.9073,-85.0695,-40.8782,10.492,55.4648\n";

struct World {
    _dir: TempDir,
    _app: tauri::App<tauri::test::MockRuntime>,
    w: tauri::WebviewWindow<tauri::test::MockRuntime>,
    data: std::path::PathBuf,
    stuck: std::path::PathBuf,
}

fn world(runs: Option<Arc<Runs>>) -> World {
    let dir = TempDir::new("cfa");
    let data = dir.write("data.csv", &csv());
    let stuck = dir.write("stuck.csv", NOT_CONVERGED);
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

/// The analysis most tests run: two correlated factors of three columns.
fn two(o: &Value, op: u64) -> Value {
    cspec(o, op, &[&FACTOR_1, &FACTOR_2], true, &[])
}

fn start(w: &World, o: &Value, spec: Value) -> Result<u64, Value> {
    call(&w.w, "stats_cfa_run", json!({ "slot": o["slot"], "spec": spec })).map(|v| v["runId"].as_u64().expect("runId"))
}

fn code_of(err: &Value) -> String {
    refusal(err)["code"].as_str().unwrap().to_string()
}

/// JSON text and back, as the view went.
fn through_text<T: serde::Serialize>(v: &T) -> Value {
    serde_json::from_str(&serde_json::to_string(v).unwrap()).unwrap()
}

fn direct_params() -> CfaParams {
    let a = |j| Alias(ColumnAlias(j));
    CfaParams {
        indicators: vec![vec![a(1), a(3), a(5)], vec![a(2), a(4), a(6)]],
        correlated_factors: true,
        missing_codes: vec![],
    }
}

/// The check point before the estimation for six columns: 1 + 6.
const ESTIMATION_POINT: usize = 7;

/* ---- the flow --------------------------------------------------------------- */

#[test]
fn open_validate_run_status_result_export_close_through_ipc() {
    let w = world(None);
    let o = open(&w);
    let dataset = load_dataset(&w.data).expect("the same file");

    let v = call(&w.w, "stats_cfa_validate", json!({ "slot": o["slot"], "spec": two(&o, 1) })).unwrap();
    assert_eq!(v["status"], "valid", "{v}");
    assert_eq!(v["errors"], json!([]));
    assert_eq!(v["estimatedResources"], json!({ "rows": 40, "columns": 6, "cells": 240 }));

    let run = start(&w, &o, two(&o, 2)).unwrap();
    let s = until_terminal_cfa(&w.w, run);
    assert_eq!(s["state"], "succeeded", "{s}");
    assert_eq!(s["stepsTotal"], stats_cfa::check_points(&direct_params()) as u64, "{s}");
    assert_eq!(s["stepsDone"], s["stepsTotal"], "{s}");

    let r = call(&w.w, "stats_cfa_result", json!({ "runId": run })).unwrap();
    assert_eq!((r["runId"].as_u64(), r["status"].as_str()), (Some(run), Some("succeeded")));
    let direct = stats_cfa::compute(&dataset, &direct_params()).expect("computes");
    assert_eq!(r["result"], through_text(&direct));
    assert_eq!(r["result"]["items"], json!(["C1", "C3", "C5", "C2", "C4", "C6"]));
    assert_eq!(r["result"]["rowsUsed"], 40);
    assert_eq!(r["result"]["factorCovariances"].as_array().unwrap().len(), 1);
    assert_eq!(r["result"]["limitations"].as_array().unwrap().len(), 10);
    let m = &r["manifest"];
    assert_eq!(m["capabilityId"], "stats.analyze.cfa");
    assert_eq!(m["outputHash"], ContentHash::of_bytes(&direct.canonical_bytes()).as_hex(), "bit for bit");
    assert_eq!((m["rowsUsed"].as_u64(), m["rowsRead"].as_u64()), (Some(40), Some(40)));

    let manifest: RunManifest<CfaCode> = serde_json::from_value(m.clone()).unwrap();
    let e = call(&w.w, "stats_cfa_export", json!({ "runId": run, "format": "csv" })).unwrap();
    let bytes: Vec<u8> = serde_json::from_value(e["bytes"].clone()).unwrap();
    assert_eq!(bytes, result_csv(&direct, &manifest));
    assert_eq!(e["sha256"], ContentHash::of_bytes(&bytes).as_hex());
    let j = call(&w.w, "stats_cfa_export", json!({ "runId": run, "format": "manifestJson" })).unwrap();
    let jb: Vec<u8> = serde_json::from_value(j["bytes"].clone()).unwrap();
    assert_eq!(jb, manifest_json(&manifest));

    assert_eq!(call(&w.w, "stats_desc_close", json!({ "slot": o["slot"] })).unwrap(), true);
}

/* ---- refusals --------------------------------------------------------------- */

#[test]
fn each_refusal_has_its_code() {
    let w = world(None);
    let o = open(&w);
    let validate = |spec: Value| call(&w.w, "stats_cfa_validate", json!({ "slot": o["slot"], "spec": spec })).unwrap();
    let one = |op, cols: &[&str]| cspec(&o, op, &[cols], true, &[]);
    let mut no_input = two(&o, 21);
    no_input["inputs"] = json!([]);
    let mut other_input = two(&o, 22);
    other_input["inputs"][0]["contentHash"] = json!("cd".repeat(32));
    let mut checked = 0;
    for (spec, code, params) in [
        (no_input, "stats.cfa.input_count", json!({ "count": 0 })),
        (other_input, "stats.cfa.input_mismatch", json!({})),
        (cspec(&o, 1, &[&["C1", "C3", "C5"]], true, &[2.0, 2.0]), "stats.cfa.missing_code_repeated", json!({ "index": 1 })),
        (cspec(&o, 2, &[], true, &[]), "stats.cfa.no_factors", json!({})),
        (cspec(&o, 3, &[&["C1"], &["C2", "C4"]], true, &[]), "stats.cfa.factor_too_few_items", json!({ "index": 1, "count": 1, "limit": 2 })),
        (cspec(&o, 4, &[&["C1", "C2"]], true, &[]), "stats.cfa.too_few_items", json!({ "count": 2, "limit": 3 })),
        (one(5, &["C3", "C1", "C5"]), "stats.cfa.items_out_of_order", json!({ "column": 1 })),
        (cspec(&o, 6, &[&["C2", "C4"], &["C1", "C3"]], true, &[]), "stats.cfa.factors_out_of_order", json!({ "index": 2 })),
        (cspec(&o, 7, &[&["C1", "C3"], &["C3", "C5"]], true, &[]), "stats.cfa.item_repeated", json!({ "column": 3 })),
        (one(8, &["C1", "C3", "C99"]), "stats.cfa.unknown_column", json!({ "column": 99 })),
        (one(9, &["C1", "C3", "C9"]), "stats.cfa.column_non_finite", json!({ "column": 9, "count": 1 })),
        (one(10, &["C1", "C3", "C7"]), "stats.cfa.column_holds_text", json!({ "column": 7, "count": 40 })),
        (one(11, &["C1", "C16", "C17"]), "stats.cfa.no_complete_rows", json!({})),
        (one(12, &["C1", "C3", "C12"]), "stats.cfa.too_few_rows", json!({ "count": 2, "limit": 4 })),
        (one(13, &["C1", "C3", "C10"]), "stats.cfa.item_constant", json!({ "column": 10 })),
    ] {
        let v = validate(spec);
        assert_eq!(v["status"], "invalid", "{code}: {v}");
        assert_eq!(v["errors"][0]["code"], code, "{v}");
        assert_eq!(v["errors"][0]["params"], params, "{code}: {v}");
        checked += 1;
    }
    assert_eq!(checked, 15);
    // A refused run, as a view: the run is made and refused by its check.
    let bad = start(&w, &o, one(14, &["C1", "C1", "C3"])).unwrap();
    let s = until_terminal_cfa(&w.w, bad);
    assert_eq!(s["state"], "refused", "{s}");
    assert_eq!(s["diagnostics"][0]["code"], "stats.cfa.item_repeated");
    // The run's own refusals: validation passes, and the run's work refuses,
    // recorded as a failed run carrying the refusal, as for every kind. C11, the
    // first column times 1e300, overflows S and refuses as S does.
    for (spec, code, params) in [
        (one(15, &["C1", "C2", "C8"]), "stats.cfa.sample_covariance_singular", json!({ "column": 8 })),
        (one(16, &["C1", "C3", "C11"]), "stats.cfa.sample_covariance_singular", json!({ "column": 11 })),
        (cspec(&o, 17, &[&["C1", "C3"], &["C2", "C4"]], false, &[]), "stats.cfa.model_not_identified", json!({})),
    ] {
        assert_eq!(validate(spec.clone())["status"], "valid", "{code}");
        let run = start(&w, &o, spec).unwrap();
        let s = until_terminal_cfa(&w.w, run);
        assert_eq!(s["state"], "failed", "{code}: {s}");
        assert_eq!(s["diagnostics"][0]["code"], code, "{s}");
        assert_eq!(s["diagnostics"][0]["params"], params, "{s}");
    }
    let stuck = call(&w.w, "stats_desc_open", json!({ "path": w.stuck.to_string_lossy() })).expect("open");
    let spec = cspec(&stuck, 18, &[&["C1", "C2", "C3", "C7", "C8"], &["C4", "C5", "C6"]], true, &[]);
    let run = call(&w.w, "stats_cfa_run", json!({ "slot": stuck["slot"], "spec": spec })).unwrap()["runId"].as_u64().unwrap();
    let s = until_terminal_cfa(&w.w, run);
    assert_eq!(s["state"], "failed", "{s}");
    assert_eq!(s["diagnostics"][0]["code"], "stats.cfa.cfa_not_converged", "{s}");
    assert_eq!(s["diagnostics"][0]["params"], json!({ "limit": 500 }), "{s}");
    // The registry before the slot.
    let mut s = two(&o, 19);
    s["capabilityVersion"] = json!(2);
    for cmd in ["stats_cfa_validate", "stats_cfa_run"] {
        let e = call(&w.w, cmd, json!({ "slot": 999, "spec": s })).unwrap_err();
        assert_eq!(code_of(&e), "capability.version_mismatch", "{cmd}");
    }
    for other in ["stats.describe", "stats.verify.ols", "stats.analyze.factor"] {
        let mut t = two(&o, 20);
        t["capabilityId"] = json!(other);
        let e = call(&w.w, "stats_cfa_run", json!({ "slot": o["slot"], "spec": t })).unwrap_err();
        assert!(code_of(&e).starts_with("capability."), "{other}: {e}");
    }
    let e = call(&w.w, "stats_cfa_validate", json!({ "slot": 999, "spec": two(&o, 23) })).unwrap_err();
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
        let mut s = cspec(&view, op, &[&["C4", "C5", "C18"]], true, codes);
        s["preparation"] = view["preparation"].clone();
        s
    };
    let validate = |spec: Value| call(&w.w, "stats_cfa_validate", json!({ "slot": o["slot"], "spec": spec })).unwrap();
    let v = validate(prepared(1, &[]));
    assert_eq!(v["status"], "valid", "a score is a column like any other: {v}");
    let v = validate(prepared(2, &[score as f64]));
    assert_eq!(v["status"], "invalid", "{v}");
    assert_eq!(v["errors"][0]["code"], "stats.cfa.code_equals_score", "{v}");
    assert_eq!(v["errors"][0]["params"]["column"], 18);
    let v = validate(cspec(&view, 3, &[&["C4", "C5", "C18"]], true, &[]));
    assert_eq!(v["errors"][0]["code"], "stats.cfa.preparation_mismatch", "{v}");
}

#[test]
fn every_command_refuses_while_statistics_is_inactive() {
    let dir = TempDir::new("cfa-inactive");
    let app_data = dir.0.join("appdata");
    std::fs::create_dir_all(&app_data).unwrap();
    let db = Arc::new(Database::in_memory().unwrap());
    let (_app, w) = app_with(&app_data, db, Arc::new(|| Arc::new(Runs::default())), Arc::new(app_lib::workspace::readiness));
    let o = json!({ "slot": 1, "inputRef": { "contentHash": "ab".repeat(32), "rows": 1, "columns": 1, "semanticRole": "table" } });
    let spec = two(&o, 1);
    let mut checked = 0;
    for (cmd, body) in [
        ("stats_cfa_validate", json!({ "slot": 1, "spec": spec })),
        ("stats_cfa_run", json!({ "slot": 1, "spec": spec })),
        ("stats_cfa_status", json!({ "runId": 1 })),
        ("stats_cfa_result", json!({ "runId": 1 })),
        ("stats_cfa_cancel", json!({ "runId": 1 })),
        ("stats_cfa_export", json!({ "runId": 1, "format": "csv" })),
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
    until_terminal_cfa(&w.w, k);
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
        ("stats_cfa", d),
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
    assert_eq!(checked, 28);
    assert_eq!(code_of(&call(&w.w, "stats_cfa_status", json!({ "runId": 99 })).unwrap_err()), "run.unknown_run");
    let k2 = start(&w, &o, two(&o, 3)).unwrap();
    until_terminal_cfa(&w.w, k2);
    let s = call(&w.w, "workspace_state", json!({})).unwrap();
    assert_eq!(s["latestRun"], json!({ "runId": k2, "kind": "cfa" }));
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
    let s = until_terminal_cfa(&w.w, run);
    assert_eq!(s["state"], "succeeded", "{s}");
    assert_eq!(s["stepsTotal"], 9);
    assert_eq!(s["stepsDone"], 9);
    let seen = seen.lock().unwrap().clone();
    let mut distinct = seen.clone();
    distinct.dedup();
    assert_eq!(distinct, (0..9).collect::<Vec<usize>>());
    let r = call(&w.w, "stats_cfa_result", json!({ "runId": run })).unwrap();
    let iterations = r["result"]["iterations"].as_u64().unwrap();
    assert!(iterations >= 2, "the premise: more than one step");
    assert_eq!(seen.iter().filter(|i| **i == ESTIMATION_POINT).count() as u64, 1 + iterations, "one at the start, one per step: {seen:?}");
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
    let c = call(&w.w, "stats_cfa_cancel", json!({ "runId": run })).unwrap();
    assert_eq!((c["state"].as_str(), c["cancelRequested"].as_bool()), (Some("running"), Some(true)), "{c}");
    go.send(()).unwrap();
    let s = until_terminal_cfa(&w.w, run);
    assert_eq!(s["state"], "cancelled", "{s}");
    assert_eq!(s["stepsDone"], steps_done, "{s}");
    for (cmd, body) in [
        ("stats_cfa_result", json!({ "runId": run })),
        ("stats_cfa_export", json!({ "runId": run, "format": "csv" })),
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
    // second step's passes the manager's 30 s ceiling. Without the iterations'
    // calls the run would sleep 11 s and end.
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
    let s = until_terminal_cfa(&w.w, run);
    assert_eq!(s["state"], "timed_out", "{s}");
    assert_eq!(code_of(&call(&w.w, "stats_cfa_result", json!({ "runId": run })).unwrap_err()), "run.timed_out");
    // The same spec on a clock that never moves: a result, using more than two
    // iterations, so the ceiling above was reached during them.
    let (hook, _) = recording();
    let w = world(Some(runs_with(Arc::new(TestClock(AtomicU64::new(1_000))), hook)));
    let o = open(&w);
    let run = start(&w, &o, two(&o, 1)).unwrap();
    assert_eq!(until_terminal_cfa(&w.w, run)["state"], "succeeded");
    let r = call(&w.w, "stats_cfa_result", json!({ "runId": run })).unwrap();
    assert!(r["result"]["iterations"].as_u64().unwrap() >= 2, "{}", r["result"]["iterations"]);
}
