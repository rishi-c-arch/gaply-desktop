//! **The descriptive summary's commands at their boundary (Research OS,
//! Phase 2B).** Every call goes through Tauri's IPC on the mock runtime,
//! with the webview's argument names, into the state `AppState::new` builds,
//! and every assertion reads the JSON the webview reads.
//!
//! * The flow: open → profile → validate → run → status → result → export →
//!   close.
//! * Refusals, each with its closed code: a file over the caps, an unknown
//!   column, Numeric asked of a text column, an unknown slot or run.
//! * Run states, through `Runs::with_hooks`: a run parked at column k and
//!   cancelled; a ceiling of 0; a second run while one is parked; eviction;
//!   every transition the commands can reach, by the record's path.
//! * Metamorphic, through the real parser: one table as csv, tsv and xlsx
//!   gives byte-identical results; a BOM and CRLF line endings change
//!   nothing.

mod desc_support;

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex, OnceLock, Weak};
use std::time::{Duration, Instant};

use serde_json::{json, Value};
use tauri::Manager;

use app_lib::runs::{CheckpointHook, Clock, DatasetSlots, Point, Runs, MAX_RUNS};
use app_lib::state::AppState;
use desc_support::*;
use gaply_core::contracts::{OperationId, RunId, RunState};
use gaply_core::stats_desc::{self, DescribeParams};
use gaply_core::stats_l1::dataset::{build_dataset, DataFingerprint, RawTable, TableRef};
use gaply_core::Database;

/* ---- fixtures --------------------------------------------------------------- */

const HEADERS: [&str; 4] = ["score", "group", "note", "aux"];

fn rows() -> Vec<Vec<C<'static>>> {
    let score = [
        "44.1257", "10.5", "95.5", "61.8129", "23.25", "50", "82", "31", "70.75",
    ];
    let group = ["a", "b", "a", "b", "a", "b", "a", "b", "a"];
    let note = ["n1", "n2", "n3", "n4", "n5", "n6", "n7", "n8", "n9"];
    let aux = ["1", "2", "3", "", "5", "6", "7", "8", "9"];
    (0..9)
        .map(|i| {
            vec![
                C::N(score[i]),
                C::T(group[i]),
                C::T(note[i]),
                if aux[i].is_empty() {
                    C::Empty
                } else {
                    C::N(aux[i])
                },
            ]
        })
        .collect()
}

fn csv() -> Vec<u8> {
    delimited(&HEADERS, &rows(), ",", "\n")
}

const ALL: [(&str, &str); 4] = [
    ("C1", "numeric"),
    ("C2", "categorical"),
    ("C3", "categorical"),
    ("C4", "numeric"),
];

struct World {
    _dir: TempDir,
    app: tauri::App<tauri::test::MockRuntime>,
    w: tauri::WebviewWindow<tauri::test::MockRuntime>,
    data: std::path::PathBuf,
}

fn world(runs: Option<Arc<Runs>>) -> World {
    let dir = TempDir::new("cmds");
    let data = dir.write("data.csv", &csv());
    let app_data = dir.0.join("appdata");
    std::fs::create_dir_all(&app_data).unwrap();
    let db = Arc::new(Database::in_memory().unwrap());
    let (app, w) = app(&app_data, db, runs);
    World {
        _dir: dir,
        app,
        w,
        data,
    }
}

fn open(w: &World) -> Value {
    call(
        &w.w,
        "stats_desc_open",
        json!({ "path": w.data.to_string_lossy() }),
    )
    .expect("open")
}

fn start(w: &World, open: &Value, op: u64, columns: &[(&str, &str)]) -> Result<u64, Value> {
    call(
        &w.w,
        "stats_desc_run",
        json!({ "slot": open["slot"], "spec": spec(open, op, columns, &[]) }),
    )
    .map(|v| v["runId"].as_u64().expect("runId"))
}

/* ---- the check-point hook --------------------------------------------------- */

struct SinceTest(Instant);
impl Clock for SinceTest {
    fn now_ms(&self) -> u64 {
        self.0.elapsed().as_millis() as u64
    }
}

/// A hook that parks the worker wherever `when` says, reports the run it
/// parked, and waits for one message on the returned sender.
fn park_when(
    when: impl Fn(RunId, Point) -> bool + Send + Sync + 'static,
) -> (CheckpointHook, Receiver<RunId>, Sender<()>) {
    let (parked_tx, parked_rx) = mpsc::channel();
    let (go_tx, go_rx) = mpsc::channel::<()>();
    let go_rx = Mutex::new(go_rx);
    let hook: CheckpointHook = Arc::new(move |run, point| {
        if when(run, point) {
            parked_tx.send(run).ok();
            go_rx
                .lock()
                .unwrap()
                .recv_timeout(Duration::from_secs(20))
                .ok();
        }
    });
    (hook, parked_rx, go_tx)
}

fn runs_with(hook: Option<CheckpointHook>, ceiling_ms: u64) -> Arc<Runs> {
    Arc::new(Runs::with_hooks(
        Arc::new(SinceTest(Instant::now())),
        hook,
        ceiling_ms,
    ))
}

