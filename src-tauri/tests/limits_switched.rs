//! **The limits by memory, switched on (Research OS, Phase 8F-2, decision
//! 288).** `stats_desc_open` and the rerun read through
//! `IngestLimits::this_computer()`, and the Statistics engine's slots take
//! this computer's slot budget. The harness builds the engine as production
//! does, so every test here through IPC runs at the tier of the machine that
//! runs it (8 GB on the Mac this was written on, likely 16 GB on CI's Linux
//! runner) and asserts only what holds on every tier:
//!
//! * a 5,001-row file opens, which the limits before 8F-2 refused;
//! * 100,001 rows is `file_too_large` with `limit_rows` 100000;
//! * 129 columns is too wide;
//! * a sparse file one byte over this computer's file bytes is `too_large`
//!   with that limit (`check_size` reads the size from metadata alone).
//!
//! The cells and the text are tested through `open_with` at
//! `IngestLimits::for_tier(MemoryTier::Gb8)`, never through IPC: on 16 and
//! 32 GB the rows and columns bind before the cells. The slot budget is
//! tested by its number; eviction stays with `limits_runtime.rs`'s small
//! injected budgets. And what keeps today's limits keeps them: `open` (the
//! tests' wrapper), `load_dataset_from_bytes` and `DatasetSlots::default()`.
//!
//! Every file is synthetic, in a folder this test creates and removes.

mod desc_support;

use std::sync::Arc;

use serde_json::{json, Value};
use tauri::Manager;

use app_lib::memory_tier::{self, MemoryTier, MethodLimits};
use app_lib::runs::DatasetSlots;
use app_lib::state::AppState;
use app_lib::stats_desc_commands::{open, open_with};
use app_lib::stats_l1_ingest::{load_dataset_from_bytes, L1IngestRefusal};
use app_lib::supplementary::IngestLimits;
use desc_support::{app, call, refusal, TempDir};
use gaply_core::research_project::DataFormat;
use gaply_core::{Database, GaplyError};

/// A CSV of `cols` columns and `rows` data rows, each cell one digit.
fn csv(cols: usize, rows: usize) -> Vec<u8> {
    let mut s = (1..=cols)
        .map(|c| format!("c{c}"))
        .collect::<Vec<_>>()
        .join(",");
    s.push('\n');
    let row = vec!["1"; cols].join(",");
    for _ in 0..rows {
        s.push_str(&row);
        s.push('\n');
    }
    s.into_bytes()
}

/// The closed code inside a refusal from a command's body.
fn refused(e: GaplyError) -> Value {
    match e {
        GaplyError::Validation(m) => serde_json::from_str(&m).expect("a closed code in JSON"),
        other => panic!("not a validation error: {other:?}"),
    }
}

struct World {
    dir: TempDir,
    app: tauri::App<tauri::test::MockRuntime>,
    w: tauri::WebviewWindow<tauri::test::MockRuntime>,
}

fn world(tag: &str) -> World {
    let dir = TempDir::new(tag);
    let app_data = dir.0.join("appdata");
    std::fs::create_dir_all(&app_data).unwrap();
    let (app, w) = app(&app_data, Arc::new(Database::in_memory().unwrap()), None);
    World { dir, app, w }
}

fn ipc_open(w: &World, path: &std::path::Path) -> Result<Value, Value> {
    call(
        &w.w,
        "stats_desc_open",
        json!({ "path": path.to_string_lossy() }),
    )
}

/* ---- the switch -------------------------------------------------------------- */

#[test]
fn this_computers_limits_are_its_tiers() {
    assert_eq!(
        IngestLimits::this_computer(),
        IngestLimits::for_tier(memory_tier::this_computer().tier)
    );
}

