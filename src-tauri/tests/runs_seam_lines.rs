//! **The run manager's tracing lines, per kind and shared (Research OS,
//! Phase 8C, difference 11 of the step).** One test in its own binary, so
//! the global subscriber sees the worker thread's events and no other test's.
//!
//! * A summary whose work panics, with no cancel: `stats_desc: the describe
//!   loop panicked`, and the run ends `Failed`.
//! * A t-test whose work panics, with no cancel: `stats_desc: the t-test
//!   panicked`, and the run ends `Failed`.
//! * Either kind whose work panics after a cancel was accepted: the shared
//!   `stats_desc: the run's work panicked`, and the run ends `Cancelled`.
//! * Every run that ends: the shared `stats_desc: run ended`.
//!
//! The panic comes from the check-point hook, which the work calls inside its
//! `catch_unwind`. Every event's target is the run manager's module.
//! The shared `stats_desc: a run event had no edge` has no producer a test
//! can reach through the public API, and is not asserted here.

mod l1_support;

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use app_lib::runs::{CheckpointHook, Clock, DatasetSlots, Point, Runs, RUN_CEILING_MS};
use app_lib::stats_l1_ingest::load_dataset_from_bytes;
use gaply_core::contracts::{OperationId, RunId, RunState};
use gaply_core::research_project::DataFormat;
use gaply_core::stats_desc::{self, Alias, DescribeParams};
use gaply_core::stats_l1::dataset::ColumnAlias;
use gaply_core::stats_ttest::{self, TTestParams, Variant};

const CSV: &[u8] = b"x,g\n1,a\n2,a\n4,a\n3,b\n5,b\n8,b\n";

struct Ticks(AtomicU64);

impl Clock for Ticks {
    fn now_ms(&self) -> u64 {
        self.0.fetch_add(1, Ordering::SeqCst)
    }
}

/// What a hook does when a run reaches `Column(0)`.
#[derive(Clone, Copy)]
enum AtColumn0 {
    Panic,
    CancelThenPanic,
}

fn wait_for(what: &str, done: impl Fn() -> bool) {
    let start = Instant::now();
    while !done() {
        assert!(start.elapsed() < Duration::from_secs(10), "{what}");
        std::thread::sleep(Duration::from_millis(1));
    }
}

/// A manager whose hook panics at `Column(0)`, after cancelling the run
/// through its own kind's cancel when asked.
fn panicking(what: AtColumn0, ttest: bool) -> Arc<Runs> {
    let me: Arc<Mutex<Option<Arc<Runs>>>> = Arc::default();
    let hook_me = Arc::clone(&me);
    let hook: CheckpointHook = Arc::new(move |run: RunId, p| {
        if p != Point::Column(0) {
            return;
        }
        if let AtColumn0::CancelThenPanic = what {
            let runs = hook_me.lock().unwrap().clone().expect("the manager");
            if ttest {
                runs.ttest_cancel(run).expect("cancelled");
            } else {
                runs.cancel(run).expect("cancelled");
            }
        }
        panic!("the test's hook panics at column 0");
    });
    let runs = Arc::new(Runs::with_hooks(
        Arc::new(Ticks(AtomicU64::new(0))),
        Some(hook),
        RUN_CEILING_MS,
    ));
    *me.lock().unwrap() = Some(Arc::clone(&runs));
    runs
}

fn events(lines: &Mutex<Vec<String>>, message: &str) -> usize {
    let want = format!("message={message} ");
    lines
        .lock()
        .unwrap()
        .iter()
        .filter(|l| l.starts_with("event app_lib::runs ") && l.contains(&want))
        .count()
}

#[test]
fn each_kind_says_its_own_panic_and_both_say_the_shared_lines() {
    let lines: Arc<Mutex<Vec<String>>> = Arc::default();
    tracing::subscriber::set_global_default(l1_support::Capture {
        lines: lines.clone(),
    })
    .expect("the only subscriber");
    tracing::callsite::rebuild_interest_cache();
    // The hook's panics are expected; keep their messages out of the output.
    std::panic::set_hook(Box::new(|_| {}));

    let slots = DatasetSlots::default();
    let dataset = load_dataset_from_bytes(CSV, DataFormat::Csv).expect("the csv opens");
    let profile = stats_desc::profile(&dataset);
    let summary = stats_desc::spec(
        OperationId(1),
        &dataset,
        DescribeParams::defaults_for(&profile),
    );
    let ttest = stats_ttest::spec(
        OperationId(2),
        &dataset,
        TTestParams {
            outcome: Alias(ColumnAlias(1)),
            group: Alias(ColumnAlias(2)),
            levels: Some([0, 1]),
            variant: Some(Variant::Welch),
            missing_codes: vec![],
        },
    );
    let slot = slots
        .get(slots.insert("synthetic".into(), dataset))
        .unwrap();

    let mut ends = Vec::new();
    for what in [AtColumn0::Panic, AtColumn0::CancelThenPanic] {
        let runs = panicking(what, false);
        let run = runs.start(Arc::clone(&slot), summary.clone()).unwrap();
        wait_for("the summary ended", || {
            runs.snapshot(run).unwrap().record.state.is_terminal()
        });
        ends.push(runs.snapshot(run).unwrap().record.state);

        let runs = panicking(what, true);
        let run = runs.start_ttest(Arc::clone(&slot), ttest.clone()).unwrap();
        wait_for("the t-test ended", || {
            runs.ttest_snapshot(run).unwrap().record.state.is_terminal()
        });
        ends.push(runs.ttest_snapshot(run).unwrap().record.state);
    }
    let _ = std::panic::take_hook();

    assert_eq!(
        ends,
        [
            RunState::Failed,
            RunState::Failed,
            RunState::Cancelled,
            RunState::Cancelled
        ]
    );
    assert_eq!(events(&lines, "stats_desc: the describe loop panicked"), 1);
    assert_eq!(events(&lines, "stats_desc: the t-test panicked"), 1);
    assert_eq!(events(&lines, "stats_desc: the run's work panicked"), 2);
    assert_eq!(events(&lines, "stats_desc: run ended"), 4);
    assert_eq!(events(&lines, "stats_desc: a run event had no edge"), 0);
}
