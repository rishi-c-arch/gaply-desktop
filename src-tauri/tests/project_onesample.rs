//! **The one-sample and paired t-tests in a project (Research OS, Phase
//! 8D-2a, decision 264).** Through the commands' plain bodies, as their Tauri
//! wrappers call them, over a root and a workspace session the test makes:
//!
//! * a one-sample and a paired result saved, the project closed and opened,
//!   each run again, the rerun `Identical`, and the report saying so in
//!   decision 189's words, with the test value's line and the roles' words;
//! * the test value read back: six values, from 0.1 to the largest double,
//!   each saved, run again `Identical`, and printed in the report exactly as
//!   the CSV export writes it;
//! * a test value serde_json's parser once read one unit in the last place
//!   off, before `float_roundtrip` was turned on (Phase 8D-2b, decision 267):
//!   its rerun is now identical, and its report line equals its CSV cell.
//!
//! Every file is synthetic and in a folder the test creates and removes.

mod project_support;

use std::time::{Duration, Instant};

use app_lib::project_commands::{self as pc, RerunProgress, RerunView, SaveTarget};
use app_lib::runs::RunKind;
use app_lib::stats_desc_commands::ExportFormat;
use app_lib::stats_onesample_commands as onesample;
use gaply_core::research_project::RerunComparison;

use project_support::*;

/// Two numeric columns whose SDs are above 1 and whose differences vary.
const PAIRS: &[u8] = b"a,b\n10,12.5\n20,21\n30,33.25\n40,41\n50,54.5\n";

fn new_project(name: &str) -> SaveTarget {
    SaveTarget::NewProject {
        name: name.to_string(),
    }
}

fn into_open(id: &str) -> SaveTarget {
    SaveTarget::OpenProject {
        project_id: id.to_string(),
    }
}

/// The rerun as `project_current` shows it, once it is no longer going.
fn settled(w: &World) -> RerunView {
    let start = Instant::now();
    loop {
        if let Some(r) = pc::current(&w.ws).unwrap().rerun {
            if r.status != RerunProgress::Going {
                return r;
            }
        }
        assert!(
            start.elapsed() < Duration::from_secs(20),
            "the rerun never settled"
        );
        std::thread::sleep(Duration::from_millis(2));
    }
}

fn report(w: &World, id: &str) -> String {
    String::from_utf8(pc::report(&w.root(), &w.ws, id).expect("the report").bytes).expect("UTF-8")
}

fn comparison(w: &World, id: &str, n: u64) -> Option<RerunComparison> {
    pc::runs(&w.root(), &w.ws, id)
        .unwrap()
        .into_iter()
        .find(|r| r.stored_run_id == n)
        .expect("the stored run")
        .comparison
}

/// The `test_value` cell of a one-sample run's CSV export, as text.
fn csv_test_value(w: &World, run: u64) -> String {
    let e = w.engine();
    let bytes = onesample::export_one_sample(&e.runs, run, ExportFormat::Csv)
        .unwrap_or_else(|_| panic!("run {run} exports"))
        .bytes;
    let text = String::from_utf8(bytes).unwrap();
    let line = text
        .lines()
        .find(|l| l.starts_with("\"test_value\","))
        .expect("a test_value row");
    line.split(',').nth(2).expect("its value").to_string()
}

/// The text after "Test value: " on each such line of the report, in order.
fn report_test_values(report: &str) -> Vec<String> {
    report
        .lines()
        .filter_map(|l| l.strip_prefix("- Test value: "))
        .map(str::to_string)
        .collect()
}

#[test]
fn a_one_sample_and_a_paired_result_are_saved_run_again_identical_and_reported() {
    let w = World::new("8d2a-project");
    let slot = w.open("pairs.csv", PAIRS);
    let one = w.one_sample(slot, 1, 12.5);
    let pair = w.paired(slot, 1, 2);
    let id = pc::save_run(&w.root(), &w.ws, one, new_project("Paired and one-sample"))
        .expect("the one-sample result saves")
        .project_id;
    pc::save_run(&w.root(), &w.ws, pair, into_open(&id)).expect("the paired result saves");
    // Closed and opened again: the reruns read the project's own copy.
    pc::close(&w.root(), &w.ws).expect("closes");
    pc::open(&w.root(), &w.ws, &id).expect("opens");

    let a = pc::rerun(&w.root(), &w.ws, 1).expect("the one-sample rerun starts");
    assert_eq!(a.kind, RunKind::OneSample);
    let ra = settled(&w);
    assert_eq!(
        (ra.status, ra.stored_run_id),
        (RerunProgress::Saved, Some(3))
    );
    let b = pc::rerun(&w.root(), &w.ws, 2).expect("the paired rerun starts");
    assert_eq!(b.kind, RunKind::Paired);
    let rb = settled(&w);
    assert_eq!(
        (rb.status, rb.stored_run_id),
        (RerunProgress::Saved, Some(4))
    );
    assert_eq!(comparison(&w, &id, 3), Some(RerunComparison::Identical));
    assert_eq!(comparison(&w, &id, 4), Some(RerunComparison::Identical));

    let r = report(&w, &id);
    for n in [1, 2] {
        let sentence =
            format!("Repeats run {n}. Its result is identical, byte for byte, to run {n}'s.");
        assert!(r.contains(&sentence), "the report repeats run {n}:\n{r}");
    }
    assert_eq!(
        report_test_values(&r),
        ["12.5", "12.5"],
        "run 1 and its rerun"
    );
    for line in [
        "- Column 1: outcome",
        "- Column 1: first",
        "- Column 2: second",
    ] {
        assert!(r.lines().any(|l| l == line), "{line}:\n{r}");
    }
    // The provenance export renders too.
    pc::provenance(&w.root(), &w.ws, &id).expect("the provenance export");
}