#[test]
fn through_ipc_the_open_holds_what_every_tier_holds() {
    let w = world("switch_ipc");

    // 5,001 rows: refused before 8F-2, opened on every tier now.
    let p = w.dir.write("rows_5001.csv", &csv(2, 5_001));
    let o = ipc_open(&w, &p).expect("a 5,001-row file opens");
    assert_eq!(o["profile"]["rows"], 5_001);

    // 100,001 rows: over every tier's rows.
    let p = w.dir.write("rows_100001.csv", &csv(1, 100_001));
    let e = ipc_open(&w, &p).unwrap_err();
    assert_eq!(
        refusal(&e),
        json!({ "code": "file_too_large", "limit_rows": 100_000 })
    );

    // 129 columns: over every tier's columns.
    let p = w.dir.write("wide_129.csv", &csv(129, 2));
    let e = ipc_open(&w, &p).unwrap_err();
    assert_eq!(
        refusal(&e),
        json!({ "code": "dataset", "refusal": { "code": "too_wide", "columns": 129, "limit": 128 } })
    );

    // One byte over this computer's file bytes, as a sparse file: refused
    // from its size, before a byte of it is read.
    let limit = IngestLimits::this_computer().max_file_size_bytes;
    let p = w.dir.0.join("sparse.csv");
    std::fs::File::create(&p)
        .and_then(|f| f.set_len(limit + 1))
        .expect("a sparse file");
    let e = ipc_open(&w, &p).unwrap_err();
    let c = refusal(&e);
    assert_eq!(c["code"], "too_large", "{c}");
    assert_eq!(c["limit"], limit, "{c}");
    assert_eq!(c["bytes"], limit + 1, "{c}");
}

#[test]
fn the_engines_slots_have_this_computers_budget() {
    let w = world("switch_slots");
    let engine = w
        .app
        .state::<AppState>()
        .workspace
        .require_statistics()
        .expect("Statistics is active");
    assert_eq!(
        engine.datasets.cell_budget(),
        MethodLimits::for_tier(memory_tier::this_computer().tier).slot_budget_cells
    );
}

/* ---- at the 8 GB tier, through open_with ------------------------------------- */

#[test]
fn at_the_8_gb_tier_a_file_over_the_cells_is_too_many_cells() {
    // 39,063 rows × 128 columns, 5,000,064 cells of one digit each, about
    // 10 MB: inside the rows, the columns and the bytes, over the cells.
    let d = TempDir::new("switch_cells");
    let p = d.write("cells_over.csv", &csv(128, 39_063));
    let e = open_with(
        &DatasetSlots::default(),
        &p,
        None,
        IngestLimits::for_tier(MemoryTier::Gb8),
    )
    .err()
    .expect("refused");
    assert_eq!(
        refused(e),
        json!({ "code": "too_many_cells", "limit_cells": 5_000_000 })
    );
}

#[test]
fn at_the_8_gb_tier_with_a_smaller_text_cap_the_text_cap_holds() {
    // The 8 GB tier's text cap is 64 MiB; a real file over it would be large,
    // so the cap is injected smaller and the rest kept.
    let d = TempDir::new("switch_text");
    let l = IngestLimits {
        max_total_chars: 1_000,
        ..IngestLimits::for_tier(MemoryTier::Gb8)
    };
    let p = d.write("text_over.csv", &csv(10, 100));
    let e = open_with(&DatasetSlots::default(), &p, None, l)
        .err()
        .expect("refused");
    assert_eq!(
        refused(e),
        json!({ "code": "content_too_large", "limit_bytes": 1_000 })
    );
}

#[test]
fn the_8_gb_tiers_slot_budget_is_ten_million_cells() {
    assert_eq!(
        MethodLimits::for_tier(MemoryTier::Gb8).slot_budget_cells,
        10_000_000
    );
}

/* ---- what keeps today's limits ------------------------------------------------ */

#[test]
fn the_tests_open_the_bytes_wrapper_and_default_slots_keep_todays_limits() {
    let d = TempDir::new("switch_today");
    let bytes = csv(2, 5_001);
    let p = d.write("rows_5001.csv", &bytes);
    let e = open(&DatasetSlots::default(), &p).err().expect("refused");
    assert_eq!(
        refused(e),
        json!({ "code": "file_too_large", "limit_rows": 5_000 })
    );
    assert_eq!(
        load_dataset_from_bytes(&bytes, DataFormat::Csv).err(),
        Some(L1IngestRefusal::FileTooLarge { limit_rows: 5_000 })
    );
    assert_eq!(DatasetSlots::default().cell_budget(), 2_560_000);
    assert_eq!(DatasetSlots::with_cell_budget(7).cell_budget(), 7);
}
