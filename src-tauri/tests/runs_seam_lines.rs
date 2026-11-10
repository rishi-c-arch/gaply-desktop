//! **The run manager's tracing lines, per kind and shared (Research OS,
//! Phase 8C, difference 11 of the step).** One test in its own binary, so
//! the global subscriber sees the worker thread's events and no other test's.
//!
//! * A summary whose work panics, with no cancel: `stats_desc: the describe
//!   loop panicked`, and the run ends `Failed`.
//! * A t-test whose work panics, with no cancel: `stats_desc: the t-test
//!   panicked`, and the run ends `Failed`.
//! * A one-sample or a paired t-test whose work panics, with no cancel
//!   (Phase 8D-2a, decision 263): `stats_desc: the one-sample t-test
//!   panicked` or `stats_desc: the paired t-test panicked`, and the run ends
//!   `Failed`.
//! * A one-way ANOVA whose work panics, with no cancel (Phase 8G-2, decision
//!   318): `stats_desc: the one-way ANOVA panicked`, and the run ends
//!   `Failed`.
//! * Correlations whose work panics, with no cancel (Phase 8H-2, decision
//!   355): `stats_desc: the correlations panicked`, and the run ends
//!   `Failed`.
//! * Cronbach's alpha whose work panics, with no cancel (Phase 8I-2,
//!   decision 380): `stats_desc: Cronbach's alpha panicked`, and the run
//!   ends `Failed`.
//! * Crosstabs whose work panics, with no cancel (Phase 8X-1, decision 580):
//!   `stats_desc: crosstabs panicked`, and the run ends `Failed`.
//! * The rank tests whose work panics, with no cancel (Phase 8Y-1, decision
//!   607): `stats_desc: the rank tests panicked`, and the run ends `Failed`.
//! * Any kind whose work panics after a cancel was accepted: the shared
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
use gaply_core::stats_alpha::{self, AlphaParams};
use gaply_core::stats_anova::{self, AnovaParams, PostHoc};
use gaply_core::stats_corr::{self, Coefficient, CorrParams, MissingHandling};
use gaply_core::stats_desc::{self, Alias, DescribeParams};
use gaply_core::stats_l1::dataset::ColumnAlias;
use gaply_core::stats_onesample::{self, OneSampleParams, PairedParams};
use gaply_core::stats_ranks::{self, IndependentRanksParams};
use gaply_core::stats_related_ranks::{self, RelatedRanksParams};
use gaply_core::stats_ttest::{self, TTestParams, Variant};
use gaply_core::stats_xtab::{self, CrosstabsParams};

const CSV: &[u8] = b"x,g\n1,a\n2,a\n4,a\n3,b\n5,b\n8,b\n";
/// Two numeric columns, for the one-sample and paired t-tests.
const PAIRS: &[u8] = b"a,b\n1,2\n2,5\n4,5\n3,7\n5,6\n8,9\n";

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

/// The run's kind, so the hook cancels through that kind's own cancel.
#[derive(Clone, Copy)]
enum Kind {
    Summary,
    Ttest,
    OneSample,
    Paired,
    Anova,
    Correlations,
    CronbachAlpha,
    Crosstabs,
    IndependentRanks,
    RelatedRanks,
}

