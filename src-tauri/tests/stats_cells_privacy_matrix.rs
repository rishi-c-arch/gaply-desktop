//! **The Data View's privacy matrix (Research OS, Phase 8Q, decision 463; ADR-012
//! Part 10), on `stats_prep_privacy_matrix.rs`'s pattern.** A cell sentinel as
//! text, as a printed number and as an 8-byte double, with the header, file and
//! path sentinels beside them, over every surface the flow has after a full scroll
//! of the table.
//!
//! Allowed: the window's view holds cells, and the `DatasetView` holds the
//! profile's headers, level labels and the file's base name, as it does today.
//! Forbidden: a window holding a header, a file name or a path; every other
//! command's answer; a window wholly out of range; a slot gone's refusal; every
//! tracing span and event from every thread (a global subscriber, so
//! `spawn_blocking`'s are covered), the `stats_desc_cells` span captured with
//! counts only; every file in the app-data folder, which is SHA-256-identical
//! before and after; and every spec, manifest and result of a run made after
//! scrolling.
//!
//! `stats_prep_csv` is not called here: it is the file the user saves, and
//! `stats_prep_privacy_matrix.rs` holds it. Nor are the commands the harness does
//! not register (`prep_support::app_with`'s list is what this can drive).
//!
//! The negative control is in two parts: the window surface must hold each cell
//! sentinel (the first assertions on the grid below), so the search can find what
//! it seeks; and the second test feeds the byte search a buffer that does hold the
//! double.
//!
//! Alone in its binary on purpose: its tracing subscriber is the global default
//! for the whole process.

mod desc_support;
mod l1_support;
mod prep_support;

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::{Arc, Mutex};

use serde_json::{json, Value};

use desc_support::{call, delimited, TempDir, C};
use gaply_core::Database;
use prep_support::{app_with, recipe, seven, start, until_terminal};

const TEXT: &str = "CELL_SENTINEL_t83";
const HDR: &str = "HDR_SENTINEL_k22";
const FILE: &str = "FILE_SENTINEL_m51";
const PATH: &str = "PATH_SENTINEL_w09";
/// A file cell's value in one row, which is no statistic of any column or group.
const NUMBER: &str = "61.8125";
/// The score in that row, the mean of 61.8125 and 85.25.
const SCORE: &str = "73.53125";
const DOUBLE: f64 = 61.8125;

const SENTINELS: [&str; 6] = [TEXT, HDR, FILE, PATH, NUMBER, SCORE];

fn table() -> Vec<u8> {
    let headers = [HDR, "q2", "q3", "q4", "arm", "site", "age", "note"];
    let q1 = [
        "44.5", "10.5", "23.25", NUMBER, "50", "95.5", "27.75", "31", "70.75", "12", "66", "18.5",
    ];
    let q2 = [
        "3.5", "11.25", "2", "85.25", "16", "99", "8.5", "4", "99", "1.5", "6", "2.25",
    ];
    let q3 = ["1", "2", "3", "4", "5", "1", "2", "3", "4", "5", "1", "2"];
    let q4 = ["2", "1", "4", "3", "5", "2", "2", "5", "1", "4", "3", "1"];
    let arm = ["a", "a", "a", "a", "a", "a", "b", "b", "b", "b", "b", "b"];
    let site = ["x", "x", "y", "y", "z", "z", "x", "x", "y", "y", "z", "z"];
    let age = [
        "31", "44", "29", "52", "37", "41", "63", "35", "48", "27", "58", "33",
    ];
    let note = [
        "n1", "n2", TEXT, "n4", "n5", "n6", "n7", "n8", "n9", "n10", "n11", "n12",
    ];
    let rows: Vec<Vec<C>> = (0..12)
        .map(|i| {
            vec![
                C::N(q1[i]),
                C::N(q2[i]),
                C::N(q3[i]),
                C::N(q4[i]),
                C::T(arm[i]),
                C::T(site[i]),
                C::N(age[i]),
                C::T(note[i]),
            ]
        })
        .collect();
    delimited(&headers, &rows, ",", "\n")
}

