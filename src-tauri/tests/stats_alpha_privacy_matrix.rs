//! **Cronbach's alpha's privacy matrix (Research OS, Phase 8I-2, decision
//! 382), in the pattern of `stats_corr_privacy_matrix.rs`.** Seven
//! sentinels, every surface alpha's commands produce, and where each
//! sentinel may appear.
//!
//! The sentinels:
//! * two headers, `HDR_SENTINEL_a71` and `HDR_SENTINEL_b29`, of the two
//!   selected items;
//! * the file name `FILE_SENTINEL_c64.csv`, in the folder `PATH_SENTINEL_d05`;
//! * `61.8129`, a value of the first selected item, which is never
//!   reverse-scored. The result holds aliases, codes and numbers only, so a
//!   selected value could reach it only as a statistic: the item's other
//!   values make its mean, its SD and every other statistic neither, the
//!   second item's reversed mean included (it is on a scale of 0 to 10), and
//!   the matrix itself is the check that nothing else equals it (or holds it
//!   as a substring);
//! * `7123.4567`, in a numeric column the user does not select;
//! * `TXT_SENTINEL_e83`, a text cell in a column the user does not select.
//!
//! Allowed: the headers, the file's base name and the unselected text cell
//! (as a level label of its column's profile) in `DatasetView` only.
//! Forbidden everywhere else, the result view and the CSV export included:
//! alpha names columns by alias, so no header reaches them (decision 379).
//! Also forbidden: the spec, the manifest (in the view and as exported
//! bytes), every other view, every refusal, every tracing span and event (a
//! global subscriber, so the worker thread and `spawn_blocking`'s are
//! covered), and every file in the app-data folder, the database's included,
//! which is SHA-256-identical before and after.
//!
//! The negative control (decision 337d's lesson) is the second test: on a
//! table of its own, an item, not reversed, constant at the sentinel over the
//! rows used, so its mean IS the sentinel, and `found()` finds it. With the
//! matrix's own pass, this shows that the placement of 61.8129, not anything
//! else, keeps it out of the matrix's result.
//!
//! Alone in its binary on purpose: its tracing subscriber is the global
//! default for the whole process.

mod alpha_support;
mod desc_support;
mod l1_support;

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::{Arc, Mutex};

use serde_json::{json, Value};

use alpha_support::{app, aspec, until_terminal_alpha};
use desc_support::{call, delimited, spec as desc_spec, until_terminal, TempDir, C};
use gaply_core::Database;

const HDR: &str = "HDR_SENTINEL_a71";
const HDR2: &str = "HDR_SENTINEL_b29";
const FILE: &str = "FILE_SENTINEL_c64";
const PATH: &str = "PATH_SENTINEL_d05";
const SEL_NUM: &str = "61.8129";
const UNSEL_NUM: &str = "7123.4567";
const TXT: &str = "TXT_SENTINEL_e83";

const SENTINELS: [&str; 7] = [HDR, HDR2, FILE, PATH, SEL_NUM, UNSEL_NUM, TXT];

fn table() -> Vec<u8> {
    let headers = [HDR, HDR2, "note", "aux"];
    let x = [
        "44.1257", "10.5", "23.25", SEL_NUM, "50", "95.5", "", "31", "70.75", "12", "66", "18.5",
    ];
    let y = [
        "3.5", "1.25", "2", "6.75", "", "9", "8.5", "4", "7", "1.5", "6", "2.25",
    ];
    let note = [
        "n1", "n2", TXT, "n4", "n5", "n6", "n7", "n8", "n9", "n10", "n11", "n12",
    ];
    let aux = [
        "1", "2", "3", "4", UNSEL_NUM, "6", "7", "8", "9", "10", "11", "12",
    ];
    let cell = |s: &'static str| if s.is_empty() { C::Empty } else { C::N(s) };
    let rows: Vec<Vec<C>> = (0..12)
        .map(|i| vec![cell(x[i]), cell(y[i]), C::T(note[i]), C::N(aux[i])])
        .collect();
    delimited(&headers, &rows, ",", "\n")
}

