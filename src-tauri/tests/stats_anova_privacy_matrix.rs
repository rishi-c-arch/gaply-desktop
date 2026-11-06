//! **The one-way ANOVA's privacy matrix (Research OS, Phase 8G-2, decision
//! 318), in the pattern of `stats_ttest_privacy_matrix.rs`.** Seven
//! sentinels, every surface the ANOVA's commands produce, and where each
//! sentinel may appear.
//!
//! The sentinels:
//! * a header, `HDR_SENTINEL_v51`, of the selected outcome;
//! * a level label, `LVL_SENTINEL_m28`, of the selected group;
//! * the file name `FILE_SENTINEL_k63.csv`, in the folder `PATH_SENTINEL_z04`;
//! * `61.8129`, an outcome value of the label's group that is neither its
//!   smallest nor its largest (10.5 and 95.5 are), so no number the ANOVA
//!   reports is it: the group table's min and max are the only values it
//!   copies, and the matrix itself is the check that nothing else equals it;
//! * `7123.4567`, in a numeric column the user does not select;
//! * `TXT_SENTINEL_e17`, a text cell in a column the user does not select.
//!
//! Allowed: the header and the label in `DatasetView`'s profile; the label in
//! the result view (its manifest apart), in the CSV export's body and in
//! `stats_anova_group_levels`' view; the file's base name in `DatasetView`
//! only; the unselected text cell only as a level label in `DatasetView`'s
//! profile. Forbidden everywhere else: the spec, the manifest (in the view
//! and as exported bytes), every other view, every refusal, every tracing
//! span and event (a global subscriber, so the worker thread and
//! `spawn_blocking`'s are covered), and every file in the app-data folder,
//! the database's included, which is SHA-256-identical before and after.
//!
//! Alone in its binary on purpose: its tracing subscriber is the global
//! default for the whole process.

mod anova_support;
mod desc_support;
mod l1_support;

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::{Arc, Mutex};

use serde_json::{json, Value};

use anova_support::{app, aspec, until_terminal_anova};
use desc_support::{call, delimited, spec as desc_spec, until_terminal, TempDir, C};
use gaply_core::Database;

const HDR: &str = "HDR_SENTINEL_v51";
const LVL: &str = "LVL_SENTINEL_m28";
const FILE: &str = "FILE_SENTINEL_k63";
const PATH: &str = "PATH_SENTINEL_z04";
const SEL_NUM: &str = "61.8129";
const UNSEL_NUM: &str = "7123.4567";
const TXT: &str = "TXT_SENTINEL_e17";

const SENTINELS: [&str; 7] = [HDR, LVL, FILE, PATH, SEL_NUM, UNSEL_NUM, TXT];

fn table() -> Vec<u8> {
    let headers = [HDR, "arm", "note", "aux"];
    // Three groups: a, b and the label. The label's group is 10.5, 61.8129,
    // 95.5 and 31, so the sentinel is neither its min nor its max.
    let y = [
        "44.1257", "10.5", "23.25", SEL_NUM, "50", "95.5", "82", "31", "70.75", "12", "66", "18.5",
    ];
    let arm = ["a", LVL, "b", LVL, "a", LVL, "b", LVL, "a", "b", "a", "b"];
    let note = [
        "n1", "n2", TXT, "n4", "n5", "n6", "n7", "n8", "n9", "n10", "n11", "n12",
    ];
    let aux = [
        "1", "2", "3", "4", UNSEL_NUM, "6", "7", "8", "9", "10", "11", "12",
    ];
    let rows: Vec<Vec<C>> = (0..12)
        .map(|i| vec![C::N(y[i]), C::T(arm[i]), C::T(note[i]), C::N(aux[i])])
        .collect();
    delimited(&headers, &rows, ",", "\n")
}