fn path(runs: &Runs, run: u64) -> Vec<RunState> {
    runs.snapshot(RunId(run)).expect("held").path
}

use RunState as S;

/* ---- the flow --------------------------------------------------------------- */

#[test]
fn open_profile_validate_run_status_result_export_close_through_ipc() {
    let runs = runs_with(None, 30_000);
    let w = world(Some(runs.clone()));

    // open → the profile, with the base name only.
    let o = open(&w);
    assert_eq!(o["displayName"], "data.csv");
    assert_eq!(o["profile"]["rows"], 9);
    let kinds: Vec<&str> = o["profile"]["columns"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["kind"].as_str().unwrap())
        .collect();
    assert_eq!(kinds, ["numeric", "categorical", "categorical", "numeric"]);
    assert_eq!(o["profile"]["columns"][3]["missing"], 1);
    assert_eq!(o["inputRef"]["contentHash"].as_str().unwrap().len(), 64);
    assert_eq!(o["inputRef"]["rows"], 9);
    assert!(
        !o.to_string().contains(&*w._dir.0.to_string_lossy()),
        "no path in the open view"
    );

    // validate
    let v = call(
        &w.w,
        "stats_desc_validate",
        json!({ "slot": o["slot"], "spec": spec(&o, 1, &ALL, &[]) }),
    )
    .expect("validate");
    assert_eq!(v["status"], "valid", "{v}");
    assert_eq!(v["estimatedResources"]["cells"], 36);

    // run → status → result
    let run = start(&w, &o, 1, &ALL).expect("run");
    let s = until_terminal(&w.w, run);
    assert_eq!(s["state"], "succeeded", "{s}");
    assert_eq!(
        (s["columnsDone"].as_u64(), s["columnsTotal"].as_u64()),
        (Some(4), Some(4))
    );
    assert_eq!(
        path(&runs, run),
        [
            S::Draft,
            S::Validating,
            S::Validated,
            S::Queued,
            S::Running,
            S::Succeeded
        ]
    );

    let r = call(&w.w, "stats_desc_result", json!({ "runId": run })).expect("result");
    assert_eq!(r["status"], "succeeded");
    assert_eq!(r["runId"], run);
    assert_eq!(r["columns"].as_array().unwrap().len(), 4);
    assert_eq!(r["columns"][0]["numeric"]["header"], "score");
    assert_eq!(r["columns"][0]["numeric"]["min"], json!({ "value": 10.5 }));
    assert_eq!(r["columns"][0]["numeric"]["max"], json!({ "value": 95.5 }));
    assert_eq!(
        r["columns"][1]["categorical"]["levels"],
        json!([{ "label": "a", "count": 5 }, { "label": "b", "count": 4 }])
    );
    let m = &r["manifest"];
    assert_eq!(m["capabilityId"], "stats.describe");
    assert_eq!(
        (m["rowsRead"].as_u64(), m["rowsUsed"].as_u64()),
        (Some(9), Some(9))
    );
    assert_eq!(m["inputs"][0], o["inputRef"]["contentHash"]);
    assert_eq!(m["reproducibility"], "bitwise");

    // The command layer adds nothing: the result is gaply_core's describe on
    // the dataset Level 1's ingest reads from the same file.
    let ds = app_lib::stats_l1_ingest::load_dataset(&w.data).unwrap();
    let params: gaply_core::stats_desc::DescribeParams =
        serde_json::from_value(spec(&o, 1, &ALL, &[])["parameters"].clone()).unwrap();
    let direct = gaply_core::stats_desc::describe(&ds, &params).unwrap();
    assert_eq!(r["columns"], serde_json::to_value(&direct.columns).unwrap());
    assert_eq!(
        m["outputHash"],
        gaply_core::contracts::ContentHash::of_bytes(&direct.canonical_bytes()).as_hex()
    );

    // export: bytes and their SHA-256, nothing written.
    let csv = call(
        &w.w,
        "stats_desc_export",
        json!({ "runId": run, "format": "csv" }),
    )
    .expect("csv");
    let bytes: Vec<u8> = serde_json::from_value(csv["bytes"].clone()).unwrap();
    assert_eq!(
        csv["sha256"],
        gaply_core::contracts::ContentHash::of_bytes(&bytes).as_hex()
    );
    assert_eq!(bytes, gaply_core::stats_desc::export::result_csv(&direct));
    let text = String::from_utf8(bytes).unwrap();
    assert!(
        text.contains("\"score\"") && text.contains("\"group\""),
        "{text}"
    );
    let mj = call(
        &w.w,
        "stats_desc_export",
        json!({ "runId": run, "format": "manifestJson" }),
    )
    .expect("manifest");
    let mbytes: Vec<u8> = serde_json::from_value(mj["bytes"].clone()).unwrap();
    let parsed: Value = serde_json::from_slice(&mbytes).unwrap();
    assert_eq!(parsed["outputHash"], m["outputHash"]);
    assert_eq!(
        mj["sha256"],
        gaply_core::contracts::ContentHash::of_bytes(&mbytes).as_hex()
    );

    // close: dropped once; the slot is then unknown.
    assert_eq!(
        call(&w.w, "stats_desc_close", json!({ "slot": o["slot"] })).unwrap(),
        json!(true)
    );
    assert_eq!(
        call(&w.w, "stats_desc_close", json!({ "slot": o["slot"] })).unwrap(),
        json!(false)
    );
    let gone = call(
        &w.w,
        "stats_desc_validate",
        json!({ "slot": o["slot"], "spec": spec(&o, 2, &ALL, &[]) }),
    )
    .unwrap_err();
    assert_eq!(refusal(&gone)["code"], "run.unknown_input");
    // The record outlives the slot: a finished result is still readable.
    assert_eq!(
        call(&w.w, "stats_desc_status", json!({ "runId": run })).unwrap()["state"],
        "succeeded"
    );
}

