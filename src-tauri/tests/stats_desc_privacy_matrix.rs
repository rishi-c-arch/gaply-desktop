//! **The descriptive summary's privacy matrix (Research OS, Phase 2B), in
//! the pattern of `stats_l1_privacy_matrix.rs`.** Seven sentinels, every
//! surface the commands produce, and where each sentinel may appear.
//!
//! The sentinels:
//! * a header, `HDR_SENTINEL_q81`, of the selected numeric column;
//! * a level label, `LVL_SENTINEL_k44`, of the selected categorical column;
//! * the file name `FILE_SENTINEL_d17.csv`, in the folder `PATH_SENTINEL_w93`;
//! * `7123.4567`, in a numeric column the user does not select;
//! * `61.8129`, in the selected numeric column, at sorted position 6 of 9,
//!   between the median (5) and q3 (7), so no statistic can be it. Checked
//!   against R 4.6.1 (`/usr/local/bin/Rscript`, 17 Oct 2026): mean
//!   52.104288888888888, sd 28.053129774921928, min 10.5, q1 31, median 50,
//!   q3 70.75, max 95.5; none equals it or prints containing it;
//! * `TXT_SENTINEL_m02`, a text cell in a column the user does not select.
//!
//! Allowed: the header and the label in `DatasetView`, in `ResultBundleView`
//! (its manifest apart) and in the CSV export's body; the file's base name in
//! `DatasetView` only; the unselected text cell only as a level label in
//! `DatasetView`'s profile. Forbidden everywhere else: the manifest, in the
//! result view and as exported bytes; every other view; every refusal; every
//! tracing span and event; and every file in the app-data folder, the
//! database's files included.
//!
//! The run happens on a temporary app-data folder with a database file the
//! test creates (`Database::open`, so the schema is migrated). Afterwards the
//! test lists the folder, compares each file's SHA-256 with its state before
//! the commands ran, and scans every byte of every file, for each sentinel as
//! text and for each number also as the 8-byte big-endian double SQLite
//! stores. The ingest's one database write named in Phase 2B's prompt, the
//! import-guard rate, is not made by this path (`stats_desc_commands.rs`'s
//! header), so the files must be unchanged.
//!
//! Alone in its binary on purpose: its tracing subscriber is the global
//! default for the whole process, so spans and events on `spawn_blocking`'s
//! threads and on the run's own worker thread are captured too.

mod desc_support;
mod l1_support;

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::{Arc, Mutex};

use serde_json::{json, Value};

use desc_support::*;
use gaply_core::Database;

const HDR: &str = "HDR_SENTINEL_q81";
const LVL: &str = "LVL_SENTINEL_k44";
const FILE: &str = "FILE_SENTINEL_d17";
const PATH: &str = "PATH_SENTINEL_w93";
const UNSEL_NUM: &str = "7123.4567";
const SEL_NUM: &str = "61.8129";
const TXT: &str = "TXT_SENTINEL_m02";

const SENTINELS: [&str; 7] = [HDR, LVL, FILE, PATH, UNSEL_NUM, SEL_NUM, TXT];

fn table() -> Vec<u8> {
    let headers = [HDR, "group", "note", "aux"];
    let score = [
        "44.1257", "10.5", "95.5", SEL_NUM, "23.25", "50", "82", "31", "70.75",
    ];
    let group = ["a", LVL, "a", LVL, "a", LVL, "a", LVL, "a"];
    let note = ["n1", "n2", TXT, "n4", "n5", "n6", "n7", "n8", "n9"];
    let aux = ["1", "2", "3", "4", UNSEL_NUM, "6", "7", "8", "9"];
    let rows: Vec<Vec<C>> = (0..9)
        .map(|i| vec![C::N(score[i]), C::T(group[i]), C::T(note[i]), C::N(aux[i])])
        .collect();
    delimited(&headers, &rows, ",", "\n")
}

