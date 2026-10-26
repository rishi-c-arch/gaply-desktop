//! **The one-sample and paired t-tests' privacy matrix (Research OS, Phase
//! 8D-2a, decision 263), in the shape of `stats_ttest_privacy_matrix.rs`.**
//! Eight sentinels, every surface the new commands produce, and where each
//! sentinel may appear.
//!
//! The sentinels:
//! * two headers, `HDR_SENTINEL_o35` and `HDR2_SENTINEL_p44`, of the selected
//!   columns (the one-sample test's outcome is the first; the paired test's
//!   first and second are both);
//! * the file name `FILE_SENTINEL_y51.csv`, in the folder `PATH_SENTINEL_k27`;
//! * `48.3917`, a value of the selected first column, neither its smallest nor
//!   its largest;
//! * `7123.4567`, in a numeric column the user does not select;
//! * `TXT_SENTINEL_w83`, a text cell in a column the user does not select;
//! * `3.14159`, the one-sample test's test value: a setting the user chose,
//!   with a decimal point and digits no statistic of either run contains.
//!
//! Allowed: the two headers, the file's base name and the unselected text
//! cell (as a level label) in `DatasetView` only, as in the t-test's matrix;
//! neither test's result carries a header or a label. The test value in the
//! one-sample spec, its result view (its manifest apart) and its CSV export,
//! and nowhere else. Forbidden everywhere else: the paired spec, both
//! manifests (in the views and as exported bytes), every other view, every
//! refusal, every tracing span and event (a global subscriber, so the worker
//! thread and `spawn_blocking`'s are covered), and every file in the app-data
//! folder, the database's included, which is SHA-256-identical before and
//! after.
//!
//! Alone in its binary on purpose: its tracing subscriber is the global
//! default for the whole process.

mod desc_support;
mod l1_support;
mod onesample_support;

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::{Arc, Mutex};

use serde_json::{json, Value};

use desc_support::{call, delimited, spec as desc_spec, until_terminal, TempDir, C};
use gaply_core::Database;
use onesample_support::{app, ospec, pspec, until_terminal_of};

const HDR: &str = "HDR_SENTINEL_o35";
const HDR2: &str = "HDR2_SENTINEL_p44";
const FILE: &str = "FILE_SENTINEL_y51";
const PATH: &str = "PATH_SENTINEL_k27";
const SEL_NUM: &str = "48.3917";
const UNSEL_NUM: &str = "7123.4567";
const TXT: &str = "TXT_SENTINEL_w83";
const TEST_VALUE: &str = "3.14159";

const SENTINELS: [&str; 8] = [HDR, HDR2, FILE, PATH, SEL_NUM, UNSEL_NUM, TXT, TEST_VALUE];

fn table() -> Vec<u8> {
    let headers = [HDR, HDR2, "note", "aux"];
    let first = [
        "44.1257", "10.5", "95.5", SEL_NUM, "23.25", "50", "82", "31", "70.75", "12",
    ];
    let second = [
        "41.5", "13.25", "90.125", "45.75", "27", "52.5", "77.25", "35", "66.5", "15.75",
    ];
    let note = ["n1", "n2", TXT, "n4", "n5", "n6", "n7", "n8", "n9", "n10"];
    let aux = ["1", "2", "3", "4", UNSEL_NUM, "6", "7", "8", "9", "10"];
    let rows: Vec<Vec<C>> = (0..10)
        .map(|i| vec![C::N(first[i]), C::N(second[i]), C::T(note[i]), C::N(aux[i])])
        .collect();
    delimited(&headers, &rows, ",", "\n")
}