/* ---- refusals --------------------------------------------------------------- */

#[test]
fn open_refuses_the_cell_text_cap_with_its_own_code_and_opens_a_file_at_it() {
    // Evaluation of Phases 2 and 3, F4 and decision 37. The ingest keeps at
    // most 2 MiB of cell text (`supplementary::MAX_TOTAL_CHARS`, counted in
    // UTF-8 bytes after each cell is clamped). Before the fix a file over it
    // was refused as `malformed`.
    let w = world(None);
    let dir = TempDir::new("textcap");

    // The evaluation's shape: 5,000 rows by 128 columns of numbers, 3.8 MB,
    // inside the row, column and size caps.
    let mut wide = (1..=128)
        .map(|c| format!("c{c}"))
        .collect::<Vec<_>>()
        .join(",");
    wide.push('\n');
    for r in 0..5_000usize {
        let row: Vec<String> = (0..128usize)
            .map(|c| format!("{}.{}", (r * 7 + c * 13) % 1000, (r + c) % 97))
            .collect();
        wide.push_str(&row.join(","));
        wide.push('\n');
    }
    assert!(wide.len() < 10 * 1024 * 1024);
    let p = dir.write("wide_5000x128.csv", wide.as_bytes());
    let e = call(
        &w.w,
        "stats_desc_open",
        json!({ "path": p.to_string_lossy() }),
    )
    .unwrap_err();
    assert_eq!(
        refusal(&e),
        json!({ "code": "content_too_large", "limit_bytes": 2_097_152 })
    );

    // Exactly at the cap: 128 four-byte headers, 2,047 rows of 128 eight-byte
    // cells and one row of 128 four-byte cells, 2,097,152 bytes of cell text.
    let at_cap = |last: &str| -> Vec<u8> {
        let mut s = (1..=128)
            .map(|c| format!("h{c:03}"))
            .collect::<Vec<_>>()
            .join(",");
        s.push('\n');
        let full = vec!["1234.567"; 128].join(",");
        for _ in 0..2_047 {
            s.push_str(&full);
            s.push('\n');
        }
        let mut cells = vec!["1.25"; 128];
        cells[127] = last;
        s.push_str(&cells.join(","));
        s.push('\n');
        s.into_bytes()
    };
    let p = dir.write("at_cap.csv", &at_cap("1.25"));
    let o = call(
        &w.w,
        "stats_desc_open",
        json!({ "path": p.to_string_lossy() }),
    )
    .expect("a file at the cap opens");
    assert_eq!(o["profile"]["rows"], 2_048, "{}", o["profile"]["rows"]);
    // One byte more is over it.
    let p = dir.write("over_cap.csv", &at_cap("1.255"));
    let e = call(
        &w.w,
        "stats_desc_open",
        json!({ "path": p.to_string_lossy() }),
    )
    .unwrap_err();
    assert_eq!(refusal(&e)["code"], "content_too_large");

    // A workbook too: 65 rows of 64 text cells of 512 characters (the cell
    // cap), 2,129,920 bytes of cell text with the headers.
    let long = "a".repeat(512);
    let headers: Vec<String> = (1..=64).map(|c| format!("t{c}")).collect();
    let headers: Vec<&str> = headers.iter().map(String::as_str).collect();
    let rows: Vec<Vec<C>> = (0..65).map(|_| vec![C::T(&long); 64]).collect();
    let p = dir.write("text_cap.xlsx", &xlsx(&headers, &rows));
    let e = call(
        &w.w,
        "stats_desc_open",
        json!({ "path": p.to_string_lossy() }),
    )
    .unwrap_err();
    assert_eq!(
        refusal(&e),
        json!({ "code": "content_too_large", "limit_bytes": 2_097_152 })
    );
}

