//! **Crosstabs' privacy matrix (Research OS, Phase 8X-1, decision 580), in
//! the pattern of `stats_anova_privacy_matrix.rs`.** Eight sentinels, every
//! surface crosstabs' commands produce, and where each sentinel may appear.
//!
//! The sentinels:
//! * a header, `HDR_SENTINEL_x91`, of the rows' column;
//! * a level label in the rows' column, `ROWLVL_SENTINEL_q37`;
//! * a level label in the columns' column, `COLLVL_SENTINEL_t58`;
//! * the file name `FILE_SENTINEL_r26.csv`, in the folder `PATH_SENTINEL_b83`;
//! * `TXT_SENTINEL_h44`, a text cell in a column the user does not select;
//! * `8317.2245`, a number in a column the user does not select;
//! * `HDR2_SENTINEL_p15`, the header of the columns' column.
//!
//! Allowed: the headers and the labels in `DatasetView`'s profile; the two
//! labels in the result view (its manifest apart) and in the CSV export's
//! body; the file's base name in `DatasetView` only; the unselected text
//! cell only as a level label in `DatasetView`'s profile. Forbidden
//! everywhere else: the spec, the manifest (in the view and as exported
//! bytes), every other view, every refusal, every tracing span and event (a
//! global subscriber, so the worker thread and `spawn_blocking`'s are
//! covered), and every file in the app-data folder, the database's
//! included, which is SHA-256-identical before and after.
//!
//! Alone in its binary on purpose: its tracing subscriber is the global
//! default for the whole process.

mod desc_support;
mod l1_support;
mod xtab_support;

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::{Arc, Mutex};

use serde_json::{json, Value};

use desc_support::{call, delimited, spec as desc_spec, until_terminal, TempDir, C};
use gaply_core::Database;
use xtab_support::{app, until_terminal_xtab, xspec};

const HDR: &str = "HDR_SENTINEL_x91";
const HDR2: &str = "HDR2_SENTINEL_p15";
const ROWLVL: &str = "ROWLVL_SENTINEL_q37";
const COLLVL: &str = "COLLVL_SENTINEL_t58";
const FILE: &str = "FILE_SENTINEL_r26";
const PATH: &str = "PATH_SENTINEL_b83";
const UNSEL_NUM: &str = "8317.2245";
const TXT: &str = "TXT_SENTINEL_h44";

const SENTINELS: [&str; 8] = [HDR, HDR2, ROWLVL, COLLVL, FILE, PATH, UNSEL_NUM, TXT];

fn table() -> Vec<u8> {
    let headers = [HDR, HDR2, "note", "aux"];
    let r = [
        ROWLVL, "a", ROWLVL, "b", "a", ROWLVL, "b", "a", ROWLVL, "b", "a", "b",
    ];
    let c = [
        "x", COLLVL, COLLVL, "x", "y", "y", COLLVL, "x", "x", "y", COLLVL, "y",
    ];
    let note = [
        "n1", "n2", TXT, "n4", "n5", "n6", "n7", "n8", "n9", "n10", "n11", "n12",
    ];
    let aux = [
        "1", "2", "3", "4", UNSEL_NUM, "6", "7", "8", "9", "10", "11", "12",
    ];
    let rows: Vec<Vec<C>> = (0..12)
        .map(|i| vec![C::T(r[i]), C::T(c[i]), C::T(note[i]), C::N(aux[i])])
        .collect();
    delimited(&headers, &rows, ",", "\n")
}