/// Where a sentinel may appear, by surface.
fn allowed(surface: &str, sentinel: &str) -> bool {
    match surface {
        "DatasetView" => [HDR, LVL, FILE, TXT].contains(&sentinel),
        "ResultBundleView (without its manifest)" | "CSV export" => [HDR, LVL].contains(&sentinel),
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

/// Every file under `dir`, by relative name, with its SHA-256.
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

#[test]
fn no_sentinel_reaches_a_surface_it_is_not_allowed_on() {
    let lines = Arc::new(Mutex::new(Vec::<String>::new()));
    tracing::subscriber::set_global_default(l1_support::Capture {
        lines: lines.clone(),
    })
    .expect("the only subscriber");
    tracing::callsite::rebuild_interest_cache();

    let root = TempDir::new("matrix");
    let folder = root.0.join(PATH);
    std::fs::create_dir_all(&folder).unwrap();
    let data = folder.join(format!("{FILE}.csv"));
    std::fs::write(&data, table()).unwrap();
    let app_data = root.0.join("appdata");
    std::fs::create_dir_all(&app_data).unwrap();
    let db = Arc::new(Database::open(&app_data.join("gaply.db")).expect("a migrated database"));
    let (_app, w) = app(&app_data, db.clone(), None);
    let before = file_hashes(&app_data);
    assert!(
        before.keys().any(|k| k == "gaply.db"),
        "the database file exists: {before:?}"
    );

    let mut surfaces: Vec<(String, String)> = Vec::new();
    let mut errors: Vec<Value> = Vec::new();
    let mut push = |name: &str, v: &Value| surfaces.push((name.to_string(), v.to_string()));

    // The flow.
    let o = call(
        &w,
        "stats_desc_open",
        json!({ "path": data.to_string_lossy() }),
    )
    .expect("open");
    push("DatasetView", &o);
    let selected = [("C1", "numeric"), ("C2", "categorical")];
    let v = call(
        &w,
        "stats_desc_validate",
        json!({ "slot": o["slot"], "spec": spec(&o, 1, &selected, &[]) }),
    )
    .unwrap();
    push("ValidationReportView", &v);
    let started = call(
        &w,
        "stats_desc_run",
        json!({ "slot": o["slot"], "spec": spec(&o, 1, &selected, &[]) }),
    )
    .unwrap();
    push("RunStartedView", &started);
    let run = started["runId"].as_u64().unwrap();
    let s = until_terminal(&w, run);
    assert_eq!(s["state"], "succeeded", "{s}");
    push("RunStateView", &s);
    let mut r = call(&w, "stats_desc_result", json!({ "runId": run })).unwrap();
    let manifest = r
        .as_object_mut()
        .unwrap()
        .remove("manifest")
        .expect("a manifest");
    push("ResultBundleView (without its manifest)", &r);
    push("manifest (in the result view)", &manifest);
    let cancel = call(&w, "stats_desc_cancel", json!({ "runId": run })).unwrap();
    push("RunStateView", &cancel);
    let csv = call(
        &w,
        "stats_desc_export",
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
        "stats_desc_export",
        json!({ "runId": run, "format": "manifestJson" }),
    )
    .unwrap();
    let mj_bytes: Vec<u8> = serde_json::from_value(mj["bytes"].clone()).unwrap();
    surfaces.push((
        "manifest (exported bytes)".into(),
        String::from_utf8(mj_bytes).unwrap(),
    ));

    // A refused run, an invalid spec and a warning, as views.
    let bad = call(
        &w,
        "stats_desc_run",
        json!({ "slot": o["slot"], "spec": spec(&o, 2, &[("C3", "numeric")], &[]) }),
    )
    .unwrap();
    let bad_state = until_terminal(&w, bad["runId"].as_u64().unwrap());
    assert_eq!(bad_state["state"], "refused");
    surfaces.push(("RunStateView".into(), bad_state.to_string()));
    let invalid = call(&w, "stats_desc_validate", json!({ "slot": o["slot"], "spec": spec(&o, 3, &[("C3", "numeric"), ("C9", "numeric")], &[]) })).unwrap();
    assert_eq!(invalid["status"], "invalid");
    surfaces.push(("ValidationReportView".into(), invalid.to_string()));

    // Refusals.
    let missing = folder.join(format!("{FILE}_missing.csv"));
    let mut long = format!("{HDR}\n");
    for i in 0..5_001 {
        long.push_str(&format!("{i}\n"));
    }
    let too_long = folder.join(format!("{FILE}_long.csv"));
    std::fs::write(&too_long, long).unwrap();
    for (cmd, body) in [
        (
            "stats_desc_open",
            json!({ "path": missing.to_string_lossy() }),
        ),
        (
            "stats_desc_open",
            json!({ "path": folder.to_string_lossy() }),
        ),
        (
            "stats_desc_open",
            json!({ "path": too_long.to_string_lossy() }),
        ),
        (
            "stats_desc_validate",
            json!({ "slot": 99, "spec": spec(&o, 4, &selected, &[]) }),
        ),
        (
            "stats_desc_run",
            json!({ "slot": 99, "spec": spec(&o, 4, &selected, &[]) }),
        ),
        ("stats_desc_status", json!({ "runId": 99 })),
        ("stats_desc_result", json!({ "runId": bad["runId"] })),
        ("stats_desc_export", json!({ "runId": 99, "format": "csv" })),
    ] {
        errors.push(call(&w, cmd, body).expect_err(cmd));
    }
    for e in &errors {
        surfaces.push(("a refusal".into(), e.to_string()));
    }
    assert_eq!(
        call(&w, "stats_desc_close", json!({ "slot": o["slot"] })).unwrap(),
        json!(true)
    );

    // Tracing: every span and event, from every thread.
    let traced = lines.lock().unwrap().clone();
    assert!(
        traced.iter().filter(|l| l.contains("stats_desc")).count() >= 8,
        "tracing ran: {traced:?}"
    );
    // Lines from the other threads: `open` runs on spawn_blocking, and a run
    // ends on its own worker thread.
    assert!(
        traced
            .iter()
            .any(|l| l.contains("stats_desc: dataset opened")),
        "spawn_blocking's events were captured"
    );
    assert!(
        traced
            .iter()
            .any(|l| l.contains("state=\"succeeded\"") && l.contains("run ended")),
        "the worker's events were captured"
    );
    let mut kinds: BTreeMap<String, usize> = BTreeMap::new();
    for l in &traced {
        *kinds
            .entry(l.split_whitespace().take(2).collect::<Vec<_>>().join(" "))
            .or_default() += 1;
    }
    println!("tracing, by kind: {kinds:?}");
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
                !contains(&bytes, &n.to_be_bytes()),
                "{name} as a double in {}",
                f.display()
            );
        }
    }
    let mut written = Vec::new();
    files_under(&folder, &mut written);
    let mut names: Vec<String> = written
        .iter()
        .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
        .collect();
    names.sort();
    assert_eq!(names, [format!("{FILE}.csv"), format!("{FILE}_long.csv")]);
    assert!(
        db.cache_get("import:seconds_per_page", i64::MAX / 2)
            .unwrap()
            .is_none(),
        "no import-guard rate was written"
    );

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

    // Not vacuous: each allowed place does hold what it is allowed to.
    for (surface, wanted) in [
        ("DatasetView", vec![HDR, LVL, FILE, TXT]),
        ("ResultBundleView (without its manifest)", vec![HDR, LVL]),
        ("CSV export", vec![HDR, LVL]),
    ] {
        for s in wanted {
            assert_eq!(
                grid[surface][SENTINELS.iter().position(|x| *x == s).unwrap()],
                "allowed",
                "{s} should be in {surface}"
            );
        }
    }
    assert!(
        grid.contains_key("manifest (exported bytes)")
            && grid.contains_key("tracing")
            && grid.contains_key("a refusal")
    );
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