#[test]
fn open_refuses_a_file_over_the_caps_with_its_code_and_holds_nothing() {
    let w = world(None);
    let dir = TempDir::new("caps");

    // 5,001 rows: over the row cap.
    let mut long = String::from("x\n");
    for i in 0..5_001 {
        long.push_str(&format!("{i}\n"));
    }
    let p = dir.write("long.csv", long.as_bytes());
    let e = call(
        &w.w,
        "stats_desc_open",
        json!({ "path": p.to_string_lossy() }),
    )
    .unwrap_err();
    assert_eq!(
        refusal(&e),
        json!({ "code": "file_too_large", "limit_rows": 5000 })
    );

    // Over 10 MiB: refused from its size before it is parsed.
    let big = vec![b'1'; 10 * 1024 * 1024 + 2];
    let p = dir.write("big.csv", &big);
    let e = call(
        &w.w,
        "stats_desc_open",
        json!({ "path": p.to_string_lossy() }),
    )
    .unwrap_err();
    let c = refusal(&e);
    assert_eq!(c["code"], "too_large", "{c}");
    assert_eq!(c["limit"], 10 * 1024 * 1024);

    // Not a regular file: a folder.
    let e = call(
        &w.w,
        "stats_desc_open",
        json!({ "path": dir.0.to_string_lossy() }),
    )
    .unwrap_err();
    assert_eq!(refusal(&e)["code"], "not_a_regular_file");

    // No refusal names the file or its folder, and nothing was held.
    assert!(!e.to_string().contains("caps"), "{e}");
    assert_eq!(
        w.app
            .state::<AppState>()
            .workspace
            .require_statistics()
            .expect("Statistics is active")
            .datasets
            .held(),
        0
    );
}

#[test]
fn validate_refuses_an_unknown_column_and_numeric_asked_of_a_text_column() {
    let w = world(None);
    let o = open(&w);

    let v = call(
        &w.w,
        "stats_desc_validate",
        json!({ "slot": o["slot"], "spec": spec(&o, 1, &[("C9", "numeric")], &[]) }),
    )
    .unwrap();
    assert_eq!(v["status"], "invalid");
    assert_eq!(
        v["errors"],
        json!([{ "code": "stats.desc.unknown_column", "params": { "column": 9 }, "severity": "error" }])
    );

    let v = call(
        &w.w,
        "stats_desc_validate",
        json!({ "slot": o["slot"], "spec": spec(&o, 2, &[("C2", "numeric")], &[]) }),
    )
    .unwrap();
    assert_eq!(v["status"], "invalid");
    assert_eq!(
        v["errors"],
        json!([{ "code": "stats.desc.numeric_on_text", "params": { "column": 2, "count": 9 }, "severity": "error" }])
    );

    // An unknown slot, and an unknown run.
    let e = call(
        &w.w,
        "stats_desc_validate",
        json!({ "slot": 99, "spec": spec(&o, 3, &ALL, &[]) }),
    )
    .unwrap_err();
    assert_eq!(refusal(&e)["code"], "run.unknown_input");
    for cmd in [
        "stats_desc_status",
        "stats_desc_result",
        "stats_desc_cancel",
    ] {
        let e = call(&w.w, cmd, json!({ "runId": 77 })).unwrap_err();
        assert_eq!(refusal(&e)["code"], "run.unknown_run", "{cmd}");
    }
    let e = call(
        &w.w,
        "stats_desc_export",
        json!({ "runId": 77, "format": "csv" }),
    )
    .unwrap_err();
    assert_eq!(refusal(&e)["code"], "run.unknown_run");
}

/* ---- run states ------------------------------------------------------------- */

#[test]
fn a_run_parked_at_column_k_and_cancelled_ends_cancelled_with_fewer_columns_processed() {
    const K: usize = 2;
    let (hook, parked, go) = park_when(|_, p| p == Point::Column(K));
    let runs = runs_with(Some(hook), 30_000);
    let w = world(Some(runs.clone()));
    let o = open(&w);
    let run = start(&w, &o, 1, &ALL).unwrap();
    parked
        .recv_timeout(Duration::from_secs(10))
        .expect("parked at column k");

    let c = call(&w.w, "stats_desc_cancel", json!({ "runId": run })).unwrap();
    assert_eq!(c["state"], "running");
    assert_eq!(c["cancelRequested"], true);
    go.send(()).unwrap();

    let s = until_terminal(&w.w, run);
    assert_eq!(s["state"], "cancelled", "{s}");
    assert_eq!(s["columnsDone"], K as u64);
    assert!(
        s["columnsDone"].as_u64() < s["columnsTotal"].as_u64(),
        "fewer than all columns: {s}"
    );
    assert_eq!(
        path(&runs, run),
        [
            S::Draft,
            S::Validating,
            S::Validated,
            S::Queued,
            S::Running,
            S::Cancelled
        ]
    );
    // Cancelled is never reported as succeeded, and has no result.
    for (cmd, body) in [
        ("stats_desc_result", json!({ "runId": run })),
        (
            "stats_desc_export",
            json!({ "runId": run, "format": "csv" }),
        ),
    ] {
        assert_eq!(
            refusal(&call(&w.w, cmd, body).unwrap_err())["code"],
            "run.cancelled",
            "{cmd}"
        );
    }
}