/// Where a sentinel may appear, by surface.
fn allowed(surface: &str, sentinel: &str) -> bool {
    match surface {
        "DatasetView" => [HDR, HDR2, FILE, TXT].contains(&sentinel),
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

/// Every sentinel `bytes` holds, as text, and each number also as the
/// 8-byte double, either order.
fn found(bytes: &[u8]) -> Vec<&'static str> {
    let mut out: Vec<&'static str> = SENTINELS
        .iter()
        .copied()
        .filter(|s| contains(bytes, s.as_bytes()))
        .collect();
    for (name, n) in [(SEL_NUM, 61.8129f64), (UNSEL_NUM, 7123.4567f64)] {
        if (contains(bytes, &n.to_be_bytes()) || contains(bytes, &n.to_le_bytes()))
            && !out.contains(&name)
        {
            out.push(name);
        }
    }
    out
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

    let root = TempDir::new("alpha-matrix");
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

    // The flow, as the screen drives it: the second item reverse-scored on
    // 0 to 10.
    let o = call(
        &w,
        "stats_desc_open",
        json!({ "path": data.to_string_lossy() }),
    )
    .expect("open");
    push(&mut surfaces, "DatasetView", &o);
    let spec = aspec(&o, 1, &["C1", "C2"], &["C2"], Some((0.0, 10.0)), &[]);
    push(&mut surfaces, "spec", &spec);
    let v = call(
        &w,
        "stats_alpha_validate",
        json!({ "slot": o["slot"], "spec": spec }),
    )
    .unwrap();
    assert_eq!(v["status"], "valid", "{v}");
    push(&mut surfaces, "AlphaValidationView", &v);
    let started = call(
        &w,
        "stats_alpha_run",
        json!({ "slot": o["slot"], "spec": spec }),
    )
    .unwrap();
    push(&mut surfaces, "RunStartedView", &started);
    let run = started["runId"].as_u64().unwrap();
    let s = until_terminal_alpha(&w, run);
    assert_eq!(s["state"], "succeeded", "{s}");
    push(&mut surfaces, "AlphaRunStateView", &s);
    push(
        &mut surfaces,
        "WorkspaceStateView",
        &call(&w, "workspace_state", json!({})).unwrap(),
    );
    let mut r = call(&w, "stats_alpha_result", json!({ "runId": run })).unwrap();
    // Positive control: the result is about the user's columns, by alias.
    assert_eq!(r["result"]["items"][0]["item"], "C1", "{r}");
    assert_eq!(r["result"]["items"][1]["reversed"], true, "{r}");
    let manifest = r
        .as_object_mut()
        .unwrap()
        .remove("manifest")
        .expect("a manifest");
    push(&mut surfaces, "AlphaResultView (without its manifest)", &r);
    push(&mut surfaces, "manifest (in the result view)", &manifest);
    push(
        &mut surfaces,
        "AlphaRunStateView",
        &call(&w, "stats_alpha_cancel", json!({ "runId": run })).unwrap(),
    );
    let csv = call(
        &w,
        "stats_alpha_export",
        json!({ "runId": run, "format": "csv" }),
    )
    .unwrap();
    let csv_bytes: Vec<u8> = serde_json::from_value(csv["bytes"].clone()).unwrap();
    let csv_text = String::from_utf8(csv_bytes).unwrap();
    assert!(csv_text.contains("\"C1\"") && csv_text.contains("\"C2\""));
    surfaces.push(("CSV export".into(), csv_text));
    surfaces.push((
        "ExportView (CSV), its other fields".into(),
        csv["sha256"].to_string(),
    ));
    let mj = call(
        &w,
        "stats_alpha_export",
        json!({ "runId": run, "format": "manifestJson" }),
    )
    .unwrap();
    let mj_bytes: Vec<u8> = serde_json::from_value(mj["bytes"].clone()).unwrap();
    surfaces.push((
        "manifest (exported bytes)".into(),
        String::from_utf8(mj_bytes).unwrap(),
    ));
    // Nothing reversed too: other numbers, same rule.
    let plain = call(
        &w,
        "stats_alpha_run",
        json!({ "slot": o["slot"], "spec": aspec(&o, 2, &["C1", "C2"], &[], None, &[]) }),
    )
    .unwrap();
    let plain_run = plain["runId"].as_u64().unwrap();
    assert_eq!(until_terminal_alpha(&w, plain_run)["state"], "succeeded");
    push(
        &mut surfaces,
        "AlphaResultView (nothing reversed)",
        &call(&w, "stats_alpha_result", json!({ "runId": plain_run })).unwrap(),
    );

    // Remove in the Output (decision 468): the forget's answer, its refusal,
    // and below its span. The record's sentinels go nowhere new.
    push(
        &mut surfaces,
        "the forget's answer",
        &call(&w, "stats_desc_forget_run", json!({ "runId": plain_run })).unwrap(),
    );
    errors.push(
        call(&w, "stats_desc_forget_run", json!({ "runId": plain_run })).expect_err("forgotten"),
    );

    // Refused runs, as views: an item that holds text, and a validation with
    // several errors.
    let bad = call(
        &w,
        "stats_alpha_run",
        json!({ "slot": o["slot"], "spec": aspec(&o, 3, &["C1", "C3"], &[], None, &[]) }),
    )
    .unwrap();
    let bad_state = until_terminal_alpha(&w, bad["runId"].as_u64().unwrap());
    assert_eq!(bad_state["state"], "refused");
    push(&mut surfaces, "AlphaRunStateView", &bad_state);
    let invalid = call(
        &w,
        "stats_alpha_validate",
        json!({ "slot": o["slot"], "spec": aspec(&o, 4, &["C3", "C9"], &["C3"], Some((1.0, 5.0)), &[99.0, 61.8]) }),
    )
    .unwrap();
    assert_eq!(invalid["status"], "invalid");
    push(&mut surfaces, "AlphaValidationView", &invalid);
    // A reversed value outside the scale: refused with the column and count.
    let outside = call(
        &w,
        "stats_alpha_validate",
        json!({ "slot": o["slot"], "spec": aspec(&o, 5, &["C1", "C2"], &["C1"], Some((0.0, 10.0)), &[]) }),
    )
    .unwrap();
    assert_eq!(
        outside["errors"][0]["code"],
        "stats.alpha.reversed_outside_scale"
    );
    push(&mut surfaces, "AlphaValidationView", &outside);

    // Refusals.
    let mut unknown = spec.clone();
    unknown["capabilityId"] = json!("stats.analyze.nonexistent");
    let mut stale = spec.clone();
    stale["capabilityVersion"] = json!(9);
    let missing = folder.join(format!("{FILE}_missing.csv"));
    let d = call(
        &w,
        "stats_desc_run",
        json!({ "slot": o["slot"], "spec": desc_spec(&o, 6, &[("C1", "numeric")], &[]) }),
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
        ("stats_alpha_validate", json!({ "slot": 99, "spec": spec })),
        ("stats_alpha_run", json!({ "slot": 99, "spec": spec })),
        (
            "stats_alpha_validate",
            json!({ "slot": o["slot"], "spec": unknown }),
        ),
        (
            "stats_alpha_run",
            json!({ "slot": o["slot"], "spec": stale }),
        ),
        ("stats_alpha_status", json!({ "runId": 99 })),
        ("stats_alpha_result", json!({ "runId": bad["runId"] })),
        ("stats_alpha_status", json!({ "runId": d })),
        ("stats_desc_result", json!({ "runId": run })),
        (
            "stats_alpha_export",
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
    errors.push(call(&w, "stats_alpha_status", json!({ "runId": run })).expect_err("inactive"));
    errors.push(
        call(
            &w,
            "stats_alpha_validate",
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
        traced.iter().any(|l| l.contains("stats_alpha: refused")),
        "alpha's own refusals were traced"
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
        assert!(
            found(&bytes).is_empty(),
            "{:?} in {}",
            found(&bytes),
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

    // Not vacuous: the dataset view holds what it is allowed to, and every
    // surface the rule forbids was collected.
    for s in [HDR, HDR2, FILE, TXT] {
        assert_eq!(
            grid["DatasetView"][SENTINELS.iter().position(|x| *x == s).unwrap()],
            "allowed",
            "{s} should be in DatasetView"
        );
    }
    for surface in [
        "spec",
        "AlphaResultView (without its manifest)",
        "CSV export",
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
}

/// The negative control (decision 337d's lesson): an item, not reversed,
/// constant at the sentinel over the rows used, so its mean is the sentinel
/// exactly, and the result holds it. It calls `stats_alpha::compute`
/// directly, with no command and no tracing of its own: the test above owns
/// the process's subscriber, and `build_dataset` logs only its two counts.
#[test]
fn the_result_holds_the_value_when_an_item_is_constant_at_it() {
    use gaply_core::stats_alpha::{compute, AlphaParams};
    use gaply_core::stats_desc::Alias;
    use gaply_core::stats_l1::dataset::{build_dataset, DataFingerprint, RawTable, TableRef};
    // The first item is 61.8129 in every row used; the sixth row, where it
    // is 3, is left out by the second item's empty cell.
    let x = [SEL_NUM, SEL_NUM, SEL_NUM, SEL_NUM, SEL_NUM, "3"];
    let y = ["3.5", "1.25", "2", "6.75", "9", ""];
    let z = ["7", "4", "5.5", "1", "2.25", "6"];
    let rows: Vec<Vec<String>> = (0..6)
        .map(|i| vec![x[i].to_string(), y[i].to_string(), z[i].to_string()])
        .collect();
    let d = build_dataset(
        RawTable::from_rows(vec![HDR.into(), HDR2.into(), "z".into()], rows),
        DataFingerprint::of_bytes(b"negative control", "test", TableRef::FirstTable),
    )
    .expect("dataset");
    let alias = |i: usize| Alias(d.schema.columns[i].alias);
    let result = compute(
        &d,
        &AlphaParams {
            items: vec![alias(0), alias(1), alias(2)],
            reversed: Vec::new(),
            scale: None,
            missing_codes: Vec::new(),
        },
    )
    .expect("alpha computes");
    assert_eq!(result.rows_used, 5);
    let bytes = serde_json::to_vec(&result).unwrap();
    assert!(
        found(&bytes).contains(&SEL_NUM),
        "61.8129 is its item's only value in the rows used, so the result must hold it as the mean"
    );
    assert!(
        !found(&bytes).contains(&HDR),
        "and still no header: the result names columns by alias"
    );
}
