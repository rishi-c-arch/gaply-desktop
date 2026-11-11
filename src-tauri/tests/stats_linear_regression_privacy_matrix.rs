//! **The linear regression's privacy matrix (Research OS, Phase 9A-1, decision
//! 674), generated from `stats_linear_regression_privacy_matrix.rs`.** Six sentinels,
//! every surface the method's commands produce, and where each may appear.
//!
//! The sentinels:
//! * `HDR_SENTINEL_k42` and `HDR2_SENTINEL_m17`, the headers of the outcome and
//!   the first predictor;
//! * the file name `FILE_SENTINEL_s63.csv`, in the folder `PATH_SENTINEL_c18`;
//! * `TXT_SENTINEL_v55`, a text cell in a column the user does not select;
//! * `7419.3361`, a number in a column the user does not select.
//!
//! Allowed: the headers in `DatasetView`'s profile; the file's base name in
//! `DatasetView` only; the unselected text cell only as a level label in
//! `DatasetView`'s profile. The method shows no label, so no sentinel may be in
//! its result, its CSV or anywhere else: the spec, the manifest (in the view and
//! as exported bytes), every other view, every refusal, every tracing span and
//! event (a global subscriber, so the worker thread and `spawn_blocking`'s are
//! covered), and every file in the app-data folder, the database's included,
//! which is SHA-256-identical before and after.
//!
//! Alone in its binary on purpose: its tracing subscriber is the global
//! default for the whole process.

mod desc_support;
mod l1_support;
mod linear_regression_support;

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::{Arc, Mutex};

use serde_json::{json, Value};

use desc_support::{call, delimited, spec as desc_spec, until_terminal, TempDir, C};
use gaply_core::Database;
use linear_regression_support::{app, lrspec, until_terminal_regression};

const HDR: &str = "HDR_SENTINEL_k42";
const HDR2: &str = "HDR2_SENTINEL_m17";
const FILE: &str = "FILE_SENTINEL_s63";
const PATH: &str = "PATH_SENTINEL_c18";
const UNSEL_NUM: &str = "7419.3361";
const TXT: &str = "TXT_SENTINEL_v55";

const SENTINELS: [&str; 6] = [HDR, HDR2, FILE, PATH, UNSEL_NUM, TXT];

fn table() -> Vec<u8> {
    let headers = [HDR, HDR2, "third", "note", "aux"];
    let y = ["3.5", "1.25", "4", "2", "5.75", "0.5", "6", "2.25", "3", "4.5", "1", "5"];
    let z = ["4", "2", "6.5", "1.5", "7", "2.5", "9", "3.25", "5.5", "6", "1.75", "8.5"];
    let t = ["5", "3", "7", "4", "8", "3.5", "9.5", "5.25", "6", "7.5", "2.75", "9"];
    let note = ["n1", "n2", TXT, "n4", "n5", "n6", "n7", "n8", "n9", "n10", "n11", "n12"];
    let aux = ["1", "2", "3", "4", UNSEL_NUM, "6", "7", "8", "9", "10", "11", "12"];
    let rows: Vec<Vec<C>> = (0..12)
        .map(|i| vec![C::N(y[i]), C::N(z[i]), C::N(t[i]), C::T(note[i]), C::N(aux[i])])
        .collect();
    delimited(&headers, &rows, ",", "\n")
}

/// Where a sentinel may appear, by surface.
fn allowed(surface: &str, sentinel: &str) -> bool {
    surface == "DatasetView" && [HDR, HDR2, FILE, TXT].contains(&sentinel)
}

fn files_under(dir: &Path, out: &mut Vec<std::path::PathBuf>) {
    for e in std::fs::read_dir(dir).unwrap().flatten() {
        let p = e.path();
        if p.is_dir() {
            files_under(&p, out);
        } else {
            out.push(p);
        }
    }
}

fn file_hashes(dir: &Path) -> BTreeMap<String, String> {
    let mut files = Vec::new();
    files_under(dir, &mut files);
    files
        .iter()
        .map(|f| {
            let rel = f.strip_prefix(dir).unwrap().to_string_lossy().into_owned();
            (
                rel,
                gaply_core::contracts::ContentHash::of_bytes(&std::fs::read(f).unwrap())
                    .as_hex()
                    .to_string(),
            )
        })
        .collect()
}

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    haystack.windows(needle.len()).any(|w| w == needle)
}