/// A manager whose hook panics at `Column(0)`, after cancelling the run
/// through its own kind's cancel when asked.
fn panicking(what: AtColumn0, kind: Kind) -> Arc<Runs> {
    let me: Arc<Mutex<Option<Arc<Runs>>>> = Arc::default();
    let hook_me = Arc::clone(&me);
    let hook: CheckpointHook = Arc::new(move |run: RunId, p| {
        if p != Point::Column(0) {
            return;
        }
        if let AtColumn0::CancelThenPanic = what {
            let runs = hook_me.lock().unwrap().clone().expect("the manager");
            match kind {
                Kind::Summary => drop(runs.cancel(run).expect("cancelled")),
                Kind::Ttest => drop(runs.ttest_cancel(run).expect("cancelled")),
                Kind::OneSample => drop(runs.one_sample_cancel(run).expect("cancelled")),
                Kind::Paired => drop(runs.paired_cancel(run).expect("cancelled")),
                Kind::Anova => drop(runs.anova_cancel(run).expect("cancelled")),
                Kind::Correlations => drop(runs.corr_cancel(run).expect("cancelled")),
                Kind::CronbachAlpha => drop(runs.alpha_cancel(run).expect("cancelled")),
                Kind::Crosstabs => drop(runs.xtab_cancel(run).expect("cancelled")),
                Kind::IndependentRanks => drop(runs.ranks_cancel(run).expect("cancelled")),
                Kind::RelatedRanks => drop(runs.related_ranks_cancel(run).expect("cancelled")),
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
    let anova = stats_anova::spec(
        OperationId(5),
        &dataset,
        AnovaParams {
            outcome: Alias(ColumnAlias(1)),
            group: Alias(ColumnAlias(2)),
            levels: None,
            post_hoc: vec![PostHoc::Tukey],
            missing_codes: vec![],
        },
    );
    let ranks = stats_ranks::spec(
        OperationId(9),
        &dataset,
        IndependentRanksParams {
            outcome: Alias(ColumnAlias(1)),
            group: Alias(ColumnAlias(2)),
            levels: None,
            missing_codes: vec![],
        },
    );
    let xtab = stats_xtab::spec(
        OperationId(8),
        &dataset,
        CrosstabsParams {
            rows: Alias(ColumnAlias(1)),
            columns: Alias(ColumnAlias(2)),
            missing_codes: vec![],
        },
    );
    let slot = slots
        .get(slots.insert("synthetic".into(), dataset))
        .unwrap();
    let pairs = load_dataset_from_bytes(PAIRS, DataFormat::Csv).expect("the csv opens");
    let one_sample = stats_onesample::one_sample_spec(
        OperationId(3),
        &pairs,
        OneSampleParams {
            outcome: Alias(ColumnAlias(1)),
            test_value: Some(2.5),
            missing_codes: vec![],
        },
    );
    let paired = stats_onesample::paired_spec(
        OperationId(4),
        &pairs,
        PairedParams {
            first: Alias(ColumnAlias(1)),
            second: Alias(ColumnAlias(2)),
            missing_codes: vec![],
        },
    );
    let corr = stats_corr::spec(
        OperationId(6),
        &pairs,
        CorrParams {
            variables: vec![Alias(ColumnAlias(1)), Alias(ColumnAlias(2))],
            coefficients: vec![Coefficient::Pearson],
            missing: MissingHandling::Pairwise,
            missing_codes: vec![],
        },
    );
    let alpha = stats_alpha::spec(
        OperationId(7),
        &pairs,
        AlphaParams {
            items: vec![Alias(ColumnAlias(1)), Alias(ColumnAlias(2))],
            reversed: vec![],
            scale: None,
            missing_codes: vec![],
        },
    );
    let related = stats_related_ranks::spec(
        OperationId(10),
        &pairs,
        RelatedRanksParams {
            columns: vec![Alias(ColumnAlias(1)), Alias(ColumnAlias(2))],
            missing_codes: vec![],
        },
    );
    let pair_slot = slots
        .get(slots.insert("synthetic pairs".into(), pairs))
        .unwrap();

    let mut ends = Vec::new();
    for what in [AtColumn0::Panic, AtColumn0::CancelThenPanic] {
        let runs = panicking(what, Kind::Summary);
        let run = runs.start(Arc::clone(&slot), summary.clone()).unwrap();
        wait_for("the summary ended", || {
            runs.snapshot(run).unwrap().record.state.is_terminal()
        });
        ends.push(runs.snapshot(run).unwrap().record.state);

        let runs = panicking(what, Kind::Ttest);
        let run = runs.start_ttest(Arc::clone(&slot), ttest.clone()).unwrap();
        wait_for("the t-test ended", || {
            runs.ttest_snapshot(run).unwrap().record.state.is_terminal()
        });
        ends.push(runs.ttest_snapshot(run).unwrap().record.state);

        let runs = panicking(what, Kind::OneSample);
        let run = runs
            .start_one_sample(Arc::clone(&pair_slot), one_sample.clone())
            .unwrap();
        wait_for("the one-sample t-test ended", || {
            runs.one_sample_snapshot(run)
                .unwrap()
                .record
                .state
                .is_terminal()
        });
        ends.push(runs.one_sample_snapshot(run).unwrap().record.state);

        let runs = panicking(what, Kind::Paired);
        let run = runs
            .start_paired(Arc::clone(&pair_slot), paired.clone())
            .unwrap();
        wait_for("the paired t-test ended", || {
            runs.paired_snapshot(run)
                .unwrap()
                .record
                .state
                .is_terminal()
        });
        ends.push(runs.paired_snapshot(run).unwrap().record.state);

        let runs = panicking(what, Kind::Anova);
        let run = runs.start_anova(Arc::clone(&slot), anova.clone()).unwrap();
        wait_for("the one-way ANOVA ended", || {
            runs.anova_snapshot(run).unwrap().record.state.is_terminal()
        });
        ends.push(runs.anova_snapshot(run).unwrap().record.state);

        let runs = panicking(what, Kind::Correlations);
        let run = runs
            .start_corr(Arc::clone(&pair_slot), corr.clone())
            .unwrap();
        wait_for("the correlations ended", || {
            runs.corr_snapshot(run).unwrap().record.state.is_terminal()
        });
        ends.push(runs.corr_snapshot(run).unwrap().record.state);

        let runs = panicking(what, Kind::CronbachAlpha);
        let run = runs
            .start_alpha(Arc::clone(&pair_slot), alpha.clone())
            .unwrap();
        wait_for("Cronbach's alpha ended", || {
            runs.alpha_snapshot(run).unwrap().record.state.is_terminal()
        });
        ends.push(runs.alpha_snapshot(run).unwrap().record.state);

        let runs = panicking(what, Kind::Crosstabs);
        let run = runs.start_xtab(Arc::clone(&slot), xtab.clone()).unwrap();
        wait_for("crosstabs ended", || {
            runs.xtab_snapshot(run).unwrap().record.state.is_terminal()
        });
        ends.push(runs.xtab_snapshot(run).unwrap().record.state);

        let runs = panicking(what, Kind::IndependentRanks);
        let run = runs.start_ranks(Arc::clone(&slot), ranks.clone()).unwrap();
        wait_for("the rank tests ended", || {
            runs.ranks_snapshot(run).unwrap().record.state.is_terminal()
        });
        ends.push(runs.ranks_snapshot(run).unwrap().record.state);

        let runs = panicking(what, Kind::RelatedRanks);
        let run = runs
            .start_related_ranks(Arc::clone(&pair_slot), related.clone())
            .unwrap();
        wait_for("the related rank tests ended", || {
            runs.related_ranks_snapshot(run)
                .unwrap()
                .record
                .state
                .is_terminal()
        });
        ends.push(runs.related_ranks_snapshot(run).unwrap().record.state);
    }
    let _ = std::panic::take_hook();

    assert_eq!(
        ends,
        [
            RunState::Failed,
            RunState::Failed,
            RunState::Failed,
            RunState::Failed,
            RunState::Failed,
            RunState::Failed,
            RunState::Failed,
            RunState::Failed,
            RunState::Failed,
            RunState::Failed,
            RunState::Cancelled,
            RunState::Cancelled,
            RunState::Cancelled,
            RunState::Cancelled,
            RunState::Cancelled,
            RunState::Cancelled,
            RunState::Cancelled,
            RunState::Cancelled,
            RunState::Cancelled,
            RunState::Cancelled
        ]
    );
    assert_eq!(events(&lines, "stats_desc: the describe loop panicked"), 1);
    assert_eq!(events(&lines, "stats_desc: the t-test panicked"), 1);
    assert_eq!(
        events(&lines, "stats_desc: the one-sample t-test panicked"),
        1
    );
    assert_eq!(events(&lines, "stats_desc: the paired t-test panicked"), 1);
    assert_eq!(events(&lines, "stats_desc: the one-way ANOVA panicked"), 1);
    assert_eq!(events(&lines, "stats_desc: the correlations panicked"), 1);
    assert_eq!(events(&lines, "stats_desc: Cronbach's alpha panicked"), 1);
    assert_eq!(events(&lines, "stats_desc: crosstabs panicked"), 1);
    assert_eq!(events(&lines, "stats_desc: the rank tests panicked"), 1);
    assert_eq!(events(&lines, "stats_desc: the related rank tests panicked"), 1);
    assert_eq!(events(&lines, "stats_desc: the run's work panicked"), 10);
    assert_eq!(events(&lines, "stats_desc: run ended"), 20);
    assert_eq!(events(&lines, "stats_desc: a run event had no edge"), 0);
}