/// Evaluation of Phases 2 and 3, F1. A cancel that lands after the LAST
/// check point has read the flag, while the last column is being summarised,
/// finds no check point left to stop at: every column is done and the worker
/// reaches its success branch with the flag set. It must still end
/// `Cancelled`, with no result. Before this test, deleting that branch's
/// cancel arm in `runs.rs` left every committed test green.
#[test]
fn a_cancel_after_the_last_check_point_ends_cancelled_with_no_result() {
    let last = ALL.len() - 1;
    let (hook, parked, go) = park_when(move |_, p| p == Point::Passed(last));
    let runs = runs_with(Some(hook), 30_000);
    let w = world(Some(runs.clone()));
    let o = open(&w);
    let run = start(&w, &o, 1, &ALL).unwrap();
    parked
        .recv_timeout(Duration::from_secs(10))
        .expect("parked after the last check point");

    let c = call(&w.w, "stats_desc_cancel", json!({ "runId": run })).unwrap();
    assert_eq!(c["state"], "running", "accepted while running: {c}");
    assert_eq!(c["cancelRequested"], true);
    go.send(()).unwrap();

    let s = until_terminal(&w.w, run);
    assert_eq!(s["state"], "cancelled", "{s}");
    assert_eq!(s["cancelRequested"], true);
    // Not vacuous: no check point was left, so every column was summarised
    // and then discarded.
    assert_eq!(s["columnsDone"], ALL.len() as u64, "{s}");
    assert_eq!(
        path(&runs, run),
        [
            S::Draft,
            S::Validating,
            S::Validated,
            S::Queued,
            S::Running,
            S::Cancelled
        ]
    );
    for (cmd, body) in [
        ("stats_desc_result", json!({ "runId": run })),
        (
            "stats_desc_export",
            json!({ "runId": run, "format": "csv" }),
        ),
    ] {
        assert_eq!(
            refusal(&call(&w.w, cmd, body).unwrap_err())["code"],
            "run.cancelled",
            "{cmd}"
        );
    }
}

/// A clock for F2 that stands still until a hook arms it. Armed, its next
/// reading on the armed thread (the check point's reading of the ceiling,
/// which comes AFTER its reading of the cancel flag) accepts a cancel of the
/// run through `Runs::cancel` and moves time on by the whole ceiling. So the
/// cancel and the ceiling land at the same check point, by construction and
/// not by timing.
struct CancelAtTheCeiling {
    now: AtomicU64,
    ceiling: u64,
    armed: Mutex<Option<(std::thread::ThreadId, RunId)>>,
    runs: OnceLock<Weak<Runs>>,
    /// The state each cancel was accepted in.
    accepted: Mutex<Vec<RunState>>,
}

impl Clock for CancelAtTheCeiling {
    fn now_ms(&self) -> u64 {
        let fire = {
            let mut armed = self.armed.lock().unwrap();
            match *armed {
                Some((thread, run)) if thread == std::thread::current().id() => {
                    *armed = None;
                    Some(run)
                }
                _ => None,
            }
        };
        if let Some(run) = fire {
            self.now.fetch_add(self.ceiling, Ordering::SeqCst);
            let runs = self.runs.get().and_then(Weak::upgrade).expect("runs");
            let snap = runs.cancel(run).expect("the run is held");
            assert!(snap.cancel_requested);
            self.accepted.lock().unwrap().push(snap.record.state);
        }
        self.now.load(Ordering::SeqCst)
    }
}

/// Evaluation of Phases 2 and 3, F2. A cancel accepted while running ends
/// the run `Cancelled` even when the ceiling also passes at the same check
/// point. 1,000 runs, each deterministic: the cancel is accepted inside the
/// check point's own reading of the clock.
#[test]
fn a_cancel_and_the_ceiling_at_the_same_check_point_end_cancelled_every_time() {
    use gaply_core::contracts::codes::Closed;
    const CEILING: u64 = 1_000;
    const K: usize = 1;
    const RUNS: usize = 1_000;
    let clock = Arc::new(CancelAtTheCeiling {
        now: AtomicU64::new(0),
        ceiling: CEILING,
        armed: Mutex::new(None),
        runs: OnceLock::new(),
        accepted: Mutex::new(Vec::new()),
    });
    let arm = clock.clone();
    let hook: CheckpointHook = Arc::new(move |run, p| {
        if p == Point::Column(K) {
            *arm.armed.lock().unwrap() = Some((std::thread::current().id(), run));
        }
    });
    let runs = Arc::new(Runs::with_hooks(clock.clone(), Some(hook), CEILING));
    clock
        .runs
        .set(Arc::downgrade(&runs))
        .ok()
        .expect("set once");

    let slots = DatasetSlots::default();
    let dataset = build_dataset(
        RawTable::from_rows(
            vec!["a".into(), "b".into(), "c".into()],
            (1..=5)
                .map(|i| vec![i.to_string(), (i * 2).to_string(), (i * 3).to_string()])
                .collect(),
        ),
        DataFingerprint::of_bytes(b"synthetic", "unit", TableRef::FirstTable),
    )
    .expect("dataset");
    let params = DescribeParams::defaults_for(&stats_desc::profile(&dataset));
    assert_eq!(params.columns.len(), 3);
    let slot = slots
        .get(slots.insert("synthetic".into(), dataset))
        .expect("slot");

    let mut ended = std::collections::BTreeMap::<String, usize>::new();
    for i in 0..RUNS as u64 {
        let spec = stats_desc::spec(OperationId(i + 1), slot.dataset(), params.clone());
        let run = runs.start(slot.clone(), spec).expect("started");
        let deadline = Instant::now() + Duration::from_secs(10);
        let s = loop {
            let s = runs.snapshot(run).expect("held");
            if s.record.state.is_terminal() {
                break s;
            }
            assert!(Instant::now() < deadline, "run {i} did not end");
            std::thread::yield_now();
        };
        *ended
            .entry(format!(
                "{} cancel_requested={} columns_done={}",
                s.record.state.wire(),
                s.cancel_requested,
                s.columns_done
            ))
            .or_default() += 1;
    }
    let accepted = clock.accepted.lock().unwrap().clone();
    // Not vacuous: every run's cancel was accepted while it ran, and each run
    // stopped at check point K, which the flag (read before the cancel) did
    // not stop, so the ceiling did.
    assert_eq!(accepted.len(), RUNS, "a cancel was accepted in every run");
    assert!(
        accepted.iter().all(|s| *s == RunState::Running),
        "{accepted:?}"
    );
    assert_eq!(
        ended.into_iter().collect::<Vec<_>>(),
        [(
            format!("cancelled cancel_requested=true columns_done={K}"),
            RUNS
        )],
        "every run must end Cancelled"
    );
}

