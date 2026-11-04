//! **`stats_desc_cells`, through IPC (Research OS, Phase 8Q, decision 459;
//! ADR-012 Part 3).** The window of cells the Data View reads: each value a
//! string and a missing cell the one null; clamped, never refused; a slot gone
//! `run.unknown_input`; a make keeping the slot's number and giving a new
//! `inputHash`, which is what lets a screen drop a stale window. It starts no
//! run and writes nothing.

mod desc_support;
mod prep_support;

use std::collections::BTreeMap;
use std::path::Path;

use serde_json::{json, Value};

use desc_support::{call, refusal};
use prep_support::{make, open, recipe, world, World};

fn window(w: &World, slot: &Value, first: (u64, u64), count: (u64, u64)) -> Result<Value, Value> {
    call(
        &w.w,
        "stats_desc_cells",
        json!({
            "slot": slot,
            "firstRow": first.0,
            "rowCount": count.0,
            "firstColumn": first.1,
            "columnCount": count.1,
        }),
    )
}

fn csv(lines: &[&str]) -> Vec<u8> {
    (lines.join("\n") + "\n").into_bytes()
}

#[test]
fn the_window_crosses_as_strings_with_a_missing_cell_the_one_null() {
    let w = world(
        "cells-wire",
        &csv(&[
            "n,t",
            "1.50,alpha",
            "-0,<img src=x onerror=y>",
            "1e300,",
            ",delta",
            "12345678901234567890,1e999",
        ]),
        None,
    );
    let v = open(&w);
    let got = window(&w, &v["slot"], (0, 0), (5, 2)).expect("a window");
    assert_eq!(got["slot"], v["slot"]);
    assert_eq!(got["inputHash"], v["inputRef"]["contentHash"]);
    assert_eq!(
        (got["rows"].clone(), got["columns"].clone()),
        (json!(5), json!(2))
    );
    assert_eq!(
        (got["firstRow"].clone(), got["firstColumn"].clone()),
        (json!(0), json!(0))
    );
    let big = format!("1{}", "0".repeat(300));
    assert_eq!(
        got["cells"],
        json!([
            ["1.5", "alpha"],
            ["-0", "<img src=x onerror=y>"],
            [big, null],
            [null, "delta"],
            ["12345678901234567000", "1e999"],
        ])
    );
}

#[test]
fn a_request_past_the_end_is_clamped_and_empty_and_never_refused() {
    let w = world("cells-past", &csv(&["a,b", "1,2", "3,4"]), None);
    let v = open(&w);
    let past = window(&w, &v["slot"], (2, 0), (5, 2)).expect("no refusal");
    assert_eq!(past["firstRow"], 2);
    assert_eq!(past["cells"], json!([]));
    let wide = window(&w, &v["slot"], (0, 9), (5, 2)).expect("no refusal");
    assert_eq!(wide["firstColumn"], 2);
    assert_eq!(wide["cells"], json!([]));
    let huge = window(&w, &v["slot"], (u64::MAX, u64::MAX), (u64::MAX, u64::MAX)).expect("huge");
    assert_eq!(huge["firstRow"], 2);
    assert_eq!(huge["cells"], json!([]));
    let edge = window(&w, &v["slot"], (1, 1), (9, 9)).expect("the edge");
    assert_eq!(edge["cells"], json!([["4"]]));
}

#[test]
fn the_caps_hold_through_the_command() {
    let header: Vec<String> = (0..70).map(|c| format!("h{c}")).collect();
    let mut lines = vec![header.join(",")];
    for r in 0..205 {
        let row: Vec<String> = (0..70).map(|c| (r * 100 + c).to_string()).collect();
        lines.push(row.join(","));
    }
    let table = lines.join("\n") + "\n";
    let w = world("cells-caps", table.as_bytes(), None);
    let v = open(&w);
    let got = window(&w, &v["slot"], (0, 0), (1000, 1000)).expect("a window");
    let cells = got["cells"].as_array().unwrap();
    assert_eq!(cells.len(), 200);
    assert_eq!(cells[0].as_array().unwrap().len(), 64);
    assert_eq!(
        (got["rows"].clone(), got["columns"].clone()),
        (json!(205), json!(70))
    );
}

#[test]
fn a_slot_gone_is_run_unknown_input() {
    let w = world("cells-gone", &csv(&["a", "1", "2"]), None);
    let v = open(&w);
    call(&w.w, "stats_desc_close", json!({ "slot": v["slot"] })).expect("close");
    let e = window(&w, &v["slot"], (0, 0), (1, 1)).expect_err("gone");
    assert_eq!(refusal(&e)["code"], "run.unknown_input", "{e}");
}

#[test]
fn a_make_keeps_the_slots_number_and_gives_a_new_input_hash_and_the_score() {
    let w = world(
        "cells-make",
        &csv(&["q1,q2,q3", "1,2,3", "4,6,8", "5,,7"]),
        None,
    );
    let v = open(&w);
    let before = window(&w, &v["slot"], (0, 0), (3, 9)).expect("before");
    assert_eq!(before["columns"], 3);
    let view = make(
        &w.w,
        &v["slot"],
        &recipe(3, "total", "mean", &["C1", "C2", "C3"]),
    );
    assert_eq!(view["slot"], v["slot"], "a make keeps the number");
    let after = window(&w, &v["slot"], (0, 0), (3, 9)).expect("after");
    assert_eq!(after["slot"], before["slot"]);
    assert_ne!(
        after["inputHash"], before["inputHash"],
        "so a stale window can be told"
    );
    assert_eq!(after["inputHash"], view["inputRef"]["contentHash"]);
    assert_eq!(after["columns"], 4);
    assert_eq!(after["cells"][1][3], "6", "the score's own column");
    assert_eq!(after["cells"][2][3], Value::Null, "a row it did not score");
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

#[test]
fn the_command_starts_no_run_and_writes_nothing() {
    let w = world("cells-writes", &csv(&["a,b", "1,2", "3,4"]), None);
    let v = open(&w);
    let before = file_hashes(&w.app_data);
    assert!(before.contains_key("gaply.db"), "{before:?}");
    let got = window(&w, &v["slot"], (0, 0), (2, 2)).expect("a window");
    assert_eq!(got["cells"][0][0], "1", "the command ran");
    let state = call(&w.w, "workspace_state", json!({})).expect("state");
    assert_eq!(state["activeRun"], Value::Null, "{state}");
    assert_eq!(file_hashes(&w.app_data), before);
}