/// Decision 264's round trip: each test value is printed in the report as the
/// CSV export writes it, and its saved spec reads back to exactly its bytes,
/// so its rerun is identical. Measured in 8D-2a: all six read back exactly
/// under serde_json 1.0.150 even without `float_roundtrip`, which 8D-2b
/// turned on (decision 267).
#[test]
fn each_test_value_reads_back_exactly_and_prints_as_the_csv_does() {
    const VALUES: [f64; 6] = [
        0.1,
        0.30000000000000004,
        1e-7,
        -123456.789,
        2.2250738585072014e-308,
        1.7976931348623157e308,
    ];
    let w = World::new("8d2a-test-values");
    // SD above 1, so every run succeeds.
    let slot = w.open("spread.csv", b"x\n10\n20\n30\n40\n50\n");
    let mut csv = Vec::new();
    let mut id = String::new();
    for (i, v) in VALUES.iter().enumerate() {
        let run = w.one_sample(slot, 1, *v);
        csv.push(csv_test_value(&w, run));
        let target = if i == 0 {
            new_project("Test values")
        } else {
            into_open(&id)
        };
        id = pc::save_run(&w.root(), &w.ws, run, target)
            .expect("saves")
            .project_id;
    }
    assert_eq!(
        csv,
        [
            "0.1",
            "0.30000000000000004",
            "0.0000001",
            "-123456.789",
            &format!("{}", VALUES[4]),
            &format!("{}", VALUES[5])
        ],
        "the CSV writes Rust's shortest form"
    );
    for n in 1..=VALUES.len() as u64 {
        pc::rerun(&w.root(), &w.ws, n).expect("the rerun starts");
        let r = settled(&w);
        assert_eq!(
            (r.status, r.code),
            (RerunProgress::Saved, None),
            "test value {}",
            VALUES[n as usize - 1]
        );
        let rerun = r.stored_run_id.expect("saved");
        assert_eq!(
            comparison(&w, &id, rerun),
            Some(RerunComparison::Identical),
            "test value {}",
            VALUES[n as usize - 1]
        );
    }
    // Each run's line, then each rerun's, in the stored order.
    let lines = report_test_values(&report(&w, &id));
    let mut want = csv.clone();
    want.extend(csv.iter().cloned());
    assert_eq!(lines, want, "each report line is its CSV cell, exactly");
}

/// A value serde_json read one unit in the last place off while it was
/// built without `float_roundtrip` (8D-2a: the rerun was refused and the
/// report printed …728 where the CSV printed …729). With the feature on
/// (Phase 8D-2b, decision 267) the saved spec reads back to its own bytes,
/// the rerun is identical, and the report line is the CSV cell.
#[test]
fn a_test_value_serde_json_once_read_one_ulp_off_reruns_identical() {
    const V: f64 = 0.10922441798986729;
    let w = World::new("8d2a-ulp-off");
    let slot = w.open("spread.csv", b"x\n10\n20\n30\n40\n50\n");
    let run = w.one_sample(slot, 1, V);
    let cell = csv_test_value(&w, run);
    let id = pc::save_run(&w.root(), &w.ws, run, new_project("One ulp"))
        .expect("saves")
        .project_id;
    pc::rerun(&w.root(), &w.ws, 1).expect("the rerun starts");
    let r = settled(&w);
    assert_eq!((r.status, r.code), (RerunProgress::Saved, None));
    let rerun = r.stored_run_id.expect("saved");
    assert_eq!(comparison(&w, &id, rerun), Some(RerunComparison::Identical));
    let lines = report_test_values(&report(&w, &id));
    println!("CSV cell: {cell}\nreport lines: {lines:?}");
    assert_eq!(cell, "0.10922441798986729");
    assert_eq!(
        lines,
        [cell.clone(), cell],
        "the run's line and its rerun's"
    );
}