/// Where a sentinel may appear, by surface.
fn allowed(surface: &str, sentinel: &str) -> bool {
    match surface {
        "DatasetView" => [HDR, HDR2, ROWLVL, COLLVL, FILE, TXT].contains(&sentinel),
        "XtabResultView (without its manifest)" | "CSV export" => {
            [ROWLVL, COLLVL].contains(&sentinel)
        }
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

    let root = TempDir::new("xtab-matrix");
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
    let o = call(
        &w,
        "stats_desc_open",
        json!({ "path": data.to_string_lossy() }),
    )
    .expect("open");
    push(&mut surfaces, "DatasetView", &o);
    let spec = xspec(&o, 1, "C1", "C2", &[]);
    push(&mut surfaces, "spec", &spec);
    let v = call(
        &w,
        "stats_xtab_validate",
        json!({ "slot": o["slot"], "spec": spec }),
    )
    .unwrap();
    assert_eq!(v["status"], "valid", "{v}");
    push(&mut surfaces, "XtabValidationView", &v);
    let started = call(
        &w,
        "stats_xtab_run",
        json!({ "slot": o["slot"], "spec": spec }),
    )
    .unwrap();
    push(&mut surfaces, "RunStartedView", &started);
    let run = started["runId"].as_u64().unwrap();
    let s = until_terminal_xtab(&w, run);
    assert_eq!(s["state"], "succeeded", "{s}");
    push(&mut surfaces, "XtabRunStateView", &s);
    push(
        &mut surfaces,
        "WorkspaceStateView",
        &call(&w, "workspace_state", json!({})).unwrap(),
    );
    let mut r = call(&w, "stats_xtab_result", json!({ "runId": run })).unwrap();
    // Code-point order: "COLLVL_…" and "ROWLVL_…" before the lower-case
    // letters.
    assert_eq!(r["result"]["rowLevels"][0]["label"], ROWLVL);
    assert_eq!(r["result"]["columnLevels"][0]["label"], COLLVL);
    let manifest = r
        .as_object_mut()
        .unwrap()
        .remove("manifest")
        .expect("a manifest");
    push(&mut surfaces, "XtabResultView (without its manifest)", &r);
    push(&mut surfaces, "manifest (in the result view)", &manifest);
    push(
        &mut surfaces,
        "XtabRunStateView",
        &call(&w, "stats_xtab_cancel", json!({ "runId": run })).unwrap(),
    );
    let csv = call(
        &w,
        "stats_xtab_export",
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
        "stats_xtab_export",
        json!({ "runId": run, "format": "manifestJson" }),
    )
    .unwrap();
    let mj_bytes: Vec<u8> = serde_json::from_value(mj["bytes"].clone()).unwrap();
    surfaces.push((
        "manifest (exported bytes)".into(),
        String::from_utf8(mj_bytes).unwrap(),
    ));

    // Remove in the Output: the forget's answer and a second forget's refusal.
    let forgotten = call(
        &w,
        "stats_xtab_run",
        json!({ "slot": o["slot"], "spec": spec }),
    )
    .unwrap()["runId"]
        .as_u64()
        .unwrap();
    assert_eq!(until_terminal_xtab(&w, forgotten)["state"], "succeeded");
    push(
        &mut surfaces,
        "the forget's answer",
        &call(&w, "stats_desc_forget_run", json!({ "runId": forgotten })).unwrap(),
    );
    errors.push(
        call(&w, "stats_desc_forget_run", json!({ "runId": forgotten })).expect_err("forgotten"),
    );

    // Refused runs, as views: one column twice, and a column of one level
    // once the code 1 leaves only the others; and a validation with several
    // errors.
    let bad = call(
        &w,
        "stats_xtab_run",
        json!({ "slot": o["slot"], "spec": xspec(&o, 2, "C1", "C1", &[]) }),
    )
    .unwrap();
    let bad_state = until_terminal_xtab(&w, bad["runId"].as_u64().unwrap());
    assert_eq!(bad_state["state"], "refused");
    push(&mut surfaces, "XtabRunStateView", &bad_state);
    let few = call(
        &w,
        "stats_xtab_run",
        json!({ "slot": o["slot"], "spec": xspec(&o, 3, "C1", "C3", &[]) }),
    )
    .unwrap();
    let few_state = until_terminal_xtab(&w, few["runId"].as_u64().unwrap());
    // The note column has twelve levels, one a sentinel: a refusal names
    // counts, never its labels.
    push(&mut surfaces, "XtabRunStateView", &few_state);
    let invalid = call(
        &w,
        "stats_xtab_validate",
        json!({ "slot": o["slot"], "spec": xspec(&o, 4, "C9", "C9", &[99.0, 9.0]) }),
    )
    .unwrap();
    assert_eq!(invalid["status"], "invalid");
    push(&mut surfaces, "XtabValidationView", &invalid);

    // Refusals.
    let mut unknown = spec.clone();
    unknown["capabilityId"] = json!("stats.analyze.nonexistent");
    let mut stale = spec.clone();
    stale["capabilityVersion"] = json!(9);
    let missing = folder.join(format!("{FILE}_missing.csv"));
    let d = call(
        &w,
        "stats_desc_run",
        json!({ "slot": o["slot"], "spec": desc_spec(&o, 5, &[("C4", "numeric")], &[]) }),
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
        ("stats_xtab_validate", json!({ "slot": 99, "spec": spec })),
        ("stats_xtab_run", json!({ "slot": 99, "spec": spec })),
        (
            "stats_xtab_validate",
            json!({ "slot": o["slot"], "spec": unknown }),
        ),
        (
            "stats_xtab_run",
            json!({ "slot": o["slot"], "spec": stale }),
        ),
        ("stats_xtab_status", json!({ "runId": 99 })),
        ("stats_xtab_result", json!({ "runId": bad["runId"] })),
        ("stats_xtab_status", json!({ "runId": d })),
        ("stats_desc_result", json!({ "runId": run })),
        ("stats_xtab_export", json!({ "runId": 99, "format": "csv" })),
    ] {
        errors.push(call(&w, cmd, body).expect_err(cmd));
    }
    assert_eq!(
        call(&w, "stats_desc_close", json!({ "slot": o["slot"] })).unwrap(),
        json!(true)
    );
    // Inactive: every command refused with the workspace's own code.
    call(&w, "workspace_deactivate", json!({})).expect("leaves");
    errors.push(call(&w, "stats_xtab_status", json!({ "runId": run })).expect_err("inactive"));
    errors.push(
        call(
            &w,
            "stats_xtab_validate",
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
        traced.iter().any(|l| l.contains("stats_xtab: refused")),
        "crosstabs' own refusals were traced"
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
        let n = 8317.2245f64;
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
        SENTINELS.map(|s| format!("{s:<19}")).join(" ")
    );
    for (surface, row) in &grid {
        println!(
            "{surface:<40} {}",
            row.iter()
                .map(|c| format!("{c:<19}"))
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

    // Not vacuous: each allowed place holds what it is allowed to.
    for (surface, wanted) in [
        ("DatasetView", vec![HDR, HDR2, ROWLVL, COLLVL, FILE, TXT]),
        ("XtabResultView (without its manifest)", vec![ROWLVL, COLLVL]),
        ("CSV export", vec![ROWLVL, COLLVL]),
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
        "XtabRunStateView",
        "XtabValidationView",
    ] {
        assert!(grid.contains_key(surface), "{surface} was collected");
    }
    assert!(
        o.to_string()
            .contains(&format!("\"displayName\":\"{FILE}.csv\"")),
        "the display name is the base name"
    );
    let note_levels = o["profile"]["columns"][2]["levels"].to_string();
    assert!(note_levels.contains(TXT));
    let mut without_levels = o.clone();
    without_levels["profile"]["columns"][2]["levels"] = Value::Null;
    assert!(
        !without_levels.to_string().contains(TXT),
        "the text cell outside its level list"
    );
}
