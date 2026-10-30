//! **The limits by memory at run time (Research OS, Phase 8F-1, decisions
//! 280, 282, 283 and 284).**
//!
//! * the dataset slots' budget in cells: an open drops the oldest slots
//!   until the cells held fit, never the dataset just opened; the count still
//!   binds at `MAX_SLOTS`; a run's handle outlives its slot; the production
//!   budget is `MAX_SLOTS × METHODS.max_cells` and never binds first;
//! * the open's progress: the table's entries, an open's rows read on
//!   success and on a refusal, its id gone once it returns, two ids apart,
//!   and a reused id as decision 280 says;
//! * through IPC: `stats_desc_open` with a `progressId` answers what it
//!   answers without one and leaves no entry; `stats_desc_open_progress`
//!   answers null for an unknown id; `stats_desc_limits` answers this
//!   computer's view;
//! * the rerun's code for every ingest refusal: the four size refusals that
//!   depend on the tier are `project.rerun_over_limits`, every other one
//!   `project.rerun_data_unreadable`.
//!
//! Every file is synthetic and small, in a folder this test creates and
//! removes.

mod desc_support;

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use serde_json::{json, Value};

use app_lib::memory_tier;
use app_lib::project_commands::rerun_ingest_code;
use app_lib::runs::{DatasetSlots, OpenProgress, MAX_SLOTS, SLOT_BUDGET_CELLS};
use app_lib::stats_desc_commands::{limits_view, open_with};
use app_lib::stats_l1_ingest::{load_dataset_from_bytes_with, L1IngestRefusal};
use app_lib::supplementary::IngestLimits;
use desc_support::{app, call, TempDir};
use gaply_core::contracts::codes::Closed;
use gaply_core::research_project::{DataFormat, ProjectCode, ProjectRefusal};
use gaply_core::stats_l1::dataset::{Dataset, DatasetRefusal};
use gaply_core::Database;

/// A dataset of `rows` rows and `cols` columns of numbers.
fn dataset(cols: usize, rows: usize) -> Dataset {
    let mut s = (1..=cols)
        .map(|c| format!("c{c}"))
        .collect::<Vec<_>>()
        .join(",");
    s.push('\n');
    for r in 0..rows {
        s.push_str(
            &(1..=cols)
                .map(|c| (r + c).to_string())
                .collect::<Vec<_>>()
                .join(","),
        );
        s.push('\n');
    }
    load_dataset_from_bytes_with(s.as_bytes(), DataFormat::Csv, IngestLimits::METHODS).unwrap()
}

/* ---- the slots' budget ------------------------------------------------------- */

#[test]
fn opens_past_the_budget_drop_the_oldest_slots() {
    let slots = DatasetSlots::with_cell_budget(10);
    let a = slots.insert("a".into(), dataset(2, 2));
    let b = slots.insert("b".into(), dataset(2, 2));
    assert_eq!((slots.held(), slots.cells_held()), (2, 8));
    let c = slots.insert("c".into(), dataset(2, 2));
    assert!(
        slots.get(a).is_err(),
        "the oldest is dropped: 12 cells is over 10"
    );
    assert!(slots.get(b).is_ok() && slots.get(c).is_ok());
    assert_eq!((slots.held(), slots.cells_held()), (2, 8));
}

#[test]
fn a_dataset_over_the_budget_alone_is_kept_as_the_newest() {
    let slots = DatasetSlots::with_cell_budget(10);
    let a = slots.insert("a".into(), dataset(2, 2));
    let big = slots.insert("big".into(), dataset(3, 4));
    assert!(slots.get(a).is_err(), "everything older goes");
    assert!(slots.get(big).is_ok(), "the dataset just opened stays");
    assert_eq!((slots.held(), slots.cells_held()), (1, 12));
    let next = slots.insert("next".into(), dataset(2, 2));
    assert!(slots.get(big).is_err() && slots.get(next).is_ok());
}

#[test]
fn the_count_still_binds_at_max_slots() {
    let slots = DatasetSlots::with_cell_budget(u64::MAX);
    let ids: Vec<u64> = (0..MAX_SLOTS + 1)
        .map(|i| slots.insert(format!("d{i}"), dataset(2, 2)))
        .collect();
    assert_eq!(slots.held(), MAX_SLOTS);
    assert!(slots.get(ids[0]).is_err());
    assert!(ids[1..].iter().all(|&id| slots.get(id).is_ok()));
}

