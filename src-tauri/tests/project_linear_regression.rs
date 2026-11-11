//! **The linear regression in a project (Research OS, Phase 9A-2, decision 686).** Through
//! the commands' plain bodies, as their Tauri wrappers call them, over a root and a workspace
//! session the test makes, on `project_related_ranks.rs`'s pattern: a regression of three
//! predictors with an exclusion, a perfect fit of one predictor, and a collinear model's
//! failed run, saved; the project closed and opened; each run read back, the two results run
//! again and the rerun `Identical`, and the report's lines: the outcome's column, the
//! predictors in the order chosen and the codes' count. Every file is synthetic and in a
//! folder the test creates and removes.

mod project_support;

use std::time::{Duration, Instant};

use app_lib::project_commands::{self as pc, RerunProgress, RerunView, SaveTarget};
use app_lib::runs::RunKind;
use gaply_core::contracts::RunState;
use gaply_core::research_project::RerunComparison;
use serde_json::Value;

use project_support::*;

/// Ten rows: y, three predictors, and `copy` equal to x1 + x2 for the collinear model. The
/// fourth row has no x2 and no copy, so each model that reads either leaves it out.
const DATA: &[u8] = b"y,x1,x2,x3,copy\n\
3.1,1,2.5,7,3.5\n4.7,2,1.25,3,3.25\n6.2,3,4,8,7\n5.9,4,,6,\n8.4,5,3.5,2,8.5\n\
7.7,6,0.75,9,6.75\n10.3,7,5,4,12\n9.8,8,2.25,1,10.25\n12.6,9,3,5,12\n11.9,10,4.5,10,14.5\n";

/// y = 2 + 3x exactly at x = 0, 3 and 4: every residual is zero.
const PERFECT: &[u8] = b"x,y\n0,2\n3,11\n4,14\n";

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

fn settled(w: &World) -> RerunView {
    let start = Instant::now();
    loop {
        if let Some(r) = pc::current(&w.ws).unwrap().rerun {
            if r.status != RerunProgress::Going {
                return r;
            }
        }
        assert!(
            start.elapsed() < Duration::from_secs(30),
            "the rerun never settled"
        );
        std::thread::sleep(Duration::from_millis(2));
    }
}

fn comparison(w: &World, id: &str, n: u64) -> Option<RerunComparison> {
    pc::runs(&w.root(), &w.ws, id)
        .unwrap()
        .into_iter()
        .find(|r| r.stored_run_id == n)
        .expect("the stored run")
        .comparison
}

/// A stored result's primary outputs.
fn outputs(w: &World, id: &str, n: u64) -> Value {
    let back = pc::run_result(&w.root(), &w.ws, id, n).expect("read back");
    assert_eq!(back.state, RunState::Succeeded, "run {n}");
    let bundle: Value = serde_json::from_str(back.bundle_json.as_deref().expect("a result")).unwrap();
    bundle["primaryOutputs"].clone()
}

#[test]
fn linear_regressions_are_saved_read_back_run_again_identical_and_reported_in_the_order_chosen() {
    let w = World::new("9a2-project");
    let slot = w.open("regression.csv", DATA);
    let perfect_slot = w.open("perfect.csv", PERFECT);
    let three = w.linear_regression(slot, 1, &[4, 2, 3], vec![]);
    let perfect = w.linear_regression(perfect_slot, 2, &[1], vec![-99.0]);
    let collinear = w.linear_regression(slot, 1, &[2, 3, 5], vec![]);
    assert_eq!(w.wait_new_kind(three), RunState::Succeeded);
    assert_eq!(w.wait_new_kind(perfect), RunState::Succeeded);
    // A run's own refusal is recorded as failed.
    assert_eq!(w.wait_new_kind(collinear), RunState::Failed);

    let id = pc::save_run(&w.root(), &w.ws, three, new_project("Regression"))
        .expect("the regression saves")
        .project_id;
    pc::save_run(&w.root(), &w.ws, perfect, into_open(&id)).expect("the perfect fit saves");
    pc::save_run(&w.root(), &w.ws, collinear, into_open(&id)).expect("the failed run saves");
    pc::close(&w.root(), &w.ws).expect("closes");
    pc::open(&w.root(), &w.ws, &id).expect("opens");

    // Read back: each stored result is the model it was, its predictors in the list's order.
    let first = outputs(&w, &id, 1);
    assert_eq!(first["outcome"], "C1");
    let predictors: Vec<&str> = first["predictors"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["column"].as_str().unwrap())
        .collect();
    assert_eq!(predictors, ["C4", "C2", "C3"], "the list's order, never the file's");
    assert_eq!(first["rowsUsed"], 9);
    assert_eq!(first["exclusions"][0]["code"], "stats.linear_regression.missing_value");
    assert_eq!(first["exclusions"][0]["params"]["column"], 3);
    let second = outputs(&w, &id, 2);
    assert_eq!(
        second["intercept"]["standardError"]["not_computable"],
        "stats.linear_regression.perfect_fit"
    );
    let back = pc::run_result(&w.root(), &w.ws, &id, 3).expect("read back");
    assert_eq!(back.state, RunState::Failed);
    assert!(back.bundle_json.is_none() && back.manifest_json.is_none());
    let failure: Value = serde_json::from_str(back.failure_json.as_deref().expect("its failure")).unwrap();
    assert_eq!(failure["failure"]["code"], "stats.linear_regression.predictors_collinear");
    assert_eq!(failure["failure"]["params"]["column"], 5, "the copy, named by its position");

    for (stored, new) in [(1, 4), (2, 5)] {
        let started = pc::rerun(&w.root(), &w.ws, stored).expect("the rerun starts");
        assert_eq!(started.kind, RunKind::LinearRegression);
        let r = settled(&w);
        assert_eq!((r.status, r.stored_run_id), (RerunProgress::Saved, Some(new)));
        assert_eq!(
            comparison(&w, &id, new),
            Some(RerunComparison::Identical),
            "run {stored} again"
        );
    }

    let r = String::from_utf8(pc::report(&w.root(), &w.ws, &id).expect("the report").bytes)
        .expect("UTF-8");
    for n in [1, 2] {
        let sentence =
            format!("Repeats run {n}. Its result is identical, byte for byte, to run {n}'s.");
        assert!(r.contains(&sentence), "the report repeats run {n}:\n{r}");
    }
    for (line, times) in [
        ("- Predictors: columns 4, 2, 3, by position, in the order chosen", 2),
        ("- Predictors: column 1, by position", 2),
        ("- Predictors: columns 2, 3, 5, by position, in the order chosen", 1),
        ("- Column 1: outcome", 3),
        ("- Column 2: outcome", 2),
        ("- Missing-value codes: 0", 3),
        (
            "- Missing-value codes: 1 (their values are in the settings fingerprint)",
            2,
        ),
    ] {
        assert_eq!(r.matches(line).count(), times, "{line}:\n{r}");
    }
    // No header reaches the report (a rough check, as project_related_ranks.rs's; the
    // privacy matrix checks headers with sentinels).
    for text in ["x1", "x2", "x3", "copy"] {
        assert!(
            !r.contains(&format!(" {text}\n")) && !r.contains(&format!(": {text}")),
            "{text} in the report"
        );
    }
}
