//! **The independent-samples t-test's privacy matrix (Research OS, Phase
//! 4B-2), in the pattern of `stats_desc_privacy_matrix.rs`.** Seven
//! sentinels, every surface the t-test's commands produce, and where each
//! sentinel may appear.
//!
//! The sentinels:
//! * a header, `HDR_SENTINEL_t35`, of the selected outcome;
//! * a level label, `LVL_SENTINEL_g72`, of the selected group (group 1: it
//!   comes before "a" in code-point order);
//! * the file name `FILE_SENTINEL_x48.csv`, in the folder `PATH_SENTINEL_q19`;
//! * `61.8129`, an outcome value, group 1's largest (sorted position 5 of
//!   5), which the t-test does not report. Checked against R 4.6.1
//!   (`/usr/local/bin/Rscript`, 18 Oct 2026,
//!   `~/gaply-corpus/os/phase4b2/r/ttest_sentinel_check.R`): under both
//!   variants t, df, p, the mean difference, its se and interval, d and its
//!   SD, both groups' means and SDs and the SD ratio; none equals it, and
//!   none printed with 15 or 17 digits contains it;
//! * `7123.4567`, in a numeric column the user does not select;
//! * `TXT_SENTINEL_r06`, a text cell in a column the user does not select.
//!
//! Allowed: the header and the label in `DatasetView`'s profile, in the
//! result view (its manifest apart) and in the CSV export's body; the file's
//! base name in `DatasetView` only; the unselected text cell only as a level
//! label in `DatasetView`'s profile. Forbidden everywhere else: the spec,
//! the manifest (in the view and as exported bytes), every other view, every
//! refusal, every tracing span and event (a global subscriber, so the worker
//! thread and `spawn_blocking`'s are covered), and every file in the
//! app-data folder, the database's included, which is SHA-256-identical
//! before and after.
//!
//! Alone in its binary on purpose: its tracing subscriber is the global
//! default for the whole process.

mod desc_support;
mod l1_support;
mod ttest_support;

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::{Arc, Mutex};

use serde_json::{json, Value};

use desc_support::{call, delimited, spec as desc_spec, until_terminal, TempDir, C};
use gaply_core::Database;
use ttest_support::{app, tspec, until_terminal_t};

const HDR: &str = "HDR_SENTINEL_t35";
const LVL: &str = "LVL_SENTINEL_g72";
const FILE: &str = "FILE_SENTINEL_x48";
const PATH: &str = "PATH_SENTINEL_q19";
const SEL_NUM: &str = "61.8129";
const UNSEL_NUM: &str = "7123.4567";
const TXT: &str = "TXT_SENTINEL_r06";

const SENTINELS: [&str; 7] = [HDR, LVL, FILE, PATH, SEL_NUM, UNSEL_NUM, TXT];

fn table() -> Vec<u8> {
    let headers = [HDR, "arm", "note", "aux"];
    let y = [
        "44.1257", "10.5", "95.5", SEL_NUM, "23.25", "50", "82", "31", "70.75", "12",
    ];
    let note = ["n1", "n2", TXT, "n4", "n5", "n6", "n7", "n8", "n9", "n10"];
    let aux = ["1", "2", "3", "4", UNSEL_NUM, "6", "7", "8", "9", "10"];
    let rows: Vec<Vec<C>> = (0..10)
        .map(|i| {
            vec![
                C::N(y[i]),
                C::T(if i % 2 == 0 { "a" } else { LVL }),
                C::T(note[i]),
                C::N(aux[i]),
            ]
        })
        .collect();
    delimited(&headers, &rows, ",", "\n")
}