/// F2, the other outcomes of the worker's last step: a cancel accepted while
/// running, then a panic in the describe loop, still ends `Cancelled`.
#[test]
fn a_cancel_accepted_before_the_describe_loop_panics_ends_cancelled() {
    let me: Arc<OnceLock<Weak<Runs>>> = Arc::new(OnceLock::new());
    let me2 = me.clone();
    let hook: CheckpointHook = Arc::new(move |run, p| {
        if p == Point::Passed(1) {
            let runs = me2.get().and_then(Weak::upgrade).expect("runs");
            assert!(runs.cancel(run).expect("held").cancel_requested);
            panic!("a deliberate panic after the cancel was accepted");
        }
    });
    let runs = runs_with(Some(hook), 30_000);
    me.set(Arc::downgrade(&runs)).ok().expect("set once");
    let w = world(Some(runs.clone()));
    let o = open(&w);
    let run = start(&w, &o, 1, &ALL).unwrap();
    let s = until_terminal(&w.w, run);
    assert_eq!(s["state"], "cancelled", "{s}");
    assert_eq!(s["cancelRequested"], true);
    assert_eq!(path(&runs, run).last(), Some(&S::Cancelled));
    assert_eq!(
        refusal(&call(&w.w, "stats_desc_result", json!({ "runId": run })).unwrap_err())["code"],
        "run.cancelled"
    );
}

#[test]
fn a_ceiling_of_zero_ends_the_run_timed_out() {
    let runs = runs_with(None, 0);
    let w = world(Some(runs.clone()));
    let o = open(&w);
    let run = start(&w, &o, 1, &ALL).unwrap();
    let s = until_terminal(&w.w, run);
    assert_eq!(s["state"], "timed_out", "{s}");
    assert_eq!(s["columnsDone"], 0);
    assert_eq!(
        path(&runs, run),
        [
            S::Draft,
            S::Validating,
            S::Validated,
            S::Queued,
            S::Running,
            S::TimedOut
        ]
    );
    assert_eq!(
        refusal(&call(&w.w, "stats_desc_result", json!({ "runId": run })).unwrap_err())["code"],
        "run.timed_out"
    );
}

#[test]
fn a_second_run_while_one_is_parked_is_refused() {
    let (hook, parked, go) = park_when(|run, p| run == RunId(1) && p == Point::Column(0));
    let runs = runs_with(Some(hook), 30_000);
    let w = world(Some(runs.clone()));
    let o = open(&w);
    let first = start(&w, &o, 1, &ALL).unwrap();
    parked
        .recv_timeout(Duration::from_secs(10))
        .expect("parked");

    let e = start(&w, &o, 2, &ALL).unwrap_err();
    assert_eq!(refusal(&e)["code"], "run.run_busy");
    assert_eq!(runs.held(), 1, "a refused start makes no record");

    go.send(()).unwrap();
    assert_eq!(until_terminal(&w.w, first)["state"], "succeeded");
    let second = start(&w, &o, 3, &ALL).expect("accepted once the first has ended");
    assert_eq!(until_terminal(&w.w, second)["state"], "succeeded");
}

