//! **The related rank tests in a project (Research OS, Phase 8Z-2, decision 655).**
//! Through the commands' plain bodies, as their Tauri wrappers call them, over a root and a
//! workspace session the test makes, on `project_ranks.rs`'s pattern: a Wilcoxon result
//! with its exact p, a Friedman result of three columns, and the Wilcoxon with its two
//! columns reversed, saved; the project closed and opened; each run read back, each run
//! again, the rerun `Identical`, and the report's lines: the columns compared in the
//! order chosen and the codes' count. Every file is synthetic and in a folder the test
//! creates and removes.

mod project_support;

use std::time::{Duration, Instant};

use app_lib::project_commands::{self as pc, RerunProgress, RerunView, SaveTarget};
use app_lib::runs::RunKind;
use gaply_core::contracts::RunState;
use gaply_core::research_project::RerunComparison;
use serde_json::Value;

use project_support::*;

/// Eight rows, the differences C1 - C2 all different in size and none zero, so the
/// Wilcoxon test has its exact p.
const DATA: &[u8] = b"c1,c2,c3\n\
1,1.5,2\n2,3,1\n3,4.25,7\n4,2.875,6\n5,8,3\n6,9.5,2\n7,5.5,9\n8,8.75,4\n";

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

fn number(v: &Value) -> f64 {
    v["value"].as_f64().unwrap_or_else(|| panic!("not a value: {v}"))
}

#[test]
fn related_rank_tests_are_saved_read_back_run_again_identical_and_reported_in_the_order_chosen() {
    let w = World::new("8z2-project");
    let slot = w.open("related.csv", DATA);
    let wilcoxon = w.related_ranks(slot, &[1, 2], vec![]);
    let friedman = w.related_ranks(slot, &[3, 1, 2], vec![-99.0]);
    let reversed = w.related_ranks(slot, &[2, 1], vec![]);
    for run in [wilcoxon, friedman, reversed] {
        assert_eq!(w.wait_new_kind(run), RunState::Succeeded);
    }
    let id = pc::save_run(&w.root(), &w.ws, wilcoxon, new_project("Related"))
        .expect("the Wilcoxon saves")
        .project_id;
    pc::save_run(&w.root(), &w.ws, friedman, into_open(&id)).expect("the Friedman saves");
    pc::save_run(&w.root(), &w.ws, reversed, into_open(&id)).expect("the reversed saves");
    pc::close(&w.root(), &w.ws).expect("closes");
    pc::open(&w.root(), &w.ws, &id).expect("opens");

    // Read back: each stored result is the test it was, in its columns' order.
    let first = outputs(&w, &id, 1);
    let t = &first["test"]["wilcoxon"];
    assert_eq!((t["first"].as_str(), t["second"].as_str()), (Some("C1"), Some("C2")));
    assert!(t["exactP"].is_number(), "the Wilcoxon has its exact p: {t}");
    let f = outputs(&w, &id, 2);
    let columns: Vec<&str> = f["test"]["friedman"]["columns"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["column"].as_str().unwrap())
        .collect();
    assert_eq!(columns, ["C3", "C1", "C2"], "the list's order, never the file's");
    let third = outputs(&w, &id, 3);
    let u = &third["test"]["wilcoxon"];
    assert_eq!((u["first"].as_str(), u["second"].as_str()), (Some("C2"), Some("C1")));
    // The same two columns reversed: z and r negated, p the same double, V and T- swapped.
    assert_eq!(number(&u["z"]), -number(&t["z"]), "z negated");
    assert_eq!(number(&u["rankBiserial"]), -number(&t["rankBiserial"]), "r negated");
    assert_eq!(number(&u["p"]).to_bits(), number(&t["p"]).to_bits(), "p the same double");
    assert_eq!(u["exactP"].as_f64().unwrap().to_bits(), t["exactP"].as_f64().unwrap().to_bits(), "exact p the same double");
    assert_eq!(u["v"], t["tMinus"]);
    assert_eq!(u["tMinus"], t["v"]);

    for (stored, new) in [(1, 4), (2, 5), (3, 6)] {
        let started = pc::rerun(&w.root(), &w.ws, stored).expect("the rerun starts");
        assert_eq!(started.kind, RunKind::RelatedRanks);
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
    for n in [1, 2, 3] {
        let sentence =
            format!("Repeats run {n}. Its result is identical, byte for byte, to run {n}'s.");
        assert!(r.contains(&sentence), "the report repeats run {n}:\n{r}");
    }
    for (line, times) in [
        ("- Columns compared: columns 1, 2, by position, in the order chosen", 2),
        ("- Columns compared: columns 2, 1, by position, in the order chosen", 2),
        ("- Columns compared: columns 3, 1, 2, by position, in the order chosen", 2),
        ("- Missing-value codes: 0", 4),
        (
            "- Missing-value codes: 1 (their values are in the settings fingerprint)",
            2,
        ),
    ] {
        assert_eq!(r.matches(line).count(), times, "{line}:\n{r}");
    }
    // No header reaches the report (a rough check, as project_ranks.rs's; the privacy
    // matrix checks headers with sentinels).
    for text in ["c1", "c2", "c3"] {
        assert!(
            !r.contains(&format!(" {text}\n")) && !r.contains(&format!(": {text}")),
            "{text} in the report"
        );
    }
}