/// Where a sentinel may appear, by surface.
fn allowed(surface: &str, sentinel: &str) -> bool {
    match surface {
        "DatasetView" => [HDR, LVL, FILE, TXT].contains(&sentinel),
        // The follow-up's view (decision 81): the group's labels, there only.
        "GroupLevelsView" => [LVL].contains(&sentinel),
        "AnovaResultView (without its manifest)" | "CSV export" => [HDR, LVL].contains(&sentinel),
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

    let root = TempDir::new("anova-matrix");
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
    let g = call(
        &w,
        "stats_anova_group_levels",
        json!({ "slot": o["slot"], "column": "C2" }),
    )
    .expect("the group's levels");
    // Code-point order: "LVL_…" (0x4c) before "a" and "b".
    assert_eq!(g["levels"][0]["label"], LVL);
    push(&mut surfaces, "GroupLevelsView", &g);
    let spec = aspec(
        &o,
        1,
        "C1",
        "C2",
        None,
        &["tukey", "games_howell", "bonferroni"],
        &[],
    );
    push(&mut surfaces, "spec", &spec);
    let v = call(
        &w,
        "stats_anova_validate",
        json!({ "slot": o["slot"], "spec": spec }),
    )
    .unwrap();
    assert_eq!(v["status"], "valid", "{v}");
    push(&mut surfaces, "AnovaValidationView", &v);
    let started = call(
        &w,
        "stats_anova_run",
        json!({ "slot": o["slot"], "spec": spec }),
    )
    .unwrap();
    push(&mut surfaces, "RunStartedView", &started);
    let run = started["runId"].as_u64().unwrap();
    let s = until_terminal_anova(&w, run);
    assert_eq!(s["state"], "succeeded", "{s}");
    push(&mut surfaces, "AnovaRunStateView", &s);
    push(
        &mut surfaces,
        "WorkspaceStateView",
        &call(&w, "workspace_state", json!({})).unwrap(),
    );
    let mut r = call(&w, "stats_anova_result", json!({ "runId": run })).unwrap();
    let manifest = r
        .as_object_mut()
        .unwrap()
        .remove("manifest")
        .expect("a manifest");
    push(&mut surfaces, "AnovaResultView (without its manifest)", &r);
    push(&mut surfaces, "manifest (in the result view)", &manifest);
    push(
        &mut surfaces,
        "AnovaRunStateView",
        &call(&w, "stats_anova_cancel", json!({ "runId": run })).unwrap(),
    );
    let csv = call(
        &w,
        "stats_anova_export",
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
        "stats_anova_export",
        json!({ "runId": run, "format": "manifestJson" }),
    )
    .unwrap();
    let mj_bytes: Vec<u8> = serde_json::from_value(mj["bytes"].clone()).unwrap();
    surfaces.push((
        "manifest (exported bytes)".into(),
        String::from_utf8(mj_bytes).unwrap(),
    ));

    // Remove in the Output (decision 486): the forget's answer, a second forget's
    // refusal, and below its span, on a run of its own so the rest is untouched.
    let forgotten = call(
        &w,
        "stats_anova_run",
        json!({ "slot": o["slot"], "spec": spec }),
    )
    .unwrap()["runId"]
        .as_u64()
        .unwrap();
    assert_eq!(until_terminal_anova(&w, forgotten)["state"], "succeeded");
    push(
        &mut surfaces,
        "the forget's answer",
        &call(&w, "stats_desc_forget_run", json!({ "runId": forgotten })).unwrap(),
    );
    errors.push(
        call(&w, "stats_desc_forget_run", json!({ "runId": forgotten })).expect_err("forgotten"),
    );

    // Refused runs, as views: an outcome that holds text, a level left out
    // so one level remains, and a validation with several errors.
    let bad = call(
        &w,
        "stats_anova_run",
        json!({ "slot": o["slot"], "spec": aspec(&o, 2, "C3", "C2", None, &[], &[]) }),
    )
    .unwrap();
    let bad_state = until_terminal_anova(&w, bad["runId"].as_u64().unwrap());
    assert_eq!(bad_state["state"], "refused");
    push(&mut surfaces, "AnovaRunStateView", &bad_state);
    let few = call(
        &w,
        "stats_anova_run",
        json!({ "slot": o["slot"], "spec": aspec(&o, 3, "C1", "C2", Some(&[0]), &["tukey"], &[]) }),
    )
    .unwrap();
    let few_state = until_terminal_anova(&w, few["runId"].as_u64().unwrap());
    assert_eq!(few_state["state"], "refused");
    push(&mut surfaces, "AnovaRunStateView", &few_state);
    let invalid = call(
        &w,
        "stats_anova_validate",
        json!({ "slot": o["slot"], "spec": aspec(&o, 4, "C3", "C9", Some(&[0, 5]), &["tukey"], &[99.0]) }),
    )
    .unwrap();
    assert_eq!(invalid["status"], "invalid");
    push(&mut surfaces, "AnovaValidationView", &invalid);

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
        ("stats_anova_validate", json!({ "slot": 99, "spec": spec })),
        ("stats_anova_run", json!({ "slot": 99, "spec": spec })),
        (
            "stats_anova_validate",
            json!({ "slot": o["slot"], "spec": unknown }),
        ),
        (
            "stats_anova_run",
            json!({ "slot": o["slot"], "spec": stale }),
        ),
        ("stats_anova_status", json!({ "runId": 99 })),
        ("stats_anova_result", json!({ "runId": bad["runId"] })),
        ("stats_anova_status", json!({ "runId": d })),
        ("stats_desc_result", json!({ "runId": run })),
        (
            "stats_anova_export",
            json!({ "runId": 99, "format": "csv" }),
        ),
        (
            "stats_anova_group_levels",
            json!({ "slot": o["slot"], "column": "C9" }),
        ),
        (
            "stats_anova_group_levels",
            json!({ "slot": 99, "column": "C2" }),
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
    errors.push(call(&w, "stats_anova_status", json!({ "runId": run })).expect_err("inactive"));
    errors.push(
        call(
            &w,
            "stats_anova_group_levels",
            json!({ "slot": o["slot"], "column": "C2" }),
        )
        .expect_err("inactive"),
    );
    errors.push(
        call(
            &w,
            "stats_anova_validate",
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
            .any(|l| l.contains("span stats_desc_forget_run")),
        "the forget's span was captured"
    );
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
        traced.iter().any(|l| l.contains("stats_anova: refused")),
        "the ANOVA's own refusals were traced"
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
        ("GroupLevelsView", vec![LVL]),
        ("AnovaResultView (without its manifest)", vec![LVL]),
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