/// Where a sentinel may appear, by surface.
fn allowed(surface: &str, sentinel: &str) -> bool {
    match surface {
        "the window" => [TEXT, NUMBER, SCORE].contains(&sentinel),
        // The profile's level labels, as today, and the file's own names.
        "DatasetView" => [TEXT, HDR, FILE, NUMBER, SCORE].contains(&sentinel),
        _ => false,
    }
}

fn holds_bytes(haystack: &[u8], needle: &[u8]) -> bool {
    haystack.windows(needle.len()).any(|w| w == needle)
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
            (
                f.strip_prefix(dir).unwrap().to_string_lossy().into_owned(),
                gaply_core::contracts::ContentHash::of_bytes(&std::fs::read(f).unwrap())
                    .as_hex()
                    .to_string(),
            )
        })
        .collect()
}

fn bytes_of(export: &Value) -> Vec<u8> {
    serde_json::from_value(export["bytes"].clone()).unwrap()
}

#[test]
fn no_cell_reaches_a_surface_it_is_not_allowed_on() {
    let lines = Arc::new(Mutex::new(Vec::<String>::new()));
    tracing::subscriber::set_global_default(l1_support::Capture {
        lines: lines.clone(),
    })
    .expect("the only subscriber");
    tracing::callsite::rebuild_interest_cache();

    let root = TempDir::new("cells-matrix");
    let folder = root.0.join(PATH);
    std::fs::create_dir_all(&folder).unwrap();
    let data = folder.join(format!("{FILE}.csv"));
    std::fs::write(&data, table()).unwrap();
    let app_data = root.0.join("appdata");
    std::fs::create_dir_all(&app_data).unwrap();
    let db = Arc::new(Database::open(&app_data.join("gaply.db")).expect("a migrated database"));
    let factory: app_lib::workspace::RunsFactory =
        Arc::new(|| Arc::new(app_lib::runs::Runs::default()));
    let (_app, w) = app_with(&app_data, db, factory);
    call(&w, "workspace_activate", json!({ "id": "statistics" })).expect("activates");
    let before = file_hashes(&app_data);
    assert!(before.keys().any(|k| k == "gaply.db"), "{before:?}");

    let mut surfaces: Vec<(String, Vec<u8>)> = Vec::new();
    let mut push = |name: &str, bytes: Vec<u8>| surfaces.push((name.to_string(), bytes));
    let json_of = |v: &Value| v.to_string().into_bytes();

    let window = |slot: &Value, first: (u64, u64), count: (u64, u64)| {
        call(
            &w,
            "stats_desc_cells",
            json!({
                "slot": slot,
                "firstRow": first.0,
                "rowCount": count.0,
                "firstColumn": first.1,
                "columnCount": count.1,
            }),
        )
    };

    let o = call(
        &w,
        "stats_desc_open",
        json!({ "path": data.to_string_lossy() }),
    )
    .expect("open");
    push("DatasetView", json_of(&o));
    let slot = o["slot"].clone();

    // A prepared table, so the window holds a file's cell and a score's.
    let mean = recipe(8, "total", "mean", &["C1", "C2"]);
    let made = call(
        &w,
        "stats_prep_make",
        json!({ "slot": slot, "recipe": mean }),
    )
    .unwrap();
    assert_eq!(made["status"], "valid", "{made}");
    let view = made["dataset"].clone();
    push("DatasetView", json_of(&view));
    let mut report = made.clone();
    report.as_object_mut().unwrap().remove("dataset");
    push("PrepMadeView (without its dataset)", json_of(&report));
    let read = call(&w, "stats_prep_recipe", json!({ "slot": slot })).unwrap();
    push("the recipe", json_of(&read));

    // Scroll the whole table: every window of every shape a screen would ask.
    for first_row in 0..12 {
        for first_column in 0..9 {
            let got = window(&slot, (first_row, first_column), (3, 4)).expect("a window");
            push("the window", json_of(&got));
        }
    }
    let whole = window(&slot, (0, 0), (200, 64)).expect("the whole table");
    assert_eq!(whole["cells"][3][0], NUMBER, "{whole}");
    assert_eq!(whole["cells"][2][7], TEXT, "{whole}");
    assert_eq!(whole["cells"][3][8], SCORE, "{whole}");
    push("the window", json_of(&whole));

    // Wholly out of range: clamped and empty.
    for (first, count) in [
        ((12, 0), (5, 5)),
        ((0, 9), (5, 5)),
        ((1000, 1000), (5, 5)),
        ((u64::MAX, u64::MAX), (u64::MAX, u64::MAX)),
        ((0, 0), (0, 0)),
    ] {
        let got = window(&slot, first, count).expect("no refusal");
        assert_eq!(got["cells"], json!([]), "{got}");
        push("a window out of range", json_of(&got));
    }

    // Every run after scrolling: its spec, manifest, result and exports.
    for m in &seven(&view, "C9") {
        push("spec", json_of(&m.spec));
        let v = call(
            &w,
            &format!("{}_validate", m.prefix),
            json!({ "slot": slot, "spec": m.spec }),
        )
        .unwrap();
        push("a method's validation view", json_of(&v));
        let run = start(&w, &slot, m).unwrap_or_else(|e| panic!("{}: {e}", m.prefix));
        let s = until_terminal(&w, m.prefix, run);
        assert_eq!(s["state"], "succeeded", "{}: {s}", m.prefix);
        push("a method's run state", json_of(&s));
        let mut r = call(&w, &format!("{}_result", m.prefix), json!({ "runId": run })).unwrap();
        let manifest = r
            .as_object_mut()
            .unwrap()
            .remove("manifest")
            .expect("a manifest");
        push("a method's result (without its manifest)", json_of(&r));
        push("manifest (in the result view)", json_of(&manifest));
        let e = call(
            &w,
            &format!("{}_export", m.prefix),
            json!({ "runId": run, "format": "csv" }),
        )
        .unwrap();
        push("a method's result CSV", bytes_of(&e));
        let mj = call(
            &w,
            &format!("{}_export", m.prefix),
            json!({ "runId": run, "format": "manifestJson" }),
        )
        .unwrap();
        push("manifest (exported bytes)", bytes_of(&mj));
    }

    // The other commands a screen calls beside the grid.
    let levels = call(
        &w,
        "stats_ttest_group_levels",
        json!({ "slot": slot, "column": "C5" }),
    )
    .unwrap();
    push("another command's answer", json_of(&levels));
    let state = call(&w, "workspace_state", json!({})).unwrap();
    push("another command's answer", json_of(&state));

    // Refusals: a slot never held, and the slot gone.
    let unknown = window(&json!(99), (0, 0), (5, 5)).expect_err("no such slot");
    assert_eq!(desc_support::refusal(&unknown)["code"], "run.unknown_input");
    push("a refusal", json_of(&unknown));
    let closed = call(&w, "stats_desc_close", json!({ "slot": slot })).unwrap();
    push("another command's answer", json_of(&closed));
    let gone = window(&slot, (0, 0), (5, 5)).expect_err("the slot is gone");
    assert_eq!(desc_support::refusal(&gone)["code"], "run.unknown_input");
    push("a refusal", json_of(&gone));

    // Tracing, from every thread. The span and its counts were captured.
    let traced = lines.lock().unwrap().clone();
    assert!(!traced.is_empty(), "the subscriber captured nothing");
    assert!(
        traced.iter().any(|l| l.contains("span stats_desc_cells")),
        "the command's span was captured"
    );
    let reads: Vec<&String> = traced
        .iter()
        .filter(|l| l.contains("window read"))
        .collect();
    assert!(reads.len() >= 100, "{} window reads traced", reads.len());
    for l in &reads {
        assert!(
            l.contains("rows=") && l.contains("columns=") && l.contains("cells="),
            "counts only: {l}"
        );
    }
    for l in &traced {
        push("tracing", l.clone().into_bytes());
    }

    // Files: the app-data folder is unchanged, byte for byte.
    assert_eq!(
        before,
        file_hashes(&app_data),
        "a file in the app-data folder was written or created"
    );
    let mut files = Vec::new();
    files_under(&app_data, &mut files);
    for f in &files {
        push(
            &format!("file {}", f.strip_prefix(&app_data).unwrap().display()),
            std::fs::read(f).unwrap(),
        );
    }
    let mut written = Vec::new();
    files_under(&folder, &mut written);
    assert_eq!(written.len(), 1, "only the test's own file: {written:?}");

    // The matrix.
    let double = DOUBLE.to_le_bytes();
    let mut grid: BTreeMap<String, Vec<&str>> = BTreeMap::new();
    let mut leaks = Vec::new();
    for (surface, bytes) in &surfaces {
        let row = grid
            .entry(surface.clone())
            .or_insert_with(|| vec!["-"; SENTINELS.len()]);
        for (i, s) in SENTINELS.iter().enumerate() {
            if holds_bytes(bytes, s.as_bytes()) {
                if allowed(surface, s) {
                    row[i] = "allowed";
                } else {
                    row[i] = "LEAK";
                    let text = String::from_utf8_lossy(bytes);
                    let at = text.find(s).unwrap_or(0);
                    let from = text.floor_char_boundary(at.saturating_sub(60));
                    let to = text.floor_char_boundary((at + s.len() + 20).min(text.len()));
                    leaks.push(format!("{s} in {surface}: …{}…", &text[from..to]));
                }
            }
        }
        if holds_bytes(bytes, &double) {
            leaks.push(format!("the 8-byte double in {surface}"));
        }
    }
    for (surface, row) in &grid {
        println!(
            "{surface:<44} {}",
            row.iter()
                .map(|c| format!("{c:<8}"))
                .collect::<Vec<_>>()
                .join(" ")
        );
    }
    println!(
        "surfaces: {}; tracing lines: {}; files: {}",
        surfaces.len(),
        traced.len(),
        files.len()
    );
    assert!(
        leaks.is_empty(),
        "sentinels where they may not be:\n{}",
        leaks.join("\n")
    );

    // The negative control, and every forbidden surface collected.
    let at =
        |surface: &str, s: &str| grid[surface][SENTINELS.iter().position(|x| *x == s).unwrap()];
    assert_eq!(at("the window", TEXT), "allowed");
    assert_eq!(at("the window", NUMBER), "allowed");
    assert_eq!(at("the window", SCORE), "allowed");
    assert_eq!(
        at("DatasetView", TEXT),
        "allowed",
        "a level label, as today"
    );
    assert_eq!(at("DatasetView", HDR), "allowed");
    assert_eq!(at("DatasetView", FILE), "allowed");
    for surface in [
        "spec",
        "a method's result (without its manifest)",
        "a method's result CSV",
        "manifest (in the result view)",
        "manifest (exported bytes)",
        "a window out of range",
        "a refusal",
        "another command's answer",
        "tracing",
        "file gaply.db",
    ] {
        assert!(grid.contains_key(surface), "{surface} was collected");
    }
}

/// The negative control for the third form: the search finds the double where it
/// is, and the text forms' search finds the text where it is.
#[test]
fn the_search_finds_each_form_where_it_is() {
    let double = DOUBLE.to_le_bytes();
    let mut buffer = b"before ".to_vec();
    buffer.extend_from_slice(&double);
    buffer.extend_from_slice(b" after");
    assert!(holds_bytes(&buffer, &double));
    assert!(!holds_bytes(b"before after", &double));
    assert!(holds_bytes(TEXT.as_bytes(), TEXT.as_bytes()));
    let printed = format!("{DOUBLE}");
    assert_eq!(printed, NUMBER, "the printed form is the double's own");
}