#[test]
fn a_slot_held_by_a_run_keeps_its_handle_when_dropped_for_the_budget() {
    let slots = DatasetSlots::with_cell_budget(10);
    let a = slots.insert("a".into(), dataset(2, 2));
    let held = slots.get(a).unwrap();
    slots.insert("b".into(), dataset(2, 2));
    slots.insert("c".into(), dataset(2, 2));
    assert!(slots.get(a).is_err(), "the slot is gone");
    assert_eq!(
        held.dataset().rows(),
        2,
        "the run's handle still reads its dataset"
    );
    assert_eq!(
        Arc::strong_count(&held),
        1,
        "and is the last one: freed when the run ends"
    );
}

#[test]
fn the_production_budget_is_four_full_slots_and_never_binds_first() {
    assert_eq!(SLOT_BUDGET_CELLS, 2_560_000);
    assert_eq!(DatasetSlots::default().cell_budget(), 2_560_000);
    assert_eq!(
        DatasetSlots::numbered_by(Arc::new(AtomicU64::new(0))).cell_budget(),
        2_560_000
    );
    let m = IngestLimits::METHODS;
    assert_eq!(m.max_cells, m.max_rows * m.max_cols);
    assert!(
        SLOT_BUDGET_CELLS >= MAX_SLOTS as u64 * m.max_cells as u64,
        "MAX_SLOTS datasets at METHODS's cap fit, so the count binds first"
    );
}

/* ---- the open's progress ---------------------------------------------------- */

#[test]
fn an_entry_lives_while_its_open_does_and_an_unknown_id_has_none() {
    let table = Arc::new(OpenProgress::default());
    assert_eq!(table.rows_read(7), None);
    let entry = table.begin(7);
    assert_eq!(table.rows_read(7), Some(0));
    entry.counter().store(3_000, Ordering::Relaxed);
    assert_eq!(table.rows_read(7), Some(3_000));
    assert_eq!(table.rows_read(8), None, "an id no open has");
    drop(entry);
    assert_eq!(table.rows_read(7), None, "gone once the open returns");
    assert_eq!(table.in_flight(), 0);
}

#[test]
fn two_opens_keep_their_own_counts_and_a_reused_id_is_the_newest_opens() {
    let table = Arc::new(OpenProgress::default());
    let one = table.begin(1);
    let two = table.begin(2);
    one.counter().store(1_000, Ordering::Relaxed);
    two.counter().store(2_000, Ordering::Relaxed);
    assert_eq!(
        (table.rows_read(1), table.rows_read(2)),
        (Some(1_000), Some(2_000))
    );
    drop(two);
    assert_eq!(
        (table.rows_read(1), table.rows_read(2)),
        (Some(1_000), None)
    );

    // A second open with an id still in flight takes its entry, and the
    // first open's return does not remove the second's.
    let second = table.begin(1);
    assert_eq!(table.rows_read(1), Some(0));
    second.counter().store(5_000, Ordering::Relaxed);
    one.counter().store(1_500, Ordering::Relaxed);
    assert_eq!(table.rows_read(1), Some(5_000));
    drop(one);
    assert_eq!(table.rows_read(1), Some(5_000), "still the second open's");
    drop(second);
    assert_eq!(table.rows_read(1), None);
}

fn csv(cols: usize, rows: usize) -> Vec<u8> {
    let mut s = (1..=cols)
        .map(|c| format!("c{c}"))
        .collect::<Vec<_>>()
        .join(",");
    s.push('\n');
    for r in 0..rows {
        s.push_str(
            &(1..=cols)
                .map(|c| (r * c % 89).to_string())
                .collect::<Vec<_>>()
                .join(","),
        );
        s.push('\n');
    }
    s.into_bytes()
}

#[test]
fn an_opens_rows_read_reach_their_final_count_on_success_and_on_a_refusal() {
    let d = TempDir::new("progress_counts");
    let slots = DatasetSlots::default();
    let ok = d.write("ok.csv", &csv(2, 2_500));
    let rows = AtomicU64::new(u64::MAX);
    let view = open_with(&slots, &ok, Some(&rows), IngestLimits::METHODS).unwrap();
    assert_eq!(view.profile.rows, 2_500);
    assert_eq!(rows.load(Ordering::Relaxed), 2_500);

    // Refused for its cells at the 1,201st row: 1,200 rows were read.
    let small = IngestLimits {
        max_cells: 2_400,
        ..IngestLimits::METHODS
    };
    let rows = AtomicU64::new(u64::MAX);
    let e = open_with(&slots, &ok, Some(&rows), small)
        .err()
        .expect("refused");
    assert!(format!("{e:?}").contains("too_many_cells"), "{e:?}");
    assert_eq!(rows.load(Ordering::Relaxed), 1_200);

    // Refused for its rows: every row up to the limit was read.
    let long = d.write("long.csv", &csv(2, 5_001));
    let rows = AtomicU64::new(u64::MAX);
    let e = open_with(&slots, &long, Some(&rows), IngestLimits::METHODS)
        .err()
        .expect("refused");
    assert!(format!("{e:?}").contains("file_too_large"), "{e:?}");
    assert_eq!(rows.load(Ordering::Relaxed), 5_000);
}