#[test]
fn eviction_keeps_the_parked_record_and_drops_the_oldest_finished_one() {
    let parked_run = RunId(MAX_RUNS as u64 + 1);
    let (hook, parked, go) = park_when(move |run, p| run == parked_run && p == Point::Column(0));
    let runs = runs_with(Some(hook), 30_000);
    let w = world(Some(runs.clone()));
    let o = open(&w);
    let mut finished = Vec::new();
    for op in 1..=MAX_RUNS as u64 {
        let run = start(&w, &o, op, &[("C1", "numeric")]).unwrap();
        assert_eq!(until_terminal(&w.w, run)["state"], "succeeded");
        finished.push(run);
    }
    assert_eq!(runs.held(), MAX_RUNS);
    let ninth = start(&w, &o, 99, &ALL).unwrap();
    assert_eq!(RunId(ninth), parked_run);
    parked
        .recv_timeout(Duration::from_secs(10))
        .expect("parked");

    assert_eq!(runs.held(), MAX_RUNS);
    assert_eq!(
        call(&w.w, "stats_desc_status", json!({ "runId": ninth })).unwrap()["state"],
        "running"
    );
    for cmd in ["stats_desc_status", "stats_desc_result"] {
        let e = call(&w.w, cmd, json!({ "runId": finished[0] })).unwrap_err();
        assert_eq!(
            refusal(&e)["code"],
            "run.unknown_run",
            "{cmd}: the oldest finished record is gone"
        );
    }
    for run in &finished[1..] {
        assert_eq!(
            call(&w.w, "stats_desc_status", json!({ "runId": run })).unwrap()["state"],
            "succeeded"
        );
    }
    go.send(()).unwrap();
    assert_eq!(until_terminal(&w.w, ninth)["state"], "succeeded");
}

/// Every edge of the state table the commands can reach, each by the
/// record's full path. Not reachable from the commands, by design: anything
/// through `AwaitingConfirmation` (Phase 4's confirmation), `ResourceLimit`
/// and `NumericalFailure` (no budget and no run-level numerical failure in
/// this capability), and `Queued –refuse→ Refused`, which only a failure to
/// spawn the worker thread produces.
#[test]
fn every_transition_the_commands_can_reach() {
    let mut edges = std::collections::BTreeSet::new();
    let mut note = |p: &[RunState]| {
        for pair in p.windows(2) {
            edges.insert(format!("{} -> {}", pair[0].wire(), pair[1].wire()));
        }
    };
    use gaply_core::contracts::codes::Closed;

    // Succeeded, Refused, NeedsUserInput.
    {
        let runs = runs_with(None, 30_000);
        let w = world(Some(runs.clone()));
        let o = open(&w);
        let run = start(&w, &o, 1, &ALL).unwrap();
        until_terminal(&w.w, run);
        note(&path(&runs, run));

        let run = start(&w, &o, 2, &[("C9", "numeric")]).unwrap();
        let s = until_terminal(&w.w, run);
        assert_eq!(s["state"], "refused");
        assert_eq!(s["diagnostics"][0]["code"], "stats.desc.unknown_column");
        assert_eq!(path(&runs, run), [S::Draft, S::Validating, S::Refused]);
        assert_eq!(
            refusal(&call(&w.w, "stats_desc_result", json!({ "runId": run })).unwrap_err())["code"],
            "run.no_result"
        );
        note(&path(&runs, run));

        let run = start(&w, &o, 3, &[]).unwrap();
        let s = until_terminal(&w.w, run);
        assert_eq!(s["state"], "needs_user_input");
        assert_eq!(
            s["diagnostics"][0]["code"],
            "stats.desc.no_columns_selected"
        );
        assert_eq!(
            path(&runs, run),
            [S::Draft, S::Validating, S::NeedsUserInput]
        );
        note(&path(&runs, run));

        // A cancel of a finished run changes nothing.
        let before = call(&w.w, "stats_desc_status", json!({ "runId": 1 })).unwrap();
        let after = call(&w.w, "stats_desc_cancel", json!({ "runId": 1 })).unwrap();
        assert_eq!(after["state"], "succeeded");
        assert_eq!(after["cancelRequested"], false);
        assert_eq!(before["elapsedMs"], after["elapsedMs"]);
        assert_eq!(path(&runs, 1).last(), Some(&S::Succeeded));
    }
    // Queued –cancel→ Cancelled: the worker parked before it takes the run.
    {
        let (hook, parked, go) = park_when(|_, p| p == Point::BeforeStart);
        let runs = runs_with(Some(hook), 30_000);
        let w = world(Some(runs.clone()));
        let o = open(&w);
        let run = start(&w, &o, 1, &ALL).unwrap();
        parked.recv_timeout(Duration::from_secs(10)).unwrap();
        let c = call(&w.w, "stats_desc_cancel", json!({ "runId": run })).unwrap();
        assert_eq!(c["state"], "cancelled");
        go.send(()).unwrap();
        std::thread::sleep(Duration::from_millis(50));
        assert_eq!(
            path(&runs, run),
            [
                S::Draft,
                S::Validating,
                S::Validated,
                S::Queued,
                S::Cancelled
            ]
        );
        assert_eq!(
            call(&w.w, "stats_desc_status", json!({ "runId": run })).unwrap()["columnsDone"],
            0
        );
        note(&path(&runs, run));
    }
    // Running –cancel→ Cancelled, and Running –time_out→ TimedOut.
    {
        let (hook, parked, go) = park_when(|_, p| p == Point::Column(1));
        let runs = runs_with(Some(hook), 30_000);
        let w = world(Some(runs.clone()));
        let o = open(&w);
        let run = start(&w, &o, 1, &ALL).unwrap();
        parked.recv_timeout(Duration::from_secs(10)).unwrap();
        call(&w.w, "stats_desc_cancel", json!({ "runId": run })).unwrap();
        go.send(()).unwrap();
        until_terminal(&w.w, run);
        note(&path(&runs, run));
    }
    {
        let runs = runs_with(None, 0);
        let w = world(Some(runs.clone()));
        let o = open(&w);
        let run = start(&w, &o, 1, &ALL).unwrap();
        until_terminal(&w.w, run);
        note(&path(&runs, run));
    }
    // Running –fail→ Failed: the work panics, and the record still ends.
    {
        let hook: CheckpointHook = Arc::new(|_, p| {
            if p == Point::Column(1) {
                panic!("a deliberate panic in the describe loop");
            }
        });
        let runs = runs_with(Some(hook), 30_000);
        let w = world(Some(runs.clone()));
        let o = open(&w);
        let run = start(&w, &o, 1, &ALL).unwrap();
        let s = until_terminal(&w.w, run);
        assert_eq!(s["state"], "failed");
        assert_eq!(
            path(&runs, run),
            [
                S::Draft,
                S::Validating,
                S::Validated,
                S::Queued,
                S::Running,
                S::Failed
            ]
        );
        assert_eq!(
            refusal(&call(&w.w, "stats_desc_result", json!({ "runId": run })).unwrap_err())["code"],
            "run.no_result"
        );
        // The next run is not blocked by it.
        assert!(start(&w, &o, 2, &ALL).is_ok());
        note(&path(&runs, run));
    }

    let expected = [
        "draft -> validating",
        "queued -> cancelled",
        "queued -> running",
        "running -> cancelled",
        "running -> failed",
        "running -> succeeded",
        "running -> timed_out",
        "validated -> queued",
        "validating -> needs_user_input",
        "validating -> refused",
        "validating -> validated",
    ];
    assert_eq!(
        edges.iter().map(String::as_str).collect::<Vec<_>>(),
        expected
    );
}