/// Where a sentinel may appear, by surface.
fn allowed(surface: &str, sentinel: &str) -> bool {
    match surface {
        "DatasetView" => [HDR, HDR2, FILE, TXT].contains(&sentinel),
        "one-sample spec"
        | "OneSampleResultView (without its manifest)"
        | "CSV export (one-sample)" => [TEST_VALUE].contains(&sentinel),
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

/// The flow of one test through its six commands, each surface collected.
/// `which` is the surface names' word: "one-sample" or "paired".
fn flow(
    w: &tauri::WebviewWindow<tauri::test::MockRuntime>,
    o: &Value,
    prefix: &str,
    which: &str,
    spec: &Value,
    surfaces: &mut Vec<(String, String)>,
) -> u64 {
    push(surfaces, &format!("{which} spec"), spec);
    let v = call(
        w,
        &format!("{prefix}_validate"),
        json!({ "slot": o["slot"], "spec": spec }),
    )
    .unwrap();
    assert_eq!(v["status"], "valid", "{v}");
    push(surfaces, "OneSampleValidationView", &v);
    let started = call(
        w,
        &format!("{prefix}_run"),
        json!({ "slot": o["slot"], "spec": spec }),
    )
    .unwrap();
    push(surfaces, "RunStartedView", &started);
    let run = started["runId"].as_u64().unwrap();
    let s = until_terminal_of(w, prefix, run);
    assert_eq!(s["state"], "succeeded", "{s}");
    push(surfaces, "OneSampleRunStateView", &s);
    push(
        surfaces,
        "WorkspaceStateView",
        &call(w, "workspace_state", json!({})).unwrap(),
    );
    let mut r = call(w, &format!("{prefix}_result"), json!({ "runId": run })).unwrap();
    let manifest = r
        .as_object_mut()
        .unwrap()
        .remove("manifest")
        .expect("a manifest");
    let view = if which == "paired" {
        "PairedResultView (without its manifest)"
    } else {
        "OneSampleResultView (without its manifest)"
    };
    push(surfaces, view, &r);
    push(surfaces, "manifest (in the result view)", &manifest);
    push(
        surfaces,
        "OneSampleRunStateView",
        &call(w, &format!("{prefix}_cancel"), json!({ "runId": run })).unwrap(),
    );
    let csv = call(
        w,
        &format!("{prefix}_export"),
        json!({ "runId": run, "format": "csv" }),
    )
    .unwrap();
    let csv_bytes: Vec<u8> = serde_json::from_value(csv["bytes"].clone()).unwrap();
    surfaces.push((
        format!("CSV export ({which})"),
        String::from_utf8(csv_bytes).unwrap(),
    ));
    surfaces.push((
        "ExportView (CSV), its other fields".into(),
        csv["sha256"].to_string(),
    ));
    let mj = call(
        w,
        &format!("{prefix}_export"),
        json!({ "runId": run, "format": "manifestJson" }),
    )
    .unwrap();
    let mj_bytes: Vec<u8> = serde_json::from_value(mj["bytes"].clone()).unwrap();
    surfaces.push((
        "manifest (exported bytes)".into(),
        String::from_utf8(mj_bytes).unwrap(),
    ));
    run
}

#[test]
fn no_sentinel_reaches_a_surface_it_is_not_allowed_on() {
    let lines = Arc::new(Mutex::new(Vec::<String>::new()));
    tracing::subscriber::set_global_default(l1_support::Capture {
        lines: lines.clone(),
    })
    .expect("the only subscriber");
    tracing::callsite::rebuild_interest_cache();

    let root = TempDir::new("onesample-matrix");
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

    // The flows, as a screen will drive them.
    let o = call(
        &w,
        "stats_desc_open",
        json!({ "path": data.to_string_lossy() }),
    )
    .expect("open");
    push(&mut surfaces, "DatasetView", &o);
    let tv: f64 = TEST_VALUE.parse().unwrap();
    let one = ospec(&o, 1, "C1", Some(tv), &[]);
    assert!(one.to_string().contains(TEST_VALUE), "the spec carries it");
    let run1 = flow(&w, &o, "stats_onesample", "one-sample", &one, &mut surfaces);
    let pair = pspec(&o, 2, "C1", "C2", &[]);
    let run2 = flow(&w, &o, "stats_paired", "paired", &pair, &mut surfaces);

    // Refused and questioned runs, as views: a column that holds text, no
    // test value, and validations with errors and a warning.
    let bad = call(
        &w,
        "stats_onesample_run",
        json!({ "slot": o["slot"], "spec": ospec(&o, 3, "C3", Some(tv), &[]) }),
    )
    .unwrap();
    let bad_state = until_terminal_of(&w, "stats_onesample", bad["runId"].as_u64().unwrap());
    assert_eq!(bad_state["state"], "refused");
    push(&mut surfaces, "OneSampleRunStateView", &bad_state);
    let asks = call(
        &w,
        "stats_onesample_run",
        json!({ "slot": o["slot"], "spec": ospec(&o, 4, "C1", None, &[]) }),
    )
    .unwrap();
    let asks_state = until_terminal_of(&w, "stats_onesample", asks["runId"].as_u64().unwrap());
    assert_eq!(asks_state["state"], "needs_user_input");
    push(&mut surfaces, "OneSampleRunStateView", &asks_state);
    for (cmd, spec) in [
        (
            "stats_onesample_validate",
            ospec(&o, 5, "C3", Some(tv), &[99.0]),
        ),
        ("stats_paired_validate", pspec(&o, 6, "C3", "C9", &[99.0])),
        ("stats_paired_validate", pspec(&o, 7, "C1", "C1", &[])),
    ] {
        let invalid = call(&w, cmd, json!({ "slot": o["slot"], "spec": spec })).unwrap();
        assert_eq!(invalid["status"], "invalid", "{cmd}: {invalid}");
        push(&mut surfaces, "OneSampleValidationView", &invalid);
    }

    // Refusals.
    let missing = folder.join(format!("{FILE}_missing.csv"));
    let d = call(
        &w,
        "stats_desc_run",
        json!({ "slot": o["slot"], "spec": desc_spec(&o, 8, &[("C1", "numeric")], &[]) }),
    )
    .unwrap()["runId"]
        .clone();
    assert_eq!(
        until_terminal(&w, d.as_u64().unwrap())["state"],
        "succeeded"
    );
    errors.push(
        call(
            &w,
            "stats_desc_open",
            json!({ "path": missing.to_string_lossy() }),
        )
        .expect_err("missing"),
    );
    errors.push(
        call(
            &w,
            "stats_desc_open",
            json!({ "path": folder.to_string_lossy() }),
        )
        .expect_err("a folder"),
    );
    for (prefix, spec, other) in [
        ("stats_onesample", &one, run2),
        ("stats_paired", &pair, run1),
    ] {
        let mut unknown = spec.clone();
        unknown["capabilityId"] = json!("stats.analyze.nonexistent");
        let mut stale = spec.clone();
        stale["capabilityVersion"] = json!(9);
        for (cmd, body) in [
            ("validate", json!({ "slot": 99, "spec": spec })),
            ("run", json!({ "slot": 99, "spec": spec })),
            ("validate", json!({ "slot": o["slot"], "spec": unknown })),
            ("run", json!({ "slot": o["slot"], "spec": stale })),
            ("status", json!({ "runId": 99 })),
            ("result", json!({ "runId": bad["runId"] })),
            ("status", json!({ "runId": d })),
            ("status", json!({ "runId": other })),
            ("export", json!({ "runId": 99, "format": "csv" })),
        ] {
            let full = format!("{prefix}_{cmd}");
            errors.push(call(&w, &full, body).expect_err(&full));
        }
    }
    assert_eq!(
        call(&w, "stats_desc_close", json!({ "slot": o["slot"] })).unwrap(),
        json!(true)
    );
    // Inactive: every command refused with the workspace's own code.
    call(&w, "workspace_deactivate", json!({})).expect("leaves");
    for (cmd, body) in [
        ("stats_onesample_status", json!({ "runId": run1 })),
        (
            "stats_onesample_validate",
            json!({ "slot": o["slot"], "spec": one }),
        ),
        ("stats_paired_result", json!({ "runId": run2 })),
        (
            "stats_paired_run",
            json!({ "slot": o["slot"], "spec": pair }),
        ),
    ] {
        errors.push(call(&w, cmd, body).expect_err("inactive"));
    }
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
        traced
            .iter()
            .any(|l| l.contains("stats_onesample: refused")),
        "the new commands' own refusals were traced"
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
        for (name, n) in [
            (SEL_NUM, 48.3917f64),
            (UNSEL_NUM, 7123.4567f64),
            (TEST_VALUE, tv),
        ] {
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
        "{:<44} {}",
        "surface",
        SENTINELS.map(|s| format!("{s:<17}")).join(" ")
    );
    for (surface, row) in &grid {
        println!(
            "{surface:<44} {}",
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

    // Not vacuous: each allowed place holds what it is allowed to.
    for (surface, wanted) in [
        ("DatasetView", vec![HDR, HDR2, FILE, TXT]),
        ("one-sample spec", vec![TEST_VALUE]),
        (
            "OneSampleResultView (without its manifest)",
            vec![TEST_VALUE],
        ),
        ("CSV export (one-sample)", vec![TEST_VALUE]),
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
        "paired spec",
        "PairedResultView (without its manifest)",
        "CSV export (paired)",
        "manifest (exported bytes)",
        "manifest (in the result view)",
        "tracing",
        "a refusal",
        "WorkspaceStateView",
        "OneSampleValidationView",
        "OneSampleRunStateView",
    ] {
        assert!(grid.contains_key(surface), "{surface} was collected");
    }
    assert_eq!(errors.len(), 2 + 2 * 9 + 4, "every refusal was collected");
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