/// Where a sentinel may appear, by surface.
fn allowed(surface: &str, sentinel: &str) -> bool {
    match surface {
        "DatasetView" => [HDR, LVL, FILE, TXT].contains(&sentinel),
        "TTestResultView (without its manifest)" | "CSV export" => [HDR, LVL].contains(&sentinel),
        _ => false,
    }
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

    let root = TempDir::new("ttest-matrix");
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

    // The flow, as the screen drives it.
    let o = call(
        &w,
        "stats_desc_open",
        json!({ "path": data.to_string_lossy() }),
    )
    .expect("open");
    push(&mut surfaces, "DatasetView", &o);
    assert_eq!(o["profile"]["columns"][1]["levels"][0]["label"], LVL);
    let spec = tspec(&o, 1, "C1", "C2", Some([0, 1]), Some("welch"), &[]);
    push(&mut surfaces, "spec", &spec);
    let v = call(
        &w,
        "stats_ttest_validate",
        json!({ "slot": o["slot"], "spec": spec }),
    )
    .unwrap();
    assert_eq!(v["status"], "valid", "{v}");
    push(&mut surfaces, "ValidationReportView", &v);
    let started = call(
        &w,
        "stats_ttest_run",
        json!({ "slot": o["slot"], "spec": spec }),
    )
    .unwrap();
    push(&mut surfaces, "RunStartedView", &started);
    let run = started["runId"].as_u64().unwrap();
    let s = until_terminal_t(&w, run);
    assert_eq!(s["state"], "succeeded", "{s}");
    push(&mut surfaces, "TTestRunStateView", &s);
    push(
        &mut surfaces,
        "WorkspaceStateView",
        &call(&w, "workspace_state", json!({})).unwrap(),
    );
    let mut r = call(&w, "stats_ttest_result", json!({ "runId": run })).unwrap();
    let manifest = r
        .as_object_mut()
        .unwrap()
        .remove("manifest")
        .expect("a manifest");
    push(&mut surfaces, "TTestResultView (without its manifest)", &r);
    push(&mut surfaces, "manifest (in the result view)", &manifest);
    push(
        &mut surfaces,
        "TTestRunStateView",
        &call(&w, "stats_ttest_cancel", json!({ "runId": run })).unwrap(),
    );
    let csv = call(
        &w,
        "stats_ttest_export",
        json!({ "runId": run, "format": "csv" }),
    )
    .unwrap();
    let csv_bytes: Vec<u8> = serde_json::from_value(csv["bytes"].clone()).unwrap();
    surfaces.push(("CSV export".into(), String::from_utf8(csv_bytes).unwrap()));
    surfaces.push((
        "ExportView (CSV), its other fields".into(),
        csv["sha256"].to_string(),
    ));
    let mj = call(
        &w,
        "stats_ttest_export",
        json!({ "runId": run, "format": "manifestJson" }),
    )
    .unwrap();
    let mj_bytes: Vec<u8> = serde_json::from_value(mj["bytes"].clone()).unwrap();
    surfaces.push((
        "manifest (exported bytes)".into(),
        String::from_utf8(mj_bytes).unwrap(),
    ));
    push(
        &mut surfaces,
        "CapabilityView list",
        &call(&w, "capability_list", json!({})).unwrap(),
    );

    // Refused and questioned runs, as views: an outcome that holds text, no
    // variant, and a validation with an error and a warning.
    let bad = call(
        &w,
        "stats_ttest_run",
        json!({ "slot": o["slot"], "spec": tspec(&o, 2, "C3", "C2", None, Some("welch"), &[]) }),
    )
    .unwrap();
    let bad_state = until_terminal_t(&w, bad["runId"].as_u64().unwrap());
    assert_eq!(bad_state["state"], "refused");
    push(&mut surfaces, "TTestRunStateView", &bad_state);
    let asks = call(
        &w,
        "stats_ttest_run",
        json!({ "slot": o["slot"], "spec": tspec(&o, 3, "C1", "C2", None, None, &[]) }),
    )
    .unwrap();
    let asks_state = until_terminal_t(&w, asks["runId"].as_u64().unwrap());
    assert_eq!(asks_state["state"], "needs_user_input");
    push(&mut surfaces, "TTestRunStateView", &asks_state);
    let invalid = call(
        &w,
        "stats_ttest_validate",
        json!({ "slot": o["slot"], "spec": tspec(&o, 4, "C3", "C9", Some([0, 5]), Some("student"), &[99.0]) }),
    )
    .unwrap();
    assert_eq!(invalid["status"], "invalid");
    push(&mut surfaces, "ValidationReportView", &invalid);

    // Refusals.
    let mut unknown = spec.clone();
    unknown["capabilityId"] = json!("stats.analyze.nonexistent");
    let mut stale = spec.clone();
    stale["capabilityVersion"] = json!(9);
    let missing = folder.join(format!("{FILE}_missing.csv"));
    let d = call(
        &w,
        "stats_desc_run",
        json!({ "slot": o["slot"], "spec": desc_spec(&o, 5, &[("C1", "numeric")], &[]) }),
    )
    .unwrap()["runId"]
        .clone();
    assert_eq!(
        until_terminal(&w, d.as_u64().unwrap())["state"],
        "succeeded"
    );
    for (cmd, body) in [
        (
            "stats_desc_open",
            json!({ "path": missing.to_string_lossy() }),
        ),
        (
            "stats_desc_open",
            json!({ "path": folder.to_string_lossy() }),
        ),
        ("stats_ttest_validate", json!({ "slot": 99, "spec": spec })),
        ("stats_ttest_run", json!({ "slot": 99, "spec": spec })),
        (
            "stats_ttest_validate",
            json!({ "slot": o["slot"], "spec": unknown }),
        ),
        (
            "stats_ttest_run",
            json!({ "slot": o["slot"], "spec": stale }),
        ),
        ("stats_ttest_status", json!({ "runId": 99 })),
        ("stats_ttest_result", json!({ "runId": bad["runId"] })),
        ("stats_ttest_status", json!({ "runId": d })),
        ("stats_desc_result", json!({ "runId": run })),
        (
            "stats_ttest_export",
            json!({ "runId": 99, "format": "csv" }),
        ),
    ] {
        errors.push(call(&w, cmd, body).expect_err(cmd));
    }
    assert_eq!(
        call(&w, "stats_desc_close", json!({ "slot": o["slot"] })).unwrap(),
        json!(true)
    );
    // Inactive: every command refused with the workspace's own code.
    call(&w, "workspace_deactivate", json!({})).expect("leaves");
    errors.push(call(&w, "stats_ttest_status", json!({ "runId": run })).expect_err("inactive"));
    errors.push(
        call(
            &w,
            "stats_ttest_validate",
            json!({ "slot": o["slot"], "spec": spec }),
        )
        .expect_err("inactive"),
    );
    for e in &errors {
        surfaces.push(("a refusal".into(), e.to_string()));
    }

    // Tracing: every span and event, from every thread.
    let traced = lines.lock().unwrap().clone();
    assert!(
        traced
            .iter()
            .any(|l| l.contains("stats_desc: dataset opened")),
        "spawn_blocking's events were captured: {traced:?}"
    );
    assert!(
        traced
            .iter()
            .any(|l| l.contains("state=\"succeeded\"") && l.contains("run ended")),
        "the worker's events were captured"
    );
    assert!(
        traced.iter().any(|l| l.contains("stats_ttest: refused")),
        "the t-test's own refusals were traced"
    );
    for l in &traced {
        surfaces.push(("tracing".into(), l.clone()));
    }

    // Files: the app-data folder is unchanged, byte for byte, and holds no
    // sentinel; the data folder holds only what the test wrote.
    let after = file_hashes(&app_data);
    assert_eq!(
        before, after,
        "a file in the app-data folder was written or created"
    );
    let mut files = Vec::new();
    files_under(&app_data, &mut files);
    for f in &files {
        let bytes = std::fs::read(f).unwrap();
        surfaces.push((
            format!("file {}", f.strip_prefix(&app_data).unwrap().display()),
            String::from_utf8_lossy(&bytes).into_owned(),
        ));
        for (name, n) in [(SEL_NUM, 61.8129f64), (UNSEL_NUM, 7123.4567f64)] {
            assert!(
                !contains(&bytes, &n.to_be_bytes()) && !contains(&bytes, &n.to_le_bytes()),
                "{name} as a double in {}",
                f.display()
            );
        }
    }
    let mut written = Vec::new();
    files_under(&folder, &mut written);
    assert_eq!(written.len(), 1, "only the test's own file: {written:?}");

    // The matrix.
    let mut grid: BTreeMap<String, Vec<&str>> = BTreeMap::new();
    let mut leaks = Vec::new();
    for (surface, text) in &surfaces {
        let row = grid
            .entry(surface.clone())
            .or_insert_with(|| vec!["-"; SENTINELS.len()]);
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
    println!(
        "{:<40} {}",
        "surface",
        SENTINELS.map(|s| format!("{s:<17}")).join(" ")
    );
    for (surface, row) in &grid {
        println!(
            "{surface:<40} {}",
            row.iter()
                .map(|c| format!("{c:<17}"))
                .collect::<Vec<_>>()
                .join(" ")
        );
    }
    println!(
        "surfaces: {}; refusals: {}; tracing lines: {}; files: {}",
        surfaces.len(),
        errors.len(),
        traced.len(),
        files.len()
    );
    assert!(
        leaks.is_empty(),
        "sentinels where they may not be:\n{}",
        leaks.join("\n")
    );

    // Not vacuous: each allowed place holds what it is allowed to, where the
    // surface carries it at all. The result and the CSV carry the two level
    // labels and no header, so the header is allowed there and absent.
    for (surface, wanted) in [
        ("DatasetView", vec![HDR, LVL, FILE, TXT]),
        ("TTestResultView (without its manifest)", vec![LVL]),
        ("CSV export", vec![LVL]),
    ] {
        for s in wanted {
            assert_eq!(
                grid[surface][SENTINELS.iter().position(|x| *x == s).unwrap()],
                "allowed",
                "{s} should be in {surface}"
            );
        }
    }
    for surface in [
        "spec",
        "manifest (exported bytes)",
        "manifest (in the result view)",
        "tracing",
        "a refusal",
        "WorkspaceStateView",
        "CapabilityView list",
    ] {
        assert!(grid.contains_key(surface), "{surface} was collected");
    }
    assert!(
        o.to_string()
            .contains(&format!("\"displayName\":\"{FILE}.csv\"")),
        "the display name is the base name"
    );
    // The unselected text cell is in the open view only as a level label of
    // its column's profile.
    let note_levels = o["profile"]["columns"][2]["levels"].to_string();
    assert!(note_levels.contains(TXT));
    let mut without_levels = o.clone();
    without_levels["profile"]["columns"][2]["levels"] = Value::Null;
    assert!(
        !without_levels.to_string().contains(TXT),
        "the text cell outside its level list"
    );
}