/* ---- metamorphic, through the real parser ----------------------------------- */

/// Opens `bytes` as `name`, runs every column, and returns the result's
/// columns, its output hash and the CSV export.
fn describe_file(w: &World, name: &str, bytes: &[u8]) -> (Value, Value, Vec<u8>) {
    let p = w._dir.write(name, bytes);
    let o = call(
        &w.w,
        "stats_desc_open",
        json!({ "path": p.to_string_lossy() }),
    )
    .unwrap_or_else(|e| panic!("{name}: {e}"));
    let run = start(w, &o, 1, &ALL).unwrap();
    assert_eq!(until_terminal(&w.w, run)["state"], "succeeded", "{name}");
    let r = call(&w.w, "stats_desc_result", json!({ "runId": run })).unwrap();
    // Not vacuous: every column was read, with its header and values.
    assert_eq!(
        r["columns"].as_array().map(Vec::len),
        Some(4),
        "{name}: {r}"
    );
    assert_eq!(r["columns"][0]["numeric"]["header"], "score", "{name}");
    assert_eq!(r["columns"][3]["numeric"]["missing"], 1, "{name}");
    let e = call(
        &w.w,
        "stats_desc_export",
        json!({ "runId": run, "format": "csv" }),
    )
    .unwrap();
    call(&w.w, "stats_desc_close", json!({ "slot": o["slot"] })).unwrap();
    (
        r["columns"].clone(),
        r["manifest"]["outputHash"].clone(),
        serde_json::from_value(e["bytes"].clone()).unwrap(),
    )
}

#[test]
fn the_same_table_as_csv_tsv_and_xlsx_gives_byte_identical_results() {
    let w = world(None);
    let csv = describe_file(&w, "t.csv", &csv());
    let tsv = describe_file(&w, "t.tsv", &delimited(&HEADERS, &rows(), "\t", "\n"));
    let xlsx = describe_file(&w, "t.xlsx", &xlsx(&HEADERS, &rows()));
    assert_eq!(csv, tsv, "csv and tsv differ");
    assert_eq!(csv, xlsx, "csv and xlsx differ");
}

#[test]
fn a_bom_and_crlf_line_endings_change_nothing() {
    let w = world(None);
    let plain = describe_file(&w, "plain.csv", &csv());
    let mut bom = b"\xEF\xBB\xBF".to_vec();
    bom.extend(csv());
    let crlf = delimited(&HEADERS, &rows(), ",", "\r\n");
    let mut both = b"\xEF\xBB\xBF".to_vec();
    both.extend(&crlf);
    assert_eq!(
        plain,
        describe_file(&w, "bom.csv", &bom),
        "a BOM changed the result"
    );
    assert_eq!(
        plain,
        describe_file(&w, "crlf.csv", &crlf),
        "CRLF changed the result"
    );
    assert_eq!(
        plain,
        describe_file(&w, "both.csv", &both),
        "a BOM and CRLF changed the result"
    );
}