/* ---- through IPC -------------------------------------------------------------- */

struct World {
    _dir: TempDir,
    _app: tauri::App<tauri::test::MockRuntime>,
    w: tauri::WebviewWindow<tauri::test::MockRuntime>,
    data: String,
}

fn world() -> World {
    let dir = TempDir::new("limits_ipc");
    let data = dir
        .write("data.csv", &csv(3, 40))
        .to_string_lossy()
        .into_owned();
    let app_data = dir.0.join("appdata");
    std::fs::create_dir_all(&app_data).unwrap();
    let (app, w) = app(&app_data, Arc::new(Database::in_memory().unwrap()), None);
    World {
        _dir: dir,
        _app: app,
        w,
        data,
    }
}

/// A dataset view with its slot number taken out, for comparing two opens.
fn without_slot(mut v: Value) -> Value {
    v.as_object_mut().unwrap().remove("slot");
    v
}

#[test]
fn an_open_with_a_progress_id_answers_as_one_without_and_leaves_no_entry() {
    let w = world();
    let plain = call(&w.w, "stats_desc_open", json!({ "path": w.data })).expect("opens");
    let counted = call(
        &w.w,
        "stats_desc_open",
        json!({ "path": w.data, "progressId": 41 }),
    )
    .expect("opens");
    assert_eq!(without_slot(plain), without_slot(counted));
    assert_eq!(
        call(
            &w.w,
            "stats_desc_open_progress",
            json!({ "progressId": 41 })
        ),
        Ok(Value::Null),
        "the open returned, so its id is gone"
    );
    assert_eq!(
        call(
            &w.w,
            "stats_desc_open_progress",
            json!({ "progressId": 999 })
        ),
        Ok(Value::Null),
        "an id no open has"
    );
    // A refused open leaves no entry either.
    let e = call(
        &w.w,
        "stats_desc_open",
        json!({ "path": format!("{}.missing", w.data), "progressId": 42 }),
    );
    assert!(e.is_err());
    assert_eq!(
        call(
            &w.w,
            "stats_desc_open_progress",
            json!({ "progressId": 42 })
        ),
        Ok(Value::Null)
    );
}

#[test]
fn stats_desc_limits_answers_this_computers_view() {
    let w = world();
    let got = call(&w.w, "stats_desc_limits", json!({})).expect("answers");
    let want = serde_json::to_value(limits_view(memory_tier::this_computer())).unwrap();
    assert_eq!(got, want);
    // Numbers and closed words only.
    let keys: Vec<&str> = got
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(
        keys,
        [
            "maxCells",
            "maxColumns",
            "maxFileBytes",
            "maxRows",
            "maxTextBytes",
            "maxWorkbookBytes",
            "memoryKnown",
            "tier"
        ]
    );
}

/* ---- the rerun's code ---------------------------------------------------------- */

#[test]
fn the_rerun_gives_over_limits_for_the_four_tier_size_refusals_and_unreadable_otherwise() {
    use L1IngestRefusal as R;
    let over = ProjectCode::RerunOverLimits;
    let unreadable = ProjectCode::RerunDataUnreadable;
    let every = [
        (R::NotReadable, unreadable),
        (
            R::TooLarge {
                bytes: 11,
                limit: 10,
            },
            over,
        ),
        (R::UnsupportedFormat, unreadable),
        (R::Malformed, unreadable),
        (R::NoTable, unreadable),
        (R::FileTooLarge { limit_rows: 5_000 }, unreadable),
        (R::FileChangedWhileReading, unreadable),
        (R::XlsNotReadable, unreadable),
        (
            R::Dataset(DatasetRefusal::TooWide {
                columns: 129,
                limit: 128,
            }),
            unreadable,
        ),
        (R::Dataset(DatasetRefusal::NoColumns), unreadable),
        (R::NotARegularFile, unreadable),
        (
            R::ArchiveTooLarge {
                declared: 2,
                limit: 1,
            },
            over,
        ),
        (R::ContentTooLarge { limit_bytes: 1 }, over),
        (R::EncodingNotUtf8, unreadable),
        (
            R::TooManyCells {
                limit_cells: 5_000_000,
            },
            over,
        ),
    ];
    for (refusal, want) in every {
        assert_eq!(rerun_ingest_code(refusal), want, "{refusal:?}");
    }
    assert_eq!(over.wire(), "project.rerun_over_limits");
    // Its refusal carries no numbers.
    let r = ProjectRefusal::from(over);
    assert_eq!((r.needed, r.limit), (None, None));
}
