//! **The rank tests in a project (Research OS, Phase 8Y-2, decision 621).**
//! Through the commands' plain bodies, as their Tauri wrappers call them, over a
//! root and a workspace session the test makes, on `project_anova.rs`'s and
//! `project_xtab.rs`'s pattern: a Mann-Whitney result with a missing code, a
//! Kruskal-Wallis result, and a Mann-Whitney result on a subset of levels, saved,
//! the project closed and opened, each run read back, each run again, the rerun
//! `Identical`, and the report's lines: the outcome and the group by role, the
//! included levels and the codes' count. Every file is synthetic and in a folder
//! the test creates and removes.

mod project_support;

use std::time::{Duration, Instant};

use app_lib::project_commands::{self as pc, RerunProgress, RerunView, SaveTarget};
use app_lib::runs::RunKind;
use gaply_core::contracts::RunState;
use gaply_core::research_project::RerunComparison;

use project_support::*;

/// A numeric outcome holding -99 twice, a two-level arm and a four-level site.
const DATA: &[u8] = b"score,arm,site\n\
1.5,a,w\n2.25,a,w\n3.75,a,x\n-99,a,x\n\
4.5,a,y\n6.125,a,y\n7,a,z\n5.25,a,z\n\
9.5,b,w\n12.75,b,w\n8,b,x\n15.5,b,x\n\
-99,b,y\n3.5,b,y\n4.25,b,z\n2.75,b,z\n";

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

/// A stored result's test form, its group levels and its N.
fn shape(w: &World, id: &str, n: u64) -> (String, Vec<u64>, u64) {
    let back = pc::run_result(&w.root(), &w.ws, id, n).expect("read back");
    let bundle: serde_json::Value =
        serde_json::from_str(back.bundle_json.as_deref().expect("a result")).unwrap();
    let o = &bundle["primaryOutputs"];
    let test = o["test"].as_object().unwrap().keys().next().unwrap().clone();
    let levels = o["groups"]
        .as_array()
        .unwrap()
        .iter()
        .map(|g| g["level"].as_u64().unwrap())
        .collect();
    (test, levels, o["n"].as_u64().unwrap())
}

#[test]
fn rank_tests_are_saved_read_back_run_again_identical_and_reported() {
    let w = World::new("8y2-project");
    let slot = w.open("ranks.csv", DATA);
    // arm, -99 ticked: Mann-Whitney on 14 values.
    let mann_whitney = w.ranks(slot, 1, 2, None, vec![-99.0]);
    // site, no code: Kruskal-Wallis on 16 values.
    let kruskal_wallis = w.ranks(slot, 1, 3, None, vec![]);
    // site, levels x and z (indexes 1 and 3): Mann-Whitney on two of four.
    let subset = w.ranks(slot, 1, 3, Some(vec![1, 3]), vec![-99.0]);
    for run in [mann_whitney, kruskal_wallis, subset] {
        assert_eq!(w.wait_new_kind(run), RunState::Succeeded);
    }
    let id = pc::save_run(&w.root(), &w.ws, mann_whitney, new_project("Ranks"))
        .expect("the Mann-Whitney saves")
        .project_id;
    pc::save_run(&w.root(), &w.ws, kruskal_wallis, into_open(&id)).expect("the Kruskal-Wallis saves");
    pc::save_run(&w.root(), &w.ws, subset, into_open(&id)).expect("the subset saves");
    pc::close(&w.root(), &w.ws).expect("closes");
    pc::open(&w.root(), &w.ws, &id).expect("opens");

    // Read back: each stored result is the test it was.
    let mw = "mann_whitney".to_string();
    let kw = "kruskal_wallis".to_string();
    assert_eq!(shape(&w, &id, 1), (mw.clone(), vec![0, 1], 14));
    assert_eq!(shape(&w, &id, 2), (kw.clone(), vec![0, 1, 2, 3], 16));
    assert_eq!(shape(&w, &id, 3), (mw.clone(), vec![1, 3], 7));

    for (stored, new) in [(1, 4), (2, 5), (3, 6)] {
        let started = pc::rerun(&w.root(), &w.ws, stored).expect("the rerun starts");
        assert_eq!(started.kind, RunKind::IndependentRanks);
        let r = settled(&w);
        assert_eq!((r.status, r.stored_run_id), (RerunProgress::Saved, Some(new)));
        assert_eq!(
            comparison(&w, &id, new),
            Some(RerunComparison::Identical),
            "run {stored} again"
        );
    }
    // The rerun included the levels the run did, and no other.
    assert_eq!(shape(&w, &id, 4), (mw.clone(), vec![0, 1], 14));
    assert_eq!(shape(&w, &id, 5), (kw, vec![0, 1, 2, 3], 16));
    assert_eq!(shape(&w, &id, 6), (mw, vec![1, 3], 7));

    let r = String::from_utf8(pc::report(&w.root(), &w.ws, &id).expect("the report").bytes)
        .expect("UTF-8");
    for n in [1, 2, 3] {
        let sentence =
            format!("Repeats run {n}. Its result is identical, byte for byte, to run {n}'s.");
        assert!(r.contains(&sentence), "the report repeats run {n}:\n{r}");
    }
    for (line, times) in [
        ("- Column 1: outcome", 6),
        ("- Column 2: group", 2),
        ("- Column 3: group", 4),
        ("- Levels included: every level of the group, in code-point order", 4),
        ("- Levels included: indexes 1, 3, in code-point order", 2),
        (
            "- Missing-value codes: 1 (their values are in the settings fingerprint)",
            4,
        ),
        ("- Missing-value codes: 0", 2),
    ] {
        assert_eq!(r.matches(line).count(), times, "{line}:\n{r}");
    }
    // No level label and no header reaches the report (a rough check; the
    // privacy matrix checks labels with sentinels).
    for text in ["score", "arm", "site"] {
        assert!(
            !r.contains(&format!(" {text}\n")) && !r.contains(&format!(": {text}")),
            "{text} in the report"
        );
    }
}