fn push(surfaces: &mut Vec<(String, String)>, name: &str, v: &Value) {
    surfaces.push((name.to_string(), v.to_string()));
}

#[test]
fn no_sentinel_reaches_a_surface_it_is_not_allowed_on() {
    let lines = Arc::new(Mutex::new(Vec::<String>::new()));
    tracing::subscriber::set_global_default(l1_support::Capture {
        lines: lines.clone(),
    })
    .expect("the only subscriber");
    tracing::callsite::rebuild_interest_cache();

    let root = TempDir::new("linear-regression-matrix");
    let folder = root.0.join(PATH);
    std::fs::create_dir_all(&folder).unwrap();
    let data = folder.join(format!("{FILE}.csv"));
    std::fs::write(&data, table()).unwrap();
    let app_data = root.0.join("appdata");
    std::fs::create_dir_all(&app_data).unwrap();
    let db = Arc::new(Database::open(&app_data.join("gaply.db")).expect("a migrated database"));
    let (_app, w) = app(&app_data, db.clone(), None);
    let before = file_hashes(&app_data);
    assert!(before.keys().any(|k| k == "gaply.db"), "{before:?}");

    let mut surfaces: Vec<(String, String)> = Vec::new();
    let mut errors: Vec<Value> = Vec::new();

    // The flow, as a screen would drive it.
    let o = call(&w, "stats_desc_open", json!({ "path": data.to_string_lossy() })).expect("open");
    push(&mut surfaces, "DatasetView", &o);
    let spec = lrspec(&o, 1, "C1", &["C2"], &[]);
    push(&mut surfaces, "spec", &spec);
    let v = call(&w, "stats_linear_regression_validate", json!({ "slot": o["slot"], "spec": spec })).unwrap();
    assert_eq!(v["status"], "valid", "{v}");
    push(&mut surfaces, "LinearRegressionValidationView", &v);
    let started = call(&w, "stats_linear_regression_run", json!({ "slot": o["slot"], "spec": spec })).unwrap();
    push(&mut surfaces, "RunStartedView", &started);
    let run = started["runId"].as_u64().unwrap();
    let s = until_terminal_regression(&w, run);
    assert_eq!(s["state"], "succeeded", "{s}");
    push(&mut surfaces, "LinearRegressionRunStateView", &s);
    push(&mut surfaces, "WorkspaceStateView", &call(&w, "workspace_state", json!({})).unwrap());
    let mut r = call(&w, "stats_linear_regression_result", json!({ "runId": run })).unwrap();
    assert_eq!(r["result"]["columns"][0]["column"], "C1", "aliases, never headers");
    assert_eq!(r["result"]["predictors"][0]["column"], "C2", "aliases, never headers");
    let manifest = r.as_object_mut().unwrap().remove("manifest").expect("a manifest");
    push(&mut surfaces, "LinearRegressionResultView (without its manifest)", &r);
    push(&mut surfaces, "manifest (in the result view)", &manifest);
    push(
        &mut surfaces,
        "LinearRegressionRunStateView",
        &call(&w, "stats_linear_regression_cancel", json!({ "runId": run })).unwrap(),
    );
    let csv = call(&w, "stats_linear_regression_export", json!({ "runId": run, "format": "csv" })).unwrap();
    let csv_bytes: Vec<u8> = serde_json::from_value(csv["bytes"].clone()).unwrap();
    surfaces.push(("CSV export".into(), String::from_utf8(csv_bytes).unwrap()));
    surfaces.push(("ExportView (CSV), its other fields".into(), csv["sha256"].to_string()));
    let mj = call(&w, "stats_linear_regression_export", json!({ "runId": run, "format": "manifestJson" })).unwrap();
    let mj_bytes: Vec<u8> = serde_json::from_value(mj["bytes"].clone()).unwrap();
    surfaces.push(("manifest (exported bytes)".into(), String::from_utf8(mj_bytes).unwrap()));

    // Two predictors.
    let three = call(
        &w,
        "stats_linear_regression_run",
        json!({ "slot": o["slot"], "spec": lrspec(&o, 2, "C1", &["C3", "C2"], &[]) }),
    )
    .unwrap()["runId"]
        .as_u64()
        .unwrap();
    assert_eq!(until_terminal_regression(&w, three)["state"], "succeeded");
    let mut fr = call(&w, "stats_linear_regression_result", json!({ "runId": three })).unwrap();
    let fm = fr.as_object_mut().unwrap().remove("manifest").unwrap();
    push(&mut surfaces, "LinearRegressionResultView (without its manifest)", &fr);
    push(&mut surfaces, "manifest (in the result view)", &fm);
    let fcsv = call(&w, "stats_linear_regression_export", json!({ "runId": three, "format": "csv" })).unwrap();
    let fb: Vec<u8> = serde_json::from_value(fcsv["bytes"].clone()).unwrap();
    surfaces.push(("CSV export".into(), String::from_utf8(fb).unwrap()));

    // Remove in the Output: the forget's answer and a second forget's refusal.
    push(
        &mut surfaces,
        "the forget's answer",
        &call(&w, "stats_desc_forget_run", json!({ "runId": three })).unwrap(),
    );
    errors.push(call(&w, "stats_desc_forget_run", json!({ "runId": three })).expect_err("forgotten"));

    // A refused run, as a view, and a validation with several errors, the
    // text column among them.
    let bad = call(
        &w,
        "stats_linear_regression_run",
        json!({ "slot": o["slot"], "spec": lrspec(&o, 3, "C1", &["C1"], &[]) }),
    )
    .unwrap();
    let bad_state = until_terminal_regression(&w, bad["runId"].as_u64().unwrap());
    assert_eq!(bad_state["state"], "refused");
    push(&mut surfaces, "LinearRegressionRunStateView", &bad_state);
    let invalid = call(
        &w,
        "stats_linear_regression_validate",
        json!({ "slot": o["slot"], "spec": lrspec(&o, 4, "C9", &["C4", "C4"], &[99.0]) }),
    )
    .unwrap();
    assert_eq!(invalid["status"], "invalid");
    push(&mut surfaces, "LinearRegressionValidationView", &invalid);

    // Refusals.
    let mut unknown = spec.clone();
    unknown["capabilityId"] = json!("stats.analyze.nonexistent");
    let mut stale = spec.clone();
    stale["capabilityVersion"] = json!(9);
    let missing = folder.join(format!("{FILE}_missing.csv"));
    let d = call(
        &w,
        "stats_desc_run",
        json!({ "slot": o["slot"], "spec": desc_spec(&o, 6, &[("C5", "numeric")], &[]) }),
    )
    .unwrap()["runId"]
        .clone();
    assert_eq!(until_terminal(&w, d.as_u64().unwrap())["state"], "succeeded");
    for (cmd, body) in [
        ("stats_desc_open", json!({ "path": missing.to_string_lossy() })),
        ("stats_linear_regression_validate", json!({ "slot": 99, "spec": spec })),
        ("stats_linear_regression_run", json!({ "slot": 99, "spec": spec })),
        ("stats_linear_regression_validate", json!({ "slot": o["slot"], "spec": unknown })),
        ("stats_linear_regression_run", json!({ "slot": o["slot"], "spec": stale })),
        ("stats_linear_regression_status", json!({ "runId": 99 })),
        ("stats_linear_regression_result", json!({ "runId": bad["runId"] })),
        ("stats_linear_regression_status", json!({ "runId": d })),
        ("stats_desc_result", json!({ "runId": run })),
        ("stats_linear_regression_export", json!({ "runId": 99, "format": "csv" })),
    ] {
        errors.push(call(&w, cmd, body).expect_err(cmd));
    }
    assert_eq!(call(&w, "stats_desc_close", json!({ "slot": o["slot"] })).unwrap(), json!(true));
    // Inactive: every command refused with the workspace's own code.
    call(&w, "workspace_deactivate", json!({})).expect("leaves");
    errors.push(call(&w, "stats_linear_regression_status", json!({ "runId": run })).expect_err("inactive"));
    errors.push(
        call(&w, "stats_linear_regression_validate", json!({ "slot": o["slot"], "spec": spec }))
            .expect_err("inactive"),
    );
    for e in &errors {
        surfaces.push(("a refusal".into(), e.to_string()));
    }

    // Tracing: every span and event, from every thread.
    let traced = lines.lock().unwrap().clone();
    assert!(
        traced.iter().any(|l| l.contains("stats_desc: dataset opened")),
        "spawn_blocking's events were captured: {traced:?}"
    );
    assert!(
        traced.iter().any(|l| l.contains("state=\"succeeded\"") && l.contains("run ended")),
        "the worker's events were captured"
    );
    assert!(
        traced.iter().any(|l| l.contains("stats_linear_regression: refused")),
        "the method's own refusals were traced"
    );
    for l in &traced {
        surfaces.push(("tracing".into(), l.clone()));
    }

    // Files: the app-data folder is unchanged, byte for byte, and holds no
    // sentinel; the data folder holds only what the test wrote.
    let after = file_hashes(&app_data);
    assert_eq!(before, after, "a file in the app-data folder was written or created");
    let mut files = Vec::new();
    files_under(&app_data, &mut files);
    for f in &files {
        let bytes = std::fs::read(f).unwrap();
        surfaces.push((
            format!("file {}", f.strip_prefix(&app_data).unwrap().display()),
            String::from_utf8_lossy(&bytes).into_owned(),
        ));
        let n = 7419.3361f64;
        assert!(
            !contains(&bytes, &n.to_be_bytes()) && !contains(&bytes, &n.to_le_bytes()),
            "{UNSEL_NUM} as a double in {}",
            f.display()
        );
    }
    let mut written = Vec::new();
    files_under(&folder, &mut written);
    assert_eq!(written.len(), 1, "only the test's own file: {written:?}");

    // The matrix.
    let mut grid: BTreeMap<String, Vec<&str>> = BTreeMap::new();
    let mut leaks = Vec::new();
    for (surface, text) in &surfaces {
        let row = grid.entry(surface.clone()).or_insert_with(|| vec!["-"; SENTINELS.len()]);
        for (i, s) in SENTINELS.iter().enumerate() {
            if text.contains(s) {
                if allowed(surface, s) {
                    row[i] = "allowed";
                } else {
                    row[i] = "LEAK";
                    leaks.push(format!("{s} in {surface}"));
                }
            }
        }
    }
    println!("{:<48} {}", "surface", SENTINELS.map(|s| format!("{s:<19}")).join(" "));
    for (surface, row) in &grid {
        println!(
            "{surface:<48} {}",
            row.iter().map(|c| format!("{c:<19}")).collect::<Vec<_>>().join(" ")
        );
    }
    println!(
        "surfaces: {}; refusals: {}; tracing lines: {}; files: {}",
        surfaces.len(),
        errors.len(),
        traced.len(),
        files.len()
    );
    assert!(leaks.is_empty(), "sentinels where they may not be:\n{}", leaks.join("\n"));

    // Not vacuous: each allowed place holds what it is allowed to.
    for s in [HDR, HDR2, FILE, TXT] {
        assert_eq!(
            grid["DatasetView"][SENTINELS.iter().position(|x| *x == s).unwrap()],
            "allowed",
            "{s} should be in DatasetView"
        );
    }
    for surface in [
        "spec",
        "manifest (exported bytes)",
        "manifest (in the result view)",
        "tracing",
        "a refusal",
        "WorkspaceStateView",
        "LinearRegressionRunStateView",
        "LinearRegressionValidationView",
        "LinearRegressionResultView (without its manifest)",
        "CSV export",
    ] {
        assert!(grid.contains_key(surface), "{surface} was collected");
    }
    assert!(
        o.to_string().contains(&format!("\"displayName\":\"{FILE}.csv\"")),
        "the display name is the base name"
    );
    let note_levels = o["profile"]["columns"][3]["levels"].to_string();
    assert!(note_levels.contains(TXT));
    let mut without_levels = o.clone();
    without_levels["profile"]["columns"][3]["levels"] = Value::Null;
    assert!(!without_levels.to_string().contains(TXT), "the text cell outside its level list");
}
